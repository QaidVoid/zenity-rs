//! Message dialog implementation (info, warning, error, question).

use crate::backend::{MouseButton, Window, WindowEvent, create_window};
use crate::error::Error;
use crate::render::{Canvas, Font, rgb};
use crate::ui::{ButtonPreset, Colors, DialogResult, Icon};
use crate::ui::widgets::Widget;
use crate::ui::widgets::button::Button;

const ICON_SIZE: u32 = 48;
const PADDING: u32 = 20;
const BUTTON_SPACING: u32 = 10;
const MIN_WIDTH: u32 = 300;
const MAX_TEXT_WIDTH: f32 = 350.0;

/// Message dialog builder.
pub struct MessageBuilder {
    title: String,
    text: String,
    icon: Option<Icon>,
    buttons: ButtonPreset,
    colors: Option<&'static Colors>,
}

impl MessageBuilder {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            text: String::new(),
            icon: None,
            buttons: ButtonPreset::Ok,
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

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn buttons(mut self, buttons: ButtonPreset) -> Self {
        self.buttons = buttons;
        self
    }

    pub fn colors(mut self, colors: &'static Colors) -> Self {
        self.colors = Some(colors);
        self
    }

    pub fn show(self) -> Result<DialogResult, Error> {
        let colors = self.colors.unwrap_or_else(|| crate::ui::detect_theme());
        let font = Font::load();

        // Create buttons
        let labels = self.buttons.labels();
        let mut buttons: Vec<Button> = labels.iter().map(|l| Button::new(l, &font)).collect();

        // Calculate button row dimensions
        let buttons_width: u32 = buttons.iter().map(|b| b.width()).sum::<u32>()
            + (buttons.len().saturating_sub(1) as u32 * BUTTON_SPACING);

        // Render message text
        let text_canvas = font
            .render(&self.text)
            .with_color(colors.text)
            .with_max_width(MAX_TEXT_WIDTH)
            .finish();

        // Calculate content width
        let icon_width = if self.icon.is_some() {
            ICON_SIZE + PADDING
        } else {
            0
        };
        let content_width = icon_width + text_canvas.width();

        // Calculate window dimensions
        let inner_width = content_width.max(buttons_width);
        let width = (inner_width + PADDING * 2).max(MIN_WIDTH) as u16;

        let text_height = text_canvas.height().max(ICON_SIZE);
        let height = (PADDING * 3 + text_height + 32) as u16; // 32 for button height

        // Create window
        let mut window = create_window(width, height)?;
        window.set_title(&self.title)?;

        // Position buttons (right-aligned)
        let mut button_x = width as i32 - PADDING as i32;
        for button in buttons.iter_mut().rev() {
            button_x -= button.width() as i32;
            button.set_position(button_x, height as i32 - PADDING as i32 - 32);
            button_x -= BUTTON_SPACING as i32;
        }

        // Create canvas
        let mut canvas = Canvas::new(width as u32, height as u32);

        // Initial draw
        draw_dialog(
            &mut canvas,
            colors,
            &font,
            &self.text,
            self.icon,
            &buttons,
            text_canvas.width(),
            text_canvas.height(),
        );
        window.set_contents(&canvas)?;
        window.show()?;

        // Event loop
        let mut dragging = false;
        loop {
            let event = window.wait_for_event()?;

            match &event {
                WindowEvent::CloseRequested => {
                    return Ok(DialogResult::Closed);
                }
                WindowEvent::RedrawRequested => {
                    draw_dialog(
                        &mut canvas,
                        colors,
                        &font,
                        &self.text,
                        self.icon,
                        &buttons,
                        text_canvas.width(),
                        text_canvas.height(),
                    );
                    window.set_contents(&canvas)?;
                }
                WindowEvent::ButtonPress(MouseButton::Left) => {
                    // Check if clicking on title bar area (for dragging)
                    // For simplicity, we'll allow dragging from anywhere not on a button
                    dragging = true;
                }
                WindowEvent::ButtonRelease(MouseButton::Left) => {
                    if dragging {
                        dragging = false;
                    }
                }
                _ => {}
            }

            // Process events for buttons
            let mut needs_redraw = false;
            for (i, button) in buttons.iter_mut().enumerate() {
                if button.process_event(&event) {
                    needs_redraw = true;
                }
                if button.was_clicked() {
                    return Ok(DialogResult::Button(i));
                }
            }

            // Handle drag
            if dragging {
                if let WindowEvent::CursorMove(_) = &event {
                    let _ = window.start_drag();
                    dragging = false;
                }
            }

            // Batch process pending events
            while let Some(event) = window.poll_for_event()? {
                match &event {
                    WindowEvent::CloseRequested => {
                        return Ok(DialogResult::Closed);
                    }
                    _ => {
                        for (i, button) in buttons.iter_mut().enumerate() {
                            if button.process_event(&event) {
                                needs_redraw = true;
                            }
                            if button.was_clicked() {
                                return Ok(DialogResult::Button(i));
                            }
                        }
                    }
                }
            }

            if needs_redraw {
                draw_dialog(
                    &mut canvas,
                    colors,
                    &font,
                    &self.text,
                    self.icon,
                    &buttons,
                    text_canvas.width(),
                    text_canvas.height(),
                );
                window.set_contents(&canvas)?;
            }
        }
    }
}

fn draw_dialog(
    canvas: &mut Canvas,
    colors: &Colors,
    font: &Font,
    text: &str,
    icon: Option<Icon>,
    buttons: &[Button],
    text_width: u32,
    text_height: u32,
) {
    // Clear background
    canvas.fill(colors.window_bg);

    let mut x = PADDING as i32;
    let y = PADDING as i32;

    // Draw icon
    if let Some(icon) = icon {
        draw_icon(canvas, x, y, icon);
        x += (ICON_SIZE + PADDING) as i32;
    }

    // Draw text
    let text_canvas = font
        .render(text)
        .with_color(colors.text)
        .with_max_width(MAX_TEXT_WIDTH)
        .finish();

    // Center text vertically with icon
    let text_y = y + (ICON_SIZE as i32 - text_height as i32) / 2;
    canvas.draw_canvas(&text_canvas, x, text_y.max(y));

    // Draw buttons
    for button in buttons {
        button.draw_to(canvas, colors, font);
    }
}

fn draw_icon(canvas: &mut Canvas, x: i32, y: i32, icon: Icon) {
    let (color, shape) = match icon {
        Icon::Info => (rgb(66, 133, 244), IconShape::Circle),    // Blue
        Icon::Warning => (rgb(251, 188, 4), IconShape::Triangle), // Yellow
        Icon::Error => (rgb(234, 67, 53), IconShape::Circle),     // Red
        Icon::Question => (rgb(52, 168, 83), IconShape::Circle),  // Green
    };

    let cx = x as f32 + ICON_SIZE as f32 / 2.0;
    let cy = y as f32 + ICON_SIZE as f32 / 2.0;
    let r = ICON_SIZE as f32 / 2.0 - 2.0;

    match shape {
        IconShape::Circle => {
            // Draw filled circle
            for dy in 0..ICON_SIZE {
                for dx in 0..ICON_SIZE {
                    let px = x as f32 + dx as f32 + 0.5;
                    let py = y as f32 + dy as f32 + 0.5;
                    let dist = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
                    if dist <= r {
                        canvas.fill_rect(
                            x as f32 + dx as f32,
                            y as f32 + dy as f32,
                            1.0,
                            1.0,
                            color,
                        );
                    }
                }
            }
        }
        IconShape::Triangle => {
            // Draw triangle (warning sign)
            let top = (cx, y as f32 + 4.0);
            let left = (x as f32 + 4.0, y as f32 + ICON_SIZE as f32 - 4.0);
            let right = (x as f32 + ICON_SIZE as f32 - 4.0, y as f32 + ICON_SIZE as f32 - 4.0);

            for dy in 0..ICON_SIZE {
                for dx in 0..ICON_SIZE {
                    let px = x as f32 + dx as f32 + 0.5;
                    let py = y as f32 + dy as f32 + 0.5;
                    if point_in_triangle(px, py, top, left, right) {
                        canvas.fill_rect(
                            x as f32 + dx as f32,
                            y as f32 + dy as f32,
                            1.0,
                            1.0,
                            color,
                        );
                    }
                }
            }
        }
    }

    // Draw symbol (!, ?, i, x)
    let symbol = match icon {
        Icon::Info => "i",
        Icon::Warning => "!",
        Icon::Error => "X",
        Icon::Question => "?",
    };

    let font = Font::load();
    let symbol_canvas = font.render(symbol).with_color(rgb(255, 255, 255)).finish();
    let sx = x + (ICON_SIZE as i32 - symbol_canvas.width() as i32) / 2;
    let sy = y + (ICON_SIZE as i32 - symbol_canvas.height() as i32) / 2;
    canvas.draw_canvas(&symbol_canvas, sx, sy);
}

enum IconShape {
    Circle,
    Triangle,
}

fn point_in_triangle(
    px: f32,
    py: f32,
    (ax, ay): (f32, f32),
    (bx, by): (f32, f32),
    (cx, cy): (f32, f32),
) -> bool {
    let v0x = cx - ax;
    let v0y = cy - ay;
    let v1x = bx - ax;
    let v1y = by - ay;
    let v2x = px - ax;
    let v2y = py - ay;

    let dot00 = v0x * v0x + v0y * v0y;
    let dot01 = v0x * v1x + v0y * v1y;
    let dot02 = v0x * v2x + v0y * v2y;
    let dot11 = v1x * v1x + v1y * v1y;
    let dot12 = v1x * v2x + v1y * v2y;

    let inv_denom = 1.0 / (dot00 * dot11 - dot01 * dot01);
    let u = (dot11 * dot02 - dot01 * dot12) * inv_denom;
    let v = (dot00 * dot12 - dot01 * dot02) * inv_denom;

    u >= 0.0 && v >= 0.0 && u + v <= 1.0
}

impl Default for MessageBuilder {
    fn default() -> Self {
        Self::new()
    }
}
