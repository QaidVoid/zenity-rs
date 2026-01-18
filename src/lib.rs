//! rask - Display simple GUI dialogs from the command line.
//!
//! This library provides simple GUI dialogs for shell scripts and command-line tools.

pub mod error;
pub(crate) mod backend;
pub(crate) mod render;
pub mod ui;

pub use error::Error;
pub use ui::{ButtonPreset, Colors, DialogResult, Icon, THEME_DARK, THEME_LIGHT};
pub use ui::message::MessageBuilder;

/// Creates a new message dialog builder.
///
/// # Example
///
/// ```no_run
/// use rask::{message, Icon, ButtonPreset};
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
