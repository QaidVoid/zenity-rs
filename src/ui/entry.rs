//! Entry dialog implementation for text input.

use crate::{
    backend::{CursorShape, Window, WindowEvent, create_window},
    error::Error,
    render::{Canvas, Font},
    ui::{
        Colors,
        widgets::{Widget, button::Button, text_input::TextInput},
    },
};

const BASE_PADDING: u32 = 20;
const BASE_BUTTON_SPACING: u32 = 10;
const BASE_INPUT_WIDTH: u32 = 300;

/// Entry dialog result.
#[derive(Debug, Clone)]
pub enum EntryResult {
    /// User entered text and clicked OK.
    Text(String),
    /// User cancelled the dialog.
    Cancelled,
    /// Dialog was closed.
    Closed,
}

impl EntryResult {
    pub fn exit_code(&self) -> i32 {
        match self {
            EntryResult::Text(_) => 0,
            EntryResult::Cancelled => 1,
            EntryResult::Closed => 1,
        }
    }
}

/// Entry dialog builder.
pub struct EntryBuilder {
    title: String,
    text: String,
    entry_text: String,
    hide_text: bool,
    width: Option<u32>,
    height: Option<u32>,
    colors: Option<&'static Colors>,
}

impl EntryBuilder {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            text: String::new(),
            entry_text: String::new(),
            hide_text: false,
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

    pub fn entry_text(mut self, entry_text: &str) -> Self {
        self.entry_text = entry_text.to_string();
        self
    }

    pub fn hide_text(mut self, hide: bool) -> Self {
        self.hide_text = hide;
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

    pub fn show(self) -> Result<EntryResult, Error> {
        let colors = self.colors.unwrap_or_else(|| crate::ui::detect_theme());

        // First pass: calculate LOGICAL dimensions using scale 1.0
        let temp_font = Font::load(1.0);
        let temp_ok = Button::new("OK", &temp_font, 1.0);
        let temp_cancel = Button::new("Cancel", &temp_font, 1.0);
        let temp_prompt_height = if !self.text.is_empty() {
            temp_font
                .render(&self.text)
                .with_max_width(BASE_INPUT_WIDTH as f32)
                .finish()
                .height()
        } else {
            0
        };
        let temp_input = TextInput::new(BASE_INPUT_WIDTH);

        let logical_buttons_width = temp_ok.width() + temp_cancel.width() + BASE_BUTTON_SPACING;
        let logical_content_width = BASE_INPUT_WIDTH.max(logical_buttons_width);
        let calc_width = logical_content_width + BASE_PADDING * 2;
        let calc_height = BASE_PADDING * 3
            + temp_prompt_height
            + (if temp_prompt_height > 0 { 10 } else { 0 })
            + temp_input.height()
            + 10
            + 32;

        drop(temp_font);
        drop(temp_ok);
        drop(temp_cancel);
        drop(temp_input);

        // Use custom dimensions if provided, otherwise use calculated defaults
        let logical_width = self.width.unwrap_or(calc_width) as u16;
        let logical_height = self.height.unwrap_or(calc_height) as u16;

        // Create window with LOGICAL dimensions
        let mut window = create_window(logical_width, logical_height)?;
        window.set_title(if self.title.is_empty() {
            "Entry"
        } else {
            &self.title
        })?;

        // Get the actual scale factor from the window (compositor scale)
        let scale = window.scale_factor();

        // Calculate physical dimensions from logical dimensions
        let physical_width = (logical_width as f32 * scale) as u32;
        let physical_height = (logical_height as f32 * scale) as u32;

        // Now create everything at PHYSICAL scale
        let font = Font::load(scale);

        // Scale dimensions for physical rendering
        let padding = (BASE_PADDING as f32 * scale) as u32;
        let button_spacing = (BASE_BUTTON_SPACING as f32 * scale) as u32;

        // Input should fill available width
        let input_width = physical_width - (padding * 2);

        // Create buttons at physical scale
        let mut ok_button = Button::new("OK", &font, scale);
        let mut cancel_button = Button::new("Cancel", &font, scale);

        // Create text input at physical scale
        let mut input = TextInput::new(input_width)
            .with_password(self.hide_text)
            .with_default_text(&self.entry_text);
        input.set_focus(true);

        // Render prompt text at physical scale (wrapped to fit)
        let prompt_canvas = if !self.text.is_empty() {
            Some(
                font.render(&self.text)
                    .with_color(colors.text)
                    .with_max_width((physical_width - padding * 2) as f32)
                    .finish(),
            )
        } else {
            None
        };
        let prompt_height = prompt_canvas.as_ref().map(|c| c.height()).unwrap_or(0);

        // Position elements in physical coordinates
        let mut y = padding as i32;
        let prompt_y = y;
        if prompt_height > 0 {
            y += prompt_height as i32 + (10.0 * scale) as i32;
        }

        // Input position
        input.set_position(padding as i32, y);
        y += input.height() as i32 + (10.0 * scale) as i32;

        // Button positions (right-aligned)
        let mut button_x = physical_width as i32 - padding as i32;
        button_x -= cancel_button.width() as i32;
        cancel_button.set_position(button_x, y);
        button_x -= button_spacing as i32 + ok_button.width() as i32;
        ok_button.set_position(button_x, y);

        // Create canvas at PHYSICAL dimensions
        let mut canvas = Canvas::new(physical_width, physical_height);

        // Draw function
        let draw = |canvas: &mut Canvas,
                    colors: &Colors,
                    font: &Font,
                    prompt_canvas: &Option<Canvas>,
                    input: &TextInput,
                    ok_button: &Button,
                    cancel_button: &Button,
                    padding: u32,
                    prompt_y: i32,
                    scale: f32| {
            let width = canvas.width() as f32;
            let height = canvas.height() as f32;
            let radius = 8.0 * scale;

            canvas.fill_dialog_bg(
                width,
                height,
                colors.window_bg,
                colors.window_border,
                colors.window_shadow,
                radius,
            );

            // Draw prompt
            if let Some(prompt) = prompt_canvas {
                canvas.draw_canvas(prompt, padding as i32, prompt_y);
            }

            // Draw input
            input.draw_to(canvas, colors, font);

            // Draw buttons
            ok_button.draw_to(canvas, colors, font);
            cancel_button.draw_to(canvas, colors, font);
        };

        // Initial draw
        draw(
            &mut canvas,
            colors,
            &font,
            &prompt_canvas,
            &input,
            &ok_button,
            &cancel_button,
            padding,
            prompt_y,
            scale,
        );
        window.set_contents(&canvas)?;
        window.show()?;

        // Event loop
        let mut window_dragging = false;
        loop {
            let event = window.wait_for_event()?;

            match &event {
                WindowEvent::CloseRequested => {
                    return Ok(EntryResult::Closed);
                }
                WindowEvent::RedrawRequested => {
                    draw(
                        &mut canvas,
                        colors,
                        &font,
                        &prompt_canvas,
                        &input,
                        &ok_button,
                        &cancel_button,
                        padding,
                        prompt_y,
                        scale,
                    );
                    window.set_contents(&canvas)?;
                }
                WindowEvent::CursorMove(pos) => {
                    if window_dragging {
                        let _ = window.start_drag();
                        window_dragging = false;
                    }

                    let cursor_x = pos.x as i32;
                    let cursor_y = pos.y as i32;

                    // Check if cursor is over the input field
                    let ix = input.x();
                    let iy = input.y();
                    let iw = input.width();
                    let ih = input.height();

                    let over_input = cursor_x >= ix
                        && cursor_x < ix + iw as i32
                        && cursor_y >= iy
                        && cursor_y < iy + ih as i32;

                    let _ = window.set_cursor(if over_input {
                        CursorShape::Text
                    } else {
                        CursorShape::Default
                    });
                }
                WindowEvent::ButtonPress(crate::backend::MouseButton::Left, _) => {
                    window_dragging = true;
                }
                WindowEvent::ButtonRelease(crate::backend::MouseButton::Left, _) => {
                    window_dragging = false;
                }
                _ => {}
            }

            // Process input events
            let mut needs_redraw = input.process_event(&event);

            // Check for Enter key submission
            if input.was_submitted() {
                return Ok(EntryResult::Text(input.text().to_string()));
            }

            // Process button events
            if ok_button.process_event(&event) {
                needs_redraw = true;
            }
            if cancel_button.process_event(&event) {
                needs_redraw = true;
            }

            if ok_button.was_clicked() {
                return Ok(EntryResult::Text(input.text().to_string()));
            }
            if cancel_button.was_clicked() {
                return Ok(EntryResult::Cancelled);
            }

            // Batch process pending events
            while let Some(event) = window.poll_for_event()? {
                match &event {
                    WindowEvent::CloseRequested => {
                        return Ok(EntryResult::Closed);
                    }
                    _ => {
                        if input.process_event(&event) {
                            needs_redraw = true;
                        }
                        if input.was_submitted() {
                            return Ok(EntryResult::Text(input.text().to_string()));
                        }
                        if ok_button.process_event(&event) {
                            needs_redraw = true;
                        }
                        if cancel_button.process_event(&event) {
                            needs_redraw = true;
                        }
                        if ok_button.was_clicked() {
                            return Ok(EntryResult::Text(input.text().to_string()));
                        }
                        if cancel_button.was_clicked() {
                            return Ok(EntryResult::Cancelled);
                        }
                    }
                }
            }

            if needs_redraw {
                draw(
                    &mut canvas,
                    colors,
                    &font,
                    &prompt_canvas,
                    &input,
                    &ok_button,
                    &cancel_button,
                    padding,
                    prompt_y,
                    scale,
                );
                window.set_contents(&canvas)?;
            }
        }
    }
}

impl Default for EntryBuilder {
    fn default() -> Self {
        Self::new()
    }
}
