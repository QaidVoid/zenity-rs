//! zenity-rs - Display simple GUI dialogs from the command line.

use std::process::ExitCode;

use lexopt::prelude::*;

use zenity_rs::{ButtonPreset, CalendarResult, EntryResult, FileSelectResult, Icon, ListResult, ProgressResult, calendar, entry, file_select, list, message, password, progress};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code as u8),
        Err(e) => {
            eprintln!("zenity-rs: {e}");
            ExitCode::from(100)
        }
    }
}

fn run() -> Result<i32, Box<dyn std::error::Error>> {
    let mut parser = lexopt::Parser::from_env();

    // Global options
    let mut title = String::new();
    let mut text = String::new();
    let mut entry_text = String::new();
    let mut timeout: Option<u32> = None;
    let mut width: Option<u32> = None;
    let mut height: Option<u32> = None;

    // Progress options
    let mut percentage: u32 = 0;
    let mut pulsate = false;
    let mut auto_close = false;

    // File selection options
    let mut directory_mode = false;
    let mut save_mode = false;
    let mut filename = String::new();

    // List options
    let mut columns: Vec<String> = Vec::new();
    let mut list_values: Vec<String> = Vec::new();
    let mut checklist = false;
    let mut radiolist = false;

    // Calendar options
    let mut cal_year: Option<u32> = None;
    let mut cal_month: Option<u32> = None;
    let mut cal_day: Option<u32> = None;

    // Dialog type
    let mut dialog_type: Option<DialogType> = None;

    while let Some(arg) = parser.next()? {
        match arg {
            Long("help") | Short('h') => {
                print_help();
                return Ok(0);
            }
            Long("version") => {
                println!("zenity-rs {VERSION}");
                return Ok(0);
            }

            // Dialog types
            Long("info") => dialog_type = Some(DialogType::Info),
            Long("warning") => dialog_type = Some(DialogType::Warning),
            Long("error") => dialog_type = Some(DialogType::Error),
            Long("question") => dialog_type = Some(DialogType::Question),
            Long("entry") => dialog_type = Some(DialogType::Entry),
            Long("password") => dialog_type = Some(DialogType::Password),
            Long("progress") => dialog_type = Some(DialogType::Progress),
            Long("file-selection") => dialog_type = Some(DialogType::FileSelection),
            Long("list") => dialog_type = Some(DialogType::List),
            Long("calendar") => dialog_type = Some(DialogType::Calendar),

            // Common options
            Long("title") => title = parser.value()?.string()?,
            Long("text") => text = parser.value()?.string()?,
            Long("entry-text") => entry_text = parser.value()?.string()?,
            Long("hide-text") => {
                // If --hide-text is specified with --entry, treat as password mode
                if dialog_type == Some(DialogType::Entry) {
                    dialog_type = Some(DialogType::Password);
                }
            }
            Long("timeout") => timeout = Some(parser.value()?.string()?.parse()?),
            Long("width") => width = Some(parser.value()?.string()?.parse()?),
            Long("height") => height = Some(parser.value()?.string()?.parse()?),

            // Progress options
            Long("percentage") => percentage = parser.value()?.string()?.parse()?,
            Long("pulsate") => pulsate = true,
            Long("auto-close") => auto_close = true,

            // File selection options
            Long("directory") => directory_mode = true,
            Long("save") => save_mode = true,
            Long("filename") => filename = parser.value()?.string()?,

            // List options
            Long("column") => columns.push(parser.value()?.string()?),
            Long("checklist") => checklist = true,
            Long("radiolist") => radiolist = true,

            // Calendar options
            Long("year") => cal_year = Some(parser.value()?.string()?.parse()?),
            Long("month") => cal_month = Some(parser.value()?.string()?.parse()?),
            Long("day") => cal_day = Some(parser.value()?.string()?.parse()?),

            Value(val) => {
                // Positional arguments - for list dialog these are row values
                if dialog_type == Some(DialogType::List) {
                    list_values.push(val.string()?);
                } else if text.is_empty() {
                    text = val.string()?;
                }
            }

            _ => return Err(arg.unexpected().into()),
        }
    }

    // Show help if no dialog type specified
    let dialog_type = match dialog_type {
        Some(dt) => dt,
        None => {
            print_help();
            return Ok(0);
        }
    };

    // Build and show the dialog
    match dialog_type {
        DialogType::Info => {
            let mut builder = message()
                .title(if title.is_empty() { "Information" } else { &title })
                .text(&text)
                .icon(Icon::Info)
                .buttons(ButtonPreset::Ok);
            if let Some(t) = timeout {
                builder = builder.timeout(t);
            }
            if let Some(w) = width {
                builder = builder.width(w);
            }
            if let Some(h) = height {
                builder = builder.height(h);
            }
            let result = builder.show()?;
            Ok(result.exit_code())
        }
        DialogType::Warning => {
            let mut builder = message()
                .title(if title.is_empty() { "Warning" } else { &title })
                .text(&text)
                .icon(Icon::Warning)
                .buttons(ButtonPreset::Ok);
            if let Some(t) = timeout {
                builder = builder.timeout(t);
            }
            if let Some(w) = width {
                builder = builder.width(w);
            }
            if let Some(h) = height {
                builder = builder.height(h);
            }
            let result = builder.show()?;
            Ok(result.exit_code())
        }
        DialogType::Error => {
            let mut builder = message()
                .title(if title.is_empty() { "Error" } else { &title })
                .text(&text)
                .icon(Icon::Error)
                .buttons(ButtonPreset::Ok);
            if let Some(t) = timeout {
                builder = builder.timeout(t);
            }
            if let Some(w) = width {
                builder = builder.width(w);
            }
            if let Some(h) = height {
                builder = builder.height(h);
            }
            let result = builder.show()?;
            Ok(result.exit_code())
        }
        DialogType::Question => {
            let mut builder = message()
                .title(if title.is_empty() { "Question" } else { &title })
                .text(&text)
                .icon(Icon::Question)
                .buttons(ButtonPreset::YesNo);
            if let Some(t) = timeout {
                builder = builder.timeout(t);
            }
            if let Some(w) = width {
                builder = builder.width(w);
            }
            if let Some(h) = height {
                builder = builder.height(h);
            }
            let result = builder.show()?;
            Ok(result.exit_code())
        }
        DialogType::Entry => {
            let mut builder = entry()
                .title(if title.is_empty() { "Entry" } else { &title })
                .text(&text)
                .entry_text(&entry_text);
            if let Some(w) = width {
                builder = builder.width(w);
            }
            if let Some(h) = height {
                builder = builder.height(h);
            }
            let result = builder.show()?;
            handle_entry_result(result)
        }
        DialogType::Password => {
            let mut builder = password()
                .title(if title.is_empty() { "Password" } else { &title })
                .text(&text);
            if let Some(w) = width {
                builder = builder.width(w);
            }
            if let Some(h) = height {
                builder = builder.height(h);
            }
            let result = builder.show()?;
            handle_entry_result(result)
        }
        DialogType::Progress => {
            let mut builder = progress()
                .title(if title.is_empty() { "Progress" } else { &title })
                .text(&text)
                .percentage(percentage)
                .pulsate(pulsate)
                .auto_close(auto_close);
            if let Some(w) = width {
                builder = builder.width(w);
            }
            if let Some(h) = height {
                builder = builder.height(h);
            }
            let result = builder.show()?;
            handle_progress_result(result)
        }
        DialogType::FileSelection => {
            let mut builder = file_select();
            if !title.is_empty() {
                builder = builder.title(&title);
            }
            builder = builder.directory(directory_mode).save(save_mode);
            if !filename.is_empty() {
                builder = builder.filename(&filename);
            }
            if let Some(w) = width {
                builder = builder.width(w);
            }
            if let Some(h) = height {
                builder = builder.height(h);
            }
            let result = builder.show()?;
            handle_file_select_result(result)
        }
        DialogType::List => {
            let mut builder = list();
            if !title.is_empty() {
                builder = builder.title(&title);
            }
            if !text.is_empty() {
                builder = builder.text(&text);
            }
            for col in &columns {
                builder = builder.column(col);
            }
            if checklist {
                builder = builder.checklist();
            } else if radiolist {
                builder = builder.radiolist();
            }

            // Build rows from list_values based on column count
            let cols = columns.len().max(1);
            for chunk in list_values.chunks(cols) {
                builder = builder.row(chunk.iter().cloned().collect());
            }

            if let Some(w) = width {
                builder = builder.width(w);
            }
            if let Some(h) = height {
                builder = builder.height(h);
            }
            let result = builder.show()?;
            handle_list_result(result)
        }
        DialogType::Calendar => {
            let mut builder = calendar();
            if !title.is_empty() {
                builder = builder.title(&title);
            }
            if !text.is_empty() {
                builder = builder.text(&text);
            }
            if let Some(y) = cal_year {
                builder = builder.year(y);
            }
            if let Some(m) = cal_month {
                builder = builder.month(m);
            }
            if let Some(d) = cal_day {
                builder = builder.day(d);
            }
            if let Some(w) = width {
                builder = builder.width(w);
            }
            if let Some(h) = height {
                builder = builder.height(h);
            }
            let result = builder.show()?;
            handle_calendar_result(result)
        }
    }
}

fn handle_list_result(result: ListResult) -> Result<i32, Box<dyn std::error::Error>> {
    match result {
        ListResult::Selected(items) => {
            for item in items {
                println!("{}", item);
            }
            Ok(0)
        }
        ListResult::Cancelled => Ok(1),
        ListResult::Closed => Ok(255),
    }
}

fn handle_calendar_result(result: CalendarResult) -> Result<i32, Box<dyn std::error::Error>> {
    match result {
        CalendarResult::Selected { year, month, day } => {
            println!("{:04}-{:02}-{:02}", year, month, day);
            Ok(0)
        }
        CalendarResult::Cancelled => Ok(1),
        CalendarResult::Closed => Ok(255),
    }
}

fn handle_file_select_result(result: FileSelectResult) -> Result<i32, Box<dyn std::error::Error>> {
    match result {
        FileSelectResult::Selected(path) => {
            println!("{}", path.display());
            Ok(0)
        }
        FileSelectResult::Cancelled => Ok(1),
        FileSelectResult::Closed => Ok(255),
    }
}

fn handle_progress_result(result: ProgressResult) -> Result<i32, Box<dyn std::error::Error>> {
    Ok(result.exit_code())
}

fn handle_entry_result(result: EntryResult) -> Result<i32, Box<dyn std::error::Error>> {
    match result {
        EntryResult::Text(text) => {
            println!("{text}");
            Ok(0)
        }
        EntryResult::Cancelled => Ok(1),
        EntryResult::Closed => Ok(255),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DialogType {
    Info,
    Warning,
    Error,
    Question,
    Entry,
    Password,
    Progress,
    FileSelection,
    List,
    Calendar,
}

fn print_help() {
    println!(
        r#"zenity-rs {VERSION} - Display simple GUI dialogs from the command line

USAGE:
    zenity-rs [OPTIONS] --<dialog-type> [VALUES...]

DIALOG TYPES:
    --info              Display an information dialog
    --warning           Display a warning dialog
    --error             Display an error dialog
    --question          Display a question dialog (Yes/No)
    --entry             Display a text entry dialog
    --password          Display a password entry dialog
    --progress          Display a progress dialog
    --file-selection    Display a file selection dialog
    --list              Display a list selection dialog
    --calendar          Display a calendar date picker

OPTIONS:
    --title=TEXT        Set the dialog title
    --text=TEXT         Set the dialog text/prompt
    --width=N           Set the dialog width
    --height=N          Set the dialog height
    --timeout=N         Auto-close after N seconds (exit code 5)
    --entry-text=TEXT   Set default text for entry dialog
    --hide-text         Hide entered text (password mode)
    --percentage=N      Initial progress percentage (0-100)
    --pulsate           Enable pulsating progress bar
    --auto-close        Close dialog when progress reaches 100%
    --directory         Select directories only (file-selection)
    --save              Save mode (file-selection)
    --filename=TEXT     Default filename for save mode
    --column=TEXT       Add a column header (list)
    --checklist         Enable multi-select with checkboxes (list)
    --radiolist         Enable single-select with radio buttons (list)
    --year=N            Initial year (calendar)
    --month=N           Initial month 1-12 (calendar)
    --day=N             Initial day 1-31 (calendar)
    -h, --help          Print this help message
    --version           Print version information

EXAMPLES:
    zenity-rs --info --text="Operation completed"
    zenity-rs --question --text="Do you want to continue?" --timeout=10
    zenity-rs --entry --text="Enter your name:" --entry-text="John"
    zenity-rs --password --text="Enter password:"
    echo "50" | zenity-rs --progress --text="Working..."
    zenity-rs --file-selection --title="Open File"
    zenity-rs --list --column="Name" --column="Size" file1 10KB file2 20KB
    zenity-rs --list --checklist --column="Select" --column="Item" FALSE A TRUE B
    zenity-rs --calendar --text="Select date:"

EXIT CODES:
    0   OK/Yes clicked, text entered, file/date selected
    1   Cancel/No clicked
    5   Timeout reached
    255 Dialog was closed
    100 Error occurred
"#
    );
}
