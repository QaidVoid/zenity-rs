//! List selection dialog implementation.

use crate::{
    backend::{create_window, MouseButton, Window, WindowEvent},
    error::Error,
    render::{rgb, Canvas, Font},
    ui::{
        widgets::{button::Button, Widget},
        Colors,
    },
};

const BASE_PADDING: u32 = 16;
const BASE_ROW_HEIGHT: u32 = 28;
const BASE_CHECKBOX_SIZE: u32 = 16;
const BASE_MIN_WIDTH: u32 = 350;
const BASE_MAX_WIDTH: u32 = 600;
const BASE_MIN_HEIGHT: u32 = 200;
const BASE_MAX_HEIGHT: u32 = 450;

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
    /// Multiple selection without checkboxes.
    Multiple,
}

/// List dialog builder.
pub struct ListBuilder {
    title: String,
    text: String,
    columns: Vec<String>,
    rows: Vec<Vec<String>>,
    mode: ListMode,
    hidden_columns: Vec<usize>,
    width: Option<u32>,
    height: Option<u32>,
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
            hidden_columns: Vec::new(),
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

    /// Enable multiple mode (multi-select without checkboxes).
    pub fn multiple(mut self) -> Self {
        self.mode = ListMode::Multiple;
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

    /// Hide a column by index (1-based, like zenity).
    /// Hidden columns are not displayed but their values are still included in output.
    pub fn hide_column(mut self, col: usize) -> Self {
        if col > 0 {
            self.hidden_columns.push(col - 1); // Convert to 0-based
        }
        self
    }

    pub fn show(self) -> Result<ListResult, Error> {
        let colors = self.colors.unwrap_or_else(|| crate::ui::detect_theme());

        // Process rows - for checklist/radiolist, first column is TRUE/FALSE
        let (rows, mut selected): (Vec<Vec<String>>, Vec<bool>) = match self.mode {
            ListMode::Checklist | ListMode::Radiolist => {
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
            }
            ListMode::Single | ListMode::Multiple => {
                (self.rows.clone(), vec![false; self.rows.len()])
            }
        };

        // Columns - skip first column header for checklist/radiolist
        let all_columns: Vec<&str> = match self.mode {
            ListMode::Checklist | ListMode::Radiolist => {
                if !self.columns.is_empty() {
                    self.columns[1..].iter().map(|s| s.as_str()).collect()
                } else {
                    vec![]
                }
            }
            ListMode::Single | ListMode::Multiple => {
                self.columns.iter().map(|s| s.as_str()).collect()
            }
        };

        // Adjust hidden column indices for radiolist/checklist mode
        // In these modes, zenity's column 1 is TRUE/FALSE which we strip,
        // so user's column N becomes internal index N-2 (N-1 for 0-based, then -1 for stripped column)
        let adjusted_hidden: Vec<usize> = match self.mode {
            ListMode::Checklist | ListMode::Radiolist => {
                self.hidden_columns
                    .iter()
                    .filter_map(|&col| col.checked_sub(1)) // Subtract 1 more for stripped TRUE/FALSE column
                    .collect()
            }
            ListMode::Single | ListMode::Multiple => self.hidden_columns.clone(),
        };

        // Determine which columns are visible (not hidden)
        let visible_col_indices: Vec<usize> = (0..all_columns.len())
            .filter(|i| !adjusted_hidden.contains(i))
            .collect();

        // Get visible columns only
        let columns: Vec<&str> = visible_col_indices
            .iter()
            .map(|&i| all_columns[i])
            .collect();

        // Create display rows with only visible columns (original rows kept for result)
        let display_rows: Vec<Vec<String>> = rows
            .iter()
            .map(|row| {
                visible_col_indices
                    .iter()
                    .filter_map(|&i| row.get(i).cloned())
                    .collect()
            })
            .collect();

        let num_cols = columns.len().max(1);
        let num_rows = rows.len();

        // First pass: calculate LOGICAL dimensions using scale 1.0
        let temp_font = Font::load(1.0);

        // Calculate logical column widths (only for visible columns)
        let mut logical_col_widths: Vec<u32> = vec![100; num_cols];
        for (i, col) in columns.iter().enumerate() {
            let (w, _) = temp_font.render(col).measure();
            logical_col_widths[i] = logical_col_widths[i].max(w as u32 + 20);
        }
        for row in &rows {
            for (vi, &orig_i) in visible_col_indices.iter().enumerate() {
                if let Some(cell) = row.get(orig_i) {
                    let (w, _) = temp_font.render(cell).measure();
                    logical_col_widths[vi] = logical_col_widths[vi].max(w as u32 + 20);
                }
            }
        }
        drop(temp_font);

        // Calculate logical total width
        let logical_checkbox_col = if self.mode != ListMode::Single {
            BASE_CHECKBOX_SIZE + 16
        } else {
            0
        };
        let logical_content_width: u32 =
            logical_col_widths.iter().sum::<u32>() + logical_checkbox_col;
        let calc_width =
            (logical_content_width + BASE_PADDING * 2).clamp(BASE_MIN_WIDTH, BASE_MAX_WIDTH);

        // Calculate logical height
        let logical_text_height = if self.text.is_empty() { 0 } else { 24 };
        let logical_header_height = if columns.is_empty() {
            0
        } else {
            BASE_ROW_HEIGHT
        };
        let logical_list_height =
            (num_rows as u32 * BASE_ROW_HEIGHT).clamp(BASE_ROW_HEIGHT * 3, BASE_MAX_HEIGHT - 100);
        let calc_height = (BASE_PADDING * 2
            + logical_text_height
            + logical_header_height
            + logical_list_height
            + 50)
            .clamp(BASE_MIN_HEIGHT, BASE_MAX_HEIGHT);

        // Use custom dimensions if provided, otherwise use calculated defaults
        let logical_width = self.width.unwrap_or(calc_width);
        let logical_height = self.height.unwrap_or(calc_height);

        // Create window with LOGICAL dimensions
        let mut window = create_window(logical_width as u16, logical_height as u16)?;
        window.set_title(if self.title.is_empty() {
            "Select"
        } else {
            &self.title
        })?;

        // Get the actual scale factor from the window (compositor scale)
        let scale = window.scale_factor();

        // Now create everything at PHYSICAL scale
        let font = Font::load(scale);

        // Scale dimensions for physical rendering
        let padding = (BASE_PADDING as f32 * scale) as u32;
        let row_height = (BASE_ROW_HEIGHT as f32 * scale) as u32;
        let checkbox_size = (BASE_CHECKBOX_SIZE as f32 * scale) as u32;

        // Calculate physical dimensions
        let physical_width = (logical_width as f32 * scale) as u32;
        let physical_height = (logical_height as f32 * scale) as u32;

        // Recalculate column widths at physical scale
        let mut col_widths: Vec<u32> = vec![(100.0 * scale) as u32; num_cols];
        for (i, col) in columns.iter().enumerate() {
            let (w, _) = font.render(col).measure();
            col_widths[i] = col_widths[i].max(w as u32 + (20.0 * scale) as u32);
        }
        for row in &display_rows {
            for (i, cell) in row.iter().enumerate() {
                if i < num_cols {
                    let (w, _) = font.render(cell).measure();
                    col_widths[i] = col_widths[i].max(w as u32 + (20.0 * scale) as u32);
                }
            }
        }

        // Calculate physical list dimensions
        let checkbox_col = if self.mode != ListMode::Single {
            checkbox_size + (16.0 * scale) as u32
        } else {
            0
        };
        let text_height = if self.text.is_empty() {
            0
        } else {
            (24.0 * scale) as u32
        };
        let list_height = (logical_list_height as f32 * scale) as u32;

        let total_content_width = checkbox_col + col_widths.iter().sum::<u32>();

        // Create buttons at physical scale
        let mut ok_button = Button::new("OK", &font, scale);
        let mut cancel_button = Button::new("Cancel", &font, scale);

        // Layout in physical coordinates
        let mut y = padding as i32;
        let text_y = y;
        if !self.text.is_empty() {
            y += text_height as i32 + (8.0 * scale) as i32;
        }

        let list_x = padding as i32;
        let list_y = y;
        let list_w = physical_width - padding * 2;
        let list_h = list_height;
        let visible_rows = (list_h / row_height) as usize;

        let button_y = (physical_height - padding - (32.0 * scale) as u32) as i32;
        let mut bx = physical_width as i32 - padding as i32;
        bx -= cancel_button.width() as i32;
        cancel_button.set_position(bx, button_y);
        bx -= (10.0 * scale) as i32 + ok_button.width() as i32;
        ok_button.set_position(bx, button_y);

        // Create canvas at PHYSICAL dimensions
        let mut canvas = Canvas::new(physical_width, physical_height);
        let mut scroll_offset = 0usize;
        let mut h_scroll_offset = 0u32;
        let mut hovered_row: Option<usize> = None;
        let mut single_selected: Option<usize> = None;
        let mut h_scroll_mode = false;

        // Track last cursor position for drag scrolling
        let mut last_cursor_pos: Option<(i32, i32)> = None;

        // Scrollbar thumb dragging state
        let mut v_thumb_drag = false;
        let mut h_thumb_drag = false;
        let mut v_thumb_drag_offset: Option<i32> = None;
        let mut h_thumb_drag_offset: Option<i32> = None;
        let mut clicking_scrollbar = false;
        let mut v_scrollbar_hovered = false;
        let mut h_scrollbar_hovered = false;

        // Create sub-canvas for the list area to enable clipping
        let mut list_canvas = Canvas::new(list_w, list_h);

        // Draw function with scaled parameters
        let draw = |canvas: &mut Canvas,
                    list_canvas: &mut Canvas,
                    colors: &Colors,
                    font: &Font,
                    text: &str,
                    columns: &[&str],
                    rows: &[Vec<String>],
                    col_widths: &[u32],
                    selected: &[bool],
                    single_selected: Option<usize>,
                    scroll_offset: usize,
                    h_scroll_offset: u32,
                    hovered_row: Option<usize>,
                    mode: ListMode,
                    ok_button: &Button,
                    cancel_button: &Button,
                    total_content_width: u32,
                    // Scaled parameters
                    padding: u32,
                    row_height: u32,
                    checkbox_size: u32,
                    checkbox_col: u32,
                    list_x: i32,
                    list_y: i32,
                    list_w: u32,
                    list_h: u32,
                    visible_rows: usize,
                    text_y: i32,
                    scale: f32,
                    v_scrollbar_hovered: bool,
                    h_scrollbar_hovered: bool| {
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

            // Draw text prompt
            if !text.is_empty() {
                let tc = font.render(text).with_color(colors.text).finish();
                canvas.draw_canvas(&tc, padding as i32, text_y);
            }

            // Clear list canvas
            list_canvas.fill(colors.input_bg);

            // List background is already filled above

            // Draw header if columns exist
            let mut data_y_local = 0i32;
            if !columns.is_empty() {
                let header_bg = darken(colors.input_bg, 0.05);
                list_canvas.fill_rect(0.0, 0.0, list_w as f32, row_height as f32, header_bg);

                let mut cx = checkbox_col as i32 - h_scroll_offset as i32;
                for (i, col) in columns.iter().enumerate() {
                    let tc = font.render(col).with_color(rgb(140, 140, 140)).finish();
                    list_canvas.draw_canvas(&tc, cx + (8.0 * scale) as i32, (6.0 * scale) as i32);
                    cx += col_widths.get(i).copied().unwrap_or((100.0 * scale) as u32) as i32;
                }

                // Separator
                list_canvas.fill_rect(
                    0.0,
                    row_height as f32,
                    list_w as f32,
                    1.0,
                    colors.input_border,
                );
                data_y_local += row_height as i32 + 1;
            }

            // Draw rows
            let data_visible = if columns.is_empty() {
                visible_rows
            } else {
                visible_rows.saturating_sub(1)
            };
            for (vi, ri) in
                (scroll_offset..rows.len().min(scroll_offset + data_visible)).enumerate()
            {
                let row = &rows[ri];
                let ry = data_y_local + (vi as u32 * row_height) as i32;

                // Background
                let is_hovered = hovered_row == Some(ri);
                let is_selected = match mode {
                    ListMode::Single => single_selected == Some(ri),
                    ListMode::Multiple | ListMode::Checklist | ListMode::Radiolist => {
                        selected.get(ri).copied().unwrap_or(false)
                    }
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

                list_canvas.fill_rect(1.0, ry as f32, (list_w - 2) as f32, row_height as f32, bg);

                // Checkbox/Radio
                if mode == ListMode::Checklist || mode == ListMode::Radiolist {
                    let check_x = (8.0 * scale) as i32 - h_scroll_offset as i32;
                    let check_y = ry + ((row_height - checkbox_size) / 2) as i32;
                    let checked = selected.get(ri).copied().unwrap_or(false);

                    if mode == ListMode::Checklist {
                        draw_checkbox(
                            list_canvas,
                            check_x,
                            check_y,
                            checked,
                            colors,
                            checkbox_size,
                            scale,
                        );
                    } else {
                        draw_radio(
                            list_canvas,
                            check_x,
                            check_y,
                            checked,
                            colors,
                            checkbox_size,
                            scale,
                        );
                    }
                }

                // Cell values
                let mut cx = checkbox_col as i32 - h_scroll_offset as i32;
                for (ci, cell) in row.iter().enumerate() {
                    if ci < col_widths.len() {
                        let text_color = if is_selected {
                            rgb(255, 255, 255)
                        } else {
                            colors.text
                        };
                        let tc = font.render(cell).with_color(text_color).finish();
                        list_canvas.draw_canvas(
                            &tc,
                            cx + (8.0 * scale) as i32,
                            ry + (6.0 * scale) as i32,
                        );
                        cx += col_widths[ci] as i32;
                    }
                }
            }

            // Vertical Scrollbar
            if rows.len() > data_visible {
                let sb_x = list_w as i32 - (8.0 * scale) as i32;
                let sb_h = list_h as f32
                    - if columns.is_empty() {
                        0.0
                    } else {
                        row_height as f32 + 1.0
                    };
                let sb_y = data_y_local as f32;
                let thumb_h = (((data_visible as f32 / rows.len() as f32 * sb_h).max(20.0 * scale))
                    as f32)
                    .min(sb_h);
                let max_thumb_y = sb_h - thumb_h;
                let thumb_y = if rows.len() > data_visible {
                    scroll_offset as f32 / (rows.len() - data_visible) as f32 * max_thumb_y
                } else {
                    0.0
                };

                let v_scrollbar_width = if v_scrollbar_hovered {
                    12.0 * scale
                } else {
                    8.0 * scale
                };

                list_canvas.fill_rounded_rect(
                    sb_x as f32,
                    sb_y,
                    v_scrollbar_width - 2.0 * scale,
                    sb_h,
                    3.0 * scale,
                    darken(colors.input_bg, 0.05),
                );
                list_canvas.fill_rounded_rect(
                    sb_x as f32,
                    sb_y + thumb_y,
                    v_scrollbar_width - 2.0 * scale,
                    thumb_h,
                    3.0 * scale,
                    if v_scrollbar_hovered {
                        colors.input_border_focused
                    } else {
                        colors.input_border
                    },
                );
            }

            // Horizontal Scrollbar
            if total_content_width > list_w {
                let h_scrollbar_width = if h_scrollbar_hovered {
                    12.0 * scale
                } else {
                    8.0 * scale
                };
                let sb_x = 0.0;
                let sb_y = list_h as i32 - h_scrollbar_width as i32;
                let sb_w = list_w as f32;
                let max_scroll = total_content_width.saturating_sub(list_w);
                let thumb_w = ((list_w as f32 / total_content_width as f32 * sb_w)
                    .max(20.0 * scale))
                .min(sb_w);
                let thumb_x = if max_scroll > 0 {
                    h_scroll_offset as f32 / max_scroll as f32 * (sb_w - thumb_w)
                } else {
                    0.0
                };

                list_canvas.fill_rounded_rect(
                    sb_x,
                    sb_y as f32,
                    sb_w,
                    h_scrollbar_width - 2.0 * scale,
                    3.0 * scale,
                    darken(colors.input_bg, 0.05),
                );
                list_canvas.fill_rounded_rect(
                    sb_x + thumb_x,
                    sb_y as f32,
                    thumb_w,
                    h_scrollbar_width - 2.0 * scale,
                    3.0 * scale,
                    if h_scrollbar_hovered {
                        colors.input_border_focused
                    } else {
                        colors.input_border
                    },
                );
            }

            // Border
            list_canvas.stroke_rounded_rect(
                0.0,
                0.0,
                list_w as f32,
                list_h as f32,
                6.0 * scale,
                colors.input_border,
                1.0,
            );

            // Draw the list canvas to main canvas
            canvas.draw_canvas(&list_canvas, list_x, list_y);

            // Buttons
            ok_button.draw_to(canvas, colors, font);
            cancel_button.draw_to(canvas, colors, font);
        };

        // Initial draw
        draw(
            &mut canvas,
            &mut list_canvas,
            colors,
            &font,
            &self.text,
            &columns,
            &display_rows,
            &col_widths,
            &selected,
            single_selected,
            scroll_offset,
            h_scroll_offset,
            hovered_row,
            self.mode,
            &ok_button,
            &cancel_button,
            total_content_width,
            padding,
            row_height,
            checkbox_size,
            checkbox_col,
            list_x,
            list_y,
            list_w,
            list_h,
            visible_rows,
            text_y,
            scale,
            v_scrollbar_hovered,
            h_scrollbar_hovered,
        );
        window.set_contents(&canvas)?;
        window.show()?;

        let header_height_px = if columns.is_empty() {
            0
        } else {
            row_height + 1
        };
        let data_y = list_y + header_height_px as i32;
        let data_visible = if columns.is_empty() {
            visible_rows
        } else {
            visible_rows.saturating_sub(1)
        };
        loop {
            let event = window.wait_for_event()?;
            let mut needs_redraw = false;

            match &event {
                WindowEvent::CloseRequested => return Ok(ListResult::Closed),
                WindowEvent::RedrawRequested => needs_redraw = true,
                WindowEvent::CursorMove(pos) => {
                    let mx = pos.x as i32;
                    let my = pos.y as i32;

                    // Store current cursor position
                    last_cursor_pos = Some((mx, my));

                    // Handle scrollbar thumb dragging
                    if v_thumb_drag || h_thumb_drag {
                        let list_mx = mx - list_x;
                        let list_my = my - list_y;

                        if v_thumb_drag && rows.len() > data_visible {
                            let sb_h_f32 = list_h as f32
                                - if columns.is_empty() {
                                    0.0
                                } else {
                                    row_height as f32 + 1.0
                                };
                            let sb_h = sb_h_f32 as i32;
                            let sb_y = if columns.is_empty() {
                                0
                            } else {
                                (row_height + 1) as i32
                            };
                            let thumb_h_f32 = ((data_visible as f32 / rows.len() as f32
                                * sb_h_f32)
                                .max(20.0 * scale))
                            .min(sb_h_f32);
                            let thumb_h = thumb_h_f32 as i32;
                            let max_thumb_y = sb_h - thumb_h;

                            // Calculate new scroll offset from thumb position
                            // Use the drag offset to maintain the relative position from where user clicked
                            let offset = v_thumb_drag_offset.unwrap_or(thumb_h / 2);
                            // list_my is relative to list canvas, need to adjust for scrollbar position
                            let thumb_y = (list_my - sb_y - offset).clamp(0, max_thumb_y);
                            let scroll_ratio = if max_thumb_y > 0 {
                                thumb_y as f32 / max_thumb_y as f32
                            } else {
                                0.0
                            };
                            scroll_offset = ((scroll_ratio * (rows.len() - data_visible) as f32)
                                as usize)
                                .clamp(0, rows.len().saturating_sub(data_visible));
                            needs_redraw = true;
                        }

                        if h_thumb_drag && total_content_width > list_w {
                            let sb_w_f32 = list_w as f32;
                            let sb_w = list_w as i32;
                            let max_scroll_u32 = total_content_width.saturating_sub(list_w);
                            let max_scroll = (max_scroll_u32 as i32).max(1);
                            let thumb_w_f32 = ((list_w as f32 / total_content_width as f32
                                * sb_w_f32)
                                .max(20.0 * scale))
                            .min(sb_w_f32);
                            let thumb_w = thumb_w_f32 as i32;
                            let max_thumb_x = sb_w - thumb_w;

                            // Calculate new horizontal scroll offset from thumb position
                            // Use the drag offset to maintain the relative position from where user clicked
                            let offset = h_thumb_drag_offset.unwrap_or(thumb_w / 2);
                            let thumb_x = (list_mx - offset).clamp(0, max_thumb_x);
                            let scroll_ratio = if max_scroll > 0 {
                                thumb_x as f32 / max_thumb_x as f32
                            } else {
                                0.0
                            };
                            h_scroll_offset = ((scroll_ratio * max_scroll as f32) as u32)
                                .clamp(0, max_scroll_u32);
                            needs_redraw = true;
                        }
                    } else {
                        let old_hovered = hovered_row;
                        hovered_row = None;

                        // Update scrollbar hover states
                        let v_scrollbar_width = if v_scrollbar_hovered {
                            12.0 * scale
                        } else {
                            8.0 * scale
                        };
                        let v_scrollbar_x = list_w as i32 - v_scrollbar_width as i32;
                        let h_scrollbar_width = if h_scrollbar_hovered {
                            12.0 * scale
                        } else {
                            8.0 * scale
                        };

                        v_scrollbar_hovered = rows.len() > data_visible
                            && mx >= list_x + v_scrollbar_x
                            && mx < list_x + list_w as i32
                            && my >= list_y
                            && my < list_y + list_h as i32;

                        h_scrollbar_hovered = total_content_width > list_w
                            && mx >= list_x
                            && mx < list_x + list_w as i32
                            && my >= list_y + list_h as i32 - h_scrollbar_width as i32
                            && my < list_y + list_h as i32;

                        // Check row hover (only if not over scrollbar)
                        let effective_v_scrollbar_width =
                            if v_scrollbar_hovered && rows.len() > data_visible {
                                12.0 * scale
                            } else if rows.len() > data_visible {
                                8.0 * scale
                            } else {
                                0.0
                            };

                        if mx >= list_x
                            && mx < list_x + list_w as i32 - effective_v_scrollbar_width as i32
                            && my >= data_y
                            && my < list_y + list_h as i32
                        {
                            let rel_y = (my - data_y) as usize;
                            let ri = scroll_offset + rel_y / row_height as usize;
                            if ri < rows.len() {
                                hovered_row = Some(ri);
                            }
                        }

                        if old_hovered != hovered_row {
                            needs_redraw = true;
                        }
                    }
                }
                WindowEvent::ButtonPress(MouseButton::Left, mods) => {
                    clicking_scrollbar = false;

                    // Check if clicking anywhere in scrollbar area (thumb OR track)
                    if let Some((mx, my)) = last_cursor_pos {
                        // Check if click is in list area (convert to list canvas coords)
                        let list_mx = mx - list_x;
                        let list_my = my - list_y;

                        if list_mx >= 0
                            && list_mx < list_w as i32
                            && list_my >= 0
                            && list_my < list_h as i32
                        {
                            // Vertical scrollbar area
                            if rows.len() > data_visible {
                                let v_scrollbar_width = if v_scrollbar_hovered {
                                    12.0 * scale
                                } else {
                                    8.0 * scale
                                };
                                let sb_x = list_w as i32 - v_scrollbar_width as i32;

                                // Block all clicks in vertical scrollbar area
                                if list_mx >= sb_x {
                                    clicking_scrollbar = true;

                                    let sb_h_f32 = list_h as f32
                                        - if columns.is_empty() {
                                            0.0
                                        } else {
                                            row_height as f32 + 1.0
                                        };
                                    let sb_h = sb_h_f32 as i32;
                                    let sb_y = if columns.is_empty() {
                                        0
                                    } else {
                                        (row_height + 1) as i32
                                    };
                                    let thumb_h_f32 = (((data_visible as f32 / rows.len() as f32
                                        * sb_h_f32)
                                        .max(20.0 * scale))
                                    .min(sb_h_f32));
                                    let thumb_h = thumb_h_f32 as i32;
                                    let max_thumb_y = (sb_h_f32 - thumb_h_f32) as i32;
                                    let thumb_y = if rows.len() > data_visible {
                                        (scroll_offset as f32 / (rows.len() - data_visible) as f32
                                            * max_thumb_y as f32)
                                            as i32
                                    } else {
                                        0
                                    };

                                    // Check if clicking specifically on the thumb for dragging
                                    if list_my >= sb_y + thumb_y
                                        && list_my < sb_y + thumb_y + thumb_h
                                    {
                                        v_thumb_drag = true;
                                        v_thumb_drag_offset = Some(list_my - (sb_y + thumb_y));
                                    }
                                }
                            }

                            // Horizontal scrollbar area
                            if total_content_width > list_w {
                                let h_scrollbar_width = if h_scrollbar_hovered {
                                    12.0 * scale
                                } else {
                                    8.0 * scale
                                };
                                let sb_h = h_scrollbar_width as i32;
                                let sb_y = list_h as i32 - sb_h;

                                // Block all clicks in horizontal scrollbar area
                                if list_my >= sb_y {
                                    clicking_scrollbar = true;

                                    let sb_w_f32 = list_w as f32;
                                    let sb_w = list_w as i32;
                                    let max_scroll_u32 = total_content_width.saturating_sub(list_w);
                                    let max_scroll = (max_scroll_u32 as i32).max(1);
                                    let thumb_w_f32 =
                                        (((list_w as f32 / total_content_width as f32 * sb_w_f32)
                                            .max(20.0 * scale))
                                        .min(sb_w_f32));
                                    let thumb_w = thumb_w_f32 as i32;
                                    let max_thumb_x = sb_w - thumb_w;
                                    let thumb_x = if max_scroll > 0 {
                                        (h_scroll_offset as f32 / max_scroll as f32
                                            * max_thumb_x as f32)
                                            as i32
                                    } else {
                                        0
                                    };

                                    // Check if clicking specifically on the thumb for dragging
                                    if list_mx >= thumb_x && list_mx < thumb_x + thumb_w {
                                        h_thumb_drag = true;
                                        h_thumb_drag_offset = Some(list_mx - thumb_x);
                                    }
                                }
                            }
                        }
                    }

                    // Only process row selection if not clicking on scrollbar
                    if !clicking_scrollbar {
                        if let Some(ri) = hovered_row {
                            match self.mode {
                                ListMode::Single => {
                                    single_selected = Some(ri);
                                }
                                ListMode::Multiple => {
                                    // Only toggle selection if Ctrl is held, otherwise select only this item
                                    if mods.contains(crate::backend::Modifiers::CTRL) {
                                        if let Some(sel) = selected.get_mut(ri) {
                                            *sel = !*sel;
                                        }
                                    } else {
                                        for s in selected.iter_mut() {
                                            *s = false;
                                        }
                                        if let Some(sel) = selected.get_mut(ri) {
                                            *sel = true;
                                        }
                                    }
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
                }
                WindowEvent::ButtonRelease(_, _) => {
                    // End scrollbar thumb dragging
                    v_thumb_drag = false;
                    h_thumb_drag = false;
                    v_thumb_drag_offset = None;
                    h_thumb_drag_offset = None;
                }
                WindowEvent::Scroll(direction) => {
                    if h_scroll_mode {
                        // Shift + wheel: horizontal scroll
                        match direction {
                            crate::backend::ScrollDirection::Up => {
                                if total_content_width > list_w {
                                    h_scroll_offset = h_scroll_offset.saturating_sub(100);
                                    needs_redraw = true;
                                }
                            }
                            crate::backend::ScrollDirection::Down => {
                                if total_content_width > list_w {
                                    let max_scroll = total_content_width.saturating_sub(list_w);
                                    h_scroll_offset = (h_scroll_offset + 100).min(max_scroll);
                                    needs_redraw = true;
                                }
                            }
                            _ => {}
                        }
                    } else {
                        // Normal wheel: vertical scroll
                        match direction {
                            crate::backend::ScrollDirection::Up => {
                                if scroll_offset > 0 {
                                    scroll_offset = scroll_offset.saturating_sub(2);
                                    needs_redraw = true;
                                }
                            }
                            crate::backend::ScrollDirection::Down => {
                                if scroll_offset + data_visible < rows.len() {
                                    scroll_offset = (scroll_offset + 2)
                                        .min(rows.len().saturating_sub(data_visible));
                                    needs_redraw = true;
                                }
                            }
                            crate::backend::ScrollDirection::Left => {
                                if total_content_width > list_w {
                                    h_scroll_offset = h_scroll_offset.saturating_sub(100);
                                    needs_redraw = true;
                                }
                            }
                            crate::backend::ScrollDirection::Right => {
                                if total_content_width > list_w {
                                    let max_scroll = total_content_width.saturating_sub(list_w);
                                    h_scroll_offset = (h_scroll_offset + 100).min(max_scroll);
                                    needs_redraw = true;
                                }
                            }
                        }
                    }
                }
                WindowEvent::KeyPress(key_event) => {
                    const KEY_UP: u32 = 0xff52;
                    const KEY_DOWN: u32 = 0xff54;
                    const KEY_LEFT: u32 = 0xff51;
                    const KEY_RIGHT: u32 = 0xff53;
                    const KEY_LSHIFT: u32 = 0xffe1;
                    const KEY_RSHIFT: u32 = 0xffe2;
                    const KEY_SPACE: u32 = 0x20;
                    const KEY_RETURN: u32 = 0xff0d;
                    const KEY_ESCAPE: u32 = 0xff1b;

                    // Handle shift for scroll mode
                    if key_event.keysym == KEY_LSHIFT || key_event.keysym == KEY_RSHIFT {
                        h_scroll_mode = true;
                        continue;
                    } else {
                        h_scroll_mode = false;
                    }

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
                            } else if self.mode == ListMode::Multiple {
                                let last_selected = selected.iter().position(|&s| s);
                                if let Some(last) = last_selected {
                                    if last > 0 {
                                        single_selected = Some(last - 1);
                                        if last - 1 < scroll_offset {
                                            scroll_offset = last - 1;
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
                            } else if self.mode == ListMode::Multiple {
                                let last_selected = selected.iter().position(|&s| s);
                                if let Some(last) = last_selected {
                                    if last + 1 < rows.len() {
                                        single_selected = Some(last + 1);
                                        if last + 1 >= scroll_offset + data_visible {
                                            scroll_offset = last + 2 - data_visible;
                                        }
                                        needs_redraw = true;
                                    }
                                } else if !rows.is_empty() {
                                    single_selected = Some(0);
                                    needs_redraw = true;
                                }
                            }
                        }
                        KEY_LEFT => {
                            if total_content_width > list_w {
                                h_scroll_offset = h_scroll_offset.saturating_sub(100);
                                needs_redraw = true;
                            }
                        }
                        KEY_RIGHT => {
                            if total_content_width > list_w {
                                let max_scroll = total_content_width.saturating_sub(list_w);
                                h_scroll_offset = (h_scroll_offset + 100).min(max_scroll);
                                needs_redraw = true;
                            }
                        }
                        KEY_SPACE => {
                            if self.mode == ListMode::Checklist || self.mode == ListMode::Multiple {
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
                WindowEvent::KeyRelease(key_event) => {
                    const KEY_LSHIFT: u32 = 0xffe1;
                    const KEY_RSHIFT: u32 = 0xffe2;

                    // Handle shift release for scroll mode
                    if key_event.keysym == KEY_LSHIFT || key_event.keysym == KEY_RSHIFT {
                        h_scroll_mode = false;
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
                match &ev {
                    WindowEvent::CloseRequested => {
                        return Ok(ListResult::Closed);
                    }
                    WindowEvent::CursorEnter(pos) | WindowEvent::CursorMove(pos) => {
                        last_cursor_pos = Some((pos.x as i32, pos.y as i32));
                    }
                    WindowEvent::ButtonPress(button, _modifiers)
                        if *button == MouseButton::Left =>
                    {
                        if let Some((list_mx, list_my)) = last_cursor_pos {
                            // Check vertical scrollbar thumb
                            if rows.len() > data_visible {
                                let sb_x = list_w as i32 - (8.0 * scale) as i32;
                                let sb_h_f32 = list_h as f32
                                    - if columns.is_empty() {
                                        0.0
                                    } else {
                                        row_height as f32 + 1.0
                                    };
                                let sb_h = sb_h_f32 as i32;
                                let sb_y = if columns.is_empty() {
                                    0
                                } else {
                                    (row_height + 1) as i32
                                };
                                let thumb_h_f32 = (((data_visible as f32 / rows.len() as f32
                                    * sb_h_f32)
                                    .max(20.0 * scale))
                                .min(sb_h_f32));
                                let thumb_h = thumb_h_f32 as i32;
                                let max_thumb_y = (sb_h_f32 - thumb_h_f32) as i32;
                                let thumb_y = if rows.len() > data_visible {
                                    (scroll_offset as f32 / (rows.len() - data_visible) as f32
                                        * max_thumb_y as f32)
                                        as i32
                                } else {
                                    0
                                };

                                if list_mx >= sb_x
                                    && list_mx < sb_x + (8.0 * scale) as i32
                                    && list_my >= thumb_y
                                    && list_my < thumb_y + thumb_h
                                {
                                    v_thumb_drag = true;
                                    v_thumb_drag_offset = Some(list_my - thumb_y);
                                }
                            }

                            // Check horizontal scrollbar thumb
                            if total_content_width > list_w {
                                let sb_h = (6.0 * scale) as i32;
                                let sb_y = list_h as i32 - sb_h;
                                let sb_w_f32 = list_w as f32;
                                let sb_w = list_w as i32;
                                let max_scroll_u32 = total_content_width.saturating_sub(list_w);
                                let max_scroll = (max_scroll_u32 as i32).max(1);
                                let thumb_w_f32 = (((list_w as f32 / total_content_width as f32
                                    * sb_w_f32)
                                    .max(20.0 * scale))
                                .min(sb_w_f32));
                                let thumb_w = thumb_w_f32 as i32;
                                let max_thumb_x = sb_w - thumb_w;
                                let thumb_x = if max_scroll > 0 {
                                    (h_scroll_offset as f32 / max_scroll as f32
                                        * max_thumb_x as f32)
                                        as i32
                                } else {
                                    0
                                };

                                if list_my >= sb_y
                                    && list_my < sb_y + sb_h
                                    && list_mx >= thumb_x
                                    && list_mx < thumb_x + thumb_w
                                {
                                    h_thumb_drag = true;
                                    h_thumb_drag_offset = Some(list_mx - thumb_x);
                                }
                            }
                        }
                    }
                    WindowEvent::ButtonRelease(_, _) => {
                        v_thumb_drag = false;
                        h_thumb_drag = false;
                        v_thumb_drag_offset = None;
                        h_thumb_drag_offset = None;
                    }
                    _ => {}
                }

                needs_redraw |= ok_button.process_event(&ev);
                needs_redraw |= cancel_button.process_event(&ev);
            }

            if needs_redraw {
                draw(
                    &mut canvas,
                    &mut list_canvas,
                    colors,
                    &font,
                    &self.text,
                    &columns,
                    &display_rows,
                    &col_widths,
                    &selected,
                    single_selected,
                    scroll_offset,
                    h_scroll_offset,
                    hovered_row,
                    self.mode,
                    &ok_button,
                    &cancel_button,
                    total_content_width,
                    padding,
                    row_height,
                    checkbox_size,
                    checkbox_col,
                    list_x,
                    list_y,
                    list_w,
                    list_h,
                    visible_rows,
                    text_y,
                    scale,
                    v_scrollbar_hovered,
                    h_scrollbar_hovered,
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

fn get_result(
    rows: &[Vec<String>],
    selected: &[bool],
    single_selected: Option<usize>,
    mode: ListMode,
) -> ListResult {
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
        ListMode::Multiple | ListMode::Checklist | ListMode::Radiolist => {
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

fn draw_checkbox(
    canvas: &mut Canvas,
    x: i32,
    y: i32,
    checked: bool,
    colors: &Colors,
    checkbox_size: u32,
    scale: f32,
) {
    // Box
    canvas.fill_rounded_rect(
        x as f32,
        y as f32,
        checkbox_size as f32,
        checkbox_size as f32,
        3.0 * scale,
        colors.input_bg,
    );
    canvas.stroke_rounded_rect(
        x as f32,
        y as f32,
        checkbox_size as f32,
        checkbox_size as f32,
        3.0 * scale,
        colors.input_border,
        1.0,
    );

    // Check mark
    if checked {
        let inset = (3.0 * scale) as i32;
        canvas.fill_rounded_rect(
            (x + inset) as f32,
            (y + inset) as f32,
            (checkbox_size as i32 - inset * 2) as f32,
            (checkbox_size as i32 - inset * 2) as f32,
            2.0 * scale,
            colors.input_border_focused,
        );
    }
}

fn draw_radio(
    canvas: &mut Canvas,
    x: i32,
    y: i32,
    checked: bool,
    colors: &Colors,
    checkbox_size: u32,
    scale: f32,
) {
    let cx = x as f32 + checkbox_size as f32 / 2.0;
    let cy = y as f32 + checkbox_size as f32 / 2.0;
    let r = checkbox_size as f32 / 2.0;

    // Outer circle (using rounded rect as approximation)
    canvas.fill_rounded_rect(
        x as f32,
        y as f32,
        checkbox_size as f32,
        checkbox_size as f32,
        r,
        colors.input_bg,
    );
    canvas.stroke_rounded_rect(
        x as f32,
        y as f32,
        checkbox_size as f32,
        checkbox_size as f32,
        r,
        colors.input_border,
        1.0,
    );

    // Inner dot
    if checked {
        let inner_r = r * 0.5;
        canvas.fill_rounded_rect(
            cx - inner_r,
            cy - inner_r,
            inner_r * 2.0,
            inner_r * 2.0,
            inner_r,
            colors.input_border_focused,
        );
    }
}
