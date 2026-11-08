//! Inspect a variable TrueType or CFF2 outline at a requested axis value.
//! cargo run --example cff2_outline -- path/to/font.otf A wght 700
use mata_docx::opentype::Face;
use std::io::{self,Error,ErrorKind};
fn main()->io::Result<()>{
    let args=std::env::args().collect::<Vec<_>>();
    if args.len()<3{
        eprintln!("usage: cff2_outline FONT_PATH CHARACTER [AXIS_TAG VALUE]");
        return Ok(());
    }
    let bytes=std::fs::read(&args[1])?;
    let mut face=Face::new("Example",bytes)?;
    if args.len()>=5{
        let tag=args[3].as_bytes();
        if tag.len()!=4{return Err(Error::new(ErrorKind::InvalidInput,"axis tag must have 4 bytes"));}
        let mut axis=[0u8;4];axis.copy_from_slice(tag);
        let value=args[4].parse::<f32>().map_err(|_|Error::new(ErrorKind::InvalidInput,"invalid axis value"))?;
        face.set_variation(axis,value)?;
    }
    let character=args[2].chars().next().ok_or_else(||Error::new(ErrorKind::InvalidInput,"character missing"))?;
    let id=face.glyph(character);
    let path=face.outline_instance(id)?;
    let bitmap=face.rasterize_glyph_instance(id,64.0)?;
    println!("glyph {id}: {} points, {} contours, bitmap {}x{}",path.points.len(),path.contours.len(),bitmap.width,bitmap.height);
    Ok(())
}
