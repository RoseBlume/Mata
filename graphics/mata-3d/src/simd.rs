//! Dependency-free, architecture-checked packed math for the software renderer.
//!
//! SSE2 is mandatory on x86-64; 32-bit x86 checks CPUID. AArch64 has NEON.
//! SIMD paths do no heap allocation and never read past the end of a slice.
//! Software pixel edge/depth tests use four lanes at a time. They may differ at
//! subpixel triangle boundaries due to changed floating-point associativity;
//! no geometry, material or resolution is discarded.
use crate::Vec3;

#[cfg(target_arch="x86")]
use std::arch::x86::*;
#[cfg(target_arch="x86_64")]
use std::arch::x86_64::*;
#[cfg(target_arch="aarch64")]
use std::arch::aarch64::*;

#[inline]
pub(crate) fn enabled()->bool{
    static ENABLED:std::sync::OnceLock<bool>=std::sync::OnceLock::new();
    *ENABLED.get_or_init(||{
        // Controlled fallback for A/B profiling or GPU driver investigations.
        if std::env::var("MATA_SIMD").is_ok_and(|v|v=="0"||v.eq_ignore_ascii_case("off")){
            return false;
        }
        #[cfg(target_arch="x86_64")]{return true;}
        #[cfg(target_arch="x86")]{return std::is_x86_feature_detected!("sse2");}
        #[cfg(target_arch="aarch64")]{return true;}
        #[allow(unreachable_code)] false
    })
}

/// Affine transform of an indexed mesh, one evaluation per distinct vertex.
/// Rows are [X,Y,Z,offset]; caller reuses the output buffer across meshes.
pub(crate) fn transform(src:&[Vec3],dst:&mut[Vec3],rows:[[f32;4];3]){
    assert_eq!(src.len(),dst.len());
    #[cfg(any(target_arch="x86",target_arch="x86_64"))]
    if enabled(){
        // SAFETY: CPUID verified on x86, mandatory feature on x86-64.
        unsafe{sse_transform(src,dst,rows)}
        return;
    }
    #[cfg(target_arch="aarch64")]
    if enabled(){
        // SAFETY: 128-bit NEON is a baseline AArch64 instruction set feature.
        unsafe{neon_transform(src,dst,rows)}
        return;
    }
    #[allow(unreachable_code)]
    for (&p,out) in src.iter().zip(dst.iter_mut()){
        *out=transform_one(p,rows);
    }
}
#[inline]
fn transform_one(p:Vec3,m:[[f32;4];3])->Vec3{
    Vec3::new(m[0][0]*p.x+m[0][1]*p.y+m[0][2]*p.z+m[0][3],
        m[1][0]*p.x+m[1][1]*p.y+m[1][2]*p.z+m[1][3],
        m[2][0]*p.x+m[2][1]*p.y+m[2][2]*p.z+m[2][3])
}
#[cfg(any(target_arch="x86",target_arch="x86_64"))]
#[target_feature(enable="sse2")]
unsafe fn sse_transform(src:&[Vec3],dst:&mut[Vec3],m:[[f32;4];3]){
    let mut i=0;
    while i+4<=src.len(){
        // Gather x/y/z from the AoS representation into contiguous SIMD lanes.
        let x=_mm_set_ps(src[i+3].x,src[i+2].x,src[i+1].x,src[i].x);
        let y=_mm_set_ps(src[i+3].y,src[i+2].y,src[i+1].y,src[i].y);
        let z=_mm_set_ps(src[i+3].z,src[i+2].z,src[i+1].z,src[i].z);
        macro_rules! eval {($row:expr)=>{{
            let row=$row;
            _mm_add_ps(_mm_add_ps(_mm_add_ps(_mm_mul_ps(x,_mm_set1_ps(row[0])),
                _mm_mul_ps(y,_mm_set1_ps(row[1]))),
                _mm_mul_ps(z,_mm_set1_ps(row[2]))),_mm_set1_ps(row[3]))
        }}}
        let mut a=[0.0f32;4];let mut b=[0.0f32;4];let mut c=[0.0f32;4];
        unsafe{
            _mm_storeu_ps(a.as_mut_ptr(),eval!(m[0]));
            _mm_storeu_ps(b.as_mut_ptr(),eval!(m[1]));
            _mm_storeu_ps(c.as_mut_ptr(),eval!(m[2]));
        }
        for k in 0..4{dst[i+k]=Vec3::new(a[k],b[k],c[k]);}
        i+=4;
    }
    for k in i..src.len(){dst[k]=transform_one(src[k],m);}
}
#[cfg(target_arch="aarch64")]
unsafe fn neon_transform(src:&[Vec3],dst:&mut[Vec3],m:[[f32;4];3]){
    let mut i=0;
    while i+4<=src.len(){
        let xx=[src[i].x,src[i+1].x,src[i+2].x,src[i+3].x];
        let yy=[src[i].y,src[i+1].y,src[i+2].y,src[i+3].y];
        let zz=[src[i].z,src[i+1].z,src[i+2].z,src[i+3].z];
        // SAFETY: the arrays hold exactly four initialized floats.
        let (x,y,z)=unsafe{(vld1q_f32(xx.as_ptr()),vld1q_f32(yy.as_ptr()),vld1q_f32(zz.as_ptr()))};
        macro_rules! eval {($row:expr)=>{{
            let row=$row;
            vaddq_f32(vaddq_f32(vaddq_f32(vmulq_n_f32(x,row[0]),
                vmulq_n_f32(y,row[1])),vmulq_n_f32(z,row[2])),vdupq_n_f32(row[3]))
        }}}
        let mut a=[0f32;4];let mut b=[0f32;4];let mut c=[0f32;4];
        unsafe{vst1q_f32(a.as_mut_ptr(),eval!(m[0]));vst1q_f32(b.as_mut_ptr(),eval!(m[1]));vst1q_f32(c.as_mut_ptr(),eval!(m[2]));}
        for k in 0..4{dst[i+k]=Vec3::new(a[k],b[k],c[k]);}
        i+=4;
    }
    for k in i..src.len(){dst[k]=transform_one(src[k],m);}
}

#[derive(Clone,Copy)]
pub(crate) struct Edges {pub e0:f32,pub e1:f32,pub e2:f32,pub z:f32,
    pub dx0:f32,pub dx1:f32,pub dx2:f32,pub dz:f32,pub sign:f32}

/// Run the entire clipped scanline, including scalar tail pixels.
/// Caller has already selected the right row and clamped xmin/xmax.
#[inline]
pub(crate) fn raster_row(depth:&mut[f32],pixels:&mut[u32],start:usize,end:usize,
    color:u32,mut e:Edges)->u32{
    debug_assert!(end<=depth.len()&&end<=pixels.len()&&start<=end);
    #[cfg(any(target_arch="x86",target_arch="x86_64"))]
    if enabled(){
        // SAFETY: feature detected; each 4-lane operation requires x+4<=end.
        return unsafe{sse_raster_row(depth,pixels,start,end,color,e)};
    }
    #[cfg(target_arch="aarch64")]
    if enabled(){
        return unsafe{neon_raster_row(depth,pixels,start,end,color,e)};
    }
    #[allow(unreachable_code)]
    {
        let mut count=0;
        for x in start..end{
            if e.e0*e.sign>=-0.002&&e.e1*e.sign>=-0.002&&e.e2*e.sign>=-0.002&&e.z>depth[x]{
                depth[x]=e.z;pixels[x]=color;count+=1;
            }
            e.e0+=e.dx0;e.e1+=e.dx1;e.e2+=e.dx2;e.z+=e.dz;
        }
        count
    }
}
#[cfg(any(target_arch="x86",target_arch="x86_64"))]
#[target_feature(enable="sse2")]
unsafe fn sse_raster_row(depth:&mut[f32],pixels:&mut[u32],start:usize,end:usize,
    color:u32,mut e:Edges)->u32{
    let lanes=_mm_set_ps(3.0,2.0,1.0,0.0);
    let threshold=_mm_set1_ps(-0.002);
    let sign=_mm_set1_ps(e.sign);
    let paint=_mm_set1_epi32(color as i32);
    let mut x=start;let mut painted=0;
    while x+4<=end{
        let v0=_mm_add_ps(_mm_set1_ps(e.e0),_mm_mul_ps(lanes,_mm_set1_ps(e.dx0)));
        let v1=_mm_add_ps(_mm_set1_ps(e.e1),_mm_mul_ps(lanes,_mm_set1_ps(e.dx1)));
        let v2=_mm_add_ps(_mm_set1_ps(e.e2),_mm_mul_ps(lanes,_mm_set1_ps(e.dx2)));
        let invz=_mm_add_ps(_mm_set1_ps(e.z),_mm_mul_ps(lanes,_mm_set1_ps(e.dz)));
        // First reject uncovered quads without reading the depth buffer.
        // Many high-detail OBJ triangles are thin or have large bounding boxes.
        let coverage=_mm_and_ps(_mm_and_ps(_mm_cmpge_ps(_mm_mul_ps(v0,sign),threshold),
            _mm_cmpge_ps(_mm_mul_ps(v1,sign),threshold)),
            _mm_cmpge_ps(_mm_mul_ps(v2,sign),threshold));
        if _mm_movemask_ps(coverage)!=0 {
            // SAFETY: x+4 <= end <= depth/pixels length.
            unsafe{
                let old=_mm_loadu_ps(depth.as_ptr().add(x));
                let mask=_mm_and_ps(coverage,_mm_cmpgt_ps(invz,old));
                let bits=_mm_movemask_ps(mask) as u32;
                if bits!=0{
                    if bits==15 {
                        // Fully covered, depth-passing quad: avoid reading
                        // old colours and avoid depth/color blending.
                        _mm_storeu_ps(depth.as_mut_ptr().add(x),invz);
                        _mm_storeu_si128(pixels.as_mut_ptr().add(x).cast(),paint);
                    } else {
                        let z=_mm_or_ps(_mm_and_ps(mask,invz),_mm_andnot_ps(mask,old));
                        _mm_storeu_ps(depth.as_mut_ptr().add(x),z);
                        let old_pixels=_mm_loadu_si128(pixels.as_ptr().add(x).cast());
                        let imask=_mm_castps_si128(mask);
                        let result=_mm_or_si128(_mm_and_si128(imask,paint),_mm_andnot_si128(imask,old_pixels));
                        _mm_storeu_si128(pixels.as_mut_ptr().add(x).cast(),result);
                    }
                    painted+=bits.count_ones();
                }
            }
        }
        e.e0+=e.dx0*4.0;e.e1+=e.dx1*4.0;e.e2+=e.dx2*4.0;e.z+=e.dz*4.0;
        x+=4;
    }
    for i in x..end{
        if e.e0*e.sign>=-0.002&&e.e1*e.sign>=-0.002&&e.e2*e.sign>=-0.002&&e.z>depth[i]{
            depth[i]=e.z;pixels[i]=color;painted+=1;
        }
        e.e0+=e.dx0;e.e1+=e.dx1;e.e2+=e.dx2;e.z+=e.dz;
    }
    painted
}
#[cfg(target_arch="aarch64")]
unsafe fn neon_raster_row(depth:&mut[f32],pixels:&mut[u32],start:usize,end:usize,
    color:u32,mut e:Edges)->u32{
    let lanes=unsafe{vld1q_f32([0f32,1.0,2.0,3.0].as_ptr())};
    let threshold=vdupq_n_f32(-0.002);
    let sign=vdupq_n_f32(e.sign);
    let mut x=start;let mut painted=0;
    while x+4<=end{
        let v0=vaddq_f32(vdupq_n_f32(e.e0),vmulq_n_f32(lanes,e.dx0));
        let v1=vaddq_f32(vdupq_n_f32(e.e1),vmulq_n_f32(lanes,e.dx1));
        let v2=vaddq_f32(vdupq_n_f32(e.e2),vmulq_n_f32(lanes,e.dx2));
        let invz=vaddq_f32(vdupq_n_f32(e.z),vmulq_n_f32(lanes,e.dz));
        unsafe{
            let old=vld1q_f32(depth.as_ptr().add(x));
            let mask=vandq_u32(vandq_u32(vcgeq_f32(vmulq_f32(v0,sign),threshold),
                vcgeq_f32(vmulq_f32(v1,sign),threshold)),
                vandq_u32(vcgeq_f32(vmulq_f32(v2,sign),threshold),vcgtq_f32(invz,old)));
            let mut selected=[0u32;4];vst1q_u32(selected.as_mut_ptr(),mask);
            let n=selected.iter().filter(|&&v|v!=0).count() as u32;
            if n>0{
                vst1q_f32(depth.as_mut_ptr().add(x),vbslq_f32(mask,invz,old));
                let old_pixels=vld1q_u32(pixels.as_ptr().add(x));
                vst1q_u32(pixels.as_mut_ptr().add(x),vbslq_u32(mask,vdupq_n_u32(color),old_pixels));
                painted+=n;
            }
        }
        e.e0+=4.0*e.dx0;e.e1+=4.0*e.dx1;e.e2+=4.0*e.dx2;e.z+=4.0*e.dz;
        x+=4;
    }
    for i in x..end{
        if e.e0*e.sign>=-0.002&&e.e1*e.sign>=-0.002&&e.e2*e.sign>=-0.002&&e.z>depth[i]{
            depth[i]=e.z;pixels[i]=color;painted+=1;
        }
        e.e0+=e.dx0;e.e1+=e.dx1;e.e2+=e.dx2;e.z+=e.dz;
    }
    painted
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn transform_matches_scalar_with_tolerance(){
        let v:Vec<_>=(0..103).map(|i|Vec3::new((i as f32)*0.112-4.0,i as f32*0.07,(i as f32).sin())).collect();
        let m=[[0.63,-1.2,0.91,4.0],[1.1,0.12,-0.85,-2.0],[0.5,-0.67,0.31,3.0]];
        let mut result=vec![Vec3::default();v.len()];transform(&v,&mut result,m);
        for (p,&expected) in v.iter().zip(result.iter()){
            let a=transform_one(*p,m);
            assert!((a.x-expected.x).abs()<0.00002);
            assert!((a.y-expected.y).abs()<0.00002);
            assert!((a.z-expected.z).abs()<0.00002);
        }
    }
    #[test]fn negative_winding_and_partial_lanes_are_rejected_correctly(){
        let e=Edges{e0:0.0,e1:-1.0,e2:-2.0,z:0.7,
            dx0:-0.4,dx1:0.0,dx2:0.0,dz:-0.09,sign:-1.0};
        let mut d=[0.3f32;13];let mut p=[0x12121212u32;13];
        let painted=raster_row(&mut d,&mut p,1,12,0xff334455,e);
        // e0 * (-1) is nonnegative at each sample, but depth only passes
        // for the first five pixels until invZ drops to 0.25.
        assert_eq!(painted,5);
        assert!(p[1..6].iter().all(|&v|v==0xff334455));
        assert!(p[6..12].iter().all(|&v|v==0x12121212));
        assert_eq!(p[12],0x12121212);
    }
    #[test]fn rows_obey_coverage_and_depth(){
        let mut depth=[0.1f32;19];let mut pixels=[17u32;19];
        let e=Edges{e0:100.0,e1:100.0,e2:100.0,z:0.2,dx0:0.0,dx1:0.0,dx2:0.0,dz:0.0,sign:1.0};
        assert_eq!(raster_row(&mut depth,&mut pixels,2,17,0xff00eeff,e),15);
        assert_eq!(depth[1],0.1);assert_eq!(pixels[17],17);
        assert!(pixels[2..17].iter().all(|&p|p==0xff00eeff));
        assert_eq!(raster_row(&mut depth,&mut pixels,2,17,0x12345678,e),0);
        let rejected=Edges{e0:-1.0,..Edges{z:0.7,..e}};
        assert_eq!(raster_row(&mut depth,&mut pixels,0,19,0,rejected),0);
    }
}
