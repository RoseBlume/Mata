//! Nearest-neighbour scene upscale. Only the texture gather is vectorized;
//! native-resolution HUD rendering is performed after this pass.
//!
//! AVX2 gathers eight 32-bit pixels per iteration on supported x86 chips.
//! A checked scalar version is used everywhere else. Mapping allocation is
//! reused between frames instead of rebuilding the x coordinates every frame.
use mata_win::Framebuffer;
#[cfg(target_arch="x86")]
use std::arch::x86::*;
#[cfg(target_arch="x86_64")]
use std::arch::x86_64::*;

#[derive(Default)]
pub struct Upscaler{map:Vec<i32>,dims:(u32,u32)}
impl Upscaler{
    pub fn new()->Self{Self::default()}
    pub fn render(&mut self,source:&Framebuffer,target:&mut Framebuffer){
        let (sw,sh,dw,dh)=(source.width as usize,source.height as usize,
            target.width as usize,target.height as usize);
        if sw==0||sh==0||dw==0||dh==0{return;}
        if source.pixels.len()<sw.saturating_mul(sh)||target.pixels.len()<dw.saturating_mul(dh){return;}
        // Integer magnification (especially 2x) is faster with contiguous
        // repeated writes than AVX2 gathers from the same source pixel.
        // The other modes still reuse the precalculated x mapping.
        let integer_x=dw>=sw && dw%sw==0;
        if !integer_x && sw<=i32::MAX as usize && self.dims!=(source.width,target.width){
            self.map.clear();self.map.reserve(dw);
            self.map.extend((0..dw).map(|x|(x*sw/dw) as i32));
            self.dims=(source.width,target.width);
        }
        #[cfg(any(target_arch="x86",target_arch="x86_64"))]
        let avx2={
            static AVX2:std::sync::OnceLock<bool>=std::sync::OnceLock::new();
            *AVX2.get_or_init(||std::is_x86_feature_detected!("avx2") &&
                !std::env::var("MATA_SIMD").is_ok_and(|v|v=="0"||v.eq_ignore_ascii_case("off")))
        };
        let mut previous_source_row=usize::MAX;
        for y in 0..dh{
            let sy=y*sh/dh;
            // Nearest-neighbour scaling often selects the identical source row
            // multiple times; memcpy the previously expanded row instead of
            // repeating every horizontal lookup/gather.
            if sy==previous_source_row {
                target.pixels.copy_within((y-1)*dw..y*dw,y*dw);
                continue;
            }
            previous_source_row=sy;
            let src=&source.pixels[sy*sw..(sy+1)*sw];
            let dst=&mut target.pixels[y*dw..(y+1)*dw];
            if dw==sw {
                dst.copy_from_slice(src);
            } else if integer_x {
                let repeat=dw/sw;
                for (&pixel,run) in src.iter().zip(dst.chunks_exact_mut(repeat)) {
                    run.fill(pixel);
                }
            } else if sw>i32::MAX as usize {
                for (x,pixel) in dst.iter_mut().enumerate(){*pixel=src[x*sw/dw];}
            } else {
                #[cfg(any(target_arch="x86",target_arch="x86_64"))]
                if avx2 {
                    // SAFETY: AVX2 checked at runtime; all map indices valid.
                    unsafe{gather_avx2(src,dst,&self.map)}
                    continue;
                }
                for (out,&sx) in dst.iter_mut().zip(self.map.iter()){
                    *out=src[sx as usize];
                }
            }
        }
    }
}
#[cfg(any(target_arch="x86",target_arch="x86_64"))]
#[target_feature(enable="avx2")]
unsafe fn gather_avx2(src:&[u32],dst:&mut[u32],map:&[i32]){
    let mut i=0;
    while i+8<=dst.len(){
        // SAFETY: map[i..i+8] always indexes valid elements of src, and
        // i+8 cannot exceed the mapped output row length.
        unsafe{
            let indices=_mm256_loadu_si256(map.as_ptr().add(i).cast());
            let pixels=_mm256_i32gather_epi32(src.as_ptr().cast::<i32>(),indices,4);
            _mm256_storeu_si256(dst.as_mut_ptr().add(i).cast(),pixels);
        }
        i+=8;
    }
    for k in i..dst.len(){dst[k]=src[map[k] as usize];}
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn matches_integer_nearest_sampling_for_non_multiple_of_eight_dimensions(){
        for (sw,sh,dw,dh) in [(1,1,19,11),(17,13,123,71),(117,93,473,299),
            (144,89,144,89),(31,23,7,5),(1,23,31,67)]{
            let src=Framebuffer{width:sw,height:sh,
                pixels:(0..sw*sh).map(|i|0xff000000|i.wrapping_mul(0x12345)).collect()};
            let mut dst=Framebuffer::new(dw,dh);
            let mut up=Upscaler::new();up.render(&src,&mut dst);
            for y in 0..dh as usize{for x in 0..dw as usize{
                assert_eq!(dst.pixels[y*dw as usize+x],
                    src.pixels[(y*sh as usize/dh as usize)*sw as usize+x*sw as usize/dw as usize]);
            }}
            // Cache hit must behave identically.
            let baseline=dst.pixels.clone();up.render(&src,&mut dst);
            assert_eq!(dst.pixels,baseline);
        }
    }
}
