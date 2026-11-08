//! Native macOS AppKit window and Metal BGRA8Unorm presenter.
//!
//! Must be constructed, polled and destroyed on the macOS main thread.
//! Source `Framebuffer::pixels` are `0xAARRGGBB` (native little-endian BGRA bytes).
//! Uses only Apple frameworks and Rust std.
use std::{cell::Cell, collections::HashSet, ffi::{c_char, c_void, CString}, sync::Mutex};
use crate::{buffer::Framebuffer, events::{Key, WindowEvent, EVENT_QUEUE, KEYS_DOWN}, window::DirtyRect};

#[repr(C)]
#[derive(Clone,Copy,Default)]
struct NativeEvent { kind:i32, a:i32, b:i32 }

unsafe extern "C" {
    fn mata_apple_window_new(title:*const c_char,w:u32,h:u32)->*mut c_void;
    fn mata_apple_window_free(p:*mut c_void);
    fn mata_apple_window_pump(p:*mut c_void);
    fn mata_apple_window_events(p:*mut c_void, out:*mut NativeEvent, cap:u32)->u32;
    fn mata_apple_window_size(p:*mut c_void,w:*mut u32,h:*mut u32);
    fn mata_apple_window_present(p:*mut c_void,pixels:*const u32,w:u32,h:u32,rects:*const u32,n:u32);
    fn mata_apple_window_title(p:*mut c_void,title:*const c_char);
    fn mata_apple_window_action(p:*mut c_void,action:i32,a:i32,b:i32);
    fn mata_apple_window_cursor(p:*mut c_void, visible:i32)->i32;
    fn mata_apple_window_cursor_visible(p:*mut c_void)->i32;
    fn mata_apple_window_cursor_focused(p:*mut c_void)->i32;
    fn mata_apple_window_handle(p:*mut c_void)->*mut c_void;
}

pub struct PlatformWindow { native:*mut c_void, last_size:Cell<(u32,u32)>, source_size:Cell<(u32,u32)> }

// Raw pointer is confined to main thread; do not implement Send/Sync.
impl PlatformWindow {
    pub fn new(title:&str,size:(u32,u32))->Self {
        EVENT_QUEUE.get_or_init(||Mutex::new(Vec::with_capacity(32)));
        KEYS_DOWN.get_or_init(||Mutex::new(HashSet::with_capacity(16)));
        let title=CString::new(title).unwrap_or_else(|_|CString::new("MataWin").unwrap());
        let native=unsafe { mata_apple_window_new(title.as_ptr(),size.0,size.1) };
        assert!(!native.is_null(),"unable to create AppKit/Metal window; create Window on the main thread");
        let s=Self { native,last_size:Cell::new(size), source_size:Cell::new(size) };
        s.last_size.set(s.client_size()); s
    }
    pub fn present(&mut self, framebuffer:&Framebuffer) {
        if framebuffer.width==0 || framebuffer.height==0 ||
            framebuffer.pixels.len()!=(framebuffer.width as usize).saturating_mul(framebuffer.height as usize) { return; }
        self.source_size.set((framebuffer.width,framebuffer.height));
        unsafe { mata_apple_window_present(self.native,framebuffer.pixels.as_ptr(),framebuffer.width,framebuffer.height,std::ptr::null(),0); }
    }
    pub fn present_dirty(&mut self, framebuffer:&Framebuffer, dirty:&[DirtyRect]) {
        if framebuffer.width==0 || framebuffer.height==0 ||
            framebuffer.pixels.len()!=(framebuffer.width as usize).saturating_mul(framebuffer.height as usize) { return; }
        if dirty.is_empty() { return; }
        let regions:Vec<u32>=dirty.iter().filter_map(|r|r.clipped(framebuffer.width, framebuffer.height))
            .flat_map(|r|[r.x,r.y,r.width,r.height]).collect();
        if regions.is_empty() { return; }
        self.source_size.set((framebuffer.width,framebuffer.height));
        unsafe { mata_apple_window_present(self.native,framebuffer.pixels.as_ptr(),framebuffer.width,framebuffer.height,regions.as_ptr(),(regions.len()/4) as u32); }
    }
    pub fn frame_ready(&self)->bool { true }
    pub fn client_size(&self)->(u32,u32) {
        let (mut w,mut h)=(0,0);
        unsafe { mata_apple_window_size(self.native,&mut w,&mut h); }
        self.last_size.set((w,h)); (w,h)
    }
    pub fn framebuffer_size(&self)->(u32,u32) { self.source_size.get() }
    pub fn pump_events(&self) {
        unsafe { mata_apple_window_pump(self.native); }
        let mut buf=[NativeEvent::default();128];
        loop {
            let n=unsafe { mata_apple_window_events(self.native,buf.as_mut_ptr(),buf.len() as u32) } as usize;
            for e in &buf[..n] {
                let event=match e.kind {
                    1=>Some(WindowEvent::Close),
                    2=>{self.last_size.set((e.a.max(0) as u32,e.b.max(0) as u32));Some(WindowEvent::Resized(e.a.max(0) as u32,e.b.max(0) as u32))},
                    3=>Some(WindowEvent::Repositioned(e.a,e.b)),
                    4|5=>{
                        let Some(key)=key_from_macos(e.a) else {continue};
                        if let Some(keys)=KEYS_DOWN.get() { let mut keys=keys.lock().unwrap();
                            if e.kind==4 {keys.insert(key);} else {keys.remove(&key);} }
                        Some(if e.kind==4 {WindowEvent::KeyPressed(key)} else {WindowEvent::KeyReleased(key)})
                    }
                    6=>Some(WindowEvent::CursorMoved(e.a,e.b)),
                    7=>Some(WindowEvent::Scroll(e.a,e.b)),
                    8=>Some(WindowEvent::Minimized),
                    9=>Some(WindowEvent::Restored(e.a.max(0) as u32,e.b.max(0) as u32)),
                    _=>None,
                };
                if let Some(event)=event {if let Some(q)=EVENT_QUEUE.get() {q.lock().unwrap().push(event);} }
            }
            if n < buf.len() {break;}
        }
    }
    pub fn set_cursor_visible(&self,visible:bool)->Result<(),String> {
        if unsafe {mata_apple_window_cursor(self.native,visible as i32)}==0 {Ok(())} else {Err("cursor operation failed".into())}
    }
    pub fn cursor_visible(&self)->bool {unsafe{mata_apple_window_cursor_visible(self.native)!=0}}
    pub fn cursor_focused(&self)->bool {unsafe{mata_apple_window_cursor_focused(self.native)!=0}}
    pub fn set_title(&self,title:&str) {if let Ok(s)=CString::new(title){unsafe{mata_apple_window_title(self.native,s.as_ptr());}}}
    fn action(&self,a:i32,b:i32,c:i32) {unsafe{mata_apple_window_action(self.native,a,b,c);}}
    pub fn hide(&self){self.action(1,0,0)}
    pub fn show(&self){self.action(2,0,0)}
    pub fn maximize(&self){self.action(3,0,0)}
    pub fn minimize(&self){self.action(4,0,0)}
    pub fn restore(&self){self.action(5,0,0)}
    pub fn reposition(&self,x:i32,y:i32){self.action(6,x,y)}
    pub fn resize(&self,width:u32,height:u32){self.action(7,width.min(i32::MAX as u32) as i32,height.min(i32::MAX as u32) as i32)}
    pub fn set_decorations(&self,enabled:bool){self.action(8,enabled as i32,0)}
    pub fn set_maximizable(&self,enabled:bool){self.action(9,enabled as i32,0)}
    pub fn set_minimizable(&self,enabled:bool){self.action(10,enabled as i32,0)}
    pub fn set_resizable(&self,enabled:bool){self.action(11,enabled as i32,0)}
    /// Native NSWindow pointer. Valid until the window is dropped.
    pub fn handle(&self)->*mut c_void{unsafe{mata_apple_window_handle(self.native)}}
}
impl Drop for PlatformWindow {fn drop(&mut self){unsafe{mata_apple_window_free(self.native);}}}

fn key_from_macos(code:i32)->Option<Key>{
    Some(match code {
        0x100=>Key::MouseLeft, 0x101=>Key::MouseRight, 0x102=>Key::MouseMiddle,
        36|76=>Key::Enter, 48=>Key::Tab, 49=>Key::Space, 51=>Key::Backspace,53=>Key::Escape,
        123=>Key::Left,124=>Key::Right,125=>Key::Down,126=>Key::Up,
        115=>Key::Home,119=>Key::End,117=>Key::Delete,
        59=>Key::LeftCtrl,62=>Key::RightCtrl,56=>Key::LeftShift,60=>Key::RightShift,
        122=>Key::F(1),120=>Key::F(2),99=>Key::F(3),118=>Key::F(4),96=>Key::F(5),
        97=>Key::F(6),98=>Key::F(7),100=>Key::F(8),101=>Key::F(9),109=>Key::F(10),
        103=>Key::F(11),111=>Key::F(12),
        // Apple virtual key codes map to physical US keyboard locations.
        0=>Key::Char('a'),1=>Key::Char('s'),2=>Key::Char('d'),3=>Key::Char('f'),
        4=>Key::Char('h'),5=>Key::Char('g'),6=>Key::Char('z'),7=>Key::Char('x'),
        8=>Key::Char('c'),9=>Key::Char('v'),11=>Key::Char('b'),12=>Key::Char('q'),
        13=>Key::Char('w'),14=>Key::Char('e'),15=>Key::Char('r'),16=>Key::Char('y'),
        17=>Key::Char('t'),31=>Key::Char('o'),32=>Key::Char('u'),34=>Key::Char('i'),
        35=>Key::Char('p'),37=>Key::Char('l'),38=>Key::Char('j'),40=>Key::Char('k'),
        45=>Key::Char('n'),46=>Key::Char('m'),42=>Key::Char('\\'),
        18=>Key::Num(1),19=>Key::Num(2),20=>Key::Num(3),21=>Key::Num(4),
        23=>Key::Num(5),22=>Key::Num(6),26=>Key::Num(7),28=>Key::Num(8),
        25=>Key::Num(9),29=>Key::Num(0),
        _=>return None
    })
}
