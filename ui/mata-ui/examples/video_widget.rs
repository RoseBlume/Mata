//! Native mata-video playback embedded in MataWin's software widget host.
//!
//! Run with: cargo run -p mata-ui --features video --example video_widget -- path/to/video.mp4
//!
//! Linux requires the GStreamer development libraries and suitable decoder
//! plugins. Windows uses Media Foundation without a GStreamer installation.
use std::{thread, time::Duration};
use mata_ui::{
    Window,
    widgets::{Video, Button, WidgetExt, Rect, WidgetWindow, WidgetEvent},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1)
        .ok_or("usage: video_widget <video-file>")?;
    let window = WidgetWindow::from_window(Window::new("MataWin video", (960, 620)))?;
    let video = Video::from_file(path, true)?;
    video.set_bounds(Rect::new(16, 16, 928, 510));
    video.set_looping(true);
    window.add_video(&video);

    let toggle = Button::new(&window, Rect::new(16, 546, 140, 42), "Play / Pause")?;
    let stop = Button::new(&window, Rect::new(172, 546, 110, 42), "Stop")?;
    let back = Button::new(&window, Rect::new(298, 546, 110, 42), "-5 sec")?;
    let forward = Button::new(&window, Rect::new(424, 546, 110, 42), "+5 sec")?;
    let mut events = Vec::new();

    loop {
        window.poll_events_into(&mut events);
        for event in &events {
            match event {
                WidgetEvent::Close => return Ok(()),
                WidgetEvent::ButtonClicked(id) if *id == toggle.id() => {
                    if video.is_playing() { video.pause(); } else { video.play(); }
                }
                WidgetEvent::ButtonClicked(id) if *id == stop.id() => video.stop()?,
                WidgetEvent::ButtonClicked(id) if *id == back.id() => {
                    video.seek((video.position() - 5.0).max(0.0))?;
                }
                WidgetEvent::ButtonClicked(id) if *id == forward.id() => {
                    let next = video.position() + 5.0;
                    video.seek(video.duration().map_or(next, |d| next.min(d)))?;
                }
                WidgetEvent::VideoError(_, error) => return Err(error.clone().into()),
                _ => {}
            }
        }
        // Decode and show due frames; the widget host requests a repaint only
        // when the frame actually changes.
        window.present();
        thread::sleep(Duration::from_millis(5));
    }
}
