// Roadmap example. Run: cargo run -p mata-ui --example roadmap_03_nested
#[path="roadmap_support/mod.rs"] mod support;
use mata_ui::widgets::*;
fn main()->Result<(),std::boxed::Box<dyn std::error::Error>> {
    let w=support::host("03 — Interactive nested child controls")?;
    w.add_content(Rect::new(20,15,650,38),"Controls in a retained ContentGroup are real focusable widgets.");
    let first=Button::new(&w,Rect::new(0,0,140,45),"Nested Save")?;
    let field=TextField::new(&w,Rect::new(0,0,200,45),"Editable child")?;
    let row=ContentGroup::row(12);
    row.append(&first);row.append(&field);
    w.add_content(Rect::new(20,80,640,60),row);
    support::drive(w,|_,e|if let WidgetEvent::ButtonClicked(id)=e {
        if *id==first.id(){println!("Saved text: {}",field.text());}
    });Ok(())
}
