//! Game physics use the same 3D vector and AABB representation as the renderer.
//! No duplicate transform/math system is required in the game.
pub use mata_3d::{Vec3 as V3, Aabb as Box3};

/// First forward hit along a normalized ray. Kept in the game because it is
/// a gameplay collision query, not a rendering operation.
pub fn ray_sphere(origin:V3,d:V3,center:V3,r:f32)->Option<f32>{
    let oc=origin-center;let b=oc.dot(d);let c=oc.dot(oc)-r*r;
    let delta=b*b-c;
    if delta<0.0{return None}
    let sq=delta.sqrt();let t=-b-sq;
    if t>=0.0{Some(t)}else{let t=-b+sq;if t>=0.0{Some(t)}else{None}}
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test] fn same_game_physics(){
        let b=Box3::new(0.0,0.0,0.0,1.0,2.0,1.0);
        assert_eq!(b.ray_hit(V3::new(-2.0,1.0,0.5),V3::new(1.0,0.0,0.0)),Some(2.0));
        assert_eq!(ray_sphere(V3::new(0.0,0.0,0.0),V3::new(0.0,0.0,1.0),V3::new(0.0,0.0,3.0),1.0),Some(2.0));
    }
}
