// Roadmap example. Run: cargo run -p mata-ui --example roadmap_09_states
#[path="roadmap_support/mod.rs"] mod support;
use mata_ui::widgets::*;
fn main()->Result<(),std::boxed::Box<dyn std::error::Error>> {
    let w=support::host("09 — Hover/focus/disabled states")?;
    let _a=Button::new(&w,Rect::new(20,80,220,55),"Hover + keyboard focus")?;
    let disabled=Button::new(&w,Rect::new(265,80,220,55),"Disabled button")?;
    disabled.set_enabled(false);
    w.add_content(Rect::new(20,10,650,38),"Hover, click and Tab; the disabled button isn't focusable.");
    support::drive(w,|_,_|());Ok(())
}
