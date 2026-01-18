#[cfg(feature = "x11")]
pub(crate) mod x11;
#[cfg(feature = "wayland")]
pub(crate) mod wayland;

use crate::error::Error;
use crate::render::Canvas;

use bitflags::bitflags;

/// Trait for connecting to a display server.
pub(crate) trait DisplayConnection: Sized {
    type Window: Window;

    fn connect() -> Result<Self, Error>;
    fn create_window(&self, width: u16, height: u16) -> Result<Self::Window, Error>;
}

/// Trait for interacting with a window.
pub(crate) trait Window {
    fn set_title(&mut self, title: &str) -> Result<(), Error>;
    fn set_contents(&mut self, canvas: &Canvas) -> Result<(), Error>;
    fn show(&mut self) -> Result<(), Error>;
    fn wait_for_event(&mut self) -> Result<WindowEvent, Error>;
    fn poll_for_event(&mut self) -> Result<Option<WindowEvent>, Error>;
    fn start_drag(&mut self) -> Result<(), Error>;
}

/// Events that can be emitted by a window.
#[derive(Debug, Clone)]
pub(crate) enum WindowEvent {
    CloseRequested,
    RedrawRequested,
    CursorEnter(CursorPos),
    CursorMove(CursorPos),
    CursorLeave,
    ButtonPress(MouseButton),
    ButtonRelease(MouseButton),
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
}

/// Creates a window using the best available backend.
/// Tries Wayland first (if WAYLAND_DISPLAY is set), then falls back to X11.
pub(crate) fn create_window(width: u16, height: u16) -> Result<AnyWindow, Error> {
    #[cfg(feature = "wayland")]
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        match wayland::Connection::connect() {
            Ok(conn) => match conn.create_window(width, height) {
                Ok(w) => return Ok(AnyWindow::Wayland(w)),
                Err(e) => eprintln!("Wayland window creation failed: {e}"),
            },
            Err(e) => eprintln!("Wayland connection failed: {e}"),
        }
    }

    #[cfg(feature = "x11")]
    {
        let conn = x11::Connection::connect()?;
        let w = conn.create_window(width, height)?;
        return Ok(AnyWindow::X11(w));
    }

    #[cfg(not(any(feature = "x11", feature = "wayland")))]
    compile_error!("At least one of 'x11' or 'wayland' features must be enabled");

    #[allow(unreachable_code)]
    Err(Error::NoDisplay)
}
