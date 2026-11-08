//! Geometry shared between selectable text rendering and pointer hit testing.
//! Selection offsets are UTF-8 byte boundaries, so multi-byte characters survive copy.
use super::{catalog::{Widget, WidgetExt, types::Align}, style::{ComputedStyle, FontProperties}, Rect, FontRenderer};
use crate::Framebuffer;

#[derive(Debug, Clone)]
pub(crate) struct TextLine {
    pub start: usize,
    pub end: usize,
    pub x: i32,
    pub y: i32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct TextLayout {
    pub inner: Rect,
    pub style: FontProperties,
    pub pt: u16,
    pub step: u32,
    pub lines: Vec<TextLine>,
}

/// Uses the identical font metrics and alignment for painting and picking.
pub(crate) fn layout(
    text: &str, widget: &Widget, bounds: Rect, inherited: &ComputedStyle,
    viewport: (u32,u32), mut font: Option<&mut FontRenderer>
) -> TextLayout {
    let mut computed = widget.style_handle().computed(bounds, viewport, Some(bounds));
    let props = widget.style_handle().properties();
    if props.font.family.is_none() { computed.font.family = inherited.font.family.clone(); }
    if props.font.size.is_none() {
        computed.font_size_px = inherited.font_size_px;
        if props.font.line_height.is_none() { computed.line_height_px = inherited.line_height_px; }
    }
    let pt = computed.font_size_px.round().clamp(1.0,u16::MAX as f32) as u16;
    let mut step = computed.line_height_px.round().max(1.0) as u32;
    #[cfg(feature="fonts")]
    if let Some(f) = font.as_deref_mut() {
        step = step.max(f.recommended_line_step(pt).unwrap_or(step));
    }
    let inner = computed.content_bounds;
    let measure = |text: &str, f: &mut Option<&mut FontRenderer>| -> (u32,u32) {
        #[cfg(feature="fonts")]
        if let Some(font) = f.as_deref_mut() {
            return font.measure_styled(text,pt,computed.font.variant,computed.font.weight,
                computed.font.stretch,Some(step)).unwrap_or((0,step));
        }
        #[cfg(not(feature="fonts"))]
        let _ = f;
        ((text.chars().count() as f32 * pt as f32 * 0.6).round() as u32,step)
    };
    // Segment indices are always byte offsets into the original UTF-8 string.
    // Keep whitespace in the previous visual line so copying a selection still
    // returns exactly the source text. Split an overlong word at char boundaries.
    let mut ranges = Vec::<(usize,usize,u32)>::new();
    let mut offset = 0usize;
    let max_line = inner.width.max(1);
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            ranges.push((offset, offset, 0));
        } else {
            let mut start = 0usize;
            while start < paragraph.len() {
                let mut last_fitting = start;
                let mut last_break = None;
                let mut end = paragraph.len();
                let mut used_width = 0u32;
                for (rel, ch) in paragraph[start..].char_indices() {
                    let index = start + rel;
                    let next = index + ch.len_utf8();
                    let advance = measure(&paragraph[index..next], &mut font).0;
                    if used_width.saturating_add(advance) > max_line && last_fitting > start {
                        end = last_break.filter(|&at| at > start).unwrap_or(last_fitting);
                        break;
                    }
                    used_width = used_width.saturating_add(advance);
                    last_fitting = next;
                    if ch.is_whitespace() || ch == '-' { last_break = Some(next); }
                }
                // If a single glyph is wider than the viewport, give it its
                // own line rather than looping forever or splitting its UTF-8.
                if end <= start {
                    end = paragraph[start..].chars().next()
                        .map(|ch| start + ch.len_utf8()).unwrap_or(paragraph.len());
                }
                ranges.push((offset + start, offset + end,
                    measure(&paragraph[start..end], &mut font).0));
                start = end;
            }
        }
        offset = offset.saturating_add(paragraph.len()).saturating_add(1);
    }
    let height = step.saturating_mul(ranges.len().min(u32::MAX as usize) as u32);
    let y = match widget.valign() {
        Align::Start => inner.y,
        Align::End => inner.y.saturating_add(inner.height.saturating_sub(height) as i32),
        _ => inner.y.saturating_add((inner.height.saturating_sub(height)/2) as i32),
    };
    let mut lines = Vec::with_capacity(ranges.len());
    for (i, (start, end, width)) in ranges.into_iter().enumerate() {
        let x = match widget.halign() {
            Align::Start => inner.x,
            Align::End => inner.x.saturating_add(inner.width.saturating_sub(width) as i32),
            _ => inner.x.saturating_add((inner.width.saturating_sub(width)/2) as i32),
        };
        lines.push(TextLine { start, end, x,
            y:y.saturating_add(step.saturating_mul(i.min(u32::MAX as usize) as u32) as i32),
            height:step });
    }
    TextLayout { inner, style:computed.font, pt, step, lines }

}

/// Measure text at an available width, including resolved padding/borders.
/// The same line breaks and font advances are used by rendering and selection.
/// The width is the content's intrinsic (shrink-to-fit) width, and the height
/// grows to accommodate wrapping. Never exceeds the supplied width.
pub(crate) fn measure_wrapped(text: &str, widget: &Widget, max_width: u32,
    inherited: &ComputedStyle, viewport: (u32,u32),
    mut font: Option<&mut FontRenderer>) -> (u32,u32) {
    let outer = Rect::new(0, 0, max_width.max(1), viewport.1.max(1));
    let geometry = layout(text, widget, outer, inherited, viewport, font.as_deref_mut());
    let extra_w = outer.width.saturating_sub(geometry.inner.width);
    let extra_h = outer.height.saturating_sub(geometry.inner.height);
    let mut max_line = 0u32;
    for segment in &geometry.lines {
        let substring = text.get(segment.start..segment.end).unwrap_or("");
        max_line = max_line.max(measure_prefix(substring, &geometry, &mut font));
    }
    (max_line.saturating_add(extra_w).max(1).min(max_width.max(1)),
     geometry.step.saturating_mul(geometry.lines.len().min(u32::MAX as usize) as u32)
        .saturating_add(extra_h).max(1))
}

/// Find the nearest UTF-8 insertion boundary by measuring prefix advances.
pub(crate) fn byte_at(text: &str, geometry: &TextLayout, x: i32, y: i32,
    mut font: Option<&mut FontRenderer>) -> usize {
    let Some(line) = geometry.lines.iter().min_by_key(|line| {
        let bottom=line.y.saturating_add(line.height as i32);
        if y<line.y {(line.y as i64-y as i64) as u64}
        else if y>=bottom {(y as i64-bottom as i64+1) as u64} else {0}
    }) else { return 0 };
    let Some(part) = text.get(line.start..line.end) else {return 0};
    if x<=line.x {return line.start;}
    let mut left = line.start;
    let mut last_width = 0i64;
    for end in part.char_indices().map(|(i,ch)| i+ch.len_utf8()) {
        let width = measure_prefix(&part[..end], geometry, &mut font) as i64;
        if (x as i64) < line.x as i64 + (last_width+width)/2 {
            return left;
        }
        left = line.start+end;
        last_width = width;
    }
    line.end
}

fn measure_prefix(text: &str, geometry:&TextLayout, font:&mut Option<&mut FontRenderer>)->u32 {
    #[cfg(feature="fonts")]
    if let Some(font) = font.as_deref_mut() {
        return font.measure_styled(text,geometry.pt,geometry.style.variant,
            geometry.style.weight,geometry.style.stretch,Some(geometry.step))
            .map(|v|v.0).unwrap_or(0);
    }
    #[cfg(not(feature="fonts"))]
    let _=font;
    (text.chars().count() as f32 * geometry.pt as f32 * 0.6).round() as u32
}

/// Blue selection rectangles under glyphs, clipped to the text content bounds.
pub(crate) fn paint_selection(fb:&mut Framebuffer,text:&str,geometry:&TextLayout,
    range:Option<(usize,usize)>,font:&mut Option<&mut FontRenderer>) {
    let Some((a,b))=range else {return};
    if a>=b || !text.is_char_boundary(a) || !text.is_char_boundary(b) || b>text.len() {return;}
    let clip=geometry.inner;
    for line in &geometry.lines {
        let start=a.max(line.start); let end=b.min(line.end);
        if start>=end {continue;}
        let Some(prefix)=text.get(line.start..start) else {continue};
        let Some(end_prefix)=text.get(line.start..end) else {continue};
        let left=line.x.saturating_add(measure_prefix(prefix,geometry,font) as i32);
        let right=line.x.saturating_add(measure_prefix(end_prefix,geometry,font) as i32);
        for y in line.y.max(clip.y).max(0)..line.y.saturating_add(line.height as i32).min(clip.y.saturating_add(clip.height as i32)).min(fb.height as i32) {
            for x in left.max(clip.x).max(0)..right.min(clip.x.saturating_add(clip.width as i32)).min(fb.width as i32) {
                fb.pixels[y as usize * fb.width as usize + x as usize] = 0xff628ac4;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn utf8_hit_respects_codepoint_boundaries() {
        let widget=super::super::catalog::Text::new("café ☺");
        let inherited=ComputedStyle::default();
        let rect=Rect::new(0,0,200,30);
        let geom=layout(&widget.text(), widget.widget(), rect,&inherited,(200,30),None);
        for x in 0..200 {let offset=byte_at(&widget.text(),&geom,x,10,None);assert!(widget.text().is_char_boundary(offset));}
    }
}

#[cfg(test)]
mod wrap_tests {
    use super::*;
    #[test]
    fn wraps_words_and_retains_original_offsets() {
        let widget = Widget::new();
        let style = ComputedStyle::default();
        let text = "one two three 🌷 four";
        let geometry = layout(text, &widget, Rect::new(0,0,72,200),&style,(72,200),None);
        assert!(geometry.lines.len() > 1);
        for segment in &geometry.lines {
            assert!(text.is_char_boundary(segment.start));
            assert!(text.is_char_boundary(segment.end));
            assert!(segment.end >= segment.start);
        }
        assert_eq!(geometry.lines.iter().map(|l| text.get(l.start..l.end).unwrap()).collect::<String>(), text);
    }
    #[test]
    fn long_unbroken_words_split_without_losing_codepoints() {
        let widget = Widget::new();
        let style = ComputedStyle::default();
        let text = "🌷🌷🌷🌷🌷🌷🌷🌷";
        let geometry = layout(text,&widget,Rect::new(0,0,24,300),&style,(24,300),None);
        assert!(geometry.lines.len()>1);
        assert_eq!(geometry.lines.iter().map(|l|&text[l.start..l.end]).collect::<String>(),text);
    }
    #[test]
    fn auto_height_contains_all_wrapped_lines() {
        // A fixed-height root clips extra lines; automatic height must contain them.
        let widget = Widget::new();
        let style = ComputedStyle::default();
        let text = "Linux: XDG Desktop Portal first; helper dialogs as fallback. Windows: native file dialogs.";
        let available_width = 250;
        let (_, natural_height) = measure_wrapped(
            text, &widget, available_width, &style, (available_width, 400), None
        );
        assert!(natural_height > 40);
        let geometry = layout(
            text, &widget, Rect::new(0, 0, available_width, natural_height),
            &style, (available_width, 400), None
        );
        assert!(geometry.lines.len() >= 2);
        let last_line = geometry.lines.last().unwrap();
        assert!(last_line.y.saturating_add(last_line.height as i32) <= natural_height as i32);
    }

    #[test]
    fn explicit_newlines_are_preserved() {
        let widget=Widget::new(); let style=ComputedStyle::default();
        let text="a\n\nb";
        let geometry=layout(text,&widget,Rect::new(0,0,200,200),&style,(200,200),None);
        assert_eq!(geometry.lines.len(),3);
        assert_eq!(&text[geometry.lines[1].start..geometry.lines[1].end],"");
    }
}
