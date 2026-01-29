//! Wayland backend implementation.

mod shm;

use std::{
    collections::VecDeque,
    os::fd::{FromRawFd, IntoRawFd},
};

use kbvm::lookup::LookupTable;
use wayland_client::{
    Connection as WaylandConnection, Dispatch, EventQueue, QueueHandle, WEnum,
    protocol::{
        wl_buffer::{self, WlBuffer},
        wl_callback::{self, WlCallback},
        wl_compositor::WlCompositor,
        wl_keyboard::{self, WlKeyboard},
        wl_output::{self, WlOutput},
        wl_pointer::{self, WlPointer},
        wl_registry::{self, WlRegistry},
        wl_seat::{self, WlSeat},
        wl_shm::WlShm,
        wl_shm_pool::WlShmPool,
        wl_surface::WlSurface,
    },
};
use wayland_protocols::xdg::shell::client::{
    xdg_surface::{self, XdgSurface},
    xdg_toplevel::{self, XdgToplevel},
    xdg_wm_base::{self, XdgWmBase},
};

use self::shm::ShmPool;
use super::{
    CursorPos, CursorShape, DEFAULT_SCALE, DisplayConnection, KeyEvent, Modifiers, MouseButton,
    ScrollDirection, Window, WindowEvent,
};
use crate::{
    error::{Error, WaylandError},
    render::Canvas,
    timing,
};

/// Wayland connection wrapper.
pub(crate) struct Connection {
    conn: WaylandConnection,
}

impl DisplayConnection for Connection {
    type Window = WaylandWindow;

    fn connect() -> Result<Self, Error> {
        let conn = WaylandConnection::connect_to_env()?;
        Ok(Self {
            conn,
        })
    }

    fn create_window(&self, width: u16, height: u16) -> Result<Self::Window, Error> {
        WaylandWindow::create(&self.conn, width, height)
    }
}

/// State for Wayland protocol handling.
pub(super) struct WaylandState {
    // Globals
    compositor: Option<WlCompositor>,
    shm: Option<WlShm>,
    xdg_wm_base: Option<XdgWmBase>,
    seat: Option<WlSeat>,
    output: Option<WlOutput>,

    // Input devices
    pointer: Option<WlPointer>,
    keyboard: Option<WlKeyboard>,

    // Window state
    surface: Option<WlSurface>,
    xdg_surface: Option<XdgSurface>,
    xdg_toplevel: Option<XdgToplevel>,

    // Configuration state
    configured: bool,
    closed: bool,

    // Scale factor from output (integer scale from wl_output)
    output_scale: i32,
    // Effective scale factor used for rendering (set when window is created)
    effective_scale: i32,

    // Input state
    last_serial: u32,
    modifier_mask: kbvm::ModifierMask,
    keyboard_group: u32,

    // Keyboard handling
    lookup_table: Option<LookupTable>,

    // Events
    pending_events: VecDeque<WindowEvent>,
}

impl WaylandState {
    fn new() -> Self {
        Self {
            compositor: None,
            shm: None,
            xdg_wm_base: None,
            seat: None,
            output: None,
            pointer: None,
            keyboard: None,
            surface: None,
            xdg_surface: None,
            xdg_toplevel: None,
            configured: false,
            closed: false,
            output_scale: 1,
            effective_scale: 1,
            last_serial: 0,
            modifier_mask: kbvm::ModifierMask::NONE,
            keyboard_group: 0,
            lookup_table: None,
            pending_events: VecDeque::new(),
        }
    }

    /// Returns the effective scale factor to use for rendering.
    /// Uses compositor scale if > 1, otherwise defaults to DEFAULT_SCALE.
    fn scale_factor(&self) -> f32 {
        if self.output_scale > 1 {
            self.output_scale as f32
        } else {
            DEFAULT_SCALE
        }
    }
}

/// Wayland window implementation.
pub(crate) struct WaylandWindow {
    conn: WaylandConnection,
    event_queue: EventQueue<WaylandState>,
    state: WaylandState,
    shm_pool: ShmPool,
    buffer: WlBuffer,
    /// Logical width (what the user requested)
    logical_width: i32,
    /// Logical height (what the user requested)
    logical_height: i32,
    /// Physical width (logical * scale)
    physical_width: i32,
    /// Physical height (logical * scale)
    physical_height: i32,
    /// Scale factor for this window
    scale: i32,
    /// Cursor theme
    cursor_theme: wayland_cursor::CursorTheme,
    /// Cursor surface for rendering cursor
    cursor_surface: WlSurface,
    /// Current cursor shape
    current_cursor: CursorShape,
}

impl WaylandWindow {
    fn create(conn: &WaylandConnection, width: u16, height: u16) -> Result<Self, Error> {
        let mut event_queue = conn.new_event_queue();
        let qh = event_queue.handle();

        let display = conn.display();
        let mut state = WaylandState::new();

        // Get the registry and bind globals
        let _registry = display.get_registry(&qh, ());

        // Roundtrip to get globals
        event_queue.roundtrip(&mut state)?;

        // Verify required globals
        let compositor = state
            .compositor
            .clone()
            .ok_or(Error::Wayland(WaylandError::MissingGlobal("wl_compositor")))?;
        let shm = state
            .shm
            .clone()
            .ok_or(Error::Wayland(WaylandError::MissingGlobal("wl_shm")))?;
        let xdg_wm_base = state
            .xdg_wm_base
            .clone()
            .ok_or(Error::Wayland(WaylandError::MissingGlobal("xdg_wm_base")))?;

        // Create surface
        let surface = compositor.create_surface(&qh, ());
        state.surface = Some(surface.clone());

        // Create xdg_surface
        let xdg_surface = xdg_wm_base.get_xdg_surface(&surface, &qh, ());
        state.xdg_surface = Some(xdg_surface.clone());

        // Create xdg_toplevel
        let xdg_toplevel = xdg_surface.get_toplevel(&qh, ());
        state.xdg_toplevel = Some(xdg_toplevel.clone());

        // Set up window properties
        xdg_toplevel.set_app_id("zenity".to_string());
        xdg_toplevel.set_min_size(width as i32, height as i32);
        xdg_toplevel.set_max_size(width as i32, height as i32);

        // Commit to get configure event
        surface.commit();

        // Wait for configure
        while !state.configured {
            event_queue.blocking_dispatch(&mut state)?;
        }

        // Do another roundtrip to ensure we have output scale
        event_queue.roundtrip(&mut state)?;

        // Get the scale factor - use compositor scale if > 1, otherwise use our default
        let scale = state.scale_factor().ceil() as i32;
        // Store the effective scale so pointer events can use the same value
        state.effective_scale = scale;

        // Calculate physical dimensions (what we actually render)
        let logical_width = width as i32;
        let logical_height = height as i32;
        let physical_width = logical_width * scale;
        let physical_height = logical_height * scale;

        // Create shared memory pool and buffer at PHYSICAL size
        let stride = physical_width * 4; // 4 bytes per pixel (ARGB8888)
        let size = (stride * physical_height) as usize;

        let shm_pool = ShmPool::new(&shm, size, &qh)?;
        let buffer = shm_pool.create_buffer(physical_width, physical_height, stride, &qh);

        // Set buffer scale so compositor knows we're rendering at higher resolution
        surface.set_buffer_scale(scale);

        // Get input devices from seat
        if let Some(seat) = &state.seat.clone() {
            state.pointer = Some(seat.get_pointer(&qh, ()));
            state.keyboard = Some(seat.get_keyboard(&qh, ()));
        }

        // Create cursor theme and surface
        let cursor_theme = wayland_cursor::CursorTheme::load(conn, shm.clone(), 24)
            .map_err(|_| Error::Wayland(WaylandError::MissingGlobal("cursor theme")))?;
        let cursor_surface = compositor.create_surface(&qh, ());

        Ok(Self {
            conn: conn.clone(),
            event_queue,
            state,
            shm_pool,
            buffer,
            logical_width,
            logical_height,
            physical_width,
            physical_height,
            scale,
            cursor_theme,
            cursor_surface,
            current_cursor: CursorShape::Default,
        })
    }

    /// Updates the cursor on the pointer
    fn update_cursor(&mut self) {
        let cursor_name = match self.current_cursor {
            CursorShape::Default => "default",
            CursorShape::Text => "text",
        };

        if let Some(cursor) = self.cursor_theme.get_cursor(cursor_name) {
            let image = &cursor[0];
            let (width, height) = image.dimensions();
            let (xhot, yhot) = image.hotspot();

            self.cursor_surface.attach(Some(&image), 0, 0);
            self.cursor_surface
                .damage_buffer(0, 0, width as i32, height as i32);
            self.cursor_surface.commit();

            if let Some(pointer) = &self.state.pointer {
                pointer.set_cursor(
                    self.state.last_serial,
                    Some(&self.cursor_surface),
                    xhot as i32,
                    yhot as i32,
                );
            }
        }
    }
}

impl Window for WaylandWindow {
    fn set_title(&mut self, title: &str) -> Result<(), Error> {
        if let Some(toplevel) = &self.state.xdg_toplevel {
            toplevel.set_title(title.trim_end_matches('\0').to_string());
        }
        Ok(())
    }

    fn set_contents(&mut self, canvas: &Canvas) -> Result<(), Error> {
        // Copy pixel data from Canvas to shared memory buffer
        let _timer = timing::Timer::new("canvas_as_argb");
        let src = canvas.as_argb();
        let dst = self.shm_pool.data_mut();
        dst[..src.len()].copy_from_slice(&src);

        // Attach buffer and damage the surface (use physical dimensions)
        if let Some(surface) = &self.state.surface {
            surface.attach(Some(&self.buffer), 0, 0);
            surface.damage_buffer(0, 0, self.physical_width, self.physical_height);
            let _timer = timing::Timer::new("wayland_commit");
            surface.commit();
        }

        self.conn.flush()?;
        Ok(())
    }

    fn show(&mut self) -> Result<(), Error> {
        self.conn.flush()?;
        Ok(())
    }

    fn wait_for_event(&mut self) -> Result<WindowEvent, Error> {
        loop {
            if let Some(event) = self.state.pending_events.pop_front() {
                return Ok(event);
            }

            if self.state.closed {
                return Ok(WindowEvent::CloseRequested);
            }

            self.conn.flush()?;
            self.event_queue.blocking_dispatch(&mut self.state)?;
        }
    }

    fn poll_for_event(&mut self) -> Result<Option<WindowEvent>, Error> {
        if let Some(event) = self.state.pending_events.pop_front() {
            return Ok(Some(event));
        }

        if self.state.closed {
            return Ok(Some(WindowEvent::CloseRequested));
        }

        self.conn.flush()?;

        // Try to prepare for reading new events
        if let Some(guard) = self.event_queue.prepare_read() {
            // Try to read events - this may fail if no data is available
            // The guard is consumed by read() call, so we don't need to cancel it
            let _ = guard.read();
        }

        self.event_queue.dispatch_pending(&mut self.state)?;

        Ok(self.state.pending_events.pop_front())
    }

    fn start_drag(&mut self) -> Result<(), Error> {
        if let (Some(toplevel), Some(seat)) = (&self.state.xdg_toplevel, &self.state.seat) {
            toplevel._move(seat, self.state.last_serial);
        }
        Ok(())
    }

    fn scale_factor(&self) -> f32 {
        self.scale as f32
    }

    fn set_cursor(&mut self, shape: CursorShape) -> Result<(), Error> {
        if self.current_cursor == shape {
            return Ok(());
        }
        self.current_cursor = shape;
        self.update_cursor();
        self.conn.flush()?;
        Ok(())
    }
}

// Registry handler - binds globals
impl Dispatch<WlRegistry, ()> for WaylandState {
    fn event(
        state: &mut Self,
        registry: &WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _conn: &WaylandConnection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match interface.as_str() {
                "wl_compositor" => {
                    state.compositor = Some(registry.bind(name, version.min(6), qh, ()));
                }
                "wl_shm" => {
                    state.shm = Some(registry.bind(name, version.min(1), qh, ()));
                }
                "xdg_wm_base" => {
                    state.xdg_wm_base = Some(registry.bind(name, version.min(6), qh, ()));
                }
                "wl_seat" => {
                    state.seat = Some(registry.bind(name, version.min(9), qh, ()));
                }
                "wl_output" => {
                    // Bind wl_output version 2+ to get scale events
                    if version >= 2 {
                        state.output = Some(registry.bind(name, version.min(4), qh, ()));
                    }
                }
                _ => {}
            }
        }
    }
}

// Empty handlers for globals we don't need events from
impl Dispatch<WlCompositor, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &WlCompositor,
        _: <WlCompositor as wayland_client::Proxy>::Event,
        _: &(),
        _: &WaylandConnection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlShm, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &WlShm,
        _: <WlShm as wayland_client::Proxy>::Event,
        _: &(),
        _: &WaylandConnection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlOutput, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _: &WlOutput,
        event: wl_output::Event,
        _: &(),
        _: &WaylandConnection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_output::Event::Scale {
            factor,
        } = event
        {
            state.output_scale = factor;
        }
    }
}

impl Dispatch<WlShmPool, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &WlShmPool,
        _: <WlShmPool as wayland_client::Proxy>::Event,
        _: &(),
        _: &WaylandConnection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlBuffer, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &WlBuffer,
        _event: wl_buffer::Event,
        _: &(),
        _: &WaylandConnection,
        _: &QueueHandle<Self>,
    ) {
        // Buffer released, can reuse - we don't need to do anything
    }
}

impl Dispatch<WlSurface, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &WlSurface,
        _: <WlSurface as wayland_client::Proxy>::Event,
        _: &(),
        _: &WaylandConnection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlCallback, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _: &WlCallback,
        event: wl_callback::Event,
        _: &(),
        _: &WaylandConnection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_callback::Event::Done {
            ..
        } = event
        {
            state.pending_events.push_back(WindowEvent::RedrawRequested);
        }
    }
}

impl Dispatch<XdgWmBase, ()> for WaylandState {
    fn event(
        _: &mut Self,
        wm_base: &XdgWmBase,
        event: xdg_wm_base::Event,
        _: &(),
        _: &WaylandConnection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping {
            serial,
        } = event
        {
            wm_base.pong(serial);
        }
    }
}

impl Dispatch<XdgSurface, ()> for WaylandState {
    fn event(
        state: &mut Self,
        xdg_surface: &XdgSurface,
        event: xdg_surface::Event,
        _: &(),
        _: &WaylandConnection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure {
            serial,
        } = event
        {
            xdg_surface.ack_configure(serial);
            state.configured = true;
            state.pending_events.push_back(WindowEvent::RedrawRequested);
        }
    }
}

impl Dispatch<XdgToplevel, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _: &XdgToplevel,
        event: xdg_toplevel::Event,
        _: &(),
        _: &WaylandConnection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_toplevel::Event::Close = event {
            state.closed = true;
            state.pending_events.push_back(WindowEvent::CloseRequested);
        }
    }
}

impl Dispatch<WlSeat, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &WlSeat,
        _: wl_seat::Event,
        _: &(),
        _: &WaylandConnection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlPointer, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _: &WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &WaylandConnection,
        _: &QueueHandle<Self>,
    ) {
        // Use effective_scale for converting logical coordinates to physical
        // This matches the scale used when creating the window buffer
        let scale = state.effective_scale;

        match event {
            wl_pointer::Event::Enter {
                serial,
                surface_x,
                surface_y,
                ..
            } => {
                state.last_serial = serial;
                // Scale coordinates from logical to physical
                state
                    .pending_events
                    .push_back(WindowEvent::CursorEnter(CursorPos {
                        x: (surface_x * scale as f64) as i16,
                        y: (surface_y * scale as f64) as i16,
                    }));
            }
            wl_pointer::Event::Leave {
                serial, ..
            } => {
                state.last_serial = serial;
                state.pending_events.push_back(WindowEvent::CursorLeave);
            }
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => {
                // Scale coordinates from logical to physical
                state
                    .pending_events
                    .push_back(WindowEvent::CursorMove(CursorPos {
                        x: (surface_x * scale as f64) as i16,
                        y: (surface_y * scale as f64) as i16,
                    }));
            }
            wl_pointer::Event::Button {
                serial,
                button,
                state: btn_state,
                ..
            } => {
                state.last_serial = serial;
                let mb = match button {
                    0x110 => MouseButton::Left,
                    0x111 => MouseButton::Right,
                    0x112 => MouseButton::Middle,
                    _ => return,
                };
                let modifiers = convert_wayland_modifiers(state.modifier_mask);
                let event = match btn_state {
                    WEnum::Value(wl_pointer::ButtonState::Pressed) => {
                        WindowEvent::ButtonPress(mb, modifiers)
                    }
                    WEnum::Value(wl_pointer::ButtonState::Released) => {
                        WindowEvent::ButtonRelease(mb, modifiers)
                    }
                    _ => return,
                };
                state.pending_events.push_back(event);
            }
            wl_pointer::Event::Axis {
                axis,
                value,
                ..
            } => {
                if let WEnum::Value(axis) = axis {
                    if axis == wl_pointer::Axis::VerticalScroll {
                        let direction = if value > 0.0.into() {
                            ScrollDirection::Down
                        } else {
                            ScrollDirection::Up
                        };
                        state
                            .pending_events
                            .push_back(WindowEvent::Scroll(direction));
                    }
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<WlKeyboard, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _: &WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &WaylandConnection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::Keymap {
                format,
                fd,
                size,
            } => {
                if format == WEnum::Value(wl_keyboard::KeymapFormat::XkbV1) {
                    if let Ok(mmap) = unsafe {
                        let file = std::fs::File::from_raw_fd(fd.into_raw_fd());
                        memmap2::Mmap::map(&file)
                    } {
                        let keymap_bytes = &mmap[..size as usize];
                        let context = kbvm::xkb::Context::default();
                        let mut diagnostics: Vec<kbvm::xkb::diagnostic::Diagnostic> = Vec::new();
                        if let Ok(keymap) =
                            context.keymap_from_bytes(&mut diagnostics, None, keymap_bytes)
                        {
                            state.lookup_table = Some(keymap.to_builder().build_lookup_table());
                        }
                    }
                }
            }
            wl_keyboard::Event::Key {
                serial,
                key,
                state: key_state,
                ..
            } => {
                state.last_serial = serial;

                let keycode = kbvm::Keycode::from_evdev(key);
                let modifiers = convert_wayland_modifiers(state.modifier_mask);

                // KEY_ESC = 1 in evdev codes
                if key == 1
                    && key_state == WEnum::Value(wl_keyboard::KeyState::Pressed)
                    && state.modifier_mask == kbvm::ModifierMask::NONE
                {
                    state.pending_events.push_back(WindowEvent::CloseRequested);
                    return;
                }

                if let Some(ref lookup_table) = state.lookup_table {
                    let group = kbvm::GroupIndex(state.keyboard_group);
                    let lookup = lookup_table.lookup(group, state.modifier_mask, keycode);

                    let keysym = lookup
                        .clone()
                        .into_iter()
                        .next()
                        .map(|p| p.keysym().0)
                        .unwrap_or(0);

                    match key_state {
                        WEnum::Value(wl_keyboard::KeyState::Pressed) => {
                            // Emit TextInput for printable characters on key press
                            let ch: Option<char> = lookup.into_iter().flat_map(|p| p.char()).next();

                            if let Some(c) = ch {
                                if !c.is_control() && !modifiers.contains(Modifiers::CTRL) {
                                    state.pending_events.push_back(WindowEvent::TextInput(c));
                                    return;
                                }
                            }

                            state
                                .pending_events
                                .push_back(WindowEvent::KeyPress(KeyEvent {
                                    keysym,
                                    modifiers,
                                }));
                        }
                        WEnum::Value(wl_keyboard::KeyState::Released) => {
                            state
                                .pending_events
                                .push_back(WindowEvent::KeyRelease(KeyEvent {
                                    keysym,
                                    modifiers,
                                }));
                        }
                        _ => {}
                    }
                }
            }
            wl_keyboard::Event::Modifiers {
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
                ..
            } => {
                let combined = mods_depressed | mods_latched | mods_locked;
                state.modifier_mask = kbvm::ModifierMask(combined);
                state.keyboard_group = group;
            }
            wl_keyboard::Event::Enter {
                serial, ..
            } => {
                state.last_serial = serial;
            }
            wl_keyboard::Event::Leave {
                serial, ..
            } => {
                state.last_serial = serial;
            }
            _ => {}
        }
    }
}

fn convert_wayland_modifiers(mask: kbvm::ModifierMask) -> Modifiers {
    let mut mods = Modifiers::empty();
    if mask.contains(kbvm::ModifierMask::SHIFT) {
        mods |= Modifiers::SHIFT;
    }
    if mask.contains(kbvm::ModifierMask::CONTROL) {
        mods |= Modifiers::CTRL;
    }
    if mask.contains(kbvm::ModifierMask::MOD1) {
        mods |= Modifiers::ALT;
    }
    if mask.contains(kbvm::ModifierMask::MOD4) {
        mods |= Modifiers::SUPER;
    }
    mods
}
