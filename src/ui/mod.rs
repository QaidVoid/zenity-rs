//! UI components and dialog implementations.

pub(crate) mod message;
pub(crate) mod widgets;

use crate::render::{rgb, Rgba};

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Info,
    Warning,
    Error,
    Question,
}

/// Button presets for message dialogs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonPreset {
    Ok,
    OkCancel,
    YesNo,
    YesNoCancel,
    Close,
}

impl ButtonPreset {
    pub fn labels(self) -> &'static [&'static str] {
        match self {
            ButtonPreset::Ok => &["OK"],
            ButtonPreset::OkCancel => &["OK", "Cancel"],
            ButtonPreset::YesNo => &["Yes", "No"],
            ButtonPreset::YesNoCancel => &["Yes", "No", "Cancel"],
            ButtonPreset::Close => &["Close"],
        }
    }
}

/// Dialog result indicating which button was pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogResult {
    Button(usize),
    Closed,
}

impl DialogResult {
    pub fn exit_code(self) -> i32 {
        match self {
            DialogResult::Button(0) => 0,
            DialogResult::Button(1) => 1,
            DialogResult::Button(2) => 2,
            DialogResult::Button(_) => 3, // Additional buttons
            DialogResult::Closed => 255,
        }
    }
}
