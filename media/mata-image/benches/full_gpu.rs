//! Benchmark native CPU vs three GPU execution runtimes on eligible formats.
//! A GPU result is only reported as real if the whole-image dispatch counter
//! advanced. Without a driver/compiled bridge, output says "CPU fallback".
use std::time::Instant;
use std::hint::black_box;
use mata_image::{DecodeBackend, DecodeOptions, GpuRuntime, Image, ImageFormat};
fn qoi_fixture(width:u32,height:u32)->Vec<u8> {
    let mut encoded=b"qoif".to_vec();
    encoded.extend(width.to_be_bytes());encoded.extend(height.to_be_bytes());
    encoded.extend([4,0,0xff,20,90,160,255]);
    let mut left=width as usize * height as usize - 1;
    while left>0 {let run=left.min(62);encoded.push(0xc0 | (run as u8 - 1));left-=run;}
    encoded.extend([0,0,0,0,0,0,0,1]);encoded
}
fn bench(name:&str,fmt:ImageFormat,source:&[u8],backend:DecodeBackend,runtime:GpuRuntime) {
    let options=DecodeOptions::default().with_backend(backend).with_gpu_runtime(runtime);
    let expected=Image::from_bytes(source).unwrap();
    let before=Image::full_gpu_dispatches(fmt);
    let warm=Image::from_bytes_with_options(source,options).unwrap();
    assert_eq!(warm,expected);
    let did_dispatch=Image::full_gpu_dispatches(fmt)>before;
    let runs=5;
    let start=Instant::now();
    for _ in 0..runs {
        let image=Image::from_bytes_with_options(black_box(source),options).unwrap();
        black_box(image);
    }
    println!("{name:<12} {:<10?} {:<7?} {:>8.3} ms  {}",backend,runtime,
        start.elapsed().as_secs_f64()*1000.0/runs as f64,
        if did_dispatch{"whole-GPU kernel"}else{"CPU fallback"});
}
fn png_webp_cases() {
    let cases = [
        ("PNG zlib",ImageFormat::Png,&include_bytes!("../tests/fixtures_png_perf/wide_stored.png")[..]),
        ("WebP VP8L",ImageFormat::WebP,&include_bytes!("../tests/fixtures_gpu/vp8l_gpu_simple_300x150.webp")[..]),
        ("WebP VP8",ImageFormat::WebP,&include_bytes!("../tests/fixtures_gpu/vp8_small.webp")[..]),
    ];
    for (name,fmt,source) in cases {
        for (backend,runtime) in [
            (DecodeBackend::Cpu,GpuRuntime::Auto),
            (DecodeBackend::IntelGpu,GpuRuntime::OpenCl),
            (DecodeBackend::AmdGpu,GpuRuntime::OpenCl),
            (DecodeBackend::AmdGpu,GpuRuntime::Hip),
            (DecodeBackend::NvidiaGpu,GpuRuntime::OpenCl),
            (DecodeBackend::NvidiaGpu,GpuRuntime::Cuda),
        ] {bench(name,fmt,source,backend,runtime);}
    }
}
fn main(){
    png_webp_cases();
    let qoi=qoi_fixture(1024,1024);
    for (backend, runtime) in [
        (DecodeBackend::Cpu,GpuRuntime::Auto),
        (DecodeBackend::IntelGpu,GpuRuntime::OpenCl),
        (DecodeBackend::AmdGpu,GpuRuntime::OpenCl),
        (DecodeBackend::AmdGpu,GpuRuntime::Hip),
        (DecodeBackend::NvidiaGpu,GpuRuntime::OpenCl),
        (DecodeBackend::NvidiaGpu,GpuRuntime::Cuda),
    ] {bench("QOI 1MP",ImageFormat::Qoi,&qoi,backend,runtime);}
}
