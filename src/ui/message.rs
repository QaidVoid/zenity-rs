//! Message dialog implementation (info, warning, error, question).

use std::time::{Duration, Instant};

use crate::backend::{MouseButton, Window, WindowEvent, create_window};
use crate::error::Error;
use crate::render::{Canvas, Font, rgb};
use crate::ui::{ButtonPreset, Colors, DialogResult, Icon};
use crate::ui::widgets::Widget;
use crate::ui::widgets::button::Button;

const BASE_ICON_SIZE: u32 = 48;
const BASE_PADDING: u32 = 20;
const BASE_BUTTON_SPACING: u32 = 10;
const BASE_MIN_WIDTH: u32 = 300;
const BASE_MAX_TEXT_WIDTH: f32 = 350.0;

/// Message dialog builder.
pub struct MessageBuilder {
    title: String,
    text: String,
    icon: Option<Icon>,
    buttons: ButtonPreset,
    timeout: Option<u32>,
    width: Option<u32>,
    height: Option<u32>,
    colors: Option<&'static Colors>,
}

impl MessageBuilder {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            text: String::new(),
            icon: None,
            buttons: ButtonPreset::Ok,
            timeout: None,
            width: None,
            height: None,
            colors: None,
        }
    }

    /// Set timeout in seconds. Dialog will auto-close after this time.
    pub fn timeout(mut self, seconds: u32) -> Self {
        self.timeout = Some(seconds);
        self
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

    pub fn width(mut self, width: u32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn height(mut self, height: u32) -> Self {
        self.height = Some(height);
        self
    }

    pub fn show(self) -> Result<DialogResult, Error> {
        let colors = self.colors.unwrap_or_else(|| crate::ui::detect_theme());

        // First pass: calculate LOGICAL dimensions using a temporary font at scale 1.0
        let temp_font = Font::load(1.0);
        let labels = self.buttons.labels();

        // Calculate logical button widths
        let temp_buttons: Vec<Button> = labels.iter().map(|l| Button::new(l, &temp_font, 1.0)).collect();
        let logical_buttons_width: u32 = temp_buttons.iter().map(|b| b.width()).sum::<u32>()
            + (temp_buttons.len().saturating_sub(1) as u32 * BASE_BUTTON_SPACING);

        // Calculate logical text size
        let temp_text = temp_font
            .render(&self.text)
            .with_max_width(BASE_MAX_TEXT_WIDTH)
            .finish();

        let logical_icon_width = if self.icon.is_some() {
            BASE_ICON_SIZE + BASE_PADDING
        } else {
            0
        };
        let logical_content_width = logical_icon_width + temp_text.width();
        let logical_inner_width = logical_content_width.max(logical_buttons_width);
        let calc_width = (logical_inner_width + BASE_PADDING * 2).max(BASE_MIN_WIDTH);
        let logical_text_height = temp_text.height().max(BASE_ICON_SIZE);
        let calc_height = BASE_PADDING * 3 + logical_text_height + 32;

        // Use custom dimensions if provided, otherwise use calculated defaults
        let logical_width = self.width.unwrap_or(calc_width) as u16;
        let logical_height = self.height.unwrap_or(calc_height) as u16;

        // Create window with LOGICAL dimensions - window will handle physical scaling
        let mut window = create_window(logical_width, logical_height)?;
        window.set_title(&self.title)?;

        // Get the actual scale factor from the window (compositor scale)
        let scale = window.scale_factor();

        // Now create everything at PHYSICAL scale
        let font = Font::load(scale);

        // Scale dimensions for physical rendering
        let icon_size = (BASE_ICON_SIZE as f32 * scale) as u32;
        let padding = (BASE_PADDING as f32 * scale) as u32;
        let button_spacing = (BASE_BUTTON_SPACING as f32 * scale) as u32;
        let max_text_width = BASE_MAX_TEXT_WIDTH * scale;
        let button_height = (32.0 * scale) as u32;

        // Create buttons at physical scale
        let mut buttons: Vec<Button> = labels.iter().map(|l| Button::new(l, &font, scale)).collect();

        // Calculate physical dimensions
        let physical_width = (logical_width as f32 * scale) as u32;
        let physical_height = (logical_height as f32 * scale) as u32;

        // Pre-render text to get actual height
        let text_canvas = font
            .render(&self.text)
            .with_color(colors.text)
            .with_max_width(max_text_width)
            .finish();
        let text_height = text_canvas.height().max(icon_size);

        // Position buttons (right-aligned) in physical coordinates
        let mut button_x = physical_width as i32 - padding as i32;
        for button in buttons.iter_mut().rev() {
            button_x -= button.width() as i32;
            button.set_position(button_x, physical_height as i32 - padding as i32 - button_height as i32);
            button_x -= button_spacing as i32;
        }

        // Create canvas at PHYSICAL dimensions
        let mut canvas = Canvas::new(physical_width, physical_height);

        // Initial draw
        draw_dialog(
            &mut canvas,
            colors,
            &font,
            &self.text,
            self.icon,
            &buttons,
            text_canvas.height(),
            scale,
        );
        window.set_contents(&canvas)?;
        window.show()?;

        // Event loop
        let mut dragging = false;
        let deadline = self.timeout.map(|secs| Instant::now() + Duration::from_secs(secs as u64));

        loop {
            // Check timeout
            if let Some(deadline) = deadline {
                if Instant::now() >= deadline {
                    return Ok(DialogResult::Timeout);
                }
            }

            // Get event (use polling with sleep if timeout is set)
            let event = if deadline.is_some() {
                match window.poll_for_event()? {
                    Some(e) => e,
                    None => {
                        std::thread::sleep(Duration::from_millis(50));
                        continue;
                    }
                }
            } else {
                window.wait_for_event()?
            };

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
                        text_canvas.height(),
                        scale,
                    );
                    window.set_contents(&canvas)?;
                }
                WindowEvent::ButtonPress(MouseButton::Left) => {
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
                    text_canvas.height(),
                    scale,
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
    text_height: u32,
    scale: f32,
) {
    // Scale dimensions
    let icon_size = (BASE_ICON_SIZE as f32 * scale) as u32;
    let padding = (BASE_PADDING as f32 * scale) as u32;
    let max_text_width = BASE_MAX_TEXT_WIDTH * scale;

    // Clear background
    canvas.fill(colors.window_bg);

    let mut x = padding as i32;
    let y = padding as i32;

    // Draw icon
    if let Some(icon) = icon {
        draw_icon(canvas, x, y, icon, scale);
        x += (icon_size + padding) as i32;
    }

    // Draw text
    let text_canvas = font
        .render(text)
        .with_color(colors.text)
        .with_max_width(max_text_width)
        .finish();

    // Center text vertically with icon
    let text_y = y + (icon_size as i32 - text_height as i32) / 2;
    canvas.draw_canvas(&text_canvas, x, text_y.max(y));

    // Draw buttons
    for button in buttons {
        button.draw_to(canvas, colors, font);
    }
}

fn draw_icon(canvas: &mut Canvas, x: i32, y: i32, icon: Icon, scale: f32) {
    let icon_size = (BASE_ICON_SIZE as f32 * scale) as u32;
    let inset = (4.0 * scale) as f32;

    let (color, shape) = match icon {
        Icon::Info => (rgb(66, 133, 244), IconShape::Circle),    // Blue
        Icon::Warning => (rgb(251, 188, 4), IconShape::Triangle), // Yellow
        Icon::Error => (rgb(234, 67, 53), IconShape::Circle),     // Red
        Icon::Question => (rgb(52, 168, 83), IconShape::Circle),  // Green
    };

    let cx = x as f32 + icon_size as f32 / 2.0;
    let cy = y as f32 + icon_size as f32 / 2.0;
    let r = icon_size as f32 / 2.0 - (2.0 * scale);

    match shape {
        IconShape::Circle => {
            // Draw filled circle
            for dy in 0..icon_size {
                for dx in 0..icon_size {
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
            let top = (cx, y as f32 + inset);
            let left = (x as f32 + inset, y as f32 + icon_size as f32 - inset);
            let right = (x as f32 + icon_size as f32 - inset, y as f32 + icon_size as f32 - inset);

            for dy in 0..icon_size {
                for dx in 0..icon_size {
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

    let font = Font::load(scale);
    let symbol_canvas = font.render(symbol).with_color(rgb(255, 255, 255)).finish();
    let sx = x + (icon_size as i32 - symbol_canvas.width() as i32) / 2;
    let sy = y + (icon_size as i32 - symbol_canvas.height() as i32) / 2;
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
