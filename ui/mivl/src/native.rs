//! Runtime renderer using the existing Mata UI and MataWin APIs.
//!
//! This is intentionally feature gated: parsing, expression evaluation, and
//! event handling do not require a GPU, a display server, or external crates.
use std::{collections::BTreeMap, thread, time::{Duration, Instant}};
use mata_ui::widgets::{Button, ContentGroup, DrawingArea, FontRenderer, Image, Rect, Text, TextField, WidgetContent, WidgetEvent, TouchPhase, WidgetExt, WidgetId, WidgetWindow};
use mata_ui::widgets::style::{FontStyle, FontVariant, FontWeight, FontStretch};
use crate::{Engine, Error, Result, LayoutRect};
use crate::text_layout::{layout_text, TextLayout, WrapMode};

/// A single touch or desktop pointer drag with its own captured target.
struct Gesture {
    view: usize,
    start:(i32,i32),
    last:(i32,i32),
    last_time:Instant,
    velocity:(f64,f64),
    dragged:bool,
}
struct Fling {velocity:(f64,f64), last_time:Instant}
struct Scrollbar {
    view: usize,
    vertical: DrawingArea, vertical_id: WidgetId,
    horizontal: DrawingArea, horizontal_id: WidgetId,
    vertical_bounds: Rect, horizontal_bounds: Rect,
    v_thumb: (u32,u32), h_thumb: (u32,u32),
    v_stamp: String, h_stamp: String,
}


enum Visual {
    Container,
    Surface { area: DrawingArea, id: WidgetId, color: u32, style: String },
    Label { area: DrawingArea, id: WidgetId, stamp: String },
    Button { control: Button, value: String },
    TextField { control: TextField },
}

/// A button owns its visual children; nested Images and Text are painted as
/// part of the Button, not registered as separate hit-test targets.
fn within_button(engine: &Engine, node: usize) -> bool {
    let mut parent=engine.parent(node);
    while let Some(index)=parent {
        if engine.kind(index)=="Button" {return true;}
        parent=engine.parent(index);
    }
    false
}

/// A lightweight reactive signature. Images are decoded only when the nested
/// subtree actually changes, not on every animation/polling frame.
fn button_signature(engine: &Engine, node: usize) -> Result<String> {
    fn walk(engine:&Engine, node:usize, out:&mut String, depth:usize)->Result<()> {
        if depth>32 {return Err(Error::runtime("button nesting exceeds 32 levels"));}
        out.push('(');out.push_str(engine.kind(node));
        for name in ["text","source","width","height","fontSize","fontFamily","visible"] {
            if engine.has_explicit(node,name) {
                let value=engine.property(node,name)?.display();
                out.push('|');out.push_str(name);out.push(':');
                out.push_str(&value.len().to_string());out.push(':');out.push_str(&value);
            }
        }
        for &child in engine.children(node) {walk(engine,child,out,depth+1)?;}
        out.push(')');Ok(())
    }
    let mut signature=String::new();walk(engine,node,&mut signature,0)?;Ok(signature)
}

fn nested_button_content(engine:&Engine,node:usize,depth:usize)->Result<Option<WidgetContent>> {
    if depth>32 {return Err(Error::runtime("button nesting exceeds 32 levels"));}
    if !engine.property(node,"visible")?.truthy() {return Ok(None);}
    match engine.kind(node) {
        "Image" => {
            let source=engine.property(node,"source")?.display();
            let image=Image::new();
            if !source.trim().is_empty() {
                image.set_file(&source).map_err(|err|Error::runtime(format!(
                    "failed to load nested Image source {source:?}: {err}")))?;
            }
            if engine.has_explicit(node,"width") || engine.has_explicit(node,"height") {
                let intrinsic=image.dimensions().unwrap_or((24,24));
                let width=if engine.has_explicit(node,"width") {
                    engine.property(node,"width")?.number()?.round().clamp(1.,4096.) as u32
                } else {intrinsic.0};
                let height=if engine.has_explicit(node,"height") {
                    engine.property(node,"height")?.number()?.round().clamp(1.,4096.) as u32
                } else {intrinsic.1};
                image.set_size(width,height);
            }
            Ok(Some(image.into()))
        },
        "Text" => {
            let text=Text::new(engine.property(node,"text")?.display());
            if engine.has_explicit(node,"fontSize") {
                text.set_font_size(engine.property(node,"fontSize")?.number()?.max(1.) as f32);
            }
            if engine.has_explicit(node,"fontFamily") {
                text.set_font_family(engine.property(node,"fontFamily")?.display());
            }
            Ok(Some(text.into()))
        },
        "Row" | "Column" | "Item" => {
            let group=if engine.kind(node)=="Column" {ContentGroup::column(6)}
                else if engine.kind(node)=="Row" {ContentGroup::row(6)}
                else {ContentGroup::overlay()};
            for &child in engine.children(node) {
                if let Some(content)=nested_button_content(engine,child,depth+1)? {group.append(content);}
            }
            Ok(Some(group.into()))
        },
        other=>Err(Error::runtime(format!(
            "unsupported {other} child inside Button; use Image, Text, Row, Column or Item"))),
    }
}

fn button_content(engine:&Engine,node:usize)->Result<WidgetContent> {
    let mut children=Vec::new();
    for &child in engine.children(node) {
        if let Some(content)=nested_button_content(engine,child,0)? {children.push(content);}
    }
    let text=engine.property(node,"text")?.display();
    if !text.is_empty() {children.push(Text::new(text).into());}
    if children.is_empty() {return Ok(WidgetContent::Empty);}
    if children.len()==1 {return Ok(children.remove(0));}
    let group=ContentGroup::row(6);
    for content in children {group.append(content);}
    Ok(group.into())
}

fn button_state(engine:&Engine,node:usize)->Result<(String,WidgetContent)> {
    Ok((button_signature(engine,node)?,button_content(engine,node)?))
}

/// Native, retained, hot-updating projection of a MIVL engine.
/// `engine` can be mutated programmatically between calls to `sync()`.
/// Widget input updates `engine` and re-evaluates all affected bindings.
///
/// Only types listed in `supported_type` are rendered, unsupported ones
/// error explicitly so missing controls do not silently disappear.
pub struct NativeApp {
    pub engine: Engine,
    window: WidgetWindow,
    visuals: Vec<Visual>,
    buttons: BTreeMap<u64, usize>,
    fields: BTreeMap<u64, usize>,
    events: Vec<WidgetEvent>,
    last_rects: Vec<Rect>,
    last_visible: Vec<bool>,
    last_enabled: Vec<bool>,
    last_title: String,
    generation: u64,
    /// Last engine state that was projected onto Mata's retained widgets.
    last_revision: u64,
    /// Keeps input/hover animations smooth, but allows static windows to idle.
    last_activity: Instant,
    view_controls: Vec<(usize, Button, Button, Option<bool>, Option<bool>)>,
    scroll_actions: BTreeMap<u64, (usize, i8)>,
    spare_visuals: Vec<Visual>,
    spare_page_controls: Vec<(Button, Button)>,
    gestures: BTreeMap<String, Gesture>,
    flings: BTreeMap<usize,Fling>,
    dragged_this_pump: bool,
    bars: Vec<Scrollbar>,
    spare_bars: Vec<Scrollbar>,
    bar_drags: BTreeMap<String,(usize,bool)>,
    // Persistent font caches prevent filesystem rescans and repeated glyph
    // decoding during scrolling, resizing, and reactive property updates.
    font_metrics: BTreeMap<String, Option<FontRenderer>>,
    measured_text: BTreeMap<(String,u16,u32,String), (u32,u32)>,
    glyph_advances: BTreeMap<(String,u16,char),u32>,
    text_layouts: BTreeMap<usize,TextLayout>,
}
impl NativeApp {
    pub fn new(engine: Engine) -> Result<Self> {
        for i in 0..engine.count() {
            if engine.kind(i)=="Image" && !within_button(&engine,i) {
                return Err(Error::runtime("MIVL native Image currently requires a Button ancestor"));
            }
            if !supported_type(engine.kind(i)) {
                return Err(Error::runtime(format!("unsupported native component '{}', node {i}", engine.kind(i))));
            }
        }
        let bounds = engine.layout()?;
        let title = engine.property(0, "title")?.display();
        let window = WidgetWindow::new(&title, (bounds[0].width.max(1), bounds[0].height.max(1)))
            .map_err(|e| Error::runtime(e.to_string()))?;
        let mut app = Self {
            engine, window, visuals: Vec::new(),
            buttons: BTreeMap::new(), fields: BTreeMap::new(), events: Vec::with_capacity(16),
            last_rects: Vec::new(), last_visible: Vec::new(), last_enabled: Vec::new(),
            last_title: title, generation: 0, last_revision: u64::MAX,
            last_activity: Instant::now(), view_controls: Vec::new(), scroll_actions: BTreeMap::new(),
            spare_visuals: Vec::new(), spare_page_controls: Vec::new(),
            gestures: BTreeMap::new(), flings: BTreeMap::new(), dragged_this_pump: false,
            bars: Vec::new(), spare_bars: Vec::new(), bar_drags: BTreeMap::new(),
            font_metrics: BTreeMap::new(), measured_text: BTreeMap::new(),
            glyph_advances: BTreeMap::new(),text_layouts: BTreeMap::new(),
        };
        app.refresh_text_metrics()?;
        app.rebuild_visuals()?;
        app.sync()?;
        app.engine.emit_at(0,"completed")?;
        app.sync()?;
        Ok(app)
    }
    /// Read glyph advances from exactly the same font renderer used to paint
    /// the text. Reusing this cache avoids rescanning font files on scrolling.
    fn advance(&mut self, family:&str, pt:u16, line:u32, ch:char) -> u32 {
        let key=(family.to_owned(),pt,ch);
        if let Some(&width)=self.glyph_advances.get(&key) {return width;}
        if !self.font_metrics.contains_key(family) {
            self.font_metrics.insert(family.to_owned(),
                FontRenderer::system_family(family,FontStyle::Normal,FontWeight::Normal).ok());
        }
        let measured=self.font_metrics.get_mut(family).and_then(Option::as_mut)
            .and_then(|font|font.measure_styled(&ch.to_string(),pt,FontVariant::Normal,
                FontWeight::Normal,FontStretch::Normal,Some(line)).ok())
            .map(|(w,_)|w);
        let width=measured.unwrap_or_else(||(pt as f64*if ch==' ' {0.35}else{0.68}).ceil() as u32);
        if self.glyph_advances.len()>=8192 {self.glyph_advances.clear();}
        self.glyph_advances.insert(key,width);
        width
    }
    /// Run the same line-breaking algorithm as the platform-free runtime,
    /// substituting Mata's actual font advances for approximate widths.
    fn refresh_text_metrics(&mut self) -> Result<()> {
        let mut resolved = BTreeMap::new();
        let mut wrapped = BTreeMap::new();
        let mut layouts = BTreeMap::new();
        for index in 0..self.engine.count() {
            if !matches!(self.engine.kind(index), "Text" | "Button" | "TextField") {continue;}
            let value=self.engine.property(index,"text")?.display();
            let family=self.engine.property(index,"fontFamily")?.display();
            let pt=self.engine.property(index,"fontSize")?.number()?.round()
                .clamp(1.0,u16::MAX as f64) as u16;
            let step=(self.engine.property(index,"lineHeight")?.number()?.max(0.5)*pt as f64)
                .ceil().clamp(1.0,u32::MAX as f64) as u32;
            if self.engine.kind(index)!="Text" {
                let key=(family.clone(),pt,step,value.clone());
                let size=if let Some(&size)=self.measured_text.get(&key){Some(size)}else{
                    if !self.font_metrics.contains_key(&family) {
                        self.font_metrics.insert(family.clone(),
                            FontRenderer::system_family(&family,FontStyle::Normal,FontWeight::Normal).ok());
                    }
                    let size=self.font_metrics.get_mut(&family).and_then(Option::as_mut)
                        .and_then(|font|font.measure_styled(&value,pt,FontVariant::Normal,
                            FontWeight::Normal,FontStretch::Normal,Some(step)).ok())
                        .map(|(w,h)|(w.saturating_add(3),h.max(step)));
                    if let Some(size)=size {
                        if self.measured_text.len()>=1024 {self.measured_text.clear();}
                        self.measured_text.insert(key,size);
                    }
                    size
                };
                if let Some(size)=size {resolved.insert(index,size);}
                continue;
            }
            let mode=WrapMode::from_properties(self.engine.property(index,"wrap")?.truthy(),
                &self.engine.property(index,"wrapMode")?.display());
            let overflow=self.engine.property(index,"textOverflow")?.display()=="ellipsis";
            let count=self.engine.property(index,"maxLines")?.number()?.max(0.0) as usize;
            let mut max_lines=if count==0 {None}else{Some(count)};
            let property_width=self.engine.property(index,"width")?;
            let property_height=self.engine.property(index,"height")?;
            let (mut pl,mut pt_top,mut pr,mut pb)=(0u32,0u32,0u32,0u32);
            let default_pad=if self.engine.has_explicit(index,"borderPadding") {"borderPadding"} else {"padding"};
            for (key,out) in [("paddingLeft",&mut pl),("paddingTop",&mut pt_top),
                ("paddingRight",&mut pr),("paddingBottom",&mut pb)] {
                let selected=if self.engine.has_explicit(index,key) {key}else{default_pad};
                *out=self.engine.property(index,selected)?.number()?.max(0.0) as u32;
            }
            let border=self.engine.property(index,"borderWidth")?.number()?.max(0.0) as u32;
            let chrome_w=pl.saturating_add(pr).saturating_add(border.saturating_mul(2));
            let chrome_h=pt_top.saturating_add(pb).saturating_add(border.saturating_mul(2));
            // `auto` width has no wrapping constraint, unless maxWidth caps it.
            let outer_width=if property_width.display()=="auto" {
                if self.engine.has_explicit(index,"maxWidth") {
                    Some(self.engine.property(index,"maxWidth")?.number()?.max(0.0) as u32)
                } else {None}
            } else {Some(property_width.number()?.max(0.0) as u32)};
            let min_width=self.engine.property(index,"minWidth")?.number()?.max(0.0) as u32;
            let max_width=self.engine.property(index,"maxWidth")?.number()?.max(0.0) as u32;
            let outer_width=outer_width.map(|w|w.max(min_width).min(max_width.max(min_width)));
            let constraint=if mode==WrapMode::None && !overflow {None}
                else {outer_width.map(|w|w.saturating_sub(chrome_w))};
            if overflow && max_lines.is_none() && property_height.display()!="auto" {
                let inner=property_height.number()?.max(0.0) as u32;
                max_lines=Some((inner.saturating_sub(chrome_h)/step.max(1)).max(1) as usize);
            }
            let natural=layout_text(&value,None,WrapMode::None,step,max_lines,overflow,
                |c|self.advance(&family,pt,step,c));
            resolved.insert(index,(natural.width.saturating_add(3),natural.height));
            let layout=if constraint.is_some() {
                layout_text(&value,constraint,mode,step,max_lines,overflow,
                    |c|self.advance(&family,pt,step,c))
            }else{natural};
            if let Some(w)=constraint {
                wrapped.insert(index,(w,(layout.width.saturating_add(3),layout.height)));
            }
            layouts.insert(index,layout);
        }
        self.text_layouts=layouts;
        self.engine.set_native_text_metrics(resolved,wrapped);
        Ok(())
    }
    /// Reconcile model changes while retaining existing controls and content roots.
    /// Unused visuals are hidden and pooled, avoiding the retired-controls
    /// accumulation that repeatedly re-registering widgets would cause.
    fn rebuild_visuals(&mut self) -> Result<()> {
        let mut old = std::mem::take(&mut self.visuals).into_iter().map(Some).collect::<Vec<_>>();
        let previous_views = std::mem::take(&mut self.view_controls);
        self.buttons.clear(); self.fields.clear(); self.scroll_actions.clear();
        let bounds = self.engine.layout()?;
        self.last_rects = vec![Rect::new(i32::MIN, i32::MIN, 0, 0);bounds.len()];
        self.last_visible = vec![false;bounds.len()];
        self.last_enabled = vec![true;bounds.len()];
        for (i,bounds) in bounds.iter().copied().enumerate() {
            let kind = self.engine.kind(i);
            if kind=="Image" && !within_button(&self.engine,i) {
                return Err(Error::runtime("MIVL native Image currently requires a Button ancestor"));
            }
            if !supported_type(kind) {
                return Err(Error::runtime(format!("unsupported native component '{}'",kind)));
            }
            let rect = to_rect(bounds);
            // Nested button content is rendered by the parent Button's retained
            // ContentPainter, never as an independent window overlay.
            if within_button(&self.engine,i) {
                self.visuals.push(Visual::Container);
                continue;
            }
            let visual = old.get_mut(i).filter(|v|v.as_ref().is_some_and(|v|visual_matches(v,kind)))
                .and_then(Option::take)
                .or_else(|| old.iter_mut().find(|v| v.as_ref().is_some_and(|v|visual_matches(v,kind))).and_then(Option::take))
                .or_else(|| self.spare_visuals.iter().position(|v|visual_matches(v,kind)).map(|at|self.spare_visuals.swap_remove(at)));
            let visual = match visual {
                Some(mut visual) => {
                    if let Visual::Label {stamp,..}= &mut visual {stamp.clear();}
                    visual
                },
                None => match kind {
                    "Text" => {
                        let area=DrawingArea::new(rect.width.max(1),rect.height.max(1));
                        let content=area.renderable_content().ok_or_else(||Error::runtime("Text surface unavailable"))?;
                        let id=self.window.add_content(rect,content);
                        Visual::Label {area,id,stamp:String::new()}
                    }
                    "Rectangle" => {
                        let color = parse_color(&self.engine.property(i,"color")?.display())?;
                        let area = DrawingArea::new(rect.width.max(1),rect.height.max(1));
                        area.framebuffer().lock().map_err(|_|Error::runtime("surface lock poisoned"))?.clear(color);
                        let content = area.renderable_content().ok_or_else(||Error::runtime("DrawingArea has no content"))?;
                        let id = self.window.add_content(rect,content);
                        Visual::Surface {area,id,color,style:String::new()}
                    }
                    "Button" => {
                        let (value,child) = button_state(&self.engine,i)?;
                        let control = Button::new(&self.window,rect,child).map_err(|e|Error::runtime(e.to_string()))?;
                        Visual::Button {control,value}
                    }
                    "TextField" => {
                        let value = self.engine.property(i,"text")?.display();
                        let control = TextField::new(&self.window,rect,&value).map_err(|e|Error::runtime(e.to_string()))?;
                        Visual::TextField {control}
                    }
                    _ => Visual::Container,
                },
            };
            match &visual {
                Visual::Button {control,..} => {self.buttons.insert(control.id().get(),i);}
                Visual::TextField {control} => {self.fields.insert(control.id().get(),i);}
                _ => {},
            }
            self.visuals.push(visual);
        }
        for visual in old.into_iter().flatten() {
            if matches!(visual,Visual::Container) {continue;}
            park_visual(&self.window,&visual);
            // Keep controls in the pool: dropping a Button/TextField handle
            // does not deregister it from WidgetWindow's retained input host.
            self.spare_visuals.push(visual);
        }
        let mut previous_views = previous_views.into_iter().map(Some).collect::<Vec<_>>();
        for i in 0..self.engine.count() {
            if self.engine.is_scrollable(i)? || matches!(self.engine.kind(i),"StackView" | "SwipeView") {
                let reused = previous_views.iter_mut().find(|v|v.as_ref().is_some_and(|(old,..)|*old==i)).and_then(Option::take);
                let (up,down) = if let Some((_,up,down,..)) = reused {(up,down)}
                    else if let Some(pair) = self.spare_page_controls.pop() {pair}
                    else {
                        (Button::new(&self.window,Rect::new(0,0,0,0),"▲").map_err(|e|Error::runtime(e.to_string()))?,
                         Button::new(&self.window,Rect::new(0,0,0,0),"▼").map_err(|e|Error::runtime(e.to_string()))?)
                    };
                self.scroll_actions.insert(up.id().get(),(i,-1));
                self.scroll_actions.insert(down.id().get(),(i,1));
                self.view_controls.push((i,up,down,None,None));
            }
        }
        for (_,up,down,..) in previous_views.into_iter().flatten() {
            up.set_visible(false);down.set_visible(false);
            self.spare_page_controls.push((up,down));
        }
        // Reuse retained framebuffer scrollbar surfaces across virtualization.
        let mut old_bars=std::mem::take(&mut self.bars);
        for i in 0..self.engine.count() {
            if !self.engine.is_scrollable(i)? {continue;}
            let bar=if let Some(at)=old_bars.iter().position(|b|b.view==i) {
                old_bars.swap_remove(at)
            } else if let Some(bar)=self.spare_bars.pop() {bar} else {
                let vertical=DrawingArea::new(1,1);
                let horizontal=DrawingArea::new(1,1);
                let vertical_id=self.window.add_content(Rect::new(0,0,0,0),
                    vertical.renderable_content().ok_or_else(||Error::runtime("missing scrollbar surface"))?);
                let horizontal_id=self.window.add_content(Rect::new(0,0,0,0),
                    horizontal.renderable_content().ok_or_else(||Error::runtime("missing scrollbar surface"))?);
                Scrollbar {view:i,vertical,vertical_id,horizontal,horizontal_id,
                    vertical_bounds:Rect::new(0,0,0,0),horizontal_bounds:Rect::new(0,0,0,0),
                    v_thumb:(0,0),h_thumb:(0,0),v_stamp:String::new(),h_stamp:String::new()}
            };
            self.bars.push(Scrollbar {view:i,..bar});
        }
        for bar in old_bars {
            self.window.set_content_bounds(bar.vertical_id,Rect::new(0,0,0,0));
            self.window.set_content_bounds(bar.horizontal_id,Rect::new(0,0,0,0));
            self.spare_bars.push(bar);
        }
        self.generation = self.engine.generation();
        Ok(())
    }
    /// Pull current properties, clipped bounds and page visibility into retained Mata widgets.
    pub fn sync(&mut self) -> Result<()> {
        self.refresh_text_metrics()?;
        if self.generation != self.engine.generation() || self.visuals.len() != self.engine.count() {
            self.rebuild_visuals()?;
        }
        let bounds = self.engine.layout()?;
        let visible_bounds = self.engine.visible_layout(&bounds)?;
        let title = self.engine.property(0,"title")?.display();
        if title != self.last_title {self.window.set_title(&title);self.last_title=title;}
        self.window.set_background(parse_color(&self.engine.property(0,"color")?.display())?);
        for (i,visual) in self.visuals.iter_mut().enumerate() {
            let visible = visible_bounds[i].is_some();
            let rect = visible_bounds[i].map(to_rect).unwrap_or_else(||Rect::new(0,0,0,0));
            let rect_changed = self.last_rects[i] != rect;
            self.last_rects[i] = rect;
            match visual {
                Visual::Container => {},
                Visual::Label {area,id,stamp} => {
                    let source=bounds[i]; // UNCLIPPED: never move glyph origin when scrolling.
                    let layout=self.text_layouts.get(&i);
                    let value=self.engine.property(i,"text")?.display();
                    let family=self.engine.property(i,"fontFamily")?.display();
                    let pt=self.engine.property(i,"fontSize")?.number()?.round()
                        .clamp(1.0,u16::MAX as f64) as u16;
                    let raw_color=parse_color(&self.engine.property(i,"color")?.display())?;
                    let opacity=self.engine.property(i,"opacity")?.number()?.clamp(0.0,1.0);
                    let alpha=((raw_color>>24) as f64*opacity).round() as u32;
                    let color=((alpha.min(255))<<24)|(raw_color&0x00ff_ffff);
                    let align=self.engine.property(i,"textAlign")?.display();
                    let valign=self.engine.property(i,"verticalAlign")?.display();
                    let default_pad=if self.engine.has_explicit(i,"borderPadding") {"borderPadding"}else{"padding"};
                    let pad=|key:&str| -> Result<u32> {
                        Ok(self.engine.property(i,if self.engine.has_explicit(i,key){key}else{default_pad})?
                            .number()?.max(0.0) as u32)
                    };
                    let (left,top,right,bottom)=(pad("paddingLeft")?,pad("paddingTop")?,
                        pad("paddingRight")?,pad("paddingBottom")?);
                    let border=self.engine.property(i,"borderWidth")?.number()?.max(0.0) as u32;
                    let left=left.saturating_add(border);let right=right.saturating_add(border);
                    let top=top.saturating_add(border);let bottom=bottom.saturating_add(border);
                    let inner_w=source.width.saturating_sub(left.saturating_add(right));
                    let inner_h=source.height.saturating_sub(top.saturating_add(bottom));
                    let signature=format!("{value:?}/{family}/{pt}/{color}/{align}/{valign}/{:?}/{:?}/{:?}/{:?}/{:?}",
                        source,rect,(left,top,right,bottom),layout.map(|l|(&l.lines,l.line_height,l.truncated)),visible);
                    let surface=area.framebuffer();
                    let mut fb=surface.lock().map_err(|_|Error::runtime("text surface lock poisoned"))?;
                    let resized=fb.width!=rect.width.max(1)||fb.height!=rect.height.max(1);
                    if resized {fb.resize(rect.width.max(1),rect.height.max(1));}
                    if resized||*stamp!=signature {
                        fb.clear(0);
                        if visible {
                            if let (Some(layout),Some(font))=(layout,self.font_metrics.get_mut(&family).and_then(Option::as_mut)) {
                                let content_left=source.x.saturating_add(left as i32);
                                let content_top=source.y.saturating_add(top as i32);
                                let shift_y=match valign.as_str() {
                                    "center" => inner_h.saturating_sub(layout.height)/2,
                                    "bottom"|"end" => inner_h.saturating_sub(layout.height),
                                    _ => 0,
                                } as i32;
                                let clip=Rect::new(0,0,rect.width,rect.height);
                                for (line_no,line) in layout.lines.iter().enumerate() {
                                    let step=(line_no as i64*layout.line_height as i64).min(i32::MAX as i64) as i32;
                                    let y=content_top.saturating_add(shift_y).saturating_add(step).saturating_sub(rect.y);
                                    if y.saturating_add(layout.line_height as i32)<=0 || y>=rect.height as i32 {continue;}
                                    let shift_x=match align.as_str() {
                                        "center" => inner_w.saturating_sub(line.width)/2,
                                        "right"|"end" => inner_w.saturating_sub(line.width),
                                        _ => 0,
                                    } as i32;
                                    let x=content_left.saturating_add(shift_x).saturating_sub(rect.x);
                                    font.draw_text_styled(&mut fb,&line.text,x,y,pt,color,Some(clip),
                                        FontStyle::Normal,FontVariant::Normal,FontWeight::Normal,
                                        FontStretch::Normal,Some(layout.line_height))
                                        .map_err(|e|Error::runtime(e.to_string()))?;
                                }
                            }
                        }
                        *stamp=signature;
                        self.window.request_redraw();
                    }
                    drop(fb);
                    if rect_changed {self.window.set_content_bounds(*id,rect);}
                }
                Visual::Surface {area,id,color,style} => {
                    let wanted=parse_color(&self.engine.property(i,"color")?.display())?;
                    let border=parse_color(&self.engine.property(i,"borderColor")?.display())?;
                    let border_width=self.engine.property(i,"borderWidth")?.number()?.max(0.0) as u32;
                    let radius=if self.engine.has_explicit(i,"border.radius") {
                        self.engine.property(i,"border.radius")?.number()?.max(0.0) as u32
                    } else {self.engine.property(i,"borderRadius")?.number()?.max(0.0) as u32};
                    let radii=self.engine.property(i,"borderRadii")?.number()?.max(0.0) as u32;
                    let corner=|name:&str| -> Result<u32> {
                        let value=self.engine.property(i,name)?.number()?.max(0.0) as u32;
                        Ok(if self.engine.has_explicit(i,name) {value} else if self.engine.has_explicit(i,"borderRadii") {radii} else {radius})
                    };
                    let corners=[corner("borderTopLeftRadius")?,corner("borderTopRightRadius")?,
                        corner("borderBottomRightRadius")?,corner("borderBottomLeftRadius")?];
                    let stamp=format!("{wanted}/{border}/{border_width}/{corners:?}");
                    let surface=area.framebuffer();
                    let mut buffer=surface.lock().map_err(|_|Error::runtime("surface lock poisoned"))?;
                    let changed=buffer.width!=rect.width.max(1) || buffer.height!=rect.height.max(1);
                    if changed {buffer.resize(rect.width.max(1),rect.height.max(1));}
                    if changed || *style!=stamp {
                        draw_rounded(&mut buffer,wanted,border,border_width,corners);
                        *style=stamp;
                        self.window.request_redraw();
                    }
                    *color=wanted;
                    drop(buffer);
                    if rect_changed {self.window.set_content_bounds(*id,rect);}
                }
                Visual::Button {control,value} => {
                    let wanted=button_signature(&self.engine,i)?;
                    if *value != wanted {
                        control.set_child(button_content(&self.engine,i)?);
                        *value=wanted;
                    }
                    if control.bounds() != rect {control.set_bounds(rect);}
                    if self.last_visible[i] != visible {control.set_visible(visible);}
                    let enabled = self.engine.property(i,"enabled")?.truthy();
                    if self.last_enabled[i] != enabled {control.set_enabled(enabled);}
                }
                Visual::TextField {control} => {
                    let wanted = self.engine.property(i,"text")?.display();
                    if control.text() != wanted {control.set_text(&wanted);}
                    if control.bounds() != rect {control.set_bounds(rect);}
                    if self.last_visible[i] != visible {control.set_visible(visible);}
                    let enabled = self.engine.property(i,"enabled")?.truthy();
                    if self.last_enabled[i] != enabled {control.set_enabled(enabled);}
                }
            }
            self.last_visible[i] = visible;
            self.last_enabled[i] = self.engine.property(i,"enabled")?.truthy();
        }
        for (i,previous,next,old_prev,old_next) in &mut self.view_controls {
            let r = bounds[*i];
            let (show_previous,show_next) = if self.engine.is_scrollable(*i)? {
                let (_,extent) = self.engine.content_extent(*i,&bounds)?;
                let offset = self.engine.property(*i,"contentY")?.number()?;
                (offset > 0.0, offset + (r.height as f64) < extent as f64)
            } else {
                let selected = self.engine.property(*i,"currentIndex")?.number()? as isize;
                (selected > 0, selected + 1 < self.engine.children(*i).len() as isize)
            };
            let visible = visible_bounds[*i].is_some() && r.width >= 28 && r.height >= 66;
            let x = r.x.saturating_add(r.width as i32 - 28);
            let upper = Rect::new(x,r.y,28,30);
            let lower = Rect::new(x,r.y.saturating_add(r.height as i32-30),28,30);
            if previous.bounds() != upper {previous.set_bounds(upper);}
            if next.bounds() != lower {next.set_bounds(lower);}
            let arrows = !self.engine.is_scrollable(*i)? || self.engine.property(*i,"scrollbarArrows")?.truthy();
            let previous_visible = visible && arrows && show_previous;
            let next_visible = visible && arrows && show_next;
            if *old_prev != Some(previous_visible) {
                previous.set_visible(previous_visible);
                *old_prev = Some(previous_visible);
            }
            if *old_next != Some(next_visible) {
                next.set_visible(next_visible);
                *old_next = Some(next_visible);
            }
        }
        for bar in &mut self.bars {
            let i=bar.view;
            let box_rect=visible_bounds[i].unwrap_or(bounds[i]);
            let (cw,ch)=self.engine.content_extent(i,&bounds)?;
            let show=visible_bounds[i].is_some() && self.engine.property(i,"scrollbarPolicy")?.display()!="hidden";
            let policy=self.engine.property(i,"scrollbarPolicy")?.display();
            let overflow=self.engine.property(i,"overflow")?.display();
            let width=self.engine.property(i,"scrollbarWidth")?.number()?.clamp(4.0,32.0) as u32;
            let vertical=show && self.engine.property(i,"overflowY")?.display()!="hidden"
                && box_rect.height>0 && (ch>box_rect.height || policy=="always" || overflow=="scroll");
            let horizontal=show && self.engine.property(i,"overflowX")?.display()!="hidden"
                && box_rect.width>0 && (cw>box_rect.width || policy=="always" || overflow=="scroll");
            let v_track=Rect::new(box_rect.x.saturating_add(box_rect.width as i32-width as i32),box_rect.y,
                if vertical {width} else {0}, if vertical {box_rect.height.saturating_sub(if horizontal {width} else {0})} else {0});
            let h_track=Rect::new(box_rect.x,box_rect.y.saturating_add(box_rect.height as i32-width as i32),
                if horizontal {box_rect.width.saturating_sub(if vertical {width} else {0})} else {0},if horizontal {width} else {0});
            bar.vertical_bounds=v_track;
            bar.horizontal_bounds=h_track;
            let thumb=|track:u32,viewport:u32,content:u32,offset:f64| {
                let length=((track as u64*viewport as u64)/(content.max(1) as u64)).clamp(20,track.max(20) as u64) as u32;
                let length=length.min(track);
                let free=track.saturating_sub(length);
                let start=(offset/(content.saturating_sub(viewport).max(1) as f64)*free as f64)
                    .clamp(0.0,free as f64).round() as u32;
                (start,length)
            };
            bar.v_thumb=thumb(v_track.height,box_rect.height,ch,self.engine.property(i,"contentY")?.number()?);
            bar.h_thumb=thumb(h_track.width,box_rect.width,cw,self.engine.property(i,"contentX")?.number()?);
            let track_color=parse_color(&self.engine.property(i,"scrollbarTrackColor")?.display())?;
            let thumb_color=parse_color(&self.engine.property(i,"scrollbarColor")?.display())?;
            for (surface,id,r,(at,len),vertical_axis,stamp) in [
                (&bar.vertical,bar.vertical_id,v_track,bar.v_thumb,true,&mut bar.v_stamp),
                (&bar.horizontal,bar.horizontal_id,h_track,bar.h_thumb,false,&mut bar.h_stamp)] {
                let wanted_stamp=format!("{}:{}:{}:{}:{}:{}:{}:{}:{}",r.x,r.y,r.width,r.height,at,len,track_color,thumb_color,vertical_axis);
                if *stamp == wanted_stamp {continue;}
                *stamp=wanted_stamp;
                let framebuffer=surface.framebuffer();
                let mut fb=framebuffer.lock().map_err(|_|Error::runtime("scrollbar surface lock poisoned"))?;
                fb.resize(r.width.max(1),r.height.max(1));
                fb.clear(track_color);
                if r.width>0 && r.height>0 {
                    for y in 0..r.height as usize {for x in 0..r.width as usize {
                        if (vertical_axis && (y as u32)>=at && (y as u32)<at+len)
                            || (!vertical_axis && (x as u32)>=at && (x as u32)<at+len) {
                            fb.pixels[y*r.width as usize+x]=thumb_color;
                        }
                    }}
                }
                drop(fb);
                self.window.set_content_bounds(id,r);
                self.window.request_redraw();
            }
        }
        self.last_revision = self.engine.revision();
        Ok(())
    }
    /// Project only changed reactive state. Unlike `sync`, this does not
    /// remeasure fonts or recompute geometry for unchanged idle frames.
    fn sync_if_changed(&mut self) -> Result<bool> {
        if self.last_revision != self.engine.revision()
            || self.generation != self.engine.generation()
            || self.visuals.len() != self.engine.count() {
            self.sync()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    /// Pick the innermost viewport under the real, clipped pointer location.
    fn view_at(&self,x:i32,y:i32,allow_pages:bool) -> Result<Option<usize>> {
        let bounds=self.engine.layout()?;
        let visible=self.engine.visible_layout(&bounds)?;
        let mut best=None::<(usize,usize)>;
        for i in 1..self.engine.count() {
            let kind=self.engine.kind(i);
            if !self.engine.is_scrollable(i)? && !(allow_pages && kind=="SwipeView") {continue;}
            let Some(r)=visible[i] else {continue;};
            if (x as i64) < r.x as i64 || (y as i64) < r.y as i64 ||
               (x as i64) >= r.x as i64+r.width as i64 || (y as i64)>=r.y as i64+r.height as i64 {continue;}
            let mut depth=0usize;
            let mut parent=self.engine.parent(i);
            while let Some(index)=parent {depth+=1;parent=self.engine.parent(index);}
            if best.is_none_or(|(d,_)|depth>d) {best=Some((depth,i));}
        }
        Ok(best.map(|(_,i)|i))
    }
    fn move_scrollbar(&mut self,view:usize,vertical:bool,x:i32,y:i32)->Result<()> {
        let Some(bar)=self.bars.iter().find(|bar|bar.view==view) else {return Ok(());};
        let rect=if vertical {bar.vertical_bounds} else {bar.horizontal_bounds};
        let (_,length)=if vertical {bar.v_thumb} else {bar.h_thumb};
        let track=if vertical {rect.height} else {rect.width};
        let pos=if vertical {y.saturating_sub(rect.y)} else {x.saturating_sub(rect.x)};
        let free=track.saturating_sub(length);
        if free==0 {return Ok(());}
        let bounds=self.engine.layout()?;
        let (cw,ch)=self.engine.content_extent(view,&bounds)?;
        let extent=if vertical {ch} else {cw};
        let viewport=if vertical {bounds[view].height} else {bounds[view].width};
        let offset=((pos-length as i32/2) as f64/free as f64)
            .clamp(0.0,1.0)*extent.saturating_sub(viewport) as f64;
        let current=self.engine.property(view,if vertical {"contentY"} else {"contentX"})?.number()?;
        let delta=offset-current;
        self.engine.scroll_at(view,if vertical {0.0} else {delta},if vertical {delta} else {0.0})
    }
    fn pointer_begin(&mut self,id:String,x:i32,y:i32) -> Result<()> {
        // Grab scrollbar tracks before ordinary view panning.
        let contains=|r:Rect|r.width>0 && r.height>0 && (x as i64)>=r.x as i64 && (y as i64)>=r.y as i64
            && (x as i64)<r.x as i64+r.width as i64 && (y as i64)<r.y as i64+r.height as i64;
        let hit=self.bars.iter().rev().find_map(|bar| {
            if contains(bar.vertical_bounds) {Some((bar.view,true))}
            else if contains(bar.horizontal_bounds) {Some((bar.view,false))}
            else {None}
        });
        if let Some((view,vertical))=hit {
            self.bar_drags.insert(id,(view,vertical));
            self.move_scrollbar(view,vertical,x,y)?;
            return Ok(());
        }
        if let Some(view)=self.view_at(x,y,true)? {
            if self.engine.is_scrollable(view)? && !self.engine.property(view,"dragEnabled")?.truthy() {return Ok(());}
            self.flings.remove(&view);
            self.gestures.insert(id,Gesture {view,start:(x,y),last:(x,y),
                last_time:Instant::now(),velocity:(0.0,0.0),dragged:false});
        }
        Ok(())
    }
    fn pointer_move(&mut self,id:&str,x:i32,y:i32) -> Result<()> {
        if let Some(&(view,vertical))=self.bar_drags.get(id) {
            return self.move_scrollbar(view,vertical,x,y);
        }
        let Some(g)=self.gestures.get_mut(id) else {return Ok(());};
        let now=Instant::now();
        let dt=now.duration_since(g.last_time).as_secs_f64().max(0.008);
        let dx=(g.last.0-x) as f64;
        let dy=(g.last.1-y) as f64;
        if (x-g.start.0).abs().max((y-g.start.1).abs())>=8 {g.dragged=true;}
        g.last=(x,y);g.last_time=now;
        g.velocity.0=(0.6*g.velocity.0+0.4*dx/dt).clamp(-2500.0,2500.0);
        g.velocity.1=(0.6*g.velocity.1+0.4*dy/dt).clamp(-2500.0,2500.0);
        let view=g.view;
        if self.engine.kind(view)=="SwipeView" {return Ok(());}
        if g.dragged {self.engine.scroll_at(view,dx,dy)?;}
        Ok(())
    }
    fn pointer_end(&mut self,id:&str,x:i32,y:i32,cancelled:bool) -> Result<()> {
        if self.bar_drags.remove(id).is_some() {self.dragged_this_pump=true;return Ok(());}
        let Some(g)=self.gestures.remove(id) else {return Ok(());};
        if cancelled {return Ok(());}
        if g.dragged {self.dragged_this_pump=true;}
        if self.engine.kind(g.view)=="SwipeView" {
            let delta=x-g.start.0;
            if delta.abs()>=50 && delta.abs()>=(y-g.start.1).abs() {
                let old=self.engine.property(g.view,"currentIndex")?.number()?;
                let count=self.engine.children(g.view).len();
                let target=(old+if delta<0 {1.0} else {-1.0}).clamp(0.0,count.saturating_sub(1) as f64);
                if target!=old {
                    self.engine.set_at(g.view,"currentIndex",target)?;
                    self.engine.emit_at(g.view,"currentIndexChanged")?;
                }
            }
        } else if g.dragged && (g.velocity.0.abs()>110.0 || g.velocity.1.abs()>110.0) {
            self.flings.insert(g.view,Fling {velocity:g.velocity,last_time:Instant::now()});
        }
        Ok(())
    }
    fn apply_flings(&mut self) -> Result<()> {
        let now=Instant::now();
        let mut finished=Vec::new();
        for (&view,fling) in &mut self.flings {
            let dt=now.duration_since(fling.last_time).as_secs_f64().min(0.05);
            fling.last_time=now;
            let (x,y)=(fling.velocity.0*dt,fling.velocity.1*dt);
            let decay=(-dt*8.0).exp();
            fling.velocity.0*=decay;fling.velocity.1*=decay;
            if fling.velocity.0.abs()<20.0 && fling.velocity.1.abs()<20.0 {finished.push(view);}
            // Stop at bounds to avoid pushing against an edge forever.
            let before=(self.engine.property(view,"contentX")?.number()?,self.engine.property(view,"contentY")?.number()?);
            self.engine.scroll_at(view,x,y)?;
            let after=(self.engine.property(view,"contentX")?.number()?,self.engine.property(view,"contentY")?.number()?);
            if before==after {finished.push(view);}
        }
        for i in finished {self.flings.remove(&i);}
        Ok(())
    }
    fn wheel(&mut self,x:i32,y:i32,dx:i32,dy:i32) -> Result<()> {
        if let Some(view)=self.view_at(x,y,false)? {
            // Windows/X11 wheel units are ±120; Wayland/Android may use
            // smaller axis values. Keep small deltas responsive.
            let normalize=|d:i32| if d.abs()>=120 {d as f64/120.0} else {d as f64/40.0};
            let step=self.engine.property(view,"scrollStep")?.number()?.max(0.0);
            self.flings.remove(&view);
            self.engine.scroll_at(view,-normalize(dx)*step,-normalize(dy)*step)?;
        }
        Ok(())
    }
    /// Handle all queued Mata events; return `false` if the native window closed.
    pub fn pump(&mut self) -> Result<bool> {
        let messages = self.engine.pump_messages()?;
        if messages > 0 {
            // Events can regenerate delegates: update widget-to-node mappings
            // before we process any queued native clicks.
            self.sync_if_changed()?;
        }
        self.window.poll_events_into(&mut self.events);
        let events = std::mem::take(&mut self.events);
        self.dragged_this_pump=false;
        // MataWin delivers native touch packets separately from mouse events.
        // Process contact movement before synthesized mouse clicks, so a flick
        // never accidentally activates a button delegate.
        for event in &events {
            if let WidgetEvent::Touched{device_id,phase,x,y}=event {
                match phase {
                    TouchPhase::Started => self.pointer_begin(device_id.clone(),*x,*y)?,
                    TouchPhase::Moved => self.pointer_move(device_id,*x,*y)?,
                    TouchPhase::Ended => self.pointer_end(device_id,*x,*y,false)?,
                    TouchPhase::Cancelled => self.pointer_end(device_id,*x,*y,true)?,
                }
            }
        }
        for event in &events {
            match event {
                WidgetEvent::Close => return Ok(false),
                WidgetEvent::Resized(width,height) => {
                    self.engine.set_at(0,"width",*width as f64)?;
                    self.engine.set_at(0,"height",*height as f64)?;
                    self.engine.emit_at(0,"resized")?;
                }
                WidgetEvent::Scrolled{x,y,dx,dy} => self.wheel(*x,*y,*dx,*dy)?,
                WidgetEvent::PointerPressed(x,y) => self.pointer_begin("mouse".into(),*x,*y)?,
                WidgetEvent::PointerMoved(x,y) => self.pointer_move("mouse",*x,*y)?,
                WidgetEvent::PointerReleased(x,y) => self.pointer_end("mouse",*x,*y,false)?,
                WidgetEvent::Touched{..} => {},
                WidgetEvent::ButtonClicked(id) => {
                    // A drag on a button delegate is not a click.
                    if self.dragged_this_pump || self.gestures.values().any(|g|g.dragged) {continue;}

                    if let Some(&(node,direction)) = self.scroll_actions.get(&id.get()) {
                        if self.engine.is_scrollable(node)? {
                            let step=self.engine.property(node,"scrollStep")?.number()?;
                            self.engine.scroll_at(node,0.0,step*direction as f64)?;
                        } else {
                            let previous = self.engine.property(node,"currentIndex")?.number()?;
                            let length = self.engine.children(node).len();
                            let next = (previous+direction as f64).clamp(0.0,length.saturating_sub(1) as f64);
                            self.engine.set_at(node,"currentIndex",next)?;
                            if previous != next {self.engine.emit_at(node,"currentIndexChanged")?;}
                        }
                    } else if let Some(&node) = self.buttons.get(&id.get()) {
                        let selection = self.engine.view_index_for_child(node);
                        self.engine.emit_at(node,"clicked")?;
                        // A handler may regenerate delegates; the view itself
                        // retains a static index for ordinary model-backed views.
                        if let Some((view,index)) = selection {
                            if view < self.engine.count() && self.engine.kind(view).ends_with("View") {
                                let previous = self.engine.property(view,"currentIndex")?.number()?;
                                if previous != index as f64 {
                                    self.engine.set_at(view,"currentIndex",index as f64)?;
                                    self.engine.emit_at(view,"currentIndexChanged")?;
                                }
                            }
                        }
                    }
                }
                WidgetEvent::TextChanged(id) | WidgetEvent::TextFieldActivated(id) => {
                    if let Some(&node) = self.fields.get(&id.get()) {
                        if let Visual::TextField {control} = &self.visuals[node] {
                            self.engine.set_at(node,"text", control.text())?;
                        }
                        self.engine.emit_at(node, match event { WidgetEvent::TextChanged(_) => "textChanged", _ => "accepted" })?;
                    }
                }
                _ => {},
            }
        }
        if messages > 0 || !events.is_empty() { self.last_activity = Instant::now(); }
        self.events = events;
        self.apply_flings()?;
        if self.sync_if_changed()? { self.last_activity = Instant::now(); }
        // Mata-UI presents a frame only when its dirty flag or a native
        // animation requires one. Polling must not force a GPU submission.
        self.window.present();
        Ok(true)
    }
    /// Main loop. Active gestures/animations get smooth polling; quiet windows
    /// sleep longer and do not recompute layout or submit identical frames.
    /// The maximum idle latency for queued async/Rust events is 50 ms.
    pub fn run(&mut self) -> Result<()> {
        while self.pump()? {
            let active = !self.flings.is_empty() || !self.gestures.is_empty()
                || !self.bar_drags.is_empty();
            thread::sleep(idle_poll_interval(active, self.last_activity.elapsed()));
        }
        Ok(())
    }
}
/// Responsiveness after interaction, then low-overhead idle polling.
/// Kept platform-neutral; does not require Windows-only message APIs.
fn idle_poll_interval(gesture_active: bool, quiet_for: Duration) -> Duration {
    if gesture_active || quiet_for < Duration::from_millis(250) {
        Duration::from_millis(16)
    } else {
        Duration::from_millis(50)
    }
}

fn visual_matches(visual: &Visual, kind: &str) -> bool {
    matches!((visual,kind),
        (Visual::Surface {..},"Rectangle") | (Visual::Label {..},"Text")
        | (Visual::Button {..},"Button") | (Visual::TextField {..},"TextField"))
        || (matches!(visual,Visual::Container) && !matches!(kind,"Rectangle" | "Text" | "Button" | "TextField"))
}
fn park_visual(window: &WidgetWindow, visual: &Visual) {
    match visual {
        Visual::Surface {id,..} | Visual::Label {id,..} => {window.set_content_bounds(*id,Rect::new(0,0,0,0));}
        Visual::Button {control,..} => control.set_visible(false),
        Visual::TextField {control} => control.set_visible(false),
        Visual::Container => {},
    }
}
fn supported_type(name: &str) -> bool {
    matches!(name,"Window" | "Rectangle" | "Item" | "Row" | "Column" | "Text" | "Button" | "TextField"
        | "Image" | "ListView" | "GridView" | "ScrollView" | "Flickable" | "StackView" | "SwipeView" | "EventListener")
}
fn is_scroll_view(name: &str) -> bool {matches!(name,"ListView" | "GridView" | "ScrollView" | "Flickable")}
fn to_rect(r: LayoutRect) -> Rect { Rect::new(r.x,r.y,r.width,r.height) }
/// Color values use #RGB, #RGBA, #RRGGBB or #RRGGBBAA; Mata framebuffers are ARGB32.
pub fn parse_color(color: &str) -> Result<u32> {
    let named = match color.to_ascii_lowercase().as_str() {
        "transparent" => Some(0x00000000), "white" => Some(0xffffffff),
        "black" => Some(0xff000000), "red" => Some(0xffff0000),
        "green" => Some(0xff00ff00), "blue" => Some(0xff0000ff), _ => None,
    };
    if let Some(color) = named { return Ok(color); }
    let hex = color.strip_prefix('#').ok_or_else(|| Error::runtime(format!("invalid color '{color}'")))?;
    let h = |s: &str| u8::from_str_radix(s,16).map_err(|_| Error::runtime("invalid hex color"));
    let (r,g,b,a) = match hex.len() {
        3 | 4 => {
            let value: Vec<u8> = hex.chars().map(|x| u8::from_str_radix(&x.to_string(),16).map(|v|v*17)).collect::<std::result::Result<_,_>>()
                .map_err(|_| Error::runtime("invalid hex color"))?;
            (value[0],value[1],value[2],if value.len()==4 {value[3]} else {255})
        }
        6 => (h(&hex[0..2])?,h(&hex[2..4])?,h(&hex[4..6])?,255),
        8 => (h(&hex[0..2])?,h(&hex[2..4])?,h(&hex[4..6])?,h(&hex[6..8])?),
        _ => return Err(Error::runtime(format!("invalid color '{color}'"))),
    };
    Ok(((a as u32)<<24)|((r as u32)<<16)|((g as u32)<<8)|(b as u32))
}

/// CPU path for four independently rounded corners with an inner border.
/// Rounded shapes are rasterized into the existing Mata framebuffer (ARGB32).
fn draw_rounded(buffer:&mut mata_win::Framebuffer,fill:u32,border:u32,thickness:u32,corners:[u32;4]) {
    let w=buffer.width;
    let h=buffer.height;
    if w==0 || h==0 {return;}
    let thickness=thickness.min(w.min(h)/2);
    let inside=|x:u32,y:u32,width:u32,height:u32,radii:[u32;4]| -> bool {
        if width==0 || height==0 || x>=width || y>=height {return false;}
        let max=(width.min(height)/2) as i64;
        let rs=radii.map(|r|(r as i64).min(max));
        let xi=x as i64;let yi=y as i64;let wi=width as i64;let hi=height as i64;
        for (cx,cy,r,corner) in [
            (rs[0],rs[0],rs[0],xi<rs[0] && yi<rs[0]),
            (wi-rs[1]-1,rs[1],rs[1],xi>=wi-rs[1] && yi<rs[1]),
            (wi-rs[2]-1,hi-rs[2]-1,rs[2],xi>=wi-rs[2] && yi>=hi-rs[2]),
            (rs[3],hi-rs[3]-1,rs[3],xi<rs[3] && yi>=hi-rs[3])
        ] {
            if corner && r>0 {
                let dx=xi-cx;let dy=yi-cy;
                if dx*dx+dy*dy > r*r {return false;}
            }
        }
        true
    };
    let inner_corner=corners.map(|r|r.saturating_sub(thickness));
    for y in 0..h {for x in 0..w {
        let value=if !inside(x,y,w,h,corners) {0}
            else if thickness>0 && (x<thickness || y<thickness || x>=w-thickness || y>=h-thickness ||
                !inside(x-thickness,y-thickness,w-2*thickness,h-2*thickness,inner_corner)) {border}
            else {fill};
        buffer.pixels[(y*w+x) as usize]=value;
    }}
}

#[cfg(test)]
mod text_render_tests {
    use super::*;
    #[test]
    fn a_clipped_framebuffer_keeps_the_full_text_origin() {
        let full=Rect::new(20,40,240,80);
        let visible=Rect::new(20,60,240,50);
        let first_line_y=full.y+8-visible.y;
        assert_eq!(first_line_y,-12);
        // Rendering to a 240x50 DrawingArea with y=-12 clips exactly the
        // top of the first line, not the entire line's original baseline.
    }
    #[test]
    fn right_alignment_uses_unclipped_width() {
        let full=Rect::new(10,5,220,50);
        let visible=Rect::new(60,5,170,50);
        let text_width=140u32;
        let x=full.x+(full.width-text_width) as i32-visible.x;
        assert_eq!(x,30);
    }
}

#[cfg(test)]
mod idle_tests {
    use super::idle_poll_interval;
    use std::time::Duration;
    #[test]
    fn idle_windows_do_not_poll_at_125hz() {
        assert_eq!(idle_poll_interval(false, Duration::from_secs(3)), Duration::from_millis(50));
    }
    #[test]
    fn gesture_and_recent_input_remain_responsive() {
        assert_eq!(idle_poll_interval(true, Duration::from_secs(3)), Duration::from_millis(16));
        assert_eq!(idle_poll_interval(false, Duration::from_millis(80)), Duration::from_millis(16));
    }
}

#[cfg(test)]
mod nested_button_tests {
    use super::*;

    #[test]
    fn image_child_is_retained_inside_button() {
        let engine=Engine::parse(r#"Window {
            Button { id: save; Image { source: ""; width: 24; height: 24 } }
        }"#).unwrap();
        let button=engine.lookup("save").unwrap();
        let (_,child)=button_state(&engine,button).unwrap();
        let WidgetContent::Image(image)=child else {panic!("expected an Image child")};
        assert_eq!(image.preferred_size(),Some((24,24)));
        let image_node=engine.children(button)[0];
        assert!(within_button(&engine,image_node));
    }

    #[test]
    fn mixed_nested_text_and_image_is_grouped() {
        let engine=Engine::parse(r#"Window {
            Button { id: save;
                Image { source: ""; width: 20; height: 20 }
                Text { text: "Save" }
            }
        }"#).unwrap();
        let button=engine.lookup("save").unwrap();
        let (_,child)=button_state(&engine,button).unwrap();
        let WidgetContent::Group(group)=child else {panic!("expected row group")};
        assert_eq!(group.len(),2);
        assert!(engine.children(button).iter().all(|&c|within_button(&engine,c)));
    }
    #[test]
    fn changing_image_source_invalidates_button_content() {
        let mut engine=Engine::parse(r#"Window {
            Button { id: save;
                Image { id: badge; source: "a.png" }
            }
        }"#).unwrap();
        let button=engine.lookup("save").unwrap();
        let before=button_signature(&engine,button).unwrap();
        engine.set("badge.source","b.png").unwrap();
        assert_ne!(before,button_signature(&engine,button).unwrap());
    }
}
