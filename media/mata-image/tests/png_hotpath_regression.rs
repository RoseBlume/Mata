//! Native-slice optimization parity for single/multiple IDAT and PNG filters.
#![cfg(feature = "png")]

use mata_image::{DecodeBackend, DecodeOptions, Image, Png};
use std::io::Cursor;

const FIXTURES: &[(&str, &[u8])] = &[
    ("wide_stored.png", include_bytes!("fixtures_png_perf/wide_stored.png")),
    ("gray4_multi.png", include_bytes!("fixtures_png_perf/gray4_multi.png")),
    ("gray4_single.png", include_bytes!("fixtures_png_perf/gray4_single.png")),
    ("gray8_multi.png", include_bytes!("fixtures_png_perf/gray8_multi.png")),
    ("gray8_single.png", include_bytes!("fixtures_png_perf/gray8_single.png")),
    ("graya8_multi.png", include_bytes!("fixtures_png_perf/graya8_multi.png")),
    ("graya8_single.png", include_bytes!("fixtures_png_perf/graya8_single.png")),
    ("indexed4_multi.png", include_bytes!("fixtures_png_perf/indexed4_multi.png")),
    ("indexed4_single.png", include_bytes!("fixtures_png_perf/indexed4_single.png")),
    ("narrow_0.png", include_bytes!("fixtures_png_perf/narrow_0.png")),
    ("narrow_1.png", include_bytes!("fixtures_png_perf/narrow_1.png")),
    ("narrow_6.png", include_bytes!("fixtures_png_perf/narrow_6.png")),
    ("narrow_9.png", include_bytes!("fixtures_png_perf/narrow_9.png")),
    ("rgb16_multi.png", include_bytes!("fixtures_png_perf/rgb16_multi.png")),
    ("rgb16_single.png", include_bytes!("fixtures_png_perf/rgb16_single.png")),
    ("rgb8_multi.png", include_bytes!("fixtures_png_perf/rgb8_multi.png")),
    ("rgb8_single.png", include_bytes!("fixtures_png_perf/rgb8_single.png")),
    ("rgba16_multi.png", include_bytes!("fixtures_png_perf/rgba16_multi.png")),
    ("rgba16_single.png", include_bytes!("fixtures_png_perf/rgba16_single.png")),
    ("rgba8_multi.png", include_bytes!("fixtures_png_perf/rgba8_multi.png")),
    ("rgba8_single.png", include_bytes!("fixtures_png_perf/rgba8_single.png")),
];

#[test]
fn optimized_slice_stream_and_rich_decode_agree() {
    for &(name, bytes) in FIXTURES {
        let cpu = Image::from_bytes(bytes)
            .unwrap_or_else(|e| panic!("{name} native: {e}"));
        let stream = Image::from_reader(&mut Cursor::new(bytes))
            .unwrap_or_else(|e| panic!("{name} stream: {e}"));
        let rich = Png::new(bytes).unwrap_or_else(|e| panic!("{name} rich: {e}"));
        assert_eq!(cpu, stream, "{name}: native vs stream");
        assert_eq!(cpu.pixels, rich.decode_pixels().unwrap(), "{name}: native vs rich");
        let mut scratch = mata_image::DecodeScratch::new();
        let reused = Image::from_bytes_with_scratch(bytes, DecodeOptions::default(), &mut scratch)
            .unwrap_or_else(|e| panic!("{name} scratch: {e}"));
        assert_eq!(cpu, reused, "{name}: scratch");
    }
}

#[test]
fn lossless_png_8bit_cases_match_independent_image_reference() {
    for &(name, bytes) in FIXTURES {
        if name.starts_with("rgb16") || name.starts_with("rgba16") { continue; }
        let actual = Image::from_bytes(bytes).unwrap();
        let expected = image::load_from_memory(bytes).unwrap().to_rgba8();
        assert_eq!(expected.width(), actual.width, "{name}");
        assert_eq!(expected.height(), actual.height, "{name}");
        for (index, pixel) in actual.pixels.iter().enumerate() {
            assert_eq!(pixel.to_be_bytes(), expected.as_raw()[index*4..index*4+4],
                "{name} pixel {index}");
        }
    }
}

#[test]
fn cpu_preference_is_identical_to_ineligible_gpu_fallback() {
    for &(name, bytes) in FIXTURES {
        let cpu = Image::from_bytes(bytes).unwrap();
        for backend in [DecodeBackend::IntelGpu, DecodeBackend::AmdGpu, DecodeBackend::NvidiaGpu] {
            let image = Image::from_bytes_with_options(bytes,
                DecodeOptions { backend, ..DecodeOptions::default() }).unwrap();
            assert_eq!(image, cpu, "{name} {backend:?}");
        }
    }
}
