//! Deterministic FPS simulation shared by all five campaign levels.
use std::f32::consts::PI;
use crate::levels::{mission, EnemyKind, PickupKind, MISSIONS};

#[derive(Clone, Copy, Debug)]
pub struct Vec2 { pub x: f32, pub y: f32 }
impl Vec2 {
    pub fn new(x:f32,y:f32)->Self {Self{x,y}}
    pub fn distance(self, other:Self)->f32 {((self.x-other.x).powi(2)+(self.y-other.y).powi(2)).sqrt()}
}
#[derive(Clone, Copy, Debug)]
pub struct Pickup {pub pos:Vec2,pub kind:PickupKind,pub active:bool}
#[derive(Clone, Copy, Debug)]
pub struct Enemy {pub pos:Vec2,pub kind:EnemyKind,pub health:i32,pub attack_timer:f32,pub active:bool,pub phase:f32}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {Playing, Won, Lost}

#[derive(Clone, Copy, Debug)]
pub struct Lift {
    pub x: i32,
    pub y: i32,
    pub height: f32,
    pub target: f32,
}

pub struct World {
    pub level:usize,
    pub cells:Vec<Vec<u8>>,
    pub player:Vec2,
    pub exit:Vec2,
    pub facing:f32,
    /// Crosshair x-coordinate in normalized camera-plane units (-0.7 .. 0.7).
    pub aim_x:f32,
    /// Feet elevation above the original floor, measured in wall heights.
    pub player_z:f32,
    pub fall_speed:f32,
    pub lifts:Vec<Lift>,
    pub health:i32,
    pub ammo:i32,
    pub score:u32,
    pub kills:u32,
    pub has_key:bool,
    pub relays_activated:u8,
    pub shots:u32,
    pub mode:Mode,
    pub enemies:Vec<Enemy>,
    pub pickups:Vec<Pickup>,
    pub time:f32,
    pub moving:bool,
    pub fire_cooldown:f32,
    pub muzzle:f32,
    pub hurt_flash:f32,
    pub notice:String,
    pub notice_t:f32,
    /// A nearby data console has been accessed. Read once with take_lore().
    pub pending_lore:Option<usize>,
    pub boss_phase:u8,
    pub boss_charge:f32,
    pub boss_cooldown:f32,
    pub boss_target:Vec2,
    pub boss_flash:f32,

}
impl World {
    pub fn new()->Self { Self::for_level(0,100,36,0) }
    pub fn for_level(level:usize,health:i32,ammo:i32,score:u32)->Self {
        let level=level.min(MISSIONS.len()-1);
        let config=mission(level);
        let mut cells:Vec<Vec<u8>>=config.map.iter().map(|s|s.as_bytes().to_vec()).collect();
        // Each mission has a two-stop elevator and a short upper mezzanine.
        // Authored coordinates are intentionally kept off mission objectives.
        const LIFTS:[(i32,i32);5]=[(8,10),(5,9),(8,12),(8,9),(6,8)];
        let (lx,ly)=LIFTS[level];
        for (x,tile) in [(lx,b'P'),(lx+1,b'U'),(lx+2,b'U')] {
            assert_eq!(cells[ly as usize][x as usize], b'.', "elevator must use an empty map cell");
            cells[ly as usize][x as usize]=tile;
        }
        let lifts=vec![Lift{x:lx,y:ly,height:0.0,target:0.0}];
        let exit=cells.iter().enumerate().find_map(|(y,row)|row.iter().position(|&c|c==b'E').map(|x|Vec2::new(x as f32+0.5,y as f32+0.5))).expect("mission must have exit");
        let enemies=config.enemies.iter().enumerate().map(|(i,&(x,y,kind))|{
            let hp=match kind{EnemyKind::Sentry=>2,EnemyKind::Hunter=>3,EnemyKind::Warden=>5,EnemyKind::Archivist=>24};
            Enemy{pos:Vec2::new(x,y),kind,health:hp,attack_timer:1.0+i as f32*0.13,active:true,phase:i as f32*1.9}
        }).collect();
        let pickups=config.pickups.iter().map(|&(x,y,kind)|Pickup{pos:Vec2::new(x,y),kind,active:true}).collect();
        Self{level,cells,player:Vec2::new(config.spawn.0,config.spawn.1),exit,
            facing:config.facing,aim_x:0.0,player_z:0.0,fall_speed:0.0,lifts,health:health.clamp(1,100),ammo:ammo.clamp(0,99),score,
            kills:0,has_key:false,relays_activated:0,shots:0,mode:Mode::Playing,enemies,pickups,
            time:0.0,moving:false,fire_cooldown:0.0,muzzle:0.0,hurt_flash:0.0,
            notice:match level{0=>"FIND THE BLUE KEY AND EXTRACT",1=>"ACTIVATE TWO POWER RELAYS",2=>"CLEAR THE VAULT AND RECOVER KEY",3=>"DISABLE CENTRAL COMMAND",_=>"SILENCE THE ARCHIVIST"}.into(),notice_t:4.0,
            pending_lore:None,boss_phase:0,boss_charge:0.0,boss_cooldown:3.0,
            boss_target:Vec2::new(0.0,0.0),boss_flash:0.0}
    }
    pub fn cell(&self,x:i32,y:i32)->u8 {
        if x<0||y<0{return b'#'}
        self.cells.get(y as usize).and_then(|r|r.get(x as usize)).copied().unwrap_or(b'#')
    }
    #[inline]
    pub fn floor_height(&self,x:i32,y:i32)->f32 {
        match self.cell(x,y) {
            b'U' => 1.25,
            b'P' => self.lifts.iter().find(|p|p.x==x&&p.y==y).map_or(0.0,|p|p.height),
            _ => 0.0,
        }
    }
    pub fn standing_on_lift(&self)->bool {
        self.cell(self.player.x.floor() as i32,self.player.y.floor() as i32)==b'P'
    }
    pub fn solid(&self,x:f32,y:f32,r:f32)->bool {
        for dy in [-r,r] {for dx in [-r,r]{
            let tx=(x+dx).floor() as i32;let ty=(y+dy).floor() as i32;
            if matches!(self.cell(tx,ty),b'#'|b'B'|b'D'|b'L'|b'T'|b't'|b'N'|b'n'){return true}
            // A player below a mezzanine cannot walk through its edge.
            if self.floor_height(tx,ty)>self.player_z+0.28 {return true}
        }}false
    }
    pub fn move_to(&mut self, dx:f32,dy:f32){
        let r=0.22;
        if !self.solid(self.player.x+dx,self.player.y,r){self.player.x+=dx;}
        if !self.solid(self.player.x,self.player.y+dy,r){self.player.y+=dy;}
    }
    pub fn look(&mut self,radians:f32){self.facing=(self.facing+radians).rem_euclid(2.0*PI);}
    pub fn aim(&mut self, mouse_dx:f32,viewport_width:f32) {
        self.aim_x=(self.aim_x + mouse_dx * 2.0 / viewport_width.max(1.0)).clamp(-0.68,0.68);
    }
    pub fn say(&mut self,text:&str,time:f32){self.notice=text.to_string();self.notice_t=time;}
    pub fn action(&mut self)->bool {
        if self.mode!=Mode::Playing{return false}
        let mut nearest:Option<(usize,usize,u8,f32)>=None;
        for (y,row) in self.cells.iter().enumerate(){for (x,&c) in row.iter().enumerate(){
            if !matches!(c,b'D'|b'L'|b'T'|b'N'|b'n'|b'P'){continue}
            let d=self.player.distance(Vec2::new(x as f32+0.5,y as f32+0.5));
            if d<1.55 && nearest.is_none_or(|v|d<v.3){nearest=Some((x,y,c,d));}
        }}
        match nearest {
            Some((x,y,b'P',_)) => {
                if let Some(lift)=self.lifts.iter_mut().find(|p|p.x==x as i32&&p.y==y as i32) {
                    lift.target=if lift.target>0.6{0.0}else{1.25};
                    let ascending=lift.target>0.5;
                    self.say(if ascending{"LIFT ASCENDING"}else{"LIFT DESCENDING"},2.0);
                    true
                } else {false}
            }
            Some((_,_,b'L',_)) if !self.has_key => {self.say("SECURITY KEY REQUIRED",1.6);false},
            Some((x,y,b'N'|b'n',_))=>{
                // The original authored layout determines stable transcript order.
                let index=mission(self.level).map.iter().take(y)
                    .map(|row|row.bytes().filter(|&b|b==b'N').count()).sum::<usize>()
                    +mission(self.level).map[y].as_bytes()[..x].iter().filter(|&&b|b==b'N').count();
                if self.cells[y][x]==b'N'{self.score+=40;}
                self.cells[y][x]=b'n';
                self.pending_lore=Some(self.level*crate::lore::PER_MISSION+index);
                self.say("ARCHIVE RECORD UNSEALED",2.0);true
            },
            Some((x,y,b'T',_)) => {
                self.cells[y][x]=b't';self.relays_activated+=1;
                self.score+=75;
                if mission(self.level).boss_required && self.relays_activated==mission(self.level).relays {
                    self.say("ALL RESONATORS OFFLINE - SHIELD DOWN",2.5);
                } else {self.say("POWER RELAY ACTIVATED +75",2.0);}
                true
            },
            Some((x,y,_,_))=>{self.cells[y][x]=b'.';self.say("GATE OPEN",1.5);true},
            None=>{self.say("NO CONTROL PANEL NEARBY",1.1);false},
        }
    }
    pub fn take_lore(&mut self)->Option<usize>{self.pending_lore.take()}
    pub fn line_of_sight(&self,from:Vec2,to:Vec2)->bool {
        let dist=from.distance(to);let n=(dist/0.12).ceil() as usize;
        if n==0{return true}
        for step in 1..n {let t=step as f32/n as f32;
            if self.solid(from.x+(to.x-from.x)*t,from.y+(to.y-from.y)*t,0.01){return false}
        }true
    }
    pub fn shoot(&mut self)->bool {
        if self.mode!=Mode::Playing||self.fire_cooldown>0.0 {return false}
        if self.ammo==0{self.say("OUT OF ENERGY CELLS",1.1);self.fire_cooldown=0.3;return false}
        self.ammo-=1;self.shots+=1;self.fire_cooldown=0.26;self.muzzle=0.11;
        let mut target=None::<(usize,f32)>;
        for (i,e) in self.enemies.iter().enumerate(){
            if !e.active{continue}
            let dist=e.pos.distance(self.player);if dist>15.0{continue}
            let angle=(e.pos.y-self.player.y).atan2(e.pos.x-self.player.x);
            let diff=(angle-(self.facing+(self.aim_x*0.66).atan())+PI).rem_euclid(2.0*PI)-PI;
            let hit_width=match e.kind{EnemyKind::Warden=>0.32,EnemyKind::Archivist=>0.77,_=>0.24};
            if diff.abs()<((hit_width/dist).atan()+0.025) && target.is_none_or(|(_,d)|dist<d) && self.line_of_sight(self.player,e.pos){target=Some((i,dist));}
        }
        if let Some((i,_))=target{
            if self.enemies[i].kind==EnemyKind::Archivist && self.relays_activated<mission(self.level).relays {
                self.say("SHIELD ACTIVE - DISABLE RESONATORS",1.6);
            } else {
                self.enemies[i].health-=1;
                if self.enemies[i].health<=0{
                    let boss=self.enemies[i].kind==EnemyKind::Archivist;
                    self.enemies[i].active=false;self.kills+=1;
                    self.score+=match self.enemies[i].kind{EnemyKind::Sentry=>100,EnemyKind::Hunter=>150,EnemyKind::Warden=>250,EnemyKind::Archivist=>2500};
                    if boss {self.boss_charge=0.0;self.boss_phase=4;self.say("ARCHIVIST OFFLINE - REACH THE UPLINK",3.5);}
                    else {self.say("HOSTILE DISABLED",1.1);}
                }
            }
        }
        true
    }
    pub fn boss_health(&self)->Option<(i32,i32)> {
        self.enemies.iter().find(|e|e.kind==EnemyKind::Archivist)
            .map(|e|(e.health.max(0),24))
    }
    pub fn boss_alive(&self)->bool {
        self.enemies.iter().any(|e|e.kind==EnemyKind::Archivist&&e.active)
    }
    pub fn hostiles_remaining(&self)->usize {self.enemies.iter().filter(|e|e.active).count()}
    pub fn objective_summary(&self)->String{
        let c=mission(self.level);
        let mut sections=Vec::new();
        if c.needs_key{sections.push(if self.has_key{"KEY OK".to_string()}else{"FIND KEY".to_string()});}
        if c.relays>0{sections.push(format!("RELAYS {}/{}",self.relays_activated,c.relays));}
        if c.clear_hostiles{sections.push(format!("HOSTILES {}",self.hostiles_remaining()));}
        if c.boss_required{sections.push(if self.boss_alive(){"ARCHIVIST ONLINE".to_string()}else{"ARCHIVIST OFFLINE".to_string()});}
        sections.push(if c.boss_required{"TRANSMIT".to_string()}else{"REACH EXIT".to_string()});
        sections.join("  |  ")
    }
    fn objectives_met(&self)->bool{
        let c=mission(self.level);
        (!c.needs_key||self.has_key) && self.relays_activated>=c.relays && (!c.clear_hostiles||self.hostiles_remaining()==0) && (!c.boss_required||!self.boss_alive())
    }
    pub fn tick(&mut self,dt:f32){
        if self.mode!=Mode::Playing{return}
        self.time+=dt;
        // Animated platforms move at a constant speed. The feet follow the
        // platform while riding; otherwise gravity handles dropping to ground.
        for lift in &mut self.lifts {
            let diff=lift.target-lift.height;
            lift.height+=diff.clamp(-dt*0.65,dt*0.65);
        }
        let tx=self.player.x.floor() as i32;let ty=self.player.y.floor() as i32;
        let support=self.floor_height(tx,ty);
        if self.cell(tx,ty)==b'P' || support>=self.player_z-0.08 {
            self.player_z=support;self.fall_speed=0.0;
        } else {
            self.fall_speed=(self.fall_speed+6.5*dt).min(7.0);
            self.player_z=(self.player_z-self.fall_speed*dt).max(support);
            if self.player_z<=support {self.fall_speed=0.0;}
        }
        self.fire_cooldown=(self.fire_cooldown-dt).max(0.0);self.muzzle=(self.muzzle-dt).max(0.0);self.hurt_flash=(self.hurt_flash-dt).max(0.0);self.boss_flash=(self.boss_flash-dt).max(0.0);self.notice_t=(self.notice_t-dt).max(0.0);
        for p in &mut self.pickups {
            if !p.active || p.pos.distance(self.player)>0.55{continue}
            p.active=false;
            match p.kind {
                PickupKind::Ammo=>{self.ammo=(self.ammo+18).min(99);self.score+=25;self.notice="ENERGY CELLS +18".into()},
                PickupKind::Health=>{self.health=(self.health+30).min(100);self.score+=25;self.notice="ARMOR REPAIRED +30".into()},
                PickupKind::Key=>{self.has_key=true;self.score+=150;self.notice="BLUE SECURITY KEY ACQUIRED".into()},
            }
            self.notice_t=2.2;
        }
        if self.player.distance(self.exit)<0.48 {
            if self.objectives_met(){self.mode=Mode::Won;self.score+=500;self.say("MISSION COMPLETE +500",100.0);return}
            if self.notice_t<0.1 {
                let c=mission(self.level);
                if c.needs_key&&!self.has_key{self.say("EXIT REQUIRES SECURITY KEY",1.6)}
                else if self.relays_activated<c.relays{self.say("ACTIVATE ALL POWER RELAYS",1.6)}
                else if c.boss_required&&self.boss_alive(){self.say("DEFEAT THE ARCHIVIST FIRST",1.6)}
                else{self.say("CLEAR REMAINING HOSTILES",1.6)}
            }
        }
        // Copy each enemy so mutable world updates never alias map collision reads.
        for i in 0..self.enemies.len(){
            if !self.enemies[i].active || self.enemies[i].kind==EnemyKind::Archivist{continue}
            let mut e=self.enemies[i];e.attack_timer-=dt;
            let d=e.pos.distance(self.player);
            if d<(match e.kind{EnemyKind::Hunter=>11.0,_=>8.5}) && d>0.83 && self.line_of_sight(e.pos,self.player){
                let speed=match e.kind{EnemyKind::Sentry=>0.65,EnemyKind::Hunter=>1.18,EnemyKind::Warden=>0.48,EnemyKind::Archivist=>0.0};
                let vx=(self.player.x-e.pos.x)/d*speed*dt;
                let vy=(self.player.y-e.pos.y)/d*speed*dt;
                if !self.solid(e.pos.x+vx,e.pos.y,0.18){e.pos.x+=vx;}
                if !self.solid(e.pos.x,e.pos.y+vy,0.18){e.pos.y+=vy;}
            }
            if d<1.13&&e.attack_timer<=0.0{
                self.health=(self.health-(match e.kind{EnemyKind::Sentry=>8,EnemyKind::Hunter=>6,EnemyKind::Warden=>13,EnemyKind::Archivist=>0})).max(0);
                self.hurt_flash=0.3;
                e.attack_timer=match e.kind{EnemyKind::Hunter=>0.9,_=>1.2};
                if self.health==0{self.mode=Mode::Lost;self.say("SIGNAL LOST",100.0);}
            }
            self.enemies[i]=e;
            if self.mode==Mode::Lost{break}
        }
        if self.mode==Mode::Playing{self.tick_boss(dt);}
    }

    fn tick_boss(&mut self,dt:f32){
        let Some(boss)=self.enemies.iter().find(|e|e.kind==EnemyKind::Archivist&&e.active).copied() else {return};
        if self.relays_activated<mission(self.level).relays{return;}
        let phase=if boss.health<=7{3}else if boss.health<=15{2}else{1};
        if phase>self.boss_phase {
            self.boss_phase=phase;
            self.boss_cooldown=if phase==1{2.0}else{0.8};
            self.boss_charge=0.0;
            if phase==2 {
                self.say("ARCHIVIST FRACTURED - GUARDIANS RELEASED",2.6);
                self.summon_guardians([(18.5,12.5),(21.5,5.5)]);
            } else if phase==3 {
                self.say("CORE OVERLOAD - FASTER BEAM CYCLE",2.7);
                self.summon_guardians([(14.5,8.5),(20.5,10.5)]);
            } else {self.say("SHIELD DISRUPTED - ARCHIVIST VULNERABLE",2.5);}
        }
        if self.boss_charge>0.0 {
            self.boss_charge-=dt;
            if self.boss_charge<=0.0 {
                self.boss_charge=0.0;
                self.boss_flash=0.28;
                self.boss_cooldown=if phase==3{2.4}else if phase==2{3.1}else{3.8};
                // The beam aims at the position at the start of the warning.
                // The player can sidestep it or break line-of-sight behind a pillar.
                let radius=if phase==3{1.0}else{0.8};
                if self.player.distance(self.boss_target)<radius
                    && self.player.distance(boss.pos)<14.0
                    && self.line_of_sight(boss.pos,self.player) {
                    self.health=(self.health-if phase==3{24}else{16}).max(0);
                    self.hurt_flash=0.32;
                    if self.health==0{self.mode=Mode::Lost;self.say("SIGNAL LOST",100.0);}
                    else {self.say("BEAM IMPACT - SEEK COVER",1.5);}
                }
            }
        } else {
            self.boss_cooldown-=dt;
            if self.boss_cooldown<=0.0 {
                self.boss_target=self.player;
                self.boss_charge=if phase==3{0.85}else{1.4};
                self.say("BEAM LOCK - DODGE OR TAKE COVER",1.5);
            }
        }
    }

    fn summon_guardians(&mut self,positions:[(f32,f32);2]){
        for (i,(x,y)) in positions.into_iter().enumerate(){
            if !self.solid(x,y,0.23){
                self.enemies.push(Enemy{
                    pos:Vec2::new(x,y),kind:EnemyKind::Hunter,health:3,
                    attack_timer:1.1+i as f32*0.5,active:true,phase:1.0+i as f32,
                });
            }
        }
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::levels::{MISSIONS,EnemyKind};
    use std::collections::{HashSet,VecDeque};
    #[test] fn all_five_maps_are_valid_and_reachable(){
        for (i,spec) in MISSIONS.iter().enumerate(){
            assert_eq!(spec.map.len(),16);
            assert!(spec.map.iter().all(|row|row.len()==24),"bad width in mission {}",i+1);
            assert!(spec.map[0].bytes().all(|c|c==b'#')&&spec.map[15].bytes().all(|c|c==b'#'));
            for row in spec.map {assert_eq!(row.as_bytes()[0],b'#');assert_eq!(row.as_bytes()[23],b'#')}
            let world=World::for_level(i,100,36,0);
            assert!(!world.solid(world.player.x,world.player.y,0.2));
            let initial=(world.player.x as usize,world.player.y as usize);
            let mut seen=HashSet::from([initial]);let mut todo=VecDeque::from([initial]);
            while let Some((x,y))=todo.pop_front(){
                for (dx,dy) in [(-1,0),(1,0),(0,-1),(0,1)]{
                    let xx=(x as i32+dx) as usize;let yy=(y as i32+dy) as usize;
                    // Gates are assumed open after interaction; consoles remain solid.
                    if !matches!(spec.map[yy].as_bytes()[xx],b'#'|b'B'|b'T'|b't'|b'N'|b'n') && seen.insert((xx,yy)){todo.push_back((xx,yy));}
                }
            }
            assert!(seen.contains(&(world.exit.x as usize,world.exit.y as usize)),"mission {} exit unreachable",i+1);
            for e in &world.enemies{assert!(seen.contains(&(e.pos.x as usize,e.pos.y as usize)),"mission {} enemy unreachable at {:?}",i+1,e.pos);}
            for p in &world.pickups{assert!(seen.contains(&(p.pos.x as usize,p.pos.y as usize)),"mission {} pickup unreachable at {:?}",i+1,p.pos);}
            for (y,row) in spec.map.iter().enumerate(){for (x,&tile) in row.as_bytes().iter().enumerate(){
                if matches!(tile,b'T'|b'N'){
                    assert!([(x-1,y),(x+1,y),(x,y-1),(x,y+1)].iter().any(|p|seen.contains(p)),"mission {} console inaccessible at ({},{})",i+1,x,y);
                }
            }}
            assert_eq!(spec.map.iter().flat_map(|r|r.bytes()).filter(|&b|b==b'N').count(),3);
            assert_eq!(spec.map.iter().flat_map(|r|r.bytes()).filter(|&b|b==b'T').count(),spec.relays as usize);
        }
    }
    #[test] fn firing_consumes_one_cell(){let mut w=World::new();assert!(w.shoot());assert_eq!(w.ammo,35);assert!(!w.shoot());}
    #[test] fn final_mission_requires_every_objective(){
        let mut w=World::for_level(3,100,50,0);
        w.player=w.exit;w.tick(0.01);assert_eq!(w.mode,Mode::Playing);
        w.has_key=true;w.relays_activated=3;
        for e in &mut w.enemies {e.active=false}
        w.tick(0.01);assert_eq!(w.mode,Mode::Won);
    }
    #[test] fn boss_shield_and_phase_change(){
        let mut w=World::for_level(4,100,90,0);
        let boss=w.enemies.iter().position(|e|e.kind==EnemyKind::Archivist).unwrap();
        assert_eq!(w.enemies[boss].health,24);
        let boss_pos=w.enemies[boss].pos;
        w.player=Vec2::new(boss_pos.x-1.5,boss_pos.y);
        w.facing=0.0;
        let _=w.shoot();assert_eq!(w.enemies[boss].health,24);
        w.relays_activated=3;
        w.fire_cooldown=0.0;
        assert!(w.shoot());assert_eq!(w.enemies[boss].health,23);
        w.tick(0.01);assert_eq!(w.boss_phase,1);
    }
    #[test] fn final_chamber_exit_requires_boss_defeat(){
        let mut w=World::for_level(4,100,80,0);
        w.player=w.exit;w.relays_activated=3;
        w.tick(0.01);assert_eq!(w.mode,Mode::Playing);
        for enemy in &mut w.enemies {
            if enemy.kind==EnemyKind::Archivist{enemy.active=false;}
        }
        w.tick(0.01);assert_eq!(w.mode,Mode::Won);
    }
    #[test] fn locked_boss_beam_can_be_dodged(){
        let mut w=World::for_level(4,100,80,0);
        w.relays_activated=3;w.boss_phase=1;
        w.player=Vec2::new(4.5,2.5);
        w.boss_target=Vec2::new(3.0,2.5);
        w.boss_charge=0.01;
        w.tick(0.02);
        assert_eq!(w.health,100);
        assert_eq!(w.boss_charge,0.0);
    }
    #[test] fn warden_has_more_health(){let w=World::for_level(3,100,30,0);assert!(w.enemies.iter().any(|e|e.kind==EnemyKind::Warden&&e.health==5));}
}

#[cfg(test)]
mod lift_tests {
    use super::*;

    #[test]
    fn each_mission_has_functioning_two_stop_lift() {
        for level in 0..MISSIONS.len() {
            let mut world=World::for_level(level,100,50,0);
            let elevator=world.lifts[0];
            world.enemies.clear();
            assert_eq!(world.cell(elevator.x,elevator.y),b'P');
            assert_eq!(world.cell(elevator.x+1,elevator.y),b'U');
            assert_eq!(world.floor_height(elevator.x+1,elevator.y),1.25);
            world.player=Vec2::new(elevator.x as f32+0.5,elevator.y as f32+0.5);
            world.lifts[0].target=1.25;
            for _ in 0..110 {world.tick(0.02);}
            assert!((world.lifts[0].height-1.25).abs()<0.001);
            assert!((world.player_z-1.25).abs()<0.001);
            world.lifts[0].target=0.0;
            for _ in 0..110 {world.tick(0.02);}
            assert!(world.lifts[0].height<0.001);
            assert!(world.player_z<0.001);
        }
    }

    #[test]
    fn crosshair_offsets_are_bounded() {
        let mut world=World::new();
        world.aim(1_000_000.0,960.0);
        assert!((world.aim_x-0.68).abs()<0.0001);
        world.aim(-2_000_000.0,960.0);
        assert!((world.aim_x+0.68).abs()<0.0001);
    }
}
