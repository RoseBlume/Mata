// Roadmap example. Run: cargo run -p mata-ui --example roadmap_13_unicode
#[path="roadmap_support/mod.rs"] mod support;
use mata_ui::widgets::*;
fn main()->Result<(),std::boxed::Box<dyn std::error::Error>> {
    let w=support::host("13 — Unicode grapheme-safe editing")?;
    w.add_content(Rect::new(20,10,660,42),"Move the caret through combined accents, flags, and joined emoji.");
    let _f=TextField::new(&w,Rect::new(20,85,640,55),"Cafe\u{301} 🇨🇦 👩\u{200d}💻 🌷")?;
    support::drive(w,|_,_|());Ok(())
}
