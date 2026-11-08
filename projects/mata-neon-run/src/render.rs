//! Neon Run's *scene composition* only. All geometry transformation, indexed
//! mesh rendering, near-plane clipping, shading, rasterization, and Z-buffer
//! management live in the standalone `mata-3d` crate.
use mata_win::Framebuffer;
use mata_3d::{Rasterizer,Camera,Material,Mesh,Transform,Model};
use crate::{math::{V3,Box3},world::{World,PickupKind,ARENA}};
use crate::hud;
use crate::environment::Environment;
use crate::enemy_rig::EnemyRigs;
use crate::visibility::ViewFrustum;
use crate::weapons::Gun;

pub struct Renderer {
    env:Environment,
    raster:Rasterizer,
    octa_mesh:Mesh,
    reactor_mesh:Mesh,
    enemies:EnemyRigs,
    pulse_carbine:Model,
    arc_cannon:Model,
    sniper:Model,
    shotgun:Model,
    minigun:Model,
}
impl Renderer {
    pub fn new()->Self {
        Self{
            env:Environment::new(),
            raster:Rasterizer::new(),
            octa_mesh:Mesh::octahedron(),
            reactor_mesh:Mesh::from_obj(include_str!("../assets/models/reactor.obj"))
                .expect("embedded reactor model must be valid OBJ geometry"),
            enemies:EnemyRigs::new(),
            sniper:Model::from_obj_mtl(
                include_str!("../assets/models/gun_sniper.obj"),
                include_str!("../assets/models/gun_sniper.mtl")).expect("sniper model"),
            shotgun:Model::from_obj_mtl(
                include_str!("../assets/models/gun_shotgun.obj"),
                include_str!("../assets/models/gun_shotgun.mtl")).expect("shotgun model"),
            minigun:Model::from_obj_mtl(
                include_str!("../assets/models/gun_minigun.obj"),
                include_str!("../assets/models/gun_minigun.mtl")).expect("minigun model"),
            pulse_carbine:Model::from_obj_mtl(
                include_str!("../assets/models/gun_pulse_carbine.obj"),
                include_str!("../assets/models/gun_pulse_carbine.mtl"))
                .expect("pulse carbine model must be valid"),
            arc_cannon:Model::from_obj_mtl(
                include_str!("../assets/models/gun_arc_cannon.obj"),
                include_str!("../assets/models/gun_arc_cannon.mtl"))
                .expect("arc cannon model must be valid"),
        }
    }
    /// Paint the original sky gradient in any screen-space band, without
    /// loading level models. Called by lightweight software raster workers.
    pub(crate) fn paint_sky(fb:&mut Framebuffer,full_height:usize,first_row:usize){
        let width=fb.width as usize;
        if width==0||full_height==0{return;}
        for (y,row) in fb.pixels.chunks_exact_mut(width).enumerate(){
            let t=(y+first_row) as f32/full_height as f32;
            let (r,g,b)=if t<0.53{
                (5.0+25.0*t,13.0+43.0*t,30.0+62.0*t)
            }else{(6.0+5.0*t,20.0+8.0*t,31.0+6.0*t)};
            row.fill(rgb(r as u32,g as u32,b as u32));
        }
    }
    pub fn render_scene(&mut self,fb:&mut Framebuffer,world:&World){
        self.render_scene_band(fb,world,fb.height as usize,0);
    }
    /// Render a disjoint horizontal band, with perspective coordinates
    /// computed in full-window space (used by the adaptive worker pool).
    pub fn render_scene_band(&mut self,fb:&mut Framebuffer,world:&World,
        full_height:usize,first_row:usize){
        let width=fb.width as usize;
        let height=fb.height as usize;
        if width==0||height==0{return}
        let mut camera=Camera::new(world.eye(),world.forward(),world.right());
        // Optic FOV is interpolated every fixed update, not snapped instantly.
        camera.focal_scale=1.0+2.4*world.aim_ease();
        let scene_focal_scale=camera.focal_scale;
        self.raster.begin_frame_band(fb,camera,full_height,first_row);
        // Compute the same sky rows as the whole scene, irrespective of band.
        // The GPU geometry path starts with a transparent target and generates
        // the sky in HLSL. Skipping these full-screen writes saves bandwidth
        // on shared-memory Intel iGPUs. Software fallback still fills every row.
        if !self.raster.gpu_capture_enabled() && !self.raster.software_capture_enabled(){
            Self::paint_sky(fb,full_height,first_row);
        }
        // A real horizontal world-space plane, then elevated emissive grid-lines.
        self.quad(fb,V3::new(0.0,-0.045,0.0),V3::new(ARENA,-0.045,0.0),V3::new(ARENA,-0.045,ARENA),V3::new(0.0,-0.045,ARENA),0xff162537,false);
        let grid=match world.stage{0=>0xff134d60,1=>0xff76522c,2=>0xff5d427e,
            3=>0xff276b7c,_=>0xff763b68};
        for i in 0..=18{
            let x=i as f32*2.0;
            self.quad(fb,V3::new(x-0.016,-0.038,0.0),V3::new(x+0.016,-0.038,0.0),V3::new(x+0.016,-0.038,ARENA),V3::new(x-0.016,-0.038,ARENA),grid,true);
            let z=i as f32*2.0;
            self.quad(fb,V3::new(0.0,-0.037,z-0.016),V3::new(ARENA,-0.037,z-0.016),V3::new(ARENA,-0.037,z+0.016),V3::new(0.0,-0.037,z+0.016),grid,true);
        }
        // Conservative hemisphere culling avoids rasterizing walls that lie
        // entirely behind the player. The bounding sphere includes the full
        // wall length and height; visible wall edges are never culled.
        let camera_eye=world.eye();
        let view_forward=world.forward();
        // Environment geometry. Height and perspective vary continuously with camera position.
        for b in &world.boxes {
            let center=(b.min+b.max)*0.5;
            let half=(b.max-b.min)*0.5;
            if (center-camera_eye).dot(view_forward) < -half.len()-1.5 {continue}
            let col=if b.max.y>4.0 {
                match world.stage{0=>0xff273c4d,1=>0xff453c32,2=>0xff303447,
                    3=>0xff264451,_=>0xff43314f}
            }else{0xff425468};
            self.cube(fb,*b,col,false);
            if b.max.y<2.0 {
                self.cube(fb,Box3::new(b.min.x,b.max.y-0.15,b.min.z,b.max.x-b.min.x,0.11,b.max.z-b.min.z),
                    match world.stage{0=>0xff00bfc6,1=>0xffffb169,2=>0xffaa87ff,
                        3=>0xff7adaed,_=>0xffff7aa6},true);
            }
        }
        self.env.draw(fb,world,&mut self.raster);

        // Archivist core: separate physical shield system and target warning.
        if world.stage==4 && world.boss_hp>0.0 {
            let p=world.boss_pos;
            self.cube(fb,Box3::new(p.x-1.15,0.0,p.z-1.15,2.3,0.55,2.3),0xff303448,false);
            let mut pose=Transform::at_scale(p+V3::new(0.0,1.60,0.0),1.80);
            pose.rotation.y=world.time*0.95;
            self.raster.mesh(fb,&self.reactor_mesh,pose,Material::glow(
                if world.boss_flash>0.0{0xffffffff}
                else if world.relays_done<3{0xff8b9dff}else{0xffff6d9c}));
            if world.relays_done<3{
                for (ox,oz) in [(-1.55,-1.55),(1.55,-1.55),(-1.55,1.55),(1.55,1.55)]{
                    self.cube(fb,Box3::new(p.x+ox-0.12,0.25,p.z+oz-0.12,0.24,3.1,0.24),0xff7a91ef,true);
                }
            }
            if let Some(target)=world.boss_target{
                self.cube(fb,Box3::new(target.x-1.8,0.035,target.z-0.045,3.6,0.06,0.09),0xffff6574,true);
                self.cube(fb,Box3::new(target.x-0.045,0.035,target.z-1.8,0.09,0.06,3.6),0xffff6574,true);
            }
            if world.boss_beam>0.0{
                let q=world.boss_beam_pos;
                self.cube(fb,Box3::new(q.x-0.24,0.0,q.z-0.24,0.48,4.4,0.48),0xffffc4df,true);
            }
        }
        // A spinning neon reactor demonstrates the general indexed OBJ mesh
        // pipeline independently of Neon Run's procedural cuboids. This is
        // purely decorative, high above the arena floor, with no new physics.
        let mut reactor=Transform::at_scale(V3::new(18.0,4.1,18.0),0.60);
        reactor.rotation=V3::new(0.25,world.time*0.7,0.12);
        self.raster.mesh(fb,&self.reactor_mesh,reactor,Material::glow(
            match world.stage {0=>0xff42e8ed,1=>0xffffb369,2=>0xffb899ff,
                3=>0xff6ce9fc,_=>0xffff7ac4}
        ));
        // No enemy LOD and no hero budget: every visible enemy uses its
        // original high-poly 4K articulated model, regardless of distance,
        // framebuffer scale, or a crowded camera view.
        // Culling must happen BEFORE computing its walking rig and before
        // handing any of its triangles to mata-3d. No arbitrary distance cut.
        // Keep the test conservative so partial limbs never disappear.
        let frustum=ViewFrustum::new(camera_eye,view_forward,world.right(),
            width,full_height,scene_focal_scale);
        for e in &world.bots {
            if !frustum.sees_enemy(e.pos){continue}
            self.enemies.draw(&mut self.raster,fb,e);
        }
        for p in &world.pickups {
            let y=0.74+0.16*(world.time*3.5+p.phase).sin();
            let color=match p.kind{PickupKind::Health=>0xff65ffa1,PickupKind::Shield=>0xff56dfff,
                PickupKind::Ammo(gun,_)=>match gun {
                    Gun::Carbine=>0xff5ce8ec,Gun::Cannon=>0xffe77bff,
                    Gun::Sniper=>0xff91dcff,Gun::Shotgun=>0xffffac5c,
                    Gun::Minigun=>0xffffdc79,
                }};
            let mut model=Transform::at_scale(p.pos+V3::new(0.0,y,0.0),0.49);
            model.rotation.y=world.time*0.95+p.phase;
            self.raster.mesh(fb,&self.octa_mesh,model,Material::two_sided(color,true));
            self.cube(fb,Box3::new(p.pos.x-0.22,0.02,p.pos.z-0.22,0.44,0.045,0.44),color,true);
        }
        for b in &world.shots{
            self.octahedron(fb,b.pos,0.23,b.color);
        }
        for s in &world.sparks{
            let r=(s.life*0.12).max(0.018);
            self.cube(fb,Box3::new(s.pos.x-r,s.pos.y-r,s.pos.z-r,r*2.0,r*2.0,r*2.0),s.color,true);
        }
        self.draw_weapon(fb,world);
    }
    /// Transform, shade and clip full-quality geometry once for all software
    /// raster bands. No scene pixels are touched in this capture pass.
    pub fn render_software_capture(&mut self,fb:&mut Framebuffer,world:&World)
        ->mata_3d::SoftwareFrame{
        self.raster.set_software_capture(true);
        self.render_scene(fb,world);
        self.raster.take_software_frame()
    }
    pub fn recycle_software_frame(&mut self,frame:mata_3d::SoftwareFrame){
        self.raster.recycle_software_frame(frame);
    }
    /// CPU command encoding with GPU rasterization: no framebuffer triangles
    /// are written here. Captures full-quality meshes at native resolution.
    pub fn render_gpu(&mut self,fb:&mut Framebuffer,world:&World)->mata_3d::GpuFrame{
        self.raster.set_gpu_capture(true);
        self.render_scene(fb,world);
        self.raster.take_gpu_frame()
    }
    pub fn recycle_gpu_frame(&mut self,frame:mata_3d::GpuFrame){
        self.raster.recycle_gpu_frame(frame);
    }
    pub fn stats(&self)->(u32,u32){(self.raster.stats.triangles,self.raster.stats.pixels)}
    pub fn draw_hud(&self,fb:&mut Framebuffer,world:&World,fps:u32,scale_percent:u32,has_audio:bool,muted:bool,threads:usize,max_threads:usize,target:u32){
        hud::draw(fb,world,self.raster.stats.triangles,self.raster.stats.pixels,fps,scale_percent,has_audio,muted,threads,max_threads,target,"CPU SOFTWARE");
    }
    fn draw_weapon(&mut self,fb:&mut Framebuffer,w:&World){
        // Camera-space first person models. Every gun performs a multi-phase
        // reload pose and an independent magazine/energy cell movement.
        let gun=w.gun();
        let aim=if gun==Gun::Sniper{w.aim_ease()}else{0.0};
        // Let the physical optic align before the scope glass fills the view.
        // Once aligned, the HUD's reticle is drawn over the world instead.
        if gun==Gun::Sniper && aim>=0.96 {return}
        self.raster.clear_depth();
        let bob=(w.walk*1.35).sin()*0.023;
        let recoil=if w.muzzle>0.0 {w.muzzle*1.3}else{0.0};
        let phase=w.reload_progress();
        let reload_drop=if w.reload>0.0 {
            if phase<0.28 {phase/0.28}
            else if phase>0.75 {((1.0-phase)/0.25).max(0.0)}
            else {1.0}
        } else {0.0};
        let base_x=if gun==Gun::Sniper{0.30}else if gun==Gun::Minigun{0.33}else{0.40};
        let mut pose=Transform::at_scale(
            V3::new(base_x+bob*0.3+reload_drop*0.10-(base_x+0.01)*aim,
                    -0.49+bob-reload_drop*0.32+0.135*aim,
                    0.89-recoil-reload_drop*0.17-0.33*aim),1.0);
        let (reload_pitch,reload_roll)=match gun {
            Gun::Carbine=>(0.65,0.38),Gun::Cannon=>(0.85,-0.47),
            Gun::Sniper=>(0.45,0.70),Gun::Shotgun=>(0.80,-0.36),
            Gun::Minigun=>(0.48,0.60),
        };
        pose.rotation=V3::new(-0.055-recoil*0.50+reload_drop*reload_pitch+0.055*aim,
            (0.015+bob*0.4)*(1.0-aim),
            (-0.015+bob*0.3)*(1.0-aim)+reload_drop*reload_roll);
        let weapon=match gun{
            Gun::Carbine=>&self.pulse_carbine,Gun::Cannon=>&self.arc_cannon,
            Gun::Sniper=>&self.sniper,Gun::Shotgun=>&self.shotgun,Gun::Minigun=>&self.minigun,
        };
        weapon.draw_view(&mut self.raster,fb,pose);
        // Dedicated animations per magazine type, with a physical animated
        // battery/clip at reload midpoint, and a moving mechanical part.
        if w.reload>0.0 {
            let slide=if phase<0.45 {phase/0.45}else{((1.0-phase)/0.55).max(0.0)};
            let magazine_color=match gun{
                Gun::Carbine=>0xff59e2ef,Gun::Cannon=>0xffd888ff,
                Gun::Sniper=>0xff89cde9,Gun::Shotgun=>0xffffa660,
                Gun::Minigun=>0xffffcb60,
            };
            let mag_x=match gun{Gun::Cannon=>0.18,Gun::Minigun=>-0.16,_=>-0.11};
            let mag_size=match gun{Gun::Minigun=>0.27,Gun::Shotgun=>0.18,_=>0.13};
            self.cube_view(fb,Box3::new(
                pose.translation.x+mag_x,pose.translation.y-0.19-slide*0.28,
                pose.translation.z+0.08,mag_size,0.17+mag_size*0.6,0.23),magazine_color,true);
            if gun==Gun::Sniper {
                // Bolt handle pulled back then pushed forward.
                self.cube_view(fb,Box3::new(pose.translation.x+0.04,
                    pose.translation.y+0.20,pose.translation.z+0.20-slide*0.24,
                    0.055,0.06,0.16),0xffaadfe7,false);
            }
            if gun==Gun::Shotgun {
                // Sliding forward handguard (pump action).
                self.cube_view(fb,Box3::new(pose.translation.x-0.14,
                    pose.translation.y-0.17,pose.translation.z+0.49-slide*0.27,
                    0.28,0.13,0.25),0xffb7723c,false);
            }
        }
        if w.muzzle>0.0 {
            let flash=match gun{Gun::Carbine=>0xffbafaff,Gun::Cannon=>0xffffccf5,
                Gun::Sniper=>0xffc9ffff,Gun::Shotgun=>0xffffdca8,Gun::Minigun=>0xffffea8f};
            let tip=match gun{Gun::Sniper=>1.97,Gun::Minigun=>1.36,
                Gun::Shotgun=>1.15,_=>1.20};
            let emitters:&[f32]=match gun{Gun::Cannon|Gun::Shotgun=>&[-0.19,0.19],_=>&[0.0]};
            for &x in emitters {
                self.octa_view(fb,pose.point(V3::new(x,0.03,tip)),
                    if gun==Gun::Shotgun{0.22}else{0.12},flash);
            }
        }
        if gun==Gun::Minigun && w.weapon_spin>0.0 {
            // Rotating emissive tips: visible even on low internal resolutions.
            for index in 0..6 {
                let theta=w.time*24.0+index as f32*std::f32::consts::TAU/6.0;
                let x=theta.cos()*0.18;let y=theta.sin()*0.18;
                self.octa_view(fb,pose.point(V3::new(x,y+0.02,1.37)),0.054,0xffffd978);
            }
        }
    }
    fn octahedron(&mut self,fb:&mut Framebuffer,center:V3,r:f32,color:u32){
        self.raster.mesh(fb,&self.octa_mesh,Transform::at_scale(center,r),
            Material::two_sided(color,true));
    }
    fn octa_view(&mut self,fb:&mut Framebuffer,c:V3,r:f32,color:u32){
        self.raster.octa_view(fb,c,r,color);
    }
    fn cube(&mut self,fb:&mut Framebuffer,b:Box3,color:u32,glow:bool){
        self.raster.box3(fb,b,if glow{Material::glow(color)}else{Material::lit(color)});
    }
    fn cube_view(&mut self,fb:&mut Framebuffer,b:Box3,color:u32,glow:bool){
        self.raster.box_view(fb,b,if glow{Material::glow(color)}else{Material::lit(color)});
    }
    fn quad(&mut self,fb:&mut Framebuffer,a:V3,b:V3,c:V3,d:V3,color:u32,glow:bool){
        self.raster.quad(fb,a,b,c,d,Material::two_sided(color,glow));
    }
}
#[inline] fn rgb(r:u32,g:u32,b:u32)->u32 {
    0xff000000|(r.min(255)<<16)|(g.min(255)<<8)|b.min(255)
}

#[cfg(test)]
mod model_tests {
    use super::*;
    #[test]
    fn embedded_weapon_and_enemy_assets_load(){
        let r=Renderer::new();
        for model in [&r.pulse_carbine,&r.arc_cannon,&r.sniper,&r.shotgun,&r.minigun]{
            assert!(model.triangle_count()>=100);
            assert!(model.parts.len()>=5);
            assert!(model.parts.iter().any(|p|p.material.emissive));
        }
        assert_ne!(r.pulse_carbine.triangle_count(),r.arc_cannon.triangle_count());
        // enemy rig geometry is tested in enemy_rig::tests, including
        // full-resolution triangle preservation for all five classes.
    }

}
