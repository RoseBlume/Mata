// Roadmap example. Run: cargo run -p mata-ui --example roadmap_02_keyboard
#[path="roadmap_support/mod.rs"] mod support;
use mata_ui::widgets::*;
fn main()->Result<(),std::boxed::Box<dyn std::error::Error>> {
    let w=support::host("02 — Enter/Space activation")?;
    w.add_content(Rect::new(20,10,650,40),"Tab to focus, Enter or Space to activate.");
    let btn=Button::new(&w,Rect::new(25,70,260,50),"Activate with Enter")?;
    let check=std::rc::Rc::new(CheckButton::new("Keyboard checkbox"));
    let id=w.add_catalog_widget(Rect::new(330,70,280,50),check.clone());
    support::drive(w,|_,e|match e {
        WidgetEvent::ButtonClicked(n) if *n==btn.id()=>println!("Button activated!"),
        WidgetEvent::CatalogChanged(n) if *n==id=>println!("Checked: {}",check.active()),
        _=>()
    });Ok(())
}
