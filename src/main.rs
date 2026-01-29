//! zenity-rs - Display simple GUI dialogs from the command line.

use std::{io::IsTerminal, process::ExitCode};

use lexopt::prelude::*;
use zenity_rs::{
    calendar, entry, file_select, forms, list, message, password, progress, scale, text_info,
    ButtonPreset, CalendarResult, EntryResult, FileSelectResult, FormsResult, Icon, ListResult,
    ProgressResult, ScaleResult, TextInfoResult,
};

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
    let mut no_wrap = false;

    // Shared options (for list, forms, file-selector)
    let mut separator = String::from("|");
    let mut multiple_mode = false;

    // Progress options
    let mut percentage: u32 = 0;
    let mut pulsate = false;
    let mut auto_close = false;
    let mut auto_kill = false;
    let mut no_cancel = false;
    let mut time_remaining = false;

    // File selection options
    let mut directory_mode = false;
    let mut save_mode = false;
    let mut filename = String::new();
    let mut file_filters: Vec<zenity_rs::FileFilter> = Vec::new();

    // List options
    let mut columns: Vec<String> = Vec::new();
    let mut list_values: Vec<String> = Vec::new();
    let mut checklist = false;
    let mut radiolist = false;
    let mut hidden_columns: Vec<usize> = Vec::new();

    // Calendar options
    let mut cal_year: Option<u32> = None;
    let mut cal_month: Option<u32> = None;
    let mut cal_day: Option<u32> = None;

    // Text info options
    let mut checkbox_text = String::new();

    // Scale options
    let mut scale_value: i32 = 0;
    let mut scale_min: i32 = 0;
    let mut scale_max: i32 = 100;
    let mut scale_step: i32 = 1;
    let mut hide_value = false;

    // Forms options
    let mut form_entries: Vec<String> = Vec::new();
    let mut form_passwords: Vec<String> = Vec::new();

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
            Long("text-info") => dialog_type = Some(DialogType::TextInfo),
            Long("scale") => dialog_type = Some(DialogType::Scale),
            Long("forms") => dialog_type = Some(DialogType::Forms),

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
            Long("no-wrap") => no_wrap = true,
            Long("separator") => separator = parser.value()?.string()?,

            // Progress options
            Long("percentage") => percentage = parser.value()?.string()?.parse()?,
            Long("pulsate") => pulsate = true,
            Long("auto-close") => auto_close = true,
            Long("auto-kill") => auto_kill = true,
            Long("no-cancel") => no_cancel = true,
            Long("time-remaining") => time_remaining = true,

            // File selection options
            Long("directory") => directory_mode = true,
            Long("save") => save_mode = true,
            Long("multiple") => {
                multiple_mode = true;
            }
            Long("filename") => filename = parser.value()?.string()?,
            Long("file-filter") => {
                let filter_spec = parser.value()?.string()?;
                // Parse "Name | Pattern1 Pattern2 Pattern3" format
                if let Some((name, patterns_str)) = filter_spec.split_once('|') {
                    let name = name.trim().to_string();
                    // Split patterns by whitespace and filter empty strings
                    let patterns: Vec<String> = patterns_str
                        .split_whitespace()
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string())
                        .collect();
                    file_filters.push(zenity_rs::FileFilter {
                        name,
                        patterns,
                    });
                } else {
                    // Just pattern provided, use it as both name and single pattern
                    file_filters.push(zenity_rs::FileFilter {
                        name: filter_spec.clone(),
                        patterns: vec![filter_spec],
                    });
                }
            }

            // List options
            Long("column") => columns.push(parser.value()?.string()?),
            Long("checklist") => checklist = true,
            Long("radiolist") => radiolist = true,
            Long("hide-column") => hidden_columns.push(parser.value()?.string()?.parse()?),

            // Calendar options
            Long("year") => cal_year = Some(parser.value()?.string()?.parse()?),
            Long("month") => cal_month = Some(parser.value()?.string()?.parse()?),
            Long("day") => cal_day = Some(parser.value()?.string()?.parse()?),

            // Text info options
            Long("checkbox") => checkbox_text = parser.value()?.string()?,

            // Scale options
            Long("value") => scale_value = parser.value()?.string()?.parse()?,
            Long("min-value") => scale_min = parser.value()?.string()?.parse()?,
            Long("max-value") => scale_max = parser.value()?.string()?.parse()?,
            Long("step") => scale_step = parser.value()?.string()?.parse()?,
            Long("hide-value") => hide_value = true,

            // Forms options
            Long("add-entry") => form_entries.push(parser.value()?.string()?),
            Long("add-password") => form_passwords.push(parser.value()?.string()?),

            // Ignored options (for compatibility with zenity)
            Long("modal") => { /* Ignored */ }

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
                .title(if title.is_empty() {
                    "Information"
                } else {
                    &title
                })
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
            if no_wrap {
                builder = builder.no_wrap(true);
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
            if no_wrap {
                builder = builder.no_wrap(true);
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
            if no_wrap {
                builder = builder.no_wrap(true);
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
            if no_wrap {
                builder = builder.no_wrap(true);
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
                .auto_close(auto_close)
                .auto_kill(auto_kill)
                .no_cancel(no_cancel)
                .time_remaining(time_remaining);
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
            builder = builder
                .directory(directory_mode)
                .save(save_mode)
                .multiple(multiple_mode)
                .separator(&separator);
            if !filename.is_empty() {
                builder = builder.filename(&filename);
            }
            for filter in file_filters {
                builder = builder.add_filter(filter);
            }
            if let Some(w) = width {
                builder = builder.width(w);
            }
            if let Some(h) = height {
                builder = builder.height(h);
            }
            let result = builder.show()?;
            handle_file_select_result(result, &separator)
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
            } else if multiple_mode {
                builder = builder.multiple();
            }
            for col in &hidden_columns {
                builder = builder.hide_column(*col);
            }

            // Determine column count for rows
            let num_columns = columns.len().max(1);

            // Build rows from list_values based on column count
            for chunk in list_values.chunks(num_columns) {
                builder = builder.row(chunk.to_vec());
            }

            // Read additional rows from stdin if data is being piped
            // Zenity format: each line is one column value, multiple lines form one row
            if !std::io::stdin().is_terminal() {
                use std::io::{self, BufRead};
                let stdin = io::stdin();
                let lines: Vec<String> = stdin.lock().lines().filter_map(|l| l.ok()).collect();
                // Group lines by num_columns to form rows
                for chunk in lines.chunks(num_columns) {
                    builder = builder.row(chunk.to_vec());
                }
            }

            if let Some(w) = width {
                builder = builder.width(w);
            }
            if let Some(h) = height {
                builder = builder.height(h);
            }
            let result = builder.show()?;
            handle_list_result(result, &separator)
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
        DialogType::TextInfo => {
            let mut builder = text_info();
            if !title.is_empty() {
                builder = builder.title(&title);
            }
            if !filename.is_empty() {
                builder = builder.filename(&filename);
            }
            let has_checkbox = !checkbox_text.is_empty();
            if has_checkbox {
                builder = builder.checkbox(&checkbox_text);
            }
            if let Some(w) = width {
                builder = builder.width(w);
            }
            if let Some(h) = height {
                builder = builder.height(h);
            }
            let result = builder.show()?;
            handle_text_info_result(result, has_checkbox)
        }
        DialogType::Scale => {
            let mut builder = scale();
            if !title.is_empty() {
                builder = builder.title(&title);
            }
            if !text.is_empty() {
                builder = builder.text(&text);
            }
            builder = builder
                .value(scale_value)
                .min_value(scale_min)
                .max_value(scale_max)
                .step(scale_step)
                .hide_value(hide_value);
            if let Some(w) = width {
                builder = builder.width(w);
            }
            if let Some(h) = height {
                builder = builder.height(h);
            }
            let result = builder.show()?;
            handle_scale_result(result)
        }
        DialogType::Forms => {
            let mut builder = forms();
            if !title.is_empty() {
                builder = builder.title(&title);
            }
            if !text.is_empty() {
                builder = builder.text(&text);
            }
            // Add fields in the order they were specified
            for label in &form_entries {
                builder = builder.add_entry(label);
            }
            for label in &form_passwords {
                builder = builder.add_password(label);
            }
            builder = builder.separator(&separator);
            if let Some(w) = width {
                builder = builder.width(w);
            }
            if let Some(h) = height {
                builder = builder.height(h);
            }
            let result = builder.show()?;
            handle_forms_result(result, &separator)
        }
    }
}

fn handle_list_result(
    result: ListResult,
    separator: &str,
) -> Result<i32, Box<dyn std::error::Error>> {
    match result {
        ListResult::Selected(items) => {
            println!("{}", items.join(separator));
            Ok(0)
        }
        ListResult::Cancelled => Ok(1),
        ListResult::Closed => Ok(255),
    }
}

fn handle_calendar_result(result: CalendarResult) -> Result<i32, Box<dyn std::error::Error>> {
    match result {
        CalendarResult::Selected {
            year,
            month,
            day,
        } => {
            println!("{:04}-{:02}-{:02}", year, month, day);
            Ok(0)
        }
        CalendarResult::Cancelled => Ok(1),
        CalendarResult::Closed => Ok(255),
    }
}

fn handle_file_select_result(
    result: FileSelectResult,
    separator: &str,
) -> Result<i32, Box<dyn std::error::Error>> {
    match result {
        FileSelectResult::Selected(path) => {
            println!("{}", path.display());
            Ok(0)
        }
        FileSelectResult::SelectedMultiple(paths) => {
            println!(
                "{}",
                paths
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(separator)
            );
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

fn handle_text_info_result(
    result: TextInfoResult,
    has_checkbox: bool,
) -> Result<i32, Box<dyn std::error::Error>> {
    match result {
        TextInfoResult::Ok {
            checkbox_checked,
        } => {
            // If checkbox was specified but not checked, return 1
            // Otherwise return 0
            if has_checkbox && !checkbox_checked {
                Ok(1)
            } else {
                Ok(0)
            }
        }
        TextInfoResult::Cancelled => Ok(1),
        TextInfoResult::Closed => Ok(255),
    }
}

fn handle_scale_result(result: ScaleResult) -> Result<i32, Box<dyn std::error::Error>> {
    match result {
        ScaleResult::Value(v) => {
            println!("{}", v);
            Ok(0)
        }
        ScaleResult::Cancelled => Ok(1),
        ScaleResult::Closed => Ok(255),
    }
}

fn handle_forms_result(
    result: FormsResult,
    separator: &str,
) -> Result<i32, Box<dyn std::error::Error>> {
    match result {
        FormsResult::Values(values) => {
            println!("{}", values.join(separator));
            Ok(0)
        }
        FormsResult::Cancelled => Ok(1),
        FormsResult::Closed => Ok(255),
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
    TextInfo,
    Scale,
    Forms,
}

fn print_help() {
    println!(
        r#"zenity-rs {VERSION} - Display simple GUI dialogs from the command line

USAGE:
    zenity-rs --<dialog-type> [OPTIONS] [VALUES...]

 COMMON OPTIONS:
    --title=TEXT        Set the dialog title
    --text=TEXT         Set the dialog text/prompt
    --width=N           Set the dialog width (minimum when --no-wrap is used)
    --height=N          Set the dialog height
    --no-wrap          Do not wrap text (width becomes minimum, content can expand)
    -h, --help          Print this help message
    --version           Print version information

 DIALOG TYPES AND OPTIONS:

  Message Dialogs:
    --info              Display an information dialog
    --warning           Display a warning dialog
    --error             Display an error dialog
    --question          Display a question dialog (Yes/No)
      --timeout=N       Auto-close after N seconds (exit code 5)
      --no-wrap        Do not wrap text (width becomes minimum, content can expand)

  --entry               Display a text entry dialog
    --entry-text=TEXT Set default text
    --hide-text       Hide entered text (password mode)

  --password            Display a password entry dialog (same as --entry --hide-text)

  --progress            Display a progress dialog (reads percentage from stdin)
    --percentage=N    Initial progress percentage (0-100)
    --pulsate         Enable pulsating/indeterminate mode
    --auto-close      Close dialog when progress reaches 100%
    --auto-kill       Kill parent process if Cancel button is pressed
    --no-cancel       Hide Cancel button
    --time-remaining  Show estimated time remaining

   --file-selection      Display a file selection dialog
    --directory       Select directories only
    --save            Save mode (allows entering new filename)
    --multiple        Allow multiple file selection
    --separator=TEXT  Output separator for multiple files (default: space)
    --filename=TEXT   Default filename/path
    --file-filter=SPEC Add file filter (e.g., "*.rs" or "Video | *.mkv *.mp4")

   --list                Display a list selection dialog
     --column=TEXT     Add a column header (can be repeated)
     --checklist       Enable multi-select with checkboxes
     --radiolist       Enable single-select with radio buttons
     --multiple        Enable multi-select without checkboxes
     --hide-column=N   Hide column N (1-based, can be repeated)
     [VALUES...]       Row values (number must match column count)

  --calendar            Display a calendar date picker
    --year=N          Initial year
    --month=N         Initial month (1-12)
    --day=N           Initial day (1-31)

  --text-info           Display scrollable text from file or stdin
    --filename=TEXT   Read text from file (otherwise reads stdin)
    --checkbox=TEXT   Add checkbox with label (for agreements)

  --scale               Display a slider to select a numeric value
    --value=N         Initial value (default: 0)
    --min-value=N     Minimum value (default: 0)
    --max-value=N     Maximum value (default: 100)
    --step=N          Step increment (default: 1)
    --hide-value      Hide the numeric value display

  --forms               Display a form with multiple input fields
    --add-entry=LABEL Add a text entry field (can be repeated)
    --add-password=LABEL Add a password field (can be repeated)
    --separator=CHAR  Output separator (default: |)

 EXAMPLES:
    zenity-rs --info --text="Operation completed"
    zenity-rs --question --text="Continue?" --timeout=10
    zenity-rs --entry --text="Enter name:" --entry-text="John"
    zenity-rs --password --text="Enter password:"
    echo "50" | zenity-rs --progress --text="Working..." --auto-close
    zenity-rs --file-selection --save --filename="output.txt"
    zenity-rs --file-selection --multiple --file-filter="*.rs" --file-filter="*.txt"
    zenity-rs --file-selection --multiple --separator="|" file1.rs file2.txt file3.rs
    zenity-rs --file-selection --file-filter="Video | *.mkv *.mp4 *.avi" --file-filter="Image | *.jpg *.png *.gif"
    zenity-rs --list --column="Name" --column="Size" file1 10KB file2 20KB
    zenity-rs --calendar --text="Select date:" --year=2024 --month=12
    zenity-rs --text-info --filename=LICENSE --checkbox="I accept"
    zenity-rs --scale --text="Volume:" --value=50 --max-value=100
    zenity-rs --forms --add-entry="Name" --add-password="Password"

EXIT CODES:
    0   OK/Yes clicked, or value selected
    1   Cancel/No clicked, or checkbox unchecked
    5   Timeout reached
    255 Dialog was closed (ESC or window close)
    100 Error occurred
"#
    );
}
