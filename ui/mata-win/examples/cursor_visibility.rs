//! Native cursor visibility demo. Escape quits, Space toggles the cursor.
//! Compiles on Windows and Linux (Wayland or X11); Android has no pointer API.
#[cfg(not(target_os = "android"))]
use mata_win::{events::{Key, WindowEvent}, Framebuffer, Window};

#[cfg(target_os = "android")]
fn main() {}

#[cfg(not(target_os = "android"))]
fn main() {
    let mut window = Window::new("MataWin cursor demo - Space toggles, Esc exits", (640, 360));
    let frame = Framebuffer::from_color(640, 360, 0xff182438);
    let mut events = Vec::with_capacity(16);
    loop {
        window.poll_events_into(&mut events);
        for event in events.iter() {
            match event {
                WindowEvent::Close | WindowEvent::KeyPressed(Key::Escape) => {
                    let _ = window.show_cursor();
                    return;
                }
                WindowEvent::KeyPressed(Key::Space) => {
                    if let Err(error) = window.set_cursor_visible(!window.cursor_visible()) {
                        eprintln!("Could not change cursor visibility: {error}");
                    }
                    println!("Cursor shown: {}, pointer over window: {}",
                        window.cursor_visible(), window.cursor_focused());
                }
                _ => {}
            }
        }
        window.present(&frame);
    }
}
