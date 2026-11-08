//! System-font discovery and cached TrueType drawing.
//!
//! `mata_fonts` owns glyph decoding/rasterization. This module handles only
//! locating a usable font, measuring text, caching glyphs, and compositing
//! their alpha masks into the `mata_win` framebuffer (0xAARRGGBB).
use mata_fonts::decoders::{ttf::Ttf, Character};
use mata_win::Framebuffer;
use std::{cell::RefCell, collections::HashMap, fs, path::{Path, PathBuf}, process::Command};

const MAX_GLYPHS: usize = 768;

struct Painter {
    face: Ttf,
    glyphs: HashMap<(u16, char), Character>,
}

thread_local! {
    // All application text is drawn on the window's event/render thread.
    static FONT: RefCell<Option<Painter>> = const { RefCell::new(None) };
}

/// Pick a system UI font, or the font given by --font / MATA_MOLECULE_FONT.
/// A failed lookup leaves the legacy 5x7 fallback in operation.
pub fn initialize(requested: Option<&Path>) -> String {
    let mut last_error = String::new();
    for path in candidates(requested) {
        if !path.is_file() { continue; }
        match fs::read(&path) {
            Ok(bytes) => match Ttf::from_vec(bytes) {
                Ok(face) => {
                    FONT.with(|slot| *slot.borrow_mut() = Some(Painter {
                        face,
                        glyphs: HashMap::with_capacity(128),
                    }));
                    return format!("mata_fonts: {}", path.display());
                }
                Err(e) => last_error = format!("{}: {e}", path.display()),
            },
            Err(e) => last_error = format!("{}: {e}", path.display()),
        }
    }
    FONT.with(|slot| *slot.borrow_mut() = None);
    if last_error.is_empty() {
        "bitmap fallback (no usable system TTF found)".to_owned()
    } else {
        format!("bitmap fallback (no usable system TTF; last error: {last_error})")
    }
}

// Font sizes are point sizes (mata_fonts assumes 96dpi), chosen to keep the
// original layout's 7/14/21-pixel-ish line spacing for scales 1/2/3.
fn points(scale: i32) -> u16 {
    match scale.clamp(1, 5) {1 => 7, 2 => 11, 3 => 17, 4 => 23, _ => 29}
}

impl Painter {
    fn glyph(&mut self, pt: u16, ch: char) -> Option<&Character> {
        let key = (pt, ch);
        if !self.glyphs.contains_key(&key) {
            let raster = self.face.character(pt, ch).or_else(|_| self.face.character(pt, '?')).ok()?;
            if self.glyphs.len() >= MAX_GLYPHS { self.glyphs.clear(); }
            self.glyphs.insert(key, raster);
        }
        self.glyphs.get(&key)
    }

    fn width(&mut self, s: &str, pt: u16) -> i32 {
        s.chars()
            .map(|ch| self.glyph(pt, ch).map_or(0, |g| g.advance_width as i32))
            .sum()
    }

    fn paint(&mut self, fb: &mut Framebuffer, x: i32, y: i32, s: &str,
             pt: u16, color: u32, clip_width: Option<i32>) {
        if fb.width == 0 || fb.height == 0 {return;}
        let clip_right = clip_width
            .map(|width| x.saturating_add(width.max(0)))
            .unwrap_or(i32::MAX)
            .min(fb.width as i32);
        let left = x.max(0);
        let mut pen = x;
        for ch in s.chars() {
            if pen >= clip_right {break;}
            let Some(g) = self.glyph(pt, ch) else {continue;};
            let gx = pen.saturating_add(g.bearing_x);
            let gy = y.saturating_add(g.baseline as i32).saturating_sub(g.bearing_y);
            alpha_glyph(fb, gx, gy, g, color, left, clip_right);
            pen = pen.saturating_add(g.advance_width as i32);
        }
    }
}

/// Returns false when no TrueType face could be loaded.
pub fn text(fb: &mut Framebuffer, x: i32, y: i32, s: &str, scale: i32, color: u32,
            clip_width: Option<i32>) -> bool {
    FONT.with(|cell| {
        let mut state = cell.borrow_mut();
        if let Some(painter) = state.as_mut() {
            painter.paint(fb, x, y, s, points(scale), color, clip_width);
            true
        } else {false}
    })
}

pub fn width(s: &str, scale: i32) -> Option<i32> {
    FONT.with(|cell| cell.borrow_mut().as_mut().map(|p| p.width(s, points(scale))))
}

/// Keep only a tail which fits a given pixel width, for editable fields.
pub fn tail(s: &str, scale: i32, max_width: i32) -> String {
    FONT.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(painter) = state.as_mut() else {
            let n = (max_width / (6 * scale.clamp(1, 5))).max(1) as usize;
            return s.chars().rev().take(n).collect::<Vec<_>>().into_iter().rev().collect();
        };
        let mut width = 0;
        let mut reversed = Vec::new();
        for ch in s.chars().rev() {
            let adv = painter.glyph(points(scale), ch).map_or(0, |g| g.advance_width as i32);
            if width + adv > max_width.max(0) {break;}
            width += adv;
            reversed.push(ch);
        }
        reversed.into_iter().rev().collect()
    })
}

fn alpha_glyph(fb: &mut Framebuffer, x: i32, y: i32, glyph: &Character,
               color: u32, clip_left: i32, clip_right: i32) {
    let left = x.max(clip_left);
    let right = x.saturating_add(glyph.width as i32).min(clip_right);
    let top = y.max(0);
    let bottom = y.saturating_add(glyph.height as i32).min(fb.height as i32);
    if right <= left || bottom <= top {return;}
    let (fr, fg, fb_color) = ((color >> 16) & 255, (color >> 8) & 255, color & 255);
    let opacity = (color >> 24) & 255;
    for py in top..bottom {
        let src_base = (py - y) as usize * glyph.width as usize;
        let dst_base = py as usize * fb.width as usize;
        for px in left..right {
            // mata_fonts stores mask opacity in the low byte of 0xRRGGBBAA.
            let coverage = glyph.pixels[src_base + (px - x) as usize] & 255;
            let alpha = (coverage * opacity + 127) / 255;
            if alpha == 0 {continue;}
            let index = dst_base + px as usize;
            if alpha == 255 {
                fb.pixels[index] = 0xff00_0000 | (color & 0x00ff_ffff);
                continue;
            }
            let dst = fb.pixels[index];
            let inv = 255 - alpha;
            let blend = |v: u32, bg: u32| -> u32 { (v * alpha + bg * inv + 127) / 255 };
            fb.pixels[index] = 0xff00_0000
                | (blend(fr, (dst >> 16) & 255) << 16)
                | (blend(fg, (dst >> 8) & 255) << 8)
                | blend(fb_color, dst & 255);
        }
    }
}

fn candidates(requested: Option<&Path>) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(path) = requested {paths.push(path.to_path_buf());}
    if let Some(path) = std::env::var_os("MATA_MOLECULE_FONT") {paths.push(path.into());}

    #[cfg(target_os = "linux")]
    {
        // Prefer the current desktop's UI family if available. Fontconfig
        // resolves family aliases and the user's installed font configuration.
        if let Some(family) = linux_desktop_family() {
            if let Some(path) = fc_match(&family) {paths.push(path);}
        }
        for family in ["sans-serif", "system-ui", "DejaVu Sans", "Liberation Sans", "Noto Sans"] {
            if let Some(path) = fc_match(family) {paths.push(path);}
        }
        paths.extend([
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
        ].into_iter().map(PathBuf::from));
    }
    #[cfg(target_os = "windows")]
    {
        let fonts = std::env::var_os("WINDIR").map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows")).join("Fonts");
        if let Some(family) = windows_ui_family() {
            if let Some(path) = windows_font_file(&family, &fonts) {paths.push(path);}
        }
        for name in ["segoeui.ttf", "SegUIVar.ttf", "tahoma.ttf", "arial.ttf"] {
            paths.push(fonts.join(name));
        }
    }
    #[cfg(target_os = "macos")]
    {
        paths.extend([
            "/System/Library/Fonts/Supplemental/Arial.ttf",
            "/System/Library/Fonts/Supplemental/Verdana.ttf",
            "/Library/Fonts/Arial.ttf",
        ].into_iter().map(PathBuf::from));
    }
    #[cfg(target_os = "android")]
    {
        paths.extend([
            "/system/fonts/Roboto-Regular.ttf",
            "/system/fonts/RobotoStatic-Regular.ttf",
            "/system/fonts/NotoSans-Regular.ttf",
        ].into_iter().map(PathBuf::from));
    }
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        for relative in [".fonts/DejaVuSans.ttf", ".local/share/fonts/DejaVuSans.ttf"] {
            paths.push(home.join(relative));
        }
    }
    paths.dedup();
    paths
}

#[cfg(target_os = "linux")]
fn fc_match(family: &str) -> Option<PathBuf> {
    let output = Command::new("fc-match")
        .args(["-f", "%{file}\n", family]).output().ok()?;
    if !output.status.success() {return None;}
    let name = String::from_utf8_lossy(&output.stdout);
    let file = name.lines().next()?.trim();
    if file.is_empty() {None} else {Some(PathBuf::from(file))}
}

#[cfg(target_os = "linux")]
fn linux_desktop_family() -> Option<String> {
    if let Ok(output) = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "font-name"]).output() {
        if output.status.success() {
            let decoded = String::from_utf8_lossy(&output.stdout);
            let text = decoded.trim().trim_matches('\'').trim_matches('"');
            let family = text.rsplit_once(' ').and_then(|(name, size)|
                size.parse::<f32>().ok().map(|_| name)).unwrap_or(text).trim();
            if !family.is_empty() {return Some(family.to_owned());}
        }
    }
    let home = std::env::var_os("HOME")?;
    let config = fs::read_to_string(PathBuf::from(home).join(".config/kdeglobals")).ok()?;
    let mut in_general = false;
    for line in config.lines() {
        let line = line.trim();
        if line.starts_with('[') {in_general = line == "[General]";}
        if in_general {
            if let Some(font) = line.strip_prefix("font=") {
                let name = font.split(',').next()?.trim();
                if !name.is_empty() {return Some(name.to_owned());}
            }
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn windows_ui_family() -> Option<String> {
    // MessageFont in WindowMetrics is a LOGFONTW structure; lfFaceName begins
    // at byte 28. This works without adding another Windows-binding crate.
    let output = Command::new("reg")
        .args(["query", r"HKCU\Control Panel\Desktop\WindowMetrics", "/v", "MessageFont"])
        .output().ok()?;
    if !output.status.success() {return None;}
    let data = String::from_utf8_lossy(&output.stdout);
    let line = data.lines().find(|s| s.contains("REG_BINARY"))?;
    let hex = line.split("REG_BINARY").nth(1)?.trim();
    let bytes = hex.as_bytes().chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok())
        .collect::<Option<Vec<u8>>>()?;
    let mut name = Vec::new();
    for pair in bytes.get(28..)?.chunks_exact(2).take(32) {
        let code = u16::from_le_bytes([pair[0], pair[1]]);
        if code == 0 {break;}
        name.push(code);
    }
    let face = String::from_utf16(&name).ok()?;
    if face.is_empty() {None} else {Some(face)}
}

#[cfg(target_os = "windows")]
fn windows_font_file(family: &str, folder: &Path) -> Option<PathBuf> {
    let output = Command::new("reg")
        .args(["query", r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts"])
        .output().ok()?;
    if !output.status.success() {return None;}
    let family = family.to_lowercase();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Some((name, value)) = line.split_once("REG_SZ") else {continue;};
        let name = name.trim().to_lowercase();
        if !name.starts_with(&family) || !name.contains("truetype")
            || name.contains("bold") || name.contains("italic") {continue;}
        let filename = PathBuf::from(value.trim());
        let path = if filename.is_absolute() {filename} else {folder.join(filename)};
        if path.is_file() {return Some(path);}
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn absent_font_fails_over() {
        // Explicitly test an absent override without depending on host fonts.
        assert!(!Path::new("/an/unlikely/to/exist/mata-font.ttf").is_file());
    }
    #[test]
    fn alpha_blends_without_overwriting_the_background() {
        let mut fb = Framebuffer::from_color(3, 3, 0xff00_0000);
        let g = Character {
            width: 1, height: 1, pixels: vec![0xffff_ff80],
            advance_width: 1, bearing_x: 0, bearing_y: 1,
            line_height: 1, baseline: 1,
        };
        alpha_glyph(&mut fb, 1, 1, &g, 0xffff_0000, 0, 3);
        assert_eq!(fb.pixels[0], 0xff00_0000);
        assert_eq!(fb.pixels[4], 0xff80_0000);
    }
    #[test]
    fn glyph_clipping_respects_right_edge() {
        let mut fb = Framebuffer::from_color(3, 3, 0xff00_0000);
        let g = Character {
            width: 2, height: 1, pixels: vec![0xffff_ffff; 2],
            advance_width: 2, bearing_x: 0, bearing_y: 1,
            line_height: 1, baseline: 1,
        };
        alpha_glyph(&mut fb, 1, 1, &g, 0xff12_3456, 0, 2);
        assert_eq!(fb.pixels[4], 0xff12_3456);
        assert_eq!(fb.pixels[5], 0xff00_0000);
    }
    #[test]
    fn point_mapping() {
        assert_eq!(points(1), 7);
        assert_eq!(points(2), 11);
        assert_eq!(points(3), 17);
    }
}
