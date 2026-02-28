//! UI components and dialog implementations.

pub(crate) mod calendar;
pub(crate) mod entry;
pub(crate) mod file_select;
pub(crate) mod forms;
pub(crate) mod list;
pub(crate) mod message;
pub(crate) mod progress;
pub(crate) mod scale;
pub(crate) mod text_info;
pub(crate) mod widgets;

use crate::render::{Rgba, rgb};

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
