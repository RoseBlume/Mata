//! Usage: cargo run --example variable_outline -- /path/to/variable-font.ttf
//! This example intentionally does not bundle or fetch font files.
use std::{env,fs,io};
use mata_docx::opentype::Face;
fn main()->io::Result<()> {
    let path=env::args().nth(1).ok_or_else(||io::Error::new(io::ErrorKind::InvalidInput,"font filename required"))?;
    let mut face=Face::new("Selected font",fs::read(path)?)?;
    for axis in face.variation_axes(){
        println!("axis {:?}: {} to {} (default {})",axis.tag,axis.minimum,axis.maximum,axis.default);
        // Choose a non-default instance to demonstrate actual gvar interpolation.
        if &axis.tag==b"wght" {face.set_variation(axis.tag,axis.maximum)?;}
    }
    let glyph_id=face.glyph('A');
    let bitmap=face.rasterize_glyph_instance(glyph_id,96.0)?;
    let mut pgm=format!("P5\n{} {}\n255\n",bitmap.width,bitmap.height).into_bytes();
    pgm.extend(bitmap.pixels.iter().map(|p|(p&255) as u8));
    fs::write("variable-outline.pgm",pgm)?;
    println!("glyph {glyph_id}: {}x{} pixels, advance {:.1}",bitmap.width,bitmap.height,bitmap.advance_px);
    Ok(())
}
