//! Implements the `wayland` module.
//!
//! This module documents both public API items and internal implementation details.

use std::{
    cell::{ Cell, RefCell },
    collections::HashSet,
    ffi::c_void,
    sync::{ atomic::{ AtomicBool, Ordering }, Arc, Mutex },
};

use wayland_client::{
    delegate_noop,
    protocol::{
        wl_buffer,
        wl_callback,
        wl_compositor,
        wl_keyboard,
        wl_pointer,
        wl_registry,
        wl_seat,
        wl_shm,
        wl_shm_pool,
        wl_surface,
    },
    Connection,
    Dispatch,
    EventQueue,
    Proxy,
    QueueHandle,
    WEnum,
};
use wayland_protocols::xdg::shell::client::{ xdg_surface, xdg_toplevel, xdg_wm_base };
use wayland_protocols::xdg::decoration::zv1::client::{
    zxdg_decoration_manager_v1,
    zxdg_toplevel_decoration_v1,
};

/// Contains the drop implementation.
///
/// Visibility: **private**.
mod drop;
/// Always-available client-side fallback if the compositor has no server decorations.
mod decorations;
/// Contains the events implementation.
///
/// Visibility: **private**.
mod events;
/// Contains the renderer implementation.
///
/// Visibility: **private**.
mod renderer;
/// Contains the show implementation.
///
/// Visibility: **private**.
mod show;

use crate::{buffer::Framebuffer, window::{DirtyRect,damage}};
use crate::events::{ Key, WindowEvent, EVENT_QUEUE, KEYS_DOWN };
use renderer::WaylandRenderer;

/// State owned by the Wayland event queue.
///
/// The low-level Wayland API dispatches protocol events through this value. It
/// intentionally contains only protocol objects and small pieces of window state;
/// the large shared-memory frame buffers live in [`WaylandRenderer`].
pub(super) struct WaylandState {
    /// Stores compositor.
    ///
    /// Visibility: **private**.
    compositor: Option<wl_compositor::WlCompositor>,
    /// Stores shm.
    ///
    /// Visibility: **private**.
    shm: Option<wl_shm::WlShm>,
    /// Stores wm base.
    ///
    /// Visibility: **private**.
    wm_base: Option<xdg_wm_base::XdgWmBase>,
    decoration_manager: Option<zxdg_decoration_manager_v1::ZxdgDecorationManagerV1>,
    server_decorations: bool,
    decorations_enabled: bool,
    toplevel: Option<xdg_toplevel::XdgToplevel>,
    pointer_position: (i32, i32),
    chrome_press_active: bool,
    /// Stores seat.
    ///
    /// Visibility: **private**.
    seat: Option<wl_seat::WlSeat>,
    /// Stores keyboard.
    ///
    /// Visibility: **private**.
    keyboard: Option<wl_keyboard::WlKeyboard>,
    /// Stores pointer.
    ///
    /// Visibility: **private**.
    pointer: Option<wl_pointer::WlPointer>,
    pointer_serial: Option<u32>,
    pointer_focused: bool,
    cursor_visible: bool,
    cursor_surface: Option<wl_surface::WlSurface>,

    /// Stores configured.
    ///
    /// Visibility: **private**.
    configured: bool,
    /// Stores client size.
    ///
    /// Visibility: **private**.
    client_size: (u32, u32),
    /// Stores maximized.
    ///
    /// Visibility: **private**.
    maximized: bool,
    maximizable: bool,
    minimizable: bool,
}

impl WaylandState {
    /// Creates a new instance.
    ///
    /// Visibility: **private**.
    #[doc = "Creates a new value. Visibility: private/internal."]
    fn new(size: (u32, u32)) -> Self {
        Self {
            compositor: None,
            shm: None,
            wm_base: None,
            decoration_manager: None,
            server_decorations: false,
            decorations_enabled: true,
            toplevel: None,
            pointer_position: (0, 0),
            chrome_press_active: false,
            seat: None,
            keyboard: None,
            pointer: None,
            pointer_serial: None,
            pointer_focused: false,
            cursor_visible: true,
            cursor_surface: None,
            configured: false,
            client_size: size,
            maximized: false,
            maximizable: true,
            minimizable: true,
        }
    }
}

/// Pushes event.
///
/// Visibility: **private**.
#[doc = "Pushes event. Visibility: private/internal."]
fn push_event(event: WindowEvent) {
    if let Some(queue) = EVENT_QUEUE.get() {
        queue.lock().unwrap().push(event);
    }
}

/// Sets key down.
///
/// Visibility: **private**.
#[doc = "Sets key down. Visibility: private/internal."]
fn set_key_down(key: Key, down: bool) {
    if let Some(keys) = KEYS_DOWN.get() {
        let mut keys = keys.lock().unwrap();
        if down {
            keys.insert(key);
        } else {
            keys.remove(&key);
        }
    }
}

/// Native Wayland window backend.
///
/// This backend talks to the user's existing Wayland compositor through the
/// session socket. It never opens `/dev/input`, DRM devices, or other privileged
/// device nodes, so normal window creation and input handling do not require
/// root privileges or `sudo`.
pub struct PlatformWindow {
    /// Stores connection.
    ///
    /// Visibility: **parent-module-visible**.
    pub(super) connection: Connection,
    /// Stores event queue.
    ///
    /// Visibility: **parent-module-visible**.
    pub(super) event_queue: RefCell<EventQueue<WaylandState>>,
    /// Stores state.
    ///
    /// Visibility: **parent-module-visible**.
    pub(super) state: RefCell<WaylandState>,

    /// Stores surface.
    ///
    /// Visibility: **parent-module-visible**.
    pub(super) surface: wl_surface::WlSurface,
    /// Stores xdg surface.
    ///
    /// Visibility: **parent-module-visible**.
    pub(super) xdg_surface: xdg_surface::XdgSurface,
    /// Stores toplevel.
    ///
    /// Visibility: **parent-module-visible**.
    pub(super) toplevel: xdg_toplevel::XdgToplevel,

    /// Size of the most recently presented software framebuffer. Applications
    /// may redraw at the compositor-configured size to avoid post-raster scaling.
    pub(super) size: Cell<(u32, u32)>,

    /// Stores renderer.
    ///
    /// Visibility: **parent-module-visible**.
    pub(super) renderer: WaylandRenderer,
    /// Stores hidden.
    ///
    /// Visibility: **parent-module-visible**.
    pub(super) hidden: Cell<bool>,
    /// Hold the cursor pixel mapping while wl_shm can read it.
    _cursor_memory: super::shared_memory::MappedMemory,
    _cursor_buffer: wl_buffer::WlBuffer,

    /// Stores resizable.
    ///
    /// Visibility: **parent-module-visible**.
    pub(super) resizable: Cell<bool>,
    pub(super) maximizable: Cell<bool>,
    pub(super) minimizable: Cell<bool>,
    /// The decoration object must be destroyed before the xdg_toplevel.
    decoration: Option<zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1>,
    decorations_enabled: Cell<bool>,
    title: RefCell<String>,
    decoration_frame: Framebuffer,
    decoration_dirty: Cell<bool>,
    last_client_decorations: bool,
    last_maximized: bool,
}

impl PlatformWindow {
    /// Creates a Wayland window, panicking if Wayland cannot be initialized.
    ///
    /// Linux runtime backend selection should normally call [`Self::try_new`] so
    /// an unavailable Wayland session can fall back to X11 instead of aborting.
    #[doc = "Creates a new value. Visibility: public."]
    pub fn new(title: &str, size: (u32, u32)) -> Self {
        Self::try_new(title, size).unwrap_or_else(|error|
            panic!("Failed to create Wayland window: {error}")
        )
    }

    /// Attempts to create a Wayland window without panicking on setup failure.
    ///
    /// Connection, registry discovery, required-global validation, and the initial
    /// XDG configure roundtrip are all part of this attempt. This allows the Linux
    /// selector to cleanly fall back to X11 when Wayland is not usable.
    #[doc = "Implements the try new operation. Visibility: public."]
    pub fn try_new(title: &str, size: (u32, u32)) -> Result<Self, String> {
        if size.0 == 0 || size.1 == 0 {
            return Err("Wayland windows must have a non-zero size".to_owned());
        }

        KEYS_DOWN.get_or_init(|| Mutex::new(HashSet::with_capacity(16)));
        EVENT_QUEUE.get_or_init(|| Mutex::new(Vec::with_capacity(32)));

        let connection = Connection::connect_to_env().map_err(|error|
            format!("failed to connect to the Wayland compositor: {error}")
        )?;
        let mut event_queue = connection.new_event_queue::<WaylandState>();
        let qh = event_queue.handle();

        let display = connection.display();
        display.get_registry(&qh, ());

        // A compositor without xdg-decoration expects us to render a titlebar.
        let initial_surface_size = (size.0, size.1.saturating_add(decorations::HEIGHT));
        let mut state = WaylandState::new(initial_surface_size);

        // First roundtrip discovers globals. The second receives immediate events
        // generated by objects bound while the registry events were dispatched,
        // notably wl_seat capabilities.
        event_queue
            .roundtrip(&mut state)
            .map_err(|error| format!("registry roundtrip failed: {error}"))?;
        event_queue
            .roundtrip(&mut state)
            .map_err(|error| format!("global initialization roundtrip failed: {error}"))?;

        let compositor = state.compositor
            .as_ref()
            .ok_or_else(|| "compositor does not expose wl_compositor".to_owned())?
            .clone();
        let shm = state.shm
            .as_ref()
            .ok_or_else(|| "compositor does not expose wl_shm".to_owned())?
            .clone();
        let wm_base = state.wm_base
            .as_ref()
            .ok_or_else(|| "compositor does not expose xdg_wm_base".to_owned())?
            .clone();

        // The Wayland protocol has no "restore system cursor" request. Supply
        // our own small cursor surface to make show/hide round trips reliable.
        use std::os::fd::AsFd;
        const CURSOR_W: i32 = 16;
        const CURSOR_H: i32 = 24;
        let mut cursor_memory = super::shared_memory::MappedMemory::new(
            (CURSOR_W * CURSOR_H * 4) as usize
        ).map_err(|error| format!("failed to allocate cursor pixels: {error}"))?;
        let pixels = cursor_memory.bytes_mut();
        const ARROW: [&str; 19] = [
            "#...............", "##..............", "#o#.............",
            "#oo#............", "#ooo#...........", "#oooo#..........",
            "#ooooo#.........", "#oooooo#........", "#ooooooo#.......",
            "#oooooooo#......", "#ooooooooo#.....", "#oooooooooo#....",
            "#ooooo######....", "#oo#oo#.........", "#o#.#oo#........",
            "##..#oo#........", "#....#oo#.......", ".....#oo#.......",
            "......##........",
        ];
        for (y, row) in ARROW.iter().enumerate() {
            for (x, pixel) in row.bytes().take(CURSOR_W as usize).enumerate() {
                let argb: u32 = match pixel {
                    b'#' => 0xff151515,
                    b'o' => 0xffffffff,
                    _ => 0,
                };
                let index = (y * CURSOR_W as usize + x) * 4;
                pixels[index..index + 4].copy_from_slice(&argb.to_ne_bytes());
            }
        }
        let pool = shm.create_pool(cursor_memory.file.as_fd(),
            (CURSOR_W * CURSOR_H * 4) as i32, &qh, ());
        let cursor_buffer = pool.create_buffer(0, CURSOR_W, CURSOR_H,
            CURSOR_W * 4, wl_shm::Format::Argb8888, &qh,
            Arc::new(AtomicBool::new(false)));
        pool.destroy();
        let cursor_surface = compositor.create_surface(&qh, ());
        cursor_surface.attach(Some(&cursor_buffer), 0, 0);
        cursor_surface.damage_buffer(0, 0, CURSOR_W, CURSOR_H);
        cursor_surface.commit();
        state.cursor_surface = Some(cursor_surface);

        let surface = compositor.create_surface(&qh, ());
        let xdg_surface = wm_base.get_xdg_surface(&surface, &qh, ());
        let toplevel = xdg_surface.get_toplevel(&qh, ());

        toplevel.set_title(title.to_owned());
        toplevel.set_app_id("mata-win".to_owned());
        state.toplevel = Some(toplevel.clone());
        // Request compositor-owned chrome whenever available. If it refuses,
        // or does not advertise the extension, draw our own controls.
        let decoration = state.decoration_manager.as_ref().map(|manager| {
            let d = manager.get_toplevel_decoration(&toplevel, &qh, ());
            d.set_mode(zxdg_toplevel_decoration_v1::Mode::ServerSide);
            d
        });

        // XDG shell windows must perform an empty initial commit. The compositor
        // then replies with the first configure event before pixels are attached.
        surface.commit();
        event_queue
            .roundtrip(&mut state)
            .map_err(|error| format!("initial configure roundtrip failed: {error}"))?;

        if !state.configured {
            return Err("compositor did not configure the XDG toplevel".to_owned());
        }
        let target_size = state.client_size;
        let renderer = WaylandRenderer::new(shm, qh, size, target_size);

        Ok(Self {
            connection,
            event_queue: RefCell::new(event_queue),
            state: RefCell::new(state),
            surface,
            xdg_surface,
            toplevel,
            size: Cell::new(size),
            renderer,
            hidden: Cell::new(false),
            _cursor_memory: cursor_memory,
            _cursor_buffer: cursor_buffer,
            resizable: Cell::new(true),
            maximizable: Cell::new(true),
            minimizable: Cell::new(true),
            decoration,
            decorations_enabled: Cell::new(true),
            title: RefCell::new(title.to_owned()),
            decoration_frame: Framebuffer::new(0, 0),
            decoration_dirty: Cell::new(true),
            last_client_decorations: false,
            last_maximized: false,
        })
    }

    /// Performs the present operation.
    ///
    /// Visibility: **public**.
    #[doc = "Implements the present operation. Visibility: public."]
    pub fn present(&mut self, fb: &Framebuffer) {
        self.present_dirty(fb, &[damage::full((fb.width, fb.height))]);
    }

    /// Presents source-pixel damage; stale wl_shm buffers catch up automatically.
    pub fn present_dirty(&mut self, fb: &Framebuffer, dirty: &[DirtyRect]) {
        if self.hidden.get() {return;}
        let state=self.state.borrow();
        if !state.configured {return;}
        let target_size=state.client_size;
        let maximized=state.maximized;
        let client_decorations = state.decorations_enabled && !state.server_decorations;
        drop(state);
        if target_size.0==0 || target_size.1==0 {return;}
        self.size.set((fb.width,fb.height));
        if client_decorations {
            let decorated_size = (fb.width, fb.height.saturating_add(decorations::HEIGHT));
            let new_frame = !self.last_client_decorations ||
                (self.decoration_frame.width,self.decoration_frame.height) != decorated_size;
            if new_frame {
                self.decoration_frame.resize(decorated_size.0,decorated_size.1);
            }
            let full_content=[damage::full((fb.width,fb.height))];
            let mut updates=decorations::update_content(&mut self.decoration_frame,fb,
                if new_frame {&full_content} else {dirty});
            if new_frame || self.decoration_dirty.replace(false) ||
                self.last_maximized != maximized {
                decorations::draw_title(&mut self.decoration_frame,&self.title.borrow(),
                    maximized,self.maximizable.get(),self.minimizable.get());
                updates.push(DirtyRect::new(0,0,decorated_size.0,decorations::HEIGHT));
            }
            self.last_client_decorations = true;
            self.last_maximized = maximized;
            self.renderer.resize_framebuffer(decorated_size);
            self.renderer.resize_target(target_size);
            self.renderer.present_dirty(&self.surface,&self.decoration_frame,&updates);
        } else {
            self.last_client_decorations = false;
            self.renderer.resize_framebuffer((fb.width,fb.height));
            self.renderer.resize_target(target_size);
            self.renderer.present_dirty(&self.surface,fb,dirty);
        }
        let _=self.connection.flush();
    }

    /// True after the compositor's frame callback (for continuously animated clients).
    pub fn frame_ready(&self) -> bool { self.renderer.frame_ready() }

    /// Hide via wl_pointer.set_cursor(None), show via the owned ARGB cursor
    /// surface. The pending policy is also applied on later pointer enters.
    pub fn set_cursor_visible(&self, visible: bool) -> Result<(), String> {
        let mut state = self.state.borrow_mut();
        state.cursor_visible = visible;
        if let (Some(pointer), Some(serial)) = (state.pointer.as_ref(), state.pointer_serial) {
            let surface = if visible { state.cursor_surface.as_ref() } else { None };
            pointer.set_cursor(serial, surface, 0, 0);
            self.connection.flush().map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn cursor_visible(&self) -> bool { self.state.borrow().cursor_visible }
    pub fn cursor_focused(&self) -> bool { self.state.borrow().pointer_focused }

    /// Sets title.
    ///
    /// Visibility: **public**.
    #[doc = "Sets title. Visibility: public."]
    pub fn set_title(&self, title: &str) {
        self.toplevel.set_title(title.to_owned());
        *self.title.borrow_mut() = title.to_owned();
        self.decoration_dirty.set(true);
        let _ = self.connection.flush();
    }

    /// Wayland's pure-Rust backend does not expose a stable native pointer like
    /// Win32's HWND. This method is retained for source compatibility and returns
    /// null. Use [`Self::surface`] or [`Self::surface_id`] for Wayland-specific code.
    #[doc = "Implements the handle operation. Visibility: public."]
    pub fn handle(&self) -> *mut c_void {
        std::ptr::null_mut()
    }

    /// Returns the underlying Wayland surface proxy.
    #[doc = "Implements the surface operation. Visibility: public."]
    pub fn surface(&self) -> &wl_surface::WlSurface {
        &self.surface
    }

    /// Returns the protocol object ID of the underlying `wl_surface`.
    ///
    /// Protocol IDs are connection-local and can be reused after destruction, so
    /// they should not be treated as process-wide unique handles.
    #[doc = "Implements the surface id operation. Visibility: public."]
    pub fn surface_id(&self) -> u32 {
        self.surface.id().protocol_id()
    }

    /// Returns the logical software framebuffer dimensions.
    #[doc = "Implements the framebuffer size operation. Visibility: public."]
    pub fn framebuffer_size(&self) -> (u32, u32) {
        self.size.get()
    }

    /// Returns the most recently configured surface size.
    #[doc = "Implements the client size operation. Visibility: public."]
    pub fn client_size(&self) -> (u32, u32) {
        let state=self.state.borrow();
        (state.client_size.0, state.client_size.1.saturating_sub(
            if state.decorations_enabled && !state.server_decorations {
                decorations::HEIGHT
            } else { 0 }
        ))
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for WaylandState {
    /// Performs the event operation.
    ///
    /// Visibility: **private**.
    #[doc = "Implements the event operation. Visibility: private/internal."]
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>
    ) {
        let wl_registry::Event::Global { name, interface, version } = event else {
            return;
        };

        match interface.as_str() {
            "wl_compositor" if state.compositor.is_none() => {
                state.compositor = Some(
                    registry.bind::<wl_compositor::WlCompositor, _, _>(name, version.min(6), qh, ())
                );
            }
            "wl_shm" if state.shm.is_none() => {
                state.shm = Some(
                    registry.bind::<wl_shm::WlShm, _, _>(name, version.min(2), qh, ())
                );
            }
            "zxdg_decoration_manager_v1" if state.decoration_manager.is_none() => {
                state.decoration_manager = Some(registry.bind::<
                    zxdg_decoration_manager_v1::ZxdgDecorationManagerV1, _, _
                >(name, version.min(1), qh, ()));
            }
            "xdg_wm_base" if state.wm_base.is_none() => {
                state.wm_base = Some(
                    registry.bind::<xdg_wm_base::XdgWmBase, _, _>(name, version.min(6), qh, ())
                );
            }
            "wl_seat" if state.seat.is_none() => {
                state.seat = Some(
                    registry.bind::<wl_seat::WlSeat, _, _>(name, version.min(9), qh, ())
                );
            }
            _ => {}
        }
    }
}

delegate_noop!(WaylandState: ignore zxdg_decoration_manager_v1::ZxdgDecorationManagerV1);

impl Dispatch<zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1, ()> for WaylandState {
    fn event(state: &mut Self,
        _: &zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1,
        event: zxdg_toplevel_decoration_v1::Event, _: &(),
        _: &Connection, _: &QueueHandle<Self>) {
        if let zxdg_toplevel_decoration_v1::Event::Configure { mode } = event {
            let server = matches!(mode,
                WEnum::Value(zxdg_toplevel_decoration_v1::Mode::ServerSide));
            if state.server_decorations != server {
                let old_chrome=state.decorations_enabled && !state.server_decorations;
                state.server_decorations = server;
                let new_chrome=state.decorations_enabled && !server;
                if old_chrome != new_chrome {
                    state.client_size.1=if new_chrome {
                        state.client_size.1.saturating_add(decorations::HEIGHT)
                    } else {
                        state.client_size.1.saturating_sub(decorations::HEIGHT)
                    };
                }
                if state.configured {
                    let chrome = if new_chrome { decorations::HEIGHT } else {0};
                    push_event(WindowEvent::Resized(state.client_size.0,
                        state.client_size.1.saturating_sub(chrome)));
                }
            }
        }
    }
}

delegate_noop!(WaylandState: ignore wl_compositor::WlCompositor);
delegate_noop!(WaylandState: ignore wl_surface::WlSurface);
delegate_noop!(WaylandState: ignore wl_shm::WlShm);
delegate_noop!(WaylandState: ignore wl_shm_pool::WlShmPool);

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for WaylandState {
    /// Performs the event operation.
    ///
    /// Visibility: **private**.
    #[doc = "Implements the event operation. Visibility: private/internal."]
    fn event(
        _: &mut Self,
        wm_base: &xdg_wm_base::XdgWmBase,
        event: xdg_wm_base::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            wm_base.pong(serial);
        }
    }
}

impl Dispatch<xdg_surface::XdgSurface, ()> for WaylandState {
    /// Performs the event operation.
    ///
    /// Visibility: **private**.
    #[doc = "Implements the event operation. Visibility: private/internal."]
    fn event(
        state: &mut Self,
        xdg_surface: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            xdg_surface.ack_configure(serial);
            state.configured = true;
        }
    }
}

impl Dispatch<xdg_toplevel::XdgToplevel, ()> for WaylandState {
    /// Performs the event operation.
    ///
    /// Visibility: **private**.
    #[doc = "Implements the event operation. Visibility: private/internal."]
    fn event(
        state: &mut Self,
        _: &xdg_toplevel::XdgToplevel,
        event: xdg_toplevel::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>
    ) {
        match event {
            xdg_toplevel::Event::Configure { width, height, states } => {
                let width = if width > 0 { width as u32 } else { state.client_size.0 };
                let height = if height > 0 { height as u32 } else { state.client_size.1 };

                let maximized = states
                    .chunks_exact(4)
                    .map(|bytes| u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
                    .any(|raw| raw == (xdg_toplevel::State::Maximized as u32));

                state.client_size = (width, height);

                if maximized {
                    state.maximized = true;
                    push_event(WindowEvent::Maximized(width, height.saturating_sub(
                        if state.decorations_enabled && !state.server_decorations {
                            decorations::HEIGHT
                        } else {0}
                    )));
                } else {
                    state.maximized = false;
                    push_event(WindowEvent::Resized(width, height.saturating_sub(
                        if state.decorations_enabled && !state.server_decorations {
                            decorations::HEIGHT
                        } else {0}
                    )));
                }
            }
            xdg_toplevel::Event::Close => {
                push_event(WindowEvent::Close);
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for WaylandState {
    /// Performs the event operation.
    ///
    /// Visibility: **private**.
    #[doc = "Implements the event operation. Visibility: private/internal."]
    fn event(
        state: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>
    ) {
        if let wl_seat::Event::Capabilities { capabilities: WEnum::Value(capabilities) } = event {
            if capabilities.contains(wl_seat::Capability::Keyboard) {
                if state.keyboard.is_none() {
                    state.keyboard = Some(seat.get_keyboard(qh, ()));
                }
            } else if let Some(keyboard) = state.keyboard.take() {
                if keyboard.version() >= 3 {
                    keyboard.release();
                }
            }

            if capabilities.contains(wl_seat::Capability::Pointer) {
                if state.pointer.is_none() {
                    state.pointer = Some(seat.get_pointer(qh, ()));
                }
            } else if let Some(pointer) = state.pointer.take() {
                state.pointer_serial = None;
                state.pointer_focused = false;
                if pointer.version() >= 3 {
                    pointer.release();
                }
            }
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for WaylandState {
    /// Performs the event operation.
    ///
    /// Visibility: **private**.
    #[doc = "Implements the event operation. Visibility: private/internal."]
    fn event(
        _: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>
    ) {
        match event {
            wl_keyboard::Event::Key { key, state: WEnum::Value(key_state), .. } => {
                let Some(key) = map_evdev_key(key) else {
                    return;
                };

                match key_state {
                    wl_keyboard::KeyState::Pressed => {
                        push_event(WindowEvent::KeyPressed(key));
                        set_key_down(key, true);
                    }
                    wl_keyboard::KeyState::Released => {
                        push_event(WindowEvent::KeyReleased(key));
                        set_key_down(key, false);
                    }
                    _ => {}
                }
            }
            wl_keyboard::Event::Leave { .. } => {
                // A focus change conceptually releases all keys for this window.
                if let Some(keys) = KEYS_DOWN.get() {
                    keys.lock().unwrap().clear();
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_pointer::WlPointer, ()> for WaylandState {
    /// Performs the event operation.
    ///
    /// Visibility: **private**.
    #[doc = "Implements the event operation. Visibility: private/internal."]
    fn event(
        state: &mut Self,
        pointer: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>
    ) {
        match event {
            wl_pointer::Event::Enter { serial, .. } => {
                state.pointer_serial = Some(serial);
                state.pointer_focused = true;
                let surface = if state.cursor_visible {
                    state.cursor_surface.as_ref()
                } else { None };
                pointer.set_cursor(serial, surface, 0, 0);
            }
            wl_pointer::Event::Leave { .. } => {
                state.pointer_serial = None;
                state.pointer_focused = false;
            }
            wl_pointer::Event::Motion { surface_x, surface_y, .. } => {
                let x = surface_x.round() as i32;
                let y = surface_y.round() as i32;
                state.pointer_position = (x,y);
                if state.decorations_enabled && !state.server_decorations {
                    if y < decorations::HEIGHT as i32 { return; }
                    push_event(WindowEvent::CursorMoved(x,y - decorations::HEIGHT as i32));
                } else {
                    push_event(WindowEvent::CursorMoved(x,y));
                }
            }
            wl_pointer::Event::Axis { axis: WEnum::Value(axis), value, .. } => {
                if state.decorations_enabled && !state.server_decorations &&
                    state.pointer_position.1 < decorations::HEIGHT as i32 { return; }
                let delta = (-value * 12.0).round() as i32;
                match axis {
                    wl_pointer::Axis::VerticalScroll => push_event(WindowEvent::Scroll(0, delta)),
                    wl_pointer::Axis::HorizontalScroll => push_event(WindowEvent::Scroll(delta, 0)),
                    _ => {}
                }
            }
            wl_pointer::Event::Button { button, state: WEnum::Value(button_state), serial, .. } => {
                if button == 0x110 && button_state == wl_pointer::ButtonState::Released &&
                    state.chrome_press_active {
                    state.chrome_press_active = false;
                    return;
                }
                if state.decorations_enabled && !state.server_decorations &&
                    state.pointer_position.1 < decorations::HEIGHT as i32 {
                    if button == 0x110 && button_state == wl_pointer::ButtonState::Pressed {
                        state.chrome_press_active = true;
                        let (x,y) = state.pointer_position;
                        match decorations::hit_test(x,y,state.client_size.0,
                            state.minimizable, state.maximizable) {
                            decorations::Action::Close => push_event(WindowEvent::Close),
                            decorations::Action::Minimize => {
                                if let Some(toplevel)=state.toplevel.as_ref() {
                                    toplevel.set_minimized();
                                }
                            }
                            decorations::Action::Maximize => {
                                if let Some(toplevel)=state.toplevel.as_ref() {
                                    if state.maximized { toplevel.unset_maximized(); }
                                    else { toplevel.set_maximized(); }
                                }
                            }
                            decorations::Action::Move => {
                                if let (Some(toplevel),Some(seat)) =
                                    (state.toplevel.as_ref(),state.seat.as_ref()) {
                                    toplevel._move(seat,serial);
                                }
                            }
                            decorations::Action::None => {}
                        }
                    } else if button == 0x110 &&
                        button_state == wl_pointer::ButtonState::Released {
                        // A content drag can end above the content boundary.
                        // Preserve the release so widget pressed state does not stick.
                        push_event(WindowEvent::KeyReleased(Key::MouseLeft));
                        set_key_down(Key::MouseLeft,false);
                    }
                    return;
                }
                let key = match button {
                    0x110 => Some(Key::MouseLeft),
                    0x111 => Some(Key::MouseRight),
                    0x112 => Some(Key::MouseMiddle),
                    _ => None,
                };
                let Some(key) = key else {
                    return;
                };
                match button_state {
                    wl_pointer::ButtonState::Pressed => {
                        push_event(WindowEvent::KeyPressed(key));
                        set_key_down(key, true);
                    }
                    wl_pointer::ButtonState::Released => {
                        push_event(WindowEvent::KeyReleased(key));
                        set_key_down(key, false);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_callback::WlCallback, Arc<AtomicBool>> for WaylandState {
    fn event(_: &mut Self, _: &wl_callback::WlCallback, event: wl_callback::Event,
        ready: &Arc<AtomicBool>, _: &Connection, _: &QueueHandle<Self>) {
        if let wl_callback::Event::Done { .. }=event {
            ready.store(true,Ordering::Release);
        }
    }
}

impl Dispatch<wl_buffer::WlBuffer, Arc<AtomicBool>> for WaylandState {
    /// Performs the event operation.
    ///
    /// Visibility: **private**.
    #[doc = "Implements the event operation. Visibility: private/internal."]
    fn event(
        _: &mut Self,
        _: &wl_buffer::WlBuffer,
        event: wl_buffer::Event,
        busy: &Arc<AtomicBool>,
        _: &Connection,
        _: &QueueHandle<Self>
    ) {
        if let wl_buffer::Event::Release = event {
            busy.store(false, Ordering::Release);
        }
    }
}

/// Implements map evdev key.
///
/// Visibility: **private**.
#[doc = "Implements the map evdev key operation. Visibility: private/internal."]
fn map_evdev_key(code: u32) -> Option<Key> {
    match code {
        1 => Some(Key::Escape),
        14 => Some(Key::Backspace),
        15 => Some(Key::Tab),
        28 => Some(Key::Enter),
        29 => Some(Key::LeftCtrl),
        42 => Some(Key::LeftShift),
        54 => Some(Key::RightShift),
        57 => Some(Key::Space),
        97 => Some(Key::RightCtrl),

        103 => Some(Key::Up),
        105 => Some(Key::Left),
        106 => Some(Key::Right),
        108 => Some(Key::Down),
        102 => Some(Key::Home),
        107 => Some(Key::End),
        111 => Some(Key::Delete),

        2..=10 => Some(Key::Num((code - 1) as u8)),
        11 => Some(Key::Num(0)),

        59..=68 => Some(Key::F((code - 58) as u8)),
        87 => Some(Key::F(11)),
        88 => Some(Key::F(12)),

        16 => Some(Key::Char('q')),
        17 => Some(Key::Char('w')),
        18 => Some(Key::Char('e')),
        19 => Some(Key::Char('r')),
        20 => Some(Key::Char('t')),
        21 => Some(Key::Char('y')),
        22 => Some(Key::Char('u')),
        23 => Some(Key::Char('i')),
        24 => Some(Key::Char('o')),
        25 => Some(Key::Char('p')),
        30 => Some(Key::Char('a')),
        31 => Some(Key::Char('s')),
        32 => Some(Key::Char('d')),
        33 => Some(Key::Char('f')),
        34 => Some(Key::Char('g')),
        35 => Some(Key::Char('h')),
        36 => Some(Key::Char('j')),
        37 => Some(Key::Char('k')),
        38 => Some(Key::Char('l')),
        44 => Some(Key::Char('z')),
        45 => Some(Key::Char('x')),
        46 => Some(Key::Char('c')),
        47 => Some(Key::Char('v')),
        48 => Some(Key::Char('b')),
        49 => Some(Key::Char('n')),
        50 => Some(Key::Char('m')),

        12 => Some(Key::Char('-')),
        13 => Some(Key::Char('=')),
        26 => Some(Key::Char('[')),
        27 => Some(Key::Char(']')),
        39 => Some(Key::Char(';')),
        40 => Some(Key::Char('\'')),
        41 => Some(Key::Char('`')),
        43 => Some(Key::Char('\\')),
        51 => Some(Key::Char(',')),
        52 => Some(Key::Char('.')),
        53 => Some(Key::Char('/')),

        _ => None,
    }
}
