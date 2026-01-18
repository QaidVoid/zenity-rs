//! Button widget.

use crate::backend::{MouseButton, WindowEvent};
use crate::render::{Canvas, Font};
use crate::ui::Colors;

use super::{Widget, point_in_rect};

/// A clickable button widget.
pub(crate) struct Button {
    label: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    hovered: bool,
    pressed: bool,
    clicked: bool,
}

const BUTTON_HEIGHT: u32 = 32;
const BUTTON_PADDING: u32 = 24;
const BUTTON_RADIUS: f32 = 5.0;

impl Button {
    pub fn new(label: &str, font: &Font) -> Self {
        let (text_w, _) = font.render(label).measure();
        let width = (text_w as u32 + BUTTON_PADDING * 2).max(80);

        Self {
            label: label.to_string(),
            x: 0,
            y: 0,
            width,
            height: BUTTON_HEIGHT,
            hovered: false,
            pressed: false,
            clicked: false,
        }
    }

    /// Returns true if the button was clicked this frame.
    pub fn was_clicked(&mut self) -> bool {
        let clicked = self.clicked;
        self.clicked = false;
        clicked
    }

    /// Draws the button to a canvas.
    pub fn draw_to(&self, canvas: &mut Canvas, colors: &Colors, font: &Font) {
        // Determine button color based on state
        let bg_color = if self.pressed {
            colors.button_pressed
        } else if self.hovered {
            colors.button_hover
        } else {
            colors.button
        };

        // Draw button background
        canvas.fill_rounded_rect(
            self.x as f32,
            self.y as f32,
            self.width as f32,
            self.height as f32,
            BUTTON_RADIUS,
            bg_color,
        );

        // Draw button outline
        canvas.stroke_rounded_rect(
            self.x as f32,
            self.y as f32,
            self.width as f32,
            self.height as f32,
            BUTTON_RADIUS,
            colors.button_outline,
            1.0,
        );

        // Draw button label
        let text_canvas = font.render(&self.label).with_color(colors.button_text).finish();
        let text_x = self.x + (self.width as i32 - text_canvas.width() as i32) / 2;
        let text_y = self.y + (self.height as i32 - text_canvas.height() as i32) / 2;
        canvas.draw_canvas(&text_canvas, text_x, text_y);
    }
}

impl Widget for Button {
    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn x(&self) -> i32 {
        self.x
    }

    fn y(&self) -> i32 {
        self.y
    }

    fn set_position(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
    }

    fn process_event(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::CursorMove(pos) | WindowEvent::CursorEnter(pos) => {
                self.hovered = point_in_rect(
                    pos.x as i32,
                    pos.y as i32,
                    self.x,
                    self.y,
                    self.width,
                    self.height,
                );
                true
            }
            WindowEvent::CursorLeave => {
                self.hovered = false;
                self.pressed = false;
                true
            }
            WindowEvent::ButtonPress(MouseButton::Left) if self.hovered => {
                self.pressed = true;
                true
            }
            WindowEvent::ButtonRelease(MouseButton::Left) => {
                if self.pressed && self.hovered {
                    self.clicked = true;
                }
                self.pressed = false;
                true
            }
            _ => false,
        }
    }

    fn draw(&self, _canvas: &mut Canvas, _colors: &Colors) {
        // Use draw_to instead for font access
    }
}
