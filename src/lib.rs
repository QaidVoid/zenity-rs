//! zenity-rs - Display simple GUI dialogs from the command line.
//!
//! This library provides simple GUI dialogs for shell scripts and command-line tools.

pub(crate) mod backend;
pub mod error;
pub(crate) mod render;
pub(crate) mod timing;
pub mod ui;

pub use error::Error;
pub use ui::{
    ButtonPreset, Colors, DialogResult, Icon, THEME_DARK, THEME_LIGHT,
    calendar::{CalendarBuilder, CalendarResult},
    entry::{EntryBuilder, EntryResult},
    file_select::{FileFilter, FileSelectBuilder, FileSelectResult},
    forms::{FormsBuilder, FormsResult},
    list::{ListBuilder, ListMode, ListResult},
    message::MessageBuilder,
    progress::{ProgressBuilder, ProgressResult},
    scale::{ScaleBuilder, ScaleResult},
    text_info::{TextInfoBuilder, TextInfoResult},
};

/// Creates a new message dialog builder.
///
/// # Example
///
/// ```no_run
/// use zenity_rs::{message, Icon, ButtonPreset};
///
/// let result = message()
///     .title("Information")
///     .text("Operation completed successfully!")
///     .icon(Icon::Info)
///     .buttons(ButtonPreset::Ok)
///     .show()
///     .unwrap();
/// ```
pub fn message() -> MessageBuilder {
    MessageBuilder::new()
}

/// Creates an info dialog (shortcut for message with info icon).
pub fn info(text: &str) -> MessageBuilder {
    MessageBuilder::new()
        .text(text)
        .icon(Icon::Info)
        .buttons(ButtonPreset::Ok)
}

/// Creates a warning dialog (shortcut for message with warning icon).
pub fn warning(text: &str) -> MessageBuilder {
    MessageBuilder::new()
        .text(text)
        .icon(Icon::Warning)
        .buttons(ButtonPreset::Ok)
}

/// Creates an error dialog (shortcut for message with error icon).
pub fn error(text: &str) -> MessageBuilder {
    MessageBuilder::new()
        .text(text)
        .icon(Icon::Error)
        .buttons(ButtonPreset::Ok)
}

/// Creates a question dialog (shortcut for message with question icon and Yes/No buttons).
pub fn question(text: &str) -> MessageBuilder {
    MessageBuilder::new()
        .text(text)
        .icon(Icon::Question)
        .buttons(ButtonPreset::YesNo)
}

/// Creates a new entry dialog builder.
pub fn entry() -> EntryBuilder {
    EntryBuilder::new()
}

/// Creates a password entry dialog (entry with hidden text).
pub fn password() -> EntryBuilder {
    EntryBuilder::new().hide_text(true)
}

/// Creates a new progress dialog builder.
pub fn progress() -> ProgressBuilder {
    ProgressBuilder::new()
}

/// Creates a new file selection dialog builder.
pub fn file_select() -> FileSelectBuilder {
    FileSelectBuilder::new()
}

/// Creates a new list selection dialog builder.
pub fn list() -> ListBuilder {
    ListBuilder::new()
}

/// Creates a new calendar date picker dialog builder.
pub fn calendar() -> CalendarBuilder {
    CalendarBuilder::new()
}

/// Creates a new text info dialog builder.
pub fn text_info() -> TextInfoBuilder {
    TextInfoBuilder::new()
}

/// Creates a new scale dialog builder.
pub fn scale() -> ScaleBuilder {
    ScaleBuilder::new()
}

/// Creates a new forms dialog builder.
pub fn forms() -> FormsBuilder {
    FormsBuilder::new()
}
