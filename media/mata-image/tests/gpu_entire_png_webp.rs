//! Correctness across automatic CPU fallback and optional full-device decoding.
//! Tests do not require a graphics card; GPU output is compared whenever a
//! supported backend is present. Failure to dispatch is a valid CPU fallback.
use mata_image::{DecodeBackend, DecodeOptions, GpuRuntime, Image, ImageFormat};

fn profiles() -> [(DecodeBackend,GpuRuntime);7] {
    [
        (DecodeBackend::IntelGpu,GpuRuntime::OpenCl),
        (DecodeBackend::AmdGpu,GpuRuntime::OpenCl),
        (DecodeBackend::NvidiaGpu,GpuRuntime::OpenCl),
        (DecodeBackend::AmdGpu,GpuRuntime::Hip),
        (DecodeBackend::NvidiaGpu,GpuRuntime::Cuda),
        (DecodeBackend::AmdGpu,GpuRuntime::Auto),
        (DecodeBackend::NvidiaGpu,GpuRuntime::Auto),
    ]
}
fn check(bytes:&[u8],format:ImageFormat) {
    let expected=Image::from_bytes(bytes).expect("CPU fixture decode");
    for (backend,runtime) in profiles() {
        let before=Image::full_gpu_dispatches(format);
        let actual=Image::from_bytes_with_options(bytes,
            DecodeOptions::default().with_backend(backend).with_gpu_runtime(runtime))
            .unwrap_or_else(|e|panic!("{format:?} {backend:?} {runtime:?}: {e}"));
        assert_eq!(expected,actual,"{format:?} {backend:?} {runtime:?}");
        assert!(Image::full_gpu_dispatches(format)>=before);
    }
}
#[cfg(feature="png")]
#[test]
fn png_entire_gpu_and_fallback_profiles() {
    // Covers stored/fixed/dynamic DEFLATE, varying row filters, IDAT chunking,
    // all common 8-bit non-palette formats, and 16-bit/palette CPU fallback.
    for bytes in [
        &include_bytes!("fixtures_png_perf/gray8_single.png")[..],
        &include_bytes!("fixtures_png_perf/graya8_multi.png")[..],
        &include_bytes!("fixtures_png_perf/rgb8_single.png")[..],
        &include_bytes!("fixtures_png_perf/rgba8_multi.png")[..],
        &include_bytes!("fixtures_png_perf/wide_stored.png")[..],
        &include_bytes!("fixtures_png_perf/indexed4_single.png")[..],
        &include_bytes!("fixtures_png_perf/rgba16_multi.png")[..],
    ] {check(bytes,ImageFormat::Png);}
}
#[cfg(feature="webp")]
#[test]
fn webp_lossless_gpu_and_complex_cpu_fallback() {
    for bytes in [
        &include_bytes!("fixtures_gpu/vp8l_gpu_simple_1x1.webp")[..],
        &include_bytes!("fixtures_gpu/vp8l_gpu_simple_300x150.webp")[..],
        &include_bytes!("fixtures_gpu/vp8l_gpu_simple_49x57.webp")[..],
        &include_bytes!("fixtures_gpu/vp8l_large.webp")[..],
        &include_bytes!("fixtures_gpu/vp8_large.webp")[..],
    ] {check(bytes,ImageFormat::WebP);}
}
#[cfg(feature="webp")]
#[test]
fn malformed_webp_container_falls_back_to_correct_error() {
    let mut bytes=include_bytes!("fixtures_gpu/vp8l_gpu_simple_31x14.webp").to_vec();
    bytes[0]=0;
    assert!(Image::from_bytes(&bytes).is_err());
    for (backend,runtime) in profiles() {
        assert!(Image::from_bytes_with_options(&bytes,
            DecodeOptions::default().with_backend(backend).with_gpu_runtime(runtime)).is_err());
    }
}
