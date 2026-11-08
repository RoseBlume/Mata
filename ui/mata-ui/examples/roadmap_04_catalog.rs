// Roadmap example. Run: cargo run -p mata-ui --example roadmap_04_catalog
#[path="roadmap_support/mod.rs"] mod support;
use mata_ui::widgets::*;
fn main()->Result<(),std::boxed::Box<dyn std::error::Error>> {
    let w=support::host("04 — Real catalog control models")?;
    w.add_content(Rect::new(20,10,650,35),"Click / Tab+Space on the controls. All paint from current state.");
    let c=std::rc::Rc::new(CheckButton::new("Notifications"));
    let sw=std::rc::Rc::new(Switch::new());
    let scale=std::rc::Rc::new(Scale::new(Orientation::Horizontal,0.0,100.0,30.0));
    let a=w.add_catalog_widget(Rect::new(20,65,320,46),c.clone());
    let b=w.add_catalog_widget(Rect::new(20,130,100,42),sw.clone());
    let d=w.add_catalog_widget(Rect::new(20,190,340,44),scale.clone());
    support::drive(w,|_,e|if let WidgetEvent::CatalogChanged(id)=e {
        if *id==a {println!("Check: {}",c.active());}
        if *id==b {println!("Switch: {}",sw.active());}
        if *id==d {println!("Scale: {}",scale.value());}
    });Ok(())
}
