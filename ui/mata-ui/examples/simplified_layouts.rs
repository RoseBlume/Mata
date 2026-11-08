use mata_ui::{Window, widgets::{WidgetWindow, Rect, Text, Button, FlexLayout, FlexItem, GridLayout, AlignItems, Insets, Stylable}};

fn main() -> Result<(), std::boxed::Box<dyn std::error::Error>> {
    let window = WidgetWindow::from_window(Window::new("mata-ui simplified layouts", (980, 620)))?;
    let header = Text::new("Flex and grid helpers");
    header.set_font_size(26.0);
    window.add_auto_content(28, 24, Some(500), None, header);

    let cards = GridLayout::new(Rect::new(28, 90, 924, 120), 3, 1).gap(16, 0).cells(3);
    for (i, rect) in cards.into_iter().enumerate() {
        let button = Button::new(&window, rect, format!("Card {}", i + 1))?;
        button.update_style(|s| {
            s.border_radius = Some(mata_ui::widgets::style::BorderRadius::all(mata_ui::widgets::style::Length::Px(16.0)));
        });
    }

    let row = FlexLayout::row(Rect::new(28, 248, 924, 44))
        .gap(12)
        .align(AlignItems::Center)
        .padding(Insets::all(0))
        .layout(&[FlexItem::fixed(170), FlexItem::fill(), FlexItem::fixed(170)]);
    Button::new(&window, row[0], "Primary")?;
    Button::new(&window, row[1], "Fluid middle")?;
    Button::new(&window, row[2], "Secondary")?;

    loop {
        let mut events = Vec::new();
        window.poll_events_into(&mut events);
        if events.iter().any(|e| matches!(e, mata_ui::widgets::WidgetEvent::Close)) { break; }
        window.present();
    }
    Ok(())
}
