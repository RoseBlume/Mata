//! UWP AppContainer window backend. No Rust-side Win32 dependencies.
//!
//! A UWP CoreApplication owns its CoreWindow; Window::new attaches to the view
//! created by the Windows SDK host in `uwp/host/MataUwpHost.cpp`.
//! The framebuffer remains numeric ARGB 0xAARRGGBB (BGRA bytes on LE CPUs).
use std::{cell::Cell, collections::HashSet, ffi::c_void, sync::Mutex};
use crate::{buffer::Framebuffer, events::{Key, WindowEvent, Touch, TouchEvent, EVENT_QUEUE, KEYS_DOWN, TOUCH_QUEUE}, window::DirtyRect};

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct NativeEvent { kind: i32, a: i32, b: i32, id: i32 }

unsafe extern "C" {
    fn mata_uwp_host_ready() -> i32;
    fn mata_uwp_host_size(width: *mut u32, height: *mut u32);
    fn mata_uwp_host_present(pixels: *const u32, width: u32, height: u32);
    fn mata_uwp_host_poll_event(event: *mut NativeEvent) -> i32;
    fn mata_uwp_host_pump();
    fn mata_uwp_host_set_title(utf8: *const u8, len: usize);
    fn mata_uwp_host_set_cursor(visible: i32);
    fn mata_uwp_host_cursor_focused() -> i32;
}

pub struct PlatformWindow {
    source_size: Cell<(u32, u32)>,
    visible: Cell<bool>,
}

impl PlatformWindow {
    pub fn new(title: &str, size: (u32, u32)) -> Self {
        assert!(unsafe { mata_uwp_host_ready() } != 0,
            "UWP Window::new must run inside mata_uwp_app_main, invoked by CoreApplication::Run");
        EVENT_QUEUE.get_or_init(|| Mutex::new(Vec::new()));
        KEYS_DOWN.get_or_init(|| Mutex::new(HashSet::new()));
        TOUCH_QUEUE.get_or_init(|| Mutex::new(Vec::new()));
        let win = Self { source_size: Cell::new(size), visible: Cell::new(true) };
        win.set_title(title);
        win
    }
    pub fn present(&mut self, frame: &Framebuffer) {
        if frame.width == 0 || frame.height == 0 { return; }
        if (frame.width as usize).checked_mul(frame.height as usize) != Some(frame.pixels.len()) { return; }
        self.source_size.set((frame.width, frame.height));
        unsafe { mata_uwp_host_present(frame.pixels.as_ptr(), frame.width, frame.height); }
    }
    pub fn present_dirty(&mut self, frame: &Framebuffer, rects: &[DirtyRect]) {
        // Currently present the entire valid source. The host's D3D11 upload
        // owns no history of damage; partial presentation must not leave stale pixels.
        if rects.iter().any(|r| r.clipped(frame.width, frame.height).is_some()) {
            self.present(frame);
        }
    }
    pub fn frame_ready(&self) -> bool { true }
    pub fn client_size(&self) -> (u32, u32) {
        let (mut w, mut h) = (0, 0);
        unsafe { mata_uwp_host_size(&mut w, &mut h); }
        (w, h)
    }
    pub fn framebuffer_size(&self) -> (u32, u32) { self.source_size.get() }
    pub fn pump_events(&self) {
        unsafe { mata_uwp_host_pump(); }
        let mut event = NativeEvent::default();
        let mut events = Vec::new();
        let mut touches = Vec::new();
        for _ in 0..4096 {
            if unsafe { mata_uwp_host_poll_event(&mut event) } == 0 { break; }
            match event.kind {
                1 => {
                    events.push(WindowEvent::Close);
                    if let Some(keys) = KEYS_DOWN.get() { keys.lock().unwrap().clear(); }
                },
                2 => events.push(WindowEvent::Resized(event.a.max(0) as u32, event.b.max(0) as u32)),
                3 | 4 => if let Some(key) = map_key(event.a) {
                    if let Some(keys) = KEYS_DOWN.get() {
                        let mut keys = keys.lock().unwrap();
                        if event.kind == 3 { keys.insert(key); } else { keys.remove(&key); }
                    }
                    events.push(if event.kind == 3 { WindowEvent::KeyPressed(key) } else { WindowEvent::KeyReleased(key) });
                },
                5 => events.push(WindowEvent::CursorMoved(event.a, event.b)),
                6 => events.push(WindowEvent::Scroll(event.a, event.b)),
                7 => events.push(WindowEvent::Minimized),
                8 => events.push(WindowEvent::Restored(event.a.max(0) as u32, event.b.max(0) as u32)),
                9..=12 => {
                    let kind = match event.kind {
                        9 => TouchEvent::Started(event.a.max(0) as u64, event.b.max(0) as u64),
                        10 => TouchEvent::Moved(event.a.max(0) as u64, event.b.max(0) as u64),
                        11 => TouchEvent::Ended(event.a.max(0) as u64, event.b.max(0) as u64),
                        _ => TouchEvent::Cancelled,
                    };
                    touches.push(Touch { device_id: format!("uwp:{}", event.id), event: kind });
                }
                _ => {}
            }
        }
        if let Some(queue) = EVENT_QUEUE.get() { queue.lock().unwrap().extend(events); }
        if let Some(queue) = TOUCH_QUEUE.get() { queue.lock().unwrap().extend(touches); }
    }
    pub fn set_cursor_visible(&self, visible: bool) -> Result<(), String> {
        unsafe { mata_uwp_host_set_cursor(visible as i32); }
        self.visible.set(visible);
        Ok(())
    }
    pub fn cursor_visible(&self) -> bool { self.visible.get() }
    pub fn cursor_focused(&self) -> bool { unsafe { mata_uwp_host_cursor_focused() != 0 } }
    pub fn set_title(&self, title: &str) { unsafe { mata_uwp_host_set_title(title.as_ptr(), title.len()); } }
    pub fn show(&self) {}
    pub fn hide(&self) {}
    pub fn maximize(&self) {} // UWP lifecycle controls view state.
    pub fn minimize(&self) {}
    pub fn restore(&self) {}
    pub fn reposition(&self, _x: i32, _y: i32) {}
    pub fn resize(&self, _w: u32, _h: u32) {}
    pub fn set_decorations(&self, _enabled: bool) {}
    pub fn set_maximizable(&self, _enabled: bool) {}
    pub fn set_minimizable(&self, _enabled: bool) {}
    pub fn set_resizable(&self, _enabled: bool) {}
    pub fn handle(&self) -> *mut c_void { std::ptr::null_mut() } // Not an HWND.
}

fn map_key(vk: i32) -> Option<Key> {
    Some(match vk {
        0x08 => Key::Backspace, 0x0D => Key::Enter, 0x09 => Key::Tab,
        0x1B => Key::Escape, 0x20 => Key::Space,
        0x25 => Key::Left, 0x26 => Key::Up, 0x27 => Key::Right, 0x28 => Key::Down,
        0x24 => Key::Home, 0x23 => Key::End, 0x2e => Key::Delete,
        0x10 | 0xA0 => Key::LeftShift, 0xA1 => Key::RightShift,
        0x11 | 0xA2 => Key::LeftCtrl, 0xA3 => Key::RightCtrl,
        0x70..=0x7B => Key::F((vk - 0x6f) as u8),
        0x30..=0x39 => Key::Num((vk - 0x30) as u8),
        0x41..=0x5A => Key::Char(char::from_u32((vk + 32) as u32)?),
        0x100 => Key::MouseLeft, 0x101 => Key::MouseRight, 0x102 => Key::MouseMiddle,
        _ => return None,
    })
}
