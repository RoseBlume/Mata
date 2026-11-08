//! Run: cargo run -p mata-ui --example scroll_sensitivity
//! The default 0.6 multiplier scrolls much less than the old raw wheel code.
#[path = "roadmap_support/mod.rs"] mod support;
use mata_ui::widgets::*;

fn main() -> Result<(), std::boxed::Box<dyn std::error::Error>> {
    let window = support::host("Scroll sensitivity | smaller wheel movement")?;
    let slower = Button::new(&window, Rect::new(20, 48, 130, 36), "Slower")?;
    let faster = Button::new(&window, Rect::new(165, 48, 130, 36), "Faster")?;
    let reset = Button::new(&window, Rect::new(310, 48, 130, 36), "Reset")?;
    let left = ContentGroup::column(7);
    let right = ContentGroup::column(7);
    for n in 0..60 {
        left.append(Text::new(format!("Shared sensitivity — row {n:02} 🌷")));
        right.append(Text::new(format!("Per-view 0.25 — row {n:02} 🌷")));
    }
    let scroll_a = window.add_scroll_content(Rect::new(20, 112, 310, 245), left, 2200);
    let scroll_b = window.add_scroll_content(Rect::new(350, 112, 310, 245), right, 2200);
    window.set_content_scroll_sensitivity(scroll_b, Some(0.25))?;
    window.add_content(Rect::new(20, 10, 640, 30),
        "Left: window sensitivity (default 0.6). Right: fixed at 0.25.");
    support::drive(window, |host, event| match event {
        WidgetEvent::ButtonClicked(id) if *id == slower.id() || *id == faster.id() || *id == reset.id() => {
            let next = if *id == reset.id() { 0.6 }
                else if *id == slower.id() { (host.scroll_sensitivity() - 0.1).max(0.0) }
                else { (host.scroll_sensitivity() + 0.1).min(10.0) };
            if let Err(error) = host.set_scroll_sensitivity(next) { eprintln!("{error}"); }
            println!("Window sensitivity: {:.2}", host.scroll_sensitivity());
        }
        WidgetEvent::Scrolled { .. } => println!("Offsets: {:?}, {:?}",
            host.scroll_offset(scroll_a), host.scroll_offset(scroll_b)),
        _ => {}
    });
    Ok(())
}
