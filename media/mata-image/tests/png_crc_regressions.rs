//! PNG CRC-32 regression tests, including the lightweight, rich, and streaming APIs.
#![cfg(feature = "png")]

use mata_image::{DecodeOptions, Image, Png};
use std::io::Cursor;
use std::path::Path;

const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

fn reference_crc32(input: &[u8]) -> u32 {
    let mut state = 0xffff_ffffu32;
    for &byte in input {
        state ^= u32::from(byte);
        for _ in 0..8 {
            state = if state & 1 == 1 { (state >> 1) ^ 0xEDB8_8320 } else { state >> 1 };
        }
    }
    !state
}

// Return the offset of the stored CRC of the first matching chunk.
fn crc_offset(bytes: &[u8], requested: &[u8; 4]) -> Option<usize> {
    if bytes.get(..8) != Some(SIGNATURE.as_slice()) {
        return None;
    }
    let mut pos = 8usize;
    while pos + 12 <= bytes.len() {
        let length = u32::from_be_bytes(bytes[pos..pos + 4].try_into().ok()?) as usize;
        let kind = bytes.get(pos + 4..pos + 8)?;
        let stored_crc_at = pos.checked_add(8)?.checked_add(length)?;
        if stored_crc_at.checked_add(4)? > bytes.len() {
            return None;
        }
        if kind == requested {
            return Some(stored_crc_at);
        }
        pos = stored_crc_at + 4;
        if kind == b"IEND" {
            break;
        }
    }
    None
}

#[test]
fn fixture_chunks_all_have_valid_standard_crc() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("benches/assets");
    let mut file_count = 0;
    let mut chunk_count = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|s| s.to_str()) != Some("png") {
            continue;
        }
        file_count += 1;
        let png = std::fs::read(&path).unwrap();
        assert_eq!(png.get(..8), Some(SIGNATURE.as_slice()));
        let mut pos = 8usize;
        while pos < png.len() {
            let size = u32::from_be_bytes(png[pos..pos + 4].try_into().unwrap()) as usize;
            let end = pos + 8 + size;
            let stored = u32::from_be_bytes(png[end..end + 4].try_into().unwrap());
            assert_eq!(reference_crc32(&png[pos + 4..end]), stored,
                "{} at byte {pos}", path.display());
            chunk_count += 1;
            let is_end = &png[pos + 4..pos + 8] == b"IEND";
            pos = end + 4;
            if is_end { break; }
        }
    }
    assert!(file_count > 0 && chunk_count > file_count);
}

#[test]
fn valid_png_fixtures_decode_with_crc_checks_enabled() {
    for name in [
        "tiny_rgba.png", "png_rgba.png", "png_rgb.png", "png_gray.png",
        "png_indexed_trns.png", "png_many_idat.png", "png_filter_4.png",
    ] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("benches/assets").join(name);
        let bytes = std::fs::read(path).unwrap();
        let from_slice = Image::from_bytes(&bytes)
            .unwrap_or_else(|e| panic!("valid PNG {name} incorrectly rejected: {e}"));
        let from_stream = Image::from_reader(&mut Cursor::new(&bytes))
            .unwrap_or_else(|e| panic!("valid streaming PNG {name} incorrectly rejected: {e}"));
        assert_eq!(from_slice.pixels, from_stream.pixels, "{name}");
        assert_eq!((from_slice.width, from_slice.height), (from_stream.width, from_stream.height));
        Png::new(&bytes).unwrap_or_else(|e| panic!("valid rich PNG {name} incorrectly rejected: {e}"));
    }
}

#[test]
fn corrupt_chunk_crc_is_rejected_but_validation_can_be_disabled() {
    let valid = include_bytes!("../benches/assets/png_rgba.png");
    for chunk_kind in [b"IHDR", b"IDAT", b"IEND"] {
        let mut modified = valid.to_vec();
        let offset = crc_offset(&modified, chunk_kind).expect("required chunk is missing");
        modified[offset] ^= 0x80;
        assert!(Image::from_bytes(&modified).is_err(), "slice: {chunk_kind:?}");
        assert!(Image::from_reader(&mut Cursor::new(&modified)).is_err(), "stream: {chunk_kind:?}");
        assert!(Png::new(&modified).is_err(), "rich parser: {chunk_kind:?}");

        let options = DecodeOptions { verify_checksums: false, ..DecodeOptions::default() };
        Image::from_bytes_with_options(&modified, options)
            .unwrap_or_else(|e| panic!("valid data with disabled checksum: {chunk_kind:?}: {e}"));
        Image::from_reader_with_options(&mut Cursor::new(&modified), options)
            .unwrap_or_else(|e| panic!("valid stream with disabled checksum: {chunk_kind:?}: {e}"));
    }
}
