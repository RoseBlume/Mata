//! Servo 0.7 backend for `widgets::WebView`.
//!
//! Servo renders into an offscreen OpenGL software context. Its RGBA image is
//! composited into mata-win's ARGB framebuffer, so the browser is an ordinary
//! clipped, movable widget on Linux/X11/Wayland and Windows, not a GTK toplevel.
//!
//! This is deliberately isolated behind `webview-servo`. It is CPU-readback
//! based, and is not intended as a substitute for GPU texture sharing.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{atomic::{AtomicBool, Ordering}, Arc},
};

use dpi::PhysicalSize;
use servo::{
    EventLoopWaker, InputEvent, Key as ServoKey, KeyState, KeyboardEvent,
    MouseButton, MouseButtonAction, MouseButtonEvent, MouseMoveEvent, NamedKey,
    RenderingContext, Servo, ServoBuilder, SoftwareRenderingContext,
    WebViewBuilder, WebViewDelegate, WheelDelta, WheelEvent, WheelMode,
};

use super::{
    Rect, WidgetError, WidgetEvent, WidgetId, WidgetWindow,
    push_event,
};
use crate::{events::Key, Framebuffer};

#[derive(Clone)]
struct Waker(Arc<AtomicBool>);
impl EventLoopWaker for Waker {
    fn clone_box(&self) -> Box<dyn EventLoopWaker> { Box::new(self.clone()) }
    fn wake(&self) { self.0.store(true, Ordering::Release); }
}

// Servo is an engine-per-process architecture; sharing one engine avoids
// separate JS runtimes and ensures all views receive events in one poll.
// The UI is single-threaded and Servo's public handles are not Send.
thread_local! {
    static ENGINE: RefCell<Option<Rc<Servo>>> = const { RefCell::new(None) };
}

fn engine() -> Rc<Servo> {
    ENGINE.with(|cell| {
        let mut engine = cell.borrow_mut();
        engine.get_or_insert_with(|| {
            // A host may already have installed its own rustls provider.
            let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
            Rc::new(ServoBuilder::default()
                .event_loop_waker(Box::new(Waker(Arc::new(AtomicBool::new(true)))))
                .build())
        }).clone()
    })
}

/// Advance the Servo engine from Mata's nonblocking event pump.
/// Background activity (network, timers, JS) remains serviced even when no
/// native window events arrive.
pub(crate) fn pump_events() {
    ENGINE.with(|cell| {
        if let Some(engine) = cell.borrow().as_ref() {
            engine.spin_event_loop();
        }
    });
}

struct PaintDelegate {
    host_dirty: Arc<AtomicBool>,
    frame_ready: Cell<bool>,
}
impl WebViewDelegate for PaintDelegate {
    fn notify_new_frame_ready(&self, _view: servo::WebView) {
        // As in the official Servo embedder model, painting is deferred to the
        // host's render pass instead of running inside this callback.
        self.frame_ready.set(true);
        self.host_dirty.store(true, Ordering::Release);
    }
}

/// Actual in-process Servo webview and its separate offscreen surface.
pub struct WebView {
    id: WidgetId,
    host_id: u64,
    engine: Rc<Servo>,
    view: servo::WebView,
    context: Rc<SoftwareRenderingContext>,
    delegate: Rc<PaintDelegate>,
    size: Cell<(u32, u32)>,
    visible: Cell<bool>,
    bounds: Cell<Rect>,
    // Last frame remains drawable while no new frame is available.
    cached: RefCell<Option<Vec<u32>>>,
}

impl WebView {
    pub fn new(parent: &WidgetWindow, id: WidgetId, bounds: Rect) -> Result<Self, WidgetError> {
        let size = (bounds.width.max(1), bounds.height.max(1));
        let context = Rc::new(SoftwareRenderingContext::new(PhysicalSize::new(size.0, size.1))
            .map_err(|err| WidgetError(format!("Servo software context: {err:?}")))?);
        context.make_current()
            .map_err(|err| WidgetError(format!("Servo OpenGL context: {err:?}")))?;
        let engine = engine();
        let delegate = Rc::new(PaintDelegate {
            host_dirty: parent.dirty.clone(),
            frame_ready: Cell::new(true),
        });
        let view = WebViewBuilder::new(&engine, context.clone())
            .delegate(delegate.clone())
            .build();
        view.show();
        push_event(parent.id(), WidgetEvent::WebViewReady(id));
        Ok(Self {
            id, host_id: parent.id(), engine, view, context, delegate,
            size: Cell::new(size), visible: Cell::new(true),
            bounds: Cell::new(bounds), cached: RefCell::new(None),
        })
    }

    pub fn url(&self) -> Option<String> { self.view.url().map(|url| url.to_string()) }
    pub fn page_title(&self) -> Option<String> { self.view.page_title() }
    pub fn can_go_back(&self) -> bool { self.view.can_go_back() }
    pub fn can_go_forward(&self) -> bool { self.view.can_go_forward() }
    pub fn set_zoom(&self, zoom: f32) { self.view.set_page_zoom(zoom); }
    pub fn evaluate_javascript(
        &self,
        source: &str,
        callback: impl FnOnce(Result<servo::JSValue, servo::JavaScriptEvaluationError>) + 'static,
    ) { self.view.evaluate_javascript(source, callback); }
    pub fn id(&self) -> WidgetId { self.id }
    pub fn bounds(&self) -> Rect { self.bounds.get() }
    pub fn visible(&self) -> bool { self.visible.get() }
    pub fn set_focused(&self, focused: bool) { self.view.set_focused(focused); }
    pub fn mouse_left(&self) {
        self.view.notify_input_event(InputEvent::MouseLeftViewport(Default::default()));
    }
    pub fn needs_redraw(&self) -> bool { self.visible.get() && self.delegate.frame_ready.get() }

    pub fn navigate(&self, uri: &str) -> Result<(), WidgetError> {
        let url = url::Url::parse(uri)
            .map_err(|err| WidgetError(format!("Invalid Servo WebView URL: {err}")))?;
        self.view.load(url);
        self.engine.spin_event_loop();
        Ok(())
    }
    pub fn go_back(&self) { if self.view.can_go_back() { self.view.go_back(1); } }
    pub fn go_forward(&self) { if self.view.can_go_forward() { self.view.go_forward(1); } }
    pub fn reload(&self) { self.view.reload(); }

    pub fn set_visible(&self, visible: bool) {
        if self.visible.replace(visible) != visible {
            if visible { self.view.show(); } else { self.view.hide(); }
            self.delegate.host_dirty.store(true, Ordering::Release);
        }
    }

    pub fn set_bounds(&self, bounds: Rect) {
        if self.bounds.replace(bounds) != bounds {
            self.delegate.host_dirty.store(true, Ordering::Release);
        }
        let size = (bounds.width.max(1), bounds.height.max(1));
        if self.size.replace(size) != size {
            self.cached.borrow_mut().take();
            self.view.resize(PhysicalSize::new(size.0, size.1));
            self.delegate.frame_ready.set(true);
        }
    }

    /// Paint the last rendered page into the exact styled rectangle.
    /// Pixels outside the parent framebuffer are clipped; the source uses
    /// straight RGBA, while the destination uses 0xAARRGGBB.
    pub fn blit(&self, target: &mut Framebuffer, bounds: Rect) {
        if !self.visible.get() || bounds.width == 0 || bounds.height == 0 { return; }
        if self.delegate.frame_ready.replace(false) || self.cached.borrow().is_none() {
            if let Err(err) = self.context.make_current() {
                push_event(self.host_id, WidgetEvent::WebViewError(self.id, format!("Servo context: {err:?}")));
                // Do not continuously retry a permanently invalid GL context.
                return;
            }
            self.view.paint();
            let (w, h) = self.size.get();
            let rect = servo::DeviceIntRect::from_origin_and_size(
                servo::DeviceIntPoint::zero(),
                servo::DeviceIntSize::new(w as i32, h as i32),
            );
            if let Some(image) = self.context.read_to_image(rect) {
                let rgba = image.as_raw();
                let pixels = rgba.chunks_exact(4).map(|p| {
                    (u32::from(p[3]) << 24) | (u32::from(p[0]) << 16)
                        | (u32::from(p[1]) << 8) | u32::from(p[2])
                }).collect();
                *self.cached.borrow_mut() = Some(pixels);
            } else {
                self.delegate.frame_ready.set(true);
            }
        }
        if let Some(pixels) = self.cached.borrow().as_ref() {
            copy_clipped(target, bounds, pixels, self.size.get());
        }
    }

    /// Translate Mata's native input into page-local Servo input events.
    /// The host determines which view owns pointer/keyboard focus.
    pub fn handle_input(&self, event: &WidgetEvent, bounds: Rect) {
        let point = |x: i32, y: i32| {
            servo::WebViewPoint::Device(servo::DevicePoint::new(
                (x - bounds.x).max(0) as f32, (y - bounds.y).max(0) as f32,
            ))
        };
        let servo_event = match *event {
            WidgetEvent::PointerMoved(x, y) =>
                Some(InputEvent::MouseMove(MouseMoveEvent::new(point(x, y)))),
            WidgetEvent::PointerPressed(x, y) => {
                self.view.set_focused(true);
                Some(InputEvent::MouseButton(MouseButtonEvent::new(
                    MouseButtonAction::Down, MouseButton::Primary, point(x, y),
                )))
            },
            WidgetEvent::PointerReleased(x, y) =>
                Some(InputEvent::MouseButton(MouseButtonEvent::new(
                    MouseButtonAction::Up, MouseButton::Primary, point(x, y),
                ))),
            WidgetEvent::PointerRightPressed(x, y) => {
                self.view.set_focused(true);
                Some(InputEvent::MouseButton(MouseButtonEvent::new(
                    MouseButtonAction::Down, MouseButton::Secondary, point(x, y),
                )))
            },
            WidgetEvent::PointerRightReleased(x, y) =>
                Some(InputEvent::MouseButton(MouseButtonEvent::new(
                    MouseButtonAction::Up, MouseButton::Secondary, point(x, y),
                ))),
            WidgetEvent::Scrolled { x, y, dx, dy } =>
                Some(InputEvent::Wheel(WheelEvent::new(
                    WheelDelta { x: f64::from(dx), y: f64::from(dy), z: 0.0,
                        mode: WheelMode::DeltaPixel },
                    point(x, y),
                ))),
            WidgetEvent::KeyPressed(key) => keyboard_event(key, KeyState::Down),
            WidgetEvent::KeyReleased(key) => keyboard_event(key, KeyState::Up),
            _ => None,
        };
        if let Some(event) = servo_event { self.view.notify_input_event(event); }
    }
}

fn keyboard_event(key: Key, state: KeyState) -> Option<InputEvent> {
    let k = match key {
        Key::Char(c) => ServoKey::Character(c.to_string()),
        Key::Num(n) => ServoKey::Character(n.to_string()),
        Key::Space => ServoKey::Character(" ".into()),
        Key::Enter => ServoKey::Named(NamedKey::Enter),
        Key::Backspace => ServoKey::Named(NamedKey::Backspace),
        Key::Escape => ServoKey::Named(NamedKey::Escape),
        Key::Tab => ServoKey::Named(NamedKey::Tab),
        Key::Up => ServoKey::Named(NamedKey::ArrowUp),
        Key::Down => ServoKey::Named(NamedKey::ArrowDown),
        Key::Left => ServoKey::Named(NamedKey::ArrowLeft),
        Key::Right => ServoKey::Named(NamedKey::ArrowRight),
        Key::LeftShift | Key::RightShift => ServoKey::Named(NamedKey::Shift),
        Key::LeftCtrl | Key::RightCtrl => ServoKey::Named(NamedKey::Control),
        _ => return None,
    };
    Some(InputEvent::Keyboard(KeyboardEvent::from_state_and_key(state, k)))
}

fn copy_clipped(target: &mut Framebuffer, bounds: Rect, src: &[u32], dims: (u32, u32)) {
    let tw = target.width as usize;
    let th = target.height as usize;
    let sw = dims.0 as usize;
    let sh = dims.1 as usize;
    let x0 = bounds.x.max(0) as usize;
    let y0 = bounds.y.max(0) as usize;
    let x1 = (i64::from(bounds.x) + i64::from(bounds.width)).max(0)
        .min(tw as i64) as usize;
    let y1 = (i64::from(bounds.y) + i64::from(bounds.height)).max(0)
        .min(th as i64) as usize;
    for y in y0..y1 {
        let sy = (y as i64 - i64::from(bounds.y)) as usize;
        if sy >= sh { break; }
        for x in x0..x1 {
            let sx = (x as i64 - i64::from(bounds.x)) as usize;
            if sx >= sw { break; }
            let Some(&src_px) = src.get(sy * sw + sx) else { continue };
            let dst = &mut target.pixels[y * tw + x];
            *dst = over(src_px, *dst);
        }
    }
}

#[inline]
fn over(src: u32, dst: u32) -> u32 {
    let a: u32 = src >> 24;
    if a == 255 { return src; }
    if a == 0 { return dst; }

    let ia: u32 = 255u32 - a;
    let chan = |shift: u32| -> u32 {
        let s: u32 = (src >> shift) & 0xffu32;
        let d: u32 = (dst >> shift) & 0xffu32;
        (s * a + d * ia + 127u32) / 255u32
    };

    0xff00_0000u32 | (chan(16u32) << 16u32)
        | (chan(8u32) << 8u32) | chan(0u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clipped_blit_does_not_overflow() {
        let mut fb = Framebuffer::from_color(2, 2, 0xff000000);
        copy_clipped(&mut fb, Rect::new(-1, -1, 2, 2), &[0xffffffff; 4], (2,2));
        assert_eq!(fb.pixels, vec![0xffffffff, 0xff000000, 0xff000000, 0xff000000]);
    }
    #[test]
    fn blend_opaque_and_transparent() {
        assert_eq!(over(0xffff0000, 0xff123456), 0xffff0000);
        assert_eq!(over(0x00000000, 0xff123456), 0xff123456);
    }
}
