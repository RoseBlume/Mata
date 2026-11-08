// Roadmap example. Run: cargo run -p mata-ui --example roadmap_10_scroll
#[path="roadmap_support/mod.rs"] mod support;
use mata_ui::widgets::*;
fn main()->Result<(),std::boxed::Box<dyn std::error::Error>> {
    let w=support::host("10 — Clipped scrollable content")?;
    w.add_content(Rect::new(20,5,650,35),"Scroll over the panel with your mouse wheel.");
    let col=ContentGroup::column(7);
    for i in 0..25 {col.append(Text::new(format!("Row {i:02} — fully retained text 🌷")));}
    let id=w.add_scroll_content(Rect::new(20,65,540,255),col,1050);
    support::drive(w,|host,e|if let WidgetEvent::Scrolled{..}=e {
        println!("Scrolled to {:?}",host.scroll_offset(id));
    });Ok(())
}
