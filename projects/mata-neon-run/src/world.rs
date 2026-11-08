//! Gameplay, simple rigid-player physics, collision, hitscan and enemy AI.
use crate::math::{V3,Box3,ray_sphere};
use crate::campaign::{self,RECORD_COUNT,MISSION_COUNT};
use crate::enemy_motion::Motion;
use crate::enemy_ai::{self,Awareness};
use crate::sound::SoundCue;
use crate::weapons::{Gun,Magazine,WEAPON_COUNT};

pub const ARENA:f32=36.0;
#[derive(Clone,Copy,PartialEq,Eq,Debug)]pub enum Mode {Playing,Paused,Between,Won,Lost}
#[derive(Clone,Copy,PartialEq,Eq,Debug,Hash)]pub enum BotKind{Runner,Striker,Tank,Marksman,Gunner}
impl BotKind {
    pub const fn ammo_gun(self)->Gun {
        match self {
            Self::Runner=>Gun::Carbine,Self::Tank=>Gun::Cannon,
            Self::Marksman=>Gun::Sniper,Self::Striker=>Gun::Shotgun,
            Self::Gunner=>Gun::Minigun,
        }
    }
}
#[derive(Clone,Copy,PartialEq,Eq,Debug)]pub enum PickupKind{Health,Shield,Ammo(Gun,i32)}
#[derive(Clone,Copy)]pub struct Bot{
    pub pos:V3,pub hp:f32,pub kind:BotKind,pub cooldown:f32,pub phase:f32,
    pub flash:f32,pub motion:Motion,
    pub awareness:Awareness,pub last_known:V3,pub search_left:f32,
    pub nav_waypoint:V3,pub nav_clock:f32,
}
impl Bot {
    fn new(pos:V3,kind:BotKind,hp:f32,cooldown:f32,phase:f32)->Self{
        let mut motion=Motion::new(pos,kind);
        // Initial orientations differ. Idle scanning is independent of Rook.
        motion.facing=phase.rem_euclid(std::f32::consts::TAU);
        Self{pos,hp,kind,cooldown,phase,flash:0.0,motion,
            awareness:Awareness::Idle,last_known:pos,search_left:0.0,
            nav_waypoint:pos,nav_clock:0.0}
    }
    fn struck_by_player(&mut self,where_from:V3){
        self.awareness=Awareness::Investigating;
        self.last_known=where_from;
        self.search_left=7.0;
        self.nav_clock=0.0;
    }
}
#[derive(Clone,Copy)]pub struct Shot{pub pos:V3,pub velocity:V3,pub age:f32,pub color:u32}
#[derive(Clone,Copy)]pub struct Pickup{pub pos:V3,pub kind:PickupKind,pub phase:f32}
#[derive(Clone,Copy)]pub struct Spark{pub pos:V3,pub velocity:V3,pub life:f32,pub color:u32}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub enum StoryView{Prologue,Mission,Record(usize),Epilogue}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub enum SiteKind{Record(usize),Relay,Key,Exit,Uplink}
#[derive(Clone,Copy)]pub struct Site{pub pos:V3,pub kind:SiteKind,pub used:bool}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub enum DecorKind {
    CeilingLight,HangingLamp,VentFan,Terminal,ConsoleBank,ServerRack,PipeBundle,
    CoolantTank,Crate,Chair,Desk,WallMonitor,WarningBeacon,FloorGrate,CeilingDuct,WallSign,
}
#[derive(Clone,Copy)]pub struct Decor{pub pos:V3,pub kind:DecorKind,pub rotation:f32}


#[derive(Clone)]
pub struct World {
    pub mode:Mode,pub stage:usize,pub score:u32,pub kills:u32,pub combo:u32,pub combo_left:f32,
    pub health:f32,pub shield:f32,pub ammo:[Magazine;WEAPON_COUNT],pub weapon:usize,
    pub ads:bool, pub aim_blend:f32, pub reload_weapon:usize, pub reload_total:f32, pub weapon_spin:f32,
    pub reload:f32,pub fire_cd:f32,pub dash_cd:f32,pub muzzle:f32,pub hurt:f32,
    pub foot:V3,pub vertical_speed:f32,pub on_ground:bool,pub yaw:f32,pub pitch:f32,
    pub time:f32,pub walk:f32,pub wave_clock:f32,pub elapsed:f32,
    pub boxes:Vec<Box3>,pub bots:Vec<Bot>,pub shots:Vec<Shot>,pub pickups:Vec<Pickup>,pub sparks:Vec<Spark>,
    pub last_hit:bool,pub showing_help:bool,pub show_stats:bool,pub high_quality:bool,
    pub sounds:Vec<SoundCue>,
    pub sites:Vec<Site>,pub decor:Vec<Decor>,pub records_found:[bool;RECORD_COUNT],pub story:Option<StoryView>,
    pub has_key:bool,pub relays_done:usize,pub relays_total:usize,
    pub boss_pos:V3,pub boss_hp:f32,pub boss_max_hp:f32,pub boss_flash:f32,
    pub boss_target:Option<V3>,pub boss_charge:f32,pub boss_cooldown:f32,
    pub boss_beam:f32,pub boss_beam_pos:V3,pub boss_reserves:u8,
    pub notice:&'static str,pub notice_time:f32,
    rng:u64,
}
impl World {
    pub fn new()->Self{
        let mut w=Self{mode:Mode::Playing,stage:0,score:0,kills:0,combo:0,combo_left:0.0,
            health:100.0,shield:60.0,ammo:Gun::ALL.map(Magazine::starting),weapon:0,
            ads:false,aim_blend:0.0,reload_weapon:0,reload_total:0.0,weapon_spin:0.0,reload:0.0,fire_cd:0.0,dash_cd:0.0,muzzle:0.0,hurt:0.0,
            foot:V3::new(18.0,0.0,4.0),vertical_speed:0.0,on_ground:true,yaw:0.0,pitch:0.0,
            time:0.0,walk:0.0,wave_clock:0.0,elapsed:0.0,boxes:Vec::new(),bots:Vec::new(),shots:Vec::new(),pickups:Vec::new(),sparks:Vec::new(),
            last_hit:false,showing_help:false,show_stats:false,high_quality:true,sounds:Vec::with_capacity(32),
            sites:Vec::new(),decor:Vec::new(),records_found:[false;RECORD_COUNT],story:Some(StoryView::Prologue),
            has_key:false,relays_done:0,relays_total:0,
            boss_pos:V3::new(18.0,0.0,18.0),boss_hp:0.0,boss_max_hp:600.0,boss_flash:0.0,
            boss_target:None,boss_charge:0.0,boss_cooldown:3.0,boss_beam:0.0,
            boss_beam_pos:V3::new(0.0,0.0,0.0),boss_reserves:0,
            notice:"",notice_time:0.0,rng:0xA57C_F0E3_1D24_55AB};
        w.start_mission();w
    }
    fn rand(&mut self)->f32{
        self.rng^=self.rng<<13;self.rng^=self.rng>>7;self.rng^=self.rng<<17;
        (self.rng>>40) as f32/(1u32<<24) as f32
    }
    pub fn eye(&self)->V3{self.foot+V3::new(0.0,1.57,0.0)}
    pub fn forward(&self)->V3{let c=self.pitch.cos();V3::new(self.yaw.sin()*c,self.pitch.sin(),self.yaw.cos()*c)}
    pub fn right(&self)->V3{V3::new(self.yaw.cos(),0.0,-self.yaw.sin())}
    pub fn look(&mut self,dx:f32,dy:f32){
        // Full unlimited rotation, but keep the stored angle bounded to
        // preserve precision over very long play sessions.
        self.yaw=(self.yaw+dx).rem_euclid(std::f32::consts::TAU);
        self.pitch=(self.pitch+dy).clamp(-1.37,1.37);
    }
    pub fn gun(&self)->Gun { Gun::from_index(self.weapon) }
    pub fn current_ammo(&self)->Magazine {self.ammo[self.weapon]}
    pub fn select(&mut self,index:usize){
        let index=index.min(WEAPON_COUNT-1);
        if self.weapon!=index {
            self.weapon=index;
            self.ads=false;
            self.aim_blend=0.0;
            self.reload=0.0; // cancel, never transfer ammo across guns
            self.reload_total=0.0;
            self.weapon_spin=0.0;
            self.fire_cd=0.0; // avoid carrying the prior weapon's shot cooldown after switching
            self.sounds.push(SoundCue::Menu);
        }
    }
    pub fn set_aim_down_sights(&mut self,enabled:bool){
        self.ads=enabled && self.gun()==Gun::Sniper && self.mode==Mode::Playing
            && self.story.is_none() && !self.showing_help && self.reload<=0.0;
    }
    /// Smoothed first-person optic transition, 0=hip pose, 1=fully scoped.
    /// Kept separately from ADS intent so right-mouse release animates back.
    pub fn aim_ease(&self)->f32 {
        let t=self.aim_blend.clamp(0.0,1.0);
        t*t*(3.0-2.0*t)
    }
    pub fn reload(&mut self){
        if self.mode!=Mode::Playing || self.story.is_some() || self.reload>0.0 {return}
        let gun=self.gun();let slot=&self.ammo[self.weapon];
        if slot.loaded>=gun.spec().magazine || slot.reserve<=0 {return}
        self.reload_total=gun.spec().reload_seconds;
        self.reload=self.reload_total;
        self.reload_weapon=self.weapon;
        self.ads=false;
        self.sounds.push(SoundCue::Reload);
    }
    pub fn reload_progress(&self)->f32 {
        if self.reload<=0.0||self.reload_total<=0.0 {0.0}
        else {(1.0-self.reload/self.reload_total).clamp(0.0,1.0)}
    }
    pub fn add_ammo(&mut self,gun:Gun,amount:i32){self.ammo[gun.index()].add_reserve(gun,amount);}
    pub fn jump(&mut self){
        if self.mode==Mode::Playing&&self.on_ground&&self.story.is_none() {
            self.vertical_speed=7.25;self.on_ground=false;
            self.sounds.push(SoundCue::Jump);
        }
    }
    fn start_mission(&mut self){
        self.sounds.push(SoundCue::WaveStart);
        self.mode=Mode::Playing;
        self.story=if self.stage==0&&self.story==Some(StoryView::Prologue){self.story}else{Some(StoryView::Mission)};
        self.vertical_speed=0.0;self.on_ground=true;self.yaw=0.0;self.pitch=0.0;
        self.ads=false;self.aim_blend=0.0;
        self.boxes.clear();self.sites.clear();self.decor.clear();self.shots.clear();self.sparks.clear();
        self.bots.clear();self.pickups.clear();
        self.has_key=false;self.relays_done=0;self.relays_total=0;
        self.boss_hp=if self.stage==4{self.boss_max_hp}else{0.0};
        self.boss_target=None;self.boss_charge=0.0;self.boss_cooldown=2.8;
        self.boss_flash=0.0;self.boss_beam=0.0;self.boss_reserves=0;
        self.notice="";self.notice_time=0.0;
        let mut record_no=self.stage*3;
        for (row,line) in campaign::MAPS[self.stage].iter().enumerate(){
            let cells=line.as_bytes();
            let mut col=0;
            while col<18 {
                if cells[col]==b'#' {
                    let from=col;
                    while col<18&&cells[col]==b'#'{col+=1;}
                    self.boxes.push(Box3::new(from as f32*2.0,0.0,row as f32*2.0,
                        (col-from) as f32*2.0,4.7,2.0));
                    continue;
                }
                let pos=V3::new(col as f32*2.0+1.0,0.0,row as f32*2.0+1.0);
                match cells[col] as char {
                    'S'=>self.foot=pos,
                    'c'=>{
                        self.boxes.push(Box3::new(pos.x-0.80,0.0,pos.z-0.80,1.6,1.35,1.6));
                        self.decor.push(Decor{pos,kind:DecorKind::Crate,rotation:0.0});
                    }
                    'L'=>{self.sites.push(Site{pos,kind:SiteKind::Record(record_no),used:self.records_found[record_no]});record_no+=1;}
                    'K'=>self.sites.push(Site{pos,kind:SiteKind::Key,used:false}),
                    'R'=>{self.sites.push(Site{pos,kind:SiteKind::Relay,used:false});self.relays_total+=1;}
                    'E'=>self.sites.push(Site{pos,kind:SiteKind::Exit,used:false}),
                    'U'=>self.sites.push(Site{pos,kind:SiteKind::Uplink,used:false}),
                    'B'=>self.boss_pos=pos,
                    'P'=>{
                        let kind=if (row+col)%3==0{PickupKind::Health}
                            else if (row+col)%3==1{PickupKind::Shield}
                            else{PickupKind::Ammo(Gun::from_index(self.stage%WEAPON_COUNT),30)};
                        self.pickups.push(Pickup{pos,kind,phase:row as f32});
                    }
                    'r'|'k'|'t'|'m'|'g'=>{
                        let kind=match cells[col]{b'r'=>BotKind::Runner,b'k'=>BotKind::Striker,
                            b't'=>BotKind::Tank,b'm'=>BotKind::Marksman,_=>BotKind::Gunner};
                        let phase=self.rand()*6.283;
                        let cooldown=1.0+self.rand();
                        self.bots.push(Bot::new(pos,kind,Self::bot_health(kind),cooldown,phase));
                    }
                    _=>{}
                }
                // Lighting fixtures and details are decorative (no unfair collision).
                // Cells are spaced 2m apart, with sufficient walkable clearance.
                if cells[col]!=b'c' {
                    let tile=(col*11+row*7+self.stage*13)%41;
                    let decor=match tile {
                        0=>Some(DecorKind::Terminal),1=>Some(DecorKind::ConsoleBank),
                        2=>Some(DecorKind::ServerRack),3=>Some(DecorKind::PipeBundle),
                        4=>Some(DecorKind::CoolantTank),5=>Some(DecorKind::Desk),
                        6=>Some(DecorKind::Chair),7=>Some(DecorKind::WallMonitor),
                        8=>Some(DecorKind::WarningBeacon),9=>Some(DecorKind::FloorGrate),
                        10=>Some(DecorKind::WallSign),11=>Some(DecorKind::CeilingDuct),
                        _=>None,
                    };
                    // Furniture must not obstruct essential routes or terminals;
                    // place props on edge walls only, facing toward the free cell.
                    if let Some(kind)=decor {
                        if matches!(kind,DecorKind::FloorGrate|DecorKind::CeilingDuct) {
                            self.decor.push(Decor{pos,kind,rotation:0.0});
                        }else if cells[col]==b'.' {
                            // Place furniture close to the surrounding bulkhead,
                            // including interior walls. The central route stays clear.
                            let walls=[
                                (col>0&&campaign::MAPS[self.stage][row].as_bytes()[col-1]==b'#',(-0.65,0.0),std::f32::consts::FRAC_PI_2),
                                (col<17&&campaign::MAPS[self.stage][row].as_bytes()[col+1]==b'#',(0.65,0.0),-std::f32::consts::FRAC_PI_2),
                                (row>0&&campaign::MAPS[self.stage][row-1].as_bytes()[col]==b'#',(0.0,-0.65),0.0),
                                (row<17&&campaign::MAPS[self.stage][row+1].as_bytes()[col]==b'#',(0.0,0.65),std::f32::consts::PI),
                            ];
                            if let Some((_,(dx,dz),angle))=walls.into_iter().find(|v|v.0){
                                self.decor.push(Decor{pos:pos+V3::new(dx,0.0,dz),kind,rotation:angle});
                            }
                        }
                    }
                }
                if row%3==1 && col%4==1 && cells[col]!=b'#' && cells[col]!=b'c' {
                    self.decor.push(Decor{pos:pos+V3::new(0.0,4.5,0.0),
                        kind:if self.stage==1{DecorKind::HangingLamp}else{DecorKind::CeilingLight},rotation:0.0});
                }else if row%5==2&&col%4==2&&cells[col]!=b'#'&&cells[col]!=b'c'{
                    self.decor.push(Decor{pos:pos+V3::new(0.0,4.55,0.0),kind:DecorKind::VentFan,rotation:0.0});
                }
                col+=1;
            }
        }
        debug_assert_eq!(record_no,(self.stage+1)*3);
    }
    fn bot_health(kind:BotKind)->f32{
        match kind{BotKind::Runner=>44.0,BotKind::Striker=>65.0,
            BotKind::Tank=>120.0,BotKind::Marksman=>55.0,BotKind::Gunner=>100.0}
    }
    pub fn dismiss_story(&mut self){
        self.story=match self.story{Some(StoryView::Prologue)=>Some(StoryView::Mission),_=>None};
    }
    pub fn open_journal(&mut self){
        if self.story.is_some(){self.dismiss_story();return}
        if let Some(index)=self.records_found.iter().position(|v|*v){self.story=Some(StoryView::Record(index));}
        else{self.notice="NO RECOVERED RECORDS";self.notice_time=2.5;}
    }
    pub fn next_record(&mut self,delta:i32){
        let Some(StoryView::Record(current))=self.story else{return};
        for step in 1..=RECORD_COUNT {
            let index=(current as i32+step as i32*delta).rem_euclid(RECORD_COUNT as i32) as usize;
            if self.records_found[index]{self.story=Some(StoryView::Record(index));break;}
        }
    }
    pub fn nearby_site(&self)->Option<usize>{
        if self.mode!=Mode::Playing{return None}
        self.sites.iter().enumerate().filter(|(_,s)|!s.used||matches!(s.kind,SiteKind::Record(_)))
            .filter_map(|(i,s)|{
                let to=s.pos-self.foot;let dist=to.horizontal().len();
                if dist>2.45{return None}
                let from=self.eye();let toward=s.pos+V3::new(0.0,1.0,0.0)-from;
                let dir=toward.normalized();
                if self.boxes.iter().any(|b|b.ray_hit(from,dir).is_some_and(|t|t<toward.len()-0.1)){return None}
                Some((i,dist))
            }).min_by(|a,b|a.1.total_cmp(&b.1)).map(|(i,_)|i)
    }
    pub fn site_label(&self,site:&Site)->&'static str{
        match site.kind{
            SiteKind::Record(_)=>"READ EVIDENCE RECORD",SiteKind::Key=>"TAKE SECURITY KEY",
            SiteKind::Relay if self.stage==4=>"DISABLE SHIELD RESONATOR",
            SiteKind::Relay=>"OPERATE MANUAL RELAY",SiteKind::Exit=>"OPEN ACCESS HATCH",
            SiteKind::Uplink=>"TRANSMIT EVIDENCE TO EARTH",
        }
    }
    pub fn objective(&self)->String{
        match self.stage{
            0=>if self.has_key{"REACH THE EXTRACTION PORTAL".into()}else{"RECOVER THE BLUE ACCESS KEY".into()},
            1=>if self.relays_done<2{format!("ACTIVATE POWER RELAYS  {}/2",self.relays_done)}else{"REACH THE CONTAINMENT HATCH".into()},
            2=>if !self.has_key{"RECOVER THE SEALED ACCESS KEY".into()}
                else if !self.bots.is_empty(){format!("DISABLE SECURITY UNITS  {} LEFT",self.bots.len())}
                else{"ENTER THE WITNESS ARCHIVE".into()},
            3=>if self.relays_done<3{format!("ACTIVATE COMMAND RELAYS  {}/3",self.relays_done)}
                else if !self.has_key{"RECOVER THE DIRECTORS KEY".into()}
                else if !self.bots.is_empty(){format!("CLEAR COMMAND SECURITY  {} LEFT",self.bots.len())}
                else{"ENTER THE ARCHIVE HEART".into()},
            _=>if self.relays_done<3{format!("DISABLE SHIELD RESONATORS  {}/3",self.relays_done)}
                else if self.boss_hp>0.0{"DISABLE THE ARCHIVIST CHASSIS".into()}
                else{"ACTIVATE THE EMERGENCY UPLINK".into()},
        }
    }
    pub fn objective_ready(&self)->bool{
        match self.stage{
            0=>self.has_key,1=>self.relays_done==2,
            2=>self.has_key&&self.bots.is_empty(),
            3=>self.relays_done==3&&self.has_key&&self.bots.is_empty(),
            _=>self.relays_done==3&&self.boss_hp<=0.0,
        }
    }
    pub fn interact(&mut self){
        if self.story.is_some(){self.dismiss_story();return}
        if self.mode!=Mode::Playing{return}
        let Some(index)=self.nearby_site() else{return};
        let kind=self.sites[index].kind;
        match kind{
            SiteKind::Record(record)=>{
                self.sites[index].used=true;self.records_found[record]=true;
                self.story=Some(StoryView::Record(record));self.sounds.push(SoundCue::Menu);
            },
            SiteKind::Key=>{
                self.has_key=true;self.sites[index].used=true;
                self.notice="PHYSICAL SECURITY KEY ACQUIRED";self.notice_time=3.0;
                self.score+=300;self.sounds.push(SoundCue::Ammo);
            },
            SiteKind::Relay=>{
                self.sites[index].used=true;self.relays_done+=1;
                self.notice=if self.stage==4{"CORE RESONATOR DISABLED"}else{"MANUAL RELAY ACTIVATED"};
                self.notice_time=2.5;self.score+=200;self.sounds.push(SoundCue::WaveClear);
            },
            SiteKind::Exit|SiteKind::Uplink=>{
                if !self.objective_ready(){
                    self.notice="ACCESS DENIED - COMPLETE THE OBJECTIVE";self.notice_time=2.5;
                    self.sounds.push(SoundCue::Menu);
                }else if kind==SiteKind::Uplink {
                    self.records_found[14]=true;
                    self.mode=Mode::Won;self.story=Some(StoryView::Epilogue);
                    self.score+=3000;self.sounds.push(SoundCue::Victory);
                }else{
                    self.mode=Mode::Between;self.wave_clock=2.3;
                    self.score+=1000;self.sounds.push(SoundCue::WaveClear);
                }
            }
        }
    }
    pub fn retry_mission(&mut self){
        self.health=100.0;self.shield=60.0;self.ammo=Gun::ALL.map(Magazine::starting);
        self.combo=0;self.combo_left=0.0;self.reload=0.0;self.fire_cd=0.0;
        self.start_mission();
    }
    fn spawn_guardians(&mut self){
        for (offset,kind) in [(V3::new(-6.0,0.0,4.0),BotKind::Runner),
            (V3::new(6.0,0.0,-4.0),BotKind::Gunner)] {
            let loc=self.boss_pos+offset;
            if self.body_collides(loc.x,loc.z,0.0,0.4,2.0){continue}
            let phase=self.rand()*6.283;
            self.bots.push(Bot::new(loc,kind,Self::bot_health(kind),1.5,phase));
        }
    }
    fn update_archivist(&mut self,dt:f32){
        if self.stage!=4||self.boss_hp<=0.0{return}
        self.boss_flash=(self.boss_flash-dt).max(0.0);
        self.boss_beam=(self.boss_beam-dt).max(0.0);
        if self.relays_done<3{return}
        if self.boss_target.is_some(){
            self.boss_charge-=dt;
            if self.boss_charge<=0.0 {
                let locked=self.boss_target.take().unwrap();
                self.boss_beam_pos=locked;self.boss_beam=0.28;
                self.boss_cooldown=if self.boss_hp<self.boss_max_hp*0.35{1.25}else{2.25};
                if (self.foot-locked).horizontal().len()<2.25{
                    let origin=self.boss_pos+V3::new(0.0,2.5,0.0);
                    let toward=self.eye()-origin;let dir=toward.normalized();
                    if !self.boxes.iter().any(|b|b.ray_hit(origin,dir).is_some_and(|t|t<toward.len()-0.1)){
                        self.take_damage(27.0);
                    }
                }
            }
        }else{
            self.boss_cooldown-=dt;
            if self.boss_cooldown<=0.0 {
                self.boss_target=Some(self.foot);
                self.boss_charge=if self.boss_hp<self.boss_max_hp*0.35{0.72}else{1.25};
            }
        }
    }
    fn body_collides(&self,x:f32,z:f32,foot_y:f32,r:f32,height:f32)->bool{
        self.boxes.iter().any(|b|foot_y<b.max.y-0.08&&foot_y+height>b.min.y+0.08&&b.intersects_circle(x,z,r))
    }
    pub fn move_player(&mut self,forward:f32,strafe:f32,sprint:bool,dash:bool,dt:f32){
        if self.mode!=Mode::Playing||self.showing_help||self.story.is_some(){return}
        let wish=self.forward().horizontal().normalized()*forward+self.right()*strafe;
        if wish.len()<0.01{return}
        let direction=wish.normalized();
        let mut speed=if sprint{9.4}else{6.6};
        if dash&&self.dash_cd<=0.0{speed=27.0;self.dash_cd=2.0;self.sounds.push(SoundCue::Dash);}
        let dx=direction.x*dt*speed;let dz=direction.z*dt*speed;
        if !self.body_collides(self.foot.x+dx,self.foot.z,self.foot.y,0.32,1.65)
            && !(self.stage==4&&self.boss_hp>0.0&&
                ((self.foot.x+dx-self.boss_pos.x).powi(2)+(self.foot.z-self.boss_pos.z).powi(2))<3.5) {
            self.foot.x+=dx;
        }
        if !self.body_collides(self.foot.x,self.foot.z+dz,self.foot.y,0.32,1.65)
            && !(self.stage==4&&self.boss_hp>0.0&&
                ((self.foot.x-self.boss_pos.x).powi(2)+(self.foot.z+dz-self.boss_pos.z).powi(2))<3.5) {
            self.foot.z+=dz;
        }
        self.walk+=dt*speed;
    }
    /// Fire one round; shotgun uses distinct pellet rays and the sniper's ADS
    /// lowers dispersion. Every projectile respects scene obstacle occlusion.
    pub fn fire(&mut self){
        if self.mode!=Mode::Playing || self.showing_help || self.story.is_some() || self.fire_cd>0.0 || self.reload>0.0 {return}
        let gun=self.gun();let spec=gun.spec();
        if self.ammo[self.weapon].loaded<=0 {self.reload();return}
        self.ammo[self.weapon].loaded-=1;
        self.sounds.push(match gun{
            Gun::Carbine=>SoundCue::Carbine,Gun::Cannon=>SoundCue::Cannon,
            Gun::Sniper=>SoundCue::Sniper,Gun::Shotgun=>SoundCue::Shotgun,Gun::Minigun=>SoundCue::Minigun,
        });
        self.fire_cd=spec.shot_delay;
        self.muzzle=match gun {Gun::Sniper=>0.18,Gun::Shotgun=>0.17,Gun::Minigun=>0.065,_=>0.11};
        self.last_hit=false;
        self.weapon_spin=if gun==Gun::Minigun {1.0}else{self.weapon_spin};
        let origin=self.eye();let forward=self.forward();let right=self.right();
        let up=forward.cross(right).normalized();
        let spread=if gun==Gun::Sniper && self.ads{0.0003}else if gun==Gun::Sniper{0.018}else{spec.spread};
        let mut damage=vec![0.0f32;self.bots.len()];
        let mut boss_damage=0.0;
        let mut sparks=Vec::with_capacity(spec.pellets);
        for pellet in 0..spec.pellets {
            let (sx,sy)=if gun==Gun::Shotgun {
                // Golden-angle disc: stable, even pellet coverage + some jitter.
                let a=pellet as f32*2.3999632 + self.rand()*0.22;
                let radius=((pellet as f32+0.5)/spec.pellets as f32).sqrt();
                (a.cos()*radius,a.sin()*radius)
            }else{(self.rand()*2.0-1.0,self.rand()*2.0-1.0)};
            let dir=(forward+right*sx*spread+up*sy*spread).normalized();
            let nearest_wall=self.boxes.iter().filter_map(|b|b.ray_hit(origin,dir))
                .fold(80.0f32,f32::min);
            let mut nearest=nearest_wall;let mut target=None;
            for (i,e) in self.bots.iter().enumerate(){
                let center=e.pos+V3::new(0.0,1.1,0.0);
                let radius=match e.kind{BotKind::Tank|BotKind::Gunner=>0.9,_=>0.68};
                if let Some(t)=ray_sphere(origin,dir,center,radius){
                    if t<nearest{nearest=t;target=Some(i);}
                }
            }
            if self.stage==4&&self.boss_hp>0.0 {
                if let Some(t)=ray_sphere(origin,dir,self.boss_pos+V3::new(0.0,1.65,0.0),1.65){
                    if t<nearest {
                        nearest=t;target=None;
                        if self.relays_done==3{boss_damage+=spec.damage;}
                        else{self.notice="SHIELD ACTIVE - CUT ALL RESONATORS";self.notice_time=1.7;}
                    }
                }
            }
            let impact=origin+dir*nearest;
            if let Some(i)=target {
                damage[i]+=spec.damage;
                if pellet==0 || gun!=Gun::Shotgun || self.rand()<0.3 {sparks.push((impact,0xff8cfdf3,2));}
            }else if nearest<80.0 && pellet==0 {sparks.push((impact,0xff6ed7eb,4));}
        }
        // Apply all pellet hits before removing dead targets: shotgun pellets
        // never miss due to shifting vector indices on an earlier hit.
        for (i,&amount) in damage.iter().enumerate(){
            if amount<=0.0{continue}
            self.last_hit=true;
            self.bots[i].hp-=amount;
            self.bots[i].flash=0.12;
            // Taking a real hit wakes this enemy regardless of range or
            // facing; it investigates the shooter's position at impact.
            self.bots[i].struck_by_player(self.foot);
        }
        if self.last_hit{self.sounds.push(SoundCue::Hit);}
        for (position,color,count) in sparks {self.spawn_sparks(position,color,count);}
        for i in (0..self.bots.len()).rev(){
            if self.bots[i].hp>0.0{continue}
            let enemy=self.bots.swap_remove(i);
            self.kills+=1;self.sounds.push(SoundCue::BotDown);
            self.combo=if self.combo_left>0.0{self.combo+1}else{1};self.combo_left=3.7;
            self.score+=100*self.combo.min(10)*(self.stage as u32+1);
            self.spawn_sparks(enemy.pos+V3::new(0.0,1.0,0.0),0xffffdb75,18);
            // Every class drops ammunition for its corresponding gun.
            let drop_gun=enemy.kind.ammo_gun();
            let d=drop_gun.spec();
            let quantity=d.drop_min+(self.rand()*(d.drop_max-d.drop_min+1) as f32) as i32;
            self.pickups.push(Pickup{pos:enemy.pos,kind:PickupKind::Ammo(drop_gun,quantity),phase:self.time});
            // Occasional health/shield supplies are separate from guaranteed ammo.
            if self.rand()<0.17 {
                let supply=if self.rand()<0.5 {PickupKind::Health}else{PickupKind::Shield};
                self.pickups.push(Pickup{pos:enemy.pos+V3::new(0.7,0.0,0.0),kind:supply,phase:self.time});
            }
        }
        if boss_damage>0.0 {
            self.boss_hp=(self.boss_hp-boss_damage).max(0.0);
            self.last_hit=true;self.boss_flash=0.14;
            if self.boss_hp<=0.0 {
                self.boss_target=None;self.bots.clear();self.shots.clear();
                self.notice="CURATOR OFFLINE - WITNESS STORAGE INTACT";self.notice_time=5.0;
                self.sounds.push(SoundCue::WaveClear);
            }else if self.boss_reserves<2 {
                let limit=if self.boss_reserves==0{self.boss_max_hp*0.65}else{self.boss_max_hp*0.30};
                if self.boss_hp<=limit {
                    self.boss_reserves+=1;self.spawn_guardians();
                    self.notice="ARCHIVIST HAS RELEASED RESERVE GUARDIANS";self.notice_time=3.0;
                }
            }
        }
        if self.ammo[self.weapon].loaded<=0 {self.reload();}
    }
    fn spawn_sparks(&mut self,pos:V3,color:u32,count:usize){
        for _ in 0..count{
            let a=self.rand()*6.283;let b=self.rand()*2.0-1.0;
            let v=V3::new(a.cos()*(1.0-b*b).sqrt(),b,a.sin()*(1.0-b*b).sqrt());
            let speed=2.0+self.rand()*4.0;let life=0.28+self.rand()*0.4;
            self.sparks.push(Spark{pos,velocity:v*speed,life,color});
        }
    }
    pub fn update(&mut self,dt:f32){
        self.time+=dt;
        // Fixed-step bounded tween (~260ms to aim, ~220ms to lower the optic).
        // Move back to hip even if a pause or a story screen interrupts aiming.
        let aim_target=if self.ads && self.mode==Mode::Playing && !self.showing_help && self.story.is_none() {1.0} else {0.0};
        let step=dt.clamp(0.0,0.05)*if aim_target>0.0 {3.85} else {4.55};
        if self.aim_blend<aim_target {self.aim_blend=(self.aim_blend+step).min(aim_target);}
        else {self.aim_blend=(self.aim_blend-step).max(aim_target);}
        if self.mode==Mode::Paused||self.mode==Mode::Won||self.mode==Mode::Lost||self.showing_help||self.story.is_some(){return}
        if self.mode==Mode::Between{
            self.wave_clock-=dt;
            if self.wave_clock<=0.0{self.stage=(self.stage+1).min(MISSION_COUNT-1);self.start_mission();}
            return;
        }
        self.notice_time=(self.notice_time-dt).max(0.0);
        self.elapsed+=dt;
        self.update_archivist(dt);
        if self.mode==Mode::Lost{return}
        self.fire_cd=(self.fire_cd-dt).max(0.0);self.dash_cd=(self.dash_cd-dt).max(0.0);
        self.weapon_spin=(self.weapon_spin-dt*1.6).max(0.0);
        self.muzzle=(self.muzzle-dt).max(0.0);self.hurt=(self.hurt-dt).max(0.0);self.combo_left=(self.combo_left-dt).max(0.0);
        if self.combo_left<=0.0{self.combo=0;}
        if self.reload>0.0{
            self.reload-=dt;
            if self.reload<=0.0{
                self.reload=0.0;
                let gun=Gun::from_index(self.reload_weapon);
                self.ammo[self.reload_weapon].reload(gun);
                self.reload_total=0.0;
                self.sounds.push(SoundCue::ReloadDone);
            }
        }
        // Continuous gravity and elevated-platform landing.
        let old_y=self.foot.y;
        self.vertical_speed-=19.0*dt;
        let new_y=old_y+self.vertical_speed*dt;
        self.on_ground=false;
        let mut base=0.0f32;
        if self.vertical_speed<=0.0{
            for b in &self.boxes {
                if b.max.y<=4.0&&b.intersects_circle(self.foot.x,self.foot.z,0.28)&&old_y>=b.max.y-0.1&&new_y<=b.max.y{
                    base=base.max(b.max.y);
                }
            }
        }
        if new_y<=base{self.foot.y=base;self.vertical_speed=0.0;self.on_ground=true;}
        else {self.foot.y=new_y;}
        // Perception-driven AI: idle until they see Rook *within the
        // individual class's distance and cone*, through clear geometry,
        // OR they actually take a hit. Losing sight leads only to the last
        // known position, never to telepathic wall-following or wall-shots.
        let player=self.foot;let player_eye=self.eye();let mut launches=Vec::new();
        for i in 0..self.bots.len(){
            let mut bot=self.bots[i];
            bot.cooldown=(bot.cooldown-dt).max(0.0);
            bot.flash=(bot.flash-dt).max(0.0);
            let previous=bot.pos;
            let sensor=bot.pos+V3::new(0.0,1.35,0.0);
            let sees=enemy_ai::can_see(sensor,bot.motion.facing,player_eye,bot.kind,&self.boxes);
            if sees {
                bot.awareness=Awareness::Chasing;
                bot.last_known=player;
                bot.search_left=5.0;
            }else if bot.awareness!=Awareness::Idle {
                bot.awareness=Awareness::Investigating;
                bot.search_left-=dt;
                if bot.search_left<=0.0 {
                    bot.awareness=Awareness::Idle;
                    bot.nav_clock=0.0;
                }
            }
            let is_active=bot.awareness!=Awareness::Idle;
            let mut facing_target=bot.pos+V3::new((bot.phase+self.time*0.50).sin(),0.0,
                (bot.phase+self.time*0.50).cos());
            if is_active{
                // Destination is player only WHILE visible. Otherwise,
                // follow the memory of where they were last detected/hit.
                let goal=if sees{player}else{bot.last_known};
                let from_goal=(goal-bot.pos).horizontal();
                let dist=from_goal.len();
                if dist>0.01 {facing_target=goal;}
                if !sees&&dist<0.90 {
                    // Investigate and scan briefly; do not keep advancing
                    // indefinitely at an already-visited location.
                    bot.search_left=bot.search_left.min(1.5);
                }
                let can_walk=dist>(if sees{4.0}else{0.45});
                if can_walk {
                    bot.nav_clock-=dt;
                    if bot.nav_clock<=0.0 || (bot.nav_waypoint-bot.pos).horizontal().len()<0.60 {
                        let direct=enemy_ai::clear_walk_path(bot.pos,goal,&self.boxes);
                        bot.nav_waypoint=if direct{goal}
                            else{enemy_ai::next_waypoint(self.stage,bot.pos,goal).unwrap_or(bot.pos)};
                        bot.nav_clock=if direct{0.22}else{0.42};
                    }
                    let delta=(bot.nav_waypoint-bot.pos).horizontal();
                    if delta.len()>0.12 {
                        let direct=delta.normalized();
                        let offset=V3::new(-direct.z,0.0,direct.x);
                        // Individuality only during direct pursuit; use a
                        // tight path heading in corridors to avoid wall drift.
                        let swirl=(self.time*0.6+bot.phase).sin();
                        let move_dir=if sees && enemy_ai::clear_walk_path(bot.pos,goal,&self.boxes){
                            (direct+offset*swirl*0.17).normalized()
                        } else {direct};
                        let speed=match bot.kind{
                            BotKind::Runner=>3.4,BotKind::Striker=>2.8,BotKind::Tank=>1.7,
                            BotKind::Marksman=>2.1,BotKind::Gunner=>2.2,
                        };
                        let x=bot.pos.x+move_dir.x*dt*speed;
                        let z=bot.pos.z+move_dir.z*dt*speed;
                        if !self.body_collides(x,bot.pos.z,0.0,0.4,2.0){bot.pos.x=x;}
                        if !self.body_collides(bot.pos.x,z,0.0,0.4,2.0){bot.pos.z=z;}
                    }
                }
            }
            bot.motion.advance(previous,bot.pos,facing_target,bot.kind,dt);
            if sees && bot.cooldown<=0.0 {
                let from=bot.pos+V3::new(0.0,1.28,0.0);
                let toward=player_eye-from;
                let distance=toward.len();
                let aim=toward.normalized();
                // Visibility and line-of-fire are rechecked from the firing
                // muzzle, even when awareness already says 'Chasing'.
                let line_clear=distance<19.0 && !self.boxes.iter().any(|b|
                    b.ray_hit(from,aim).is_some_and(|t|t>=0.0&&t<distance-0.15));
                if line_clear {
                    launches.push(Shot{pos:from,velocity:aim*match bot.kind{
                        BotKind::Tank=>8.0,BotKind::Marksman=>17.0,
                        BotKind::Gunner=>13.0,_=>11.0,
                    },age:0.0,color:match bot.kind{
                        BotKind::Runner=>0xffffc36f,BotKind::Striker=>0xffff6fa9,
                        BotKind::Tank=>0xffc58bff,BotKind::Marksman=>0xff9bedff,
                        BotKind::Gunner=>0xffffbb63,
                    }});
                    bot.cooldown=match bot.kind{
                        BotKind::Runner=>1.6,BotKind::Striker=>1.0,BotKind::Tank=>2.3,
                        BotKind::Marksman=>2.8,BotKind::Gunner=>0.46,
                    };
                } else {bot.cooldown=0.22;}
            }
            self.bots[i]=bot;
        }
        if !launches.is_empty(){self.sounds.push(SoundCue::BotShot);}
        self.shots.extend(launches);
        // Swap-remove projectiles and pickups to keep per-frame work bounded.
        let mut i=0;
        while i<self.shots.len(){
            let mut s=self.shots[i];s.age+=dt;s.pos=s.pos+s.velocity*dt;
            let hit_wall=self.boxes.iter().any(|b|s.pos.x>=b.min.x&&s.pos.x<=b.max.x&&s.pos.y>=b.min.y&&s.pos.y<=b.max.y&&s.pos.z>=b.min.z&&s.pos.z<=b.max.z);
            let hit_player=(s.pos-(self.eye()-V3::new(0.0,0.2,0.0))).len()<0.55;
            if hit_player {self.take_damage(11.0);}
            if hit_wall||hit_player||s.age>5.0{self.shots.swap_remove(i);}else{self.shots[i]=s;i+=1;}
        }
        let mut i=0;
        while i<self.pickups.len(){
            let p=self.pickups[i];let dist=(p.pos-self.foot).horizontal().len();
            if dist<1.05 && (p.pos.y-self.foot.y).abs()<2.0{
                match p.kind{
                    PickupKind::Ammo(gun,count)=>{self.add_ammo(gun,count);self.sounds.push(SoundCue::Ammo);}
                    PickupKind::Health=>{self.health=(self.health+35.0).min(100.0);self.sounds.push(SoundCue::Health);}
                    PickupKind::Shield=>{self.shield=(self.shield+40.0).min(100.0);self.sounds.push(SoundCue::Shield);}
                }
                self.score+=75;self.pickups.swap_remove(i);
            }else{i+=1;}
        }
        for s in &mut self.sparks{s.pos=s.pos+s.velocity*dt;s.velocity.y-=12.0*dt;s.life-=dt;}
        self.sparks.retain(|s|s.life>0.0);

    }
    fn take_damage(&mut self,amount:f32){
        let absorb=amount.min(self.shield);self.shield-=absorb;self.health=(self.health-(amount-absorb)).max(0.0);
        self.sounds.push(if absorb>=amount{SoundCue::ShieldHit}else{SoundCue::Hurt});
        self.hurt=0.18;
        if self.health<=0.0{self.mode=Mode::Lost;self.sounds.push(SoundCue::Defeat);}
    }
}

#[cfg(test)] mod weapon_tests {
    use super::*;
    #[test] fn all_five_enemy_classes_have_distinct_ammo_types(){
        let mut w=World::new();w.showing_help=false;w.story=None;
        assert!(w.bots.len()>=5);
        assert!(w.bots.iter().map(|b|b.kind).collect::<std::collections::HashSet<_>>().len()>=4);
        let kinds=[BotKind::Runner,BotKind::Tank,BotKind::Marksman,BotKind::Striker,BotKind::Gunner];
        assert_eq!(kinds.map(BotKind::ammo_gun),Gun::ALL);
        for gun in Gun::ALL {
            let i=gun.index(); let before=w.ammo[i];
            w.add_ammo(gun,5);
            assert!(w.ammo[i].reserve>=before.reserve);
            // Other magazine values were not changed by this pickup.
            assert_eq!(w.ammo[i].loaded,before.loaded);
        }
    }
    #[test] fn weapon_switch_keeps_its_own_magazine(){
        let mut w=World::new();w.showing_help=false;w.story=None;
        w.fire();let carbine=w.ammo[0];
        w.select(2);w.fire();
        assert_eq!(w.ammo[0],carbine);
        assert_eq!(w.ammo[2].loaded,Gun::Sniper.spec().magazine-1);
    }
    #[test] fn enemy_death_always_drops_its_matching_ammunition(){
        for kind in [BotKind::Runner,BotKind::Tank,BotKind::Marksman,
                     BotKind::Striker,BotKind::Gunner] {
            let mut w=World::new();w.showing_help=false;w.story=None;
            w.pickups.clear();w.bots.clear();
            w.bots.push(Bot::new(w.foot+V3::new(0.0,0.0,3.0),kind,1.0,3.0,0.0));
            w.select(kind.ammo_gun().index());w.fire_cd=0.0;
            w.fire();
            let gun=kind.ammo_gun();let spec=gun.spec();
            assert!(w.bots.is_empty());
            assert!(w.pickups.iter().any(|p| match p.kind {
                PickupKind::Ammo(g,n)=>g==gun&&n>=spec.drop_min&&n<=spec.drop_max,
                _=>false,
            }),"missing correctly typed drop for {kind:?}");
        }
    }
    #[test] fn idle_enemy_does_not_chase_through_wall(){
        let mut w=World::new();w.story=None;
        w.bots.clear();w.boxes.clear();
        let bot_pos=V3::new(6.0,0.0,6.0);
        let mut bot=Bot::new(bot_pos,BotKind::Runner,100.0,0.0,0.0);
        bot.motion.facing=0.0;
        w.foot=V3::new(6.0,0.0,10.0);
        w.boxes.push(Box3::new(4.0,0.0,8.0,4.0,4.7,1.0));
        w.bots.push(bot);
        for _ in 0..20{w.update(1.0/120.0);}
        assert_eq!(w.bots[0].awareness,Awareness::Idle);
        assert!((w.bots[0].pos-bot_pos).len()<0.01);
        assert!(w.shots.is_empty());
    }
    #[test] fn enemy_detects_only_inside_range_and_field_of_view(){
        let mut w=World::new();w.story=None;
        w.bots.clear();w.boxes.clear();
        let bot_pos=V3::new(10.0,0.0,10.0);
        let mut bot=Bot::new(bot_pos,BotKind::Runner,100.0,10.0,0.0);
        bot.motion.facing=0.0;
        w.bots.push(bot);
        w.foot=V3::new(10.0,0.0,6.0);
        w.update(1.0/120.0);
        assert_eq!(w.bots[0].awareness,Awareness::Idle);
        w.foot=V3::new(10.0,0.0,28.0);
        w.update(1.0/120.0);
        assert_eq!(w.bots[0].awareness,Awareness::Idle);
        w.foot=V3::new(10.0,0.0,14.0);
        w.update(1.0/120.0);
        assert_eq!(w.bots[0].awareness,Awareness::Chasing);
    }
    #[test] fn taking_damage_wakes_enemy_without_sight(){
        let mut w=World::new();w.story=None;w.bots.clear();w.boxes.clear();
        let pos=V3::new(9.0,0.0,14.0);
        let mut bot=Bot::new(pos,BotKind::Tank,120.0,10.0,0.0);
        bot.motion.facing=std::f32::consts::PI; // looking away from player
        w.bots.push(bot);
        w.foot=V3::new(9.0,0.0,10.0);w.yaw=0.0;
        w.fire();
        assert_eq!(w.bots[0].awareness,Awareness::Investigating);
        assert_eq!(w.bots[0].last_known,w.foot);
    }
    #[test] fn loses_sight_and_forgets_player_after_search(){
        let mut w=World::new();w.story=None;w.bots.clear();w.boxes.clear();
        let p=V3::new(8.0,0.0,8.0);
        let mut bot=Bot::new(p,BotKind::Tank,120.0,10.0,0.0);
        bot.motion.facing=0.0;w.bots.push(bot);
        w.foot=V3::new(8.0,0.0,13.0);w.update(0.01);
        assert_eq!(w.bots[0].awareness,Awareness::Chasing);
        let memorized=w.bots[0].last_known;
        w.foot=V3::new(29.0,0.0,29.0);
        w.boxes.push(Box3::new(16.0,0.0,12.0,2.0,4.7,17.0));
        w.update(0.01);
        assert_eq!(w.bots[0].awareness,Awareness::Investigating);
        assert_eq!(w.bots[0].last_known,memorized);
        for _ in 0..800 {w.update(0.01);}
        assert_eq!(w.bots[0].awareness,Awareness::Idle);
    }
    #[test] fn sniper_optic_zooms_in_and_out_smoothly(){
        let mut w=World::new();w.story=None;w.select(2);
        assert_eq!(w.aim_blend,0.0);
        w.set_aim_down_sights(true);
        w.update(1.0/120.0);
        assert!(w.aim_blend>0.0&&w.aim_blend<1.0);
        assert!(w.aim_ease()>0.0&&w.aim_ease()<1.0);
        for _ in 0..35{w.update(1.0/120.0);}
        assert!((w.aim_blend-1.0).abs()<1e-5);
        w.set_aim_down_sights(false);
        w.update(1.0/120.0);
        assert!(w.aim_blend<1.0&&w.aim_blend>0.0);
        for _ in 0..35{w.update(1.0/120.0);}
        assert_eq!(w.aim_blend,0.0);
    }
    #[test] fn changing_weapons_resets_sniper_optic(){
        let mut w=World::new();w.story=None;w.select(2);
        w.set_aim_down_sights(true);w.update(0.05);
        assert!(w.aim_blend>0.0);
        w.select(0);
        assert_eq!(w.aim_blend,0.0);
        assert!(!w.ads);
    }
    #[test] fn sniper_scope_is_hold_to_aim_only(){
        let mut w=World::new();w.showing_help=false;w.story=None;w.select(2);
        w.set_aim_down_sights(true);assert!(w.ads);
        w.set_aim_down_sights(false);assert!(!w.ads);
        w.select(3);w.set_aim_down_sights(true);assert!(!w.ads);
    }
    #[test] fn mission_exit_needs_objective(){
        let mut w=World::new();w.story=None;
        let exit=w.sites.iter().find(|s|s.kind==SiteKind::Exit).unwrap().pos;
        w.foot=exit;w.interact();assert_eq!(w.mode,Mode::Playing);
        let key=w.sites.iter().find(|s|s.kind==SiteKind::Key).unwrap().pos;
        w.foot=key;w.interact();assert!(w.has_key);
        w.foot=exit;w.interact();assert_eq!(w.mode,Mode::Between);
        w.update(2.5);assert_eq!(w.stage,1);
    }
    #[test] fn journal_survives_mission_load(){
        let mut w=World::new();w.story=None;
        let site=w.sites.iter().find(|s|matches!(s.kind,SiteKind::Record(_))).unwrap().pos;
        w.foot=site;w.interact();assert_eq!(w.story,Some(StoryView::Record(0)));
        w.stage=1;w.start_mission();w.story=None;w.open_journal();
        assert_eq!(w.story,Some(StoryView::Record(0)));
    }
    #[test] fn look_can_rotate_many_complete_turns(){
        let mut w=World::new();
        for _ in 0..1000{w.look(0.35,0.0);}
        assert!(w.yaw>=0.0&&w.yaw<std::f32::consts::TAU);
        assert!(w.forward().x.is_finite());
        assert!(w.forward().z.is_finite());
    }
    #[test] fn shotgun_fires_one_shell_per_volley(){
        let mut w=World::new();w.showing_help=false;w.story=None;w.select(3);
        let before=w.ammo[3].loaded;w.fire();
        assert_eq!(w.ammo[3].loaded,before-1);
    }
}
