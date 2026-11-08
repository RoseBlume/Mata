//! OpenType EBDT/EBLC bitmap strike reader (1-,2-,4-,8-bit grayscale).
//! Supports OpenType image formats 1,2,5,6,7,8,9, and index-subtable
//! formats 1..5. Rendering uses bounded native bitmap masks.
use std::io::{self,Error,ErrorKind};
use crate::{colr::ColorBitmap,opentype::Face};
fn bad(s:&'static str)->io::Error{Error::new(ErrorKind::InvalidData,s)}
fn be16(b:&[u8],p:usize)->Option<u16>{let s=b.get(p..p.checked_add(2)?)?;Some(u16::from_be_bytes([s[0],s[1]]))}
fn be32(b:&[u8],p:usize)->Option<u32>{let s=b.get(p..p.checked_add(4)?)?;Some(u32::from_be_bytes([s[0],s[1],s[2],s[3]]))}
#[derive(Debug,Clone)]struct Mask{w:usize,h:usize,x:i32,y:i32,advance:u32,pixels:Vec<u8>}
impl Mask{
 fn new(data:&[u8],metrics:&[u8],bit_depth:usize,row_aligned:bool)->io::Result<Self>{
  if !matches!(bit_depth,1|2|4|8){return Err(bad("unsupported EBDT bit depth"));}
  if metrics.len()<5{return Err(bad("EBDT bitmap metrics truncated"));}
  let(w,h)=(metrics[1] as usize,metrics[0] as usize);
  if w>4096||h>4096||w.saturating_mul(h)>4_194_304{return Err(bad("EBDT raster exceeds size limit"));}
  let row_bits=w.checked_mul(bit_depth).ok_or_else(||bad("EBDT raster overflow"))?;
  let stride=if row_aligned{(row_bits+7)/8*8}else{row_bits};
  let total=if h==0{0}else{stride.checked_mul(h-1).and_then(|v|v.checked_add(row_bits)).ok_or_else(||bad("EBDT raster overflow"))?};
  if data.len()< (total+7)/8{return Err(bad("EBDT bitmap data truncated"));}
  let mut pixels=Vec::with_capacity(w*h);
  for y in 0..h{for x in 0..w{
   let bit=y*stride+x*bit_depth;let mut value=0u32;
   for z in 0..bit_depth{let offset=bit+z;let byte=data[offset/8];
    value=(value<<1)|((byte>>(7-offset%8))&1) as u32;}
   pixels.push((value*255/((1<<bit_depth)-1) as u32) as u8);
  }}
  Ok(Self{w,h,x:metrics[2] as i8 as i32,y:metrics[3] as i8 as i32,
      advance:metrics[4] as u32,pixels})
 }
 fn to_bitmap(&self,ppem:f32,strike_ppem:u16)->io::Result<ColorBitmap>{
  let ratio=(ppem/(strike_ppem.max(1) as f32)).clamp(0.25,8.0);
  let w=(self.w as f32*ratio).round() as usize;let h=(self.h as f32*ratio).round() as usize;
  if w>4096||h>4096||w.saturating_mul(h)>4_194_304{return Err(bad("scaled EBDT image too large"));}
  let mut pixels=vec![0u32;w*h];
  for y in 0..h{for x in 0..w{
   let iy=(y as f32/ratio).floor() as usize;let ix=(x as f32/ratio).floor() as usize;
   let a=self.pixels[iy.min(self.h-1)*self.w+ix.min(self.w-1)] as u32;
   pixels[y*w+x]=a;
  }}
  Ok(ColorBitmap{width:w as u32,height:h as u32,pixels,
   bearing_x:(self.x as f32*ratio).round() as i32,
   bearing_y:(self.y as f32*ratio).round() as i32,
   advance_px:self.advance as f32*ratio})
 }
}
#[derive(Clone,Copy)]struct Strike{offset:usize,count:usize,ppem:u16,depth:usize}
fn strike(eblc:&[u8],glyph:u16,ppem:f32)->io::Result<Option<Strike>>{
 let count=be32(eblc,4).ok_or_else(||bad("EBLC header truncated"))? as usize;
 if count>256{return Err(bad("EBLC strike count too large"));}
 let mut best:Option<(f32,Strike)>=None;
 for index in 0..count{
  let at=8+index*48;
  if glyph<be16(eblc,at+40).ok_or_else(||bad("EBLC strike truncated"))?
   ||glyph>be16(eblc,at+42).ok_or_else(||bad("EBLC strike truncated"))?{continue;}
  let pixel=*eblc.get(at+45).ok_or_else(||bad("EBLC strike ppem missing"))? as u16;
  let bits=*eblc.get(at+46).ok_or_else(||bad("EBLC bit depth missing"))? as usize;
  let candidate=Strike{offset:be32(eblc,at).ok_or_else(||bad("EBLC offset missing"))? as usize,
   count:be32(eblc,at+8).ok_or_else(||bad("EBLC subtable count missing"))? as usize,ppem:pixel,depth:bits};
  if candidate.count>4096{return Err(bad("EBLC too many subtables"));}
  let delta=(ppem-pixel as f32).abs();
  if best.as_ref().map(|(d,_)|delta<*d).unwrap_or(true){best=Some((delta,candidate));}
 }
 Ok(best.map(|(_,s)|s))
}
fn glyph_entry<'a>(ebdt:&'a [u8],eblc:&[u8],glyph:u16,s:Strike)->io::Result<Option<(u16,&'a [u8],Option<[u8;8]>)>>{
 for index in 0..s.count{
  let at=s.offset.checked_add(index*8).ok_or_else(||bad("EBLC record overflow"))?;
  let first=be16(eblc,at).ok_or_else(||bad("EBLC index truncated"))?;
  let last=be16(eblc,at+2).ok_or_else(||bad("EBLC index truncated"))?;
  if first>last{return Err(bad("invalid EBLC glyph range"));}
  if glyph<first||glyph>last{continue;}
  let off=be32(eblc,at+4).ok_or_else(||bad("EBLC subtable offset truncated"))? as usize;
  let sub=s.offset.checked_add(off).ok_or_else(||bad("EBLC subtable offset overflow"))?;
  let format=be16(eblc,sub).ok_or_else(||bad("EBLC index format missing"))?;
  let image_format=be16(eblc,sub+2).ok_or_else(||bad("EBLC image format missing"))?;
  let base=be32(eblc,sub+4).ok_or_else(||bad("EBLC image offset missing"))? as usize;
  let id=(glyph-first) as usize;
  let (begin,end,metrics)=match format{
   1=>{let at=sub.checked_add(8+id*4).ok_or_else(||bad("EBLC offset overflow"))?;
    (be32(eblc,at).ok_or_else(||bad("EBLC index truncated"))? as usize,
     be32(eblc,at+4).ok_or_else(||bad("EBLC index truncated"))? as usize,None)},
   2|5=>{
    let size=be32(eblc,sub+8).ok_or_else(||bad("EBLC fixed image size missing"))? as usize;
    let metrics: [u8;8]=eblc.get(sub+12..sub+20).ok_or_else(||bad("EBLC fixed metrics truncated"))?.try_into().map_err(|_|bad("EBLC metric width"))?;
    if format==5{
      let count=be32(eblc,sub+20).ok_or_else(||bad("EBLC sparse count truncated"))? as usize;
      if count>65_536{return Err(bad("EBLC sparse count too large"));}
      let list=eblc.get(sub+24..sub+24+count*2).ok_or_else(||bad("EBLC sparse IDs truncated"))?;
      let Some(slot)=list.chunks_exact(2).position(|v|u16::from_be_bytes([v[0],v[1]])==glyph)else{return Ok(None)};
      (slot*size,(slot+1)*size,Some(metrics))
    }else{(id*size,(id+1)*size,Some(metrics))}
   },
   3=>{let at=sub.checked_add(8+id*2).ok_or_else(||bad("EBLC offset overflow"))?;
    (be16(eblc,at).ok_or_else(||bad("EBLC sparse index truncated"))? as usize,
     be16(eblc,at+2).ok_or_else(||bad("EBLC sparse index truncated"))? as usize,None)},
   4=>{let count=be32(eblc,sub+8).ok_or_else(||bad("EBLC sparse index count truncated"))? as usize;
    if count>65_536{return Err(bad("EBLC sparse index too long"));}
    let mut value=None;
    for i in 0..count{
     let at=sub+12+i*4;
     if be16(eblc,at)==Some(glyph){value=Some((be16(eblc,at+2).ok_or_else(||bad("EBLC sparse offset missing"))? as usize,
          be16(eblc,at+6).ok_or_else(||bad("EBLC sparse end missing"))? as usize));break;}
    }
    let Some((a,b))=value else{return Ok(None)};(a,b,None)
   },
   _=>return Ok(None),
  };
  if end<begin{return Err(bad("EBLC reversed offsets"));}
  if begin==end{return Ok(None);}
  let absolute=base.checked_add(begin).ok_or_else(||bad("EBDT offset overflow"))?;
  let absolute_end=base.checked_add(end).ok_or_else(||bad("EBDT offset overflow"))?;
  let data=ebdt.get(absolute..absolute_end).ok_or_else(||bad("EBDT glyph outside table"))?;
  return Ok(Some((image_format,data,metrics)));
 }
 Ok(None)
}
fn get_mask(ebdt:&[u8],eblc:&[u8],glyph:u16,s:Strike,depth:usize)->io::Result<Option<Mask>>{
 if depth>12{return Err(bad("EBDT composite depth exceeded"));}
 let Some((format,data,large_metrics))=glyph_entry(ebdt,eblc,glyph,s)? else{return Ok(None)};
 let metric_length=match format{1|2|8=>5usize,5=>0,6|7|9=>8,_=>return Ok(None)};
 let metrics=data.get(0..metric_length).ok_or_else(||bad("EBDT metrics missing"))?;
 let metrics=if format==5{large_metrics.as_ref().map(|m|&m[..]).ok_or_else(||bad("EBDT format 5 requires EBLC metrics"))?}
  else{metrics};
 if format<=7{
  return Mask::new(data.get(metric_length..).ok_or_else(||bad("EBDT payload missing"))?,metrics,s.depth,matches!(format,1|6)).map(Some);
 }
 let (w,h)=(metrics[1] as usize,metrics[0] as usize);
 if w.saturating_mul(h)>4_194_304{return Err(bad("EBDT composite size exceeded"));}
 let mut out=Mask{w,h,x:metrics[2] as i8 as i32,y:metrics[3] as i8 as i32,
  advance:metrics[4] as u32,pixels:vec![0;w*h]};
 let begin=if format==8{6usize}else{8usize};
 let count=be16(data,begin).ok_or_else(||bad("EBDT composite count truncated"))? as usize;
 if count>256{return Err(bad("EBDT component count exceeded"));}
 for i in 0..count{
  let at=begin+2+i*4;
  let id=be16(data,at).ok_or_else(||bad("EBDT component glyph ID missing"))?;
  let dx=*data.get(at+2).ok_or_else(||bad("EBDT component dx missing"))? as i8 as i32;
  let dy=*data.get(at+3).ok_or_else(||bad("EBDT component dy missing"))? as i8 as i32;
  if let Some(part)=get_mask(ebdt,eblc,id,s,depth+1)?{
   for y in 0..part.h{for x in 0..part.w{
    let tx=x as i32+dx;let ty=y as i32+dy;
    if tx>=0&&ty>=0&&(tx as usize)<w&&(ty as usize)<h{
      let at=ty as usize*w+tx as usize;
      out.pixels[at]=out.pixels[at].max(part.pixels[y*part.w+x]);
    }
   }}
  }
 }
 Ok(Some(out))
}
/// Shared EBLC/CBLC indexed glyph lookup, including sparse and fixed-metric
/// subtable formats. Returns `(image_format, bytes, big_metrics, ppem, depth)`.
pub(crate) fn indexed_glyph<'a>(data:&'a [u8],index:&[u8],glyph:u16,ppem:f32)
    ->io::Result<Option<(u16,&'a [u8],Option<[u8;8]>,u16,usize)>>{
    let Some(s)=strike(index,glyph,ppem)? else{return Ok(None)};
    Ok(glyph_entry(data,index,glyph,s)?.map(|(format,bytes,metrics)|
        (format,bytes,metrics,s.ppem,s.depth)))
}
/// Render an EBDT monochrome or grayscale glyph as black RGBA with its
/// stored alpha. A color CPAL/COLR or PNG strike should be preferred.
pub fn rasterize(face:&Face,glyph:u16,ppem:f32)->io::Result<Option<ColorBitmap>>{
 let (Some(ebdt),Some(eblc))=(face.table(*b"EBDT"),face.table(*b"EBLC"))else{return Ok(None)};
 let Some(s)=strike(eblc,glyph,ppem)?else{return Ok(None)};
 let Some(mask)=get_mask(ebdt,eblc,glyph,s,0)?else{return Ok(None)};
 if mask.w==0||mask.h==0{return Ok(None)};
 mask.to_bitmap(ppem,s.ppem).map(Some)
}

#[cfg(test)]
mod tests {
    use super::Mask;
    #[test]
    fn one_bit_rows_are_byte_aligned_in_format_one(){
        // Two rows: bits `10` and `01`, each padded to a byte boundary.
        let m=Mask::new(&[0b1000_0000,0b0100_0000],&[2,2,0,2,2],1,true).unwrap();
        assert_eq!(m.pixels,&[255,0,0,255]);
    }
    #[test]
    fn two_bit_greyscale_is_bit_packed(){
        let m=Mask::new(&[0b0001_1011],&[1,4,0,1,4],2,false).unwrap();
        assert_eq!(m.pixels,&[0,85,170,255]);
    }
}
