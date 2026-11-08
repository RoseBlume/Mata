//! Low-allocation educational virion renderer built on the existing mata-3d API.
use std::f32::consts::{PI,TAU};
use mata_3d::{Camera,Material,Mesh,Rasterizer,Transform,Vec3,GpuFrame};
use mata_win::Framebuffer;
use mata_molecule::virus::{VirusModel,ViralComponent};
use crate::draw::Rect;
use crate::lod::SphereLods;
use crate::render_budget::RenderPreset;
use mata_3d::parallel::ParallelSoftware;
use std::hash::{Hash,Hasher};
use std::time::Instant;

pub struct VirusViewer {raster:Rasterizer,viewport:Framebuffer,beads:SphereLods,camera_distance:f32,
    parallel:ParallelSoftware,segment_scratch:Mesh,last_key:Option<u64>,
    gpu_enabled:bool,gpu_frame:Option<GpuFrame>,gpu_rect:Option<Rect>,preset:RenderPreset}
impl VirusViewer {
    pub fn new()->Self {Self{raster:{let mut r=Rasterizer::new();r.max_distance=None;r},viewport:Framebuffer::new(0,0),
        beads:SphereLods::new(),camera_distance:1.0,parallel:ParallelSoftware::new(),
        segment_scratch:Mesh{vertices:Vec::with_capacity(16),triangles:Vec::with_capacity(16)},last_key:None,
        gpu_enabled:false,gpu_frame:None,gpu_rect:None,preset:RenderPreset::Auto}}
    pub fn set_render_preset(&mut self,preset:RenderPreset){
        if self.preset!=preset {self.preset=preset;self.invalidate();}
    }
    pub fn set_gpu_enabled(&mut self,enable:bool){
        if self.gpu_enabled!=enable{self.gpu_enabled=enable;self.invalidate();self.gpu_rect=None;}
        self.beads.set_preset(self.preset,enable);
    }
    pub fn gpu_scene(&self)->Option<(Rect,&GpuFrame)>{
        if self.gpu_enabled{self.gpu_rect.and_then(|r|self.gpu_frame.as_ref().map(|f|(r,f)))}
        else{None}
    }
    pub fn invalidate(&mut self){self.last_key=None;}
    fn render_key(virus:&VirusModel,selected:Option<ViralComponent>,rect:Rect,ax:f32,ay:f32,zoom:f32,pan:(f32,f32))->u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut h=DefaultHasher::new();
        (virus as *const VirusModel as usize).hash(&mut h);
        virus.components.len().hash(&mut h);virus.cutaway.hash(&mut h);
        virus.visible.hash(&mut h);
        for i in [rect.x,rect.y,rect.w,rect.h] {i.hash(&mut h);}
        for f in [ax,ay,zoom,pan.0,pan.1] {f.to_bits().hash(&mut h);}
        selected.hash(&mut h);
        h.finish()
    }
    pub fn draw(&mut self,frame:&mut Framebuffer,rect:Rect,virus:&VirusModel,selected:Option<ViralComponent>,ax:f32,ay:f32,zoom:f32,pan:(f32,f32)) {
        if rect.w<2||rect.h<2||rect.w>4096||rect.h>2304{self.gpu_rect=None;return;}
        let (width,height)=(rect.w as u32,rect.h as u32);
        let key=Self::render_key(virus,selected,rect,ax,ay,zoom,pan);
        if self.last_key!=Some(key) || self.viewport.width!=width || self.viewport.height!=height {
        let start=Instant::now();
        if self.viewport.width!=width||self.viewport.height!=height {self.viewport.resize_and_clear(width,height,0xff101a2a);}
        for (y,row) in self.viewport.pixels.chunks_exact_mut(width as usize).enumerate(){
            row.fill(if y<height as usize/2{0xff162a3e}else{0xff0d1c2d});
        }
        // Rendering-only scaling keeps geometry inside older mata-3d's 65-unit cutoff.
        let distance=(37.0/zoom.max(0.00001)).max(0.035);
        let scale=(52.0/(distance+16.0)).min(1.0);
        let distance=distance*scale;
        let mut camera=Camera::new(Vec3::new(-pan.0*distance/0.88,-pan.1*distance/0.88,-distance),Vec3::new(0.,0.,1.),Vec3::new(1.,0.,0.));
        camera.focal_scale=0.88;
        self.raster.fog_max=0.0;
        if self.gpu_enabled {
            self.raster.set_gpu_capture(true);
            self.raster.begin_frame(&self.viewport,camera);
        }else{
            self.parallel.begin(&mut self.raster,&self.viewport,camera);
        }
        self.camera_distance=distance;
        self.beads.configure(virus.components.len().saturating_mul(3),false);
        let rot=|p:Vec3|rotate(p,ax,ay)*scale;
        if virus.is_visible(ViralComponent::Envelope){
            let color=if selected==Some(ViralComponent::Envelope){0xfffcdd94}else{ViralComponent::Envelope.color()};
            // The cutaway omits the positive-X half of the membrane shell.
            for plane in 0..3 {for band in [8.9f32,9.35,9.8] {
                for n in 0..48 {
                    let a=n as f32*TAU/48.0;let b=(n+1) as f32*TAU/48.0;
                    let p=ellipsoid(plane,band,a);let q=ellipsoid(plane,band,b);
                    if virus.cutaway && (p.x>0.0||q.x>0.0){continue;}
                    self.segment(rot(p),rot(q),0.028*scale,color);
                }
            }}
        }
        if virus.is_visible(ViralComponent::Rna) {
            // Schematic RNA, NOT experimentally determined single-genome coordinates.
            for i in 0..210 {
                let t=i as f32/210.0;let u=(i+1) as f32/210.0;
                let coil=|f:f32| {let a=f*PI*19.0;let radius=2.4+4.0*(f*PI).sin().abs();
                    Vec3::new(radius*a.cos(),(f*2.0-1.0)*4.5,radius*a.sin())};
                self.segment(rot(coil(t)),rot(coil(u)),0.085*scale,if selected==Some(ViralComponent::Rna){0xffffe6a0}else{ViralComponent::Rna.color()});
            }
        }
        for obj in &virus.components {
            if !virus.is_visible(obj.component) {continue;}
            if virus.cutaway && obj.position.x>0.0 && matches!(obj.component,ViralComponent::Spike|ViralComponent::MembraneProtein|ViralComponent::EnvelopeProtein){continue;}
            let color=if selected==Some(obj.component){0xffffe2a0}else{obj.component.color()};
            if obj.component==ViralComponent::Spike {
                let stem=obj.position+obj.direction*2.2;
                self.segment(rot(obj.position-obj.direction*1.0),rot(stem),0.14*scale,color);
                let basis=if obj.direction.y.abs()<0.95 {Vec3::new(0.,1.,0.)}else{Vec3::new(1.,0.,0.)};
                let u=obj.direction.cross(basis).normalized();let v=obj.direction.cross(u).normalized();
                for j in 0..3 {
                    let theta=j as f32*TAU/3.0;
                    let tip=stem+u*(0.46*theta.cos())+v*(0.46*theta.sin());
                    self.ball(rot(tip),0.48*scale,color);
                }
            }else {self.ball(rot(obj.position),obj.radius*scale,color);}
        }
        if self.gpu_enabled {
            let mut next=self.raster.take_gpu_frame();
            crate::view::map_gpu_frame_to_window(&mut next,rect,frame.width,frame.height);
            if let Some(old)=self.gpu_frame.replace(next){self.raster.recycle_gpu_frame(old);}
            self.gpu_rect=Some(rect);
        }else{
            self.parallel.finish(&mut self.raster,&mut self.viewport);
            self.gpu_rect=None;
        }
        self.parallel.observe_render_ms(start.elapsed().as_secs_f32()*1000.0);
        self.last_key=Some(key);
        }
        // Clipped framebuffer copy: callers may resize the window during rendering.
        let src_x=(0i64-i64::from(rect.x)).max(0) as usize;
        let dst_x=rect.x.max(0) as usize;
        let count=(width as usize).saturating_sub(src_x).min((frame.width as usize).saturating_sub(dst_x));
        if count==0{return;}
        for y in 0..height as usize {
            let dst_y=i64::from(rect.y)+y as i64;
            if dst_y<0||dst_y>=frame.height as i64 {continue;}
            let source=y*width as usize+src_x;
            let dest=dst_y as usize*frame.width as usize+dst_x;
            frame.pixels[dest..dest+count].copy_from_slice(&self.viewport.pixels[source..source+count]);
        }
    }
    fn ball(&mut self,center:Vec3,radius:f32,color:u32) {
        let transform=Transform{translation:center,scale:Vec3::new(radius,radius,radius),rotation:Vec3::new(0.,0.,0.)};
        let height=self.viewport.height;
        if !self.raster.sphere_visible(center,radius){return;}
        if !self.raster.capture_sphere(center,radius,color) {
            self.raster.mesh(&mut self.viewport,self.beads.at(center,radius,self.camera_distance,height),transform,Material::two_sided(color,false));
        }
    }
    fn segment(&mut self,a:Vec3,b:Vec3,r:f32,color:u32) {
        let delta=b-a;if delta.len()<1e-5{return;}
        if !self.raster.capture_bond(a,b,r,color) {
            crate::view::cylinder_sides_into(&mut self.segment_scratch,a,b,r,6);
            self.raster.mesh(&mut self.viewport,&self.segment_scratch,Transform::default(),Material::two_sided(color,false));
        }
    }
    pub fn pick(virus:&VirusModel,rect:Rect,x:i32,y:i32,ax:f32,ay:f32,zoom:f32,pan:(f32,f32))->Option<ViralComponent> {
        if rect.w<2||rect.h<2{return None;}
        let raw_distance=(37.0/zoom.max(0.00001)).max(0.035);
        let scale=(52.0/(raw_distance+16.0)).min(1.0);
        let dist=raw_distance*scale;
        let mut best=None::<(f32,ViralComponent)>;
        for obj in &virus.components {
            if !virus.is_visible(obj.component){continue;}
            if virus.cutaway && obj.position.x>0.0 && matches!(obj.component,ViralComponent::Spike|ViralComponent::MembraneProtein|ViralComponent::EnvelopeProtein){continue;}
            let center=if obj.component==ViralComponent::Spike {obj.position+obj.direction*2.2}else{obj.position};
            let p=rotate(center,ax,ay)*scale;
            let depth=p.z+dist;
            if depth<0.1{continue;}
            let focal=0.88*rect.h as f32/depth;
            let px=rect.x as f32+rect.w as f32*0.5+(p.x+pan.0*dist/0.88)*focal;
            let py=rect.y as f32+rect.h as f32*0.5-(p.y+pan.1*dist/0.88)*focal;
            let d=(px-x as f32).hypot(py-y as f32);
            let size=(obj.radius.max(0.45)*scale*focal).max(7.0);
            if d<size*1.6 {let score=d/size; if best.is_none_or(|(old,_)|score<old){best=Some((score,obj.component));}}
        }
        best.map(|(_,kind)|kind)
    }
}
fn ellipsoid(plane:usize,r:f32,t:f32)->Vec3{let(a,b)=(r*t.cos(),r*t.sin());match plane{0=>Vec3::new(a,b,0.),1=>Vec3::new(a,0.,b),_=>Vec3::new(0.,a,b)}}
fn rotate(p:Vec3,ax:f32,ay:f32)->Vec3{let(s,c)=ax.sin_cos();let p=Vec3::new(p.x,p.y*c-p.z*s,p.y*s+p.z*c);let(s,c)=ay.sin_cos();Vec3::new(p.x*c+p.z*s,p.y,-p.x*s+p.z*c)}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn educational_virion_draws_nonbackground_pixels_without_window() {
        let mut viewer=VirusViewer::new();
        let mut output=Framebuffer::from_color(460,360,0xff000000);
        let model=VirusModel::sars_cov_2();
        viewer.draw(&mut output,Rect::new(0,0,460,360),&model,Some(ViralComponent::Spike),-0.2,0.4,1.0,(0.0,0.0));
        assert_eq!(output.pixels.len(),460*360);
        assert!(output.pixels.iter().any(|&c|c!=0xff162a3e && c!=0xff0d1c2d));
    }
}
