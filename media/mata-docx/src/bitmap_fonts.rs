//! Color bitmap strikes from OpenType sbix and CBDT/CBLC.
//!
//! Decodes PNG/JPEG/TIFF payloads with the Rust `mata-image` crate. All
//! decoding is in-process: no system codec FFI is introduced by mata-docx.
use std::io::{self,Error,ErrorKind};
use crate::{opentype::Face,colr::ColorBitmap};
fn bad(message:&str)->io::Error{Error::new(ErrorKind::InvalidData,message)}
fn u16be(data:&[u8],p:usize)->Option<u16>{let v=data.get(p..p.checked_add(2)?)?;Some(u16::from_be_bytes([v[0],v[1]]))}
fn u32be(data:&[u8],p:usize)->Option<u32>{let v=data.get(p..p.checked_add(4)?)?;Some(u32::from_be_bytes([v[0],v[1],v[2],v[3]]))}
#[derive(Debug,Clone)]struct Png {width:u32,height:u32,pixels:Vec<u32>}
/// Decode embedded PNG at any bit depth/interlacing supported by mata-image,
/// keeping a glyph-sized bound on the resulting pixel buffer.
fn decode_png(encoded:&[u8])->io::Result<Png>{
    if !encoded.starts_with(b"\x89PNG\r\n\x1a\n")||encoded.len()>32*1024*1024{
        return Err(bad("invalid or oversized OpenType PNG"));
    }
    let image=mata_image::Image::from_bytes(encoded).map_err(|e|
        Error::new(ErrorKind::InvalidData,format!("OpenType PNG decoding failed: {e}")))?;
    let n=(image.width as usize).checked_mul(image.height as usize)
        .ok_or_else(||bad("OpenType PNG dimensions overflow"))?;
    if n==0||n>16_777_216||image.width>4096||image.height>4096||image.pixels.len()!=n{
        return Err(bad("OpenType PNG raster exceeds limits"));
    }
    Ok(Png{width:image.width,height:image.height,pixels:image.pixels})
}

/// Decode an embedded sbix image using the already-existing Mata raster codecs.
/// The encoded image is bounded before dispatch and decoded dimensions are
/// checked before any glyph scaling. Pixels have Mata's RRGGBBAA packing.
fn decode_embedded_image(image: &EmbeddedSbit) -> io::Result<Png> {
    if !matches!(&image.image_tag, b"png " | b"jpg " | b"tiff") {
        return Err(Error::new(ErrorKind::Unsupported, "unsupported sbix graphic type"));
    }
    if image.data.is_empty() || image.data.len() > 8 * 1024 * 1024 {
        return Err(bad("sbix encoded raster exceeds limit"));
    }
    let decoded = mata_image::Image::from_bytes(&image.data)
        .map_err(|err| Error::new(ErrorKind::InvalidData,
            format!("unable to decode sbix PNG/JPEG/TIFF: {err}")))?;
    let area = (decoded.width as usize).checked_mul(decoded.height as usize)
        .ok_or_else(||bad("sbix raster dimensions overflow"))?;
    if decoded.width == 0 || decoded.height == 0 || decoded.width > 4096 ||
       decoded.height > 4096 || area > 16_777_216 || decoded.pixels.len() != area {
        return Err(bad("sbix decoded image size invalid"));
    }
    Ok(Png { width:decoded.width, height:decoded.height, pixels: decoded.pixels })
}

fn cbdt_glyph(cbdt:&[u8],cblc:&[u8],gid:u16,ppem:f32)
        ->io::Result<Option<(Png,i32,i32,u32,u16)>>{
 let Some((format,data,metrics,strike,depth))=
     crate::ebdt::indexed_glyph(cbdt,cblc,gid,ppem)? else{return Ok(None)};
 match format{
  17|18|19=>{
   let metric_len=match format{17=>5usize,18=>8,_=>0};
   let info=if format==19{metrics.as_ref().map(|v|&v[..]).ok_or_else(||bad("CBDT format 19 requires CBLC metrics"))?}
      else{data.get(..metric_len).ok_or_else(||bad("CBDT metrics truncated"))?};
   let png_len=u32be(data,metric_len).ok_or_else(||bad("CBDT PNG size missing"))? as usize;
   let png=data.get(metric_len+4..metric_len+4+png_len).ok_or_else(||bad("CBDT PNG truncated"))?;
   let image=decode_png(png)?;
   if info[0] as u32!=image.height||info[1] as u32!=image.width{
    return Err(bad("CBDT PNG dimensions differ from glyph metrics"));
   }
   Ok(Some((image,info[2] as i8 as i32,info[3] as i8 as i32,info[4] as u32,strike)))
  },
  // Legacy full-color uncompressed BGRA bitmaps are permitted by the CBDT
  // extension to EBDT. Pixels are stored with premultiplied channels.
  1|2|5|6|7 if depth==32=>{
   let length=match format{1|2=>5,5=>0,_=>8};
   let info=if format==5{metrics.as_ref().map(|v|&v[..]).ok_or_else(||bad("CBDT fixed metrics missing"))?}
       else{data.get(..length).ok_or_else(||bad("CBDT metrics truncated"))?};
   let width=info[1] as usize;let height=info[0] as usize;
   if width.saturating_mul(height)>4_194_304{return Err(bad("CBDT BGRA size limit exceeded"));}
   let bytes=data.get(length..length+width.saturating_mul(height).saturating_mul(4))
       .ok_or_else(||bad("CBDT BGRA pixels truncated"))?;
   let mut pixels=Vec::with_capacity(width*height);
   for c in bytes.chunks_exact(4){let a=c[3] as u32;
    let channel=|v:u8|if a==0{0}else{((v as u32*255+a/2)/a).min(255)};
    pixels.push((channel(c[2])<<24)|(channel(c[1])<<16)|(channel(c[0])<<8)|a);
   }
   Ok(Some((Png{width:width as u32,height:height as u32,pixels},
       info[2] as i8 as i32,info[3] as i8 as i32,info[4] as u32,strike)))
  },
  _=>Ok(None),
 }
}
fn scaled(png:Png,x:i32,y:i32,advance:f32,strike_ppem:u16,wanted:f32)->io::Result<ColorBitmap>{
 let ratio=if strike_ppem==0{1.0}else{(wanted/(strike_ppem as f32)).clamp(0.25,8.0)};
 let width=(png.width as f32*ratio).round().max(1.0) as usize;
 let height=(png.height as f32*ratio).round().max(1.0) as usize;
 if width>4096||height>4096||width.checked_mul(height).unwrap_or(usize::MAX)>16_777_216{return Err(bad("scaled bitmap glyph too large"));}
 let mut pixels=vec![0; width*height];
 for v in 0..height{for u in 0..width{
  let sx=((u as f32/ratio) as usize).min(png.width as usize-1);
  let sy=((v as f32/ratio) as usize).min(png.height as usize-1);
  pixels[v*width+u]=png.pixels[sy*png.width as usize+sx];
 }}
 Ok(ColorBitmap{width:width as u32,height:height as u32,pixels,
   bearing_x:(x as f32*ratio).round() as i32,
   bearing_y:(y as f32*ratio).round() as i32,
   advance_px:advance*ratio})
}
/// Rasterize an `sbix` PNG/JPEG/TIFF or CBDT/CBLC bitmap glyph.
/// Supports native Mata codecs for `sbix` and retains the bounds-checked
/// internal PNG/BGRA handling for CBDT.
pub fn rasterize(face:&Face,glyph:u16,ppem:f32)->io::Result<Option<ColorBitmap>>{
 if !ppem.is_finite()||ppem<=0.||ppem>4096.{return Err(bad("invalid color glyph ppem"));}
 if face.table(*b"sbix").is_some(){
  if let Some(source)=sbix_image(face,glyph,ppem)?{
   match decode_embedded_image(&source){
     Ok(image)=>{
       let advance=face.advance_units_instance(glyph)
          /face.units_per_em as f32 * source.strike_ppem as f32;
       return scaled(image,source.x,source.y,advance,source.strike_ppem,ppem).map(Some);
     },
     Err(e) if e.kind()==ErrorKind::Unsupported=>{},
     Err(e)=>return Err(e),
   }
  }
 }
 if let (Some(cbdt),Some(cblc))=(face.table(*b"CBDT"),face.table(*b"CBLC")){
  if let Some((bitmap,x,y,advance,strike))=cbdt_glyph(cbdt,cblc,glyph,ppem)?{
   return scaled(bitmap,x,y,advance as f32,strike,ppem).map(Some);
  }
 }
 Ok(None)
}

/// Encoded bitmap payload in an OpenType `sbix` strike. The renderer can
/// decode PNG, JPEG and TIFF through `mata-image` and this API also returns
/// the original bytes for callers requiring metadata or lossless extraction.
#[derive(Debug,Clone)]
pub struct EmbeddedSbit {
    /// `b"png "`, `b"jpg "`, `b"tiff"`, or other documented sbix tag.
    pub image_tag:[u8;4],
    pub data:Vec<u8>,
    pub x:i32,pub y:i32,pub strike_ppem:u16,
}
/// Return a raw sbix image, resolving `dupe` aliases. This is intentionally
/// distinct from decoding: a supported container does not imply every image
/// codec can be rendered by the standard-library-only build.
pub fn sbix_image(face:&Face,glyph:u16,ppem:f32)->io::Result<Option<EmbeddedSbit>>{
    if !ppem.is_finite()||ppem<=0.||ppem>4096.{return Err(bad("invalid sbix ppem"));}
    let Some(sbix)=face.table(*b"sbix")else{return Ok(None)};
    let count=face.table(*b"maxp").and_then(|m|u16be(m,4)).unwrap_or(0) as usize;
    if glyph as usize>=count{return Ok(None)};
    let strikes=u32be(sbix,4).ok_or_else(||bad("sbix header truncated"))? as usize;
    if strikes>256{return Err(bad("sbix strike limit exceeded"));}
    let mut best:Option<(f32,usize,u16)>=None;
    for i in 0..strikes{
        let at=u32be(sbix,8+i*4).ok_or_else(||bad("sbix strike offset missing"))? as usize;
        let size=u16be(sbix,at).ok_or_else(||bad("sbix strike truncated"))?;
        if size==0{continue;}
        let delta=(size as f32-ppem).abs();
        if best.as_ref().map(|(d,_,_)|delta<*d).unwrap_or(true){best=Some((delta,at,size));}
    }
    let Some((_,strike,strike_ppem))=best else{return Ok(None)};
    let mut id=glyph as usize;
    for _ in 0..8{
        let begin=u32be(sbix,strike+4+id*4).ok_or_else(||bad("sbix index truncated"))? as usize;
        let end=u32be(sbix,strike+4+(id+1)*4).ok_or_else(||bad("sbix index truncated"))? as usize;
        if end<=begin{return Ok(None)};
        let from=strike.checked_add(begin).ok_or_else(||bad("sbix offset overflow"))?;
        let to=strike.checked_add(end).ok_or_else(||bad("sbix offset overflow"))?;
        let chunk=sbix.get(from..to).ok_or_else(||bad("sbix data outside strike"))?;
        if chunk.len()<8||chunk.len()>8*1024*1024{return Err(bad("invalid sbix glyph payload size"));}
        let x=u16be(chunk,0).unwrap_or(0) as i16 as i32;
        let y=u16be(chunk,2).unwrap_or(0) as i16 as i32;
        let mut tag=[0u8;4];tag.copy_from_slice(&chunk[4..8]);
        if &tag==b"dupe"{
            id=u16be(chunk,8).ok_or_else(||bad("sbix duplicate target missing"))? as usize;
            if id>=count{return Err(bad("sbix duplicate target out of bounds"));}
            continue;
        }
        return Ok(Some(EmbeddedSbit{image_tag:tag,data:chunk[8..].to_vec(),x,y,strike_ppem}));
    }
    Err(bad("sbix duplicate recursion limit exceeded"))
}
