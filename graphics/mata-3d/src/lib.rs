//! Reusable CPU 3D rasterizer using `mata_win::Framebuffer` as a render target.
//!
//! - Perspective camera with per-pixel reciprocal-Z depth testing.
//! - Near-plane triangle clipping, clipping to framebuffer bounds.
//! - Meshes (indexed triangles), transforms and a small Wavefront OBJ reader.
//! - Solid materials, ambient/directional light, optional distance fog.
//! - Cubes and octahedra for games that generate their geometry procedurally.
//! - No GPU API, shader compiler, external math crate or frame-sized transient
//!   allocation: depth storage is reused whenever framebuffer size is stable.
//!
//! `begin_frame` clears the internal depth buffer, **not the framebuffer**:
//! draw your own background, then submit triangles/meshes. `clear_depth`
//! allows a first-person model to draw over the finished world scene.

use std::ops::{Add, Div, Mul, Sub};
use mata_win::Framebuffer;
mod simd;
pub mod shader_dispatch;
pub mod gpu_backend;
pub mod parallel;
use std::sync::atomic::{AtomicU64,Ordering};
static NEXT_GPU_CAPTURE_ID:AtomicU64=AtomicU64::new(1);

pub const DEFAULT_NEAR: f32 = 0.075;

/// GPU-ready triangle vertices in clip space. The CPU still performs model
/// transforms, lighting and near-plane clipping; the GPU performs triangle
/// fill, interpolated depth testing and native-resolution output.
#[repr(C)]
#[derive(Clone,Copy,Debug,PartialEq)]
pub struct GpuVertex {
    /// Clip-space position (12 bytes, full 32-bit precision).
    pub position:[f32;3],
    /// RGBA8_UNORM as little-endian bytes [R,G,B,A]. The D3D11 input layout
    /// unpacks this to float4 automatically; exact original 8-bit colours
    /// are preserved while cutting per-vertex upload traffic from 28 to 16 B.
    pub rgba8:u32,
}

/// One GPU-resident draw instance of a unit sphere. Unlike `GpuVertex`,
/// the CPU only projects the center once; D3D11 expands the shared mesh in
/// the vertex shader. 32 bytes per atom irrespective of tessellation.
#[repr(C)]
#[derive(Clone,Copy,Debug,PartialEq)]
pub struct GpuSphereInstance {
    /// Screen-space center (NDC X/Y), camera-space positive depth and radius.
    pub center_depth:[f32;4],
    /// NDC projected X/Y radius; camera near plane for reciprocal depth.
    pub scale:[f32;2],
    pub near:f32,
    /// Little-endian RGBA8_UNORM (bytes R,G,B,A).
    pub rgba8:u32,
}

/// A cylindrical bond generated from a single shared unit-cylinder mesh.
/// Endpoints are in camera space, so the GPU can orient and shade every bond.
#[repr(C)]
#[derive(Clone,Copy,Debug,PartialEq)]
pub struct GpuBondInstance {
    /// Start X/Y/Z and world-space cylinder radius.
    pub start_radius:[f32;4],
    /// End X/Y/Z and near-plane depth.
    pub end_near:[f32;4],
    /// NDC projection scale X/Y and viewport origin X/Y.
    pub projection:[f32;4],
    pub rgba8:u32,
}

#[derive(Clone,Debug,Default)]
pub struct GpuFrame {
    pub vertices:Vec<GpuVertex>,
    /// GPU hardware-instanced atoms; empty for legacy game renderers.
    pub spheres:Vec<GpuSphereInstance>,
    pub bonds:Vec<GpuBondInstance>,
    /// Unique ID for completed captures. 0 means manually constructed or
    /// modified geometry and DISABLES GPU upload caching. If editing vertices
    /// after a successful presentation, reset this field to 0 first.
    pub generation:u64,
    /// Vertex offsets at which the first-person depth buffer must be cleared.
    pub depth_clears:Vec<usize>,
}

#[cfg(target_os="windows")]
pub mod intel_gpu;


#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 { pub x:f32, pub y:f32, pub z:f32 }
impl Vec3 {
    pub const fn new(x:f32,y:f32,z:f32)->Self {Self{x,y,z}}
    #[inline] pub fn dot(self,b:Self)->f32 {self.x*b.x+self.y*b.y+self.z*b.z}
    #[inline] pub fn cross(self,b:Self)->Self {Self::new(self.y*b.z-self.z*b.y,self.z*b.x-self.x*b.z,self.x*b.y-self.y*b.x)}
    #[inline] pub fn len(self)->f32 {self.dot(self).sqrt()}
    #[inline] pub fn normalized(self)->Self {self/self.len().max(1e-8)}
    #[inline] pub fn horizontal(self)->Self {Self::new(self.x,0.0,self.z)}
}
impl Add for Vec3 {type Output=Self; fn add(self,b:Self)->Self{Self::new(self.x+b.x,self.y+b.y,self.z+b.z)}}
impl Sub for Vec3 {type Output=Self; fn sub(self,b:Self)->Self{Self::new(self.x-b.x,self.y-b.y,self.z-b.z)}}
impl Mul<f32> for Vec3 {type Output=Self; fn mul(self,b:f32)->Self{Self::new(self.x*b,self.y*b,self.z*b)}}
impl Div<f32> for Vec3 {type Output=Self; fn div(self,b:f32)->Self{Self::new(self.x/b,self.y/b,self.z/b)}}

#[derive(Clone,Copy,Debug,PartialEq)]
pub struct Aabb { pub min:Vec3,pub max:Vec3 }
impl Aabb {
    pub fn new(x:f32,y:f32,z:f32,w:f32,h:f32,d:f32)->Self {
        Self{min:Vec3::new(x,y,z),max:Vec3::new(x+w,y+h,z+d)}
    }
    /// Circular footprint collision in the X/Z plane.
    pub fn intersects_circle(&self,x:f32,z:f32,r:f32)->bool {
        let dx=x-x.clamp(self.min.x,self.max.x);
        let dz=z-z.clamp(self.min.z,self.max.z);
        dx*dx+dz*dz<r*r
    }
    /// Nearest intersection in front of a ray. Handles parallel axes.
    pub fn ray_hit(&self,o:Vec3,d:Vec3)->Option<f32> {
        let mut near=0.0f32;let mut far=f32::INFINITY;
        for (a,dir,lo,hi) in [(o.x,d.x,self.min.x,self.max.x),(o.y,d.y,self.min.y,self.max.y),(o.z,d.z,self.min.z,self.max.z)] {
            if dir.abs()<1e-7 {if a<lo||a>hi{return None}} else {
                let t0=(lo-a)/dir;let t1=(hi-a)/dir;
                near=near.max(t0.min(t1));far=far.min(t0.max(t1));
                if far<near{return None}
            }
        }
        Some(near)
    }
}

/// A lightweight instance of cached geometry. `radius` must enclose the
/// mesh after scaling, for conservative frustum culling.
#[derive(Clone,Copy,Debug)]
pub struct MeshInstance {pub transform:Transform,pub material:Material,pub radius:f32}

#[derive(Clone,Copy,Debug)]
pub struct Camera {
    pub position:Vec3,
    pub forward:Vec3,
    pub right:Vec3,
    pub up:Vec3,
    /// Focal distance in framebuffer *height* units (0.96 matches Neon Run).
    pub focal_scale:f32,
    pub near:f32,
}
impl Camera {
    pub fn new(position:Vec3,forward:Vec3,right:Vec3)->Self {
        let f=forward.normalized();let r=right.normalized();
        Self{position,forward:f,right:r,up:f.cross(r).normalized(),focal_scale:0.96,near:DEFAULT_NEAR}
    }
    #[inline] pub fn view(&self,p:Vec3)->Vec3 {
        let v=p-self.position;
        Vec3::new(v.dot(self.right),v.dot(self.up),v.dot(self.forward))
    }
}

#[derive(Clone,Copy,Debug)]
pub struct Material { pub color:u32, pub emissive:bool, pub double_sided:bool }
impl Material {
    pub const fn lit(color:u32)->Self { Self{color,emissive:false,double_sided:false} }
    pub const fn glow(color:u32)->Self { Self{color,emissive:true,double_sided:false} }
    pub const fn two_sided(color:u32,emissive:bool)->Self { Self{color,emissive,double_sided:true} }
}

#[derive(Clone,Copy,Debug)]
pub struct Transform { pub translation:Vec3, pub scale:Vec3, pub rotation:Vec3 }
impl Default for Transform {
    fn default()->Self {Self{translation:Vec3::default(),scale:Vec3::new(1.0,1.0,1.0),rotation:Vec3::default()}}
}
impl Transform {
    pub fn from_translation(position:Vec3)->Self {Self{translation:position,..Default::default()}}
    pub fn at_scale(position:Vec3,uniform:f32)->Self {Self{translation:position,scale:Vec3::new(uniform,uniform,uniform),..Default::default()}}
    /// Euler angles in radians, applied X then Y then Z.
    pub fn point(&self,p:Vec3)->Vec3 {PreparedTransform::new(*self).point(p)}
}

/// Cached Euler sines/cosines: a mesh never recalculates trig per vertex.
#[derive(Clone,Copy)]
struct PreparedTransform {
    translation:Vec3,scale:Vec3,
    sx:f32,cx:f32,sy:f32,cy:f32,sz:f32,cz:f32,
}
impl PreparedTransform {
    fn new(t:Transform)->Self {
        let (sx,cx)=t.rotation.x.sin_cos();
        let (sy,cy)=t.rotation.y.sin_cos();
        let (sz,cz)=t.rotation.z.sin_cos();
        Self{translation:t.translation,scale:t.scale,sx,cx,sy,cy,sz,cz}
    }
    /// Coefficients for four-wide affine SIMD transforms. Exactly one
    /// trigonometric preparation per mesh instance, not per triangle.
    fn matrix(&self)->[[f32;4];3]{
        let (sx,cx,sy,cy,sz,cz)=(self.sx,self.cx,self.sy,self.cy,self.sz,self.cz);
        let q=self.scale;
        [
            [q.x*cy*cz, q.y*(sx*sy*cz-cx*sz), q.z*(cx*sy*cz+sx*sz),self.translation.x],
            [q.x*cy*sz, q.y*(sx*sy*sz+cx*cz), q.z*(cx*sy*sz-sx*cz),self.translation.y],
            [-q.x*sy,   q.y*sx*cy,              q.z*cx*cy,              self.translation.z],
        ]
    }
    #[inline] fn point(&self,p:Vec3)->Vec3 {
        let p=Vec3::new(p.x*self.scale.x,p.y*self.scale.y,p.z*self.scale.z);
        let p=Vec3::new(p.x,p.y*self.cx-p.z*self.sx,p.y*self.sx+p.z*self.cx);
        let p=Vec3::new(p.x*self.cy+p.z*self.sy,p.y,-p.x*self.sy+p.z*self.cy);
        Vec3::new(p.x*self.cz-p.y*self.sz,p.x*self.sz+p.y*self.cz,p.z)+self.translation
    }
}

#[derive(Clone,Debug,Default)]
pub struct Mesh { pub vertices:Vec<Vec3>, pub triangles:Vec<[u32;3]> }
impl Mesh {
    pub fn new(vertices:Vec<Vec3>,triangles:Vec<[u32;3]>)->Result<Self,String> {
        if triangles.iter().flat_map(|t|t.iter()).any(|&i|i as usize>=vertices.len()) {
            return Err("mesh triangle references a missing vertex".into());
        }
        Ok(Self{vertices,triangles})
    }
    /// Unit cube centered at origin, with outward-wound faces.
    pub fn cube()->Self {
        let vertices=vec![Vec3::new(-0.5,-0.5,-0.5),Vec3::new(0.5,-0.5,-0.5),
            Vec3::new(0.5,0.5,-0.5),Vec3::new(-0.5,0.5,-0.5),
            Vec3::new(-0.5,-0.5,0.5),Vec3::new(0.5,-0.5,0.5),
            Vec3::new(0.5,0.5,0.5),Vec3::new(-0.5,0.5,0.5)];
        let mut triangles=Vec::with_capacity(12);
        for face in [[0,3,2,1],[4,5,6,7],[0,4,7,3],[1,2,6,5],[3,7,6,2],[0,1,5,4]] {
            triangles.push([face[0],face[1],face[2]]);
            triangles.push([face[0],face[2],face[3]]);
        }
        Self{vertices,triangles}
    }
    /// Eight triangles around six vertices; ideal for low-poly pickups/bolts.
    pub fn octahedron()->Self {
        Self {vertices:vec![Vec3::new(0.0,1.0,0.0),Vec3::new(1.0,0.0,0.0),
            Vec3::new(0.0,0.0,1.0),Vec3::new(-1.0,0.0,0.0),
            Vec3::new(0.0,0.0,-1.0),Vec3::new(0.0,-1.0,0.0)],
            triangles:vec![[0,1,2],[0,2,3],[0,3,4],[0,4,1],[5,2,1],[5,3,2],[5,4,3],[5,1,4]]}
    }
    /// Parse a basic Wavefront OBJ mesh (`v` and `f` records). Faces with
    /// 4+ vertices are fan-triangulated. `f v/vt/vn` and negative OBJ indices
    /// are supported. Materials/UVs/normals are deliberately not imported.
    pub fn from_obj(source:&str)->Result<Self,String> {
        let mut vertices=Vec::new();let mut triangles=Vec::new();
        for (line_no,line) in source.lines().enumerate() {
            let mut words=line.split_whitespace();
            match words.next() {
                Some("v")=>{
                    let mut next=|| -> Result<f32,String> {
                        words.next().ok_or_else(||format!("line {}: short vertex",line_no+1))?
                            .parse::<f32>().map_err(|_|format!("line {}: invalid vertex",line_no+1))
                    };
                    vertices.push(Vec3::new(next()?,next()?,next()?));
                },
                Some("f")=>{
                    let mut face=Vec::new();
                    for part in words.take_while(|w|!w.starts_with('#')) {
                        let raw=part.split('/').next().unwrap_or("");
                        let value=raw.parse::<i64>().map_err(|_|format!("line {}: invalid face",line_no+1))?;
                        let index=if value>0 {value-1} else if value<0 {vertices.len() as i64+value} else {-1};
                        if index<0 || index>=vertices.len() as i64 {return Err(format!("line {}: face index out of bounds",line_no+1));}
                        face.push(index as u32);
                    }
                    if face.len()<3 {return Err(format!("line {}: face must have 3 vertices",line_no+1));}
                    for i in 1..face.len()-1 {triangles.push([face[0],face[i],face[i+1]]);}
                },
                _=>{},
            }
        }
        Self::new(vertices,triangles)
    }
}

#[derive(Clone,Copy,Debug,Default)]
pub struct RenderStats {pub triangles:u32,pub pixels:u32}
#[derive(Clone,Copy,Debug)]struct Screen {x:f32,y:f32,invz:f32}

/// One full-precision projected triangle produced during one scene traversal.
#[derive(Clone,Copy,Debug)]
struct SoftwareTriangle {vertices:[Screen;3],color:u32}

/// An ordered, reusable projected-triangle stream shared across software
/// rasterizer workers. No mesh or model data is copied to workers.
#[derive(Debug,Default)]
pub struct SoftwareFrame {
    triangles:Vec<SoftwareTriangle>,
    depth_clears:Vec<usize>,
    bands:Vec<SoftwareBand>,
}
#[derive(Debug,Default)]
struct SoftwareBand {first_row:usize,end_row:usize,triangles:Vec<usize>}
impl SoftwareFrame {
    /// Number of fully detailed projected triangles in this frame.
    pub fn len(&self)->usize {self.triangles.len()}
    /// Whether there are any projected triangles.
    pub fn is_empty(&self)->bool {self.triangles.is_empty()}
    /// Index every triangle into only the horizontal render bands it overlaps.
    /// Indices are inserted in draw order, preserving depth test semantics.
    pub fn prepare_bands(&mut self,height:usize,count:usize){
        let count=count.max(1).min(height.max(1));
        self.bands.resize_with(count,SoftwareBand::default);
        for (i,band) in self.bands.iter_mut().enumerate(){
            band.first_row=height*i/count;
            band.end_row=height*(i+1)/count;
            band.triangles.clear();
        }
        for (index,triangle) in self.triangles.iter().enumerate(){
            let y_min=triangle.vertices.iter().map(|v|v.y).fold(f32::INFINITY,f32::min);
            let y_max=triangle.vertices.iter().map(|v|v.y).fold(f32::NEG_INFINITY,f32::max);
            for band in &mut self.bands {
                if y_min<band.end_row as f32 && y_max>=band.first_row as f32 {
                    band.triangles.push(index);
                }
            }
        }
    }
}

pub struct Rasterizer {
    depth:Vec<f32>, width:usize,height:usize,focal:f32,
    viewport_height:usize, y_offset:usize,
    camera:Camera,
    pub stats:RenderStats,
    pub fog_color:u32,
    pub fog_start:f32,
    pub fog_end:f32,
    pub fog_max:f32,
    pub light:Vec3,
    /// Optional distance cutoff for world geometry. Use None for molecular datasets
    /// whose coordinate ranges may be much larger than game worlds.
    pub max_distance:Option<f32>,
    gpu_capture:bool,
    gpu_frame:GpuFrame,
    software_capture:bool,
    software_frame:SoftwareFrame,
    // Reused per-mesh scratch. Distinct indexed vertices are transformed once
    // instead of up to three times for every triangle on 4K characters.
    transformed:Vec<Vec3>,
}
impl Default for Rasterizer {fn default()->Self{Self::new()}}
impl Rasterizer {
    pub fn new()->Self {
        Self{depth:Vec::new(),width:0,height:0,focal:1.0,viewport_height:0,y_offset:0,
            camera:Camera::new(Vec3::default(),Vec3::new(0.0,0.0,1.0),Vec3::new(1.0,0.0,0.0)),
            stats:RenderStats::default(),fog_color:0xff091827,
            fog_start:18.0,fog_end:63.0,fog_max:0.65,light:Vec3::new(0.5,0.88,0.5).normalized(),
            max_distance:Some(65.0),
            gpu_capture:false,gpu_frame:GpuFrame::default(),
            software_capture:false,software_frame:SoftwareFrame::default(),transformed:Vec::new()}
    }
    /// Select GPU geometry capture. CPU rasterization is skipped while active.
    /// Call before begin_frame; disable and take the GPU batch after rendering.
    pub fn set_gpu_capture(&mut self,enabled:bool){
        self.gpu_capture=enabled;
        if enabled {
            self.software_capture=false;
            self.gpu_frame.vertices.clear();self.gpu_frame.spheres.clear();self.gpu_frame.bonds.clear();self.gpu_frame.depth_clears.clear();
        }
    }
    pub fn take_gpu_frame(&mut self)->GpuFrame{
        self.gpu_capture=false;
        self.gpu_frame.generation=NEXT_GPU_CAPTURE_ID.fetch_add(1,Ordering::Relaxed);
        std::mem::take(&mut self.gpu_frame)
    }
    /// Return a submitted command buffer to the rasterizer to reuse its
    /// allocation next frame. Prevents reallocation of the often very large
    /// full-resolution articulated enemy vertex stream on each frame.
    pub fn recycle_gpu_frame(&mut self,mut previous:GpuFrame){
        previous.vertices.clear();
        previous.spheres.clear();
        previous.bonds.clear();
        previous.depth_clears.clear();
        previous.generation=0;
        if previous.vertices.capacity()>self.gpu_frame.vertices.capacity() ||
           previous.spheres.capacity()>self.gpu_frame.spheres.capacity() ||
           previous.bonds.capacity()>self.gpu_frame.bonds.capacity(){
            self.gpu_frame=previous;
        }
    }
    /// Capture fully transformed and shaded triangles once for multicore CPU
    /// rasterization. This uses no GPU hardware or GPU runtime.
    pub fn set_software_capture(&mut self,enabled:bool){
        self.software_capture=enabled;
        if enabled {
            self.gpu_capture=false;
            self.software_frame.triangles.clear();
            self.software_frame.depth_clears.clear();
        }
    }
    /// Returns the captured command stream and disables software capture.
    pub fn take_software_frame(&mut self)->SoftwareFrame{
        self.software_capture=false;
        std::mem::take(&mut self.software_frame)
    }
    /// Reuse the large command-buffer allocations after all bands finish.
    pub fn recycle_software_frame(&mut self,mut previous:SoftwareFrame){
        previous.triangles.clear();previous.depth_clears.clear();
        for band in &mut previous.bands {band.triangles.clear();}
        if previous.triangles.capacity()>self.software_frame.triangles.capacity(){
            self.software_frame=previous;
        }
    }
    /// Whether this rasterizer is recording a software draw-command frame.
    pub fn software_capture_enabled(&self)->bool{self.software_capture}
    /// Rasterize an already transformed command stream into one disjoint
    /// horizontal band. Every worker owns a separate depth buffer and pixels.
    /// Reuses the exact same edge/depth rasterizer as direct software drawing.
    pub fn draw_software_frame_band(&mut self,target:&mut Framebuffer,
        frame:&SoftwareFrame,viewport_height:usize,y_offset:usize){
        self.software_capture=false;
        self.gpu_capture=false;
        let camera=self.camera;
        self.begin_frame_band(target,camera,viewport_height,y_offset);
        let band=frame.bands.iter().find(|band|band.first_row==y_offset
            && band.end_row==y_offset+target.height as usize);
        let mut draw_one=|this:&mut Self,index:usize,clear_index:&mut usize|{
            while *clear_index<frame.depth_clears.len()
                && frame.depth_clears[*clear_index]<=index {
                this.clear_depth();*clear_index+=1;
            }
            let triangle=&frame.triangles[index];
            let mut vertices=triangle.vertices;
            for vertex in &mut vertices {vertex.y-=y_offset as f32;}
            this.rasterize_projected(target,vertices,triangle.color);
        };
        let mut clear_index=0usize;
        if let Some(band)=band {
            for &i in &band.triangles {draw_one(self,i,&mut clear_index);}
        } else {
            // Unindexed streams remain fully supported by this public API.
            for i in 0..frame.triangles.len(){draw_one(self,i,&mut clear_index);}
        }
    }
    /// Used to avoid generating a CPU sky gradient when the shader generates
    /// precisely the same background in the hardware compositor.
    pub fn gpu_capture_enabled(&self)->bool{self.gpu_capture}
    /// Prepare a new frame (reuse depth allocation and clear Z/stats).
    pub fn begin_frame(&mut self,target:&Framebuffer,camera:Camera) {
        self.begin_frame_band(target,camera,target.height as usize,0);
    }
    /// Render a vertical band while retaining the perspective and clipping of
    /// the entire viewport. The framebuffer contains only `target.height`
    /// consecutive rows, starting at `y_offset` of `viewport_height`.
    /// Different bands never write overlapping pixels or depth cells.
    pub fn begin_frame_band(&mut self,target:&Framebuffer,camera:Camera,
        viewport_height:usize,y_offset:usize) {
        self.width=target.width as usize;self.height=target.height as usize;
        self.viewport_height=viewport_height;self.y_offset=y_offset;
        debug_assert!(y_offset+self.height<=viewport_height);
        if !self.gpu_capture && !self.software_capture {
            let pixels=self.width.saturating_mul(self.height);
            if self.depth.len()!=pixels {self.depth.resize(pixels,0.0)}
            self.depth.fill(0.0);
        }
        self.camera=camera;
        self.focal=viewport_height as f32*camera.focal_scale;
        self.stats=RenderStats::default();
    }
    /// Reset only the depth buffer, e.g. before a 3D first-person weapon.
    pub fn clear_depth(&mut self){
        if self.gpu_capture{self.gpu_frame.depth_clears.push(self.gpu_frame.vertices.len());}
        else if self.software_capture {
            self.software_frame.depth_clears.push(self.software_frame.triangles.len());
        } else {self.depth.fill(0.0);}
    }
    pub fn camera(&self)->Camera{self.camera}
    /// Capture a cylinder as a hardware instance instead of expanding its
    /// 16+ triangles on the CPU. Returns false for near-plane crossing bonds
    /// to retain the existing exact near-plane clipping fallback.
    pub fn capture_bond(&mut self,a:Vec3,b:Vec3,radius:f32,argb:u32)->bool {
        if !self.gpu_capture || radius<=0.0 || !radius.is_finite()
           || self.width==0 || self.viewport_height==0 {return false;}
        let len=(b-a).len();
        if len<=1e-6 || !len.is_finite(){return false;}
        if !self.sphere_visible((a+b)*0.5,len*0.5+radius){return true;}
        let a=self.camera.view(a);let b=self.camera.view(b);
        let near=self.camera.near.max(0.001);
        if a.z<=near+radius || b.z<=near+radius{return false;}
        if ![a.x,a.y,a.z,b.x,b.y,b.z].iter().all(|v|v.is_finite()){return false;}
        self.gpu_frame.bonds.push(GpuBondInstance{
            start_radius:[a.x,a.y,a.z,radius],end_near:[b.x,b.y,b.z,near],
            projection:[2.0*self.focal/self.width as f32,
                        2.0*self.focal/self.viewport_height as f32,0.0,0.0],
            rgba8:((argb>>16)&255) | (argb&0x0000_ff00) |
                 ((argb&255)<<16) | 0xff00_0000,
        });
        true
    }
    /// Submit a unit sphere for a native D3D11 DrawInstanced call. Returns
    /// `false` on software renderers and for near-plane intersecting spheres,
    /// which must use the existing clipped triangle path instead.
    /// Frustum culling never changes input molecules.
    pub fn capture_sphere(&mut self,center:Vec3,radius:f32,argb:u32)->bool {
        if !self.gpu_capture || radius<=0.0 || !radius.is_finite()
            || self.width==0 || self.viewport_height==0 {return false;}
        if !self.sphere_visible(center,radius){return true;}
        let p=self.camera.view(center);
        let near=self.camera.near.max(0.001);
        if p.z-radius<=near || !p.z.is_finite(){return false;}
        let fx=self.focal/p.z;
        let cx=2.0*(self.width as f32*0.5+p.x*fx)/self.width as f32-1.0;
        let cy=1.0-2.0*(self.viewport_height as f32*0.5-p.y*fx)/self.viewport_height as f32;
        let sx=2.0*radius*fx/self.width as f32;
        let sy=2.0*radius*fx/self.viewport_height as f32;
        if ![cx,cy,sx,sy].iter().all(|x|x.is_finite()){return false;}
        self.gpu_frame.spheres.push(GpuSphereInstance{
            center_depth:[cx,cy,p.z,radius],scale:[sx,sy],near,
            rgba8:((argb>>16)&255) | (argb&0x0000_ff00) |
                 ((argb&255)<<16) | 0xff00_0000,
        });
        true
    }
    /// Conservative sphere/viewport-frustum check. Does not discard a sphere
    /// that intersects the near plane or visible canvas edges. Coordinates
    /// are world-space, matching `mesh()` / `triangle()`.
    pub fn sphere_visible(&self,center:Vec3,radius:f32)->bool {
        if self.width==0 || self.viewport_height==0 || !radius.is_finite() || radius<0.0 {
            return false;
        }
        let p=self.camera.view(center);
        if ![p.x,p.y,p.z].iter().all(|v|v.is_finite()){return false;}
        if p.z+radius < self.camera.near.max(0.001){return false;}
        if let Some(far)=self.max_distance {
            if (center-self.camera.position).len()>far+radius{return false;}
        }
        let inv_focal=1.0/self.focal.max(0.001);
        let tx=self.width as f32*0.5*inv_focal;
        let top=(self.viewport_height as f32*0.5-self.y_offset as f32)*inv_focal;
        let bottom=(self.y_offset as f32+self.height as f32-self.viewport_height as f32*0.5)*inv_focal;
        p.x.abs()<=p.z*tx+radius*(1.0+tx*tx).sqrt()
            && p.y<=p.z*top+radius*(1.0+top*top).sqrt()
            && -p.y<=p.z*bottom+radius*(1.0+bottom*bottom).sqrt()
    }
    /// Frustum-test a world-space bounding box without testing each mesh
    /// triangle. The enclosing sphere makes this conservative even if rotated.
    pub fn bounds_visible(&self,bounds:Aabb)->bool {
        let center=(bounds.min+bounds.max)*0.5;
        let radius=(bounds.max-bounds.min).len()*0.5;
        self.sphere_visible(center,radius)
    }
    /// CPU instancing of a shared mesh with per-instance transforms/colors.
    /// The existing GPU-capture path also accepts these calls, but retains CPU
    /// vertex transforms; this does NOT claim GPU hardware instanced draw calls.
    pub fn mesh_instances(&mut self,fb:&mut Framebuffer,mesh:&Mesh,instances:&[MeshInstance]) {
        for instance in instances {
            if self.sphere_visible(instance.transform.translation,instance.radius) {
                self.mesh(fb,mesh,instance.transform,instance.material);
            }
        }
    }
    /// Draw a world-space triangle; all depth testing is internal.
    pub fn triangle(&mut self,fb:&mut Framebuffer,a:Vec3,b:Vec3,c:Vec3,m:Material) {
        if self.width==0||self.height==0{return}
        let center=(a+b+c)/3.0;
        // Reject invisible/back-facing geometry before doing any sqrt.
        let normal=(b-a).cross(c-a);
        if !m.double_sided && normal.dot(self.camera.position-center)<=0.0 {return}
        let delta=center-self.camera.position;
        let distance_squared=delta.dot(delta);
        if self.max_distance.is_some_and(|limit|distance_squared>limit*limit){return}
        let distance=distance_squared.sqrt();
        let mut color=m.color;
        if !m.emissive {
            color=shade(color,0.65+0.35*normal.normalized().dot(self.light).abs());
        }
        let fog=((distance-self.fog_start)/(self.fog_end-self.fog_start).max(0.001)).clamp(0.0,self.fog_max);
        color=mix(color,self.fog_color,fog);
        self.clip_triangle(fb,[self.camera.view(a),self.camera.view(b),self.camera.view(c)],color);
    }
    /// Camera-space geometry is useful for first-person models: avoids
    /// reconstructing the world camera matrix for every animated weapon.
    pub fn triangle_view(&mut self,fb:&mut Framebuffer,tri:[Vec3;3],color:u32) {
        self.clip_triangle(fb,tri,color);
    }
    pub fn quad(&mut self,fb:&mut Framebuffer,a:Vec3,b:Vec3,c:Vec3,d:Vec3,m:Material){
        self.triangle(fb,a,b,c,m);self.triangle(fb,a,c,d,m);
    }
    /// Quickly submit an axis-aligned cuboid without allocating a mesh.
    pub fn box3(&mut self,fb:&mut Framebuffer,b:Aabb,m:Material){self.box_internal(fb,b,m,false)}
    pub fn box_view(&mut self,fb:&mut Framebuffer,b:Aabb,m:Material){self.box_internal(fb,b,m,true)}
    fn box_internal(&mut self,fb:&mut Framebuffer,b:Aabb,m:Material,view:bool){
        let l=b.min;let h=b.max;
        let v=[Vec3::new(l.x,l.y,l.z),Vec3::new(h.x,l.y,l.z),Vec3::new(h.x,h.y,l.z),Vec3::new(l.x,h.y,l.z),
            Vec3::new(l.x,l.y,h.z),Vec3::new(h.x,l.y,h.z),Vec3::new(h.x,h.y,h.z),Vec3::new(l.x,h.y,h.z)];
        const F:[[usize;4];6]=[[0,3,2,1],[4,5,6,7],[0,4,7,3],[1,2,6,5],[3,7,6,2],[0,1,5,4]];
        const N:[Vec3;6]=[Vec3::new(0.0,0.0,-1.0),Vec3::new(0.0,0.0,1.0),
            Vec3::new(-1.0,0.0,0.0),Vec3::new(1.0,0.0,0.0),Vec3::new(0.0,1.0,0.0),Vec3::new(0.0,-1.0,0.0)];
        let camera=if view {Vec3::default()} else {self.camera.position};
        for (i,f) in F.iter().enumerate() {
            let center=(v[f[0]]+v[f[1]]+v[f[2]]+v[f[3]])/4.0;
            if !m.double_sided && N[i].dot(camera-center)<=0.0 {continue}
            // Face-specific variation makes boxes legible at Neon Run's low
            // internal framebuffer resolutions.
            let factor=if m.emissive{1.0}else{[0.86,0.72,0.65,0.88,1.0,0.58][i]};
            let tint=shade(m.color,factor);
            for (a,b,c) in [(f[0],f[1],f[2]),(f[0],f[2],f[3])] {
                if view {self.triangle_view(fb,[v[a],v[b],v[c]],tint)}
                else {self.triangle(fb,v[a],v[b],v[c],Material{color:tint,..m})}
            }
        }
    }
    /// General indexed mesh + position/rotation/scale. Use Mesh::from_obj
    /// to load simple 3D models, or build triangles procedurally.
    pub fn mesh(&mut self,fb:&mut Framebuffer,mesh:&Mesh,transform:Transform,mat:Material) {
        let prepared=PreparedTransform::new(transform);
        self.transformed.resize(mesh.vertices.len(),Vec3::default());
        simd::transform(&mesh.vertices,&mut self.transformed,prepared.matrix());
        for &[a,b,c] in &mesh.triangles {
            let (Some(a),Some(b),Some(c))=(self.transformed.get(a as usize).copied(),
                self.transformed.get(b as usize).copied(),
                self.transformed.get(c as usize).copied()) else {continue};
            self.triangle(fb,a,b,c,mat);
        }
    }
    /// Draw a transformed indexed mesh in camera space (first-person objects).
    /// The world camera's position/rotation and distance fog are intentionally
    /// bypassed; lighting uses the face normal in camera coordinates.
    pub fn mesh_view(&mut self,fb:&mut Framebuffer,mesh:&Mesh,transform:Transform,mat:Material){
        let prepared=PreparedTransform::new(transform);
        self.transformed.resize(mesh.vertices.len(),Vec3::default());
        simd::transform(&mesh.vertices,&mut self.transformed,prepared.matrix());
        for &[a,b,c] in &mesh.triangles {
            let (Some(a),Some(b),Some(c))=(self.transformed.get(a as usize).copied(),
                self.transformed.get(b as usize).copied(),
                self.transformed.get(c as usize).copied()) else {continue};
            let normal=(b-a).cross(c-a);
            if !mat.double_sided && normal.dot(Vec3::default()-(a+b+c)/3.0)<=0.0 {continue;}
            let color=if mat.emissive{mat.color}else{
                shade(mat.color,0.72+0.28*normal.normalized().dot(Vec3::new(-0.45,0.85,-0.45)).abs())
            };
            self.triangle_view(fb,[a,b,c],color);
        }
    }
    /// Generic octahedron primitive; mesh drawing is the default path.
    pub fn octahedron(&mut self,fb:&mut Framebuffer,center:Vec3,r:f32,color:u32){
        let v=[center+Vec3::new(0.0,r,0.0),center+Vec3::new(r,0.0,0.0),center+Vec3::new(0.0,0.0,r),
            center+Vec3::new(-r,0.0,0.0),center+Vec3::new(0.0,0.0,-r),center+Vec3::new(0.0,-r,0.0)];
        for [a,b,c] in [[0,1,2],[0,2,3],[0,3,4],[0,4,1],[5,2,1],[5,3,2],[5,4,3],[5,1,4]] {
            self.triangle(fb,v[a],v[b],v[c],Material::two_sided(color,true));
        }
    }
    pub fn octa_view(&mut self,fb:&mut Framebuffer,c:Vec3,r:f32,color:u32){
        let a=c+Vec3::new(0.0,r,0.0);let b=c+Vec3::new(-r,-r,0.0);let d=c+Vec3::new(r,-r,0.0);
        self.triangle_view(fb,[a,b,d],color);
        self.triangle_view(fb,[a,b,c+Vec3::new(0.0,0.0,r*2.0)],color);
        self.triangle_view(fb,[a,d,c+Vec3::new(0.0,0.0,r*2.0)],color);
    }
    fn clip_triangle(&mut self,fb:&mut Framebuffer,pts:[Vec3;3],color:u32){
        if self.width==0||self.height==0{return}
        let near=self.camera.near.max(0.001);
        let mut output=[Vec3::default();4];let mut n=0usize;
        for i in 0..3 {
            let a=pts[i];let b=pts[(i+1)%3];
            if !(a.z.is_finite() && b.z.is_finite()){return}
            let ia=a.z>=near;let ib=b.z>=near;
            if ia {output[n]=a;n+=1;}
            if ia!=ib {
                let t=(near-a.z)/(b.z-a.z);
                output[n]=a+(b-a)*t;n+=1;
            }
        }
        if n<3{return}
        // Conservative homogeneous viewport rejection before three reciprocal
        // depths / triangle and before any pixel work. Clipped vertices all
        // have z>=near, so x/z and y/z screen bounds are valid here.
        let aspect_x=(self.width as f32*0.5)/self.focal;
        let upper_y=(self.viewport_height as f32*0.5-self.y_offset as f32)/self.focal;
        let lower_y=(self.viewport_height as f32*0.5
            -self.y_offset as f32-self.height as f32)/self.focal;
        let visible=&output[..n];
        if visible.iter().all(|v|v.x< -aspect_x*v.z)
            ||visible.iter().all(|v|v.x>aspect_x*v.z)
            ||visible.iter().all(|v|v.y>upper_y*v.z)
            ||visible.iter().all(|v|v.y<lower_y*v.z){return}
        for i in 1..n-1 {self.rasterize(fb,output[0],output[i],output[i+1],color)}
    }
    fn rasterize(&mut self,fb:&mut Framebuffer,a:Vec3,b:Vec3,c:Vec3,color:u32){
        let project=|v:Vec3|Screen{x:self.width as f32*0.5+v.x/v.z*self.focal,
            y:self.viewport_height as f32*0.5-v.y/v.z*self.focal-self.y_offset as f32,invz:1.0/v.z};
        let a=project(a);let b=project(b);let c=project(c);
        self.rasterize_projected(fb,[a,b,c],color);
    }
    // Shared projected-edge path: direct drawing, CPU capture, and replay
    // produce identical triangle coverage and reciprocal-depth interpolation.
    fn rasterize_projected(&mut self,fb:&mut Framebuffer,
        [a,b,c]:[Screen;3],color:u32){
        if ![a.x,a.y,b.x,b.y,c.x,c.y].iter().all(|x|x.is_finite()){return}
        let area=edge(a.x,a.y,b.x,b.y,c.x,c.y);
        if area.abs()<0.02 {return}
        let xmin=(a.x.min(b.x).min(c.x).max(0.0)).floor() as usize;
        let xmax=(a.x.max(b.x).max(c.x).min(self.width as f32)).ceil() as usize;
        let ymin=(a.y.min(b.y).min(c.y).max(0.0)).floor() as usize;
        let ymax=(a.y.max(b.y).max(c.y).min(self.height as f32)).ceil() as usize;
        if xmin>=xmax||ymin>=ymax{return}
        self.stats.triangles=self.stats.triangles.saturating_add(1);
        if self.software_capture {
            self.software_frame.triangles.push(SoftwareTriangle{vertices:[a,b,c],color});
            return;
        }
        if self.gpu_capture{
            let near=self.camera.near.max(0.001);
            let make=|v:Screen| -> GpuVertex{
                GpuVertex{position:[2.0*v.x/self.width as f32-1.0,
                    1.0-2.0*(v.y+self.y_offset as f32)/self.viewport_height as f32,
                    (near*v.invz).clamp(0.0,1.0)],
                    rgba8:((color>>16)&255) | (color&0x0000_ff00) |
                        ((color&255)<<16) | 0xff00_0000}
            };
            self.gpu_frame.vertices.extend([make(a),make(b),make(c)]);
            return;
        }
        let sign=if area>0.0{1.0}else{-1.0};let inv_area=1.0/area;
        let sx=xmin as f32+0.5;let sy=ymin as f32+0.5;
        let mut e0_row=edge(b.x,b.y,c.x,c.y,sx,sy);
        let mut e1_row=edge(c.x,c.y,a.x,a.y,sx,sy);
        let mut e2_row=edge(a.x,a.y,b.x,b.y,sx,sy);
        let ex0=c.y-b.y;let ey0=b.x-c.x;
        let ex1=a.y-c.y;let ey1=c.x-a.x;
        let ex2=b.y-a.y;let ey2=a.x-b.x;
        let dzdx=(ex0*a.invz+ex1*b.invz+ex2*c.invz)*inv_area;
        let dzdy=(ey0*a.invz+ey1*b.invz+ey2*c.invz)*inv_area;
        let mut z_row=(e0_row*a.invz+e1_row*b.invz+e2_row*c.invz)*inv_area;
        let width=self.width;
        let depth=&mut self.depth;
        let pixels=&mut fb.pixels;
        let mut painted=0u32;
        for y in ymin..ymax {
            let row=y*width;
            painted=painted.saturating_add(simd::raster_row(
                &mut depth[row..row+width],&mut pixels[row..row+width],xmin,xmax,color,
                simd::Edges{e0:e0_row,e1:e1_row,e2:e2_row,z:z_row,
                    dx0:ex0,dx1:ex1,dx2:ex2,dz:dzdx,sign}));
            e0_row+=ey0;e1_row+=ey1;e2_row+=ey2;z_row+=dzdy;
        }
        self.stats.pixels=self.stats.pixels.saturating_add(painted);
    }
}
#[inline] fn edge(ax:f32,ay:f32,bx:f32,by:f32,x:f32,y:f32)->f32 {(x-ax)*(by-ay)-(y-ay)*(bx-ax)}
#[inline] fn rgb(r:u32,g:u32,b:u32)->u32 {0xff000000|(r.min(255)<<16)|(g.min(255)<<8)|b.min(255)}
#[inline] fn shade(color:u32,f:f32)->u32 {
    rgb((((color>>16)&255) as f32*f) as u32,(((color>>8)&255) as f32*f) as u32,((color&255) as f32*f) as u32)
}
#[inline] fn mix(a:u32,b:u32,t:f32)->u32 {
    let k=|shift| {
        let first=((a>>shift)&255u32) as f32;
        let second=((b>>shift)&255u32) as f32;
        (first*(1.0-t)+second*t) as u32
    };
    rgb(k(16),k(8),k(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn vertical_bands_match_whole_frame(){
        let cam=Camera::new(Vec3::default(),Vec3::new(0.0,0.0,1.0),Vec3::new(1.0,0.0,0.0));
        let a=Vec3::new(-0.6,-0.8,3.0);
        let b=Vec3::new(0.8,-0.6,3.0);
        let c=Vec3::new(0.2,0.9,3.0);
        let mut whole=Framebuffer::from_color(80,60,0);
        let mut raster=Rasterizer::new();
        raster.begin_frame(&whole,cam);
        raster.triangle_view(&mut whole,[a,b,c],0xff78aaee);
        for &(start,height) in &[(0,19),(19,17),(36,24)] {
            let mut tile=Framebuffer::from_color(80,height as u32,0);
            let mut r=Rasterizer::new();
            r.begin_frame_band(&tile,cam,60,start);
            r.triangle_view(&mut tile,[a,b,c],0xff78aaee);
            let expected=&whole.pixels[start*80..(start+height)*80];
            assert_eq!(tile.pixels,expected);
        }
    }
    #[test] fn configurable_distance_and_conservative_frustum() {
        let mut r=Rasterizer::new();
        let f=Framebuffer::from_color(200,100,0);
        let camera=Camera::new(Vec3::default(),Vec3::new(0.,0.,1.),Vec3::new(1.,0.,0.));
        r.begin_frame(&f,camera);
        assert!(r.sphere_visible(Vec3::new(0.,0.,3.),1.));
        assert!(!r.sphere_visible(Vec3::new(80.,0.,3.),1.));
        assert!(r.sphere_visible(Vec3::new(0.,0.,0.),1.));
        assert!(!r.sphere_visible(Vec3::new(0.,0.,-4.),1.));
        assert!(!r.sphere_visible(Vec3::new(0.,0.,200.),1.));
        r.max_distance=None;
        assert!(r.sphere_visible(Vec3::new(0.,0.,200.),1.));
    }
    #[test] fn math_and_intersections(){
        let a=Vec3::new(1.0,0.0,0.0);let b=Vec3::new(0.0,0.0,1.0);
        assert_eq!(a.cross(b),Vec3::new(0.0,-1.0,0.0));
        let box3=Aabb::new(0.0,0.0,0.0,1.0,2.0,1.0);
        assert_eq!(box3.ray_hit(Vec3::new(-2.0,1.0,0.5),a),Some(2.0));
        assert!(box3.ray_hit(Vec3::new(-2.0,3.0,0.5),a).is_none());
    }
    #[test] fn mesh_obj_parser(){
        let m=Mesh::from_obj("v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf 1/1 2/2 3/3 4/4\n").unwrap();
        assert_eq!(m.vertices.len(),4);assert_eq!(m.triangles.len(),2);
        assert!(Mesh::from_obj("f 1 2 3").is_err());
        assert_eq!(Mesh::octahedron().triangles.len(),8);
    }
    #[test] fn transform_and_mesh_render() {
        let mut transform=Transform::at_scale(Vec3::new(0.0,0.0,4.0),1.0);
        transform.rotation.y=std::f32::consts::FRAC_PI_2;
        let result=transform.point(Vec3::new(1.0,0.0,0.0));
        assert!((result.z-3.0).abs()<0.0001);
        let mut fb=Framebuffer::from_color(64,64,0);
        let mut renderer=Rasterizer::new();
        renderer.begin_frame(&fb,Camera::new(Vec3::default(),Vec3::new(0.0,0.0,1.0),Vec3::new(1.0,0.0,0.0)));
        renderer.mesh(&mut fb,&Mesh::cube(),transform,Material::lit(0xff55cc88));
        assert!(renderer.stats.triangles>0);
        assert!(renderer.stats.pixels>0);
    }
    #[test] fn simd_indexed_transform_matches_prepared_transform(){
        let mut t=Transform::at_scale(Vec3::new(2.4,-0.8,3.7),1.87);
        t.rotation=Vec3::new(0.43,-1.21,0.36);
        let p=PreparedTransform::new(t);
        let src=(0..101).map(|i|Vec3::new(i as f32*0.13-5.0,
            (i as f32*0.38).sin(),(i as f32*0.4).cos())).collect::<Vec<_>>();
        let mut dst=vec![Vec3::default();src.len()];
        simd::transform(&src,&mut dst,p.matrix());
        for (&src,&got) in src.iter().zip(dst.iter()){
            let expected=p.point(src);
            assert!((expected.x-got.x).abs()<0.00002);
            assert!((expected.y-got.y).abs()<0.00002);
            assert!((expected.z-got.z).abs()<0.00002);
        }
    }
    #[test] fn depth_near_clip_and_resize(){
        let mut fb=Framebuffer::from_color(80,60,0);
        let mut r=Rasterizer::new();
        let cam=Camera::new(Vec3::default(),Vec3::new(0.0,0.0,1.0),Vec3::new(1.0,0.0,0.0));
        r.begin_frame(&fb,cam);
        let tri=|depth|[Vec3::new(-0.5,-0.5,depth),Vec3::new(0.5,-0.5,depth),Vec3::new(0.0,0.5,depth)];
        let far=tri(4.0);let near=tri(2.0);
        r.triangle_view(&mut fb,far,0xff003355);
        r.triangle_view(&mut fb,near,0xff33ff55);
        assert_eq!(fb.pixels[30*80+40],0xff33ff55);
        let before=r.stats.pixels;
        let intersect=[Vec3::new(-0.2,-0.2,0.01),Vec3::new(0.3,-0.1,1.0),Vec3::new(0.0,0.2,1.0)];
        r.triangle_view(&mut fb,intersect,0xffbbeeff);
        assert!(r.stats.pixels>before);
        fb.resize_and_clear(40,30,0);
        r.begin_frame(&fb,cam);
        assert_eq!(r.depth.len(),1200);
    }
}

/// Multi-material OBJ and MTL importer, and convenient scene/model drawing.
pub mod model;
pub use model::{Model,ModelPart};

#[cfg(test)] mod gpu_capture_generation_tests {
    use super::*;
    #[test] fn distinct_captures_have_distinct_ids(){
        let mut raster=Rasterizer::new();
        raster.set_gpu_capture(true);
        let first=raster.take_gpu_frame();
        raster.set_gpu_capture(true);
        let second=raster.take_gpu_frame();
        assert_ne!(first.generation,0);
        assert_ne!(first.generation,second.generation);
        raster.recycle_gpu_frame(first);
    }
}

#[cfg(test)] mod gpu_capture_tests {
    use super::*;
    #[test] fn gpu_capture_skips_software_raster_and_preserves_geometry(){
        let mut raster=Rasterizer::new();
        let mut fb=Framebuffer::from_color(200,160,0xff000000);
        let mut camera=Camera::new(Vec3::default(),Vec3::new(0.0,0.0,1.0),Vec3::new(1.0,0.0,0.0));
        camera.near=0.1;
        raster.set_gpu_capture(true);
        raster.begin_frame(&fb,camera);
        raster.triangle_view(&mut fb,[Vec3::new(-1.0,-1.0,4.0),Vec3::new(0.0,1.0,4.0),Vec3::new(1.0,-1.0,4.0)],0xffff00ff);
        raster.clear_depth();
        let scene=raster.take_gpu_frame();
        assert_eq!(scene.vertices.len(),3);
        assert_eq!(scene.depth_clears,vec![3]);
        assert!(scene.vertices.iter().all(|v|v.position.iter().all(|x|x.is_finite())));
        assert_eq!(std::mem::size_of::<GpuVertex>(),16);
        assert!(scene.vertices.iter().all(|v|v.rgba8==0xffff_00ff));
        assert!(fb.pixels.iter().all(|&p|p==0xff000000));
    }
}

#[cfg(test)] mod gpu_allocation_tests {
    use super::*;
    #[test] fn rgba8_byte_order_preserves_source_channels() {
        let packed_color=|color:u32| ((color>>16)&255) | (color&0x0000_ff00)
            | ((color&255)<<16) | 0xff00_0000;
        assert_eq!(packed_color(0xff12_3456).to_le_bytes(),[0x12,0x34,0x56,0xff]);
        assert_eq!(packed_color(0xff00_0000).to_le_bytes(),[0,0,0,255]);
        assert_eq!(packed_color(0xffff_ffff).to_le_bytes(),[255,255,255,255]);
    }
    #[test] fn reuses_gpu_vertex_and_depth_boundary_allocations() {
        let mut renderer=Rasterizer::new();
        renderer.set_gpu_capture(true);
        renderer.gpu_frame.vertices.reserve(9000);
        renderer.gpu_frame.depth_clears.reserve(10);
        renderer.gpu_frame.depth_clears.push(9);
        let old=renderer.take_gpu_frame();
        assert!(old.vertices.capacity()>=9000);
        renderer.recycle_gpu_frame(old);
        renderer.set_gpu_capture(true);
        assert!(renderer.gpu_frame.vertices.capacity()>=9000);
        assert!(renderer.gpu_frame.depth_clears.is_empty());
    }
}

#[cfg(test)] mod software_command_tests {
    use super::*;

    #[test]
    fn replayed_bands_match_direct_rasterization_and_weapon_depth_reset(){
        let camera=Camera::new(Vec3::default(),Vec3::new(0.0,0.0,1.0),
            Vec3::new(1.0,0.0,0.0));
        let mut whole=Framebuffer::from_color(121,79,0xff091827);
        let mut direct=Rasterizer::new();
        direct.begin_frame(&whole,camera);
        let far=[Vec3::new(-1.4,-0.9,5.0),Vec3::new(1.5,-0.9,5.0),
            Vec3::new(0.0,1.2,5.0)];
        let near=[Vec3::new(-0.3,-0.7,1.5),Vec3::new(0.7,-0.4,2.3),
            Vec3::new(-0.2,0.8,2.0)];
        let weapon=[Vec3::new(-0.6,-0.5,4.0),Vec3::new(0.7,-0.5,4.0),
            Vec3::new(0.2,0.5,4.0)];
        direct.triangle_view(&mut whole,far,0xff556677);
        direct.triangle_view(&mut whole,near,0xff22cc88);
        direct.clear_depth();
        direct.triangle_view(&mut whole,weapon,0xffeebb44);
        let mut recording=Rasterizer::new();
        let target=Framebuffer::from_color(121,79,0xff091827);
        recording.set_software_capture(true);
        recording.begin_frame(&target,camera);
        recording.triangle_view(&mut Framebuffer::from_color(121,79,0),far,0xff556677);
        recording.triangle_view(&mut Framebuffer::from_color(121,79,0),near,0xff22cc88);
        recording.clear_depth();
        recording.triangle_view(&mut Framebuffer::from_color(121,79,0),weapon,0xffeebb44);
        let mut commands=recording.take_software_frame();
        assert_eq!(commands.depth_clears.len(),1);
        assert!(commands.len()>=3);
        commands.prepare_bands(79,4);
        let mut assembled=Framebuffer::from_color(121,79,0xff091827);
        let mut raster=Rasterizer::new();
        for i in 0..4usize {
            let start=79*i/4;
            let end=79*(i+1)/4;
            let mut band=Framebuffer::from_color(121,(end-start) as u32,0xff091827);
            raster.draw_software_frame_band(&mut band,&commands,79,start);
            assembled.pixels[start*121..end*121].copy_from_slice(&band.pixels);
        }
        assert_eq!(whole.pixels,assembled.pixels);
        recording.recycle_software_frame(commands);
        assert!(recording.software_frame.triangles.capacity()>=3);
    }

    #[test]
    fn software_commands_with_no_binning_still_replay(){
        let camera=Camera::new(Vec3::default(),Vec3::new(0.0,0.0,1.0),
            Vec3::new(1.0,0.0,0.0));
        let mut fb=Framebuffer::from_color(41,23,0);
        let mut capture=Rasterizer::new();
        capture.set_software_capture(true);
        capture.begin_frame(&fb,camera);
        capture.triangle_view(&mut fb,[Vec3::new(-1.0,-1.0,3.0),
            Vec3::new(1.0,-1.0,3.0),Vec3::new(0.0,1.0,3.0)],0xffbbaacc);
        let commands=capture.take_software_frame();
        let mut playback=Rasterizer::new();
        playback.draw_software_frame_band(&mut fb,&commands,23,0);
        assert!(fb.pixels.iter().any(|p|*p==0xffbbaacc));
    }
}

#[cfg(test)] mod sphere_instance_tests {
    use super::*;
    #[test] fn gpu_instances_use_32_byte_packed_layout() {
        assert_eq!(std::mem::size_of::<GpuSphereInstance>(),32);
        let mut raster=Rasterizer::new();
        let fb=Framebuffer::new(256,256);
        let cam=Camera::new(Vec3::new(0.,0.,-10.),Vec3::new(0.,0.,1.),Vec3::new(1.,0.,0.));
        raster.set_gpu_capture(true);raster.begin_frame(&fb,cam);
        assert!(raster.capture_sphere(Vec3::default(),1.0,0xff123456));
        let frame=raster.take_gpu_frame();
        assert_eq!(frame.spheres.len(),1);
        assert!(frame.vertices.is_empty());
        assert_eq!(frame.spheres[0].rgba8.to_le_bytes(),[0x12,0x34,0x56,0xff]);
        assert_eq!(frame.spheres[0].center_depth[2],10.0);
    }
    #[test] fn instanced_bond_uses_camera_space_and_compact_layout() {
        assert_eq!(std::mem::size_of::<GpuBondInstance>(),52);
        let mut raster=Rasterizer::new();
        let fb=Framebuffer::new(120,80);
        let cam=Camera::new(Vec3::new(0.,0.,-10.),Vec3::new(0.,0.,1.),Vec3::new(1.,0.,0.));
        raster.set_gpu_capture(true);raster.begin_frame(&fb,cam);
        assert!(raster.capture_bond(Vec3::new(-1.,0.,0.),Vec3::new(1.,0.,0.),0.1,0xffc0ff44));
        let frame=raster.take_gpu_frame();
        assert_eq!(frame.bonds.len(),1);
        assert!(frame.vertices.is_empty());
        assert_eq!(frame.bonds[0].start_radius[2],10.0);
        assert_eq!(frame.bonds[0].end_near[2],10.0);
        assert_eq!(frame.bonds[0].rgba8.to_le_bytes(),[0xc0,0xff,0x44,0xff]);
    }
    #[test] fn near_plane_intersection_falls_back_to_clipped_triangles() {
        let mut raster=Rasterizer::new();
        let fb=Framebuffer::new(64,64);
        let cam=Camera::new(Vec3::default(),Vec3::new(0.,0.,1.),Vec3::new(1.,0.,0.));
        raster.set_gpu_capture(true);raster.begin_frame(&fb,cam);
        assert!(!raster.capture_sphere(Vec3::new(0.,0.,0.1),0.3,0xffffffff));
        assert!(raster.take_gpu_frame().spheres.is_empty());
    }
}
