//! Retained, nestable visual content for software widgets.
//!
//! A button (or a standalone window content root) can display a caller-owned
//! `Text`, `Image`, `Picture`, or a `ContentGroup`. Clones share widget state:
//! updating the original text or image updates the next host presentation.

use std::sync::{Arc, Mutex, Weak, atomic::{AtomicBool, Ordering}};
#[cfg(feature = "fonts")]
use std::collections::HashMap;

use crate::Framebuffer;
use super::{Rect, FontRenderer, style::ComputedStyle};
#[cfg(feature = "fonts")]
use super::style::Length;
use super::catalog::{Text, Label, Image, Picture, WidgetExt};

/// Layout behavior for a group of child widgets.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ContentDirection { Row, Column, Overlay }

#[derive(Debug)]
struct GroupState {
    direction: ContentDirection,
    gap: u32,
    children: Vec<WidgetContent>,
    dirty: Option<Weak<AtomicBool>>,
}

/// An editable, sharable layout container. Groups can contain other groups.
#[derive(Debug, Clone)]
pub struct ContentGroup(Arc<Mutex<GroupState>>);

impl ContentGroup {
    pub fn new(direction: ContentDirection, gap: u32) -> Self {
        Self(Arc::new(Mutex::new(GroupState {
            direction, gap, children: Vec::new(), dirty: None,
        })))
    }
    pub fn row(gap: u32) -> Self { Self::new(ContentDirection::Row, gap) }
    pub fn column(gap: u32) -> Self { Self::new(ContentDirection::Column, gap) }
    pub fn overlay() -> Self { Self::new(ContentDirection::Overlay, 0) }

    /// Append retained child content. The child is shared, not copied as pixels.
    pub fn append(&self, child: impl Into<WidgetContent>) {
        let child = child.into();
        let dirty = self.0.lock().unwrap().dirty.as_ref().and_then(Weak::upgrade);
        if let Some(dirty) = &dirty { child.attach(dirty); }
        self.0.lock().unwrap().children.push(child);
        if let Some(dirty) = dirty { dirty.store(true, Ordering::Release); }
    }
    pub(crate) fn replace_children(&self, children: Vec<WidgetContent>) {
        let dirty = self.0.lock().unwrap().dirty.as_ref().and_then(Weak::upgrade);
        if let Some(flag) = &dirty {
            for child in &children { child.attach(flag); }
        }
        self.0.lock().unwrap().children = children;
        if let Some(flag) = dirty { flag.store(true, Ordering::Release); }
    }
    pub fn clear(&self) {
        let mut group = self.0.lock().unwrap();
        group.children.clear();
        if let Some(dirty) = group.dirty.as_ref().and_then(Weak::upgrade) {
            dirty.store(true, Ordering::Release);
        }
    }
    pub fn set_gap(&self, gap: u32) {
        let mut group = self.0.lock().unwrap();
        group.gap = gap;
        if let Some(dirty) = group.dirty.as_ref().and_then(Weak::upgrade) {
            dirty.store(true, Ordering::Release);
        }
    }
    pub fn len(&self) -> usize { self.0.lock().unwrap().children.len() }

    fn attach(&self, dirty: &Arc<AtomicBool>, depth: u8) {
        if depth == 0 { return; }
        let children = {
            let mut group = self.0.lock().unwrap();
            group.dirty = Some(Arc::downgrade(dirty));
            group.children.clone()
        };
        for child in &children { child.attach_at(dirty, depth - 1); }
    }
}

/// A renderable child of a button, group, or content root.
#[derive(Debug, Clone)]
pub enum WidgetContent {
    Empty,
    Text(Text),
    Label(Label),
    Image(Image),
    Picture(Picture),
    Group(ContentGroup),
    /// Software-painted instance of a native MataWin catalog widget.
    Painted(super::catalog::PaintedWidget),
    /// Retained DrawingArea/GLArea framebuffer; reads pixels at paint time.
    Surface { widget: super::catalog::Widget, pixels: Arc<Mutex<Framebuffer>> },
}

impl WidgetContent {
    pub fn empty() -> Self { Self::Empty }
    pub fn row(gap: u32) -> ContentGroup { ContentGroup::row(gap) }
    pub fn column(gap: u32) -> ContentGroup { ContentGroup::column(gap) }
    pub fn overlay() -> ContentGroup { ContentGroup::overlay() }

    pub(crate) fn attach(&self, dirty: &Arc<AtomicBool>) { self.attach_at(dirty, 32); }
    fn attach_at(&self, dirty: &Arc<AtomicBool>, depth: u8) {
        if depth == 0 { return; }
        match self {
            Self::Text(v) => v.widget().attach_dirty(dirty),
            Self::Label(v) => v.widget().attach_dirty(dirty),
            Self::Image(v) => v.widget().attach_dirty(dirty),
            Self::Picture(v) => v.widget().attach_dirty(dirty),
            Self::Group(v) => v.attach(dirty, depth - 1),
            Self::Painted(v) => v.widget.attach_dirty(dirty),
            Self::Surface {widget,..} => widget.attach_dirty(dirty),
            Self::Empty => (),
        }
    }
    pub(crate) fn simple_text(&self) -> Option<String> {
        match self { Self::Text(v) => Some(v.text()), Self::Label(v) => Some(v.text()), _ => None }
    }
    fn preferred(&self, inherited: &ComputedStyle) -> (u32, u32) {
        self.preferred_at(inherited, 32)
    }
    fn preferred_at(&self, inherited: &ComputedStyle, depth: u8) -> (u32, u32) {
        if depth == 0 { return (1, 1); }
        match self {
            Self::Text(v) => preferred_text(&v.text(), v.widget(), inherited),
            Self::Label(v) => preferred_text(&v.text(), v.widget(), inherited),
            Self::Image(v) => v.image_data().map(|x| (x.width, x.height)).unwrap_or((24, 24)),
            Self::Picture(v) => v.image().map(|x| (x.width, x.height)).unwrap_or((24, 24)),
            Self::Group(group) => {
                let Ok(group) = group.0.try_lock() else { return (1, 1); };
                let mut width = 0u32;
                let mut height = 0u32;
                for child in &group.children {
                    let (w, h) = child.preferred_at(inherited, depth - 1);
                    match group.direction {
                        ContentDirection::Row => { width = width.saturating_add(w); height = height.max(h); },
                        ContentDirection::Column => { height = height.saturating_add(h); width = width.max(w); },
                        ContentDirection::Overlay => { width = width.max(w); height = height.max(h); },
                    }
                }
                let gaps = group.gap.saturating_mul(group.children.len().saturating_sub(1) as u32);
                match group.direction {
                    ContentDirection::Row => width = width.saturating_add(gaps),
                    ContentDirection::Column => height = height.saturating_add(gaps),
                    ContentDirection::Overlay => (),
                }
                (width.max(1), height.max(1))
            }
            Self::Painted(v) => {
                let b=v.widget.bounds();
                (b.width.max(160),b.height.max(90))
            },
            Self::Surface { pixels, .. } => match pixels.try_lock() {
                Ok(fb) => (fb.width.max(1),fb.height.max(1)),
                Err(_) => (32,32),
            },
            Self::Empty => (0, 0),
        }
    }
}

fn preferred_text(text: &str, widget: &super::catalog::Widget, inherited: &ComputedStyle) -> (u32, u32) {
    let style = widget.style_handle().properties();
    #[cfg(feature = "fonts")]
    let size = match style.font.size {
        Some(Length::Px(px)) => px.max(1.0),
        _ => inherited.font_size_px.max(1.0),
    };
    #[cfg(not(feature = "fonts"))]
    let size = inherited.font_size_px.max(1.0);
    let _ = style;
    (((text.chars().count() as f32 * size * 0.6).ceil() as u32).max(1),
      (size * 1.4).ceil() as u32)
}

impl From<Text> for WidgetContent { fn from(v: Text) -> Self { Self::Text(v) } }
impl From<&Text> for WidgetContent { fn from(v: &Text) -> Self { Self::Text(v.clone()) } }
impl From<Label> for WidgetContent { fn from(v: Label) -> Self { Self::Label(v) } }
impl From<&Label> for WidgetContent { fn from(v: &Label) -> Self { Self::Label(v.clone()) } }
impl From<Image> for WidgetContent { fn from(v: Image) -> Self { Self::Image(v) } }
impl From<&Image> for WidgetContent { fn from(v: &Image) -> Self { Self::Image(v.clone()) } }
impl From<Picture> for WidgetContent { fn from(v: Picture) -> Self { Self::Picture(v) } }
impl From<&Picture> for WidgetContent { fn from(v: &Picture) -> Self { Self::Picture(v.clone()) } }
impl From<super::catalog::Box> for WidgetContent {
    fn from(v: super::catalog::Box) -> Self { Self::Group(v.content_group()) }
}
impl From<&super::catalog::Box> for WidgetContent {
    fn from(v: &super::catalog::Box) -> Self { Self::Group(v.content_group()) }
}
impl From<ContentGroup> for WidgetContent { fn from(v: ContentGroup) -> Self { Self::Group(v) } }
impl From<&ContentGroup> for WidgetContent { fn from(v: &ContentGroup) -> Self { Self::Group(v.clone()) } }
impl From<&str> for WidgetContent { fn from(v: &str) -> Self { Self::Text(Text::new(v)) } }
impl From<String> for WidgetContent { fn from(v: String) -> Self { Self::Text(Text::new(v)) } }
impl From<&String> for WidgetContent { fn from(v: &String) -> Self { Self::Text(Text::new(v.clone())) } }

/// Shared drawing context for nested content, avoiding separate font caches.
pub(crate) struct ContentPainter<'a> {
    pub fb: &'a mut Framebuffer,
    #[cfg(feature = "fonts")]
    pub font: &'a mut Option<FontRenderer>,
    #[cfg(feature = "fonts")]
    pub cache: &'a mut HashMap<String, FontRenderer>,
}

impl ContentPainter<'_> {
    pub(crate) fn paint(&mut self, content: &WidgetContent, bounds: Rect, inherited: &ComputedStyle) {
        self.paint_at(content, bounds, inherited, 32);
    }
    fn paint_at(&mut self, content: &WidgetContent, bounds: Rect, inherited: &ComputedStyle, depth: u8) {
        if depth == 0 || bounds.width == 0 || bounds.height == 0 { return; }
        match content {
            WidgetContent::Text(v) => self.text(&v.text(), v.widget(), bounds, inherited),
            WidgetContent::Label(v) => self.text(&v.text(), v.widget(), bounds, inherited),
            WidgetContent::Image(v) => {
                if v.widget().visible() { v.blit_within(&mut *self.fb, bounds, inherited.opacity); }
            }
            WidgetContent::Picture(v) => {
                if v.widget().visible() { v.blit_within(&mut *self.fb, bounds, inherited.opacity); }
            }
            WidgetContent::Group(g) => {
                // try_lock prevents a self-nested group from deadlocking the UI thread.
                let Ok(group) = g.0.try_lock() else { return; };
                let count = group.children.len();
                if count == 0 { return; }
                if group.direction == ContentDirection::Overlay {
                    for child in &group.children { self.paint_at(child, bounds, inherited, depth - 1); }
                    return;
                }
                let horizontal = group.direction == ContentDirection::Row;
                let full = if horizontal { bounds.width } else { bounds.height };
                let total_gap = group.gap.saturating_mul(count.saturating_sub(1) as u32);
                let usable = full.saturating_sub(total_gap);
                let preferred: Vec<u32> = group.children.iter().map(|child| {
                    let size = child.preferred(inherited);
                    (if horizontal { size.0 } else { size.1 }).max(1)
                }).collect();
                let sum = preferred.iter().map(|n| *n as u64).sum::<u64>().max(1);
                let mut offset = 0u32;
                for (i, child) in group.children.iter().enumerate() {
                    let span = if i + 1 == count {
                        usable.saturating_sub(offset)
                    } else {
                        (((usable as u64) * (preferred[i] as u64)) / sum).min(usable as u64) as u32
                    };
                    let x = if horizontal { bounds.x.saturating_add(offset.saturating_add(group.gap.saturating_mul(i as u32)).min(i32::MAX as u32) as i32) } else { bounds.x };
                    let y = if horizontal { bounds.y } else { bounds.y.saturating_add(offset.saturating_add(group.gap.saturating_mul(i as u32)).min(i32::MAX as u32) as i32) };
                    let rect = Rect::new(x, y, if horizontal { span } else { bounds.width }, if horizontal { bounds.height } else { span });
                    self.paint_at(child, rect, inherited, depth - 1);
                    offset = offset.saturating_add(span);
                }
            }
            WidgetContent::Painted(v) => {
                super::catalog::native_painter::paint(&mut *self.fb,bounds,v,inherited.opacity);
            }
            WidgetContent::Surface { widget, pixels } => {
                if widget.visible() {
                    if let Ok(src) = pixels.try_lock() {
                        if src.width > 0 && src.height > 0 && bounds.width > 0 && bounds.height > 0 {
                            let x0 = (bounds.x as i64).max(0).min(self.fb.width as i64);
                            let y0 = (bounds.y as i64).max(0).min(self.fb.height as i64);
                            let x1 = ((bounds.x as i64) + bounds.width as i64).min(self.fb.width as i64).max(x0);
                            let y1 = ((bounds.y as i64) + bounds.height as i64).min(self.fb.height as i64).max(y0);
                            let opacity = inherited.opacity * widget.opacity();
                            for yy in y0..y1 { for xx in x0..x1 {
                                let sx = (((xx - bounds.x as i64) * src.width as i64)/bounds.width as i64) as usize;
                                let sy = (((yy - bounds.y as i64) * src.height as i64)/bounds.height as i64) as usize;
                                let color=src.pixels[sy*src.width as usize+sx];
                                let alpha=((color>>24)&255) as f32*opacity;
                                let a=alpha.round().clamp(0.0,255.0) as u32;
                                if a==0 {continue}
                                let dst=&mut self.fb.pixels[yy as usize*self.fb.width as usize+xx as usize];
                                if a==255 {*dst=color;} else {
                                    let inv=255-a;
                                    let r=(((color>>16)&255)*a+((*dst>>16)&255)*inv+127)/255;
                                    let g=(((color>>8)&255)*a+((*dst>>8)&255)*inv+127)/255;
                                    let b=((color&255)*a+(*dst&255)*inv+127)/255;
                                    *dst=0xff000000|(r<<16)|(g<<8)|b;
                                }
                            }}
                        }
                    }
                }
            }
            WidgetContent::Empty => (),
        }
    }

    fn text(&mut self, value: &str, widget: &super::catalog::Widget, bounds: Rect, inherited: &ComputedStyle) {
        if !widget.visible() { return; }
        #[cfg(feature = "fonts")]
        {
            let mut computed = widget.style_handle().computed(bounds, (self.fb.width, self.fb.height), Some(bounds));
            let props = widget.style_handle().properties();
            // Text without an explicit family/size inherits the parent button's typography.
            if props.font.family.is_none() { computed.font.family = inherited.font.family.clone(); }
            if props.font.size.is_none() {
                computed.font_size_px = inherited.font_size_px;
            }
            let color = computed.foreground_color.or(inherited.foreground_color).unwrap_or(0xfff2f4f7);
            let alpha = ((color >> 24) & 255) as f32 * computed.opacity * inherited.opacity;
            let color = ((alpha.round().clamp(0.0, 255.0) as u32) << 24) | (color & 0xffffff);
            let pt = computed.font_size_px.round().clamp(1.0, u16::MAX as f32) as u16;
            let line = Some(computed.line_height_px.round().max(1.0) as u32);
            if let Some(font) = super::window::font_for_style(&mut *self.font, &mut *self.cache, &computed.font) {
                let (w, h) = font.measure_styled(value, pt, computed.font.variant, computed.font.weight, computed.font.stretch, line).unwrap_or((0, 0));
                let x = bounds.x.saturating_add((bounds.width.saturating_sub(w) / 2) as i32);
                let y = bounds.y.saturating_add((bounds.height.saturating_sub(h) / 2) as i32);
                let _ = font.draw_text_styled(&mut *self.fb, value, x, y, pt, color, Some(bounds),
                    computed.font.style, computed.font.variant, computed.font.weight,
                    computed.font.stretch, line);
            }
        }
        #[cfg(not(feature = "fonts"))]
        { let _ = (value, bounds, inherited); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_group_keeps_retained_text() {
        let label = Text::new("First");
        let row = ContentGroup::row(4);
        row.append(&label);
        let column = ContentGroup::column(2);
        column.append(row.clone());
        assert_eq!(column.len(), 1);
        assert_eq!(row.len(), 1);
        label.set_text("Second");
        // The content clone and original share their state.
        assert_eq!(label.text(), "Second");
    }
    #[test]
    fn mutated_text_and_typography_invalidate_host() {
        let text = Text::new("Start");
        let row = ContentGroup::row(8);
        row.append(&text);
        let content = WidgetContent::from(row);
        let dirty = Arc::new(AtomicBool::new(false));
        content.attach(&dirty);
        dirty.store(false, Ordering::Release);
        text.set_text("Updated");
        assert!(dirty.swap(false, Ordering::AcqRel));
        text.set_font_size(23.0);
        assert!(dirty.swap(false, Ordering::AcqRel));
        text.set_font_family("Arial");
        assert!(dirty.load(Ordering::Acquire));
    }

    #[test]
    fn catalog_box_retains_renderable_children() {
        use crate::widgets::catalog::{Box as WidgetBox, Orientation};
        let text = Text::new("A nested box");
        let box_widget = WidgetBox::new(Orientation::Horizontal, 8);
        box_widget.append(&text);
        let node = WidgetContent::from(&box_widget);
        let inherited = ComputedStyle::default();
        assert!(node.preferred(&inherited).0 > 16);
        text.set_text("Longer nested box contents");
        assert!(node.preferred(&inherited).0 > 100);
    }

    #[test]
    fn image_inside_group_paints_framebuffer() {
        let image = Image::new();
        image.set_image(super::super::catalog::ImageData::from_argb32(
            1, 1, vec![0xffff_0000]
        ).unwrap());
        let group = ContentGroup::row(0);
        group.append(image);
        let node = WidgetContent::from(group);
        let mut fb = Framebuffer::from_color(20, 20, 0xff00_0000);
        let inherited = ComputedStyle::default();
        #[cfg(feature = "fonts")]
        {
            let mut font = None;
            let mut cache = HashMap::new();
            ContentPainter { fb: &mut fb, font: &mut font, cache: &mut cache }
                .paint(&node, Rect::new(0, 0, 20, 20), &inherited);
        }
        #[cfg(not(feature = "fonts"))]
        ContentPainter { fb: &mut fb }
            .paint(&node, Rect::new(0, 0, 20, 20), &inherited);
        assert_eq!(fb.pixels[0], 0xffff_0000);
    }

    #[test]
    fn legacy_string_is_a_text_node() {
        let v = WidgetContent::from("Save");
        assert_eq!(v.simple_text().as_deref(), Some("Save"));
    }
}
