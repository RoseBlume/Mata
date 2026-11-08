// Roadmap example. Run: cargo run -p mata-ui --example roadmap_01_focus
#[path="roadmap_support/mod.rs"] mod support;
use mata_ui::widgets::*;
fn main()->Result<(),std::boxed::Box<dyn std::error::Error>> {
    let w=support::host("01 — Tab and Shift+Tab focus")?;
    let _label=w.add_content(Rect::new(25,10,600,30),"Tab moves between fields, buttons, and catalog toggles.");
    let _a=TextField::new(&w,Rect::new(20,70,250,42),"First input")?;
    let _b=Button::new(&w,Rect::new(300,70,180,42),"Second: button")?;
    let c=std::rc::Rc::new(Switch::new());
    w.add_catalog_widget(Rect::new(520,70,100,42),c);
    support::drive(w,|_,e| if let WidgetEvent::KeyPressed(_)=e {});
    Ok(())
}
