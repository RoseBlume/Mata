//! Authored station props, ceiling structures, interactive consoles and lamps.
//! Embedded OBJ/MTL models parsed at startup; no third-party game crates.
use mata_win::Framebuffer;
use mata_3d::{Rasterizer,Material,Model,Transform};
use crate::{math::{Box3,V3},world::{World,SiteKind,DecorKind,ARENA}};

pub struct Environment {
    ceiling_light:Model,
    hanging_lamp:Model,
    vent_fan:Model,
    door_frame:Model,
    door_panel:Model,
    terminal:Model,
    console_bank:Model,
    server_rack:Model,
    pipe_bundle:Model,
    coolant_tank:Model,
    storage_crate:Model,
    chair:Model,
    desk:Model,
    wall_monitor:Model,
    warning_beacon:Model,
    relay:Model,
    record_station:Model,
    uplink:Model,
    floor_grate:Model,
    ceiling_duct:Model,
    wall_sign:Model,
}
impl Environment {
    pub fn new()->Self{Self{
        ceiling_light:Model::from_obj_mtl(include_str!("../assets/models/env_ceiling_light.obj"),
            include_str!("../assets/models/env_ceiling_light.mtl")).expect("valid ceiling_light environment mesh"),
        hanging_lamp:Model::from_obj_mtl(include_str!("../assets/models/env_hanging_lamp.obj"),
            include_str!("../assets/models/env_hanging_lamp.mtl")).expect("valid hanging_lamp environment mesh"),
        vent_fan:Model::from_obj_mtl(include_str!("../assets/models/env_vent_fan.obj"),
            include_str!("../assets/models/env_vent_fan.mtl")).expect("valid vent_fan environment mesh"),
        door_frame:Model::from_obj_mtl(include_str!("../assets/models/env_door_frame.obj"),
            include_str!("../assets/models/env_door_frame.mtl")).expect("valid door_frame environment mesh"),
        door_panel:Model::from_obj_mtl(include_str!("../assets/models/env_door_panel.obj"),
            include_str!("../assets/models/env_door_panel.mtl")).expect("valid door_panel environment mesh"),
        terminal:Model::from_obj_mtl(include_str!("../assets/models/env_terminal.obj"),
            include_str!("../assets/models/env_terminal.mtl")).expect("valid terminal environment mesh"),
        console_bank:Model::from_obj_mtl(include_str!("../assets/models/env_console_bank.obj"),
            include_str!("../assets/models/env_console_bank.mtl")).expect("valid console_bank environment mesh"),
        server_rack:Model::from_obj_mtl(include_str!("../assets/models/env_server_rack.obj"),
            include_str!("../assets/models/env_server_rack.mtl")).expect("valid server_rack environment mesh"),
        pipe_bundle:Model::from_obj_mtl(include_str!("../assets/models/env_pipe_bundle.obj"),
            include_str!("../assets/models/env_pipe_bundle.mtl")).expect("valid pipe_bundle environment mesh"),
        coolant_tank:Model::from_obj_mtl(include_str!("../assets/models/env_coolant_tank.obj"),
            include_str!("../assets/models/env_coolant_tank.mtl")).expect("valid coolant_tank environment mesh"),
        storage_crate:Model::from_obj_mtl(include_str!("../assets/models/env_crate.obj"),
            include_str!("../assets/models/env_crate.mtl")).expect("valid crate environment mesh"),
        chair:Model::from_obj_mtl(include_str!("../assets/models/env_chair.obj"),
            include_str!("../assets/models/env_chair.mtl")).expect("valid chair environment mesh"),
        desk:Model::from_obj_mtl(include_str!("../assets/models/env_desk.obj"),
            include_str!("../assets/models/env_desk.mtl")).expect("valid desk environment mesh"),
        wall_monitor:Model::from_obj_mtl(include_str!("../assets/models/env_wall_monitor.obj"),
            include_str!("../assets/models/env_wall_monitor.mtl")).expect("valid wall_monitor environment mesh"),
        warning_beacon:Model::from_obj_mtl(include_str!("../assets/models/env_warning_beacon.obj"),
            include_str!("../assets/models/env_warning_beacon.mtl")).expect("valid warning_beacon environment mesh"),
        relay:Model::from_obj_mtl(include_str!("../assets/models/env_relay.obj"),
            include_str!("../assets/models/env_relay.mtl")).expect("valid relay environment mesh"),
        record_station:Model::from_obj_mtl(include_str!("../assets/models/env_record_station.obj"),
            include_str!("../assets/models/env_record_station.mtl")).expect("valid record_station environment mesh"),
        uplink:Model::from_obj_mtl(include_str!("../assets/models/env_uplink.obj"),
            include_str!("../assets/models/env_uplink.mtl")).expect("valid uplink environment mesh"),
        floor_grate:Model::from_obj_mtl(include_str!("../assets/models/env_floor_grate.obj"),
            include_str!("../assets/models/env_floor_grate.mtl")).expect("valid floor_grate environment mesh"),
        ceiling_duct:Model::from_obj_mtl(include_str!("../assets/models/env_ceiling_duct.obj"),
            include_str!("../assets/models/env_ceiling_duct.mtl")).expect("valid ceiling_duct environment mesh"),
        wall_sign:Model::from_obj_mtl(include_str!("../assets/models/env_wall_sign.obj"),
            include_str!("../assets/models/env_wall_sign.mtl")).expect("valid wall_sign environment mesh"),
    }}
    pub fn draw(&self,fb:&mut Framebuffer,world:&World,r:&mut Rasterizer){
        // Opaque underside of a physically enclosed station. (The ceiling is
        // a visible surface rather than an artificial sky gradient.)
        let ceiling=match world.stage{
            0=>0xff273946,1=>0xff413b38,2=>0xff2a3549,
            3=>0xff243e4b,_=>0xff302d46,
        };
        let lit=match world.stage{
            0=>0xff4ed6e8,1=>0xffefa95d,2=>0xffb18bff,
            3=>0xff6be4ee,_=>0xffed6daa,
        };
        let top=4.76;
        r.quad(fb,V3::new(0.0,top,0.0),V3::new(ARENA,top,0.0),
            V3::new(ARENA,top,ARENA),V3::new(0.0,top,ARENA),Material::two_sided(ceiling,false));
        // Bulkhead ribs, modular ceiling joints and service ducts.
        for step in 0..=18 {
            let v=step as f32*2.0;
            r.box3(fb,Box3::new(v-0.040,4.67,0.0,0.080,0.06,ARENA),Material::lit(0xff131f2c));
            r.box3(fb,Box3::new(0.0,4.67,v-0.040,ARENA,0.06,0.080),Material::lit(0xff131f2c));
        }
        for decor in &world.decor {
            if (decor.pos-world.foot).horizontal().len()>22.0{continue}
            let model=match decor.kind {
                DecorKind::CeilingLight=>&self.ceiling_light,
                DecorKind::HangingLamp=>&self.hanging_lamp,
                DecorKind::VentFan=>&self.vent_fan,
                DecorKind::Terminal=>&self.terminal,
                DecorKind::ConsoleBank=>&self.console_bank,
                DecorKind::ServerRack=>&self.server_rack,
                DecorKind::PipeBundle=>&self.pipe_bundle,
                DecorKind::CoolantTank=>&self.coolant_tank,
                DecorKind::Crate=>&self.storage_crate,
                DecorKind::Chair=>&self.chair,
                DecorKind::Desk=>&self.desk,
                DecorKind::WallMonitor=>&self.wall_monitor,
                DecorKind::WarningBeacon=>&self.warning_beacon,
                DecorKind::FloorGrate=>&self.floor_grate,
                DecorKind::CeilingDuct=>&self.ceiling_duct,
                DecorKind::WallSign=>&self.wall_sign,
            };
            let mut transform=Transform::at_scale(decor.pos,1.0);
            transform.rotation.y=decor.rotation;
            model.draw_world(r,fb,transform,None);
            if matches!(decor.kind,DecorKind::CeilingLight|DecorKind::HangingLamp){
                // Stylized local illumination footprint on the floor beneath
                // the luminaire; no global dynamic point-light computation.
                let p=decor.pos;
                let faint=if (world.time*4.5 + p.x*3.0).sin()>-0.98{lit}else{0xff263747};
                r.quad(fb,V3::new(p.x-1.10,0.012,p.z-0.36),V3::new(p.x+1.10,0.012,p.z-0.36),
                    V3::new(p.x+1.10,0.012,p.z+0.36),V3::new(p.x-1.10,0.012,p.z+0.36),
                    Material::two_sided(faint,true));
            }
        }
        for site in &world.sites {
            if (site.pos-world.foot).horizontal().len()>22.0{continue}
            let p=site.pos;
            match site.kind {
                SiteKind::Record(i)=>{
                    self.draw_model(r,fb,&self.record_station,p,0.0);
                    if !world.records_found[i] {
                        r.box3(fb,Box3::new(p.x-0.52,1.36,p.z-0.32,1.04,0.055,0.035),
                            Material::glow(0xffd494ff));
                    }
                }
                SiteKind::Relay=>{
                    self.draw_model(r,fb,&self.relay,p,0.0);
                    let on=if site.used{0xff45f9b5}else{0xffffb261};
                    r.box3(fb,Box3::new(p.x-0.31,1.46,p.z-0.41,0.62,0.13,0.06),Material::glow(on));
                    // Mechanical switch lever visually drops when operated.
                    let lever_y=if site.used{1.65}else{2.02};
                    r.box3(fb,Box3::new(p.x-0.06,lever_y,p.z-0.42,0.12,0.15,0.17),Material::lit(on));
                }
                SiteKind::Key=>{
                    self.draw_model(r,fb,&self.terminal,p,0.0);
                    if !site.used {
                        let mut pose=Transform::at_scale(p+V3::new(0.0,2.35+(world.time*3.0).sin()*0.09,0.0),0.35);
                        pose.rotation.y=world.time;
                        self.record_station.draw_world(r,fb,pose,Some(Material::glow(0xff6ea9ff)));
                    }
                }
                SiteKind::Exit|SiteKind::Uplink=>{
                    // Animated, powered double sliding airlock. Objective locks
                    // control panels independently from world collision.
                    self.draw_model(r,fb,&self.door_frame,p,0.0);
                    let opened=world.objective_ready();
                    let spread=if opened{1.02}else{0.0};
                    self.draw_model(r,fb,&self.door_panel,p+V3::new(-0.50-spread,0.0,0.04),0.0);
                    self.draw_model(r,fb,&self.door_panel,p+V3::new(0.50+spread,0.0,0.04),0.0);
                    r.box3(fb,Box3::new(p.x-0.39,3.15,p.z-0.20,0.78,0.09,0.08),
                        Material::glow(if opened{0xff60edab}else{0xffff607c}));
                    if site.kind==SiteKind::Uplink{
                        self.draw_model(r,fb,&self.uplink,p+V3::new(2.0,0.0,0.0),0.0);
                    }
                }
            }
        }
    }
    fn draw_model(&self,r:&mut Rasterizer,fb:&mut Framebuffer,model:&Model,pos:V3,rot:f32){
        let mut pose=Transform::at_scale(pos,1.0);pose.rotation.y=rot;
        model.draw_world(r,fb,pose,None);
    }
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn environment_models_are_embedded(){
        let e=Environment::new();
        assert!(e.terminal.triangle_count()>120);
        assert!(e.server_rack.triangle_count()>200);
        assert!(e.door_panel.triangle_count()>30);
    }
}
