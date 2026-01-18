//! Calendar date picker dialog implementation.

use crate::backend::{Window, WindowEvent, MouseButton, create_window};
use crate::error::Error;
use crate::render::{Canvas, Font, Rgba, rgb};
use crate::ui::Colors;
use crate::ui::widgets::Widget;
use crate::ui::widgets::button::Button;

const PADDING: u32 = 16;
const CELL_SIZE: u32 = 36;
const HEADER_HEIGHT: u32 = 40;
const DAY_HEADER_HEIGHT: u32 = 28;

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
            CalendarResult::Selected { .. } => 0,
            CalendarResult::Cancelled => 1,
            CalendarResult::Closed => 255,
        }
    }

    /// Returns the date as a string in YYYY-MM-DD format.
    pub fn to_string(&self) -> Option<String> {
        match self {
            CalendarResult::Selected { year, month, day } => {
                Some(format!("{:04}-{:02}-{:02}", year, month, day))
            }
            _ => None,
        }
    }
}

/// Calendar dialog builder.
pub struct CalendarBuilder {
    title: String,
    text: String,
    year: Option<u32>,
    month: Option<u32>,
    day: Option<u32>,
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

    pub fn show(self) -> Result<CalendarResult, Error> {
        let colors = self.colors.unwrap_or_else(|| crate::ui::detect_theme());
        let font = Font::load();

        // Get current date as default
        let now = current_date();
        let mut year = self.year.unwrap_or(now.0);
        let mut month = self.month.unwrap_or(now.1);
        let mut selected_day = self.day.unwrap_or(now.2);

        // Create buttons
        let mut ok_button = Button::new("OK", &font);
        let mut cancel_button = Button::new("Cancel", &font);

        // Calculate dimensions
        let grid_width = CELL_SIZE * 7;
        let text_height = if self.text.is_empty() { 0 } else { 24 };
        let width = grid_width + PADDING * 2;
        let height = PADDING * 2 + text_height + HEADER_HEIGHT + DAY_HEADER_HEIGHT + CELL_SIZE * 6 + 50;

        // Create window
        let mut window = create_window(width as u16, height as u16)?;
        window.set_title(if self.title.is_empty() { "Select Date" } else { &self.title })?;

        // Layout
        let mut y = PADDING as i32;
        let text_y = y;
        if !self.text.is_empty() {
            y += text_height as i32 + 8;
        }

        let calendar_x = PADDING as i32;
        let calendar_y = y;

        let button_y = (height - PADDING - 32) as i32;
        let mut bx = width as i32 - PADDING as i32;
        bx -= cancel_button.width() as i32;
        cancel_button.set_position(bx, button_y);
        bx -= 10 + ok_button.width() as i32;
        ok_button.set_position(bx, button_y);

        let mut canvas = Canvas::new(width, height);
        let mut mouse_x = 0i32;
        let mut mouse_y = 0i32;
        let mut hovered_day: Option<u32> = None;

        // Draw function
        let draw = |canvas: &mut Canvas,
                    colors: &Colors,
                    font: &Font,
                    text: &str,
                    year: u32,
                    month: u32,
                    selected_day: u32,
                    hovered_day: Option<u32>,
                    ok_button: &Button,
                    cancel_button: &Button| {
            canvas.fill(colors.window_bg);

            // Draw text prompt
            if !text.is_empty() {
                let tc = font.render(text).with_color(colors.text).finish();
                canvas.draw_canvas(&tc, PADDING as i32, text_y);
            }

            // Calendar background
            let cal_h = HEADER_HEIGHT + DAY_HEADER_HEIGHT + CELL_SIZE * 6;
            canvas.fill_rounded_rect(
                calendar_x as f32, calendar_y as f32,
                grid_width as f32, cal_h as f32,
                8.0, colors.input_bg,
            );

            // Header with month/year and navigation
            let header_y = calendar_y;
            let header_bg = darken(colors.input_bg, 0.03);
            canvas.fill_rounded_rect(
                calendar_x as f32, header_y as f32,
                grid_width as f32, HEADER_HEIGHT as f32,
                8.0, header_bg,
            );
            // Cover bottom corners
            canvas.fill_rect(
                calendar_x as f32, (header_y + HEADER_HEIGHT as i32 - 8) as f32,
                grid_width as f32, 8.0,
                header_bg,
            );

            // Navigation arrows
            let nav_color = colors.text;
            let prev_month = font.render("<").with_color(nav_color).finish();
            canvas.draw_canvas(&prev_month, calendar_x + 12, header_y + 12);

            let next_month = font.render(">").with_color(nav_color).finish();
            canvas.draw_canvas(&next_month, calendar_x + grid_width as i32 - 20, header_y + 12);

            // Month/Year text
            let month_name = month_name(month);
            let header_text = format!("{} {}", month_name, year);
            let ht = font.render(&header_text).with_color(colors.text).finish();
            let ht_x = calendar_x + (grid_width as i32 - ht.width() as i32) / 2;
            canvas.draw_canvas(&ht, ht_x, header_y + 12);

            // Day headers
            let day_header_y = header_y + HEADER_HEIGHT as i32;
            let days = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];
            for (i, day) in days.iter().enumerate() {
                let dx = calendar_x + (i as u32 * CELL_SIZE) as i32;
                let dt = font.render(day).with_color(rgb(140, 140, 140)).finish();
                let dtx = dx + (CELL_SIZE as i32 - dt.width() as i32) / 2;
                canvas.draw_canvas(&dt, dtx, day_header_y + 6);
            }

            // Calendar grid
            let grid_y = day_header_y + DAY_HEADER_HEIGHT as i32;
            let first_day = first_day_of_month(year, month);
            let days_in_month = days_in_month(year, month);
            let today = current_date();

            for day in 1..=days_in_month {
                let cell_idx = (first_day + day - 1) as i32;
                let row = cell_idx / 7;
                let col = cell_idx % 7;

                let cx = calendar_x + col * CELL_SIZE as i32;
                let cy = grid_y + row * CELL_SIZE as i32;

                let is_selected = day == selected_day;
                let is_hovered = hovered_day == Some(day);
                let is_today = year == today.0 && month == today.1 && day == today.2;

                // Cell background
                if is_selected {
                    canvas.fill_rounded_rect(
                        (cx + 2) as f32, (cy + 2) as f32,
                        (CELL_SIZE - 4) as f32, (CELL_SIZE - 4) as f32,
                        4.0, colors.input_border_focused,
                    );
                } else if is_hovered {
                    canvas.fill_rounded_rect(
                        (cx + 2) as f32, (cy + 2) as f32,
                        (CELL_SIZE - 4) as f32, (CELL_SIZE - 4) as f32,
                        4.0, darken(colors.input_bg, 0.08),
                    );
                }

                // Today indicator (ring)
                if is_today && !is_selected {
                    canvas.stroke_rounded_rect(
                        (cx + 4) as f32, (cy + 4) as f32,
                        (CELL_SIZE - 8) as f32, (CELL_SIZE - 8) as f32,
                        4.0, colors.input_border_focused, 2.0,
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
                let dtx = cx + (CELL_SIZE as i32 - dt.width() as i32) / 2;
                let dty = cy + (CELL_SIZE as i32 - dt.height() as i32) / 2;
                canvas.draw_canvas(&dt, dtx, dty);
            }

            // Border
            canvas.stroke_rounded_rect(
                calendar_x as f32, calendar_y as f32,
                grid_width as f32, cal_h as f32,
                8.0, colors.input_border, 1.0,
            );

            // Buttons
            ok_button.draw_to(canvas, colors, font);
            cancel_button.draw_to(canvas, colors, font);
        };

        // Initial draw
        draw(
            &mut canvas, colors, &font, &self.text,
            year, month, selected_day, hovered_day,
            &ok_button, &cancel_button,
        );
        window.set_contents(&canvas)?;
        window.show()?;

        let grid_y = calendar_y + HEADER_HEIGHT as i32 + DAY_HEADER_HEIGHT as i32;

        loop {
            let event = window.wait_for_event()?;
            let mut needs_redraw = false;

            match &event {
                WindowEvent::CloseRequested => return Ok(CalendarResult::Closed),
                WindowEvent::RedrawRequested => needs_redraw = true,
                WindowEvent::CursorMove(pos) => {
                    mouse_x = pos.x as i32;
                    mouse_y = pos.y as i32;

                    let old_hovered = hovered_day;
                    hovered_day = None;

                    // Check if over grid
                    if mouse_x >= calendar_x && mouse_x < calendar_x + grid_width as i32
                        && mouse_y >= grid_y && mouse_y < grid_y + (CELL_SIZE * 6) as i32
                    {
                        let col = (mouse_x - calendar_x) / CELL_SIZE as i32;
                        let row = (mouse_y - grid_y) / CELL_SIZE as i32;
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
                WindowEvent::ButtonPress(MouseButton::Left) => {
                    let header_y = calendar_y;

                    // Check navigation
                    if mouse_y >= header_y && mouse_y < header_y + HEADER_HEIGHT as i32 {
                        // Previous month
                        if mouse_x >= calendar_x && mouse_x < calendar_x + 32 {
                            if month == 1 {
                                month = 12;
                                year -= 1;
                            } else {
                                month -= 1;
                            }
                            // Clamp selected day
                            selected_day = selected_day.min(days_in_month(year, month));
                            needs_redraw = true;
                        }
                        // Next month
                        else if mouse_x >= calendar_x + grid_width as i32 - 32 {
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
                    if let Some(day) = hovered_day {
                        selected_day = day;
                        needs_redraw = true;
                    }
                }
                WindowEvent::KeyPress(key_event) => {
                    const KEY_LEFT: u32 = 0xff51;
                    const KEY_RIGHT: u32 = 0xff53;
                    const KEY_UP: u32 = 0xff52;
                    const KEY_DOWN: u32 = 0xff54;
                    const KEY_RETURN: u32 = 0xff0d;
                    const KEY_ESCAPE: u32 = 0xff1b;

                    match key_event.keysym {
                        KEY_LEFT => {
                            if selected_day > 1 {
                                selected_day -= 1;
                            } else {
                                // Previous month
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
                                // Next month
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
                                // Previous month
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
                            return Ok(CalendarResult::Selected { year, month, day: selected_day });
                        }
                        KEY_ESCAPE => {
                            return Ok(CalendarResult::Cancelled);
                        }
                        _ => {}
                    }
                }
                _ => {}
            }

            needs_redraw |= ok_button.process_event(&event);
            needs_redraw |= cancel_button.process_event(&event);

            if ok_button.was_clicked() {
                return Ok(CalendarResult::Selected { year, month, day: selected_day });
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
                draw(
                    &mut canvas, colors, &font, &self.text,
                    year, month, selected_day, hovered_day,
                    &ok_button, &cancel_button,
                );
                window.set_contents(&canvas)?;
            }
        }
    }
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
        2 => if is_leap_year(year) { 29 } else { 28 },
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
