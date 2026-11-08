//! Cached illustrative angular isosurfaces for real d and f atomic orbitals.
//!
//! The real spherical-harmonic angular factors yield the proper arrangement
//! of nodal planes/cones and lobes. Their radial envelope is intentionally
//! schematic: NOT hydrogenic radial wavefunctions or molecular orbitals.
//! Positive/negative phase meshes get contrasting shades of the same color.
use mata_3d::{Mesh,Vec3};
use std::f32::consts::{PI,TAU};

pub struct Surface {pub positive:Mesh,pub negative:Mesh}

pub struct OrbitalSurfaces {
    d_low:Vec<Surface>,d_high:Vec<Surface>,
    f_low:Vec<Surface>,f_high:Vec<Surface>,
    // Camera-oriented vertices are recomputed only when view angles change.
    // Each residue reuses these immutable triangle buffers with Transform.
    oriented_d_low:Vec<Surface>,oriented_d_high:Vec<Surface>,
    oriented_f_low:Vec<Surface>,oriented_f_high:Vec<Surface>,
    orientation:Option<(f32,f32)>,
}

impl OrbitalSurfaces {
    pub fn new()->Self {
        let make=|kind,count,longitude,latitude|(0..count)
            .map(|idx|make_surface(kind,idx,longitude,latitude)).collect::<Vec<Surface>>();
        let (d_low,d_high,f_low,f_high)=(make('d',5,16,12),make('d',5,30,22),
            make('f',7,16,12),make('f',7,30,22));
        Self {
            oriented_d_low:copy_surfaces(&d_low),oriented_d_high:copy_surfaces(&d_high),
            oriented_f_low:copy_surfaces(&f_low),oriented_f_high:copy_surfaces(&f_high),
            d_low,d_high,f_low,f_high,orientation:None,
        }
    }
    pub fn get(&self,kind:char,index:usize,high:bool)->Option<&Surface> {
        let group=match (kind,high) {
            ('d',false)=>&self.d_low,('d',true)=>&self.d_high,
            ('f',false)=>&self.f_low,('f',true)=>&self.f_high,
            _=>return None,
        };
        group.get(index)
    }
    pub fn set_orientation(&mut self,ax:f32,ay:f32) {
        if self.orientation==Some((ax,ay)){return;}
        self.orientation=Some((ax,ay));
        orient_group(&self.d_low,&mut self.oriented_d_low,ax,ay);
        orient_group(&self.d_high,&mut self.oriented_d_high,ax,ay);
        orient_group(&self.f_low,&mut self.oriented_f_low,ax,ay);
        orient_group(&self.f_high,&mut self.oriented_f_high,ax,ay);
    }
    pub fn get_oriented(&self,kind:char,index:usize,high:bool)->Option<&Surface> {
        let group=match (kind,high) {
            ('d',false)=>&self.oriented_d_low,('d',true)=>&self.oriented_d_high,
            ('f',false)=>&self.oriented_f_low,('f',true)=>&self.oriented_f_high,
            _=>return None,
        };
        group.get(index)
    }

}

fn copy_surfaces(src:&[Surface])->Vec<Surface> {
    src.iter().map(|s|Surface {
        positive:Mesh{vertices:s.positive.vertices.clone(),triangles:s.positive.triangles.clone()},
        negative:Mesh{vertices:s.negative.vertices.clone(),triangles:s.negative.triangles.clone()},
    }).collect()
}
fn orient_group(original:&[Surface],out:&mut [Surface],ax:f32,ay:f32) {
    for (from,to) in original.iter().zip(out) {
        for (src,dst) in [(&from.positive,&mut to.positive),(&from.negative,&mut to.negative)] {
            for (&v,dest) in src.vertices.iter().zip(&mut dst.vertices) {
                *dest=crate::view::rotate(v,ax,ay);
            }
        }
    }
}

/// Real solid-harmonic angular basis on a unit sphere (unnormalized).
/// d: xy, xz, yz, x2-y2, 3z2-r2
/// f: z(5z2-3r2), x(5z2-r2), y(5z2-r2), z(x2-y2), xyz,
///    x(x2-3y2), y(3x2-y2).
fn amplitude(kind:char,index:usize,xyz:Vec3)->f32 {
    let (x,y,z)=(xyz.x,xyz.y,xyz.z);
    match (kind,index) {
        ('d',0)=>x*y,('d',1)=>x*z,('d',2)=>y*z,
        ('d',3)=>x*x-y*y,('d',4)=>3.0*z*z-1.0,
        ('f',0)=>z*(5.0*z*z-3.0),
        ('f',1)=>x*(5.0*z*z-1.0),
        ('f',2)=>y*(5.0*z*z-1.0),
        ('f',3)=>z*(x*x-y*y),
        ('f',4)=>x*y*z,
        ('f',5)=>x*(x*x-3.0*y*y),
        ('f',6)=>y*(3.0*x*x-y*y),
        _=>0.0,
    }
}

fn make_surface(kind:char,index:usize,longitudes:usize,latitudes:usize)->Surface {
    let mut angular=Vec::with_capacity((longitudes+1)*(latitudes+1));
    let mut max_amplitude=0.0f32;
    for latitude in 0..=latitudes {
        let theta=PI*latitude as f32/latitudes as f32;
        for longitude in 0..=longitudes {
            let phi=TAU*longitude as f32/longitudes as f32;
            let xyz=Vec3::new(theta.sin()*phi.cos(),theta.sin()*phi.sin(),theta.cos());
            let a=amplitude(kind,index,xyz);
            max_amplitude=max_amplitude.max(a.abs());
            angular.push((xyz,a));
        }
    }
    let scale=1.0/max_amplitude.max(0.000001);
    let vertices=angular.iter().map(|(xyz,a)|
        *xyz*(a.abs()*scale).powf(0.64).max(0.0001)
    ).collect::<Vec<_>>();
    let mut positive=Vec::new();let mut negative=Vec::new();
    let width=longitudes+1;
    for latitude in 0..latitudes {
        for longitude in 0..longitudes {
            let a=(latitude*width+longitude) as u32;
            let b=a+1;let c=a+width as u32;let d=c+1;
            for tri in [[a,c,b],[b,c,d]] {
                let [x,y,z]=tri.map(|i|angular[i as usize].1);
                // Do not interpolate a solid face through a nodal plane.
                let min=x.min(y).min(z);let max=x.max(y).max(z);
                if min< -1e-5 && max>1e-5 {continue;}
                if x+y+z>=0.0 {positive.push(tri);}else{negative.push(tri);}
            }
        }
    }
    Surface{positive:Mesh{vertices:vertices.clone(),triangles:positive},
        negative:Mesh{vertices,triangles:negative}}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn all_orbitals_have_distinct_positive_and_negative_regions() {
        let library=OrbitalSurfaces::new();
        for (kind,count) in [('d',5),('f',7)] {
            for i in 0..count {
                let s=library.get(kind,i,false).unwrap();
                assert!(!s.positive.triangles.is_empty(),"{kind}{i} positive");
                assert!(!s.negative.triangles.is_empty(),"{kind}{i} negative");
                assert!(s.positive.vertices.iter().all(|v|v.x.is_finite()&&v.y.is_finite()&&v.z.is_finite()));
                assert!(library.get(kind,i,true).unwrap().positive.triangles.len()>s.positive.triangles.len());
            }
        }
    }
    #[test] fn dz2_has_equatorial_nodal_cone_and_f_has_three_angular_nodes() {
        // d_z2 changes sign between equator and poles.
        assert!(amplitude('d',4,Vec3::new(0.0,0.0,1.0))>0.0);
        assert!(amplitude('d',4,Vec3::new(1.0,0.0,0.0))<0.0);
        // xyz f harmonic is zero on all three coordinate planes.
        assert_eq!(amplitude('f',4,Vec3::new(0.0,0.5,0.5)),0.0);
        assert_eq!(amplitude('f',4,Vec3::new(0.5,0.0,0.5)),0.0);
        assert_eq!(amplitude('f',4,Vec3::new(0.5,0.5,0.0)),0.0);
    }
}
