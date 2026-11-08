// Roadmap example. Run: cargo run -p mata-ui --example roadmap_05_context_menu
#[path="roadmap_support/mod.rs"] mod support;
use mata_ui::widgets::*;
fn main()->Result<(),std::boxed::Box<dyn std::error::Error>> {
    let w=support::host("05 — Software context menus")?;
    let b=Button::new(&w,Rect::new(30,70,380,55),"Right-click this button")?;
    w.set_context_menu(b.id(),vec![MenuItem::new(1,"Copy"),
        MenuItem::new(2,"Disabled action").disabled(),MenuItem::new(3,"Properties")]);
    w.add_content(Rect::new(30,15,640,36),"Right-click, use Up/Down/Enter or click an item. Escape closes.");
    support::drive(w,|_,e|if let WidgetEvent::MenuItemActivated{owner,item}=e {
        if *owner==b.id(){println!("Chosen menu item {item}");}
    });Ok(())
}
