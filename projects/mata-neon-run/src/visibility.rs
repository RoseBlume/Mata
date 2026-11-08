//! Conservative whole-character view-frustum test: no rig transforms, material
//! draws or triangle setup for actors wholly outside the view. It deliberately
//! accepts marginal cases to avoid popping articulated limbs at the borders.
//! Uses the same camera forward/right/up convention as the scene renderer.
use crate::math::V3;

/// Includes the full articulated character, swinging limbs and idle sway.
/// The radius is deliberately larger than the bounds of any one enemy model.
pub const ENEMY_RADIUS:f32=3.35;

#[derive(Clone,Copy)]
pub struct ViewFrustum {
    pub eye:V3,pub forward:V3,pub right:V3,pub up:V3,
    pub tan_half_horizontal:f32,pub tan_half_vertical:f32,
}
impl ViewFrustum {
    pub fn new(eye:V3,forward:V3,right:V3,width:usize,height:usize,focal_scale:f32)->Self {
        let up=forward.cross(right).normalized();
        // The software renderer's vertical projection is close to 90 degrees.
        // The extra 30% is intentional: never reject partly visible meshes,
        // including scope FOV changes and their swinging arms.
        let vertical=1.30/focal_scale.max(0.05);
        let aspect=width.max(1) as f32/height.max(1) as f32;
        Self{eye,forward,right,up,
            tan_half_horizontal:vertical*aspect,tan_half_vertical:vertical}
    }
    /// Bounding-sphere test against near and all four side planes.
    /// If false, skip the entire articulated mesh without evaluating a joint.
    #[inline]
    pub fn contains_sphere(&self,center:V3,radius:f32)->bool{
        let delta=center-self.eye;
        let z=delta.dot(self.forward);
        if z+radius<0.04{return false}
        let x=delta.dot(self.right).abs();
        let y=delta.dot(self.up).abs();
        // Plane normals have length sqrt(1+slope^2), so apply that to
        // sphere radius. This handles partial visibility at screen edges.
        let hx=self.tan_half_horizontal;
        let vy=self.tan_half_vertical;
        x<=z*hx+radius*(1.0+hx*hx).sqrt()
            && y<=z*vy+radius*(1.0+vy*vy).sqrt()
    }
    pub fn sees_enemy(&self,feet:V3)->bool{
        self.contains_sphere(feet+V3::new(0.0,1.4,0.0),ENEMY_RADIUS)
    }
}
#[cfg(test)]mod tests{
    use super::*;
    fn camera()->ViewFrustum{ViewFrustum::new(V3::new(0.0,1.6,0.0),V3::new(0.0,0.0,1.0),
        V3::new(1.0,0.0,0.0),1920,1080,1.0)}
    #[test]fn keeps_near_frustum_edges(){
        let c=camera();
        assert!(c.sees_enemy(V3::new(0.0,0.0,15.0)));
        assert!(c.sees_enemy(V3::new(18.0,0.0,15.0)));
    }
    #[test]fn skips_wholly_offscreen_enemies(){
        let c=camera();
        assert!(!c.sees_enemy(V3::new(0.0,0.0,-13.0)));
        assert!(!c.sees_enemy(V3::new(60.0,0.0,8.0)));
        assert!(!c.sees_enemy(V3::new(0.0,60.0,8.0)));
    }
    #[test]fn zoom_reduces_visible_angle_without_reducing_model_quality(){
        let wide=camera();
        let zoom=ViewFrustum::new(wide.eye,wide.forward,wide.right,1920,1080,3.4);
        let p=V3::new(32.0,0.0,18.0);
        assert!(wide.sees_enemy(p));
        assert!(!zoom.sees_enemy(p));
    }
}
