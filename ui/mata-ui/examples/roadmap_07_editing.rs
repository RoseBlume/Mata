// Roadmap example. Run: cargo run -p mata-ui --example roadmap_07_editing
#[path="roadmap_support/mod.rs"] mod support;
use mata_ui::widgets::*;
fn main()->Result<(),std::boxed::Box<dyn std::error::Error>> {
    let w=support::host("07 — Selection and editing shortcuts")?;
    w.add_content(Rect::new(20,10,665,45),"Try Shift+arrows, Ctrl+arrows, Home/End, Delete and mouse drag.");
    let _field=TextField::new(&w,Rect::new(20,80,630,44),"Edit this text and select multiple words")?;
    support::drive(w,|_,e|if let WidgetEvent::TextChanged(_)=e {println!("Edited")});Ok(())
}
