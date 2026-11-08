//! Budgeted, adaptive screen-space atom tessellation.
//!
//! `SphereLods` never allocates sphere geometry during drawing. Projection
//! alone is insufficient: 2,000 moderately large atoms at 1,728 triangles
//! each makes a software rasterizer unusable. A scene-wide triangle allowance,
//! interaction downgrade and feedback from actual frame cost are also used.
use mata_3d::{Mesh,Vec3};
use crate::render_budget::RenderPreset;
use std::f32::consts::{PI,TAU};

const FACES:[usize;5]=[8,96,320,768,1728];


pub struct SphereLods {
    meshes:[Mesh;5],
    max_tier:usize,
    quality:f32,
    motion:bool,
    cheap_frames:u16,
    last_ms:f32,
    preset:RenderPreset,
    gpu:bool,
}
impl SphereLods {
    pub fn new()->Self {
        Self{meshes:[octahedron(),sphere(6,8),sphere(10,16),sphere(16,24),sphere(24,36)],
            max_tier:4,quality:1.0,motion:false,cheap_frames:0,last_ms:0.0,
            preset:RenderPreset::Auto,gpu:false}
    }
    /// Assign a total geometry budget BEFORE entering the per-atom loop.
    /// Counts must be scoped to atoms actually being drawn (not the whole PDB
    /// when a residue is isolated). The budget is deliberately conservative
    /// for the CPU-only `mata-3d` rasterizer.
    pub fn set_preset(&mut self,preset:RenderPreset,gpu:bool){
        if self.preset!=preset && preset==RenderPreset::Detail {
            self.quality=1.0;
        }
        self.preset=preset;self.gpu=gpu;
    }
    pub fn configure(&mut self,visible_atoms:usize,moving:bool) {
        self.motion=moving;
        let atoms=visible_atoms.max(1);
        let budget=(self.preset.triangles(self.gpu,moving) as f32*0.5*self.quality) as usize;
        let per_atom=budget/atoms;
        self.max_tier=FACES.iter().rposition(|&count|count<=per_atom).unwrap_or(0);
    }
    /// Feedback is sampled after a presented frame. Changes are damped to
    /// prevent LOD oscillating around the frame-time target.
    pub fn observe_frame_ms(&mut self,ms:f32) {
        if !ms.is_finite()||ms<=0.0{return;}
        self.last_ms=ms;
        if self.preset==RenderPreset::Detail{return;}
        if ms>44.0 {
            self.quality=(self.quality*0.84).max(0.28);
            self.cheap_frames=0;
        }else if ms<19.0 && !self.motion {
            self.cheap_frames=self.cheap_frames.saturating_add(1);
            if self.cheap_frames>=8 {
                self.quality=(self.quality+0.035).min(1.0);
                self.cheap_frames=0;
            }
        }else{self.cheap_frames=0;}
    }
    pub fn quality(&self)->f32{self.quality}
    pub fn last_frame_ms(&self)->f32{self.last_ms}
    /// Approximate projected radius in pixels, assuming camera focal scale .88.
    /// Radius and center must have the scene's uniform rasterizer scale.
    pub fn at(&self,center:Vec3,radius:f32,camera_distance:f32,viewport_height:u32)->&Mesh {
        let depth=center.z+camera_distance;
        let radius_px=if depth>0.001 && depth.is_finite() {
            0.88*viewport_height as f32*radius.abs()/depth
        }else{0.0};
        let screen_tier=tier(radius_px * self.quality * if self.motion{0.68}else{1.0});
        &self.meshes[screen_tier.min(self.max_tier)]
    }
}
/// Match silhouette fidelity to actual pixel footprint. A 25-pixel-radius
/// sphere emphatically does NOT need 1,728 software-rasterized triangles.
fn tier(radius_px:f32)->usize {
    if radius_px<5.0 {0}
    else if radius_px<15.0 {1}
    else if radius_px<38.0 {2}
    else if radius_px<95.0 {3}
    else {4}
}
fn sphere(rows:usize,cols:usize)->Mesh {
    let mut vertices=Vec::with_capacity((rows+1)*(cols+1));
    let mut triangles=Vec::with_capacity(2*rows*cols);
    for row in 0..=rows {
        let phi=row as f32*PI/rows as f32;
        for col in 0..=cols {
            let theta=col as f32*TAU/cols as f32;
            vertices.push(Vec3::new(theta.cos()*phi.sin(),phi.cos(),theta.sin()*phi.sin()));
        }
    }
    for row in 0..rows {for col in 0..cols {
        let a=(row*(cols+1)+col) as u32;
        let b=a+(cols+1) as u32;
        triangles.push([a,b,a+1]);triangles.push([a+1,b,b+1]);
    }}
    Mesh{vertices,triangles}
}
fn octahedron()->Mesh {
    Mesh{vertices:vec![
        Vec3::new(0.,1.,0.),Vec3::new(1.,0.,0.),
        Vec3::new(0.,0.,1.),Vec3::new(-1.,0.,0.),
        Vec3::new(0.,0.,-1.),Vec3::new(0.,-1.,0.),
    ],triangles:vec![
        [0,1,2],[0,2,3],[0,3,4],[0,4,1],
        [5,2,1],[5,3,2],[5,4,3],[5,1,4],
    ]}
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn projection_levels_preserve_monotonicity(){
        assert_eq!([tier(1.),tier(5.),tier(15.),tier(38.),tier(95.)],[0,1,2,3,4]);
    }
    #[test] fn mesh_faces_and_depth(){
        let meshes=SphereLods::new();
        assert_eq!((0..5).map(|i|meshes.meshes[i].triangles.len()).collect::<Vec<_>>(),FACES.to_vec());
        let p=Vec3::new(0.,0.,0.);
        assert!(meshes.at(p,1.,12.,700).triangles.len()>meshes.at(p,1.,600.,700).triangles.len());
    }
    #[test] fn dense_scene_is_budgeted(){
        let mut meshes=SphereLods::new();
        meshes.configure(2000,false);
        assert!(FACES[meshes.max_tier]*2000<=RenderPreset::Auto.triangles(false,false)/2);
        let stationary=meshes.max_tier;
        meshes.configure(2000,true);
        assert!(meshes.max_tier<=stationary);
    }
    #[test] fn slow_frames_reduce_quality_and_recovery_is_gradual(){
        let mut meshes=SphereLods::new();
        meshes.observe_frame_ms(80.);
        assert!(meshes.quality()<1.);
        let after_slow=meshes.quality();
        meshes.configure(4,false);
        for _ in 0..8{meshes.observe_frame_ms(8.);}
        assert!(meshes.quality()>after_slow);
    }
}
