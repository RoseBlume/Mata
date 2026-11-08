//! Deterministic, frame-rate-independent tower-defense simulation.
//! No external game engine or random-number crate is required.

pub const CANVAS_W: i32 = 1440;
pub const CANVAS_H: i32 = 900;
pub const COLS: i32 = 15;
pub const ROWS: i32 = 10;
pub const TILE: f32 = 60.0;
pub const BOARD_X: f32 = 68.0;
pub const BOARD_Y: f32 = 166.0;
pub const LAST_WAVE: u32 = 20;

#[derive(Clone, Copy, Debug, Default)]
pub struct Vec2 { pub x: f32, pub y: f32 }
impl Vec2 {
    pub const fn new(x: f32, y: f32) -> Self { Self { x, y } }
    pub fn distance(self, rhs: Self) -> f32 { ((self.x-rhs.x).powi(2)+(self.y-rhs.y).powi(2)).sqrt() }
    pub fn toward(self, rhs: Self, alpha: f32) -> Self { Self::new(self.x+(rhs.x-self.x)*alpha,self.y+(rhs.y-self.y)*alpha) }
}
pub fn tile_center(col: i32, row: i32) -> Vec2 {
    Vec2::new(BOARD_X+(col as f32+0.5)*TILE,BOARD_Y+(row as f32+0.5)*TILE)
}
pub fn tile_at(x: f32,y: f32) -> Option<(i32,i32)> {
    let c=((x-BOARD_X)/TILE).floor() as i32;
    let r=((y-BOARD_Y)/TILE).floor() as i32;
    (c>=0 && r>=0 && c<COLS && r<ROWS).then_some((c,r))
}
pub fn on_path(col: i32,row: i32) -> bool {
    (row==2 && (0..=9).contains(&col)) ||
    (col==9 && (2..=5).contains(&row)) ||
    (row==5 && (4..=9).contains(&col)) ||
    (col==4 && (5..=8).contains(&row)) ||
    (row==8 && (4..=14).contains(&col))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TowerKind { Bolt, Cannon, Frost, Prism }
impl TowerKind {
    pub const ALL: [Self;4]=[Self::Bolt,Self::Cannon,Self::Frost,Self::Prism];
    pub fn name(self) -> &'static str { match self { Self::Bolt=>"SENTINEL",Self::Cannon=>"ARTILLERY",Self::Frost=>"CRYO ARRAY",Self::Prism=>"PRISM BEAM" } }
    pub fn role(self) -> &'static str { match self { Self::Bolt=>"Fast single target",Self::Cannon=>"Area explosion",Self::Frost=>"Slows enemies",Self::Prism=>"Instant energy strike" } }
    pub fn price(self) -> i32 { match self { Self::Bolt=>85,Self::Cannon=>145,Self::Frost=>115,Self::Prism=>210 } }
    pub fn color(self) -> u32 { match self { Self::Bolt=>0xFF54D7B2,Self::Cannon=>0xFFFFAB5F,Self::Frost=>0xFF6CB9F2,Self::Prism=>0xFFD59BFF } }
    pub fn base_range(self) -> f32 { match self { Self::Bolt=>144.0,Self::Cannon=>162.0,Self::Frost=>147.0,Self::Prism=>174.0 } }
    pub fn base_damage(self) -> f32 { match self { Self::Bolt=>19.0,Self::Cannon=>48.0,Self::Frost=>8.0,Self::Prism=>15.0 } }
    pub fn interval(self) -> f32 { match self { Self::Bolt=>0.38,Self::Cannon=>1.42,Self::Frost=>0.82,Self::Prism=>0.25 } }
    pub fn splash(self) -> f32 { if self==Self::Cannon { 66.0 } else { 0.0 } }
    pub fn bullet_speed(self) -> f32 { match self { Self::Bolt=>520.0,Self::Cannon=>330.0,Self::Frost=>380.0,Self::Prism=>0.0 } }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyKind { Scout, Grunt, Bulwark, Phantom }
impl EnemyKind {
    pub fn name(self) -> &'static str { match self {Self::Scout=>"SCOUT",Self::Grunt=>"GRUNT",Self::Bulwark=>"BULWARK",Self::Phantom=>"PHANTOM"} }
    pub fn color(self) -> u32 {match self { Self::Scout=>0xFFFFBE74,Self::Grunt=>0xFFEF707A,Self::Bulwark=>0xFFAB88D9,Self::Phantom=>0xFF70DBC8 }}
    pub fn speed(self) -> f32 { match self { Self::Scout=>105.0,Self::Grunt=>70.0,Self::Bulwark=>44.0,Self::Phantom=>135.0 } }
    pub fn hp(self) -> f32 { match self { Self::Scout=>57.0,Self::Grunt=>118.0,Self::Bulwark=>360.0,Self::Phantom=>86.0 } }
    pub fn reward(self) -> i32 { match self { Self::Scout=>12,Self::Grunt=>16,Self::Bulwark=>38,Self::Phantom=>22 } }
    pub fn leaks(self) -> i32 {if self==Self::Bulwark { 2 } else { 1 }}
}

pub struct Enemy {
    pub id: u64,
    pub kind: EnemyKind,
    pub distance: f32,
    pub pos: Vec2,
    pub hp: f32,
    pub max_hp: f32,
    pub speed: f32,
    pub slow_time: f32,
    pub reward: i32,
}

pub struct Tower {
    pub kind: TowerKind,
    pub col: i32,
    pub row: i32,
    pub level: u8,
    pub cooldown: f32,
    pub invested: i32,
    pub facing: f32,
    pub firing: f32,
}
impl Tower {
    pub fn pos(&self) -> Vec2 { tile_center(self.col,self.row) }
    pub fn damage(&self) -> f32 { self.kind.base_damage()*(1.0+0.68*(self.level as f32-1.0)) }
    pub fn range(&self) -> f32 { self.kind.base_range()+16.0*(self.level as f32-1.0) }
    pub fn interval(&self) -> f32 {self.kind.interval()*(0.90f32).powi(self.level as i32-1)}
    pub fn upgrade_cost(&self) -> i32 { (self.kind.price() as f32 *(0.75+0.47*self.level as f32)).round() as i32 }
    pub fn sell_value(&self) -> i32 { (self.invested as f32*0.7).floor() as i32 }
}

pub struct Projectile {
    pub pos: Vec2,
    pub target: u64,
    pub kind: TowerKind,
    pub damage: f32,
    pub radius: f32,
    pub speed: f32,
    pub ttl: f32,
}
#[derive(Clone)]
pub struct Effect { pub from: Vec2, pub at: Vec2, pub color: u32, pub time: f32, pub max_time: f32, pub radius: f32 }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase { Ready, Running, Won, Lost }

pub struct Game {
    pub towers: Vec<Tower>,
    pub enemies: Vec<Enemy>,
    pub projectiles: Vec<Projectile>,
    pub effects: Vec<Effect>,
    pub phase: Phase,
    pub wave: u32,
    pub lives: i32,
    pub gold: i32,
    pub kills: u32,
    pub build: Option<TowerKind>,
    pub selected: Option<usize>,
    pub paused: bool,
    pub double_speed: bool,
    pub elapsed: f32,
    pub status: String,
    pub status_timer: f32,
    pub next_enemy_id: u64,
    pub spawn_left: u32,
    pub spawn_index: u32,
    pub spawn_timer: f32,
    pub path_points: Vec<Vec2>,
    pub path_length: f32,
}
impl Game {
    pub fn new() -> Self {
        let path_points= [(-1,2),(9,2),(9,5),(4,5),(4,8),(15,8)].map(|(c,r)|tile_center(c,r)).to_vec();
        let path_length=path_points.windows(2).map(|w|w[0].distance(w[1])).sum();
        Self {
            towers:Vec::new(),enemies:Vec::new(),projectiles:Vec::new(),effects:Vec::new(),
            phase:Phase::Ready,wave:0,lives:20,gold:330,kills:0,
            build:None,selected:None,paused:false,double_speed:false,elapsed:0.0,
            status:"Build your defenses. The first wave is ready.".into(),status_timer:7.0,
            next_enemy_id:0,spawn_left:0,spawn_index:0,spawn_timer:0.0,
            path_points,path_length,
        }
    }
    pub fn announce(&mut self,message: impl Into<String>) { self.status=message.into();self.status_timer=3.0; }
    pub fn point_on_path(&self, mut distance: f32) -> Vec2 {
        for w in self.path_points.windows(2) {
            let len=w[0].distance(w[1]);
            if distance<=len {return w[0].toward(w[1],(distance/len).clamp(0.0,1.0));}
            distance-=len;
        }
        *self.path_points.last().unwrap()
    }
    pub fn tower_at(&self,col:i32,row:i32)->Option<usize> {self.towers.iter().position(|t|t.col==col&&t.row==row)}
    pub fn can_place(&self,col:i32,row:i32)->bool {
        (0..COLS).contains(&col)&&(0..ROWS).contains(&row)&&!on_path(col,row)&&self.tower_at(col,row).is_none()
    }
    pub fn choose_build(&mut self, kind: TowerKind) {
        if matches!(self.phase,Phase::Lost|Phase::Won) {return;}
        self.selected=None;
        if self.build==Some(kind) {self.build=None;} else {self.build=Some(kind);}
    }
    pub fn click_grid(&mut self,col:i32,row:i32) {
        if matches!(self.phase,Phase::Lost|Phase::Won) {return;}
        if let Some(kind)=self.build {
            if !self.can_place(col,row) {self.announce("This tile is blocked. Choose open ground.");return;}
            if self.gold<kind.price() {self.announce("Not enough credits to deploy.");return;}
            self.gold-=kind.price();
            self.towers.push(Tower {kind,col,row,level:1,cooldown:0.15,invested:kind.price(),facing:-1.57,firing:0.0});
            self.selected=Some(self.towers.len()-1);
            self.build=None;
            self.announce(format!("{} deployed. Select it to upgrade.",kind.name()));
        } else {
            self.selected=self.tower_at(col,row);
        }
    }
    pub fn upgrade_selected(&mut self) {
        let Some(idx)=self.selected else{return};
        let Some(t)=self.towers.get_mut(idx) else {self.selected=None;return};
        if t.level>=4 {self.announce("This tower has reached maximum level.");return;}
        let cost=t.upgrade_cost();
        if self.gold<cost {self.announce("Not enough credits to upgrade.");return;}
        self.gold-=cost;t.invested+=cost;t.level+=1;
        self.announce("Tower upgraded: damage, range and attack speed increased.");
    }
    pub fn sell_selected(&mut self) {
        if let Some(idx)=self.selected.take() {
            if idx<self.towers.len() {
                let t=self.towers.remove(idx);
                self.gold+=t.sell_value();
                self.announce(format!("{} sold for {} credits.",t.kind.name(),t.sell_value()));
            }
        }
    }
    pub fn start_wave(&mut self) {
        if self.phase!=Phase::Ready {return;}
        self.wave+=1;self.phase=Phase::Running;self.paused=false;
        self.spawn_left=7+self.wave*2;
        self.spawn_index=0;
        self.spawn_timer=0.12;
        self.announce(format!("INCOMING // Wave {:02} is entering the grid",self.wave));
    }
    fn kind_for_spawn(&self,idx:u32)->EnemyKind {
        if self.wave>=10 && idx%8==6 {EnemyKind::Bulwark}
        else if self.wave>=6 && idx%7==4 {EnemyKind::Phantom}
        else if self.wave>=3 && idx%3==2 {EnemyKind::Grunt}
        else {EnemyKind::Scout}
    }
    fn spawn_enemy(&mut self) {
        let kind=self.kind_for_spawn(self.spawn_index);
        let multiplier=1.0+(self.wave as f32-1.0)*0.19+(self.wave as f32-1.0).powf(1.36)*0.038;
        self.next_enemy_id+=1;
        self.enemies.push(Enemy {
            id:self.next_enemy_id,kind,distance:0.0,pos:self.path_points[0],
            hp:kind.hp()*multiplier,max_hp:kind.hp()*multiplier,
            speed:kind.speed()*(1.0+0.012*(self.wave as f32-1.0)),slow_time:0.0,
            reward:kind.reward()+(self.wave/4) as i32*2,
        });
        self.spawn_index+=1;
    }
    fn damage(&mut self, point:Vec2, target:u64, kind:TowerKind, amount:f32, radius:f32) {
        let mut did_hit=false;
        for e in &mut self.enemies {
            if e.hp<=0.0 {continue;}
            let dist=e.pos.distance(point);
            if (radius>0.0 && dist<=radius) || (radius<=0.0 && e.id==target) {
                let effective=if radius>0.0 {(1.0-0.45*(dist/radius)).max(0.45)}else{1.0};
                e.hp-=(amount*effective)*if e.kind==EnemyKind::Bulwark {0.83}else{1.0};
                if kind==TowerKind::Frost {e.slow_time=2.3;}
                did_hit=true;
            }
        }
        if did_hit {
            self.effects.push(Effect {from:point,at:point,color:kind.color(),time:0.34,max_time:0.34,radius:if radius>0.0 {radius}else{19.0}});
        }
    }
    pub fn update(&mut self,dt:f32) {
        let dt=dt.clamp(0.0,0.05);
        self.elapsed+=dt;
        self.status_timer=(self.status_timer-dt).max(0.0);
        for effect in &mut self.effects {effect.time-=dt;}
        self.effects.retain(|v|v.time>0.0);
        for tower in &mut self.towers {tower.firing=(tower.firing-dt).max(0.0);}
        if self.phase!=Phase::Running || self.paused {return;}

        if self.spawn_left>0 {
            self.spawn_timer-=dt;
            if self.spawn_timer<=0.0 {
                self.spawn_enemy();self.spawn_left-=1;
                self.spawn_timer+=(0.85-(self.wave as f32*0.022)).max(0.28);
            }
        }

        let points=&self.path_points;
        let path_length=self.path_length;
        let mut leaks=0i32;
        for enemy in &mut self.enemies {
            enemy.slow_time=(enemy.slow_time-dt).max(0.0);
            let factor=if enemy.slow_time>0.0 {0.48}else{1.0};
            enemy.distance+=enemy.speed*factor*dt;
            let mut d=enemy.distance;
            let mut at=*points.last().unwrap();
            for line in points.windows(2) {
                let len=line[0].distance(line[1]);
                if d<=len {at=line[0].toward(line[1],(d/len).clamp(0.0,1.0));break;}
                d-=len;
            }
            enemy.pos=at;
            if enemy.distance>=path_length {leaks+=enemy.kind.leaks();enemy.hp=-1.0e9;}
        }

        // Target the enemy furthest along the route, breaking ties by HP.
        for tower in &mut self.towers {
            tower.cooldown-=dt;
            if tower.cooldown>0.0 {continue;}
            let pos=tower.pos();
            let target=self.enemies.iter()
                .filter(|e|e.hp>0.0 && pos.distance(e.pos)<=tower.range())
                .max_by(|a,b| a.distance.total_cmp(&b.distance).then(a.hp.total_cmp(&b.hp)));
            if let Some(target)=target {
                tower.facing=(target.pos.y-pos.y).atan2(target.pos.x-pos.x);
                tower.firing=0.17;
                if tower.kind==TowerKind::Prism {
                    self.effects.push(Effect {from:pos,at:target.pos,color:tower.kind.color(),time:0.11,max_time:0.11,radius:0.0});
                    target_id_fire(&mut self.projectiles,pos,target.id,tower);
                } else {
                    target_id_fire(&mut self.projectiles,pos,target.id,tower);
                }
                tower.cooldown+=tower.interval();
            } else {tower.cooldown=0.07;}
        }

        let mut flying=std::mem::take(&mut self.projectiles);
        for mut p in flying.drain(..) {
            p.ttl-=dt;
            if p.ttl<=0.0 {continue;}
            let Some(target)=self.enemies.iter().find(|e|e.id==p.target&&e.hp>0.0) else {continue;};
            let dest=target.pos;
            if p.kind==TowerKind::Prism {
                self.damage(dest,p.target,p.kind,p.damage,p.radius);
                continue;
            }
            let distance=p.pos.distance(dest);
            let travel=p.speed*dt;
            if distance<=travel.max(9.0) {
                self.damage(dest,p.target,p.kind,p.damage,p.radius);
            } else {
                p.pos=p.pos.toward(dest,travel/distance);
                self.projectiles.push(p);
            }
        }
        let mut earned=0;
        let mut killed=0;
        self.enemies.retain(|e|{
            if e.hp<=0.0 {
                if e.hp> -1.0e8 {earned+=e.reward;killed+=1;}
                false
            } else {true}
        });
        self.gold+=earned;self.kills+=killed;
        if leaks>0 {
            self.lives=(self.lives-leaks).max(0);
            self.announce(format!("BREACH // {} integrity lost",leaks));
            if self.lives==0 {self.phase=Phase::Lost;self.paused=false;self.build=None;self.selected=None;return;}
        }
        if self.spawn_left==0 && self.enemies.is_empty() {
            if self.wave>=LAST_WAVE {
                self.phase=Phase::Won;
                self.announce("GRID SECURED // All waves defeated!");
            } else {
                self.phase=Phase::Ready;
                let reward=45+(self.wave as i32)*12;
                self.gold+=reward;
                self.announce(format!("Wave {:02} cleared! +{} credits. Prepare defenses.",self.wave,reward));
            }
        }
    }
}
fn target_id_fire(projectiles:&mut Vec<Projectile>,from:Vec2,id:u64,t:&Tower) {
    projectiles.push(Projectile {pos:from,target:id,kind:t.kind,damage:t.damage(),radius:t.kind.splash()*(1.0+0.10*(t.level as f32-1.0)),speed:t.kind.bullet_speed(),ttl:4.0});
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn map_and_placement() {
        let mut g=Game::new();assert!(g.can_place(0,0));assert!(!g.can_place(0,2));
        g.choose_build(TowerKind::Bolt);g.click_grid(0,0);
        assert_eq!(g.towers.len(),1);assert_eq!(g.gold,245);
        assert!(!g.can_place(0,0));
    }
    #[test] fn upgrade_and_sell_refund() {
        let mut g=Game::new();g.choose_build(TowerKind::Bolt);g.click_grid(1,0);
        g.upgrade_selected();assert_eq!(g.towers[0].level,2);
        let refund=g.towers[0].sell_value();let before=g.gold;
        g.sell_selected();assert_eq!(g.gold,before+refund);assert!(g.towers.is_empty());
    }
    #[test] fn wave_spawns_and_resolves() {
        let mut g=Game::new();g.start_wave();assert_eq!(g.phase,Phase::Running);
        for _ in 0..3600 {g.update(1.0/60.0);if g.phase!=Phase::Running {break;}}
        assert_eq!(g.phase,Phase::Ready);assert!(g.lives<20);assert!(g.gold>330);
    }
    #[test] fn path_is_connected() {
        let g=Game::new();assert!(g.path_length>1800.0);
        assert_eq!(g.point_on_path(g.path_length).x,tile_center(15,8).x);
    }
}
