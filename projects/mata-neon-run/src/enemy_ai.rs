//! Enemy sight and navigation. This is SIMULATION ONLY: enemies outside the
//! player's view are still allowed to think, but never get submitted to the
//! graphics pipeline (visibility.rs). These two systems are independent.
use crate::campaign;
use crate::math::{Box3,V3};
use crate::world::BotKind;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Awareness {Idle,Chasing,Investigating}

#[derive(Clone,Copy)]
pub struct Vision {pub range:f32,pub cosine:f32}
impl Vision {
    pub fn for_kind(kind:BotKind)->Self {
        match kind {
            // Full cones 130/125/110/105/140 degrees respectively.
            BotKind::Runner=>Self{range:15.0,cosine:0.423},
            BotKind::Striker=>Self{range:13.5,cosine:0.462},
            BotKind::Tank=>Self{range:12.5,cosine:0.574},
            BotKind::Marksman=>Self{range:22.0,cosine:0.609},
            BotKind::Gunner=>Self{range:17.5,cosine:0.342},
        }
    }
}
/// True only if the player is inside the range and horizontal field of view,
/// and there is a clear 3D ray from the bot sensor to player eye.
/// Props have no collision, so only gameplay-solid wall/cover AABBs block sight.
pub fn can_see(origin:V3,facing:f32,target:V3,kind:BotKind,walls:&[Box3])->bool {
    let delta=target-origin;
    let dist2=delta.dot(delta);
    let vision=Vision::for_kind(kind);
    if dist2<0.001{return true}
    if dist2>vision.range*vision.range{return false}
    let flat=delta.horizontal();
    let len2=flat.dot(flat);
    if len2>0.001 {
        let look=V3::new(facing.sin(),0.0,facing.cos());
        if flat.dot(look)<vision.cosine*len2.sqrt(){return false}
    }
    let dist=dist2.sqrt();let dir=delta/dist;
    !walls.iter().any(|wall|wall.ray_hit(origin,dir).is_some_and(|t|t>=0.0&&t<dist-0.08))
}

// These are only *walking routes*; vision and shooting still require a clear
// physical 3D ray. Navigation is fixed-grid because all missions use the same
// authored 18x18 cell grid. Avoids external pathfinding libraries.
fn cell(p:V3)->Option<usize>{
    let x=(p.x/2.0).floor() as i32;
    let z=(p.z/2.0).floor() as i32;
    if (0..18).contains(&x)&&(0..18).contains(&z){Some(z as usize*18+x as usize)}else{None}
}
fn walkable(stage:usize,index:usize)->bool {
    let x=index%18;let z=index/18;
    !matches!(campaign::MAPS[stage][z].as_bytes()[x],b'#'|b'c')
}
/// Return the centre of the first cell on a shortest path. The BFS queue and
/// predecessor storage are fixed-size stack arrays and allocate nothing.
/// None means a route is unknown; the bot should not walk through a wall.
pub fn next_waypoint(stage:usize,from:V3,to:V3)->Option<V3>{
    if stage>=campaign::MISSION_COUNT{return None}
    let start=cell(from)?;let goal=cell(to)?;
    if !walkable(stage,start)||!walkable(stage,goal){return None}
    if start==goal {return Some(to)}
    let mut predecessor=[usize::MAX;324];
    let mut queue=[0usize;324];
    let mut head=0usize;let mut tail=1usize;
    queue[0]=start;predecessor[start]=start;
    while head<tail {
        let v=queue[head];head+=1;
        if v==goal{break}
        let x=v%18;let z=v/18;
        let neighbors=[
            if x>0{Some(v-1)}else{None},
            if x<17{Some(v+1)}else{None},
            if z>0{Some(v-18)}else{None},
            if z<17{Some(v+18)}else{None},
        ];
        for next in neighbors.into_iter().flatten(){
            if predecessor[next]!=usize::MAX||!walkable(stage,next){continue}
            predecessor[next]=v;
            queue[tail]=next;tail+=1;
        }
    }
    if predecessor[goal]==usize::MAX{return None}
    let mut current=goal;
    while predecessor[current]!=start{current=predecessor[current]}
    let x=current%18;let z=current/18;
    Some(V3::new(x as f32*2.0+1.0,0.0,z as f32*2.0+1.0))
}
/// Check that a direct ground route is unblocked. If a wall obstructs it,
/// the bot navigates to an actual grid waypoint rather than wall-walking.
pub fn clear_walk_path(start:V3,goal:V3,walls:&[Box3])->bool{
    let from=start+V3::new(0.0,0.9,0.0);
    let end=goal+V3::new(0.0,0.9,0.0);
    let delta=end-from;let d=delta.len();
    if d<0.25{return true}
    let dir=delta/d;
    !walls.iter().any(|b|b.ray_hit(from,dir).is_some_and(|t|t>=0.0&&t<d-0.16))
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn no_detection_through_wall(){
        let from=V3::new(4.0,1.3,3.0);let player=V3::new(4.0,1.6,8.0);
        let wall=Box3::new(3.0,0.0,5.0,2.0,4.7,1.0);
        assert!(!can_see(from,0.0,player,BotKind::Runner,&[wall]));
        assert!(can_see(from,0.0,player,BotKind::Runner,&[]));
    }
    #[test]fn vision_checks_range_and_facing(){
        let p=V3::new(0.0,1.3,0.0);
        assert!(!can_see(p,0.0,V3::new(0.0,1.6,-5.0),BotKind::Runner,&[]));
        assert!(!can_see(p,0.0,V3::new(0.0,1.6,28.0),BotKind::Runner,&[]));
        assert!(can_see(p,0.0,V3::new(0.0,1.6,5.0),BotKind::Runner,&[]));
    }
    #[test]fn navigation_routes_around_walls(){
        let map=&campaign::MAPS[0];
        let start=V3::new(3.0,0.0,3.0);
        let goal=V3::new(3.0,0.0,11.0);
        assert_eq!(map[4].as_bytes()[1],b'#');
        let waypoint=next_waypoint(0,start,goal).unwrap();
        assert_ne!(waypoint,start);
        assert!(walkable(0,cell(waypoint).unwrap()));
    }
}
