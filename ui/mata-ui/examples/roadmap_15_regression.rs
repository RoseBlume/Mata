// Roadmap example. Run: cargo run -p mata-ui --example roadmap_15_regression
#[path="roadmap_support/mod.rs"] mod support;
use mata_ui::widgets::*;
fn main()->Result<(),std::boxed::Box<dyn std::error::Error>> {
    let w=support::host("15 — Input event regression test window")?;
    w.add_content(Rect::new(15,15,650,50),"Try Tab, Shift+Tab, right-click, edits, and switch states.");
    let b=Button::new(&w,Rect::new(20,85,220,45),"Clickable / focusable")?;
    let _f=TextField::new(&w,Rect::new(260,85,390,45),"Editing regression")?;
    w.set_context_menu(b.id(),vec![MenuItem::new(1,"Test action")]);
    support::drive(w,|host,e|{
        println!("Input: {e:?}");
        if let WidgetEvent::ButtonClicked(_)=e {
            // Capture with: open /tmp/mata-ui-regression.ppm
            let _=qa::save_ppm(&host.framebuffer(),"/tmp/mata-ui-regression.ppm");
        }
    });Ok(())
}
