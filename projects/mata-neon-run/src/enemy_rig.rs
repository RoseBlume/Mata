//! High-fidelity, twelve-piece articulated OBJ rigs. The original *complete*
//! material-tagged meshes are partitioned by the generator, never decimated.
//! Transforms are evaluated per frame while parsed Models remain cached.
//! The same joint controls drive both authored quality tiers.
use crate::enemy_motion::hip_width;
use crate::math::V3;
use crate::world::{Bot,BotKind};
use mata_3d::{Model,Rasterizer,Transform,Material};
use mata_win::Framebuffer;

const BODY:usize=0;
const HEAD:usize=1;
const THIGH_L:usize=2;
const SHIN_L:usize=3;
const FOOT_L:usize=4;
const THIGH_R:usize=5;
const SHIN_R:usize=6;
const FOOT_R:usize=7;
const ARM_L:usize=8;
const FOREARM_L:usize=9;
const ARM_R:usize=10;
const FOREARM_R:usize=11;
const COUNT:usize=12;

struct EnemyRig{meshes:[Model;COUNT]}
macro_rules! rig_obj {
    ($name:literal,$suffix:literal,$part:literal)=>{
        include_str!(concat!("../assets/models/rig/enemy_",$name,$suffix,"_",$part,".obj"))
    };
}
macro_rules! rig {
    ($name:literal,$suffix:literal)=>{{
        let mat=include_str!(concat!("../assets/models/enemy_",$name,$suffix,".mtl"));
        let load=|obj:&str| Model::from_obj_mtl(obj,mat).expect("embedded rig part must load");
        EnemyRig{meshes:[
            load(rig_obj!($name,$suffix,"body")),
            load(rig_obj!($name,$suffix,"head")),
            load(rig_obj!($name,$suffix,"thigh_l")),
            load(rig_obj!($name,$suffix,"shin_l")),
            load(rig_obj!($name,$suffix,"foot_l")),
            load(rig_obj!($name,$suffix,"thigh_r")),
            load(rig_obj!($name,$suffix,"shin_r")),
            load(rig_obj!($name,$suffix,"foot_r")),
            load(rig_obj!($name,$suffix,"upper_arm_l")),
            load(rig_obj!($name,$suffix,"forearm_l")),
            load(rig_obj!($name,$suffix,"upper_arm_r")),
            load(rig_obj!($name,$suffix,"forearm_r")),
        ]}
    }};
}
// Only load the full 4K parts. Do not parse or allocate other LOD tiers
// for every render worker when no visible actor ever uses those tiers.
pub struct EnemyRigs{hero_4k:[EnemyRig;5]}
impl EnemyRigs {
    pub fn new()->Self {
        Self{hero_4k:[rig!("runner","_4k"),rig!("striker","_4k"),rig!("tank","_4k"),
            rig!("marksman","_4k"),rig!("gunner","_4k")]}
    }
    /// Called only after whole-rig view-frustum rejection in render.rs.
    pub fn draw(&self,raster:&mut Rasterizer,fb:&mut Framebuffer,bot:&Bot){
        let index=match bot.kind {
            BotKind::Runner=>0,BotKind::Striker=>1,BotKind::Tank=>2,
            BotKind::Marksman=>3,BotKind::Gunner=>4,
        };
        self.hero_4k[index].draw(raster,fb,bot);
    }
}

#[inline]fn yaw(p:V3,angle:f32)->V3{
    let (s,c)=angle.sin_cos();
    V3::new(c*p.x+s*p.z,p.y,-s*p.x+c*p.z)
}
#[inline]fn pitch(p:V3,angle:f32)->V3{
    let (s,c)=angle.sin_cos();
    V3::new(p.x,c*p.y-s*p.z,s*p.y+c*p.z)
}
#[inline]fn roll(p:V3,angle:f32)->V3{
    let (s,c)=angle.sin_cos();
    V3::new(c*p.x-s*p.y,s*p.x+c*p.y,p.z)
}
/// A parent's root at `base`, with joint pivot given in authored model space.
#[inline]fn at(base:V3,pivot:V3,heading:f32,scale:f32,pitch_angle:f32,roll:f32)->Transform{
    let mut t=Transform::at_scale(base+yaw(pivot*scale,heading),scale);
    t.rotation=V3::new(pitch_angle,heading,roll);
    t
}

impl EnemyRig{
    fn piece(&self,n:usize,raster:&mut Rasterizer,fb:&mut Framebuffer,t:Transform,
        hit:bool){
        let flash=if hit {Some(Material::two_sided(0xffe8ffff,true))}else{None};
        self.meshes[n].draw_world(raster,fb,t,flash);
    }
    fn draw(&self,raster:&mut Rasterizer,fb:&mut Framebuffer,bot:&Bot){
        let motion=&bot.motion;
        let activity=motion.movement;
        let cycle=motion.cycle;
        let (stride,c)=cycle.sin_cos();
        let size=match bot.kind{BotKind::Tank=>0.91,BotKind::Gunner=>0.98,_=>1.0};
        let heavy=matches!(bot.kind,BotKind::Tank|BotKind::Gunner);
        let speed_scale=if heavy{0.55}else{1.0};
        // Weight moves over the stance foot twice per cycle. Idle breathing is
        // deliberately subtle and does not move planted feet.
        let bob=(0.025*(1.0-(2.0*cycle).cos())*activity*speed_scale)
            +0.0035*(bot.phase+motion.clock*1.6).sin();
        let base=bot.pos+V3::new(0.0,bob,0.0);
        let heading=motion.facing;
        let flash=bot.flash>0.0;
        let sway=activity*stride*if heavy{0.018}else{0.035};
        self.piece(BODY,raster,fb,at(base,V3::new(0.0,0.0,0.0),heading,size,
            0.016*activity*c,sway),flash);
        let head_y=match bot.kind{BotKind::Tank=>2.28,BotKind::Gunner=>1.92,_=>1.86};
        let mut head=at(base,V3::new(0.0,head_y,0.0),heading,size,
            activity*0.018*(cycle+0.60).cos(),sway*0.32);
        head.rotation.y+=activity*0.03*(cycle+0.40).sin();
        self.piece(HEAD,raster,fb,head,flash);
        for (side,sign) in [(0,-1.0f32),(1,1.0)]{
            self.draw_leg(raster,fb,bot,base,heading,size,activity,
                side,sign,flash);
            self.draw_arm(raster,fb,bot,base,heading,size,activity,
                side,sign,flash);
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn draw_leg(&self,raster:&mut Rasterizer,fb:&mut Framebuffer,bot:&Bot,
        base:V3,heading:f32,scale:f32,activity:f32,side:usize,sign:f32,
        flash:bool){
        let (upper,shin,foot) = if side==0 {(THIGH_L,SHIN_L,FOOT_L)}
                                 else {(THIGH_R,SHIN_R,FOOT_R)};
        let hip_x=hip_width(bot.kind)*sign;
        let hip=V3::new(hip_x,1.05,-0.02);
        let knee_rest=V3::new(hip_x+sign*0.065,0.56,0.17);
        let ankle_rest=V3::new(hip_x+sign*0.02,0.13,0.12);
        let plant=if side==0{bot.motion.left.position}else{bot.motion.right.position};
        // Solve a 2-bone hinge in the robot's sagittal (forward/up) plane.
        // Knee bends forward while swing foot arcs clear of the floor.
        let local=yaw((plant-base)/scale,-heading);
        let ankle_goal=V3::new(local.x,local.y+0.13,local.z);
        let down=hip.y-ankle_goal.y;
        let forward=ankle_goal.z-hip.z;
        let upper_len=(knee_rest-hip).len();
        let lower_len=(ankle_rest-knee_rest).len();
        let length=(forward*forward+down*down).sqrt().clamp(
            (upper_len-lower_len).abs()+0.003,upper_len+lower_len-0.003);
        let line=forward.atan2(down);
        let cos_hip=((upper_len*upper_len+length*length-lower_len*lower_len)/
            (2.0*upper_len*length)).clamp(-1.0,1.0);
        let thigh_direction=line+cos_hip.acos();
        let lower_forward=forward-upper_len*thigh_direction.sin();
        let lower_down=down-upper_len*thigh_direction.cos();
        let shin_direction=lower_forward.atan2(lower_down);
        // Rest geometry already has a forward knee angle. Subtract the rest
        // angle or each joint will jump as soon as the first frame is drawn.
        let upper_rest=(knee_rest.z-hip.z).atan2(hip.y-knee_rest.y);
        let lower_rest=(ankle_rest.z-knee_rest.z).atan2(knee_rest.y-ankle_rest.y);
        let hip_bend=-(thigh_direction-upper_rest)*activity;
        let shin_bend=-(shin_direction-lower_rest)*activity;
        // Lateral hip articulation prevents complete sideways skating when
        // the AI strafes while keeping its head oriented toward the player.
        let lateral_error=ankle_goal.x-ankle_rest.x;
        let hip_roll=(lateral_error/(down.abs().max(0.45))).atan().clamp(-0.20,0.20)*activity;
        let thigh_t=at(base,hip,heading,scale,hip_bend,hip_roll);
        self.piece(upper,raster,fb,thigh_t,flash);
        // Forward kinematics keeps adjacent pieces joined at their hinges;
        // neither ankle nor foot is a separate disconnected bobbing object.
        let knee=hip+roll(pitch(knee_rest-hip,hip_bend),hip_roll);
        let shin_t=at(base,knee,heading,scale,shin_bend,hip_roll);
        self.piece(shin,raster,fb,shin_t,flash);
        let ankle=knee+roll(pitch(ankle_rest-knee_rest,shin_bend),hip_roll);
        let foot_pitch=((-hip_bend*0.12-shin_bend*0.13)*activity).clamp(-0.22,0.22);
        let foot_t=at(base,ankle,heading,scale,foot_pitch,0.0);
        self.piece(foot,raster,fb,foot_t,flash);
    }
    #[allow(clippy::too_many_arguments)]
    fn draw_arm(&self,raster:&mut Rasterizer,fb:&mut Framebuffer,bot:&Bot,
        base:V3,heading:f32,scale:f32,activity:f32,side:usize,sign:f32,
        flash:bool){
        let (upper,forearm)=if side==0{(ARM_L,FOREARM_L)}else{(ARM_R,FOREARM_R)};
        let arm_x=match bot.kind{BotKind::Runner=>0.46,BotKind::Striker=>0.62,
            BotKind::Tank=>1.02,BotKind::Marksman=>0.52,BotKind::Gunner=>0.73};
        let sy=if bot.kind==BotKind::Tank{1.95}else{1.48};
        let elbow_y=if bot.kind==BotKind::Tank{1.30}else{0.99};
        let shoulder=V3::new(sign*arm_x,sy,0.02);
        let elbow_rest=V3::new(sign*(arm_x+0.06),elbow_y,0.13);
        let amplitude=match bot.kind{
            BotKind::Runner=>0.44,BotKind::Striker=>0.33,BotKind::Tank=>0.13,
            BotKind::Marksman=>0.19,BotKind::Gunner=>0.14
        };
        let side_phase=if side==0{bot.motion.cycle}else{bot.motion.cycle+std::f32::consts::PI};
        let shoulder_pitch=activity*amplitude*side_phase.sin();
        let arm_roll=activity*sign*0.035*(side_phase.cos().abs());
        let upper_t=at(base,shoulder,heading,scale,shoulder_pitch,arm_roll);
        self.piece(upper,raster,fb,upper_t,flash);
        let elbow=shoulder+pitch(elbow_rest-shoulder,shoulder_pitch);
        let elbow_flex=(0.13+0.28*side_phase.sin().max(0.0))*activity;
        let forearm_t=at(base,elbow,heading,scale,shoulder_pitch-elbow_flex,arm_roll*0.30);
        self.piece(forearm,raster,fb,forearm_t,flash);
    }
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn all_five_4k_rigs_preserve_full_quality(){
        let rigs=EnemyRigs::new();
        for (i,rig) in rigs.hero_4k.iter().enumerate(){
            let tris=rig.meshes.iter().map(|mesh|mesh.triangle_count() as usize).sum::<usize>();
            assert!(tris>30_000,"character {i} must use full 4K geometry");
            for part in &rig.meshes {assert!(part.triangle_count()>0)}
        }
    }
}
