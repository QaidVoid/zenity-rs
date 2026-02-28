# zenity-rs

A lightweight, pure Rust implementation for displaying GUI dialogs from the command line. Designed for shell scripts and CLI tools that need simple user interaction.

## Features

- **Message dialogs**: Info, warning, error, and question dialogs
- **Input dialogs**: Text entry and password input
- **Progress dialog**: With stdin control and pulsating mode
- **File selection**: Open/save dialogs with directory navigation
- **List selection**: Single-select, checklist, and radiolist modes
- **Calendar**: Date picker dialog
- **Text info**: Display scrollable text from file or stdin
- **Scale**: Slider to select a numeric value
- **Forms**: Multiple input fields in a single dialog

### Highlights

- Pure Rust with no GTK/Qt dependencies
- Native X11 and Wayland support
- Small static binary (~1.5MB with musl)
- Automatic theme detection (light/dark)
- Respects system keyboard layout

## Installation

```bash
cargo install zenity-rs
```

Or build from source:

```bash
git clone https://github.com/user/zenity-rs
cd zenity-rs
cargo build --release -Z build-std=std,panic_abort
```

## Usage

### Message Dialogs

```bash
# Information dialog
zenity-rs --info --text="Operation completed successfully"

# Warning dialog
zenity-rs --warning --text="This action cannot be undone"

# Error dialog
zenity-rs --error --text="Failed to save file"

# Question dialog (Yes/No)
zenity-rs --question --text="Do you want to continue?"
```

### Input Dialogs

```bash
# Text entry
zenity-rs --entry --text="Enter your name:" --entry-text="Default"

# Password input
zenity-rs --password --text="Enter password:"
```

### Progress Dialog

```bash
# Static progress
zenity-rs --progress --text="Processing..." --percentage=50

# Pulsating progress
zenity-rs --progress --text="Please wait..." --pulsate

# Controlled via stdin
(
  echo "10"
  sleep 1
  echo "50"
  sleep 1
  echo "100"
) | zenity-rs --progress --text="Working..." --auto-close
```

### File Selection

```bash
# Open file
zenity-rs --file-selection --title="Select a file"

# Save file
zenity-rs --file-selection --save --filename="output.txt"

# Select directory
zenity-rs --file-selection --directory
```

### List Selection

```bash
# Simple list
zenity-rs --list --column="Name" --column="Size" file1 10KB file2 20KB

# Checklist (multi-select)
zenity-rs --list --checklist --column="Select" --column="Item" FALSE "Option A" TRUE "Option B"

# Radiolist (single-select)
zenity-rs --list --radiolist --column="Select" --column="Item" FALSE "Option A" TRUE "Option B"
```

### Calendar

```bash
# Date picker
zenity-rs --calendar --text="Select a date"

# With initial date
zenity-rs --calendar --year=2024 --month=12 --day=25
```

### Text Info

```bash
# Display file contents
zenity-rs --text-info --filename=README.md --title="Read Me"

# Display from stdin
cat LICENSE | zenity-rs --text-info --title="License"

# With checkbox (for agreements)
zenity-rs --text-info --filename=LICENSE --checkbox="I accept the terms"
```

### Scale

```bash
# Basic slider
zenity-rs --scale --text="Select volume:"

# With custom range and initial value
zenity-rs --scale --text="Brightness:" --value=75 --min-value=0 --max-value=100

# With step increment
zenity-rs --scale --text="Select:" --min-value=0 --max-value=1000 --step=10

# Hide the value display
zenity-rs --scale --text="Level:" --hide-value
```

### Forms

```bash
# Multiple text fields
zenity-rs --forms --text="Enter details:" --add-entry="Name" --add-entry="Email"

# With password field
zenity-rs --forms --text="Login:" --add-entry="Username" --add-password="Password"

# Custom separator (default is |)
zenity-rs --forms --add-entry="First" --add-entry="Last" --separator=","
```

### Common Options

```bash
--title=TEXT      # Set dialog title
--text=TEXT       # Set dialog text/prompt
--width=N         # Set dialog width
--height=N        # Set dialog height
--timeout=N       # Auto-close after N seconds
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | OK/Yes clicked, or selection made |
| 1 | Cancel/No clicked, or dialog closed (ESC/window close) |
| 5 | Timeout reached |
| 100 | Error occurred |

## Building

### Default (X11 + Wayland)

```bash
cargo build --release -Z build-std=std,panic_abort
```

### X11 only

```bash
cargo build --release --no-default-features --features x11 -Z build-std=std,panic_abort
```

### Wayland only

```bash
cargo build --release --no-default-features --features wayland -Z build-std=std,panic_abort
```

### Static binary (musl)

```bash
cargo build --release --target x86_64-unknown-linux-musl -Z build-std=std,panic_abort
```

## License

MIT
