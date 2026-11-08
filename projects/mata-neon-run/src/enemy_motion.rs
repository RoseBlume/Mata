//! Deterministic, movement-driven robot locomotion.
//!
//! A cycle is advanced by ground *distance actually travelled*, not wall time:
//! blocked robots do not march in place. During stance, each foot keeps its
//! world-space location. During swing it follows a smooth raised arc toward
//! the next anticipated plant. This runs on the simulation thread (not the
//! renderer worker) so all scanline bands receive exactly the same pose.
use crate::math::V3;
use crate::world::BotKind;
use std::f32::consts::{PI,TAU};

#[derive(Clone,Copy)]
pub struct Footstep {
    pub position:V3,
    pub from:V3,
    pub to:V3,
    pub swinging:bool,
}
impl Footstep {
    fn new(x:f32,origin:V3)->Self {
        let p=origin+V3::new(x,0.0,0.12);
        Self{position:p,from:p,to:p,swinging:false}
    }
}
#[derive(Clone,Copy)]
pub struct Motion {
    pub cycle:f32,
    pub clock:f32,
    pub movement:f32,
    pub facing:f32,
    pub velocity:V3,
    pub left:Footstep,
    pub right:Footstep,
}
impl Motion {
    pub fn new(origin:V3,kind:BotKind)->Self{
        let width=hip_width(kind);
        Self{cycle:0.0,clock:0.0,movement:0.0,facing:0.0,velocity:V3::new(0.0,0.0,0.0),
            left:Footstep::new(-width,origin),right:Footstep::new(width,origin)}
    }
    /// `before` and `after` are *collision-resolved* positions.
    pub fn advance(&mut self,before:V3,after:V3,player:V3,kind:BotKind,dt:f32){
        if !dt.is_finite() || dt<=0.0{return}
        let dt=dt.min(0.10);
        self.clock+=dt;
        let delta=(after-before).horizontal();
        let distance=delta.len();
        let speed=distance/dt;
        let max_speed=match kind{BotKind::Runner=>3.4,BotKind::Striker=>2.8,
            BotKind::Tank=>1.7,BotKind::Marksman=>2.1,BotKind::Gunner=>2.2};
        let desired=(speed/max_speed).clamp(0.0,1.0);
        self.movement+=(desired-self.movement)*(1.0-(-12.0*dt).exp());
        self.movement=self.movement.clamp(0.0,1.0);
        self.velocity=if speed>0.001 {delta/dt}else{V3::new(0.0,0.0,0.0)};
        let look=(player-after).horizontal();
        if look.len()>0.001{
            let target=look.x.atan2(look.z);
            let diff=(target-self.facing+PI).rem_euclid(TAU)-PI;
            let turn_rate=if kind==BotKind::Tank{3.5}else{7.0};
            let turn_blend=1.0-(-dt*turn_rate).exp();
            self.facing=(self.facing+diff*turn_blend).rem_euclid(TAU);
        }
        // A full cycle contains two steps, one for each foot.
        self.cycle=(self.cycle+distance*TAU/(2.0*step_length(kind))).rem_euclid(TAU);
        let dir=if distance>0.0001{delta/distance}
            else{V3::new(self.facing.sin(),0.0,self.facing.cos())};
        let side=V3::new(self.facing.cos(),0.0,-self.facing.sin());
        let stride=step_length(kind);
        let width=hip_width(kind);
        Self::place_foot(&mut self.left,self.cycle,after,dir,side,-width,stride,self.movement);
        Self::place_foot(&mut self.right,self.cycle+PI,after,dir,side,width,stride,self.movement);
    }
    fn place_foot(foot:&mut Footstep,phase:f32,root:V3,direction:V3,
        right:V3,lateral:f32,stride:f32,activity:f32){
        let u=phase.rem_euclid(TAU)/TAU;
        let swinging=u>=0.60;
        if swinging&&!foot.swinging{
            foot.from=foot.position;
            foot.to=root+right*lateral+direction*(stride*0.56)+V3::new(0.0,-root.y,0.0);
        }
        if swinging {
            let t=((u-0.60)/0.40).clamp(0.0,1.0);
            let smooth=t*t*(3.0-2.0*t);
            let lift=(t*PI).sin().max(0.0)*(0.23*activity);
            foot.position=foot.from*(1.0-smooth)+foot.to*smooth+V3::new(0.0,lift,0.0);
        }else if foot.swinging {
            foot.position=foot.to;
        }
        foot.swinging=swinging;
        // Ease a stopped robot back to neutral stance if it has stopped partway
        // through a step, rather than leaving its foot suspended indefinitely.
        if activity<0.10 {
            foot.position.y*=activity*10.0;
        }
    }
}

pub fn hip_width(kind:BotKind)->f32{
    match kind{BotKind::Runner=>0.24,BotKind::Striker=>0.37,BotKind::Tank=>0.49,
        BotKind::Marksman=>0.31,BotKind::Gunner=>0.43}
}
pub fn step_length(kind:BotKind)->f32{
    match kind{BotKind::Runner=>0.95,BotKind::Striker=>0.76,BotKind::Tank=>0.56,
        BotKind::Marksman=>0.72,BotKind::Gunner=>0.65}
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn stationary_enemy_does_not_march(){
        let p=V3::new(9.0,0.0,9.0);
        let mut motion=Motion::new(p,BotKind::Runner);
        for _ in 0..120{motion.advance(p,p,V3::new(9.0,0.0,12.0),BotKind::Runner,1.0/60.0);}
        assert_eq!(motion.cycle,0.0);
        assert!(motion.movement<0.001);
        assert_eq!(motion.left.position.x,9.0-0.24);
    }
    #[test]fn planted_foot_does_not_slide_on_forward_motion(){
        let mut p=V3::new(8.0,0.0,8.0);
        let mut motion=Motion::new(p,BotKind::Runner);
        let plant=motion.left.position;
        for _ in 0..12{
            let next=p+V3::new(0.0,0.0,0.02);
            motion.advance(p,next,V3::new(8.0,0.0,16.0),BotKind::Runner,1.0/60.0);
            p=next;
        }
        assert!((motion.left.position-plant).len()<0.0001);
    }
    #[test]fn target_turn_uses_shortest_arc(){
        let p=V3::new(0.0,0.0,0.0);
        let mut motion=Motion::new(p,BotKind::Tank);
        motion.facing=TAU-0.01;
        motion.advance(p,p,V3::new(0.01,0.0,1.0),BotKind::Tank,0.016);
        assert!(motion.facing>TAU-0.1||motion.facing<0.1);
    }
}
