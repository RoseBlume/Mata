// Roadmap example. Run: cargo run -p mata-ui --example roadmap_12_dialogs
#[path="roadmap_support/mod.rs"] mod support;
use mata_ui::widgets::*;
fn main()->Result<(),std::boxed::Box<dyn std::error::Error>> {
    let w=support::host("12 — Desktop file dialogs")?;
    let b=Button::new(&w,Rect::new(20,85,340,50),"Choose a file")?;
    // Width is constrained; height grows to fit every wrapped line.
    w.add_auto_content(20, 15, Some(650), None,
        "Linux: XDG Desktop Portal first; helper dialogs as fallback. Windows: native file dialogs.");
    support::drive(w,|host,e|if let WidgetEvent::ButtonClicked(id)=e {
        if *id==b.id(){
            #[cfg(any(target_os="linux",target_os="windows"))]
            match open_files_dialog(host).filter_extensions(["txt","md","rs"]).show() {
                Ok(value)=>println!("Picked: {value:?}"),Err(error)=>eprintln!("Dialog: {error}")
            }
        }
    });Ok(())
}
