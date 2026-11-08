//! Image-only and mixed Image/Text buttons, composed from existing widgets.
use mata_ui::{Window, widgets::{Button, ContentGroup, Image, ImageData, Rect, Text, WidgetEvent, WidgetWindow}};

fn badge(color:u32) -> Image {
    let mut pixels=vec![0u32; 24*24];
    for y in 0..24usize { for x in 0..24usize {
        if (x as i32-12).pow(2)+(y as i32-12).pow(2) < 104 {
            pixels[y*24+x]=color;
        }
    }}
    let image=Image::new();
    image.set_image(ImageData::from_argb32(24,24,pixels).unwrap());
    image.set_size(24,24);
    image
}
fn main() -> Result<(),Box<dyn std::error::Error>> {
    let window=WidgetWindow::from_window(Window::new("Image child buttons",(360,145)))?;
    let save_icon=badge(0xff42c9a8);
    let open_icon=badge(0xff68aaff);
    let save=Button::new(&window,Rect::new(16,24,42,42), &save_icon)?;
    let open=Button::new(&window,Rect::new(66,24,42,42), open_icon)?;
    let content=ContentGroup::row(6);
    content.append(badge(0xffffca55));
    content.append(Text::new("Confirm"));
    let confirm=Button::new(&window,Rect::new(116,24,185,42),content)?;
    let mut events=Vec::new();
    loop {
        window.poll_events_into(&mut events);
        for event in &events {match event {
            WidgetEvent::Close=>return Ok(()),
            WidgetEvent::ButtonClicked(id) if *id==save.id()=>println!("Save"),
            WidgetEvent::ButtonClicked(id) if *id==open.id()=>println!("Open"),
            WidgetEvent::ButtonClicked(id) if *id==confirm.id()=>println!("Confirm"),
            _=>(),
        }}
        window.present();
    }
}
