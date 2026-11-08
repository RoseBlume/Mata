//! macOS Metal / ARGB framebuffer smoke test.
//! cargo run --release --target aarch64-apple-darwin --example macos_metal
#![cfg(target_os="macos")]
use mata_win::{Framebuffer, Window};
use mata_win::events::WindowEvent;
fn main(){
    let mut win=Window::new("MataWin — Metal ARGB",(900,540));
    let mut fb=Framebuffer::new(900,540);
    let mut time=0u32;
    'main:loop {
        for event in win.poll_events() {
            if let WindowEvent::Close=event {break 'main;}
        }
        let (w,h)=win.client_size();
        if w==0||h==0 {std::thread::sleep(std::time::Duration::from_millis(20));continue;}
        if (w,h)!=(fb.width,fb.height){fb.resize(w,h);}
        for y in 0..h {for x in 0..w {
            let r=((x+time)/3%256) as u32;
            let g=((y+time)/2%256) as u32;
            fb.pixels[(y*w+x) as usize]=0xff00_0000|(r<<16)|(g<<8)|0x60;
        }}
        win.present(&fb); time=time.wrapping_add(1);
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
}
