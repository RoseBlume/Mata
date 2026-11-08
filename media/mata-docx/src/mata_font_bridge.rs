//! Adapters for Mata's own rasterizer. This is a *real dependency* on the
//! workspace's `mata-fonts`, not a reimplementation or a compatibility stub.
//!
//! The OpenType engine in this crate still performs shaping and layout. The
//! borrowed Mata font handles the actual glyph rasterization. Code constructing
//! a `Ttf` remains in the host application, which knows where font files live.
use std::io::{self, Error, ErrorKind};
use mata_fonts::decoders::{ttf::Ttf, Character};

/// A rasterized glyph with the original Unicode scalar and source UTF-8 range.
#[derive(Debug, Clone)]
pub struct RasterizedGlyph {
    pub scalar: char,
    pub byte_offset: usize,
    pub bitmap: Character,
}

/// Rasterize Unicode scalars with the host application's `mata-fonts` TTF.
///
/// This is deliberately scalar-level: shaping/GSUB must be performed *before*
/// outline rasterization, and this API should not be treated as a complete
/// implementation of joined Arabic or Indic clusters.
pub fn rasterize_text(font: &mut Ttf, text: &str, point_size: u16)
    -> io::Result<Vec<RasterizedGlyph>>
{
    if point_size == 0 { return Err(Error::new(ErrorKind::InvalidInput,"point size must be positive")); }
    let mut result = Vec::new();
    for (byte_offset, scalar) in text.char_indices() {
        let bitmap = font.character(point_size, scalar)
            .map_err(|e| Error::new(ErrorKind::InvalidData, format!("mata-fonts failed to rasterize {scalar:?}: {e}")))?;
        result.push(RasterizedGlyph { scalar, byte_offset, bitmap });
    }
    Ok(result)
}

/// Derive a horizontal advance from `mata-fonts` rendered glyph metrics.
/// The glyph metrics are in raster pixels at the requested point size; the
/// caller provides a point-to-pixel conversion appropriate to its DPI.
pub fn raster_advance_pt(glyph: &Character, dpi: f32) -> f32 {
    if !dpi.is_finite() || dpi <= 0.0 { return 0.0; }
    glyph.advance_width as f32 * 72.0 / dpi
}

/// Immutable representation of one typeface for font fallback selection.
/// The rasterizer is borrowed separately to avoid imposing a constructor,
/// ownership model, or memory layout on the `mata-fonts` crate.
#[derive(Clone, Debug)]
pub struct RegisteredFont {
    pub family: String,
    pub source: std::path::PathBuf,
    pub weight: u16,
    pub italic: bool,
}

/// One rasterized and positioned OpenType glyph. Positions are in points;
/// bitmap data and its format are owned by `mata-fonts`.
#[derive(Debug,Clone)]
pub struct PositionedBitmap {
    pub face_index:usize,
    pub glyph_id:u16,
    pub cluster:usize,
    pub x_pt:f32,
    pub y_pt:f32,
    pub bitmap:Character,
}

/// Render a shaped glyph sequence using a caller-supplied glyph-ID rasterizer.
///
/// `Ttf::character` takes Unicode scalars, not OpenType glyph IDs; supplying
/// glyph-ID bitmaps explicitly is essential for ligatures, contextual forms,
/// combining marks and variable fonts. The host adapter may use a newer
/// `mata-fonts` glyph-ID API without tying this crate to an undocumented
/// constructor or changing the document model.
///
/// `fetch(face_index,glyph_id)` should return the bitmap at the size used to
/// shape the run. This function applies GPOS advances and offsets once.
pub fn rasterize_shaped_with(
    run:&crate::opentype::ShapedRun,
    mut fetch:impl FnMut(usize,u16)->io::Result<Character>,
)->io::Result<Vec<PositionedBitmap>>{
    let mut pen=0.0f32;
    let mut result=Vec::with_capacity(run.glyphs.len());
    for glyph in &run.glyphs{
        if !glyph.advance_pt.is_finite()||!glyph.x_offset_pt.is_finite()||!glyph.y_offset_pt.is_finite(){
            return Err(Error::new(ErrorKind::InvalidInput,"non-finite glyph geometry"));
        }
        let bitmap=fetch(glyph.face_index,glyph.glyph_id)?;
        result.push(PositionedBitmap{
            face_index:glyph.face_index,glyph_id:glyph.glyph_id,cluster:glyph.cluster,
            x_pt:pen+glyph.x_offset_pt,y_pt:glyph.y_offset_pt,bitmap,
        });
        pen+=glyph.advance_pt;
    }
    Ok(result)
}
