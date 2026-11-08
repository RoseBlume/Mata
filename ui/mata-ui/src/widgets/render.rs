//! Software rendering helpers for the built-in controls.

use crate::Framebuffer;
use super::{
    controls::{ ButtonState, TextFieldState },
    FontRenderer,
    geometry::Rect,
    style::ResolvedBorderRadius,
};

/// Constant used for bg.
///
/// Visibility: **crate-visible**.
pub(crate) const BG: u32 = 0xff1f2329;
/// Constant used for control.
///
/// Visibility: **private**.
const CONTROL: u32 = 0xff343942;
const HOVER: u32 = 0xff434a55;
const PRESSED: u32 = 0xff272c33;
const BORDER: u32 = 0xff6b7480;
const FOCUS_BORDER: u32 = 0xff7fb7ff;
const TEXT: u32 = 0xfff2f4f7;
const DISABLED_TEXT: u32 = 0xff9ba2aa;
/// Constant used for font pt.
///
/// Visibility: **crate-visible**.
#[allow(dead_code)] // Retained for optional codecs, benchmarks, tests, or future integration.
pub(crate) const FONT_PT: u16 = 12;

/// Draws button.
///
/// Visibility: **crate-visible**.
#[doc = "Draws button. Visibility: crate-visible."]
pub(crate) fn draw_button(fb: &mut Framebuffer, s: &ButtonState) {
    let opacity = s.computed.opacity;
    let mut fill = s.computed.background_color.unwrap_or(
        if !s.enabled {
            0xff2a2e34
        } else if s.pressed {
            PRESSED
        } else if s.hovered {
            HOVER
        } else {
            CONTROL
        }
    );
    // Hover and press feedback still works when semantic themes supply an
    // explicit background, instead of becoming visually inert.
    if s.computed.background_color.is_some() && s.enabled {
        if s.pressed {fill=shade(fill,-22);} else if s.hovered {fill=shade(fill,12);}
    }
    let border = s.computed.border_color.unwrap_or(BORDER);
    paint_box(
        fb,
        s.bounds,
        s.computed.border_radius,
        with_opacity(border, opacity),
        with_opacity(fill, opacity)
    );
    // Text and images are rendered by ContentPainter using the caller-owned
    // widget's own font family, font size, and image data.
}

fn shade(color:u32,delta:i32)->u32 {
    let ch=|shift:u32| (((color>>shift)&255) as i32+delta).clamp(0,255) as u32;
    (color&0xff00_0000)|(ch(16)<<16)|(ch(8)<<8)|ch(0)
}

/// Draws text field.
///
/// Visibility: **crate-visible**.
#[doc = "Draws text field. Visibility: crate-visible."]
pub(crate) fn draw_text_field(
    fb: &mut Framebuffer,
    font: Option<&mut FontRenderer>,
    s: &TextFieldState,
    focused: bool
) {
    #[cfg(not(feature = "fonts"))]
    let _ = font;
    let opacity = s.computed.opacity;
    let fill = s.computed.background_color.unwrap_or(
        if s.enabled {
            0xff181b20
        } else {
            0xff26292e
        }
    );
    let border = s.computed.border_color.unwrap_or(if focused { FOCUS_BORDER } else { BORDER });
    paint_box(
        fb,
        s.bounds,
        s.computed.border_radius,
        with_opacity(border, opacity),
        with_opacity(fill, opacity)
    );
    #[cfg(feature = "fonts")]
    let mut inner = s.computed.content_bounds;
    #[cfg(feature = "fonts")]
    if inner == s.bounds {
        inner = Rect::new(
            s.bounds.x + 6,
            s.bounds.y + 3,
            s.bounds.width.saturating_sub(12),
            s.bounds.height.saturating_sub(6)
        );
    }
    #[cfg(feature = "fonts")]
    if let Some(font) = font {
        let display=if s.preedit.is_empty() {s.text.clone()} else {
            let mut value=s.text.clone();
            if value.is_char_boundary(s.cursor.min(value.len())) {
                value.insert_str(s.cursor.min(value.len()),&s.preedit);
            }
            value
        };
        let color = with_opacity(
            s.computed.foreground_color.unwrap_or(if s.enabled { TEXT } else { DISABLED_TEXT }),
            opacity
        );
        let pt = font_pt(s.computed.font_size_px);
        let line = Some(s.computed.line_height_px.round().max(1.0) as u32);
        let (_, th) = font
            .measure_styled(
                &display,
                pt,
                s.computed.font.variant,
                s.computed.font.weight,
                s.computed.font.stretch,
                line
            )
            .unwrap_or((0, 0));
        let y = inner.y + ((inner.height.saturating_sub(th) / 2) as i32);
        let cursor = s.cursor.min(s.text.len());
        let prefix = &s.text[..cursor];
        let pw = font
            .measure_styled(
                prefix,
                pt,
                s.computed.font.variant,
                s.computed.font.weight,
                s.computed.font.stretch,
                line
            )
            .map(|v| v.0)
            .unwrap_or(0);
        let tw = font
            .measure_styled(
                &display,
                pt,
                s.computed.font.variant,
                s.computed.font.weight,
                s.computed.font.stretch,
                line
            )
            .map(|v| v.0)
            .unwrap_or(0);
        let max = inner.width.saturating_sub(2);
        let scroll = if pw > max { pw - max } else if tw > max && pw == tw { tw - max } else { 0 };
        let x = inner.x - (scroll as i32);
        if let Some((a,b))=s.selection {
            if a<=b && s.text.get(..a).is_some() && s.text.get(..b).is_some(){
                let style=&s.computed.font;
                let mut width=|part:&str|font.measure_styled(part,pt,style.variant,style.weight,style.stretch,line)
                    .map(|v|v.0).unwrap_or(0);
                let left=width(&s.text[..a]) as i32;
                let right=width(&s.text[..b]) as i32;
                let left=(x+left).max(inner.x);
                let right=(x+right).min(inner.x+inner.width as i32);
                if right>left {
                    for yy in inner.y.max(0)..(inner.y+inner.height as i32).min(fb.height as i32) {
                        for xx in left.max(0)..right.min(fb.width as i32) {
                            fb.pixels[yy as usize*fb.width as usize+xx as usize]=0xff628ac4;
                        }
                    }
                }
            }
        }
        let _ = font.draw_text_styled(
            fb,
            &display,
            x,
            y,
            pt,
            color,
            Some(inner),
            s.computed.font.style,
            s.computed.font.variant,
            s.computed.font.weight,
            s.computed.font.stretch,
            line
        );
        if focused && s.enabled {
            let cx = inner.x + (pw.saturating_sub(scroll) as i32);
            vline(fb, cx, inner.y + 2, inner.y + (inner.height as i32) - 2, color)
        }
    }
}

/// Implements font pt.
///
/// Visibility: **private**.
#[doc = "Implements the font pt operation. Visibility: private/internal."]
#[cfg(feature = "fonts")]
fn font_pt(px: f32) -> u16 {
    px.round().clamp(1.0, u16::MAX as f32) as u16
}
/// Implements paint box.
///
/// Visibility: **private**.
#[doc = "Implements the paint box operation. Visibility: private/internal."]
fn paint_box(fb: &mut Framebuffer, r: Rect, radius: ResolvedBorderRadius, border: u32, fill: u32) {
    fill_rounded_rect(fb, r, radius, border);
    if r.width > 2 && r.height > 2 {
        let inner = Rect::new(r.x + 1, r.y + 1, r.width - 2, r.height - 2);
        let rad = shrink_radius(radius, 1.0);
        fill_rounded_rect(fb, inner, rad, fill)
    }
}
/// Fill a retained content widget's computed background using the same
/// rounded-rectangle painter as ordinary native Mata UI controls.
pub(crate) fn paint_content_background(fb: &mut Framebuffer, bounds: Rect,
    radius: ResolvedBorderRadius, color: u32, opacity: f32) {
    fill_rounded_rect(fb, bounds, radius, with_opacity(color, opacity));
}

/// Paint a one-pixel CSS-like outline without covering a transparent interior.
/// Uses the native rounded-rectangle geometry, and can be applied to retained
/// labels and other content roots (not just interactive controls).
pub(crate) fn paint_content_outline(fb: &mut Framebuffer, bounds: Rect,
    radius: ResolvedBorderRadius, color: u32, opacity: f32) {
    if bounds.width == 0 || bounds.height == 0 {return;}
    let inner = Rect::new(bounds.x.saturating_add(1),bounds.y.saturating_add(1),
        bounds.width.saturating_sub(2),bounds.height.saturating_sub(2));
    let inner_radius=shrink_radius(radius,1.0);
    let x0=bounds.x.max(0);
    let y0=bounds.y.max(0);
    let x1=(bounds.x as i64+bounds.width as i64).min(fb.width as i64) as i32;
    let y1=(bounds.y as i64+bounds.height as i64).min(fb.height as i64) as i32;
    let color=with_opacity(color,opacity);
    for y in y0..y1 {for x in x0..x1 {
        if rounded_contains(bounds,radius,x,y) &&
           (x<inner.x || y<inner.y || x>=inner.x.saturating_add(inner.width as i32) ||
            y>=inner.y.saturating_add(inner.height as i32) ||
            !rounded_contains(inner,inner_radius,x,y)) {
            blend_at(fb,x,y,color);
        }
    }}
}

/// Implements shrink radius.
///
/// Visibility: **private**.
#[doc = "Implements the shrink radius operation. Visibility: private/internal."]
fn shrink_radius(mut r: ResolvedBorderRadius, v: f32) -> ResolvedBorderRadius {
    for c in [&mut r.top_left, &mut r.top_right, &mut r.bottom_right, &mut r.bottom_left] {
        c.x = (c.x - v).max(0.0);
        c.y = (c.y - v).max(0.0);
    }
    r
}
/// Implements fill rounded rect.
///
/// Visibility: **private**.
#[doc = "Implements the fill rounded rect operation. Visibility: private/internal."]
fn fill_rounded_rect(fb: &mut Framebuffer, r: Rect, radius: ResolvedBorderRadius, color: u32) {
    if r.width == 0 || r.height == 0 {
        return;
    }
    let l = r.x.max(0);
    let t = r.y.max(0);
    let rr = (r.x + (r.width as i32)).min(fb.width as i32);
    let bb = (r.y + (r.height as i32)).min(fb.height as i32);
    for y in t..bb {
        for x in l..rr {
            if rounded_contains(r, radius, x, y) {
                blend_at(fb, x, y, color);
            }
        }
    }
}
/// Implements rounded contains.
///
/// Visibility: **private**.
#[doc = "Implements the rounded contains operation. Visibility: private/internal."]
fn rounded_contains(r: Rect, rad: ResolvedBorderRadius, x: i32, y: i32) -> bool {
    let fx = (x as f32) + 0.5;
    let fy = (y as f32) + 0.5;
    let left = r.x as f32;
    let top = r.y as f32;
    let right = left + (r.width as f32);
    let bottom = top + (r.height as f32);
    #[doc = "Implements the inside operation. Visibility: private/internal."]
    fn inside(cx: f32, cy: f32, rx: f32, ry: f32, x: f32, y: f32) -> bool {
        if rx <= 0.0 || ry <= 0.0 {
            return true;
        }
        let dx = (x - cx) / rx;
        let dy = (y - cy) / ry;
        dx * dx + dy * dy <= 1.0
    }
    if fx < left + rad.top_left.x && fy < top + rad.top_left.y {
        return inside(
            left + rad.top_left.x,
            top + rad.top_left.y,
            rad.top_left.x,
            rad.top_left.y,
            fx,
            fy
        );
    }
    if fx > right - rad.top_right.x && fy < top + rad.top_right.y {
        return inside(
            right - rad.top_right.x,
            top + rad.top_right.y,
            rad.top_right.x,
            rad.top_right.y,
            fx,
            fy
        );
    }
    if fx > right - rad.bottom_right.x && fy > bottom - rad.bottom_right.y {
        return inside(
            right - rad.bottom_right.x,
            bottom - rad.bottom_right.y,
            rad.bottom_right.x,
            rad.bottom_right.y,
            fx,
            fy
        );
    }
    if fx < left + rad.bottom_left.x && fy > bottom - rad.bottom_left.y {
        return inside(
            left + rad.bottom_left.x,
            bottom - rad.bottom_left.y,
            rad.bottom_left.x,
            rad.bottom_left.y,
            fx,
            fy
        );
    }
    true
}
/// Returns the value configured with opacity.
///
/// Visibility: **private**.
#[doc = "Provides access configured with opacity. Visibility: private/internal."]
fn with_opacity(color: u32, opacity: f32) -> u32 {
    let a = ((color >> 24) & 255) as f32;
    (((a * opacity.clamp(0.0, 1.0)).round() as u32) << 24) | (color & 0x00ff_ffff)
}
/// Implements blend at.
///
/// Visibility: **private**.
#[doc = "Implements the blend at operation. Visibility: private/internal."]
fn blend_at(fb: &mut Framebuffer, x: i32, y: i32, src: u32) {
    if x < 0 || y < 0 || x >= (fb.width as i32) || y >= (fb.height as i32) {
        return;
    }
    let i = (y as usize) * (fb.width as usize) + (x as usize);
    fb.pixels[i] = alpha_over(src, fb.pixels[i]);
}
/// Implements alpha over.
///
/// Visibility: **private**.
#[doc = "Implements the alpha over operation. Visibility: private/internal."]
fn alpha_over(src: u32, dst: u32) -> u32 {
    let a = (src >> 24) & 255;
    if a == 255 {
        return src;
    }
    if a == 0 {
        return dst;
    }
    let inv = 255 - a;
    let sr = (src >> 16) & 255;
    let sg = (src >> 8) & 255;
    let sb = src & 255;
    let dr = (dst >> 16) & 255;
    let dg = (dst >> 8) & 255;
    let db = dst & 255;
    let r = (sr * a + dr * inv + 127) / 255;
    let g = (sg * a + dg * inv + 127) / 255;
    let b = (sb * a + db * inv + 127) / 255;
    0xff00_0000 | (r << 16) | (g << 8) | b
}
/// Implements vline.
///
/// Visibility: **private**.
#[doc = "Implements the vline operation. Visibility: private/internal."]
fn vline(fb: &mut Framebuffer, x: i32, y0: i32, y1: i32, color: u32) {
    if x < 0 || x >= (fb.width as i32) || fb.height == 0 {
        return;
    }
    let t = y0
        .min(y1)
        .max(0)
        .min((fb.height as i32) - 1);
    let b = y0
        .max(y1)
        .max(0)
        .min((fb.height as i32) - 1);
    for y in t..=b {
        blend_at(fb, x, y, color);
    }
}
/// Implements prev boundary.
///
/// Visibility: **crate-visible**.
#[doc = "Implements the prev boundary operation. Visibility: crate-visible."]
pub(crate) fn prev_boundary(s: &str, index: usize) -> usize {
    let i = index.min(s.len());
    s[..i]
        .char_indices()
        .next_back()
        .map(|(i, _)| i)
        .unwrap_or(0)
}
/// Implements next boundary.
///
/// Visibility: **crate-visible**.
#[doc = "Implements the next boundary operation. Visibility: crate-visible."]
pub(crate) fn next_boundary(s: &str, index: usize) -> usize {
    let i = index.min(s.len());
    if i >= s.len() {
        return s.len();
    }
    i + s[i..].chars().next().map(char::len_utf8).unwrap_or(0)
}
