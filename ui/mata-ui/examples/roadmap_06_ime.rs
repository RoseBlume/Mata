// Roadmap example. Run: cargo run -p mata-ui --example roadmap_06_ime
#[path="roadmap_support/mod.rs"] mod support;
use mata_ui::widgets::*;
fn main()->Result<(),std::boxed::Box<dyn std::error::Error>> {
    let w=support::host("06 — IME integration surface")?;
    w.add_content(Rect::new(20,15,670,38),"A backend can supply preedit then commit; this demo simulates it.");
    let field=TextField::new(&w,Rect::new(20,75,570,45),"")?;
    let button=Button::new(&w,Rect::new(20,140,220,45),"Commit Japanese text")?;
    w.focus_control(field.id())?;
    let mut out=Vec::new();w.update_ime_preedit("かな",&mut out);
    support::drive(w,|host,e|if let WidgetEvent::ButtonClicked(id)=e {
        if *id==button.id(){host.focus_control(field.id()).ok();let mut events=Vec::new();
            host.commit_text("漢字",&mut events);println!("Committed: {}",field.text());}
    });Ok(())
}
