//! Progress dialog implementation.

use std::io::{BufRead, BufReader};
use std::sync::mpsc::{self, TryRecvError};
use std::thread;
use std::time::Duration;

use crate::backend::{Window, WindowEvent, create_window};
use crate::error::Error;
use crate::render::{Canvas, Font};
use crate::ui::Colors;
use crate::ui::widgets::Widget;
use crate::ui::widgets::button::Button;
use crate::ui::widgets::progress_bar::ProgressBar;

const BASE_PADDING: u32 = 20;
const BASE_BAR_WIDTH: u32 = 300;
const BASE_TEXT_HEIGHT: u32 = 20;
const BASE_BUTTON_HEIGHT: u32 = 32;

/// Progress dialog result.
#[derive(Debug, Clone)]
pub enum ProgressResult {
    /// Progress completed (reached 100% or stdin closed).
    Completed,
    /// User cancelled the dialog.
    Cancelled,
    /// Dialog was closed.
    Closed,
}

impl ProgressResult {
    pub fn exit_code(&self) -> i32 {
        match self {
            ProgressResult::Completed => 0,
            ProgressResult::Cancelled => 1,
            ProgressResult::Closed => 255,
        }
    }
}

/// Message from stdin reader thread.
enum StdinMessage {
    Progress(u32),
    Text(String),
    Pulsate,
    Done,
}

/// Progress dialog builder.
pub struct ProgressBuilder {
    title: String,
    text: String,
    percentage: u32,
    pulsate: bool,
    auto_close: bool,
    width: Option<u32>,
    height: Option<u32>,
    colors: Option<&'static Colors>,
}

impl ProgressBuilder {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            text: String::new(),
            percentage: 0,
            pulsate: false,
            auto_close: false,
            width: None,
            height: None,
            colors: None,
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

    pub fn show(self) -> Result<ProgressResult, Error> {
        let colors = self.colors.unwrap_or_else(|| crate::ui::detect_theme());

        // First pass: calculate LOGICAL dimensions using scale 1.0
        let temp_font = Font::load(1.0);
        let temp_button = Button::new("Cancel", &temp_font, 1.0);
        let temp_bar = ProgressBar::new(BASE_BAR_WIDTH, 1.0);

        let calc_width = BASE_BAR_WIDTH + BASE_PADDING * 2;
        let calc_height = BASE_PADDING * 3 + BASE_TEXT_HEIGHT + 10 + temp_bar.height() + 10 + BASE_BUTTON_HEIGHT;
        drop(temp_font);
        drop(temp_button);
        drop(temp_bar);

        // Use custom dimensions if provided, otherwise use calculated defaults
        let logical_width = self.width.unwrap_or(calc_width) as u16;
        let logical_height = self.height.unwrap_or(calc_height) as u16;

        // Create window with LOGICAL dimensions
        let mut window = create_window(logical_width, logical_height)?;
        window.set_title(if self.title.is_empty() {
            "Progress"
        } else {
            &self.title
        })?;

        // Get the actual scale factor from the window (compositor scale)
        let scale = window.scale_factor();

        // Now create everything at PHYSICAL scale
        let font = Font::load(scale);
        let mut cancel_button = Button::new("Cancel", &font, scale);

        // Scale dimensions for physical rendering
        let padding = (BASE_PADDING as f32 * scale) as u32;
        let bar_width = (BASE_BAR_WIDTH as f32 * scale) as u32;
        let text_height = (BASE_TEXT_HEIGHT as f32 * scale) as u32;
        let button_height = (BASE_BUTTON_HEIGHT as f32 * scale) as u32;

        // Calculate physical dimensions
        let physical_width = (logical_width as f32 * scale) as u32;
        let physical_height = (logical_height as f32 * scale) as u32;

        // Create progress bar at physical scale
        let mut progress_bar = ProgressBar::new(bar_width, scale);
        progress_bar.set_percentage(self.percentage);
        if self.pulsate {
            progress_bar.set_pulsating(true);
        }

        // Current status text
        let mut status_text = self.text.clone();

        // Position elements in physical coordinates
        let text_y = padding as i32;
        let bar_y = text_y + text_height as i32 + 10;
        progress_bar.set_position(padding as i32, bar_y);

        let button_y = bar_y + progress_bar.height() as i32 + (10.0 * scale) as i32;
        let button_x = physical_width as i32 - padding as i32 - cancel_button.width() as i32;
        cancel_button.set_position(button_x, button_y);

        // Create canvas at PHYSICAL dimensions
        let mut canvas = Canvas::new(physical_width, physical_height);

        // Start stdin reader thread
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

                if trimmed.starts_with('#') {
                    // Status text update
                    let text = trimmed[1..].trim().to_string();
                    if tx.send(StdinMessage::Text(text)).is_err() {
                        break;
                    }
                } else if trimmed.eq_ignore_ascii_case("pulsate") {
                    if tx.send(StdinMessage::Pulsate).is_err() {
                        break;
                    }
                } else if let Ok(num) = trimmed.parse::<u32>() {
                    if tx.send(StdinMessage::Progress(num.min(100))).is_err() {
                        break;
                    }
                }
            }

            let _ = tx.send(StdinMessage::Done);
        });

        // Draw function
        let draw = |canvas: &mut Canvas,
                    colors: &Colors,
                    font: &Font,
                    status_text: &str,
                    progress_bar: &ProgressBar,
                    cancel_button: &Button,
                    padding: u32,
                    text_y: i32| {
            canvas.fill(colors.window_bg);

            // Draw status text
            if !status_text.is_empty() {
                let text_canvas = font.render(status_text).with_color(colors.text).finish();
                canvas.draw_canvas(&text_canvas, padding as i32, text_y);
            }

            // Draw progress bar
            progress_bar.draw(canvas, colors);

            // Draw cancel button
            cancel_button.draw_to(canvas, colors, font);
        };

        // Initial draw
        draw(
            &mut canvas,
            colors,
            &font,
            &status_text,
            &progress_bar,
            &cancel_button,
            padding,
            text_y,
        );
        window.set_contents(&canvas)?;
        window.show()?;

        let mut stdin_done = false;
        let auto_close = self.auto_close;

        // Event loop with timeout for animation
        loop {
            // Check for stdin messages
            loop {
                match rx.try_recv() {
                    Ok(StdinMessage::Progress(p)) => {
                        progress_bar.set_percentage(p);
                        if p >= 100 && auto_close {
                            return Ok(ProgressResult::Completed);
                        }
                    }
                    Ok(StdinMessage::Text(t)) => {
                        status_text = t;
                    }
                    Ok(StdinMessage::Pulsate) => {
                        progress_bar.set_pulsating(true);
                    }
                    Ok(StdinMessage::Done) => {
                        stdin_done = true;
                        if auto_close {
                            return Ok(ProgressResult::Completed);
                        }
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        stdin_done = true;
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
                            &progress_bar,
                            &cancel_button,
                            padding,
                            text_y,
                        );
                        window.set_contents(&canvas)?;
                        std::thread::sleep(Duration::from_millis(16));
                        continue;
                    }
                }
            } else {
                // Block waiting for events if not pulsating
                if stdin_done {
                    Some(window.wait_for_event()?)
                } else {
                    // Poll with short sleep to check stdin
                    match window.poll_for_event()? {
                        Some(e) => Some(e),
                        None => {
                            std::thread::sleep(Duration::from_millis(50));
                            continue;
                        }
                    }
                }
            };

            if let Some(event) = event {
                match &event {
                    WindowEvent::CloseRequested => {
                        return Ok(ProgressResult::Closed);
                    }
                    WindowEvent::RedrawRequested => {
                        draw(
                            &mut canvas,
                            colors,
                            &font,
                            &status_text,
                            &progress_bar,
                            &cancel_button,
                            padding,
                            text_y,
                        );
                        window.set_contents(&canvas)?;
                    }
                    _ => {}
                }

                // Process button events
                cancel_button.process_event(&event);

                if cancel_button.was_clicked() {
                    return Ok(ProgressResult::Cancelled);
                }
            }

            // Redraw if needed
            draw(
                &mut canvas,
                colors,
                &font,
                &status_text,
                &progress_bar,
                &cancel_button,
                padding,
                text_y,
            );
            window.set_contents(&canvas)?;
        }
    }
}

impl Default for ProgressBuilder {
    fn default() -> Self {
        Self::new()
    }
}
