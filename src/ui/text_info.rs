//! Text info dialog implementation for displaying text from files or stdin.

use std::{collections::HashMap, io::Read};

use crate::{
    backend::{Window, WindowEvent},
    error::Error,
    render::{Canvas, Font},
    ui::{
        BASE_BUTTON_HEIGHT, BASE_BUTTON_SPACING, BASE_CORNER_RADIUS, BASE_MIN_THUMB,
        BASE_TITLE_FONT_SIZE, Colors, KEY_DOWN, KEY_END, KEY_ESCAPE, KEY_HOME, KEY_PAGE_DOWN,
        KEY_PAGE_UP, KEY_RETURN, KEY_UP, Thumb, darken, open_window,
        widgets::{Widget, button::Button, point_in_rect, point_in_widget},
    },
};

const BASE_PADDING: u32 = 16;
const SCROLLBAR_INSET: f32 = 4.0;
const SCROLLBAR_HIT_WIDTH: f32 = 12.0;

/// Height of the scrollbar track inside a text area `area_h` tall.
fn scrollbar_track_h(area_h: u32, scale: f32) -> f32 {
    area_h as f32 - 2.0 * SCROLLBAR_INSET * scale
}

/// Left edge and width of the scrollbar hit target, relative to the text area.
fn scrollbar_hit_x(area_w: u32, scale: f32) -> (f32, f32) {
    let w = SCROLLBAR_HIT_WIDTH * scale;
    (area_w as f32 - w, w)
}
const BASE_LINE_HEIGHT: u32 = 20;
const BASE_CHECKBOX_SIZE: u32 = 16;
const BASE_MIN_WIDTH: u32 = 400;
const BASE_MIN_HEIGHT: u32 = 300;
const BASE_DEFAULT_WIDTH: u32 = 500;
const BASE_DEFAULT_HEIGHT: u32 = 400;

/// Text info dialog result.
#[derive(Debug, Clone)]
pub enum TextInfoResult {
    /// User clicked OK. Contains whether checkbox was checked (if present).
    Ok { checkbox_checked: bool },
    /// User cancelled the dialog.
    Cancelled,
    /// Dialog was closed.
    Closed,
}

impl TextInfoResult {
    pub fn exit_code(&self) -> i32 {
        match self {
            TextInfoResult::Ok {
                checkbox_checked,
            } => {
                if *checkbox_checked {
                    0
                } else {
                    1
                }
            }
            TextInfoResult::Cancelled => 1,
            TextInfoResult::Closed => 1,
        }
    }
}

/// Text info dialog builder.
pub struct TextInfoBuilder {
    title: String,
    filename: Option<String>,
    checkbox_text: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    colors: Option<&'static Colors>,
}

impl TextInfoBuilder {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            filename: None,
            checkbox_text: None,
            width: None,
            height: None,
            colors: None,
        }
    }

    pub fn title(mut self, title: &str) -> Self {
        self.title = title.to_string();
        self
    }

    /// Set the filename to read text from. If not set, reads from stdin.
    pub fn filename(mut self, filename: &str) -> Self {
        self.filename = Some(filename.to_string());
        self
    }

    /// Add a checkbox at the bottom (e.g., "I agree to the terms").
    pub fn checkbox(mut self, text: &str) -> Self {
        self.checkbox_text = Some(text.to_string());
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

    pub fn show(self) -> Result<TextInfoResult, Error> {
        let colors = self.colors.unwrap_or_else(|| crate::ui::detect_theme());

        // Read content from file or stdin
        let raw = if let Some(ref filename) = self.filename {
            std::fs::read(filename).map_err(Error::Io)?
        } else {
            let mut buf = Vec::new();
            std::io::stdin().read_to_end(&mut buf).map_err(Error::Io)?;
            buf
        };
        let content = String::from_utf8_lossy(&raw);

        let has_checkbox = self.checkbox_text.is_some();

        // Use provided dimensions or defaults
        let logical_width = self.width.unwrap_or(BASE_DEFAULT_WIDTH).max(BASE_MIN_WIDTH);
        let logical_height = self
            .height
            .unwrap_or(BASE_DEFAULT_HEIGHT)
            .max(BASE_MIN_HEIGHT);

        // Create window with LOGICAL dimensions
        let (mut window, scale, physical_width, physical_height) =
            open_window(&self.title, "Text", logical_width, logical_height)?;

        // Now create everything at PHYSICAL scale
        let font = Font::load(scale);

        // Scale dimensions for physical rendering
        let padding = (BASE_PADDING as f32 * scale) as u32;
        let line_height = (BASE_LINE_HEIGHT as f32 * scale) as u32;
        let checkbox_size = (BASE_CHECKBOX_SIZE as f32 * scale) as u32;

        // Create buttons at physical scale
        let mut ok_button = Button::new("OK", &font, scale);
        let mut cancel_button = Button::new("Cancel", &font, scale);

        // Layout calculation
        let title_height = if self.title.is_empty() {
            0
        } else {
            line_height + (8.0 * scale) as u32
        };
        let button_height = (BASE_BUTTON_HEIGHT as f32 * scale) as u32;
        let checkbox_row_height = if has_checkbox {
            checkbox_size + (8.0 * scale) as u32
        } else {
            0
        };
        let button_spacing = (24.0 * scale) as u32;
        let button_y = (physical_height - padding - button_height) as i32;
        let checkbox_y = if has_checkbox {
            button_y - checkbox_row_height as i32 - (8.0 * scale) as i32
        } else {
            button_y
        };

        // Text area bounds (with more spacing below it)
        let text_area_x = padding as i32;
        let text_area_y = padding as i32 + title_height as i32;
        let text_area_w = physical_width - padding * 2;
        let text_area_bottom = if has_checkbox {
            checkbox_y as u32 - button_spacing
        } else {
            button_y as u32 - button_spacing
        };
        let text_area_h = text_area_bottom - padding - (8.0 * scale) as u32;

        // Calculate text wrapping - split content into wrapped lines
        let max_text_width = text_area_w - (16.0 * scale) as u32; // Account for scrollbar
        let mut wrapped_lines: Vec<String> = Vec::new();

        for line in content.lines() {
            wrap_line(&font, line, max_text_width, &mut wrapped_lines);
        }

        let total_lines = wrapped_lines.len();
        let visible_lines = (text_area_h / line_height) as usize;

        // Button positions (right-aligned)
        let mut bx = physical_width as i32 - padding as i32;
        bx -= cancel_button.width() as i32;
        cancel_button.set_position(bx, button_y);
        bx -= (BASE_BUTTON_SPACING as f32 * scale) as i32 + ok_button.width() as i32;
        ok_button.set_position(bx, button_y);

        // State
        let mut scroll_offset = 0usize;
        let mut checkbox_checked = false;
        let mut checkbox_hovered = false;
        let mut scrollbar_hovered = false;

        // Create canvas at PHYSICAL dimensions
        let mut canvas = Canvas::new(physical_width, physical_height);

        // Pre-render the static chrome (bg + title + text-area) and each text
        // line ONCE into opaque canvases. Per-frame work then reduces to raw
        // byte copies (blit_region) instead of re-rasterizing the background and
        // dozens of text lines every scroll frame.
        let radius = BASE_CORNER_RADIUS * scale;
        let title_font_size = BASE_TITLE_FONT_SIZE * scale;
        let title_font = Font::load_with_size(title_font_size);
        let mut chrome_canvas = Canvas::new(physical_width, physical_height);
        chrome_canvas.fill_dialog_bg(
            physical_width as f32,
            physical_height as f32,
            colors.window_bg,
            colors.window_border,
            colors.window_shadow,
            radius,
        );
        if !self.title.is_empty() {
            let title_rendered = title_font
                .render(&self.title)
                .with_color(colors.text)
                .finish();
            let title_x = (physical_width as i32 - title_rendered.width() as i32) / 2;
            chrome_canvas.draw_canvas(&title_rendered, title_x, padding as i32);
        }
        chrome_canvas.fill_rounded_rect(
            text_area_x as f32,
            text_area_y as f32,
            text_area_w as f32,
            text_area_h as f32,
            6.0 * scale,
            colors.input_bg,
        );
        chrome_canvas.stroke_rounded_rect(
            text_area_x as f32,
            text_area_y as f32,
            text_area_w as f32,
            text_area_h as f32,
            6.0 * scale,
            colors.input_border,
            1.0,
        );
        // Lines are rasterized on demand and kept in a small cache so scrolling
        // stays a raw blit without pre-rendering the whole document up front.
        let mut line_cache: HashMap<usize, Canvas> = HashMap::new();

        // Draw function
        let draw = |canvas: &mut Canvas,
                    colors: &Colors,
                    font: &Font,
                    chrome: &Canvas,
                    line_cache: &mut HashMap<usize, Canvas>,
                    wrapped_lines: &[String],
                    scroll_offset: usize,
                    visible_lines: usize,
                    checkbox_text: &Option<String>,
                    checkbox_checked: bool,
                    checkbox_hovered: bool,
                    ok_button: &Button,
                    cancel_button: &Button,
                    // Scaled parameters
                    padding: u32,
                    line_height: u32,
                    checkbox_size: u32,
                    text_area_x: i32,
                    text_area_y: i32,
                    text_area_w: u32,
                    text_area_h: u32,
                    checkbox_y: i32,
                    scale: f32,
                    scrollbar_hovered: bool| {
            // Chrome (opaque) - raw byte copy, far faster than re-rasterizing the
            // full dialog background every frame.
            let cw = canvas.width();
            let ch = canvas.height();
            canvas.blit_region(chrome, 0, 0, cw, ch, 0, 0);

            // Visible text lines (opaque, cached) - raw copy each.
            let text_padding = (8.0 * scale) as i32;
            let visible = scroll_offset..wrapped_lines.len().min(scroll_offset + visible_lines);
            if line_cache.len() > visible_lines * 8 {
                line_cache.retain(|idx, _| visible.contains(idx));
            }
            for (i, line_idx) in visible.enumerate() {
                let lc = line_cache.entry(line_idx).or_insert_with(|| {
                    render_line(font, &wrapped_lines[line_idx], colors, line_height)
                });
                if lc.width() > 1 {
                    let y = text_area_y + text_padding + (i as u32 * line_height) as i32;
                    canvas.blit_region(
                        lc,
                        0,
                        0,
                        lc.width(),
                        lc.height(),
                        (text_area_x + text_padding) as u32,
                        y as u32,
                    );
                }
            }

            // Scrollbar
            if let Some(thumb) = Thumb::new(
                scrollbar_track_h(text_area_h, scale),
                visible_lines as f32,
                wrapped_lines.len() as f32,
                scroll_offset as f32,
                BASE_MIN_THUMB * scale,
            ) {
                let scrollbar_width = if scrollbar_hovered {
                    12.0 * scale
                } else {
                    8.0 * scale
                };
                let sb_x = text_area_x + text_area_w as i32 - scrollbar_width as i32;
                let sb_y = text_area_y as f32 + SCROLLBAR_INSET * scale;

                // Track
                canvas.fill_rounded_rect(
                    sb_x as f32,
                    sb_y,
                    scrollbar_width - 2.0 * scale,
                    scrollbar_track_h(text_area_h, scale),
                    3.0 * scale,
                    darken(colors.input_bg, 0.05),
                );
                // Thumb
                canvas.fill_rounded_rect(
                    sb_x as f32,
                    sb_y + thumb.offset,
                    scrollbar_width - 2.0 * scale,
                    thumb.len,
                    3.0 * scale,
                    if scrollbar_hovered {
                        colors.input_border_focused
                    } else {
                        colors.input_border
                    },
                );
            }

            // Border
            canvas.stroke_rounded_rect(
                text_area_x as f32,
                text_area_y as f32,
                text_area_w as f32,
                text_area_h as f32,
                6.0 * scale,
                colors.input_border,
                1.0,
            );

            // Checkbox
            if let Some(cb_text) = checkbox_text {
                let cb_x = padding as i32;
                let cb_y = checkbox_y;

                // Checkbox box
                let cb_bg = if checkbox_hovered {
                    darken(colors.input_bg, 0.06)
                } else {
                    colors.input_bg
                };
                canvas.fill_rounded_rect(
                    cb_x as f32,
                    cb_y as f32,
                    checkbox_size as f32,
                    checkbox_size as f32,
                    3.0 * scale,
                    cb_bg,
                );
                canvas.stroke_rounded_rect(
                    cb_x as f32,
                    cb_y as f32,
                    checkbox_size as f32,
                    checkbox_size as f32,
                    3.0 * scale,
                    colors.input_border,
                    1.0,
                );

                // Check mark
                if checkbox_checked {
                    let inset = (3.0 * scale) as i32;
                    canvas.fill_rounded_rect(
                        (cb_x + inset) as f32,
                        (cb_y + inset) as f32,
                        (checkbox_size as i32 - inset * 2) as f32,
                        (checkbox_size as i32 - inset * 2) as f32,
                        2.0 * scale,
                        colors.input_border_focused,
                    );
                }

                // Label
                let label_x = cb_x + checkbox_size as i32 + (8.0 * scale) as i32;
                let tc = font.render(cb_text).with_color(colors.text).finish();
                canvas.draw_canvas(&tc, label_x, cb_y);
            }

            // Buttons
            ok_button.draw_to(canvas, colors, font);
            cancel_button.draw_to(canvas, colors, font);
        };

        let mut window_dragging = false;

        // Scrollbar thumb dragging state
        let mut thumb_drag = false;
        let mut thumb_drag_offset: Option<i32> = None;
        let mut last_cursor_pos: Option<(i32, i32)> = None;
        let mut clicking_scrollbar: bool;

        // Initial draw
        draw(
            &mut canvas,
            colors,
            &font,
            &chrome_canvas,
            &mut line_cache,
            &wrapped_lines,
            scroll_offset,
            visible_lines,
            &self.checkbox_text,
            checkbox_checked,
            checkbox_hovered,
            &ok_button,
            &cancel_button,
            padding,
            line_height,
            checkbox_size,
            text_area_x,
            text_area_y,
            text_area_w,
            text_area_h,
            checkbox_y,
            scale,
            scrollbar_hovered,
        );
        window.set_contents(&canvas)?;
        window.show()?;

        // Event loop
        loop {
            let event = window.wait_for_event()?;
            let mut needs_redraw = false;

            match &event {
                WindowEvent::CloseRequested => return Ok(TextInfoResult::Closed),
                WindowEvent::RedrawRequested => needs_redraw = true,
                WindowEvent::CursorEnter(pos) | WindowEvent::CursorMove(pos) => {
                    if window_dragging {
                        let _ = window.start_drag();
                        window_dragging = false;
                    }

                    let mx = pos.x as i32;
                    let my = pos.y as i32;

                    // Store current cursor position
                    last_cursor_pos = Some((mx, my));

                    // Handle scrollbar thumb dragging
                    let track = scrollbar_track_h(text_area_h, scale);
                    if thumb_drag
                        && let Some(thumb) = Thumb::new(
                            track,
                            visible_lines as f32,
                            total_lines as f32,
                            scroll_offset as f32,
                            BASE_MIN_THUMB * scale,
                        )
                    {
                        let max_scroll = total_lines - visible_lines;
                        let grab = thumb_drag_offset.unwrap_or((thumb.len / 2.0) as i32) as f32;
                        let pos = (my - text_area_y) as f32 - SCROLLBAR_INSET * scale - grab;
                        scroll_offset = (thumb.scroll_at(track, pos, max_scroll as f32) as usize)
                            .min(max_scroll);
                        needs_redraw = true;
                    } else {
                        // Update scrollbar hover state (always, not just when there's a checkbox)
                        let scrollbar_width = if scrollbar_hovered {
                            12.0 * scale
                        } else {
                            8.0 * scale
                        };
                        let scrollbar_x = text_area_x + text_area_w as i32 - scrollbar_width as i32;

                        scrollbar_hovered = total_lines > visible_lines
                            && mx >= scrollbar_x
                            && mx < text_area_x + text_area_w as i32
                            && my >= text_area_y
                            && my < text_area_y + text_area_h as i32;

                        if has_checkbox {
                            // Check if hovering checkbox area (only if not over scrollbar)
                            let cb_x = padding as i32;
                            let cb_row_width = checkbox_size as i32 + (8.0 * scale) as i32 + 200; // Approximate label width
                            let old_hovered = checkbox_hovered;
                            checkbox_hovered = !scrollbar_hovered
                                && mx >= cb_x
                                && mx < cb_x + cb_row_width
                                && my >= checkbox_y
                                && my < checkbox_y + checkbox_size as i32;

                            if old_hovered != checkbox_hovered {
                                needs_redraw = true;
                            }
                        }
                    }
                }
                WindowEvent::ButtonPress(crate::backend::MouseButton::Left, _) => {
                    // Dragging the background moves the window; dragging the text area,
                    // checkbox or a button belongs to the dialog
                    let (cx, cy) = last_cursor_pos.unwrap_or((i32::MIN, i32::MIN));
                    window_dragging =
                        !point_in_rect(cx, cy, text_area_x, text_area_y, text_area_w, text_area_h)
                            && !point_in_widget(cx, cy, &ok_button)
                            && !point_in_widget(cx, cy, &cancel_button)
                            && !(has_checkbox
                                && point_in_rect(
                                    cx,
                                    cy,
                                    text_area_x,
                                    checkbox_y,
                                    text_area_w,
                                    checkbox_size,
                                ));

                    clicking_scrollbar = false;

                    // Check if clicking anywhere in scrollbar area (thumb OR track)
                    if let Some((mx, my)) = last_cursor_pos
                        && total_lines > visible_lines
                    {
                        let scrollbar_width = if scrollbar_hovered {
                            12.0 * scale
                        } else {
                            8.0 * scale
                        };
                        let scrollbar_x = text_area_x + text_area_w as i32 - scrollbar_width as i32;

                        // Block all clicks in scrollbar area
                        if mx >= scrollbar_x
                            && mx < text_area_x + text_area_w as i32
                            && my >= text_area_y
                            && my < text_area_y + text_area_h as i32
                        {
                            clicking_scrollbar = true;

                            // Now check if clicking specifically on the thumb for dragging
                            let text_area_mx = mx - text_area_x;
                            let text_area_my = my - text_area_y;

                            let (sb_x, sb_w) = scrollbar_hit_x(text_area_w, scale);
                            let inset = SCROLLBAR_INSET * scale;

                            if let Some(thumb) = Thumb::new(
                                scrollbar_track_h(text_area_h, scale),
                                visible_lines as f32,
                                total_lines as f32,
                                scroll_offset as f32,
                                BASE_MIN_THUMB * scale,
                            ) && (text_area_mx as f32) >= sb_x
                                && (text_area_mx as f32) < sb_x + sb_w
                                && thumb.contains(text_area_my as f32 - inset)
                            {
                                thumb_drag = true;
                                thumb_drag_offset =
                                    Some(text_area_my - (inset + thumb.offset) as i32);
                            }
                        }
                    }

                    // Only process checkbox click if not clicking on scrollbar
                    if !clicking_scrollbar && checkbox_hovered {
                        checkbox_checked = !checkbox_checked;
                        needs_redraw = true;
                    }
                }
                WindowEvent::ButtonRelease(_, _) => {
                    window_dragging = false;
                    thumb_drag = false;
                    thumb_drag_offset = None;
                }
                WindowEvent::Scroll(direction) => {
                    match direction {
                        crate::backend::ScrollDirection::Up => {
                            if scroll_offset > 0 {
                                scroll_offset = scroll_offset.saturating_sub(3);
                                needs_redraw = true;
                            }
                        }
                        crate::backend::ScrollDirection::Down => {
                            let max_scroll = total_lines.saturating_sub(visible_lines);
                            if scroll_offset < max_scroll {
                                scroll_offset = (scroll_offset + 3).min(max_scroll);
                                needs_redraw = true;
                            }
                        }
                        _ => {}
                    }
                }
                WindowEvent::TextInput(c) => {
                    // Handle space for checkbox toggle (TextInput is sent for printable chars)
                    if *c == ' ' && has_checkbox {
                        checkbox_checked = !checkbox_checked;
                        needs_redraw = true;
                    }
                }
                WindowEvent::KeyPress(key_event) => {
                    let max_scroll = total_lines.saturating_sub(visible_lines);

                    match key_event.keysym {
                        KEY_UP => {
                            if scroll_offset > 0 {
                                scroll_offset = scroll_offset.saturating_sub(1);
                                needs_redraw = true;
                            }
                        }
                        KEY_DOWN => {
                            if scroll_offset < max_scroll {
                                scroll_offset = (scroll_offset + 1).min(max_scroll);
                                needs_redraw = true;
                            }
                        }
                        KEY_PAGE_UP => {
                            scroll_offset = scroll_offset.saturating_sub(visible_lines);
                            needs_redraw = true;
                        }
                        KEY_PAGE_DOWN => {
                            scroll_offset = (scroll_offset + visible_lines).min(max_scroll);
                            needs_redraw = true;
                        }
                        KEY_HOME => {
                            if scroll_offset > 0 {
                                scroll_offset = 0;
                                needs_redraw = true;
                            }
                        }
                        KEY_END => {
                            if scroll_offset < max_scroll {
                                scroll_offset = max_scroll;
                                needs_redraw = true;
                            }
                        }
                        KEY_RETURN => {
                            return Ok(TextInfoResult::Ok {
                                checkbox_checked,
                            });
                        }
                        KEY_ESCAPE => {
                            return Ok(TextInfoResult::Cancelled);
                        }
                        _ => {}
                    }
                }
                _ => {}
            }

            needs_redraw |= ok_button.process_event(&event);
            needs_redraw |= cancel_button.process_event(&event);

            if ok_button.was_clicked() {
                return Ok(TextInfoResult::Ok {
                    checkbox_checked,
                });
            }
            if cancel_button.was_clicked() {
                return Ok(TextInfoResult::Cancelled);
            }

            // Batch process pending events
            while let Some(ev) = window.poll_for_event()? {
                match &ev {
                    WindowEvent::CloseRequested => {
                        return Ok(TextInfoResult::Closed);
                    }
                    WindowEvent::CursorEnter(pos) | WindowEvent::CursorMove(pos) => {
                        last_cursor_pos = Some((pos.x as i32, pos.y as i32));
                    }
                    WindowEvent::ButtonPress(button, _modifiers)
                        if *button == crate::backend::MouseButton::Left =>
                    {
                        if let Some((mx, my)) = last_cursor_pos
                            && total_lines > visible_lines
                        {
                            let (sb_x, sb_w) = scrollbar_hit_x(text_area_w, scale);
                            let inset = SCROLLBAR_INSET * scale;
                            let local_x = (mx - text_area_x) as f32;
                            let local_y = (my - text_area_y) as f32;

                            if let Some(thumb) = Thumb::new(
                                scrollbar_track_h(text_area_h, scale),
                                visible_lines as f32,
                                total_lines as f32,
                                scroll_offset as f32,
                                BASE_MIN_THUMB * scale,
                            ) && local_x >= sb_x
                                && local_x < sb_x + sb_w
                                && thumb.contains(local_y - inset)
                            {
                                thumb_drag = true;
                                thumb_drag_offset =
                                    Some(my - text_area_y - (inset + thumb.offset) as i32);
                            }
                        }
                    }
                    WindowEvent::ButtonRelease(_, _) => {
                        thumb_drag = false;
                        thumb_drag_offset = None;
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
                    &chrome_canvas,
                    &mut line_cache,
                    &wrapped_lines,
                    scroll_offset,
                    visible_lines,
                    &self.checkbox_text,
                    checkbox_checked,
                    checkbox_hovered,
                    &ok_button,
                    &cancel_button,
                    padding,
                    line_height,
                    checkbox_size,
                    text_area_x,
                    text_area_y,
                    text_area_w,
                    text_area_h,
                    checkbox_y,
                    scale,
                    scrollbar_hovered,
                );
                window.set_contents(&canvas)?;
            }
        }
    }
}

impl Default for TextInfoBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Rasterizes one wrapped line into an opaque canvas, or a 1x1 placeholder
/// for an empty line.
fn render_line(font: &Font, line: &str, colors: &Colors, line_height: u32) -> Canvas {
    if line.is_empty() {
        return Canvas::new(1, 1);
    }
    let tc = font.render(line).with_color(colors.text).finish();
    let mut lc = Canvas::new(tc.width().max(1), line_height);
    lc.fill(colors.input_bg);
    lc.draw_canvas(&tc, 0, 0);
    lc
}

/// Splits one logical line into pieces no wider than `max_width` pixels,
/// breaking after the last whitespace that fits and hard-breaking otherwise.
///
/// Each piece is found with an exponential probe followed by a binary search
/// over character counts, so the number of measurements per piece is
/// logarithmic in its length rather than linear.
fn wrap_line(font: &Font, line: &str, max_width: u32, out: &mut Vec<String>) {
    if line.is_empty() {
        out.push(String::new());
        return;
    }

    let fits = |s: &str| font.render(s).measure().0 as u32 <= max_width;
    let prefix_end =
        |s: &str, chars: usize| s.char_indices().nth(chars).map_or(s.len(), |(i, _)| i);

    let mut remaining = line;
    while !remaining.is_empty() {
        let mut lo = 0;
        let mut hi = 1;
        loop {
            let end = prefix_end(remaining, hi);
            if !fits(&remaining[..end]) {
                break;
            }
            if end == remaining.len() {
                out.push(remaining.to_string());
                return;
            }
            lo = hi;
            hi *= 2;
        }
        while hi - lo > 1 {
            let mid = lo + (hi - lo) / 2;
            if fits(&remaining[..prefix_end(remaining, mid)]) {
                lo = mid;
            } else {
                hi = mid;
            }
        }

        let fit_end = prefix_end(remaining, lo);
        let mut break_at = remaining[..fit_end]
            .char_indices()
            .rev()
            .find(|(_, c)| c.is_whitespace())
            .map_or(fit_end, |(i, c)| i + c.len_utf8());
        if break_at == 0 {
            break_at = prefix_end(remaining, 1);
        }

        out.push(remaining[..break_at].trim_end().to_string());
        remaining = remaining[break_at..].trim_start();
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    #[test]
    fn wraps_long_single_line_quickly() {
        let font = Font::load(1.0);
        let max_width = 400;
        let mut line = "lorem ipsum dolor ".repeat(300);
        line.push_str(&"x".repeat(2_000));

        let start = Instant::now();
        let mut pieces = Vec::new();
        wrap_line(&font, &line, max_width, &mut pieces);
        let elapsed = start.elapsed();

        assert!(pieces.len() > 20);
        for piece in &pieces {
            assert!(!piece.is_empty());
            assert!(
                font.render(piece).measure().0 as u32 <= max_width,
                "{piece:?}"
            );
        }
        let non_ws = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
        assert_eq!(non_ws(&pieces.concat()), non_ws(&line));
        assert!(
            elapsed < Duration::from_secs(5),
            "wrapping took {elapsed:?}"
        );
    }
}
