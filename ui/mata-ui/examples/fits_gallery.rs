//! Interactive FITS gallery: pan/zoom, linear/Asinh/log/sqrt stretches,
//! Bayer demosaic, image-cube RGB and assignable R/G/B monochrome channels.
//!
//! Run from the workspace root:
//!   cargo run -p mata-ui --example fits_gallery -- /path/to/fits
//! Default folder: <workspace>/assets_ignored/fits then <workspace>/fits.
//! Built on MataWin's native Window + Framebuffer + FontRenderer APIs.
//!
//! FITS orientation and 80-column cards / 2880-byte HDU blocking are handled
//! in fits_support.rs. Compressed (.fz) FITS is intentionally unsupported.

mod support;
mod fits_support;

use std::{error::Error, fs, path::{Path,PathBuf}, thread, time::{Duration,Instant}};
use mata_ui::{Window,Framebuffer,events::{Key,WindowEvent},widgets::{FontRenderer,Rect}};
use fits_support::{Adjust,Bayer,FilterKind,FitsImage,FitsKind,Mode,Slots,FalseColorSettings,FalseColorPalette,PRESET_COUNT,Stretch,palette_color,read_fits,develop_with_green,generate_bayer_green,save_bmp,bayer_confidence};
use support::{collect_files,workspace_manifest_dir};

const BG:u32=0xff10_141c;
const PANEL:u32=0xff1d_2430;
const BTN:u32=0xff30_3949;
const HOVER:u32=0xff45_546a;
const SELECT:u32=0xff225e_86;
const ACCENT:u32=0xff58_b9dc;
const GHOST:u32=0xff29_3445;
const TEXT:u32=0xffe9_f1fa;
const MUTED:u32=0xffa9_b9c8;
const VIEW_BG:u32=0xff04_070d;
const WIN_SIZE:(u32,u32)=(1510,900);

fn is_fits(p:&Path)->bool {matches!(p.extension().and_then(|v|v.to_str()).map(|v|v.to_ascii_lowercase()).as_deref(),Some("fits"|"fit"|"fts"))}
fn directory()->PathBuf{
    if let Some(arg)=std::env::args_os().nth(1){return PathBuf::from(arg);}
    let root=workspace_manifest_dir();
    let first=root.join("assets_ignored").join("fits");
    if first.is_dir(){return first;}
    let second=root.join("fits");
    if second.is_dir(){second}else{PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/demo_fits")}
}
fn basename(p:&Path)->String {p.file_name().map(|s|s.to_string_lossy().into_owned()).unwrap_or_else(||p.display().to_string())}

/// Match sibling exposures conservatively by common filename stem, image size
/// and object name. This is NOT astrometric registration; alignment must already
/// be supplied by the user (e.g. registered frames from a stacking program).
fn filter_from_filename(path:&Path)->FilterKind {
    let stem=path.file_stem().map(|s|s.to_string_lossy().into_owned()).unwrap_or_default();
    stem.split(|c:char|!c.is_ascii_alphanumeric()).collect::<Vec<_>>().into_iter().rev()
        .map(FilterKind::recognize).find(|k|*k!=FilterKind::Other)
        .unwrap_or(FilterKind::Other)
}
fn group_stem(path:&Path)->String {
    let stem=path.file_stem().map(|s|s.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    stem.split(|c:char|!c.is_ascii_alphanumeric())
        .filter(|s|!s.is_empty() && FilterKind::recognize(s)==FilterKind::Other)
        .collect::<Vec<_>>().join("_")
}
fn scan_filter_group(files:&[PathBuf],idx:usize,current:&mut FitsImage)->(Slots,usize){
    let mut slots=Slots::default();
    if current.kind.is_table() || current.planes.len()!=1{return (slots,0);}
    if current.filter_kind==FilterKind::Other {
        current.filter_kind=filter_from_filename(&files[idx]);
    }
    slots.assign_filter(current,&basename(&files[idx]));
    let mut matched=0;
    let parent=files[idx].parent();
    let stem=group_stem(&files[idx]);
    if stem.is_empty(){return (slots,matched);}
    // Bound disk IO when browsing a directory with thousands of FITS files.
    for (j,path) in files.iter().enumerate() {
        if j==idx || path.parent()!=parent || group_stem(path)!=stem {continue;}
        if matched>=24 {break;}
        let label=basename(path);
        match read_fits(path){Ok(mut frame)=>{
            if frame.planes.len()!=1 || frame.width!=current.width || frame.height!=current.height {continue;}
            if !current.name.is_empty() && !frame.name.is_empty()
                && !frame.name.eq_ignore_ascii_case(&current.name){continue;}
            if frame.filter_kind==FilterKind::Other {frame.filter_kind=filter_from_filename(path);}
            if frame.filter_kind==FilterKind::Other {continue;}
            slots.assign_filter(&frame,&label);
            matched+=1;
        },Err(e)=>eprintln!("Skipping companion {}: {e}",path.display())}
    }
    (slots,matched)
}
fn automatic_mode(img:&FitsImage,slots:&Slots)->Mode {
    if img.kind.is_table(){return Mode::Gray;}
    if let Some(bayer)=img.bayer{return Mode::Bayer(bayer);}
    if img.planes.len()==3 && !img.cube_is_spectral {return Mode::CubeRgb;}
    if slots.ready(){return Mode::AssignedRgb;}
    if slots.ready_sho(){return Mode::Sho;}
    if slots.ready_hoo(){return Mode::Hoo;}
    if (img.filter_kind!=FilterKind::Other && img.filter_kind!=FilterKind::Luminance)
        || img.wavelength_nm.is_some(){
        return Mode::FilterTint;
    }
    if img.planes.len()==1{return Mode::StructureTint;}
    Mode::Gray
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
enum Action {
    Prev,Next,Fit,ZoomIn,ZoomOut,Auto,Mono,FalseColor,Palette,HueDown,HueUp,StructureTint,FilterTint,
    Bayer,CubeRgb,FilterRgb,Sho,Hoo,Stretch,ExposureDown,ExposureUp,GammaDown,GammaUp,
    SatDown,SatUp,Balance,AssignR,AssignG,AssignB,Export,
}
const ACTIONS:[Action;20]=[
    Action::Prev,Action::Next,Action::Fit,Action::ZoomOut,Action::ZoomIn,
    Action::Auto,Action::Mono,Action::FalseColor,Action::StructureTint,Action::FilterTint,
    Action::Bayer,Action::CubeRgb,Action::FilterRgb,Action::Sho,Action::Hoo,
    Action::AssignR,Action::AssignG,Action::AssignB,Action::Balance,Action::Export,
];
#[derive(Clone,Copy)]
struct Button { action:Action, rect:Rect }

/// A ghostbar is a retained value, hitbox, and track. The track is drawn as a
/// translucent/full-range "ghost" behind a filled active portion and draggable
/// thumb; all processing is performed by `Gallery::set_bar`.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
enum BarKind {
    Exposure,Gamma,Black,White,Saturation,Stretch,Background,
    HueShift,ShadowHue,MidHue,HighHue,StarWhite,
    BayerPattern,RedGain,GreenGain,BlueGain,
}
impl BarKind {
    const GENERAL:[Self;7]=[Self::Exposure,Self::Gamma,Self::Black,Self::White,
        Self::Saturation,Self::Stretch,Self::Background];
    const FALSE_COLOR:[Self;5]=[Self::HueShift,Self::ShadowHue,
        Self::MidHue,Self::HighHue,Self::StarWhite];
    const COLOR_CHANNELS:[Self;3]=[Self::RedGain,Self::GreenGain,Self::BlueGain];
    fn name(self)->&'static str {match self {
        Self::Exposure=>"Exposure",Self::Gamma=>"Gamma",Self::Black=>"Black point",
        Self::White=>"White point",Self::Saturation=>"Saturation",
        Self::Stretch=>"Stretch method",Self::Background=>"Sky correction",
        Self::HueShift=>"Palette hue shift",
        Self::ShadowHue=>"Shadows hue",Self::MidHue=>"Midtones hue",
        Self::HighHue=>"Highlights hue",Self::StarWhite=>"Star whitening",
        Self::BayerPattern=>"Bayer pattern",Self::RedGain=>"Red gain",
        Self::GreenGain=>"Green gain",Self::BlueGain=>"Blue gain",
    }}
    fn range(self)->(f32,f32,usize){match self {
        Self::Exposure=>(-6.,6.,0), Self::Gamma=>(0.2,4.,0),
        Self::Black=>(-0.2,0.35,0),Self::White=>(0.5,1.8,0),
        Self::Saturation=>(0.,3.,0), Self::Stretch=>(0.,3.,4),
        Self::Background=>(0.,1.,2),
        Self::HueShift=>(-180.,180.,0),Self::ShadowHue|Self::MidHue|Self::HighHue=>(0.,360.,0),
        Self::StarWhite=>(0.,1.,0),Self::BayerPattern=>(0.,3.,4),
        Self::RedGain|Self::GreenGain|Self::BlueGain=>(0.25,3.,0),
    }}
    fn from_normalized(self,t:f32)->f32{
        let (lo,hi,steps)=self.range();
        let t=t.clamp(0.,1.);
        if steps>1 {(t*(steps-1) as f32).round()} else {lo+(hi-lo)*t}
    }
    fn norm(self,v:f32)->f32 {
        let (lo,hi,steps)=self.range();
        let value=if steps>1{v/(steps-1) as f32}else{(v-lo)/(hi-lo)};
        value.clamp(0.,1.)
    }
}
#[derive(Clone,Copy)]
struct Ghostbar {kind:BarKind,row:Rect,track:Rect}
struct Layout {
    view:Rect, sidebar:Rect, inspector:Rect,status:Rect,title:Rect,dropdown:Rect,popup:Rect,
    buttons:Vec<Button>,bars:Vec<Ghostbar>,panel_viewport:Rect,panel_max_scroll:i32,
}
impl Layout{
    fn new(w:u32,h:u32,mode:Mode,panel_scroll:i32)->Self {
        let mut x=16i32;
        let mut y=61i32;
        let right=w.saturating_sub(16) as i32;
        let mut buttons=Vec::with_capacity(ACTIONS.len());
        for action in ACTIONS {
            let label=action.default_label();
            let bw=((label.len() as u32)*9+22).clamp(64,145);
            if x+bw as i32 > right && x>16 {x=16;y+=42;}
            buttons.push(Button{action,rect:Rect::new(x,y,bw,34)});
            x+=bw as i32+7;
        }
        let top=y+52;
        let status_height=66i32;
        let bottom=(h as i32-status_height-12).max(top+1);
        let body_w=w.saturating_sub(32);
        let left_width=if w>=920 {209}else if w>=750 {154}else{0};
        let right_width=if w>=1050{300}else if w>=710{246}else{w.saturating_sub(32).min(238)};
        let gap=if left_width>0{12}else{0};
        let available=body_w.saturating_sub(left_width+right_width+24+gap);
        let sidebar=Rect::new(16,top,left_width,(bottom-top).max(1) as u32);
        let view=Rect::new(16+left_width as i32+gap as i32,top,available.max(1),(bottom-top).max(1) as u32);
        let inspector=Rect::new((w-16-right_width) as i32,top,right_width,(bottom-top).max(1) as u32);
        let status=Rect::new(16,bottom+9,body_w,(h as i32-bottom-13).max(1) as u32);
        let palette_mode=matches!(mode,Mode::FalseColor);
        let dropdown=Rect::new(inspector.x+12,inspector.y+79,inspector.width.saturating_sub(24),32);
        let panel_top=if palette_mode {122}else{72};
        let panel_viewport=Rect::new(inspector.x+8,inspector.y+panel_top,
            inspector.width.saturating_sub(16),inspector.height.saturating_sub((panel_top+64).max(0) as u32));
        let popup_h=inspector.height.saturating_sub(144).min(440).max(100);
        let popup=Rect::new(inspector.x+9,inspector.y+115,inspector.width.saturating_sub(18),popup_h);
        // Put controls for the selected color mode first. They are immediately
        // visible instead of being buried beneath the global tone controls.
        let mut types=Vec::new();
        if matches!(mode,Mode::FalseColor){types.extend_from_slice(&BarKind::FALSE_COLOR);}
        if matches!(mode,Mode::Bayer(_)) {types.push(BarKind::BayerPattern);types.extend_from_slice(&BarKind::COLOR_CHANNELS);}
        if matches!(mode,Mode::CubeRgb|Mode::AssignedRgb|Mode::Sho|Mode::Hoo) {types.extend_from_slice(&BarKind::COLOR_CHANNELS);}
        types.extend_from_slice(&BarKind::GENERAL);
        let row_h=56;
        let panel_max_scroll=((types.len() as i32)*row_h-(panel_viewport.height as i32)).max(0);
        let scroll=panel_scroll.clamp(0,panel_max_scroll);
        let mut bars=Vec::with_capacity(types.len());
        for (i,kind) in types.into_iter().enumerate(){
            let ry=panel_viewport.y+(i as i32)*row_h-scroll;
            let row=Rect::new(panel_viewport.x,ry,panel_viewport.width,row_h as u32);
            let track=Rect::new(row.x+7,row.y+29,row.width.saturating_sub(14),13);
            bars.push(Ghostbar{kind,row,track});
        }
        Self{view,sidebar,inspector,status,title:Rect::new(18,12,w.saturating_sub(36),37),buttons,bars,dropdown,popup,panel_viewport,panel_max_scroll}
    }
    fn action_at(&self,x:i32,y:i32)->Option<Action>{self.buttons.iter().find(|b|b.rect.contains(x,y)).map(|b|b.action)}
    fn bar_at(&self,x:i32,y:i32)->Option<Ghostbar> {
        if !self.panel_viewport.contains(x,y){return None;}
        self.bars.iter().copied().find(|b|b.row.contains(x,y)
            && b.row.y>=self.panel_viewport.y
            && b.row.y+(b.row.height as i32)<=self.panel_viewport.y+self.panel_viewport.height as i32)
    }
    fn bar_for(&self,kind:BarKind)->Option<Ghostbar>{self.bars.iter().copied().find(|b|b.kind==kind)}
}
impl Action {
    fn default_label(self)->&'static str{
        match self{
            Self::Prev=>"< Prev",Self::Next=>"Next >",Self::Fit=>"Fit",Self::ZoomIn=>"Zoom +",Self::ZoomOut=>"Zoom -",
            Self::Auto=>"Auto color",Self::Mono=>"Mono",Self::FalseColor=>"False color",
            Self::Palette=>"Palette",Self::HueDown=>"Hue -",Self::HueUp=>"Hue +",
            Self::StructureTint=>"Guess (art)",Self::FilterTint=>"Filter tint",
            Self::Bayer=>"Debayer",Self::CubeRgb=>"Cube RGB",Self::FilterRgb=>"Filter RGB",
            Self::Sho=>"SHO",Self::Hoo=>"HOO",Self::Stretch=>"Stretch",
            Self::ExposureDown=>"EV -",Self::ExposureUp=>"EV +",Self::GammaDown=>"Gamma -",Self::GammaUp=>"Gamma +",
            Self::SatDown=>"Saturation -",Self::SatUp=>"Saturation +",Self::Balance=>"Sky balance",
            Self::AssignR=>"Set R",Self::AssignG=>"Set G",Self::AssignB=>"Set B",Self::Export=>"Save BMP",
        }
    }
    fn label(self,g:&Gallery)->String{
        match self{
            Self::Bayer=>match g.mode {Mode::Bayer(b)=>format!("Bayer {}",b.label()),_=>"Debayer".into()},
            Self::Palette=>format!("Palette {}",g.false_color.palette.label()),
            Self::Stretch=>format!("{} stretch",g.adjust.stretch.label()),
            Self::Balance=>format!("Balance {}",if g.adjust.neutralize_background {"ON"}else{"OFF"}),
            Self::ExposureDown|Self::ExposureUp|Self::GammaDown|Self::GammaUp=>self.default_label().into(),
            _=>self.default_label().into(),
        }
    }
    fn highlighted(self,g:&Gallery)->bool {
        matches!((self,g.mode),
            (Self::Mono,Mode::Gray)|(Self::FalseColor,Mode::FalseColor)|
            (Self::Palette,Mode::FalseColor)|(Self::HueDown,Mode::FalseColor)|(Self::HueUp,Mode::FalseColor)|
            (Self::StructureTint,Mode::StructureTint)|(Self::FilterTint,Mode::FilterTint)|
            (Self::Bayer,Mode::Bayer(_))|(Self::CubeRgb,Mode::CubeRgb)|
            (Self::FilterRgb,Mode::AssignedRgb)|(Self::Sho,Mode::Sho)|(Self::Hoo,Mode::Hoo))
    }
}

// The popup only lays out visible rows; 391 palette names never become 391
// retained widgets. Keyboard characters filter the catalog in-place.
#[derive(Default)]
struct PaletteDropdown{open:bool,query:String,first:usize}
impl PaletteDropdown{
    fn filtered(&self)->Vec<usize>{
        let query=self.query.trim().to_ascii_lowercase();
        (0..PRESET_COUNT).filter(|i|query.is_empty()||
            FalseColorPalette::from_index(*i).label().to_ascii_lowercase().contains(&query)).collect()
    }
    fn close(&mut self){self.open=false;self.query.clear();self.first=0;}
    fn scroll(&mut self,delta:i32,shown:usize){
        let len=self.filtered().len();
        self.first=(self.first as i32+delta).clamp(0,len.saturating_sub(shown) as i32) as usize;
    }
}
const POPUP_ROW_H:i32=37;
const POPUP_SEARCH_H:i32=34;
fn popup_rows(layout:&Layout)->usize{
    (layout.popup.height.saturating_sub(POPUP_SEARCH_H as u32)/POPUP_ROW_H as u32).max(1) as usize
}
fn popup_index(g:&Gallery,layout:&Layout,x:i32,y:i32)->Option<usize>{
    if !layout.popup.contains(x,y) || y<layout.popup.y+POPUP_SEARCH_H{return None;}
    let filtered=g.palette_dropdown.filtered();
    let row=((y-layout.popup.y-POPUP_SEARCH_H)/POPUP_ROW_H) as usize;
    if row>=popup_rows(layout){return None;}
    filtered.get(g.palette_dropdown.first+row).copied()
}

#[derive(Clone,Copy)]
struct View{zoom:f64,cx:f64,cy:f64}
impl View {
    fn new(w:usize,h:usize)->Self{Self{zoom:1.,cx:w as f64*0.5,cy:h as f64*0.5}}
    fn scale(self,r:Rect,w:usize,h:usize)->f64{
        let fit=(r.width as f64/w as f64).min(r.height as f64/h as f64);
        fit*self.zoom
    }
    fn clamp(&mut self,r:Rect,w:usize,h:usize){
        let scale=self.scale(r,w,h).max(1e-12);
        let hw=r.width as f64/(2.*scale);
        let hh=r.height as f64/(2.*scale);
        self.cx=if 2.*hw>=w as f64 {w as f64*0.5}else{self.cx.clamp(hw,w as f64-hw)};
        self.cy=if 2.*hh>=h as f64 {h as f64*0.5}else{self.cy.clamp(hh,h as f64-hh)};
    }
    fn zoom_at(&mut self,factor:f64,x:f64,y:f64,r:Rect,w:usize,h:usize){
        let old=self.scale(r,w,h).max(1e-12);
        let mx=r.x as f64+r.width as f64*0.5;
        let my=r.y as f64+r.height as f64*0.5;
        let atx=self.cx+(x-mx)/old;
        let aty=self.cy+(y-my)/old;
        self.zoom=(self.zoom*factor).clamp(0.25,64.);
        let new=self.scale(r,w,h).max(1e-12);
        self.cx=atx-(x-mx)/new;
        self.cy=aty-(y-my)/new;
        self.clamp(r,w,h);
    }
    fn drag(&mut self,dx:i32,dy:i32,r:Rect,w:usize,h:usize){
        let scale=self.scale(r,w,h).max(1e-12);
        self.cx-=dx as f64/scale;
        self.cy-=dy as f64/scale;
        self.clamp(r,w,h);
    }
}

struct Gallery {
    files:Vec<PathBuf>, index:usize, image:FitsImage, pixels:Vec<u32>,
    mode:Mode,adjust:Adjust,false_color:FalseColorSettings,slots:Slots,view:View,
    message:String, sidebar_top:usize,panel_scroll:i32,palette_dropdown:PaletteDropdown,last_develop_ms:u128,green_cache:Option<(Bayer,Vec<f32>)>,cfa_confidence:f32,cfa_pattern:Option<Bayer>,
}
impl Gallery{
    fn new(files:Vec<PathBuf>)->Result<Self,Box<dyn Error>> {
        let mut skipped=0usize;
        let found=files.iter().enumerate().find_map(|(index,path)|{
            match read_fits(path){
                Ok(image)=>Some((index,image)),
                Err(e)=>{
                    skipped+=1;
                    if skipped<=5 {eprintln!("Skipping {}: {e}",path.display());}
                    None
                }
            }
        });
        if skipped>5 {eprintln!("Skipped {} other unsupported or invalid FITS files (messages suppressed)",skipped-5);}
        let (index,mut image)=found.ok_or("no supported image or TESS VECTOR_* table found (CBV FITS files use BINTABLE extensions)")?;
        let (slots,_) = scan_filter_group(&files,index,&mut image);
        let mode=automatic_mode(&image,&slots);
        let mut g=Self{files,index,pixels:vec![],view:View::new(image.width,image.height),image,mode,adjust:Adjust::default(),false_color:FalseColorSettings::default(),slots,message:String::new(),sidebar_top:0,panel_scroll:0,palette_dropdown:PaletteDropdown::default(),last_develop_ms:0,green_cache:None,cfa_confidence:0.,cfa_pattern:None};
        g.rebuild()?;
        Ok(g)
    }
    fn rebuild(&mut self)->Result<(),Box<dyn Error>>{
        let start=Instant::now();
        if let Mode::Bayer(pattern)=self.mode {
            if self.green_cache.as_ref().is_some_and(|(b,v)|*b!=pattern || v.len()!=self.image.width*self.image.height){
                self.green_cache=None;
            }
            if self.green_cache.is_none() && self.image.width*self.image.height<=24_000_000 {
                self.green_cache=Some((pattern,generate_bayer_green(&self.image,pattern)));
            }
            if self.cfa_pattern!=Some(pattern){
                self.cfa_confidence=bayer_confidence(&self.image.planes[0],self.image.width,self.image.height,pattern);
                self.cfa_pattern=Some(pattern);
            }
        }
        let green=self.green_cache.as_ref().and_then(|(pattern,plane)|{
            (self.mode==Mode::Bayer(*pattern)).then_some(plane.as_slice())
        });
        self.pixels=develop_with_green(&self.image,self.mode,self.adjust,&self.slots,self.false_color,green)?;
        self.last_develop_ms=start.elapsed().as_millis();
        Ok(())
    }
    fn choose_palette(&mut self,index:usize)->bool{
        let palette=FalseColorPalette::from_index(index);
        let changed=self.false_color.palette!=palette || self.mode!=Mode::FalseColor;
        self.false_color.palette=palette;
        self.mode=Mode::FalseColor;
        changed
    }
    fn bar_value(&self,kind:BarKind)->f32{match kind{
        BarKind::Exposure=>self.adjust.exposure,BarKind::Gamma=>self.adjust.gamma,
        BarKind::Black=>self.adjust.black_point,BarKind::White=>self.adjust.white_point,
        BarKind::Saturation=>self.adjust.saturation,
        BarKind::Stretch=>match self.adjust.stretch {Stretch::Linear=>0.,Stretch::Asinh=>1.,Stretch::Log=>2.,Stretch::Sqrt=>3.},
        BarKind::Background=>if self.adjust.neutralize_background{1.}else{0.},
        BarKind::HueShift=>self.false_color.hue_shift_deg,
        BarKind::ShadowHue=>self.false_color.shadows_hue,
        BarKind::MidHue=>self.false_color.midtones_hue,
        BarKind::HighHue=>self.false_color.highlights_hue,
        BarKind::StarWhite=>self.false_color.star_whitening,
        BarKind::BayerPattern=>match self.mode {Mode::Bayer(Bayer::Rggb)=>0.,Mode::Bayer(Bayer::Bggr)=>1.,
            Mode::Bayer(Bayer::Grbg)=>2.,Mode::Bayer(Bayer::Gbrg)=>3.,_=>0.},
        BarKind::RedGain=>self.adjust.rgb_gain[0],BarKind::GreenGain=>self.adjust.rgb_gain[1],
        BarKind::BlueGain=>self.adjust.rgb_gain[2],
    }}
    fn bar_label(&self,kind:BarKind)->String{match kind{
        BarKind::Stretch=>self.adjust.stretch.label().to_owned(),
        BarKind::Background=>if self.adjust.neutralize_background{"ON".into()}else{"OFF".into()},
        BarKind::BayerPattern=>if let Mode::Bayer(b)=self.mode{b.label().into()}else{"N/A".into()},
        BarKind::HueShift|BarKind::ShadowHue|BarKind::MidHue|BarKind::HighHue=>format!("{:.0}°",self.bar_value(kind)),
        BarKind::Exposure=>format!("{:+.2} EV",self.bar_value(kind)),
        BarKind::StarWhite=>format!("{:.0}%",self.bar_value(kind)*100.),
        _=>format!("{:.2}",self.bar_value(kind)),
    }}
    fn set_bar(&mut self,kind:BarKind,t:f32)->bool {
        let old=self.bar_value(kind);
        let old_palette=self.false_color.palette;
        let v=kind.from_normalized(t);
        match kind {
            BarKind::Exposure=>self.adjust.exposure=v,
            BarKind::Gamma=>self.adjust.gamma=v,
            BarKind::Black=>self.adjust.black_point=v.min(self.adjust.white_point-0.05),
            BarKind::White=>self.adjust.white_point=v.max(self.adjust.black_point+0.05),
            BarKind::Saturation=>self.adjust.saturation=v,
            BarKind::Stretch=>self.adjust.stretch=match v as i32{0=>Stretch::Linear,1=>Stretch::Asinh,2=>Stretch::Log,_=>Stretch::Sqrt},
            BarKind::Background=>self.adjust.neutralize_background=v>=0.5,
            BarKind::HueShift=>self.false_color.hue_shift_deg=v,
            BarKind::ShadowHue=>{self.false_color.shadows_hue=v;self.false_color.palette=FalseColorPalette::Custom;},
            BarKind::MidHue=>{self.false_color.midtones_hue=v;self.false_color.palette=FalseColorPalette::Custom;},
            BarKind::HighHue=>{self.false_color.highlights_hue=v;self.false_color.palette=FalseColorPalette::Custom;},
            BarKind::StarWhite=>self.false_color.star_whitening=v,
            BarKind::BayerPattern=>self.mode=Mode::Bayer(match v as i32{0=>Bayer::Rggb,1=>Bayer::Bggr,2=>Bayer::Grbg,_=>Bayer::Gbrg}),
            BarKind::RedGain=>self.adjust.rgb_gain[0]=v,
            BarKind::GreenGain=>self.adjust.rgb_gain[1]=v,
            BarKind::BlueGain=>self.adjust.rgb_gain[2]=v,
        }
        (self.bar_value(kind)-old).abs()>0.0005 || self.false_color.palette!=old_palette
    }
    fn reset_bar(&mut self,kind:BarKind)->bool{
        let a=Adjust::default();let c=FalseColorSettings::default();
        let (lo,hi,steps)=kind.range();
        let v=match kind{
            BarKind::Exposure=>a.exposure,BarKind::Gamma=>a.gamma,BarKind::Black=>a.black_point,
            BarKind::White=>a.white_point,BarKind::Saturation=>a.saturation,BarKind::Stretch=>1.,
            BarKind::Background=>1.,BarKind::HueShift=>c.hue_shift_deg,
            BarKind::ShadowHue=>c.shadows_hue,BarKind::MidHue=>c.midtones_hue,BarKind::HighHue=>c.highlights_hue,
            BarKind::StarWhite=>c.star_whitening,BarKind::BayerPattern=>0.,
            BarKind::RedGain|BarKind::GreenGain|BarKind::BlueGain=>1.,
        };
        let normalized=if steps>1{v/(steps-1) as f32}else{(v-lo)/(hi-lo)};
        self.set_bar(kind,normalized)
    }
    fn load(&mut self,index:usize)->Result<(),Box<dyn Error>>{
        let mut image=read_fits(&self.files[index])?;
        let (auto_slots,_) = scan_filter_group(&self.files,index,&mut image);
        self.index=index;
        self.view=View::new(image.width,image.height);
        self.panel_scroll=0;
        self.palette_dropdown.open=false;
        self.mode=automatic_mode(&image,&auto_slots);
        self.slots=auto_slots;
        self.image=image;
        self.green_cache=None;
        self.cfa_pattern=None;
        self.rebuild()?;
        self.message.clear();
        Ok(())
    }
    fn navigation(&mut self,delta:isize){
        let len=self.files.len() as isize;
        let mut last_error=String::new();
        for step in 1..=self.files.len(){
            let target=(self.index as isize+delta*step as isize).rem_euclid(len) as usize;
            match self.load(target){
                Ok(())=>return,
                Err(e)=>last_error=format!("Could not open {}: {e}",basename(&self.files[target])),
            }
        }
        self.message=last_error;
    }
    fn apply(&mut self,action:Action,layout:&Layout){
        let center=(layout.view.x as f64+layout.view.width as f64/2.,layout.view.y as f64+layout.view.height as f64/2.);
        let (w,h)=(self.image.width,self.image.height);
        if self.image.kind.is_table() && matches!(action,
            Action::Bayer|Action::CubeRgb|Action::FilterRgb|Action::Sho|Action::Hoo|
            Action::FilterTint|Action::StructureTint|Action::AssignR|Action::AssignG|Action::AssignB)
        {
            self.message="CBV data are time-series vectors, not sensor pixels; use Mono or False color".into();
            return;
        }
        match action{
            Action::Prev=>{self.navigation(-1);return;},
            Action::Next=>{self.navigation(1);return;},
            Action::Fit=>{self.view=View::new(w,h);return;},
            Action::ZoomIn=>{self.view.zoom_at(1.35,center.0,center.1,layout.view,w,h);return;},
            Action::ZoomOut=>{self.view.zoom_at(1./1.35,center.0,center.1,layout.view,w,h);return;},
            Action::Auto=>{
                let (slots,_) = scan_filter_group(&self.files,self.index,&mut self.image);
                self.slots=slots;
                self.mode=automatic_mode(&self.image,&self.slots);
            },
            Action::Mono=>self.mode=Mode::Gray,
            Action::FalseColor=>self.mode=Mode::FalseColor,
            Action::Palette=>{self.false_color.palette=self.false_color.palette.next();self.mode=Mode::FalseColor;},
            Action::HueDown=>{self.false_color.hue_shift_deg=wrap_hue(self.false_color.hue_shift_deg-15.);self.mode=Mode::FalseColor;},
            Action::HueUp=>{self.false_color.hue_shift_deg=wrap_hue(self.false_color.hue_shift_deg+15.);self.mode=Mode::FalseColor;},
            Action::StructureTint=>self.mode=Mode::StructureTint,
            Action::FilterTint=>self.mode=Mode::FilterTint,
            Action::Bayer=>{
                self.mode=match self.mode{Mode::Bayer(b)=>Mode::Bayer(b.next()),_=>Mode::Bayer(self.image.bayer.unwrap_or(Bayer::Rggb))};
            },
            Action::CubeRgb=>{
                if self.image.planes.len()!=3 {self.message="This HDU has no 3-plane RGB cube".into();return;}
                self.mode=Mode::CubeRgb;
            },
            Action::FilterRgb=>{
                if !self.slots.ready(){self.message="Select a FITS frame and use Set R, Set G, Set B; all dimensions must match".into();return;}
                if self.slots.red.as_ref().unwrap().width!=w || self.slots.red.as_ref().unwrap().height!=h {
                    self.message="Assigned RGB channels differ from selected image dimensions".into();return;
                }
                self.mode=Mode::AssignedRgb;
            },
            Action::Sho=>{
                if !self.slots.ready_sho(){self.message="SHO requires registered SII + H-alpha + OIII frames".into();return;}
                self.mode=Mode::Sho;
            },
            Action::Hoo=>{
                if !self.slots.ready_hoo(){self.message="HOO requires registered H-alpha + OIII frames".into();return;}
                self.mode=Mode::Hoo;
            },
            Action::Stretch=>self.adjust.stretch=self.adjust.stretch.next(),
            Action::SatDown=>self.adjust.saturation=(self.adjust.saturation-0.15).max(0.),
            Action::SatUp=>self.adjust.saturation=(self.adjust.saturation+0.15).min(3.),
            Action::Balance=>self.adjust.neutralize_background=!self.adjust.neutralize_background,
            Action::ExposureDown=>self.adjust.exposure=(self.adjust.exposure-0.5).max(-6.),
            Action::ExposureUp=>self.adjust.exposure=(self.adjust.exposure+0.5).min(6.),
            Action::GammaDown=>self.adjust.gamma=(self.adjust.gamma-0.15).max(0.2),
            Action::GammaUp=>self.adjust.gamma=(self.adjust.gamma+0.15).min(4.),
            Action::AssignR|Action::AssignG|Action::AssignB=>{
                if self.image.planes.len()!=1{self.message="Use monochrome 2D frames for filter RGB slots".into();return;}
                let channel=match action{Action::AssignR=>0,Action::AssignG=>1,_=>2};
                let label=basename(&self.files[self.index]);
                self.slots.assign(&self.image,channel,&label);
                self.message=format!("Assigned {label} to {}",["RED","GREEN","BLUE"][channel]);
                return;
            },
            Action::Export=>{
                let source=&self.files[self.index];
                let folder=source.parent().unwrap_or(Path::new(".")).join("developed");
                let safe=source.file_stem().map(|s|s.to_string_lossy().into_owned()).unwrap_or_else(||"image".into());
                let output=folder.join(format!("{safe}_{}_developed.bmp",self.mode.label().replace(' ',"_").to_ascii_lowercase()));
                let result=fs::create_dir_all(&folder).and_then(|()|save_bmp(&output,w,h,&self.pixels));
                self.message=match result{Ok(())=>format!("Saved {}",output.display()),Err(e)=>format!("BMP export failed: {e}")};
                return;
            }
        }
        if let Err(e)=self.rebuild(){self.message=e.to_string();}else{self.message.clear();}
    }
    fn scroll_sidebar_to_selection(&mut self,layout:&Layout){
        if layout.sidebar.width==0{return;}
        let visible=(layout.sidebar.height.saturating_sub(80)/30).max(1) as usize;
        if self.index<self.sidebar_top{self.sidebar_top=self.index;}
        if self.index>=self.sidebar_top+visible{self.sidebar_top=self.index+1-visible;}
    }
}

fn fill(fb:&mut Framebuffer,r:Rect,c:u32){
    let x0=r.x.max(0).min(fb.width as i32) as usize;
    let y0=r.y.max(0).min(fb.height as i32) as usize;
    let x1=r.x.saturating_add(r.width as i32).max(0).min(fb.width as i32) as usize;
    let y1=r.y.saturating_add(r.height as i32).max(0).min(fb.height as i32) as usize;
    if x0>=x1||y0>=y1{return;}
    for y in y0..y1{fb.pixels[y*fb.width as usize+x0..y*fb.width as usize+x1].fill(c);}
}
fn text(font:&mut Option<FontRenderer>,fb:&mut Framebuffer,s:&str,x:i32,y:i32,size:u16,color:u32,clip:Rect){
    if let Some(font)=font{let _=font.draw_text(fb,s,x,y,size,color,Some(clip));}
}
fn short(s:&str,max_chars:usize)->String{
    let mut chars=s.chars();
    let part:String=chars.by_ref().take(max_chars).collect();
    if chars.next().is_some(){format!("{part}...")}else{part}
}
fn wrap_hue(v:f32)->f32{
    let mut h=(v+180.).rem_euclid(360.)-180.;
    if h<=-179.999 { h=180.; }
    h
}
fn draw_centered(font:&mut Option<FontRenderer>,fb:&mut Framebuffer,s:&str,r:Rect,size:u16,color:u32){
    let (w,h)=font.as_mut().and_then(|f|f.measure(s,size).ok()).unwrap_or((s.len() as u32*8,size as u32));
    let x=r.x+((r.width.saturating_sub(w))/2) as i32;
    let y=r.y+((r.height.saturating_sub(h))/2) as i32;
    text(font,fb,s,x,y,size,color,r);
}

fn ghostbars(fb:&mut Framebuffer,font:&mut Option<FontRenderer>,layout:&Layout,g:&Gallery,cursor:(i32,i32),pending:bool){
    let r=layout.inspector;
    fill(fb,r,PANEL);
    let heading=Rect::new(r.x+12,r.y+11,r.width.saturating_sub(24),25);
    text(font,fb,"COLOR DEVELOPMENT",heading.x,heading.y,15,TEXT,heading);
    let sub=Rect::new(r.x+12,r.y+40,r.width.saturating_sub(24),22);
    text(font,fb,"Drag bars / scroll for more",sub.x,sub.y,12,MUTED,sub);
    if matches!(g.mode,Mode::FalseColor){
        let d=layout.dropdown;
        fill(fb,d,if d.contains(cursor.0,cursor.1){HOVER}else{BTN});
        let label=format!("{}  ▼",g.false_color.palette.label());
        text(font,fb,&short(&label,((d.width as usize).saturating_sub(22)/8).max(1)),d.x+10,d.y+8,13,TEXT,d);
    }
    // Each bar has a full-range ghost track, colored progress, and a thumb.
    // Deliberately don't draw clipped partial rows: the list scrolls by pixels
    // while keeping each clickable row and its text inside the panel viewport.
    for bar in &layout.bars {
        let r0=bar.row;
        if r0.y<layout.panel_viewport.y || r0.y+r0.height as i32>
            layout.panel_viewport.y+layout.panel_viewport.height as i32 {continue;}
        let hovering=r0.contains(cursor.0,cursor.1);
        if hovering{fill(fb,r0,0xff26_3548);}
        let left=Rect::new(r0.x+7,r0.y+6,r0.width.saturating_sub(110),20);
        text(font,fb,bar.kind.name(),left.x,left.y,13,if hovering{TEXT}else{MUTED},left);
        let label=g.bar_label(bar.kind);
        let estimate=(label.chars().count()*8+6) as i32;
        let val=Rect::new(r0.x+(r0.width as i32-estimate-8).max(8),r0.y+6,
            (estimate.max(1)) as u32,20);
        text(font,fb,&label,val.x,val.y,13,TEXT,val);
        fill(fb,bar.track,GHOST);
        let t=bar.kind.norm(g.bar_value(bar.kind));
        let reached=(t*bar.track.width as f32).round() as u32;
        if reached>0 {fill(fb,Rect::new(bar.track.x,bar.track.y,reached,bar.track.height),ACCENT);}
        let bx=bar.track.x+(bar.track.width.saturating_sub(1) as f32*t).round() as i32;
        fill(fb,Rect::new(bx-3,bar.track.y-5,7,bar.track.height+10),TEXT);
        if hovering {fill(fb,Rect::new(bar.track.x,bar.track.y+bar.track.height as i32+5,
            bar.track.width,1),ACCENT);}
    }
    // Fixed footer with an actual preview of the chosen LUT.
    let footer=Rect::new(r.x+8,r.y+r.height as i32-63,r.width.saturating_sub(16),55);
    fill(fb,footer,0xff17_1e2a);
    if matches!(g.mode,Mode::FalseColor){
        let strip=Rect::new(footer.x+9,footer.y+8,footer.width.saturating_sub(18),16);
        for x in 0..strip.width{
            let t=x as f32/(strip.width.saturating_sub(1).max(1)) as f32;
            fill(fb,Rect::new(strip.x+x as i32,strip.y,1,strip.height),palette_color(g.false_color,t));
        }
        text(font,fb,"CUSTOM: edit shadow/mid/high hue",footer.x+9,footer.y+30,11,MUTED,footer);
    }else{
        text(font,fb,"Right-click bar to reset",footer.x+9,footer.y+10,12,MUTED,footer);
        if matches!(g.mode,Mode::Bayer(_)) {
            text(font,fb,"CFA pattern is meaningful on RAW only",footer.x+9,footer.y+30,11,MUTED,footer);
        }else{
            text(font,fb,"Settings are applied to the output",footer.x+9,footer.y+30,11,MUTED,footer);
        }
    }
    if pending{
        let pos=Rect::new(r.x+10,r.y+59,r.width.saturating_sub(20),18);
        text(font,fb,"Updating when drag pauses...",pos.x,pos.y,11,ACCENT,pos);
    }
    if layout.panel_max_scroll>0 && layout.panel_viewport.height>0{
        let vis=layout.panel_viewport.height as f32;
        let total=vis+layout.panel_max_scroll as f32;
        let thumb_h=((vis*vis/total).round() as u32).clamp(18,layout.panel_viewport.height);
        let scroll=g.panel_scroll.clamp(0,layout.panel_max_scroll) as f32/layout.panel_max_scroll as f32;
        let thumb_y=layout.panel_viewport.y+((vis-thumb_h as f32)*scroll).round() as i32;
        fill(fb,Rect::new(r.x+r.width as i32-4,layout.panel_viewport.y,2,layout.panel_viewport.height),GHOST);
        fill(fb,Rect::new(r.x+r.width as i32-4,thumb_y,2,thumb_h),ACCENT);
    }
}

fn raster(fb:&mut Framebuffer,r:Rect,img:&FitsImage,pixels:&[u32],view:View){
    fill(fb,r,VIEW_BG);
    let scale=view.scale(r,img.width,img.height);
    let mx=r.x as f64+r.width as f64*0.5;
    let my=r.y as f64+r.height as f64*0.5;
    let fw=fb.width as usize;
    let minx=r.x.max(0) as usize;let miny=r.y.max(0) as usize;
    let maxx=(r.x+r.width as i32).max(0).min(fb.width as i32) as usize;
    let maxy=(r.y+r.height as i32).max(0).min(fb.height as i32) as usize;
    if scale<=0. || minx>=maxx || miny>=maxy{return;}
    // Screen X -> source X is constant for all rows. Precomputing this avoids
    // millions of floating-point divisions on every pan and zoom redraw.
    let inv=1./scale;
    let columns:Vec<usize>=(minx..maxx).map(|x|{
        let sx=view.cx+((x as f64+0.5)-mx)*inv;
        if sx>=0. && sx<img.width as f64{sx as usize}else{usize::MAX}
    }).collect();
    for y in miny..maxy{
        let sy=view.cy+((y as f64+0.5)-my)*inv;
        if sy<0. || sy>=img.height as f64{continue;}
        let row=(sy as usize)*img.width;
        let dst=&mut fb.pixels[y*fw+minx..y*fw+maxx];
        for (pixel,&sx) in dst.iter_mut().zip(columns.iter()){
            if sx!=usize::MAX {*pixel=pixels[row+sx];}
        }
    }
}
fn sidebar_click(g:&Gallery,layout:&Layout,x:i32,y:i32)->Option<usize>{
    if !layout.sidebar.contains(x,y) {return None;}
    let rel=y-layout.sidebar.y-49;
    if rel<0{return None;}
    let idx=g.sidebar_top+(rel/30) as usize;
    (idx<g.files.len()).then_some(idx)
}
fn render(fb:&mut Framebuffer,font:&mut Option<FontRenderer>,layout:&Layout,g:&Gallery,cursor:(i32,i32),pending:bool){
    fb.clear(BG);
    fill(fb,Rect::new(0,0,fb.width,49),PANEL);
    let filename=basename(&g.files[g.index]);
    let title=format!("FITS LAB  /  {}",short(&filename,65));
    text(font,fb,&title,22,15,21,TEXT,layout.title);
    for b in &layout.buttons{
        let label=b.action.label(g);
        let selected=b.action.highlighted(g);
        let hover=b.rect.contains(cursor.0,cursor.1);
        fill(fb,b.rect,if selected{SELECT}else if hover{HOVER}else{BTN});
        let width=b.rect.width as usize;
        draw_centered(font,fb,&short(&label,(width/8).max(1)),b.rect,14,TEXT);
    }
    raster(fb,layout.view,&g.image,&g.pixels,g.view);
    if layout.sidebar.width>0{
        fill(fb,layout.sidebar,PANEL);
        text(font,fb,&format!("FITS FILES  ({})",g.files.len()),layout.sidebar.x+10,layout.sidebar.y+12,15,TEXT,layout.sidebar);
        text(font,fb,"Scroll to browse",layout.sidebar.x+10,layout.sidebar.y+32,12,MUTED,layout.sidebar);
        let visible=(layout.sidebar.height.saturating_sub(80)/30).max(1) as usize;
        let bottom=(g.sidebar_top+visible).min(g.files.len());
        for idx in g.sidebar_top..bottom{
            let y=layout.sidebar.y+49+30*(idx-g.sidebar_top) as i32;
            let rect=Rect::new(layout.sidebar.x+5,y,layout.sidebar.width-10,27);
            let active=g.index==idx;
            fill(fb,rect,if active{SELECT}else if rect.contains(cursor.0,cursor.1){HOVER}else{PANEL});
            let label=short(&format!("{}  {}",idx+1,basename(&g.files[idx])),((rect.width as usize)/8).max(1));
            text(font,fb,&label,rect.x+6,rect.y+5,13,if active{TEXT}else{MUTED},rect);
        }
        let (red,green,blue)=(&g.slots.red,&g.slots.green,&g.slots.blue);
        let status=format!("RGB:{}{}{}  HOS:{}{}{}",
            red.as_ref().map(|_|"R").unwrap_or("-"),
            green.as_ref().map(|_|"G").unwrap_or("-"),
            blue.as_ref().map(|_|"B").unwrap_or("-"),
            g.slots.ha.as_ref().map(|_|"H").unwrap_or("-"),
            g.slots.oiii.as_ref().map(|_|"O").unwrap_or("-"),
            g.slots.sii.as_ref().map(|_|"S").unwrap_or("-"));
        let at_y=layout.sidebar.y+layout.sidebar.height as i32-24;
        text(font,fb,&status,layout.sidebar.x+10,at_y,12,MUTED,layout.sidebar);
    }
    ghostbars(fb,font,layout,g,cursor,pending);
    fill(fb,layout.status,PANEL);
    let palette_info=if matches!(g.mode,Mode::FalseColor){format!(" | {} {:+.0}°",g.false_color.palette.label(),g.false_color.hue_shift_deg)}else{String::new()};
    let bayer_info=if let Mode::Bayer(_)=g.mode{format!(" | CFA {:.0}%",g.cfa_confidence*100.)}else{String::new()};
    let table_note=if let FitsKind::CbvHeatmap{cadences,vectors,extension}=&g.image.kind{
        format!(" | CBV {} vectors / {} cadences | {}",vectors.len(),cadences,extension)
    }else{String::new()};
    let info=format!("{} / {} | {}x{} | BITPIX {} | {}{}{}{} | Zoom {:.0}% | EV {:+.1} | Gamma {:.2} | Sat {:.2} | Filter {} | Develop {}ms",
        g.index+1,g.files.len(),g.image.width,g.image.height,g.image.bitpix,g.mode.label(),palette_info,bayer_info,table_note,g.view.zoom*100.,g.adjust.exposure,g.adjust.gamma,g.adjust.saturation,
        if g.image.filter.is_empty(){g.image.filter_kind.label()}else{&g.image.filter},g.last_develop_ms);
    let available=(layout.status.width as usize/8).max(1);
    text(font,fb,&short(&info,available),layout.status.x+12,layout.status.y+7,14,TEXT,layout.status);
    let bayer_tip=if let Mode::Bayer(_)=g.mode {
        let conf=g.cfa_confidence;
        if conf<0.18 {"Low CFA confidence: this file probably is not a raw Bayer mosaic, so debayering can remain nearly monochrome | B:cycle pattern | A:auto"}
        else if conf<0.38 {"Weak CFA confidence: some color may be recovered, but the source does not strongly resemble raw Bayer CFA data | B:cycle pattern"}
        else {"Debayer from Bayer CFA mosaic | B:cycle pattern | Wheel:zoom | Drag:pan | S:export"}
    } else {""};
    let tip=if g.message.is_empty() && g.image.kind.is_table() {
        "TESS CBV HEATMAP (not a photograph): X=sampled cadence; Y=vector index; values independently normalized per vector"
    }else if g.message.is_empty(){match g.mode {
        Mode::FalseColor=>"ARTISTIC color: right-side ghostbars change palette and colors | Wheel:image zoom | Drag:image pan | S:export",
        Mode::StructureTint=>"ARTISTIC ESTIMATE - 1 channel cannot reveal true color | A:auto | Wheel:zoom | Drag:pan | S:export",
        Mode::FilterTint=>"ARTISTIC wavelength tint - NOT measured RGB | A:auto | +/-:zoom | S:export",
        Mode::Sho|Mode::Hoo=>"FALSE-COLOR filter mapping; inputs MUST already be aligned | A:auto | S:export",
        Mode::AssignedRgb=>"R/G/B filtered inputs: assumed aligned; no registration or calibration | A:auto | S:export",
        Mode::CubeRgb=>"RGB planes assumed by order; inspect CTYPE3 if this is a spectral cube | A:auto | S:export",
        Mode::Bayer(_)=>bayer_tip,
        _=>"Wheel:zoom at cursor | Drag:pan | Left/Right:image | A:auto color | +/-:zoom | S:export",
    }}else{&g.message};
    text(font,fb,&short(tip,available),layout.status.x+12,layout.status.y+29,13,if g.message.is_empty(){MUTED}else{0xff76_d8ea},layout.status);
    if g.palette_dropdown.open && matches!(g.mode,Mode::FalseColor){
        let p=layout.popup;
        fill(fb,p,0xff0d_1622);
        let header=Rect::new(p.x+6,p.y+5,p.width.saturating_sub(12),POPUP_SEARCH_H as u32-8);
        fill(fb,header,BTN);
        let q=if g.palette_dropdown.query.is_empty(){
            format!("Type to filter {} palettes...",PRESET_COUNT)
        }else{format!("Search: {}_",g.palette_dropdown.query)};
        text(font,fb,&short(&q,((header.width as usize)/8).max(1)),header.x+7,header.y+5,12,TEXT,header);
        let filtered=g.palette_dropdown.filtered();
        let shown=popup_rows(layout);
        for (position,&index) in filtered.iter().skip(g.palette_dropdown.first).take(shown).enumerate(){
            let y=p.y+POPUP_SEARCH_H+position as i32*POPUP_ROW_H;
            let row=Rect::new(p.x+5,y,p.width.saturating_sub(10),POPUP_ROW_H as u32-2);
            let selected=g.false_color.palette.index()==index;
            fill(fb,row,if selected {SELECT} else if row.contains(cursor.0,cursor.1){HOVER}else{PANEL});
            let swatch=Rect::new(row.x+7,row.y+8,47,16);
            let settings=FalseColorSettings{palette:FalseColorPalette::from_index(index),..g.false_color};
            for i in 0..swatch.width {
                let t=i as f32/(swatch.width.saturating_sub(1).max(1)) as f32;
                fill(fb,Rect::new(swatch.x+i as i32,swatch.y,1,swatch.height),palette_color(settings,t));
            }
            let name=FalseColorPalette::from_index(index).label();
            text(font,fb,&short(&name,(row.width.saturating_sub(66) as usize/8).max(1)),row.x+62,row.y+9,12,TEXT,row);
        }
        let footer=Rect::new(p.x+8,p.y+p.height as i32-14,p.width.saturating_sub(16),12);
        text(font,fb,&format!("{} matching · wheel to scroll",filtered.len()),footer.x,footer.y,10,MUTED,footer);
    }
}

fn run()->Result<(),Box<dyn Error>>{
    let path=directory();
    if !path.is_dir(){return Err(format!("FITS folder not found: {}",path.display()).into());}
    let files=collect_files(&path,is_fits)?;
    if files.is_empty(){return Err(format!("No .fits, .fit or .fts files under {}",path.display()).into());}
    let mut gallery=Gallery::new(files)?;
    let mut window=Window::new("MataWin FITS Lab - Ghostbar Inspector",WIN_SIZE);
    let (w,h)=window.client_size();
    let mut fb=Framebuffer::from_color(w.max(1),h.max(1),BG);
    let mut font=FontRenderer::system_default().ok();
    let mut events=Vec::with_capacity(48);
    let mut cursor=(0i32,0i32);
    let mut dragging=false;
    let mut active_ghost:Option<BarKind>=None;
    let mut pending_rebuild=false;
    let mut last_bar_change=Instant::now();
    let mut dirty=true;
    loop{
        window.poll_events_into(&mut events);
        let native=window.client_size();
        if native.0>0 && native.1>0 && (native.0!=fb.width || native.1!=fb.height){
            fb.resize_and_clear(native.0,native.1,BG);
            dirty=true;
        }
        let layout=Layout::new(fb.width,fb.height,gallery.mode,gallery.panel_scroll);
        gallery.panel_scroll=gallery.panel_scroll.clamp(0,layout.panel_max_scroll);
        gallery.view.clamp(layout.view,gallery.image.width,gallery.image.height);
        for event in events.iter().copied(){
            if gallery.palette_dropdown.open {
                match event {
                    WindowEvent::KeyPressed(Key::Escape)=>{gallery.palette_dropdown.close();dirty=true;continue;},
                    WindowEvent::KeyPressed(Key::MouseRight)=>{
                        gallery.palette_dropdown.close();dirty=true;continue;
                    },
                    WindowEvent::KeyPressed(Key::Backspace)=>{
                        gallery.palette_dropdown.query.pop();gallery.palette_dropdown.first=0;dirty=true;continue;
                    },
                    WindowEvent::KeyPressed(Key::Char(ch)) if !ch.is_control()=>{
                        if gallery.palette_dropdown.query.len()<64 {gallery.palette_dropdown.query.push(ch);gallery.palette_dropdown.first=0;}
                        dirty=true;continue;
                    },
                    WindowEvent::KeyPressed(Key::Enter)=>{
                        if let Some(&id)=gallery.palette_dropdown.filtered().get(gallery.palette_dropdown.first){
                            gallery.choose_palette(id);
                            if let Err(e)=gallery.rebuild(){gallery.message=e.to_string();}
                        }
                        gallery.palette_dropdown.close();pending_rebuild=false;dirty=true;continue;
                    },
                    WindowEvent::Scroll(_,dy)=>{
                        let steps=if dy.abs()>=120{dy/120}else{dy.signum()};
                        gallery.palette_dropdown.scroll(-steps*3,popup_rows(&layout));dirty=true;continue;
                    },
                    WindowEvent::KeyPressed(Key::MouseLeft)=>{
                        if let Some(id)=popup_index(&gallery,&layout,cursor.0,cursor.1){
                            gallery.choose_palette(id);
                            gallery.palette_dropdown.close();
                            if let Err(e)=gallery.rebuild(){gallery.message=e.to_string();}
                            pending_rebuild=false;
                        }else if !layout.dropdown.contains(cursor.0,cursor.1){gallery.palette_dropdown.close();}
                        else {gallery.palette_dropdown.close();}
                        dirty=true;continue;
                    },
                    _=>{},
                }
            }
            match event{
                WindowEvent::Close|WindowEvent::KeyPressed(Key::Escape)=>return Ok(()),
                WindowEvent::Resized(..)|WindowEvent::Maximized(..)|WindowEvent::Restored(..)=>dirty=true,
                WindowEvent::CursorMoved(x,y)=>{
                    if dragging && layout.view.contains(cursor.0,cursor.1){
                        gallery.view.drag(x-cursor.0,y-cursor.1,layout.view,gallery.image.width,gallery.image.height);
                    }
                    if let Some(kind)=active_ghost {
                        if let Some(bar)=layout.bar_for(kind){
                            let u=(x-bar.track.x) as f32/bar.track.width.max(1) as f32;
                            if gallery.set_bar(kind,u) {pending_rebuild=true;last_bar_change=Instant::now();}
                        }
                    }
                    cursor=(x,y);dirty=true;
                },
                WindowEvent::KeyPressed(Key::MouseLeft)=>{
                    if matches!(gallery.mode,Mode::FalseColor)&&layout.dropdown.contains(cursor.0,cursor.1){
                        gallery.palette_dropdown.open=true;
                        gallery.palette_dropdown.query.clear();
                        let selected=gallery.false_color.palette.index();
                        gallery.palette_dropdown.first=selected.saturating_sub(3).min(PRESET_COUNT.saturating_sub(popup_rows(&layout)));
                        active_ghost=None;
                        dirty=true;
                        continue;
                    }
                    if let Some(action)=layout.action_at(cursor.0,cursor.1){
                        // Preserve the latest slider changes when switching modes.
                        if pending_rebuild && action==Action::Export {
                            if let Err(e)=gallery.rebuild(){gallery.message=e.to_string();}
                        }
                        pending_rebuild=false;active_ghost=None;
                        gallery.apply(action,&layout);
                    }
                    else if let Some(bar)=layout.bar_at(cursor.0,cursor.1){
                        active_ghost=Some(bar.kind);
                        let u=(cursor.0-bar.track.x) as f32/bar.track.width.max(1) as f32;
                        if gallery.set_bar(bar.kind,u){pending_rebuild=true;last_bar_change=Instant::now();}
                    }
                    else if let Some(idx)=sidebar_click(&gallery,&layout,cursor.0,cursor.1){
                        pending_rebuild=false;active_ghost=None;
                        if let Err(e)=gallery.load(idx){gallery.message=format!("Could not open {}: {e}",basename(&gallery.files[idx]));}
                    }else if layout.view.contains(cursor.0,cursor.1){dragging=true;}
                    gallery.scroll_sidebar_to_selection(&layout);
                    dirty=true;
                },
                WindowEvent::KeyReleased(Key::MouseLeft)=>{
                    dragging=false;
                    if active_ghost.take().is_some() && pending_rebuild {
                        if let Err(e)=gallery.rebuild(){gallery.message=e.to_string();}
                        pending_rebuild=false;
                    }
                    dirty=true;
                },
                WindowEvent::KeyPressed(Key::MouseRight)=>{
                    if let Some(bar)=layout.bar_at(cursor.0,cursor.1){
                        if gallery.reset_bar(bar.kind){
                            if let Err(e)=gallery.rebuild(){gallery.message=e.to_string();}
                            pending_rebuild=false;
                        }
                        dirty=true;
                    }
                },
                WindowEvent::Scroll(_,dy) if layout.view.contains(cursor.0,cursor.1)=>{
                    let steps=if dy.abs()>=120 {(dy as f64/120.).clamp(-5.,5.)} else {(dy as f64).clamp(-5.,5.)};
                    gallery.view.zoom_at(1.22f64.powf(steps),cursor.0 as f64,cursor.1 as f64,layout.view,gallery.image.width,gallery.image.height);
                    dirty=true;
                },
                WindowEvent::Scroll(_,dy) if layout.inspector.contains(cursor.0,cursor.1)=>{
                    let steps=if dy.abs()>=120 {dy/120}else{dy.signum()};
                    gallery.panel_scroll=(gallery.panel_scroll-steps*56).clamp(0,layout.panel_max_scroll);
                    dirty=true;
                },
                WindowEvent::Scroll(_,dy) if layout.sidebar.contains(cursor.0,cursor.1)=>{
                    let steps=if dy.abs()>=120 {dy/120}else{dy.signum()};
                    gallery.sidebar_top=gallery.sidebar_top.saturating_sub(steps.max(0) as usize)
                        .saturating_add((-steps).max(0) as usize).min(gallery.files.len().saturating_sub(1));
                    dirty=true;
                },
                WindowEvent::KeyPressed(Key::Left)=>{pending_rebuild=false;active_ghost=None;gallery.navigation(-1);gallery.scroll_sidebar_to_selection(&layout);dirty=true;},
                WindowEvent::KeyPressed(Key::Right)=>{pending_rebuild=false;active_ghost=None;gallery.navigation(1);gallery.scroll_sidebar_to_selection(&layout);dirty=true;},
                WindowEvent::KeyPressed(Key::Char(ch))=>{
                    if ch.to_ascii_lowercase()=='p' && matches!(gallery.mode,Mode::FalseColor){
                        gallery.palette_dropdown.open=true;
                        gallery.palette_dropdown.query.clear();
                        gallery.palette_dropdown.first=gallery.false_color.palette.index().saturating_sub(3)
                            .min(PRESET_COUNT.saturating_sub(popup_rows(&layout)));
                        dirty=true;
                        continue;
                    }
                    let action=match ch.to_ascii_lowercase(){
                        'f'=>Some(Action::Fit),'+'=>Some(Action::ZoomIn),'-'=>Some(Action::ZoomOut),
                        's'=>Some(Action::Export),'a'=>Some(Action::Auto),'m'=>Some(Action::Mono),'c'=>Some(Action::FalseColor),
                        'q'=>Some(Action::HueDown),'e'=>Some(Action::HueUp),
                        'i'=>Some(Action::StructureTint),'w'=>Some(Action::FilterTint),
                        'n'=>Some(Action::Sho),'o'=>Some(Action::Hoo),'v'=>Some(Action::Balance),
                        '['=>Some(Action::SatDown),']'=>Some(Action::SatUp),
                        'b'=>Some(Action::Bayer),'r'=>Some(Action::AssignR),'g'=>Some(Action::AssignG),
                        'u'=>Some(Action::AssignB),'t'=>Some(Action::Stretch),_=>None,
                    };
                    if let Some(action)=action{
                        if pending_rebuild && action==Action::Export {let _=gallery.rebuild();}
                        pending_rebuild=false;active_ghost=None;
                        gallery.apply(action,&layout);dirty=true;
                    }
                },
                _=>{},
            }
        }
        // Full-resolution development can be expensive for 20+ MP FITS images.
        // Ghostbars themselves stay responsive during a drag; image development
        // is committed on mouse release or after the pointer rests for 300 ms.
        if pending_rebuild && last_bar_change.elapsed()>=Duration::from_millis(300){
            if let Err(e)=gallery.rebuild(){gallery.message=e.to_string();}
            pending_rebuild=false;
            dirty=true;
        }
        if dirty{
            let layout=Layout::new(fb.width,fb.height,gallery.mode,gallery.panel_scroll);
            gallery.panel_scroll=gallery.panel_scroll.clamp(0,layout.panel_max_scroll);
            gallery.view.clamp(layout.view,gallery.image.width,gallery.image.height);
            render(&mut fb,&mut font,&layout,&gallery,cursor,pending_rebuild);
            if window.client_size()==(fb.width,fb.height){window.present(&fb);}
            dirty=false;
        }else{thread::sleep(Duration::from_millis(8));}
    }
}
fn main(){if let Err(e)=run(){eprintln!("fits_gallery: {e}");std::process::exit(1);}}

#[cfg(test)]mod tests{
    use super::*;
    #[test] fn companions_match_filter_series(){
        assert_eq!(group_stem(Path::new("M42_Ha.fits")),group_stem(Path::new("M42_OIII.fits")));
        assert_eq!(group_stem(Path::new("04_filter_R.fits")),group_stem(Path::new("04_filter_G.fits")));
        assert_ne!(group_stem(Path::new("M42_Ha.fits")),group_stem(Path::new("M31_Ha.fits")));
        assert_eq!(filter_from_filename(Path::new("M42_Ha.fit")),FilterKind::HAlpha);
    }
    #[test] fn ghostbar_sidebar_layout_and_value_mapping(){
        let r=Layout::new(1510,900,Mode::FalseColor,0);
        assert!(r.view.width>300 && r.inspector.width>=240);
        assert_eq!(r.bars.first().unwrap().kind,BarKind::HueShift);
        assert!(r.bars.iter().any(|b|b.kind==BarKind::ShadowHue));
        assert!(r.panel_max_scroll>0);
        let a=BarKind::Exposure;
        assert!((a.from_normalized(0.5)-0.).abs()<1e-6);
        assert!((a.norm(6.)-1.).abs()<1e-6);
        assert_eq!(PRESET_COUNT,391);
        let b=Layout::new(1510,900,Mode::Bayer(Bayer::Rggb),0);
        assert_eq!(b.bars.first().unwrap().kind,BarKind::BayerPattern);
        assert!(b.bars.iter().any(|row|row.kind==BarKind::RedGain));
    }
    #[test] fn searchable_palette_dropdown_filters_names(){
        let mut menu=PaletteDropdown::default();
        assert_eq!(menu.filtered().len(),PRESET_COUNT);
        menu.query="Orion".to_owned();
        assert_eq!(menu.filtered().len(),16);
        assert!(menu.filtered().iter().all(|&i|FalseColorPalette::from_index(i).label().contains("Orion")));
        menu.query="something-does-not-exist".to_owned();
        assert!(menu.filtered().is_empty());
    }
    #[test] fn false_color_slider_ranges_are_bounded(){
        let c=BarKind::StarWhite;
        assert_eq!(c.from_normalized(-2.),0.);
        assert_eq!(c.from_normalized(5.),1.);
    }
    #[test] fn zoom_preserves_anchor(){
        let r=Rect::new(10,20,900,600);
        let mut v=View::new(2000,1200);
        let anchor=(213.,322.);
        let old=v.scale(r,2000,1200);
        let before=(v.cx+(anchor.0-(r.x as f64+r.width as f64/2.))/old,v.cy+(anchor.1-(r.y as f64+r.height as f64/2.))/old);
        v.zoom_at(2.,anchor.0,anchor.1,r,2000,1200);
        let new=v.scale(r,2000,1200);
        let after=(v.cx+(anchor.0-(r.x as f64+r.width as f64/2.))/new,v.cy+(anchor.1-(r.y as f64+r.height as f64/2.))/new);
        assert!((before.0-after.0).abs()<1e-5); assert!((before.1-after.1).abs()<1e-5);
    }
}
