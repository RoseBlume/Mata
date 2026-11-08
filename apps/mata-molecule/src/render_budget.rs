//! Bounded, screen-space geometry preparation for integrated GPUs and small CPUs.
//! This module ONLY changes drawn proxies. Source atom coordinates and bond
//! tables remain intact for measurements, selection, and file export.
use std::collections::HashMap;
use mata_3d::{Camera,Vec3};

#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash)]
pub enum RenderPreset { Auto, Economy, Detail }
impl RenderPreset {
    pub fn next(self)->Self {match self{Self::Auto=>Self::Economy,Self::Economy=>Self::Detail,Self::Detail=>Self::Auto}}
    pub fn label(self)->&'static str {match self{Self::Auto=>"AUTO",Self::Economy=>"ECONOMY",Self::Detail=>"DETAIL"}}
    /// This is an approximation, including atoms AND their bonds/ribbons.
    /// A hard per-frame global cap would require new GPU primitives; these
    /// limits keep the common worst-case under a predictable workload.
    pub fn triangles(self,gpu:bool,moving:bool)->usize{
        let stationary=match (self,gpu){
            (Self::Economy,false)=>30_000,(Self::Economy,true)=>55_000,
            (Self::Auto,false)=>75_000,(Self::Auto,true)=>135_000,
            (Self::Detail,false)=>140_000,(Self::Detail,true)=>300_000,
        };
        if moving {stationary*2/3}else{stationary}
    }
    /// Native GPU instancing only uploads 32 bytes per atom instead of
    /// expanding a full triangle mesh per atom. Keep explicit ceilings for
    /// integrated GPUs; the screen-space representative filter runs first.
    pub fn atom_limit(self,gpu:bool,moving:bool)->usize{
        if gpu {
            let count=match self {Self::Economy=>8_000,Self::Auto=>20_000,Self::Detail=>45_000};
            return if moving {count*2/3}else{count};
        }
        (self.triangles(false,moving)/18).max(128)
    }
    pub fn bond_limit(self,gpu:bool,moving:bool)->usize{
        if gpu {
            let count=match self {Self::Economy=>8_000,Self::Auto=>20_000,Self::Detail=>45_000};
            return if moving {count*2/3}else{count};
        }
        (self.triangles(false,moving)/48).max(1)
    }
    pub fn trace_points(self,gpu:bool,moving:bool)->usize{
        (self.triangles(gpu,moving)/48).clamp(250,4_000)
    }
    pub fn ribbon_segments(self,gpu:bool,moving:bool)->usize{
        (self.triangles(gpu,moving)/24).clamp(500,10_000)
    }
}

/// Keeps at most one foreground atom per screen tile and element/visual class,
/// when the full visible atom cloud exceeds the allowed marker budget.
/// Chosen positions remain real source atom positions (no invented centroids).
/// For close inspection, callers skip this step when under the budget.
///
/// Using an element class prevents a large carbon cloud from completely
/// hiding isolated oxygen/nitrogen markers within the same tile.
pub fn representative_mask(
    total:usize,limit:usize,width:u32,height:u32,camera:Camera,
    points:impl Iterator<Item=(usize,Vec3,u32)>,
)->Vec<bool>{
    if total<=limit{return vec![true;total];}
    let mut mask=vec![false;total];
    let w=width.max(1) as f32;let h=height.max(1) as f32;
    let target=limit.max(1);
    // Favor visual distribution over file order. The factor anticipates 4-6
    // element classes, leaving headroom for dense heteroatom-rich structures.
    let tile=((w*h/(target as f32)*4.0).sqrt()).max(2.0);
    let focal=h*camera.focal_scale;
    let mut tiles:HashMap<(i32,i32,u32),(usize,f32)>=HashMap::with_capacity(target.min(total));
    for (id,position,class) in points {
        if id>=total {continue;}
        let v=camera.view(position);
        if !(v.z.is_finite()&&v.z>=camera.near.max(0.001)) {continue;}
        let x=w*0.5+focal*v.x/v.z;
        let y=h*0.5-focal*v.y/v.z;
        if !x.is_finite()||!y.is_finite()||x<0.0||x>=w||y<0.0||y>=h {continue;}
        let key=((x/tile) as i32,(y/tile) as i32,class);
        match tiles.entry(key){
            std::collections::hash_map::Entry::Vacant(entry)=>{entry.insert((id,v.z));},
            std::collections::hash_map::Entry::Occupied(mut entry)=>{
                // Nearest wins. Stable source index breaks exact-depth ties.
                let (old_id,old_z)=*entry.get();
                if v.z<old_z || (v.z==old_z && id<old_id){entry.insert((id,v.z));}
            }
        }
    }
    let mut winners=tiles.into_iter().map(|(key,(id,depth))|(key,id,depth)).collect::<Vec<_>>();
    // Never expand the GPU upload beyond the advertised marker budget, even
    // if there are many distinct atom types in the same screen tile.
    if winners.len()>target {
        // Favor frontmost atoms, yet disperse selections across the image.
        // Sorting by coarse spatial cell first keeps large-scale morphology.
        winners.sort_unstable_by(|a,b|{
            let ax=(a.0.0/4,a.0.1/4);let bx=(b.0.0/4,b.0.1/4);
            ax.cmp(&bx).then(a.2.total_cmp(&b.2)).then(a.1.cmp(&b.1))
        });
        let stride=winners.len().div_ceil(target);
        let mut picked=Vec::with_capacity(target);
        for chunk in winners.chunks(stride){
            if let Some(item)=chunk.iter().min_by(|a,b|a.2.total_cmp(&b.2)){picked.push(item.1);}
        }
        // If ceil stepping produced fewer markers than budget, nothing is
        // gained by inventing or interpolating extra coordinates.
        for i in picked {mask[i]=true;}
    }else{for (_,id,_) in winners {mask[id]=true;}}
    mask
}

#[cfg(test)]mod tests {
    use super::*;
    fn camera()->Camera{Camera::new(Vec3::new(0.,0.,-15.),Vec3::new(0.,0.,1.),Vec3::new(1.,0.,0.))}
    #[test]fn cap_and_real_source_indices(){
        let points=(0..50_000).map(|i| (i,Vec3::new((i%200) as f32/25.-4.,(i/200) as f32/30.-4.,(i%9) as f32/10.),i as u32%5));
        let mask=representative_mask(50_000,3000,800,500,camera(),points);
        assert!(mask.iter().filter(|v|**v).count()<=3000);
        assert!(mask.iter().any(|v|*v));
    }
    #[test]fn small_source_is_unchanged(){
        let mask=representative_mask(3,10,40,30,camera(),std::iter::empty());
        assert_eq!(mask,[true,true,true]);
    }
    #[test]fn near_view_uses_more_visible_proxies(){
        let points=|z:f32|(0..10_000).map(move |i|
            (i,Vec3::new(i as f32%100.0/20.0-2.5,
                (i/100) as f32/20.0-2.5,z),0xff888888));
        let distant=representative_mask(10_000,2000,800,600,camera(),points(40.));
        let closer=representative_mask(10_000,2000,800,600,camera(),points(1.));
        assert!(distant.iter().filter(|&&x|x).count()<=2000);
        assert!(closer.iter().filter(|&&x|x).count()<=2000);
        assert!(closer.iter().filter(|&&x|x).count()>=distant.iter().filter(|&&x|x).count());
    }
    #[test]fn modes_are_ordered(){
        assert_eq!(RenderPreset::Auto.atom_limit(true,false),20_000);
        assert_eq!(RenderPreset::Economy.bond_limit(true,true),5_333);
        for gpu in [false,true] {
            assert!(RenderPreset::Economy.triangles(gpu,false)<RenderPreset::Auto.triangles(gpu,false));
            assert!(RenderPreset::Auto.triangles(gpu,false)<RenderPreset::Detail.triangles(gpu,false));
        }
    }
}
