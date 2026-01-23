#[cfg(feature = "wayland")]
pub(crate) mod wayland;
#[cfg(feature = "x11")]
pub(crate) mod x11;

use bitflags::bitflags;

use crate::{error::Error, render::Canvas};

/// Default scale factor for rendering
pub(crate) const DEFAULT_SCALE: f32 = 1.0;

/// Trait for connecting to a display server.
pub(crate) trait DisplayConnection: Sized {
    type Window: Window;

    fn connect() -> Result<Self, Error>;
    fn create_window(&self, width: u16, height: u16) -> Result<Self::Window, Error>;
}

/// Cursor shape types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum CursorShape {
    /// Default arrow cursor.
    #[default]
    Default,
    /// Text input (I-beam) cursor.
    Text,
}

/// Trait for interacting with a window.
pub(crate) trait Window {
    fn set_title(&mut self, title: &str) -> Result<(), Error>;
    fn set_contents(&mut self, canvas: &Canvas) -> Result<(), Error>;
    fn show(&mut self) -> Result<(), Error>;
    fn wait_for_event(&mut self) -> Result<WindowEvent, Error>;
    fn poll_for_event(&mut self) -> Result<Option<WindowEvent>, Error>;
    fn start_drag(&mut self) -> Result<(), Error>;
    fn scale_factor(&self) -> f32;
    fn set_cursor(&mut self, shape: CursorShape) -> Result<(), Error>;
}

/// Events that can be emitted by a window.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) enum WindowEvent {
    CloseRequested,
    RedrawRequested,
    CursorEnter(CursorPos),
    CursorMove(CursorPos),
    CursorLeave,
    ButtonPress(MouseButton, Modifiers),
    ButtonRelease(MouseButton, Modifiers),
    Scroll(ScrollDirection),
    KeyPress(KeyEvent),
    KeyRelease(KeyEvent),
    TextInput(char),
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct CursorPos {
    pub x: i16,
    pub y: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MouseButton {
    Left,
    Middle,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone)]
pub(crate) struct KeyEvent {
    pub keysym: u32,
    pub modifiers: Modifiers,
}

bitflags! {
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub(crate) struct Modifiers: u8 {
        const SHIFT = 0x01;
        const CTRL  = 0x02;
        const ALT   = 0x04;
        const SUPER = 0x08;
    }
}

/// Type-erased window that can be either X11 or Wayland.
pub(crate) enum AnyWindow {
    #[cfg(feature = "x11")]
    X11(x11::X11Window),
    #[cfg(feature = "wayland")]
    Wayland(wayland::WaylandWindow),
}

impl Window for AnyWindow {
    fn set_title(&mut self, title: &str) -> Result<(), Error> {
        match self {
            #[cfg(feature = "x11")]
            AnyWindow::X11(w) => w.set_title(title),
            #[cfg(feature = "wayland")]
            AnyWindow::Wayland(w) => w.set_title(title),
        }
    }

    fn set_contents(&mut self, canvas: &Canvas) -> Result<(), Error> {
        match self {
            #[cfg(feature = "x11")]
            AnyWindow::X11(w) => w.set_contents(canvas),
            #[cfg(feature = "wayland")]
            AnyWindow::Wayland(w) => w.set_contents(canvas),
        }
    }

    fn show(&mut self) -> Result<(), Error> {
        match self {
            #[cfg(feature = "x11")]
            AnyWindow::X11(w) => w.show(),
            #[cfg(feature = "wayland")]
            AnyWindow::Wayland(w) => w.show(),
        }
    }

    fn wait_for_event(&mut self) -> Result<WindowEvent, Error> {
        match self {
            #[cfg(feature = "x11")]
            AnyWindow::X11(w) => w.wait_for_event(),
            #[cfg(feature = "wayland")]
            AnyWindow::Wayland(w) => w.wait_for_event(),
        }
    }

    fn poll_for_event(&mut self) -> Result<Option<WindowEvent>, Error> {
        match self {
            #[cfg(feature = "x11")]
            AnyWindow::X11(w) => w.poll_for_event(),
            #[cfg(feature = "wayland")]
            AnyWindow::Wayland(w) => w.poll_for_event(),
        }
    }

    fn start_drag(&mut self) -> Result<(), Error> {
        match self {
            #[cfg(feature = "x11")]
            AnyWindow::X11(w) => w.start_drag(),
            #[cfg(feature = "wayland")]
            AnyWindow::Wayland(w) => w.start_drag(),
        }
    }

    fn scale_factor(&self) -> f32 {
        match self {
            #[cfg(feature = "x11")]
            AnyWindow::X11(w) => w.scale_factor(),
            #[cfg(feature = "wayland")]
            AnyWindow::Wayland(w) => w.scale_factor(),
        }
    }

    fn set_cursor(&mut self, shape: CursorShape) -> Result<(), Error> {
        match self {
            #[cfg(feature = "x11")]
            AnyWindow::X11(w) => w.set_cursor(shape),
            #[cfg(feature = "wayland")]
            AnyWindow::Wayland(w) => w.set_cursor(shape),
        }
    }
}

/// Creates a window using the best available backend.
/// Prefers Wayland, falls back to X11.
pub(crate) fn create_window(width: u16, height: u16) -> Result<AnyWindow, Error> {
    #[cfg(feature = "wayland")]
    if let Some(window) = try_wayland(width, height) {
        return Ok(window);
    }

    #[cfg(feature = "x11")]
    return try_x11(width, height);

    #[cfg(not(any(feature = "x11", feature = "wayland")))]
    compile_error!("At least one of 'x11' or 'wayland' features must be enabled");
}

#[cfg(feature = "wayland")]
fn try_wayland(width: u16, height: u16) -> Option<AnyWindow> {
    let socket_name = find_wayland_socket()?;

    let _guard = SocketGuard::new(&socket_name);

    match wayland::Connection::connect() {
        Ok(conn) => {
            match conn.create_window(width, height) {
                Ok(w) => {
                    std::mem::forget(conn);
                    return Some(AnyWindow::Wayland(w));
                }
                Err(e) => eprintln!("Wayland window creation failed: {e}"),
            }
        }
        Err(e) => eprintln!("Wayland connection failed: {e}"),
    }

    None
}

#[cfg(feature = "wayland")]
fn find_wayland_socket() -> Option<String> {
    if std::env::var_os("WAYLAND_SOCKET").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some()
    {
        return None;
    }

    let xdg_runtime = std::env::var_os("XDG_RUNTIME_DIR")?;
    let xdg_path = std::path::PathBuf::from(&xdg_runtime);

    let rd = std::fs::read_dir(&xdg_path).ok()?;

    let mut chosen: Option<String> = None;
    let mut candidate_count: usize = 0;

    for entry in rd.flatten() {
        let fname = entry.file_name();
        if let Some(s) = fname.to_str() {
            if let Some(suffix) = s.strip_prefix("wayland-") {
                if suffix.is_empty() || !suffix.chars().all(|c| c.is_ascii_digit()) {
                    continue;
                }

                candidate_count += 1;

                if s == "wayland-0" {
                    chosen = Some(s.to_string());
                    break;
                }

                if chosen.is_none() {
                    chosen = Some(s.to_string());
                }
            }
        }
    }

    if candidate_count > 1 {
        eprintln!("zenity-rs: multiple wayland socket candidates found, using first");
    }

    chosen
}

#[cfg(feature = "x11")]
fn try_x11(width: u16, height: u16) -> Result<AnyWindow, Error> {
    let conn = x11::Connection::connect()?;
    let w = conn.create_window(width, height)?;
    Ok(AnyWindow::X11(w))
}

#[cfg(feature = "wayland")]
struct SocketGuard {
    old_value: Option<std::ffi::OsString>,
}

#[cfg(feature = "wayland")]
impl SocketGuard {
    fn new(path: &str) -> Self {
        let old_value = std::env::var_os("WAYLAND_DISPLAY");
        std::env::set_var("WAYLAND_DISPLAY", path);
        Self {
            old_value,
        }
    }
}

#[cfg(feature = "wayland")]
impl Drop for SocketGuard {
    fn drop(&mut self) {
        match &self.old_value {
            Some(old) => std::env::set_var("WAYLAND_DISPLAY", old),
            None => std::env::remove_var("WAYLAND_DISPLAY"),
        }
    }
}
