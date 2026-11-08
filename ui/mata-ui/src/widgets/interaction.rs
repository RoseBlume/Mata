//! Widget interaction building blocks shared by native and catalog controls.
//! No additional dependencies; platform accessibility and IME bridges can use
//! these stable in-process models without requiring an OS service at build time.
use super::WidgetId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessibilityRole { Button, Checkbox, Switch, TextField, Label, Image, List, Menu, MenuItem, Region }

#[derive(Debug, Clone)]
pub struct AccessibilityNode {
    pub role: AccessibilityRole,
    pub name: String,
    pub description: Option<String>,
    pub disabled: bool,
    pub checked: Option<bool>,
}
impl AccessibilityNode {
    pub fn new(role:AccessibilityRole,name:impl Into<String>)->Self {
        Self {role,name:name.into(),description:None,disabled:false,checked:None}
    }
    pub fn description(mut self,text:impl Into<String>)->Self {self.description=Some(text.into());self}
    pub fn disabled(mut self,value:bool)->Self {self.disabled=value;self}
    pub fn checked(mut self,value:bool)->Self {self.checked=Some(value);self}
}

/// Semantic palette with fixed, opaque `0xAARRGGBB` entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub window:u32, pub surface:u32, pub input:u32, pub text:u32,
    pub border:u32, pub accent:u32, pub selection:u32,
}
impl Theme {
    pub const fn dark()->Self {Self {window:0xff1f2329,surface:0xff333a45,
        input:0xff22272e,text:0xfff6f7fb,border:0xff707b89,
        accent:0xff779ce9,selection:0xff436aaa}}
    pub const fn light()->Self {Self {window:0xfff5f7fa,surface:0xffe9edf3,
        input:0xffffffff,text:0xff1c2633,border:0xff7e8a98,
        accent:0xff315db5,selection:0xffb4ceff}}
    /// Snapshot the host's system appearance. Use `SystemAppearanceMonitor`
    /// and reapply the theme when the monitor reports a change.
    pub fn system()->Self {
        if super::system_appearance::system_color_scheme().is_some_and(
            super::system_appearance::SystemColorScheme::is_dark)
        { Self::dark() } else { Self::light() }
    }
}

/// Reserved item ID for an automatically generated Copy context menu.
pub const AUTO_COPY_ITEM:u64=u64::MAX;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItem {pub id:u64,pub label:String,pub enabled:bool}
impl MenuItem {
    pub fn new(id:u64,label:impl Into<String>)->Self {Self{id,label:label.into(),enabled:true}}
    pub fn disabled(mut self)->Self {self.enabled=false;self}
}

/// Popup geometry and keyboard selection. Dismissal, hit-testing, and event
/// delivery are owned by WidgetWindow; nothing is an OS-native child window.
#[derive(Debug, Clone)]
pub(crate) struct ContextMenu {
    pub owner: WidgetId,
    pub x:i32,pub y:i32,pub width:u32,pub row_height:u32,
    pub items:Vec<MenuItem>,pub selected:usize,
}
impl ContextMenu {
    pub fn new(owner:WidgetId,x:i32,y:i32,viewport:(u32,u32),items:Vec<MenuItem>)->Self {
        let width=220u32.min(viewport.0);
        let row_height=32;
        let height=(items.len() as u32).saturating_mul(row_height).min(viewport.1);
        let x=x.clamp(0,viewport.0.saturating_sub(width) as i32);
        let y=y.clamp(0,viewport.1.saturating_sub(height) as i32);
        let selected=items.iter().position(|i|i.enabled).unwrap_or(0);
        Self{owner,x,y,width,row_height,items,selected}
    }
    pub fn selected_item(&self)->Option<u64> {self.items.get(self.selected).filter(|i|i.enabled).map(|i|i.id)}
    pub fn item_at(&self,x:i32,y:i32)->Option<u64> {
        if x<self.x || y<self.y || x>=self.x+self.width as i32 {return None;}
        self.items.get(((y-self.y) as u32/self.row_height) as usize).filter(|i|i.enabled).map(|i|i.id)
    }
    pub fn move_selected(&mut self,forward:bool) {
        if self.items.is_empty(){return;}
        for step in 1..=self.items.len() {
            let n=self.items.len();
            let next=if forward {(self.selected+step)%n} else {(self.selected+n-step%n)%n};
            if self.items[next].enabled {self.selected=next;break;}
        }
    }
}

pub(crate) fn fill_rect(fb:&mut crate::Framebuffer,r:super::Rect,color:u32) {
    let x0=(r.x as i64).max(0).min(fb.width as i64) as usize;
    let y0=(r.y as i64).max(0).min(fb.height as i64) as usize;
    let x1=(r.x as i64+r.width as i64).max(0).min(fb.width as i64) as usize;
    let y1=(r.y as i64+r.height as i64).max(0).min(fb.height as i64) as usize;
    for y in y0..y1 {for x in x0..x1 {fb.pixels[y*fb.width as usize+x]=color;}}
}
/// Save only out-of-viewport pixels touched by a retained child control.
/// This keeps interactive controls in scroll views clipped without altering
/// existing control painters. A tighter clip-aware painter can replace it later.
pub(crate) fn capture_outside_clip(fb:&crate::Framebuffer,bounds:super::Rect,
    clip:super::Rect)->Vec<(usize,u32)> {
    let mut saved=Vec::new();
    let x0=(bounds.x as i64).max(0).min(fb.width as i64) as i32;
    let x1=(bounds.x as i64+bounds.width as i64).max(0).min(fb.width as i64) as i32;
    let y0=(bounds.y as i64).max(0).min(fb.height as i64) as i32;
    let y1=(bounds.y as i64+bounds.height as i64).max(0).min(fb.height as i64) as i32;
    for y in y0..y1 {for x in x0..x1 {
        if !clip.contains(x,y) {
            let i=y as usize*fb.width as usize+x as usize;
            saved.push((i,fb.pixels[i]));
        }
    }}
    saved
}
pub(crate) fn restore_pixels(fb:&mut crate::Framebuffer,saved:Vec<(usize,u32)>) {
    for (index,color) in saved {fb.pixels[index]=color;}
}

pub(crate) fn outline(fb:&mut crate::Framebuffer,r:super::Rect,color:u32) {
    if r.width<1||r.height<1{return;}
    fill_rect(fb,super::Rect::new(r.x,r.y,r.width,2),color);
    fill_rect(fb,super::Rect::new(r.x,r.y+r.height as i32-2,r.width,2),color);
    fill_rect(fb,super::Rect::new(r.x,r.y,2,r.height),color);
    fill_rect(fb,super::Rect::new(r.x+r.width as i32-2,r.y,2,r.height),color);
}

/// Production-size renderer for the first interactive catalog controls.
/// Returns true only when the real widget model has a matching visual.
pub(crate) fn paint_live_catalog(fb:&mut crate::Framebuffer,bounds:super::Rect,
    kind:&str,state:&super::catalog::PaintState)->bool {
    let c=|x:i32,y:i32,w:u32,h:u32|super::Rect::new(x,y,w,h);
    let (x,y,w,h)=(bounds.x,bounds.y,bounds.width,bounds.height);
    if w<8||h<8{return false;}
    match kind {
        "ToggleButton" => {
            fill_rect(fb,bounds,if state.active.unwrap_or(false){0xff456fbc}else{0xff444c57});
            outline(fb,bounds,0xff8e96aa);
        }
        "CheckButton" => {
            let box_size=h.min(25).saturating_sub(4);
            let box_y=y+((h-box_size)/2) as i32;
            fill_rect(fb,c(x+4,box_y,box_size,box_size),0xff2b3342);
            outline(fb,c(x+4,box_y,box_size,box_size),0xffc1c8d2);
            if state.active.unwrap_or(false) {
                fill_rect(fb,c(x+8,box_y+4,box_size.saturating_sub(8),box_size.saturating_sub(8)),0xff78abe7);
            }
        }
        "Switch" => {
            let track_h=h.min(28).max(10);
            let track_w=w.min(60).max(20);
            let top=y+((h-track_h)/2) as i32;
            let active=state.active.unwrap_or(false);
            fill_rect(fb,c(x+4,top,track_w,track_h),if active{0xff396fd4}else{0xff535d68});
            let knob=track_h.saturating_sub(8).max(3);
            fill_rect(fb,c(x+if active {track_w as i32-knob as i32} else {8},top+4,knob,knob),0xfff5f7fd);
        }
        "Range"|"Scale"|"ProgressBar"|"LevelBar" => {
            let proportion=state.value.unwrap_or(0.0).clamp(0.0,1.0);
            let height=h.min(14);
            let top=y+((h-height)/2) as i32;
            fill_rect(fb,c(x,top,w,height),0xff414953);
            fill_rect(fb,c(x,top,((w as f64)*proportion) as u32,height),0xff629def);
        }
        _ => return false,
    }
    true
}

pub(crate) fn next_focus(ids:&[WidgetId],current:Option<WidgetId>,backwards:bool)->Option<WidgetId> {
    if ids.is_empty(){return None;}
    match current.and_then(|id|ids.iter().position(|&x|x==id)) {
        Some(i) if backwards=>Some(ids[(i+ids.len()-1)%ids.len()]),
        Some(i)=>Some(ids[(i+1)%ids.len()]),
        None if backwards=>ids.last().copied(),
        None=>ids.first().copied(),
    }
}
pub(crate) fn extend_selection(old:Option<(usize,usize)>,cursor:usize,new:usize)->Option<(usize,usize)> {
    let anchor=old.map(|(a,b)|if cursor==a {b} else {a}).unwrap_or(cursor);
    if anchor==new {None}else{Some((anchor.min(new),anchor.max(new)))}
}

// Minimal Unicode extended grapheme handling for ordinary editing, including
// combining marks, emoji VS-16, skin tones, joined emoji and pairs of flags.
// This is not a substitute for the full UAX #29 segmentation / UAX #9 BiDi
// and language shaping engine. Never split inside UTF-8 code points.
fn extender(c:char)->bool {
    let n=c as u32;
    (0x0300..=0x036f).contains(&n)||(0x1ab0..=0x1aff).contains(&n)
        ||(0x1dc0..=0x1dff).contains(&n)||(0x20d0..=0x20ff).contains(&n)
        ||(0xfe20..=0xfe2f).contains(&n)||(0xfe00..=0xfe0f).contains(&n)
        ||(0x1f3fb..=0x1f3ff).contains(&n)||n==0x200d
}
fn regional(c:char)->bool {(0x1f1e6..=0x1f1ff).contains(&(c as u32))}
pub(crate) fn next_grapheme(text:&str,cursor:usize)->usize {
    if cursor>=text.len(){return text.len();}
    let start=text.char_indices().find(|(i,_)|*i>=cursor).map(|(i,_)|i).unwrap_or(text.len());
    if start==text.len(){return start;}
    let mut iter=text[start..].char_indices();
    let (_,first)=iter.next().unwrap();
    let mut end=start+first.len_utf8();
    let mut ri=if regional(first){1}else{0};
    let mut join_next=false;
    for (offset,c) in iter {
        if extender(c) || join_next || (regional(c) && ri%2==1) {
            if regional(c){ri+=1;}
            join_next=c=='\u{200d}';
            end=start+offset+c.len_utf8();
        } else {break;}
    }
    end
}
pub(crate) fn prev_grapheme(text:&str,cursor:usize)->usize {
    let end=cursor.min(text.len());
    let mut previous=0;
    let mut pos=0;
    while pos<end {
        previous=pos;
        let next=next_grapheme(text,pos);
        if next<=pos{break;}
        pos=next;
    }
    previous
}
pub(crate) fn word_boundary(text:&str,cursor:usize,right:bool)->usize {
    let mut pos=cursor.min(text.len());
    if right {
        let ws=text[pos..].chars().next().is_some_and(char::is_whitespace);
        while pos<text.len() {
            let n=next_grapheme(text,pos);
            let other=text[n..].chars().next().is_some_and(char::is_whitespace);
            pos=n;
            if n==text.len() || (!ws && other){break;}
            if ws && !other {break;}
        }
    } else {
        if pos==0{return 0;}
        pos=prev_grapheme(text,pos);
        let ws=text[pos..].chars().next().is_some_and(char::is_whitespace);
        while pos>0 {
            let n=prev_grapheme(text,pos);
            let other=text[n..].chars().next().is_some_and(char::is_whitespace);
            if other!=ws{break;}
            pos=n;
        }
    }
    pos
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_clusters_are_never_split() {
        let s="a\u{301}🌷🇨🇦👩\u{200d}💻";
        let mut at=0;let mut breaks=Vec::new();
        while at<s.len(){at=next_grapheme(s,at);breaks.push(at)}
        assert_eq!(breaks.len(),4);
        assert_eq!(prev_grapheme(s,breaks[2]),breaks[1]);
    }
    #[test]
    fn selection_anchor_is_preserved(){assert_eq!(extend_selection(Some((1,4)),4,6),Some((1,6)));}
    #[test]
    fn popup_clamps_and_ignores_disabled_items(){
        let id=WidgetId(7);
        let menu=ContextMenu::new(id,400,400,(280,200),vec![MenuItem::new(1,"a").disabled(),MenuItem::new(2,"b")]);
        assert!(menu.x>=0 && menu.y>=0);assert_eq!(menu.selected_item(),Some(2));
    }
    #[test]
    fn theme_colors_are_opaque(){for c in [Theme::dark(),Theme::light()] {assert_eq!(c.text>>24,255)}}
}
