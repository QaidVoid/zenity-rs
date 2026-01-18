use std::fmt;

#[derive(Debug)]
pub enum Error {
    #[cfg(feature = "x11")]
    X11(X11Error),
    #[cfg(feature = "wayland")]
    Wayland(WaylandError),
    NoDisplay,
    Io(std::io::Error),
}

#[cfg(feature = "x11")]
#[derive(Debug)]
pub enum X11Error {
    Connect(x11rb::errors::ConnectError),
    Connection(x11rb::errors::ConnectionError),
    Reply(x11rb::errors::ReplyError),
    NoVisual,
}

#[cfg(feature = "wayland")]
#[derive(Debug)]
pub enum WaylandError {
    Connect(wayland_client::ConnectError),
    Dispatch(wayland_client::DispatchError),
    MissingGlobal(&'static str),
    NotConfigured,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            #[cfg(feature = "x11")]
            Error::X11(e) => write!(f, "X11 error: {e}"),
            #[cfg(feature = "wayland")]
            Error::Wayland(e) => write!(f, "Wayland error: {e}"),
            Error::NoDisplay => write!(f, "no display server available"),
            Error::Io(e) => write!(f, "IO error: {e}"),
        }
    }
}

impl std::error::Error for Error {}

#[cfg(feature = "x11")]
impl fmt::Display for X11Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            X11Error::Connect(e) => write!(f, "connect: {e}"),
            X11Error::Connection(e) => write!(f, "connection: {e}"),
            X11Error::Reply(e) => write!(f, "reply: {e}"),
            X11Error::NoVisual => write!(f, "no suitable visual found"),
        }
    }
}

#[cfg(feature = "wayland")]
impl fmt::Display for WaylandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WaylandError::Connect(e) => write!(f, "connect: {e}"),
            WaylandError::Dispatch(e) => write!(f, "dispatch: {e}"),
            WaylandError::MissingGlobal(name) => write!(f, "missing global: {name}"),
            WaylandError::NotConfigured => write!(f, "surface not configured"),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

#[cfg(feature = "x11")]
impl From<x11rb::errors::ConnectError> for Error {
    fn from(e: x11rb::errors::ConnectError) -> Self {
        Error::X11(X11Error::Connect(e))
    }
}

#[cfg(feature = "x11")]
impl From<x11rb::errors::ConnectionError> for Error {
    fn from(e: x11rb::errors::ConnectionError) -> Self {
        Error::X11(X11Error::Connection(e))
    }
}

#[cfg(feature = "x11")]
impl From<x11rb::errors::ReplyError> for Error {
    fn from(e: x11rb::errors::ReplyError) -> Self {
        Error::X11(X11Error::Reply(e))
    }
}

#[cfg(feature = "x11")]
impl From<x11rb::errors::ReplyOrIdError> for Error {
    fn from(e: x11rb::errors::ReplyOrIdError) -> Self {
        match e {
            x11rb::errors::ReplyOrIdError::ConnectionError(e) => Error::X11(X11Error::Connection(e)),
            x11rb::errors::ReplyOrIdError::X11Error(e) => Error::X11(X11Error::Reply(e.into())),
            x11rb::errors::ReplyOrIdError::IdsExhausted => Error::X11(X11Error::NoVisual),
        }
    }
}

#[cfg(feature = "wayland")]
impl From<wayland_client::ConnectError> for Error {
    fn from(e: wayland_client::ConnectError) -> Self {
        Error::Wayland(WaylandError::Connect(e))
    }
}

#[cfg(feature = "wayland")]
impl From<wayland_client::DispatchError> for Error {
    fn from(e: wayland_client::DispatchError) -> Self {
        Error::Wayland(WaylandError::Dispatch(e))
    }
}

#[cfg(feature = "wayland")]
impl From<wayland_client::backend::WaylandError> for Error {
    fn from(e: wayland_client::backend::WaylandError) -> Self {
        // Convert to IO error since WaylandError is usually an IO issue
        Error::Io(std::io::Error::other(e.to_string()))
    }
}
