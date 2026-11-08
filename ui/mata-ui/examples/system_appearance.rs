//! Demonstrates OS color preference detection without blocking the event loop.
use mata_ui::{Window, widgets::{WidgetWindow, Theme, SystemAppearanceMonitor, SystemColorScheme, WidgetEvent}};

fn main() -> Result<(), std::boxed::Box<dyn std::error::Error>> {
    let window=WidgetWindow::from_window(Window::new("System appearance", (640, 320)))?;
    window.set_theme(Theme::system());
    let monitor=SystemAppearanceMonitor::new();
    let mut events=Vec::new();
    loop {
        events.clear();
        window.poll_events_into(&mut events);
        if events.iter().any(|e|matches!(e, WidgetEvent::Close)) {break;}
        if let Some(preference)=monitor.poll() {
            let dark=preference.is_some_and(SystemColorScheme::is_dark);
            window.set_theme(if dark {Theme::dark()}else{Theme::light()});
        }
        window.present();
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
    Ok(())
}
