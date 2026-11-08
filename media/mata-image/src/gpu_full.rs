//! Opt-in whole-image device decoding including selected PNG and VP8L streams,
//! QOI, Type-0 WBMP, BMP and true-colour TGA.
//!
//! Metadata/header checks stay on the host; compressed QOI opcodes and TGA RLE
//! are decoded entirely by device kernels. Unsupported cases use the existing
//! CPU decoder. The backend must report success before a result is accepted.
use crate::{DecodeBackend, DecodeOptions, GpuRuntime, ImageFormat};
use std::sync::atomic::{AtomicU64, Ordering};

static FULL_QOI: AtomicU64 = AtomicU64::new(0);
static FULL_TGA: AtomicU64 = AtomicU64::new(0);
static FULL_WBMP: AtomicU64 = AtomicU64::new(0);
static FULL_BMP: AtomicU64 = AtomicU64::new(0);
static FULL_PNG: AtomicU64 = AtomicU64::new(0);
static FULL_WEBP: AtomicU64 = AtomicU64::new(0);

pub(crate) fn dispatches(format: ImageFormat) -> u64 {
    match format {
        ImageFormat::Qoi => FULL_QOI.load(Ordering::Relaxed),
        ImageFormat::Tga => FULL_TGA.load(Ordering::Relaxed),
        ImageFormat::Wbmp => FULL_WBMP.load(Ordering::Relaxed),
        ImageFormat::Bmp => FULL_BMP.load(Ordering::Relaxed),
        ImageFormat::Png => FULL_PNG.load(Ordering::Relaxed),
        ImageFormat::WebP => FULL_WEBP.load(Ordering::Relaxed),
        _ => 0,
    }
}

#[derive(Clone, Copy)]
struct Job {
    format: ImageFormat,
    width: u32,
    height: u32,
    offset: u32,
    mode: u32,
}

fn be32(input: &[u8]) -> Option<u32> { Some(u32::from_be_bytes(input.try_into().ok()?)) }
fn le16(input: &[u8]) -> Option<u16> { Some(u16::from_le_bytes(input.try_into().ok()?)) }
fn le32(input: &[u8]) -> Option<u32> { Some(u32::from_le_bytes(input.try_into().ok()?)) }

fn wbmp_int(bytes: &[u8], pos: &mut usize) -> Option<u32> {
    let mut value=0u32;
    for _ in 0..5 {
        let b=*bytes.get(*pos)?;
        *pos += 1;
        if value > u32::MAX >> 7 { return None; }
        value = (value << 7) | u32::from(b & 127);
        if b & 128 == 0 { return Some(value); }
    }
    None
}

fn parse(format: ImageFormat, bytes: &[u8]) -> Option<Job> {
    let job = match format {
        ImageFormat::Qoi => {
            if bytes.get(..4)? != b"qoif" || bytes.len() < 22 || !matches!(bytes[12],3|4) || bytes[13]>1 { return None; }
            Job {format,width:be32(bytes.get(4..8)?)?,height:be32(bytes.get(8..12)?)?,offset:14,mode:0}
        }
        ImageFormat::Tga => {
            if bytes.len()<18 || bytes[1]!=0 || !matches!(bytes[2],2|10) { return None; }
            let bytes_per_pixel=match bytes[16] {24=>3u32,32=>4u32,_=>return None};
            let offset=18usize.checked_add(bytes[0] as usize)?;
            if offset>bytes.len() { return None; }
            let mut mode=bytes_per_pixel;
            if bytes[2]==10 {mode|=0x100;}
            if bytes[17]&0x20!=0 {mode|=0x200;}
            if bytes[17]&0x10!=0 {mode|=0x400;}
            Job {format,width:le16(bytes.get(12..14)?)? as u32,
                 height:le16(bytes.get(14..16)?)? as u32,offset:u32::try_from(offset).ok()?,mode}
        }
        ImageFormat::Bmp => {
            if bytes.len()<54 || bytes.get(..2)?!=b"BM" {return None;}
            let dib=le32(bytes.get(14..18)?)?;
            if dib<40 || (14u64+dib as u64)>bytes.len() as u64 {return None;}
            let width=le32(bytes.get(18..22)?)? as i32;
            let signed_h=le32(bytes.get(22..26)?)? as i32;
            let bpp=le16(bytes.get(28..30)?)?;
            if width<=0 || signed_h==0 || signed_h==i32::MIN ||
               le16(bytes.get(26..28)?)?!=1 || le32(bytes.get(30..34)?)?!=0 ||
               !matches!(bpp,24|32) {return None;}
            let height=signed_h.unsigned_abs();
            let pixel_bytes=u32::from(bpp)/8;
            let stride=(u64::try_from(width).ok()?*u64::from(pixel_bytes)+3)&!3;
            let off=le32(bytes.get(10..14)?)?;
            if u64::from(off)+stride*u64::from(height)>bytes.len() as u64 {return None;}
            let mut mode=pixel_bytes;
            if signed_h<0 {mode|=0x200;}
            Job {format,width:width as u32,height,offset:off,mode}
        }
        ImageFormat::Wbmp => {
            if bytes.get(0..2)? != [0,0] {return None;}
            let mut off=2;
            let width=wbmp_int(bytes,&mut off)?;
            let height=wbmp_int(bytes,&mut off)?;
            let row=usize::try_from(width).ok()?.checked_add(7)?/8;
            let end=off.checked_add(row.checked_mul(height as usize)?)?;
            if end>bytes.len() {return None;}
            Job {format,width,height,offset:u32::try_from(off).ok()?,mode:u32::try_from(row).ok()?}
        }
        _=>return None,
    };
    if job.width==0 || job.height==0 {return None;}
    Some(job)
}

/// Attempts an end-to-end device pixel decode, never suppressing CPU errors.
/// Explicit runtime selection is strict; Auto tries the vendor-native API first.
pub(crate) fn decode(format: ImageFormat, bytes: &[u8], options: &DecodeOptions) -> Option<(u32,u32,Vec<u32>)> {
    if options.backend == DecodeBackend::Cpu {return None;}
    let job=parse(format,bytes)?;
    let count=options.checked_pixel_count(job.width,job.height).ok()?;
    options.checked_rgba_bytes(job.width,job.height).ok()?;
    if count==0 || count>u32::MAX as usize || bytes.len()>u32::MAX as usize {return None;}
    let try_cl=||crate::gpu::full_decode_opencl(bytes, count, job.width,job.height,job.offset,job.mode,job.format,options.backend);
    let try_native=|runtime| native::decode(runtime, bytes,count,job.width,job.height,job.offset,job.mode,job.format,options.backend);
    let pixels = match options.gpu_runtime {
        GpuRuntime::OpenCl => try_cl(),
        GpuRuntime::Cuda => try_native(GpuRuntime::Cuda),
        GpuRuntime::Hip => try_native(GpuRuntime::Hip),
        GpuRuntime::Auto => match options.backend {
            DecodeBackend::NvidiaGpu => try_native(GpuRuntime::Cuda).or_else(try_cl),
            DecodeBackend::AmdGpu => try_native(GpuRuntime::Hip).or_else(try_cl),
            _ => try_cl(),
        }
    }?;
    if pixels.len()!=count {return None;}
    match format {
        ImageFormat::Qoi => {FULL_QOI.fetch_add(1,Ordering::Relaxed);}
        ImageFormat::Tga => {FULL_TGA.fetch_add(1,Ordering::Relaxed);}
        ImageFormat::Wbmp => {FULL_WBMP.fetch_add(1,Ordering::Relaxed);}
        ImageFormat::Bmp => {FULL_BMP.fetch_add(1,Ordering::Relaxed);}
        _=>return None,
    }
    Some((job.width,job.height,pixels))
}

/// GPU VP8L profile with full device entropy/colour decoding, covering
/// no-transform single-group streams. Missing profiles use the CPU decoder.
pub(crate) fn decode_webp_vp8l(vp8l:&[u8],w:u32,h:u32,options:&DecodeOptions)->Option<Vec<u32>> {
    if options.backend==DecodeBackend::Cpu {return None;}
    let try_cl=||crate::gpu::webp_lossless_opencl(vp8l,w,h,options.backend);
    let try_native=|runtime|native::decode_webp(runtime,vp8l,w,h,options.backend);
    let decoded=match options.gpu_runtime{
        GpuRuntime::OpenCl=>try_cl(),
        GpuRuntime::Cuda=>try_native(GpuRuntime::Cuda),
        GpuRuntime::Hip=>try_native(GpuRuntime::Hip),
        GpuRuntime::Auto=>match options.backend{
            DecodeBackend::NvidiaGpu=>try_native(GpuRuntime::Cuda).or_else(try_cl),
            DecodeBackend::AmdGpu=>try_native(GpuRuntime::Hip).or_else(try_cl),
            _=>try_cl(),
        }
    }?;
    if decoded.len()!=(w as usize).checked_mul(h as usize)? {return None;}
    FULL_WEBP.fetch_add(1,Ordering::Relaxed);
    Some(decoded)
}

/// Whole-device PNG decode for 8-bit noninterlaced gray/gray-alpha/RGB/RGBA.
/// The caller has already validated PNG structure, chunk CRC and limits.
/// Returns None on driver failure or an unsupported profile, never partial pixels.
pub(crate) fn decode_png(zlib:&[u8],w:u32,h:u32,channels:u32,options:&DecodeOptions)->Option<Vec<u32>> {
    if options.backend==DecodeBackend::Cpu {return None;}
    let try_cl=||crate::gpu::png_zlib_opencl(zlib,w,h,channels,options.backend);
    let try_native=|runtime|native::decode_png(runtime,zlib,w,h,channels,options.backend);
    let decoded=match options.gpu_runtime {
        GpuRuntime::OpenCl=>try_cl(),
        GpuRuntime::Hip=>try_native(GpuRuntime::Hip),
        GpuRuntime::Cuda=>try_native(GpuRuntime::Cuda),
        GpuRuntime::Auto=>match options.backend {
            DecodeBackend::NvidiaGpu=>try_native(GpuRuntime::Cuda).or_else(try_cl),
            DecodeBackend::AmdGpu=>try_native(GpuRuntime::Hip).or_else(try_cl),
            _=>try_cl(),
        }
    }?;
    if decoded.len() != (w as usize).checked_mul(h as usize)? {return None;}
    FULL_PNG.fetch_add(1,Ordering::Relaxed);
    Some(decoded)
}

#[cfg(all(any(feature="gpu-cuda",feature="gpu-hip"),any(target_os="linux",target_os="windows")))]
mod native {
    use super::*;
    use std::ffi::{c_char,c_void,CStr};
    use std::sync::OnceLock;
    type Decoder = unsafe extern "C" fn(u32,*const u8,usize,u32,u32,u32,u32,*mut u32)->i32;
    type PngDecoder = unsafe extern "C" fn(*const u8,usize,u32,u32,u32,*mut u32)->i32;
    type WebpDecoder = unsafe extern "C" fn(*const u8,usize,u32,u32,*mut u32)->i32;
    #[cfg(target_os="linux")]
    #[link(name="dl")]
    unsafe extern "C" {
        fn dlopen(path:*const c_char,mode:i32)->*mut c_void;
        fn dlsym(lib:*mut c_void,name:*const c_char)->*mut c_void;
    }
    #[cfg(target_os="windows")]
    #[link(name="kernel32")]
    unsafe extern "system" {
        fn LoadLibraryA(path:*const c_char)->*mut c_void;
        fn GetProcAddress(lib:*mut c_void,name:*const c_char)->*mut c_void;
    }
    fn load(name:&CStr,symbol:&CStr)->Option<Decoder> {
        #[cfg(target_os="linux")]
        let lib=unsafe{dlopen(name.as_ptr(),2)};
        #[cfg(target_os="windows")]
        let lib=unsafe{LoadLibraryA(name.as_ptr())};
        if lib.is_null(){return None;}
        #[cfg(target_os="linux")]
        let sym=unsafe{dlsym(lib,symbol.as_ptr())};
        #[cfg(target_os="windows")]
        let sym=unsafe{GetProcAddress(lib,symbol.as_ptr())};
        if sym.is_null(){return None;}
        // Deliberately retain the module for the lifetime of the process:
        // cached function pointers must never outlive a dlclosed library.
        Some(unsafe{std::mem::transmute::<*mut c_void,Decoder>(sym)})
    }
    #[cfg(target_os="linux")]
    const CUDA_NAME: &CStr=c"libmataimage_cuda.so";
    #[cfg(target_os="linux")]
    const HIP_NAME: &CStr=c"libmataimage_hip.so";
    #[cfg(target_os="windows")]
    const CUDA_NAME: &CStr=c"mataimage_cuda.dll";
    #[cfg(target_os="windows")]
    const HIP_NAME: &CStr=c"mataimage_hip.dll";
    static CUDA: OnceLock<Option<Decoder>>=OnceLock::new();
    static HIP: OnceLock<Option<Decoder>>=OnceLock::new();
    static CUDA_WEBP:OnceLock<Option<WebpDecoder>>=OnceLock::new();
    static HIP_WEBP:OnceLock<Option<WebpDecoder>>=OnceLock::new();
    pub(super) fn decode_webp(runtime:GpuRuntime,vp8l:&[u8],w:u32,h:u32,backend:DecodeBackend)->Option<Vec<u32>>{
        let f=match (runtime,backend){
            #[cfg(feature="gpu-cuda")]
            (GpuRuntime::Cuda,DecodeBackend::NvidiaGpu)=>*CUDA_WEBP.get_or_init(||load(CUDA_NAME,c"mataimage_cuda_webp_decode").map(|f|unsafe{std::mem::transmute::<Decoder,WebpDecoder>(f)})),
            #[cfg(feature="gpu-hip")]
            (GpuRuntime::Hip,DecodeBackend::AmdGpu)=>*HIP_WEBP.get_or_init(||load(HIP_NAME,c"mataimage_hip_webp_decode").map(|f|unsafe{std::mem::transmute::<Decoder,WebpDecoder>(f)})),
            _=>None,
        }?;
        let count=(w as usize).checked_mul(h as usize)?;
        let mut pixels=vec![0u32;count];
        (unsafe{f(vp8l.as_ptr(),vp8l.len(),w,h,pixels.as_mut_ptr())}==0).then_some(pixels)
    }
    static CUDA_PNG:OnceLock<Option<PngDecoder>>=OnceLock::new();
    static HIP_PNG:OnceLock<Option<PngDecoder>>=OnceLock::new();
    pub(super) fn decode_png(runtime:GpuRuntime,zlib:&[u8],width:u32,height:u32,channels:u32,backend:DecodeBackend)->Option<Vec<u32>> {
        let symbol=match (runtime,backend) {
            #[cfg(feature="gpu-cuda")]
            (GpuRuntime::Cuda,DecodeBackend::NvidiaGpu)=>*CUDA_PNG.get_or_init(||load(CUDA_NAME,c"mataimage_cuda_png_decode").map(|f|unsafe{std::mem::transmute::<Decoder,PngDecoder>(f)})),
            #[cfg(feature="gpu-hip")]
            (GpuRuntime::Hip,DecodeBackend::AmdGpu)=>*HIP_PNG.get_or_init(||load(HIP_NAME,c"mataimage_hip_png_decode").map(|f|unsafe{std::mem::transmute::<Decoder,PngDecoder>(f)})),
            _=>None,
        }?;
        let count=(width as usize).checked_mul(height as usize)?;
        let mut pixels=vec![0u32;count];
        (unsafe{symbol(zlib.as_ptr(),zlib.len(),width,height,channels,pixels.as_mut_ptr())}==0).then_some(pixels)
    }
    pub(super) fn decode(runtime:GpuRuntime,source:&[u8],count:usize,width:u32,height:u32,offset:u32,mode:u32,
        format:ImageFormat,backend:DecodeBackend)->Option<Vec<u32>> {
        let function=match (runtime,backend) {
            #[cfg(feature="gpu-cuda")]
            (GpuRuntime::Cuda,DecodeBackend::NvidiaGpu)=>*CUDA.get_or_init(||load(CUDA_NAME,c"mataimage_cuda_decode")),
            #[cfg(feature="gpu-hip")]
            (GpuRuntime::Hip,DecodeBackend::AmdGpu)=>*HIP.get_or_init(||load(HIP_NAME,c"mataimage_hip_decode")),
            _=>None,
        }?;
        let id=match format { ImageFormat::Qoi=>1, ImageFormat::Tga=>2,ImageFormat::Wbmp=>3,ImageFormat::Bmp=>4,_=>return None };
        let mut pixels=vec![0u32;count];
        let result=unsafe{function(id,source.as_ptr(),source.len(),width,height,offset,mode,pixels.as_mut_ptr())};
        (result==0).then_some(pixels)
    }
}
#[cfg(not(all(any(feature="gpu-cuda",feature="gpu-hip"),any(target_os="linux",target_os="windows"))))]
mod native {
    use super::*;
    pub(super) fn decode_webp(_:GpuRuntime,_:&[u8],_:u32,_:u32,_:DecodeBackend)->Option<Vec<u32>> {None}
    pub(super) fn decode_png(_:GpuRuntime,_:&[u8],_:u32,_:u32,_:u32,_:DecodeBackend)->Option<Vec<u32>> {None}
    pub(super) fn decode(_:GpuRuntime,_:&[u8],_:usize,_:u32,_:u32,_:u32,_:u32,_:ImageFormat,_:DecodeBackend)->Option<Vec<u32>> {None}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn qoi_header_and_limits() {
        let mut qoi=b"qoif\0\0\0\x01\0\0\0\x01\x04\0\xff\x12\x34\x56\xff\0\0\0\0\0\0\0\x01".to_vec();
        assert!(parse(ImageFormat::Qoi,&qoi).is_some());
        qoi[12]=9;
        assert!(parse(ImageFormat::Qoi,&qoi).is_none());
    }
    #[test] fn tga_raw_and_rle_headers() {
        let mut src=[0u8;21];src[2]=2;src[12]=1;src[14]=1;src[16]=24;
        assert_eq!(parse(ImageFormat::Tga,&src).unwrap().mode,3);
        src[2]=10;
        assert_eq!(parse(ImageFormat::Tga,&src).unwrap().mode,0x103);
    }
}
