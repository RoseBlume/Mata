//! Check that in-memory, rich and streaming PNG entry points agree when CPU
//! or a GPU backend is requested. These catch mismatched decoder arguments.

#[cfg(feature = "png")]
use mata_image::{DecodeBackend, DecodeOptions, Image, Png};

#[cfg(feature = "png")]
fn options(backend: DecodeBackend) -> DecodeOptions {
    DecodeOptions { backend, ..DecodeOptions::default() }
}

#[cfg(feature = "png")]
#[test]
fn png_reader_and_rich_entry_points_match_cpu() {
    for bytes in [
        include_bytes!("fixtures_gpu/rgb_small.png").as_slice(),
        include_bytes!("fixtures_gpu/rgba_small.png").as_slice(),
        include_bytes!("fixtures_gpu/l_small.png").as_slice(),
        include_bytes!("fixtures_gpu/la_small.png").as_slice(),
    ] {
        let expected = Image::from_bytes_with_options(bytes, options(DecodeBackend::Cpu)).unwrap();
        let rich = Png::new(bytes).unwrap();
        assert_eq!(rich.decode_pixels().unwrap(), expected.pixels);
        for backend in [DecodeBackend::Cpu, DecodeBackend::IntelGpu,
                        DecodeBackend::AmdGpu, DecodeBackend::NvidiaGpu] {
            let direct = Image::from_bytes_with_options(bytes, options(backend)).unwrap();
            let mut stream = std::io::Cursor::new(bytes);
            let streamed = Image::from_reader_with_options(&mut stream, options(backend)).unwrap();
            assert_eq!(direct, expected, "byte slice {backend:?}");
            assert_eq!(streamed, expected, "streaming {backend:?}");
        }
    }
}

#[cfg(feature = "png")]
#[test]
#[ignore = "large-image GPU dispatch check; run with --ignored on an OpenCL GPU"]
fn large_png_streaming_can_use_gpu_and_matches_cpu() {
    for bytes in [
        include_bytes!("fixtures_gpu/rgb_large.png").as_slice(),
        include_bytes!("fixtures_gpu/rgba_large.png").as_slice(),
    ] {
        let expected = Image::from_bytes_with_options(bytes, options(DecodeBackend::Cpu)).unwrap();
        for backend in [DecodeBackend::IntelGpu, DecodeBackend::AmdGpu, DecodeBackend::NvidiaGpu] {
            let before = backend.png_dispatches();
            let mut stream = std::io::Cursor::new(bytes);
            let actual = Image::from_reader_with_options(&mut stream, options(backend)).unwrap();
            assert_eq!(actual, expected, "streaming {backend:?}");
            if backend.supports_png_unpack() {
                assert!(backend.png_dispatches() > before,
                    "{backend:?}: GPU was available, but the streaming PNG path did not dispatch");
            }
        }
    }
}
