//! File selection dialog implementation with enhanced UI.

mod breadcrumbs;
mod draw;
mod fs;
mod layout;
mod render;

use std::path::{Path, PathBuf};

use breadcrumbs::breadcrumb_layout;
use draw::{MAX_POPUP_ITEMS, SidebarGlyph, draw_completion_popup};
use fs::{
    Browser, DirEntry, MountPoint, QuickAccess, build_quick_access, find_all_completions,
    get_mounted_drives,
};
use layout::Layout;
use render::{draw_chrome, draw_dynamic};

use crate::{
    backend::{CursorShape, MouseButton, Window, WindowEvent},
    error::Error,
    render::{Canvas, Font},
    ui::{
        Colors, KEY_BACKSPACE, KEY_DOWN, KEY_ESCAPE, KEY_RETURN, KEY_UP, button_row_y, open_window,
        place_ok_cancel,
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

        let layout = Layout {
            scale,
            window_width,
            padding,
            sidebar_x,
            sidebar_width,
            main_x,
            main_y,
            main_w,
            main_h,
            toolbar_height,
            path_bar_height,
            section_header_height,
            content_gap,
            header_offset,
            item_height,
            visible_items,
            list_y,
            list_h,
            size_col_x,
            size_col_width,
            date_col_x,
            search_x,
            search_y,
            button_y,
            filename_y,
            scrollbar_gutter,
            toolbar_buttons,
            sidebar_rows,
        };

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

        // Chrome layer: everything that only changes on navigation or hover.

        // Dynamic layer: the scrollable file list + scrollbar + inputs + buttons.
        // Redrawn every frame on top of the cached chrome.

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
                &layout,
                &browser,
                &quick_access,
                &mounted_drives,
                hovered_sidebar,
                hovered_toolbar,
                &search_input,
            );
            chrome_sig = Some(sig);
        }
        canvas.blit_region(&chrome_canvas, 0, 0, window_width, window_height, 0, 0);
        draw_dynamic(
            &mut canvas,
            colors,
            &font,
            &layout,
            &browser,
            title,
            hovered_entry,
            scrollbar_hovered,
            hovered_toolbar,
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
                        && let Some((_, _, _, thumb_h)) = layout.scrollbar_thumb(
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

                        hovered_sidebar = layout.sidebar_row_at(mouse_x, mouse_y);
                        hovered_entry = None;
                        hovered_toolbar = layout
                            .toolbar_buttons
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
                        || layout
                            .toolbar_buttons
                            .iter()
                            .any(|b| b.contains(mouse_x, mouse_y))
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
                        if let Some((_, thumb_y, _, thumb_h)) = layout.scrollbar_thumb(
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
                    if let Some(action) = layout
                        .toolbar_buttons
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
                            && let Some((_, thumb_y, _, thumb_h)) = layout.scrollbar_thumb(
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
                        &layout,
                        &browser,
                        &quick_access,
                        &mounted_drives,
                        hovered_sidebar,
                        hovered_toolbar,
                        &search_input,
                    );
                    chrome_sig = Some(sig);
                }
                canvas.blit_region(&chrome_canvas, 0, 0, window_width, window_height, 0, 0);
                draw_dynamic(
                    &mut canvas,
                    colors,
                    &font,
                    &layout,
                    &browser,
                    title,
                    hovered_entry,
                    scrollbar_hovered,
                    hovered_toolbar,
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
