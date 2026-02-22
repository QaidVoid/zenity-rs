//! Scale dialog implementation for selecting a numeric value with a slider.

use crate::{
    backend::{MouseButton, Window, WindowEvent, create_window},
    error::Error,
    render::{Canvas, Font},
    ui::{
        Colors,
        widgets::{Widget, button::Button},
    },
};

const BASE_PADDING: u32 = 20;
const BASE_SLIDER_HEIGHT: u32 = 8;
const BASE_THUMB_SIZE: u32 = 20;
const BASE_SLIDER_WIDTH: u32 = 300;
const BASE_MIN_WIDTH: u32 = 350;

/// Scale dialog result.
#[derive(Debug, Clone)]
pub enum ScaleResult {
    /// User selected a value and clicked OK.
    Value(i32),
    /// User cancelled the dialog.
    Cancelled,
    /// Dialog was closed.
    Closed,
}

impl ScaleResult {
    pub fn exit_code(&self) -> i32 {
        match self {
            ScaleResult::Value(_) => 0,
            ScaleResult::Cancelled => 1,
            ScaleResult::Closed => 255,
        }
    }
}

/// Scale dialog builder.
pub struct ScaleBuilder {
    title: String,
    text: String,
    value: i32,
    min_value: i32,
    max_value: i32,
    step: i32,
    hide_value: bool,
    width: Option<u32>,
    height: Option<u32>,
    colors: Option<&'static Colors>,
}

impl ScaleBuilder {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            text: String::new(),
            value: 0,
            min_value: 0,
            max_value: 100,
            step: 1,
            hide_value: false,
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

    /// Set the initial value.
    pub fn value(mut self, value: i32) -> Self {
        self.value = value;
        self
    }

    /// Set the minimum value (default: 0).
    pub fn min_value(mut self, min: i32) -> Self {
        self.min_value = min;
        self
    }

    /// Set the maximum value (default: 100).
    pub fn max_value(mut self, max: i32) -> Self {
        self.max_value = max;
        self
    }

    /// Set the step increment (default: 1).
    pub fn step(mut self, step: i32) -> Self {
        self.step = step.max(1);
        self
    }

    /// Hide the value display.
    pub fn hide_value(mut self, hide: bool) -> Self {
        self.hide_value = hide;
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

    pub fn show(self) -> Result<ScaleResult, Error> {
        let colors = self.colors.unwrap_or_else(|| crate::ui::detect_theme());

        // Clamp initial value to range
        let mut value = self.value.clamp(self.min_value, self.max_value);

        // First pass: calculate LOGICAL dimensions using scale 1.0
        let temp_font = Font::load(1.0);
        let temp_ok = Button::new("OK", &temp_font, 1.0);
        let temp_cancel = Button::new("Cancel", &temp_font, 1.0);
        let temp_prompt_height = if !self.text.is_empty() {
            temp_font.render(&self.text).finish().height()
        } else {
            0
        };

        let logical_buttons_width = temp_ok.width() + temp_cancel.width() + 10;
        let logical_content_width = BASE_SLIDER_WIDTH.max(logical_buttons_width);
        let calc_width = (logical_content_width + BASE_PADDING * 2).max(BASE_MIN_WIDTH);

        // Height: padding + text + slider area + value display + buttons + padding
        let value_display_height = if self.hide_value { 0 } else { 24 };
        let calc_height = BASE_PADDING * 2
            + temp_prompt_height
            + (if temp_prompt_height > 0 { 16 } else { 0 })
            + BASE_THUMB_SIZE + 16  // Slider area with some margin
            + value_display_height
            + 32 + 16; // Buttons

        drop(temp_font);
        drop(temp_ok);
        drop(temp_cancel);

        // Use custom dimensions if provided, otherwise use calculated defaults
        let logical_width = self.width.unwrap_or(calc_width) as u16;
        let logical_height = self.height.unwrap_or(calc_height) as u16;

        // Create window with LOGICAL dimensions
        let mut window = create_window(logical_width, logical_height)?;
        window.set_title(if self.title.is_empty() {
            "Scale"
        } else {
            &self.title
        })?;

        // Get the actual scale factor from the window (compositor scale)
        let scale = window.scale_factor();

        // Now create everything at PHYSICAL scale
        let font = Font::load(scale);

        // Scale dimensions for physical rendering
        let padding = (BASE_PADDING as f32 * scale) as u32;
        let slider_height = (BASE_SLIDER_HEIGHT as f32 * scale) as u32;
        let thumb_size = (BASE_THUMB_SIZE as f32 * scale) as u32;
        let slider_width = (BASE_SLIDER_WIDTH as f32 * scale) as u32;

        // Calculate physical dimensions
        let physical_width = (logical_width as f32 * scale) as u32;
        let physical_height = (logical_height as f32 * scale) as u32;

        // Create buttons at physical scale
        let mut ok_button = Button::new("OK", &font, scale);
        let mut cancel_button = Button::new("Cancel", &font, scale);

        // Render prompt text at physical scale
        let prompt_canvas = if !self.text.is_empty() {
            Some(font.render(&self.text).with_color(colors.text).finish())
        } else {
            None
        };
        let prompt_height = prompt_canvas.as_ref().map(|c| c.height()).unwrap_or(0);

        // Layout calculation
        let mut y = padding as i32;
        let prompt_y = y;
        if prompt_height > 0 {
            y += prompt_height as i32 + (16.0 * scale) as i32;
        }

        // Slider position (centered horizontally)
        let slider_x = (physical_width - slider_width) as i32 / 2;
        let slider_y = y + (thumb_size as i32 - slider_height as i32) / 2;
        let thumb_y = y;
        y += thumb_size as i32 + (16.0 * scale) as i32;

        // Button positions (right-aligned)
        let button_y = physical_height as i32 - padding as i32 - (32.0 * scale) as i32;
        let mut button_x = physical_width as i32 - padding as i32;
        button_x -= cancel_button.width() as i32;
        cancel_button.set_position(button_x, button_y);
        button_x -= (10.0 * scale) as i32 + ok_button.width() as i32;
        ok_button.set_position(button_x, button_y);

        // State
        let mut dragging = false;
        let mut thumb_hovered = false;
        let mut cursor_x = 0i32;
        let mut cursor_y = 0i32;

        // Create canvas at PHYSICAL dimensions
        let mut canvas = Canvas::new(physical_width, physical_height);

        // Helper to calculate thumb position from value
        let value_to_thumb_x = |val: i32| -> i32 {
            let range = (self.max_value - self.min_value) as f32;
            let ratio = if range > 0.0 {
                (val - self.min_value) as f32 / range
            } else {
                0.0
            };
            slider_x + (ratio * (slider_width - thumb_size) as f32) as i32
        };

        // Helper to calculate value from x position
        let x_to_value = |x: i32| -> i32 {
            let track_start = slider_x + thumb_size as i32 / 2;
            let track_end = slider_x + slider_width as i32 - thumb_size as i32 / 2;
            let track_width = track_end - track_start;

            let ratio = if track_width > 0 {
                ((x - track_start) as f32 / track_width as f32).clamp(0.0, 1.0)
            } else {
                0.0
            };

            let range = self.max_value - self.min_value;
            let raw_value = self.min_value + (ratio * range as f32) as i32;

            // Snap to step
            let steps = (raw_value - self.min_value) / self.step;
            (self.min_value + steps * self.step).clamp(self.min_value, self.max_value)
        };

        // Draw function
        let draw = |canvas: &mut Canvas,
                    colors: &Colors,
                    font: &Font,
                    prompt_canvas: &Option<Canvas>,
                    value: i32,
                    thumb_hovered: bool,
                    dragging: bool,
                    ok_button: &Button,
                    cancel_button: &Button,
                    hide_value: bool,
                    // Layout params
                    padding: u32,
                    slider_x: i32,
                    slider_y: i32,
                    slider_width: u32,
                    slider_height: u32,
                    thumb_y: i32,
                    thumb_size: u32,
                    value_y: i32,
                    prompt_y: i32,
                    physical_width: u32,
                    scale: f32,
                    value_to_thumb_x: &dyn Fn(i32) -> i32| {
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

            // Draw slider track background
            canvas.fill_rounded_rect(
                slider_x as f32,
                slider_y as f32,
                slider_width as f32,
                slider_height as f32,
                slider_height as f32 / 2.0,
                colors.progress_bg,
            );

            // Draw filled portion of track
            let thumb_x = value_to_thumb_x(value);
            let fill_width = (thumb_x - slider_x + thumb_size as i32 / 2) as f32;
            if fill_width > 0.0 {
                canvas.fill_rounded_rect(
                    slider_x as f32,
                    slider_y as f32,
                    fill_width.min(slider_width as f32),
                    slider_height as f32,
                    slider_height as f32 / 2.0,
                    colors.progress_fill,
                );
            }

            // Draw track border
            canvas.stroke_rounded_rect(
                slider_x as f32,
                slider_y as f32,
                slider_width as f32,
                slider_height as f32,
                slider_height as f32 / 2.0,
                colors.progress_border,
                1.0,
            );

            // Draw thumb
            let thumb_color = if dragging {
                colors.button_pressed
            } else if thumb_hovered {
                colors.button_hover
            } else {
                colors.button
            };
            canvas.fill_rounded_rect(
                thumb_x as f32,
                thumb_y as f32,
                thumb_size as f32,
                thumb_size as f32,
                thumb_size as f32 / 2.0,
                thumb_color,
            );
            canvas.stroke_rounded_rect(
                thumb_x as f32,
                thumb_y as f32,
                thumb_size as f32,
                thumb_size as f32,
                thumb_size as f32 / 2.0,
                colors.button_outline,
                1.0,
            );

            // Draw value display
            if !hide_value {
                let value_text = value.to_string();
                let value_canvas = font.render(&value_text).with_color(colors.text).finish();
                let value_x = (physical_width - value_canvas.width()) as i32 / 2;
                canvas.draw_canvas(&value_canvas, value_x, value_y);
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
            value,
            thumb_hovered,
            dragging,
            &ok_button,
            &cancel_button,
            self.hide_value,
            padding,
            slider_x,
            slider_y,
            slider_width,
            slider_height,
            thumb_y,
            thumb_size,
            y,
            prompt_y,
            physical_width,
            scale,
            &value_to_thumb_x,
        );
        window.set_contents(&canvas)?;
        window.show()?;

        // Event loop
        let mut window_dragging = false;
        loop {
            let event = window.wait_for_event()?;
            let mut needs_redraw = false;

            match &event {
                WindowEvent::CloseRequested => return Ok(ScaleResult::Closed),
                WindowEvent::RedrawRequested => needs_redraw = true,
                WindowEvent::CursorMove(pos) => {
                    if window_dragging {
                        let _ = window.start_drag();
                        window_dragging = false;
                    }

                    cursor_x = pos.x as i32;
                    cursor_y = pos.y as i32;

                    // Check thumb hover
                    let thumb_x = value_to_thumb_x(value);
                    let old_hovered = thumb_hovered;
                    thumb_hovered = cursor_x >= thumb_x
                        && cursor_x < thumb_x + thumb_size as i32
                        && cursor_y >= thumb_y
                        && cursor_y < thumb_y + thumb_size as i32;

                    if old_hovered != thumb_hovered {
                        needs_redraw = true;
                    }

                    // Handle dragging
                    if dragging {
                        let new_value = x_to_value(cursor_x);
                        if new_value != value {
                            value = new_value;
                            needs_redraw = true;
                        }
                    }
                }
                WindowEvent::ButtonPress(MouseButton::Left, _) => {
                    window_dragging = true;
                    let mx = cursor_x;
                    let my = cursor_y;

                    // Check if clicking on thumb
                    let thumb_x = value_to_thumb_x(value);
                    if mx >= thumb_x
                        && mx < thumb_x + thumb_size as i32
                        && my >= thumb_y
                        && my < thumb_y + thumb_size as i32
                    {
                        dragging = true;
                        needs_redraw = true;
                    }
                    // Check if clicking on track
                    else if mx >= slider_x
                        && mx < slider_x + slider_width as i32
                        && my >= slider_y
                        && my < slider_y + slider_height as i32 + thumb_size as i32
                    {
                        let new_value = x_to_value(mx);
                        if new_value != value {
                            value = new_value;
                            needs_redraw = true;
                        }
                        dragging = true;
                    }
                }
                WindowEvent::ButtonRelease(MouseButton::Left, _) => {
                    window_dragging = false;
                    if dragging {
                        dragging = false;
                        needs_redraw = true;
                    }
                }
                WindowEvent::KeyPress(key_event) => {
                    const KEY_LEFT: u32 = 0xff51;
                    const KEY_RIGHT: u32 = 0xff53;
                    const KEY_HOME: u32 = 0xff50;
                    const KEY_END: u32 = 0xff57;
                    const KEY_RETURN: u32 = 0xff0d;
                    const KEY_ESCAPE: u32 = 0xff1b;

                    match key_event.keysym {
                        KEY_LEFT => {
                            let new_value = (value - self.step).max(self.min_value);
                            if new_value != value {
                                value = new_value;
                                needs_redraw = true;
                            }
                        }
                        KEY_RIGHT => {
                            let new_value = (value + self.step).min(self.max_value);
                            if new_value != value {
                                value = new_value;
                                needs_redraw = true;
                            }
                        }
                        KEY_HOME => {
                            if value != self.min_value {
                                value = self.min_value;
                                needs_redraw = true;
                            }
                        }
                        KEY_END => {
                            if value != self.max_value {
                                value = self.max_value;
                                needs_redraw = true;
                            }
                        }
                        KEY_RETURN => {
                            return Ok(ScaleResult::Value(value));
                        }
                        KEY_ESCAPE => {
                            return Ok(ScaleResult::Cancelled);
                        }
                        _ => {}
                    }
                }
                _ => {}
            }

            needs_redraw |= ok_button.process_event(&event);
            needs_redraw |= cancel_button.process_event(&event);

            if ok_button.was_clicked() {
                return Ok(ScaleResult::Value(value));
            }
            if cancel_button.was_clicked() {
                return Ok(ScaleResult::Cancelled);
            }

            // Batch process pending events
            while let Some(ev) = window.poll_for_event()? {
                match &ev {
                    WindowEvent::CloseRequested => return Ok(ScaleResult::Closed),
                    WindowEvent::CursorMove(pos) if dragging => {
                        let new_value = x_to_value(pos.x as i32);
                        if new_value != value {
                            value = new_value;
                            needs_redraw = true;
                        }
                    }
                    WindowEvent::ButtonRelease(MouseButton::Left, _) => {
                        if dragging {
                            dragging = false;
                            needs_redraw = true;
                        }
                    }
                    _ => {}
                }
                needs_redraw |= ok_button.process_event(&ev);
                needs_redraw |= cancel_button.process_event(&ev);
            }

            if needs_redraw {
                draw(
                    &mut canvas,
                    colors,
                    &font,
                    &prompt_canvas,
                    value,
                    thumb_hovered,
                    dragging,
                    &ok_button,
                    &cancel_button,
                    self.hide_value,
                    padding,
                    slider_x,
                    slider_y,
                    slider_width,
                    slider_height,
                    thumb_y,
                    thumb_size,
                    y,
                    prompt_y,
                    physical_width,
                    scale,
                    &value_to_thumb_x,
                );
                window.set_contents(&canvas)?;
            }
        }
    }
}

impl Default for ScaleBuilder {
    fn default() -> Self {
        Self::new()
    }
}
