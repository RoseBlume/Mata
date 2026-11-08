// Roadmap example. Run: cargo run -p mata-ui --example roadmap_14_builder
#[path="roadmap_support/mod.rs"] mod support;
use mata_ui::widgets::*;
fn main()->Result<(),std::boxed::Box<dyn std::error::Error>> {
    let w=support::host("14 — Concise typed widget builders")?;
    let _field=TextField::builder(&w).bounds(Rect::new(20,70,300,45))
        .text("Built without temporary setters").build()?;
    let button=Button::builder(&w,"Builder button").bounds(Rect::new(340,70,230,45)).build()?;
    support::drive(w,|_,e|if let WidgetEvent::ButtonClicked(id)=e {
        if *id==button.id(){println!("Builder button clicked");}
    });Ok(())
}
