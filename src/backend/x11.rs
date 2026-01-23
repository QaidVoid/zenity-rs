//! X11 backend implementation.

use std::{ops::Deref, rc::Rc};

use kbvm::{lookup::LookupTable, xkb::x11::KbvmX11Ext};
use x11rb::{
    connection::Connection as X11rbConnection,
    properties::WmSizeHints,
    protocol::{
        xproto::{
            self, AtomEnum, ClientMessageEvent, ConfigureWindowAux, ConnectionExt as _,
            CreateWindowAux, EventMask, ImageFormat, KeyButMask, PropMode, StackMode, VisualClass,
            WindowClass,
        },
        Event,
    },
    rust_connection::RustConnection,
    wrapper::ConnectionExt as _,
};

use super::{
    CursorPos, CursorShape, DisplayConnection, KeyEvent, Modifiers, MouseButton, ScrollDirection,
    Window, WindowEvent,
};
use crate::{
    error::{Error, X11Error},
    render::Canvas,
};

x11rb::atom_manager! {
    pub Atoms: AtomCookie {
        UTF8_STRING,

        WM_PROTOCOLS,
        WM_DELETE_WINDOW,

        _NET_WM_NAME,
        _NET_WM_WINDOW_TYPE,
        _NET_WM_WINDOW_TYPE_DIALOG,

        _NET_WM_MOVERESIZE,
    }
}

enum WindowType {
    Dialog,
}

#[derive(Clone)]
pub(crate) struct Connection {
    inner: Rc<RustConnection>,
    screen: usize,
}

impl Deref for Connection {
    type Target = RustConnection;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DisplayConnection for Connection {
    type Window = X11Window;

    fn connect() -> Result<Self, Error> {
        let (conn, screen) = x11rb::connect(None)?;
        Ok(Self {
            inner: Rc::new(conn),
            screen,
        })
    }

    fn create_window(&self, width: u16, height: u16) -> Result<Self::Window, Error> {
        X11Window::create(self.clone(), width, height)
    }
}

const MOVERESIZE_MOVE: u32 = 8;
const KEYCODE_ESC: u8 = 9;
const WM_CLASS: &[u8] = b"zenity\0Zenity\0";

// X11 cursor font character constants
const XC_LEFT_PTR: u16 = 68; // Default arrow
const XC_XTERM: u16 = 152; // Text I-beam

pub(crate) struct X11Window {
    atoms: Atoms,
    conn: Connection,
    window: xproto::Window,
    gc: xproto::Gcontext,
    lookup_table: LookupTable,
    xkb_group: u8,
    cursor_default: xproto::Cursor,
    cursor_text: xproto::Cursor,
    current_cursor: CursorShape,
}

impl X11Window {
    fn create(conn: Connection, width: u16, height: u16) -> Result<Self, Error> {
        let atoms = Atoms::new(&conn.inner)?.reply()?;

        let screen = conn
            .inner
            .setup()
            .roots
            .get(conn.screen)
            .ok_or(Error::X11(X11Error::NoVisual))?;

        // Find a 24-bit TrueColor visual
        let visuals = screen
            .allowed_depths
            .iter()
            .flat_map(|d| d.visuals.iter().map(move |vis| (vis, d.depth)));

        let mut vid = None;
        for (vty, depth) in visuals {
            if depth == 24
                && vty.class == VisualClass::TRUE_COLOR
                && vty.red_mask == 0xff0000
                && vty.green_mask == 0xff00
                && vty.blue_mask == 0xff
            {
                vid = Some(vty.visual_id);
                break;
            }
        }

        let vid = vid.ok_or(Error::X11(X11Error::NoVisual))?;

        let attrs = CreateWindowAux::new()
            .event_mask(
                EventMask::EXPOSURE
                    | EventMask::STRUCTURE_NOTIFY
                    | EventMask::VISIBILITY_CHANGE
                    | EventMask::KEY_PRESS
                    | EventMask::KEY_RELEASE
                    | EventMask::PROPERTY_CHANGE
                    | EventMask::POINTER_MOTION
                    | EventMask::ENTER_WINDOW
                    | EventMask::LEAVE_WINDOW
                    | EventMask::BUTTON_PRESS
                    | EventMask::BUTTON_RELEASE,
            )
            .border_pixel(0)
            .colormap(0);

        let window = conn.generate_id()?;
        conn.inner
            .create_window(
                24,
                window,
                screen.root,
                0,
                0,
                width,
                height,
                0,
                WindowClass::INPUT_OUTPUT,
                vid,
                &attrs,
            )?
            .check()?;

        let gc = conn.generate_id()?;
        conn.create_gc(
            gc,
            window,
            &xproto::CreateGCAux::new().graphics_exposures(0),
        )?;

        // Opt into getting ClientMessage event on close instead of SIGTERM
        conn.change_property32(
            PropMode::REPLACE,
            window,
            atoms.WM_PROTOCOLS,
            AtomEnum::ATOM,
            &[atoms.WM_DELETE_WINDOW],
        )?;

        // Configure size hints to prevent resizing
        WmSizeHints {
            max_size: Some((width.into(), height.into())),
            min_size: Some((width.into(), height.into())),
            ..Default::default()
        }
        .set_normal_hints(&conn.inner, window)?
        .check()?;

        // Initialize keyboard handling with kbvm
        conn.setup_xkb_extension()
            .map_err(|_| Error::X11(X11Error::NoVisual))?;
        let device_id = conn
            .get_xkb_core_device_id()
            .map_err(|_| Error::X11(X11Error::NoVisual))?;
        let keymap = conn
            .get_xkb_keymap(device_id)
            .map_err(|_| Error::X11(X11Error::NoVisual))?;
        let lookup_table = keymap.to_builder().build_lookup_table();

        // Create cursors from the cursor font for the default arrow only.
        // IMPORTANT: do NOT set any window cursor during creation — letting the
        // compositor/WM choose the initial cursor allows it to follow themes.
        //
        // We'll create a glyph cursor for the I-beam so we can explicitly set it
        // when entering text fields. When leaving the text field we'll clear the
        // window cursor (set cursor to 0) so the compositor can restore the
        // themed default pointer.
        let cursor_font = conn.generate_id()?;
        conn.open_font(cursor_font, b"cursor")?;

        let cursor_default = conn.generate_id()?;
        conn.create_glyph_cursor(
            cursor_default,
            cursor_font,
            cursor_font,
            XC_LEFT_PTR,
            XC_LEFT_PTR + 1,
            0,
            0,
            0, // foreground: black
            0xffff,
            0xffff,
            0xffff, // background: white
        )?;

        let cursor_text = conn.generate_id()?;
        conn.create_glyph_cursor(
            cursor_text,
            cursor_font,
            cursor_font,
            XC_XTERM,
            XC_XTERM + 1,
            0,
            0,
            0,
            0xffff,
            0xffff,
            0xffff,
        )?;

        conn.close_font(cursor_font)?;

        let win = X11Window {
            atoms,
            conn,
            window,
            gc,
            lookup_table,
            xkb_group: 0,
            cursor_default,
            cursor_text,
            current_cursor: CursorShape::Default,
        };
        win.set_class(WM_CLASS)?;
        win.set_window_type(WindowType::Dialog)?;

        Ok(win)
    }

    fn set_class(&self, cls: &[u8]) -> Result<(), Error> {
        self.conn
            .change_property8(
                PropMode::REPLACE,
                self.window,
                AtomEnum::WM_CLASS,
                AtomEnum::STRING,
                cls,
            )?
            .check()?;
        Ok(())
    }

    fn set_window_type(&self, ty: WindowType) -> Result<(), Error> {
        let atom = match ty {
            WindowType::Dialog => self.atoms._NET_WM_WINDOW_TYPE_DIALOG,
        };
        self.conn
            .change_property32(
                PropMode::REPLACE,
                self.window,
                self.atoms._NET_WM_WINDOW_TYPE,
                AtomEnum::ATOM,
                &[atom],
            )?
            .check()?;
        Ok(())
    }

    fn cvt_event(&mut self, ev: Event) -> Option<WindowEvent> {
        Some(match ev {
            Event::ClientMessage(msg) if msg.data.as_data32()[0] == self.atoms.WM_DELETE_WINDOW => {
                WindowEvent::CloseRequested
            }
            Event::KeyPress(press) if press.event == self.window => {
                // ESC without modifiers closes the dialog
                if press.detail == KEYCODE_ESC
                    && !press
                        .state
                        .intersects(KeyButMask::CONTROL | KeyButMask::SHIFT | KeyButMask::MOD1)
                {
                    return Some(WindowEvent::CloseRequested);
                }

                let modifiers = convert_modifiers(press.state);
                let keycode = kbvm::Keycode::from_x11(press.detail.into());
                let mods = convert_to_kbvm_mods(press.state);

                let group = kbvm::GroupIndex(self.xkb_group as u32);
                let lookup = self.lookup_table.lookup(group, mods, keycode);

                let keysym = lookup
                    .clone()
                    .into_iter()
                    .next()
                    .map(|p| p.keysym().0)
                    .unwrap_or(0);

                // Get character from lookup and emit TextInput for printable characters
                let ch: Option<char> = lookup.into_iter().flat_map(|p| p.char()).next();
                if let Some(c) = ch {
                    if !c.is_control() && !modifiers.contains(Modifiers::CTRL) {
                        return Some(WindowEvent::TextInput(c));
                    }
                }

                WindowEvent::KeyPress(KeyEvent {
                    keysym,
                    modifiers,
                })
            }
            Event::KeyRelease(release) if release.event == self.window => {
                let modifiers = convert_modifiers(release.state);
                let keycode = kbvm::Keycode::from_x11(release.detail.into());
                let mods = convert_to_kbvm_mods(release.state);

                let group = kbvm::GroupIndex(self.xkb_group as u32);
                let keysym = self
                    .lookup_table
                    .lookup(group, mods, keycode)
                    .into_iter()
                    .next()
                    .map(|p| p.keysym().0)
                    .unwrap_or(0);

                WindowEvent::KeyRelease(KeyEvent {
                    keysym,
                    modifiers,
                })
            }
            Event::Expose(ex) if ex.count == 0 => WindowEvent::RedrawRequested,
            Event::EnterNotify(e) => {
                WindowEvent::CursorEnter(CursorPos {
                    x: e.event_x,
                    y: e.event_y,
                })
            }
            Event::LeaveNotify(_) => WindowEvent::CursorLeave,
            Event::MotionNotify(e) => {
                WindowEvent::CursorMove(CursorPos {
                    x: e.event_x,
                    y: e.event_y,
                })
            }
            Event::ButtonPress(e) => {
                match e.detail {
                    4 => return Some(WindowEvent::Scroll(ScrollDirection::Up)),
                    5 => return Some(WindowEvent::Scroll(ScrollDirection::Down)),
                    _ => {
                        let mods = convert_modifiers(e.state);
                        mouse_button(e.detail).map(|mb| WindowEvent::ButtonPress(mb, mods))?
                    }
                }
            }
            Event::ButtonRelease(e) => {
                match e.detail {
                    4 | 5 => return None,
                    _ => {
                        let mods = convert_modifiers(e.state);
                        mouse_button(e.detail).map(|mb| WindowEvent::ButtonRelease(mb, mods))?
                    }
                }
            }
            _ => return None,
        })
    }
}

fn convert_modifiers(state: KeyButMask) -> Modifiers {
    let mut mods = Modifiers::empty();
    if state.contains(KeyButMask::SHIFT) {
        mods |= Modifiers::SHIFT;
    }
    if state.contains(KeyButMask::CONTROL) {
        mods |= Modifiers::CTRL;
    }
    if state.contains(KeyButMask::MOD1) {
        mods |= Modifiers::ALT;
    }
    if state.contains(KeyButMask::MOD4) {
        mods |= Modifiers::SUPER;
    }
    mods
}

fn convert_to_kbvm_mods(state: KeyButMask) -> kbvm::ModifierMask {
    let mut mods = kbvm::ModifierMask::NONE;
    if state.contains(KeyButMask::SHIFT) {
        mods = mods | kbvm::ModifierMask::SHIFT;
    }
    if state.contains(KeyButMask::CONTROL) {
        mods = mods | kbvm::ModifierMask::CONTROL;
    }
    if state.contains(KeyButMask::MOD1) {
        mods = mods | kbvm::ModifierMask::MOD1;
    }
    if state.contains(KeyButMask::MOD4) {
        mods = mods | kbvm::ModifierMask::MOD4;
    }
    mods
}

impl Window for X11Window {
    fn set_title(&mut self, title: &str) -> Result<(), Error> {
        let title = if title.ends_with('\0') {
            title.to_string()
        } else {
            format!("{title}\0")
        };

        self.conn
            .change_property8(
                PropMode::REPLACE,
                self.window,
                AtomEnum::WM_NAME,
                AtomEnum::STRING,
                title.as_bytes(),
            )?
            .check()?;
        self.conn
            .change_property8(
                PropMode::REPLACE,
                self.window,
                self.atoms._NET_WM_NAME,
                self.atoms.UTF8_STRING,
                title.as_bytes(),
            )?
            .check()?;

        Ok(())
    }

    fn set_contents(&mut self, canvas: &Canvas) -> Result<(), Error> {
        let data = canvas.as_argb();
        self.conn
            .put_image(
                ImageFormat::Z_PIXMAP,
                self.window,
                self.gc,
                canvas.width().try_into().unwrap(),
                canvas.height().try_into().unwrap(),
                0,
                0,
                0,
                24,
                &data,
            )?
            .check()?;
        Ok(())
    }

    fn show(&mut self) -> Result<(), Error> {
        self.conn.map_window(self.window)?;
        self.conn.configure_window(
            self.window,
            &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE),
        )?;
        self.conn.flush()?;
        Ok(())
    }

    fn wait_for_event(&mut self) -> Result<WindowEvent, Error> {
        loop {
            let ev = self.conn.wait_for_event()?;
            if let Some(ev) = self.cvt_event(ev) {
                return Ok(ev);
            }
        }
    }

    fn poll_for_event(&mut self) -> Result<Option<WindowEvent>, Error> {
        loop {
            match self.conn.poll_for_event()? {
                Some(ev) => {
                    if let Some(ev) = self.cvt_event(ev) {
                        return Ok(Some(ev));
                    }
                }
                None => return Ok(None),
            }
        }
    }

    fn start_drag(&mut self) -> Result<(), Error> {
        let pointer = self.conn.query_pointer(self.window)?.reply()?;

        let window_pos = self
            .conn
            .translate_coordinates(self.window, pointer.root, 0, 0)?
            .reply()?;

        let x = (window_pos.dst_x + pointer.win_x) as u32;
        let y = (window_pos.dst_y + pointer.win_y) as u32;

        self.conn
            .send_event(
                false,
                pointer.root,
                EventMask::SUBSTRUCTURE_NOTIFY | EventMask::SUBSTRUCTURE_REDIRECT,
                ClientMessageEvent::new(
                    32,
                    self.window,
                    self.atoms._NET_WM_MOVERESIZE,
                    [x, y, MOVERESIZE_MOVE, 1, 1],
                ),
            )?
            .check()?;

        Ok(())
    }

    fn scale_factor(&self) -> f32 {
        super::DEFAULT_SCALE
    }

    fn set_cursor(&mut self, shape: CursorShape) -> Result<(), Error> {
        if self.current_cursor == shape {
            return Ok(());
        }

        // When entering a text field set an I-beam glyph cursor.
        // When leaving (switching back to Default) clear the window cursor
        // (cursor = 0) so the compositor/WM can restore the themed default.
        let cursor_id: u32 = match shape {
            CursorShape::Text => self.cursor_text,
            CursorShape::Default => 0, // clear the cursor attribute
        };

        self.conn.change_window_attributes(
            self.window,
            &xproto::ChangeWindowAttributesAux::new().cursor(cursor_id),
        )?;
        self.conn.flush()?;

        self.current_cursor = shape;
        Ok(())
    }
}

fn mouse_button(detail: u8) -> Option<MouseButton> {
    Some(match detail {
        1 => MouseButton::Left,
        2 => MouseButton::Middle,
        3 => MouseButton::Right,
        _ => return None,
    })
}
