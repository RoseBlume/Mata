// Roadmap example. Run: cargo run -p mata-ui --example roadmap_11_accessibility
#[path="roadmap_support/mod.rs"] mod support;
use mata_ui::widgets::*;
fn main()->Result<(),std::boxed::Box<dyn std::error::Error>> {
    let w=support::host("11 — In-process accessibility semantics")?;
    let b=Button::new(&w,Rect::new(25,80,340,50),"Open settings")?;
    w.set_accessibility(b.id(),AccessibilityNode::new(AccessibilityRole::Button,"Open settings")
        .description("Opens the application's preferences"));
    println!("Accessible nodes: {:?}",w.accessibility_snapshot());
    w.add_content(Rect::new(20,10,660,38),"A11y metadata is available; native screen-reader bridge is pending.");
    support::drive(w,|_,_|());Ok(())
}
