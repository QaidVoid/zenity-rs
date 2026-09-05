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
    render::{Font, Rgba, rgb},
    ui::widgets::{Widget, button::Button},
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
/// Shortest a scrollbar thumb may shrink to, however long the content is.
pub(crate) const BASE_MIN_THUMB: f32 = 20.0;

/// Y coordinate of the button row along the bottom edge of a dialog.
pub(crate) fn button_row_y(physical_height: u32, padding: u32, scale: f32) -> i32 {
    let button_height = (BASE_BUTTON_HEIGHT as f32 * scale) as u32;
    physical_height.saturating_sub(padding + button_height) as i32
}

/// Right-align OK and Cancel on the button row, Cancel outermost.
pub(crate) fn place_ok_cancel(
    ok: &mut Button,
    cancel: &mut Button,
    physical_width: u32,
    padding: u32,
    y: i32,
    scale: f32,
) {
    let spacing = (BASE_BUTTON_SPACING as f32 * scale) as i32;
    let mut x = physical_width as i32 - padding as i32;
    x -= cancel.width() as i32;
    cancel.set_position(x, y);
    x -= spacing + ok.width() as i32;
    ok.set_position(x, y);
}

/// Width an OK and Cancel pair needs in logical units, gap included.
///
/// Dialogs size their window from logical measurements taken at scale 1.0
/// before the real font exists, so `font` must be loaded at that scale.
pub(crate) fn ok_cancel_logical_width(font: &Font) -> u32 {
    Button::new("OK", font, 1.0).width()
        + Button::new("Cancel", font, 1.0).width()
        + BASE_BUTTON_SPACING
}

/// Scale each color channel toward black by `amount`, leaving alpha untouched.
pub(crate) fn darken(color: Rgba, amount: f32) -> Rgba {
    Rgba {
        r: (color.r as f32 * (1.0 - amount)) as u8,
        g: (color.g as f32 * (1.0 - amount)) as u8,
        b: (color.b as f32 * (1.0 - amount)) as u8,
        a: color.a,
    }
}

/// Position and length of a scrollbar thumb along its track.
///
/// Both are measured along the scroll axis, so the same type serves vertical
/// and horizontal bars, and `visible`/`total` may be counted in either items or
/// pixels as long as they share a unit.
pub(crate) struct Thumb {
    /// Distance from the start of the track to the start of the thumb.
    pub(crate) offset: f32,
    /// Length of the thumb along the track.
    pub(crate) len: f32,
}

impl Thumb {
    /// Size a thumb showing `visible` of `total` units, scrolled to `scroll`.
    ///
    /// Returns `None` when the content already fits and no bar should be drawn.
    /// Call this from drawing, hit-testing and dragging alike so the three can
    /// never disagree about where the thumb is.
    pub(crate) fn new(
        track: f32,
        visible: f32,
        total: f32,
        scroll: f32,
        min_len: f32,
    ) -> Option<Self> {
        if track <= 0.0 || visible <= 0.0 || total <= visible {
            return None;
        }

        let len = (visible / total * track).clamp(min_len.min(track), track);
        let max_scroll = total - visible;
        let offset = (scroll / max_scroll).clamp(0.0, 1.0) * (track - len);

        Some(Self {
            offset,
            len,
        })
    }

    /// Whether `pos`, measured from the start of the track, lands on the thumb.
    pub(crate) fn contains(&self, pos: f32) -> bool {
        pos >= self.offset && pos < self.offset + self.len
    }

    /// Scroll position matching a thumb dragged to `offset` along the track.
    pub(crate) fn scroll_at(&self, track: f32, offset: f32, max_scroll: f32) -> f32 {
        let travel = track - self.len;
        if travel <= 0.0 {
            return 0.0;
        }
        (offset.clamp(0.0, travel) / travel) * max_scroll
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_thumb_when_content_fits() {
        assert!(Thumb::new(100.0, 10.0, 10.0, 0.0, 20.0).is_none());
        assert!(Thumb::new(100.0, 10.0, 5.0, 0.0, 20.0).is_none());
        assert!(Thumb::new(0.0, 1.0, 10.0, 0.0, 20.0).is_none());
    }

    #[test]
    fn thumb_spans_the_visible_fraction() {
        let thumb = Thumb::new(100.0, 25.0, 100.0, 0.0, 0.0).unwrap();
        assert_eq!(thumb.len, 25.0);
        assert_eq!(thumb.offset, 0.0);
    }

    #[test]
    fn thumb_never_shrinks_below_the_minimum_or_grows_past_the_track() {
        assert_eq!(
            Thumb::new(100.0, 1.0, 1_000.0, 0.0, 20.0).unwrap().len,
            20.0
        );
        assert_eq!(Thumb::new(10.0, 1.0, 1_000.0, 0.0, 20.0).unwrap().len, 10.0);
    }

    #[test]
    fn thumb_reaches_the_end_of_the_track_at_max_scroll() {
        let thumb = Thumb::new(100.0, 25.0, 100.0, 75.0, 0.0).unwrap();
        assert_eq!(thumb.offset + thumb.len, 100.0);
    }

    #[test]
    fn thumb_offset_is_clamped_to_the_track() {
        let thumb = Thumb::new(100.0, 25.0, 100.0, 9_999.0, 0.0).unwrap();
        assert_eq!(thumb.offset + thumb.len, 100.0);
    }

    #[test]
    fn contains_covers_the_thumb_only() {
        let thumb = Thumb::new(100.0, 25.0, 100.0, 25.0, 0.0).unwrap();
        assert_eq!(thumb.offset, 25.0);
        assert!(thumb.contains(25.0));
        assert!(thumb.contains(49.9));
        assert!(!thumb.contains(24.9));
        assert!(!thumb.contains(50.0));
    }

    #[test]
    fn scroll_at_round_trips_the_thumb_offset() {
        let thumb = Thumb::new(100.0, 25.0, 100.0, 30.0, 0.0).unwrap();
        assert_eq!(thumb.scroll_at(100.0, thumb.offset, 75.0), 30.0);
        assert_eq!(thumb.scroll_at(100.0, -10.0, 75.0), 0.0);
        assert_eq!(thumb.scroll_at(100.0, 9_999.0, 75.0), 75.0);
    }

    #[test]
    fn darken_keeps_alpha() {
        let out = darken(Rgba::new(200, 100, 50, 128), 0.5);
        assert_eq!((out.r, out.g, out.b, out.a), (100, 50, 25, 128));
    }
}
