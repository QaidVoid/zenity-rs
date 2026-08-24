//! Text input widget for single-line text entry.

use std::{
    cell::RefCell,
    time::{Duration, Instant},
};

use super::Widget;
use crate::{
    backend::{Modifiers, WindowEvent},
    render::{Canvas, Font, Rgba},
    ui::{
        Colors, KEY_BACKSPACE, KEY_DELETE, KEY_END, KEY_HOME, KEY_KP_ENTER, KEY_LEFT, KEY_RETURN,
        KEY_RIGHT, KEY_TAB,
    },
};

const INPUT_HEIGHT: u32 = 32;
const INPUT_RADIUS: f32 = 5.0;
const INPUT_PADDING: i32 = 8;
const DOUBLE_CLICK: Duration = Duration::from_millis(400);
const KEY_A: u32 = 0x61;

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
    /// Where a selection started; the other end is `cursor_pos`.
    anchor: Option<usize>,
    dragging: bool,
    last_click: Option<Instant>,
    /// Latest pointer position; button events carry modifiers, not coordinates.
    pointer: (i32, i32),
    /// X offset of every character boundary, measured while drawing. Mouse
    /// hit-testing needs the font, which only `draw_to` has.
    offsets: RefCell<(String, Vec<f32>)>,
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
            anchor: None,
            dragging: false,
            last_click: None,
            pointer: (0, 0),
            offsets: RefCell::new((String::new(), Vec::new())),
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
        self.anchor = None;
        self.completion = None;
    }

    /// Cursor position and selection anchor, for callers that cache their
    /// drawing and need to know when the caret moved.
    pub fn caret(&self) -> (usize, Option<usize>) {
        (self.cursor_pos, self.anchor)
    }

    /// The selected character range, if any.
    fn selection(&self) -> Option<(usize, usize)> {
        let anchor = self.anchor?;
        (anchor != self.cursor_pos)
            .then(|| (anchor.min(self.cursor_pos), anchor.max(self.cursor_pos)))
    }

    /// Removes the selected text and places the cursor where it was. Returns
    /// whether anything was removed.
    fn delete_selection(&mut self) -> bool {
        let Some((start, end)) = self.selection() else {
            self.anchor = None;
            return false;
        };
        let range = self.byte_position(start)..self.byte_position(end);
        self.text.drain(range);
        self.cursor_pos = start;
        self.anchor = None;
        self.completion = None;
        true
    }

    /// Moves the cursor, extending the selection when `extend` is set.
    fn move_cursor(&mut self, to: usize, extend: bool) {
        if extend {
            self.anchor.get_or_insert(self.cursor_pos);
        } else {
            self.anchor = None;
        }
        self.cursor_pos = to;
    }

    /// Character boundary nearest to a window x coordinate.
    fn char_at_x(&self, x: i32) -> usize {
        let offsets = self.offsets.borrow();
        let target = (x - self.x - INPUT_PADDING) as f32;
        offsets
            .1
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| (*a - target).abs().total_cmp(&(*b - target).abs()))
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    /// Character range of the word around `pos`, used for double-click.
    fn word_at(&self, pos: usize) -> (usize, usize) {
        let chars: Vec<char> = self.display_text().chars().collect();
        if chars.is_empty() {
            return (0, 0);
        }
        let is_word = |c: char| c.is_alphanumeric() || c == '_';
        let mid = pos.min(chars.len() - 1);
        if !is_word(chars[mid]) {
            return (mid, mid + 1);
        }
        let start = chars[..mid]
            .iter()
            .rposition(|&c| !is_word(c))
            .map_or(0, |i| i + 1);
        let end = chars[mid..]
            .iter()
            .position(|&c| !is_word(c))
            .map_or(chars.len(), |i| mid + i);
        (start, end)
    }

    /// Handles a mouse event, returning whether the widget needs a redraw.
    fn handle_mouse(&mut self, event: &WindowEvent) -> bool {
        use crate::backend::MouseButton;
        match event {
            WindowEvent::CursorEnter(pos) | WindowEvent::CursorMove(pos) => {
                self.pointer = (pos.x as i32, pos.y as i32);
                if !self.dragging {
                    return false;
                }
                let pos = self.char_at_x(self.pointer.0);
                let moved = pos != self.cursor_pos;
                self.cursor_pos = pos;
                moved
            }
            WindowEvent::ButtonPress(MouseButton::Left, _) => {
                let (x, y) = self.pointer;
                let inside = x >= self.x
                    && x < self.x + self.width as i32
                    && y >= self.y
                    && y < self.y + self.height as i32;
                if !inside {
                    return false;
                }
                let pos = self.char_at_x(x);
                let double = self
                    .last_click
                    .is_some_and(|last| last.elapsed() < DOUBLE_CLICK);
                self.last_click = Some(Instant::now());
                if double {
                    let (start, end) = self.word_at(pos);
                    self.anchor = Some(start);
                    self.cursor_pos = end;
                    self.dragging = false;
                } else {
                    self.cursor_pos = pos;
                    self.anchor = Some(pos);
                    self.dragging = true;
                }
                true
            }
            WindowEvent::ButtonRelease(MouseButton::Left, _) => {
                self.dragging = false;
                false
            }
            _ => false,
        }
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

    /// Inserts a character at the cursor position, replacing any selection.
    fn insert_char(&mut self, c: char) {
        self.delete_selection();
        let byte_pos = self.byte_position(self.cursor_pos);
        self.text.insert(byte_pos, c);
        self.cursor_pos += 1;
        self.completion = None;
    }

    /// Deletes the selection, or the character before the cursor (backspace).
    fn delete_before(&mut self) {
        if self.delete_selection() {
            return;
        }
        if self.cursor_pos > 0 {
            let byte_pos = self.byte_position(self.cursor_pos - 1);
            let end_pos = self.byte_position(self.cursor_pos);
            self.text.drain(byte_pos..end_pos);
            self.cursor_pos -= 1;
            self.completion = None;
        }
    }

    /// Deletes the selection, or the character after the cursor (delete).
    fn delete_after(&mut self) {
        if self.delete_selection() {
            return;
        }
        if self.cursor_pos < self.char_count() {
            let byte_pos = self.byte_position(self.cursor_pos);
            let end_pos = self.byte_position(self.cursor_pos + 1);
            self.text.drain(byte_pos..end_pos);
            self.completion = None;
        }
    }

    fn move_left(&mut self, extend: bool) {
        let to = match self.selection() {
            Some((start, _)) if !extend => start,
            _ => self.cursor_pos.saturating_sub(1),
        };
        self.move_cursor(to, extend);
    }

    fn move_right(&mut self, extend: bool) {
        let to = match self.selection() {
            Some((_, end)) if !extend => end,
            _ => (self.cursor_pos + 1).min(self.char_count()),
        };
        self.move_cursor(to, extend);
    }

    fn move_home(&mut self, extend: bool) {
        self.move_cursor(0, extend);
    }

    fn move_end(&mut self, extend: bool) {
        self.move_cursor(self.char_count(), extend);
    }

    fn select_all(&mut self) {
        self.anchor = Some(0);
        self.cursor_pos = self.char_count();
    }

    fn handle_key(&mut self, keysym: u32, modifiers: Modifiers) -> bool {
        let extend = modifiers.contains(Modifiers::SHIFT);
        match keysym {
            KEY_A if modifiers.contains(Modifiers::CTRL) => {
                self.select_all();
                true
            }
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
                    self.move_home(extend);
                } else {
                    self.move_left(extend);
                }
                true
            }
            KEY_RIGHT => {
                if modifiers.contains(Modifiers::CTRL) {
                    self.move_end(extend);
                } else {
                    self.move_right(extend);
                }
                true
            }
            KEY_HOME => {
                self.move_home(extend);
                true
            }
            KEY_END => {
                self.move_end(extend);
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
                    self.anchor = None;
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
        self.measure_offsets(&display, font);
        let (text_to_render, text_color): (&str, Rgba) = if display.is_empty() && !self.focused {
            (&self.placeholder, colors.input_placeholder)
        } else {
            (&display, colors.text)
        };

        // Selection highlight, painted behind the text
        if self.focused
            && let Some((start, end)) = self.selection()
        {
            let offsets = self.offsets.borrow();
            if let (Some(from), Some(to)) = (offsets.1.get(start), offsets.1.get(end)) {
                let limit = (self.width as i32 - 2 * INPUT_PADDING) as f32;
                let from = from.min(limit);
                let to = to.min(limit);
                canvas.fill_rect(
                    (self.x + INPUT_PADDING) as f32 + from,
                    (self.y + 5) as f32,
                    to - from,
                    (self.height - 10) as f32,
                    colors.row_selected,
                );
            }
        }

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
            let cursor_x = self.x
                + INPUT_PADDING
                + self
                    .offsets
                    .borrow()
                    .1
                    .get(self.cursor_pos)
                    .copied()
                    .unwrap_or(0.0) as i32;

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

    /// Caches the x offset of every character boundary of `display`, so mouse
    /// hit-testing and the cursor can be positioned without re-measuring.
    fn measure_offsets(&self, display: &str, font: &Font) {
        let mut cache = self.offsets.borrow_mut();
        if cache.0 == display && !cache.1.is_empty() {
            return;
        }
        let mut offsets = Vec::with_capacity(display.chars().count() + 1);
        offsets.push(0.0);
        for (i, _) in display.char_indices().skip(1) {
            offsets.push(font.render(&display[..i]).measure().0);
        }
        if !display.is_empty() {
            offsets.push(font.render(display).measure().0);
        }
        *cache = (display.to_string(), offsets);
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
            WindowEvent::ButtonPress(..)
            | WindowEvent::ButtonRelease(..)
            | WindowEvent::CursorMove(_)
            | WindowEvent::CursorEnter(_) => self.handle_mouse(event),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn input(text: &str) -> TextInput {
        let mut input = TextInput::new(200).with_default_text(text);
        input.set_focus(true);
        input
    }

    #[test]
    fn double_click_picks_the_word_under_the_cursor() {
        let input = input("hello big world");
        assert_eq!(input.word_at(2), (0, 5));
        assert_eq!(input.word_at(5), (5, 6));
        assert_eq!(input.word_at(7), (6, 9));
        assert_eq!(input.word_at(12), (10, 15));
    }

    #[test]
    fn typing_replaces_the_selection() {
        let mut input = input("hello");
        input.select_all();
        input.insert_char('x');
        assert_eq!(input.text(), "x");
        assert_eq!(input.cursor_pos, 1);
        assert_eq!(input.selection(), None);
    }

    #[test]
    fn backspace_removes_the_selection_only() {
        let mut input = input("hello world");
        input.anchor = Some(0);
        input.cursor_pos = 6;
        input.delete_before();
        assert_eq!(input.text(), "world");
        assert_eq!(input.cursor_pos, 0);
    }

    #[test]
    fn shift_arrows_extend_and_plain_arrows_collapse() {
        let mut input = input("abcdef");
        input.cursor_pos = 2;
        input.move_right(true);
        input.move_right(true);
        assert_eq!(input.selection(), Some((2, 4)));
        input.move_left(false);
        assert_eq!(input.selection(), None);
        assert_eq!(input.cursor_pos, 2);
    }

    #[test]
    fn ctrl_a_selects_everything() {
        let mut input = input("abc");
        input.handle_key(KEY_A, Modifiers::CTRL);
        assert_eq!(input.selection(), Some((0, 3)));
    }
}
