//! UI components and dialog implementations.

pub(crate) mod calendar;
pub(crate) mod entry;
pub(crate) mod file_select;
pub(crate) mod forms;
pub(crate) mod icons;
pub(crate) mod list;
pub(crate) mod message;
pub(crate) mod progress;
pub(crate) mod scale;
pub(crate) mod text_info;
pub(crate) mod widgets;

use crate::{
    backend::{AnyWindow, Window, create_window},
    error::Error,
    render::{Rgba, rgb},
};

/// Create a dialog window sized in logical units, title it, and report the
/// compositor scale factor together with the matching physical size.
///
/// Every dialog derives its physical geometry this way, so the logical to
/// physical conversion lives here rather than being repeated per dialog. Pass
/// an empty `default_title` when the caller has already resolved the title.
pub(crate) fn open_window(
    title: &str,
    default_title: &str,
    logical_width: u32,
    logical_height: u32,
) -> Result<(AnyWindow, f32, u32, u32), Error> {
    let mut window = create_window(logical_width as u16, logical_height as u16)?;
    window.set_title(if title.is_empty() {
        default_title
    } else {
        title
    })?;

    let scale = window.scale_factor();
    let physical_width = (logical_width as f32 * scale) as u32;
    let physical_height = (logical_height as f32 * scale) as u32;

    Ok((window, scale, physical_width, physical_height))
}

// XKB keysym constants shared across dialog implementations
pub(crate) const KEY_BACKSPACE: u32 = 0xff08;
pub(crate) const KEY_TAB: u32 = 0xff09;
pub(crate) const KEY_RETURN: u32 = 0xff0d;
pub(crate) const KEY_ESCAPE: u32 = 0xff1b;
pub(crate) const KEY_HOME: u32 = 0xff50;
pub(crate) const KEY_LEFT: u32 = 0xff51;
pub(crate) const KEY_UP: u32 = 0xff52;
pub(crate) const KEY_RIGHT: u32 = 0xff53;
pub(crate) const KEY_DOWN: u32 = 0xff54;
pub(crate) const KEY_PAGE_UP: u32 = 0xff55;
pub(crate) const KEY_PAGE_DOWN: u32 = 0xff56;
pub(crate) const KEY_END: u32 = 0xff57;
pub(crate) const KEY_KP_ENTER: u32 = 0xff8d;
pub(crate) const KEY_DELETE: u32 = 0xffff;
pub(crate) const KEY_ISO_LEFT_TAB: u32 = 0xfe20;
pub(crate) const KEY_LSHIFT: u32 = 0xffe1;
pub(crate) const KEY_RSHIFT: u32 = 0xffe2;
pub(crate) const KEY_SPACE: u32 = 0x20;

// Shared layout constants (logical, at scale 1.0)
pub(crate) const BASE_CORNER_RADIUS: f32 = 8.0;
pub(crate) const BASE_BUTTON_HEIGHT: u32 = 32;
pub(crate) const BASE_BUTTON_SPACING: u32 = 10;
pub(crate) const BASE_TITLE_FONT_SIZE: f32 = 18.0 * 1.5;

/// Color theme for dialogs.
#[derive(Debug, Clone, Copy)]
pub struct Colors {
    pub window_bg: Rgba,
    pub text: Rgba,
    pub button: Rgba,
    pub button_hover: Rgba,
    pub button_pressed: Rgba,
    pub button_outline: Rgba,
    pub button_text: Rgba,
    pub input_bg: Rgba,
    pub input_bg_focused: Rgba,
    pub input_border: Rgba,
    pub input_border_focused: Rgba,
    pub input_placeholder: Rgba,
    pub progress_bg: Rgba,
    pub progress_fill: Rgba,
    pub progress_border: Rgba,
    pub window_border: Rgba,
    pub window_shadow: Rgba,
    /// Highlight color for primary buttons, focus and selection.
    pub accent: Rgba,
    /// Text drawn on top of a solid `accent` fill.
    pub accent_text: Rgba,
    /// Raised panel behind grouped controls.
    pub surface: Rgba,
    /// Recessed panel behind scrollable content.
    pub surface_alt: Rgba,
    /// Hairline between sections.
    pub separator: Rgba,
    /// Translucent overlay for a hovered row.
    pub row_hover: Rgba,
    /// Translucent accent for a selected row.
    pub row_selected: Rgba,
    /// De-emphasized text such as column headers and metadata.
    pub text_muted: Rgba,
}

/// Light theme colors.
pub static THEME_LIGHT: Colors = Colors {
    window_bg: rgb(250, 250, 250),
    text: rgb(30, 30, 30),
    button: rgb(230, 230, 230),
    button_hover: rgb(220, 220, 220),
    button_pressed: rgb(200, 200, 200),
    button_outline: rgb(180, 180, 180),
    button_text: rgb(30, 30, 30),
    input_bg: rgb(255, 255, 255),
    input_bg_focused: rgb(255, 255, 255),
    input_border: rgb(200, 200, 200),
    input_border_focused: rgb(100, 150, 200),
    input_placeholder: rgb(150, 150, 150),
    progress_bg: rgb(230, 230, 230),
    progress_fill: rgb(70, 140, 220),
    progress_border: rgb(200, 200, 200),
    window_border: rgb(180, 180, 180),
    window_shadow: Rgba::new(0, 0, 0, 50),
    accent: rgb(53, 132, 228),
    accent_text: rgb(255, 255, 255),
    surface: rgb(240, 240, 240),
    surface_alt: rgb(255, 255, 255),
    separator: rgb(220, 220, 220),
    row_hover: Rgba::new(0, 0, 0, 15),
    row_selected: Rgba::new(53, 132, 228, 45),
    text_muted: rgb(110, 110, 110),
};

/// Dark theme colors.
pub static THEME_DARK: Colors = Colors {
    window_bg: rgb(45, 45, 45),
    text: rgb(230, 230, 230),
    button: rgb(70, 70, 70),
    button_hover: rgb(80, 80, 80),
    button_pressed: rgb(60, 60, 60),
    button_outline: rgb(100, 100, 100),
    button_text: rgb(230, 230, 230),
    input_bg: rgb(60, 60, 60),
    input_bg_focused: rgb(65, 65, 65),
    input_border: rgb(90, 90, 90),
    input_border_focused: rgb(100, 150, 200),
    input_placeholder: rgb(120, 120, 120),
    progress_bg: rgb(60, 60, 60),
    progress_fill: rgb(70, 140, 220),
    progress_border: rgb(90, 90, 90),
    window_border: rgb(70, 70, 70),
    window_shadow: Rgba::new(0, 0, 0, 80),
    accent: rgb(84, 148, 228),
    accent_text: rgb(255, 255, 255),
    surface: rgb(54, 54, 54),
    surface_alt: rgb(38, 38, 38),
    separator: rgb(66, 66, 66),
    row_hover: Rgba::new(255, 255, 255, 18),
    row_selected: Rgba::new(84, 148, 228, 64),
    text_muted: rgb(150, 150, 150),
};

/// Detect the current system theme.
/// Returns dark theme if detection fails.
pub fn detect_theme() -> &'static Colors {
    // Try to detect theme from environment
    if let Ok(theme) = std::env::var("GTK_THEME") {
        if theme.to_lowercase().contains("dark") {
            return &THEME_DARK;
        }
        return &THEME_LIGHT;
    }

    // Try gsettings
    if let Ok(output) = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "color-scheme"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("dark") {
            return &THEME_DARK;
        }
        if stdout.contains("light") || stdout.contains("default") {
            return &THEME_LIGHT;
        }
    }

    // Default to dark
    &THEME_DARK
}

/// Icon types for message dialogs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Icon {
    Info,
    Warning,
    Error,
    Question,
    Custom(String),
}

impl Icon {
    /// Map zenity icon names to Icon variants
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "dialog-information" | "info" => Some(Icon::Info),
            "dialog-warning" | "warning" => Some(Icon::Warning),
            "dialog-error" | "error" => Some(Icon::Error),
            "dialog-question" | "question" => Some(Icon::Question),
            other => Some(Icon::Custom(other.to_string())),
        }
    }
}

/// Button presets for message dialogs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ButtonPreset {
    Ok,
    OkCancel,
    YesNo,
    YesNoCancel,
    Close,
    Empty,
    Custom(Vec<String>),
}

impl ButtonPreset {
    pub fn labels(&self) -> Vec<String> {
        match self {
            ButtonPreset::Ok => vec!["OK".to_string()],
            ButtonPreset::OkCancel => vec!["OK".to_string(), "Cancel".to_string()],
            ButtonPreset::YesNo => vec!["Yes".to_string(), "No".to_string()],
            ButtonPreset::YesNoCancel => {
                vec!["Yes".to_string(), "No".to_string(), "Cancel".to_string()]
            }
            ButtonPreset::Close => vec!["Close".to_string()],
            ButtonPreset::Empty => vec![],
            ButtonPreset::Custom(labels) => labels.clone(),
        }
    }

    /// Number of buttons this preset renders.
    pub fn len(&self) -> usize {
        match self {
            ButtonPreset::Ok | ButtonPreset::Close => 1,
            ButtonPreset::OkCancel | ButtonPreset::YesNo => 2,
            ButtonPreset::YesNoCancel => 3,
            ButtonPreset::Empty => 0,
            ButtonPreset::Custom(labels) => labels.len(),
        }
    }

    /// Whether this preset renders no buttons at all.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Dialog result indicating which button was pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogResult {
    Button(usize),
    Closed,
    Timeout,
}

impl DialogResult {
    pub fn exit_code(self) -> i32 {
        match self {
            DialogResult::Button(0) => 0,
            DialogResult::Button(1) => 1,
            DialogResult::Button(2) => 2,
            DialogResult::Button(_) => 3, // Additional buttons
            DialogResult::Timeout => 5,
            DialogResult::Closed => 1,
        }
    }
}
