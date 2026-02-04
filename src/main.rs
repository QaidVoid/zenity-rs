//! zenity-rs - Display simple GUI dialogs from the command line.

use std::{io::IsTerminal, process::ExitCode};

use lexopt::prelude::*;
use zenity_rs::{
    ButtonPreset, CalendarResult, EntryResult, FileSelectResult, FormsResult, Icon, ListResult,
    ProgressResult, ScaleResult, TextInfoResult, calendar, entry, file_select, forms, list,
    message, password, progress, scale, text_info,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn handle_message_result(
    result: zenity_rs::DialogResult,
    extra_buttons: &[String],
    default_cancel_index: Option<usize>,
) -> i32 {
    match result {
        zenity_rs::DialogResult::Button(idx) => {
            if idx < extra_buttons.len() {
                // Extra button clicked - print the label and return exit code 1
                println!("{}", extra_buttons[idx]);
                1
            } else if let Some(cancel_idx) = default_cancel_index {
                if idx == cancel_idx {
                    // Default cancel button (or No button) clicked
                    1
                } else {
                    // Default OK (or Yes) button clicked
                    0
                }
            } else {
                // No cancel button, so first button is OK
                if idx == 0 { 0 } else { 1 }
            }
        }
        zenity_rs::DialogResult::Closed => 255,
        zenity_rs::DialogResult::Timeout => 5,
    }
}

fn get_icon(icon_name: &Option<String>, default: Icon) -> Icon {
    match icon_name {
        None => default,
        Some(name) => Icon::from_name(name).unwrap_or(default),
    }
}

fn get_button_preset(
    ok_label: &str,
    cancel_label: &str,
    _extra_buttons: &[String],
    switch_mode: bool,
    default: ButtonPreset,
) -> ButtonPreset {
    if switch_mode {
        return ButtonPreset::Empty;
    }
    if !ok_label.is_empty() || !cancel_label.is_empty() {
        let mut labels = Vec::new();
        if !ok_label.is_empty() {
            labels.push(ok_label.to_string());
        }
        if !cancel_label.is_empty() {
            labels.push(cancel_label.to_string());
        }
        if !labels.is_empty() {
            return ButtonPreset::Custom(labels);
        }
    }
    default
}

fn apply_message_options(
    builder: zenity_rs::MessageBuilder,
    timeout: Option<u32>,
    width: Option<u32>,
    height: Option<u32>,
    no_wrap: bool,
    no_markup: bool,
    ellipsize: bool,
    switch_mode: bool,
    _extra_buttons: &[String],
) -> zenity_rs::MessageBuilder {
    let mut builder = builder;
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
    if no_markup {
        builder = builder.no_markup(true);
    }
    if ellipsize {
        builder = builder.ellipsize(true);
    }
    if switch_mode {
        builder = builder.switch(true);
    }
    for btn in _extra_buttons {
        builder = builder.extra_button(btn);
    }
    builder
}

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

    // Message dialog options
    let mut icon_name: Option<String> = None;
    let mut no_markup = false;
    let mut ellipsize = false;
    let mut switch_mode = false;
    let mut extra_buttons: Vec<String> = Vec::new();
    let mut ok_label = String::new();
    let mut cancel_label = String::new();

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
            Long("no-markup") => no_markup = true,
            Long("ellipsize") => ellipsize = true,
            Long("icon-name") | Long("icon") => icon_name = Some(parser.value()?.string()?),
            Long("switch") => switch_mode = true,
            Long("extra-button") => extra_buttons.push(parser.value()?.string()?),
            Long("ok-label") => ok_label = parser.value()?.string()?,
            Long("cancel-label") => cancel_label = parser.value()?.string()?,
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
            let builder = message()
                .title(if title.is_empty() {
                    "Information"
                } else {
                    &title
                })
                .text(&text)
                .icon(get_icon(&icon_name, Icon::Info))
                .buttons(get_button_preset(
                    &ok_label,
                    &cancel_label,
                    &extra_buttons,
                    switch_mode,
                    ButtonPreset::Ok,
                ));
            let builder = apply_message_options(
                builder,
                timeout,
                width,
                height,
                no_wrap,
                no_markup,
                ellipsize,
                switch_mode,
                &extra_buttons,
            );
            let result = builder.show()?;
            Ok(handle_message_result(result, &extra_buttons, None))
        }
        DialogType::Warning => {
            let builder = message()
                .title(if title.is_empty() { "Warning" } else { &title })
                .text(&text)
                .icon(get_icon(&icon_name, Icon::Warning))
                .buttons(get_button_preset(
                    &ok_label,
                    &cancel_label,
                    &extra_buttons,
                    switch_mode,
                    ButtonPreset::Ok,
                ));
            let builder = apply_message_options(
                builder,
                timeout,
                width,
                height,
                no_wrap,
                no_markup,
                ellipsize,
                switch_mode,
                &extra_buttons,
            );
            let result = builder.show()?;
            Ok(handle_message_result(result, &extra_buttons, None))
        }
        DialogType::Error => {
            let builder = message()
                .title(if title.is_empty() { "Error" } else { &title })
                .text(&text)
                .icon(get_icon(&icon_name, Icon::Error))
                .buttons(get_button_preset(
                    &ok_label,
                    &cancel_label,
                    &extra_buttons,
                    switch_mode,
                    ButtonPreset::Ok,
                ));
            let builder = apply_message_options(
                builder,
                timeout,
                width,
                height,
                no_wrap,
                no_markup,
                ellipsize,
                switch_mode,
                &extra_buttons,
            );
            let result = builder.show()?;
            Ok(handle_message_result(result, &extra_buttons, None))
        }
        DialogType::Question => {
            let builder = message()
                .title(if title.is_empty() { "Question" } else { &title })
                .text(&text)
                .icon(get_icon(&icon_name, Icon::Question))
                .buttons(get_button_preset(
                    &ok_label,
                    &cancel_label,
                    &extra_buttons,
                    switch_mode,
                    ButtonPreset::YesNo,
                ));
            let builder = apply_message_options(
                builder,
                timeout,
                width,
                height,
                no_wrap,
                no_markup,
                ellipsize,
                switch_mode,
                &extra_buttons,
            );
            let result = builder.show()?;
            Ok(handle_message_result(
                result,
                &extra_buttons,
                Some(1 + extra_buttons.len()),
            ))
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
                let lines: Vec<String> = stdin.lock().lines().map_while(Result::ok).collect();
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
    --icon=ICON        Set the icon name (e.g., dialog-information, dialog-warning)
    --ok-label=TEXT    Set the label of the OK button
    --cancel-label=TEXT Set the label of the Cancel button
    --extra-button=TEXT Add an extra button (outputs label text, exit code 1+)
    --switch           Suppress OK/Cancel buttons, only show extra buttons
    --no-markup        Do not enable pango markup (for compatibility)
    --ellipsize        Enable ellipsizing in dialog text (for compatibility)
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
      --icon=ICON      Set the icon name (also accepts --icon-name for compatibility)
      --switch         Only show extra buttons (suppress OK/Cancel)
      --extra-button=TEXT Add extra buttons
      --no-markup      Do not enable pango markup (for compatibility)
      --ellipsize      Enable ellipsizing in dialog text (for compatibility)

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
