//! Live demonstrations of measured text width/height and automatic wrapping.
//! cargo run -p mata-ui --example auto_text_layout
use mata_ui::{Window, widgets::{WidgetWindow, WidgetEvent, Text, ContentGroup, Rect, Button}};

fn main() -> Result<(), std::boxed::Box<dyn std::error::Error>> {
    let window=WidgetWindow::from_window(Window::new("Mata — automatic text layout",(820,600)))?;

    // Both dimensions omitted: shrink to the rendered text, within the window.
    let heading=Text::new("Auto-width and auto-height 🌷");
    heading.set_font_size(23.0);
    let header_id=window.add_auto_content(20,16,None,None,&heading);

    // A fixed width, but height determined by the number of wrapped lines.
    let paragraph=Text::new("Try resizing the window. This paragraph automatically wraps at word boundaries, and its height grows without needing to specify line counts. Text selection works across the wrapped lines, including Unicode 🌷 characters.");
    paragraph.set_selectable(true);
    let paragraph_id=window.add_auto_content(20,75,Some(310),None,&paragraph);

    // Rect width=0 and height=0 is also shorthand for content-derived size.
    let dynamic=Text::new("Click Update to make this text longer.");
    let dynamic_id=window.add_content(Rect::new(360,75,0,0),&dynamic);

    // Parent column now gives each text child its measured height.
    let column=ContentGroup::column(10);
    column.append(Text::new("Column child A: natural height"));
    column.append(Text::new("Column child B: A longer piece of text that can be wrapped and moves the following sibling down rather than overlapping it."));
    column.append(Text::new("Column child C: follows B automatically"));
    let column_id=window.add_auto_content(20,300,Some(600),None,column);

    let update=Button::new(&window,Rect::new(20,530,240,42),"Update dynamic text")?;
    let mut expanded=false;
    let mut events=Vec::new();
    loop {
        window.poll_events_into(&mut events);
        for event in &events {
            match event {
                WidgetEvent::Close=>return Ok(()),
                WidgetEvent::ButtonClicked(id) if *id==update.id()=>{
                    expanded=!expanded;
                    dynamic.set_text(if expanded {
                        "The text was updated. The width stays within the available client area, and the height changes according to the number of wrapped lines. 🌷🌷🌷"
                    } else {"Click Update to make this text longer."});
                    println!("Header: {:?}; paragraph: {:?}; dynamic: {:?}; column: {:?}",
                        window.content_bounds(header_id),window.content_bounds(paragraph_id),
                        window.content_bounds(dynamic_id),window.content_bounds(column_id));
                }
                _=>(),
            }
        }
        window.present();
    }
}
