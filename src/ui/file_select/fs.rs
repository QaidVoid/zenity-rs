//! Filesystem access and formatting for the file selection dialog.
//!
//! Nothing here touches the window or the canvas, so it can be exercised
//! without a display.

use std::{
    collections::HashSet,
    fs::{self, Metadata},
    path::{Path, PathBuf},
    time::SystemTime,
};

use super::FileFilter;

pub(super) struct DirEntry {
    pub(super) name: String,
    pub(super) path: PathBuf,
    pub(super) is_dir: bool,
    pub(super) size: u64,
    pub(super) modified: Option<SystemTime>,
}

/// Quick access location.
#[derive(Clone)]
pub(super) struct QuickAccess {
    pub(super) name: String,
    pub(super) path: PathBuf,
    pub(super) icon: QuickAccessIcon,
}

#[derive(Clone, Copy)]
pub(super) enum QuickAccessIcon {
    Home,
    Desktop,
    Documents,
    Downloads,
    Pictures,
    Music,
    Videos,
    Folder,
}

/// Represents a mounted drive
#[derive(Clone)]
pub(super) struct MountPoint {
    pub(super) device: String,
    pub(super) mount_point: PathBuf,
    pub(super) label: Option<String>,
}

/// Icon for mount point type
#[derive(Clone, Copy)]
pub(super) enum MountIcon {
    UsbDrive,
    ExternalHdd,
    Optical,
    Generic,
}

pub(super) fn build_quick_access(start_dir: &Path) -> Vec<QuickAccess> {
    let mut items = Vec::new();

    let mut push = |name: &str, path: Option<PathBuf>, icon: QuickAccessIcon| {
        if let Some(path) = path {
            items.push(QuickAccess {
                name: name.to_string(),
                path,
                icon,
            });
        }
    };

    push("Home", dirs::home_dir(), QuickAccessIcon::Home);
    push("Desktop", dirs::desktop_dir(), QuickAccessIcon::Desktop);
    push(
        "Documents",
        dirs::document_dir(),
        QuickAccessIcon::Documents,
    );
    push(
        "Downloads",
        dirs::download_dir(),
        QuickAccessIcon::Downloads,
    );
    push("Pictures", dirs::picture_dir(), QuickAccessIcon::Pictures);
    push("Music", dirs::audio_dir(), QuickAccessIcon::Music);
    push("Videos", dirs::video_dir(), QuickAccessIcon::Videos);

    // The folder the dialog opens in and the one it was launched from, so both
    // stay one click away
    for dir in [Some(start_dir.to_path_buf()), std::env::current_dir().ok()]
        .into_iter()
        .flatten()
    {
        if items.iter().any(|i| i.path == dir) {
            continue;
        }
        if let Some(name) = dir.file_name().and_then(|n| n.to_str()) {
            items.push(QuickAccess {
                name: name.to_string(),
                path: dir.clone(),
                icon: QuickAccessIcon::Folder,
            });
        }
    }

    items
}

pub(super) fn get_mounted_drives() -> Vec<MountPoint> {
    let mut drives = Vec::new();

    // Parse /run/mount/utab for user-mounted drives (much cleaner than /proc/mounts)
    if let Ok(content) = std::fs::read_to_string("/run/mount/utab") {
        for line in content.lines() {
            let mut device: Option<String> = None;
            let mut mount_point: Option<PathBuf> = None;

            // Parse KEY=VALUE pairs
            for pair in line.split_whitespace() {
                let mut kv = pair.split('=');
                if let Some(key) = kv.next() {
                    let value = kv.next();
                    match key {
                        "SRC" => {
                            device = value.map(|v| v.to_string());
                        }
                        "TARGET" => {
                            mount_point = value.map(PathBuf::from);
                        }
                        _ => {}
                    }
                }
            }

            // We have both source and target, create a mount point entry
            if let (Some(dev), Some(mp)) = (device, mount_point) {
                // Skip root filesystem
                if mp.as_os_str() == "/" {
                    continue;
                }

                let label = get_volume_label(&dev);

                drives.push(MountPoint {
                    device: dev,
                    mount_point: mp,
                    label,
                });
            }
        }
    }

    drives
}

fn get_volume_label(device: &str) -> Option<String> {
    use std::process::Command;

    let output = Command::new("lsblk")
        .args(["-o", "LABEL", "-n", device])
        .output()
        .ok()?;

    let label = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if label.is_empty() { None } else { Some(label) }
}

pub(super) fn get_mount_icon(device: &str) -> MountIcon {
    // Check for USB by looking for symlink in /dev/disk/by-id/usb-*
    let is_usb = device
        .strip_prefix("/dev/")
        .map(|_dev| {
            std::fs::read_dir("/dev/disk/by-id")
                .ok()
                .map(|entries| {
                    entries
                        .filter_map(|e| e.ok())
                        .filter(|e| e.file_name().to_string_lossy().starts_with("usb-"))
                        .any(|e| {
                            e.path()
                                .canonicalize()
                                .ok()
                                .as_ref()
                                .and_then(|p| p.to_str())
                                .map(|p| device.contains(p))
                                .unwrap_or(false)
                        })
                })
                .unwrap_or(false)
        })
        .unwrap_or(false);

    if is_usb {
        return MountIcon::UsbDrive;
    }

    if device.starts_with("/dev/sr") || device.starts_with("/dev/scd") {
        return MountIcon::Optical;
    }

    if device.starts_with("/dev/nvme") || device.starts_with("/dev/mmc") {
        return MountIcon::ExternalHdd;
    }

    MountIcon::Generic
}

fn load_directory(path: &Path, entries: &mut Vec<DirEntry>, dirs_only: bool, show_hidden: bool) {
    entries.clear();

    if let Some(parent) = path.parent() {
        entries.push(DirEntry {
            name: "..".to_string(),
            path: parent.to_path_buf(),
            is_dir: true,
            size: 0,
            modified: None,
        });
    }

    let mut dirs: Vec<DirEntry> = Vec::new();
    let mut files: Vec<DirEntry> = Vec::new();

    if let Ok(read_dir) = fs::read_dir(path) {
        for entry in read_dir.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();

            if !show_hidden && name.starts_with('.') {
                continue;
            }

            let metadata = entry.path().metadata().ok();
            let is_dir = metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false);

            if dirs_only && !is_dir {
                continue;
            }

            let size = metadata.as_ref().map(Metadata::len).unwrap_or(0);
            let modified = metadata.as_ref().and_then(|m| m.modified().ok());

            let de = DirEntry {
                name,
                path: entry.path(),
                is_dir,
                size,
                modified,
            };

            if is_dir {
                dirs.push(de);
            } else {
                files.push(de);
            }
        }
    }

    dirs.sort_by_key(|a| a.name.to_lowercase());
    files.sort_by_key(|a| a.name.to_lowercase());

    entries.extend(dirs);
    entries.extend(files);
}

fn update_filtered(
    all: &[DirEntry],
    search: &str,
    filtered: &mut Vec<usize>,
    filters: &[FileFilter],
) {
    filtered.clear();
    for (i, entry) in all.iter().enumerate() {
        let matches_search = search.is_empty() || entry.name.to_lowercase().contains(search);
        if entry.is_dir {
            if matches_search {
                filtered.push(i);
            }
        } else {
            let matches_filter = filters.is_empty() || matches_any_filter(&entry.name, filters);
            if matches_filter && matches_search {
                filtered.push(i);
            }
        }
    }
}

fn matches_any_filter(name: &str, filters: &[FileFilter]) -> bool {
    let name_lower = name.to_lowercase();
    for filter in filters {
        for pattern in &filter.patterns {
            if matches_pattern(&name_lower, pattern) {
                return true;
            }
        }
    }
    false
}

fn matches_pattern(name: &str, pattern: &str) -> bool {
    let pattern_lower = pattern.to_lowercase();
    if pattern_lower == "*" {
        return true;
    }

    if pattern_lower.starts_with("*") && pattern_lower.ends_with("*") {
        let inner = &pattern_lower[1..pattern_lower.len() - 1];
        name.contains(inner)
    } else if let Some(suffix) = pattern_lower.strip_prefix("*") {
        name.ends_with(suffix)
    } else if pattern_lower.ends_with("*") {
        let prefix = &pattern_lower[..pattern_lower.len() - 1];
        name.starts_with(prefix)
    } else {
        name == pattern_lower
    }
}

/// Returns all file entry names matching `prefix` (case-insensitive), up to `max` items.
pub(super) fn find_all_completions(
    entries: &[DirEntry],
    text: &str,
    max: usize,
    files_only: bool,
    prefix_only: bool,
) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    let text_lower = text.to_lowercase();
    entries
        .iter()
        .filter(|e| {
            (!files_only || !e.is_dir) && {
                let name_lower = e.name.to_lowercase();
                if prefix_only {
                    name_lower.starts_with(&text_lower)
                } else {
                    name_lower.contains(&text_lower)
                }
            }
        })
        .take(max)
        .map(|e| e.name.clone())
        .collect()
}

/// Formats a byte count with a binary unit suffix.
pub(super) fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

pub(super) fn format_date(time: Option<SystemTime>) -> String {
    match time {
        Some(t) => {
            let duration = t.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();
            let secs = duration.as_secs();
            // Simple date format (just show relative or basic)
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let diff = now.saturating_sub(secs);

            if diff < 60 {
                "Just now".to_string()
            } else if diff < 3600 {
                format!("{} min ago", diff / 60)
            } else if diff < 86400 {
                format!("{} hr ago", diff / 3600)
            } else if diff < 86400 * 7 {
                let days = diff / 86400;
                format!("{days} day{} ago", if days == 1 { "" } else { "s" })
            } else {
                const MONTHS: [&str; 12] = [
                    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov",
                    "Dec",
                ];
                let (year, month, day) = civil_from_days((secs / 86400) as i64);
                let (this_year, _, _) = civil_from_days((now / 86400) as i64);
                let month = MONTHS[(month - 1) as usize];
                if year == this_year {
                    format!("{day} {month}")
                } else {
                    format!("{day} {month} {year}")
                }
            }
        }
        None => "-".to_string(),
    }
}

/// Converts days since the Unix epoch into a civil (year, month, day) in UTC.
///
/// Days-to-civil conversion from Howard Hinnant's `chrono`-compatible algorithm.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe as i64 + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// The directory on screen, how the user got there, and what is selected in it.
///
/// `show()` keeps one of these instead of threading the same seven values
/// through every navigation helper by hand.
pub(super) struct Browser {
    pub(super) current_dir: PathBuf,
    pub(super) all_entries: Vec<DirEntry>,
    /// Indices into `all_entries` surviving the search text and the filters.
    pub(super) filtered_entries: Vec<usize>,
    /// Indices into `all_entries`, not into `filtered_entries`.
    pub(super) selected_indices: HashSet<usize>,
    pub(super) scroll_offset: usize,
    pub(super) show_hidden: bool,
    history: Vec<PathBuf>,
    history_index: usize,
    directory_mode: bool,
    filters: Vec<FileFilter>,
}

impl Browser {
    /// Opens `start`, reading its contents immediately.
    pub(super) fn new(
        start: PathBuf,
        directory_mode: bool,
        filters: Vec<FileFilter>,
        search: &str,
    ) -> Self {
        let mut browser = Self {
            history: vec![start.clone()],
            history_index: 0,
            current_dir: start,
            all_entries: Vec::new(),
            filtered_entries: Vec::new(),
            selected_indices: HashSet::new(),
            scroll_offset: 0,
            show_hidden: false,
            directory_mode,
            filters,
        };
        browser.reload(search);
        browser
    }

    /// Rereads the current directory, clearing the selection and the scroll.
    pub(super) fn reload(&mut self, search: &str) {
        load_directory(
            &self.current_dir,
            &mut self.all_entries,
            self.directory_mode,
            self.show_hidden,
        );
        self.refilter(search);
        self.selected_indices.clear();
        self.scroll_offset = 0;
    }

    /// Reapplies `search` and the filters without touching the disk.
    pub(super) fn refilter(&mut self, search: &str) {
        update_filtered(
            &self.all_entries,
            search,
            &mut self.filtered_entries,
            &self.filters,
        );
    }

    /// Moves to `dest` and records the step in history.
    ///
    /// Re-entering the current directory, or one that no longer exists, does
    /// nothing.
    pub(super) fn navigate_to(&mut self, dest: PathBuf, search: &str) {
        if dest == self.current_dir || !dest.exists() {
            return;
        }
        self.history.truncate(self.history_index + 1);
        self.history.push(dest.clone());
        self.history_index = self.history.len() - 1;
        self.current_dir = dest;
        self.reload(search);
    }

    /// Steps back through history, if there is anywhere to go.
    pub(super) fn go_back(&mut self, search: &str) {
        if self.can_go_back() {
            self.history_index -= 1;
            self.current_dir = self.history[self.history_index].clone();
            self.reload(search);
        }
    }

    /// Steps forward through history, if there is anywhere to go.
    pub(super) fn go_forward(&mut self, search: &str) {
        if self.can_go_forward() {
            self.history_index += 1;
            self.current_dir = self.history[self.history_index].clone();
            self.reload(search);
        }
    }

    pub(super) fn can_go_back(&self) -> bool {
        self.history_index > 0
    }

    pub(super) fn can_go_forward(&self) -> bool {
        self.history_index + 1 < self.history.len()
    }

    /// Identifies the history position for the chrome cache signature.
    pub(super) fn history_step(&self) -> (usize, usize) {
        (self.history_index, self.history.len())
    }
}

#[cfg(test)]
mod tests {
    use super::civil_from_days;

    #[test]
    fn days_convert_to_civil_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
        assert_eq!(civil_from_days(20_689), (2026, 8, 24));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
    }
}
