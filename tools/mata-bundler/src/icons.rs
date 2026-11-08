use std::{fs, path::Path};
use crate::{archive, png_decode};
use crate::{bail, tools, Result};

const DESKTOP_SIZES: &[u32] = &[16, 24, 32, 48, 64, 128, 256, 512];
const ICO_SIZES: &[u32] = &[16, 24, 32, 48, 64, 128, 256];
const ANDROID_SIZES: &[(&str, u32)] = &[
    ("mipmap-mdpi", 48),
    ("mipmap-hdpi", 72),
    ("mipmap-xhdpi", 96),
    ("mipmap-xxhdpi", 144),
    ("mipmap-xxxhdpi", 192),
];

pub fn generate(root: &Path, source: &Path) -> Result<()> {
    let source_path = if source.is_absolute() { source.to_path_buf() } else { root.join(source) };
    if !source_path.is_file() {
        return bail(format!("icon source not found: {}", source_path.display()));
    }
    let (width,height,pixels) = png_decode::read_png(&fs::read(&source_path)?)
        .map_err(|e| format!("failed to decode {}: {e}", source_path.display()))?;
    let icons_dir = root.join("icons");
    let android_dir = icons_dir.join("android");
    tools::ensure_dir(&icons_dir)?;
    tools::ensure_dir(&android_dir)?;

    for &size in DESKTOP_SIZES {
        let resized = resize_rgba(&pixels, width, height, size as usize, size as usize);
        let data = encode_png(size, size, &resized)?;
        let path = icons_dir.join(format!("icon-{size}x{size}.png"));
        fs::write(&path, data)?;
    }

    let mut ico_images = Vec::new();
    for &size in ICO_SIZES {
        let resized = resize_rgba(&pixels, width, height, size as usize, size as usize);
        ico_images.push((size, encode_png(size, size, &resized)?));
    }
    let ico = encode_ico(&ico_images)?;
    fs::write(icons_dir.join("icon.ico"), ico)?;

    for &(dir, size) in ANDROID_SIZES {
        let target_dir = android_dir.join(dir);
        tools::ensure_dir(&target_dir)?;
        let resized = resize_rgba(&pixels, width, height, size as usize, size as usize);
        let data = encode_png(size, size, &resized)?;
        fs::write(target_dir.join("ic_launcher.png"), &data)?;
        fs::write(target_dir.join("ic_launcher_round.png"), data)?;
    }

    println!("Generated desktop icons in {}", icons_dir.display());
    println!("Generated Android icons in {}", android_dir.display());
    Ok(())
}

fn resize_rgba(src: &[u32], src_w: usize, src_h: usize, dst_w: usize, dst_h: usize) -> Vec<u32> {
    if src_w == dst_w && src_h == dst_h { return src.to_vec(); }
    let mut out = vec![0u32; dst_w.saturating_mul(dst_h)];
    let scale_x = src_w as f32 / dst_w as f32;
    let scale_y = src_h as f32 / dst_h as f32;
    for y in 0..dst_h {
        let sy = ((y as f32 + 0.5) * scale_y - 0.5).clamp(0.0, (src_h - 1) as f32);
        let y0 = sy.floor() as usize;
        let y1 = (y0 + 1).min(src_h - 1);
        let fy = sy - y0 as f32;
        for x in 0..dst_w {
            let sx = ((x as f32 + 0.5) * scale_x - 0.5).clamp(0.0, (src_w - 1) as f32);
            let x0 = sx.floor() as usize;
            let x1 = (x0 + 1).min(src_w - 1);
            let fx = sx - x0 as f32;
            let c00 = src[y0 * src_w + x0];
            let c10 = src[y0 * src_w + x1];
            let c01 = src[y1 * src_w + x0];
            let c11 = src[y1 * src_w + x1];
            out[y * dst_w + x] = bilinear(c00, c10, c01, c11, fx, fy);
        }
    }
    out
}

fn bilinear(c00: u32, c10: u32, c01: u32, c11: u32, fx: f32, fy: f32) -> u32 {
    let (r00, g00, b00, a00) = unpack_rgba(c00);
    let (r10, g10, b10, a10) = unpack_rgba(c10);
    let (r01, g01, b01, a01) = unpack_rgba(c01);
    let (r11, g11, b11, a11) = unpack_rgba(c11);
    let r = interp(interp(r00, r10, fx), interp(r01, r11, fx), fy);
    let g = interp(interp(g00, g10, fx), interp(g01, g11, fx), fy);
    let b = interp(interp(b00, b10, fx), interp(b01, b11, fx), fy);
    let a = interp(interp(a00, a10, fx), interp(a01, a11, fx), fy);
    pack_rgba(r, g, b, a)
}

#[inline]
fn interp(a: u8, b: u8, t: f32) -> u8 {
    ((a as f32) + (b as f32 - a as f32) * t).round().clamp(0.0, 255.0) as u8
}

#[inline]
fn unpack_rgba(pixel: u32) -> (u8, u8, u8, u8) {
    (((pixel >> 24) & 0xff) as u8, ((pixel >> 16) & 0xff) as u8, ((pixel >> 8) & 0xff) as u8, (pixel & 0xff) as u8)
}

#[inline]
fn pack_rgba(r: u8, g: u8, b: u8, a: u8) -> u32 {
    ((r as u32) << 24) | ((g as u32) << 16) | ((b as u32) << 8) | (a as u32)
}

fn encode_png(width: u32, height: u32, pixels: &[u32]) -> Result<Vec<u8>> {
    let expected = width as usize * height as usize;
    if pixels.len() != expected {
        return bail(format!("PNG pixel count mismatch: expected {expected}, got {}", pixels.len()));
    }
    let mut raw = Vec::with_capacity((width as usize * 4 + 1) * height as usize);
    for row in pixels.chunks_exact(width as usize) {
        raw.push(0); // filter type 0
        for &px in row {
            let (r, g, b, a) = unpack_rgba(px);
            raw.extend_from_slice(&[r, g, b, a]);
        }
    }
    let idat = archive::zlib_stored(&raw);

    let mut out = Vec::new();
    out.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);

    let mut ihdr = [0u8; 13];
    ihdr[0..4].copy_from_slice(&width.to_be_bytes());
    ihdr[4..8].copy_from_slice(&height.to_be_bytes());
    ihdr[8] = 8; // bit depth
    ihdr[9] = 6; // RGBA
    ihdr[10] = 0;
    ihdr[11] = 0;
    ihdr[12] = 0;
    write_png_chunk(&mut out, *b"IHDR", &ihdr);
    write_png_chunk(&mut out, *b"IDAT", &idat);
    write_png_chunk(&mut out, *b"IEND", &[]);
    Ok(out)
}

fn write_png_chunk(out: &mut Vec<u8>, kind: [u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(&kind);
    out.extend_from_slice(data);
    let mut crc_data = Vec::with_capacity(4 + data.len());
    crc_data.extend_from_slice(&kind);
    crc_data.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_data).to_be_bytes());
}

fn encode_ico(images: &[(u32, Vec<u8>)]) -> Result<Vec<u8>> {
    if images.is_empty() { return bail("ICO requires at least one image"); }
    let count = images.len() as u16;
    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    let directory_size = 6 + images.len() * 16;
    let mut offset = directory_size as u32;
    for (size, data) in images {
        let size_byte = if *size >= 256 { 0 } else { *size as u8 };
        out.push(size_byte);
        out.push(size_byte);
        out.push(0); // colors
        out.push(0);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset = offset.saturating_add(data.len() as u32);
    }
    for (_, data) in images {
        out.extend_from_slice(data);
    }
    Ok(out)
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in bytes {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg() & 0xedb8_8320;
            crc = (crc >> 1) ^ mask;
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_encoder_writes_signature() {
        let pixels = vec![0xff00_00ff; 4];
        let png = encode_png(2, 2, &pixels).unwrap();
        assert_eq!(&png[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
        let (width, height, roundtrip) = crate::png_decode::read_png(&png).unwrap();
        assert_eq!((width,height), (2,2));
        assert_eq!(roundtrip,pixels);
    }

    #[test]
    fn ico_encoder_writes_header() {
        let img = encode_png(16, 16, &vec![0u32; 16 * 16]).unwrap();
        let ico = encode_ico(&[(16, img)]).unwrap();
        assert_eq!(&ico[..4], &[0, 0, 1, 0]);
    }

    #[test]
    fn resize_outputs_requested_dimensions() {
        let pixels = vec![0xffff_ffff; 4];
        let resized = resize_rgba(&pixels, 2, 2, 3, 5);
        assert_eq!(resized.len(), 15);
    }
}
