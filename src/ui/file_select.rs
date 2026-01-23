//! File selection dialog implementation with enhanced UI.

use std::{
    collections::HashSet,
    fs::{self, Metadata},
    path::{Path, PathBuf},
    time::SystemTime,
};

use crate::{
    backend::{create_window, MouseButton, Window, WindowEvent},
    error::Error,
    render::{rgb, Canvas, Font, Rgba},
    ui::{
        widgets::{button::Button, text_input::TextInput, Widget},
        Colors,
    },
};

// Layout constants (logical, at scale 1.0)
const BASE_WINDOW_WIDTH: u32 = 700;
const BASE_WINDOW_HEIGHT: u32 = 500;
const BASE_PADDING: u32 = 12;
const BASE_SIDEBAR_WIDTH: u32 = 160;
const BASE_TOOLBAR_HEIGHT: u32 = 36;
const BASE_PATH_BAR_HEIGHT: u32 = 32;
const BASE_SEARCH_WIDTH: u32 = 200;
const BASE_ITEM_HEIGHT: u32 = 28;
const BASE_ICON_SIZE: u32 = 20;

// Column widths (logical)
const BASE_NAME_COL_WIDTH: u32 = 280;
const BASE_SIZE_COL_WIDTH: u32 = 80;

/// File selection dialog result.
#[derive(Debug, Clone)]
pub enum FileSelectResult {
    Selected(PathBuf),
    SelectedMultiple(Vec<PathBuf>),
    Cancelled,
    Closed,
}

impl FileSelectResult {
    pub fn exit_code(&self) -> i32 {
        match self {
            FileSelectResult::Selected(_) | FileSelectResult::SelectedMultiple(_) => 0,
            FileSelectResult::Cancelled => 1,
            FileSelectResult::Closed => 255,
        }
    }
}

/// Quick access location.
#[derive(Clone)]
struct QuickAccess {
    name: &'static str,
    path: PathBuf,
    icon: QuickAccessIcon,
}

#[derive(Clone, Copy)]
enum QuickAccessIcon {
    Home,
    Desktop,
    Documents,
    Downloads,
    Pictures,
    Music,
    Videos,
}

/// File filter pattern.
#[derive(Debug, Clone)]
pub struct FileFilter {
    pub pattern: String,
}

/// File selection dialog builder.
pub struct FileSelectBuilder {
    title: String,
    directory: bool,
    save: bool,
    filename: String,
    start_path: Option<PathBuf>,
    width: Option<u32>,
    height: Option<u32>,
    colors: Option<&'static Colors>,
    filters: Vec<FileFilter>,
    multiple: bool,
    separator: String,
}

impl FileSelectBuilder {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            directory: false,
            save: false,
            filename: String::new(),
            start_path: None,
            width: None,
            height: None,
            colors: None,
            filters: Vec::new(),
            multiple: false,
            separator: String::from(" "),
        }
    }

    pub fn title(mut self, title: &str) -> Self {
        self.title = title.to_string();
        self
    }

    pub fn directory(mut self, directory: bool) -> Self {
        self.directory = directory;
        self
    }

    pub fn save(mut self, save: bool) -> Self {
        self.save = save;
        self
    }

    pub fn filename(mut self, filename: &str) -> Self {
        self.filename = filename.to_string();
        self
    }

    pub fn start_path(mut self, path: &Path) -> Self {
        self.start_path = Some(path.to_path_buf());
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

    pub fn add_filter(mut self, filter: FileFilter) -> Self {
        self.filters.push(filter);
        self
    }

    pub fn multiple(mut self, multiple: bool) -> Self {
        self.multiple = multiple;
        self
    }

    pub fn separator(mut self, separator: &str) -> Self {
        self.separator = separator.to_string();
        self
    }

    pub fn show(self) -> Result<FileSelectResult, Error> {
        let colors = self.colors.unwrap_or_else(|| crate::ui::detect_theme());

        // Use custom dimensions if provided, otherwise use defaults
        let logical_width = self.width.unwrap_or(BASE_WINDOW_WIDTH);
        let logical_height = self.height.unwrap_or(BASE_WINDOW_HEIGHT);

        // Create window with LOGICAL dimensions first
        let mut window = create_window(logical_width as u16, logical_height as u16)?;
        let title = if self.title.is_empty() {
            if self.directory {
                "Select Directory"
            } else if self.save {
                "Save File"
            } else {
                "Open File"
            }
        } else {
            &self.title
        };
        window.set_title(title)?;

        // Get the actual scale factor from the window (compositor scale)
        let scale = window.scale_factor();

        // Now create everything at PHYSICAL scale
        let font = Font::load(scale);

        // Scale dimensions for physical rendering
        let window_width = (logical_width as f32 * scale) as u32;
        let window_height = (logical_height as f32 * scale) as u32;
        let padding = (BASE_PADDING as f32 * scale) as u32;
        let sidebar_width = (BASE_SIDEBAR_WIDTH as f32 * scale) as u32;
        let toolbar_height = (BASE_TOOLBAR_HEIGHT as f32 * scale) as u32;
        let path_bar_height = (BASE_PATH_BAR_HEIGHT as f32 * scale) as u32;
        let search_width = (BASE_SEARCH_WIDTH as f32 * scale) as u32;
        let item_height = (BASE_ITEM_HEIGHT as f32 * scale) as u32;
        let name_col_width = (BASE_NAME_COL_WIDTH as f32 * scale) as u32;
        let size_col_width = (BASE_SIZE_COL_WIDTH as f32 * scale) as u32;

        // Build quick access locations
        let quick_access = build_quick_access();

        // Create UI elements at physical scale
        let mut ok_button = Button::new(if self.save { "Save" } else { "Open" }, &font, scale);
        let mut cancel_button = Button::new("Cancel", &font, scale);

        // Search input
        let mut search_input = TextInput::new(search_width).with_placeholder("Search...");

        // Navigation history
        let mut history: Vec<PathBuf> = Vec::new();
        let mut history_index: usize = 0;

        // Current state
        let mut current_dir = self
            .start_path
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from("/")));
        history.push(current_dir.clone());

        let mut all_entries: Vec<DirEntry> = Vec::new();
        let mut filtered_entries: Vec<usize> = Vec::new(); // Indices into all_entries
        let mut selected_indices: HashSet<usize> = HashSet::new();
        let mut scroll_offset: usize = 0;
        let mut show_hidden = false;
        let mut search_text = String::new();
        let mut hovered_quick_access: Option<usize> = None;
        let mut hovered_entry: Option<usize> = None;

        // Load initial directory
        load_directory(&current_dir, &mut all_entries, self.directory, show_hidden);
        update_filtered(
            &all_entries,
            &search_text,
            &mut filtered_entries,
            &self.filters,
        );

        // Calculate layout in physical coordinates
        let sidebar_x = padding as i32;
        let sidebar_y = (padding + toolbar_height + (8.0 * scale) as u32) as i32;
        let sidebar_h = window_height
            - padding * 2
            - toolbar_height
            - (8.0 * scale) as u32
            - (44.0 * scale) as u32;

        let main_x = (padding + sidebar_width + (12.0 * scale) as u32) as i32;
        let main_y = sidebar_y;
        let main_w = window_width - padding * 2 - sidebar_width - (12.0 * scale) as u32;
        let main_h = sidebar_h;

        let header_offset = (28.0 * scale) as u32; // Column headers
        let list_y = main_y + path_bar_height as i32 + header_offset as i32;
        let list_h = main_h - path_bar_height - header_offset;
        let visible_items = (list_h / item_height) as usize;

        // Position buttons
        let button_y = (window_height - padding - (32.0 * scale) as u32) as i32;
        let mut bx = window_width as i32 - padding as i32;
        bx -= cancel_button.width() as i32;
        cancel_button.set_position(bx, button_y);
        bx -= (10.0 * scale) as i32 + ok_button.width() as i32;
        ok_button.set_position(bx, button_y);

        // Position search input
        let search_x = window_width as i32 - padding as i32 - search_width as i32;
        let search_y = padding as i32 + (2.0 * scale) as i32;
        search_input.set_position(search_x, search_y);

        // Create canvas at PHYSICAL dimensions
        let mut canvas = Canvas::new(window_width, window_height);
        let mut mouse_x = 0i32;
        let mut mouse_y = 0i32;

        // Draw function - captures scaled variables from enclosing scope
        let draw = |canvas: &mut Canvas,
                    colors: &Colors,
                    font: &Font,
                    current_dir: &Path,
                    quick_access: &[QuickAccess],
                    all_entries: &[DirEntry],
                    filtered_entries: &[usize],
                    selected_indices: &HashSet<usize>,
                    scroll_offset: usize,
                    hovered_quick_access: Option<usize>,
                    hovered_entry: Option<usize>,
                    show_hidden: bool,
                    search_input: &TextInput,
                    ok_button: &Button,
                    cancel_button: &Button,
                    history: &[PathBuf],
                    history_index: usize,
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

            // Toolbar background
            let toolbar_bg = darken(colors.window_bg, 0.03);
            canvas.fill_rect(
                0.0,
                0.0,
                window_width as f32,
                (toolbar_height + padding) as f32,
                toolbar_bg,
            );

            // Navigation buttons
            let nav_y = padding as i32 + (4.0 * scale) as i32;
            let can_back = history_index > 0;
            let can_forward = history_index + 1 < history.len();

            // Back button
            draw_nav_button(
                canvas,
                padding as i32,
                nav_y,
                "<",
                can_back,
                colors,
                font,
                scale,
            );
            // Forward button
            draw_nav_button(
                canvas,
                (padding as f32 + 32.0 * scale) as i32,
                nav_y,
                ">",
                can_forward,
                colors,
                font,
                scale,
            );
            // Up button
            let can_up = current_dir.parent().is_some();
            draw_nav_button(
                canvas,
                (padding as f32 + 68.0 * scale) as i32,
                nav_y,
                "^",
                can_up,
                colors,
                font,
                scale,
            );
            // Home button
            draw_nav_button(
                canvas,
                (padding as f32 + 104.0 * scale) as i32,
                nav_y,
                "~",
                true,
                colors,
                font,
                scale,
            );
            // Hidden files toggle
            let toggle_x = (padding as f32 + 150.0 * scale) as i32;
            draw_toggle(
                canvas,
                toggle_x,
                nav_y,
                ".*",
                show_hidden,
                colors,
                font,
                scale,
            );

            // Search input
            search_input.draw_to(canvas, colors, font);

            // Sidebar
            let sidebar_bg = darken(colors.window_bg, 0.02);
            canvas.fill_rounded_rect(
                sidebar_x as f32,
                sidebar_y as f32,
                sidebar_width as f32,
                sidebar_h as f32,
                6.0 * scale,
                sidebar_bg,
            );

            // Quick access items
            for (i, qa) in quick_access.iter().enumerate() {
                let y = sidebar_y + (8.0 * scale) as i32 + (i as i32 * (32.0 * scale) as i32);
                let is_hovered = hovered_quick_access == Some(i);
                let is_current = qa.path == current_dir;

                if is_current {
                    canvas.fill_rounded_rect(
                        (sidebar_x + (4.0 * scale) as i32) as f32,
                        y as f32,
                        (sidebar_width - (8.0 * scale) as u32) as f32,
                        28.0 * scale,
                        4.0 * scale,
                        colors.input_border_focused,
                    );
                } else if is_hovered {
                    canvas.fill_rounded_rect(
                        (sidebar_x + (4.0 * scale) as i32) as f32,
                        y as f32,
                        (sidebar_width - (8.0 * scale) as u32) as f32,
                        28.0 * scale,
                        4.0 * scale,
                        darken(colors.window_bg, 0.05),
                    );
                }

                // Icon
                draw_quick_access_icon(
                    canvas,
                    sidebar_x + (12.0 * scale) as i32,
                    y + (4.0 * scale) as i32,
                    qa.icon,
                    colors,
                    scale,
                );

                // Name
                let text_color = if is_current {
                    rgb(255, 255, 255)
                } else {
                    colors.text
                };
                let name_canvas = font.render(qa.name).with_color(text_color).finish();
                canvas.draw_canvas(
                    &name_canvas,
                    sidebar_x + (36.0 * scale) as i32,
                    y + (6.0 * scale) as i32,
                );
            }

            // Main area background
            canvas.fill_rounded_rect(
                main_x as f32,
                main_y as f32,
                main_w as f32,
                main_h as f32,
                6.0 * scale,
                colors.input_bg,
            );

            // Path bar (breadcrumbs)
            draw_breadcrumbs(
                canvas,
                main_x + (8.0 * scale) as i32,
                main_y + (6.0 * scale) as i32,
                main_w - (16.0 * scale) as u32,
                current_dir,
                colors,
                font,
            );

            // Column headers
            let header_y = main_y + path_bar_height as i32;
            let header_bg = darken(colors.input_bg, 0.03);
            canvas.fill_rect(
                main_x as f32,
                header_y as f32,
                main_w as f32,
                26.0 * scale,
                header_bg,
            );

            let header_text = rgb(150, 150, 150);
            let name_header = font.render("Name").with_color(header_text).finish();
            canvas.draw_canvas(
                &name_header,
                main_x + (32.0 * scale) as i32,
                header_y + (5.0 * scale) as i32,
            );
            let size_header = font.render("Size").with_color(header_text).finish();
            canvas.draw_canvas(
                &size_header,
                main_x + name_col_width as i32 + (8.0 * scale) as i32,
                header_y + (5.0 * scale) as i32,
            );
            let date_header = font.render("Modified").with_color(header_text).finish();
            canvas.draw_canvas(
                &date_header,
                main_x + name_col_width as i32 + size_col_width as i32 + (16.0 * scale) as i32,
                header_y + (5.0 * scale) as i32,
            );

            // Separator line
            canvas.fill_rect(
                main_x as f32,
                (header_y + (26.0 * scale) as i32) as f32,
                main_w as f32,
                1.0,
                colors.input_border,
            );

            // File list
            let list_x = main_x;
            for (vi, &ei) in filtered_entries
                .iter()
                .skip(scroll_offset)
                .take(visible_items)
                .enumerate()
            {
                let entry = &all_entries[ei];
                let y = list_y + (vi as u32 * item_height) as i32;
                let is_selected = selected_indices.contains(&ei);
                let is_hovered = hovered_entry == Some(ei);

                // Alternating background
                let row_bg = if vi % 2 == 1 {
                    darken(colors.input_bg, 0.02)
                } else {
                    colors.input_bg
                };

                // Selection/hover highlight
                if is_selected {
                    canvas.fill_rect(
                        (list_x + 2) as f32,
                        y as f32,
                        (main_w - 4) as f32,
                        item_height as f32,
                        colors.input_border_focused,
                    );
                } else if is_hovered {
                    canvas.fill_rect(
                        (list_x + 2) as f32,
                        y as f32,
                        (main_w - 4) as f32,
                        item_height as f32,
                        darken(colors.input_bg, 0.06),
                    );
                } else {
                    canvas.fill_rect(
                        list_x as f32,
                        y as f32,
                        main_w as f32,
                        item_height as f32,
                        row_bg,
                    );
                }

                // Icon
                let icon_x = list_x + (8.0 * scale) as i32;
                let icon_y = y + (4.0 * scale) as i32;
                if entry.is_dir {
                    draw_folder_icon(canvas, icon_x, icon_y, colors, scale);
                } else {
                    draw_file_icon(canvas, icon_x, icon_y, &entry.name, colors, scale);
                }

                // Name
                let text_color = if is_selected {
                    rgb(255, 255, 255)
                } else {
                    colors.text
                };
                let display_name = truncate_name(&entry.name, 35);
                let name_canvas = font.render(&display_name).with_color(text_color).finish();
                canvas.draw_canvas(
                    &name_canvas,
                    list_x + (32.0 * scale) as i32,
                    y + (6.0 * scale) as i32,
                );

                // Size (for files)
                if !entry.is_dir {
                    let size_str = format_size(entry.size);
                    let size_color = if is_selected {
                        rgb(220, 220, 220)
                    } else {
                        rgb(140, 140, 140)
                    };
                    let size_canvas = font.render(&size_str).with_color(size_color).finish();
                    canvas.draw_canvas(
                        &size_canvas,
                        list_x + name_col_width as i32 + (8.0 * scale) as i32,
                        y + (6.0 * scale) as i32,
                    );
                }

                // Date
                let date_str = format_date(entry.modified);
                let date_color = if is_selected {
                    rgb(220, 220, 220)
                } else {
                    rgb(140, 140, 140)
                };
                let date_canvas = font.render(&date_str).with_color(date_color).finish();
                canvas.draw_canvas(
                    &date_canvas,
                    list_x + name_col_width as i32 + size_col_width as i32 + (16.0 * scale) as i32,
                    y + (6.0 * scale) as i32,
                );
            }

            // Scrollbar
            if filtered_entries.len() > visible_items {
                let scrollbar_x = main_x + main_w as i32 - (8.0 * scale) as i32;
                let scrollbar_h = list_h as f32;
                let thumb_h = (visible_items as f32 / filtered_entries.len() as f32 * scrollbar_h)
                    .max(20.0 * scale);
                let thumb_y = scroll_offset as f32 / filtered_entries.len() as f32 * scrollbar_h;

                // Track
                canvas.fill_rounded_rect(
                    scrollbar_x as f32,
                    list_y as f32,
                    6.0 * scale,
                    scrollbar_h,
                    3.0 * scale,
                    darken(colors.input_bg, 0.05),
                );
                // Thumb
                canvas.fill_rounded_rect(
                    scrollbar_x as f32,
                    list_y as f32 + thumb_y,
                    6.0 * scale,
                    thumb_h,
                    3.0 * scale,
                    colors.input_border,
                );
            }

            // Border
            canvas.stroke_rounded_rect(
                main_x as f32,
                main_y as f32,
                main_w as f32,
                main_h as f32,
                6.0 * scale,
                colors.input_border,
                1.0,
            );

            // Buttons
            ok_button.draw_to(canvas, colors, font);
            cancel_button.draw_to(canvas, colors, font);

            // Status bar
            let status = format!("{} items", filtered_entries.len());
            let status_canvas = font.render(&status).with_color(rgb(120, 120, 120)).finish();
            canvas.draw_canvas(&status_canvas, main_x, button_y + (8.0 * scale) as i32);
        };

        // Initial draw
        draw(
            &mut canvas,
            colors,
            &font,
            &current_dir,
            &quick_access,
            &all_entries,
            &filtered_entries,
            &selected_indices,
            scroll_offset,
            hovered_quick_access,
            hovered_entry,
            show_hidden,
            &search_input,
            &ok_button,
            &cancel_button,
            &history,
            history_index,
            scale,
        );
        window.set_contents(&canvas)?;
        window.show()?;

        // Event loop
        loop {
            let event = window.wait_for_event()?;
            let mut needs_redraw = false;

            match &event {
                WindowEvent::CloseRequested => return Ok(FileSelectResult::Closed),
                WindowEvent::RedrawRequested => needs_redraw = true,
                WindowEvent::CursorMove(pos) => {
                    mouse_x = pos.x as i32;
                    mouse_y = pos.y as i32;

                    // Update hover states
                    let old_qa = hovered_quick_access;
                    let old_entry = hovered_entry;

                    hovered_quick_access = None;
                    hovered_entry = None;

                    // Check quick access hover
                    if mouse_x >= sidebar_x
                        && mouse_x < sidebar_x + sidebar_width as i32
                        && mouse_y >= sidebar_y
                    {
                        let rel_y = mouse_y - sidebar_y - (8.0 * scale) as i32;
                        if rel_y >= 0 {
                            let idx = (rel_y as f32 / (32.0 * scale)) as usize;
                            if idx < quick_access.len() {
                                hovered_quick_access = Some(idx);
                            }
                        }
                    }

                    // Check file list hover
                    if mouse_x >= main_x
                        && mouse_x < main_x + main_w as i32
                        && mouse_y >= list_y
                        && mouse_y < list_y + list_h as i32
                    {
                        let rel_y = (mouse_y - list_y) as usize;
                        let idx = scroll_offset + rel_y / item_height as usize;
                        if idx < filtered_entries.len() {
                            hovered_entry = Some(filtered_entries[idx]);
                        }
                    }

                    if old_qa != hovered_quick_access || old_entry != hovered_entry {
                        needs_redraw = true;
                    }
                }
                WindowEvent::ButtonPress(MouseButton::Left, _) => {
                    // Toolbar buttons
                    let nav_y = padding as i32 + (4.0 * scale) as i32;
                    let btn_size = (28.0 * scale) as i32;
                    if mouse_y >= nav_y && mouse_y < nav_y + btn_size {
                        // Back
                        if mouse_x >= padding as i32 && mouse_x < padding as i32 + btn_size {
                            if history_index > 0 {
                                history_index -= 1;
                                current_dir = history[history_index].clone();
                                load_directory(
                                    &current_dir,
                                    &mut all_entries,
                                    self.directory,
                                    show_hidden,
                                );
                                update_filtered(
                                    &all_entries,
                                    &search_text,
                                    &mut filtered_entries,
                                    &self.filters,
                                );
                                selected_indices.clear();
                                scroll_offset = 0;
                                needs_redraw = true;
                            }
                        }
                        // Forward
                        else if mouse_x >= (padding as f32 + 32.0 * scale) as i32
                            && mouse_x < (padding as f32 + 60.0 * scale) as i32
                        {
                            if history_index + 1 < history.len() {
                                history_index += 1;
                                current_dir = history[history_index].clone();
                                load_directory(
                                    &current_dir,
                                    &mut all_entries,
                                    self.directory,
                                    show_hidden,
                                );
                                update_filtered(
                                    &all_entries,
                                    &search_text,
                                    &mut filtered_entries,
                                    &self.filters,
                                );
                                selected_indices.clear();
                                scroll_offset = 0;
                                needs_redraw = true;
                            }
                        }
                        // Up
                        else if mouse_x >= (padding as f32 + 68.0 * scale) as i32
                            && mouse_x < (padding as f32 + 96.0 * scale) as i32
                        {
                            if let Some(parent) = current_dir.parent() {
                                navigate_to(
                                    parent.to_path_buf(),
                                    &mut current_dir,
                                    &mut history,
                                    &mut history_index,
                                );
                                load_directory(
                                    &current_dir,
                                    &mut all_entries,
                                    self.directory,
                                    show_hidden,
                                );
                                update_filtered(
                                    &all_entries,
                                    &search_text,
                                    &mut filtered_entries,
                                    &self.filters,
                                );
                                selected_indices.clear();
                                scroll_offset = 0;
                                needs_redraw = true;
                            }
                        }
                        // Home
                        else if mouse_x >= (padding as f32 + 104.0 * scale) as i32
                            && mouse_x < (padding as f32 + 132.0 * scale) as i32
                        {
                            if let Some(home) = dirs::home_dir() {
                                navigate_to(
                                    home,
                                    &mut current_dir,
                                    &mut history,
                                    &mut history_index,
                                );
                                load_directory(
                                    &current_dir,
                                    &mut all_entries,
                                    self.directory,
                                    show_hidden,
                                );
                                update_filtered(
                                    &all_entries,
                                    &search_text,
                                    &mut filtered_entries,
                                    &self.filters,
                                );
                                selected_indices.clear();
                                scroll_offset = 0;
                                needs_redraw = true;
                            }
                        }
                        // Hidden toggle
                        else if mouse_x >= (padding as f32 + 150.0 * scale) as i32
                            && mouse_x < (padding as f32 + 178.0 * scale) as i32
                        {
                            show_hidden = !show_hidden;
                            load_directory(
                                &current_dir,
                                &mut all_entries,
                                self.directory,
                                show_hidden,
                            );
                            update_filtered(
                                &all_entries,
                                &search_text,
                                &mut filtered_entries,
                                &self.filters,
                            );
                            selected_indices.clear();
                            scroll_offset = 0;
                            needs_redraw = true;
                        }
                    }

                    // Quick access click
                    if let Some(idx) = hovered_quick_access {
                        let qa = &quick_access[idx];
                        if qa.path.exists() {
                            navigate_to(
                                qa.path.clone(),
                                &mut current_dir,
                                &mut history,
                                &mut history_index,
                            );
                            load_directory(
                                &current_dir,
                                &mut all_entries,
                                self.directory,
                                show_hidden,
                            );
                            update_filtered(
                                &all_entries,
                                &search_text,
                                &mut filtered_entries,
                                &self.filters,
                            );
                            selected_indices.clear();
                            scroll_offset = 0;
                            needs_redraw = true;
                        }
                    }

                    // File list click
                    if let Some(ei) = hovered_entry {
                        if self.multiple {
                            // Toggle selection in multiple mode
                            if selected_indices.contains(&ei) {
                                selected_indices.remove(&ei);
                            } else {
                                selected_indices.insert(ei);
                            }
                        } else {
                            // Single click - activate if already selected (double click behavior)
                            if selected_indices.contains(&ei) {
                                let entry = &all_entries[ei];
                                if entry.is_dir {
                                    navigate_to(
                                        entry.path.clone(),
                                        &mut current_dir,
                                        &mut history,
                                        &mut history_index,
                                    );
                                    load_directory(
                                        &current_dir,
                                        &mut all_entries,
                                        self.directory,
                                        show_hidden,
                                    );
                                    update_filtered(
                                        &all_entries,
                                        &search_text,
                                        &mut filtered_entries,
                                        &self.filters,
                                    );
                                    selected_indices.clear();
                                    scroll_offset = 0;
                                } else if !self.directory {
                                    return Ok(FileSelectResult::Selected(entry.path.clone()));
                                }
                            } else {
                                selected_indices.clear();
                                selected_indices.insert(ei);
                            }
                        }
                        needs_redraw = true;
                    }

                    // Search input focus
                    let in_search = mouse_x >= search_x
                        && mouse_x < search_x + search_width as i32
                        && mouse_y >= search_y
                        && mouse_y < search_y + (32.0 * scale) as i32;
                    search_input.set_focus(in_search);
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
                            if scroll_offset + visible_items < filtered_entries.len() {
                                scroll_offset = (scroll_offset + 3)
                                    .min(filtered_entries.len().saturating_sub(visible_items));
                                needs_redraw = true;
                            }
                        }
                        _ => {}
                    }
                }
                WindowEvent::KeyPress(key_event) => {
                    const KEY_UP: u32 = 0xff52;
                    const KEY_DOWN: u32 = 0xff54;
                    const KEY_RETURN: u32 = 0xff0d;
                    const KEY_ESCAPE: u32 = 0xff1b;
                    const KEY_BACKSPACE: u32 = 0xff08;

                    if !search_input.has_focus() {
                        match key_event.keysym {
                            KEY_UP => {
                                if !filtered_entries.is_empty() {
                                    let new_index =
                                        if let Some(&sel) = selected_indices.iter().next() {
                                            if let Some(pos) =
                                                filtered_entries.iter().position(|&e| e == sel)
                                            {
                                                if pos > 0 {
                                                    Some(filtered_entries[pos - 1])
                                                } else {
                                                    Some(sel)
                                                }
                                            } else {
                                                Some(filtered_entries[0])
                                            }
                                        } else {
                                            Some(filtered_entries[0])
                                        };

                                    if let Some(idx) = new_index {
                                        if self.multiple {
                                            if selected_indices.contains(&idx) {
                                                selected_indices.remove(&idx);
                                            } else {
                                                selected_indices.insert(idx);
                                            }
                                        } else {
                                            selected_indices.clear();
                                            selected_indices.insert(idx);
                                        }

                                        if let Some(pos) =
                                            filtered_entries.iter().position(|&e| e == idx)
                                        {
                                            if pos < scroll_offset {
                                                scroll_offset = pos;
                                            }
                                        }
                                        needs_redraw = true;
                                    }
                                }
                            }
                            KEY_DOWN => {
                                if !filtered_entries.is_empty() {
                                    let new_index =
                                        if let Some(&sel) = selected_indices.iter().next() {
                                            if let Some(pos) =
                                                filtered_entries.iter().position(|&e| e == sel)
                                            {
                                                if pos + 1 < filtered_entries.len() {
                                                    Some(filtered_entries[pos + 1])
                                                } else {
                                                    Some(sel)
                                                }
                                            } else {
                                                Some(filtered_entries[0])
                                            }
                                        } else {
                                            Some(filtered_entries[0])
                                        };

                                    if let Some(idx) = new_index {
                                        if self.multiple {
                                            if selected_indices.contains(&idx) {
                                                selected_indices.remove(&idx);
                                            } else {
                                                selected_indices.insert(idx);
                                            }
                                        } else {
                                            selected_indices.clear();
                                            selected_indices.insert(idx);
                                        }

                                        if let Some(pos) =
                                            filtered_entries.iter().position(|&e| e == idx)
                                        {
                                            if pos + 1 >= scroll_offset + visible_items {
                                                scroll_offset = pos + 1 - visible_items + 1;
                                            }
                                        }
                                        needs_redraw = true;
                                    }
                                }
                            }
                            KEY_RETURN => {
                                if self.multiple && !selected_indices.is_empty() {
                                    let selected_files: Vec<PathBuf> = selected_indices
                                        .iter()
                                        .filter(|&ei| !all_entries[*ei].is_dir)
                                        .map(|&ei| all_entries[ei].path.clone())
                                        .collect();
                                    if !selected_files.is_empty() {
                                        return Ok(FileSelectResult::SelectedMultiple(
                                            selected_files,
                                        ));
                                    }
                                } else if let Some(&sel) = selected_indices.iter().next() {
                                    let entry = &all_entries[sel];
                                    if entry.is_dir {
                                        navigate_to(
                                            entry.path.clone(),
                                            &mut current_dir,
                                            &mut history,
                                            &mut history_index,
                                        );
                                        load_directory(
                                            &current_dir,
                                            &mut all_entries,
                                            self.directory,
                                            show_hidden,
                                        );
                                        update_filtered(
                                            &all_entries,
                                            &search_text,
                                            &mut filtered_entries,
                                            &self.filters,
                                        );
                                        selected_indices.clear();
                                        scroll_offset = 0;
                                        needs_redraw = true;
                                    } else if !self.directory {
                                        return Ok(FileSelectResult::Selected(entry.path.clone()));
                                    }
                                }
                            }
                            KEY_BACKSPACE => {
                                if let Some(parent) = current_dir.parent() {
                                    navigate_to(
                                        parent.to_path_buf(),
                                        &mut current_dir,
                                        &mut history,
                                        &mut history_index,
                                    );
                                    load_directory(
                                        &current_dir,
                                        &mut all_entries,
                                        self.directory,
                                        show_hidden,
                                    );
                                    update_filtered(
                                        &all_entries,
                                        &search_text,
                                        &mut filtered_entries,
                                        &self.filters,
                                    );
                                    selected_indices.clear();
                                    scroll_offset = 0;
                                    needs_redraw = true;
                                }
                            }
                            KEY_ESCAPE => {
                                return Ok(FileSelectResult::Cancelled);
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }

            // Process search input
            if search_input.process_event(&event) {
                let new_search = search_input.text().to_lowercase();
                if new_search != search_text {
                    search_text = new_search;
                    update_filtered(
                        &all_entries,
                        &search_text,
                        &mut filtered_entries,
                        &self.filters,
                    );
                    selected_indices.clear();
                    scroll_offset = 0;
                }
                needs_redraw = true;
            }

            // Process buttons
            needs_redraw |= ok_button.process_event(&event);
            needs_redraw |= cancel_button.process_event(&event);

            if ok_button.was_clicked() {
                if self.multiple && !selected_indices.is_empty() {
                    let selected_files: Vec<PathBuf> = selected_indices
                        .iter()
                        .filter(|&ei| !all_entries[*ei].is_dir)
                        .map(|&ei| all_entries[ei].path.clone())
                        .collect();
                    if !selected_files.is_empty() {
                        return Ok(FileSelectResult::SelectedMultiple(selected_files));
                    }
                } else if let Some(&sel) = selected_indices.iter().next() {
                    let entry = &all_entries[sel];
                    if self.directory && entry.is_dir {
                        return Ok(FileSelectResult::Selected(entry.path.clone()));
                    } else if !self.directory && !entry.is_dir {
                        return Ok(FileSelectResult::Selected(entry.path.clone()));
                    }
                } else if self.directory {
                    return Ok(FileSelectResult::Selected(current_dir.clone()));
                }
            }

            if cancel_button.was_clicked() {
                return Ok(FileSelectResult::Cancelled);
            }

            // Batch pending events
            while let Some(ev) = window.poll_for_event()? {
                if let WindowEvent::CloseRequested = ev {
                    return Ok(FileSelectResult::Closed);
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
                    &mut canvas,
                    colors,
                    &font,
                    &current_dir,
                    &quick_access,
                    &all_entries,
                    &filtered_entries,
                    &selected_indices,
                    scroll_offset,
                    hovered_quick_access,
                    hovered_entry,
                    show_hidden,
                    &search_input,
                    &ok_button,
                    &cancel_button,
                    &history,
                    history_index,
                    scale,
                );
                window.set_contents(&canvas)?;
            }
        }
    }
}

impl Default for FileSelectBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// Helper types and functions

struct DirEntry {
    name: String,
    path: PathBuf,
    is_dir: bool,
    size: u64,
    modified: Option<SystemTime>,
}

fn build_quick_access() -> Vec<QuickAccess> {
    let mut items = Vec::new();

    if let Some(home) = dirs::home_dir() {
        items.push(QuickAccess {
            name: "Home",
            path: home,
            icon: QuickAccessIcon::Home,
        });
    }
    if let Some(desktop) = dirs::desktop_dir() {
        items.push(QuickAccess {
            name: "Desktop",
            path: desktop,
            icon: QuickAccessIcon::Desktop,
        });
    }
    if let Some(docs) = dirs::document_dir() {
        items.push(QuickAccess {
            name: "Documents",
            path: docs,
            icon: QuickAccessIcon::Documents,
        });
    }
    if let Some(dl) = dirs::download_dir() {
        items.push(QuickAccess {
            name: "Downloads",
            path: dl,
            icon: QuickAccessIcon::Downloads,
        });
    }
    if let Some(pics) = dirs::picture_dir() {
        items.push(QuickAccess {
            name: "Pictures",
            path: pics,
            icon: QuickAccessIcon::Pictures,
        });
    }
    if let Some(music) = dirs::audio_dir() {
        items.push(QuickAccess {
            name: "Music",
            path: music,
            icon: QuickAccessIcon::Music,
        });
    }
    if let Some(videos) = dirs::video_dir() {
        items.push(QuickAccess {
            name: "Videos",
            path: videos,
            icon: QuickAccessIcon::Videos,
        });
    }

    items
}

fn load_directory(path: &Path, entries: &mut Vec<DirEntry>, dirs_only: bool, show_hidden: bool) {
    entries.clear();

    if let Some(parent) = path.parent() {
        entries.push(DirEntry {
            name: "..".to_string(),
            path: parent.to_path_buf(),
            is_dir: true,
            size: 0,
            modified: None,
        });
    }

    let mut dirs: Vec<DirEntry> = Vec::new();
    let mut files: Vec<DirEntry> = Vec::new();

    if let Ok(read_dir) = fs::read_dir(path) {
        for entry in read_dir.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();

            if !show_hidden && name.starts_with('.') {
                continue;
            }

            let metadata = entry.path().metadata().ok();
            let is_dir = metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false);

            if dirs_only && !is_dir {
                continue;
            }

            let size = metadata.as_ref().map(Metadata::len).unwrap_or(0);
            let modified = metadata.as_ref().and_then(|m| m.modified().ok());

            let de = DirEntry {
                name,
                path: entry.path(),
                is_dir,
                size,
                modified,
            };

            if is_dir {
                dirs.push(de);
            } else {
                files.push(de);
            }
        }
    }

    dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    entries.extend(dirs);
    entries.extend(files);
}

fn update_filtered(
    all: &[DirEntry],
    search: &str,
    filtered: &mut Vec<usize>,
    filters: &[FileFilter],
) {
    filtered.clear();
    for (i, entry) in all.iter().enumerate() {
        if entry.is_dir {
            filtered.push(i);
        } else {
            let matches_filter = filters.is_empty() || matches_any_filter(&entry.name, filters);
            let matches_search = search.is_empty() || entry.name.to_lowercase().contains(search);
            if matches_filter && matches_search {
                filtered.push(i);
            }
        }
    }
}

fn matches_any_filter(name: &str, filters: &[FileFilter]) -> bool {
    let name_lower = name.to_lowercase();
    for filter in filters {
        if matches_pattern(&name_lower, &filter.pattern) {
            return true;
        }
    }
    false
}

fn matches_pattern(name: &str, pattern: &str) -> bool {
    let pattern_lower = pattern.to_lowercase();
    if pattern_lower == "*" {
        return true;
    }

    if pattern_lower.starts_with("*") && pattern_lower.ends_with("*") {
        let inner = &pattern_lower[1..pattern_lower.len() - 1];
        name.contains(inner)
    } else if pattern_lower.starts_with("*") {
        let suffix = &pattern_lower[1..];
        name.ends_with(suffix)
    } else if pattern_lower.ends_with("*") {
        let prefix = &pattern_lower[..pattern_lower.len() - 1];
        name.starts_with(prefix)
    } else {
        name == &pattern_lower
    }
}

fn navigate_to(
    dest: PathBuf,
    current: &mut PathBuf,
    history: &mut Vec<PathBuf>,
    index: &mut usize,
) {
    // Truncate forward history
    history.truncate(*index + 1);
    history.push(dest.clone());
    *index = history.len() - 1;
    *current = dest;
}

fn darken(color: Rgba, amount: f32) -> Rgba {
    rgb(
        (color.r as f32 * (1.0 - amount)) as u8,
        (color.g as f32 * (1.0 - amount)) as u8,
        (color.b as f32 * (1.0 - amount)) as u8,
    )
}

fn truncate_name(name: &str, max_len: usize) -> String {
    if name.chars().count() > max_len {
        format!("{}...", name.chars().take(max_len - 3).collect::<String>())
    } else {
        name.to_string()
    }
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn format_date(time: Option<SystemTime>) -> String {
    match time {
        Some(t) => {
            let duration = t.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();
            let secs = duration.as_secs();
            // Simple date format (just show relative or basic)
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let diff = now.saturating_sub(secs);

            if diff < 60 {
                "Just now".to_string()
            } else if diff < 3600 {
                format!("{} min ago", diff / 60)
            } else if diff < 86400 {
                format!("{} hrs ago", diff / 3600)
            } else if diff < 86400 * 7 {
                format!("{} days ago", diff / 86400)
            } else {
                // Convert to date-ish
                let days_since_epoch = secs / 86400;
                let years = 1970 + days_since_epoch / 365;
                format!("{}", years)
            }
        }
        None => "-".to_string(),
    }
}

fn draw_nav_button(
    canvas: &mut Canvas,
    x: i32,
    y: i32,
    label: &str,
    enabled: bool,
    colors: &Colors,
    font: &Font,
    scale: f32,
) {
    let bg = if enabled {
        colors.button
    } else {
        darken(colors.button, 0.1)
    };
    let size = 28.0 * scale;
    canvas.fill_rounded_rect(x as f32, y as f32, size, size, 4.0 * scale, bg);

    let text_color = if enabled {
        colors.button_text
    } else {
        rgb(100, 100, 100)
    };
    let tc = font.render(label).with_color(text_color).finish();
    canvas.draw_canvas(&tc, x + (10.0 * scale) as i32, y + (6.0 * scale) as i32);
}

fn draw_toggle(
    canvas: &mut Canvas,
    x: i32,
    y: i32,
    label: &str,
    active: bool,
    colors: &Colors,
    font: &Font,
    scale: f32,
) {
    let bg = if active {
        colors.input_border_focused
    } else {
        colors.button
    };
    let size = 28.0 * scale;
    canvas.fill_rounded_rect(x as f32, y as f32, size, size, 4.0 * scale, bg);

    let text_color = if active {
        rgb(255, 255, 255)
    } else {
        colors.button_text
    };
    let tc = font.render(label).with_color(text_color).finish();
    canvas.draw_canvas(&tc, x + (6.0 * scale) as i32, y + (6.0 * scale) as i32);
}

fn draw_breadcrumbs(
    canvas: &mut Canvas,
    x: i32,
    y: i32,
    _max_w: u32,
    path: &Path,
    colors: &Colors,
    font: &Font,
) {
    let mut cx = x;
    let components: Vec<_> = path.components().collect();
    let start = if components.len() > 4 {
        components.len() - 4
    } else {
        0
    };

    if start > 0 {
        let tc = font.render("...").with_color(rgb(120, 120, 120)).finish();
        canvas.draw_canvas(&tc, cx, y);
        cx += tc.width() as i32 + 8;
    }

    for (i, comp) in components.iter().enumerate().skip(start) {
        let name = comp.as_os_str().to_string_lossy();
        let display = if name.is_empty() { "/" } else { &name };

        let is_last = i == components.len() - 1;
        let text_color = if is_last {
            colors.text
        } else {
            rgb(120, 120, 120)
        };

        let tc = font.render(display).with_color(text_color).finish();
        canvas.draw_canvas(&tc, cx, y);
        cx += tc.width() as i32;

        if !is_last {
            let sep = font.render(" / ").with_color(rgb(100, 100, 100)).finish();
            canvas.draw_canvas(&sep, cx, y);
            cx += sep.width() as i32;
        }
    }
}

fn draw_folder_icon(canvas: &mut Canvas, x: i32, y: i32, colors: &Colors, scale: f32) {
    let folder_color = rgb(240, 180, 70); // Golden folder
    let icon_size = (BASE_ICON_SIZE as f32 * scale) as f32;
    // Folder body
    canvas.fill_rounded_rect(
        x as f32,
        (y + (4.0 * scale) as i32) as f32,
        icon_size,
        14.0 * scale,
        2.0 * scale,
        folder_color,
    );
    // Folder tab
    canvas.fill_rounded_rect(
        x as f32,
        y as f32,
        10.0 * scale,
        6.0 * scale,
        2.0 * scale,
        folder_color,
    );
    let _ = colors;
}

fn draw_file_icon(canvas: &mut Canvas, x: i32, y: i32, name: &str, colors: &Colors, scale: f32) {
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    let icon_size = (BASE_ICON_SIZE as f32 * scale) as f32;

    let icon_color = match ext.as_str() {
        "rs" => rgb(220, 120, 70),          // Rust orange
        "py" => rgb(70, 130, 180),          // Python blue
        "js" | "ts" => rgb(240, 220, 80),   // JS yellow
        "html" | "htm" => rgb(220, 80, 50), // HTML red
        "css" => rgb(80, 120, 200),         // CSS blue
        "json" | "yaml" | "yml" | "toml" => rgb(150, 150, 150),
        "md" | "txt" => rgb(180, 180, 180),
        "png" | "jpg" | "jpeg" | "gif" | "svg" => rgb(100, 180, 100), // Green for images
        _ => rgb(160, 160, 160),
    };

    // File body
    canvas.fill_rounded_rect(
        x as f32,
        y as f32,
        16.0 * scale,
        icon_size,
        2.0 * scale,
        icon_color,
    );
    // Folded corner
    canvas.fill_rect(
        (x + (10.0 * scale) as i32) as f32,
        y as f32,
        6.0 * scale,
        6.0 * scale,
        darken(icon_color, 0.2),
    );
    let _ = colors;
}

fn draw_quick_access_icon(
    canvas: &mut Canvas,
    x: i32,
    y: i32,
    icon: QuickAccessIcon,
    colors: &Colors,
    scale: f32,
) {
    let color = match icon {
        QuickAccessIcon::Home => rgb(100, 180, 100),
        QuickAccessIcon::Desktop => rgb(120, 120, 200),
        QuickAccessIcon::Documents => rgb(200, 180, 100),
        QuickAccessIcon::Downloads => rgb(100, 160, 220),
        QuickAccessIcon::Pictures => rgb(180, 120, 180),
        QuickAccessIcon::Music => rgb(220, 120, 120),
        QuickAccessIcon::Videos => rgb(180, 100, 200),
    };

    canvas.fill_rounded_rect(
        x as f32,
        y as f32,
        16.0 * scale,
        16.0 * scale,
        3.0 * scale,
        color,
    );
    let _ = colors;
}
