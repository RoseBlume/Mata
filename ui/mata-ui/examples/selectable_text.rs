//! Demonstrates opt-in text selection and secondary mouse clicks.
//! cargo run -p mata-ui --example selectable_text
use mata_ui::{Window,widgets::{Text,Rect,WidgetEvent,WidgetWindow,WidgetExt}};

fn main() -> Result<(),Box<dyn std::error::Error>> {
    let host=WidgetWindow::from_window(Window::new("Mata selectable text",(680,240)))?;
    let text=Text::new("Select some text 🌷 with your mouse.\nCtrl+C copies the selection. Right-click to copy too.");
    text.set_font_size(20.0);
    text.set_selectable(true); // required: otherwise Text behaves like a static label
    let widget_id=text.widget().id();
    host.add_content(Rect::new(24,36,620,130),&text);
    let mut events=Vec::new();
    loop {
        host.poll_events_into(&mut events);
        for event in &events {
            match event {
                WidgetEvent::Close=>return Ok(()),
                WidgetEvent::ContextMenuRequested{widget:Some(id),..} if *id==widget_id=>{
                    // Here an app can open its own menu; this demo copies directly.
                    if host.copy_selection(){println!("Copied: {}",host.selected_text().unwrap_or_default());}
                }
                WidgetEvent::TextSelectionChanged(id) if *id==widget_id=>{
                    if let Some(selection)=text.selected_text(){println!("Selected: {selection}");}
                }
                _=>{}
            }
        }
        host.present();
    }
}
