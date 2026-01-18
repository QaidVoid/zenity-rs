//! List selection dialog implementation.

use crate::backend::{Window, WindowEvent, MouseButton, create_window};
use crate::error::Error;
use crate::render::{Canvas, Font, rgb};
use crate::ui::Colors;
use crate::ui::widgets::Widget;
use crate::ui::widgets::button::Button;

const PADDING: u32 = 16;
const ROW_HEIGHT: u32 = 28;
const CHECKBOX_SIZE: u32 = 16;
const MIN_WIDTH: u32 = 350;
const MAX_WIDTH: u32 = 600;
const MIN_HEIGHT: u32 = 200;
const MAX_HEIGHT: u32 = 450;

/// List dialog result.
#[derive(Debug, Clone)]
pub enum ListResult {
    /// User selected item(s). Contains the values from the first column.
    Selected(Vec<String>),
    /// User cancelled.
    Cancelled,
    /// Dialog was closed.
    Closed,
}

impl ListResult {
    pub fn exit_code(&self) -> i32 {
        match self {
            ListResult::Selected(_) => 0,
            ListResult::Cancelled => 1,
            ListResult::Closed => 255,
        }
    }
}

/// List selection mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListMode {
    /// Single selection (default).
    Single,
    /// Multiple selection with checkboxes.
    Checklist,
    /// Multiple selection with radio buttons (single select visually).
    Radiolist,
}

/// List dialog builder.
pub struct ListBuilder {
    title: String,
    text: String,
    columns: Vec<String>,
    rows: Vec<Vec<String>>,
    mode: ListMode,
    colors: Option<&'static Colors>,
}

impl ListBuilder {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            text: String::new(),
            columns: Vec::new(),
            rows: Vec::new(),
            mode: ListMode::Single,
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

    /// Add a column header.
    pub fn column(mut self, name: &str) -> Self {
        self.columns.push(name.to_string());
        self
    }

    /// Add a row of data.
    pub fn row(mut self, values: Vec<String>) -> Self {
        self.rows.push(values);
        self
    }

    /// Set selection mode.
    pub fn mode(mut self, mode: ListMode) -> Self {
        self.mode = mode;
        self
    }

    /// Enable checklist mode (multi-select with checkboxes).
    pub fn checklist(mut self) -> Self {
        self.mode = ListMode::Checklist;
        self
    }

    /// Enable radiolist mode (single-select with radio buttons).
    pub fn radiolist(mut self) -> Self {
        self.mode = ListMode::Radiolist;
        self
    }

    pub fn colors(mut self, colors: &'static Colors) -> Self {
        self.colors = Some(colors);
        self
    }

    pub fn show(self) -> Result<ListResult, Error> {
        let colors = self.colors.unwrap_or_else(|| crate::ui::detect_theme());
        let font = Font::load();

        // Process rows - for checklist/radiolist, first column is TRUE/FALSE
        let (rows, mut selected): (Vec<Vec<String>>, Vec<bool>) = if self.mode != ListMode::Single {
            let mut processed_rows = Vec::new();
            let mut selections = Vec::new();

            for row in &self.rows {
                if !row.is_empty() {
                    let is_selected = row[0].eq_ignore_ascii_case("true");
                    selections.push(is_selected);
                    processed_rows.push(row[1..].to_vec());
                }
            }
            (processed_rows, selections)
        } else {
            (self.rows.clone(), vec![false; self.rows.len()])
        };

        // Columns - skip first column header for checklist/radiolist
        let columns: Vec<&str> = if self.mode != ListMode::Single && !self.columns.is_empty() {
            self.columns[1..].iter().map(|s| s.as_str()).collect()
        } else {
            self.columns.iter().map(|s| s.as_str()).collect()
        };

        let num_cols = columns.len().max(1);
        let num_rows = rows.len();

        // Calculate column widths
        let mut col_widths: Vec<u32> = vec![100; num_cols];

        // Measure header widths
        for (i, col) in columns.iter().enumerate() {
            let (w, _) = font.render(col).measure();
            col_widths[i] = col_widths[i].max(w as u32 + 20);
        }

        // Measure data widths
        for row in &rows {
            for (i, cell) in row.iter().enumerate() {
                if i < num_cols {
                    let (w, _) = font.render(cell).measure();
                    col_widths[i] = col_widths[i].max(w as u32 + 20);
                }
            }
        }

        // Calculate total width
        let checkbox_col = if self.mode != ListMode::Single { CHECKBOX_SIZE + 16 } else { 0 };
        let content_width: u32 = col_widths.iter().sum::<u32>() + checkbox_col;
        let width = (content_width + PADDING * 2).clamp(MIN_WIDTH, MAX_WIDTH);

        // Calculate height
        let text_height = if self.text.is_empty() { 0 } else { 24 };
        let header_height = if columns.is_empty() { 0 } else { ROW_HEIGHT };
        let list_height = (num_rows as u32 * ROW_HEIGHT).clamp(ROW_HEIGHT * 3, MAX_HEIGHT - 100);
        let height = (PADDING * 2 + text_height + header_height + list_height + 50).clamp(MIN_HEIGHT, MAX_HEIGHT);

        // Create buttons
        let mut ok_button = Button::new("OK", &font);
        let mut cancel_button = Button::new("Cancel", &font);

        // Create window
        let mut window = create_window(width as u16, height as u16)?;
        window.set_title(if self.title.is_empty() { "Select" } else { &self.title })?;

        // Layout
        let mut y = PADDING as i32;

        let text_y = y;
        if !self.text.is_empty() {
            y += text_height as i32 + 8;
        }

        let list_x = PADDING as i32;
        let list_y = y;
        let list_w = width - PADDING * 2;
        let list_h = list_height;
        let visible_rows = (list_h / ROW_HEIGHT) as usize;

        let button_y = (height - PADDING - 32) as i32;
        let mut bx = width as i32 - PADDING as i32;
        bx -= cancel_button.width() as i32;
        cancel_button.set_position(bx, button_y);
        bx -= 10 + ok_button.width() as i32;
        ok_button.set_position(bx, button_y);

        let mut canvas = Canvas::new(width, height);
        let mut mouse_x = 0i32;
        let mut mouse_y = 0i32;
        let mut scroll_offset = 0usize;
        let mut hovered_row: Option<usize> = None;
        let mut single_selected: Option<usize> = None;

        // Draw function
        let draw = |canvas: &mut Canvas,
                    colors: &Colors,
                    font: &Font,
                    text: &str,
                    columns: &[&str],
                    rows: &[Vec<String>],
                    col_widths: &[u32],
                    selected: &[bool],
                    single_selected: Option<usize>,
                    scroll_offset: usize,
                    hovered_row: Option<usize>,
                    mode: ListMode,
                    ok_button: &Button,
                    cancel_button: &Button| {
            canvas.fill(colors.window_bg);

            // Draw text prompt
            if !text.is_empty() {
                let tc = font.render(text).with_color(colors.text).finish();
                canvas.draw_canvas(&tc, PADDING as i32, text_y);
            }

            // List background
            canvas.fill_rounded_rect(
                list_x as f32, list_y as f32,
                list_w as f32, list_h as f32,
                6.0, colors.input_bg,
            );

            // Draw header if columns exist
            let mut data_y = list_y;
            if !columns.is_empty() {
                let header_bg = darken(colors.input_bg, 0.05);
                canvas.fill_rect(
                    list_x as f32, list_y as f32,
                    list_w as f32, ROW_HEIGHT as f32,
                    header_bg,
                );

                let mut cx = list_x + checkbox_col as i32;
                for (i, col) in columns.iter().enumerate() {
                    let tc = font.render(col).with_color(rgb(140, 140, 140)).finish();
                    canvas.draw_canvas(&tc, cx + 8, list_y + 6);
                    cx += col_widths.get(i).copied().unwrap_or(100) as i32;
                }

                // Separator
                canvas.fill_rect(
                    list_x as f32, (list_y + ROW_HEIGHT as i32) as f32,
                    list_w as f32, 1.0,
                    colors.input_border,
                );
                data_y += ROW_HEIGHT as i32 + 1;
            }

            // Draw rows
            let data_visible = if columns.is_empty() { visible_rows } else { visible_rows.saturating_sub(1) };
            for (vi, ri) in (scroll_offset..rows.len().min(scroll_offset + data_visible)).enumerate() {
                let row = &rows[ri];
                let ry = data_y + (vi as u32 * ROW_HEIGHT) as i32;

                // Background
                let is_hovered = hovered_row == Some(ri);
                let is_selected = match mode {
                    ListMode::Single => single_selected == Some(ri),
                    _ => selected.get(ri).copied().unwrap_or(false),
                };

                let bg = if is_selected {
                    colors.input_border_focused
                } else if is_hovered {
                    darken(colors.input_bg, 0.06)
                } else if vi % 2 == 1 {
                    darken(colors.input_bg, 0.02)
                } else {
                    colors.input_bg
                };

                canvas.fill_rect(
                    (list_x + 1) as f32, ry as f32,
                    (list_w - 2) as f32, ROW_HEIGHT as f32,
                    bg,
                );

                // Checkbox/Radio
                if mode != ListMode::Single {
                    let check_x = list_x + 8;
                    let check_y = ry + ((ROW_HEIGHT - CHECKBOX_SIZE) / 2) as i32;
                    let checked = selected.get(ri).copied().unwrap_or(false);

                    if mode == ListMode::Checklist {
                        draw_checkbox(canvas, check_x, check_y, checked, colors);
                    } else {
                        draw_radio(canvas, check_x, check_y, checked, colors);
                    }
                }

                // Cell values
                let mut cx = list_x + checkbox_col as i32;
                for (ci, cell) in row.iter().enumerate() {
                    if ci < col_widths.len() {
                        let text_color = if is_selected { rgb(255, 255, 255) } else { colors.text };
                        let tc = font.render(cell).with_color(text_color).finish();
                        canvas.draw_canvas(&tc, cx + 8, ry + 6);
                        cx += col_widths[ci] as i32;
                    }
                }
            }

            // Scrollbar
            if rows.len() > data_visible {
                let sb_x = list_x + list_w as i32 - 8;
                let sb_h = list_h as f32 - if columns.is_empty() { 0.0 } else { ROW_HEIGHT as f32 + 1.0 };
                let sb_y = data_y as f32;
                let thumb_h = (data_visible as f32 / rows.len() as f32 * sb_h).max(20.0);
                let thumb_y = scroll_offset as f32 / rows.len() as f32 * sb_h;

                canvas.fill_rounded_rect(sb_x as f32, sb_y, 6.0, sb_h, 3.0, darken(colors.input_bg, 0.05));
                canvas.fill_rounded_rect(sb_x as f32, sb_y + thumb_y, 6.0, thumb_h, 3.0, colors.input_border);
            }

            // Border
            canvas.stroke_rounded_rect(
                list_x as f32, list_y as f32,
                list_w as f32, list_h as f32,
                6.0, colors.input_border, 1.0,
            );

            // Buttons
            ok_button.draw_to(canvas, colors, font);
            cancel_button.draw_to(canvas, colors, font);
        };

        // Initial draw
        draw(
            &mut canvas, colors, &font, &self.text, &columns, &rows, &col_widths,
            &selected, single_selected, scroll_offset, hovered_row, self.mode,
            &ok_button, &cancel_button,
        );
        window.set_contents(&canvas)?;
        window.show()?;

        let header_height_px = if columns.is_empty() { 0 } else { ROW_HEIGHT + 1 };
        let data_y = list_y + header_height_px as i32;
        let data_visible = if columns.is_empty() { visible_rows } else { visible_rows.saturating_sub(1) };

        loop {
            let event = window.wait_for_event()?;
            let mut needs_redraw = false;

            match &event {
                WindowEvent::CloseRequested => return Ok(ListResult::Closed),
                WindowEvent::RedrawRequested => needs_redraw = true,
                WindowEvent::CursorMove(pos) => {
                    mouse_x = pos.x as i32;
                    mouse_y = pos.y as i32;

                    let old_hovered = hovered_row;
                    hovered_row = None;

                    if mouse_x >= list_x && mouse_x < list_x + list_w as i32
                        && mouse_y >= data_y && mouse_y < list_y + list_h as i32
                    {
                        let rel_y = (mouse_y - data_y) as usize;
                        let ri = scroll_offset + rel_y / ROW_HEIGHT as usize;
                        if ri < rows.len() {
                            hovered_row = Some(ri);
                        }
                    }

                    if old_hovered != hovered_row {
                        needs_redraw = true;
                    }
                }
                WindowEvent::ButtonPress(MouseButton::Left) => {
                    if let Some(ri) = hovered_row {
                        match self.mode {
                            ListMode::Single => {
                                single_selected = Some(ri);
                            }
                            ListMode::Checklist => {
                                if let Some(sel) = selected.get_mut(ri) {
                                    *sel = !*sel;
                                }
                            }
                            ListMode::Radiolist => {
                                // Only one can be selected
                                for s in selected.iter_mut() {
                                    *s = false;
                                }
                                if let Some(sel) = selected.get_mut(ri) {
                                    *sel = true;
                                }
                            }
                        }
                        needs_redraw = true;
                    }
                }
                WindowEvent::Scroll(direction) => {
                    match direction {
                        crate::backend::ScrollDirection::Up => {
                            if scroll_offset > 0 {
                                scroll_offset = scroll_offset.saturating_sub(2);
                                needs_redraw = true;
                            }
                        }
                        crate::backend::ScrollDirection::Down => {
                            if scroll_offset + data_visible < rows.len() {
                                scroll_offset = (scroll_offset + 2).min(rows.len().saturating_sub(data_visible));
                                needs_redraw = true;
                            }
                        }
                        _ => {}
                    }
                }
                WindowEvent::KeyPress(key_event) => {
                    const KEY_UP: u32 = 0xff52;
                    const KEY_DOWN: u32 = 0xff54;
                    const KEY_SPACE: u32 = 0x20;
                    const KEY_RETURN: u32 = 0xff0d;
                    const KEY_ESCAPE: u32 = 0xff1b;

                    match key_event.keysym {
                        KEY_UP => {
                            if self.mode == ListMode::Single {
                                if let Some(sel) = single_selected {
                                    if sel > 0 {
                                        single_selected = Some(sel - 1);
                                        if sel - 1 < scroll_offset {
                                            scroll_offset = sel - 1;
                                        }
                                        needs_redraw = true;
                                    }
                                } else if !rows.is_empty() {
                                    single_selected = Some(0);
                                    needs_redraw = true;
                                }
                            }
                        }
                        KEY_DOWN => {
                            if self.mode == ListMode::Single {
                                if let Some(sel) = single_selected {
                                    if sel + 1 < rows.len() {
                                        single_selected = Some(sel + 1);
                                        if sel + 1 >= scroll_offset + data_visible {
                                            scroll_offset = sel + 2 - data_visible;
                                        }
                                        needs_redraw = true;
                                    }
                                } else if !rows.is_empty() {
                                    single_selected = Some(0);
                                    needs_redraw = true;
                                }
                            }
                        }
                        KEY_SPACE => {
                            if self.mode == ListMode::Checklist {
                                if let Some(ri) = hovered_row.or(single_selected) {
                                    if let Some(sel) = selected.get_mut(ri) {
                                        *sel = !*sel;
                                        needs_redraw = true;
                                    }
                                }
                            }
                        }
                        KEY_RETURN => {
                            // Return selected
                            return Ok(get_result(&rows, &selected, single_selected, self.mode));
                        }
                        KEY_ESCAPE => {
                            return Ok(ListResult::Cancelled);
                        }
                        _ => {}
                    }
                }
                _ => {}
            }

            needs_redraw |= ok_button.process_event(&event);
            needs_redraw |= cancel_button.process_event(&event);

            if ok_button.was_clicked() {
                return Ok(get_result(&rows, &selected, single_selected, self.mode));
            }
            if cancel_button.was_clicked() {
                return Ok(ListResult::Cancelled);
            }

            while let Some(ev) = window.poll_for_event()? {
                if let WindowEvent::CloseRequested = ev {
                    return Ok(ListResult::Closed);
                }
                if let WindowEvent::CursorMove(pos) = ev {
                    mouse_x = pos.x as i32;
                    mouse_y = pos.y as i32;
                }
                needs_redraw |= ok_button.process_event(&ev);
                needs_redraw |= cancel_button.process_event(&ev);
            }

            if needs_redraw {
                draw(
                    &mut canvas, colors, &font, &self.text, &columns, &rows, &col_widths,
                    &selected, single_selected, scroll_offset, hovered_row, self.mode,
                    &ok_button, &cancel_button,
                );
                window.set_contents(&canvas)?;
            }
        }
    }
}

impl Default for ListBuilder {
    fn default() -> Self {
        Self::new()
    }
}

fn get_result(rows: &[Vec<String>], selected: &[bool], single_selected: Option<usize>, mode: ListMode) -> ListResult {
    let mut result = Vec::new();

    match mode {
        ListMode::Single => {
            if let Some(idx) = single_selected {
                if let Some(row) = rows.get(idx) {
                    if let Some(val) = row.first() {
                        result.push(val.clone());
                    }
                }
            }
        }
        _ => {
            for (i, &sel) in selected.iter().enumerate() {
                if sel {
                    if let Some(row) = rows.get(i) {
                        if let Some(val) = row.first() {
                            result.push(val.clone());
                        }
                    }
                }
            }
        }
    }

    if result.is_empty() {
        ListResult::Cancelled
    } else {
        ListResult::Selected(result)
    }
}

fn darken(color: crate::render::Rgba, amount: f32) -> crate::render::Rgba {
    rgb(
        (color.r as f32 * (1.0 - amount)) as u8,
        (color.g as f32 * (1.0 - amount)) as u8,
        (color.b as f32 * (1.0 - amount)) as u8,
    )
}

fn draw_checkbox(canvas: &mut Canvas, x: i32, y: i32, checked: bool, colors: &Colors) {
    // Box
    canvas.fill_rounded_rect(
        x as f32, y as f32,
        CHECKBOX_SIZE as f32, CHECKBOX_SIZE as f32,
        3.0, colors.input_bg,
    );
    canvas.stroke_rounded_rect(
        x as f32, y as f32,
        CHECKBOX_SIZE as f32, CHECKBOX_SIZE as f32,
        3.0, colors.input_border, 1.0,
    );

    // Check mark
    if checked {
        canvas.fill_rounded_rect(
            (x + 3) as f32, (y + 3) as f32,
            (CHECKBOX_SIZE - 6) as f32, (CHECKBOX_SIZE - 6) as f32,
            2.0, colors.input_border_focused,
        );
    }
}

fn draw_radio(canvas: &mut Canvas, x: i32, y: i32, checked: bool, colors: &Colors) {
    let cx = x as f32 + CHECKBOX_SIZE as f32 / 2.0;
    let cy = y as f32 + CHECKBOX_SIZE as f32 / 2.0;
    let r = CHECKBOX_SIZE as f32 / 2.0;

    // Outer circle (using rounded rect as approximation)
    canvas.fill_rounded_rect(
        x as f32, y as f32,
        CHECKBOX_SIZE as f32, CHECKBOX_SIZE as f32,
        r, colors.input_bg,
    );
    canvas.stroke_rounded_rect(
        x as f32, y as f32,
        CHECKBOX_SIZE as f32, CHECKBOX_SIZE as f32,
        r, colors.input_border, 1.0,
    );

    // Inner dot
    if checked {
        let inner_r = r * 0.5;
        canvas.fill_rounded_rect(
            cx - inner_r, cy - inner_r,
            inner_r * 2.0, inner_r * 2.0,
            inner_r, colors.input_border_focused,
        );
    }
}
