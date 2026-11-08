//! Embedded color bitmap glyphs from OpenType CBDT/CBLC emoji fonts.
//!
//! Many emoji fonts (including Noto Color Emoji) contain PNG images rather than
//! TrueType `glyf` outlines. This small reader intentionally handles their
//! Unicode cmap format 12 and CBDT PNG formats 17 and 18. Invalid/truncated
//! external fonts are ignored and normal text rendering remains available.
//! Pixel data is decoded by the existing `mata-image` PNG decoder.

use std::{fs, path::PathBuf, sync::OnceLock};
use mata_fonts::decoders::Character;

#[derive(Clone, Copy, Debug)]
struct Table { offset: usize, length: usize }

impl Table {
    fn slice<'a>(&self, bytes: &'a [u8], start: usize, len: usize) -> Option<&'a [u8]> {
        let end = start.checked_add(len)?;
        if end > self.length { return None; }
        bytes.get(self.offset.checked_add(start)? .. self.offset.checked_add(end)?)
    }
    fn u16(&self, bytes: &[u8], at: usize) -> Option<u16> {
        let v = self.slice(bytes, at, 2)?;
        Some(u16::from_be_bytes([v[0], v[1]]))
    }
    fn u32(&self, bytes: &[u8], at: usize) -> Option<u32> {
        let v = self.slice(bytes, at, 4)?;
        Some(u32::from_be_bytes([v[0], v[1], v[2], v[3]]))
    }
}

fn u16be(bytes: &[u8], start: usize) -> Option<u16> {
    let v = bytes.get(start .. start.checked_add(2)?)?;
    Some(u16::from_be_bytes([v[0], v[1]]))
}
fn u32be(bytes: &[u8], start: usize) -> Option<u32> {
    let v = bytes.get(start .. start.checked_add(4)?)?;
    Some(u32::from_be_bytes([v[0], v[1], v[2], v[3]]))
}

pub(super) struct ColorEmojiFont {
    bytes: Vec<u8>,
    cmap: Table,
    groups: usize,
    cbdt: Table,
    cblc: Table,
}

impl ColorEmojiFont {
    fn parse(bytes: Vec<u8>) -> Option<Self> {
        if bytes.len() < 12 { return None; }
        let find = |tag: &[u8;4]| -> Option<Table> {
            let count = u16be(&bytes, 4)? as usize;
            if count > 1024 || 12usize.checked_add(count.checked_mul(16)?)? > bytes.len() { return None; }
            for i in 0..count {
                let pos = 12 + i * 16;
                if bytes.get(pos..pos+4)? != &tag[..] { continue; }
                let offset = u32be(&bytes, pos+8)? as usize;
                let length = u32be(&bytes, pos+12)? as usize;
                if offset.checked_add(length)? > bytes.len() { return None; }
                return Some(Table { offset, length });
            }
            None
        };
        let cmap_table = find(b"cmap")?;
        let cbdt = find(b"CBDT")?;
        let cblc = find(b"CBLC")?;
        let count = cmap_table.u16(&bytes, 2)? as usize;
        if count > 128 { return None; }
        let mut cmap = None;
        let mut groups = 0usize;
        for i in 0..count {
            let record = 4 + i * 8;
            let subtbl = cmap_table.u32(&bytes, record + 4)? as usize;
            if cmap_table.u16(&bytes, subtbl) != Some(12) { continue; }
            let size = cmap_table.u32(&bytes, subtbl + 4)? as usize;
            let n = cmap_table.u32(&bytes, subtbl + 12)? as usize;
            let end = subtbl.checked_add(size)?;
            if n > 200_000 || size < 16 || end > cmap_table.length || n.checked_mul(12)?.checked_add(16)? > size { continue; }
            cmap = Some(Table { offset: cmap_table.offset + subtbl, length: size });
            groups = n;
            break;
        }
        if cblc.u32(&bytes, 4)? > 64 { return None; }
        Some(Self { bytes, cmap: cmap?, groups, cbdt, cblc })
    }

    fn find_glyph(&self, ch: char) -> Option<u16> {
        let scalar = ch as u32;
        let mut lo = 0usize;
        let mut hi = self.groups;
        while lo < hi {
            let mid = (lo + hi)/2;
            let at = 16 + mid * 12;
            let from = self.cmap.u32(&self.bytes, at)?;
            let to = self.cmap.u32(&self.bytes, at + 4)?;
            if scalar < from { hi = mid; }
            else if scalar > to { lo = mid + 1; }
            else {
                let start = self.cmap.u32(&self.bytes, at + 8)?;
                let id = start.checked_add(scalar - from)?;
                return u16::try_from(id).ok().filter(|id| *id != 0);
            }
        }
        None
    }

    fn glyph_png(&self, id: u16, pixels: u32) -> Option<(Vec<u32>,u32,u32,u32,i32,i32)> {
        let strikes = self.cblc.u32(&self.bytes, 4)? as usize;
        let mut best: Option<(u32, usize)> = None;
        for s in 0..strikes {
            let strike = 8usize.checked_add(s.checked_mul(48)?)?;
            let start = self.cblc.u16(&self.bytes, strike + 40)?;
            let end = self.cblc.u16(&self.bytes, strike + 42)?;
            if id < start || id > end { continue; }
            let ppem = *self.cblc.slice(&self.bytes,strike+44,1)?.first()? as u32;
            let diff = ppem.abs_diff(pixels);
            if best.is_none_or(|(old,_)|diff < old) { best=Some((diff, strike)); }
        }
        let (_, strike) = best?;
        let ppem = *self.cblc.slice(&self.bytes,strike+44,1)?.first()? as u32;
        if ppem == 0 {return None;}
        let index_offset = self.cblc.u32(&self.bytes, strike)? as usize;
        let n = self.cblc.u32(&self.bytes, strike+8)? as usize;
        if n > 5000 {return None;}
        for i in 0..n {
            let rec = index_offset.checked_add(i.checked_mul(8)?)?;
            let start = self.cblc.u16(&self.bytes, rec)?;
            let end = self.cblc.u16(&self.bytes, rec+2)?;
            if id < start || id > end {continue;}
            let sub_offset = self.cblc.u32(&self.bytes, rec+4)? as usize;
            let sub = index_offset.checked_add(sub_offset)?;
            let format = self.cblc.u16(&self.bytes, sub)?;
            let image_format = self.cblc.u16(&self.bytes, sub+2)?;
            let base = self.cblc.u32(&self.bytes, sub+4)? as usize;
            // Fixed 32-bit offsets (format 1) and compact 16-bit offsets (format 3).
            let index = (id - start) as usize;
            let (begin, end) = match format {
                1 => {
                    let at = sub.checked_add(8 + index.checked_mul(4)?)?;
                    (self.cblc.u32(&self.bytes, at)? as usize,
                     self.cblc.u32(&self.bytes, at+4)? as usize)
                },
                3 => {
                    let at = sub.checked_add(8 + index.checked_mul(2)?)?;
                    (self.cblc.u16(&self.bytes, at)? as usize,
                     self.cblc.u16(&self.bytes, at+2)? as usize)
                },
                _ => return None,
            };
            if end <= begin {return None;}
            let data_offset = base.checked_add(begin)?;
            let bytes = self.cbdt.slice(&self.bytes, data_offset, end-begin)?;
            // Formats 17/18 contain metrics followed by PNG length;
            // format 19 contains PNG length only (metrics are in the index).
            let (metrics_len, height, width, bearing_x, bearing_y, advance) = match image_format {
                17 if bytes.len() >= 9 => (5, bytes[0] as u32,bytes[1] as u32,
                    bytes[2] as i8 as i32,bytes[3] as i8 as i32,bytes[4] as u32),
                18 if bytes.len() >= 12 => (8, bytes[0] as u32,bytes[1] as u32,
                    bytes[2] as i8 as i32,bytes[3] as i8 as i32,bytes[4] as u32),
                _ => return None,
            };
            let png_len = u32be(bytes, metrics_len)? as usize;
            if png_len > 2_000_000 {return None;}
            let png = bytes.get(metrics_len+4..metrics_len.checked_add(4)?.checked_add(png_len)?)?;
            let decoded = mata_image::Image::from_bytes(png).ok()?;
            if decoded.width != width || decoded.height != height { return None; }
            if decoded.pixels.len() != (width as usize).checked_mul(height as usize)? {return None;}
            // Some CBDT fonts use image dimensions larger than their strike
            // ppem. Fit those bitmaps to one text em rather than overflowing
            // the line box and clipping the next line.
            let fact = pixels as f32 / ppem.max(width).max(height) as f32;
            let scaled_w = (width as f32*fact).round().max(1.0) as u32;
            let scaled_h = (height as f32*fact).round().max(1.0) as u32;
            // Guard against pathological input fonts before allocating.
            if scaled_w > 512 || scaled_h > 512 {return None;}
            let out = scale_rgba(&decoded.pixels,width,height,scaled_w,scaled_h);
            return Some((out,scaled_w,scaled_h,
                (advance as f32 * fact).round().max(1.0) as u32,
                (bearing_x as f32 * fact).round() as i32,
                (bearing_y as f32 * fact).round() as i32));
        }
        None
    }

    pub(super) fn rasterize(&self, ch: char, pt: u16) -> Option<Character> {
        if pt == 0 {return None;}
        let id = self.find_glyph(ch)?;
        let pixels = (pt as f32 * (96.0/72.0)).round().max(1.0) as u32;
        let (rgba,width,height,advance,bearing_x,bearing_y)=self.glyph_png(id,pixels)?;
        Some(Character {
            pixels:rgba,width,height,advance_width:advance,bearing_x,bearing_y,
            baseline:pixels, line_height:pixels.max(height),
        })
    }
}

// Linear two-dimensional resampling in premultiplied alpha prevents dark
// fringes on transparent pixels of the PNG when reduced for UI-sized text.
fn scale_rgba(src: &[u32], sw: u32, sh: u32, dw: u32, dh: u32) -> Vec<u32> {
    let mut output=Vec::with_capacity((dw*dh) as usize);
    for y in 0..dh {
        let fy=((y as f32 + 0.5)*sh as f32 / dh as f32-0.5).max(0.0);
        let y0=(fy as u32).min(sh-1);let y1=(y0+1).min(sh-1);let dy=fy-y0 as f32;
        for x in 0..dw {
            let fx=((x as f32 + 0.5)*sw as f32 / dw as f32-0.5).max(0.0);
            let x0=(fx as u32).min(sw-1);let x1=(x0+1).min(sw-1);let dx=fx-x0 as f32;
            let mut sum_a=0.0f32;let mut sums=[0.0f32;3];
            for (sx,sy,w) in [
                (x0,y0,(1.0-dx)*(1.0-dy)),(x1,y0,dx*(1.0-dy)),
                (x0,y1,(1.0-dx)*dy),(x1,y1,dx*dy),
            ] {
                let p=src[(sy*sw+sx) as usize];let a=(p&255) as f32 * w;
                sum_a+=a;
                sums[0]+=((p>>24)&255) as f32*a;
                sums[1]+=((p>>16)&255) as f32*a;
                sums[2]+=((p>>8)&255) as f32*a;
            }
            let rgb=if sum_a<=0.0 {[0,0,0]}else{[
                (sums[0]/sum_a).round().clamp(0.0,255.0) as u32,
                (sums[1]/sum_a).round().clamp(0.0,255.0) as u32,
                (sums[2]/sum_a).round().clamp(0.0,255.0) as u32,
            ]};
            output.push((rgb[0]<<24)|(rgb[1]<<16)|(rgb[2]<<8)|(sum_a.round().clamp(0.0,255.0) as u32));
        }
    }
    output
}

fn font_candidates() -> Vec<PathBuf> {
    let mut list=vec![
        PathBuf::from("/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf"),
        PathBuf::from("/usr/share/fonts/noto/NotoColorEmoji.ttf"),
        PathBuf::from("/usr/local/share/fonts/NotoColorEmoji.ttf"),
        PathBuf::from("/system/fonts/NotoColorEmoji.ttf"),
    ];
    if let Some(home)=std::env::var_os("HOME").map(PathBuf::from) {
        list.push(home.join(".local/share/fonts/NotoColorEmoji.ttf"));
        list.push(home.join(".fonts/NotoColorEmoji.ttf"));
    }
    list
}

pub(super) fn system_emoji() -> Option<&'static ColorEmojiFont> {
    static FONT: OnceLock<Option<ColorEmojiFont>> = OnceLock::new();
    FONT.get_or_init(|| {
        for path in font_candidates() {
            if let Ok(bytes)=fs::read(path) {
                if let Some(font)=ColorEmojiFont::parse(bytes) {return Some(font);}
            }
        }
        None
    }).as_ref()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gracefully_handles_invalid_sfnt() {
        assert!(ColorEmojiFont::parse(b"not a font".to_vec()).is_none());
    }
    #[test]
    fn alpha_and_color_preserved_through_scaling() {
        let image=[0xff0000ff,0x00ff00ff,0x00000000,0x0000ffff];
        let result=scale_rgba(&image,2,2,1,1);
        assert!(result[0]&255>0);
        assert!((result[0]>>24)&255>0);
    }
    #[test]
    fn test_installed_noto_if_available() {
        if let Some(font)=system_emoji() {
            let glyph=font.rasterize('🌷',20).expect("Noto Color Emoji supports tulip");
            assert!(glyph.width>0 && glyph.height>0);
            assert!(glyph.pixels.iter().any(|v|(v&255)>0));
        }
    }
}
