//! Text info dialog implementation for displaying text from files or stdin.

use std::io::Read;

use crate::{
    backend::{Window, WindowEvent, create_window},
    error::Error,
    render::{Canvas, Font, RingBufferCache, rgb},
    timing,
    ui::{
        Colors,
        widgets::{Widget, button::Button},
    },
};

const BASE_PADDING: u32 = 16;
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
            TextInfoResult::Closed => 255,
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
        let content = if let Some(ref filename) = self.filename {
            std::fs::read_to_string(filename).map_err(|e| Error::Io(e))?
        } else {
            let mut buf = String::new();
            std::io::stdin()
                .read_to_string(&mut buf)
                .map_err(|e| Error::Io(e))?;
            buf
        };

        let has_checkbox = self.checkbox_text.is_some();

        // Use provided dimensions or defaults
        let logical_width = self.width.unwrap_or(BASE_DEFAULT_WIDTH).max(BASE_MIN_WIDTH);
        let logical_height = self
            .height
            .unwrap_or(BASE_DEFAULT_HEIGHT)
            .max(BASE_MIN_HEIGHT);

        // Create window with LOGICAL dimensions
        let mut window = create_window(logical_width as u16, logical_height as u16)?;
        window.set_title(if self.title.is_empty() {
            "Text"
        } else {
            &self.title
        })?;

        // Get the actual scale factor from the window (compositor scale)
        let scale = window.scale_factor();

        // Now create everything at PHYSICAL scale
        let font = Font::load(scale);

        // Scale dimensions for physical rendering
        let padding = (BASE_PADDING as f32 * scale) as u32;
        let line_height = (BASE_LINE_HEIGHT as f32 * scale) as u32;
        let checkbox_size = (BASE_CHECKBOX_SIZE as f32 * scale) as u32;

        // Calculate physical dimensions
        let physical_width = (logical_width as f32 * scale) as u32;
        let physical_height = (logical_height as f32 * scale) as u32;

        // Create buttons at physical scale
        let mut ok_button = Button::new("OK", &font, scale);
        let mut cancel_button = Button::new("Cancel", &font, scale);

        // Layout calculation
        let button_height = (32.0 * scale) as u32;
        let checkbox_row_height = if has_checkbox {
            checkbox_size + (8.0 * scale) as u32
        } else {
            0
        };
        let button_y = (physical_height - padding - button_height) as i32;
        let checkbox_y = if has_checkbox {
            button_y - checkbox_row_height as i32 - (8.0 * scale) as i32
        } else {
            button_y
        };

        // Text area bounds
        let text_area_x = padding as i32;
        let text_area_y = padding as i32;
        let text_area_w = physical_width - padding * 2;
        let text_area_h = if has_checkbox {
            checkbox_y as u32 - padding - (8.0 * scale) as u32
        } else {
            button_y as u32 - padding - (8.0 * scale) as u32
        };

        // Calculate text wrapping - split content into wrapped lines
        let max_text_width = text_area_w - (16.0 * scale) as u32; // Account for scrollbar
        let mut wrapped_lines: Vec<String> = Vec::new();

        for line in content.lines() {
            if line.is_empty() {
                wrapped_lines.push(String::new());
            } else {
                // Wrap long lines
                let mut remaining = line;
                while !remaining.is_empty() {
                    let (line_w, _) = font.render(remaining).measure();
                    if line_w as u32 <= max_text_width {
                        wrapped_lines.push(remaining.to_string());
                        break;
                    }

                    // Find break point
                    let mut break_at = remaining.len();
                    for (i, _) in remaining.char_indices().rev() {
                        let test = &remaining[..i];
                        let (w, _) = font.render(test).measure();
                        if w as u32 <= max_text_width {
                            // Try to break at word boundary
                            if let Some(space_pos) = test.rfind(|c: char| c.is_whitespace()) {
                                break_at = space_pos + 1;
                            } else {
                                break_at = i;
                            }
                            break;
                        }
                    }

                    if break_at == 0 {
                        break_at = 1; // Ensure progress
                    }

                    wrapped_lines.push(remaining[..break_at].trim_end().to_string());
                    remaining = remaining[break_at..].trim_start();
                }
            }
        }

        let total_lines = wrapped_lines.len();
        let visible_lines = (text_area_h / line_height) as usize;

        // Create line canvas cache with capacity for visible lines + buffer
        let cache_capacity = visible_lines + 10;
        let mut line_cache: RingBufferCache<Canvas> = RingBufferCache::new(cache_capacity);

        // Cache invalidation function (call on window resize or theme change)
        // Note: Currently there are no resize/theme events in WindowEvent,
        // but when they are added, call line_cache.invalidate_all() to handle them
        let _invalidate_cache = || {
            line_cache.invalidate_all();
        };

        // Button positions (right-aligned)
        let mut bx = physical_width as i32 - padding as i32;
        bx -= cancel_button.width() as i32;
        cancel_button.set_position(bx, button_y);
        bx -= (10.0 * scale) as i32 + ok_button.width() as i32;
        ok_button.set_position(bx, button_y);

        // State
        let mut scroll_offset = 0usize;
        let mut checkbox_checked = false;
        let mut checkbox_hovered = false;

        // Create canvas at PHYSICAL dimensions
        let mut canvas = Canvas::new(physical_width, physical_height);

        // Draw function
        let draw = |canvas: &mut Canvas,
                    colors: &Colors,
                    font: &Font,
                    wrapped_lines: &[String],
                    scroll_offset: usize,
                    visible_lines: usize,
                    checkbox_text: &Option<String>,
                    checkbox_checked: bool,
                    checkbox_hovered: bool,
                    ok_button: &Button,
                    cancel_button: &Button,
                    line_cache: &mut RingBufferCache<Canvas>,
                    // Scaled parameters
                    padding: u32,
                    line_height: u32,
                    checkbox_size: u32,
                    text_area_x: i32,
                    text_area_y: i32,
                    text_area_w: u32,
                    text_area_h: u32,
                    checkbox_y: i32,
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

            // Text area background
            canvas.fill_rounded_rect(
                text_area_x as f32,
                text_area_y as f32,
                text_area_w as f32,
                text_area_h as f32,
                6.0 * scale,
                colors.input_bg,
            );

            // Draw visible lines
            let text_padding = (8.0 * scale) as i32;
            for (i, line_idx) in
                (scroll_offset..wrapped_lines.len().min(scroll_offset + visible_lines)).enumerate()
            {
                let line = &wrapped_lines[line_idx];
                if !line.is_empty() {
                    let y = text_area_y + text_padding + (i as u32 * line_height) as i32;

                    // Check cache for rendered line
                    if let Some(cached_canvas) = line_cache.get(&line_idx) {
                        // Use cached canvas
                        canvas.draw_canvas(cached_canvas, text_area_x + text_padding, y);
                    } else {
                        // Cache miss - render and store
                        let _timer = timing::Timer::new("glyph_raster_text_info_line");
                        let tc = font.render(line).with_color(colors.text).finish();
                        canvas.draw_canvas(&tc, text_area_x + text_padding, y);

                        // Store in cache (clone the canvas for caching)
                        line_cache.insert(line_idx, tc.clone());
                    }
                }
            }

            // Scrollbar
            if wrapped_lines.len() > visible_lines {
                let sb_x = text_area_x + text_area_w as i32 - (10.0 * scale) as i32;
                let sb_y = text_area_y as f32 + 4.0 * scale;
                let sb_h = text_area_h as f32 - 8.0 * scale;
                let thumb_h =
                    (visible_lines as f32 / wrapped_lines.len() as f32 * sb_h).max(20.0 * scale);
                let max_scroll = wrapped_lines.len().saturating_sub(visible_lines);
                let thumb_y = if max_scroll > 0 {
                    scroll_offset as f32 / max_scroll as f32 * (sb_h - thumb_h)
                } else {
                    0.0
                };

                // Track
                canvas.fill_rounded_rect(
                    sb_x as f32,
                    sb_y,
                    6.0 * scale,
                    sb_h,
                    3.0 * scale,
                    darken(colors.input_bg, 0.05),
                );
                // Thumb
                canvas.fill_rounded_rect(
                    sb_x as f32,
                    sb_y + thumb_y,
                    6.0 * scale,
                    thumb_h,
                    3.0 * scale,
                    colors.input_border,
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
                let _timer = timing::Timer::new("glyph_raster_text_info_checkbox");
                let tc = font.render(cb_text).with_color(colors.text).finish();
                canvas.draw_canvas(&tc, label_x, cb_y);
            }

            // Buttons
            ok_button.draw_to(canvas, colors, font);
            cancel_button.draw_to(canvas, colors, font);
        };

        // Initial draw
        draw(
            &mut canvas,
            colors,
            &font,
            &wrapped_lines,
            scroll_offset,
            visible_lines,
            &self.checkbox_text,
            checkbox_checked,
            checkbox_hovered,
            &ok_button,
            &cancel_button,
            &mut line_cache,
            padding,
            line_height,
            checkbox_size,
            text_area_x,
            text_area_y,
            text_area_w,
            text_area_h,
            checkbox_y,
            scale,
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
                WindowEvent::CursorMove(pos) => {
                    if has_checkbox {
                        let mx = pos.x as i32;
                        let my = pos.y as i32;

                        // Check if hovering checkbox area
                        let cb_x = padding as i32;
                        let cb_row_width = checkbox_size as i32 + (8.0 * scale) as i32 + 200; // Approximate label width
                        let old_hovered = checkbox_hovered;
                        checkbox_hovered = mx >= cb_x
                            && mx < cb_x + cb_row_width
                            && my >= checkbox_y
                            && my < checkbox_y + checkbox_size as i32;

                        if old_hovered != checkbox_hovered {
                            needs_redraw = true;
                        }
                    }
                }
                WindowEvent::ButtonPress(crate::backend::MouseButton::Left, _) => {
                    if checkbox_hovered {
                        checkbox_checked = !checkbox_checked;
                        needs_redraw = true;
                    }
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
                    const KEY_UP: u32 = 0xff52;
                    const KEY_DOWN: u32 = 0xff54;
                    const KEY_PAGE_UP: u32 = 0xff55;
                    const KEY_PAGE_DOWN: u32 = 0xff56;
                    const KEY_HOME: u32 = 0xff50;
                    const KEY_END: u32 = 0xff57;
                    const KEY_RETURN: u32 = 0xff0d;
                    const KEY_ESCAPE: u32 = 0xff1b;

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
                if let WindowEvent::CloseRequested = ev {
                    return Ok(TextInfoResult::Closed);
                }
                needs_redraw |= ok_button.process_event(&ev);
                needs_redraw |= cancel_button.process_event(&ev);
            }

            if needs_redraw {
                draw(
                    &mut canvas,
                    colors,
                    &font,
                    &wrapped_lines,
                    scroll_offset,
                    visible_lines,
                    &self.checkbox_text,
                    checkbox_checked,
                    checkbox_hovered,
                    &ok_button,
                    &cancel_button,
                    &mut line_cache,
                    padding,
                    line_height,
                    checkbox_size,
                    text_area_x,
                    text_area_y,
                    text_area_w,
                    text_area_h,
                    checkbox_y,
                    scale,
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

fn darken(color: crate::render::Rgba, amount: f32) -> crate::render::Rgba {
    rgb(
        (color.r as f32 * (1.0 - amount)) as u8,
        (color.g as f32 * (1.0 - amount)) as u8,
        (color.b as f32 * (1.0 - amount)) as u8,
    )
}
