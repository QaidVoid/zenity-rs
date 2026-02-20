//! Text input widget for single-line text entry.

use super::Widget;
use crate::{
    backend::{Modifiers, WindowEvent},
    render::{Canvas, Font, Rgba},
    ui::Colors,
};

const INPUT_HEIGHT: u32 = 32;
const INPUT_RADIUS: f32 = 5.0;
const INPUT_PADDING: i32 = 8;

// XKB keysym constants
const KEY_BACKSPACE: u32 = 0xff08;
const KEY_DELETE: u32 = 0xffff;
const KEY_LEFT: u32 = 0xff51;
const KEY_RIGHT: u32 = 0xff53;
const KEY_HOME: u32 = 0xff50;
const KEY_END: u32 = 0xff57;
const KEY_RETURN: u32 = 0xff0d;
const KEY_KP_ENTER: u32 = 0xff8d;
const KEY_TAB: u32 = 0xff09;

/// A single-line text input widget.
pub struct TextInput {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    text: String,
    cursor_pos: usize,
    focused: bool,
    password: bool,
    placeholder: String,
    submitted: bool,
    completion: Option<String>,
    tab_pressed: bool,
}

impl TextInput {
    pub fn new(width: u32) -> Self {
        Self {
            x: 0,
            y: 0,
            width,
            height: INPUT_HEIGHT,
            text: String::new(),
            cursor_pos: 0,
            focused: false,
            password: false,
            placeholder: String::new(),
            submitted: false,
            completion: None,
            tab_pressed: false,
        }
    }

    pub fn with_password(mut self, password: bool) -> Self {
        self.password = password;
        self
    }

    pub fn with_placeholder(mut self, placeholder: &str) -> Self {
        self.placeholder = placeholder.to_string();
        self
    }

    pub fn with_default_text(mut self, text: &str) -> Self {
        self.text = text.to_string();
        self.cursor_pos = self.char_count();
        self
    }

    /// Returns the current text content.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Sets the text content and moves cursor to end.
    pub fn set_text(&mut self, text: &str) {
        self.text = text.to_string();
        self.cursor_pos = self.char_count();
        self.completion = None;
    }

    /// Returns true if Enter was pressed.
    pub fn was_submitted(&mut self) -> bool {
        let submitted = self.submitted;
        self.submitted = false;
        submitted
    }

    /// Sets the current completion suggestion (the suffix after the user's text).
    pub fn set_completion(&mut self, completion: Option<String>) {
        self.completion = completion;
    }

    /// Returns true if Tab was pressed (consumed once per check).
    pub fn was_tab_pressed(&mut self) -> bool {
        let pressed = self.tab_pressed;
        self.tab_pressed = false;
        pressed
    }

    /// Returns the display text (masked if password mode).
    fn display_text(&self) -> String {
        if self.password {
            "*".repeat(self.char_count())
        } else {
            self.text.clone()
        }
    }

    /// Returns the number of characters in the text.
    fn char_count(&self) -> usize {
        self.text.chars().count()
    }

    /// Converts a character position to a byte position.
    fn byte_position(&self, char_pos: usize) -> usize {
        self.text
            .char_indices()
            .nth(char_pos)
            .map(|(i, _)| i)
            .unwrap_or(self.text.len())
    }

    /// Inserts a character at the cursor position.
    fn insert_char(&mut self, c: char) {
        let byte_pos = self.byte_position(self.cursor_pos);
        self.text.insert(byte_pos, c);
        self.cursor_pos += 1;
        self.completion = None;
    }

    /// Deletes the character before the cursor (backspace).
    fn delete_before(&mut self) {
        if self.cursor_pos > 0 {
            let byte_pos = self.byte_position(self.cursor_pos - 1);
            let end_pos = self.byte_position(self.cursor_pos);
            self.text.drain(byte_pos..end_pos);
            self.cursor_pos -= 1;
            self.completion = None;
        }
    }

    /// Deletes the character after the cursor (delete).
    fn delete_after(&mut self) {
        if self.cursor_pos < self.char_count() {
            let byte_pos = self.byte_position(self.cursor_pos);
            let end_pos = self.byte_position(self.cursor_pos + 1);
            self.text.drain(byte_pos..end_pos);
            self.completion = None;
        }
    }

    fn move_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
        }
    }

    fn move_right(&mut self) {
        if self.cursor_pos < self.char_count() {
            self.cursor_pos += 1;
        }
    }

    fn move_home(&mut self) {
        self.cursor_pos = 0;
    }

    fn move_end(&mut self) {
        self.cursor_pos = self.char_count();
    }

    fn handle_key(&mut self, keysym: u32, modifiers: Modifiers) -> bool {
        match keysym {
            KEY_BACKSPACE => {
                self.delete_before();
                true
            }
            KEY_DELETE => {
                self.delete_after();
                true
            }
            KEY_LEFT => {
                if modifiers.contains(Modifiers::CTRL) {
                    self.move_home();
                } else {
                    self.move_left();
                }
                true
            }
            KEY_RIGHT => {
                if modifiers.contains(Modifiers::CTRL) {
                    self.move_end();
                } else {
                    self.move_right();
                }
                true
            }
            KEY_HOME => {
                self.move_home();
                true
            }
            KEY_END => {
                self.move_end();
                true
            }
            KEY_RETURN | KEY_KP_ENTER => {
                self.submitted = true;
                true
            }
            KEY_TAB => {
                if let Some(suffix) = self.completion.take() {
                    self.text.push_str(&suffix);
                    self.cursor_pos = self.char_count();
                }
                self.tab_pressed = true;
                true
            }
            _ => false,
        }
    }

    /// Draws the text input to a canvas.
    pub fn draw_to(&self, canvas: &mut Canvas, colors: &Colors, font: &Font) {
        // Draw background
        let bg_color = if self.focused {
            colors.input_bg_focused
        } else {
            colors.input_bg
        };

        canvas.fill_rounded_rect(
            self.x as f32,
            self.y as f32,
            self.width as f32,
            self.height as f32,
            INPUT_RADIUS,
            bg_color,
        );

        // Draw border
        let border_color = if self.focused {
            colors.input_border_focused
        } else {
            colors.input_border
        };

        canvas.stroke_rounded_rect(
            self.x as f32,
            self.y as f32,
            self.width as f32,
            self.height as f32,
            INPUT_RADIUS,
            border_color,
            1.0,
        );

        // Draw text or placeholder
        let display = self.display_text();
        let (text_to_render, text_color): (&str, Rgba) = if display.is_empty() && !self.focused {
            (&self.placeholder, colors.input_placeholder)
        } else {
            (&display, colors.text)
        };

        if !text_to_render.is_empty() {
            let text_canvas = font.render(text_to_render).with_color(text_color).finish();
            let text_y = self.y + (self.height as i32 - text_canvas.height() as i32) / 2;

            // Clip text to input width
            let available_width = (self.width as i32 - 2 * INPUT_PADDING) as u32;
            if text_canvas.width() > available_width {
                // Create a sub-pixmap with only the visible portion
                let mut visible_canvas =
                    crate::render::Canvas::new(available_width, text_canvas.height());
                visible_canvas.pixmap.draw_pixmap(
                    0,
                    0,
                    text_canvas.pixmap.as_ref(),
                    &tiny_skia::PixmapPaint::default(),
                    tiny_skia::Transform::identity(),
                    None,
                );
                canvas.draw_canvas(&visible_canvas, self.x + INPUT_PADDING, text_y);
            } else {
                canvas.draw_canvas(&text_canvas, self.x + INPUT_PADDING, text_y);
            }
        }

        // Draw cursor
        if self.focused {
            let cursor_x = if self.cursor_pos == 0 {
                self.x + INPUT_PADDING
            } else {
                let before_cursor = if self.password {
                    "*".repeat(self.cursor_pos)
                } else {
                    self.text.chars().take(self.cursor_pos).collect()
                };
                let text_before = font.render(&before_cursor).with_color(text_color).finish();
                self.x + INPUT_PADDING + text_before.width() as i32
            };

            let cursor_y = self.y + 6;
            let cursor_height = self.height as i32 - 12;

            // Draw cursor line
            canvas.fill_rect(
                cursor_x as f32,
                cursor_y as f32,
                1.0,
                cursor_height as f32,
                colors.text,
            );

            // Draw ghost completion text after cursor
            if let Some(ref suffix) = self.completion {
                if !suffix.is_empty() {
                    let ghost_canvas = font
                        .render(suffix)
                        .with_color(colors.input_placeholder)
                        .finish();
                    let ghost_y = self.y + (self.height as i32 - ghost_canvas.height() as i32) / 2;
                    let ghost_x = cursor_x + 1;
                    let available = (self.x + self.width as i32 - INPUT_PADDING - ghost_x) as u32;
                    if available > 0 {
                        if ghost_canvas.width() > available {
                            let mut clipped =
                                crate::render::Canvas::new(available, ghost_canvas.height());
                            clipped.pixmap.draw_pixmap(
                                0,
                                0,
                                ghost_canvas.pixmap.as_ref(),
                                &tiny_skia::PixmapPaint::default(),
                                tiny_skia::Transform::identity(),
                                None,
                            );
                            canvas.draw_canvas(&clipped, ghost_x, ghost_y);
                        } else {
                            canvas.draw_canvas(&ghost_canvas, ghost_x, ghost_y);
                        }
                    }
                }
            }
        }
    }

    pub fn set_focus(&mut self, focused: bool) {
        self.focused = focused;
    }

    pub fn has_focus(&self) -> bool {
        self.focused
    }
}

impl Widget for TextInput {
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
            WindowEvent::ButtonPress(crate::backend::MouseButton::Left, _) => {
                // Check if clicked inside
                // Focus handling is done by the dialog
                false
            }
            WindowEvent::TextInput(c) if self.focused => {
                self.insert_char(*c);
                true
            }
            WindowEvent::KeyPress(key_event) if self.focused => {
                self.handle_key(key_event.keysym, key_event.modifiers)
            }
            _ => false,
        }
    }

    fn draw(&self, _canvas: &mut Canvas, _colors: &Colors) {
        // Use draw_to instead for font access
    }
}
