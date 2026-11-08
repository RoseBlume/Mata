//! Widget host, input dispatch, focus management, and presentation.

use std::{
    cell::{ Cell, RefCell },
    collections::HashMap,
    rc::Rc,
    sync::{ Arc, Mutex, atomic::{ AtomicBool, Ordering } },
    time::{ Duration, Instant },
};

#[cfg(feature = "fonts")]
use std::path::Path;

use crate::{ Framebuffer, Window as NativeWindow, events::{ Key, WindowEvent } };

use super::{
    controls::{ sync_tree_states, ControlEntry, TextFieldState },
    events::{ drain_events, next_id, WidgetError, WidgetEvent, WidgetId },
    render::{ draw_button, draw_text_field, next_boundary, BG },
    content::{ContentPainter, WidgetContent, SelectableHit},
    catalog::WidgetExt,
};

#[cfg(feature = "fonts")]
use super::FontRenderer;

#[cfg(feature = "video")]
use super::{ catalog::Video, events::push_event };

#[cfg(any(feature = "webview", feature = "webview-servo"))]
use std::{ rc::Weak as RcWeak, sync::Weak as ArcWeak };

#[cfg(any(feature = "webview", feature = "webview-servo"))]
use super::geometry::ResizeState;

#[cfg(any(feature = "webview", feature = "webview-servo"))]
use super::webview_platform;

#[cfg(any(feature = "webview", feature = "webview-servo"))]
/// Stores the state and data for web view resize entry.
///
/// Visibility: **private**.
struct WebViewResizeEntry {
    /// Stores inner.
    ///
    /// Visibility: **private**.
    inner: RcWeak<webview_platform::WebView>,
    /// Stores resize.
    ///
    /// Visibility: **private**.
    resize: ArcWeak<Mutex<ResizeState>>,
    /// Stores style.
    ///
    /// Visibility: **private**.
    style: crate::widgets::style::StyleHandle,
}

/// Lower default than the historical unnormalized scroll multiplier.
pub const DEFAULT_SCROLL_SENSITIVITY: f32 = 0.6;
const PIXELS_PER_WHEEL_NOTCH: f32 = 48.0;
const WHEEL_UNITS_PER_NOTCH: f32 = 120.0;

fn validate_scroll_sensitivity(value: f32) -> Result<(), WidgetError> {
    if !value.is_finite() || !(0.0..=10.0).contains(&value) {
        return Err(WidgetError("scroll sensitivity must be a finite number between 0 and 10".into()));
    }
    Ok(())
}

// A renderer may already be borrowed by a caller that re-enters layout
// (for example while processing a nested UI update). Do not panic in that
// case: keep the previous geometry and mark the window dirty so its next
// presentation retries after the outstanding borrow ends.
#[cfg(feature = "fonts")]
fn try_layout_font<'a>(
    font: &'a RefCell<Option<FontRenderer>>,
    dirty: &AtomicBool,
) -> Option<std::cell::RefMut<'a, Option<FontRenderer>>> {
    match font.try_borrow_mut() {
        Ok(renderer) => Some(renderer),
        Err(_) => {
            dirty.store(true, Ordering::Release);
            None
        }
    }
}

/// Converts native wheel deltas to logical pixels; keeps fractions so high-resolution
/// touchpad / trackpad input still works when sensitivity is low.
fn wheel_pixels(raw: i32, sensitivity: f32, remainder: &mut f32) -> i32 {
    if sensitivity == 0.0 { *remainder = 0.0; return 0; }
    let exact = (raw as f32 / WHEEL_UNITS_PER_NOTCH) * PIXELS_PER_WHEEL_NOTCH
        * sensitivity + *remainder;
    let whole = exact.trunc().clamp(i32::MIN as f32, i32::MAX as f32) as i32;
    *remainder = exact.fract();
    whole
}

/// A standalone retained visual root; interactive buttons are still registered
/// separately as controls so their existing input/focus behavior is preserved.
/// A live catalog widget, rendered from its current state on every frame.
/// Unlike a snapshot `WidgetContent::Painted`, this root also participates in
/// pointer hit testing and invokes the widget's real state-change methods.
struct CatalogRoot {
    id: WidgetId,
    requested_bounds: super::Rect,
    bounds: super::Rect,
    widget: Rc<dyn super::catalog::WidgetExt>,
}

struct ContentRoot {
    id: WidgetId,
    // The caller's dimensions. Zero means auto on that axis.
    requested_bounds: super::Rect,
    bounds: super::Rect,
    auto_overflow: bool,
    content: WidgetContent,
    scroll_extent: Option<u32>,
    scroll_y: i32,
    scroll_sensitivity: Option<f32>,
    scroll_remainder: f32,
}

/// Platform-tagged, non-owning native window handle.
///
/// The handle is valid only while its `WidgetWindow` is alive. Callers must
/// not close or otherwise take ownership of the underlying OS window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeWindowHandle {
    #[cfg(target_os = "windows")]
    Win32(std::ptr::NonNull<std::ffi::c_void>),
    Unsupported,
}

/// A MataWin software-widget host. No GTK/Win32 child controls are created.
pub struct WidgetWindow {
    /// Stores id.
    ///
    /// Visibility: **crate-visible**.
    pub(crate) id: u64,
    /// Stores window.
    ///
    /// Visibility: **private**.
    window: RefCell<NativeWindow>,
    /// Stores framebuffer.
    ///
    /// Visibility: **private**.
    framebuffer: RefCell<Framebuffer>,
    /// Stores controls.
    ///
    /// Visibility: **private**.
    controls: RefCell<Vec<ControlEntry>>,
    content_roots: RefCell<Vec<ContentRoot>>,
    catalog_roots: RefCell<Vec<CatalogRoot>>,
    catalog_pressed: Cell<Option<WidgetId>>,
    popup: RefCell<Option<super::interaction::ContextMenu>>,
    context_menus: RefCell<HashMap<WidgetId, Vec<super::interaction::MenuItem>>>,
    accessibility: RefCell<HashMap<WidgetId, super::interaction::AccessibilityNode>>,
    scroll_clips: RefCell<HashMap<WidgetId, super::Rect>>,
    scroll_sensitivity: Cell<f32>,
    selection_drag: RefCell<Option<SelectableHit>>,
    selection_focus: RefCell<Option<super::catalog::Text>>,
    right_pressed: Cell<bool>,
    /// Stores render order.
    ///
    /// Visibility: **private**.
    render_order: RefCell<Vec<usize>>,
    /// Stores style references.
    ///
    /// Visibility: **private**.
    style_references: RefCell<HashMap<WidgetId, crate::widgets::Rect>>,
    /// Stores focus.
    ///
    /// Visibility: **crate-visible**.
    pub(crate) focus: Arc<Mutex<Option<WidgetId>>>,
    /// Stores cursor.
    ///
    /// Visibility: **private**.
    cursor: Cell<(i32, i32)>,
    /// Stores left shift down.
    ///
    /// Visibility: **private**.
    left_shift_down: Cell<bool>,
    /// Stores right shift down.
    ///
    /// Visibility: **private**.
    right_shift_down: Cell<bool>,
    /// Timestamp for the first Backspace press and next scheduled repeat.
    backspace_repeat: RefCell<Option<(Instant,Instant)>>,
    left_ctrl_down: Cell<bool>,
    right_ctrl_down: Cell<bool>,
    /// Stores font.
    ///
    /// Visibility: **private**.
    #[cfg(feature = "fonts")]
    font: RefCell<Option<FontRenderer>>,
    /// Stores font cache.
    ///
    /// Visibility: **private**.
    #[cfg(feature = "fonts")]
    font_cache: RefCell<HashMap<String, FontRenderer>>,
    /// Stores background.
    ///
    /// Visibility: **private**.
    background: Cell<u32>,
    /// Stores dirty.
    ///
    /// Visibility: **crate-visible**.
    pub(crate) dirty: Arc<AtomicBool>,
    #[cfg(feature = "video")]
    videos: RefCell<Vec<Video>>,
    /// Stores last present.
    ///
    /// Visibility: **private**.
    last_present: Cell<Instant>,
    /// Stores was animating.
    ///
    /// Visibility: **private**.
    was_animating: Cell<bool>,
    /// Stores native events.
    ///
    /// Visibility: **private**.
    native_events: RefCell<Vec<WindowEvent>>,
    /// Stores host size.
    ///
    /// Visibility: **crate-visible**.
    pub(crate) host_size: Rc<Cell<(u32, u32)>>,
    #[cfg(any(feature = "webview", feature = "webview-servo"))]
    /// Stores webviews.
    ///
    /// Visibility: **private**.
    webviews: RefCell<Vec<WebViewResizeEntry>>,
    #[cfg(feature = "webview-servo")]
    servo_focused: Cell<Option<WidgetId>>,
    #[cfg(feature = "webview-servo")]
    servo_pressed: Cell<Option<WidgetId>>,
    #[cfg(feature = "webview-servo")]
    servo_hovered: Cell<Option<WidgetId>>,
}
impl WidgetWindow {
    /// Attaches the MataWin software-widget layer to an existing [`Window`].
    ///
    /// This is the preferred constructor for applications: create the native
    /// `mata_win::Window` first, configure it as needed, and then move it into
    /// the widget host. The host continues to expose the same underlying
    /// window for event pumping and framebuffer presentation.
    #[doc = "Implements the from window operation. Visibility: public."]
    pub fn from_window(window: NativeWindow) -> Result<Self, WidgetError> {
        let (width, height) = window.client_size();
        let size = (width.max(1), height.max(1));
        Ok(Self {
            id: next_id(),
            window: RefCell::new(window),
            framebuffer: RefCell::new(Framebuffer::from_color(size.0, size.1, BG)),
            controls: RefCell::new(Vec::with_capacity(16)),
            content_roots: RefCell::new(Vec::new()),
            catalog_roots: RefCell::new(Vec::new()),
            catalog_pressed: Cell::new(None),
            popup: RefCell::new(None),
            context_menus: RefCell::new(HashMap::new()),
            accessibility: RefCell::new(HashMap::new()),
            scroll_clips: RefCell::new(HashMap::new()),
            scroll_sensitivity: Cell::new(DEFAULT_SCROLL_SENSITIVITY),
            selection_drag: RefCell::new(None),
            selection_focus: RefCell::new(None),
            right_pressed: Cell::new(false),
            render_order: RefCell::new(Vec::with_capacity(16)),
            style_references: RefCell::new(HashMap::with_capacity(16)),
            focus: Arc::new(Mutex::new(None)),
            cursor: Cell::new((0, 0)),
            left_shift_down: Cell::new(false),
            right_shift_down: Cell::new(false),
            backspace_repeat: RefCell::new(None),
            left_ctrl_down: Cell::new(false),
            right_ctrl_down: Cell::new(false),
            #[cfg(feature = "fonts")]
            font: RefCell::new(FontRenderer::system_default().ok()),
            #[cfg(feature = "fonts")]
            font_cache: RefCell::new(HashMap::new()),
            background: Cell::new(BG),
            dirty: Arc::new(AtomicBool::new(true)),
            #[cfg(feature = "video")]
            videos: RefCell::new(Vec::new()),
            last_present: Cell::new(Instant::now() - Duration::from_millis(17)),
            was_animating: Cell::new(false),
            native_events: RefCell::new(Vec::with_capacity(16)),
            host_size: Rc::new(Cell::new(size)),
            #[cfg(any(feature = "webview", feature = "webview-servo"))]
            webviews: RefCell::new(Vec::with_capacity(4)),
            #[cfg(feature = "webview-servo")]
            servo_focused: Cell::new(None),
            #[cfg(feature = "webview-servo")]
            servo_pressed: Cell::new(None),
            #[cfg(feature = "webview-servo")]
            servo_hovered: Cell::new(None),
        })
    }

    /// Exposes a non-owning, explicitly tagged operating-system window handle.
    /// The handle must only be used while this widget window remains alive.
    pub fn os_handle(&self) -> NativeWindowHandle {
        #[cfg(target_os = "windows")]
        {
            return match std::ptr::NonNull::new(self.native_handle()) {
                Some(hwnd) => NativeWindowHandle::Win32(hwnd),
                None => NativeWindowHandle::Unsupported,
            };
        }
        #[cfg(not(target_os = "windows"))]
        {
            NativeWindowHandle::Unsupported
        }
    }

    /// XDG Desktop Portal parent identifier. X11 can use its XID directly;
    /// the Wayland backend currently has no xdg-foreign exported handle and
    /// returns an empty string (unparented portal dialog).
    #[cfg(target_os = "linux")]
    pub fn portal_parent_window(&self) -> String {
        let raw = self.window.borrow().handle() as usize;
        if raw == 0 { String::new() } else { format!("x11:{raw:x}") }
    }

    /// Returns the host HWND for creating an OS-native document text control.
    /// This handle is owned by WidgetWindow and must not be destroyed by callers.
    #[cfg(target_os = "windows")]
    pub fn windows_hwnd(&self) -> *mut std::ffi::c_void {
        self.native_handle()
    }

    /// Convenience constructor retained for compatibility.
    ///
    /// New examples use [`WidgetWindow::from_window`] so that the application
    /// creates and configures its `mata_win::Window` explicitly.
    #[doc = "Creates a new value. Visibility: public."]
    pub fn new(title: &str, size: (u32, u32)) -> Result<Self, WidgetError> {
        let size = (size.0.max(1), size.1.max(1));
        Self::from_window(NativeWindow::new(title, size))
    }
    /// Sets font file.
    ///
    /// Visibility: **public**.
    #[doc = "Sets font file. Visibility: public."]
    #[cfg(feature = "fonts")]
    pub fn set_font_file(&self, path: impl AsRef<Path>) -> Result<(), WidgetError> {
        *self.font.borrow_mut() = Some(FontRenderer::from_file(path)?);
        self.request_redraw();
        Ok(())
    }
    /// Sets font bytes.
    ///
    /// Visibility: **public**.
    #[doc = "Sets font bytes. Visibility: public."]
    #[cfg(feature = "fonts")]
    pub fn set_font_bytes(&self, bytes: &[u8]) -> Result<(), WidgetError> {
        *self.font.borrow_mut() = Some(FontRenderer::from_bytes(bytes)?);
        self.request_redraw();
        Ok(())
    }
    /// Register a CSS `@font-face` family for native text measurement and paint.
    /// Font bytes must be a TrueType font supported by `mata-fonts`; WOFF2 is
    /// decoded to OpenType by the browser loader before this call.
    #[cfg(feature = "fonts")]
    pub fn register_font_face(&self, family: &str, bytes: &[u8]) -> Result<(), WidgetError> {
        let name = family.trim().trim_matches(|c| c == '\'' || c == '"').to_ascii_lowercase();
        if name.is_empty() {return Err(WidgetError("font family is empty".into()));}
        let renderer=FontRenderer::from_bytes(bytes)?;
        self.font_cache.borrow_mut().insert(format!("web:{name}"),renderer);
        self.request_redraw();
        Ok(())
    }
    /// Remove page-scoped fonts when navigating, without disturbing system fonts.
    #[cfg(feature = "fonts")]
    pub fn clear_web_fonts(&self) {
        self.font_cache.borrow_mut().retain(|key,_|!key.starts_with("web:"));
        self.request_redraw();
    }
    #[inline]
    #[doc = "Sets background. Visibility: public."]
    pub fn set_background(&self, color: u32) {
        if self.background.replace(color) != color {
            self.request_redraw()
        }
    }
    #[inline]
    #[doc = "Implements the size operation. Visibility: public."]
    pub fn size(&self) -> (u32, u32) {
        self.host_size.get()
    }
    #[inline]
    #[doc = "Sets title. Visibility: public."]
    pub fn set_title(&self, title: &str) {
        self.window.borrow().set_title(title)
    }
    #[inline]
    #[doc = "Implements the id operation. Visibility: crate-visible."]
#[allow(dead_code)] // Retained for optional codecs, benchmarks, tests, or future integration.
    pub(crate) fn id(&self) -> u64 {
        self.id
    }
    #[inline]
    #[doc = "Implements the native handle operation. Visibility: crate-visible."]
#[allow(dead_code)] // Retained for optional codecs, benchmarks, tests, or future integration.
    pub(crate) fn native_handle(&self) -> *mut std::ffi::c_void {
        self.window.borrow().handle()
    }
    /// Registers a native video widget with the software renderer.
    ///
    /// Registered video frames are clocked on `present()` and painted beneath
    /// the ordinary interactive controls. The widget remains usable outside
    /// this host by calling `update()` and `blit()` directly.
    #[cfg(feature = "video")]
    pub fn add_video(&self, video: &Video) {
        use super::catalog::WidgetExt;
        let mut videos = self.videos.borrow_mut();
        if videos.iter().any(|existing| existing.id() == video.id()) { return; }
        videos.push(video.clone());
        video.register_redraw(&self.dirty);
        drop(videos);
        self.request_redraw();
    }

    /// Removes a video previously added with `add_video`.
    #[cfg(feature = "video")]
    pub fn remove_video(&self, id: WidgetId) {
        use super::catalog::WidgetExt;
        self.videos.borrow_mut().retain(|video| video.id() != id);
        self.request_redraw();
    }

    /// Marks the software widget surface dirty. The next `present()` rerenders it.
    #[inline]
    #[doc = "Implements the request redraw operation. Visibility: public."]
    pub fn request_redraw(&self) {
        self.dirty.store(true, Ordering::Release)
    }
    #[doc = "Implements the register operation. Visibility: crate-visible."]
    pub(crate) fn register(&self, c: ControlEntry) {
        let mut controls = self.controls.borrow_mut();
        controls.push(c);
        sync_tree_states(&controls);
        drop(controls);
        self.request_redraw();
    }
    #[cfg(any(feature = "webview", feature = "webview-servo"))]
    /// Registers webview.
    ///
    /// Visibility: **crate-visible**.
    #[doc = "Registers webview. Visibility: crate-visible."]
    pub(crate) fn register_webview(
        &self,
        inner: &Rc<webview_platform::WebView>,
        resize: &Arc<Mutex<ResizeState>>,
        style: &crate::widgets::style::StyleHandle
    ) {
        self.webviews
            .borrow_mut()
            .push(WebViewResizeEntry {
                inner: Rc::downgrade(inner),
                resize: Arc::downgrade(resize),
                style: style.clone(),
            });
    }
    /// Performs the resize widgets operation.
    ///
    /// Visibility: **private**.
    #[doc = "Resizes widgets. Visibility: private/internal."]
    fn resize_widgets(&self, size: (u32, u32)) {
        let size = (size.0.max(1), size.1.max(1));
        self.host_size.set(size);
        for control in self.controls.borrow().iter() {
            control.apply_resize(size);
        }
        self.resolve_control_styles();
        #[cfg(any(feature = "webview", feature = "webview-servo"))]
        self.sync_webviews();
    }

    /// Invalidates the old raster surface and lays every software widget out
    /// against the new native client-pixel dimensions. No old frame is scaled
    /// into the replacement framebuffer.
    #[doc = "Resizes render surface. Visibility: private/internal."]
    fn resize_render_surface(&self, size: (u32, u32)) {
        let size = (size.0.max(1), size.1.max(1));
        self.framebuffer.borrow_mut().resize_and_clear(size.0, size.1, self.background.get());
        self.resize_widgets(size);
        // A resize can move responsive controls underneath a stationary cursor.
        // Re-evaluate hover immediately rather than waiting for another mouse
        // movement message.
        let (x, y) = self.cursor.get();
        self.update_hover(x, y);
        self.request_redraw();
    }

    /// Synchronizes the software render target to the native drawable client
    /// area. This runs both while polling and immediately before presentation,
    /// so a coalesced/missed resize event cannot leave an old low-resolution
    /// framebuffer stretched across a larger window.
    #[doc = "Synchronizes native size. Visibility: private/internal."]
    fn sync_native_size(&self) -> Option<(u32, u32)> {
        let native = self.window.borrow().client_size();
        if native.0 == 0 || native.1 == 0 {
            return None;
        }
        if native != self.host_size.get() {
            self.resize_render_surface(native);
        }
        Some(native)
    }

    #[inline]
    /// Performs the resize event size operation.
    ///
    /// Visibility: **private**.
    #[doc = "Resizes event size. Visibility: private/internal."]
    fn resize_event_size(&self, fallback: (u32, u32)) -> (u32, u32) {
        self.sync_native_size().unwrap_or((fallback.0.max(1), fallback.1.max(1)))
    }

    #[cfg(any(feature = "webview", feature = "webview-servo"))]
    /// Performs the sync webviews operation.
    ///
    /// Visibility: **private**.
    #[doc = "Synchronizes webviews. Visibility: private/internal."]
    fn sync_webviews(&self) {
        let size = self.size();
        let references: HashMap<WidgetId, crate::widgets::Rect> = self.controls
            .borrow()
            .iter()
            .map(|c| (c.id(), c.bounds()))
            .collect();
        self.webviews.borrow_mut().retain(|entry| {
            let (Some(inner), Some(resize)) = (entry.inner.upgrade(), entry.resize.upgrade()) else {
                return false;
            };
            let base = resize.lock().unwrap().resolve(size);
            let reference = entry.style.relative_to().and_then(|id| references.get(&id).copied());
            let bounds = entry.style.computed(base, size, reference).bounds;
            inner.set_bounds(bounds);
            true
        });
    }

    #[cfg(feature = "webview-servo")]
    fn sync_servo_frames(&self) {
        for entry in self.webviews.borrow().iter() {
            if let Some(inner) = entry.inner.upgrade() {
                if inner.needs_redraw() {
                    self.request_redraw();
                }
            }
        }
    }

    #[cfg(feature = "webview-servo")]
    fn forward_servo_input(&self, events: &[WidgetEvent]) {
        for event in events {
            let point = match event {
                WidgetEvent::PointerMoved(x, y) | WidgetEvent::PointerPressed(x, y)
                | WidgetEvent::PointerReleased(x, y)
                | WidgetEvent::PointerRightPressed(x, y)
                | WidgetEvent::PointerRightReleased(x, y) => Some((*x, *y)),
                WidgetEvent::Scrolled { x, y, .. } => Some((*x, *y)),
                _ => None,
            };
            if let Some((x, y)) = point {
                let pressed = matches!(event, WidgetEvent::PointerPressed(..) | WidgetEvent::PointerRightPressed(..));
                let released = matches!(event, WidgetEvent::PointerReleased(..) | WidgetEvent::PointerRightReleased(..));
                let moving = matches!(event, WidgetEvent::PointerMoved(..));
                let captured = self.servo_pressed.get();
                let entries = self.webviews.borrow();
                let hit = entries.iter().rev().find_map(|entry| {
                    let inner = entry.inner.upgrade()?;
                    let bounds = inner.bounds();
                    (inner.visible() && bounds.contains(x, y)).then_some(inner)
                });
                let target = if (released || moving) && captured.is_some() {
                    entries.iter().find_map(|entry| {
                        let inner = entry.inner.upgrade()?;
                        (Some(inner.id()) == captured).then_some(inner)
                    }).or(hit.clone())
                } else {
                    hit.clone()
                };
                if moving {
                    let new_hover = hit.as_ref().map(|inner| inner.id());
                    let old_hover = self.servo_hovered.replace(new_hover);
                    if old_hover != new_hover {
                        for entry in entries.iter() {
                            if let Some(inner) = entry.inner.upgrade() {
                                if Some(inner.id()) == old_hover { inner.mouse_left(); break; }
                            }
                        }
                    }
                }
                if pressed {
                    let new_focus = target.as_ref().map(|inner| inner.id());
                    let old_focus = self.servo_focused.replace(new_focus);
                    self.servo_pressed.set(new_focus);
                    if old_focus != new_focus {
                        for entry in entries.iter() {
                            if let Some(inner) = entry.inner.upgrade() {
                                if Some(inner.id()) == old_focus { inner.set_focused(false); break; }
                            }
                        }
                    }
                    if new_focus.is_some() {
                        *self.focus.lock().unwrap() = None;
                    }
                }
                if let Some(inner) = target {
                    inner.handle_input(event, inner.bounds());
                }
                if released { self.servo_pressed.set(None); }
            } else if matches!(event, WidgetEvent::KeyPressed(_) | WidgetEvent::KeyReleased(_)) {
                if let Some(id) = self.servo_focused.get() {
                    if self.focused_text_field() { continue; }
                    for entry in self.webviews.borrow().iter() {
                        if let Some(inner) = entry.inner.upgrade() {
                            if inner.id() == id {
                                inner.handle_input(event, inner.bounds());
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Performs the poll events into operation.
    ///
    /// Visibility: **public**.
    #[doc = "Implements the poll events into operation. Visibility: public."]
    pub fn poll_events_into(&self, out: &mut Vec<WidgetEvent>) {
        #[cfg(any(feature = "webview", feature = "webview-servo"))]
        {
            webview_platform::pump_events();
            self.sync_webviews();
        }
        let mut native = self.native_events.borrow_mut();
        self.window.borrow().poll_events_into(&mut native);
        // Query the actual drawable size after the native message pump. The
        // resize event is still forwarded below, but rendering no longer relies
        // on that event having exact/current dimensions.
        self.sync_native_size();
        out.clear();
        for (index,event) in native.iter().copied().enumerate() {
            // X11 autorepeat may report an artificial KeyReleased immediately
            // followed by KeyPressed. Keep the held-key timer through that pair.
            if matches!(event,WindowEvent::KeyReleased(Key::Backspace))
                && matches!(native.get(index+1),Some(WindowEvent::KeyPressed(Key::Backspace))) {continue;}
            self.handle_native(event, out);
        }
        native.clear();
        // Ignore compositor/OS key-repeat events and apply a single consistent
        // 1000 ms initial delay with 50 ms repeats across all native backends.
        let now=Instant::now();
        let mut repeating=self.backspace_repeat.borrow_mut();
        if let Some((_,mut next))=*repeating {
            let mut count=0;
            while now>=next && count<5 {
                self.backspace(out);
                next+=Duration::from_millis(50);
                count+=1;
            }
            *repeating=Some((now,next));
        }
        drop(repeating);
        // Touch packets have an independent queue in mata-win. Expose them as
        // ordinary UI events so consumers need not handle the OS window itself.
        let mut touches = Vec::new();
        self.window.borrow().poll_touches_into(&mut touches);
        for touch in touches {
            use crate::widgets::events::TouchPhase;
            use mata_win::events::TouchEvent;
            let (phase, x, y) = match touch.event {
                TouchEvent::Started(x, y) => (TouchPhase::Started, x, y),
                TouchEvent::Moved(x, y) => (TouchPhase::Moved, x, y),
                TouchEvent::Ended(x, y) => (TouchPhase::Ended, x, y),
                TouchEvent::Cancelled => (TouchPhase::Cancelled, 0, 0),
            };
            out.push(WidgetEvent::Touched {
                device_id: touch.device_id,
                phase,
                x: x.min(i32::MAX as u64) as i32,
                y: y.min(i32::MAX as u64) as i32,
            });
        }
        #[cfg(feature = "webview-servo")]
        self.forward_servo_input(out);
        drain_events(self.id, out);
    }
    /// Performs the poll events operation.
    ///
    /// Visibility: **public**.
    #[doc = "Implements the poll events operation. Visibility: public."]
    pub fn poll_events(&self) -> Vec<WidgetEvent> {
        let mut out = Vec::with_capacity(16);
        self.poll_events_into(&mut out);
        out
    }

    /// Rerasterizes the entire software widget tree at the current native client
    /// resolution and only then submits the frame to the platform renderer.
    ///
    /// A resize can race with rendering. If that happens, `present_exact` rejects
    /// the stale-size frame and this method rerenders once at the newest size
    /// instead of stretching the previous raster image.
    /// Presents only when the widget tree is dirty or an animation needs a frame.
    /// Static focused windows therefore consume essentially no presentation GPU
    /// bandwidth instead of uploading the full framebuffer in a tight loop.
    #[doc = "Implements the present operation. Visibility: public."]
    /// True if the software host will submit a frame on the next present().
    /// Useful to synchronize Win32 child surfaces with a Direct2D parent.
    pub fn redraw_pending(&self) -> bool {
        self.dirty.load(Ordering::Acquire)
    }

    pub fn present(&self) {
        self.sync_native_size();
        #[cfg(feature = "video")]
        for video in self.videos.borrow().iter() {
            use super::catalog::WidgetExt;
            match video.update() {
                Ok(true) => self.request_redraw(),
                Ok(false) => {}
                Err(error) => {
                    video.pause();
                    push_event(self.id, WidgetEvent::VideoError(video.id(), error.to_string()));
                }
            }
        }
        #[cfg(feature = "webview-servo")]
        self.sync_servo_frames();
        let animating = self.controls.borrow().iter().any(ControlEntry::animation_running);
        let was_animating = self.was_animating.get();
        let dirty = self.dirty.load(Ordering::Acquire);
        if !dirty && !animating && !was_animating {
            return;
        }

        // Animation-only updates are capped at about 60 Hz. Input/style changes
        // bypass the cap so hover, typing, and focus remain immediate.
        if !dirty && animating && self.last_present.get().elapsed() < Duration::from_millis(16) {
            return;
        }

        // Consume the current invalidation before rendering. If another setter
        // runs during the render it sets the flag again and that update is not lost.
        self.dirty.swap(false, Ordering::AcqRel);
        for _ in 0..2 {
            self.sync_native_size();
            self.render();
            let presented = {
                let fb = self.framebuffer.borrow();
                debug_assert_eq!((fb.width, fb.height), self.host_size.get());
                self.window.borrow_mut().present_exact(&fb)
            };
            if presented {
                self.last_present.set(Instant::now());
                self.was_animating.set(animating);
                return;
            }
        }
        self.request_redraw();
    }

    /// Returns a copy of the current framebuffer. For large windows this is a
    /// full pixel-buffer copy; prefer `with_framebuffer` when a borrow is enough.
    #[doc = "Implements the framebuffer operation. Visibility: public."]
    pub fn framebuffer(&self) -> Framebuffer {
        self.framebuffer.borrow().clone()
    }
    /// Returns the value configured with framebuffer.
    ///
    /// Visibility: **public**.
    #[doc = "Provides access configured with framebuffer. Visibility: public."]
    pub fn with_framebuffer<R>(&self, f: impl FnOnce(&Framebuffer) -> R) -> R {
        let fb = self.framebuffer.borrow();
        f(&fb)
    }

    /// Draws a standalone retained content tree, which can itself contain
    /// nested row/column/overlay layouts, text, and images. Interactive button
    /// hit-testing still belongs to `Button` controls.
    pub fn add_content(&self, bounds: super::Rect, child: impl Into<WidgetContent>) -> WidgetId {
        let content = child.into();
        content.attach(&self.dirty);
        let id = WidgetId(next_id());
        self.content_roots.borrow_mut().push(ContentRoot { id, requested_bounds: bounds, bounds, auto_overflow: bounds.height == 0,
            content, scroll_extent:None, scroll_y:0, scroll_sensitivity:None, scroll_remainder:0.0 });
        self.request_redraw();
        id
    }

    /// Add a retained content node whose absent width and/or height are
    /// measured from its contents. Each missing dimension is independently
    /// shrink-to-fit, and capped by the remaining window client area.
    /// If the auto height exceeds the viewport, it scrolls instead of clipping.
    pub fn add_auto_content(&self, x:i32, y:i32,
        width:Option<u32>, height:Option<u32>, child:impl Into<WidgetContent>) -> WidgetId {
        self.add_content(super::Rect::new(x,y,width.unwrap_or(0),height.unwrap_or(0)),child)
    }

    /// Return the current laid-out bounds. Zero dimensions supplied to
    /// `add_content`/`set_content_bounds` are resolved automatically.
    pub fn content_bounds(&self,id:WidgetId)->Option<super::Rect> {
        self.resolve_auto_content();
        self.content_roots.borrow().iter().find(|r|r.id==id).map(|r|r.bounds)
    }

    /// Resolve auto sizes using the same text measurement as drawing.
    /// In particular, a changed Text, font size or window width is reflowed
    /// before pointer hit-testing and rendering on the next presentation.
    fn resolve_auto_content(&self) {
        let viewport=self.size();
        // REFCELL_LAYOUT_GUARD_V3: never take an unchecked font RefCell borrow
        // during automatic sizing, including re-entrant hit testing.
        #[cfg(feature="fonts")]
        let Some(mut font) = try_layout_font(&self.font, &self.dirty) else {
            // Retry on the next frame instead of borrowing a RefCell twice.
            return;
        };
        let mut roots=self.content_roots.borrow_mut();
        for root in roots.iter_mut() {
            let requested=root.requested_bounds;
            let max_w=viewport.0.saturating_sub(requested.x.max(0) as u32).max(1);
            let max_h=viewport.1.saturating_sub(requested.y.max(0) as u32).max(1);
            if requested.width>0 && requested.height>0 {root.bounds=requested;continue;}
            let inherited=super::style::ComputedStyle::default();
            // Explicit CSS width/height override only an auto dimension.
            let (css_width,css_height,css_min_width,css_min_height,css_max_width,css_max_height) =
                match &root.content {
                    WidgetContent::Text(t)=>Some(t.widget().style_handle().properties()),
                    WidgetContent::Label(t)=>Some(t.widget().style_handle().properties()),
                    _=>None,
                }.map(|props| {
                    use super::style::{LengthContext,Axis};
                    let ctx=LengthContext::new(viewport,viewport,16.0,16.0);
                    let len=|v:Option<super::style::Length>,axis:Axis|->Option<u32>{
                        v.and_then(|v|v.resolve(axis,ctx)).filter(|n|n.is_finite()&&*n>=0.0)
                            .map(|n|n.ceil().min(u32::MAX as f32) as u32)
                    };
                    (len(props.width,Axis::Horizontal),len(props.height,Axis::Vertical),
                     len(props.min_width,Axis::Horizontal),len(props.min_height,Axis::Vertical),
                     len(props.max_width,Axis::Horizontal),len(props.max_height,Axis::Vertical))
                }).unwrap_or((None,None,None,None,None,None));
            let mut font_ref:Option<&mut super::FontRenderer>={
                #[cfg(feature="fonts")]
                {font.as_mut()}
                #[cfg(not(feature="fonts"))]
                {None}
            };
            let available=requested.width.max(css_width.unwrap_or(0)).max(1);
            let initial=if requested.width==0&&css_width.is_none(){max_w}else{available};
            let (measured_w,_) =root.content.measure_at(initial,&inherited,&mut font_ref,32);
            let width=if requested.width>0 {requested.width}else {
                let mut w=css_width.unwrap_or(measured_w);
                w=w.max(css_min_width.unwrap_or(1)).min(css_max_width.unwrap_or(max_w)).min(max_w);
                w.max(1)
            };
            let (_,natural_h)=root.content.measure_at(width,&inherited,&mut font_ref,32);
            let height=if requested.height>0 {requested.height}else {
                css_height.unwrap_or(natural_h).max(css_min_height.unwrap_or(1))
                    .min(css_max_height.unwrap_or(max_h)).min(max_h).max(1)
            };
            root.bounds=super::Rect::new(requested.x,requested.y,width,height);
            if root.auto_overflow {
                root.scroll_extent=(natural_h>height).then_some(natural_h);
                root.scroll_y=root.scroll_y.clamp(0,
                    natural_h.saturating_sub(height).min(i32::MAX as u32) as i32);
            }
        }
        // Live catalog text widgets use the same auto-size policy when their
        // rectangle omits either dimension. Non-text controls remain unchanged.
        let mut catalog=self.catalog_roots.borrow_mut();
        for root in catalog.iter_mut() {
            let wanted=root.requested_bounds;
            if wanted.width>0 && wanted.height>0 {root.bounds=wanted;continue}
            let Some(content)=root.widget.renderable_content() else {continue};
            if !matches!(content,WidgetContent::Text(_)|WidgetContent::Label(_)){continue}
            let max_w=viewport.0.saturating_sub(wanted.x.max(0) as u32).max(1);
            let max_h=viewport.1.saturating_sub(wanted.y.max(0) as u32).max(1);
            let inherited=super::style::ComputedStyle::default();
            let mut font_ref:Option<&mut super::FontRenderer>={
                #[cfg(feature="fonts")]
                {font.as_mut()}
                #[cfg(not(feature="fonts"))]
                {None}
            };
            use super::style::{Axis,LengthContext};
            let props=root.widget.widget().style_handle().properties();
            let ctx=LengthContext::new(viewport,viewport,16.0,16.0);
            let resolve=|value:Option<super::style::Length>,axis:Axis|->Option<u32>{
                value.and_then(|v|v.resolve(axis,ctx)).filter(|n|n.is_finite()&&*n>=0.0)
                    .map(|n|n.ceil().min(u32::MAX as f32) as u32)
            };
            let width=if wanted.width>0{wanted.width}else{
                resolve(props.width,Axis::Horizontal)
                    .unwrap_or_else(||content.measure_at(max_w,&inherited,&mut font_ref,32).0)
                    .max(resolve(props.min_width,Axis::Horizontal).unwrap_or(1))
                    .min(resolve(props.max_width,Axis::Horizontal).unwrap_or(max_w))
                    .min(max_w).max(1)
            };
            let height=if wanted.height>0{wanted.height}else{
                resolve(props.height,Axis::Vertical)
                    .unwrap_or_else(||content.measure_at(width,&inherited,&mut font_ref,32).1)
                    .max(resolve(props.min_height,Axis::Vertical).unwrap_or(1))
                    .min(resolve(props.max_height,Axis::Vertical).unwrap_or(max_h))
                    .min(max_h).max(1)
            };
            root.bounds=super::Rect::new(wanted.x,wanted.y,width,height);
        }
    }

    /// Append a scrollable, clipped content viewport. The extent is its
    /// logical document height; the viewport is the supplied rectangle.
    pub fn add_scroll_content(&self,bounds:super::Rect,
        child:impl Into<WidgetContent>,content_height:u32)->WidgetId {
        let id=self.add_content(bounds,child);
        if let Some(root)=self.content_roots.borrow_mut().iter_mut().find(|r|r.id==id){
            root.auto_overflow=false;
            root.scroll_extent=Some(content_height.max(bounds.height));
        }
        self.request_redraw();id
    }
    /// Sensitivity multiplier applied to mouse-wheel scrolling (default: 0.6).
    /// A value of 1.0 moves 48 logical pixels per standard wheel notch,
    /// 0.5 moves 24 pixels, and 0.0 disables wheel movement. Wheel events
    /// reported to the application retain their original device deltas.
    /// This setting affects all scrollable content unless overridden per view.
    pub fn scroll_sensitivity(&self) -> f32 { self.scroll_sensitivity.get() }

    pub fn set_scroll_sensitivity(&self, sensitivity: f32) -> Result<(), WidgetError> {
        validate_scroll_sensitivity(sensitivity)?;
        self.scroll_sensitivity.set(sensitivity);
        Ok(())
    }

    /// Override the wheel sensitivity of one scrollable content root.
    /// Returns `false` for a missing ID or a non-scrollable root.
    /// `None` restores the window-wide sensitivity.
    pub fn set_content_scroll_sensitivity(
        &self, id: WidgetId, sensitivity: Option<f32>,
    ) -> Result<bool, WidgetError> {
        if let Some(value) = sensitivity { validate_scroll_sensitivity(value)?; }
        let mut roots = self.content_roots.borrow_mut();
        let Some(root) = roots.iter_mut().find(|r| r.id == id && r.scroll_extent.is_some()) else {
            return Ok(false);
        };
        root.scroll_sensitivity = sensitivity;
        root.scroll_remainder = 0.0;
        Ok(true)
    }

    pub fn content_scroll_sensitivity(&self, id: WidgetId) -> Option<f32> {
        self.content_roots.borrow().iter()
            .find(|r| r.id == id && r.scroll_extent.is_some())
            .map(|r| r.scroll_sensitivity.unwrap_or(self.scroll_sensitivity.get()))
    }

    /// Scroll to a logical pixel offset, clamped to the remaining content.
    pub fn set_scroll_offset(&self,id:WidgetId,offset:i32)->bool {
        let mut roots=self.content_roots.borrow_mut();
        let Some(root)=roots.iter_mut().find(|r|r.id==id) else{return false};
        let Some(extent)=root.scroll_extent else{return false};
        root.scroll_y=offset.clamp(0,extent.saturating_sub(root.bounds.height) as i32);
        root.scroll_remainder=0.0;
        drop(roots);self.request_redraw();true
    }
    pub fn scroll_offset(&self,id:WidgetId)->Option<i32> {
        self.content_roots.borrow().iter().find(|r|r.id==id && r.scroll_extent.is_some()).map(|r|r.scroll_y)
    }

    /// Registers a live, owned catalog widget. Rendering always reads its current
    /// state, and mouse clicks are dispatched to `WidgetExt::pointer_activate`.
    /// The caller can also retain a clone of the `Rc` for programmatic updates.
    pub fn add_catalog_widget(&self, bounds: super::Rect, widget: Rc<dyn super::catalog::WidgetExt>) -> WidgetId {
        widget.widget().attach_dirty(&self.dirty);
        let id = widget.id();
        self.catalog_roots.borrow_mut().push(CatalogRoot { id, requested_bounds:bounds, bounds, widget });
        self.request_redraw();
        id
    }

    /// Associate a software context menu with a control or catalog widget.
    pub fn set_context_menu(&self, id: WidgetId, items: Vec<super::interaction::MenuItem>) {
        self.context_menus.borrow_mut().insert(id, items);
    }
    /// Display a context menu at a window-client coordinate (clamped to viewport).
    pub fn show_context_menu(&self, owner: WidgetId, x: i32, y: i32,
        items: Vec<super::interaction::MenuItem>) {
        let size=self.size();
        *self.popup.borrow_mut()=Some(super::interaction::ContextMenu::new(owner,x,y,size,items));
        self.request_redraw();
    }
    pub fn dismiss_context_menu(&self) {
        if self.popup.borrow_mut().take().is_some() {self.request_redraw();}
    }
    /// Store semantic metadata for a widget. This is an in-process accessibility
    /// tree; native AT-SPI/UIA exposure remains a separate integration task.
    pub fn set_accessibility(&self, id: WidgetId, node: super::interaction::AccessibilityNode) {
        self.accessibility.borrow_mut().insert(id,node);
    }
    pub fn accessibility_snapshot(&self) -> Vec<(WidgetId,super::interaction::AccessibilityNode)> {
        use super::interaction::{AccessibilityNode,AccessibilityRole};
        let registered=self.accessibility.borrow().clone();
        let mut snapshot=Vec::new();
        for c in self.controls.borrow().iter() {
            if !c.alive_visible() {continue;}
            let id=c.id();
            let fallback=match c {
                ControlEntry::Button{state,..}=>AccessibilityNode::new(AccessibilityRole::Button,
                    state.lock().unwrap().label.clone()),
                ControlEntry::TextField{..}=>AccessibilityNode::new(AccessibilityRole::TextField,"Text input"),
            };
            snapshot.push((id,registered.get(&id).cloned().unwrap_or(fallback)));
        }
        for r in self.catalog_roots.borrow().iter() {
            if !r.widget.widget().visible(){continue;}
            let id=r.id;
            let fallback=AccessibilityNode::new(AccessibilityRole::Region,"Catalog control");
            snapshot.push((id,registered.get(&id).cloned().unwrap_or(fallback)));
        }
        snapshot
    }
    /// Apply a semantic color scheme to standard software controls.
    /// Custom per-widget color properties should be reapplied afterwards.
    pub fn set_theme(&self, theme: super::interaction::Theme) {
        self.set_background(theme.window);
        for c in self.controls.borrow().iter() {
            let is_text=c.is_text();
            c.style_handle().update_properties(|s| {
                s.background_color=Some(if is_text {theme.input} else {theme.surface});
                s.foreground_color=Some(theme.text);
                s.border_color=Some(theme.border);
            });
        }
        self.request_redraw();
    }
    /// Supply preedit text from an OS input method. The visible composition
    /// belongs to the focused editor and is not committed to the text buffer.
    pub fn update_ime_preedit(&self,text:&str,out:&mut Vec<WidgetEvent>) {
        let Some(id)=*self.focus.lock().unwrap() else{return};
        let Some(state)=self.text_state(id) else{return};
        state.lock().unwrap().preedit=text.to_owned();
        out.push(WidgetEvent::ImePreeditChanged(id,text.to_owned()));
        self.request_redraw();
    }
    /// Commit a Unicode string atomically. Native preedit protocol integration
    /// still needs a platform backend to call these two APIs.
    pub fn commit_text(&self,text:&str,out:&mut Vec<WidgetEvent>) {
        let Some(id)=*self.focus.lock().unwrap() else{return};
        let Some(state)=self.text_state(id) else{return};
        let mut s=state.lock().unwrap();
        if !s.enabled{return;}
        s.preedit.clear();
        let text=text.replace('\r'," ").replace('\n'," ").replace('\t'," ");
        if let Some((a,b))=s.selection.take() {
            if s.text.get(a..b).is_some() {s.text.replace_range(a..b,"");s.cursor=a;}
        }
        let cursor=s.cursor.min(s.text.len());
        if !s.text.is_char_boundary(cursor){return;}
        s.text.insert_str(cursor,&text);s.cursor=cursor+text.len();
        out.push(WidgetEvent::TextChanged(id));
        self.request_redraw();
    }

    /// Measure using the same font, weight, and glyph advances as native text
    /// painting. Browser layout should not guess widths from another shaper.
    #[cfg(feature = "fonts")]
    pub fn measure_text_for_style(
        &self,
        text: &str,
        style: &super::style::FontProperties,
    ) -> Option<(u32, u32)> {
        use super::style::{Axis, LengthContext};
        let viewport = self.size();
        let context = LengthContext::new(viewport, viewport, 16.0, 16.0);
        let size = style.size.resolve(Axis::Vertical, context)?;
        let pt = size.round().clamp(1.0, u16::MAX as f32) as u16;
        let line = style.line_height.and_then(|value|value.resolve(Axis::Vertical,context))
            .map(|value|value.round().max(1.0) as u32);
        let mut default_font = self.font.borrow_mut();
        let mut cache = self.font_cache.borrow_mut();
        let font = font_for_style(&mut *default_font, &mut *cache, style)?;
        font.measure_styled(text, pt, style.variant, style.weight, style.stretch, line).ok()
    }

    pub fn set_catalog_bounds(&self, id: WidgetId, bounds: super::Rect) -> bool {
        if let Some(root) = self.catalog_roots.borrow_mut().iter_mut().find(|root| root.id == id) {
            root.bounds = bounds;
            root.requested_bounds = bounds;
            self.request_redraw();
            true
        } else { false }
    }

    /// Unregisters an interactive Button or TextField.
    ///
    /// Unlike `remove_content`, this operates on the native control registry.
    /// Clearing focus and relative-layout references prevents stale widgets from
    /// affecting future input and CSS positioning after a DOM rebuild.
    pub fn remove_control(&self, id: WidgetId) -> bool {
        let mut controls = self.controls.borrow_mut();
        let old = controls.len();
        controls.retain(|control| control.id() != id);
        let changed = old != controls.len();
        if changed {
            sync_tree_states(&controls);
            drop(controls);
            let mut focus = self.focus.lock().unwrap();
            if *focus == Some(id) { *focus = None; }
            drop(focus);
            self.style_references.borrow_mut().remove(&id);
            self.accessibility.borrow_mut().remove(&id);
            self.context_menus.borrow_mut().remove(&id);
            self.request_redraw();
        }
        changed
    }

    /// Removes a widget by ID, regardless of which Mata UI widget registry
    /// owns it (controls, live catalog widgets, or standalone content).
    ///
    /// In particular, `remove_content` alone cannot remove Buttons/TextFields.
    pub fn remove_widget(&self, id: WidgetId) -> bool {
        let control = self.remove_control(id);
        let catalog = self.remove_catalog_widget(id);
        let content = self.remove_content(id);
        control || catalog || content
    }

    pub fn remove_catalog_widget(&self, id: WidgetId) -> bool {
        let mut roots = self.catalog_roots.borrow_mut();
        let old = roots.len();
        roots.retain(|root| root.id != id);
        let changed = old != roots.len();
        drop(roots);
        if self.catalog_pressed.get() == Some(id) { self.catalog_pressed.set(None); }
        let clear_focus=self.selection_focus.borrow().as_ref().is_some_and(|t|t.widget().id()==id);
        let clear_drag=self.selection_drag.borrow().as_ref().is_some_and(|h|h.text.widget().id()==id);
        if clear_focus {self.selection_focus.borrow_mut().take();}
        if clear_drag {self.selection_drag.borrow_mut().take();}
        if changed {
            self.accessibility.borrow_mut().remove(&id);
            self.context_menus.borrow_mut().remove(&id);
            let focused=*self.focus.lock().unwrap()==Some(id);
            if focused {self.clear_focus();}
            self.request_redraw();
        }
        changed
    }

    /// Repositions a standalone content root.
    pub fn set_content_bounds(&self, id: WidgetId, bounds: super::Rect) -> bool {
        let mut roots = self.content_roots.borrow_mut();
        if let Some(root) = roots.iter_mut().find(|r| r.id == id) {
            root.bounds = bounds;
            root.requested_bounds = bounds;
            root.auto_overflow = bounds.height == 0;
            drop(roots);
            self.request_redraw();
            true
        } else { false }
    }

    /// Unregisters a standalone content root.
    pub fn remove_content(&self, id: WidgetId) -> bool {
        let mut roots = self.content_roots.borrow_mut();
        let before = roots.len();
        roots.retain(|r| r.id != id);
        let changed = roots.len() != before;
        drop(roots);
        if changed { self.request_redraw(); }
        changed
    }

    /// Selected read-only text, if any. Unlike TextField selection, it is
    /// controlled by explicitly calling Text::set_selectable(true).
    pub fn selected_text(&self) -> Option<String> {
        self.selection_focus.borrow().as_ref().and_then(|text| text.selected_text())
    }

    /// Copy the highlighted, opt-in text to the system clipboard.
    pub fn copy_selection(&self) -> bool {
        let Some(value)=self.selected_text() else {return false};
        super::clipboard::copy_text(&value).is_ok()
    }

    /// Find the topmost selectable native text content under this pointer.
    fn selectable_at(&self,x:i32,y:i32)->Option<SelectableHit> {
        // hit_test() synchronizes layout, which measures auto-sized content
        // using the same RefCell<FontRenderer>. Perform layout/hit-testing
        // before taking the font's mutable borrow for selectable text.
        // Otherwise, resolve_auto_content() panics on a nested borrow.
        let control = self.hit_test(x, y);
        #[cfg(feature="fonts")]
        let mut renderer = try_layout_font(&self.font, &self.dirty)?;
        // Controls render above catalogs, which render above plain content.
        if let Some(control)=control {
            if let ControlEntry::Button{state,..}=control {
                let s=state.lock().unwrap();
                return s.content.selectable_at(s.computed.content_bounds,&s.computed,x,y,
                    {
                        #[cfg(feature="fonts")]
                        { renderer.as_mut() }
                        #[cfg(not(feature="fonts"))]
                        { None }
                    });
            }
            return None; // editable TextField owns its separate selection model
        }
        for root in self.catalog_roots.borrow().iter().rev() {
            if !root.widget.widget().visible() || !root.widget.widget().sensitive() || !root.bounds.contains(x,y){continue;}
            if let Some(content)=root.widget.renderable_content() {
                let mut inherited=super::style::ComputedStyle::default();
                inherited.bounds=root.bounds;inherited.content_bounds=root.bounds;
                if let Some(hit)=content.selectable_at(root.bounds,&inherited,x,y,
                    {
                        #[cfg(feature="fonts")]
                        { renderer.as_mut() }
                        #[cfg(not(feature="fonts"))]
                        { None }
                    }){return Some(hit)}
            }
            return None;
        }
        for root in self.content_roots.borrow().iter().rev() {
            if !root.bounds.contains(x,y){continue;}
            let mut inherited=super::style::ComputedStyle::default();
            inherited.bounds=root.bounds;inherited.content_bounds=root.bounds;
            let bounds=if let Some(extent)=root.scroll_extent {
                super::Rect::new(root.bounds.x,root.bounds.y-root.scroll_y,root.bounds.width,extent)
            } else {root.bounds};
            if let Some(hit)=root.content.selectable_at(bounds,&inherited,x,y,
                {
                        #[cfg(feature="fonts")]
                        { renderer.as_mut() }
                        #[cfg(not(feature="fonts"))]
                        { None }
                    }){return Some(hit)}
            return None;
        }
        None
    }

    fn selection_byte_at(&self,hit:&SelectableHit,x:i32,y:i32)->usize {
        let text=hit.text.text();
        let widget=hit.text.widget();
        #[cfg(feature="fonts")]
        {
            let mut computed=widget.style_handle().computed(hit.bounds,self.size(),Some(hit.bounds));
            let props=widget.style_handle().properties();
            if props.font.family.is_none(){computed.font.family=hit.inherited.font.family.clone();}
            if props.font.size.is_none(){computed.font_size_px=hit.inherited.font_size_px;}
            let mut default=self.font.borrow_mut();
            let mut cache=self.font_cache.borrow_mut();
            let font=font_for_style(&mut *default,&mut *cache,&computed.font);
            // Reborrow the same font twice so rasterization and picking agree.
            let mut font=font;
            let layout=super::text_selection::layout(&text,widget,hit.bounds,&hit.inherited,self.size(),font.as_deref_mut());
            return super::text_selection::byte_at(&text,&layout,x,y,font.as_deref_mut());
        }
        #[cfg(not(feature="fonts"))]
        {
            let layout=super::text_selection::layout(&text,widget,hit.bounds,&hit.inherited,self.size(),None);
            super::text_selection::byte_at(&text,&layout,x,y,None)
        }
    }

    /// Returns true if the pointer began selecting enabled, read-only text.
    fn begin_selection(&self,x:i32,y:i32,out:&mut Vec<WidgetEvent>)->bool {
        let Some(hit)=self.selectable_at(x,y) else {
            if let Some(old)=self.selection_focus.borrow_mut().take(){old.clear_selection()}
            self.selection_drag.borrow_mut().take();
            return false;
        };
        let index=self.selection_byte_at(&hit,x,y);
        if let Some(old)=self.selection_focus.borrow_mut().replace(hit.text.clone()) {
            if old.widget().id()!=hit.text.widget().id(){old.clear_selection();}
        }
        hit.text.set_selection(index,index);
        // Static text is not a TextField; don't keep keyboard focus on an old editor.
        let had_focus=self.focus.lock().unwrap().take().is_some();
        if had_focus {self.request_redraw();}
        out.push(WidgetEvent::TextSelectionChanged(hit.text.widget().id()));
        *self.selection_drag.borrow_mut()=Some(hit);
        true
    }

    fn drag_selection(&self,x:i32,y:i32,out:&mut Vec<WidgetEvent>) {
        let hit=self.selection_drag.borrow().clone();
        let Some(hit)=hit else{return};
        let index=self.selection_byte_at(&hit,x,y);
        let anchor=hit.text.selection_anchor();
        if hit.text.selection_caret()!=index {
            hit.text.set_selection(anchor,index);
            out.push(WidgetEvent::TextSelectionChanged(hit.text.widget().id()));
        }
    }

    /// Target for a right click (even if its text was not made selectable).
    fn right_click_target(&self,x:i32,y:i32)->Option<WidgetId>{
        self.selectable_at(x,y).map(|hit|hit.text.widget().id())
            .or_else(||self.hit_test(x,y).map(|c|c.id()))
            .or_else(||self.catalog_roots.borrow().iter().rev().find(|r|r.bounds.contains(x,y)).map(|r|r.id))
            .or_else(||self.content_roots.borrow().iter().rev().find(|r|r.bounds.contains(x,y)).map(|r|r.id))
    }

    /// Performs the render operation.
    ///
    /// Visibility: **public**.
    #[doc = "Implements the render operation. Visibility: public."]
    pub fn render(&self) {
        #[cfg(feature="fonts")]
        if self.font.try_borrow_mut().is_err() {
            // Do not clear the framebuffer or publish an incomplete layout.
            self.request_redraw();
            return;
        }
        self.resolve_auto_content();
        self.resolve_control_styles();
        let mut fb = self.framebuffer.borrow_mut();
        fb.clear(self.background.get());
        #[cfg(feature = "video")]
        for video in self.videos.borrow().iter() {
            video.blit(&mut fb);
        }
        let controls = self.controls.borrow();
        let mut order = self.render_order.borrow_mut();
        order.clear();
        order.extend(0..controls.len());
        order.sort_unstable_by_key(|index| controls[*index].z_index());
        let focused = *self.focus.lock().unwrap();
        #[cfg(feature = "fonts")]
        let mut default_font = self.font.borrow_mut();
        #[cfg(feature = "fonts")]
        let mut font_cache = self.font_cache.borrow_mut();
        for root in self.content_roots.borrow().iter() {
            let mut inherited = super::style::ComputedStyle::default();
            inherited.bounds = root.bounds;
            inherited.content_bounds = root.bounds;
            if let Some(extent)=root.scroll_extent {
                let mut scratch=Framebuffer::from_color(root.bounds.width.max(1),root.bounds.height.max(1),
                    self.background.get());
                let rect=super::Rect::new(0,-root.scroll_y,root.bounds.width,extent);
                let mut local=super::style::ComputedStyle::default();
                local.bounds=rect;local.content_bounds=rect;
                let mut clipped=ContentPainter{fb:&mut scratch,
                    #[cfg(feature="fonts")] font:&mut default_font,
                    #[cfg(feature="fonts")] cache:&mut font_cache};
                clipped.paint(&root.content,rect,&local);
                // Copy the viewport, never pixels outside its rectangle.
                for yy in 0..root.bounds.height {
                    for xx in 0..root.bounds.width {
                        let x=root.bounds.x+xx as i32;let y=root.bounds.y+yy as i32;
                        if x>=0 && y>=0 && x<fb.width as i32 && y<fb.height as i32 {
                            let index = y as usize * fb.width as usize + x as usize;
                            fb.pixels[index] =
                                scratch.pixels[yy as usize*scratch.width as usize+xx as usize];
                        }
                    }
                }
            } else {
                let mut painter=ContentPainter{fb:&mut fb,
                    #[cfg(feature="fonts")] font:&mut default_font,
                    #[cfg(feature="fonts")] cache:&mut font_cache};
                painter.paint(&root.content,root.bounds,&inherited);
            }
        }
        // Dynamic catalog widgets are painted from their current Rust state, not
        // the state captured when a gallery view was originally constructed.
        for root in self.catalog_roots.borrow().iter() {
            let mut inherited = super::style::ComputedStyle::default();
            inherited.bounds = root.bounds;
            inherited.content_bounds = root.bounds;
            if let Some(node)=root.widget.renderable_content() {
                let handled=match &node {
                    WidgetContent::Painted(item)=>super::interaction::paint_live_catalog(&mut fb,root.bounds,
                        item.kind,&item.state),
                    _=>false,
                };
                if handled {
                    if let WidgetContent::Painted(item)=&node {
                        if let Some(text)=item.state.text.as_ref() {
                            let x=if item.kind=="CheckButton" {root.bounds.x+36} else {root.bounds.x+10};
                            let rect=super::Rect::new(x,root.bounds.y+5,
                                root.bounds.width.saturating_sub((x-root.bounds.x) as u32),root.bounds.height.saturating_sub(10));
                            let label=super::catalog::Text::new(text.clone());
                            let mut painter=ContentPainter{fb:&mut fb,
                                #[cfg(feature="fonts")] font:&mut default_font,
                                #[cfg(feature="fonts")] cache:&mut font_cache};
                            painter.paint(&WidgetContent::Text(label),rect,&inherited);
                        }
                    }
                } else {
                    let mut painter=ContentPainter{fb:&mut fb,
                        #[cfg(feature="fonts")] font:&mut default_font,
                        #[cfg(feature="fonts")] cache:&mut font_cache};
                    painter.paint(&node,root.bounds,&inherited);
                }
            }
        }
        for &index in order.iter() {
            let control_id=controls[index].id();
            let saved=self.scroll_clips.borrow().get(&control_id).copied().map(|clip|
                super::interaction::capture_outside_clip(&fb,controls[index].bounds(),clip));
            match &controls[index] {
                ControlEntry::Button { state, .. } => {
                    let s = state.lock().unwrap();
                    if s.alive && s.visible {
                        draw_button(&mut fb, &s);
                        let area = s.computed.content_bounds;
                        let mut painter = ContentPainter {
                            fb: &mut fb,
                            #[cfg(feature = "fonts")]
                            font: &mut default_font,
                            #[cfg(feature = "fonts")]
                            cache: &mut font_cache,
                        };
                        if s.enabled {
                            painter.paint(&s.content, area, &s.computed);
                        } else {
                            let mut disabled = s.computed.clone();
                            disabled.opacity *= 0.65;
                            painter.paint(&s.content, area, &disabled);
                        }
                    }
                }
                ControlEntry::TextField { id, state } => {
                    let s = state.lock().unwrap();
                    if s.alive && s.visible {
                        #[cfg(feature = "fonts")]
                        let font = font_for_style(
                            &mut default_font,
                            &mut font_cache,
                            &s.computed.font
                        );
                        #[cfg(not(feature = "fonts"))]
                        let font = None;
                        draw_text_field(&mut fb, font, &s, focused == Some(*id));
                    }
                }
            }
            if let Some(saved)=saved {super::interaction::restore_pixels(&mut fb,saved);}
        }
        // Strong focus-visible ring for both buttons and catalog controls.
        if let Some(id)=focused {
            if let Some(c)=controls.iter().find(|c|c.id()==id && c.focusable()) {
                super::interaction::outline(&mut fb,c.bounds(),0xff89b9ff);
            } else if let Some(r)=self.catalog_roots.borrow().iter().find(|r|r.id==id) {
                super::interaction::outline(&mut fb,r.bounds,0xff89b9ff);
            }
        }
        #[cfg(feature = "webview-servo")]
        for entry in self.webviews.borrow().iter() {
            if let Some(inner) = entry.inner.upgrade() {inner.blit(&mut fb, inner.bounds());}
        }
        // A software popup always renders last, above ordinary controls.
        if let Some(menu)=self.popup.borrow().as_ref() {
            let menu=menu.clone();
            for (i,item) in menu.items.iter().enumerate() {
                let y=menu.y.saturating_add((i as i32)*menu.row_height as i32);
                let rect=super::Rect::new(menu.x,y,menu.width,menu.row_height);
                super::interaction::fill_rect(&mut fb,rect,
                    if i==menu.selected {0xff395c9d} else {0xff333943});
                let label=super::catalog::Text::new(item.label.clone());
                let mut inherited=super::style::ComputedStyle::default();
                inherited.foreground_color=Some(if item.enabled {0xfff5f7fa} else {0xff999da7});
                inherited.font_size_px=14.0;
                inherited.bounds=rect;inherited.content_bounds=rect;
                let area=super::Rect::new(rect.x+10,rect.y+6,rect.width.saturating_sub(20),rect.height.saturating_sub(10));
                let mut painter=ContentPainter{fb:&mut fb,
                    #[cfg(feature="fonts")] font:&mut default_font,
                    #[cfg(feature="fonts")] cache:&mut font_cache};
                painter.paint(&super::WidgetContent::Text(label),area,&inherited);
            }
            super::interaction::outline(&mut fb,super::Rect::new(menu.x,menu.y,menu.width,
                (menu.items.len() as u32).saturating_mul(menu.row_height)),0xff8e98a9);
        }
    }

    fn sync_interactive_content(&self) {
        self.resolve_auto_content();
        let inherited=super::style::ComputedStyle::default();
        #[cfg(feature="fonts")]
        let Some(mut renderer) = try_layout_font(&self.font, &self.dirty) else {
            return;
        };
        let mut placements=Vec::new();
        let mut clip_map=HashMap::new();
        for root in self.content_roots.borrow().iter(){
            let initial_count=placements.len();
            let bounds=if root.scroll_extent.is_some() {
                super::Rect::new(root.bounds.x,root.bounds.y-root.scroll_y,
                    root.bounds.width,root.scroll_extent.unwrap_or(root.bounds.height))
            } else {root.bounds};
            root.content.collect_interactive(bounds,&inherited,&mut placements,32,{
                #[cfg(feature="fonts")]
                {renderer.as_mut()}
                #[cfg(not(feature="fonts"))]
                {None}
            });
            if root.scroll_extent.is_some() {
                for (id,_) in &placements[initial_count..] {clip_map.insert(*id,root.bounds);}
            }
        }
        *self.scroll_clips.borrow_mut()=clip_map;
        let controls=self.controls.borrow();
        // Buttons themselves can contain other interactive live controls.
        for c in controls.iter(){
            if let ControlEntry::Button{state,..}=c {
                let s=state.lock().unwrap();
                s.content.collect_interactive(s.computed.content_bounds,&s.computed,&mut placements,32,{
                    #[cfg(feature="fonts")]
                    {renderer.as_mut()}
                    #[cfg(not(feature="fonts"))]
                    {None}
                });
            }
        }
        for (id,bounds) in placements {
            if let Some(c)=controls.iter().find(|c|c.id()==id) {
                match c {
                    ControlEntry::Button{state,..}=>{
                        let mut s=state.lock().unwrap();s.bounds=bounds;s.layout_base=bounds;
                    }
                    ControlEntry::TextField{state,..}=>{
                        let mut s=state.lock().unwrap();s.bounds=bounds;s.layout_base=bounds;
                    }
                }
            }
        }
    }

    /// Resolves control styles from the current state.
    ///
    /// Visibility: **private**.
    #[doc = "Resolves control styles. Visibility: private/internal."]
    fn resolve_control_styles(&self) {
        self.sync_interactive_content();
        let controls = self.controls.borrow();
        let size = self.size();
        let focused = *self.focus.lock().unwrap();
        let mut references = self.style_references.borrow_mut();
        references.clear();
        let additional = controls.len().saturating_sub(references.capacity());
        references.reserve(additional);
        for c in controls.iter() {
            references.insert(c.id(), c.bounds());
        }
        // Two passes allow a widget to position itself relative to a sibling
        // regardless of which sibling was registered first.
        for _ in 0..2 {
            for c in controls.iter() {
                c.resolve_style(size, &references, focused);
                references.insert(c.id(), c.bounds());
            }
        }
    }

    /// Performs the hit test operation.
    ///
    /// Visibility: **private**.
    #[doc = "Implements the hit test operation. Visibility: private/internal."]
    fn hit_test(&self, x: i32, y: i32) -> Option<ControlEntry> {
        self.resolve_control_styles();
        self.controls
            .borrow()
            .iter()
            .filter(|c| c.alive_visible() && c.bounds().contains(x, y)
                && self.scroll_clips.borrow().get(&c.id()).is_none_or(|clip|clip.contains(x,y)))
            .max_by_key(|c| c.z_index())
            .cloned()
    }

    /// Performs the handle native operation.
    ///
    /// Visibility: **private**.
    #[doc = "Implements the handle native operation. Visibility: private/internal."]
    fn handle_native(&self, event: WindowEvent, out: &mut Vec<WidgetEvent>) {
        // An open popup owns keyboard and pointer events until dismissed.
        if self.route_menu(event,out) {return;}
        match event {
            WindowEvent::KeyPressed(key) => out.push(WidgetEvent::KeyPressed(key)),
            WindowEvent::KeyReleased(key) => out.push(WidgetEvent::KeyReleased(key)),
            _ => {},
        }
        match event {
            WindowEvent::Close => out.push(WidgetEvent::Close),
            | WindowEvent::Resized(w, h)
            | WindowEvent::Maximized(w, h)
            | WindowEvent::Restored(w, h) => {
                let size = self.resize_event_size((w, h));
                if self.host_size.get() != size {
                    self.resize_render_surface(size);
                }
                out.push(WidgetEvent::Resized(size.0, size.1));
            }
            WindowEvent::CursorMoved(x, y) => {
                // WidgetWindow's framebuffer, layout and Windows client area are
                // intentionally 1:1. Keep the native client coordinates exactly
                // as delivered instead of applying another resize/DPI transform.
                self.sync_native_size();
                self.cursor.set((x, y));
                self.update_hover(x, y);
                self.drag_selection(x,y,out);
                self.drag_field_selection(x);
                out.push(WidgetEvent::PointerMoved(x, y));
            }
            WindowEvent::KeyPressed(Key::MouseLeft) => {
                let p = self.cursor.get();
                if !self.begin_selection(p.0,p.1,out) { self.pointer_down(p.0, p.1); }
                out.push(WidgetEvent::PointerPressed(p.0, p.1));
            }
            WindowEvent::KeyReleased(Key::MouseLeft) => {
                let p = self.cursor.get();
                let dragging=self.selection_drag.borrow().is_some();
                if dragging {
                    self.drag_selection(p.0,p.1,out);
                    self.selection_drag.borrow_mut().take();
                } else { self.pointer_up(p.0, p.1, out); }
                out.push(WidgetEvent::PointerReleased(p.0, p.1));
            }
            WindowEvent::KeyPressed(Key::MouseRight) => {
                self.right_pressed.set(true);
                let (x,y)=self.cursor.get();
                out.push(WidgetEvent::PointerRightPressed(x,y));
            }
            WindowEvent::KeyReleased(Key::MouseRight) => {
                let was_down=self.right_pressed.replace(false);
                let (x,y)=self.cursor.get();
                out.push(WidgetEvent::PointerRightReleased(x,y));
                if was_down {
                    let widget=self.right_click_target(x,y);
                    if let Some(id)=widget {
                        if let Some(items)=self.context_menus.borrow().get(&id).cloned() {
                            self.show_context_menu(id,x,y,items);
                        } else if self.selection_focus.borrow().as_ref().is_some_and(|t|
                            t.widget().id()==id && t.selected_text().is_some()) {
                            self.show_context_menu(id,x,y,vec![super::interaction::MenuItem::new(
                                super::interaction::AUTO_COPY_ITEM,"Copy selected text")]);
                        }
                    }
                    out.push(WidgetEvent::ContextMenuRequested {widget,x,y});
                }
            }
            WindowEvent::Scroll(dx, dy) => {
                let (x,y)=self.cursor.get();
                let mut changed = false;
                if let Some(root) = self.content_roots.borrow_mut().iter_mut().rev().find(|r|
                    r.scroll_extent.is_some() && r.bounds.contains(x,y)) {
                    let sensitivity = root.scroll_sensitivity.unwrap_or(self.scroll_sensitivity.get());
                    let delta = wheel_pixels(dy, sensitivity, &mut root.scroll_remainder);
                    let max = root.scroll_extent.unwrap_or(0)
                        .saturating_sub(root.bounds.height).min(i32::MAX as u32) as i32;
                    let next = root.scroll_y.saturating_sub(delta).clamp(0,max);
                    changed = root.scroll_y != next;
                    root.scroll_y = next;
                    // Never accumulate wheel remainder against an immovable edge.
                    if (next == 0 && delta > 0) || (next == max && delta < 0) {
                        root.scroll_remainder = 0.0;
                    }
                }
                if changed { self.request_redraw(); }
                out.push(WidgetEvent::Scrolled { x, y, dx, dy });
            }
            WindowEvent::KeyPressed(Key::LeftCtrl) => self.left_ctrl_down.set(true),
            WindowEvent::KeyPressed(Key::RightCtrl) => self.right_ctrl_down.set(true),
            WindowEvent::KeyReleased(Key::LeftCtrl) => self.left_ctrl_down.set(false),
            WindowEvent::KeyReleased(Key::RightCtrl) => self.right_ctrl_down.set(false),
            WindowEvent::KeyReleased(Key::Backspace) => {*self.backspace_repeat.borrow_mut()=None;},
            WindowEvent::KeyPressed(Key::LeftShift) => self.left_shift_down.set(true),
            WindowEvent::KeyPressed(Key::RightShift) => self.right_shift_down.set(true),
            WindowEvent::KeyReleased(Key::LeftShift) => self.left_shift_down.set(false),
            WindowEvent::KeyReleased(Key::RightShift) => self.right_shift_down.set(false),
            WindowEvent::KeyPressed(key @ (Key::Char(_) | Key::Num(_))) => {
                if self.left_ctrl_down.get() || self.right_ctrl_down.get() {
                    if let Key::Char(ch)=key {
                        match ch.to_ascii_lowercase() {
                            'a'=>{let editor={let f=*self.focus.lock().unwrap();f.is_some_and(|id|self.is_text(id))};if editor{self.text_select_all()}else if let Some(text)=self.selection_focus.borrow().as_ref(){text.select_all();out.push(WidgetEvent::TextSelectionChanged(text.widget().id()));}},
                            'c'=>{self.text_copy();if self.focus.lock().unwrap().is_none_or(|id|!self.is_text(id)){self.copy_selection();}},
                            'x'=>self.text_cut(out),
                            'v'=>self.text_paste(out),
                            _=>{}
                        }
                    }
                } else if let Some(ch) = self.text_input_char(key) {
                    if self.focus.lock().unwrap().is_none_or(|id| !self.is_text(id)) {
                        out.push(WidgetEvent::TextInput(ch));
                    }
                    self.insert_char(ch, out)
                }
            }
            WindowEvent::KeyPressed(Key::Space) => {
                if self.focused_text_field() { self.insert_char(' ',out); }
                else if !self.activate_focused(out) {out.push(WidgetEvent::TextInput(' '));}
            },
            WindowEvent::KeyPressed(Key::Backspace) => {
                let mut repeat=self.backspace_repeat.borrow_mut();
                if repeat.is_none() {
                    let now=Instant::now();
                    *repeat=Some((now,now+Duration::from_secs(1)));
                    drop(repeat);
                    self.backspace(out);
                }
            },
            WindowEvent::KeyPressed(Key::Left) => self.move_cursor(false),
            WindowEvent::KeyPressed(Key::Right) => self.move_cursor(true),
            WindowEvent::KeyPressed(Key::Home) => self.move_cursor_to(false),
            WindowEvent::KeyPressed(Key::End) => self.move_cursor_to(true),
            WindowEvent::KeyPressed(Key::Delete) => self.delete_forward(out),
            WindowEvent::KeyPressed(Key::Enter) => {self.activate_focused(out);},
            WindowEvent::KeyPressed(Key::Tab) => self.focus_next(),
            _ => {}
        }
    }
    /// Updates hover.
    ///
    /// Visibility: **private**.
    #[doc = "Updates hover. Visibility: private/internal."]
    fn update_hover(&self, x: i32, y: i32) {
        let top = self.hit_test(x, y).map(|c| c.id());
        let mut changed = false;
        for c in self.controls.borrow().iter() {
            let hovered = top == Some(c.id());
            match c {
                ControlEntry::Button { state, .. } => {
                    let mut s = state.lock().unwrap();
                    let next = hovered && s.enabled;
                    if s.hovered != next {
                        s.hovered = next;
                        changed = true;
                    }
                }
                ControlEntry::TextField { state, .. } => {
                    let mut s = state.lock().unwrap();
                    let next = hovered && s.enabled;
                    if s.hovered != next {
                        s.hovered = next;
                        changed = true;
                    }
                }
            }
        }
        if changed {
            self.request_redraw()
        }
    }
    /// Performs the pointer down operation.
    ///
    /// Visibility: **private**.
    #[doc = "Implements the pointer down operation. Visibility: private/internal."]
    fn pointer_down(&self, x: i32, y: i32) {
        self.catalog_pressed.set(None);
        let hit = self.hit_test(x, y);
        // Software controls need keyboard focus returned from a Mata DocEdit
        // or WebView2 child HWND. Otherwise text fields receive clicks but no
        // keystrokes after typing in the document/browser.
        #[cfg(target_os = "windows")]
        if hit.is_some() {
            #[link(name = "user32")]
            unsafe extern "system" {
                fn SetFocus(hwnd: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
            }
            unsafe { SetFocus(self.native_handle()); }
        }
        match hit {
            Some(ControlEntry::Button { id, state }) => {
                let mut s = state.lock().unwrap();
                if s.enabled {
                    s.pressed = true;
                    *self.focus.lock().unwrap() = Some(id);
                    self.request_redraw()
                }
            }
            Some(ControlEntry::TextField { id, state }) => {
                let enabled = {
                    let mut s = state.lock().unwrap();
                    let enabled = s.enabled;
                    if enabled {
                        s.pressed = true;
                    }
                    enabled
                };
                if enabled {
                    *self.focus.lock().unwrap() = Some(id);
                    self.place_cursor(id, x);
                    self.request_redraw()
                }
            }
            None => {
                let catalog=self.catalog_roots.borrow().iter().rev().find(|root|
                    root.widget.widget().visible() && root.widget.widget().sensitive() &&
                    root.bounds.contains(x,y)).map(|root|root.id);
                self.catalog_pressed.set(catalog);
                let mut focus=self.focus.lock().unwrap();
                if *focus!=catalog {*focus=catalog;drop(focus);self.request_redraw();}
            }
        }
    }
    fn drag_field_selection(&self,x:i32) {
        let id=self.controls.borrow().iter().find_map(|c|match c {
            ControlEntry::TextField{id,state} if state.lock().unwrap().pressed=>Some(*id),
            _=>None,
        });
        if let Some(id)=id {self.place_cursor(id,x);}
    }

    /// Performs the pointer up operation.
    ///
    /// Visibility: **private**.
    #[doc = "Implements the pointer up operation. Visibility: private/internal."]
    fn pointer_up(&self, x: i32, y: i32, out: &mut Vec<WidgetEvent>) {
        if let Some(id) = self.catalog_pressed.replace(None) {
            if let Some(root) = self.catalog_roots.borrow().iter().find(|root| root.id == id && root.bounds.contains(x, y)) {
                // Use the visible viewport, NOT the host rectangle, when
                // translating pointer coordinates. The catalog painter keeps
                // a 320:174 aspect ratio and letterboxes tall/wide panels.
                let content_bounds = match root.widget.renderable_content() {
                    Some(WidgetContent::Painted(ref item)) if matches!(item.kind,
                        "CheckButton"|"ToggleButton"|"Switch"|"Range"|"Scale"|"ProgressBar"|"LevelBar") => root.bounds,
                    Some(WidgetContent::Painted(_)) => super::catalog::native_painter::aspect_fit(root.bounds,320,174),
                    _ => root.bounds,
                };
                if content_bounds.contains(x, y) {
                    let w = content_bounds.width.max(1) as f32;
                    let h = content_bounds.height.max(1) as f32;
                    let rel_x = ((x - content_bounds.x) as f32 / w).clamp(0.0, 1.0);
                    let rel_y = ((y - content_bounds.y) as f32 / h).clamp(0.0, 1.0);
                    if root.widget.pointer_activate(rel_x, rel_y) {
                        out.push(WidgetEvent::CatalogChanged(id));
                        self.request_redraw();
                    }
                }
            }
        }
        let mut changed = false;
        for c in self.controls.borrow().iter() {
            match c {
                ControlEntry::Button { id, state } => {
                    let mut s = state.lock().unwrap();
                    let click =
                        s.alive && s.visible && s.enabled && s.pressed && s.bounds.contains(x, y);
                    if s.pressed {
                        changed = true;
                    }
                    s.pressed = false;
                    if click {
                        out.push(WidgetEvent::ButtonClicked(*id));
                    }
                }
                ControlEntry::TextField { state, .. } => {
                    let mut s = state.lock().unwrap();
                    if s.pressed {
                        changed = true;
                    }
                    s.pressed = false;
                    s.mouse_anchor = None;
                }
            }
        }
        if changed {
            self.request_redraw()
        }
    }
    /// Performs the text state operation.
    ///
    /// Visibility: **private**.
    #[doc = "Implements the text state operation. Visibility: private/internal."]
    fn text_state(&self, id: WidgetId) -> Option<Arc<Mutex<TextFieldState>>> {
        self.controls
            .borrow()
            .iter()
            .find_map(|c| {
                match c {
                    ControlEntry::TextField { id: other, state } if *other == id =>
                        Some(state.clone()),
                    _ => None,
                }
            })
    }
    /// Currently focused interactive control, if any. An HTML adapter can
    /// keep a focused native editor's cursor stable during DOM layout refreshes.
    /// Give keyboard focus to an existing, visible native text control.
    /// Used by embeddable Mata webviews; does not synthesize mouse events.
    pub fn focus_control(&self, id: WidgetId) -> Result<(), WidgetError> {
        let valid=self.controls.borrow().iter().any(|c|c.id()==id && c.focusable())
            ||self.catalog_roots.borrow().iter().any(|r|r.id==id &&
                r.widget.widget().visible() && r.widget.widget().sensitive());
        if !valid {return Err(WidgetError("Widget is not a visible enabled focus target".into()));}
        *self.focus.lock().map_err(|e| WidgetError(e.to_string()))? = Some(id);
        self.request_redraw();
        Ok(())
    }
    /// Return keyboard focus from a child control to the parent widget host.
    pub fn clear_focus(&self) {
        if let Ok(mut focused) = self.focus.lock() { *focused = None; }
        self.request_redraw();
    }
    pub fn focused_control_id(&self)->Option<WidgetId> {
        *self.focus.lock().unwrap()
    }
    /// Returns whether a visible TextField currently owns keyboard focus.
    /// Browser hosts use this to let an address field handle Ctrl+A/C/V
    /// without accidentally selecting or copying the HTML document.
    pub fn focused_text_field(&self)->bool {
        let focus=*self.focus.lock().unwrap();
        focus.is_some_and(|id|self.is_text(id))
    }
    /// Returns whether text.
    ///
    /// Visibility: **private**.
    #[doc = "Returns whether text. Visibility: private/internal."]
    fn is_text(&self, id: WidgetId) -> bool {
        self.controls
            .borrow()
            .iter()
            .any(|c| c.id() == id && c.is_text() && c.alive_visible())
    }
    /// Converts a physical key into the character a text field should insert.
    ///
    /// The native backends expose letter, number-row, and punctuation keys in
    /// their unshifted form. Text input applies the current Shift modifier here
    /// so every backend gets the same behavior.
    #[doc = "Implements the text input char operation. Visibility: private/internal."]
    fn text_input_char(&self, key: Key) -> Option<char> {
        key_to_text_char(key, self.left_shift_down.get() || self.right_shift_down.get())
    }

    /// Inserts char.
    ///
    /// Visibility: **private**.
    #[doc = "Implements the insert char operation. Visibility: private/internal."]
    fn insert_char(&self, ch: char, out: &mut Vec<WidgetEvent>) {
        if ch.is_control() {
            return;
        }
        let Some(id) = *self.focus.lock().unwrap() else {
            return;
        };
        let Some(state) = self.text_state(id) else {
            return;
        };
        let mut s = state.lock().unwrap();
        if !s.enabled {
            return;
        }
        if let Some((a,b))=s.selection.take() {
            if s.text.get(a..b).is_some(){s.text.replace_range(a..b,"");s.cursor=a;}
        }
        let cursor = s.cursor.min(s.text.len());
        s.text.insert(cursor, ch);
        s.cursor = cursor + ch.len_utf8();
        out.push(WidgetEvent::TextChanged(id));
        self.request_redraw();
    }
    /// Select text in the focused TextField (Ctrl+A).
    fn text_select_all(&self) {
        let Some(id)=*self.focus.lock().unwrap() else{return};
        let Some(state)=self.text_state(id) else{return};
        let mut s=state.lock().unwrap();
        s.selection=Some((0,s.text.len()));s.cursor=s.text.len();
        self.request_redraw();
    }
    fn text_copy(&self) {
        let Some(id)=*self.focus.lock().unwrap() else{return};
        let Some(state)=self.text_state(id) else{return};
        let s=state.lock().unwrap();
        if let Some((a,b))=s.selection {
            if let Some(text)=s.text.get(a..b){let _=super::clipboard::copy_text(text);}
        }
    }
    fn text_cut(&self,out:&mut Vec<WidgetEvent>) {
        self.text_copy();
        let Some(id)=*self.focus.lock().unwrap() else{return};
        let Some(state)=self.text_state(id) else{return};
        let mut s=state.lock().unwrap();
        if let Some((a,b))=s.selection.take() {
            if a<b && s.text.get(a..b).is_some() {
                s.text.replace_range(a..b,"");s.cursor=a;
                out.push(WidgetEvent::TextChanged(id));self.request_redraw();
            }
        }
    }
    fn text_paste(&self,out:&mut Vec<WidgetEvent>) {
        let Ok(text)=super::clipboard::paste_text() else{return};
        let Some(id)=*self.focus.lock().unwrap() else{return};
        let Some(state)=self.text_state(id) else{return};
        let mut s=state.lock().unwrap();
        if !s.enabled{return;}
        if let Some((a,b))=s.selection.take() {
            if s.text.get(a..b).is_some(){s.text.replace_range(a..b,"");s.cursor=a;}
        }
        let cursor=s.cursor.min(s.text.len());
        // Single-line field: normalize whitespace in pasted content.
        let text=text.replace('\r'," ").replace('\n'," ").replace('\t'," ");
        s.text.insert_str(cursor,&text);s.cursor=cursor+text.len();
        out.push(WidgetEvent::TextChanged(id));self.request_redraw();
    }
    /// Performs the backspace operation.
    ///
    /// Visibility: **private**.
    #[doc = "Implements the backspace operation. Visibility: private/internal."]
    fn backspace(&self, out: &mut Vec<WidgetEvent>) {
        let Some(id) = *self.focus.lock().unwrap() else {
            return;
        };
        let Some(state) = self.text_state(id) else {
            return;
        };
        let mut s = state.lock().unwrap();
        if !s.enabled {return;}
        if let Some((a,b))=s.selection.take() {
            if a<b && s.text.get(a..b).is_some(){
                s.text.replace_range(a..b,"");s.cursor=a;
                out.push(WidgetEvent::TextChanged(id));self.request_redraw();
                return;
            }
        }
        if s.cursor==0{return;}
        let end = s.cursor.min(s.text.len());
        let start = super::interaction::prev_grapheme(&s.text, end);
        s.text.replace_range(start..end, "");
        s.cursor = start;
        out.push(WidgetEvent::TextChanged(id));
        self.request_redraw();
    }
    /// Performs the move cursor operation.
    ///
    /// Visibility: **private**.
    #[doc = "Implements the move cursor operation. Visibility: private/internal."]
    fn move_cursor(&self, right: bool) {
        let Some(id)=*self.focus.lock().unwrap() else{return};
        let Some(state)=self.text_state(id) else{return};
        let mut s=state.lock().unwrap();
        let old=s.cursor;
        let ctrl=self.left_ctrl_down.get()||self.right_ctrl_down.get();
        let new=if ctrl {super::interaction::word_boundary(&s.text,old,right)}
            else if right {super::interaction::next_grapheme(&s.text,old)}
            else {super::interaction::prev_grapheme(&s.text,old)};
        if self.left_shift_down.get()||self.right_shift_down.get() {
            s.selection=super::interaction::extend_selection(s.selection,old,new);
        } else {s.selection=None;}
        s.cursor=new;
        if new!=old {drop(s);self.request_redraw();}
    }
    /// Performs the focus next operation.
    ///
    /// Visibility: **private**.
    #[doc = "Implements the focus next operation. Visibility: private/internal."]
    fn focus_next(&self) {
        let mut ids: Vec<_> = self.controls.borrow().iter().filter_map(|c| {
            c.focusable().then_some(c.id())
        }).collect();
        ids.extend(self.catalog_roots.borrow().iter().filter_map(|r|
            (r.widget.widget().visible() && r.widget.widget().sensitive()).then_some(r.id)));
        let current=*self.focus.lock().unwrap();
        let backward=self.left_shift_down.get() || self.right_shift_down.get();
        let next=super::interaction::next_focus(&ids,current,backward);
        if current!=next {
            *self.focus.lock().unwrap()=next;
            self.request_redraw();
        }
    }

    fn activate_focused(&self, out:&mut Vec<WidgetEvent>) -> bool {
        let Some(id)=*self.focus.lock().unwrap() else {return false};
        if self.is_text(id) {out.push(WidgetEvent::TextFieldActivated(id));return true;}
        for c in self.controls.borrow().iter() {
            if c.id()==id && c.focusable() {
                if let ControlEntry::Button{..}=c {
                    out.push(WidgetEvent::ButtonClicked(id));
                    return true;
                }
            }
        }
        if let Some(r)=self.catalog_roots.borrow().iter().find(|r|
            r.id==id && r.widget.widget().visible() && r.widget.widget().sensitive()) {
            if r.widget.pointer_activate(0.5,0.5) {
                out.push(WidgetEvent::CatalogChanged(id));
                self.request_redraw();
            }
            return true;
        }
        false
    }
    fn move_cursor_to(&self,end:bool) {
        let Some(id)=*self.focus.lock().unwrap() else{return};
        let Some(state)=self.text_state(id) else{return};
        let mut s=state.lock().unwrap();
        let target=if end{s.text.len()}else{0};
        let shift=self.left_shift_down.get()||self.right_shift_down.get();
        let old=s.cursor;
        if shift {s.selection=super::interaction::extend_selection(s.selection,old,target)}
        else {s.selection=None;}
        s.cursor=target;
        drop(s);self.request_redraw();
    }
    fn delete_forward(&self,out:&mut Vec<WidgetEvent>) {
        let Some(id)=*self.focus.lock().unwrap() else{return};
        let Some(state)=self.text_state(id) else{return};
        let mut s=state.lock().unwrap();
        if !s.enabled{return;}
        let (a,b)=s.selection.take().filter(|(a,b)|a<b)
            .unwrap_or_else(|| (s.cursor,super::interaction::next_grapheme(&s.text,s.cursor)));
        if a<b && s.text.get(a..b).is_some() {
            s.text.replace_range(a..b,"");s.cursor=a;
            out.push(WidgetEvent::TextChanged(id));self.request_redraw();
        }
    }
    fn route_menu(&self,event:WindowEvent,out:&mut Vec<WidgetEvent>)->bool {
        let mut popup=self.popup.borrow_mut();
        let Some(menu)=popup.as_mut() else{return false};
        match event {
            WindowEvent::KeyPressed(Key::Escape)=>{*popup=None;self.request_redraw();true},
            WindowEvent::KeyPressed(Key::Up)=>{menu.move_selected(false);self.request_redraw();true},
            WindowEvent::KeyPressed(Key::Down)=>{menu.move_selected(true);self.request_redraw();true},
            WindowEvent::KeyPressed(Key::Enter) | WindowEvent::KeyPressed(Key::Space)=>{
                if let Some(item)=menu.selected_item() {
                    if item==super::interaction::AUTO_COPY_ITEM {self.copy_selection();}
                    out.push(WidgetEvent::MenuItemActivated{owner:menu.owner,item});
                }
                *popup=None;self.request_redraw();true
            },
            WindowEvent::KeyPressed(Key::MouseLeft) => true,
            WindowEvent::KeyReleased(Key::MouseLeft)=>{
                let (x,y)=self.cursor.get();
                if let Some(item)=menu.item_at(x,y) {
                    if item==super::interaction::AUTO_COPY_ITEM {self.copy_selection();}
                    out.push(WidgetEvent::MenuItemActivated{owner:menu.owner,item});
                }
                *popup=None;self.request_redraw();true
            },
            WindowEvent::KeyPressed(Key::MouseRight)|WindowEvent::KeyReleased(Key::MouseRight)=>true,
            _=>false,
        }
    }

    /// Performs the place cursor operation.
    ///
    /// Visibility: **private**.
    #[doc = "Implements the place cursor operation. Visibility: private/internal."]
    fn place_cursor(&self, id: WidgetId, x: i32) {
        let Some(state) = self.text_state(id) else {
            return;
        };
        let mut s = state.lock().unwrap();
        let area = if s.computed.content_bounds == s.bounds {
            crate::widgets::Rect::new(
                s.bounds.x + 6,
                s.bounds.y + 3,
                s.bounds.width.saturating_sub(12),
                s.bounds.height.saturating_sub(6)
            )
        } else {
            s.computed.content_bounds
        };
        let rel = x.saturating_sub(area.x).max(0) as u32;
        #[cfg(feature = "fonts")]
        let pt = s.computed.font_size_px.round().clamp(1.0, u16::MAX as f32) as u16;
        #[cfg(feature = "fonts")]
        let line = Some(s.computed.line_height_px.round().max(1.0) as u32);
        #[cfg(feature = "fonts")]
        let mut best = 0;
        #[cfg(feature = "fonts")]
        let mut default_font = self.font.borrow_mut();
        #[cfg(feature = "fonts")]
        let mut font_cache = self.font_cache.borrow_mut();
        #[cfg(feature = "fonts")]
        if let Some(font) = font_for_style(&mut default_font, &mut font_cache, &s.computed.font) {
            for (byte, _) in s.text.char_indices() {
                let w = font
                    .measure_styled(
                        &s.text[..byte],
                        pt,
                        s.computed.font.variant,
                        s.computed.font.weight,
                        s.computed.font.stretch,
                        line
                    )
                    .map(|v| v.0)
                    .unwrap_or(0);
                if w <= rel {
                    best = byte;
                } else {
                    break;
                }
            }
            let full = font
                .measure_styled(
                    &s.text,
                    pt,
                    s.computed.font.variant,
                    s.computed.font.weight,
                    s.computed.font.stretch,
                    line
                )
                .map(|v| v.0)
                .unwrap_or(0);
            if rel >= full {
                best = s.text.len();
            }
        } else {
            best = s.text.len();
        }
        // A fontless build keeps text editing functional. Hit testing uses
        // an approximate cell width; exact glyph advance needs `fonts`.
        #[cfg(not(feature = "fonts"))]
        let best = {
            let cell = (s.computed.font_size_px * 0.55).max(1.0);
            let target = (rel as f32 / cell).round() as usize;
            s.text.char_indices().map(|(i, _)| i).chain(std::iter::once(s.text.len()))
                .nth(target).unwrap_or(s.text.len())
        };
        let old = s.cursor;
        s.cursor = best;
        if s.pressed {
            let anchor=*s.mouse_anchor.get_or_insert(best);
            s.selection=super::interaction::extend_selection(None,anchor,best);
        } else {s.selection=None;}
        if old != best {
            drop(s);
            self.request_redraw()
        }
    }
}

/// Maps the crate's logical key values to printable text using a conventional
/// US keyboard Shift layer. Backends intentionally report the unshifted key so
/// modifier handling stays consistent in one place.
#[doc = "Implements the key to text char operation. Visibility: private/internal."]
fn key_to_text_char(key: Key, shift: bool) -> Option<char> {
    match key {
        Key::Num(n) if n <= 9 => {
            if shift {
                Some(match n {
                    0 => ')',
                    1 => '!',
                    2 => '@',
                    3 => '#',
                    4 => '$',
                    5 => '%',
                    6 => '^',
                    7 => '&',
                    8 => '*',
                    9 => '(',
                    _ => unreachable!(),
                })
            } else {
                Some((b'0' + n) as char)
            }
        }
        Key::Char(ch) => {
            if !shift {
                return Some(ch);
            }
            Some(match ch {
                'a'..='z' => ch.to_ascii_uppercase(),
                'A'..='Z' => ch,
                '`' => '~',
                '-' => '_',
                '=' => '+',
                '[' => '{',
                ']' => '}',
                '\\' => '|',
                ';' => ':',
                '\'' => '"',
                ',' => '<',
                '.' => '>',
                '/' => '?',
                _ => ch,
            })
        }
        _ => None,
    }
}

#[cfg(test)]
/// Contains the text input tests implementation.
///
/// Visibility: **private**.
mod text_input_tests {
    use super::key_to_text_char;
    use crate::events::Key;

    #[test]
    /// Implements shift capitalizes letters.
    ///
    /// Visibility: **private**.
    #[doc = "Implements the shift capitalizes letters operation. Visibility: private/internal."]
    fn shift_capitalizes_letters() {
        assert_eq!(key_to_text_char(Key::Char('a'), true), Some('A'));
        assert_eq!(key_to_text_char(Key::Char('z'), true), Some('Z'));
        assert_eq!(key_to_text_char(Key::Char('a'), false), Some('a'));
    }

    #[test]
    /// Implements number row uses shift symbols.
    ///
    /// Visibility: **private**.
    #[doc = "Implements the number row uses shift symbols operation. Visibility: private/internal."]
    fn number_row_uses_shift_symbols() {
        let shifted = [')', '!', '@', '#', '$', '%', '^', '&', '*', '('];
        for n in 0..=9 {
            assert_eq!(key_to_text_char(Key::Num(n), false), Some((b'0' + n) as char));
            assert_eq!(key_to_text_char(Key::Num(n), true), Some(shifted[n as usize]));
        }
    }

    #[test]
    /// Implements punctuation uses shift layer.
    ///
    /// Visibility: **private**.
    #[doc = "Implements the punctuation uses shift layer operation. Visibility: private/internal."]
    fn punctuation_uses_shift_layer() {
        let pairs = [
            ('`', '~'),
            ('-', '_'),
            ('=', '+'),
            ('[', '{'),
            (']', '}'),
            ('\\', '|'),
            (';', ':'),
            ('\'', '"'),
            (',', '<'),
            ('.', '>'),
            ('/', '?'),
        ];
        for (plain, shifted) in pairs {
            assert_eq!(key_to_text_char(Key::Char(plain), true), Some(shifted));
        }
    }
}

/// Constant used for font renderer cache limit.
///
/// Visibility: **private**.
#[cfg(feature = "fonts")]
const FONT_RENDERER_CACHE_LIMIT: usize = 4;

/// Implements font for style.
///
/// Visibility: **private**.
#[doc = "Implements the font for style operation. Visibility: private/internal."]
#[cfg(feature = "fonts")]
pub(crate) fn font_for_style<'a>(
    default: &'a mut Option<FontRenderer>,
    cache: &'a mut HashMap<String, FontRenderer>,
    font: &crate::widgets::style::FontProperties
) -> Option<&'a mut FontRenderer> {
    for family in &font.family {
        let face_key=format!("web:{}",family.trim().trim_matches(|c| c == '\'' || c == '"').to_ascii_lowercase());
        if cache.contains_key(&face_key) {return cache.get_mut(&face_key);}
        let key = format!("{}|{:?}|{:?}", family.to_ascii_lowercase(), font.style, font.weight);
        if !cache.contains_key(&key) {
            // Each FontRenderer owns decoded font data plus a glyph cache. Keep
            // the family/style cache deliberately small so applications that
            // cycle through many fonts do not retain them for the window's
            // entire lifetime. The active family is reloaded on demand.
            if cache.len() >= FONT_RENDERER_CACHE_LIMIT {
                cache.retain(|name,_| name.starts_with("web:"));
            }
            if let Ok(renderer) = FontRenderer::system_family(family, font.style, font.weight) {
                cache.insert(key.clone(), renderer);
            }
        }
        if cache.contains_key(&key) {
            return cache.get_mut(&key);
        }
    }
    default.as_mut()
}

#[cfg(test)]
mod scroll_sensitivity_tests {
    use super::*;

    #[test]
    fn default_is_gentler_than_one_to_one() {
        let mut remainder = 0.0;
        assert_eq!(wheel_pixels(-120, DEFAULT_SCROLL_SENSITIVITY, &mut remainder), -28);
        assert_eq!(wheel_pixels(-120, DEFAULT_SCROLL_SENSITIVITY, &mut remainder), -29);
    }

    #[test]
    fn fractional_deltas_are_not_lost() {
        let mut remainder = 0.0;
        let total: i32 = (0..120).map(|_| wheel_pixels(-1, 0.5, &mut remainder)).sum();
        assert_eq!(total, -24);
    }

    #[test]
    fn rejects_invalid_sensitivity() {
        for value in [-1.0, f32::INFINITY, f32::NAN, 10.1] {
            assert!(validate_scroll_sensitivity(value).is_err());
        }
        assert!(validate_scroll_sensitivity(0.0).is_ok());
        assert!(validate_scroll_sensitivity(10.0).is_ok());
    }
}

#[cfg(all(test, feature = "fonts"))]
mod font_borrow_regression_tests {
    use super::*;

    #[test]
    fn nested_layout_font_borrow_defers_without_panicking() {
        let font: RefCell<Option<FontRenderer>> = RefCell::new(None);
        let dirty = AtomicBool::new(false);
        let active_borrow = font.borrow_mut();
        assert!(try_layout_font(&font, &dirty).is_none());
        assert!(dirty.load(Ordering::Acquire));
        drop(active_borrow);
        // The next layout pass can acquire the renderer again.
        assert!(try_layout_font(&font, &dirty).is_some());
    }

    #[test]
    fn normal_layout_font_borrow_does_not_dirty_window() {
        let font: RefCell<Option<FontRenderer>> = RefCell::new(None);
        let dirty = AtomicBool::new(false);
        assert!(try_layout_font(&font, &dirty).is_some());
        assert!(!dirty.load(Ordering::Acquire));
    }
}
