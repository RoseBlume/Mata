//! Software 2.5D raycaster. No GPU API or other game framework.
use std::{fs::File, path::Path, collections::HashMap};
use mata_win::Framebuffer;
use mata_ui::widgets::FontRenderer;
use mata_3d::{Camera, Instance, Mesh, SoftwareRenderer, Vec3};
use mata_image::Image;
use crate::{world::{World,Vec2},levels::{self,PickupKind,EnemyKind},campaign::{Campaign,Screen},lore,story};

const FOV_SCALE:f32=0.66;
const TEX_SIZE:usize=64;
const SHADE_LEVELS:usize=40;
#[derive(Clone)]
pub struct Texture{pixels:Vec<u32>,lit:Vec<u32>,w:usize,h:usize}
impl Texture {
    fn read(name:&str)->Self {
        let path=Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/textures").join(format!("{name}.png"));
        if let Ok(mut file)=File::open(path){
            if let Ok(img)=Image::from_reader(&mut file){
                if img.width>0 && img.height>0 && img.pixels.len()==(img.width as usize)*(img.height as usize){
                    return Self::with_shades(img.pixels,img.width as usize,img.height as usize);
                }
            }
        }
        // A missing asset should not prevent opening the game.
        Self::with_shades(vec![0x778899ff;TEX_SIZE*TEX_SIZE],TEX_SIZE,TEX_SIZE)
    }
    fn with_shades(pixels:Vec<u32>,w:usize,h:usize)->Self {
        let size=pixels.len();
        let mut lit=vec![0u32;size*SHADE_LEVELS];
        for shade in 0..SHADE_LEVELS {
            let factor=shade as f32*1.2/(SHADE_LEVELS-1) as f32;
            for (dst,&rgba) in lit[shade*size..(shade+1)*size].iter_mut().zip(&pixels) {
                *dst=tint(rgba,factor);
            }
        }
        Self{pixels,lit,w,h}
    }
    #[inline]fn px(&self,x:usize,y:usize)->u32{self.pixels[(y%self.h)*self.w+(x%self.w)]}
    #[inline]fn lit(&self,x:usize,y:usize,shade:usize)->u32 {
        self.lit[shade.min(SHADE_LEVELS-1)*self.pixels.len()+y.min(self.h-1)*self.w+x.min(self.w-1)]
    }
}

pub struct Renderer {wall:Texture,brick:Texture,door:Texture,locked:Texture,terminal:Texture,powered:Texture,floor:Texture,lift:Texture,corewall:Texture,corefloor:Texture,archive:Texture,archivist:Texture,enemy:Texture,hunter:Texture,warden:Texture,ammo:Texture,health:Texture,key:Texture,exit:Texture,weapon:Texture,models:SoftwareRenderer,ammo_mesh:Mesh,health_mesh:Mesh,key_mesh:Mesh,lift_beacon:Mesh,mesh_enabled:bool,story_cache:HashMap<(usize,usize),Vec<String>>}
impl Renderer {
    pub fn new()->Self {Self{
        wall:Texture::read("wall"),brick:Texture::read("brick"),door:Texture::read("door"),locked:Texture::read("locked"),terminal:Texture::read("terminal"),powered:Texture::read("powered"),floor:Texture::read("floor"),lift:Texture::read("lift"),corewall:Texture::read("corewall"),corefloor:Texture::read("corefloor"),archive:Texture::read("archive"),archivist:Texture::read("archivist"),
        enemy:Texture::read("sentry"),hunter:Texture::read("hunter"),warden:Texture::read("warden"),ammo:Texture::read("ammo"),health:Texture::read("health"),key:Texture::read("key"),exit:Texture::read("exit"),weapon:Texture::read("blaster"),
        models:SoftwareRenderer::new(),
        ammo_mesh:Mesh::from_obj_str(
            include_str!("../assets/models/energy_cartridge.obj"),0xff72d6ff)
            .unwrap_or_else(|_|Mesh::prism(8,0xff72d6ff)),
        health_mesh:Mesh::octahedron(0xff54ff9b),
        key_mesh:Mesh::octahedron(0xffffd36f),
        lift_beacon:Mesh::cube(0xffe5a84d),
        mesh_enabled:true,story_cache:HashMap::new()
    }}
    /// Optional runtime switch: F2 toggles the mesh pass for profiling.
    pub fn toggle_meshes(&mut self) { self.mesh_enabled=!self.mesh_enabled; }
    pub fn render(&mut self,fb:&mut Framebuffer,campaign:&mut Campaign,font:&mut Option<FontRenderer>,minimap:bool,fps:f32){
        let world=&campaign.world;
        let w=fb.width as usize;let h=fb.height as usize;
        if w==0||h==0{return}
        self.models.begin_frame(fb);
        self.background(fb,world);
        let dirx=world.facing.cos();let diry=world.facing.sin();
        let planex=-diry*FOV_SCALE;let planey=dirx*FOV_SCALE;
        let eye=world.player_z+0.5;
        for x in 0..w {
            let camera_x=2.0*x as f32/w as f32-1.0;
            let ray_x=dirx+planex*camera_x;let ray_y=diry+planey*camera_x;
            let mut cell_x=world.player.x.floor() as i32;let mut cell_y=world.player.y.floor() as i32;
            let delta_x=if ray_x.abs()<1e-6{1e20}else{(1.0/ray_x).abs()};
            let delta_y=if ray_y.abs()<1e-6{1e20}else{(1.0/ray_y).abs()};
            let (step_x,mut side_x)=if ray_x<0.0{(-1,(world.player.x-cell_x as f32)*delta_x)}else{(1,((cell_x+1) as f32-world.player.x)*delta_x)};
            let (step_y,mut side_y)=if ray_y<0.0{(-1,(world.player.y-cell_y as f32)*delta_y)}else{(1,((cell_y+1) as f32-world.player.y)*delta_y)};
            let mut vertical=false;let mut material=b'#';
            // Store up to six raised faces per ray without a heap allocation.
            let mut faces=[(0.0f32,0.0f32,0.0f32);6];
            let mut face_count=0usize;
            for _ in 0..96{
                if side_x<side_y{side_x+=delta_x;cell_x+=step_x;vertical=true;}else{side_y+=delta_y;cell_y+=step_y;vertical=false;}
                let c=world.cell(cell_x,cell_y);
                if c==b'P'||c==b'U' {
                    let z=world.floor_height(cell_x,cell_y);
                    if z>0.02 && face_count<faces.len() {
                        let d=(if vertical{side_x-delta_x}else{side_y-delta_y}).max(0.09);
                        let pos=if vertical{world.player.y+d*ray_y}else{world.player.x+d*ray_x};
                        faces[face_count]=(d,z,pos-pos.floor());face_count+=1;
                    }
                }
                if matches!(c,b'#'|b'B'|b'D'|b'L'|b'T'|b't'|b'N'|b'n'){material=c;break}
            }
            let dist=(if vertical{side_x-delta_x}else{side_y-delta_y}).max(0.09);

            let wall_h=(h as f32/dist).max(1.0);
            let top=h as f32*0.5+(eye-1.0)*wall_h;
            let y1=top.max(0.0) as usize;
            let y2=(h as f32*0.5+eye*wall_h).min(h as f32).max(0.0) as usize;
            let mut hit=if vertical{world.player.y+dist*ray_y}else{world.player.x+dist*ray_x};
            hit-=hit.floor();
            let texture=match material{b'D'=>&self.door,b'L'=>&self.locked,b'T'=>&self.terminal,b't'=>&self.powered,b'B'=>&self.brick,b'N'|b'n'=>&self.archive,b'#' if world.level==4=>&self.corewall,_=>&self.wall};
            let mut tx=(hit*texture.w as f32) as usize;
            if (vertical&&ray_x>0.0)||(!vertical&&ray_y<0.0){tx=texture.w.saturating_sub(1).saturating_sub(tx);}
            let shade=(1.0/(1.0+dist*0.115)).clamp(0.17,1.0)*if vertical{0.78}else{1.0};
            let shade_id=shade_index(shade);
            let tex_step=texture.h as f32/wall_h;
            let mut tex_pos=(y1 as f32-top)*tex_step;
            for y in y1..y2 {
                fb.pixels[y*w+x]=texture.lit(tx,tex_pos.max(0.0) as usize,shade_id);
                tex_pos+=tex_step;
            }
            self.models.draw_wall_span(x,y1,y2,dist);
            // Raised platforms are solid visual side faces, but their tops
            // remain walkable. Draw near faces over far ones.
            for &(d,z,hit) in faces[..face_count].iter().rev() {
                let projection=h as f32/d;
                let face_top=h as f32*0.5+(eye-z)*projection;
                let face_bottom=h as f32*0.5+eye*projection;
                let begin=face_top.max(0.0).min(h as f32) as usize;
                let end=face_bottom.max(0.0).min(h as f32) as usize;
                let tx=((hit* self.lift.w as f32) as usize).min(self.lift.w-1);
                let k=shade_index((0.85/(1.0+d*0.12)).max(0.19));
                let mut tex_pos=(begin as f32-face_top)*self.lift.h as f32 / (z*projection).max(0.01);
                let step=self.lift.h as f32/(z*projection).max(0.01);
                for y in begin..end {
                    fb.pixels[y*w+x]=self.lift.lit(tx,tex_pos.max(0.0) as usize,k);
                    tex_pos+=step;
                }
                self.models.draw_wall_span(x,begin,end,d);
            }
        }
        if self.mesh_enabled {self.draw_models(fb,world);}
        self.draw_sprites(fb,world,dirx,diry,planex,planey);
        self.overlay(fb,campaign,font,minimap,fps);
    }
    fn background(&mut self,fb:&mut Framebuffer,world:&World){
        let w=fb.width as usize;let h=fb.height as usize;
        let horizon=h/2;
        // Wall ceiling: simple ambient distance gradient and overhead light bars.
        for y in 0..horizon {
            let t=y as f32/horizon.max(1) as f32;
            let base = 21.0_f32 + t * 25.0;
            let ambient=levels::mission(world.level).atmosphere;
            let ar=((ambient>>16)&255) as f32;let ag=((ambient>>8)&255) as f32;let ab=(ambient&255) as f32;
            let color=0xff000000|(((ar+base*0.30).min(255.0) as u32)<<16)|(((ag+base*0.35).min(255.0) as u32)<<8)|((ab+base*0.42).min(255.0) as u32);
            let row=&mut fb.pixels[y*w..(y+1)*w];row.fill(color);
            if y%59<2 && y<horizon/2 { for (x,p) in row.iter_mut().enumerate(){if x%120>45&&x%120<87{*p=0xff294357}} }
        }
        let dirx=world.facing.cos();let diry=world.facing.sin();
        let leftx=dirx+diry*FOV_SCALE;let lefty=diry-dirx*FOV_SCALE;
        let rightx=dirx-diry*FOV_SCALE;let righty=diry+dirx*FOV_SCALE;
        for y in horizon..h {
            let p=(y as f32-h as f32*0.5+0.5).max(0.5);
            let eye=0.5+world.player_z;
            let dist=(eye*h as f32)/p;
            let step_x=dist*(rightx-leftx)/w as f32;
            let step_y=dist*(righty-lefty)/w as f32;
            let mut fx=world.player.x+dist*leftx;
            let mut fy=world.player.y+dist*lefty;
            let k=shade_index((0.98/(1.0+dist*0.12)).max(0.16));
            self.models.fill_row_depth(y,dist);
            let row=&mut fb.pixels[y*w..(y+1)*w];
            let normal_floor=if world.level==4{&self.corefloor}else{&self.floor};
            for (x,px) in row.iter_mut().enumerate(){
                let tx=fx.floor() as i32;let ty=fy.floor() as i32;
                let z=world.floor_height(tx,ty);
                if z>0.02 && eye>z {
                    // Cast onto a raised plane rather than the ground.
                    let ratio=(eye-z)/eye;
                    let sx=world.player.x+(fx-world.player.x)*ratio;
                    let sy=world.player.y+(fy-world.player.y)*ratio;
                    if world.floor_height(sx.floor() as i32,sy.floor() as i32)>0.02 {
                        let raised_depth=(eye-z)*h as f32/p;
                        self.models.set_depth(x,y,raised_depth);
                        *px=self.lift.lit(((sx-sx.floor())*self.lift.w as f32) as usize,
                            ((sy-sy.floor())*self.lift.h as f32) as usize,k);
                    } else {
                        *px=normal_floor.lit(((fx-fx.floor())*normal_floor.w as f32) as usize,
                            ((fy-fy.floor())*normal_floor.h as f32) as usize,k);
                    }
                } else {
                    *px=normal_floor.lit(((fx-fx.floor())*normal_floor.w as f32) as usize,
                        ((fy-fy.floor())*normal_floor.h as f32) as usize,k);
                }
                fx+=step_x;fy+=step_y;
            }
        }
    }
    /// Actual CPU-rasterized triangles; billboards still handle living enemies.
    /// Pickups share the original World positions and disappear when collected.
    fn draw_models(&mut self,fb:&mut Framebuffer,world:&World){
        let eye=world.player_z+0.5;
        let camera=Camera::raycast(Vec3::new(world.player.x,world.player.y,eye),world.facing,FOV_SCALE);
        for pickup in &world.pickups {
            if !pickup.active{continue;}
            let mesh=match pickup.kind {
                PickupKind::Ammo=>&self.ammo_mesh,
                PickupKind::Health=>&self.health_mesh,
                PickupKind::Key=>&self.key_mesh,
            };
            let dist2=(pickup.pos.x-world.player.x).powi(2)+(pickup.pos.y-world.player.y).powi(2);
            if dist2>18.0*18.0 {continue;}
            let floor=world.floor_height(pickup.pos.x.floor() as i32,pickup.pos.y.floor() as i32);
            let bob=0.07*(world.time*3.4).sin();
            let instance=Instance::new(mesh,Vec3::new(pickup.pos.x,pickup.pos.y,floor+0.48+bob))
                .scaled(Vec3::new(0.50,0.50,0.56)).rotated(world.time*1.45).pitched(0.17);
            self.models.draw_instance(fb,&camera,instance);
        }
        // Four small 3D signal lamps follow each animated lift. The lift base
        // itself remains in the existing raycast geometry and collision model.
        for lift in &world.lifts {
            for (dx,dy) in [(-0.36,-0.36),(0.36,-0.36),(-0.36,0.36),(0.36,0.36)] {
                let x=lift.x as f32+0.5+dx;let y=lift.y as f32+0.5+dy;
                let instance=Instance::new(&self.lift_beacon,Vec3::new(x,y,lift.height+0.085))
                    .scaled(Vec3::new(0.11,0.11,0.17));
                self.models.draw_instance(fb,&camera,instance);
            }
        }
    }

    fn draw_sprites(&self,fb:&mut Framebuffer,world:&World,dirx:f32,diry:f32,planex:f32,planey:f32){
        #[derive(Clone,Copy)]
        enum Kind{Enemy(EnemyKind),Ammo,Health,Key,Exit}
        let mut sprites:Vec<(Vec2,Kind,f32)>=Vec::with_capacity(world.enemies.len()+world.pickups.len()+1);
        for e in &world.enemies {if e.active{sprites.push((e.pos,Kind::Enemy(e.kind),
                if e.kind==EnemyKind::Archivist {2.0+(world.time*2.0).sin()*0.06}
                else {1.0+(world.time*3.0+e.phase).sin()*0.028}));}}
        // In profiling mode, restore the original billboards so essential
        // consumables remain visible when the mesh renderer is disabled.
        if !self.mesh_enabled {
            for p in &world.pickups {
                if p.active {
                    let kind=match p.kind {PickupKind::Ammo=>Kind::Ammo,
                        PickupKind::Health=>Kind::Health,PickupKind::Key=>Kind::Key};
                    sprites.push((p.pos,kind,0.6));
                }
            }
        }
        sprites.push((world.exit,Kind::Exit,1.05));
        sprites.sort_by(|a,b|{
            let da=(a.0.x-world.player.x).powi(2)+(a.0.y-world.player.y).powi(2);
            let db=(b.0.x-world.player.x).powi(2)+(b.0.y-world.player.y).powi(2);
            db.total_cmp(&da)
        });
        let det=planex*diry-dirx*planey;if det.abs()<1e-7{return}
        let inv=1.0/det;let w=fb.width as i32;let h=fb.height as i32;
        for (pos,kind,scale) in sprites{
            let rx=pos.x-world.player.x;let ry=pos.y-world.player.y;
            let tx=inv*(diry*rx-dirx*ry);
            let depth=inv*(-planey*rx+planex*ry);
            if depth<=0.12||depth>24.0{continue}
            let cx=((w as f32)*0.5*(1.0+tx/depth)) as i32;
            let sw=((h as f32/depth)*scale).clamp(1.0,5000.0) as i32;
            let sh=sw;
            let bob=match kind {Kind::Ammo|Kind::Health|Kind::Key=>
                (world.time*3.0).sin()*6.0,_=>0.0};
            let cy=(h as f32*0.5+(h as f32/depth)*(0.15+world.player_z)+bob) as i32;
            let left=cx-sw/2;let top=cy-sh/2;
            if left>=w||left+sw<0||top>=h||top+sh<0 {continue}
            let tex=match kind{Kind::Enemy(EnemyKind::Sentry)=>&self.enemy,Kind::Enemy(EnemyKind::Hunter)=>&self.hunter,Kind::Enemy(EnemyKind::Warden)=>&self.warden,Kind::Enemy(EnemyKind::Archivist)=>&self.archivist,Kind::Ammo=>&self.ammo,Kind::Health=>&self.health,Kind::Key=>&self.key,Kind::Exit=>&self.exit};
            let shade=if matches!(kind,Kind::Enemy(EnemyKind::Archivist)) {
                if world.relays_activated<levels::mission(world.level).relays {0.88} else {1.15}
            } else {(1.05/(1.0+depth*0.095)).clamp(0.22,1.0)};
            for x in left.max(0)..(left+sw).min(w){
                let sx=((x-left) as usize*tex.w/sw as usize).min(tex.w-1);
                for y in top.max(0)..(top+sh).min(h){
                    let sy=((y-top) as usize*tex.h/sh as usize).min(tex.h-1);
                    let rgba=tex.px(sx,sy);
                    let alpha=(rgba&255) as u8;
                    if alpha<95{continue}
                    // Test against walls, lifts, floor and the new 3D objects
                    // at the exact pixel instead of the raycaster's whole-column Z.
                    if depth>=self.models.depth_at(x as usize,y as usize){continue;}
                    fb.pixels[y as usize*w as usize+x as usize]=tint(rgba,shade);
                }
            }
        }
    }
    fn overlay(&mut self,fb:&mut Framebuffer,campaign:&mut Campaign,font:&mut Option<FontRenderer>,minimap:bool,fps:f32){
        let world=&campaign.world;
        let w=fb.width as i32;let h=fb.height as i32;
        // Transparent UI backing areas use opaque palette on purpose.
        rect(fb,0,h-82,w,82,0xff081420);
        rect(fb,0,h-82,w,3,0xff00c3c9);
        // Crosshair and weapon sway.
        let cx=(w as f32*(0.5+world.aim_x*0.5)).round() as i32;
        let cy=h/2;
        let cross=if world.muzzle>0.0{0xffffd37a}else{0xffe0f3de};
        rect(fb,cx-1,cy-13,2,8,cross);rect(fb,cx-1,cy+5,2,8,cross);
        rect(fb,cx-13,cy-1,8,2,cross);rect(fb,cx+5,cy-1,8,2,cross);
        let gun_scale=(w as f32/960.0).clamp(0.75,1.8);
        let gun_w=(270.0*gun_scale) as i32;
        let gun_h=(250.0*gun_scale) as i32;
        let bob=if world.moving{(world.time*11.0).sin()*5.0*gun_scale}else{(world.time*2.3).sin()*2.0};
        let recoil=if world.muzzle>0.0{18.0*gun_scale}else{0.0};
        blit_scaled(fb,&self.weapon,w/2-gun_w/2,h-66-gun_h+(bob+recoil) as i32,gun_w,gun_h);
        if world.muzzle>0.0 {rect(fb,cx-3,cy-31,6,8,0xffffeec7);}
        // HUD frames and colored health/ammo meters.
        rect(fb,20,h-65,175,14,0xff142c3c);
        rect(fb,20,h-65,175*world.health/100,14,if world.health>35{0xff25dca7}else{0xffff6577});
        rect(fb,20,h-35,175,8,0xff142c3c);
        rect(fb,20,h-35,(175*world.ammo.min(99)/99).max(0),8,0xff62acee);
        text(font,fb,&format!("ARMOR  {:03}%",world.health),20,h-80,17,0xffe6f8ff);
        text(font,fb,&format!("ENERGY  {:02}",world.ammo),20,h-49,14,0xffb3dfff);
        text(font,fb,&format!("SCORE {:06}",world.score),w-188,h-75,17,0xffffd376);
        text(font,fb,&format!("KILLS {}/{}",world.kills,world.enemies.len()),w-188,h-49,15,0xffd9e8f3);
        text(font,fb,&format!("TOTAL KILLS {}",campaign.completed_kills+world.kills),w-188,h-26,13,0xff82fff4);
        if world.notice_t>0.0 {let len=world.notice.len() as i32*10;let nx=(w-len).max(0)/2;
            text(font,fb,&world.notice,nx,15,18,0xffffd9a2);
        }
        text(font,fb,&format!("MISSION {} / {}  -  {}",world.level+1,levels::MISSIONS.len(),levels::mission(world.level).name),18,h-106,14,0xffcce8ff);
        if let Some((hp,total))=world.boss_health(){
            let barw=(w-440).clamp(140,370);let bx=(w-barw)/2;
            rect(fb,bx-3,66,barw+6,30,0xff060b1e);
            rect(fb,bx,74,barw,11,0xff342247);
            rect(fb,bx,74,barw*hp/total,11,if world.relays_activated<3{0xffa386eb}else{0xffff8764});
            let label=if hp==0{"ARCHIVIST - SILENCED"}
                else if world.relays_activated<3{"ARCHIVIST - SHIELDED"}
                else if world.boss_phase>=3{"ARCHIVIST - CORE OVERLOAD"}
                else if world.boss_phase>=2{"ARCHIVIST - FRACTURED"}
                else {"ARCHIVIST - EXPOSED"};
            text(font,fb,label,bx,52,15,0xffffdcbe);
            if world.boss_charge>0.0 {
                let flash=if (world.time*13.0) as i32%2==0{0xffffb460}else{0xffe15b5b};
                rect(fb,bx,90,barw,3,flash);
                text(font,fb,"WARNING: TARGET LOCK - MOVE!",bx,99,15,flash);
            }
        }
        if campaign.screen==Screen::Playing {
            let nearby=world.cells.iter().enumerate().any(|(yy,row)|row.iter().enumerate().any(|(xx,&v)|{
                matches!(v,b'N'|b'n'|b'T'|b'L'|b'D'|b'P') && world.player.distance(Vec2::new(xx as f32+0.5,yy as f32+0.5))<1.55
            }));
            if nearby{text(font,fb,"[ E ]  INTERACT / OPERATE LIFT",w/2-145,h-142,14,0xffe6c3ff);}
            text(font,fb,&format!("ELEVATION {:.1}M",world.player_z),w-182,h-142,14,0xff80eada);
            text(font,fb,"J - JOURNAL",w-122,h-118,13,0xffc7bbdf);
        }
        if world.boss_flash>0.0 {
            let color=0xffff9b5d;
            rect(fb,0,0,w,7,color);rect(fb,0,h-7,w,7,color);
        }
        let objective=world.objective_summary();
        text(font,fb,&objective,(w/2-((objective.len() as i32)*4)).max(210),34,13,0xffffd49a);
        text(font,fb,&format!("{:02.0} FPS",fps.min(999.0)),w-89,9,13,0xff9eaeb9);
        let mesh_status=if self.mesh_enabled {
            format!("F2 3D  {} TRI",self.models.stats().triangles)
        } else {"F2 3D OFF".to_string()};
        text(font,fb,&mesh_status,w-180,25,12,0xff8cddf4);
        if minimap{draw_minimap(fb,world);}
        if world.hurt_flash>0.0 {
            let t=(world.hurt_flash/0.3*0.62).min(0.62);
            for x in 0..w {rect(fb,x,0,1,10,0xffa8384b);rect(fb,x,h-8,1,8,0xffa8384b)}
            // Thin red edge, not a full-screen flash obscuring gameplay.
            if t>0.1{rect(fb,0,0,9,h,0xffa8384b);rect(fb,w-9,0,9,h,0xffa8384b)}
        }
        if campaign.screen!=Screen::Playing {
            self.campaign_panel(fb,campaign,font);
        }
    }
    fn campaign_panel(&mut self,fb:&mut Framebuffer,campaign:&mut Campaign,font:&mut Option<FontRenderer>){
        if campaign.screen==Screen::Story || campaign.screen==Screen::StoryIndex {
            self.story_panel(fb,campaign,font);
            return;
        }
        let w=fb.width as i32;let h=fb.height as i32;
        let panel_w=(w-24).min(860).max(1);
        let panel_h=(h-28).min(515).max(1);
        let x=(w-panel_w)/2;let y=(h-panel_h)/2;
        rect(fb,x,y,panel_w,panel_h,0xff0a192b);
        rect(fb,x,y,panel_w,4,0xff34decd);
        rect(fb,x+10,y+10,panel_w-20,1,0xff31425f);
        let tx=x+24;let mut ty=y+22;
        let mut line=|message:&str,size:u16,color:u32,spacing:i32,fb:&mut Framebuffer,font:&mut Option<FontRenderer>|{
            text(font,fb,message,tx,ty,size,color);
            ty+=spacing;
        };
        let m=levels::mission(campaign.world.level);
        let teal=0xff82f1d7;let gold=0xffffd18b;let body=0xffcee0f1;let faded=0xff9cb7ca;
        match campaign.screen{
            Screen::Title=>{
                line("MATA // OUTPOST ZERO",31,gold,49,fb,font);
                line("THE ECHO DIRECTIVE  |  FIVE-ACT CAMPAIGN",18,teal,44,fb,font);
                line("They called it a station. It became a memory.",18,body,35,fb,font);
                line("Enter ASTRAEUS. Find the missing crew.",17,body,34,fb,font);
                line("Discover what the Archive has done.",17,body,54,fb,font);
                line("ENTER / N  -  NEW CAMPAIGN",19,gold,34,fb,font);
                if campaign.has_save {
                    line("C  -  CONTINUE CAMPAIGN",19,teal,30,fb,font);
                    line(&format!("CHECKPOINT: MISSION {} / {}",campaign.saved_level()+1,levels::MISSIONS.len()),15,body,28,fb,font);
                }
                if campaign.finished{line("STORY COMPLETED - FREE TO REPLAY",15,teal,28,fb,font);}
                line("B - STORY LIBRARY   |   ESC - EXIT",15,teal,30,fb,font);
            }
            Screen::Prologue=>{
                line("PROLOGUE // THE FALSE CALL",26,gold,43,fb,font);
                for entry in lore::PROLOGUE{line(entry,16,body,36,fb,font);}
                line("ENTER  -  LISTEN TO THE BRIEFING",18,teal,34,fb,font);
            }
            Screen::Briefing=>{
                line(&format!("ACT {} / {}   |   {}",campaign.world.level+1,levels::MISSIONS.len(),m.location),17,teal,32,fb,font);
                line(m.name,26,gold,42,fb,font);
                line("FROM THE STATION MANUSCRIPT",14,teal,26,fb,font);
                for entry in story::opening_lines(campaign.world.level+1,67,3){line(&entry,15,body,27,fb,font);}
                line("OPERATION ORDERS",17,teal,33,fb,font);
                for detail in m.briefing{line(detail,16,body,27,fb,font);}
                line("Three optional archives are hidden in this sector.",14,faded,28,fb,font);
                line("B - COMPLETED CHAPTERS / STORY LIBRARY",14,teal,27,fb,font);
                line("ENTER  -  DEPLOY     M  -  TITLE",18,gold,27,fb,font);
            }
            Screen::Paused=>{
                line("MISSION PAUSED",30,gold,56,fb,font);
                line(&format!("{}  |  ACT {} / {}",m.name,campaign.world.level+1,levels::MISSIONS.len()),17,body,44,fb,font);
                line("ESC / ENTER  -  RESUME",19,teal,38,fb,font);
                line("J  -  OPEN EVIDENCE JOURNAL",18,body,36,fb,font);
                line("B  -  OPEN STORY LIBRARY",17,body,36,fb,font);
                line("R  -  RESTART FROM CHECKPOINT",17,body,36,fb,font);
                line("M  -  MAIN MENU",17,body,36,fb,font);
            }
            Screen::Journal=>{
                let record=&lore::RECORDS[campaign.journal_entry];
                line(&format!("ASTRAEUS RECORD {:02} / {}",campaign.journal_entry+1,lore::RECORDS.len()),17,teal,31,fb,font);
                line(record.title,25,gold,40,fb,font);
                line(&format!("FROM: {}",record.author),16,body,29,fb,font);
                line(record.stamp,15,faded,41,fb,font);
                let first=campaign.journal_page*lore::LINES_PER_PAGE;
                for entry in record.lines.iter().skip(first).take(lore::LINES_PER_PAGE){line(entry,17,body,31,fb,font);}
                let page_count=record.lines.len().div_ceil(lore::LINES_PER_PAGE);
                let footer_y=y+panel_h-70;
                text(font,fb,&format!("PAGE {} / {}    |    DISCOVERED {} / {}",campaign.journal_page+1,page_count,campaign.lore_mask.count_ones(),lore::RECORDS.len()),tx,footer_y,15,teal);
                text(font,fb,"UP/DOWN PAGE   LEFT/RIGHT RECORD   E/J/ESC CLOSE",tx,footer_y+29,14,gold);
            }
            Screen::Debrief=>{
                line("CHAPTER COMPLETE",29,teal,39,fb,font);
                line(m.name,22,gold,37,fb,font);
                line(&format!("SCORE +{}  |  KILLS {}  |  TIME {:.1} SEC",campaign.level_score,campaign.world.kills,campaign.last_level_time),17,body,35,fb,font);
                line("NEWLY DECODED TRANSMISSION",17,teal,33,fb,font);
                for detail in m.revelation{line(detail,17,body,32,fb,font);}
                line("",12,body,19,fb,font);
                line(&format!("CAMPAIGN SCORE: {}",campaign.world.score),18,gold,34,fb,font);
                line(&campaign.save_status,15,teal,32,fb,font);
                line("B  -  READ THE COMPLETED STORY CHAPTER",15,teal,27,fb,font);
                line("ENTER  -  NEXT ACT",19,gold,32,fb,font);
            }
            Screen::GameOver=>{
                line("SIGNAL LOST",32,0xffff938d,64,fb,font);
                line("The last mission checkpoint remains intact.",18,body,42,fb,font);
                line("EVIDENCE ALREADY RECOVERED IS SAFE.",18,teal,49,fb,font);
                line("R / ENTER  -  RETRY",19,gold,43,fb,font);
                line("M  -  RETURN TO TITLE",17,body,37,fb,font);
            }
            Screen::Ending=>{
                line("EPILOGUE // THE FIRST FREE SIGNAL",25,teal,42,fb,font);
                for entry in lore::EPILOGUE{line(entry,16,body,32,fb,font);}
                line(&format!("FINAL SCORE: {}  |  TOTAL KILLS: {}",campaign.world.score,campaign.completed_kills),16,gold,30,fb,font);
                line(&format!("EVIDENCE FOUND: {} / {}  |  BEST: {}",campaign.lore_mask.count_ones(),lore::RECORDS.len(),campaign.best_score),16,gold,32,fb,font);
                line("B  -  READ ALL NINE STORY SECTIONS",16,gold,29,fb,font);
                line("ENTER  -  RETURN TO TITLE",18,teal,28,fb,font);
            }
            Screen::Playing|Screen::Story|Screen::StoryIndex=>{}
        }
    }

    /// Full-screen readable manuscript with dynamic wrapping and cached lines.
    /// Text is never raster-upscaled on resize; page lengths respond to the
    /// actual window dimensions and stay within the visible panel.
    fn story_panel(&mut self,fb:&mut Framebuffer,campaign:&mut Campaign,font:&mut Option<FontRenderer>){
        let w=fb.width as i32;let h=fb.height as i32;
        let pw=(w-18).min(1050).max(1);let ph=(h-18).min(720).max(1);
        let x=(w-pw)/2;let y=(h-ph)/2;
        rect(fb,x,y,pw,ph,0xff081727);
        rect(fb,x,y,pw,5,0xff52e5d0);
        rect(fb,x+16,y+62,pw-32,1,0xff29465b);
        let teal=0xff8cefdc;let gold=0xffffd08c;
        let body=0xffd2dfed;let gray=0xff9cb7c7;
        if campaign.screen==Screen::StoryIndex {
            text(font,fb,"THE ASTRAEUS STORY ARCHIVE",x+26,y+22,24,gold);
            text(font,fb,"Chapters unlock as the campaign progresses.",x+26,y+78,16,body);
            let gap=((ph-167)/story::CHAPTER_TITLES.len() as i32).clamp(16,38);
            for (index,title) in story::CHAPTER_TITLES.iter().enumerate(){
                let cy=y+115+index as i32*gap;
                let selected=index==campaign.story_selection;
                if selected {rect(fb,x+20,cy-5,pw-40,gap,0xff213d50);}
                let unlocked=campaign.story_unlocked(index);
                let prefix=if selected {">"} else {" "};
                let label=if unlocked {format!("{prefix} {:02}  {title}",index+1)}
                    else {format!("{prefix} {:02}  [LOCKED]",index+1)};
                let size=if pw<620{13}else{16};
                text(font,fb,&label,x+27,cy,size,if !unlocked{0xff657f90}else if selected{gold}else{teal});
            }
            text(font,fb,"UP/DOWN SELECT    ENTER READ    ESC/B RETURN",x+26,y+ph-38,14,gold);
            return;
        }
        let chapter=campaign.story_chapter;
        let title=story::CHAPTER_TITLES[chapter];
        let heading_size=if pw<730{17}else{21};
        // Truncate the heading if the viewport is very narrow.
        let head_limit=((pw-60)/10).max(12) as usize;
        let shown=if title.chars().count()>head_limit {
            format!("{}...",title.chars().take(head_limit.saturating_sub(3)).collect::<String>())
        } else {title.to_string()};
        text(font,fb,&shown,x+26,y+21,heading_size,gold);
        text(font,fb,"ASTRAEUS // THE ECHO DIRECTIVE - RECONSTRUCTED TRANSCRIPT",x+26,y+49,12,teal);
        let small=ph<375;
        let font_size=if small{13}else{16};
        let line_height=if small{19}else{25};
        let columns=((pw-60)/if small{8}else{10}).clamp(18,108) as usize;
        let per_page=((ph-166)/line_height).max(1) as usize;
        let lines=self.story_cache.entry((chapter,columns)).or_insert_with(||story::wrapped_lines(chapter,columns));
        let count=story::page_count(lines.len(),per_page);
        campaign.story_page_count=count;
        campaign.story_page=campaign.story_page.min(count-1);
        let first=campaign.story_page*per_page;
        for (offset,entry) in lines.iter().skip(first).take(per_page).enumerate(){
            if !entry.is_empty() {
                let head=entry.chars().any(|c|c.is_alphabetic()) &&
                    entry.chars().filter(|c|c.is_alphabetic()).all(|c|c.is_uppercase());
                text(font,fb,entry,x+27,y+84+offset as i32*line_height,font_size,if head {teal} else {body});
            }
        }
        rect(fb,x+18,y+ph-71,pw-36,1,0xff29465b);
        text(font,fb,&format!("CHAPTER {:02} / 09    PAGE {} / {}",chapter+1,campaign.story_page+1,count),x+26,y+ph-61,14,teal);
        let footer=if campaign.story_page+1<count{"ENTER/DOWN NEXT  UP PREV  LEFT/RIGHT CHAPTER"}
            else {"ENTER CONTINUE  UP PREV  LEFT/RIGHT CHAPTER"};
        text(font,fb,footer,x+26,y+ph-39,13,gold);
        text(font,fb,"ESC/B RETURN  |  STORY LIBRARY: CLOSE AND PRESS B",x+26,y+ph-20,12,gray);
    }

}

#[inline]fn tint(rgba:u32,factor:f32)->u32{
    let r=((rgba>>24)&255) as f32;let g=((rgba>>16)&255) as f32;let b=((rgba>>8)&255) as f32;
    0xff000000|((r*factor).min(255.0) as u32)<<16|((g*factor).min(255.0) as u32)<<8|(b*factor).min(255.0) as u32
}
#[inline]fn shade_index(factor:f32)->usize {
    ((factor.clamp(0.0,1.2)*(SHADE_LEVELS-1) as f32/1.2).round() as usize).min(SHADE_LEVELS-1)
}
fn rect(fb:&mut Framebuffer,x:i32,y:i32,width:i32,height:i32,c:u32){
    let w=fb.width as i32;let h=fb.height as i32;
    let x0=x.max(0).min(w);let y0=y.max(0).min(h);let x1=(x+width).max(x0).min(w);let y1=(y+height).max(y0).min(h);
    if x0>=x1||y0>=y1{return}
    for yy in y0..y1{let row=&mut fb.pixels[yy as usize*w as usize+x0 as usize..yy as usize*w as usize+x1 as usize];row.fill(c);}
}
fn blit_scaled(fb:&mut Framebuffer,tex:&Texture,x:i32,y:i32,w:i32,h:i32){
    if w<=0||h<=0{return}
    for yy in y.max(0)..(y+h).min(fb.height as i32){
        let sy=((yy-y) as usize*tex.h/h as usize).min(tex.h-1);
        for xx in x.max(0)..(x+w).min(fb.width as i32){
            let sx=((xx-x) as usize*tex.w/w as usize).min(tex.w-1);
            let rgba=tex.px(sx,sy);
            if rgba&255>=128{fb.pixels[yy as usize*fb.width as usize+xx as usize]=tint(rgba,1.0);}
        }
    }
}
fn text(font:&mut Option<FontRenderer>,fb:&mut Framebuffer,s:&str,x:i32,y:i32,size:u16,color:u32){
    if let Some(f)=font{let _=f.draw_text(fb,s,x,y,size,color,None);}
}
fn draw_minimap(fb:&mut Framebuffer,world:&World){
    let step=8;let ox=16;let oy=42;let rows=world.cells.len() as i32;
    rect(fb,ox-5,oy-5,24*step+10,rows*step+10,0xff06151f);
    for (y,row) in world.cells.iter().enumerate(){for (x,&tile) in row.iter().enumerate(){
        let color=match tile{b'#'=>0xff617b8a,b'B'=>0xffa88a65,b'D'=>0xffb2804e,b'L'=>0xffe4c65c,b'T'=>0xfffd8057,b't'=>0xff42dbaa,b'N'=>0xffc585e8,b'n'=>0xff81b1d5,b'E'=>0xff11dfc9,b'P'=>0xffffb24a,b'U'=>0xff72b7ee,_=>0xff152837};
        rect(fb,ox+x as i32*step,oy+y as i32*step,step-1,step-1,color);
    }}
    for e in &world.enemies {if e.active{let sz=if e.kind==EnemyKind::Archivist{9}else{5};
        rect(fb,ox+(e.pos.x*step as f32) as i32-sz/2,oy+(e.pos.y*step as f32) as i32-sz/2,sz,sz,
            if e.kind==EnemyKind::Archivist{0xffde79fa}else{0xffef7869});}}
    for p in &world.pickups{if p.active{let color=if p.kind==PickupKind::Key{0xff53ddff}else{0xffffdd75};rect(fb,ox+(p.pos.x*step as f32) as i32-2,oy+(p.pos.y*step as f32) as i32-2,4,4,color);}}
    if world.boss_charge>0.0 {
        let bx=ox+(world.boss_target.x*step as f32) as i32;
        let by=oy+(world.boss_target.y*step as f32) as i32;
        rect(fb,bx-5,by-1,11,2,0xffffa754);
        rect(fb,bx-1,by-5,2,11,0xffffa754);
    }
    let px=ox+(world.player.x*step as f32) as i32;let py=oy+(world.player.y*step as f32) as i32;
    rect(fb,px-3,py-3,6,6,0xfff8f4e4);
    for i in 1..10{rect(fb,px+(world.facing.cos()*i as f32) as i32,py+(world.facing.sin()*i as f32) as i32,2,2,0xfff8f4e4);}
}
