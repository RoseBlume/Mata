//! Shared loop for the 15 first-roadmap feature demonstrations.
use mata_ui::{Window,widgets::{WidgetEvent,WidgetWindow}};
pub fn host(title:&str)->Result<WidgetWindow,Box<dyn std::error::Error>> {
    Ok(WidgetWindow::from_window(Window::new(title,(700,400)))?)
}
pub fn drive(window:WidgetWindow,mut callback:impl FnMut(&WidgetWindow,&WidgetEvent)) {
    let mut events=Vec::new();
    loop {
        window.poll_events_into(&mut events);
        for event in &events {
            if matches!(event,WidgetEvent::Close) {return;}
            callback(&window,event);
        }
        window.present();
        std::thread::sleep(std::time::Duration::from_millis(8));
    }
}
