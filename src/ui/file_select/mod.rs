//! File selection dialog implementation with enhanced UI.

mod breadcrumbs;
mod draw;
mod fs;

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use breadcrumbs::{breadcrumb_layout, draw_breadcrumbs};
use draw::{
    MAX_POPUP_ITEMS, SidebarGlyph, draw_completion_popup, draw_mount_icon, draw_place_icon,
    file_icon_color, folder_tint,
};
use fs::{
    Browser, DirEntry, MountPoint, QuickAccess, build_quick_access, find_all_completions,
    format_date, format_size, get_mount_icon, get_mounted_drives,
};

use crate::{
    backend::{CursorShape, MouseButton, Window, WindowEvent},
    error::Error,
    render::{Canvas, Font},
    ui::{
        BASE_CORNER_RADIUS, BASE_MIN_THUMB, Colors, KEY_BACKSPACE, KEY_DOWN, KEY_ESCAPE,
        KEY_RETURN, KEY_UP, Thumb, button_row_y, ellipsize, icons, open_window, place_ok_cancel,
        widgets::{Widget, button::Button, text_input::TextInput},
    },
};

// Layout constants (logical, at scale 1.0)
const BASE_WINDOW_WIDTH: u32 = 700;
const BASE_WINDOW_HEIGHT: u32 = 500;
const BASE_PADDING: u32 = 12;
const BASE_SIDEBAR_WIDTH: u32 = 164;
const BASE_TOOLBAR_HEIGHT: u32 = 36;
const BASE_TOOLBAR_BUTTON: u32 = 28;
const BASE_PATH_BAR_HEIGHT: u32 = 34;
const BASE_SEARCH_WIDTH: u32 = 200;
const BASE_ITEM_HEIGHT: u32 = 30;
const BASE_ICON_SIZE: u32 = 18;
const BASE_SECTION_HEADER_HEIGHT: u32 = 26;

// Column widths (logical)
const BASE_SIZE_COL_WIDTH: u32 = 90;
const BASE_DATE_COL_WIDTH: u32 = 110;
const BASE_COLUMN_HEADER_HEIGHT: u32 = 28;
const BASE_FILENAME_ROW_HEIGHT: u32 = 58;
const BASE_FOOTER_HEIGHT: u32 = 44;
const BASE_CONTENT_GAP: u32 = 12;
const BASE_FILENAME_LABEL_HEIGHT: u32 = 20;
const BASE_ROW_RADIUS: f32 = 6.0;
const BASE_SCROLLBAR_GUTTER: u32 = 14;

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
            FileSelectResult::Closed => 1,
        }
    }
}

/// A toolbar button, laid out once and shared by drawing and hit-testing.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ToolbarAction {
    Back,
    Forward,
    Up,
    Home,
    ToggleHidden,
}

impl ToolbarAction {
    /// Tooltip shown while hovering, since the buttons carry no label.
    fn tooltip(self, show_hidden: bool) -> &'static str {
        match self {
            ToolbarAction::Back => "Back",
            ToolbarAction::Forward => "Forward",
            ToolbarAction::Up => "Parent folder",
            ToolbarAction::Home => "Home",
            ToolbarAction::ToggleHidden if show_hidden => "Hide hidden files",
            ToolbarAction::ToggleHidden => "Show hidden files",
        }
    }
}

struct ToolbarButton {
    action: ToolbarAction,
    x: i32,
    y: i32,
    size: u32,
}

impl ToolbarButton {
    fn contains(&self, mx: i32, my: i32) -> bool {
        mx >= self.x
            && mx < self.x + self.size as i32
            && my >= self.y
            && my < self.y + self.size as i32
    }
}

/// A sidebar line, laid out once and shared by drawing and hit-testing.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SidebarRow {
    Header(&'static str),
    Place(usize),
    Drive(usize),
}

/// File filter pattern.
#[derive(Debug, Clone)]
pub struct FileFilter {
    pub name: String,
    pub patterns: Vec<String>,
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

    pub fn show(mut self) -> Result<FileSelectResult, Error> {
        let colors = self.colors.unwrap_or_else(|| crate::ui::detect_theme());

        // Save mode flag
        let save_mode = self.save && !self.directory;

        // Smallest window that still fits the fixed chrome plus one item row
        let min_logical_width = BASE_PADDING * 2
            + BASE_SIDEBAR_WIDTH
            + BASE_CONTENT_GAP
            + BASE_ITEM_HEIGHT
            + if self.directory {
                0
            } else {
                BASE_SIZE_COL_WIDTH
            }
            + BASE_DATE_COL_WIDTH
            + BASE_SCROLLBAR_GUTTER;
        let min_logical_height = BASE_PADDING * 2
            + BASE_TOOLBAR_HEIGHT
            + BASE_CONTENT_GAP
            + BASE_PATH_BAR_HEIGHT
            + BASE_COLUMN_HEADER_HEIGHT
            + BASE_ITEM_HEIGHT
            + BASE_FOOTER_HEIGHT
            + if save_mode {
                BASE_FILENAME_ROW_HEIGHT
            } else {
                0
            };

        // Use custom dimensions if provided, otherwise use defaults
        let logical_width = self
            .width
            .unwrap_or(BASE_WINDOW_WIDTH)
            .max(min_logical_width);
        let logical_height = self
            .height
            .unwrap_or(BASE_WINDOW_HEIGHT)
            .max(min_logical_height);

        // Create window with LOGICAL dimensions first
        // Resolved here rather than inside open_window because the save-mode
        // filename label reuses it.
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
        let (mut window, scale, window_width, window_height) =
            open_window(title, "", logical_width, logical_height)?;

        // Now create everything at PHYSICAL scale
        let font = Font::load(scale);

        // Scale dimensions for physical rendering
        let padding = (BASE_PADDING as f32 * scale) as u32;
        let sidebar_width = (BASE_SIDEBAR_WIDTH as f32 * scale) as u32;
        let toolbar_height = (BASE_TOOLBAR_HEIGHT as f32 * scale) as u32;
        let path_bar_height = (BASE_PATH_BAR_HEIGHT as f32 * scale) as u32;
        let search_width = (BASE_SEARCH_WIDTH as f32 * scale) as u32;
        let item_height = (BASE_ITEM_HEIGHT as f32 * scale) as u32;

        // Load mounted drives
        let mounted_drives = get_mounted_drives();

        // Create UI elements at physical scale
        let mut ok_button =
            Button::new(if self.save { "Save" } else { "Open" }, &font, scale).primary();
        let mut cancel_button = Button::new("Cancel", &font, scale);

        // Search input
        let mut search_input = TextInput::new(search_width)
            .with_scale(scale)
            .with_placeholder("Search...");

        // Current state
        // Resolve the initial directory (and optional preselected file name) from
        // --filename / start_path. A directory opens in place; a file path opens
        // its parent and yields the file name for preselection (zenity semantics).
        let (current_dir, preselected_name) = match &self.start_path {
            Some(p) => (p.clone(), None),
            None if self.filename.is_empty() => {
                (dirs::home_dir().unwrap_or_else(|| PathBuf::from("/")), None)
            }
            None => {
                let path = Path::new(&self.filename);
                if path.is_dir() {
                    (path.to_path_buf(), None)
                } else {
                    let name = path.file_name().map(|n| n.to_string_lossy().to_string());
                    let dir = path
                        .parent()
                        .filter(|p| !p.as_os_str().is_empty() && p.is_dir())
                        .map(|p| p.to_path_buf())
                        .unwrap_or_else(|| {
                            std::env::current_dir().unwrap_or_else(|_| {
                                dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
                            })
                        });
                    (dir, name)
                }
            }
        };
        // Build quick access locations
        let quick_access = build_quick_access(&current_dir);

        let mut search_text = String::new();
        let mut browser = Browser::new(
            current_dir,
            self.directory,
            std::mem::take(&mut self.filters),
            &search_text,
        );
        let mut hovered_sidebar: Option<SidebarRow> = None;
        let mut hovered_entry: Option<usize> = None;
        let mut hovered_toolbar: Option<ToolbarAction> = None;

        // Tab-completion state for filename input (save mode)
        let mut completion_matches: Vec<String> = Vec::new();
        let mut completion_popup_index: usize = 0;

        // Tab-completion state for search input
        let mut search_matches: Vec<String> = Vec::new();
        let mut search_popup_index: usize = 0;

        let mut window_dragging = false;

        // Scrollbar thumb dragging state
        let mut thumb_drag = false;
        let mut thumb_drag_offset: Option<i32> = None;
        let mut scrollbar_hovered = false;

        // Calculate layout in physical coordinates
        let filename_row_height = if save_mode {
            (BASE_FILENAME_ROW_HEIGHT as f32 * scale) as u32
        } else {
            0
        };
        let content_gap = (BASE_CONTENT_GAP as f32 * scale) as u32;
        let footer_height = (BASE_FOOTER_HEIGHT as f32 * scale) as u32;
        let sidebar_x = padding as i32;
        let sidebar_y = (padding + toolbar_height + content_gap) as i32;
        let sidebar_h = window_height
            - padding * 2
            - toolbar_height
            - content_gap
            - footer_height
            - filename_row_height;

        let main_x = (padding + sidebar_width + content_gap) as i32;
        let main_y = sidebar_y;
        let main_w = window_width - padding * 2 - sidebar_width - content_gap;
        let main_h = sidebar_h;

        let header_offset = (BASE_COLUMN_HEADER_HEIGHT as f32 * scale) as u32;
        let list_y = main_y + path_bar_height as i32 + header_offset as i32;
        let list_h = main_h - path_bar_height - header_offset;
        let visible_items = (list_h / item_height) as usize;

        // Column layout inside the file pane. Directory mode never shows a size,
        // so the column is dropped and the name column takes the space.
        let size_col_width = if self.directory {
            0
        } else {
            (BASE_SIZE_COL_WIDTH as f32 * scale) as u32
        };
        let date_col_width = (BASE_DATE_COL_WIDTH as f32 * scale) as u32;
        let scrollbar_gutter = (BASE_SCROLLBAR_GUTTER as f32 * scale) as u32;
        let name_col_width = main_w
            .saturating_sub(size_col_width + date_col_width + scrollbar_gutter)
            .max(item_height);
        let size_col_x = main_x + name_col_width as i32;
        let date_col_x = size_col_x + size_col_width as i32 + (12.0 * scale) as i32;

        // Preselect the file named by --filename in single-selection open mode,
        // scrolling it into view.
        if let Some((idx, pos)) = (!save_mode && !self.directory && !self.multiple)
            .then_some(())
            .and(preselected_name.as_deref())
            .and_then(|name| {
                browser
                    .filtered_entries
                    .iter()
                    .enumerate()
                    .find_map(|(pos, &i)| {
                        browser.all_entries[i]
                            .name
                            .eq_ignore_ascii_case(name)
                            .then_some((i, pos))
                    })
            })
        {
            browser.selected_indices.insert(idx);
            browser.scroll_offset = if pos < browser.scroll_offset {
                pos
            } else if pos >= browser.scroll_offset + visible_items {
                pos + 1 - visible_items
            } else {
                browser.scroll_offset
            };
        }

        // Sidebar lines, laid out once for both drawing and hit-testing
        let section_header_height = (BASE_SECTION_HEADER_HEIGHT as f32 * scale) as u32;
        let sidebar_rows: Vec<(SidebarRow, i32)> = {
            let mut rows = Vec::new();
            let mut y = sidebar_y;
            rows.push((SidebarRow::Header("Places"), y));
            y += section_header_height as i32;
            for i in 0..quick_access.len() {
                rows.push((SidebarRow::Place(i), y));
                y += item_height as i32;
            }
            if !mounted_drives.is_empty() {
                y += content_gap as i32;
                rows.push((SidebarRow::Header("Drives"), y));
                y += section_header_height as i32;
                for i in 0..mounted_drives.len() {
                    rows.push((SidebarRow::Drive(i), y));
                    y += item_height as i32;
                }
            }
            rows
        };
        let sidebar_row_at = |mx: i32, my: i32| -> Option<SidebarRow> {
            if mx < sidebar_x || mx >= sidebar_x + sidebar_width as i32 {
                return None;
            }
            sidebar_rows
                .iter()
                .find(|(row, y)| {
                    !matches!(row, SidebarRow::Header(_))
                        && my >= *y
                        && my < *y + item_height as i32
                })
                .map(|(row, _)| *row)
        };

        // Toolbar buttons, laid out once for both drawing and hit-testing
        let toolbar_button_size = (BASE_TOOLBAR_BUTTON as f32 * scale) as u32;
        let toolbar_buttons: Vec<ToolbarButton> = {
            let y = padding as i32 + (toolbar_height as i32 - toolbar_button_size as i32) / 2;
            let tight_gap = (4.0 * scale) as i32;
            let group_gap = (16.0 * scale) as i32;
            let mut x = padding as i32;
            let mut buttons = Vec::new();
            for (i, action) in [
                ToolbarAction::Back,
                ToolbarAction::Forward,
                ToolbarAction::Up,
                ToolbarAction::Home,
                ToolbarAction::ToggleHidden,
            ]
            .into_iter()
            .enumerate()
            {
                if i > 0 {
                    let gap = if i == 2 || i == 4 {
                        group_gap
                    } else {
                        tight_gap
                    };
                    x += toolbar_button_size as i32 + gap;
                }
                buttons.push(ToolbarButton {
                    action,
                    x,
                    y,
                    size: toolbar_button_size,
                });
            }
            buttons
        };

        // Position buttons
        let button_y = button_row_y(window_height, padding, scale);
        place_ok_cancel(
            &mut ok_button,
            &mut cancel_button,
            window_width,
            padding,
            button_y,
            scale,
        );

        // Position filename area (label above, full-width input below, save mode only)
        let filename_y = button_y - filename_row_height as i32;
        let filename_label_h = (BASE_FILENAME_LABEL_HEIGHT as f32 * scale) as i32;
        let mut filename_input = if save_mode {
            let mut input = TextInput::new(main_w)
                .with_scale(scale)
                .with_placeholder("Enter filename...");
            if let Some(name) = &preselected_name {
                input = input.with_default_text(name);
            }
            input.set_focus(true);
            input.set_position(main_x, filename_y + filename_label_h);
            Some(input)
        } else {
            None
        };

        // Position search input, centered in the toolbar row
        let search_height = search_input.height();
        let search_x = window_width as i32 - padding as i32 - search_width as i32;
        let search_y = padding as i32 + (toolbar_height as i32 - search_input.height() as i32) / 2;
        search_input.set_position(search_x, search_y);

        // Create canvas at PHYSICAL dimensions
        let mut canvas = Canvas::new(window_width, window_height);

        // Chrome-layer cache. The static parts of the dialog (background, toolbar,
        // nav buttons, sidebar, path bar, column headers) are re-rendered only
        // when one of these chrome-affecting values changes; during scroll only the
        // file list is repainted on top.
        #[derive(Clone, PartialEq)]
        struct ChromeSig {
            dir: PathBuf,
            show_hidden: bool,
            hovered_sidebar: Option<SidebarRow>,
            hovered_toolbar: Option<ToolbarAction>,
            history_index: usize,
            history_len: usize,
            search: String,
            search_focused: bool,
            search_caret: (usize, Option<usize>),
            filename: Option<(String, bool)>,
            qa_len: usize,
            drives_len: usize,
        }
        let mut chrome_canvas = Canvas::new(window_width, window_height);
        let mut chrome_sig: Option<ChromeSig> = None;

        let mut mouse_x = 0i32;
        let mut mouse_y = 0i32;

        // Text sits on a common baseline: rendered canvases are cropped to their
        // glyph bounds, so centering them individually would misalign rows.
        let baseline_in = |top: i32, height: u32| -> i32 {
            top + ((height as f32 - font.line_height()) / 2.0 + font.ascent()).round() as i32
        };

        // Completion popup rects (x, y, w, h), shared by drawing and hit-testing.
        // The search popup drops below its field; the filename popup rises above it.
        let popup_height =
            |count: usize| (count.min(MAX_POPUP_ITEMS) as i32) * POPUP_ITEM_HEIGHT + 2;
        let search_popup_rect = |count: usize| {
            (
                search_x,
                search_y + search_height as i32 + (4.0 * scale) as i32,
                search_width as i32,
                popup_height(count),
            )
        };
        let filename_popup_rect = |count: usize| {
            (
                main_x,
                filename_y + filename_label_h - popup_height(count),
                main_w as i32,
                popup_height(count),
            )
        };
        let popup_item_at = |rect: (i32, i32, i32, i32), count: usize, mx: i32, my: i32| {
            let (x, y, w, h) = rect;
            if mx < x || mx >= x + w || my < y || my >= y + h {
                return None;
            }
            let idx = ((my - y - 1) / POPUP_ITEM_HEIGHT) as usize;
            (idx < count.min(MAX_POPUP_ITEMS)).then_some(idx)
        };

        // Which popup item, if any, sits under the pointer. `true` marks the search
        // popup, `false` the save-mode filename popup.
        let popup_item_hit =
            |search: &[String], completion: &[String], search_focused: bool, mx: i32, my: i32| {
                if !search.is_empty() && search_focused {
                    popup_item_at(search_popup_rect(search.len()), search.len(), mx, my)
                        .map(|i| (i, true))
                } else if !completion.is_empty() {
                    popup_item_at(
                        filename_popup_rect(completion.len()),
                        completion.len(),
                        mx,
                        my,
                    )
                    .map(|i| (i, false))
                } else {
                    None
                }
            };

        // Scrollbar thumb rect (x, y, w, h), shared by drawing, hover, hit-testing
        // and drag. None when every entry already fits.
        let scrollbar_thumb = |total: usize, scroll: usize, hovered: bool| {
            let thumb = Thumb::new(
                list_h as f32,
                visible_items as f32,
                total as f32,
                scroll as f32,
                BASE_MIN_THUMB * scale,
            )?;
            let w = if hovered { 8.0 * scale } else { 5.0 * scale };
            let x = main_x as f32 + main_w as f32 - scrollbar_gutter as f32 / 2.0 - w / 2.0;
            Some((x, list_y as f32 + thumb.offset, w, thumb.len))
        };

        // Chrome layer: everything that only changes on navigation or hover.
        let draw_chrome = |canvas: &mut Canvas,
                           colors: &Colors,
                           font: &Font,
                           current_dir: &Path,
                           quick_access: &[QuickAccess],
                           mounted_drives: &[MountPoint],
                           hovered_sidebar: Option<SidebarRow>,
                           hovered_toolbar: Option<ToolbarAction>,
                           can_go_back: bool,
                           can_go_forward: bool,
                           show_hidden: bool,
                           search_input: &TextInput,
                           scale: f32| {
            let width = canvas.width() as f32;
            let height = canvas.height() as f32;
            let radius = BASE_CORNER_RADIUS * scale;
            let pane_radius = 8.0 * scale;
            let row_radius = BASE_ROW_RADIUS * scale;
            let toolbar_icon = 17.0 * scale;
            let sidebar_icon = 16.0 * scale;

            canvas.fill_dialog_bg(
                width,
                height,
                colors.window_bg,
                colors.window_border,
                colors.window_shadow,
                radius,
            );

            // ===== TOOLBAR =====
            for button in &toolbar_buttons {
                let enabled = match button.action {
                    ToolbarAction::Back => can_go_back,
                    ToolbarAction::Forward => can_go_forward,
                    ToolbarAction::Up => current_dir.parent().is_some(),
                    ToolbarAction::Home | ToolbarAction::ToggleHidden => true,
                };
                let active = button.action == ToolbarAction::ToggleHidden && show_hidden;

                if active {
                    canvas.fill_rounded_rect(
                        button.x as f32,
                        button.y as f32,
                        button.size as f32,
                        button.size as f32,
                        row_radius,
                        colors.accent,
                    );
                } else if enabled && hovered_toolbar == Some(button.action) {
                    canvas.fill_rounded_rect(
                        button.x as f32,
                        button.y as f32,
                        button.size as f32,
                        button.size as f32,
                        row_radius,
                        colors.row_hover,
                    );
                }

                let tint = if active {
                    colors.accent_text
                } else if enabled {
                    colors.text
                } else {
                    colors.text_muted.with_alpha(110)
                };
                let ix = button.x as f32 + (button.size as f32 - toolbar_icon) / 2.0;
                let iy = button.y as f32 + (button.size as f32 - toolbar_icon) / 2.0;
                match button.action {
                    ToolbarAction::Back => icons::chevron_left(canvas, ix, iy, toolbar_icon, tint),
                    ToolbarAction::Forward => {
                        icons::chevron_right(canvas, ix, iy, toolbar_icon, tint)
                    }
                    ToolbarAction::Up => icons::arrow_up(canvas, ix, iy, toolbar_icon, tint),
                    ToolbarAction::Home => icons::home(canvas, ix, iy, toolbar_icon, tint),
                    ToolbarAction::ToggleHidden => {
                        if show_hidden {
                            icons::eye(canvas, ix, iy, toolbar_icon, tint)
                        } else {
                            icons::eye_off(canvas, ix, iy, toolbar_icon, tint)
                        }
                    }
                }
            }

            search_input.draw_to(canvas, colors, font);
            icons::search(
                canvas,
                (search_x + search_input.width() as i32) as f32 - 26.0 * scale,
                search_y as f32 + (search_input.height() as f32 - 15.0 * scale) / 2.0,
                15.0 * scale,
                colors.text_muted,
            );

            canvas.fill_rect(
                1.0,
                (padding + toolbar_height) as f32 + content_gap as f32 / 2.0,
                width - 2.0,
                1.0,
                colors.separator,
            );

            // ===== SIDEBAR =====
            for (row, y) in &sidebar_rows {
                let y = *y;
                let (icon, label, is_current) = match row {
                    SidebarRow::Header(label) => {
                        let (text, base) = font
                            .render(label)
                            .with_color(colors.text_muted)
                            .finish_with_baseline();
                        canvas.draw_canvas(
                            &text,
                            sidebar_x + (8.0 * scale) as i32,
                            baseline_in(y, section_header_height) - base,
                        );
                        continue;
                    }
                    SidebarRow::Place(i) => {
                        let qa = &quick_access[*i];
                        (
                            SidebarGlyph::Place(qa.icon),
                            qa.name.clone(),
                            qa.path == current_dir,
                        )
                    }
                    SidebarRow::Drive(i) => {
                        let drive = &mounted_drives[*i];
                        let label = drive.label.clone().unwrap_or_else(|| {
                            drive
                                .mount_point
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or(&drive.device)
                                .to_string()
                        });
                        (
                            SidebarGlyph::Mount(get_mount_icon(&drive.device)),
                            label,
                            drive.mount_point == current_dir,
                        )
                    }
                };

                let fill = if is_current {
                    Some(colors.row_selected)
                } else if hovered_sidebar == Some(*row) {
                    Some(colors.row_hover)
                } else {
                    None
                };
                if let Some(fill) = fill {
                    canvas.fill_rounded_rect(
                        sidebar_x as f32,
                        y as f32,
                        sidebar_width as f32,
                        item_height as f32,
                        row_radius,
                        fill,
                    );
                }

                let icon_tint = if is_current {
                    colors.accent
                } else {
                    colors.text_muted
                };
                let icon_x = sidebar_x as f32 + 10.0 * scale;
                let icon_y = y as f32 + (item_height as f32 - sidebar_icon) / 2.0;
                match icon {
                    SidebarGlyph::Place(kind) => {
                        draw_place_icon(canvas, icon_x, icon_y, sidebar_icon, kind, icon_tint)
                    }
                    SidebarGlyph::Mount(kind) => {
                        draw_mount_icon(canvas, icon_x, icon_y, sidebar_icon, kind, icon_tint)
                    }
                }

                let text_x = sidebar_x + (34.0 * scale) as i32;
                let avail =
                    (sidebar_x + sidebar_width as i32 - text_x - (8.0 * scale) as i32).max(0);
                let label = ellipsize(&label, font, avail as f32);
                let (text, base) = font
                    .render(&label)
                    .with_color(colors.text)
                    .finish_with_baseline();
                canvas.draw_canvas(&text, text_x, baseline_in(y, item_height) - base);
            }

            // ===== FILE PANE =====
            canvas.fill_rounded_rect(
                main_x as f32,
                main_y as f32,
                main_w as f32,
                main_h as f32,
                pane_radius,
                colors.surface_alt,
            );

            draw_breadcrumbs(
                canvas,
                main_x + (12.0 * scale) as i32,
                baseline_in(main_y, path_bar_height),
                main_w - (24.0 * scale) as u32,
                current_dir,
                colors,
                font,
                scale,
            );

            let header_y = main_y + path_bar_height as i32;
            canvas.fill_rect(
                main_x as f32,
                header_y as f32,
                main_w as f32,
                1.0,
                colors.separator,
            );

            let header_baseline = baseline_in(header_y, header_offset);
            let mut header_label = |text: &str, x: i32, right_align: bool| {
                let (text, base) = font
                    .render(text)
                    .with_color(colors.text_muted)
                    .finish_with_baseline();
                let x = if right_align {
                    x - text.width() as i32
                } else {
                    x
                };
                canvas.draw_canvas(&text, x, header_baseline - base);
            };
            header_label("Name", main_x + (38.0 * scale) as i32, false);
            if size_col_width > 0 {
                header_label("Size", size_col_x + size_col_width as i32, true);
            }
            header_label("Modified", date_col_x, false);

            canvas.fill_rect(
                main_x as f32,
                (header_y + header_offset as i32 - 1) as f32,
                main_w as f32,
                1.0,
                colors.separator,
            );
        };

        // Dynamic layer: the scrollable file list + scrollbar + inputs + buttons.
        // Redrawn every frame on top of the cached chrome.
        let draw_dynamic = |canvas: &mut Canvas,
                            colors: &Colors,
                            font: &Font,
                            all_entries: &[DirEntry],
                            filtered_entries: &[usize],
                            selected_indices: &HashSet<usize>,
                            scroll_offset: usize,
                            hovered_entry: Option<usize>,
                            scale: f32,
                            scrollbar_hovered: bool,
                            hovered_toolbar: Option<ToolbarAction>,
                            show_hidden: bool,
                            ok_button: &Button,
                            cancel_button: &Button,
                            filename_input: Option<&TextInput>| {
            let row_radius = BASE_ROW_RADIUS * scale;
            let icon_size = BASE_ICON_SIZE as f32 * scale;
            let row_inset = (3.0 * scale).max(1.0);

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

                if is_selected || is_hovered {
                    canvas.fill_rounded_rect(
                        main_x as f32 + row_inset,
                        y as f32,
                        main_w as f32 - row_inset * 2.0,
                        item_height as f32,
                        row_radius,
                        if is_selected {
                            colors.row_selected
                        } else {
                            colors.row_hover
                        },
                    );
                }

                let icon_x = main_x as f32 + 10.0 * scale;
                let icon_y = y as f32 + (item_height as f32 - icon_size) / 2.0;
                if entry.is_dir {
                    icons::folder(canvas, icon_x, icon_y, icon_size, folder_tint(colors));
                } else {
                    icons::document(
                        canvas,
                        icon_x,
                        icon_y,
                        icon_size,
                        file_icon_color(&entry.name, colors),
                    );
                }

                let name_x = main_x + (38.0 * scale) as i32;
                let name_w = (size_col_x - name_x - (12.0 * scale) as i32).max(0) as f32;
                let baseline = baseline_in(y, item_height);
                let name = ellipsize(&entry.name, font, name_w);
                let (name_canvas, base) = font
                    .render(&name)
                    .with_color(colors.text)
                    .finish_with_baseline();
                canvas.draw_canvas(&name_canvas, name_x, baseline - base);

                if !entry.is_dir {
                    let (size_canvas, base) = font
                        .render(&format_size(entry.size))
                        .with_color(colors.text_muted)
                        .finish_with_baseline();
                    canvas.draw_canvas(
                        &size_canvas,
                        size_col_x + size_col_width as i32 - size_canvas.width() as i32,
                        baseline - base,
                    );
                }

                let (date_canvas, base) = font
                    .render(&format_date(entry.modified))
                    .with_color(colors.text_muted)
                    .finish_with_baseline();
                canvas.draw_canvas(&date_canvas, date_col_x, baseline - base);
            }

            // Scrollbar
            if let Some((x, y, w, h)) =
                scrollbar_thumb(filtered_entries.len(), scroll_offset, scrollbar_hovered)
            {
                canvas.fill_rounded_rect(
                    x,
                    y,
                    w,
                    h,
                    w / 2.0,
                    if scrollbar_hovered {
                        colors.accent
                    } else {
                        colors.text_muted.with_alpha(130)
                    },
                );
            }

            canvas.stroke_rounded_rect(
                main_x as f32,
                main_y as f32,
                main_w as f32,
                main_h as f32,
                8.0 * scale,
                colors.separator,
                1.0,
            );

            // Filename input (save mode): label above, input below
            if let Some(fi) = filename_input {
                let label_canvas = font.render(title).with_color(colors.text_muted).finish();
                canvas.draw_canvas(&label_canvas, main_x, filename_y + (2.0 * scale) as i32);
                fi.draw_to(canvas, colors, font);
            }

            ok_button.draw_to(canvas, colors, font);
            cancel_button.draw_to(canvas, colors, font);

            let status = format!(
                "{} item{}",
                filtered_entries.len(),
                if filtered_entries.len() == 1 { "" } else { "s" }
            );
            let (status_canvas, base) = font
                .render(&status)
                .with_color(colors.text_muted)
                .finish_with_baseline();
            canvas.draw_canvas(
                &status_canvas,
                padding as i32,
                baseline_in(button_y, ok_button.height()) - base,
            );

            // Tooltip for the hovered toolbar button
            if let Some(button) = hovered_toolbar
                .and_then(|action| toolbar_buttons.iter().find(|b| b.action == action))
            {
                let (label, base) = font
                    .render(button.action.tooltip(show_hidden))
                    .with_color(colors.text)
                    .finish_with_baseline();
                let pad_x = (8.0 * scale) as i32;
                let tip_h = (24.0 * scale) as u32;
                let tip_w = label.width() as i32 + pad_x * 2;
                let tip_x = (button.x + button.size as i32 / 2 - tip_w / 2)
                    .clamp(padding as i32, window_width as i32 - padding as i32 - tip_w);
                let tip_y = button.y + button.size as i32 + (6.0 * scale) as i32;
                canvas.fill_rounded_rect(
                    tip_x as f32,
                    tip_y as f32,
                    tip_w as f32,
                    tip_h as f32,
                    BASE_ROW_RADIUS * scale,
                    colors.surface,
                );
                canvas.stroke_rounded_rect(
                    tip_x as f32,
                    tip_y as f32,
                    tip_w as f32,
                    tip_h as f32,
                    BASE_ROW_RADIUS * scale,
                    colors.separator,
                    1.0,
                );
                canvas.draw_canvas(&label, tip_x + pad_x, baseline_in(tip_y, tip_h) - base);
            }
        };

        // Initial draw
        let sig = ChromeSig {
            dir: browser.current_dir.to_path_buf(),
            show_hidden: browser.show_hidden,
            hovered_sidebar,
            hovered_toolbar,
            history_index: browser.history_step().0,
            history_len: browser.history_step().1,
            search: search_input.text().to_owned(),
            search_focused: search_input.has_focus(),
            search_caret: search_input.caret(),
            filename: filename_input
                .as_ref()
                .map(|f| (f.text().to_owned(), f.has_focus())),
            qa_len: quick_access.len(),
            drives_len: mounted_drives.len(),
        };
        if chrome_sig.as_ref() != Some(&sig) {
            draw_chrome(
                &mut chrome_canvas,
                colors,
                &font,
                &browser.current_dir,
                &quick_access,
                &mounted_drives,
                hovered_sidebar,
                hovered_toolbar,
                browser.can_go_back(),
                browser.can_go_forward(),
                browser.show_hidden,
                &search_input,
                scale,
            );
            chrome_sig = Some(sig);
        }
        canvas.blit_region(&chrome_canvas, 0, 0, window_width, window_height, 0, 0);
        draw_dynamic(
            &mut canvas,
            colors,
            &font,
            &browser.all_entries,
            &browser.filtered_entries,
            &browser.selected_indices,
            browser.scroll_offset,
            hovered_entry,
            scale,
            scrollbar_hovered,
            hovered_toolbar,
            browser.show_hidden,
            &ok_button,
            &cancel_button,
            filename_input.as_ref(),
        );
        if save_mode && !completion_matches.is_empty() {
            let (x, y, _, _) = filename_popup_rect(completion_matches.len());
            draw_completion_popup(
                &mut canvas,
                &font,
                colors,
                &completion_matches,
                completion_popup_index,
                (x, y),
                main_w,
            );
        }
        if !search_matches.is_empty() && search_input.has_focus() {
            let (x, y, _, _) = search_popup_rect(search_matches.len());
            draw_completion_popup(
                &mut canvas,
                &font,
                colors,
                &search_matches,
                search_popup_index,
                (x, y),
                search_width,
            );
        }
        window.set_contents(&canvas)?;
        window.show()?;

        // Event loop
        loop {
            let event = window.wait_for_event()?;
            let mut needs_redraw = false;
            let mut enter_pressed = false;
            let mut ok_pressed = false;

            match &event {
                WindowEvent::CloseRequested => return Ok(FileSelectResult::Closed),
                WindowEvent::RedrawRequested => needs_redraw = true,
                WindowEvent::CursorEnter(pos) | WindowEvent::CursorMove(pos) => {
                    if window_dragging {
                        let _ = window.start_drag();
                        window_dragging = false;
                    }

                    mouse_x = pos.x as i32;
                    mouse_y = pos.y as i32;

                    // Handle scrollbar thumb dragging
                    if thumb_drag
                        && let Some((_, _, _, thumb_h)) = scrollbar_thumb(
                            browser.filtered_entries.len(),
                            browser.scroll_offset,
                            true,
                        )
                    {
                        let max_scroll = browser.filtered_entries.len() - visible_items;
                        let travel = list_h as f32 - thumb_h;
                        let offset = thumb_drag_offset.unwrap_or(thumb_h as i32 / 2);
                        let thumb_y = (mouse_y - list_y - offset).clamp(0, travel.max(0.0) as i32);
                        let ratio = if travel > 0.0 {
                            thumb_y as f32 / travel
                        } else {
                            0.0
                        };
                        let target = (ratio * max_scroll as f32).round() as usize;
                        if target != browser.scroll_offset {
                            browser.scroll_offset = target.min(max_scroll);
                            needs_redraw = true;
                        }
                    }

                    let over_text_input = |input: &TextInput| {
                        mouse_x >= input.x()
                            && mouse_x < input.x() + input.width() as i32
                            && mouse_y >= input.y()
                            && mouse_y < input.y() + input.height() as i32
                    };
                    let over_input = over_text_input(&search_input)
                        || filename_input.as_ref().is_some_and(over_text_input);
                    let _ = window.set_cursor(if over_input {
                        CursorShape::Text
                    } else {
                        CursorShape::Default
                    });

                    // An open completion popup floats above the list, so it takes
                    // the pointer instead of the row behind it
                    let popup_hover = popup_item_hit(
                        &search_matches,
                        &completion_matches,
                        search_input.has_focus(),
                        mouse_x,
                        mouse_y,
                    );
                    if let Some((idx, is_search)) = popup_hover {
                        let index = if is_search {
                            &mut search_popup_index
                        } else {
                            &mut completion_popup_index
                        };
                        if *index != idx {
                            *index = idx;
                            needs_redraw = true;
                        }
                    }

                    // Update hover states (only when not dragging)
                    if !thumb_drag {
                        let old_sidebar = hovered_sidebar;
                        let old_entry = hovered_entry;
                        let old_toolbar = hovered_toolbar;

                        hovered_sidebar = sidebar_row_at(mouse_x, mouse_y);
                        hovered_entry = None;
                        hovered_toolbar = toolbar_buttons
                            .iter()
                            .find(|b| b.contains(mouse_x, mouse_y))
                            .map(|b| b.action);

                        if popup_hover.is_some() {
                            hovered_sidebar = None;
                            hovered_toolbar = None;
                        }

                        let scrollbar_x = main_x + main_w as i32 - scrollbar_gutter as i32;

                        scrollbar_hovered = mouse_x >= scrollbar_x
                            && mouse_x < main_x + main_w as i32
                            && mouse_y >= list_y
                            && mouse_y < list_y + list_h as i32
                            && !browser.filtered_entries.is_empty();

                        if popup_hover.is_none()
                            && mouse_x >= main_x
                            && mouse_x < scrollbar_x
                            && mouse_y >= list_y
                            && mouse_y < list_y + list_h as i32
                        {
                            let rel_y = (mouse_y - list_y) as usize;
                            let idx = browser.scroll_offset + rel_y / item_height as usize;
                            if idx < browser.filtered_entries.len() {
                                hovered_entry = Some(browser.filtered_entries[idx]);
                            }
                        }

                        if old_sidebar != hovered_sidebar
                            || old_entry != hovered_entry
                            || old_toolbar != hovered_toolbar
                        {
                            needs_redraw = true;
                        }
                    }
                }
                WindowEvent::ButtonPress(MouseButton::Left, _) => {
                    let mut clicking_scrollbar = false;
                    // Dragging the background moves the window. Anything the dialog
                    // can drag itself (text selection, the scrollbar, rows) has to
                    // keep the press, or the compositor grabs the pointer instead.
                    let in_widget = |x: i32, y: i32, w: u32, h: u32| {
                        mouse_x >= x
                            && mouse_x < x + w as i32
                            && mouse_y >= y
                            && mouse_y < y + h as i32
                    };
                    window_dragging = !(in_widget(main_x, main_y, main_w, main_h)
                        || in_widget(sidebar_x, sidebar_y, sidebar_width, sidebar_h)
                        || in_widget(
                            search_input.x(),
                            search_input.y(),
                            search_input.width(),
                            search_input.height(),
                        )
                        || filename_input
                            .as_ref()
                            .is_some_and(|fi| in_widget(fi.x(), fi.y(), fi.width(), fi.height()))
                        || toolbar_buttons.iter().any(|b| b.contains(mouse_x, mouse_y))
                        || in_widget(
                            ok_button.x(),
                            ok_button.y(),
                            ok_button.width(),
                            ok_button.height(),
                        )
                        || in_widget(
                            cancel_button.x(),
                            cancel_button.y(),
                            cancel_button.width(),
                            cancel_button.height(),
                        ));
                    // A click inside an open popup belongs to that popup alone: it must
                    // not select the row behind it, and it must not drop the focus the
                    // popup depends on before its own handler runs below.
                    let clicked_popup = popup_item_hit(
                        &search_matches,
                        &completion_matches,
                        search_input.has_focus(),
                        mouse_x,
                        mouse_y,
                    )
                    .is_some();

                    // The scrollbar gutter swallows clicks so they never select a row
                    if !clicked_popup
                        && !browser.filtered_entries.is_empty()
                        && mouse_x >= main_x + main_w as i32 - scrollbar_gutter as i32
                        && mouse_x < main_x + main_w as i32
                        && mouse_y >= list_y
                        && mouse_y < list_y + list_h as i32
                    {
                        clicking_scrollbar = true;
                        if let Some((_, thumb_y, _, thumb_h)) = scrollbar_thumb(
                            browser.filtered_entries.len(),
                            browser.scroll_offset,
                            scrollbar_hovered,
                        ) && (mouse_y as f32) >= thumb_y
                            && (mouse_y as f32) < thumb_y + thumb_h
                        {
                            thumb_drag = true;
                            thumb_drag_offset = Some(mouse_y - thumb_y as i32);
                        }
                    }

                    // Toolbar buttons
                    if let Some(action) = toolbar_buttons
                        .iter()
                        .find(|b| b.contains(mouse_x, mouse_y))
                        .map(|b| b.action)
                    {
                        match action {
                            ToolbarAction::Back if browser.can_go_back() => {
                                browser.go_back(&search_text);
                                needs_redraw = true;
                            }
                            ToolbarAction::Forward if browser.can_go_forward() => {
                                browser.go_forward(&search_text);
                                needs_redraw = true;
                            }
                            ToolbarAction::Up => {
                                if let Some(parent) = browser.current_dir.parent() {
                                    browser.navigate_to(parent.to_path_buf(), &search_text);
                                    needs_redraw = true;
                                }
                            }
                            ToolbarAction::Home => {
                                if let Some(home) = dirs::home_dir() {
                                    browser.navigate_to(home, &search_text);
                                    needs_redraw = true;
                                }
                            }
                            ToolbarAction::ToggleHidden => {
                                browser.show_hidden = !browser.show_hidden;
                                browser.reload(&search_text);
                                needs_redraw = true;
                            }
                            ToolbarAction::Back | ToolbarAction::Forward => {}
                        }
                    }

                    // Breadcrumb (path bar) click
                    if mouse_y >= main_y
                        && mouse_y < main_y + path_bar_height as i32
                        && mouse_x >= main_x
                        && mouse_x < main_x + main_w as i32
                    {
                        let crumbs = breadcrumb_layout(
                            &browser.current_dir,
                            main_x + (8.0 * scale) as i32,
                            main_w as i32 - (16.0 * scale) as i32,
                            &font,
                        );
                        // The last segment is the current dir; skip it to avoid a no-op reload.
                        for c in crumbs.iter().take(crumbs.len().saturating_sub(1)) {
                            if mouse_x >= c.x && mouse_x < c.x + c.w {
                                browser.navigate_to(c.path.clone(), &search_text);
                                needs_redraw = true;
                                break;
                            }
                        }
                    }

                    // Sidebar click
                    if !clicking_scrollbar && !clicked_popup {
                        let target = match hovered_sidebar {
                            Some(SidebarRow::Place(i)) => Some(quick_access[i].path.clone()),
                            Some(SidebarRow::Drive(i)) => {
                                Some(mounted_drives[i].mount_point.clone())
                            }
                            _ => None,
                        };
                        if let Some(path) = target {
                            browser.navigate_to(path, &search_text);
                            needs_redraw = true;
                        }

                        // File list click
                        if let Some(ei) = hovered_entry {
                            let reclicked = browser.selected_indices.contains(&ei);
                            // Clicking an already selected directory opens it.
                            // Multi-selecting directories is only meaningful in
                            // directory mode, where they stay toggleable and
                            // only `..` still navigates.
                            let open_dir = reclicked
                                && browser.all_entries[ei].is_dir
                                && (!self.multiple
                                    || !self.directory
                                    || browser.all_entries[ei].name == "..");

                            if open_dir {
                                let dest = browser.all_entries[ei].path.clone();
                                browser.navigate_to(dest, &search_text);
                            } else if self.multiple {
                                // Toggle selection in multiple mode
                                if reclicked {
                                    browser.selected_indices.remove(&ei);
                                } else {
                                    browser.selected_indices.insert(ei);
                                }
                            } else if reclicked {
                                let entry = &browser.all_entries[ei];
                                if save_mode {
                                    // In save mode, double-click on file populates filename
                                    if let Some(ref mut fi) = filename_input {
                                        fi.set_text(&entry.name);
                                        completion_matches.clear();
                                        completion_popup_index = 0;
                                    }
                                } else if !self.directory {
                                    return Ok(FileSelectResult::Selected(entry.path.clone()));
                                }
                            } else {
                                browser.selected_indices.clear();
                                browser.selected_indices.insert(ei);
                                // In save mode, single click on file populates filename input
                                if save_mode {
                                    let entry = &browser.all_entries[ei];
                                    if !entry.is_dir
                                        && let Some(ref mut fi) = filename_input
                                    {
                                        fi.set_text(&entry.name);
                                        completion_matches.clear();
                                        completion_popup_index = 0;
                                    }
                                }
                            }
                            needs_redraw = true;
                        }
                    }

                    // Input focus management
                    if !clicked_popup {
                        let search_focused_before = search_input.has_focus();
                        let filename_focused_before =
                            filename_input.as_ref().is_some_and(|f| f.has_focus());
                        let in_search = mouse_x >= search_x
                            && mouse_x < search_x + search_width as i32
                            && mouse_y >= search_y
                            && mouse_y < search_y + (32.0 * scale) as i32;

                        if save_mode {
                            // In save mode, filename input keeps focus unless search is clicked
                            if in_search {
                                search_input.set_focus(true);
                                if let Some(ref mut fi) = filename_input {
                                    fi.set_focus(false);
                                }
                                // Clear filename popup when switching to search
                                completion_matches.clear();
                                completion_popup_index = 0;
                            } else {
                                search_input.set_focus(false);
                                if let Some(ref mut fi) = filename_input {
                                    fi.set_focus(true);
                                }
                                // Clear search popup when switching to filename
                                search_matches.clear();
                                search_popup_index = 0;
                                search_input.set_completion(None);
                            }
                        } else {
                            if !in_search && !search_matches.is_empty() {
                                search_matches.clear();
                                search_popup_index = 0;
                                search_input.set_completion(None);
                            }
                            search_input.set_focus(in_search);
                        }
                        if search_input.has_focus() != search_focused_before
                            || filename_input.as_ref().is_some_and(|f| f.has_focus())
                                != filename_focused_before
                        {
                            needs_redraw = true;
                        }
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
                            if browser.scroll_offset > 0 {
                                browser.scroll_offset = browser.scroll_offset.saturating_sub(3);
                                needs_redraw = true;
                            }
                        }
                        crate::backend::ScrollDirection::Down
                            if browser.scroll_offset + visible_items
                                < browser.filtered_entries.len() =>
                        {
                            browser.scroll_offset = (browser.scroll_offset + 3)
                                .min(browser.filtered_entries.len().saturating_sub(visible_items));
                            needs_redraw = true;
                        }
                        _ => {}
                    }
                }
                WindowEvent::KeyPress(key_event) => {
                    let filename_has_focus =
                        filename_input.as_ref().is_some_and(|fi| fi.has_focus());

                    if key_event.keysym == KEY_ESCAPE {
                        if search_input.has_focus() {
                            if !search_matches.is_empty() {
                                // Close search popup first
                                search_matches.clear();
                                search_popup_index = 0;
                                search_input.set_completion(None);
                            } else {
                                search_input.set_focus(false);
                                // In save mode, return focus to filename input
                                if let Some(ref mut fi) = filename_input {
                                    fi.set_focus(true);
                                }
                            }
                            needs_redraw = true;
                        } else if filename_has_focus {
                            if !completion_matches.is_empty() {
                                // Close popup first
                                completion_matches.clear();
                                completion_popup_index = 0;
                                if let Some(ref mut fi) = filename_input {
                                    fi.set_completion(None);
                                }
                            } else {
                                if let Some(ref mut fi) = filename_input {
                                    fi.set_focus(false);
                                }
                            }
                            needs_redraw = true;
                        } else {
                            return Ok(FileSelectResult::Cancelled);
                        }
                    }
                    if !search_input.has_focus() && !filename_has_focus {
                        match key_event.keysym {
                            KEY_UP => {
                                if !browser.filtered_entries.is_empty() {
                                    let new_index = if let Some(&sel) =
                                        browser.selected_indices.iter().next()
                                    {
                                        if let Some(pos) =
                                            browser.filtered_entries.iter().position(|&e| e == sel)
                                        {
                                            if pos > 0 {
                                                Some(browser.filtered_entries[pos - 1])
                                            } else {
                                                Some(sel)
                                            }
                                        } else {
                                            Some(browser.filtered_entries[0])
                                        }
                                    } else {
                                        Some(browser.filtered_entries[0])
                                    };

                                    if let Some(idx) = new_index {
                                        if self.multiple {
                                            if browser.selected_indices.contains(&idx) {
                                                browser.selected_indices.remove(&idx);
                                            } else {
                                                browser.selected_indices.insert(idx);
                                            }
                                        } else {
                                            browser.selected_indices.clear();
                                            browser.selected_indices.insert(idx);
                                        }

                                        if let Some(pos) =
                                            browser.filtered_entries.iter().position(|&e| e == idx)
                                            && pos < browser.scroll_offset
                                        {
                                            browser.scroll_offset = pos;
                                        }
                                        needs_redraw = true;
                                    }
                                }
                            }
                            KEY_DOWN => {
                                if !browser.filtered_entries.is_empty() {
                                    let new_index = if let Some(&sel) =
                                        browser.selected_indices.iter().next()
                                    {
                                        if let Some(pos) =
                                            browser.filtered_entries.iter().position(|&e| e == sel)
                                        {
                                            if pos + 1 < browser.filtered_entries.len() {
                                                Some(browser.filtered_entries[pos + 1])
                                            } else {
                                                Some(sel)
                                            }
                                        } else {
                                            Some(browser.filtered_entries[0])
                                        }
                                    } else {
                                        Some(browser.filtered_entries[0])
                                    };

                                    if let Some(idx) = new_index {
                                        if self.multiple {
                                            if browser.selected_indices.contains(&idx) {
                                                browser.selected_indices.remove(&idx);
                                            } else {
                                                browser.selected_indices.insert(idx);
                                            }
                                        } else {
                                            browser.selected_indices.clear();
                                            browser.selected_indices.insert(idx);
                                        }

                                        if let Some(pos) =
                                            browser.filtered_entries.iter().position(|&e| e == idx)
                                            && pos >= browser.scroll_offset + visible_items
                                        {
                                            browser.scroll_offset = (pos + 1 - visible_items).min(
                                                browser
                                                    .filtered_entries
                                                    .len()
                                                    .saturating_sub(visible_items),
                                            );
                                        }
                                        needs_redraw = true;
                                    }
                                }
                            }
                            KEY_RETURN => enter_pressed = true,
                            KEY_BACKSPACE => {
                                if let Some(parent) = browser.current_dir.parent() {
                                    browser.navigate_to(parent.to_path_buf(), &search_text);
                                    needs_redraw = true;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }

            // Process search input (with completion popup)
            {
                let mut search_popup_handled = false;

                // Handle search popup keyboard navigation
                if !search_matches.is_empty()
                    && search_input.has_focus()
                    && let WindowEvent::KeyPress(key_event) = &event
                {
                    const POPUP_KEY_UP: u32 = 0xff52;
                    const POPUP_KEY_DOWN: u32 = 0xff54;
                    match key_event.keysym {
                        POPUP_KEY_UP => {
                            if search_popup_index > 0 {
                                search_popup_index -= 1;
                            } else {
                                search_popup_index = search_matches.len() - 1;
                            }
                            let text = search_input.text().to_string();
                            let name = &search_matches[search_popup_index];
                            if name.to_lowercase().starts_with(&text.to_lowercase()) {
                                let pc = text.chars().count();
                                search_input.set_completion(Some(name.chars().skip(pc).collect()));
                            } else {
                                search_input.set_completion(None);
                            }
                            needs_redraw = true;
                            search_popup_handled = true;
                        }
                        POPUP_KEY_DOWN => {
                            search_popup_index = (search_popup_index + 1) % search_matches.len();
                            let text = search_input.text().to_string();
                            let name = &search_matches[search_popup_index];
                            if name.to_lowercase().starts_with(&text.to_lowercase()) {
                                let pc = text.chars().count();
                                search_input.set_completion(Some(name.chars().skip(pc).collect()));
                            } else {
                                search_input.set_completion(None);
                            }
                            needs_redraw = true;
                            search_popup_handled = true;
                        }
                        _ => {}
                    }
                }

                // Handle click on search popup item
                if !search_matches.is_empty()
                    && search_input.has_focus()
                    && let WindowEvent::ButtonPress(MouseButton::Left, _) = &event
                {
                    let rect = search_popup_rect(search_matches.len());
                    if let Some(idx) = popup_item_at(rect, search_matches.len(), mouse_x, mouse_y) {
                        search_input.set_text(&search_matches[idx]);
                        search_matches.clear();
                        search_popup_index = 0;
                        let new_search = search_input.text().to_lowercase();
                        if new_search != search_text {
                            search_text = new_search;
                            browser.refilter(&search_text);
                            browser.selected_indices.clear();
                            browser.scroll_offset = 0;
                        }
                        needs_redraw = true;
                        search_popup_handled = true;
                    }
                }

                if !search_popup_handled {
                    let search_text_before = search_input.text().to_string();
                    if search_input.process_event(&event) {
                        needs_redraw = true;
                    }
                    let new_search = search_input.text().to_lowercase();
                    if new_search != search_text {
                        search_text = new_search;
                        browser.refilter(&search_text);
                        browser.selected_indices.clear();
                        browser.scroll_offset = 0;
                    }
                    // Detect text change → recompute search completions
                    if search_input.text() != search_text_before {
                        search_popup_index = 0;
                        let text = search_input.text().to_string();
                        search_matches = find_all_completions(
                            &browser.all_entries,
                            &text,
                            MAX_POPUP_ITEMS,
                            false,
                            false,
                        );
                        // Only show ghost text if the first match is a prefix match
                        if !search_matches.is_empty()
                            && search_matches[0]
                                .to_lowercase()
                                .starts_with(&text.to_lowercase())
                        {
                            let pc = text.chars().count();
                            search_input
                                .set_completion(Some(search_matches[0].chars().skip(pc).collect()));
                        } else {
                            search_input.set_completion(None);
                        }
                    }
                    // Tab pressed → accept highlighted completion
                    if search_input.was_tab_pressed() {
                        let text = search_input.text().to_string();
                        if !text.is_empty() {
                            search_matches = find_all_completions(
                                &browser.all_entries,
                                &text,
                                MAX_POPUP_ITEMS,
                                false,
                                false,
                            );
                            search_popup_index = 0;
                            if !search_matches.is_empty()
                                && search_matches[0]
                                    .to_lowercase()
                                    .starts_with(&text.to_lowercase())
                            {
                                let pc = text.chars().count();
                                search_input.set_completion(Some(
                                    search_matches[0].chars().skip(pc).collect(),
                                ));
                            } else {
                                search_input.set_completion(None);
                            }
                        }
                        // Re-filter after tab acceptance changed the text
                        let new_search = search_input.text().to_lowercase();
                        if new_search != search_text {
                            search_text = new_search;
                            browser.refilter(&search_text);
                            browser.selected_indices.clear();
                            browser.scroll_offset = 0;
                        }
                        needs_redraw = true;
                    }
                    // Enter with popup open -> accept highlighted item
                    if search_input.was_submitted() && !search_matches.is_empty() {
                        search_input.set_text(&search_matches[search_popup_index]);
                        search_matches.clear();
                        search_popup_index = 0;
                        let new_search = search_input.text().to_lowercase();
                        if new_search != search_text {
                            search_text = new_search;
                            browser.refilter(&search_text);
                            browser.selected_indices.clear();
                            browser.scroll_offset = 0;
                        }
                        needs_redraw = true;
                    }
                }
            }

            // Process filename input (save mode)
            if let Some(ref mut fi) = filename_input {
                let mut popup_handled = false;

                // Handle popup keyboard navigation before passing event to input
                if !completion_matches.is_empty()
                    && let WindowEvent::KeyPress(key_event) = &event
                {
                    const POPUP_KEY_UP: u32 = 0xff52;
                    const POPUP_KEY_DOWN: u32 = 0xff54;
                    match key_event.keysym {
                        POPUP_KEY_UP => {
                            if completion_popup_index > 0 {
                                completion_popup_index -= 1;
                            } else {
                                completion_popup_index = completion_matches.len() - 1;
                            }
                            let prefix = fi.text().to_string();
                            let name = &completion_matches[completion_popup_index];
                            let pc = prefix.chars().count();
                            fi.set_completion(Some(name.chars().skip(pc).collect()));
                            needs_redraw = true;
                            popup_handled = true;
                        }
                        POPUP_KEY_DOWN => {
                            completion_popup_index =
                                (completion_popup_index + 1) % completion_matches.len();
                            let prefix = fi.text().to_string();
                            let name = &completion_matches[completion_popup_index];
                            let pc = prefix.chars().count();
                            fi.set_completion(Some(name.chars().skip(pc).collect()));
                            needs_redraw = true;
                            popup_handled = true;
                        }
                        _ => {}
                    }
                }

                // Handle click on popup item
                if !completion_matches.is_empty()
                    && let WindowEvent::ButtonPress(MouseButton::Left, _) = &event
                {
                    let popup_x = main_x;
                    let popup_w = main_w as i32;
                    let visible = completion_matches.len().min(MAX_POPUP_ITEMS) as i32;
                    let popup_h = visible * POPUP_ITEM_HEIGHT + 2;
                    let popup_y = filename_y + filename_label_h - popup_h;
                    if mouse_x >= popup_x
                        && mouse_x < popup_x + popup_w
                        && mouse_y >= popup_y
                        && mouse_y < popup_y + popup_h
                    {
                        let idx = ((mouse_y - popup_y - 1) / POPUP_ITEM_HEIGHT) as usize;
                        if idx < completion_matches.len().min(MAX_POPUP_ITEMS) {
                            fi.set_text(&completion_matches[idx]);
                            completion_matches.clear();
                            completion_popup_index = 0;
                            needs_redraw = true;
                            popup_handled = true;
                        }
                    }
                }

                if !popup_handled {
                    let text_before = fi.text().to_string();
                    if fi.process_event(&event) {
                        needs_redraw = true;
                    }
                    // Detect text change -> recompute completions
                    if fi.text() != text_before {
                        completion_popup_index = 0;
                        let prefix = fi.text().to_string();
                        completion_matches = find_all_completions(
                            &browser.all_entries,
                            &prefix,
                            MAX_POPUP_ITEMS,
                            true,
                            true,
                        );
                        if !completion_matches.is_empty() {
                            let pc = prefix.chars().count();
                            fi.set_completion(Some(
                                completion_matches[0].chars().skip(pc).collect(),
                            ));
                        } else {
                            fi.set_completion(None);
                        }
                    }
                    // Tab pressed -> accept highlighted completion
                    if fi.was_tab_pressed() {
                        let prefix = fi.text().to_string();
                        if !prefix.is_empty() {
                            // Recompute matches from new text (Tab may have accepted a suffix)
                            completion_matches = find_all_completions(
                                &browser.all_entries,
                                &prefix,
                                MAX_POPUP_ITEMS,
                                true,
                                true,
                            );
                            completion_popup_index = 0;
                            if !completion_matches.is_empty() {
                                let pc = prefix.chars().count();
                                fi.set_completion(Some(
                                    completion_matches[0].chars().skip(pc).collect(),
                                ));
                            } else {
                                fi.set_completion(None);
                            }
                        }
                        needs_redraw = true;
                    }
                    if fi.was_submitted() {
                        // If popup is open, accept the highlighted item instead of submitting
                        if !completion_matches.is_empty() {
                            fi.set_text(&completion_matches[completion_popup_index]);
                            completion_matches.clear();
                            completion_popup_index = 0;
                            needs_redraw = true;
                        } else {
                            let name = fi.text().trim().to_string();
                            if !name.is_empty() {
                                return Ok(FileSelectResult::Selected(
                                    browser.current_dir.join(&name),
                                ));
                            }
                        }
                    }
                }
            }

            // Process buttons
            needs_redraw |= ok_button.process_event(&event);
            needs_redraw |= cancel_button.process_event(&event);

            if ok_button.was_clicked() {
                // In save mode, use filename input text
                if save_mode {
                    if let Some(ref fi) = filename_input {
                        let name = fi.text().trim().to_string();
                        if !name.is_empty() {
                            return Ok(FileSelectResult::Selected(browser.current_dir.join(&name)));
                        }
                    }
                } else {
                    ok_pressed = true;
                }
            }

            // Enter and OK share one activation path. A lone directory is
            // entered rather than returned, except that OK in directory mode
            // returns it; `..` is never a result.
            if enter_pressed || ok_pressed {
                let selected: Vec<&DirEntry> = browser
                    .selected_indices
                    .iter()
                    .map(|&ei| &browser.all_entries[ei])
                    .collect();
                if self.multiple {
                    let paths: Vec<PathBuf> = selected
                        .iter()
                        .filter(|e| e.is_dir == self.directory && e.name != "..")
                        .map(|e| e.path.clone())
                        .collect();
                    if !paths.is_empty() {
                        return Ok(FileSelectResult::SelectedMultiple(paths));
                    }
                }
                let enter_dir = match selected.as_slice() {
                    [entry]
                        if entry.is_dir
                            && (enter_pressed || !self.directory || entry.name == "..") =>
                    {
                        Some(entry.path.clone())
                    }
                    _ => None,
                };
                if let Some(dest) = enter_dir {
                    browser.navigate_to(dest, &search_text);
                    needs_redraw = true;
                } else if !self.multiple
                    && let Some(entry) = selected.first()
                {
                    return Ok(FileSelectResult::Selected(entry.path.clone()));
                } else if ok_pressed && self.directory && selected.is_empty() {
                    return Ok(FileSelectResult::Selected(browser.current_dir.clone()));
                }
            }

            if cancel_button.was_clicked() {
                return Ok(FileSelectResult::Cancelled);
            }

            // Batch pending events
            while let Some(ev) = window.poll_for_event()? {
                match &ev {
                    WindowEvent::CloseRequested => {
                        return Ok(FileSelectResult::Closed);
                    }
                    WindowEvent::CursorEnter(pos) | WindowEvent::CursorMove(pos) => {
                        mouse_x = pos.x as i32;
                        mouse_y = pos.y as i32;
                    }
                    WindowEvent::ButtonPress(button, _modifiers)
                        if *button == MouseButton::Left =>
                    {
                        if mouse_x >= main_x + main_w as i32 - scrollbar_gutter as i32
                            && mouse_x < main_x + main_w as i32
                            && mouse_y >= list_y
                            && mouse_y < list_y + list_h as i32
                            && let Some((_, thumb_y, _, thumb_h)) = scrollbar_thumb(
                                browser.filtered_entries.len(),
                                browser.scroll_offset,
                                scrollbar_hovered,
                            )
                            && (mouse_y as f32) >= thumb_y
                            && (mouse_y as f32) < thumb_y + thumb_h
                        {
                            thumb_drag = true;
                            thumb_drag_offset = Some(mouse_y - thumb_y as i32);
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
                let sig = ChromeSig {
                    dir: browser.current_dir.to_path_buf(),
                    show_hidden: browser.show_hidden,
                    hovered_sidebar,
                    hovered_toolbar,
                    history_index: browser.history_step().0,
                    history_len: browser.history_step().1,
                    search: search_input.text().to_owned(),
                    search_focused: search_input.has_focus(),
                    search_caret: search_input.caret(),
                    filename: filename_input
                        .as_ref()
                        .map(|f| (f.text().to_owned(), f.has_focus())),
                    qa_len: quick_access.len(),
                    drives_len: mounted_drives.len(),
                };
                if chrome_sig.as_ref() != Some(&sig) {
                    draw_chrome(
                        &mut chrome_canvas,
                        colors,
                        &font,
                        &browser.current_dir,
                        &quick_access,
                        &mounted_drives,
                        hovered_sidebar,
                        hovered_toolbar,
                        browser.can_go_back(),
                        browser.can_go_forward(),
                        browser.show_hidden,
                        &search_input,
                        scale,
                    );
                    chrome_sig = Some(sig);
                }
                canvas.blit_region(&chrome_canvas, 0, 0, window_width, window_height, 0, 0);
                draw_dynamic(
                    &mut canvas,
                    colors,
                    &font,
                    &browser.all_entries,
                    &browser.filtered_entries,
                    &browser.selected_indices,
                    browser.scroll_offset,
                    hovered_entry,
                    scale,
                    scrollbar_hovered,
                    hovered_toolbar,
                    browser.show_hidden,
                    &ok_button,
                    &cancel_button,
                    filename_input.as_ref(),
                );
                if save_mode && !completion_matches.is_empty() {
                    let (x, y, _, _) = filename_popup_rect(completion_matches.len());
                    draw_completion_popup(
                        &mut canvas,
                        &font,
                        colors,
                        &completion_matches,
                        completion_popup_index,
                        (x, y),
                        main_w,
                    );
                }
                if !search_matches.is_empty() && search_input.has_focus() {
                    draw_completion_popup(
                        &mut canvas,
                        &font,
                        colors,
                        &search_matches,
                        search_popup_index,
                        {
                            let (x, y, _, _) = search_popup_rect(search_matches.len());
                            (x, y)
                        },
                        search_width,
                    );
                }
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

const POPUP_ITEM_HEIGHT: i32 = 26;
