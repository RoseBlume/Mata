//! CPU-rendered cell exploration using only mata-3d, mata_win and std.
use std::f32::consts::{PI,TAU};
use mata_3d::{Camera,Material,Mesh,Rasterizer,Transform,Vec3,GpuFrame};
use mata_win::Framebuffer;
use mata_molecule::cell::{CellData,GeneratedCell,OrganelleKind,Volume};
use crate::draw::Rect;
use crate::lod::SphereLods;
use crate::render_budget::RenderPreset;
use mata_3d::parallel::ParallelSoftware;
use std::hash::{Hash,Hasher};
use std::time::Instant;

#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash)]
pub enum VolumeMode { Surface, MaxIntensity, Xy, Xz, Yz }
impl VolumeMode {pub fn label(self)->&'static str {match self{Self::Surface=>"3D SURFACE",Self::MaxIntensity=>"3D MIP",Self::Xy=>"XY SLICE",Self::Xz=>"XZ SLICE",Self::Yz=>"YZ SLICE"}}}

pub struct CellViewer {pub raster:Rasterizer,viewport:Framebuffer,lods:SphereLods,
    parallel:ParallelSoftware,segment_scratch:Mesh,last_key:Option<u64>,
    gpu_enabled:bool,gpu_frame:Option<GpuFrame>,gpu_rect:Option<Rect>,preset:RenderPreset}
impl CellViewer {
    pub fn new()->Self {Self{raster:{let mut r=Rasterizer::new();r.max_distance=None;r},
        viewport:Framebuffer::new(0,0),lods:SphereLods::new(),parallel:ParallelSoftware::new(),
        segment_scratch:Mesh{vertices:Vec::with_capacity(16),triangles:Vec::with_capacity(16)},last_key:None,
        gpu_enabled:false,gpu_frame:None,gpu_rect:None,preset:RenderPreset::Auto}}
    pub fn invalidate(&mut self){self.last_key=None;}
    pub fn set_render_preset(&mut self,preset:RenderPreset){
        if self.preset!=preset {self.preset=preset;self.invalidate();}
    }
    pub fn set_gpu_enabled(&mut self,enable:bool){
        if self.gpu_enabled!=enable{self.gpu_enabled=enable;self.invalidate();self.gpu_rect=None;}
        self.lods.set_preset(self.preset,enable);
    }
    pub fn gpu_scene(&self)->Option<(Rect,&GpuFrame)>{
        if self.gpu_enabled {self.gpu_rect.and_then(|r|self.gpu_frame.as_ref().map(|f|(r,f)))}
        else{None}
    }
    fn render_key(data:&CellData,rect:Rect,ax:f32,ay:f32,zoom:f32,pan:(f32,f32),selected:Option<usize>,mode:VolumeMode,slice:f32,gain:f32)->u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut h=DefaultHasher::new();
        (data as *const CellData as usize).hash(&mut h);
        for i in [rect.x,rect.y,rect.w,rect.h] {i.hash(&mut h);}
        for f in [ax,ay,zoom,pan.0,pan.1,slice,gain] {f.to_bits().hash(&mut h);}
        selected.hash(&mut h);mode.hash(&mut h);
        match data {
            CellData::Generated(c)=>{c.organelles.len().hash(&mut h);
                for o in &c.organelles {o.visible.hash(&mut h);}
            },
            CellData::Microscopy(v)=>{v.dims.hash(&mut h);}
        }
        h.finish()
    }
    fn start(&mut self,r:Rect)->bool {
        let (w,h)=(r.w.max(1) as u32,r.h.max(1) as u32);
        if w>4096 || h>2304{return false;}
        if self.viewport.width!=w||self.viewport.height!=h{self.viewport.resize_and_clear(w,h,0xff101b2b);}
        for y in 0..h as usize {
            let color=if y<h as usize/2{0xff13293a}else{0xff101f30};
            self.viewport.pixels[y*w as usize..(y+1)*w as usize].fill(color);
        }
        true
    }
    pub fn draw(&mut self,fb:&mut Framebuffer,rect:Rect,data:&CellData,ax:f32,ay:f32,zoom:f32,pan:(f32,f32),selected:Option<usize>,mode:VolumeMode,slice:f32,gain:f32) {
        let key=Self::render_key(data,rect,ax,ay,zoom,pan,selected,mode,slice,gain);
        let cached=self.last_key==Some(key) && self.viewport.width==rect.w.max(1) as u32
            && self.viewport.height==rect.h.max(1) as u32;
        if !cached {
            if !self.start(rect){self.gpu_rect=None;return;}
            let start=Instant::now();
            match data {
            CellData::Generated(c)=>self.generated(c,ax,ay,zoom,pan,selected),
            CellData::Microscopy(v)=>self.microscopy(v,ax,ay,zoom,pan,mode,slice,gain),
            }
            if self.gpu_enabled && matches!(data,CellData::Generated(_)) {
                let mut next=self.raster.take_gpu_frame();
                crate::view::map_gpu_frame_to_window(&mut next,rect,fb.width,fb.height);
                if let Some(old)=self.gpu_frame.replace(next){self.raster.recycle_gpu_frame(old);}
                self.gpu_rect=Some(rect);
            }else{
                self.parallel.finish(&mut self.raster,&mut self.viewport);
                self.gpu_rect=None;
            }
            if matches!(data,CellData::Generated(_)) {
                self.parallel.observe_render_ms(start.elapsed().as_secs_f32()*1000.0);
            }
            self.last_key=Some(key);
        }
        // Blit fully clipped, so very small windows cannot panic.
        let src_x=(0i64-i64::from(rect.x)).max(0) as usize;
        let dest_x=rect.x.max(0) as usize;
        let width=(self.viewport.width as usize).saturating_sub(src_x).min((fb.width as usize).saturating_sub(dest_x));
        if width==0{return;}
        for y in 0..self.viewport.height as usize {
            let dst_y=i64::from(rect.y)+y as i64;
            if dst_y<0||dst_y>=i64::from(fb.height){continue;}
            let src=y*self.viewport.width as usize+src_x;
            let dst=dst_y as usize*fb.width as usize+dest_x;
            fb.pixels[dst..dst+width].copy_from_slice(&self.viewport.pixels[src..src+width]);
        }
    }
    fn generated(&mut self,cell:&GeneratedCell,ax:f32,ay:f32,zoom:f32,pan:(f32,f32),selected:Option<usize>) {
        let extent=cell.size_um.x.max(cell.size_um.y).max(cell.size_um.z)*0.7;
        let z=zoom.max(0.00001);
        let distance=(extent*3.0/z).max(0.03);
        // Fit into older mata-3d's hardcoded triangle-distance budget.
        let scale=(52.0/(distance+extent+2.0)).min(1.0);
        let distance=distance*scale;
        let mut camera=Camera::new(Vec3::new(-pan.0*distance/0.88,-pan.1*distance/0.88,-distance),Vec3::new(0.0,0.0,1.0),Vec3::new(1.0,0.0,0.0));
        camera.focal_scale=0.88;
        self.raster.fog_max=0.0;
        if self.gpu_enabled {
            self.raster.set_gpu_capture(true);
            self.raster.begin_frame(&self.viewport,camera);
        }else{
            self.parallel.begin(&mut self.raster,&self.viewport,camera);
        }
        self.lods.configure(cell.organelles.len(),false);
        // A sparse cage depicts transparent cell boundaries without hiding internal organelles.
        for (i,o) in cell.organelles.iter().enumerate(){
            if !o.visible{continue;}
            if matches!(o.kind,OrganelleKind::Membrane|OrganelleKind::CellWall){
                for plane in 0..3 {
                    let segments=52;
                    for s in 0..segments {
                        let a=s as f32*TAU/segments as f32;
                        let b=(s+1) as f32*TAU/segments as f32;
                        let p=on_ellipse(o.center,o.radii,plane,a);
                        let q=on_ellipse(o.center,o.radii,plane,b);
                        let color=if selected==Some(i){0xffffd68c}else{o.kind.color()};
                        self.segment(rotate(p,ax,ay)*scale,rotate(q,ax,ay)*scale,0.025*scale,color);
                    }
                }
            }
        }
        for (i,o) in cell.organelles.iter().enumerate(){
            if !o.visible || matches!(o.kind,OrganelleKind::Membrane|OrganelleKind::CellWall){continue;}
            let p=rotate(o.center,ax,ay)*scale;
            let factor=if selected==Some(i){1.10}else{1.0};
            let t=Transform{translation:p,scale:o.radii* (scale*factor),rotation:Vec3::new(ax,ay,0.0)};
            let atom_radius=o.radii.x.max(o.radii.y).max(o.radii.z)*scale*factor;
            let height=self.viewport.height;
            if !self.raster.sphere_visible(p,atom_radius){continue;}
            self.raster.mesh(&mut self.viewport,self.lods.at(p,atom_radius,distance,height),t,Material::two_sided(if selected==Some(i){0xffffdf83}else{o.kind.color()},false));
            if selected==Some(i) {
                // Outline in the object's principal plane.
                for s in 0..42 {
                    let a=s as f32*TAU/42.0;let b=(s+1) as f32*TAU/42.0;
                    let pa=Vec3::new(o.center.x+o.radii.x*1.11*a.cos(),o.center.y+o.radii.y*1.11*a.sin(),o.center.z);
                    let pb=Vec3::new(o.center.x+o.radii.x*1.11*b.cos(),o.center.y+o.radii.y*1.11*b.sin(),o.center.z);
                    self.segment(rotate(pa,ax,ay)*scale,rotate(pb,ax,ay)*scale,0.040*scale,0xffffffb5);
                }
            }
        }
    }
    fn segment(&mut self,a:Vec3,b:Vec3,r:f32,color:u32) {
        let d=b-a;if d.len()<0.00001{return;}
        if !self.raster.capture_bond(a,b,r,color) {
            crate::view::cylinder_sides_into(&mut self.segment_scratch,a,b,r,6);
            self.raster.mesh(&mut self.viewport,&self.segment_scratch,Transform::default(),Material::two_sided(color,false));
        }
    }
    /// Approximate screen-space picking for illustrative organelles.  Source
    /// microscopy has no organelle labels, so it uses voxel inspection instead.
    pub fn pick_organelle(cell:&GeneratedCell,rect:Rect,x:i32,y:i32,ax:f32,ay:f32,zoom:f32,pan:(f32,f32))->Option<usize> {
        if rect.w<=0 || rect.h<=0{return None;}
        let extent=cell.size_um.x.max(cell.size_um.y).max(cell.size_um.z)*0.7;
        let distance=(extent*3.0/zoom.max(0.00001)).max(0.03);
        let scale=(52.0/(distance+extent+2.0)).min(1.0);
        let distance=distance*scale;
        let offset=Vec3::new(-pan.0*distance/0.88,-pan.1*distance/0.88,-distance);
        let mut candidates=Vec::<(usize,f32,f32)>::new();
        for (i,o) in cell.organelles.iter().enumerate() {
            if !o.visible || matches!(o.kind,OrganelleKind::Membrane|OrganelleKind::CellWall){continue;}
            let p=rotate(o.center,ax,ay)*scale-offset;
            if p.z<=0.075{continue;}
            let focal=0.88*rect.h as f32/p.z;
            let sx=rect.x as f32+rect.w as f32*0.5+p.x*focal;
            let sy=rect.y as f32+rect.h as f32*0.5-p.y*focal;
            let radius=o.radii.x.max(o.radii.y).max(o.radii.z)*scale*focal;
            let hit=(sx-x as f32).hypot(sy-y as f32)/radius.max(1.0);
            if hit<1.0 {candidates.push((i,hit, radius));}
        }
        // Prefer more specifically localized structures over giant enclosing organelles.
        candidates.into_iter().min_by(|a,b|a.2.total_cmp(&b.2).then(a.1.total_cmp(&b.1))).map(|x|x.0)
    }

    /// Read a selected voxel in source coordinates. MIP reports the peak sample
    /// along the picked ray; other modes report the selected section's voxel.
    pub fn inspect_voxel(vol:&Volume,r:Rect,x:i32,y:i32,mode:VolumeMode,slice:f32,ax:f32,ay:f32,zoom:f32,pan:(f32,f32))->Option<([usize;3],f32)> {
        if r.w<=0||r.h<=0{return None;}
        let [dx,dy,dz]=vol.dims;
        let physical=[dx as f32*vol.spacing_um[0],dy as f32*vol.spacing_um[1],dz as f32*vol.spacing_um[2]];
        let largest=physical.into_iter().fold(1e-6f32,f32::max);
        let normal=[physical[0]/largest,physical[1]/largest,physical[2]/largest];
        let aspect=r.w as f32/r.h as f32;
        let u=((((x-r.x) as f32+0.5)/r.w as f32)*2.0-1.0)*aspect/zoom.max(0.00001)+pan.0*2.0;
        let v=(1.0-((y-r.y) as f32+0.5)/r.h as f32*2.0)/zoom.max(0.00001)+pan.1*2.0;
        let unit=slice.clamp(0.,1.)*2.-1.;
        let ray=if matches!(mode,VolumeMode::MaxIntensity|VolumeMode::Surface) {
            let o=unrotate(Vec3::new(u,v,-2.),ax,ay);let d=unrotate(Vec3::new(0.,0.,1.),ax,ay);
            ray_box(o,d,normal).map(|(a,b)|(o,d,a,b))
        }else{None};
        let count=if matches!(mode,VolumeMode::MaxIntensity|VolumeMode::Surface) {128}else{1};
        let mut best:Option<([usize;3],f32)>=None;
        for s in 0..count {
            let p=match mode {
                VolumeMode::Surface|VolumeMode::MaxIntensity=>{let (o,d,a,b)=ray?;o+d*(a+(s as f32+0.5)*(b-a)/count as f32)},
                VolumeMode::Xy=>Vec3::new(u,v,unit*normal[2]),
                VolumeMode::Xz=>Vec3::new(u,unit*normal[1],v),
                VolumeMode::Yz=>Vec3::new(unit*normal[0],u,v),
            };
            if p.x.abs()>normal[0]||p.y.abs()>normal[1]||p.z.abs()>normal[2]{continue;}
            let ix=(((p.x/normal[0]*0.5+0.5)*dx as f32) as usize).min(dx-1);
            let iy=(((p.y/normal[1]*0.5+0.5)*dy as f32) as usize).min(dy-1);
            let iz=(((p.z/normal[2]*0.5+0.5)*dz as f32) as usize).min(dz-1);
            let value=vol.value(ix,iy,iz);
            if best.as_ref().is_none_or(|(_,previous)|value>*previous){best=Some(([ix,iy,iz],value));}
        }
        best
    }
    fn microscopy(&mut self,vol:&Volume,ax:f32,ay:f32,zoom:f32,pan:(f32,f32),mode:VolumeMode,slice:f32,gain:f32) {
        let w=self.viewport.width as usize;let h=self.viewport.height as usize;
        let aspect=w as f32/h.max(1) as f32;
        let [dx,dy,dz]=vol.dims;
        // Viewing modes use three orthogonal axes; 3D MIP applies both rotations.
        let (low,high)=vol.min_max();let inv=1.0/(high-low).max(1e-12);
        // Screen sampling and ray probes are visualization-only. Voxel
        // inspection, intensity statistics, and original data stay lossless.
        let step=if mode==VolumeMode::MaxIntensity {
            match self.preset {RenderPreset::Economy=>4,RenderPreset::Auto=>2,RenderPreset::Detail=>1}
        }else{1};
        let samples=if mode==VolumeMode::MaxIntensity {
            match self.preset {RenderPreset::Economy=>24,RenderPreset::Auto=>48,RenderPreset::Detail=>96}
        }else{1};
        let physical=[dx as f32*vol.spacing_um[0],dy as f32*vol.spacing_um[1],dz as f32*vol.spacing_um[2]];
        let largest=physical.into_iter().fold(1e-6f32,f32::max);
        let normalized=[physical[0]/largest,physical[1]/largest,physical[2]/largest];
        if mode==VolumeMode::Surface {
            // "gain" is the 0..1 iso-level in Surface mode (MIP uses it for brightness).
            self.surface(vol,ax,ay,zoom,pan,gain,normalized);
            return;
        }
        for yy in (0..h).step_by(step){
            let v=1.0-(yy as f32+0.5)/h as f32*2.0;
            for xx in (0..w).step_by(step){
                let u=((xx as f32+0.5)/w as f32*2.0-1.0)*aspect;
                let u=u/zoom.max(0.00001)+pan.0*2.0;
                let v=v/zoom.max(0.00001)+pan.1*2.0;
                let mut value=0.0f32;let mut hit=false;
                let ray=if matches!(mode,VolumeMode::MaxIntensity|VolumeMode::Surface) {
                    let origin=unrotate(Vec3::new(u,v,-2.0),ax,ay);
                    let direction=unrotate(Vec3::new(0.,0.,1.),ax,ay);
                    ray_box(origin,direction,normalized).map(|(a,b)|(origin,direction,a,b))
                }else{None};
                if matches!(mode,VolumeMode::MaxIntensity|VolumeMode::Surface) && ray.is_none(){continue;}
                for s in 0..samples {
                    let unit=slice.clamp(0.0,1.0)*2.0-1.0;
                    let p=match mode {
                        VolumeMode::Surface|VolumeMode::MaxIntensity=>{
                            let (origin,direction,a,b)=ray.unwrap();
                            let t=a+(s as f32+0.5)*(b-a)/(samples as f32);
                            origin+direction*t
                        },
                        VolumeMode::Xy=>Vec3::new(u,v,unit*normalized[2]),
                        VolumeMode::Xz=>Vec3::new(u,unit*normalized[1],v),
                        VolumeMode::Yz=>Vec3::new(unit*normalized[0],u,v),
                    };
                    if p.x.abs()>normalized[0] || p.y.abs()>normalized[1] || p.z.abs()>normalized[2]{continue;}
                    let ix=(((p.x/normalized[0]*0.5+0.5)*dx as f32) as usize).min(dx-1);
                    let iy=(((p.y/normalized[1]*0.5+0.5)*dy as f32) as usize).min(dy-1);
                    let iz=(((p.z/normalized[2]*0.5+0.5)*dz as f32) as usize).min(dz-1);
                    let raw=vol.value(ix,iy,iz);
                    let intensity=((raw-low)*inv*gain).clamp(0.0,1.0);
                    value=value.max(intensity);hit=true;
                }
                if !hit || value <= 0.015 {continue;}
                let r=(value.sqrt()*188.0+18.0) as u32;
                let g=(value.sqrt()*218.0+19.0) as u32;
                let b=(value.sqrt()*247.0+27.0) as u32;
                let color=0xff000000 | (r.min(255)<<16)|(g.min(255)<<8)|b.min(255);
                for ry in yy..(yy+step).min(h) {for rx in xx..(xx+step).min(w){self.viewport.pixels[ry*w+rx]=color;}}
            }
        }
    }
    /// Trilinear sample in the unit volume used for ray/surface intersection.
    fn sample(vol:&Volume,p:Vec3,r:[f32;3])->f32 {
        let d=vol.dims;
        let fx=((p.x/r[0]*0.5+0.5)*(d[0]-1) as f32).clamp(0.,(d[0]-1) as f32);
        let fy=((p.y/r[1]*0.5+0.5)*(d[1]-1) as f32).clamp(0.,(d[1]-1) as f32);
        let fz=((p.z/r[2]*0.5+0.5)*(d[2]-1) as f32).clamp(0.,(d[2]-1) as f32);
        let (x,y,z)=(fx.floor() as usize,fy.floor() as usize,fz.floor() as usize);
        let (x1,y1,z1)=((x+1).min(d[0]-1),(y+1).min(d[1]-1),(z+1).min(d[2]-1));
        let (tx,ty,tz)=(fx-x as f32,fy-y as f32,fz-z as f32);
        let a=vol.value(x,y,z)*(1.-tx)+vol.value(x1,y,z)*tx;
        let b=vol.value(x,y1,z)*(1.-tx)+vol.value(x1,y1,z)*tx;
        let c=vol.value(x,y,z1)*(1.-tx)+vol.value(x1,y,z1)*tx;
        let d=vol.value(x,y1,z1)*(1.-tx)+vol.value(x1,y1,z1)*tx;
        (a*(1.-ty)+b*ty)*(1.-tz)+(c*(1.-ty)+d*ty)*tz
    }
    /// Fast nearest-neighbor ray probe. Trilinear interpolation is reserved for
    /// the visible contour, reducing the work for empty background voxels.
    fn sample_nearest(vol:&Volume,p:Vec3,r:[f32;3])->f32 {
        let d=vol.dims;
        let x=(((p.x/r[0]*0.5+0.5)*(d[0]-1) as f32).round() as usize).min(d[0]-1);
        let y=(((p.y/r[1]*0.5+0.5)*(d[1]-1) as f32).round() as usize).min(d[1]-1);
        let z=(((p.z/r[2]*0.5+0.5)*(d[2]-1) as f32).round() as usize).min(d[2]-1);
        vol.value(x,y,z)
    }
    /// Row-disjoint parallel isosurface ray marching. Retains the same 96
    /// probes, gradient-based normal and threshold, with no additional crates.
    fn surface(&mut self,vol:&Volume,ax:f32,ay:f32,zoom:f32,pan:(f32,f32),contour:f32,r:[f32;3]) {
        let w=self.viewport.width as usize;
        let h=self.viewport.height as usize;
        if w==0||h==0{return;}
        let (lo,hi)=vol.min_max();
        let threshold=lo+(hi-lo)*contour.clamp(0.001,0.999);
        let (steps,stride)=match self.preset {
            RenderPreset::Economy=>(48usize,4usize),
            RenderPreset::Auto=>(96usize,2usize),
            RenderPreset::Detail=>(128usize,1usize),
        };
        let logical=std::thread::available_parallelism().map_or(1,|n|n.get());
        // Leave one logical core available for the UI; constrain tiny jobs.
        let lanes=logical.saturating_sub(1).max(1).min((h/96).max(1));
        if lanes<=1 {
            Self::surface_rows_quality(vol,&mut self.viewport.pixels,0,w,h,ax,ay,zoom,pan,threshold,r,steps,stride);
            return;
        }
        let band_rows=h.div_ceil(lanes).div_ceil(stride)*stride;
        std::thread::scope(|scope| {
            for (index,band) in self.viewport.pixels.chunks_mut(band_rows*w).enumerate() {
                let start=index*band_rows;
                scope.spawn(move ||{
                    Self::surface_rows_quality(vol,band,start,w,h,ax,ay,zoom,pan,threshold,r,steps,stride);
                });
            }
        });
    }
    /// Retain the original deterministic 96-step/2-pixel test entrypoint.
    #[cfg(test)]
    fn surface_rows(vol:&Volume,pixels:&mut [u32],first_row:usize,w:usize,h:usize,
        ax:f32,ay:f32,zoom:f32,pan:(f32,f32),threshold:f32,r:[f32;3]) {
        Self::surface_rows_quality(vol,pixels,first_row,w,h,ax,ay,zoom,pan,threshold,r,96,2);
    }
    fn surface_rows_quality(vol:&Volume,pixels:&mut [u32],first_row:usize,w:usize,h:usize,
        ax:f32,ay:f32,zoom:f32,pan:(f32,f32),threshold:f32,r:[f32;3],steps:usize,stride:usize) {
        let aspect=w as f32/h.max(1) as f32;
        let eps=Vec3::new(2.*r[0]/vol.dims[0].max(1) as f32,
            2.*r[1]/vol.dims[1].max(1) as f32,
            2.*r[2]/vol.dims[2].max(1) as f32);
        let light=Vec3::new(-0.5,0.65,-0.8).normalized();
        let rows=pixels.len()/w;
        for row in (0..rows).step_by(stride) {
            let yy=first_row+row;
            let screen_v=1.0-((yy as f32+0.5)/h as f32)*2.;
            for xx in (0..w).step_by(stride) {
                let screen_u=((xx as f32+0.5)/w as f32*2.-1.)*aspect;
                let u=screen_u/zoom.max(0.00001)+pan.0*2.;
                let v=screen_v/zoom.max(0.00001)+pan.1*2.;
                let o=unrotate(Vec3::new(u,v,-2.),ax,ay);
                let d=unrotate(Vec3::new(0.,0.,1.),ax,ay);
                let Some((entry,exit))=ray_box(o,d,r) else{continue};
                let dt=(exit-entry)/steps as f32;
                let mut first=None;
                let mut previous=Self::sample_nearest(vol,o+d*entry,r);
                for i in 1..=steps {
                    let t=entry+dt*i as f32;
                    let current=Self::sample_nearest(vol,o+d*t,r);
                    if current>=threshold {
                        let alpha=if current>previous && previous<threshold {
                            ((threshold-previous)/(current-previous)).clamp(0.,1.)
                        }else{1.};
                        first=Some(o+d*(t-dt+dt*alpha));
                        break;
                    }
                    previous=current;
                }
                let Some(point)=first else {continue};
                let grad=Vec3::new(
                    Self::sample(vol,point+Vec3::new(eps.x,0.,0.),r)-Self::sample(vol,point-Vec3::new(eps.x,0.,0.),r),
                    Self::sample(vol,point+Vec3::new(0.,eps.y,0.),r)-Self::sample(vol,point-Vec3::new(0.,eps.y,0.),r),
                    Self::sample(vol,point+Vec3::new(0.,0.,eps.z),r)-Self::sample(vol,point-Vec3::new(0.,0.,eps.z),r));
                let n=if grad.len()>1e-7 {rotate(grad.normalized()*-1.,ax,ay)} else {Vec3::new(0.,0.,-1.)};
                let brightness=(0.29+0.68*n.dot(light).max(0.)).clamp(0.28,1.0);
                let c=((brightness*210.) as u32).min(255);
                let rr=(c as f32*0.82) as u32;
                let gg=(c as f32*0.94) as u32;
                let color=0xff000000|(rr<<16)|(gg<<8)|c;
                for ry in row..(row+stride).min(rows) {
                    for rx in xx..(xx+stride).min(w) {pixels[ry*w+rx]=color;}
                }
            }
        }
    }

}
fn rotate(p:Vec3,ax:f32,ay:f32)->Vec3 {
    let (s,c)=ax.sin_cos();let p=Vec3::new(p.x,p.y*c-p.z*s,p.y*s+p.z*c);
    let (s,c)=ay.sin_cos();Vec3::new(p.x*c+p.z*s,p.y,-p.x*s+p.z*c)
}
fn unrotate(p:Vec3,ax:f32,ay:f32)->Vec3 {
    let (s,c)=ay.sin_cos();let p=Vec3::new(p.x*c-p.z*s,p.y,p.x*s+p.z*c);
    let (s,c)=ax.sin_cos();Vec3::new(p.x,p.y*c+p.z*s,-p.y*s+p.z*c)
}
fn on_ellipse(c:Vec3,r:Vec3,plane:usize,a:f32)->Vec3 {
    match plane {0=>Vec3::new(c.x+r.x*a.cos(),c.y+r.y*a.sin(),c.z),
        1=>Vec3::new(c.x+r.x*a.cos(),c.y,c.z+r.z*a.sin()),
        _=>Vec3::new(c.x,c.y+r.y*a.cos(),c.z+r.z*a.sin())}
}

// Intersect a volume-aligned box. This keeps MIP useful even for very thin Z stacks.
fn ray_box(origin:Vec3,dir:Vec3,radii:[f32;3])->Option<(f32,f32)> {
    let mut lo=0.0f32;let mut hi=4.0f32;
    for (p,v,r) in [(origin.x,dir.x,radii[0]),(origin.y,dir.y,radii[1]),(origin.z,dir.z,radii[2])] {
        if v.abs()<1e-7 {if p.abs()>r{return None;}}
        else {let a=(-r-p)/v;let b=(r-p)/v;lo=lo.max(a.min(b));hi=hi.min(a.max(b));if hi<lo{return None;}}
    }
    (hi>lo).then_some((lo,hi))
}
#[cfg(test)]mod cell_view_tests {
    use super::*;
    #[test]fn projection_ray_handles_thin_volume(){let result=ray_box(Vec3::new(0.,0.,-2.),Vec3::new(0.,0.,1.),[1.,1.,0.01]).unwrap();assert!(result.1>result.0);assert!(result.1-result.0<0.021);}
    #[test]fn misses_outside_volume(){assert!(ray_box(Vec3::new(2.,0.,-2.),Vec3::new(0.,0.,1.),[1.,1.,1.]).is_none());}
    #[test]fn surface_mode_has_distinct_label(){assert_eq!(VolumeMode::Surface.label(),"3D SURFACE");}
    #[test]fn trilinear_sample_center(){
        let vol=Volume::from_voxels([2,2,2],vec![0.,1.,1.,0.,1.,0.,0.,1.]).unwrap();
        assert!((CellViewer::sample(&vol,Vec3::new(0.,0.,0.),[1.,1.,1.])-0.5).abs()<1e-6);
    }
    #[test]fn surface_parallel_bands_match_serial() {
        let d=12usize;
        let volume=Volume::from_voxels([d,d,d],(0..d*d*d).map(|i|if i%d<4{1.0}else{0.0}).collect()).unwrap();
        let r=[1.,1.,1.];let (w,h)=(88usize,72usize);
        let mut serial=vec![0xff102034;w*h];let mut bands=serial.clone();
        let (lo,hi)=volume.min_max();let threshold=(lo+hi)*0.5;
        CellViewer::surface_rows(&volume,&mut serial,0,w,h,0.25,0.7,1.0,(0.,0.),threshold,r);
        let middle=36*w;
        CellViewer::surface_rows(&volume,&mut bands[..middle],0,w,h,0.25,0.7,1.0,(0.,0.),threshold,r);
        CellViewer::surface_rows(&volume,&mut bands[middle..],36,w,h,0.25,0.7,1.0,(0.,0.),threshold,r);
        assert_eq!(serial,bands);
    }
    #[test]fn surface_draws_sphere_without_volume_box() {
        let d=24usize;
        let mut values=Vec::with_capacity(d*d*d);
        for z in 0..d {for y in 0..d {for x in 0..d {
            let delta=(x as f32-11.5).powi(2)+(y as f32-11.5).powi(2)+(z as f32-11.5).powi(2);
            values.push(if delta<65. {1.0}else{0.0});
        }}}
        let vol=CellData::Microscopy(Volume::from_voxels([d,d,d],values).unwrap());
        let mut view=CellViewer::new();
        let mut frame=Framebuffer::from_color(96,96,0xff000000);
        view.draw(&mut frame,Rect::new(0,0,96,96),&vol,0.0,0.0,1.0,(0.,0.),None,VolumeMode::Surface,0.5,0.5);
        let center=frame.pixels[48*96+48];
        let edge=frame.pixels[3*96+3];
        assert_ne!(center,edge,"visible isodensity object expected");
        assert_eq!(edge,0xff13293a,"surface must not paint the volume cuboid");
    }
}
