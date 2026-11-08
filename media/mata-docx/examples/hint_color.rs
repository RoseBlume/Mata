use mata_docx::{opentype::Face,truetype_hinting::HintLimits};
use std::{env,fs};
fn main()->std::io::Result<()> {
    let font=env::args().nth(1).expect("usage: hint_color <font.ttf> [glyph-id]");
    let glyph=env::args().nth(2).and_then(|v|v.parse().ok()).unwrap_or(36u16);
    let face=Face::new("Example",fs::read(font)?)?;
    let ppem=20.0;
    match face.rasterize_hinted_glyph_instance(glyph,ppem,HintLimits::default()){
        Ok(bitmap)=>println!("hinted glyph: {} x {}, advance {}",bitmap.width,bitmap.height,bitmap.advance_px),
        Err(error)=>eprintln!("TrueType hinting unavailable for this glyph: {error}"),
    }
    if let Some(bitmap)=face.rasterize_colr_glyph_instance(glyph,ppem,0,0x000000ff)? {
        println!("color glyph: {} x {}",bitmap.width,bitmap.height);
    }
    if let Some(doc)=face.svg_glyph_document(glyph)? {
        println!("SVG glyph record {}–{}; {} bytes",doc.first_glyph,doc.last_glyph,doc.xml.len());
    }
    Ok(())
}
