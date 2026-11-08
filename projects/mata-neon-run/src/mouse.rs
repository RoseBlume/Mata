//! Infinite relative look without allowing the OS cursor to hit a screen edge.
//! No Cargo dependencies; Win32 and Xlib are used only on their own platforms.
//! The pointer returns to its initial location each poll, while the user sees
//! a hidden cursor. Warp-generated CursorMoved events are deliberately ignored.
use mata_win::{Window,events::WindowEvent};

#[cfg(target_os="windows")]
mod native {
    #[repr(C)] #[derive(Copy,Clone)] struct Point { x:i32,y:i32 }
    #[link(name="user32")]
    unsafe extern "system" {
        fn GetCursorPos(point:*mut Point)->i32;
        fn SetCursorPos(x:i32,y:i32)->i32;
    }
    pub struct Relative { anchor:Option<(i32,i32)> }
    impl Relative {
        pub fn new()->Self {Self{anchor:None}}
        fn point()->Option<(i32,i32)> {
            let mut p=Point{x:0,y:0};
            if unsafe{GetCursorPos(&mut p)}!=0{Some((p.x,p.y))}else{None}
        }
        pub fn begin(&mut self){self.anchor=Self::point();}
        pub fn end(&mut self){self.anchor=None;}
        pub fn sample(&mut self)->Option<(f32,f32)>{
            let anchor=self.anchor?;
            let now=Self::point()?;
            let dx=(now.0-anchor.0).clamp(-500,500) as f32;
            let dy=(now.1-anchor.1).clamp(-500,500) as f32;
            if dx!=0.0||dy!=0.0 {unsafe{SetCursorPos(anchor.0,anchor.1)};}
            Some((dx,dy))
        }
    }
}
#[cfg(target_os="linux")]
mod native {
    use std::ffi::{c_char,c_int,c_uint,c_ulong,c_void};
    #[link(name="X11")]
    unsafe extern "C" {
        fn XOpenDisplay(name:*const c_char)->*mut c_void;
        fn XCloseDisplay(display:*mut c_void)->c_int;
        fn XDefaultScreen(display:*mut c_void)->c_int;
        fn XRootWindow(display:*mut c_void,screen:c_int)->c_ulong;
        fn XQueryPointer(display:*mut c_void,w:c_ulong,root_return:*mut c_ulong,
            child_return:*mut c_ulong,root_x:*mut c_int,root_y:*mut c_int,
            win_x:*mut c_int,win_y:*mut c_int,mask:*mut c_uint)->c_int;
        fn XWarpPointer(display:*mut c_void,src:c_ulong,dst:c_ulong,sx:c_int,sy:c_int,
            sw:c_uint,sh:c_uint,dx:c_int,dy:c_int)->c_int;
        fn XFlush(display:*mut c_void)->c_int;
    }
    pub struct Relative { display:*mut c_void,root:c_ulong,anchor:Option<(i32,i32)> }
    impl Relative {
        pub fn new()->Self {
            // XOpenDisplay fails on Wayland-only sessions. In that case Mouse
            // will use the existing CursorMoved event fallback instead.
            let display=unsafe{XOpenDisplay(std::ptr::null())};
            let root=if display.is_null(){0}else{unsafe{XRootWindow(display,XDefaultScreen(display))}};
            Self{display,root,anchor:None}
        }
        fn point(&self)->Option<(i32,i32)> {
            if self.display.is_null(){return None}
            let(mut root,mut child,mut x,mut y,mut wx,mut wy,mut mask)=(0,0,0,0,0,0,0);
            let ok=unsafe{XQueryPointer(self.display,self.root,&mut root,&mut child,&mut x,
                &mut y,&mut wx,&mut wy,&mut mask)};
            if ok!=0{Some((x,y))}else{None}
        }
        pub fn begin(&mut self){self.anchor=self.point();}
        pub fn end(&mut self){self.anchor=None;}
        pub fn sample(&mut self)->Option<(f32,f32)> {
            let anchor=self.anchor?;
            let now=self.point()?;
            let dx=(now.0-anchor.0).clamp(-500,500) as f32;
            let dy=(now.1-anchor.1).clamp(-500,500) as f32;
            if dx!=0.0||dy!=0.0 {
                unsafe{
                    XWarpPointer(self.display,0,self.root,0,0,0,0,anchor.0,anchor.1);
                    XFlush(self.display);
                }
            }
            Some((dx,dy))
        }
    }
    impl Drop for Relative{fn drop(&mut self){if !self.display.is_null(){unsafe{XCloseDisplay(self.display)};}}}
}
#[cfg(not(any(target_os="windows",target_os="linux")))]
mod native {
    pub struct Relative;
    impl Relative{
        pub fn new()->Self{Self}
        pub fn begin(&mut self){}
        pub fn end(&mut self){}
        pub fn sample(&mut self)->Option<(f32,f32)>{None}
    }
}

pub struct Mouse {native:native::Relative,last:Option<(i32,i32)>,hidden:bool}
impl Mouse {
    pub fn new(_: &Window)->Self{Self{native:native::Relative::new(),last:None,hidden:false}}
    pub fn sample(&mut self,window:&Window,active:bool,events:&[WindowEvent])->(f32,f32){
        let want_hidden=active&&window.cursor_focused();
        if self.hidden!=want_hidden{
            if let Err(err)=window.set_cursor_visible(!want_hidden){
                eprintln!("[Neon Run input] cursor visibility: {err}");
            }
            self.hidden=want_hidden;
            self.last=None;
            if want_hidden {self.native.begin();}else{self.native.end();}
        }
        if !want_hidden{return (0.0,0.0)}
        if let Some(delta)=self.native.sample(){return delta;}
        // Unsupported platforms: retain bounded event delta behaviour until
        // their window backend exposes a relative-pointer protocol.
        let mut out=(0.0,0.0);
        for &event in events {
            if let WindowEvent::CursorMoved(x,y)=event {
                if let Some((px,py))=self.last {
                    out.0+=(x-px).clamp(-100,100) as f32;
                    out.1+=(y-py).clamp(-100,100) as f32;
                }
                self.last=Some((x,y));
            }
        }
        out
    }
}
