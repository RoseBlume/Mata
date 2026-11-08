//! Game simulation. No external libraries, IO, windowing, or rendering.
use std::f32::consts::TAU;

pub const WORLD_W:f32=3600.0;
pub const WORLD_H:f32=2800.0;
pub const ROUND_SECONDS:f32=150.0;

/// Keep the center movable even after the hole becomes wider than the world.
fn clamp_hole_center(value:f32,radius:f32,extent:f32)->f32 {
    if radius<extent*0.5 {value.clamp(radius,extent-radius)}
    else {value.clamp(0.0,extent)}
}

#[derive(Clone,Copy,Debug,Default,PartialEq)]
pub struct V {pub x:f32,pub y:f32}
impl V {
    pub const fn new(x:f32,y:f32)->Self{Self{x,y}}
    pub fn distance2(self,b:V)->f32 {(self.x-b.x).powi(2)+(self.y-b.y).powi(2)}
    pub fn len(self)->f32 {(self.x*self.x+self.y*self.y).sqrt()}
    pub fn normalized(self)->Self {let l=self.len();if l>0.001 {Self::new(self.x/l,self.y/l)}else{Self::default()}}
}
impl std::ops::Add for V {type Output=Self;fn add(self,b:Self)->Self{Self::new(self.x+b.x,self.y+b.y)}}
impl std::ops::Sub for V {type Output=Self;fn sub(self,b:Self)->Self{Self::new(self.x-b.x,self.y-b.y)}}
impl std::ops::Mul<f32> for V {type Output=Self;fn mul(self,n:f32)->Self{Self::new(self.x*n,self.y*n)}}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Kind {Snack,Cone,Bin,Bench,Tree,Car,House}
impl Kind {
    pub fn radius(self)->f32{match self {Self::Snack=>7.0,Self::Cone=>10.0,Self::Bin=>13.0,Self::Bench=>18.0,Self::Tree=>22.0,Self::Car=>32.0,Self::House=>66.0}}
    pub fn points(self)->u32 {match self {Self::Snack=>3,Self::Cone=>6,Self::Bin=>11,Self::Bench=>18,Self::Tree=>30,Self::Car=>70,Self::House=>185}}
    pub fn required(self)->f32 {match self {Self::Snack=>0.0,Self::Cone=>23.0,Self::Bin=>29.0,Self::Bench=>36.0,Self::Tree=>44.0,Self::Car=>60.0,Self::House=>94.0}}
    pub fn name(self)->&'static str {match self {Self::Snack=>"STREET JUNK",Self::Cone=>"TRAFFIC CONES",Self::Bin=>"BINS",Self::Bench=>"BENCHES",Self::Tree=>"TREES",Self::Car=>"CARS",Self::House=>"BUILDINGS"}}
}
#[derive(Clone,Copy,Debug)]
pub struct Prop {pub pos:V,pub kind:Kind,pub tint:u8,pub alive:bool,pub respawn_at:f32}
#[derive(Clone,Debug)]
pub struct Hole {pub pos:V,pub vel:V,pub radius:f32,mass:f64,pub score:u32,pub name:&'static str,pub color:u32,pub energy:f32,pub respawn:f32,pub target:V,pub target_delay:f32,pub eats:u32}
impl Hole {
    pub fn new(pos:V,name:&'static str,color:u32)->Self{Self{pos,vel:V::default(),radius:23.5,mass:23.5_f64*23.5_f64,score:0,name,color,energy:1.0,respawn:0.0,target:pos,target_delay:0.0,eats:0}}
    pub fn alive(&self)->bool{self.respawn<=0.0}
    pub fn grow(&mut self,points:u32) {
        self.score=self.score.saturating_add(points);
        self.eats=self.eats.saturating_add(1);
        // Keep growing without a gameplay cap. Store squared radius as f64 so
        // small pickups still accumulate after the hole becomes enormous.
        self.mass+=f64::from(points)*13.0;
        self.radius=self.mass.sqrt() as f32;
    }
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Phase {Title,Playing,Finished}
#[derive(Clone,Debug)]
pub struct Particle {pub pos:V,pub vel:V,pub life:f32,pub color:u32}
#[derive(Clone,Copy,Debug,Default)]
pub struct Input {pub movement:V,pub boosting:bool}
#[derive(Clone,Debug)]
pub struct Random(u64);
impl Random {
    pub fn new(seed:u64)->Self{Self(seed.max(1))}
    pub fn next(&mut self)->u32 {let mut x=self.0;x^=x<<13;x^=x>>7;x^=x<<17;self.0=x;(x>>16) as u32}
    pub fn float(&mut self)->f32 {self.next() as f32/u32::MAX as f32}
    pub fn range(&mut self,lo:f32,hi:f32)->f32{lo+(hi-lo)*self.float()}
    pub fn idx(&mut self,n:usize)->usize{(self.next() as usize)%n}
}

pub struct Game {
    pub phase:Phase,pub paused:bool,pub elapsed:f32,pub holes:Vec<Hole>,pub props:Vec<Prop>,
    pub particles:Vec<Particle>,pub rng:Random,pub combo:u32,pub combo_timer:f32,
    pub last_eaten:Option<Kind>,pub last_eaten_timer:f32,pub winner:usize,pub player_eaten:u32,
}
impl Game {
    pub fn new()->Self {
        let mut rng=Random::new(0xAEEB_5133_7725_2019);
        let mut props=Vec::with_capacity(1400);
        // Large buildings sit inside the blocks; roads form a readable city grid.
        for ix in 0..10 {for iy in 0..8 {
            let center=V::new(ix as f32*360.0+180.0,iy as f32*350.0+175.0);
            if center.x<WORLD_W-80.0&&center.y<WORLD_H-80.0 {
                props.push(Prop{pos:center,kind:Kind::House,tint:rng.idx(5) as u8,alive:true,respawn_at:0.0});
            }
        }}
        for _ in 0..1260 {
            let pos=V::new(rng.range(44.0,WORLD_W-44.0),rng.range(44.0,WORLD_H-44.0));
            let n=rng.float();
            let kind=if n<0.43 {Kind::Snack}else if n<0.56 {Kind::Cone}else if n<0.69{Kind::Bin}
                     else if n<0.80{Kind::Bench}else if n<0.91{Kind::Tree}else{Kind::Car};
            // Don't spawn props inside buildings (houses are placed in every block).
            let crowded=props.iter().take(80).any(|b|pos.distance2(b.pos)<(b.kind.radius()+kind.radius()+12.0).powi(2));
            if crowded{continue;}
            props.push(Prop{pos,kind,tint:rng.idx(6) as u8,alive:true,respawn_at:0.0});
        }
        let holes=vec![
            Hole::new(V::new(1800.0,1390.0),"YOU",0xFF9B7BFF),
            Hole::new(V::new(500.0,520.0),"NOVA",0xFFFF7783),
            Hole::new(V::new(3000.0,600.0),"MINT",0xFF5BE4B6),
            Hole::new(V::new(400.0,2270.0),"BYTE",0xFFFFCB6B),
            Hole::new(V::new(2990.0,2130.0),"VOID",0xFF67CEFC),
            Hole::new(V::new(1650.0,340.0),"ECHO",0xFFFF98D8),
        ];
        Self{phase:Phase::Title,paused:false,elapsed:0.0,holes,props,particles:Vec::new(),rng,combo:0,combo_timer:0.0,last_eaten:None,last_eaten_timer:0.0,winner:0,player_eaten:0}
    }
    pub fn remaining(&self)->f32{(ROUND_SECONDS-self.elapsed).max(0.0)}
    pub fn start(&mut self){self.phase=Phase::Playing;self.paused=false}
    pub fn toggle_pause(&mut self){if self.phase==Phase::Playing{self.paused=!self.paused}}
    pub fn next_unlock(&self)->Option<(Kind,f32)>{
        [Kind::Cone,Kind::Bin,Kind::Bench,Kind::Tree,Kind::Car,Kind::House].into_iter().find_map(|k|
            (self.holes[0].radius<k.required()).then_some((k,k.required())))
    }
    pub fn standings(&self)->Vec<usize> {
        let mut list=(0..self.holes.len()).collect::<Vec<_>>();
        list.sort_by_key(|&i|std::cmp::Reverse(self.holes[i].score));list
    }
    fn add_burst(&mut self,pos:V,color:u32,count:usize){
        if self.particles.len()>140{self.particles.drain(..self.particles.len()-100);}
        for _ in 0..count {
            let a=self.rng.float()*TAU;let s=self.rng.range(42.0,155.0);
            self.particles.push(Particle{pos,vel:V::new(a.cos()*s,a.sin()*s),life:self.rng.range(0.25,0.65),color});
        }
    }
    pub fn update(&mut self,dt:f32,input:Input){
        if self.phase!=Phase::Playing||self.paused{return;}
        let dt=dt.clamp(0.0,0.05);
        self.elapsed+=dt;
        self.combo_timer=(self.combo_timer-dt).max(0.0);
        if self.combo_timer==0.0{self.combo=0;}
        self.last_eaten_timer=(self.last_eaten_timer-dt).max(0.0);
        if self.remaining()<=0.0{self.phase=Phase::Finished;self.winner=self.standings()[0];return;}
        for prop in &mut self.props {
            if !prop.alive && self.elapsed>=prop.respawn_at {prop.alive=true;}
        }
        for particle in &mut self.particles {particle.pos=particle.pos+particle.vel*dt;particle.life-=dt;}
        self.particles.retain(|p|p.life>0.0);
        for i in 0..self.holes.len(){
            if self.holes[i].respawn>0.0 {
                self.holes[i].respawn=(self.holes[i].respawn-dt).max(0.0);
                if self.holes[i].respawn==0.0 {let spawn=self.random_spawn();self.holes[i].pos=spawn;}
                continue;
            }
            let mut direction=if i==0 {input.movement.normalized()}else{self.ai_direction(i,dt)};
            if direction.len()>1.0{direction=direction.normalized();}
            let hole=&mut self.holes[i];
            let boosted=if i==0 {input.boosting && hole.energy>0.08}else{hole.energy>0.4&&hole.target_delay<0.12&&i%3==0};
            if boosted{hole.energy=(hole.energy-dt*0.34).max(0.0);}else{hole.energy=(hole.energy+dt*0.16).min(1.0);}
            let speed=(225.0/(1.0+(hole.radius/120.0))).max(90.0)*if boosted{1.64}else{1.0};
            let desired=direction*speed;
            let t=(dt*9.5).min(1.0);hole.vel=hole.vel*(1.0-t)+desired*t;
            hole.pos=hole.pos+hole.vel*dt;
            // Keep small holes fully inside the city. Once a hole is larger
            // than half the city, keep its *center* inside instead. Otherwise
            // f32::clamp would panic because min > max after enough growth.
            hole.pos.x=clamp_hole_center(hole.pos.x,hole.radius,WORLD_W);
            hole.pos.y=clamp_hole_center(hole.pos.y,hole.radius,WORLD_H);
        }
        // Objects can be swallowed only after the hole is large enough.
        for i in 0..self.holes.len(){
            if !self.holes[i].alive(){continue;}
            let hole_pos=self.holes[i].pos;
            let hole_radius=self.holes[i].radius;
            for j in 0..self.props.len() {
                let prop=&self.props[j];
                if !prop.alive||hole_radius<prop.kind.required(){continue;}
                let grab=(hole_radius-prop.kind.radius()*0.25).max(3.0);
                if hole_pos.distance2(prop.pos)>grab*grab {continue;}
                let kind=prop.kind;let at=prop.pos;let points=kind.points();
                let cooldown=if kind==Kind::House {65.0}else{self.rng.range(18.0,38.0)};
                self.props[j].alive=false;self.props[j].respawn_at=self.elapsed+cooldown;
                let earned=if i==0 {
                    self.combo=if self.combo_timer>0.0 {self.combo.saturating_add(1)}else{1};
                    self.combo_timer=3.2;
                    self.last_eaten=Some(kind);self.last_eaten_timer=1.6;self.player_eaten=self.player_eaten.saturating_add(1);
                    points+(self.combo.min(12)/4)*points/4
                }else{points};
                self.holes[i].grow(earned);
                let color=match kind{Kind::Snack=>0xFFFFD783,Kind::Cone=>0xFFFF9E60,Kind::Bin=>0xFF8CA8E9,
                    Kind::Bench=>0xFFE5B995,Kind::Tree=>0xFF5BDC9A,Kind::Car=>0xFF84CCF1,Kind::House=>0xFFB9A8FD};
                self.add_burst(at,color,if kind==Kind::House{14}else{5});
            }
        }
        // Larger holes absorb opponents. A knockout delays respawn and costs score.
        for i in 0..self.holes.len(){for j in (i+1)..self.holes.len(){
            if !self.holes[i].alive()||!self.holes[j].alive(){continue;}
            let (big,small)=if self.holes[i].radius>self.holes[j].radius*1.16 {(i,j)}
                else if self.holes[j].radius>self.holes[i].radius*1.16 {(j,i)}else{continue;};
            let a=&self.holes[big];let b=&self.holes[small];
            let close=a.pos.distance2(b.pos)<(a.radius-b.radius*0.25).max(4.0).powi(2);
            if close {
                let bounty=90+self.holes[small].score/8;
                let at=self.holes[small].pos;let color=self.holes[small].color;
                self.holes[big].grow(bounty);
                self.holes[small].score=(self.holes[small].score as f32*0.70) as u32;
                let shrunken=(self.holes[small].radius*0.82).max(23.5);
                self.holes[small].radius=shrunken;
                self.holes[small].mass=f64::from(shrunken).powi(2);
                self.holes[small].respawn=3.0;
                self.holes[small].vel=V::default();
                if small==0{self.combo=0;self.combo_timer=0.0;}
                self.add_burst(at,color,19);
            }
        }}
    }
    fn random_spawn(&mut self)->V{
        V::new(self.rng.range(180.0,WORLD_W-180.0),self.rng.range(180.0,WORLD_H-180.0))
    }
    fn ai_direction(&mut self,i:usize,dt:f32)->V {
        let pos=self.holes[i].pos;
        self.holes[i].target_delay-=dt;
        if self.holes[i].target_delay<=0.0 || pos.distance2(self.holes[i].target)<50.0*50.0 {
            let r=self.holes[i].radius;let mut nearest=f32::MAX;let mut found=None;
            // AI opportunistically consumes smaller opponents and props.
            for (j,h) in self.holes.iter().enumerate(){
                if i!=j&&h.alive()&&r>h.radius*1.28 {
                    let d=pos.distance2(h.pos)*0.60;
                    if d<nearest {nearest=d;found=Some(h.pos);}
                }
            }
            for prop in &self.props {
                if !prop.alive||r<prop.kind.required(){continue;}
                let d=pos.distance2(prop.pos);
                if d<nearest{nearest=d;found=Some(prop.pos);}
            }
            let next_target=found.unwrap_or_else(||V::new(self.rng.range(50.0,WORLD_W-50.0),self.rng.range(50.0,WORLD_H-50.0)));
            self.holes[i].target=next_target;
            self.holes[i].target_delay=self.rng.range(0.3,0.75);
        }
        (self.holes[i].target-pos).normalized()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn deterministic_city(){let a=Game::new();let b=Game::new();assert_eq!(a.props.len(),b.props.len());assert_eq!(a.props[99].pos,b.props[99].pos);}
    #[test] fn radius_grows_past_old_cap_and_city(){
        let mut h=Hole::new(V::new(5.0,5.0),"X",0);
        h.grow(100);assert!(h.radius>23.5);assert_eq!(h.score,100);
        h.grow(10_000_000);
        assert!(h.radius>165.0);
        assert!(h.radius>WORLD_W);
        let before=h.radius;
        h.grow(10_000_000);
        assert!(h.radius>before);
    }
    #[test] fn enormous_holes_can_move_without_invalid_clamps(){
        let mut g=Game::new();g.start();
        g.holes[0].grow(10_000_000);
        assert!(g.holes[0].radius>WORLD_W);
        for _ in 0..60 {g.update(1.0/60.0,Input{movement:V::new(1.0,-1.0),boosting:false});}
        assert!((0.0..=WORLD_W).contains(&g.holes[0].pos.x));
        assert!((0.0..=WORLD_H).contains(&g.holes[0].pos.y));
    }
    #[test] fn continued_growth_after_score_saturates(){
        let mut h=Hole::new(V::new(5.0,5.0),"X",0);
        h.grow(u32::MAX);
        let prior=h.radius;
        h.grow(u32::MAX);
        assert_eq!(h.score,u32::MAX);
        assert!(h.radius>prior);
    }
    #[test] fn timer_and_pause(){let mut g=Game::new();g.update(0.02,Input::default());assert_eq!(g.elapsed,0.0);g.start();g.toggle_pause();g.update(0.02,Input::default());assert_eq!(g.elapsed,0.0);g.toggle_pause();g.update(0.02,Input::default());assert!(g.elapsed>0.0);}
    #[test] fn lock_large_objects(){let mut g=Game::new();g.start();let i=g.props.iter().position(|p|p.kind==Kind::House).unwrap();g.holes[0].pos=g.props[i].pos;g.update(0.01,Input::default());assert!(g.props[i].alive);}
    #[test] fn no_oob_when_moving(){let mut g=Game::new();g.start();for _ in 0..1000 {g.update(0.05,Input{movement:V::new(-1.0,-1.0),boosting:true});}assert!((0.0..=WORLD_W).contains(&g.holes[0].pos.x));}
}
