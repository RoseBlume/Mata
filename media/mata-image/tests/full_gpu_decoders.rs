//! End-to-end regression cases. On machines without GPU runtimes these still
//! guarantee that explicit GPU preferences retain correct CPU fallback.
use mata_image::{DecodeBackend, DecodeOptions, GpuRuntime, Image, ImageFormat};

fn all_profiles() -> [(DecodeBackend,GpuRuntime);8] {
    [
        (DecodeBackend::IntelGpu,GpuRuntime::OpenCl),
        (DecodeBackend::AmdGpu,GpuRuntime::OpenCl),
        (DecodeBackend::NvidiaGpu,GpuRuntime::OpenCl),
        (DecodeBackend::AmdGpu,GpuRuntime::Hip),
        (DecodeBackend::NvidiaGpu,GpuRuntime::Cuda),
        (DecodeBackend::AmdGpu,GpuRuntime::Auto),
        (DecodeBackend::NvidiaGpu,GpuRuntime::Auto),
        (DecodeBackend::Cpu,GpuRuntime::Auto),
    ]
}
fn compare(bytes:&[u8],format:ImageFormat) {
    let expected=Image::from_bytes(bytes).expect("CPU fixture");
    for (backend,gpu_runtime) in all_profiles() {
        let options=DecodeOptions::default().with_backend(backend).with_gpu_runtime(gpu_runtime);
        let before=Image::full_gpu_dispatches(format);
        let actual=Image::from_bytes_with_options(bytes,options)
            .unwrap_or_else(|e|panic!("{format:?} {backend:?} {gpu_runtime:?}: {e}"));
        assert_eq!(actual,expected,"{format:?} {backend:?} {gpu_runtime:?}");
        let after=Image::full_gpu_dispatches(format);
        let _actual_dispatches=after.saturating_sub(before);
    }
}
#[cfg(feature="qoi")]
#[test] fn qoi_full_decode() {
    let mut data=b"qoif\0\0\0\x09\0\0\0\x08\x04\0".to_vec();
    // RGBA, run, RGB, QOI_INDEX (the first index), then a second run.
    data.extend([0xff,15,25,35,45,0xc2,0xfe,70,80,90,0xc0]);
    // Supply remaining pixels as long repeat runs of that second RGB colour.
    let mut output_so_far=1+3+1+1;
    while output_so_far<72 {
        let run=(72-output_so_far).min(62);
        data.push(0xc0u8 | (run as u8-1));
        output_so_far+=run;
    }
    data.extend([0,0,0,0,0,0,0,1]);
    compare(&data,ImageFormat::Qoi);
    let mut invalid=data;
    invalid.pop();
    assert!(Image::from_bytes(&invalid).is_err());
}
#[cfg(feature="tga")]
#[test] fn tga_raw_and_rle_all_origins() {
    for raw in [true,false] {
        for descriptor in [0u8,0x20,0x10,0x30] {
            let w=13u16; let h=9u16;
            let mut data=vec![0u8;18];
            data[2]=if raw{2}else{10};
            data[12..14].copy_from_slice(&w.to_le_bytes());
            data[14..16].copy_from_slice(&h.to_le_bytes());
            data[16]=32;data[17]=descriptor;
            if raw {
                for i in 0..u32::from(w)*u32::from(h) {
                    data.extend([(i*7) as u8,(i*13) as u8,(i*17) as u8,((i*3)|127) as u8]);
                }
            } else {
                // A mix of repeated and raw packets.
                data.push(0x80|31);data.extend([6,7,8,90]);
                data.push(32);for i in 0..33u8 {data.extend([i,i.wrapping_mul(3),i.wrapping_mul(5),255]);}
                data.push(0x80|51);data.extend([2,8,16,128]);
                // 32 + 33 + 52 = 117 = 13x9.
            }
            compare(&data,ImageFormat::Tga);
        }
    }
}
#[cfg(feature="wbmp")]
#[test] fn wbmp_bit_rows_and_padding() {
    let mut data=vec![0,0,0x81,0x01,0x02]; // width 129, height 2
    for y in 0..2 {for x in 0..17 {data.push(((x*7+y*81)&255) as u8);}}
    compare(&data,ImageFormat::Wbmp);
}
#[cfg(feature="bmp")]
#[test] fn bmp_uncompressed_padding_and_top_down() {
    for bpp in [24u16,32u16] {
        for signed_h in [7i32,-7i32] {
            let w=11u32;let h=7u32;
            let stride=((w*(u32::from(bpp)/8)+3)/4)*4;
            let size=stride*h;
            let mut data=vec![0u8;54+size as usize];
            data[..2].copy_from_slice(b"BM");
            let len=data.len() as u32;
            data[2..6].copy_from_slice(&len.to_le_bytes());
            data[10..14].copy_from_slice(&54u32.to_le_bytes());
            data[14..18].copy_from_slice(&40u32.to_le_bytes());
            data[18..22].copy_from_slice(&w.to_le_bytes());
            data[22..26].copy_from_slice(&signed_h.to_le_bytes());
            data[26..28].copy_from_slice(&1u16.to_le_bytes());
            data[28..30].copy_from_slice(&bpp.to_le_bytes());
            let channels=(bpp/8) as usize;
            for y in 0..h as usize {for x in 0..w as usize {
                let off=54+y*stride as usize+x*channels;
                data[off..off+3].copy_from_slice(&[(x*9) as u8,(y*7) as u8,(x*3+y*5) as u8]);
                if channels==4 {data[off+3]=0;}
            }}
            compare(&data,ImageFormat::Bmp);
        }
    }
}
