//! OpenType COLR v0 + CPAL v0/v1 color outlines using the native contour
//! rasterizer. Unsupported COLR v1 paint graphs return `Ok(None)`.
use std::io::{self,Error,ErrorKind};
use crate::opentype::Face;
fn invalid(s:&str)->io::Error{Error::new(ErrorKind::InvalidData,s)}
fn u16be(d:&[u8],at:usize)->Option<u16>{let s=d.get(at..at.checked_add(2)?)?;Some(u16::from_be_bytes([s[0],s[1]]))}
fn u32be(d:&[u8],at:usize)->Option<u32>{let s=d.get(at..at.checked_add(4)?)?;Some(u32::from_be_bytes([s[0],s[1],s[2],s[3]]))}
/// Pixels use `0xRRGGBBAA`, like the Mata framebuffer format.
#[derive(Debug,Clone)]pub struct ColorBitmap{
    pub width:u32,pub height:u32,pub pixels:Vec<u32>,
    pub bearing_x:i32,pub bearing_y:i32,pub advance_px:f32,
}
pub(crate) fn layer_color(cpal:&[u8],palette:usize,index:u16,foreground:u32)->Option<u32>{
    if index==0xffff{return Some(foreground);}
    let num_entries=u16be(cpal,2)? as usize;
    let palettes=u16be(cpal,4)? as usize;
    let records=u16be(cpal,6)? as usize;
    if palette>=palettes||index as usize>=num_entries{return None;}
    let first=u16be(cpal,12+palette*2)? as usize;
    let record=first.checked_add(index as usize)?;
    if record>=records{return None;}
    let at=(u32be(cpal,8)? as usize).checked_add(record.checked_mul(4)?)?;
    let c=cpal.get(at..at+4)?;
    Some(((c[2] as u32)<<24)|((c[1] as u32)<<16)|((c[0] as u32)<<8)|(c[3] as u32))
}
pub(crate) fn blend_over(dst:u32,src:u32)->u32{
    let sa=src&255;let da=dst&255;
    let out_a=sa+(da*(255-sa)+127)/255;
    if out_a==0{return 0;}
    let mut out=0u32;
    for shift in [24,16,8]{
        let sc=(src>>shift)&255;let dc=(dst>>shift)&255;
        let numerator=sc*sa*255+dc*da*(255-sa);
        let denom=255*out_a;
        out|=((numerator+denom/2)/denom).min(255)<<shift;
    }
    out|out_a
}
/// Rasterize COLR v0 layer records using CPAL palette colors and a caller
/// selected foreground for palette index 0xffff. Returns `None` for a plain
/// outline font or unsupported COLR version.
pub fn rasterize(face:&Face,glyph:u16,ppem:f32,palette:usize,foreground:u32)->io::Result<Option<ColorBitmap>>{
    let Some(colr)=face.table(*b"COLR")else{return Ok(None)};
    let Some(cpal)=face.table(*b"CPAL")else{return Ok(None)};
    if u16be(colr,0)==Some(1){
        if let Some(v1)=crate::colr_v1::rasterize(face,glyph,ppem,palette,foreground)? {return Ok(Some(v1));}
    }
    if !matches!(u16be(colr,0),Some(0|1))||u16be(cpal,0).is_none(){return Ok(None);}
    let count=u16be(colr,2).ok_or_else(||invalid("truncated COLR base glyph count"))? as usize;
    let base_off=u32be(colr,4).ok_or_else(||invalid("truncated COLR base offset"))? as usize;
    let layer_off=u32be(colr,8).ok_or_else(||invalid("truncated COLR layer offset"))? as usize;
    let total=u16be(colr,12).ok_or_else(||invalid("truncated COLR layers"))? as usize;
    if count>65535||total>65535{return Err(invalid("COLR record limit exceeded"));}
    let mut lo=0usize;let mut hi=count;
    while lo<hi{
        let mid=(lo+hi)/2;
        let Some(at)=base_off.checked_add(mid*6)else{return Err(invalid("COLR offset overflow"))};
        let Some(id)=u16be(colr,at)else{return Err(invalid("COLR base record truncated"))};
        if id<glyph{lo=mid+1}else{hi=mid;}
    }
    if lo>=count{return Ok(None);}
    let Some(at)=base_off.checked_add(lo*6)else{return Err(invalid("COLR offset overflow"))};
    if u16be(colr,at)!=Some(glyph){return Ok(None);}
    let first=u16be(colr,at+2).ok_or_else(||invalid("COLR layer index missing"))? as usize;
    let n=u16be(colr,at+4).ok_or_else(||invalid("COLR layer count missing"))? as usize;
    if n>4096 || first.checked_add(n).map(|v|v>total).unwrap_or(true){return Err(invalid("COLR invalid glyph layer range"));}
    if n==0{return Ok(None);}
    let mut images=Vec::with_capacity(n);
    let (mut left,mut top,mut right,mut bottom)=(i32::MAX,i32::MAX,i32::MIN,i32::MIN);
    for i in first..first+n{
        let at=layer_off.checked_add(i.checked_mul(4).ok_or_else(||invalid("COLR layer offset overflow"))?).ok_or_else(||invalid("COLR offset overflow"))?;
        let id=u16be(colr,at).ok_or_else(||invalid("COLR layer glyph truncated"))?;
        let idx=u16be(colr,at+2).ok_or_else(||invalid("COLR layer palette index truncated"))?;
        let color=layer_color(cpal,palette,idx,foreground).ok_or_else(||invalid("invalid CPAL palette/index"))?;
        let bitmap=face.rasterize_glyph_instance(id,ppem)?;
        if bitmap.width==0||bitmap.height==0{continue;}
        let x=bitmap.bearing_x;let y=-bitmap.bearing_y;
        left=left.min(x);top=top.min(y);
        right=right.max(x.saturating_add(bitmap.width as i32));bottom=bottom.max(y.saturating_add(bitmap.height as i32));
        images.push((bitmap,color));
    }
    let advance=face.advance_units_instance(glyph)*ppem/face.units_per_em as f32;
    if images.is_empty(){return Ok(Some(ColorBitmap{width:0,height:0,pixels:Vec::new(),bearing_x:0,bearing_y:0,advance_px:advance}));}
    let width=right.saturating_sub(left) as usize;let height=bottom.saturating_sub(top) as usize;
    if width>4096||height>4096||width.checked_mul(height).map(|n|n>16_777_216).unwrap_or(true){return Err(invalid("COLR bitmap exceeds raster limit"));}
    let mut pixels=vec![0u32;width*height];
    for (bitmap,color) in images{
        let at_x=(bitmap.bearing_x-left) as usize;let at_y=(-bitmap.bearing_y-top) as usize;
        for y in 0..bitmap.height as usize{for x in 0..bitmap.width as usize{
            let coverage=bitmap.pixels[y*bitmap.width as usize+x].min(255);
            if coverage==0{continue;}
            let sa=((color&255)*coverage+127)/255;
            let src=(color&!255)|sa;
            let at=(y+at_y)*width+x+at_x;
            pixels[at]=blend_over(pixels[at],src);
        }}
    }
    Ok(Some(ColorBitmap{width:width as u32,height:height as u32,pixels,
        bearing_x:left,bearing_y:-top,advance_px:advance}))
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn bgra_palette_and_foreground(){
        let mut table=vec![0u8;24];
        table[2..4].copy_from_slice(&1u16.to_be_bytes());
        table[4..6].copy_from_slice(&1u16.to_be_bytes());
        table[6..8].copy_from_slice(&1u16.to_be_bytes());
        table[8..12].copy_from_slice(&20u32.to_be_bytes());
        table[20..24].copy_from_slice(&[4,3,2,128]);
        assert_eq!(layer_color(&table,0,0,0x112233ff),Some(0x02030480));
        assert_eq!(layer_color(&table,0,0xffff,0x112233ff),Some(0x112233ff));
    }
    #[test]
    fn alpha_compositing(){
        assert_eq!(blend_over(0x00000000,0xff0000ff),0xff0000ff);
        assert_eq!(blend_over(0x0000ffff,0xff0000ff),0xff0000ff);
    }
}
