// Roadmap example. Run: cargo run -p mata-ui --example roadmap_08_theme
#[path="roadmap_support/mod.rs"] mod support;
use mata_ui::widgets::*;
fn main()->Result<(),std::boxed::Box<dyn std::error::Error>> {
    let w=support::host("08 — Semantic light/dark themes")?;
    let button=Button::new(&w,Rect::new(20,90,220,48),"Switch theme")?;
    let _field=TextField::new(&w,Rect::new(265,90,280,48),"Themed text field")?;
    w.set_theme(Theme::dark());let mut light=false;
    support::drive(w,|host,e|if let WidgetEvent::ButtonClicked(id)=e {
        if *id==button.id(){light=!light;host.set_theme(if light{Theme::light()}else{Theme::dark()});}
    });Ok(())
}
