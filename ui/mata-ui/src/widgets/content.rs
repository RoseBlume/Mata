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
    /// Reference to a live Button/TextField already registered with a host.
    Interactive(super::WidgetId),
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
            Self::Interactive(_) => (),
            Self::Painted(v) => v.widget.attach_dirty(dirty),
            Self::Surface {widget,..} => widget.attach_dirty(dirty),
            Self::Empty => (),
        }
    }
    pub(crate) fn simple_text(&self) -> Option<String> {
        match self { Self::Text(v) => Some(v.text()), Self::Label(v) => Some(v.text()), _ => None }
    }
    #[cfg(test)]
    fn preferred(&self,inherited:&ComputedStyle)->(u32,u32) {
        self.measure_at(16_384,inherited,&mut None,32)
    }

    /// Content-driven size under an available width, shared by root sizing,
    /// nested row/column layout, and pointer hit testing.
    pub(crate) fn measure_at(&self, available_width:u32, inherited:&ComputedStyle,
        font: &mut Option<&mut FontRenderer>, depth:u8) -> (u32,u32) {
        if depth==0 {return (0,0)}
        let max_width=available_width.max(1);
        match self {
            Self::Text(v)=>super::text_selection::measure_wrapped(&v.text(),v.widget(),max_width,
                inherited,(max_width,u32::MAX/4),font.as_deref_mut()),
            Self::Label(v)=>super::text_selection::measure_wrapped(&v.text(),v.widget(),max_width,
                inherited,(max_width,u32::MAX/4),font.as_deref_mut()),
            Self::Image(v)=>v.preferred_size().or_else(||v.image_data().map(|x|(x.width,x.height)))
                .map(|(w,h)|(w.min(max_width),h)).unwrap_or((24.min(max_width),24)),
            Self::Picture(v)=>v.image().map(|x|(x.width.min(max_width),x.height))
                .unwrap_or((24.min(max_width),24)),
            Self::Group(g)=>{
                let Ok(group)=g.0.try_lock() else{return (0,0)};
                let count=group.children.len();
                let gap=group.gap.saturating_mul(count.saturating_sub(1) as u32);
                let mut sizes:Vec<(u32,u32)>=group.children.iter()
                    .map(|c|c.measure_at(max_width,inherited,font,depth-1)).collect();
                if group.direction==ContentDirection::Row {
                    let usable=max_width.saturating_sub(gap);
                    let widths=distribute(&sizes,usable);
                    for (item, (size,width)) in group.children.iter().zip(sizes.iter_mut().zip(widths)) {
                        *size=item.measure_at(width.max(1),inherited,font,depth-1);
                    }
                    (sizes.iter().fold(gap,|a,s|a.saturating_add(s.0)).min(max_width),
                        sizes.iter().map(|s|s.1).max().unwrap_or(0))
                } else if group.direction==ContentDirection::Column {
                    (sizes.iter().map(|s|s.0).max().unwrap_or(0).min(max_width),
                        sizes.iter().fold(gap,|a,s|a.saturating_add(s.1)))
                } else {
                    (sizes.iter().map(|s|s.0).max().unwrap_or(0).min(max_width),
                        sizes.iter().map(|s|s.1).max().unwrap_or(0))
                }
            }
            Self::Painted(v)=>{
                let b=v.widget.bounds();(b.width.max(160).min(max_width),b.height.max(90))
            }
            Self::Surface{pixels,..}=>pixels.try_lock().ok().map(|fb|
                (fb.width.max(1).min(max_width),fb.height.max(1))).unwrap_or((32.min(max_width),32)),
            Self::Interactive(_)=>(120.min(max_width),36),
            Self::Empty=>(0,0),
        }
    }
}

/// Preserve natural child dimensions unless the row is too narrow.
fn distribute(sizes:&[(u32,u32)],usable:u32)->Vec<u32> {
    let sum:u64 = sizes.iter().map(|s|s.0 as u64).sum();
    if sum <= usable as u64 {return sizes.iter().map(|s|s.0).collect()}
    if sum==0 {return vec![0;sizes.len()]}
    let mut used=0u32;
    sizes.iter().enumerate().map(|(i,s)|{
        let width=if i+1==sizes.len(){usable.saturating_sub(used)}else{
            ((usable as u64*s.0 as u64)/sum).min(usable as u64) as u32
        };
        used=used.saturating_add(width);width
    }).collect()
}

/// A single layout algorithm for drawing, text selection and interactive
/// hit boxes, so changes to text wrapping never desynchronize mouse targets.
fn child_rects(group:&GroupState,bounds:Rect,inherited:&ComputedStyle,
    font:&mut Option<&mut FontRenderer>,depth:u8)->Vec<Rect> {
    let children=&group.children;
    if children.is_empty() {return Vec::new()}
    if group.direction==ContentDirection::Overlay {return vec![bounds;children.len()]}
    let horizontal=group.direction==ContentDirection::Row;
    let count=children.len();
    let gap_total=group.gap.saturating_mul(count.saturating_sub(1) as u32);
    let usable=bounds.width.saturating_sub(gap_total);
    let mut sizes:Vec<(u32,u32)>=children.iter().map(|c|
        c.measure_at(if horizontal{usable.max(1)}else{bounds.width.max(1)},inherited,font,depth)).collect();
    if horizontal {
        let widths=distribute(&sizes,usable);
        for (child,(size,width)) in children.iter().zip(sizes.iter_mut().zip(widths)) {
            *size=child.measure_at(width.max(1),inherited,font,depth);
        }
    }
    let mut offset=0u32;
    sizes.iter().enumerate().map(|(i,(w,h))|{
        let position=offset.saturating_add(group.gap.saturating_mul(i as u32));
        let delta=position.min(i32::MAX as u32) as i32;
        let rect=if horizontal {
            Rect::new(bounds.x.saturating_add(delta),bounds.y,*w,bounds.height)
        }else{
            Rect::new(bounds.x,bounds.y.saturating_add(delta),bounds.width,*h)
        };
        offset=offset.saturating_add(if horizontal{*w}else{*h});rect
    }).collect()
}

impl From<&super::controls::Button> for WidgetContent {
    fn from(v:&super::controls::Button)->Self {Self::Interactive(v.id())}
}
impl From<&super::controls::TextField> for WidgetContent {
    fn from(v:&super::controls::TextField)->Self {Self::Interactive(v.id())}
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

/// A selectable text node and the bounds actually used by its content painter.
#[derive(Clone)]
pub(crate) struct SelectableHit {
    pub text: Text,
    pub bounds: Rect,
    pub inherited: ComputedStyle,
}

impl WidgetContent {
    /// Resolve interactive child rectangles from the SAME proportional row/column
    /// algorithm used to paint retained visual content.
    pub(crate) fn collect_interactive(&self,bounds:Rect,inherited:&ComputedStyle,
        out:&mut Vec<(super::WidgetId,Rect)>,depth:u8,
        mut font:Option<&mut FontRenderer>) {
        if depth==0{return}
        match self {
            Self::Interactive(id)=>out.push((*id,bounds)),
            Self::Group(g)=>{
                let Ok(group)=g.0.try_lock() else{return};
                let rectangles=child_rects(&group,bounds,inherited,&mut font,depth-1);
                for (child,rect) in group.children.iter().zip(rectangles) {
                    child.collect_interactive(rect,inherited,out,depth-1,font.as_deref_mut());
                }
            }
            _=>(),
        }
    }

    /// Find selectable text using the same child rectangles as the painter.
    pub(crate) fn selectable_at(&self,bounds:Rect,inherited:&ComputedStyle,
        x:i32,y:i32,mut font:Option<&mut FontRenderer>)->Option<SelectableHit> {
        self.selectable_at_depth(bounds,inherited,x,y,32,font.as_deref_mut())
    }
    fn selectable_at_depth(&self,bounds:Rect,inherited:&ComputedStyle,
        x:i32,y:i32,depth:u8,mut font:Option<&mut FontRenderer>)->Option<SelectableHit> {
        if depth==0||!bounds.contains(x,y){return None}
        match self {
            Self::Text(t) if t.selectable() && t.widget().visible() && t.widget().sensitive() =>
                Some(SelectableHit{text:t.clone(),bounds,inherited:inherited.clone()}),
            Self::Label(t) if t.selectable() && t.widget().visible() && t.widget().sensitive() =>
                Some(SelectableHit{text:t.as_text_widget(),bounds,inherited:inherited.clone()}),
            Self::Group(g)=>{
                let Ok(group)=g.0.try_lock() else {return None};
                let rects=child_rects(&group,bounds,inherited,&mut font,depth-1);
                for (child,rect) in group.children.iter().zip(rects).rev() {
                    if let Some(hit)=child.selectable_at_depth(rect,inherited,x,y,depth-1,font.as_deref_mut()) {
                        return Some(hit)
                    }
                }
                None
            }
            _=>None
        }
    }

}

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
            WidgetContent::Text(v) => self.text(&v.text(), v.widget(), bounds, inherited, v.selection_range()),
            WidgetContent::Label(v) => self.text(&v.text(), v.widget(), bounds, inherited, v.selection_range()),
            WidgetContent::Image(v) => {
                if v.widget().visible() {
                    let bounds = if let Some((width, height)) = v.preferred_size() {
                        let width = width.min(bounds.width);
                        let height = height.min(bounds.height);
                        Rect::new(bounds.x.saturating_add((bounds.width-width) as i32 / 2),
                            bounds.y.saturating_add((bounds.height-height) as i32 / 2), width, height)
                    } else {bounds};
                    v.blit_within(&mut *self.fb, bounds, inherited.opacity);
                }
            }
            WidgetContent::Picture(v) => {
                if v.widget().visible() { v.blit_within(&mut *self.fb, bounds, inherited.opacity); }
            }
            WidgetContent::Group(g) => {
                // Reflow children using their measured heights/widths. Child
                // rectangles match both pointer hit testing and painting.
                let Ok(group)=g.0.try_lock() else {return};
                let mut font:Option<&mut FontRenderer> = {
                    #[cfg(feature="fonts")]
                    { self.font.as_mut() }
                    #[cfg(not(feature="fonts"))]
                    { None }
                };
                let rectangles=child_rects(&group,bounds,inherited,&mut font,depth-1);
                drop(font);
                for (child,rect) in group.children.iter().zip(rectangles) {
                    self.paint_at(child,rect,inherited,depth-1);
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
            WidgetContent::Interactive(_) | WidgetContent::Empty => (),
        }
    }

    fn text(&mut self, value: &str, widget: &super::catalog::Widget, bounds: Rect,
            inherited: &ComputedStyle, selection: Option<(usize,usize)>) {
        if !widget.visible() { return; }
        let mut computed = widget.style_handle().computed(bounds, (self.fb.width, self.fb.height), Some(bounds));
        let props = widget.style_handle().properties();
        if let Some(background) = computed.background_color {
            super::render::paint_content_background(
                &mut *self.fb,bounds,computed.border_radius,background,
                computed.opacity * inherited.opacity
            );
        }
        if let Some(border) = computed.border_color {
            super::render::paint_content_outline(
                &mut *self.fb,bounds,computed.border_radius,border,
                computed.opacity * inherited.opacity
            );
        }
        #[cfg(feature = "fonts")]
        {
            // Text without explicit typography inherits its parent's properties.
            if props.font.family.is_none() { computed.font.family = inherited.font.family.clone(); }
            if props.font.size.is_none() { computed.font_size_px = inherited.font_size_px; }
            let color = computed.foreground_color.or(inherited.foreground_color).unwrap_or(0xfff2f4f7);
            let alpha = ((color >> 24) & 255) as f32 * computed.opacity * inherited.opacity;
            let color = ((alpha.round().clamp(0.0, 255.0) as u32) << 24) | (color & 0xffffff);
            let pt = computed.font_size_px.round().clamp(1.0, u16::MAX as f32) as u16;
            if let Some(font) = super::window::font_for_style(&mut *self.font, &mut *self.cache, &computed.font) {
                let geometry=super::text_selection::layout(value,widget,bounds,inherited,
                    (self.fb.width,self.fb.height),Some(&mut *font));
                super::text_selection::paint_selection(&mut *self.fb,value,&geometry,selection,&mut Some(&mut *font));
                for segment in &geometry.lines {
                    if let Some(line_text)=value.get(segment.start..segment.end) {
                        let _=font.draw_text_styled(&mut *self.fb,line_text,segment.x,segment.y,pt,color,
                            Some(geometry.inner),geometry.style.style,geometry.style.variant,
                            geometry.style.weight,geometry.style.stretch,Some(geometry.step));
                    }
                }
            }
        }
        #[cfg(not(feature = "fonts"))]
        { let _ = (value, bounds, inherited, computed, props, selection); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_is_a_first_class_button_child() {
        let image=Image::new();
        image.set_image(super::super::catalog::ImageData::from_argb32(2, 2,
            vec![0xff00ff00; 4]).unwrap());
        image.set_size(18,18);
        let node=WidgetContent::from(&image);
        assert!(matches!(node,WidgetContent::Image(_)));
        assert_eq!(node.preferred(&ComputedStyle::default()),(18,18));
        assert!(node.simple_text().is_none());
    }

    #[test]
    fn updating_nested_image_invalidates_the_button() {
        let picture=Image::new();
        let child=WidgetContent::from(&picture);
        let dirty=Arc::new(AtomicBool::new(false));
        child.attach(&dirty);
        picture.set_image(super::super::catalog::ImageData::from_argb32(
            1,1,vec![0xff00_aaff]).unwrap());
        assert!(dirty.swap(false,Ordering::AcqRel));
        picture.set_size(20,20);
        assert!(dirty.load(Ordering::Acquire));
    }

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

#[cfg(test)]
mod natural_layout_tests {
    use super::*;
    #[test]
    fn auto_text_height_increases_when_width_is_constrained() {
        let text = Text::new("The quick brown fox jumps over the lazy dog 🌷");
        let node = WidgetContent::from(&text);
        let style = ComputedStyle::default();
        let wide = node.measure_at(500, &style, &mut None, 32);
        let narrow = node.measure_at(85, &style, &mut None, 32);
        assert!(narrow.1 > wide.1);
        assert!(narrow.0 <= 85);
    }
    #[test]
    fn column_reflows_when_text_changes() {
        let text = Text::new("short");
        let column = ContentGroup::column(6);
        column.append(&text);
        column.append(Text::new("after"));
        let content = WidgetContent::Group(column);
        let inherited = ComputedStyle::default();
        let before=content.measure_at(90,&inherited,&mut None,32);
        text.set_text("This is a much longer paragraph that must wrap to several lines before the next widget.");
        let after=content.measure_at(90,&inherited,&mut None,32);
        assert!(after.1 > before.1);
    }
    #[test]
    fn layout_children_uses_natural_column_heights() {
        let group=ContentGroup::column(4);
        group.append(Text::new("a"));
        group.append(Text::new("one two three four five six seven eight"));
        let state=group.0.lock().unwrap();
        let rects=child_rects(&state,Rect::new(0,0,85,250),
            &ComputedStyle::default(),&mut None,31);
        assert_eq!(rects.len(),2);
        assert_eq!(rects[1].y,rects[0].y+rects[0].height as i32+4);
    }
}
