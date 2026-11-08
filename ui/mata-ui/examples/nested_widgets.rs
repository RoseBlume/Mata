//! A button can now own caller-created Text/Image widgets or an entire layout.
//! The image is generated in memory so this example needs no mata_image decoder.

use mata_ui::{
    Window,
    widgets::{
        Button, ContentGroup, Image, ImageData, Rect, Text, WidgetEvent, WidgetWindow,
        Box as WidgetBox, Orientation,
    },
};

fn icon() -> Image {
    const SIZE: usize = 24;
    let mut pixels = vec![0u32; SIZE * SIZE];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let circle = (x as i32 - 12).pow(2) + (y as i32 - 12).pow(2) < 110;
            pixels[y * SIZE + x] = if circle { 0xff48c9a8 } else { 0x00000000 };
        }
    }
    let icon = Image::new();
    icon.set_image(ImageData::from_argb32(24, 24, pixels).unwrap());
    icon
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let window = WidgetWindow::from_window(Window::new("Nested MataWin widgets", (680, 280)))?;

    // Text is now your own widget, not a baked-in string on Button.
    let label = Text::new("Save changes");
    label.set_font_family("Arial");
    label.set_font_size(21.0);

    let row = WidgetBox::new(Orientation::Horizontal, 12);
    let badge = icon();
    row.append(&badge);
    row.append(&label);

    // The Button owns a retained child tree; nested content clicks the Button.
    let save = Button::new(&window, Rect::new(30, 65, 280, 72), row)?;

    let heading = Text::new("Build your own contents");
    heading.set_font_family("Liberation Sans");
    heading.set_font_size(20.0);
    let explanation = Text::new("Text + image + row + column");
    explanation.set_font_size(14.0);
    let column = ContentGroup::column(4);
    column.append(heading);
    column.append(explanation);
    let description = Button::new(&window, Rect::new(335, 65, 315, 72), column)?;

    // Standalone trees (not just button contents) can be added to the window.
    let help = Text::new("Click a button to update its own child Text widget.");
    help.set_font_size(15.0);
    window.add_content(Rect::new(30, 165, 620, 38), help);

    let mut events = Vec::new();
    loop {
        window.poll_events_into(&mut events);
        for event in &events {
            match event {
                WidgetEvent::Close => return Ok(()),
                WidgetEvent::ButtonClicked(id) if *id == save.id() => {
                    // The retained text handle drives redraw automatically.
                    label.set_text("Saved!");
                }
                WidgetEvent::ButtonClicked(id) if *id == description.id() => {
                    label.set_text("Clicked the other button");
                }
                _ => (),
            }
        }
        window.present();
    }
}
