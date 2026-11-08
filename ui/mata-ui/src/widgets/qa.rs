//! Lightweight deterministic framebuffer regression helpers. Screenshot files
//! use plain PPM so tests need no image codec, OS renderer, or system fonts.
use std::{io::{self,Write},path::Path};
use crate::Framebuffer;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct PixelDiff {pub changed:usize,pub largest_channel_delta:u8}
/// Compare two equal-sized native pixel buffers. Mismatched dimensions count
/// as an error so a layout resize cannot accidentally pass pixel regression.
pub fn compare(a:&Framebuffer,b:&Framebuffer)->Option<PixelDiff> {
    if a.width!=b.width || a.height!=b.height{return None;}
    let mut result=PixelDiff{changed:0,largest_channel_delta:0};
    for (&x,&y) in a.pixels.iter().zip(&b.pixels) {
        if x!=y {result.changed+=1;}
        for shift in [0,8,16,24] {
            let delta=(((x>>shift)&255) as i16-((y>>shift)&255) as i16).unsigned_abs() as u8;
            result.largest_channel_delta=result.largest_channel_delta.max(delta);
        }
    }
    Some(result)
}
/// Save an RGB PPM capture of the rendered `mata-ui` framebuffer.
pub fn save_ppm(fb:&Framebuffer,path:impl AsRef<Path>)->io::Result<()> {
    let mut file=std::io::BufWriter::new(std::fs::File::create(path)?);
    write!(file,"P6\n{} {}\n255\n",fb.width,fb.height)?;
    for &argb in &fb.pixels {
        file.write_all(&[((argb>>16)&255) as u8,((argb>>8)&255) as u8,(argb&255) as u8])?;
    }
    file.flush()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pixel_diffs_are_deterministic(){
        let a=Framebuffer::from_color(3,2,0xff000000);
        let mut b=a.clone();b.pixels[2]=0xffff0000;
        let diff=compare(&a,&b).unwrap();
        assert_eq!(diff.changed,1);assert_eq!(diff.largest_channel_delta,255);
    }
    #[test]
    fn images_with_different_bounds_are_not_compared(){
        assert!(compare(&Framebuffer::from_color(1,1,0),&Framebuffer::from_color(2,1,0)).is_none());
    }
}
