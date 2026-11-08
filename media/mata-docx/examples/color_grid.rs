//! Document-grid roundtrip and shaped, variable-font raster export.
//! Supply your own licensed OpenType font; no font is bundled.
use std::{env,fs};
use mata_docx::{Document,DocumentGrid,layout::Engine};
fn main()->Result<(),Box<dyn std::error::Error>>{
    let font_path=env::args().nth(1).ok_or("usage: cargo run --example color_grid -- path/to/font.ttf")?;
    let mut doc=Document::new();
    doc.final_section_mut().doc_grid=Some(DocumentGrid{
        kind:"lines".into(),line_pitch_twips:Some(360),char_space:None
    });
    doc.add_heading("Shaped variable-font text",1);
    doc.add_paragraph("English العربية हिन्दी বাংলা नमस्ते かきく 漢字");
    let mut engine=Engine::new();
    engine.fonts.add("ExampleFont",fs::read(font_path)?)?;
    let pages=doc.to_png_pages_shaped(&engine,96.0)?;
    for (index,page) in pages.into_iter().enumerate(){
        fs::write(format!("mata-docx-shaped-{}.png",index+1),page)?;
    }
    Ok(())
}
