//! Browser window backed by a host-side HTML Canvas and WebGPU/Canvas2D.
//!
//! This module adds NO Rust dependencies. The JS glue is in `wasm/host`.
//! All host calls must remain on the browser's main thread.
use std::{cell::Cell, collections::HashSet, ffi::c_void, sync::Mutex};
use crate::{buffer::Framebuffer, events::{Key, Touch, TouchEvent, WindowEvent, EVENT_QUEUE, KEYS_DOWN, TOUCH_QUEUE}, window::DirtyRect};

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct WebEvent { kind: i32, a: i32, b: i32, id: i32 }

#[link(wasm_import_module = "env")]
unsafe extern "C" {
    fn mata_web_window_new(title: *const u8, title_len: usize, width: u32, height: u32) -> u32;
    fn mata_web_window_drop(id: u32);
    fn mata_web_window_size(id: u32, out: *mut u32);
    fn mata_web_window_present(id: u32, pixels: *const u32, width: u32, height: u32, regions: *const u32, regions_len: u32);
    fn mata_web_window_event(id: u32, out: *mut WebEvent) -> i32;
    fn mata_web_window_action(id: u32, action: i32, a: i32, b: i32);
    fn mata_web_cursor_visible(id: u32) -> i32;
    fn mata_web_cursor_focused(id: u32) -> i32;
    fn mata_web_window_title(id: u32, data: *const u8, len: usize);
}

pub struct PlatformWindow {
    id: u32,
    source_size: Cell<(u32, u32)>,
    visible: Cell<bool>,
}
impl PlatformWindow {
    pub fn new(title: &str, size: (u32, u32)) -> Self {
        EVENT_QUEUE.get_or_init(|| Mutex::new(Vec::new()));
        KEYS_DOWN.get_or_init(|| Mutex::new(HashSet::new()));
        TOUCH_QUEUE.get_or_init(|| Mutex::new(Vec::new()));
        let id = unsafe { mata_web_window_new(title.as_ptr(), title.len(), size.0, size.1) };
        assert_ne!(id, 0, "The Mata browser host must be initialized before Window::new");
        Self { id, source_size: Cell::new(size), visible: Cell::new(true) }
    }
    pub fn present(&mut self, framebuffer: &Framebuffer) {
        if valid(framebuffer) {
            self.source_size.set((framebuffer.width, framebuffer.height));
            unsafe { mata_web_window_present(self.id, framebuffer.pixels.as_ptr(), framebuffer.width, framebuffer.height, std::ptr::null(), 0); }
        }
    }
    pub fn present_dirty(&mut self, framebuffer: &Framebuffer, dirty: &[DirtyRect]) {
        if !valid(framebuffer) || dirty.is_empty() { return; }
        let rects: Vec<u32> = dirty.iter().filter_map(|r| r.clipped(framebuffer.width, framebuffer.height))
            .flat_map(|r| [r.x, r.y, r.width, r.height]).collect();
        if !rects.is_empty() {
            self.source_size.set((framebuffer.width, framebuffer.height));
            unsafe { mata_web_window_present(self.id, framebuffer.pixels.as_ptr(), framebuffer.width, framebuffer.height, rects.as_ptr(), (rects.len()/4) as u32); }
        }
    }
    pub fn frame_ready(&self) -> bool { true }
    pub fn client_size(&self) -> (u32,u32) {
        let mut dimensions=[0u32;2];
        unsafe { mata_web_window_size(self.id, dimensions.as_mut_ptr()); }
        (dimensions[0], dimensions[1])
    }
    pub fn framebuffer_size(&self) -> (u32,u32) { self.source_size.get() }
    pub fn pump_events(&self) {
        let mut events=Vec::new();
        let mut touches=Vec::new();
        let mut next=WebEvent::default();
        // Bound each pump to avoid pathological input floods blocking a frame.
        for _ in 0..4096 {
            if unsafe { mata_web_window_event(self.id, &mut next) } == 0 { break; }
            match next.kind {
                1 => { events.push(WindowEvent::Close); if let Some(keys)=KEYS_DOWN.get() { keys.lock().unwrap().clear(); } },
                2 => events.push(WindowEvent::Resized(next.a.max(0) as u32, next.b.max(0) as u32)),
                3 | 4 => {
                    if let Some(key)=map_key(next.a) {
                        if let Some(keys)=KEYS_DOWN.get() {
                            let mut keys=keys.lock().unwrap();
                            if next.kind==3 {keys.insert(key);} else {keys.remove(&key);}
                        }
                        events.push(if next.kind==3 {WindowEvent::KeyPressed(key)} else {WindowEvent::KeyReleased(key)});
                    }
                }
                5 => events.push(WindowEvent::CursorMoved(next.a, next.b)),
                6 => events.push(WindowEvent::Scroll(next.a,next.b)),
                7 => events.push(WindowEvent::Minimized),
                8 => events.push(WindowEvent::Restored(next.a.max(0) as u32,next.b.max(0) as u32)),
                9..=12 => {
                    let event=match next.kind {
                        9=>TouchEvent::Started(next.a.max(0) as u64,next.b.max(0) as u64),
                        10=>TouchEvent::Moved(next.a.max(0) as u64,next.b.max(0) as u64),
                        11=>TouchEvent::Ended(next.a.max(0) as u64,next.b.max(0) as u64),
                        _=>TouchEvent::Cancelled,
                    };
                    touches.push(Touch{device_id:format!("browser:{}",next.id),event});
                }
                _=>{}
            }
        }
        if let Some(q)=EVENT_QUEUE.get(){q.lock().unwrap().extend(events);}
        if let Some(q)=TOUCH_QUEUE.get(){q.lock().unwrap().extend(touches);}
    }
    pub fn set_cursor_visible(&self, visible:bool)->Result<(),String> {
        unsafe {mata_web_window_action(self.id,1,visible as i32,0)};
        self.visible.set(visible); Ok(())
    }
    pub fn cursor_visible(&self)->bool {
        self.visible.get() && unsafe {mata_web_cursor_visible(self.id)!=0}
    }
    pub fn cursor_focused(&self)->bool {unsafe {mata_web_cursor_focused(self.id)!=0}}
    pub fn set_title(&self,title:&str){unsafe{mata_web_window_title(self.id,title.as_ptr(),title.len());}}
    pub fn hide(&self){unsafe{mata_web_window_action(self.id,2,0,0);}}
    pub fn show(&self){unsafe{mata_web_window_action(self.id,3,0,0);}}
    pub fn maximize(&self){unsafe{mata_web_window_action(self.id,4,0,0);}}
    pub fn minimize(&self){unsafe{mata_web_window_action(self.id,5,0,0);}}
    pub fn restore(&self){unsafe{mata_web_window_action(self.id,6,0,0);}}
    pub fn reposition(&self,_x:i32,_y:i32){} // Browsers prohibit moving top-level windows.
    pub fn resize(&self,w:u32,h:u32){unsafe{mata_web_window_action(self.id,7,w.min(i32::MAX as u32) as i32,h.min(i32::MAX as u32) as i32);}}
    pub fn set_decorations(&self,_enabled:bool){} // No native window decorations on Canvas.
    pub fn set_maximizable(&self,_enabled:bool){}
    pub fn set_minimizable(&self,_enabled:bool){}
    pub fn set_resizable(&self,_enabled:bool){}
    pub fn handle(&self)->*mut c_void {self.id as usize as *mut c_void}
}
impl Drop for PlatformWindow {fn drop(&mut self){unsafe{mata_web_window_drop(self.id);}}}
fn valid(fb:&Framebuffer)->bool {
    fb.width>0 && fb.height>0 && (fb.width as usize).checked_mul(fb.height as usize)==Some(fb.pixels.len())
}
fn map_key(code:i32)->Option<Key> {
    Some(match code {
        1=>Key::Backspace,2=>Key::Enter,3=>Key::Tab,4=>Key::Escape,
        5=>Key::LeftCtrl,6=>Key::RightCtrl,7=>Key::LeftShift,8=>Key::RightShift,
        9=>Key::Space,10=>Key::Up,11=>Key::Down,12=>Key::Left,13=>Key::Right,
        20=>Key::MouseLeft,21=>Key::MouseRight,22=>Key::MouseMiddle,
        100..=111=>Key::F((code-99) as u8),
        200..=209=>Key::Num((code-200) as u8),
        1000..=0x10ffff=>Key::Char(char::from_u32((code-1000) as u32)?),
        _=>return None,
    })
}
