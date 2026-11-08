//! Responsive retained-independent UI, custom-rasterized through MataWin.
//! Coordinates are logical UI pixels; only the presenter handles DPI scaling.
use mata_win::Framebuffer;
use mata_ui::widgets::{FontRenderer,Rect};
use mata_image::Image;
use crate::slots::SlotInfo;
use crate::engine::{Game,ORES,STAGES,ResearchAmount,research_name,research_description,research_rank_limit,BuyAmount,COUNT,PRESTIGE_COUNT,RESEARCH_COUNT,number};

const BG:u32=0xff0b101b;
const SURFACE:u32=0xff141e2d;
const PANEL:u32=0xff192638;
const WHITE:u32=0xfff4f7ff;
const MUTED:u32=0xffa5b6ce;
const GOLD:u32=0xffffc970;

#[derive(Clone,Copy,PartialEq,Eq)]
pub enum Tab { Mines, Prestige, Research, Ledger }
impl Tab { pub fn label(self)->&'static str{ match self {Self::Mines=>"OPERATIONS",Self::Prestige=>"PRESTIGE",Self::Research=>"RESEARCH",Self::Ledger=>"LEDGER"}} }
#[derive(Clone,Copy)]
pub enum Action {
    Tab(Tab), Quantity(BuyAmount), Miner(usize), Equipment(usize), Ascend(usize),
    Research(usize,usize), ResearchQuantity(ResearchAmount), ToggleAutoResearch,
    ToggleAuto, ToggleGear, ToggleAutoPrestige,
    EditMinGain, GainChar(char), GainBackspace, GainCommit, GainCancel, ToggleSound, Save,
    MenuOpen, MenuResume, MenuSave, MenuNewSlot, MenuLoadSlot(u32),
    MenuResetAsk, MenuResetConfirm, MenuResetCancel,
}
#[derive(Clone,Copy)]
struct R{x:f32,y:f32,w:f32,h:f32}
impl R {
    fn new(x:f32,y:f32,w:f32,h:f32)->Self {Self{x,y,w,h}}
    /// Visible overlap (never rounds items to whole rows).
    fn intersection(self,clip:Self)->Option<Self>{
        let x=self.x.max(clip.x);
        let y=self.y.max(clip.y);
        let right=(self.x+self.w).min(clip.x+clip.w);
        let bottom=(self.y+self.h).min(clip.y+clip.h);
        (right>x && bottom>y).then(||Self::new(x,y,right-x,bottom-y))
    }
}
// All interactive geometry is rasterized and hit-tested using the same
// rounded native-pixel rectangle. Inverting the fractional layout scale in
// hit-testing can otherwise shift a control by several pixels during resize.
fn physical_rect(r:R,scale:f32,origin:(f32,f32))->(i32,i32,i32,i32){
    let left=(r.x*scale+origin.0).round() as i32;
    let top=(r.y*scale+origin.1).round() as i32;
    let right=((r.x+r.w)*scale+origin.0).round() as i32;
    let bottom=((r.y+r.h)*scale+origin.1).round() as i32;
    (left,top,right,bottom)
}
// Size the desktop sidebar against the actual available logical height.
// The notification strip owns the last 35px, so the controls card must never
// extend into that region (including after maximize or DPI/scale changes).
#[derive(Clone,Copy)]
struct SidebarLayout {
    multiplier:R,
    milestone:R,
    controls:R,
}
impl SidebarLayout {
    fn new(x:f32,width:f32,top:f32,logical_height:f32)->Self {
        const GAP:f32=12.0;
        const CONTROLS_HEIGHT:f32=105.0;
        const FOOTER_HEIGHT:f32=35.0;
        const FOOTER_GAP:f32=10.0;
        let multiplier_y=top+5.0;
        let preferred_controls_y=top+420.0;
        let controls_y=preferred_controls_y.min(
            logical_height-FOOTER_HEIGHT-FOOTER_GAP-CONTROLS_HEIGHT);
        // Preserve the tall-window proportions when possible. In short/wide
        // windows reclaim vertical space from the upper cards rather than
        // pushing the interactive buttons beneath the footer.
        // Reserve a useful milestone card as well: at maximized 16:9
        // dimensions the actual logical height can be below 720px.
        let upper_room=(controls_y-multiplier_y-2.0*GAP).max(0.0);
        let multiplier_height=(upper_room*0.65).min(235.0);
        let milestone_y=multiplier_y+multiplier_height+GAP;
        let milestone_height=(controls_y-GAP-milestone_y).max(0.0);
        Self {
            multiplier:R::new(x,multiplier_y,width,multiplier_height),
            milestone:R::new(x,milestone_y,width,milestone_height),
            controls:R::new(x,controls_y,width,CONTROLS_HEIGHT),
        }
    }
}
struct Hot {rect:R, action:Action}

pub struct Interface {
    pub tab:Tab,
    pub amount:BuyAmount,
    pub research_amount:ResearchAmount,
    pub scroll:f32,
    pub pending:Option<usize>,
    pub notice:String,
    hot:Vec<Hot>,
    scale:f32,
    origin:(f32,f32),
    logical:(f32,f32),
    sprites:Vec<Image>,
    pub sound_enabled:bool,
    pub min_gain_editing:bool,
    min_gain_input:String,
    min_gain_select_all:bool,
    pub menu_open:bool,
    pub menu_reset_confirm:bool,
    pub active_slot:u32,
    menu_slots:Vec<SlotInfo>,
    menu_scroll:f32,
}

impl Interface {
    pub fn new()->Self{
        // Assets are embedded so Windows/Linux builds and Android APKs use the same sprites.
        const IMAGES:[&[u8];COUNT]=[
            include_bytes!("../assets/ores/coal.png"),
            include_bytes!("../assets/ores/iron.png"),
            include_bytes!("../assets/ores/copper.png"),
            include_bytes!("../assets/ores/silver.png"),
            include_bytes!("../assets/ores/gold.png"),
            include_bytes!("../assets/ores/platinum.png"),
            include_bytes!("../assets/ores/diamond.png"),
            include_bytes!("../assets/ores/emerald.png"),
            include_bytes!("../assets/ores/uranium.png"),
            include_bytes!("../assets/ores/mythril.png"),
            include_bytes!("../assets/ores/obsidian.png"),
            include_bytes!("../assets/ores/titanium.png"),
            include_bytes!("../assets/ores/cobalt.png"),
            include_bytes!("../assets/ores/lithium.png"),
            include_bytes!("../assets/ores/palladium.png"),
            include_bytes!("../assets/ores/rhodium.png"),
            include_bytes!("../assets/ores/iridium.png"),
            include_bytes!("../assets/ores/osmium.png"),
            include_bytes!("../assets/ores/tungsten.png"),
            include_bytes!("../assets/ores/neodymium.png"),
            include_bytes!("../assets/ores/sapphire.png"),
            include_bytes!("../assets/ores/ruby.png"),
            include_bytes!("../assets/ores/alexandrite.png"),
            include_bytes!("../assets/ores/tanzanite.png"),
            include_bytes!("../assets/ores/benitoite.png"),
            include_bytes!("../assets/ores/painite.png"),
            include_bytes!("../assets/ores/astatine.png"),
            include_bytes!("../assets/ores/star_metal.png"),
            include_bytes!("../assets/ores/aetherite.png"),
            include_bytes!("../assets/ores/void_crystal.png"),
            include_bytes!("../assets/ores/bauxite.png"),
            include_bytes!("../assets/ores/chromite.png"),
            include_bytes!("../assets/ores/molybdenite.png"),
            include_bytes!("../assets/ores/vanadinite.png"),
            include_bytes!("../assets/ores/zircon.png"),
            include_bytes!("../assets/ores/spodumene.png"),
            include_bytes!("../assets/ores/monazite.png"),
            include_bytes!("../assets/ores/columbite.png"),
            include_bytes!("../assets/ores/tantalite.png"),
            include_bytes!("../assets/ores/scheelite.png"),
            include_bytes!("../assets/ores/peridot.png"),
            include_bytes!("../assets/ores/aquamarine.png"),
            include_bytes!("../assets/ores/opal.png"),
            include_bytes!("../assets/ores/jadeite.png"),
            include_bytes!("../assets/ores/grandidierite.png"),
            include_bytes!("../assets/ores/jeremejevite.png"),
            include_bytes!("../assets/ores/musgravite.png"),
            include_bytes!("../assets/ores/taaffeite.png"),
            include_bytes!("../assets/ores/serendibite.png"),
            include_bytes!("../assets/ores/red_beryl.png"),
            include_bytes!("../assets/ores/hibonite.png"),
            include_bytes!("../assets/ores/eosphorite.png"),
            include_bytes!("../assets/ores/starlight_ore.png"),
            include_bytes!("../assets/ores/solarite.png"),
            include_bytes!("../assets/ores/nebulium.png"),
            include_bytes!("../assets/ores/quasar_shard.png"),
            include_bytes!("../assets/ores/chronite.png"),
            include_bytes!("../assets/ores/riftstone.png"),
            include_bytes!("../assets/ores/singularity_glass.png"),
            include_bytes!("../assets/ores/eternium.png"),
        ];
        let sprites=IMAGES.iter().filter_map(|x|Image::from_bytes(x).ok()).collect();
        Self {tab:Tab::Mines,amount:BuyAmount::One,research_amount:ResearchAmount::One,scroll:0.0,pending:None,
            notice:"Welcome, chief geologist. Your empire begins here.".into(),
            hot:Vec::new(),scale:1.0,origin:(0.0,0.0),logical:(1200.0,760.0),sprites,sound_enabled:true,
            min_gain_editing:false,min_gain_input:String::new(),min_gain_select_all:false,
            menu_open:true,menu_reset_confirm:false,active_slot:1,menu_slots:Vec::new(),menu_scroll:0.0}
    }
    /// Begin editing the minimum automatic prestige reward. Uses the same
    /// Big scientific notation accepted by the save format.
    pub fn focus_min_gain(&mut self,game:&Game){
        if !self.min_gain_editing {
            self.min_gain_input=game.auto_prestige_min_gain.storage();
            self.min_gain_select_all=true;
        }
        self.min_gain_editing=true;
    }
    /// Return true if the key belongs to the active text field. Enter commits,
    /// Escape cancels, Backspace edits, and numbers / exponent punctuation type.
    /// Each change is kept locally until the user commits a positive value.
    pub fn input_min_gain(&mut self,key:mata_win::events::Key,game:&mut Game)->Option<Result<(),String>>{
        use mata_win::events::Key;
        if !self.min_gain_editing {return None}
        match key {
            Key::Enter=>return Some(self.commit_min_gain(game)),
            Key::Escape=>{
                self.min_gain_editing=false;
                self.min_gain_input.clear();
                self.min_gain_select_all=false;
                return Some(Ok(()))
            }
            Key::Backspace=>{
                if self.min_gain_select_all {self.min_gain_input.clear();self.min_gain_select_all=false;}
                else {self.min_gain_input.pop();}
            }
            Key::Num(n) if n<=9=>{
                if self.min_gain_select_all {self.min_gain_input.clear();self.min_gain_select_all=false;}
                if self.min_gain_input.len()<256{self.min_gain_input.push(char::from(b'0'+n));}
            }
            Key::Char(ch) if ch.is_ascii_digit() || matches!(ch,'.'|'e'|'E'|'+'|'-')=>{
                if self.min_gain_select_all {self.min_gain_input.clear();self.min_gain_select_all=false;}
                if self.min_gain_input.len()<256{self.min_gain_input.push(ch);}
            }
            _=>{}
        }
        Some(Ok(()))
    }
    pub fn commit_min_gain(&mut self,game:&mut Game)->Result<(),String>{
        game.set_auto_prestige_min_gain(&self.min_gain_input)?;
        self.min_gain_editing=false;
        self.min_gain_input.clear();
        self.min_gain_select_all=false;
        Ok(())
    }
    pub fn update_slots(&mut self,slots:Vec<SlotInfo>,active:u32){
        self.menu_slots=slots;
        self.active_slot=active;
    }
    pub fn open_menu(&mut self){
        self.menu_open=true;
        self.menu_reset_confirm=false;
        self.min_gain_editing=false;
    }
    pub fn resume(&mut self){self.menu_open=false;self.menu_reset_confirm=false;}
    // Clip scrollable actions exactly as their artwork is clipped. A button
    // scrolled underneath the toolbar/footer must never remain interactive.
    fn add_scrolled_hot(&mut self,rect:R,action:Action,viewport:R){
        if let Some(visible)=rect.intersection(viewport) {
            self.hot.push(Hot{rect:visible,action});
        }
    }
    pub fn hit(&self,x:i32,y:i32)->Option<Action>{
        // Ignore hit targets covered by the opaque bottom notification bar.
        // Scrollable rows below the viewport must not remain clickable.
        let (left,top,right,bottom)=physical_rect(
            R::new(0.,0.,self.logical.0,self.logical.1-35.),self.scale,self.origin);
        if x<left || y<top || x>=right || y>=bottom {return None;}
        self.hot.iter().rev().find(|h|{
            let (x0,y0,x1,y1)=physical_rect(h.rect,self.scale,self.origin);
            x>=x0 && x<x1 && y>=y0 && y<y1
        }).map(|h|h.action)
    }
    pub fn drag_scroll(&mut self, delta_y:f32){
        if self.menu_open {
            self.menu_scroll=(self.menu_scroll-delta_y/self.scale).clamp(0.0,100_000.0);
        } else {
            self.scroll=(self.scroll-delta_y/self.scale).clamp(0.0,100_000.0);
        }
    }
    pub fn scroll(&mut self,delta:i32){
        if self.menu_open {
            self.menu_scroll=(self.menu_scroll-(delta as f32)*0.75).clamp(0.0,100_000.0);
        } else {
            self.scroll=(self.scroll-(delta as f32)*0.75).clamp(0.0,100_000.0);
        }
    }
    pub fn draw(&mut self,fb:&mut Framebuffer,font:&mut FontRenderer,game:&Game){
        self.hot.clear();
        let width=fb.width as f32;
        let height=fb.height as f32;
        let mobile=height>width*1.15 || width<670.0;
        let logical_width=if mobile {500.0} else {1200.0};
        // A logical minimum height makes the virtual canvas taller than the
        // real client area on maximized/wide windows. That hides bottom UI
        // and causes hit-test clipping. Use the exact physical aspect ratio.
        let logical_height=if width>0.0 {height/width*logical_width} else {height};
        self.scale=width/logical_width;
        // Draw at the actual framebuffer size instead of stretching a smaller bitmap.
        self.origin=(0.0,0.0);
        self.logical=(logical_width,logical_height);
        let mut p=Painter{fb,font,scale:self.scale,origin:self.origin,clip:None};
        p.fill(R::new(0.,0.,logical_width,logical_height),BG);
        // Subtle mine-shaft grid, clipped to viewport.
        for x in (0..(logical_width as i32)).step_by(42){ p.fill(R::new(x as f32,0.,1.,logical_height),0xff111b2a); }
        for y in (0..(logical_height as i32)).step_by(42){ p.fill(R::new(0.,y as f32,logical_width,1.),0xff111b2a); }
        if self.menu_open {
            self.draw_menu(&mut p,game,mobile);
            return;
        }
        self.header(&mut p,game,mobile);
        self.hot.push(Hot{rect:R::new(logical_width-115.,11.,91.,32.),action:Action::MenuOpen});
        self.tabs(&mut p,mobile);
        let top=if mobile{212.0}else{208.0};
        match self.tab {
            Tab::Mines => self.mines(&mut p,game,mobile,top),
            Tab::Prestige => self.prestige(&mut p,game,mobile,top),
            Tab::Research => self.research(&mut p,game,mobile,top),
            Tab::Ledger => self.ledger(&mut p,game,mobile,top),
        }
        if self.min_gain_editing && mobile && self.tab==Tab::Prestige {
            self.touch_min_gain_keyboard(&mut p);
        }
        p.fill(R::new(0.,logical_height-35.,logical_width,35.),0xff121f2f);
        p.label(&self.notice,18.,logical_height-26.,12,MUTED);
    }

    /// Main menu shares native-client rendering/hit-testing with gameplay.
    /// Only currently visible slot rows can be activated.
    fn draw_menu(&mut self,p:&mut Painter<'_>,game:&Game,mobile:bool){
        let w=self.logical.0;
        let h=self.logical.1;
        let pad=if mobile{16.}else{32.};
        p.fill(R::new(0.,0.,w,150.),0xff101b2a);
        p.fill(R::new(0.,0.,w,5.),GOLD);
        p.label("MINERAL EMPIRE",pad,26.,if mobile{25}else{34},WHITE);
        p.label("THE DEEP FRONTIER  /  SAVE MANAGEMENT",pad,if mobile{64.}else{76.},if mobile{11}else{15},MUTED);
        p.label(&format!("ACTIVE SLOT {:02}   |   TREASURY ${}",self.active_slot,number(&game.cash)),pad,113.,if mobile{12}else{15},GOLD);

        let actions:[(&str,Action,u32);4]=[
            ("CONTINUE",Action::MenuResume,0xff65c7a4),
            ("SAVE",Action::MenuSave,0xff517c96),
            ("NEW SLOT",Action::MenuNewSlot,0xff7c9ac3),
            ("RESET CURRENT",Action::MenuResetAsk,0xffb86f6e),
        ];
        let gap=10.;
        let cols=if mobile{2}else{4};
        let button_width=(w-2.*pad-(cols-1) as f32*gap)/cols as f32;
        let button_h=if mobile{42.}else{45.};
        let y0=170.;
        for (i,(label,action,color)) in actions.iter().enumerate(){
            let x=pad+(i%cols) as f32*(button_width+gap);
            let y=y0+(i/cols) as f32*(button_h+gap);
            let rect=R::new(x,y,button_width,button_h);
            p.button(rect,label,true,*color);
            self.hot.push(Hot{rect,action:*action});
        }
        let list_top=if mobile{y0+2.*(button_h+gap)+35.}else{y0+button_h+45.};
        p.label("SAVED EXPEDITIONS",pad,list_top-30.,17,WHITE);
        let viewport=R::new(pad,list_top,w-2.*pad,(h-47.-list_top).max(0.));
        let mut entries=self.menu_slots.clone();
        if !entries.iter().any(|s|s.id==self.active_slot) {
            entries.push(SlotInfo{id:self.active_slot,bytes:0,is_active:true});
        }
        entries.sort_by_key(|s|s.id);
        let row_h=if mobile{77.}else{73.};
        let row_gap=9.;
        let content_h=entries.len() as f32*(row_h+row_gap);
        self.menu_scroll=self.menu_scroll.clamp(0.0,(content_h-viewport.h).max(0.));
        p.set_clip(Some(viewport));
        for (i,slot) in entries.iter().enumerate(){
            let y=list_top+i as f32*(row_h+row_gap)-self.menu_scroll;
            let r=R::new(pad,y,viewport.w,row_h);
            let Some(visible)=r.intersection(viewport) else {continue};
            p.set_clip(Some(visible));
            p.fill(r,if slot.is_active{0xff244258}else{PANEL});
            p.fill(R::new(pad,y,5.,row_h),if slot.is_active{GOLD}else{0xff537f9a});
            p.label(&format!("SLOT {:02}{}",slot.id,if slot.is_active{"  /  ACTIVE"}else{""}),pad+17.,y+10.,if mobile{15}else{18},WHITE);
            let summary=if slot.bytes==0{"New, unsaved expedition".to_owned()}else{format!("Saved game  |  {} KB",(slot.bytes+1023)/1024)};
            p.label(&summary,pad+17.,y+39.,if mobile{11}else{13},MUTED);
            let load_w=if mobile{92.}else{125.};
            let button=R::new(pad+viewport.w-load_w-12.,y+18.,load_w,38.);
            p.button(button,if slot.is_active{"PLAY"}else{"LOAD"},true,0xff537f9a);
            self.add_scrolled_hot(button,Action::MenuLoadSlot(slot.id),viewport);
            p.set_clip(Some(viewport));
        }
        p.set_clip(None);
        p.fill(R::new(0.,h-35.,w,35.),0xff121f2f);
        let footer=if self.notice.len()>90{format!("{}…",self.notice.chars().take(87).collect::<String>())}else{self.notice.clone()};
        p.label(&footer,18.,h-26.,if mobile{10}else{12},MUTED);
        if self.menu_reset_confirm {
            // Modal covers existing buttons and invalidates their hit targets.
            self.hot.clear();
            p.fill(R::new(0.,0.,w,h),0xff101b2a);
            let dialog_width=(w-2.*pad).min(520.);
            let dialog_height=230.;
            let x=(w-dialog_width)/2.;
            let y=((h-dialog_height)/2.).max(10.);
            p.fill(R::new(x,y,dialog_width,dialog_height),PANEL);
            p.fill(R::new(x,y,dialog_width,5.),0xffc87f6e);
            p.label("RESET THIS SLOT?",x+18.,y+20.,if mobile{20}else{24},WHITE);
            p.label(&format!("Slot {:02} will start over.",self.active_slot),x+18.,y+69.,14,MUTED);
            p.label("A backup of its old save will be kept.",x+18.,y+99.,if mobile{11}else{13},MUTED);
            let button_w=(dialog_width-48.)/2.;
            let cancel=R::new(x+16.,y+167.,button_w,44.);
            let confirm=R::new(x+32.+button_w,y+167.,button_w,44.);
            p.button(cancel,"CANCEL",true,0xff517c96);
            p.button(confirm,"RESET SLOT",true,0xffbe766e);
            self.hot.push(Hot{rect:cancel,action:Action::MenuResetCancel});
            self.hot.push(Hot{rect:confirm,action:Action::MenuResetConfirm});
        }
    }

    /// A custom numeric keypad lets the Android/native framebuffer editor work
    /// without relying on a Java IME or a system text widget.
    fn touch_min_gain_keyboard(&mut self,p:&mut Painter<'_>){
        let pad=14.0;
        let gap=6.0;
        let width=self.logical.0-2.0*pad;
        let cell_w=(width-3.0*gap)/4.0;
        let cell_h=35.0;
        let y0=self.logical.1-35.0-(4.0*cell_h+3.0*gap+34.0)-8.0;
        p.fill(R::new(pad-5.,y0-4.,width+10.,4.0*cell_h+3.0*gap+42.),0xff162639);
        p.label("EDIT MINIMUM GAIN  /  SCIENTIFIC NOTATION",pad+5.,y0+5.,11,MUTED);
        let keys:[(&str,Action);16]=[
            ("7",Action::GainChar('7')),("8",Action::GainChar('8')),("9",Action::GainChar('9')),("E",Action::GainChar('e')),
            ("4",Action::GainChar('4')),("5",Action::GainChar('5')),("6",Action::GainChar('6')),("-",Action::GainChar('-')),
            ("1",Action::GainChar('1')),("2",Action::GainChar('2')),("3",Action::GainChar('3')),(".",Action::GainChar('.')),
            ("0",Action::GainChar('0')),("BACK",Action::GainBackspace),("CANCEL",Action::GainCancel),("OK",Action::GainCommit),
        ];
        for (i,(label,action)) in keys.into_iter().enumerate(){
            let x=pad+(i%4) as f32*(cell_w+gap);
            let y=y0+31.0+(i/4) as f32*(cell_h+gap);
            let rect=R::new(x,y,cell_w,cell_h);
            p.button(rect,label,true,if label=="OK"{0xff63bfa5}else{0xff49647e});
            self.hot.push(Hot{rect,action});
        }
    }
    fn header(&self,p:&mut Painter<'_>,game:&Game,mobile:bool){
        let w=self.logical.0;
        p.fill(R::new(0.,0.,w,143.),0xff101b2a);
        p.fill(R::new(0.,0.,w,5.),GOLD);
        p.label("MINERAL  /  EMPIRE",24.,21.,if mobile{24}else{27},WHITE);
        p.label("BUILD  |  EXTRACT  |  TRANSCEND",24.,57.,12,MUTED);
        if mobile{
            p.label(&format!("${}",number(&game.cash)),24.,81.,30,GOLD);
            p.label(&format!("+${}/sec",number(game.total_rate())),270.,93.,14,0xff8de0c6);
        }else{
            p.label("TREASURY",580.,25.,12,MUTED);
            p.label(&format!("${}",number(&game.cash)),580.,42.,32,GOLD);
            p.label(&format!("INCOME   +${} / SEC",number(game.total_rate())),930.,44.,15,0xff8de0c6);
            p.label(&format!("TOTAL EXTRACTED   ${}",number(&game.all_time_earned)),930.,70.,12,MUTED);
        }
        let menu_button=R::new(w-115.,11.,91.,32.);
        p.button(menu_button,"MENU",true,0xff537f9a);
        // The overlay and the regular game use the same hitbox projection.
        // Header normally takes &self; menu hot registration is performed by draw().
        p.fill(R::new(24.,130.,w-48.,1.),0xff304157);
    }
    fn tabs(&mut self,p:&mut Painter<'_>,mobile:bool){
        let w=self.logical.0;
        let tabs=[Tab::Mines,Tab::Prestige,Tab::Research,Tab::Ledger];
        let gap=if mobile{4.0}else{12.0};
        let padx=if mobile{12.0}else{24.0};
        let bw=(w-2.0*padx-gap*3.0)/4.0;
        for (i,tab) in tabs.iter().enumerate(){
            let b=R::new(padx+i as f32*(bw+gap),151.,bw,46.);
            p.button(b,tab.label(),*tab==self.tab,0xff26445b);
            self.hot.push(Hot{rect:b,action:Action::Tab(*tab)});
        }
    }
    fn mines(&mut self,p:&mut Painter<'_>,g:&Game,mobile:bool,top:f32){
        let w=self.logical.0;
        let content_width=if mobile{w-28.0}else{w-382.0};
        let x=if mobile{14.0}else{24.0};
        let qty_y=top+4.0;
        p.label("ACTIVE SHAFTS",x,qty_y+8.,18,WHITE);
        let qx=x+content_width-if mobile{210.0}else{244.0};
        let qtys=[BuyAmount::One,BuyAmount::Ten,BuyAmount::Max];
        for (idx,mode) in qtys.iter().enumerate(){
            let b=R::new(qx+idx as f32*if mobile{72.0}else{82.0},qty_y,if mobile{66.0}else{75.0},35.);
            p.button(b,mode.name(),*mode==self.amount,0xff28455c);
            self.hot.push(Hot{rect:b,action:Action::Quantity(*mode)});
        }
        let start=top+51.;
        let cardh=if mobile{122.0}else{121.0};
        let gap=9.0;
        let viewport=R::new(x,start,content_width,(self.logical.1-43.-start).max(0.0));
        let content_height=COUNT as f32*cardh+(COUNT.saturating_sub(1)) as f32*gap;
        let max_scroll=(content_height-viewport.h).max(0.0);
        self.scroll=self.scroll.clamp(0.0,max_scroll);
        p.set_clip(Some(viewport));
        for i in 0..COUNT {
            let y=start+i as f32*(cardh+gap)-self.scroll;
            let row=R::new(x,y,content_width,cardh);
            // The first row can start *above* the viewport while its bottom
            // is still visible. Do not discard it until fully out of view.
            if row.intersection(viewport).is_none() {continue;}
            let ore=ORES[i];let unlocked=g.unlocked(i);
            p.fill(row,if unlocked{SURFACE}else{0xff101925});
            p.stroke(row,if unlocked{0xff293c50}else{0xff1b293a},1.);
            p.fill(R::new(x,y,4.,cardh),if unlocked{ore.color}else{0xff273344});
            if let Some(image)=self.sprites.get(i) {p.sprite(image,x+12.,y+15.,76.,76.);}
            let name_x=x+99.;
            p.label(ore.name,name_x,y+12.,if mobile{19}else{21},if unlocked{WHITE}else{MUTED});
            p.label(ore.hint,name_x,y+37.,11,MUTED);
            if unlocked {
                p.label(&format!("RIGS  {}   /   TECH  {}",g.miners[i],g.equipment[i]),name_x,y+58.,12,0xffb5c7df);
                p.label(&format!("+${}/s",number(g.ore_rate(i))),name_x,y+79.,14,ore.color);
            }else{
                if !g.ore_tier_available(i) {
                    p.label(&format!("UNLOCK AT PRESTIGE TIER {}",Game::ore_required_tier(i)),
                        name_x,y+66.,12,0xff8295ac);
                }else{
                    p.label(&format!("UNLOCK AT ${} THIS RUN",number(g.unlock_cost(i))),
                        name_x,y+66.,12,0xff8295ac);
                }
            }
            if unlocked {
                let bx=x+content_width-if mobile{170.0}else{180.0};
                let bw=if mobile{153.0}else{162.0};
                let desired=match self.amount{BuyAmount::One=>1,BuyAmount::Ten=>10,BuyAmount::Max=>g.max_affordable(i)};
                let price=g.miner_cost(i,desired);
                let affordable=price<=g.cash && desired>0;
                let buy=R::new(bx,y+12.,bw,43.);
                p.button(buy,&format!("HIRE  ${}",number(&price)),affordable,ore.color);
                self.add_scrolled_hot(buy,Action::Miner(i),viewport);
                let equip=R::new(bx,y+66.,bw,42.);
                let affordable_eq=g.cash>=g.equipment_cost(i)&&g.miners[i]>0;
                p.button(equip,&format!("GEAR  ${}",number(g.equipment_cost(i))),affordable_eq,0xff517c96);
                self.add_scrolled_hot(equip,Action::Equipment(i),viewport);
            }
        }
        p.set_clip(None);
        if !mobile {
            let xx=x+content_width+16.;let ww=w-xx-24.;
            let sidebar=SidebarLayout::new(xx,ww,top,self.logical.1);
            let multiplier=sidebar.multiplier;
            p.fill(multiplier,PANEL);
            p.label("EMPIRE MULTIPLIER",xx+18.,multiplier.y+17.,13,MUTED);
            p.label(&format!("x{}",number(g.global_multiplier())),xx+18.,multiplier.y+40.,35,GOLD);
            // As the first card shrinks, tighten the legacy spacing too.
            let progress=((multiplier.h-170.)/65.).clamp(0.0,1.0);
            let legacy_heading=multiplier.y+75.+progress*29.;
            let legacy_start=legacy_heading+27.;
            let legacy_spacing=17.+progress*5.;
            p.label("PERMANENT LEGACIES",xx+18.,legacy_heading,14,WHITE);
            // The sidebar is deliberately compact even with fourteen tiers.
            // Show the most recent unlocked layer and its neighboring balances.
            let highest=(0..PRESTIGE_COUNT).rev()
                .find(|&i|g.prestige_lifetime[i].is_positive() || g.prestige[i].is_positive())
                .unwrap_or(0);
            let first=highest.saturating_sub(3);
            for (line,i) in (first..=(first+3).min(PRESTIGE_COUNT-1)).enumerate() {
                p.label(&format!("{}  {}",STAGES[i].currency,number(&g.prestige[i])),
                    xx+18.,legacy_start+line as f32*legacy_spacing,13,STAGES[i].accent);
            }

            let milestone=sidebar.milestone;
            p.fill(milestone,PANEL);
            p.label("NEXT MILESTONE",xx+18.,milestone.y+12.,15,WHITE);
            let next=(1..COUNT).find(|&i| !g.unlocked(i));
            if let Some(next)=next {
                p.label(ORES[next].name,xx+18.,milestone.y+36.,20,ORES[next].color);
                if milestone.h>=86. {
                    let requirement=if !g.ore_tier_available(next){
                        format!("Reach prestige tier {}",Game::ore_required_tier(next))
                    }else{
                        format!("$ {} run earnings",number(g.unlock_cost(next)))
                    };
                    p.label(&requirement,xx+18.,milestone.y+64.,13,MUTED);
                }
            }else{
                p.label("All shafts discovered",xx+18.,milestone.y+37.,17,0xff8de0c6);
            }

            let controls=sidebar.controls;
            p.fill(controls,PANEL);
            p.label("EMPIRE CONTROLS",xx+14.,controls.y+10.,13,WHITE);
            if ww>=285. && ((g.auto_buy && g.auto_hire_unlocked()) ||
                (g.auto_gear && g.auto_gear_unlocked())) {
                let until=(crate::engine::AUTO_PURCHASE_INTERVAL-g.auto_economy_elapsed)
                    .clamp(0.,crate::engine::AUTO_PURCHASE_INTERVAL).ceil() as u64;
                p.label(&format!("NEXT AUTO: {until}s"),xx+160.,controls.y+11.,11,MUTED);
            }
            let toggle_w=(ww-29.)/2.;
            let auto=R::new(xx+12.,controls.y+34.,toggle_w,30.);
            p.button(auto,if g.auto_buy{"AUTO-HIRE: ON"}else{"AUTO-HIRE: OFF"},
                g.auto_hire_unlocked(),0xff64cfa7);
            self.hot.push(Hot{rect:auto,action:Action::ToggleAuto});
            let gear=R::new(xx+17.+toggle_w,controls.y+34.,toggle_w,30.);
            p.button(gear,if g.auto_gear{"AUTO-GEAR: ON"}else{"AUTO-GEAR: OFF"},
                g.auto_gear_unlocked(),0xff83b9ed);
            self.hot.push(Hot{rect:gear,action:Action::ToggleGear});
            let sound=R::new(xx+12.,controls.y+70.,(ww-29.)/2.,26.);
            p.button(sound,if self.sound_enabled{"SOUND ON"}else{"SOUND OFF"},true,0xff547a9e);
            self.hot.push(Hot{rect:sound,action:Action::ToggleSound});
            let save=R::new(xx+17.+(ww-29.)/2.,controls.y+70.,(ww-29.)/2.,26.);
            p.button(save,"SAVE",true,0xff547a9e);
            self.hot.push(Hot{rect:save,action:Action::Save});
        }
    }

    fn prestige(&mut self,p:&mut Painter<'_>,g:&Game,mobile:bool,top:f32){
        let w=self.logical.0;
        let left=if mobile{14.0}else{24.0};
        let gap=if mobile{13.0}else{18.0};
        let columns=if mobile{1}else{2};
        let boxw=(w-2.0*left-gap*(columns-1) as f32)/columns as f32;
        let boxh=if mobile{178.0}else{218.0};

        // Fixed, clickable minimum-gain text field above the scroll viewport.
        // One Auto-Prestige unlock controls *every lower tier*, never the
        // player's current highest reached tier.
        let current=g.current_prestige_tier();
        let controls=R::new(left,top+3.,w-2.0*left,if mobile{112.0}else{91.0});
        p.fill(controls,PANEL);
        p.label("AUTO-PRESTIGE  /  LOWER TIERS ONLY",left+12.,top+8.,if mobile{11}else{13},MUTED);
        let available=g.auto_prestige_unlocked() && current>0;
        let toggle=R::new(left+8.,top+34.,if mobile{143.0}else{210.0},36.);
        p.button(toggle,if g.auto_prestige{"AUTO: ON"}else{"AUTO: OFF"},available,0xff63bfa5);
        self.hot.push(Hot{rect:toggle,action:Action::ToggleAutoPrestige});
        let field=R::new(toggle.x+toggle.w+8.,toggle.y,(controls.x+controls.w-8.-(toggle.x+toggle.w+8.)).max(50.),36.);
        let text=if self.min_gain_editing {
            format!("MIN GAIN  {}|",self.min_gain_input)
        } else {
            format!("MIN GAIN  {}  (EDIT)",number(g.auto_prestige_target_gain()))
        };
        p.input_field(field,&text,self.min_gain_editing);
        self.hot.push(Hot{rect:field,action:Action::EditMinGain});
        let status=if current==0 {
            "Reach a second prestige tier to automate lower tiers.".to_owned()
        } else {
            format!("CURRENT: {}  /  AUTO TIERS 1-{}{}",STAGES[current].name,current,
                if available{""}else{"  /  RESEARCH AUTO-PRESTIGE TO UNLOCK"})
        };
        p.label(&status,left+12.,top+if mobile{81.0}else{75.0},if mobile{10}else{11},MUTED);
        let start=top+controls.h+14.;
        let viewport=R::new(left,start,w-left*2.,(self.logical.1-43.-start).max(0.0));
        let rows=PRESTIGE_COUNT.div_ceil(columns);
        let content_height=rows as f32*boxh+(rows.saturating_sub(1)) as f32*gap;
        self.scroll=self.scroll.clamp(0.0,(content_height-viewport.h).max(0.0));
        p.set_clip(Some(viewport));
        for i in 0..PRESTIGE_COUNT {
            let col=i%columns;let row=i/columns;
            let x=left+col as f32*(boxw+gap);
            let y=start+row as f32*(boxh+gap)-self.scroll;
            let card=R::new(x,y,boxw,boxh);
            let Some(visible)=card.intersection(viewport) else {continue};
            // Clip each prestige card independently to avoid long descriptions
            // spilling into the neighboring column while partially scrolled.
            p.set_clip(Some(visible));
            let stage=STAGES[i];let can=g.prestige_available(i);
            p.fill(card,PANEL);
            p.fill(R::new(x,y,boxw,5.),stage.accent);
            p.label(&format!("TIER {:02}: {}",i+1,stage.name),x+16.,y+13.,if mobile{18}else{21},WHITE);
            p.label(&format!("{}: {}",stage.currency,number(&g.prestige[i])),x+16.,y+43.,if mobile{16}else{19},stage.accent);
            p.label(stage.description,x+16.,y+76.,if mobile{11}else{13},MUTED);
            p.label(&format!("Requires ${} earned this run",stage.threshold),x+16.,y+107.,12,MUTED);
            if i>0 {
                p.label(&format!("Prior tier needs {} lifetime units",Game::prestige_prerequisite(i)),
                    x+16.,y+128.,11,MUTED);
            }
            let b=R::new(x+16.,y+boxh-55.,boxw-32.,41.);
            let text=if can {
                if self.pending==Some(i) {"CONFIRM RESET".to_string()}
                else {format!("ASCEND  +{} {}",number(g.prestige_gain(i)),stage.currency)}
            }else {"LOCKED - KEEP MINING".into()};
            p.button(b,&text,can,stage.accent);
            self.add_scrolled_hot(b,Action::Ascend(i),viewport);
            p.set_clip(Some(viewport));
        }
        p.set_clip(None);
    }
    fn research(&mut self,p:&mut Painter<'_>,g:&Game,mobile:bool,top:f32){
        let w=self.logical.0;
        let pad=if mobile{14.0}else{24.0};let bw=w-2.0*pad;
        p.label("RESEARCH LABORATORY",pad,top+2.,19,WHITE);
        if !mobile {
            p.label("Upgrade your empire with tier-specific specializations.",pad,top+31.,13,MUTED);
        }
        // Both quantity selection and the automation switch are outside of
        // the scrollable rows, so click targets remain fixed while scrolling.
        let auto_width=if mobile{118.0}else{188.0};
        let auto=R::new(pad+bw-auto_width,top+3.,auto_width,34.);
        p.button(auto,if g.auto_research{"AUTO-RESEARCH ON"}else{"AUTO-RESEARCH OFF"},
            g.auto_research_unlocked(),0xff65cea9);
        self.hot.push(Hot{rect:auto,action:Action::ToggleAutoResearch});
        let qty_y=top+52.;
        p.label("BUY RESEARCH",pad,qty_y+9.,if mobile{11}else{13},MUTED);
        let amount_width=if mobile{63.0}else{80.0};
        let amount_gap=if mobile{5.0}else{8.0};
        let total_width=ResearchAmount::ALL.len() as f32*amount_width+3.0*amount_gap;
        let start_x=pad+bw-total_width;
        for (i,qty) in ResearchAmount::ALL.iter().enumerate(){
            let b=R::new(start_x+i as f32*(amount_width+amount_gap),qty_y,amount_width,34.);
            p.button(b,qty.label(),*qty==self.research_amount,0xff4b839d);
            self.hot.push(Hot{rect:b,action:Action::ResearchQuantity(*qty)});
        }
        let list_top=top+99.;
        let rowh=if mobile{124.0}else{104.0};
        let viewport=R::new(pad,list_top,bw,(self.logical.1-43.-list_top).max(0.0));
        let content_height=(PRESTIGE_COUNT*RESEARCH_COUNT) as f32*rowh+
            (PRESTIGE_COUNT*RESEARCH_COUNT-1) as f32*7.0;
        self.scroll=self.scroll.clamp(0.0,(content_height-viewport.h).max(0.0));
        p.set_clip(Some(viewport));
        for i in 0..PRESTIGE_COUNT {
            for j in 0..RESEARCH_COUNT {
                let index=i*RESEARCH_COUNT+j;
                let y=list_top+index as f32*(rowh+7.0)-self.scroll;
                let row=R::new(pad,y,bw,rowh);
                let Some(visible)=row.intersection(viewport) else {continue};
                p.set_clip(Some(visible));
                let stage=STAGES[i];let owned=g.research[i][j];
                p.fill(row,PANEL);
                p.fill(R::new(pad,y,4.,rowh),stage.accent);
                let xx=pad+16.;
                let name=research_name(i,j);
                p.label(&format!("{} / {}",stage.name,name),xx,y+12.,if mobile{15}else{18},WHITE);
                p.label(&format!("Level {}",owned),xx,y+39.,14,stage.accent);
                p.label(research_description(i,j),xx,y+63.,12,MUTED);
                let request=self.research_amount.count();
                let total=g.research_bulk_cost(i,j,request);
                let maxed=owned>=research_rank_limit(j);
                let can_buy=!maxed && g.prestige[i]>=g.research_cost(i,j);
                let b=R::new(pad+bw-if mobile{155.0}else{214.0},y+12.,if mobile{141.0}else{194.0},42.);
                let caption=if maxed {"MAXED".to_string()}
                    else{format!("{}  {}",self.research_amount.label(),number(&total))};
                p.button(b,&caption,can_buy,stage.accent);
                self.add_scrolled_hot(b,Action::Research(i,j),viewport);
                p.set_clip(Some(viewport));
            }
        }
        p.set_clip(None);
    }
    fn ledger(&mut self,p:&mut Painter<'_>,g:&Game,mobile:bool,top:f32){
        let pad=if mobile{14.0}else{24.0};let w=self.logical.0;
        p.fill(R::new(pad,top+6.,w-pad*2.,200.),PANEL);
        p.label("THE GEOLOGIST'S LEDGER",pad+18.,top+22.,22,WHITE);
        let stats=[
            format!("Total ore revenue: ${}",number(&g.all_time_earned)),
            format!("Current run: ${}",number(&g.run_earned)),
            format!("Time exploring: {:.1} hours",g.elapsed_seconds/3600.),
            format!("Achievements: {} / 8",g.achievements.iter().filter(|&&a|a).count()),
        ];
        for (i,item) in stats.iter().enumerate(){p.label(item,pad+18.,top+62.+i as f32*29.,14,MUTED);}
        let reached=g.prestige_lifetime.iter().filter(|x|x.is_positive()).count();
        p.label(&format!("Prestige discoveries: {}/{}    |    Current tier: {}    |    Auto: lower tiers",
            reached,PRESTIGE_COUNT,STAGES[g.current_prestige_tier()].name),pad+18.,top+176.,12,MUTED);
        let actions=[
            (Action::ToggleAuto,if g.auto_buy{"AUTO HIRE"}else{"HIRE OFF"}),
            (Action::ToggleGear,if g.auto_gear{"AUTO GEAR"}else{"GEAR OFF"}),
            (Action::ToggleAutoPrestige,if g.auto_prestige{"AUTO RESET"}else{"RESET OFF"}),
            (Action::ToggleSound,if self.sound_enabled{"SOUND ON"}else{"SOUND OFF"}),
            (Action::Save,"SAVE NOW")
        ];
        let button_width=(w-2.0*pad-32.)/5.;
        for (i,(action,label)) in actions.into_iter().enumerate(){
            let r=R::new(pad+i as f32*(button_width+8.),top+218.,button_width,39.);
            p.button(r,label,true,0xff4d8c93);
            self.hot.push(Hot{rect:r,action});
        }
        p.label("MILESTONES",pad,top+274.,19,WHITE);
        let names=["First crew", "Millionaire", "Five shafts", "Land surveyor", "Master smelter", "Public company", "Star miner", "Mythril discovered"];
        for i in 0..8 {
            let y=top+308.+i as f32*44.;
            if y>self.logical.1-60. {break;}
            p.fill(R::new(pad,y,w-2.0*pad,37.),if g.achievements[i]{0xff1d3d42}else{0xff192738});
            p.label(if g.achievements[i]{"[DONE]"}else{"[LOCK]"},pad+12.,y+9.,12,if g.achievements[i]{0xff84e7b4}else{MUTED});
            p.label(names[i],pad+95.,y+8.,14,WHITE);
        }
    }
}

/// Determine an actual fitting label before drawing. A clip alone only
/// hides overflow; measuring text ensures the visible part is legible.
fn fit_button_label(font:&mut FontRenderer,label:&str,width_px:u32,scale:f32,preferred:u16,minimum:u16)->(String,u16){
    if width_px==0{return (String::new(),minimum)}
    let physical=|pt:u16|((pt as f32*scale).round().clamp(8.,46.)) as u16;
    for pt in (minimum..=preferred).rev(){
        if font.measure(label,physical(pt)).map(|(w,_)|w<=width_px).unwrap_or(false){
            return (label.to_owned(),pt);
        }
    }
    // Preserve a useful prefix, including the action, even when a decimal
    // exponent is thousands of digits long. Binary search is Unicode-safe.
    let chars:Vec<char>=label.chars().collect();
    let mut low=0usize;
    let mut high=chars.len();
    while low<high {
        let mid=low+(high-low+1)/2;
        let candidate=format!("{}…",chars[..mid].iter().collect::<String>());
        let fits=font.measure(&candidate,physical(minimum))
            .map(|(w,_)|w<=width_px).unwrap_or(false);
        if fits{low=mid}else{high=mid-1}
    }
    let out=if low==0 {
        if font.measure("…",physical(minimum)).map(|(w,_)|w<=width_px).unwrap_or(false){"…".to_owned()}else{String::new()}
    }else{format!("{}…",chars[..low].iter().collect::<String>())};
    (out,minimum)
}

struct Painter<'a>{fb:&'a mut Framebuffer,font:&'a mut FontRenderer,scale:f32,origin:(f32,f32),clip:Option<(i32,i32,i32,i32)>}
impl Painter<'_>{
    fn set_clip(&mut self,rect:Option<R>){
        self.clip=rect.map(|r|self.project(r));
    }
    fn clip_bounds(&self)->(i32,i32,i32,i32){
        let screen=(0,0,self.fb.width as i32,self.fb.height as i32);
        match self.clip {
            Some((l,t,r,b))=>(l.max(screen.0),t.max(screen.1),r.min(screen.2),b.min(screen.3)),
            None=>screen,
        }
    }
    fn project(&self,r:R)->(i32,i32,i32,i32){
        physical_rect(r,self.scale,self.origin)
    }
    fn fill(&mut self,r:R,color:u32){
        let (x,y,x1,y1)=self.project(r);
        let (clip_left,clip_top,clip_right,clip_bottom)=self.clip_bounds();
        let fw=self.fb.width as i32;
        for cy in y.max(clip_top)..y1.min(clip_bottom){
            let left=x.max(clip_left);let right=x1.min(clip_right);
            if right<=left{continue;}
            let start=cy as usize*fw as usize+left as usize;
            self.fb.pixels[start..start+(right-left) as usize].fill(color);
        }
    }
    fn stroke(&mut self,r:R,color:u32,b:f32){
        self.fill(R::new(r.x,r.y,r.w,b),color);
        self.fill(R::new(r.x,r.y+r.h-b,r.w,b),color);
        self.fill(R::new(r.x,r.y,b,r.h),color);
        self.fill(R::new(r.x+r.w-b,r.y,b,r.h),color);
    }
    fn label(&mut self,text:&str,x:f32,y:f32,sz:u16,color:u32){
        let scale=self.scale;
        let px=(x*scale+self.origin.0).round() as i32;
        let py=(y*scale+self.origin.1).round() as i32;
        let size=((sz as f32)*scale).round().clamp(8.0,46.0) as u16;
        let (l,t,r,b)=self.clip_bounds();
        if r<=l || b<=t {return;}
        let clip=Rect::new(l,t,(r-l) as u32,(b-t) as u32);
        let _=self.font.draw_text(self.fb,text,px,py,size,color,Some(clip));
    }
    fn button(&mut self,r:R,label:&str,active:bool,color:u32){
        self.fill(r,if active{color}else{0xff253448});
        self.stroke(r,if active{0xff648ea2}else{0xff3a4b5e},1.);
        // Fit by *glyph advance width* in physical pixels, never by the
        // number of characters. The old algorithm failed on huge exponents.
        let saved=self.clip;
        let (l,t,right,bottom)=self.project(r);
        self.clip=Some(match saved {
            Some((sl,st,sr,sb))=>(l.max(sl),t.max(st),right.min(sr),bottom.min(sb)),
            None=>(l,t,right,bottom),
        });
        let available=(right-l-16).max(0) as u32;
        let (fitted,size)=fit_button_label(self.font,label,available,self.scale,13,9);
        self.label(&fitted,r.x+8.,r.y+(r.h-17.)/2.,size,if active{0xff071422}else{0xffafbdcb});
        self.clip=saved;
    }
    fn input_field(&mut self,r:R,text:&str,focused:bool){
        self.fill(r,0xff0d1724);
        self.stroke(r,if focused{0xff86dbc5}else{0xff6c829e},if focused{2.}else{1.});
        let old=self.clip;
        let (l,t,right,bottom)=self.project(r);
        self.clip=Some(match old {
            Some((sl,st,sr,sb))=>(l.max(sl),t.max(st),right.min(sr),bottom.min(sb)),
            None=>(l,t,right,bottom),
        });
        // Keep the active insertion point visible for very long exponents.
        let display=if focused && text.len()>38 {
            format!("…{}", &text[text.len()-37..])
        } else {text.to_owned()};
        self.label(&display,r.x+9.,r.y+8.,12,WHITE);
        self.clip=old;
    }
    fn sprite(&mut self,image:&Image,x:f32,y:f32,w:f32,h:f32){
        if image.width==0 || image.height==0{return;}
        let (x0,y0,x1,y1)=self.project(R::new(x,y,w,h));
        let sw=image.width as usize;let sh=image.height as usize;
        let ww=(x1-x0).max(1) as usize;let hh=(y1-y0).max(1) as usize;
        let (clip_left,clip_top,clip_right,clip_bottom)=self.clip_bounds();
        let fw=self.fb.width as i32;
        for dy in y0.max(clip_top)..y1.min(clip_bottom){
            let iy=((dy-y0) as usize*sh/hh).min(sh-1);
            for dx in x0.max(clip_left)..x1.min(clip_right){
                let ix=((dx-x0) as usize*sw/ww).min(sw-1);
                let pixel=image.pixels[iy*sw+ix];
                let a=(pixel&255) as u32;if a==0{continue;}
                let index=dy as usize*fw as usize+dx as usize;
                let bg=self.fb.pixels[index];
                let sr=(pixel>>24)&255;let sg=(pixel>>16)&255;let sb=(pixel>>8)&255;
                let inv=255-a;
                let r=(sr*a+((bg>>16)&255)*inv)/255;
                let g=(sg*a+((bg>>8)&255)*inv)/255;
                let b=(sb*a+(bg&255)*inv)/255;
                self.fb.pixels[index]=0xff00_0000|(r<<16)|(g<<8)|b;
            }
        }
    }
}

#[cfg(test)]
mod resize_tests {
    use super::*;
    #[test]
    fn menu_actions_and_reset_modal_cover_gameplay(){
        let mut font=FontRenderer::from_bytes(include_bytes!("../assets/fonts/DejaVuSans.ttf")).unwrap();
        let mut interface=Interface::new();
        interface.update_slots(vec![SlotInfo{id:1,bytes:4096,is_active:true},SlotInfo{id:2,bytes:999,is_active:false}],1);
        let game=Game::default();
        for (w,h) in [(1200,720),(500,860)] {
            let mut framebuffer=Framebuffer::from_color(w,h,BG);
            interface.draw(&mut framebuffer,&mut font,&game);
            let click=if w==1200 {(620,190)}else{(50,250)};
            assert!(matches!(interface.hit(click.0,click.1),Some(Action::MenuNewSlot)));
            interface.menu_reset_confirm=true;
            interface.draw(&mut framebuffer,&mut font,&game);
            assert!(interface.hit(click.0,click.1).is_none(),"modal must block underlying menu actions");
            interface.menu_reset_confirm=false;
        }
    }
    #[test]
    fn long_button_labels_fit_without_overflow(){
        let mut font=FontRenderer::from_bytes(include_bytes!("../assets/fonts/DejaVuSans.ttf")).unwrap();
        for width in [75,100,141,162,194,240] {
            for scale in [1.,1.25,1.7] {
                let label="HIRE  $3.472e27999999999999999999999999999999999";
                let (text,size)=fit_button_label(&mut font,label,width,scale,13,9);
                let px=(size as f32*scale).round().clamp(8.,46.) as u16;
                assert!(font.measure(&text,px).unwrap().0<=width);
            }
        }
    }



    #[test]
    fn min_gain_textfield_commit_and_cancel(){
        use mata_win::events::Key;
        let mut ui=Interface::new();let mut game=Game::default();
        ui.focus_min_gain(&game);
        for _ in 0..ui.min_gain_input.len(){ui.input_min_gain(Key::Backspace,&mut game);}
        for key in [Key::Num(2),Key::Char('e'),Key::Num(5),Key::Num(0),Key::Num(0)]{
            ui.input_min_gain(key,&mut game);
        }
        assert_eq!(game.auto_prestige_min_gain,crate::big::Big::one());
        assert!(ui.input_min_gain(Key::Enter,&mut game).unwrap().is_ok());
        assert_eq!(game.auto_prestige_min_gain,"2e500".parse().unwrap());
        ui.focus_min_gain(&game);
        ui.input_min_gain(Key::Escape,&mut game);
        assert!(!ui.min_gain_editing);
        assert_eq!(game.auto_prestige_min_gain,"2e500".parse().unwrap());
    }
    #[test]
    fn large_prestige_catalog_scrolls_and_keeps_fixed_controls(){
        let rows=PRESTIGE_COUNT.div_ceil(2);
        let viewport=R::new(24.,310.,1152.,400.);
        let card_h=218.;let gap=18.;
        let total=rows as f32*card_h+(rows-1) as f32*gap;
        assert!(total>viewport.h);
        assert!(R::new(24.,310.-30.,550.,card_h).intersection(viewport).is_some());
        assert_eq!(PRESTIGE_COUNT,14);
    }
    #[test]
    fn partially_scrolled_items_remain_visible_until_fully_outside(){
        let viewport=R::new(24.,260.,794.,400.);
        let top_cut=R::new(24.,190.,794.,121.);
        let visible=top_cut.intersection(viewport).expect("row bottom still visible");
        assert_eq!((visible.y,visible.h),(260.,51.));
        assert!(R::new(24.,139.,794.,121.).intersection(viewport).is_none());
        let bottom_cut=R::new(24.,625.,794.,121.);
        let visible=bottom_cut.intersection(viewport).expect("row top still visible");
        assert_eq!((visible.y,visible.h),(625.,35.));
    }

    #[test]
    fn clipped_scroll_hitboxes_match_visible_pixels_at_various_scales(){
        let viewport=R::new(24.,260.,794.,400.);
        let buy=R::new(680.,241.,138.,43.);
        let mut ui=Interface::new();
        ui.logical=(1200.,700.);
        ui.add_scrolled_hot(buy,Action::Miner(2),viewport);
        assert_eq!(ui.hot.len(),1);
        assert_eq!(ui.hot[0].rect.y,260.);
        assert_eq!(ui.hot[0].rect.h,24.);
        for scale in [1.0,1.2,1.36]{
            ui.scale=scale;
            let (l,t,r,b)=physical_rect(ui.hot[0].rect,scale,(0.,0.));
            assert!(matches!(ui.hit((l+r)/2,(t+b)/2),Some(Action::Miner(2))));
            assert!(ui.hit((l+r)/2,t-1).is_none());
            assert!(ui.hit((l+r)/2,b).is_none());
        }
    }
    #[test]
    fn hitboxes_match_exact_rasterized_rectangles_at_fractional_scales(){
        let mut ui=Interface::new();
        ui.logical=(1200.,760.);
        let control=R::new(910.,310.,162.,43.);
        ui.hot.push(Hot{rect:control,action:Action::Miner(3)});
        for (width,height) in [(1220,800),(1630,950),(900,720),(1201,801)] {
            ui.scale=width as f32/1200.;
            ui.logical=(1200.,height as f32/ui.scale);
            let (x0,y0,x1,y1)=physical_rect(control,ui.scale,ui.origin);
            assert!(matches!(ui.hit((x0+x1)/2,(y0+y1)/2),Some(Action::Miner(3))));
            assert!(ui.hit(x0-1,(y0+y1)/2).is_none());
            assert!(ui.hit(x1,(y0+y1)/2).is_none());
        }
    }

    #[test]
    fn sidebar_controls_stay_above_footer_across_desktop_sizes(){
        // Covers 16:9 maximized windows (<720 logical pixels), wide layouts,
        // and ordinary restored windows without the platform renderer.
        for height in [640.,660.,680.,700.,720.,740.,760.,780.,800.,900.] {
            let layout=SidebarLayout::new(858.,318.,208.,height);
            let footer_top=height-35.;
            assert!(layout.multiplier.h>=160.);
            assert!(layout.milestone.h>=85.);
            assert!(layout.multiplier.y+layout.multiplier.h+10.<=layout.milestone.y);
            assert!(layout.milestone.y+layout.milestone.h+10.<=layout.controls.y);
            assert!(layout.controls.y+layout.controls.h<=footer_top-10.);
            let sound_bottom=layout.controls.y+70.+26.;
            assert!(sound_bottom<=layout.controls.y+layout.controls.h);
            assert!(sound_bottom<footer_top);
        }
    }

    #[test]
    fn maximized_window_controls_hit_test_inside_client_area(){
        for (client_width,client_height) in [(1632.,923.),(1920.,1040.),(1220.,790.)] {
            let logical_height=client_height/client_width*1200.;
            let scale=client_width/1200.;
            let layout=SidebarLayout::new(858.,318.,208.,logical_height);
            let mut ui=Interface::new();
            ui.logical=(1200.,logical_height);
            ui.scale=scale;
            let sound=R::new(layout.controls.x+12.,layout.controls.y+70.,
                (layout.controls.w-29.)/2.,26.);
            let save=R::new(layout.controls.x+17.+(layout.controls.w-29.)/2.,
                layout.controls.y+70.,(layout.controls.w-29.)/2.,26.);
            ui.hot.push(Hot{rect:sound,action:Action::ToggleSound});
            ui.hot.push(Hot{rect:save,action:Action::Save});
            for (button,is_sound) in [(sound,true),(save,false)] {
                let (left,top,right,bottom)=physical_rect(button,scale,(0.,0.));
                assert!(bottom<=client_height as i32, "button leaves window after resize");
                assert!(left>=0 && right<=client_width as i32 && top>=0);
                let expected=if is_sound {Action::ToggleSound} else {Action::Save};
                assert!(matches!((ui.hit((left+right)/2,(top+bottom)/2),expected),
                    (Some(Action::ToggleSound),Action::ToggleSound)|(Some(Action::Save),Action::Save)));
            }
        }
    }

    #[test]
    fn footer_does_not_activate_scrolled_controls(){
        let mut ui=Interface::new();
        ui.logical=(1200.,720.);
        ui.scale=1.0;
        ui.hot.push(Hot{rect:R::new(100.,680.,180.,50.),action:Action::Miner(0)});
        assert!(ui.hit(150,700).is_none());
        assert!(matches!(ui.hit(150,682),Some(Action::Miner(0))));
    }
}
