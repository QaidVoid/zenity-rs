//! Progress dialog implementation.

use std::{
    io::{BufRead, BufReader},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, TryRecvError},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[cfg(unix)]
use libc::{SIGTERM, getppid, kill};

use crate::{
    backend::{AnyWindow, Window, WindowEvent},
    error::Error,
    render::{Canvas, Font},
    ui::{
        BASE_BUTTON_HEIGHT, BASE_BUTTON_SPACING, BASE_CORNER_RADIUS, Colors, ellipsize,
        open_window,
        widgets::{Widget, button::Button, point_in_widget, progress_bar::ProgressBar},
    },
};

const BASE_PADDING: u32 = 20;
const BASE_BAR_WIDTH: u32 = 300;
const BASE_TEXT_HEIGHT: u32 = 20;

/// Progress dialog result.
#[derive(Debug, Clone)]
pub enum ProgressResult {
    /// Progress completed (reached 100% or stdin closed).
    Completed,
    /// User cancelled the dialog.
    Cancelled,
    /// Dialog was closed.
    Closed,
    /// The --timeout elapsed before the dialog finished.
    Timeout,
}

impl ProgressResult {
    pub fn exit_code(&self) -> i32 {
        match self {
            ProgressResult::Timeout => 5,
            ProgressResult::Completed => 0,
            ProgressResult::Cancelled => 1,
            ProgressResult::Closed => 1,
        }
    }
}

/// Update sent to a running progress dialog.
enum ProgressMessage {
    Progress(u32),
    Text(String),
    Pulsate(bool),
    Done,
}

/// Reads the zenity progress protocol from stdin on a background thread.
fn spawn_stdin_reader() -> mpsc::Receiver<ProgressMessage> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let stdin = std::io::stdin();
        let reader = BufReader::new(stdin.lock());

        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => break,
            };

            let trimmed = line.trim();

            if let Some(text) = trimmed.strip_prefix('#') {
                // Status text update
                let text = text.trim().to_string();
                if tx.send(ProgressMessage::Text(text)).is_err() {
                    break;
                }
            } else if trimmed.eq_ignore_ascii_case("pulsate") {
                if tx.send(ProgressMessage::Pulsate(true)).is_err() {
                    break;
                }
            } else if let Ok(num) = trimmed.parse::<u32>()
                && tx.send(ProgressMessage::Progress(num.min(100))).is_err()
            {
                break;
            }
        }

        let _ = tx.send(ProgressMessage::Done);
    });

    rx
}

/// State shared between a running dialog and the objects driving it.
///
/// The close request is a flag rather than a [`ProgressMessage`] so that
/// [`ProgressDialog`] does not have to hold a [`mpsc::Sender`]. Holding one
/// would keep the channel alive and stop [`ProgressHandle::finish`] from ever
/// closing an [`ProgressBuilder::auto_close`] dialog.
#[derive(Default)]
struct DialogState {
    cancelled: AtomicBool,
    close_requested: AtomicBool,
}

/// Handle for updating a progress dialog started with
/// [`ProgressBuilder::spawn`].
///
/// Clones share one dialog, so work split across threads can report through
/// its own handle. The handle is [`Send`] but not [`Sync`]: give each thread a
/// clone rather than sharing one behind an [`Arc`].
#[derive(Clone)]
pub struct ProgressHandle {
    tx: mpsc::Sender<ProgressMessage>,
    state: Arc<DialogState>,
}

impl ProgressHandle {
    /// Sets the progress bar to `percentage`, clamped to 100.
    pub fn set_percentage(&self, percentage: u32) {
        let _ = self.tx.send(ProgressMessage::Progress(percentage.min(100)));
    }

    /// Replaces the status text shown above the progress bar.
    pub fn set_text(&self, text: &str) {
        let _ = self.tx.send(ProgressMessage::Text(text.to_string()));
    }

    /// Switches the progress bar between its indeterminate animation and the
    /// percentage set by [`ProgressHandle::set_percentage`].
    ///
    /// Pulsating suits phases whose length is unknown, such as scanning a file
    /// before a transfer starts.
    pub fn set_pulsating(&self, pulsating: bool) {
        let _ = self.tx.send(ProgressMessage::Pulsate(pulsating));
    }

    /// Reports whether the dialog stopped before completing, either because the
    /// user cancelled or closed it or because it failed.
    ///
    /// Long-running callers should poll this and abort their work when it turns
    /// true.
    pub fn is_cancelled(&self) -> bool {
        self.state.cancelled.load(Ordering::Acquire)
    }

    /// Signals that this handle has no more updates.
    ///
    /// Once the last handle is gone, a dialog built with
    /// [`ProgressBuilder::auto_close`] closes. Use [`ProgressHandle::close`]
    /// to dismiss a dialog that does not close itself.
    pub fn finish(self) {}

    /// Dismisses the dialog, whether or not it auto-closes.
    ///
    /// [`ProgressDialog::join`] then reports [`ProgressResult::Completed`].
    pub fn close(self) {
        self.state.close_requested.store(true, Ordering::Release);
    }
}

/// A progress dialog running on a background thread.
///
/// Dropping this dismisses the dialog and waits for its thread, so keep it
/// alive for as long as the dialog should stay on screen.
#[must_use = "dropping a ProgressDialog closes the dialog immediately"]
pub struct ProgressDialog {
    thread: Option<JoinHandle<Result<ProgressResult, Error>>>,
    state: Arc<DialogState>,
}

impl ProgressDialog {
    /// Waits for the dialog to close and returns its result.
    pub fn join(mut self) -> Result<ProgressResult, Error> {
        let thread = self
            .thread
            .take()
            .expect("dialog thread is taken by join or drop, never both");
        match thread.join() {
            Ok(result) => result,
            Err(panic) => std::panic::resume_unwind(panic),
        }
    }
}

impl Drop for ProgressDialog {
    fn drop(&mut self) {
        if let Some(thread) = self.thread.take() {
            self.state.close_requested.store(true, Ordering::Release);
            let _ = thread.join();
        }
    }
}

/// Progress dialog builder.
pub struct ProgressBuilder {
    title: String,
    text: String,
    percentage: u32,
    pulsate: bool,
    auto_close: bool,
    auto_kill: bool,
    no_cancel: bool,
    show_time_remaining: bool,
    width: Option<u32>,
    height: Option<u32>,
    colors: Option<&'static Colors>,
    timeout: Option<u32>,
}

impl ProgressBuilder {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            text: String::new(),
            percentage: 0,
            pulsate: false,
            auto_close: false,
            auto_kill: false,
            no_cancel: false,
            show_time_remaining: false,
            width: None,
            height: None,
            colors: None,
            timeout: None,
        }
    }

    pub fn title(mut self, title: &str) -> Self {
        self.title = title.to_string();
        self
    }

    pub fn text(mut self, text: &str) -> Self {
        self.text = text.to_string();
        self
    }

    pub fn percentage(mut self, percentage: u32) -> Self {
        self.percentage = percentage.min(100);
        self
    }

    pub fn pulsate(mut self, pulsate: bool) -> Self {
        self.pulsate = pulsate;
        self
    }

    pub fn auto_close(mut self, auto_close: bool) -> Self {
        self.auto_close = auto_close;
        self
    }

    pub fn auto_kill(mut self, auto_kill: bool) -> Self {
        self.auto_kill = auto_kill;
        self
    }

    /// Close the dialog on its own after `seconds`, reporting a timeout.
    pub fn timeout(mut self, seconds: u32) -> Self {
        self.timeout = Some(seconds);
        self
    }

    pub fn colors(mut self, colors: &'static Colors) -> Self {
        self.colors = Some(colors);
        self
    }

    pub fn width(mut self, width: u32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn height(mut self, height: u32) -> Self {
        self.height = Some(height);
        self
    }

    pub fn no_cancel(mut self, no_cancel: bool) -> Self {
        self.no_cancel = no_cancel;
        self
    }

    pub fn time_remaining(mut self, show_time_remaining: bool) -> Self {
        self.show_time_remaining = show_time_remaining;
        self
    }

    /// Creates the dialog window, returning it with its scale factor and
    /// physical size.
    fn open(&self) -> Result<(AnyWindow, f32, u32, u32), Error> {
        // First pass: calculate LOGICAL dimensions using scale 1.0
        let temp_font = Font::load(1.0);
        let temp_button = Button::new("Cancel", &temp_font, 1.0);
        let temp_bar = ProgressBar::new(BASE_BAR_WIDTH, 1.0);

        let calc_width = BASE_BAR_WIDTH + BASE_PADDING * 2;
        let time_remaining_height = if self.show_time_remaining { 24 } else { 0 };
        let calc_height = BASE_PADDING * 3
            + BASE_TEXT_HEIGHT
            + time_remaining_height
            + 10
            + temp_bar.height()
            + 10
            + BASE_BUTTON_HEIGHT;
        drop(temp_font);
        drop(temp_button);

        // Use custom dimensions if provided, otherwise use calculated defaults
        let logical_width = self.width.unwrap_or(calc_width) as u16;
        let logical_height = self.height.unwrap_or(calc_height) as u16;

        // Create window with LOGICAL dimensions
        open_window(
            &self.title,
            "Progress",
            logical_width as u32,
            logical_height as u32,
        )
    }

    /// Displays the dialog, reading progress updates from stdin.
    ///
    /// Blocks until the dialog closes. Use [`ProgressBuilder::spawn`] to drive
    /// the dialog from your own code instead of from stdin.
    pub fn show(self) -> Result<ProgressResult, Error> {
        let (window, scale, physical_width, physical_height) = self.open()?;
        self.run(
            window,
            scale,
            physical_width,
            physical_height,
            spawn_stdin_reader(),
            Arc::new(DialogState::default()),
        )
    }

    /// Opens the dialog on a background thread and returns a handle for
    /// updating it.
    ///
    /// Unlike [`ProgressBuilder::show`], this does not block and does not read
    /// stdin. The caller drives the dialog through the returned
    /// [`ProgressHandle`] and waits for it to close with
    /// [`ProgressDialog::join`].
    ///
    /// # Example
    ///
    /// ```no_run
    /// let (handle, dialog) = zenity_rs::progress()
    ///     .title("Copying")
    ///     .auto_close(true)
    ///     .spawn()
    ///     .unwrap();
    ///
    /// for percentage in 0..=100 {
    ///     if handle.is_cancelled() {
    ///         break;
    ///     }
    ///     handle.set_percentage(percentage);
    /// }
    ///
    /// handle.finish();
    /// dialog.join().unwrap();
    /// ```
    pub fn spawn(mut self) -> Result<(ProgressHandle, ProgressDialog), Error> {
        // auto_kill signals the process feeding progress over stdin. Driven
        // in-process there is no such process, and getppid() would be whatever
        // launched the caller: their shell, terminal or supervisor.
        self.auto_kill = false;

        let (tx, rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        let state = Arc::new(DialogState::default());

        let thread_state = Arc::clone(&state);
        let thread = thread::spawn(move || {
            let (window, scale, physical_width, physical_height) = match self.open() {
                Ok(opened) => {
                    let _ = ready_tx.send(());
                    opened
                }
                Err(e) => return Err(e),
            };

            let result = self.run(
                window,
                scale,
                physical_width,
                physical_height,
                rx,
                Arc::clone(&thread_state),
            );
            if !matches!(result, Ok(ProgressResult::Completed)) {
                thread_state.cancelled.store(true, Ordering::Release);
            }
            result
        });

        // The window failed to open, so the thread already returned its error.
        if ready_rx.recv().is_err() {
            return match thread.join() {
                Ok(Err(e)) => Err(e),
                Ok(Ok(_)) => Err(Error::NoDisplay),
                Err(panic) => std::panic::resume_unwind(panic),
            };
        }

        Ok((
            ProgressHandle {
                tx,
                state: Arc::clone(&state),
            },
            ProgressDialog {
                thread: Some(thread),
                state,
            },
        ))
    }

    fn run(
        self,
        mut window: AnyWindow,
        scale: f32,
        physical_width: u32,
        physical_height: u32,
        rx: mpsc::Receiver<ProgressMessage>,
        state: Arc<DialogState>,
    ) -> Result<ProgressResult, Error> {
        let colors = self.colors.unwrap_or_else(|| crate::ui::detect_theme());

        // Now create everything at PHYSICAL scale
        let font = Font::load(scale);
        let mut cancel_button = if self.no_cancel {
            None
        } else {
            Some(Button::new("Cancel", &font, scale))
        };

        // Scale dimensions for physical rendering
        let padding = (BASE_PADDING as f32 * scale) as u32;
        let bar_width = physical_width.saturating_sub(padding * 2);
        let text_height = (BASE_TEXT_HEIGHT as f32 * scale) as u32;

        // Create progress bar at physical scale
        let mut progress_bar = ProgressBar::new(bar_width, scale);
        progress_bar.set_percentage(self.percentage);
        if self.pulsate {
            progress_bar.set_pulsating(true);
        }

        // Current status text
        let mut status_text = self.text.clone();

        // Time remaining calculation
        let start_time = std::time::Instant::now();
        let mut time_remaining_text = String::new();

        // Position elements in physical coordinates
        let text_y = padding as i32;
        let time_remaining_offset = if self.show_time_remaining { 24 } else { 0 };
        let bar_y = text_y + text_height as i32 + 10 + time_remaining_offset;
        progress_bar.set_position(padding as i32, bar_y);

        let button_y =
            bar_y + progress_bar.height() as i32 + (BASE_BUTTON_SPACING as f32 * scale) as i32;
        if let Some(ref mut cancel_button) = cancel_button {
            let button_x = physical_width as i32 - padding as i32 - cancel_button.width() as i32;
            cancel_button.set_position(button_x, button_y);
        }

        // Create canvas at PHYSICAL dimensions
        let mut canvas = Canvas::new(physical_width, physical_height);

        // Draw function
        let draw = |canvas: &mut Canvas,
                    colors: &Colors,
                    font: &Font,
                    status_text: &str,
                    time_remaining_text: &str,
                    progress_bar: &ProgressBar,
                    cancel_button: &Option<Button>,
                    padding: u32,
                    text_y: i32,
                    show_time_remaining: bool,
                    scale: f32| {
            let width = canvas.width() as f32;
            let height = canvas.height() as f32;
            let radius = BASE_CORNER_RADIUS * scale;

            canvas.fill_dialog_bg(
                width,
                height,
                colors.window_bg,
                colors.window_border,
                colors.window_shadow,
                radius,
            );

            // Draw status text
            if !status_text.is_empty() {
                let max_w = width - (padding * 2) as f32;
                let label = ellipsize(status_text, font, max_w);
                let text_canvas = font.render(&label).with_color(colors.text).finish();
                canvas.draw_canvas(&text_canvas, padding as i32, text_y);
            }

            // Draw time remaining text
            if show_time_remaining && !time_remaining_text.is_empty() {
                let text_canvas = font
                    .render(time_remaining_text)
                    .with_color(colors.text)
                    .finish();
                let time_remaining_y = if !status_text.is_empty() {
                    text_y + 24
                } else {
                    text_y
                };
                canvas.draw_canvas(&text_canvas, padding as i32, time_remaining_y);
            }

            // Draw progress bar
            progress_bar.draw(canvas, colors);

            // Draw cancel button
            if let Some(button) = cancel_button {
                button.draw_to(canvas, colors, font);
            }
        };

        let format_time_remaining = |seconds: f64| -> String {
            if seconds < 60.0 {
                format!("{:.0}s remaining", seconds)
            } else if seconds < 3600.0 {
                let mins = (seconds / 60.0).floor();
                let secs = seconds % 60.0;
                format!("{:.0}m {:.0}s remaining", mins, secs)
            } else {
                let hours = (seconds / 3600.0).floor();
                let mins = ((seconds % 3600.0) / 60.0).floor();
                let secs = seconds % 60.0;
                format!("{:.0}h {:.0}m {:.0}s remaining", hours, mins, secs)
            }
        };

        // Initial draw
        draw(
            &mut canvas,
            colors,
            &font,
            &status_text,
            &time_remaining_text,
            &progress_bar,
            &cancel_button,
            padding,
            text_y,
            self.show_time_remaining,
            scale,
        );
        window.set_contents(&canvas)?;
        window.show()?;

        let auto_close = self.auto_close;

        // Event loop with timeout for animation
        let mut window_dragging = false;
        let mut cursor_x = 0i32;
        let mut cursor_y = 0i32;
        let mut input_done = false;
        let deadline = self
            .timeout
            .map(|secs| Instant::now() + Duration::from_secs(secs as u64));

        loop {
            if state.close_requested.load(Ordering::Acquire) {
                return Ok(ProgressResult::Completed);
            }

            if let Some(deadline) = deadline
                && Instant::now() >= deadline
            {
                return Ok(ProgressResult::Timeout);
            }

            let mut needs_redraw = false;

            while !input_done {
                match rx.try_recv() {
                    Ok(ProgressMessage::Progress(p)) => {
                        progress_bar.set_percentage(p);
                        if self.show_time_remaining && !self.pulsate && p > 0 {
                            let elapsed = start_time.elapsed().as_secs_f64();
                            let progress_fraction = p as f64 / 100.0;
                            let estimated_total = elapsed / progress_fraction;
                            let remaining = (estimated_total - elapsed).max(0.0);
                            time_remaining_text = format_time_remaining(remaining);
                        }
                        needs_redraw = true;
                        if p >= 100 && auto_close {
                            return Ok(ProgressResult::Completed);
                        }
                    }
                    Ok(ProgressMessage::Text(t)) => {
                        status_text = t;
                        needs_redraw = true;
                    }
                    Ok(ProgressMessage::Pulsate(pulsating)) => {
                        progress_bar.set_pulsating(pulsating);
                        needs_redraw = true;
                    }
                    Ok(ProgressMessage::Done) => {
                        needs_redraw = true;
                        if auto_close {
                            return Ok(ProgressResult::Completed);
                        }
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        input_done = true;
                        if auto_close {
                            return Ok(ProgressResult::Completed);
                        }
                        break;
                    }
                }
            }

            // Poll for window events (non-blocking if pulsating)
            let event = if progress_bar.is_pulsating() {
                // Use short timeout for animation
                match window.poll_for_event()? {
                    Some(e) => Some(e),
                    None => {
                        // Tick animation and redraw
                        progress_bar.tick();
                        draw(
                            &mut canvas,
                            colors,
                            &font,
                            &status_text,
                            &time_remaining_text,
                            &progress_bar,
                            &cancel_button,
                            padding,
                            text_y,
                            self.show_time_remaining,
                            scale,
                        );
                        window.set_contents(&canvas)?;
                        std::thread::sleep(Duration::from_millis(16));
                        continue;
                    }
                }
            } else {
                // Poll with short sleep to check stdin
                window.poll_for_event()?
            };

            if let Some(event) = event {
                match &event {
                    WindowEvent::CloseRequested => {
                        return Ok(ProgressResult::Closed);
                    }
                    WindowEvent::RedrawRequested => {
                        needs_redraw = true;
                    }
                    WindowEvent::CursorMove(pos) => {
                        cursor_x = pos.x as i32;
                        cursor_y = pos.y as i32;
                        if window_dragging {
                            let _ = window.start_drag();
                            window_dragging = false;
                        }
                    }
                    WindowEvent::ButtonPress(crate::backend::MouseButton::Left, _) => {
                        window_dragging = !cancel_button
                            .as_ref()
                            .is_some_and(|b| point_in_widget(cursor_x, cursor_y, b));
                    }
                    WindowEvent::ButtonRelease(crate::backend::MouseButton::Left, _) => {
                        window_dragging = false;
                    }
                    _ => {}
                }

                // Process button events
                if let Some(ref mut cancel_button) = cancel_button {
                    cancel_button.process_event(&event);

                    if cancel_button.was_clicked() {
                        if self.auto_kill {
                            #[cfg(unix)]
                            unsafe {
                                kill(getppid(), SIGTERM);
                            }
                        }
                        return Ok(ProgressResult::Cancelled);
                    }
                }
            }

            // Redraw if needed (this ensures progress updates even when not focused)
            if needs_redraw {
                draw(
                    &mut canvas,
                    colors,
                    &font,
                    &status_text,
                    &time_remaining_text,
                    &progress_bar,
                    &cancel_button,
                    padding,
                    text_y,
                    self.show_time_remaining,
                    scale,
                );
                window.set_contents(&canvas)?;
            }

            // Short sleep to prevent CPU spinning when idle
            if !needs_redraw && !progress_bar.is_pulsating() {
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

impl Default for ProgressBuilder {
    fn default() -> Self {
        Self::new()
    }
}
