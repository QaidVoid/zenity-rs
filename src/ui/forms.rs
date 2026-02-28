//! Forms dialog implementation for multiple input fields.

use crate::{
    backend::{CursorShape, Window, WindowEvent, create_window},
    error::Error,
    render::{Canvas, Font},
    ui::{
        Colors, KEY_ESCAPE, KEY_ISO_LEFT_TAB, KEY_RETURN, KEY_TAB,
        widgets::{Widget, button::Button, text_input::TextInput},
    },
};

const BASE_PADDING: u32 = 20;
const BASE_FIELD_HEIGHT: u32 = 32;
const BASE_FIELD_SPACING: u32 = 12;
const BASE_LABEL_WIDTH: u32 = 120;
const BASE_INPUT_WIDTH: u32 = 250;
const BASE_MIN_WIDTH: u32 = 420;

/// Field type for forms.
#[derive(Debug, Clone)]
pub enum FormField {
    /// Text entry field.
    Entry(String),
    /// Password field (hidden text).
    Password(String),
}

impl FormField {
    pub fn label(&self) -> &str {
        match self {
            FormField::Entry(label) => label,
            FormField::Password(label) => label,
        }
    }

    pub fn is_password(&self) -> bool {
        matches!(self, FormField::Password(_))
    }
}

/// Forms dialog result.
#[derive(Debug, Clone)]
pub enum FormsResult {
    /// User entered values and clicked OK.
    Values(Vec<String>),
    /// User cancelled the dialog.
    Cancelled,
    /// Dialog was closed.
    Closed,
}

impl FormsResult {
    pub fn exit_code(&self) -> i32 {
        match self {
            FormsResult::Values(_) => 0,
            FormsResult::Cancelled => 1,
            FormsResult::Closed => 1,
        }
    }
}

/// Forms dialog builder.
pub struct FormsBuilder {
    title: String,
    text: String,
    fields: Vec<FormField>,
    separator: String,
    width: Option<u32>,
    height: Option<u32>,
    colors: Option<&'static Colors>,
}

impl FormsBuilder {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            text: String::new(),
            fields: Vec::new(),
            separator: "|".to_string(),
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

    /// Add a text entry field.
    pub fn add_entry(mut self, label: &str) -> Self {
        self.fields.push(FormField::Entry(label.to_string()));
        self
    }

    /// Add a password field.
    pub fn add_password(mut self, label: &str) -> Self {
        self.fields.push(FormField::Password(label.to_string()));
        self
    }

    /// Set the output separator (default: "|").
    pub fn separator(mut self, sep: &str) -> Self {
        self.separator = sep.to_string();
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

    pub fn show(self) -> Result<FormsResult, Error> {
        if self.fields.is_empty() {
            return Ok(FormsResult::Values(Vec::new()));
        }

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

        let logical_buttons_width = temp_ok.width() + temp_cancel.width() + 10;
        let logical_content_width =
            (BASE_LABEL_WIDTH + BASE_INPUT_WIDTH + 10).max(logical_buttons_width);
        let calc_width = (logical_content_width + BASE_PADDING * 2).max(BASE_MIN_WIDTH);

        // Height: padding + text + fields + buttons + padding
        let fields_height = self.fields.len() as u32 * (BASE_FIELD_HEIGHT + BASE_FIELD_SPACING);
        let calc_height = BASE_PADDING * 2
            + temp_prompt_height
            + (if temp_prompt_height > 0 { 16 } else { 0 })
            + fields_height
            + 16
            + 32; // Button area

        drop(temp_font);
        drop(temp_ok);
        drop(temp_cancel);

        // Use custom dimensions if provided, otherwise use calculated defaults
        let logical_width = self.width.unwrap_or(calc_width) as u16;
        let logical_height = self.height.unwrap_or(calc_height) as u16;

        // Create window with LOGICAL dimensions
        let mut window = create_window(logical_width, logical_height)?;
        window.set_title(if self.title.is_empty() {
            "Forms"
        } else {
            &self.title
        })?;

        // Get the actual scale factor from the window (compositor scale)
        let scale = window.scale_factor();

        // Now create everything at PHYSICAL scale
        let font = Font::load(scale);

        // Scale dimensions for physical rendering
        let padding = (BASE_PADDING as f32 * scale) as u32;
        let field_height = (BASE_FIELD_HEIGHT as f32 * scale) as u32;
        let field_spacing = (BASE_FIELD_SPACING as f32 * scale) as u32;
        let label_width = (BASE_LABEL_WIDTH as f32 * scale) as u32;
        let input_width = (BASE_INPUT_WIDTH as f32 * scale) as u32;

        // Calculate physical dimensions
        let physical_width = (logical_width as f32 * scale) as u32;
        let physical_height = (logical_height as f32 * scale) as u32;

        // Create buttons at physical scale
        let mut ok_button = Button::new("OK", &font, scale);
        let mut cancel_button = Button::new("Cancel", &font, scale);

        // Render prompt text at physical scale (wrapped to fit)
        let prompt_canvas = if !self.text.is_empty() {
            Some(
                font.render(&self.text)
                    .with_color(colors.text)
                    .with_max_width(input_width as f32)
                    .finish(),
            )
        } else {
            None
        };
        let prompt_height = prompt_canvas.as_ref().map(|c| c.height()).unwrap_or(0);

        // Create text inputs for each field
        let mut inputs: Vec<TextInput> = self
            .fields
            .iter()
            .map(|field| TextInput::new(input_width).with_password(field.is_password()))
            .collect();

        // Set first input as focused
        if !inputs.is_empty() {
            inputs[0].set_focus(true);
        }
        let mut focused_index = 0usize;

        // Layout calculation
        let mut y = padding as i32;
        let prompt_y = y;
        if prompt_height > 0 {
            y += prompt_height as i32 + (16.0 * scale) as i32;
        }

        // Position inputs
        let label_x = padding as i32;
        let input_x = padding as i32 + label_width as i32 + (10.0 * scale) as i32;
        let mut field_positions: Vec<i32> = Vec::new();

        for (i, input) in inputs.iter_mut().enumerate() {
            let field_y = y + (i as u32 * (field_height + field_spacing)) as i32;
            field_positions.push(field_y);
            input.set_position(input_x, field_y);
        }

        // Button positions (right-aligned)
        let button_y = physical_height as i32 - padding as i32 - (32.0 * scale) as i32;
        let mut button_x = physical_width as i32 - padding as i32;
        button_x -= cancel_button.width() as i32;
        cancel_button.set_position(button_x, button_y);
        button_x -= (10.0 * scale) as i32 + ok_button.width() as i32;
        ok_button.set_position(button_x, button_y);

        // Track cursor position
        let mut cursor_x = 0i32;
        let mut cursor_y = 0i32;

        // Create canvas at PHYSICAL dimensions
        let mut canvas = Canvas::new(physical_width, physical_height);

        // Draw function
        let draw = |canvas: &mut Canvas,
                    colors: &Colors,
                    font: &Font,
                    prompt_canvas: &Option<Canvas>,
                    fields: &[FormField],
                    inputs: &[TextInput],
                    ok_button: &Button,
                    cancel_button: &Button,
                    // Layout params
                    padding: u32,
                    label_x: i32,
                    field_positions: &[i32],
                    field_height: u32,
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

            // Draw fields
            for (i, (field, input)) in fields.iter().zip(inputs.iter()).enumerate() {
                let field_y = field_positions[i];

                // Draw label (vertically centered with input, wrapped if needed)
                let label_canvas = font
                    .render(field.label())
                    .with_color(colors.text)
                    .with_max_width(label_width as f32)
                    .finish();
                let label_y = field_y + (field_height as i32 - label_canvas.height() as i32) / 2;
                canvas.draw_canvas(&label_canvas, label_x, label_y);

                // Draw input
                input.draw_to(canvas, colors, font);
            }

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
            &self.fields,
            &inputs,
            &ok_button,
            &cancel_button,
            padding,
            label_x,
            &field_positions,
            field_height,
            prompt_y,
            scale,
        );
        window.set_contents(&canvas)?;
        window.show()?;

        // Event loop
        let mut window_dragging = false;
        loop {
            let event = window.wait_for_event()?;
            let mut needs_redraw = false;

            match &event {
                WindowEvent::CloseRequested => return Ok(FormsResult::Closed),
                WindowEvent::RedrawRequested => needs_redraw = true,
                WindowEvent::CursorMove(pos) => {
                    if window_dragging {
                        let _ = window.start_drag();
                        window_dragging = false;
                    }

                    cursor_x = pos.x as i32;
                    cursor_y = pos.y as i32;

                    // Check if cursor is over any input field and update cursor shape
                    let mut over_input = false;
                    for input in inputs.iter() {
                        let ix = input.x();
                        let iy = input.y();
                        let iw = input.width();
                        let ih = input.height();

                        if cursor_x >= ix
                            && cursor_x < ix + iw as i32
                            && cursor_y >= iy
                            && cursor_y < iy + ih as i32
                        {
                            over_input = true;
                            break;
                        }
                    }
                    let _ = window.set_cursor(if over_input {
                        CursorShape::Text
                    } else {
                        CursorShape::Default
                    });
                }
                WindowEvent::ButtonPress(crate::backend::MouseButton::Left, _) => {
                    window_dragging = true;
                    // Check if clicking on any input field
                    for (i, input) in inputs.iter().enumerate() {
                        let ix = input.x();
                        let iy = input.y();
                        let iw = input.width();
                        let ih = input.height();

                        if cursor_x >= ix
                            && cursor_x < ix + iw as i32
                            && cursor_y >= iy
                            && cursor_y < iy + ih as i32
                        {
                            if i != focused_index {
                                inputs[focused_index].set_focus(false);
                                focused_index = i;
                                inputs[focused_index].set_focus(true);
                                needs_redraw = true;
                            }
                            break;
                        }
                    }
                }
                WindowEvent::ButtonRelease(crate::backend::MouseButton::Left, _) => {
                    window_dragging = false;
                }
                WindowEvent::KeyPress(key_event) => {
                    match key_event.keysym {
                        KEY_TAB => {
                            // Move to next field
                            inputs[focused_index].set_focus(false);
                            focused_index = (focused_index + 1) % inputs.len();
                            inputs[focused_index].set_focus(true);
                            needs_redraw = true;
                        }
                        KEY_ISO_LEFT_TAB => {
                            // Move to previous field (Shift+Tab)
                            inputs[focused_index].set_focus(false);
                            focused_index = if focused_index == 0 {
                                inputs.len() - 1
                            } else {
                                focused_index - 1
                            };
                            inputs[focused_index].set_focus(true);
                            needs_redraw = true;
                        }
                        KEY_RETURN => {
                            // Submit form
                            let values: Vec<String> = inputs
                                .iter()
                                .map(|input| input.text().to_string())
                                .collect();
                            return Ok(FormsResult::Values(values));
                        }
                        KEY_ESCAPE => {
                            return Ok(FormsResult::Cancelled);
                        }
                        _ => {}
                    }
                }
                _ => {}
            }

            // Process input events for focused field
            if inputs[focused_index].process_event(&event) {
                needs_redraw = true;
            }

            // Check for submission via input
            if inputs[focused_index].was_submitted() {
                let values: Vec<String> = inputs
                    .iter()
                    .map(|input| input.text().to_string())
                    .collect();
                return Ok(FormsResult::Values(values));
            }

            // Process button events
            needs_redraw |= ok_button.process_event(&event);
            needs_redraw |= cancel_button.process_event(&event);

            if ok_button.was_clicked() {
                let values: Vec<String> = inputs
                    .iter()
                    .map(|input| input.text().to_string())
                    .collect();
                return Ok(FormsResult::Values(values));
            }
            if cancel_button.was_clicked() {
                return Ok(FormsResult::Cancelled);
            }

            // Batch process pending events
            while let Some(ev) = window.poll_for_event()? {
                match &ev {
                    WindowEvent::CloseRequested => return Ok(FormsResult::Closed),
                    _ => {
                        if inputs[focused_index].process_event(&ev) {
                            needs_redraw = true;
                        }
                        if inputs[focused_index].was_submitted() {
                            let values: Vec<String> = inputs
                                .iter()
                                .map(|input| input.text().to_string())
                                .collect();
                            return Ok(FormsResult::Values(values));
                        }
                        needs_redraw |= ok_button.process_event(&ev);
                        needs_redraw |= cancel_button.process_event(&ev);
                    }
                }
            }

            if needs_redraw {
                draw(
                    &mut canvas,
                    colors,
                    &font,
                    &prompt_canvas,
                    &self.fields,
                    &inputs,
                    &ok_button,
                    &cancel_button,
                    padding,
                    label_x,
                    &field_positions,
                    field_height,
                    prompt_y,
                    scale,
                );
                window.set_contents(&canvas)?;
            }
        }
    }
}

impl Default for FormsBuilder {
    fn default() -> Self {
        Self::new()
    }
}
