//! File-backed PNG and WebP reconstruction regressions. These fixtures are
//! generated specifically for these tests, so benches/assets is not required.
use mata_image::{DecodeBackend, DecodeOptions, Image};

fn vendors() -> [DecodeBackend; 3] {
    [DecodeBackend::IntelGpu, DecodeBackend::AmdGpu, DecodeBackend::NvidiaGpu]
}

fn decode(bytes: &[u8], backend: DecodeBackend) -> Image {
    Image::from_bytes_with_options(bytes,
        DecodeOptions { backend, ..DecodeOptions::default() }).unwrap()
}

#[test]
#[cfg(feature = "png")]
fn png_small_colour_modes_match_for_all_backend_preferences() {
    for bytes in [
        include_bytes!("fixtures_gpu/rgb_small.png").as_slice(),
        include_bytes!("fixtures_gpu/rgba_small.png").as_slice(),
        include_bytes!("fixtures_gpu/l_small.png").as_slice(),
        include_bytes!("fixtures_gpu/la_small.png").as_slice(),
    ] {
        let cpu = decode(bytes, DecodeBackend::Cpu);
        for backend in vendors() { assert_eq!(decode(bytes, backend), cpu); }
    }
}

#[test]
#[cfg(feature = "webp")]
fn webp_small_lossy_and_lossless_match_for_all_backend_preferences() {
    for bytes in [
        include_bytes!("fixtures_gpu/vp8_small.webp").as_slice(),
        include_bytes!("fixtures_gpu/vp8l_small.webp").as_slice(),
    ] {
        let cpu = decode(bytes, DecodeBackend::Cpu);
        for backend in vendors() { assert_eq!(decode(bytes, backend), cpu); }
    }
}

#[test]
#[cfg(feature = "png")]
#[ignore = "large images: run explicitly to validate hardware OpenCL and CPU fallback"]
fn large_png_gpu_reconstruction_matches_cpu() {
    for bytes in [
        include_bytes!("fixtures_gpu/rgb_large.png").as_slice(),
        include_bytes!("fixtures_gpu/rgba_large.png").as_slice(),
    ] {
        let cpu = decode(bytes, DecodeBackend::Cpu);
        for backend in vendors() {
            let before = backend.png_dispatches();
            let actual = decode(bytes, backend);
            assert_eq!(actual, cpu, "{backend:?}");
            if backend.supports_png_unpack() {
                assert!(backend.png_dispatches() > before,
                    "{backend:?}: PNG GPU reconstruction never executed");
            }
        }
    }
}

#[test]
#[cfg(feature = "webp")]
#[ignore = "large images: run explicitly on an OpenCL-equipped machine"]
fn large_webp_gpu_reconstruction_matches_cpu() {
    for bytes in [
        include_bytes!("fixtures_gpu/vp8_large.webp").as_slice(),
        include_bytes!("fixtures_gpu/vp8l_large.webp").as_slice(),
    ] {
        let cpu = decode(bytes, DecodeBackend::Cpu);
        for backend in vendors() {
            let before = backend.webp_dispatches();
            let actual = decode(bytes, backend);
            assert_eq!(actual, cpu, "{backend:?}");
            if backend.supports_webp_reconstruction() &&
                bytes == include_bytes!("fixtures_gpu/vp8_large.webp").as_slice() {
                assert!(backend.webp_dispatches() > before,
                    "{backend:?}: VP8 GPU reconstruction never executed");
            }
            // Lossless WebP uses a subtract-green transform only when the
            // encoder selects it; GPU dispatch there is bitstream-dependent.
        }
    }
}
