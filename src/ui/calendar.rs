//! Calendar date picker dialog implementation.

use crate::{
    backend::{MouseButton, Window, WindowEvent, create_window},
    error::Error,
    render::{Canvas, Font, Rgba, rgb},
    ui::{
        Colors,
        widgets::{Widget, button::Button},
    },
};

const BASE_PADDING: u32 = 16;
const BASE_CELL_SIZE: u32 = 36;
const BASE_HEADER_HEIGHT: u32 = 40;
const BASE_DAY_HEADER_HEIGHT: u32 = 28;
const BASE_DROPDOWN_ITEM_HEIGHT: u32 = 24;

/// Calendar dialog result.
#[derive(Debug, Clone)]
pub enum CalendarResult {
    /// User selected a date.
    Selected { year: u32, month: u32, day: u32 },
    /// User cancelled.
    Cancelled,
    /// Dialog was closed.
    Closed,
}

impl CalendarResult {
    pub fn exit_code(&self) -> i32 {
        match self {
            CalendarResult::Selected {
                ..
            } => 0,
            CalendarResult::Cancelled => 1,
            CalendarResult::Closed => 255,
        }
    }

    /// Returns the date as a string in YYYY-MM-DD format.
    pub fn to_string(&self) -> Option<String> {
        match self {
            CalendarResult::Selected {
                year,
                month,
                day,
            } => Some(format!("{:04}-{:02}-{:02}", year, month, day)),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum DropdownState {
    None,
    Month,
    Year,
}

/// Calendar dialog builder.
pub struct CalendarBuilder {
    title: String,
    text: String,
    year: Option<u32>,
    month: Option<u32>,
    day: Option<u32>,
    width: Option<u32>,
    height: Option<u32>,
    colors: Option<&'static Colors>,
}

impl CalendarBuilder {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            text: String::new(),
            year: None,
            month: None,
            day: None,
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

    /// Set initial year.
    pub fn year(mut self, year: u32) -> Self {
        self.year = Some(year);
        self
    }

    /// Set initial month (1-12).
    pub fn month(mut self, month: u32) -> Self {
        self.month = Some(month.clamp(1, 12));
        self
    }

    /// Set initial day (1-31).
    pub fn day(mut self, day: u32) -> Self {
        self.day = Some(day.clamp(1, 31));
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

    pub fn show(self) -> Result<CalendarResult, Error> {
        let colors = self.colors.unwrap_or_else(|| crate::ui::detect_theme());

        // Calculate logical dimensions at scale 1.0
        let logical_grid_width = BASE_CELL_SIZE * 7;
        let logical_text_height = if self.text.is_empty() { 0 } else { 24 };
        let calc_width = logical_grid_width + BASE_PADDING * 2;
        let calc_height = BASE_PADDING * 2
            + logical_text_height
            + BASE_HEADER_HEIGHT
            + BASE_DAY_HEADER_HEIGHT
            + BASE_CELL_SIZE * 6
            + 50;

        // Use custom dimensions if provided, otherwise use calculated defaults
        let logical_width = self.width.unwrap_or(calc_width);
        let logical_height = self.height.unwrap_or(calc_height);

        // Create window with LOGICAL dimensions
        let mut window = create_window(logical_width as u16, logical_height as u16)?;
        window.set_title(if self.title.is_empty() {
            "Calendar selection"
        } else {
            &self.title
        })?;

        // Get the actual scale factor from the window (compositor scale)
        let scale = window.scale_factor();

        // Now create everything at PHYSICAL scale
        let font = Font::load(scale);

        // Scale dimensions for physical rendering
        let padding = (BASE_PADDING as f32 * scale) as u32;
        let cell_size = (BASE_CELL_SIZE as f32 * scale) as u32;
        let header_height = (BASE_HEADER_HEIGHT as f32 * scale) as u32;
        let day_header_height = (BASE_DAY_HEADER_HEIGHT as f32 * scale) as u32;
        let dropdown_item_height = (BASE_DROPDOWN_ITEM_HEIGHT as f32 * scale) as u32;

        // Calculate physical dimensions
        let grid_width = cell_size * 7;
        let text_height = if self.text.is_empty() {
            0
        } else {
            (24.0 * scale) as u32
        };
        let width = grid_width + padding * 2;
        let height = padding * 2
            + text_height
            + header_height
            + day_header_height
            + cell_size * 6
            + (50.0 * scale) as u32;

        // Get current date as default
        let now = current_date();
        let mut year = self.year.unwrap_or(now.0);
        let mut month = self.month.unwrap_or(now.1);
        let mut selected_day = self.day.unwrap_or(now.2);

        // Create buttons at physical scale
        let mut ok_button = Button::new("OK", &font, scale);
        let mut cancel_button = Button::new("Cancel", &font, scale);

        // Layout in physical coordinates
        let mut y = padding as i32;
        let text_y = y;
        if !self.text.is_empty() {
            y += text_height as i32 + (8.0 * scale) as i32;
        }

        let calendar_x = padding as i32;
        let calendar_y = y;

        let button_y = (height - padding - (32.0 * scale) as u32) as i32;
        let mut bx = width as i32 - padding as i32;
        bx -= cancel_button.width() as i32;
        cancel_button.set_position(bx, button_y);
        bx -= (10.0 * scale) as i32 + ok_button.width() as i32;
        ok_button.set_position(bx, button_y);

        // Create canvas at PHYSICAL dimensions
        let mut canvas = Canvas::new(width, height);
        let mut mouse_x = 0i32;
        let mut mouse_y = 0i32;
        let mut hovered_day: Option<u32> = None;
        let mut dropdown = DropdownState::None;
        let mut dropdown_hover: Option<usize> = None;
        let mut year_scroll_offset: i32 = 0;

        // Initial draw
        draw_calendar(
            &mut canvas,
            colors,
            &font,
            &self.text,
            text_y,
            calendar_x,
            calendar_y,
            grid_width,
            year,
            month,
            selected_day,
            hovered_day,
            dropdown,
            dropdown_hover,
            year_scroll_offset,
            &ok_button,
            &cancel_button,
            scale,
        );
        window.set_contents(&canvas)?;
        window.show()?;

        let grid_y = calendar_y + header_height as i32 + day_header_height as i32;

        loop {
            let event = window.wait_for_event()?;
            let mut needs_redraw = false;

            match &event {
                WindowEvent::CloseRequested => return Ok(CalendarResult::Closed),
                WindowEvent::RedrawRequested => needs_redraw = true,
                WindowEvent::CursorMove(pos) => {
                    mouse_x = pos.x as i32;
                    mouse_y = pos.y as i32;

                    // Handle dropdown hover
                    if dropdown != DropdownState::None {
                        let old_hover = dropdown_hover;
                        dropdown_hover = get_dropdown_hover(
                            dropdown, mouse_x, mouse_y, calendar_x, calendar_y, scale,
                        );
                        if old_hover != dropdown_hover {
                            needs_redraw = true;
                        }
                    } else {
                        // Handle day hover
                        let old_hovered = hovered_day;
                        hovered_day = None;

                        if mouse_x >= calendar_x
                            && mouse_x < calendar_x + grid_width as i32
                            && mouse_y >= grid_y
                            && mouse_y < grid_y + (cell_size * 6) as i32
                        {
                            let col = (mouse_x - calendar_x) / cell_size as i32;
                            let row = (mouse_y - grid_y) / cell_size as i32;
                            let cell_idx = row * 7 + col;

                            let first_day = first_day_of_month(year, month);
                            let days_in = days_in_month(year, month);

                            let day = cell_idx - first_day as i32 + 1;
                            if day >= 1 && day <= days_in as i32 {
                                hovered_day = Some(day as u32);
                            }
                        }

                        if old_hovered != hovered_day {
                            needs_redraw = true;
                        }
                    }
                }
                WindowEvent::ButtonPress(MouseButton::Left, _) => {
                    let header_y = calendar_y;

                    // Handle dropdown selection
                    if dropdown != DropdownState::None {
                        if let Some(idx) = dropdown_hover {
                            match dropdown {
                                DropdownState::Month => {
                                    month = idx as u32 + 1;
                                    selected_day = selected_day.min(days_in_month(year, month));
                                }
                                DropdownState::Year => {
                                    let base_year = year as i32 - 5 + year_scroll_offset;
                                    year = (base_year + idx as i32).max(1) as u32;
                                    selected_day = selected_day.min(days_in_month(year, month));
                                }
                                DropdownState::None => {}
                            }
                        }
                        dropdown = DropdownState::None;
                        dropdown_hover = None;
                        needs_redraw = true;
                    }
                    // Check header clicks
                    else if mouse_y >= header_y && mouse_y < header_y + header_height as i32 {
                        // Calculate actual positions based on text widths
                        let month_name = month_name(month);
                        let month_text_width = font.render(month_name).finish().width() as i32;
                        let year_str = year.to_string();
                        let year_text_width = font.render(&year_str).finish().width() as i32;

                        let prev_arrow_end = calendar_x + 28;
                        let month_x = calendar_x + 35;
                        let month_end = month_x + month_text_width;
                        let year_x = month_x + month_text_width + 8;
                        let year_end = year_x + year_text_width;
                        let today_x = calendar_x + grid_width as i32 - 70;
                        let next_arrow_start = calendar_x + grid_width as i32 - 24;

                        // Check in order from left to right
                        if mouse_x < prev_arrow_end {
                            // Previous month
                            if month == 1 {
                                month = 12;
                                year -= 1;
                            } else {
                                month -= 1;
                            }
                            selected_day = selected_day.min(days_in_month(year, month));
                            needs_redraw = true;
                        } else if mouse_x >= month_x && mouse_x < month_end + 5 {
                            // Month click
                            dropdown = DropdownState::Month;
                            dropdown_hover = Some((month - 1) as usize);
                            needs_redraw = true;
                        } else if mouse_x >= year_x && mouse_x < year_end + 5 {
                            // Year click
                            dropdown = DropdownState::Year;
                            dropdown_hover = Some(5); // Current year is at index 5
                            year_scroll_offset = 0;
                            needs_redraw = true;
                        } else if mouse_x >= today_x && mouse_x < next_arrow_start {
                            // Today click
                            let today = current_date();
                            year = today.0;
                            month = today.1;
                            selected_day = today.2;
                            needs_redraw = true;
                        } else if mouse_x >= next_arrow_start {
                            // Next month
                            if month == 12 {
                                month = 1;
                                year += 1;
                            } else {
                                month += 1;
                            }
                            selected_day = selected_day.min(days_in_month(year, month));
                            needs_redraw = true;
                        }
                    }
                    // Check day click
                    else if let Some(day) = hovered_day {
                        selected_day = day;
                        needs_redraw = true;
                    }
                }
                WindowEvent::Scroll(dir) => {
                    if dropdown == DropdownState::Year {
                        match dir {
                            crate::backend::ScrollDirection::Up => {
                                year_scroll_offset -= 1;
                                needs_redraw = true;
                            }
                            crate::backend::ScrollDirection::Down => {
                                year_scroll_offset += 1;
                                needs_redraw = true;
                            }
                            _ => {}
                        }
                    }
                }
                WindowEvent::KeyPress(key_event) => {
                    const KEY_LEFT: u32 = 0xff51;
                    const KEY_RIGHT: u32 = 0xff53;
                    const KEY_UP: u32 = 0xff52;
                    const KEY_DOWN: u32 = 0xff54;
                    const KEY_RETURN: u32 = 0xff0d;
                    const KEY_ESCAPE: u32 = 0xff1b;

                    // Handle dropdown keyboard navigation
                    if dropdown != DropdownState::None {
                        let max_items = match dropdown {
                            DropdownState::Month => 12,
                            DropdownState::Year => 11,
                            DropdownState::None => 0,
                        };

                        match key_event.keysym {
                            KEY_ESCAPE => {
                                dropdown = DropdownState::None;
                                dropdown_hover = None;
                                needs_redraw = true;
                            }
                            KEY_UP => {
                                let current = dropdown_hover.unwrap_or(0);
                                if current > 0 {
                                    dropdown_hover = Some(current - 1);
                                } else if dropdown == DropdownState::Year {
                                    // Scroll up for year dropdown
                                    year_scroll_offset -= 1;
                                }
                                needs_redraw = true;
                            }
                            KEY_DOWN => {
                                let current = dropdown_hover.unwrap_or(0);
                                if current + 1 < max_items {
                                    dropdown_hover = Some(current + 1);
                                } else if dropdown == DropdownState::Year {
                                    // Scroll down for year dropdown
                                    year_scroll_offset += 1;
                                }
                                needs_redraw = true;
                            }
                            KEY_RETURN => {
                                if let Some(idx) = dropdown_hover {
                                    match dropdown {
                                        DropdownState::Month => {
                                            month = idx as u32 + 1;
                                            selected_day =
                                                selected_day.min(days_in_month(year, month));
                                        }
                                        DropdownState::Year => {
                                            let base_year = year as i32 - 5 + year_scroll_offset;
                                            year = (base_year + idx as i32).max(1) as u32;
                                            selected_day =
                                                selected_day.min(days_in_month(year, month));
                                        }
                                        DropdownState::None => {}
                                    }
                                }
                                dropdown = DropdownState::None;
                                dropdown_hover = None;
                                needs_redraw = true;
                            }
                            _ => {}
                        }
                    } else {
                        match key_event.keysym {
                            KEY_LEFT => {
                                if selected_day > 1 {
                                    selected_day -= 1;
                                } else {
                                    if month == 1 {
                                        month = 12;
                                        year -= 1;
                                    } else {
                                        month -= 1;
                                    }
                                    selected_day = days_in_month(year, month);
                                }
                                needs_redraw = true;
                            }
                            KEY_RIGHT => {
                                if selected_day < days_in_month(year, month) {
                                    selected_day += 1;
                                } else {
                                    if month == 12 {
                                        month = 1;
                                        year += 1;
                                    } else {
                                        month += 1;
                                    }
                                    selected_day = 1;
                                }
                                needs_redraw = true;
                            }
                            KEY_UP => {
                                if selected_day > 7 {
                                    selected_day -= 7;
                                } else {
                                    if month == 1 {
                                        month = 12;
                                        year -= 1;
                                    } else {
                                        month -= 1;
                                    }
                                    let days_prev = days_in_month(year, month);
                                    selected_day = days_prev - (7 - selected_day);
                                }
                                needs_redraw = true;
                            }
                            KEY_DOWN => {
                                let days_in = days_in_month(year, month);
                                if selected_day + 7 <= days_in {
                                    selected_day += 7;
                                } else {
                                    let overflow = selected_day + 7 - days_in;
                                    if month == 12 {
                                        month = 1;
                                        year += 1;
                                    } else {
                                        month += 1;
                                    }
                                    selected_day = overflow;
                                }
                                needs_redraw = true;
                            }
                            KEY_RETURN => {
                                return Ok(CalendarResult::Selected {
                                    year,
                                    month,
                                    day: selected_day,
                                });
                            }
                            KEY_ESCAPE => {
                                return Ok(CalendarResult::Cancelled);
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }

            needs_redraw |= ok_button.process_event(&event);
            needs_redraw |= cancel_button.process_event(&event);

            if ok_button.was_clicked() {
                return Ok(CalendarResult::Selected {
                    year,
                    month,
                    day: selected_day,
                });
            }
            if cancel_button.was_clicked() {
                return Ok(CalendarResult::Cancelled);
            }

            while let Some(ev) = window.poll_for_event()? {
                if let WindowEvent::CloseRequested = ev {
                    return Ok(CalendarResult::Closed);
                }
                if let WindowEvent::CursorMove(pos) = ev {
                    mouse_x = pos.x as i32;
                    mouse_y = pos.y as i32;
                }
                needs_redraw |= ok_button.process_event(&ev);
                needs_redraw |= cancel_button.process_event(&ev);
            }

            if needs_redraw {
                draw_calendar(
                    &mut canvas,
                    colors,
                    &font,
                    &self.text,
                    text_y,
                    calendar_x,
                    calendar_y,
                    grid_width,
                    year,
                    month,
                    selected_day,
                    hovered_day,
                    dropdown,
                    dropdown_hover,
                    year_scroll_offset,
                    &ok_button,
                    &cancel_button,
                    scale,
                );
                window.set_contents(&canvas)?;
            }
        }
    }
}

fn draw_calendar(
    canvas: &mut Canvas,
    colors: &Colors,
    font: &Font,
    text: &str,
    text_y: i32,
    calendar_x: i32,
    calendar_y: i32,
    grid_width: u32,
    year: u32,
    month: u32,
    selected_day: u32,
    hovered_day: Option<u32>,
    dropdown: DropdownState,
    dropdown_hover: Option<usize>,
    year_scroll_offset: i32,
    ok_button: &Button,
    cancel_button: &Button,
    scale: f32,
) {
    // Scale dimensions
    let padding = (BASE_PADDING as f32 * scale) as u32;
    let cell_size = (BASE_CELL_SIZE as f32 * scale) as u32;
    let header_height = (BASE_HEADER_HEIGHT as f32 * scale) as u32;
    let day_header_height = (BASE_DAY_HEADER_HEIGHT as f32 * scale) as u32;
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

    // Calendar background
    let cal_h = header_height + day_header_height + cell_size * 6;
    canvas.fill_rounded_rect(
        calendar_x as f32,
        calendar_y as f32,
        grid_width as f32,
        cal_h as f32,
        8.0 * scale,
        colors.input_bg,
    );

    // Header with month/year and navigation
    let header_y = calendar_y;
    let header_bg = darken(colors.input_bg, 0.03);
    canvas.fill_rounded_rect(
        calendar_x as f32,
        header_y as f32,
        grid_width as f32,
        header_height as f32,
        8.0 * scale,
        header_bg,
    );
    // Cover bottom corners
    canvas.fill_rect(
        calendar_x as f32,
        (header_y + header_height as i32 - (8.0 * scale) as i32) as f32,
        grid_width as f32,
        8.0 * scale,
        header_bg,
    );

    // Navigation arrows
    let nav_color = colors.text;

    // Previous arrow
    let prev_arrow = font.render("<").with_color(nav_color).finish();
    canvas.draw_canvas(
        &prev_arrow,
        calendar_x + (10.0 * scale) as i32,
        header_y + (12.0 * scale) as i32,
    );

    // Next arrow
    let next_arrow = font.render(">").with_color(nav_color).finish();
    canvas.draw_canvas(
        &next_arrow,
        calendar_x + grid_width as i32 - (18.0 * scale) as i32,
        header_y + (12.0 * scale) as i32,
    );

    // Month name (clickable)
    let month_name_str = month_name(month);
    let month_text = font.render(month_name_str).with_color(colors.text).finish();
    let month_x = calendar_x + (35.0 * scale) as i32;
    canvas.draw_canvas(&month_text, month_x, header_y + (12.0 * scale) as i32);

    // Year (clickable)
    let year_str = year.to_string();
    let year_text = font.render(&year_str).with_color(colors.text).finish();
    let year_x = month_x + month_text.width() as i32 + (8.0 * scale) as i32;
    canvas.draw_canvas(&year_text, year_x, header_y + (12.0 * scale) as i32);

    // "Today" link (right side) - green color for action
    let today_color = rgb(80, 160, 100);
    let today_text = font.render("Today").with_color(today_color).finish();
    let today_x = calendar_x + grid_width as i32
        - (24.0 * scale) as i32
        - today_text.width() as i32
        - (8.0 * scale) as i32;
    canvas.draw_canvas(&today_text, today_x, header_y + (12.0 * scale) as i32);

    // Day headers
    let day_header_y = header_y + header_height as i32;
    let days = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];
    for (i, day) in days.iter().enumerate() {
        let dx = calendar_x + (i as u32 * cell_size) as i32;
        let dt = font.render(day).with_color(rgb(140, 140, 140)).finish();
        let dtx = dx + (cell_size as i32 - dt.width() as i32) / 2;
        canvas.draw_canvas(&dt, dtx, day_header_y + (6.0 * scale) as i32);
    }

    // Calendar grid
    let grid_y = day_header_y + day_header_height as i32;
    let first_day = first_day_of_month(year, month);
    let days_in_month = days_in_month(year, month);
    let today = current_date();

    for day in 1..=days_in_month {
        let cell_idx = (first_day + day - 1) as i32;
        let row = cell_idx / 7;
        let col = cell_idx % 7;

        let cx = calendar_x + col * cell_size as i32;
        let cy = grid_y + row * cell_size as i32;

        let is_selected = day == selected_day;
        let is_hovered = hovered_day == Some(day);
        let is_today = year == today.0 && month == today.1 && day == today.2;

        // Cell background
        if is_selected {
            canvas.fill_rounded_rect(
                (cx + (2.0 * scale) as i32) as f32,
                (cy + (2.0 * scale) as i32) as f32,
                (cell_size - (4.0 * scale) as u32) as f32,
                (cell_size - (4.0 * scale) as u32) as f32,
                4.0 * scale,
                colors.input_border_focused,
            );
        } else if is_hovered {
            canvas.fill_rounded_rect(
                (cx + (2.0 * scale) as i32) as f32,
                (cy + (2.0 * scale) as i32) as f32,
                (cell_size - (4.0 * scale) as u32) as f32,
                (cell_size - (4.0 * scale) as u32) as f32,
                4.0 * scale,
                darken(colors.input_bg, 0.08),
            );
        }

        // Today indicator (ring)
        if is_today && !is_selected {
            canvas.stroke_rounded_rect(
                (cx + (4.0 * scale) as i32) as f32,
                (cy + (4.0 * scale) as i32) as f32,
                (cell_size - (8.0 * scale) as u32) as f32,
                (cell_size - (8.0 * scale) as u32) as f32,
                4.0 * scale,
                colors.input_border_focused,
                2.0 * scale,
            );
        }

        // Day number
        let day_str = day.to_string();
        let text_color = if is_selected {
            rgb(255, 255, 255)
        } else if col == 0 {
            rgb(200, 100, 100) // Sunday in red-ish
        } else {
            colors.text
        };
        let dt = font.render(&day_str).with_color(text_color).finish();
        let dtx = cx + (cell_size as i32 - dt.width() as i32) / 2;
        let dty = cy + (cell_size as i32 - dt.height() as i32) / 2;
        canvas.draw_canvas(&dt, dtx, dty);
    }

    // Border
    canvas.stroke_rounded_rect(
        calendar_x as f32,
        calendar_y as f32,
        grid_width as f32,
        cal_h as f32,
        8.0 * scale,
        colors.input_border,
        1.0,
    );

    // Buttons (draw before dropdowns so dropdowns appear on top)
    ok_button.draw_to(canvas, colors, font);
    cancel_button.draw_to(canvas, colors, font);

    // Draw dropdowns on top of everything
    if dropdown == DropdownState::Month {
        draw_month_dropdown(
            canvas,
            colors,
            font,
            calendar_x,
            calendar_y,
            month,
            dropdown_hover,
            scale,
        );
    } else if dropdown == DropdownState::Year {
        draw_year_dropdown(
            canvas,
            colors,
            font,
            calendar_x,
            calendar_y,
            year,
            year_scroll_offset,
            dropdown_hover,
            scale,
        );
    }
}

fn draw_month_dropdown(
    canvas: &mut Canvas,
    colors: &Colors,
    font: &Font,
    calendar_x: i32,
    calendar_y: i32,
    current_month: u32,
    hover: Option<usize>,
    scale: f32,
) {
    let header_height = (BASE_HEADER_HEIGHT as f32 * scale) as u32;
    let dropdown_item_height = (BASE_DROPDOWN_ITEM_HEIGHT as f32 * scale) as u32;

    let dropdown_x = calendar_x + (30.0 * scale) as i32;
    let dropdown_y = calendar_y + header_height as i32;
    let dropdown_w = (100.0 * scale) as u32;
    let dropdown_h = 6 * dropdown_item_height; // Show 6 items at a time

    // Background with shadow effect
    canvas.fill_rounded_rect(
        (dropdown_x + (3.0 * scale) as i32) as f32,
        (dropdown_y + (3.0 * scale) as i32) as f32,
        dropdown_w as f32,
        (dropdown_h * 2) as f32,
        6.0 * scale,
        rgb(0, 0, 0),
    );
    canvas.fill_rounded_rect(
        dropdown_x as f32,
        dropdown_y as f32,
        dropdown_w as f32,
        (dropdown_h * 2) as f32,
        6.0 * scale,
        colors.window_bg,
    );

    // Items
    for i in 0..12usize {
        let item_y = dropdown_y + (i as u32 * dropdown_item_height) as i32;
        let is_current = i + 1 == current_month as usize;
        let is_hovered = hover == Some(i);

        // Hover background - subtle gray
        if is_hovered {
            canvas.fill_rounded_rect(
                (dropdown_x + (4.0 * scale) as i32) as f32,
                (item_y + (2.0 * scale) as i32) as f32,
                (dropdown_w - (8.0 * scale) as u32) as f32,
                (dropdown_item_height - (4.0 * scale) as u32) as f32,
                4.0 * scale,
                rgb(70, 130, 180), // Steel blue for hover
            );
        }

        // Current month gets a checkmark
        let name = month_name(i as u32 + 1);
        let display_name = if is_current {
            format!("{} *", name)
        } else {
            name.to_string()
        };

        let text_color = if is_hovered {
            rgb(255, 255, 255)
        } else if is_current {
            rgb(70, 180, 130) // Teal for current
        } else {
            colors.text
        };
        let tc = font.render(&display_name).with_color(text_color).finish();
        canvas.draw_canvas(
            &tc,
            dropdown_x + (10.0 * scale) as i32,
            item_y + (4.0 * scale) as i32,
        );
    }

    // Border
    canvas.stroke_rounded_rect(
        dropdown_x as f32,
        dropdown_y as f32,
        dropdown_w as f32,
        (dropdown_h * 2) as f32,
        6.0 * scale,
        colors.input_border,
        1.0,
    );
}

fn draw_year_dropdown(
    canvas: &mut Canvas,
    colors: &Colors,
    font: &Font,
    calendar_x: i32,
    calendar_y: i32,
    current_year: u32,
    scroll_offset: i32,
    hover: Option<usize>,
    scale: f32,
) {
    let header_height = (BASE_HEADER_HEIGHT as f32 * scale) as u32;
    let dropdown_item_height = (BASE_DROPDOWN_ITEM_HEIGHT as f32 * scale) as u32;

    let dropdown_x = calendar_x + (100.0 * scale) as i32;
    let dropdown_y = calendar_y + header_height as i32;
    let dropdown_w = (70.0 * scale) as u32;
    let visible_years = 11usize;
    let dropdown_h = visible_years as u32 * dropdown_item_height;

    // Background with shadow
    canvas.fill_rounded_rect(
        (dropdown_x + (3.0 * scale) as i32) as f32,
        (dropdown_y + (3.0 * scale) as i32) as f32,
        dropdown_w as f32,
        dropdown_h as f32,
        6.0 * scale,
        rgb(0, 0, 0),
    );
    canvas.fill_rounded_rect(
        dropdown_x as f32,
        dropdown_y as f32,
        dropdown_w as f32,
        dropdown_h as f32,
        6.0 * scale,
        colors.window_bg,
    );

    // Years centered around current year
    let base_year = current_year as i32 - 5 + scroll_offset;

    for i in 0..visible_years {
        let yr = base_year + i as i32;
        if yr < 1 {
            continue;
        }

        let item_y = dropdown_y + (i as u32 * dropdown_item_height) as i32;
        let is_current = yr == current_year as i32;
        let is_hovered = hover == Some(i);

        // Hover background
        if is_hovered {
            canvas.fill_rounded_rect(
                (dropdown_x + (4.0 * scale) as i32) as f32,
                (item_y + (2.0 * scale) as i32) as f32,
                (dropdown_w - (8.0 * scale) as u32) as f32,
                (dropdown_item_height - (4.0 * scale) as u32) as f32,
                4.0 * scale,
                rgb(70, 130, 180), // Steel blue for hover
            );
        }

        let yr_str = if is_current {
            format!("* {} *", yr)
        } else {
            yr.to_string()
        };

        let text_color = if is_hovered {
            rgb(255, 255, 255)
        } else if is_current {
            rgb(70, 180, 130) // Teal for current
        } else {
            colors.text
        };
        let tc = font.render(&yr_str).with_color(text_color).finish();
        let tx = dropdown_x + (dropdown_w as i32 - tc.width() as i32) / 2;
        canvas.draw_canvas(&tc, tx, item_y + (4.0 * scale) as i32);
    }

    // Border
    canvas.stroke_rounded_rect(
        dropdown_x as f32,
        dropdown_y as f32,
        dropdown_w as f32,
        dropdown_h as f32,
        6.0 * scale,
        colors.input_border,
        1.0,
    );
}

fn get_dropdown_hover(
    dropdown: DropdownState,
    mouse_x: i32,
    mouse_y: i32,
    calendar_x: i32,
    calendar_y: i32,
    scale: f32,
) -> Option<usize> {
    let header_height = (BASE_HEADER_HEIGHT as f32 * scale) as u32;
    let dropdown_item_height = (BASE_DROPDOWN_ITEM_HEIGHT as f32 * scale) as u32;

    let dropdown_y = calendar_y + header_height as i32;

    match dropdown {
        DropdownState::Month => {
            let dropdown_x = calendar_x + (30.0 * scale) as i32;
            let dropdown_w = (100.0 * scale) as i32;
            let dropdown_h = (12 * dropdown_item_height) as i32;

            if mouse_x >= dropdown_x
                && mouse_x < dropdown_x + dropdown_w
                && mouse_y >= dropdown_y
                && mouse_y < dropdown_y + dropdown_h
            {
                let idx = (mouse_y - dropdown_y) / dropdown_item_height as i32;
                return Some(idx as usize);
            }
        }
        DropdownState::Year => {
            let dropdown_x = calendar_x + (100.0 * scale) as i32;
            let dropdown_w = (70.0 * scale) as i32;
            let visible_years = 11u32;
            let dropdown_h = (visible_years * dropdown_item_height) as i32;

            if mouse_x >= dropdown_x
                && mouse_x < dropdown_x + dropdown_w
                && mouse_y >= dropdown_y
                && mouse_y < dropdown_y + dropdown_h
            {
                let idx = (mouse_y - dropdown_y) / dropdown_item_height as i32;
                return Some(idx as usize);
            }
        }
        DropdownState::None => {}
    }
    None
}

impl Default for CalendarBuilder {
    fn default() -> Self {
        Self::new()
    }
}

fn darken(color: Rgba, amount: f32) -> Rgba {
    rgb(
        (color.r as f32 * (1.0 - amount)) as u8,
        (color.g as f32 * (1.0 - amount)) as u8,
        (color.b as f32 * (1.0 - amount)) as u8,
    )
}

/// Get current date as (year, month, day).
fn current_date() -> (u32, u32, u32) {
    use std::time::{SystemTime, UNIX_EPOCH};

    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Simple date calculation
    let days = secs / 86400;
    let mut year = 1970u32;
    let mut remaining = days;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        year += 1;
    }

    let mut month = 1u32;
    loop {
        let days_in = days_in_month(year, month) as u64;
        if remaining < days_in {
            break;
        }
        remaining -= days_in;
        month += 1;
    }

    let day = remaining as u32 + 1;
    (year, month, day)
}

fn is_leap_year(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 => 31,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        3 => 31,
        4 => 30,
        5 => 31,
        6 => 30,
        7 => 31,
        8 => 31,
        9 => 30,
        10 => 31,
        11 => 30,
        12 => 31,
        _ => 30,
    }
}

/// Get the day of week (0=Sunday) for the first day of the month.
fn first_day_of_month(year: u32, month: u32) -> u32 {
    // Zeller's congruence (adjusted for Sunday=0)
    let mut y = year as i32;
    let mut m = month as i32;

    if m < 3 {
        m += 12;
        y -= 1;
    }

    let k = y % 100;
    let j = y / 100;

    let h = (1 + (13 * (m + 1)) / 5 + k + k / 4 + j / 4 - 2 * j) % 7;
    ((h + 6) % 7) as u32 // Convert to Sunday=0
}

fn month_name(month: u32) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "Unknown",
    }
}
