//! Game-specific weapon roster: separate ammunition, reload profiles and drops.
//! All balancing values are fictional, for the Neon Run arena game.

pub const WEAPON_COUNT: usize = 5;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Gun { Carbine, Cannon, Sniper, Shotgun, Minigun }
impl Gun {
    pub const ALL: [Gun;WEAPON_COUNT]=[Gun::Carbine,Gun::Cannon,Gun::Sniper,Gun::Shotgun,Gun::Minigun];
    pub const fn index(self)->usize {self as usize}
    pub fn from_index(index:usize)->Self {Self::ALL[index.min(WEAPON_COUNT-1)]}
    pub const fn spec(self)->WeaponSpec { SPECS[self.index()] }
}
#[derive(Clone,Copy,Debug)]
pub struct WeaponSpec {
    pub name:&'static str,
    pub ammo_name:&'static str,
    pub magazine:i32,
    pub initial_reserve:i32,
    pub reserve_cap:i32,
    pub reload_seconds:f32,
    pub shot_delay:f32,
    pub damage:f32,
    pub pellets:usize,
    pub spread:f32,
    pub drop_min:i32,
    pub drop_max:i32,
}
pub const SPECS:[WeaponSpec;WEAPON_COUNT]=[
    WeaponSpec{name:"PULSE CARBINE",ammo_name:"PULSE CELLS",magazine:32,initial_reserve:128,reserve_cap:320,reload_seconds:1.05,shot_delay:0.105,damage:22.0,pellets:1,spread:0.009,drop_min:13,drop_max:27},
    WeaponSpec{name:"ARC CANNON",ammo_name:"ARC CHARGES",magazine:10,initial_reserve:30,reserve_cap:100,reload_seconds:1.65,shot_delay:0.40,damage:55.0,pellets:1,spread:0.016,drop_min:3,drop_max:7},
    WeaponSpec{name:"LANCER-7",ammo_name:"RAIL SLUGS",magazine:5,initial_reserve:15,reserve_cap:55,reload_seconds:2.1,shot_delay:1.0,damage:125.0,pellets:1,spread:0.003,drop_min:2,drop_max:5},
    WeaponSpec{name:"SHOTGUN",ammo_name:"SHELLS",magazine:8,initial_reserve:32,reserve_cap:100,reload_seconds:1.8,shot_delay:0.72,damage:14.0,pellets:9,spread:0.095,drop_min:4,drop_max:10},
    WeaponSpec{name:"MINIGUN",ammo_name:"BELT ROUNDS",magazine:100,initial_reserve:260,reserve_cap:800,reload_seconds:3.0,shot_delay:0.065,damage:11.0,pellets:1,spread:0.048,drop_min:25,drop_max:55},
];
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Magazine {pub loaded:i32,pub reserve:i32}
impl Magazine {
    pub fn starting(gun:Gun)->Self {
        let spec=gun.spec();Self{loaded:spec.magazine,reserve:spec.initial_reserve}
    }
    pub fn reload(&mut self,gun:Gun){
        let needed=(gun.spec().magazine-self.loaded).max(0);
        let moved=needed.min(self.reserve.max(0));
        self.loaded+=moved;self.reserve-=moved;
    }
    pub fn add_reserve(&mut self,gun:Gun,amount:i32){
        self.reserve=self.reserve.saturating_add(amount.max(0)).min(gun.spec().reserve_cap);
    }
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn every_gun_has_distinct_ammo_and_reasonable_supply() {
        for (i,g) in Gun::ALL.iter().enumerate(){
            let s=g.spec();assert_eq!(i,g.index());
            assert!(s.magazine>0&&s.initial_reserve>0&&s.drop_max>=s.drop_min&&s.drop_min>0);
            assert!(s.drop_max<=s.magazine*2);assert!(s.reload_seconds>=0.6);
            for other in Gun::ALL.iter().skip(i+1) {assert_ne!(s.ammo_name,other.spec().ammo_name);}
        }
    }
    #[test] fn loading_consumes_only_matching_reserve(){
        let mut gun=Magazine{loaded:0,reserve:3};gun.reload(Gun::Sniper);
        assert_eq!(gun,Magazine{loaded:3,reserve:0});gun.add_reserve(Gun::Sniper,10000);
        assert_eq!(gun.reserve,Gun::Sniper.spec().reserve_cap);
    }
}
