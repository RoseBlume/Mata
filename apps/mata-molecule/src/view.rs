use crate::{Molecule,Vec3};
use crate::draw::{self,Rect};
use mata_win::Framebuffer;
use mata_molecule::vsepr_centers;
use crate::ribbon::{self,BackboneNode,Secondary};
use crate::lod::SphereLods;
use crate::render_budget::{RenderPreset,representative_mask};
use crate::electron;
use crate::orbitals::OrbitalSurfaces;
use mata_3d::{Camera,Mesh,Transform,Material,Rasterizer,GpuFrame,parallel::ParallelSoftware};
use std::hash::{Hash,Hasher};
use std::time::Instant;
use std::f32::consts::{PI,TAU};

fn neighbors(m:&Molecule,node:usize)->Vec<usize>{m.bonds.iter().filter_map(|b|if b.a==node{Some(b.b)}else if b.b==node{Some(b.a)}else{None}).collect()}
fn xy_positions(m:&Molecule)->Vec<(f32,f32)>{
 let n=m.atoms.len();let mut coords=vec![(0.0,0.0);n];if n==0{return coords;}
 let spread=m.atoms.iter().map(|a|a.pos.x.abs()+a.pos.y.abs()).fold(0.0f32,f32::max);
 if (m.from_file||m.bonds.iter().any(|b|b.order==4)) && spread>0.01 {
     return m.atoms.iter().map(|a|(a.pos.x,a.pos.y)).collect();
 }
 let root=(0..n).filter(|&i|m.atoms[i].symbol!="H").max_by_key(|&i|m.degree(i)).unwrap_or(0);
 let mut visited=vec![false;n];visited[root]=true;let mut queue=std::collections::VecDeque::new();queue.push_back((root,None::<usize>));
 while let Some((cur,parent))=queue.pop_front(){
   let mut children:Vec<_>=neighbors(m,cur).into_iter().filter(|&i|!visited[i]).collect();children.sort_by_key(|&i|m.atoms[i].symbol=="H");
   let deg=children.len();if deg==0{continue;}
   let mut angles=Vec::new();
   if let Some(p)=parent {
     let back=(coords[p].1-coords[cur].1).atan2(coords[p].0-coords[cur].0);
     if deg==1 {let target=if m.degree(cur)==2 && m.lone_pairs(cur)>0{112.0_f32.to_radians()}else if m.bonds.iter().any(|b|(b.a==cur||b.b==cur)&&b.order>=2){PI}else{2.05};angles.push(back+target);}
     else {let sweep=if deg==2{1.95}else{2.85};let opposite=back+PI;for k in 0..deg{angles.push(opposite-sweep*0.5+sweep*(k as f32/((deg-1).max(1) as f32)));}}
   }else if deg==1 {angles.push(0.0)}
   else if deg==2 {
     let linear=m.lone_pairs(cur)==0&&m.bonds.iter().any(|b|(b.a==cur||b.b==cur)&&b.order>=2);
     if linear {angles.extend([0.0,PI]);}else{let half=if m.lone_pairs(cur)>0{52.5_f32.to_radians()}else{60.0_f32.to_radians()};angles.extend([-half,half]);}
   } else {for k in 0..deg{angles.push(-PI*0.5+TAU*k as f32/deg as f32);}}
   for (i,ch) in children.into_iter().enumerate(){let length=if m.atoms[ch].symbol=="H"{0.95}else{1.4};let angle=angles[i];coords[ch]=(coords[cur].0+length*angle.cos(),coords[cur].1+length*angle.sin());visited[ch]=true;queue.push_back((ch,Some(cur)));}
 }
 // Non-tree ring edges and disconnected fragments can overlap; preserved imported
 // graph topology is drawn explicitly, while coordinates are only a layout.
 coords
}

pub fn lewis(fb:&mut Framebuffer,m:&Molecule,rect:Rect){
 if m.atoms.len()>750 {
     draw::text(fb,rect.x+16,rect.y+28,"MACROMOLECULE: 2D LEWIS VIEW LIMITED",2,draw::GOLD);
     draw::text(fb,rect.x+16,rect.y+55,"USE 3D MODEL; IMPORTED BONDS MAY BE INFERRED",1,draw::MUTED);
     return;
 }
 if rect.w<20||rect.h<20{return;}let original=xy_positions(m);if original.is_empty(){return;}
 let (mut minx,mut maxx,mut miny,mut maxy)=(f32::INFINITY,f32::NEG_INFINITY,f32::INFINITY,f32::NEG_INFINITY);
 for (x,y) in &original{minx=minx.min(*x);maxx=maxx.max(*x);miny=miny.min(*y);maxy=maxy.max(*y);}
 let range_x=(maxx-minx).max(0.1);let range_y=(maxy-miny).max(0.1);
 let scale=((rect.w-120).max(15) as f32/range_x).min((rect.h-115).max(15) as f32/range_y).min(125.0).max(10.0);
 let cx=rect.x as f32+rect.w as f32*0.5;let cy=rect.y as f32+rect.h as f32*0.50;
 let coords=original.iter().map(|&(x,y)|(cx+(x-(minx+maxx)*0.5)*scale,cy+(y-(miny+maxy)*0.5)*scale)).collect::<Vec<_>>();
 let labeled_centers=vsepr_centers(m).into_iter().map(|c|c.atom_index).collect::<std::collections::HashSet<_>>();
 // Bonds first, atom labels second.
 for b in &m.bonds{let (ax,ay)=coords[b.a];let (bx,by)=coords[b.b];
   let dx=bx-ax;let dy=by-ay;let len=(dx*dx+dy*dy).sqrt().max(0.01);let ux=dx/len;let uy=dy/len;
   let margin=(len*0.14).min(19.0);let x1=ax+ux*margin;let y1=ay+uy*margin;let x2=bx-ux*margin;let y2=by-uy*margin;
   let vx=-uy;let vy=ux;let n=if b.order==4{1}else{b.order as usize};
   for k in 0..n {let offset=(k as f32-(n as f32-1.0)/2.0)*7.0;
     draw::line(fb,x1+vx*offset,y1+vy*offset,x2+vx*offset,y2+vy*offset,2,0xffb7c8db);
   }
 }
 // Aromatic bonds are marked by a small dotted ring, rather than implying
 // alternating localized double bonds with a specific Kekule assignment.
 let ar=m.bonds.iter().filter(|b|b.order==4).count();
 if ar>=5 {let ids=m.atoms.iter().enumerate().filter(|(i,a)|a.symbol!="H"&&m.bonds.iter().any(|b|b.order==4&&(b.a==*i||b.b==*i))).map(|(i,_)|i).collect::<Vec<_>>();
   if !ids.is_empty(){let (x,y)=ids.iter().fold((0.0,0.0),|(x,y),&i|(x+coords[i].0,y+coords[i].1));let (x,y)=(x/ids.len() as f32,y/ids.len() as f32);let radius=ids.iter().map(|&i|((coords[i].0-x).powi(2)+(coords[i].1-y).powi(2)).sqrt()).fold(f32::INFINITY,f32::min)*0.53;
     for i in 0..84{if (i/4)%2!=0{continue;}let a=i as f32*TAU/84.0;let b=(i+1) as f32*TAU/84.0;draw::line(fb,x+radius*a.cos(),y+radius*a.sin(),x+radius*b.cos(),y+radius*b.sin(),1,draw::GOLD);}
   }
 }
 for (i,a) in m.atoms.iter().enumerate(){let (x,y)=coords[i];let x=x as i32;let y=y as i32;
   draw::circle(fb,x,y,17,draw::PANEL);
   let color=if m.estimated_atoms.binary_search(&i).is_ok(){0xffffaa58}else{draw::element_color(&a.symbol)};draw::circle(fb,x,y,15,0xff1c3047);
   draw::centered(fb,x,y-7,&a.symbol,2,color);
   if labeled_centers.contains(&i) {draw::text(fb,x+13,y+13,&(i+1).to_string(),1,draw::BLUE);}
   if let Some(site)=&a.site {
     if m.atoms.len()<60 {draw::centered(fb,x,y+21,&site.atom_name,1,draw::MUTED);}
   }
   if a.charge!=0{let t=if a.charge==1{"+".to_string()}else if a.charge==-1{"-".to_string()}else{format!("{}{}",a.charge.abs(),if a.charge>0{'+'}else{'-'})};draw::text(fb,x+14,y-22,&t,1,draw::GOLD);}
   let lp=m.lone_pairs(i);if lp>0{
     let neigh=neighbors(m,i);let mut angles=Vec::new();for &id in &neigh{let (xx,yy)=coords[id];angles.push((yy-y as f32).atan2(xx-x as f32));}
     let mut positions=Vec::<f32>::new();
     for _ in 0..lp.min(4) {let mut best=(f32::NEG_INFINITY,0.0);for k in 0..32 {let theta=k as f32*TAU/32.0;
       let nearest_bond=angles.iter().map(|a|{let z=(theta-*a).abs().rem_euclid(TAU);z.min(TAU-z)}).fold(PI,f32::min);
       let nearest_pair=positions.iter().map(|a:&f32|{let z=(theta-*a).abs().rem_euclid(TAU);z.min(TAU-z)}).fold(PI,f32::min);
       let score=nearest_bond.min(nearest_pair)*1.0;
       if score>best.0{best=(score,theta);}
     }
     positions.push(best.1);
     }
     for angle in positions {let xx=x as f32+angle.cos()*28.0;let yy=y as f32+angle.sin()*28.0;let tx=-angle.sin()*4.0;let ty=angle.cos()*4.0;
       draw::circle(fb,(xx+tx) as i32,(yy+ty) as i32,2,draw::GOLD);draw::circle(fb,(xx-tx) as i32,(yy-ty) as i32,2,draw::GOLD);
     }
   }
 }
}

fn cylinder(a:Vec3,b:Vec3,r:f32)->Mesh {
 let dir=(b-a).normalized();let normal=if dir.y.abs()<0.9{Vec3::new(0.0,1.0,0.0)}else{Vec3::new(1.0,0.0,0.0)};
 let u=dir.cross(normal).normalized()*r;let v=dir.cross(u).normalized()*r;
 let mut vertices=Vec::new();let mut triangles=Vec::new();let steps=12;
 for i in 0..steps {let theta=i as f32*TAU/steps as f32;let o=u*theta.cos()+v*theta.sin();vertices.push(a+o);vertices.push(b+o);}
 for i in 0..steps {let p=(2*i) as u32;let q=(2*((i+1)%steps)) as u32;triangles.push([p,p+1,q]);triangles.push([q,p+1,q+1]);}
 Mesh{vertices,triangles}
}
pub(super) fn rotate(p:Vec3,ax:f32,ay:f32)->Vec3{
 let (sx,cx)=ax.sin_cos();let (sy,cy)=ay.sin_cos();let first=Vec3::new(p.x,p.y*cx-p.z*sx,p.y*sx+p.z*cx);
 Vec3::new(first.x*cy+first.z*sy,first.y,-first.x*sy+first.z*cy)
}
fn radius(s:&str)->f32{match s{"H"=>0.205,"C"=>0.32,"N"=>0.325,"O"=>0.32,"F"=>0.31,"S"=>0.38,"P"=>0.39,"Cl"|"Br"|"I"=>0.38,_=>0.35}}

/// Protein structures are displayed as a lightweight C-alpha trace, retaining
/// the complete backbone instead of clipping away all atoms beyond index 2000.
pub fn is_protein(m:&Molecule)->bool {
    m.atoms.len()>750 && m.atoms.iter().filter(|a|a.site.as_ref().is_some_and(|s|s.atom_name.trim()=="CA" && !s.hetero)).take(31).count()>30
}

/// Interactive protein representation. AllAtoms draws every available atom and
/// source/inferred covalent bond, rather than replacing the structure with a trace.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum DetailMode { Backbone, AllAtoms, Ribbon }

/// An import has a recognizable polymer backbone rather than arbitrary atoms.
pub fn supports_ribbon(m:&Molecule)->bool {
    m.atoms.iter().filter(|a|a.site.as_ref().is_some_and(|s|
        s.atom_name.trim()=="CA" && !s.hetero &&
        mata_molecule::protein_inspect::one_letter(&s.residue_name)!='X')).take(4).count()>=4
}

/// Biochemical residue classes: distinct and predictable for chain inspection.
/// These colors describe amino-acid properties, NOT atomic elements.
pub fn residue_class(name:&str)->(&'static str,u32) {
    match name.trim().to_ascii_uppercase().as_str() {
        "ASP"|"GLU" => ("ACIDIC",0xffff7777),
        "LYS"|"ARG"|"HIS" => ("BASIC",0xff6ba9ff),
        "SER"|"THR"|"ASN"|"GLN"|"CYS" => ("POLAR",0xff57e6c2),
        "PHE"|"TRP"|"TYR" => ("AROMATIC",0xffd58aff),
        "GLY"|"PRO" => ("SPECIAL",0xffffd36c),
        "ALA"|"VAL"|"LEU"|"ILE"|"MET"|"MSE" => ("NONPOLAR",0xffffab60),
        _ => ("OTHER",0xffa2afbf),
    }
}

pub fn residue_legend()->[(&'static str,u32);7] {
    [residue_class("ALA"),residue_class("SER"),residue_class("ASP"),
     residue_class("LYS"),residue_class("PHE"),residue_class("GLY"),residue_class("UNK")]
}

/// Moving the eye parallel to the image plane yields screen-space panning,
/// independent of the molecular coordinate scale and existing rasterizer's
/// world-unit cutoff. Values are fractions of the viewport height.
fn pan_camera(distance:f32,pan:(f32,f32))->Camera {
    let mut camera=Camera::new(
        Vec3::new(-pan.0*distance/0.88,-pan.1*distance/0.88,-distance),
        Vec3::new(0.0,0.0,1.0),Vec3::new(1.0,0.0,0.0));
    camera.focal_scale=0.88;
    camera
}

pub fn chain_color(chain:&str)->u32{
    const PALETTE:[u32;8]=[0xff47c7ff,0xffffad58,0xffac87ff,0xff4fe5b4,0xffec7191,0xffffd45a,0xff83b5f9,0xffdf9ff0];
    let mut hash=0u32;
    for byte in chain.bytes(){hash=hash.wrapping_mul(37).wrapping_add(byte as u32);}
    PALETTE[hash as usize%PALETTE.len()]
}
/// Fit molecule geometry into the 65-world-unit triangle-culling budget used
/// by older versions of mata-3d. A uniform scale applied to BOTH the camera
/// and the atoms/bonds preserves the perspective projection, so imported PDB
/// coordinates do not need to be altered. Leave headroom for mesh radii.
///
/// This works with the unmodified Rasterizer (no max_draw_distance field).
fn view_scale(distance:f32,extent:f32)->f32 {
    let outer=distance+extent+2.0;
    if !outer.is_finite() || outer<=0.0 {return 1.0;}
    (56.0/outer).min(1.0)
}

/// Camera-to-scene distance for the current zoom. No subjective near/far
/// limits are applied. f64 avoids overflowing a f32 before uniform scene
/// scaling brings distant molecular structures into mata-3d's render range.
fn camera_distance(base_distance:f32, zoom:f32)->f32 {
    let z=if zoom.is_finite() && zoom>0.0 {zoom as f64} else {1.0};
    ((base_distance as f64 / z).max(f32::MIN_POSITIVE as f64)
        .min(f32::MAX as f64 / 32.0)) as f32
}

/// Keep the fog usable when coordinates were uniformly scaled for preview.
fn configure_view_range(raster:&mut Rasterizer,distance:f32,extent:f32,protein:bool) {
    if protein {
        raster.fog_start = (distance - extent).max(0.0);
        raster.fog_end = distance + extent;
        raster.fog_max = 0.20;
    } else {
        raster.fog_start = 18.0;
        raster.fog_end = 63.0;
        raster.fog_max = 0.65;
    }
}

/// Render at native resolution when cheap, lower during interaction / overload.
/// The UI frame is always native resolution; only this 3D render surface is
/// scaled, so text and controls remain crisp.
fn render_dimensions(rect:Rect,quality:f32,moving:bool)->(u32,u32){
    let fraction=if moving {0.65}
        else if quality<0.43{0.66}
        else if quality<0.65{0.78}
        else if quality<0.86{0.90}
        else{1.0};
    let w=((rect.w.max(1) as f32*fraction).round() as u32).clamp(1,4096);
    let h=((rect.h.max(1) as f32*fraction).round() as u32).clamp(1,2304);
    (w,h)
}
fn blit_model(fb:&mut Framebuffer,viewport:&Framebuffer,rect:Rect){
    if rect.w<=0 || rect.h<=0 || viewport.width==0 || viewport.height==0{return;}
    let (vw,vh)=(viewport.width as usize,viewport.height as usize);
    let (fw,fh)=(fb.width as usize,fb.height as usize);
    let left=rect.x.max(0) as usize;
    let top=rect.y.max(0) as usize;
    let right=(i64::from(rect.x)+i64::from(rect.w)).max(0).min(fw as i64) as usize;
    let bottom=(i64::from(rect.y)+i64::from(rect.h)).max(0).min(fh as i64) as usize;
    if right<=left || bottom<=top{return;}
    let rect_w=rect.w as usize;let rect_h=rect.h as usize;
    // Fast native-resolution path: one memcpy for every visible scanline.
    if vw==rect_w && vh==rect_h {
        let width=right-left;
        for y in top..bottom {
            let sy=(y as i64-rect.y as i64) as usize;
            let sx=(left as i64-rect.x as i64) as usize;
            let src=sy*vw+sx;
            let dst=y*fw+left;
            fb.pixels[dst..dst+width].copy_from_slice(&viewport.pixels[src..src+width]);
        }
        return;
    }
    // Integer-based nearest-neighbor scaling avoids floating-point work per
    // destination pixel and keeps output entirely within the target rect.
    for y in top..bottom {
        let sy=((y as i64-rect.y as i64) as usize*vh/rect_h).min(vh-1);
        let src=sy*vw;
        let dst=y*fw;
        for x in left..right {
            let sx=((x as i64-rect.x as i64) as usize*vw/rect_w).min(vw-1);
            fb.pixels[dst+x]=viewport.pixels[src+sx];
        }
    }
}

/// Map `mata-3d` clip-space vertices from the isolated molecule viewport
/// into the complete window's clip space. Geometry is clipped again by the
/// D3D11 hardware scissor to prevent atoms from covering adjacent UI panels.
pub(crate) fn map_gpu_frame_to_window(frame:&mut GpuFrame,rect:Rect,width:u32,height:u32){
    if width==0||height==0{return;}
    let sw=rect.w.max(1) as f32/width as f32;
    let sh=rect.h.max(1) as f32/height as f32;
    let x=(2.0*rect.x as f32+rect.w as f32)/width as f32-1.0;
    let y=1.0-(2.0*rect.y as f32+rect.h as f32)/height as f32;
    for v in &mut frame.vertices {
        v.position[0]=v.position[0].mul_add(sw,x);
        v.position[1]=v.position[1].mul_add(sh,y);
    }
    for sphere in &mut frame.spheres {
        sphere.center_depth[0]=sphere.center_depth[0].mul_add(sw,x);
        sphere.center_depth[1]=sphere.center_depth[1].mul_add(sh,y);
        sphere.scale[0]*=sw;
        sphere.scale[1]*=sh;
    }
    for bond in &mut frame.bonds {
        bond.projection[0]*=sw;
        bond.projection[1]*=sh;
        bond.projection[2]=bond.projection[2].mul_add(sw,x);
        bond.projection[3]=bond.projection[3].mul_add(sh,y);
    }
}

// Model/selection/camera state fingerprint. Viewports are reused when only
// the surrounding immediate-mode UI changes. Data stays in its original file;
// only already-rendered pixels are cached, and all camera controls still work.
fn molecule_state_key(files:&[&Molecule],rect:Rect,ax:f32,ay:f32,zoom:f32,pan:(f32,f32),
    quality:f32,moving:bool,mode:u8,chain:Option<&str>,residue_colors:bool,
    highlight:Option<(usize,&[usize])>,focus:bool,atom_focus:Option<usize>,
    show_electrons:bool,orbital_mask:u8)->u64 {
    use std::collections::hash_map::DefaultHasher;
    let mut h=DefaultHasher::new();
    for v in [rect.x,rect.y,rect.w,rect.h] {v.hash(&mut h);}
    for v in [ax,ay,zoom,pan.0,pan.1,quality] {v.to_bits().hash(&mut h);}
    mode.hash(&mut h);moving.hash(&mut h);chain.hash(&mut h);residue_colors.hash(&mut h);
    focus.hash(&mut h);atom_focus.hash(&mut h);show_electrons.hash(&mut h);
    orbital_mask.hash(&mut h);
    if let Some((i,ids))=highlight {i.hash(&mut h);ids.hash(&mut h);}
    for m in files {
        ((*m as *const Molecule) as usize).hash(&mut h);
        m.atoms.len().hash(&mut h);m.bonds.len().hash(&mut h);
        m.apply_templates.hash(&mut h);
        for atom in m.atoms.first().into_iter().chain(m.atoms.last()) {
            for v in [atom.pos.x,atom.pos.y,atom.pos.z] {v.to_bits().hash(&mut h);}
        }
    }
    h.finish()
}

pub struct Molecule3d {
    pub raster:Rasterizer,lods:SphereLods,orbitals:OrbitalSurfaces,
    pub viewport:Framebuffer,pub interactive:bool,
    bond_scratch:Mesh,
    parallel:ParallelSoftware,
    last_scene_key:Option<u64>,
    gpu_enabled:bool, gpu_frame:Option<GpuFrame>, gpu_rect:Option<Rect>,
    preset:RenderPreset,
}
impl Molecule3d {
 pub fn new()->Self{Self{raster:{let mut r=Rasterizer::new();r.max_distance=None;r},lods:SphereLods::new(),orbitals:OrbitalSurfaces::new(),viewport:Framebuffer::new(0,0),
    interactive:false,bond_scratch:Mesh{vertices:Vec::with_capacity(24),triangles:Vec::with_capacity(24)},
    parallel:ParallelSoftware::new(),last_scene_key:None,
    gpu_enabled:false,gpu_frame:None,gpu_rect:None,preset:RenderPreset::Auto}}
 pub fn observe_frame_ms(&mut self,ms:f32){self.lods.observe_frame_ms(ms);}
 pub fn set_render_preset(&mut self,preset:RenderPreset){
    if self.preset!=preset {self.preset=preset;self.invalidate_scene();}
 }
 pub fn lod_quality(&self)->f32{self.lods.quality()}
 pub fn active_render_threads(&self)->usize{self.parallel.active_threads()}
 pub fn invalidate_scene(&mut self){self.last_scene_key=None;}
 /// Activate D3D11 command capture only while the native GPU compositor is available.
 /// A change invalidates the old framebuffer/GPU command cache.
 pub fn set_gpu_enabled(&mut self,enable:bool){
     if self.gpu_enabled!=enable {
         self.gpu_enabled=enable;self.invalidate_scene();
         self.lods.set_preset(self.preset,enable);
         self.gpu_rect=None;
     }
 }
 /// Retains the captured triangles on the CPU to permit cached UI redraws
 /// without rebuilding thousands of atom/bond meshes.
 pub fn gpu_scene(&self)->Option<(Rect,&GpuFrame)>{
     if self.gpu_enabled {self.gpu_rect.and_then(|rect|self.gpu_frame.as_ref().map(|f|(rect,f)))}
     else{None}
 }
 fn begin_scene(&mut self,camera:Camera){
     if self.gpu_enabled {
         self.raster.set_gpu_capture(true);
         self.raster.begin_frame(&self.viewport,camera);
     } else {
         self.parallel.begin(&mut self.raster,&self.viewport,camera);
     }
 }
 fn end_scene(&mut self,window:&Framebuffer,rect:Rect){
     if self.gpu_enabled {
         let mut next=self.raster.take_gpu_frame();
         // Rasterizer emits viewport-local normalized screen coordinates.
         // Map them into the real HWND client rectangle (not an upscaled
         // low-resolution image). D3D11 clips the result to `rect`.
         map_gpu_frame_to_window(&mut next,rect,window.width,window.height);
         if let Some(previous)=self.gpu_frame.replace(next) {
             self.raster.recycle_gpu_frame(previous);
         }
         self.gpu_rect=Some(rect);
     } else {
         self.parallel.finish(&mut self.raster,&mut self.viewport);
         self.gpu_rect=None;
     }
 }
 pub fn draw(&mut self,fb:&mut Framebuffer,m:&Molecule,rect:Rect,ax:f32,ay:f32,zoom:f32){
    self.draw_with_pan(fb,m,rect,ax,ay,zoom,(0.0,0.0));
 }
 fn draw_with_pan(&mut self,fb:&mut Framebuffer,m:&Molecule,rect:Rect,ax:f32,ay:f32,zoom:f32,pan:(f32,f32)){
    let key=molecule_state_key(&[m],rect,ax,ay,zoom,pan,self.lods.quality(),self.interactive,
        255,None,false,None,false,None,false,0);
    if !self.interactive && self.last_scene_key==Some(key) && self.viewport.width>0 {
        blit_model(fb,&self.viewport,rect);return;
    }
    let render_start=Instant::now();
    self.last_scene_key=None;
    let (w,h)=if self.gpu_enabled {(rect.w.clamp(1,4096) as u32,rect.h.clamp(1,2304) as u32)}
        else{render_dimensions(rect,self.lods.quality(),self.interactive)};
    if self.viewport.width!=w||self.viewport.height!=h{self.viewport.resize_and_clear(w,h,0xff0f2134);}
    for y in 0..h {let c=if y<h/2{0xff13263a}else{0xff102034};let row=y as usize*w as usize;self.viewport.pixels[row..row+w as usize].fill(c);}
    if is_protein(m) {
        self.lods.set_preset(self.preset,self.gpu_enabled);
        self.lods.configure(m.atoms.len().min(600),self.interactive);
        // Keep one bead per residue, sorted in input order; sample regularly if
        // the protein is too big, but maintain the first-to-last spatial extent.
        let ca=m.atoms.iter().filter_map(|a|{
            let site=a.site.as_ref()?;
            if site.atom_name.trim()=="CA" && !site.hetero {
                Some((a.pos,site.chain_id.as_str()))
            }else{None}
        }).collect::<Vec<_>>();
        if !ca.is_empty(){
            let stride=ca.len().div_ceil(600).max(1);
            let centroid=ca.iter().fold(Vec3::default(),|sum,(p,_)|sum+*p)/ca.len() as f32;
            let rotated=ca.iter().map(|(p,_)|rotate(*p-centroid,ax,ay)).collect::<Vec<_>>();
            let extent=rotated.iter().map(|p|p.len()).fold(1.2f32,f32::max);
            let distance=camera_distance(extent*2.9+3.2,zoom);
            let scale=view_scale(distance*(1.0+pan.0.hypot(pan.1)/0.88),extent);
            let rotated=rotated.into_iter().map(|p|p*scale).collect::<Vec<_>>();
            let (distance,extent)=(distance*scale,extent*scale);
            let camera=pan_camera(distance,pan);
            configure_view_range(&mut self.raster,distance,extent,true);
            self.begin_scene(camera);
            let mut previous:Option<usize>=None;
            for i in (0..ca.len()).step_by(stride).chain(std::iter::once(ca.len()-1)) {
                if let Some(j)=previous {
                    if j==i{continue;}
                    // A big gap between consecutive backbone points may be a
                    // chain break or missing residues. Do not connect chains.
                    if ca[j].1==ca[i].1 && (ca[i].0-ca[j].0).len()>0.01 && (ca[i].0-ca[j].0).len()<4.7*(i-j) as f32 {
                        if self.raster.sphere_visible((rotated[j]+rotated[i])*0.5,
                            (rotated[i]-rotated[j]).len()*0.5+0.20*scale) {
                            cylinder_sides_into(&mut self.bond_scratch,rotated[j],rotated[i],0.20*scale,12);
                            self.raster.mesh(&mut self.viewport,&self.bond_scratch,Transform::default(),
                                Material::two_sided(chain_color(ca[i].1),false));
                        }
                    }
                }
                previous=Some(i);
            }
            self.end_scene(fb,rect);
            self.parallel.observe_render_ms(render_start.elapsed().as_secs_f32()*1000.0);
            self.last_scene_key=Some(key);
            blit_model(fb,&self.viewport,rect);
            return;
        }
    }
    // A V2000 MOL can contain 2D coordinates. Preserve the original on disk,
    // but use a schematic VSEPR embedding for clearly non-planar centres.
    let lift_2d = m.atoms.len()<=750 && m.from_file && m.atoms.iter().all(|a|a.pos.z.abs()<0.0001)
        && !m.bonds.iter().any(|b|b.order==4)
        && (0..m.atoms.len()).any(|i|m.degree(i)>=4
            || (m.atoms[i].symbol=="N" && m.degree(i)==3 && m.lone_pairs(i)==1));
    let preview=if lift_2d{let mut clone=m.clone();crate::layout3d(&mut clone);Some(clone)}else{None};
    let atoms=preview.as_ref().map(|x|x.atoms.as_slice()).unwrap_or(&m.atoms);
    // All coordinates contribute to framing, even under heavy load. Only the
    // *rendered marker representatives* are reduced, not molecule data.
    let visible=atoms.len();
    self.lods.set_preset(self.preset,self.gpu_enabled);
    self.lods.configure(visible,self.interactive);
    // Center imported coordinates around the structure rather than around the
    // coordinate system origin (which can be anywhere in MOL/SDF/MOL2 files).
    let centroid=if atoms.is_empty(){Vec3::default()}else{atoms.iter().fold(Vec3::default(),|sum,a|sum+a.pos)/atoms.len() as f32};
    let positions=atoms.iter().map(|a|rotate(a.pos-centroid,ax,ay)).collect::<Vec<_>>();
    let extent=positions.iter().map(|p|p.len()).fold(1.2f32,f32::max);
    let distance=camera_distance(extent*2.9+3.2,zoom);
    let scale=view_scale(distance*(1.0+pan.0.hypot(pan.1)/0.88),extent);
    let positions=positions.into_iter().map(|p|p*scale).collect::<Vec<_>>();
    let (distance,extent)=(distance*scale,extent*scale);
    let camera=pan_camera(distance,pan);
    configure_view_range(&mut self.raster,distance,extent,false);
    let limit=((self.preset.atom_limit(self.gpu_enabled,self.interactive) as f32
        *self.lods.quality()).round() as usize).max(128);
    let keep=representative_mask(visible,limit,w,h,camera,
        positions.iter().enumerate().map(|(i,&p)|(i,p,draw::element_color(&atoms[i].symbol))));
    // Sphere shape quality is based on markers actually drawn, not all file
    // atoms; zooming in progressively restores rounded atoms.
    self.lods.configure(keep.iter().filter(|&&show|show).count(),self.interactive);
    let bonds_visible=m.bonds.iter().filter(|b|b.a<visible && b.b<visible
        && keep[b.a] && keep[b.b]).count();
    let bond_allowance=self.preset.bond_limit(self.gpu_enabled,self.interactive);
    let bond_stride=bonds_visible.div_ceil(bond_allowance.max(1)).max(1);
    self.begin_scene(camera);
    let mut accepted_bonds=0usize;
    for b in &m.bonds {
        if b.a>=visible||b.b>=visible || !keep[b.a] || !keep[b.b] {continue;}
        accepted_bonds+=1;
        if (accepted_bonds-1)%bond_stride!=0 {continue;}
        let a=positions[b.a];let z=positions[b.b];let d=z-a;let norm=d.normalized();
        let shorten_a=radius(&m.atoms[b.a].symbol)*0.45*scale;let shorten_b=radius(&m.atoms[b.b].symbol)*0.45*scale;
        let aa=a+norm*shorten_a;let bb=z-norm*shorten_b;
        if !self.raster.sphere_visible((aa+bb)*0.5,(bb-aa).len()*0.5+0.15*scale){continue;}
        let normal=if norm.y.abs()<0.9{Vec3::new(0.0,1.0,0.0)}else{Vec3::new(1.0,0.0,0.0)};
        let offset=norm.cross(normal).normalized()*(0.11*scale);
        let count=if b.order==4{1}else{b.order.min(3)} as usize;
        let b_order=b.order;
        for k in 0..count {let shift=offset*(k as f32-(count as f32-1.0)/2.0);
           let a=aa+shift;let b=bb+shift;
           let r=if count>1{0.066*scale}else{0.105*scale};
           let color=if b_order==4{draw::GOLD}else{0xff8ea2b7};
           if !self.raster.capture_bond(a,b,r,color) {
               cylinder_sides_into(&mut self.bond_scratch,a,b,r,8);
               self.raster.mesh(&mut self.viewport,&self.bond_scratch,Transform::default(),Material::two_sided(color,false));
           }
        }
    }
    for (i,a) in atoms.iter().enumerate(){
        if !keep[i]{continue;}
        let r=radius(&a.symbol)*scale;
        if !self.raster.sphere_visible(positions[i],r){continue;}
        let t=Transform::at_scale(positions[i],r);
        let color=if m.estimated_atoms.binary_search(&i).is_ok(){0xffffaa58}else{draw::element_color(&a.symbol)};
        if !self.raster.capture_sphere(positions[i],r,color) {
            self.raster.mesh(&mut self.viewport,self.lods.at(positions[i],r,distance,h),t,Material::two_sided(color,false));
        }
    }
    self.end_scene(fb,rect);
    self.parallel.observe_render_ms(render_start.elapsed().as_secs_f32()*1000.0);
    self.last_scene_key=Some(key);
    blit_model(fb,&self.viewport,rect);
 }
}

#[cfg(test)]
mod render_scale_tests {
    use super::*;
    #[test]fn interactive_viewport_is_smaller() {
        let r=Rect::new(0,0,1440,900);
        assert_eq!(render_dimensions(r,1.0,false),(1440,900));
        assert!(render_dimensions(r,1.0,true).0<1000);
        assert!(render_dimensions(r,0.3,false).1<700);
    }
    #[test]fn upscale_preserves_rect_and_offscreen_clipping() {
        let mut target=Framebuffer::from_color(8,8,0);
        let source=Framebuffer::from_color(2,2,0xff456789);
        blit_model(&mut target,&source,Rect::new(2,2,4,4));
        assert_eq!(target.pixels[3*8+3],0xff456789);
        assert_eq!(target.pixels[0],0);
        blit_model(&mut target,&source,Rect::new(-2,-2,4,4));
        assert_eq!(target.pixels[0],0xff456789);
    }
}

#[cfg(test)]
mod render_distance_tests {
    use super::*;

    #[test]
    fn protein_geometry_fits_legacy_rasterizer_range() {
        // Camera-space triangle distances will remain below the fixed 65-unit
        // draw cap even for the >150-unit original protein viewing distance.
        let scale=view_scale(154.9,52.3);
        assert!(scale<1.0);
        assert!((154.9+52.3+2.0)*scale<=56.001);
        // Uniformly scaling positions, camera distance AND mesh radius leaves
        // projected apparent size unchanged (apart from depth precision).
        assert!((2.0/154.9 - (2.0*scale)/(154.9*scale)).abs()<0.00001);
    }

    #[test]
    fn camera_distance_does_not_stop_at_six_times_zoom() {
        assert!(camera_distance(18.0,7.0)<3.0);
        assert!(camera_distance(18.0,100.0)<camera_distance(18.0,7.0));
        assert!(camera_distance(18.0,0.1)>camera_distance(18.0,0.25));
        assert!(camera_distance(18.0,f32::MAX).is_finite());
        assert!(camera_distance(18.0,f32::MIN_POSITIVE).is_finite());
    }

    #[test]
    fn small_molecules_keep_original_scene_scale() {
        assert_eq!(view_scale(8.0,2.0),1.0);
        let mut raster=Rasterizer::new();
        configure_view_range(&mut raster,40.0,12.0,true);
        assert_eq!(raster.fog_max,0.20);
        configure_view_range(&mut raster,8.0,2.0,false);
        assert_eq!(raster.fog_max,0.65);
    }
}

/// A unified camera and depth buffer for each visible structure. Imported
/// files are placed in separate lanes unless a particular chain or residue is
/// isolated. Atom positions in `Molecule` are never modified by this view.
impl Molecule3d {
 pub fn draw_scene(&mut self, fb:&mut Framebuffer, files:&[&Molecule], rect:Rect,
     ax:f32,ay:f32,zoom:f32,pan:(f32,f32),mode:DetailMode,
     chain:Option<&str>, residue_colors:bool,
     highlight:Option<(usize,&[usize])>,focus:bool,
     atom_focus:Option<usize>,show_electrons:bool,orbital_mask:u8) {
    if files.is_empty(){return;}
    let key=molecule_state_key(files,rect,ax,ay,zoom,pan,self.lods.quality(),self.interactive,
        match mode {DetailMode::Backbone=>0,DetailMode::Ribbon=>1,DetailMode::AllAtoms=>2},
        chain,residue_colors,highlight,focus,atom_focus,show_electrons,orbital_mask);
    if !self.interactive && self.last_scene_key==Some(key) && self.viewport.width>0 {
        blit_model(fb,&self.viewport,rect);return;
    }
    let render_start=Instant::now();
    self.last_scene_key=None;
    if files.len()==1 && highlight.is_none() && chain.is_none() &&
        (mode==DetailMode::Backbone ||
         (mode==DetailMode::Ribbon && !supports_ribbon(files[0])) ||
         (mode==DetailMode::AllAtoms && !is_protein(files[0]))) {
        self.draw_with_pan(fb,files[0],rect,ax,ay,zoom,pan);
        return;
    }
    let atom_count=if focus {highlight.map_or(0,|(_,ids)|ids.len()+12)} else {
        files.iter().take(12).map(|m|m.atoms.len()).sum::<usize>()
    };
    self.lods.set_preset(self.preset,self.gpu_enabled);
    self.lods.configure(atom_count,self.interactive);
    if show_electrons && orbital_mask&0b1100!=0 {
        self.orbitals.set_orientation(ax,ay);
    }
    let (w,h)=if self.gpu_enabled {(rect.w.clamp(1,4096) as u32,rect.h.clamp(1,2304) as u32)}
        else{render_dimensions(rect,self.lods.quality(),self.interactive)};
    if self.viewport.width!=w||self.viewport.height!=h {
        self.viewport.resize_and_clear(w,h,0xff102034);
    }
    for y in 0..h {
        let c=if y<h/2{0xff13263a}else{0xff102034};
        let start=y as usize*w as usize;
        self.viewport.pixels[start..start+w as usize].fill(c);
    }
    struct Slot<'a> { molecule:&'a Molecule, center:Vec3, offset:f32, extent:f32 }
    let mut slots=Vec::new();
    for (file_index,&m) in files.iter().take(12).enumerate() {
        // A chain filter is applied only to the active structure; other files
        // are suppressed rather than implicitly being aligned with it.
        if chain.is_some() && file_index>0 {break;}
        let mut center=Vec3::default();
        let mut count=0usize;
        for a in &m.atoms {
            if file_index==0 && !in_chain(a,chain) {continue;}
            center=center+a.pos;count+=1;
        }
        if count==0{continue;}
        center=center/count as f32;
        let mut extent=1.0f32;
        for a in &m.atoms {
            if file_index==0 && !in_chain(a,chain) {continue;}
            extent=extent.max((a.pos-center).len());
        }
        slots.push(Slot{molecule:m,center,offset:0.0,extent});
    }
    if slots.is_empty(){self.last_scene_key=Some(key);blit_model(fb,&self.viewport,rect);return;}
    // A residue is one amino acid; a second click selects a particular atom
    // and centers it without losing the surrounding bonded residue geometry.
    let focus_target=if focus {highlight.and_then(|(file_index,ids)|{
        let m=files.get(file_index)?;
        let sum=ids.iter().filter_map(|&i|m.atoms.get(i))
            .fold((Vec3::default(),0usize),|(p,n),a|(p+a.pos,n+1));
        let centroid=(sum.1>0).then_some(sum.0/sum.1 as f32)?;
        let center=atom_focus.filter(|a|ids.contains(a))
            .and_then(|i|m.atoms.get(i)).map_or(centroid,|atom|atom.pos);
        Some((file_index,center))
    })}else{None};
    let focus_size=focus_target.and_then(|(file_index,_)|{
        let (selected,ids)=highlight?;
        let m=files.get(file_index)?;
        if selected!=file_index {return None;}
        let centroid=ids.iter().filter_map(|&i|m.atoms.get(i))
            .fold((Vec3::default(),0usize),|(p,n),a|(p+a.pos,n+1));
        if centroid.1==0{return None;}
        let center=centroid.0/centroid.1 as f32;
        Some(ids.iter().filter_map(|&i|m.atoms.get(i))
            .map(|a|(a.pos-center).len()).fold(1.2f32,f32::max))
    });
    let scene_extent=if let Some(size)=focus_size {(size*1.8+3.0).clamp(5.5,35.0)} else {
        let gap=6.0;
        let width=slots.iter().map(|s|2.0*s.extent).sum::<f32>()+
            gap*slots.len().saturating_sub(1) as f32;
        let mut cursor=-width*0.5;
        for slot in &mut slots {
            slot.offset=cursor+slot.extent;
            cursor+=2.0*slot.extent+gap;
        }
        slots.iter().map(|s|s.offset.abs()+s.extent).fold(1.2f32,f32::max)
    };
    let distance=if focus_target.is_some() {
        camera_distance(scene_extent*2.3+4.0,zoom)
    } else {camera_distance(scene_extent*2.9+3.2,zoom)};
    let scale=view_scale(distance*(1.0+pan.0.hypot(pan.1)/0.88),scene_extent);
    let camera=pan_camera(distance*scale,pan);
    configure_view_range(&mut self.raster,distance*scale,scene_extent*scale,true);
    self.begin_scene(camera);
    let max_ribbon=(self.preset.ribbon_segments(self.gpu_enabled,self.interactive)
        /slots.len().max(1)).max(64);
    for (file_index,slot) in slots.iter().enumerate() {
        if focus_target.is_some_and(|(target,_)|target!=file_index){continue;}
        let m=slot.molecule;
        let base=focus_target.map_or(slot.center,|(_,position)|position);
        let offset=if focus_target.is_some(){0.0}else{slot.offset};
        let point=|p:Vec3|rotate(p-base+Vec3::new(offset,0.0,0.0),ax,ay)*scale;
        let focus_ids=if focus_target.is_some() {highlight.and_then(|(target,ids)|
            (target==file_index).then_some(ids))}else{None};
        let bonded=if show_electrons {atom_focus.map(|idx|electron::bonded_atoms(m,idx))}
            else{None};
        let mut focus_mask=vec![false;m.atoms.len()];
        if let Some(ids)=focus_ids {
            for &idx in ids {if let Some(flag)=focus_mask.get_mut(idx){*flag=true;}}
            if let Some(cluster)=&bonded {for &idx in cluster {focus_mask[idx]=true;}}
        }
        let in_scope=|idx:usize|m.atoms.get(idx).is_some_and(|a|{
            (file_index!=0 || in_chain(a,chain)) &&
            (focus_ids.is_none() || focus_mask[idx])
        });
        let simplified_ribbon=mode==DetailMode::Ribbon && focus_ids.is_none()
            && m.atoms.len()/9>max_ribbon;
        if supports_ribbon(m) && mode==DetailMode::Ribbon && !simplified_ribbon {
            // A geometric cartoon (not an imported DSSP assignment). One
            // cohesive strip per observed CA interval; no connections across
            // missing residues or unrelated protein chains.
            let nodes=ribbon::backbone_nodes(m,|idx|in_scope(idx));
            for i in 0..nodes.len().saturating_sub(1) {
                if !ribbon::consecutive(&nodes[i],&nodes[i+1]) {continue;}
                let mesh=ribbon_segment_mesh(&nodes,i,&point);
                let color=if chain.is_some() && residue_colors {residue_class(&nodes[i].name).1}
                    else{nodes[i].secondary.color()};
                self.raster.mesh(&mut self.viewport,&mesh,Transform::default(),
                    Material::two_sided(color,false));
            }
        }else if is_protein(m) && (mode==DetailMode::Backbone || simplified_ribbon) {
            // One C-alpha position per observed amino acid. Retain complete
            // chain topology; never connect points from different chains.
            let ca=m.atoms.iter().enumerate().filter_map(|(i,a)|{
                let site=a.site.as_ref()?;
                (site.atom_name.trim()=="CA" && !site.hetero && in_scope(i))
                    .then_some((i,a.pos,site.chain_id.as_str(),site.residue_name.as_str()))
            }).collect::<Vec<_>>();
            let limit=self.preset.trace_points(self.gpu_enabled,self.interactive)
                .checked_div(slots.len().max(1)).unwrap_or(1).max(2);
            let stride=ca.len().div_ceil(limit).max(1);
            let mut previous:Option<usize>=None;
            for i in (0..ca.len()).step_by(stride).chain(ca.len().checked_sub(1)) {
                if let Some(j)=previous {
                    if j==i{continue;}
                    let length=(ca[i].1-ca[j].1).len();
                    if ca[j].2==ca[i].2 && length>0.01 && length<4.7*(i-j) as f32 {
                        let a=point(ca[j].1);let b=point(ca[i].1);
                        if !self.raster.sphere_visible((a+b)*0.5,(b-a).len()*0.5+0.18*scale){previous=Some(i);continue;}
                        let c=if chain.is_some() && residue_colors {residue_class(ca[i].3).1}
                          else if files.len()==1{chain_color(ca[i].2)}
                          else{chain_color(&format!("{file_index}:{}",ca[i].2))};
                        if !self.raster.capture_bond(a,b,0.18*scale,c) {
                            cylinder_sides_into(&mut self.bond_scratch,a,b,0.18*scale,6);
                            self.raster.mesh(&mut self.viewport,&self.bond_scratch,Transform::default(),Material::two_sided(c,false));
                        }
                    }
                }
                previous=Some(i);
            }
        }else{
            // Overloaded protein/virus all-atom views retain selected *real*
            // source atom locations, with one foreground representative per
            // projected tile and element class. Zooming/focusing increases
            // spatial detail; no atoms or bonds are removed from source files.
            let protein=is_protein(m);
            let limit=((self.preset.atom_limit(self.gpu_enabled,self.interactive) as f32
                *self.lods.quality()) as usize /slots.len().max(1)).max(128);
            let keep=if focus_ids.is_some() || m.atoms.len()<=limit {
                vec![true;m.atoms.len()]
            }else{
                representative_mask(m.atoms.len(),limit,w,h,camera,
                    m.atoms.iter().enumerate().filter(|(i,_)|in_scope(*i))
                        .map(|(i,a)|(i,point(a.pos),draw::element_color(&a.symbol))))
            };
            let marker_count=keep.iter().filter(|&&v|v).count().max(1);
            self.lods.configure(marker_count*slots.len().max(1),self.interactive);
            let eligible_bonds=m.bonds.iter().filter(|b|
                in_scope(b.a) && in_scope(b.b) && keep[b.a] && keep[b.b]).count();
            let bond_allowance=(self.preset.bond_limit(self.gpu_enabled,self.interactive)
                /slots.len().max(1)).max(1);
            let bond_stride=eligible_bonds.div_ceil(bond_allowance).max(1);
            let mut accepted_bonds=0usize;
            for b in &m.bonds {
                if !in_scope(b.a)||!in_scope(b.b) || !keep[b.a] || !keep[b.b] {continue;}
                accepted_bonds+=1;
                if (accepted_bonds-1)%bond_stride!=0 {continue;}
                let atom_a=&m.atoms[b.a];let atom_b=&m.atoms[b.b];
                let a=point(atom_a.pos);let z=point(atom_b.pos);
                let direction=z-a;let length=direction.len();
                if length<0.00001 || !self.raster.sphere_visible((a+z)*0.5,length*0.5+0.22*scale) {continue;}
                let order=if protein && m.apply_templates {mata_molecule::protein_inspect::display_bond_order(m,b)}
                  else{b.order};
                let parts=if order==4 {1}else{order.clamp(1,3)} as usize;
                let normal=direction/length;
                let reference=if normal.y.abs()<0.9{Vec3::new(0.0,1.0,0.0)}
                   else{Vec3::new(1.0,0.0,0.0)};
                let step=normal.cross(reference).normalized()*(0.11*scale);
                let bond_color=if protein && chain.is_some() && residue_colors {
                    atom_a.site.as_ref().map_or(0xff8ea2b7,|s|residue_class(&s.residue_name).1)
                }else if order==4 {draw::GOLD}else{0xff8ea2b7};
                for j in 0..parts {
                    let offset=step*(j as f32-(parts as f32-1.0)*0.5);
                    let aa=a+offset;let bb=z+offset;
                    if !self.raster.capture_bond(aa,bb,0.075*scale,bond_color) {
                        cylinder_sides_into(&mut self.bond_scratch,aa,bb,0.075*scale,if protein{4}else{8});
                        self.raster.mesh(&mut self.viewport,&self.bond_scratch,Transform::default(),Material::two_sided(bond_color,false));
                    }
                }
            }
            for (index,atom) in m.atoms.iter().enumerate() {
                if !in_scope(index) || !keep[index] {continue;}
                let p=point(atom.pos);
                let r=radius(&atom.symbol)*scale*if protein{0.75}else{1.0};
                if !self.raster.sphere_visible(p,r){continue;}
                let t=Transform::at_scale(p,r);
                let color=if m.estimated_atoms.binary_search(&index).is_ok(){0xffffaa58}else{draw::element_color(&atom.symbol)};
                if !self.raster.capture_sphere(p,r,color) {
                    self.raster.mesh(&mut self.viewport,self.lods.at(p,r,distance*scale,h),t,
                        Material::two_sided(color,false));
                }
            }
        }
        if let Some((target,ids))=highlight {
            if target==file_index {
                let selected:std::collections::HashSet<usize>=ids.iter().copied().collect();
                for b in &m.bonds {
                    if selected.contains(&b.a)&&selected.contains(&b.b) &&
                        in_scope(b.a)&&in_scope(b.b) {
                        let a=point(m.atoms[b.a].pos);let z=point(m.atoms[b.b].pos);
                        if (z-a).len()>0.0001 {
                            if self.raster.sphere_visible((a+z)*0.5,(z-a).len()*0.5+0.13*scale) {
                                if !self.raster.capture_bond(a,z,0.13*scale,draw::GOLD) {
                                    cylinder_sides_into(&mut self.bond_scratch,a,z,0.13*scale,6);
                                    self.raster.mesh(&mut self.viewport,&self.bond_scratch,Transform::default(),Material::two_sided(draw::GOLD,false));
                                }
                            }
                        }
                    }
                }
                for &i in ids {
                    if !in_scope(i){continue;}
                    let a=&m.atoms[i];
                    let p=point(a.pos);
                    let r=radius(&a.symbol)*1.23*scale;
                    let transform=Transform::at_scale(p,r);
                    let color=if m.estimated_atoms.binary_search(&i).is_ok(){0xffffaa58}else{draw::element_color(&a.symbol)};
                    if !self.raster.capture_sphere(p,r,color) {
                        self.raster.mesh(&mut self.viewport,self.lods.at(p,r,distance*scale,h),transform,
                            Material::two_sided(color,false));
                    }
                }
            }
        }
        // High-contrast selection halo distinguishes the clicked atom from
        // the rest of the highlighted amino acid, even with clouds turned off.
        if let Some(center)=atom_focus.filter(|&a|focus_ids.is_some() && in_scope(a)) {
            for k in 0..12 {
                let angle=TAU*k as f32/12.0;
                let off=Vec3::new(angle.cos()*0.52,angle.sin()*0.52,0.0);
                let p=point(m.atoms[center].pos+off);
                let r=0.046*scale;
                if !self.raster.capture_sphere(p,r,draw::GOLD) {
                    self.raster.mesh(&mut self.viewport,self.lods.at(p,r,distance*scale,h),
                        Transform::at_scale(p,r),Material::two_sided(draw::GOLD,false));
                }
            }
        }
        if show_electrons && focus_ids.is_some() {
            if let Some(center)=atom_focus.filter(|&a|in_scope(a)) {
                draw_electron_overlay(&mut self.raster,&mut self.viewport,&self.lods,&self.orbitals,
                    m,center,&point,scale,distance*scale,h,orbital_mask,self.interactive);
            }
        }
    }
    self.end_scene(fb,rect);
    self.parallel.observe_render_ms(render_start.elapsed().as_secs_f32()*1000.0);
    self.last_scene_key=Some(key);
    blit_model(fb,&self.viewport,rect);
 }
}

/// Neutral-atom reference isosurfaces around the clicked atom and directly
/// bonded neighbors. All surfaces are *illustrative*, never molecular orbitals.
/// Independent bit flags let the user suppress inner d/f shells when they
/// obscure chemistry; 1=s, 2=p, 4=d, 8=f.
fn draw_electron_overlay(raster:&mut Rasterizer,fb:&mut Framebuffer,lods:&SphereLods,
    surfaces:&OrbitalSurfaces,m:&Molecule,center:usize,point:&impl Fn(Vec3)->Vec3,
    scale:f32,distance:f32,h:u32,orbital_mask:u8,interactive:bool) {
    if orbital_mask==0{return;}
    let cluster=electron::bonded_atoms(m,center);
    // High-poly angular surfaces only when a single orbital is genuinely
    // large in the viewport; never use them during camera manipulation.
    let high=!interactive && (0.88*0.85*scale*h as f32/distance.max(0.001))>115.0;
    for &idx in &cluster {
        let atom=&m.atoms[idx];
        let Some(subshells)=electron::outer_subshells(&atom.symbol) else{continue;};
        for shell in subshells {
            let mask=match shell.kind {'s'=>1,'p'=>2,'d'=>4,'f'=>8,_=>0};
            if orbital_mask&mask==0 {continue;}
            let n=shell.principal;
            let electrons=shell.occupancy;
            match shell.kind {
                's'=>{
                    let r=0.46+(n as f32-1.0)*0.045;
                    // Sparse surface points keep the nuclei and bonds visible.
                    for k in 0..12 {
                        let theta=TAU*k as f32/12.0;
                        for plane in 0..2 {
                            let off=if plane==0 {Vec3::new(theta.cos(),theta.sin(),0.0)}
                                else{Vec3::new(0.0,theta.cos(),theta.sin())};
                            let pos=point(atom.pos+off*r);
                            let radius=0.033*scale;
                            raster.mesh(fb,lods.at(pos,radius,distance,h),Transform::at_scale(pos,radius),
                                Material::two_sided(0xff58ddf4,false));
                        }
                    }
                },
                'p'=>{
                    let axes=[Vec3::new(1.0,0.0,0.0),Vec3::new(0.0,1.0,0.0),Vec3::new(0.0,0.0,1.0)];
                    for axis in 0..(electrons.min(3) as usize) {
                        let color=if electrons>3+axis as u8 {0xffff8ebd}else{0xffa494ff};
                        for sign in [-1.0f32,1.0] {
                            let pos=point(atom.pos+axes[axis]*(0.62*sign));
                            let r=0.205*scale;
                            raster.mesh(fb,lods.at(pos,r,distance,h),Transform::at_scale(pos,r),
                                Material::two_sided(color,false));
                        }
                    }
                },
                'd'|'f'=>{
                    let total=if shell.kind=='d'{5}else{7};
                    let radius=if shell.kind=='d'{0.79}else{0.92};
                    let radius=radius*(1.0+(n as f32-3.0).max(0.0)*0.025);
                    let (positive,negative)=if shell.kind=='d' {
                        (0xff75e6b6,0xff278868)
                    }else{(0xffffb56b,0xffba6834)};
                    // Hund filling: singly occupy each orbital, then pair.
                    // Orbital phases (not spin or electrical charge) are
                    // distinguished with two shades on each surface.
                    for i in 0..(electrons.min(total) as usize) {
                        let Some(shape)=surfaces.get_oriented(shell.kind,i,high) else{continue;};
                        let center=point(atom.pos);
                        // Positions already rotated and templates oriented once
                        // per camera angle. No per-atom vertex Vec or triangle
                        // copy occurs in this hot loop.
                        for (template,color) in [(&shape.positive,positive),(&shape.negative,negative)] {
                            raster.mesh(fb,template,Transform::at_scale(center,radius*scale),
                                Material::two_sided(color,false));
                        }
                    }
                },
                _=>{},
            }
        }
    }
    // Educational bond-region markers show imported connectivity only; they
    // are not calculated bonding-electron probability distributions.
    for b in &m.bonds {
        if b.a!=center&&b.b!=center {continue;}
        let (Some(a),Some(z))=(m.atoms.get(b.a),m.atoms.get(b.b))else{continue;};
        for t in [0.40,0.50,0.60] {
            let pos=point(a.pos*(1.0-t)+z.pos*t);
            let r=0.08*scale;
            raster.mesh(fb,lods.at(pos,r,distance,h),Transform::at_scale(pos,r),
                Material::two_sided(0xffffca65,false));
        }
    }
}

/// Project atoms using the same centering, rotation, camera distance and
/// perspective as draw_scene. Called only after mouse release (never per frame).
/// The caller passes one visible active file; focus refers to atom indices in
/// that same displayed molecule, including any reconstructed hydrogens.
pub fn pick_atom(m:&Molecule,rect:Rect,x:i32,y:i32,ax:f32,ay:f32,
    zoom:f32,pan:(f32,f32),chain:Option<&str>,
    focus_ids:Option<&[usize]>,atom_focus:Option<usize>,mode:DetailMode,
    extra_files:&[&Molecule])->Option<usize> {
    if !rect.hit(x,y)||rect.w<=0||rect.h<=0{return None;}
    let positions:Vec<usize>=if let Some(ids)=focus_ids {
        ids.iter().copied().filter(|&i|i<m.atoms.len()).collect()
    }else{
        m.atoms.iter().enumerate().filter(|(_,a)|in_chain(a,chain))
            .map(|(i,_)|i).collect()
    };
    if positions.is_empty(){return None;}
    let origin=positions.iter().fold(Vec3::default(),|sum,&i|sum+m.atoms[i].pos)/positions.len() as f32;
    let focused=focus_ids.is_some();
    let extent=positions.iter().map(|&i|(m.atoms[i].pos-origin).len())
        .fold(if focused{1.2f32}else{1.0f32},f32::max);
    let mut scene_extent=if focused {(extent*1.8+3.0).clamp(5.5,35.0)}else{extent};
    let mut lane_offset=0.0f32;
    // Multiple visible files get separate horizontal lanes in draw_scene.
    // Reproduce that placement so the active file is picked at its actual
    // screen position rather than at the center of the whole canvas.
    if !focused && chain.is_none() && !extra_files.is_empty() {
        let mut extents=vec![extent];
        for m in extra_files.iter().take(11) {
            if m.atoms.is_empty(){continue;}
            let center=m.atoms.iter().fold(Vec3::default(),|p,a|p+a.pos)/m.atoms.len() as f32;
            let ext=m.atoms.iter().map(|a|(a.pos-center).len()).fold(1.0f32,f32::max);
            extents.push(ext);
        }
        let gap=6.0;
        let width=extents.iter().map(|e|2.0*e).sum::<f32>()+gap*(extents.len()-1) as f32;
        lane_offset=-width*0.5+extent;
        let mut cursor=-width*0.5;
        scene_extent=0.0;
        for e in extents {
            let offset=cursor+e;
            scene_extent=scene_extent.max(offset.abs()+e);
            cursor+=2.0*e+gap;
        }
    }
    let base=if focused {atom_focus.and_then(|i|focus_ids.filter(|ids|ids.contains(&i))
        .and_then(|_|m.atoms.get(i)).map(|a|a.pos)).unwrap_or(origin)}else{origin};
    let distance=camera_distance(if focused {scene_extent*2.3+4.0}
        else{scene_extent*2.9+3.2},zoom);
    let perspective=|index:usize|->Option<(f32,f32,f32)> {
        let p=rotate(m.atoms[index].pos-base+Vec3::new(lane_offset,0.0,0.0),ax,ay);
        let depth=distance+p.z;
        if depth<0.01||!depth.is_finite(){return None;}
        let factor=0.88*rect.h as f32/depth;
        let sx=rect.x as f32+rect.w as f32*0.5 +p.x*factor+pan.0*distance/depth*rect.h as f32;
        let sy=rect.y as f32+rect.h as f32*0.5 -p.y*factor-pan.1*distance/depth*rect.h as f32;
        Some((sx,sy,factor))
    };
    let ca_only=!focused && ((mode!=DetailMode::AllAtoms && is_protein(m)) ||
        (mode==DetailMode::Ribbon && supports_ribbon(m)));
    let mut best:Option<(usize,f32,f32)>=None;
    for &i in &positions {
        let atom=&m.atoms[i];
        if ca_only&&!atom.site.as_ref().is_some_and(|s|s.atom_name.trim()=="CA"&&!s.hetero){continue;}
        let Some((sx,sy,factor))=perspective(i)else{continue;};
        let radius_px=radius(&atom.symbol)*factor*if is_protein(m){0.75}else{1.0};
        let reach=if focused {radius_px.max(7.0)+9.0}else if ca_only{20.0}else{radius_px.max(5.0)+7.0};
        let delta=((sx-x as f32).powi(2)+(sy-y as f32).powi(2)).sqrt();
        if delta>reach {continue;}
        let score=delta/reach;
        if best.is_none_or(|(_,old,old_depth)|score<old ||
            ((score-old).abs()<0.025&&factor>old_depth)) {
            best=Some((i,score,factor));
        }
    }
    // Backbone/ribbon models are continuous strips, not just clickable CA
    // points. Accept a click close to a rendered CA-CA segment, selecting
    // whichever residue endpoint is nearest along the segment.
    if ca_only {
        let mut previous=None::<(usize,f32,f32)>;
        for &i in &positions {
            let a=&m.atoms[i];
            if !a.site.as_ref().is_some_and(|site|site.atom_name.trim()=="CA"&&!site.hetero){continue;}
            let Some((sx,sy,_))=perspective(i)else{continue;};
            if let Some((j,jx,jy))=previous {
                let xlen=sx-jx;let ylen=sy-jy;
                let length=xlen*xlen+ylen*ylen;
                let compatible=m.atoms[j].site.as_ref().zip(a.site.as_ref())
                    .is_some_and(|(old,new)|old.chain_id==new.chain_id &&
                        (a.pos-m.atoms[j].pos).len()<4.7);
                if compatible&&length>0.001 {
                    let t=(((x as f32-jx)*xlen+(y as f32-jy)*ylen)/length).clamp(0.0,1.0);
                    let dx=(jx+t*xlen)-x as f32;let dy=(jy+t*ylen)-y as f32;
                    let dist=(dx*dx+dy*dy).sqrt();
                    if dist<12.0 {
                        let score=dist/12.0;
                        if best.is_none_or(|(_,old,_)|score<old) {
                            best=Some((if t<0.5{j}else{i},score,0.0));
                        }
                    }
                }
            }
            previous=Some((i,sx,sy));
        }
    }
    best.map(|(index,_,_)|index)
}

/// A curved, two-sided cartoon with thickness. Helices have broad rounded
/// strips, beta-like extended runs end in an arrow, and loops are narrow.
/// Cross sections are oriented by the observed carbonyl oxygen where possible
/// and remain stable under camera rotation.
fn ribbon_segment_mesh(
    nodes:&[BackboneNode], i:usize, point:&impl Fn(Vec3)->Vec3,
) -> Mesh {
    let b=&nodes[i];let c=&nodes[i+1];
    let p0=if i>0 && ribbon::consecutive(&nodes[i-1],b){nodes[i-1].ca}else{b.ca};
    let p3=if i+2<nodes.len() && ribbon::consecutive(c,&nodes[i+2]){nodes[i+2].ca}else{c.ca};
    let section=b.secondary;
    let strand_end=section==Secondary::Strand &&
        (c.secondary!=Secondary::Strand || i+2==nodes.len() ||
         !ribbon::consecutive(c,&nodes[i+2]) || nodes[i+2].secondary!=Secondary::Strand);
    // Two subdivisions per CA-CA link keep large fibrils responsive with the
    // supplied CPU rasterizer, while Catmull-Rom removes the zig-zag corners.
    const STEPS:usize=3;
    let curve=|t:f32|ribbon::smooth_segment(p0,b.ca,c.ca,p3,t);
    let mut vertices=Vec::with_capacity((STEPS+1)*4);
    let mut triangles=Vec::with_capacity(STEPS*8);
    let mut prev_side=None::<Vec3>;
    for k in 0..=STEPS {
        let t=k as f32/STEPS as f32;
        let center=curve(t);
        let tangent=(curve((t+0.015).min(1.0))-curve((t-0.015).max(0.0))).normalized();
        let start_side=ribbon_side(b,tangent);
        let mut end_side=ribbon_side(c,tangent);
        if start_side.dot(end_side)<0.0 {end_side=end_side*-1.0;}
        let mut side=start_side*(1.0-t)+end_side*t;
        side=side-tangent*side.dot(tangent);
        if side.len()<0.0001 {side=prev_side.unwrap_or(start_side);}
        side=side.normalized();
        if let Some(prev)=prev_side {if side.dot(prev)<0.0 {side=side*-1.0;}}
        prev_side=Some(side);
        let normal=tangent.cross(side).normalized();
        let width=match section {
            Secondary::Helix=>0.81,
            Secondary::Strand if strand_end => {
                // Pointed arrowhead on the end of a beta-like strand.
                if t<0.33 {0.77+1.00*t}else{(1.10*(1.0-t)/0.67).max(0.025)}
            },
            Secondary::Strand=>0.79,
            Secondary::Loop=>0.22,
        };
        let height=match section {Secondary::Loop=>0.14,_=>0.18};
        for pos in [center-side*width+normal*height,
                    center+side*width+normal*height,
                    center-side*width-normal*height,
                    center+side*width-normal*height] {
            vertices.push(point(pos));
        }
        if k>0 {
            let a=((k-1)*4) as u32;let z=(k*4) as u32;
            // Top, bottom, and two outer edges (two triangles each).
            for (p,q,r,s) in [(0,1,0,1),(2,3,2,3),(0,2,0,2),(1,3,1,3)] {
                triangles.push([a+p,a+q,z+r]);
                triangles.push([z+r,a+q,z+s]);
            }
        }
    }
    Mesh{vertices,triangles}
}

fn ribbon_side(node:&BackboneNode,tangent:Vec3)->Vec3 {
    let raw=match (node.c,node.o) {
        (Some(c),Some(o)) if (o-c).len()>0.01 => o-c,
        _=>if tangent.y.abs()<0.85 {Vec3::new(0.0,1.0,0.0)}
           else {Vec3::new(1.0,0.0,0.0)},
    };
    let side=raw-tangent*raw.dot(tangent);
    if side.len()>1.0e-5 {side.normalized()}
    else {
        let reference=if tangent.z.abs()<0.8 {Vec3::new(0.0,0.0,1.0)}
            else {Vec3::new(0.0,1.0,0.0)};
        tangent.cross(reference).normalized()
    }
}

fn in_chain(atom:&mata_molecule::chemistry::Atom,chain:Option<&str>)->bool {
    chain.is_none_or(|wanted|atom.site.as_ref().is_some_and(|site|site.chain_id==wanted))
}
fn cylinder_sides(a:Vec3,b:Vec3,r:f32,steps:usize)->Mesh {
    let mut mesh=Mesh{vertices:Vec::with_capacity(steps*2),triangles:Vec::with_capacity(steps*2)};
    cylinder_sides_into(&mut mesh,a,b,r,steps);
    mesh
}
pub(crate) fn cylinder_sides_into(mesh:&mut Mesh,a:Vec3,b:Vec3,r:f32,steps:usize) {
    mesh.vertices.clear();mesh.triangles.clear();
    let dir=(b-a).normalized();
    let normal=if dir.y.abs()<0.9{Vec3::new(0.0,1.0,0.0)}else{Vec3::new(1.0,0.0,0.0)};
    let u=dir.cross(normal).normalized()*r;
    let v=dir.cross(u).normalized()*r;
    for i in 0..steps {
        let theta=i as f32*TAU/steps as f32;
        let o=u*theta.cos()+v*theta.sin();
        mesh.vertices.push(a+o);mesh.vertices.push(b+o);
    }
    for i in 0..steps {
        let p=(2*i) as u32;
        let q=(2*((i+1)%steps)) as u32;
        mesh.triangles.push([p,p+1,q]);mesh.triangles.push([q,p+1,q+1]);
    }
}
#[cfg(test)]
mod residue_picking_tests {
    use super::*;
    use mata_molecule::chemistry::AtomSite;
    fn add(m:&mut Molecule,atom:&str,residue:&str,num:&str,x:f32)->usize {
        let i=m.atom(atom);
        m.atoms[i].pos=Vec3::new(x,0.0,0.0);
        m.atoms[i].site=Some(AtomSite{atom_name:"CA".into(),residue_name:residue.into(),
            residue_number:num.into(),chain_id:"A".into(),insertion_code:String::new(),
            alternate_location:String::new(),occupancy:1.0,b_factor:0.0,
            hetero:false,serial:(i+1).to_string()});
        i
    }
    #[test] fn focused_atom_is_at_the_camera_center(){
        let mut m=Molecule::new("Amino acid");
        add(&mut m,"C","ALA","1",-2.0);
        add(&mut m,"N","ALA","1",2.0);
        let screen=Rect::new(10,20,800,600);
        assert_eq!(pick_atom(&m,screen,410,320,0.,0.,1.,(0.,0.),None,
            Some(&[0,1]),Some(0),DetailMode::AllAtoms,&[]),Some(0));
    }
    #[test] fn empty_canvas_space_does_not_pick_and_panning_changes_projection(){
        let mut m=Molecule::new("Residue");
        add(&mut m,"C","GLY","3",0.0);
        let r=Rect::new(0,0,800,600);
        assert_eq!(pick_atom(&m,r,400,300,0.,0.,1.,(0.,0.),None,
            Some(&[0]),None,DetailMode::AllAtoms,&[]),Some(0));
        assert_eq!(pick_atom(&m,r,10,10,0.,0.,1.,(0.,0.),None,
            Some(&[0]),None,DetailMode::AllAtoms,&[]),None);
        assert_eq!(pick_atom(&m,r,400,300,0.,0.,1.,(0.3,0.0),None,
            Some(&[0]),None,DetailMode::AllAtoms,&[]),None);
    }
}

#[cfg(test)]
mod inspection_view_tests {
    use super::*;
    use mata_molecule::chemistry::{Atom,AtomSite};
    #[test]
    fn residue_classes_and_colors_are_stable() {
        assert_eq!(residue_class("ASP").0,"ACIDIC");
        assert_eq!(residue_class("ARG").0,"BASIC");
        assert_eq!(residue_class("TRP").0,"AROMATIC");
        assert_eq!(residue_class("GLY").0,"SPECIAL");
        assert_eq!(residue_class("VAL").0,"NONPOLAR");
        assert_eq!(residue_class("foo").0,"OTHER");
    }
    #[test]
    fn only_selected_chain_passes_filter() {
        let mut atom=Atom::new("C");
        assert!(!in_chain(&atom,Some("A")));
        atom.site=Some(AtomSite{atom_name:"CA".into(),residue_name:"ALA".into(),
            chain_id:"A".into(),residue_number:"1".into(),insertion_code:String::new(),
            alternate_location:String::new(),occupancy:1.0,b_factor:0.0,
            hetero:false,serial:"1".into()});
        assert!(in_chain(&atom,Some("A")));
        assert!(!in_chain(&atom,Some("B")));
    }
    #[test]
    fn pan_changes_camera_without_touching_molecule() {
        let origin=pan_camera(10.0,(0.0,0.0));
        let moved=pan_camera(10.0,(0.25,-0.2));
        assert!(moved.position.x<origin.position.x);
        assert!(moved.position.y>origin.position.y);
    }
}

#[cfg(test)]
mod gpu_window_mapping_tests {
    use super::*;
    #[test]fn viewport_corners_map_to_window_clip_space() {
        let mut frame=GpuFrame{vertices:vec![
            mata_3d::GpuVertex{position:[-1.0,1.0,0.8],rgba8:42},
            mata_3d::GpuVertex{position:[1.0,-1.0,0.2],rgba8:21},
        ],spheres:Vec::new(),bonds:Vec::new(),depth_clears:Vec::new(),generation:0};
        frame.spheres.push(mata_3d::GpuSphereInstance{
            center_depth:[0.0,0.0,12.0,0.3],scale:[0.1,0.2],near:0.075,rgba8:0xffffffff,
        });
        frame.bonds.push(mata_3d::GpuBondInstance{
            start_radius:[0.0,0.0,12.0,0.1],end_near:[1.0,1.0,12.0,0.075],
            projection:[0.3,0.4,0.0,0.0],rgba8:0xffffffff,
        });
        map_gpu_frame_to_window(&mut frame,Rect::new(100,50,400,200),800,600);
        for (actual,expected) in frame.vertices[0].position.iter().zip([-0.75,1.0-100.0/600.0,0.8]){
            assert!((*actual-expected).abs()<0.0001);
        }
        let sphere=&frame.spheres[0];
        assert!((sphere.center_depth[0]+0.25).abs()<0.0001);
        assert!((sphere.center_depth[1]-(1.0-300.0/600.0)).abs()<0.0001);
        assert!((sphere.scale[0]-0.05).abs()<0.0001);
        assert!((sphere.scale[1]-0.2/3.0).abs()<0.0001);
        let bond=&frame.bonds[0];
        assert!((bond.projection[0]-0.15).abs()<0.0001);
        assert!((bond.projection[1]-(0.4/3.0)).abs()<0.0001);
        for (actual,expected) in frame.vertices[1].position.iter().zip([0.25,1.0-500.0/600.0,0.2]){
            assert!((*actual-expected).abs()<0.0001);
        }
    }
    #[test]fn disabling_gpu_invalidates_scene_cache(){
        let mut view=Molecule3d::new();view.last_scene_key=Some(11);
        view.set_gpu_enabled(true);assert_eq!(view.last_scene_key,None);
        view.last_scene_key=Some(7);view.set_gpu_enabled(false);
        assert!(view.gpu_scene().is_none());assert_eq!(view.last_scene_key,None);
    }
}
