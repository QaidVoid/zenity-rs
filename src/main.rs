//! rask - Display simple GUI dialogs from the command line.

use std::process::ExitCode;

use lexopt::prelude::*;

use rask::{ButtonPreset, DialogResult, Icon, message};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    match run() {
        Ok(result) => ExitCode::from(result.exit_code() as u8),
        Err(e) => {
            eprintln!("rask: {e}");
            ExitCode::from(100)
        }
    }
}

fn run() -> Result<DialogResult, Box<dyn std::error::Error>> {
    let mut parser = lexopt::Parser::from_env();

    // Global options
    let mut title = String::new();
    let mut text = String::new();

    // Dialog type
    let mut dialog_type: Option<DialogType> = None;

    while let Some(arg) = parser.next()? {
        match arg {
            Long("help") | Short('h') => {
                print_help();
                return Ok(DialogResult::Button(0));
            }
            Long("version") => {
                println!("rask {VERSION}");
                return Ok(DialogResult::Button(0));
            }

            // Dialog types
            Long("info") => dialog_type = Some(DialogType::Info),
            Long("warning") => dialog_type = Some(DialogType::Warning),
            Long("error") => dialog_type = Some(DialogType::Error),
            Long("question") => dialog_type = Some(DialogType::Question),

            // Common options
            Long("title") => title = parser.value()?.string()?,
            Long("text") => text = parser.value()?.string()?,

            // TODO: Add more dialog types
            Long("entry") | Long("password") | Long("progress") | Long("file-selection")
            | Long("list") | Long("calendar") => {
                return Err(format!("dialog type not yet implemented: {:?}", arg).into());
            }

            Value(val) => {
                // Positional argument - treat as text if text is empty
                if text.is_empty() {
                    text = val.string()?;
                }
            }

            _ => return Err(arg.unexpected().into()),
        }
    }

    // Default to info if no dialog type specified
    let dialog_type = dialog_type.unwrap_or(DialogType::Info);

    // Build and show the dialog
    let result = match dialog_type {
        DialogType::Info => {
            message()
                .title(if title.is_empty() { "Information" } else { &title })
                .text(&text)
                .icon(Icon::Info)
                .buttons(ButtonPreset::Ok)
                .show()?
        }
        DialogType::Warning => {
            message()
                .title(if title.is_empty() { "Warning" } else { &title })
                .text(&text)
                .icon(Icon::Warning)
                .buttons(ButtonPreset::Ok)
                .show()?
        }
        DialogType::Error => {
            message()
                .title(if title.is_empty() { "Error" } else { &title })
                .text(&text)
                .icon(Icon::Error)
                .buttons(ButtonPreset::Ok)
                .show()?
        }
        DialogType::Question => {
            message()
                .title(if title.is_empty() { "Question" } else { &title })
                .text(&text)
                .icon(Icon::Question)
                .buttons(ButtonPreset::YesNo)
                .show()?
        }
    };

    Ok(result)
}

#[derive(Debug, Clone, Copy)]
enum DialogType {
    Info,
    Warning,
    Error,
    Question,
}

fn print_help() {
    println!(
        r#"rask {VERSION} - Display simple GUI dialogs from the command line

USAGE:
    rask [OPTIONS] --<dialog-type> [TEXT]

DIALOG TYPES:
    --info              Display an information dialog
    --warning           Display a warning dialog
    --error             Display an error dialog
    --question          Display a question dialog (Yes/No)
    --entry             Display a text entry dialog (not yet implemented)
    --password          Display a password dialog (not yet implemented)
    --progress          Display a progress dialog (not yet implemented)
    --file-selection    Display a file selection dialog (not yet implemented)
    --list              Display a list dialog (not yet implemented)
    --calendar          Display a calendar dialog (not yet implemented)

OPTIONS:
    --title=TEXT        Set the dialog title
    --text=TEXT         Set the dialog text
    -h, --help          Print this help message
    --version           Print version information

EXAMPLES:
    rask --info --text="Operation completed"
    rask --question --text="Do you want to continue?"
    rask --error --title="Error" --text="Something went wrong"

EXIT CODES:
    0   OK/Yes button clicked
    1   Cancel/No button clicked
    255 Dialog was closed
    100 Error occurred
"#
    );
}
