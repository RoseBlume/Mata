use crate::{game::{self,Game,Phase,TowerKind,EnemyKind,Vec2,TILE,BOARD_X,BOARD_Y,COLS,ROWS},paint::{Painter,with_alpha}};

const WHITE:u32=0xFFE7F3F6;
const MUTED:u32=0xFF8EA3AF;
const BG:u32=0xFF0B141F;
const PANEL:u32=0xFF142431;
const LINE:u32=0xFF294152;
const MINT:u32=0xFF56D9B0;
const GOLD:u32=0xFFFFC77C;

fn hash(x:i32,y:i32)->u32 {
    let mut v=(x as u32).wrapping_mul(0x9E3779B9)^(y as u32).wrapping_mul(0x85EBCA6B);
    v^=v>>16;v=v.wrapping_mul(0x7feb352d);v^=v>>15;v=v.wrapping_mul(0x846ca68b);v^(v>>16)
}
fn within(x:i32,y:i32,bx:i32,by:i32,w:i32,h:i32)->bool{x>=bx&&y>=by&&x<bx+w&&y<by+h}

pub fn draw(p:&mut Painter<'_>,g:&Game,mouse:Option<(i32,i32)>) {
    background(p,g);
    board(p,g,mouse);
    sidebar(p,g,mouse);
    footer(p,g);
    if matches!(g.phase,Phase::Won|Phase::Lost) {end_overlay(p,g);}
    else if g.paused {paused_overlay(p);}
}
fn background(p:&mut Painter<'_>,g:&Game){
    p.vgradient(0,0,1440,900,BG,0xFF111C29);
    // Sparse constellation-like interface background; deterministic, no assets.
    for i in 0..70 {
        let x=(hash(i,3)%1440) as i32;
        let y=(hash(8,i)%900) as i32;
        let c=with_alpha(0xFF75B9D4,if i%6==0 {43}else{18});
        p.rect(x,y,if i%8==0{2}else{1},1,c);
    }
    p.rect(0,0,1440,109,0xFF101D2A);
    p.rect(0,107,1440,2,0xFF283B4C);
    p.rounded(42,29,48,48,12,0xFF1B524E);
    p.diamond(66,53,16,0xFF56D9B0);
    p.diamond(66,53,9,0xFF193E3D);
    p.circle(66.0,53.0,4.0,WHITE);
    p.label("AEGIS",110,22,30,WHITE);
    p.label("/ GRID DEFENSE",223,36,15,MINT);
    p.label("TACTICAL OPERATIONS  /  SECTOR 07",112,71,11,MUTED);
    stat(p,558,26,126,"WAVE",&format!("{:02} / {:02}",g.wave,game::LAST_WAVE),MINT);
    stat(p,698,26,140,"CREDITS",&format!("{}",g.gold),GOLD);
    stat(p,852,26,124,"INTEGRITY",&format!("{} / 20",g.lives),if g.lives<6 {0xFFF17B86}else{WHITE});
    p.rounded(1008,31,111,51,9,0xFF1C2D3C);
    p.label("[P] PAUSE",1024,46,12,WHITE);
    p.rounded(1130,31,109,51,9,if g.double_speed {0xFF285849}else{0xFF1C2D3C});
    p.label("[TAB] 2X",1140,46,12,if g.double_speed {MINT}else{WHITE});
    p.rounded(1251,31,149,51,9,0xFF1C2D3C);
    p.label(if g.phase==Phase::Running {"SYSTEM ACTIVE"} else {"STANDBY"},1262,46,12,if g.phase==Phase::Running {MINT}else{MUTED});
}
fn stat(p:&mut Painter<'_>,x:i32,y:i32,w:i32,title:&str,value:&str,col:u32){
    p.rounded(x,y,w,57,10,0xFF1A2A37);
    p.label(title,x+13,y+8,9,MUTED);
    p.label(value,x+13,y+27,17,col);
}
fn board(p:&mut Painter<'_>,g:&Game,mouse:Option<(i32,i32)>){
    p.rounded(38,129,962,684,18,0xFF07111D);
    p.rounded(42,134,954,673,16,0xFF1C2B33);
    p.rect(68,151,900,1,0xFF42605C);
    p.label("FIELD / ALPHA",69,137,10,MINT);
    p.label("ROUTE A-07    DEFEND THE CORE",728,137,10,MUTED);
    // Terrain tiles: subtle texture and grid boundaries.
    for row in 0..ROWS {for col in 0..COLS {
        let x=(BOARD_X+col as f32*TILE) as i32;
        let y=(BOARD_Y+row as f32*TILE) as i32;
        let noise=hash(col,row);
        let ground=match noise%5 {0=>0xFF1B3937,1=>0xFF1D3838,2=>0xFF203D3A,3=>0xFF203A38,_=>0xFF1D3A34};
        p.rect(x,y,60,60,0xFF152D31);
        p.rect(x+1,y+1,58,58,ground);
        if !game::on_path(col,row) {
            if noise%6<3 {
                let dx=10+(noise%33) as i32;
                let dy=9+((noise>>8)%36) as i32;
                p.line((x+dx-4) as f32,(y+dy+4) as f32,(x+dx) as f32,(y+dy-3) as f32,1.0,with_alpha(MINT,40));
                p.line((x+dx+3) as f32,(y+dy+4) as f32,(x+dx) as f32,(y+dy-2) as f32,1.0,with_alpha(MINT,33));
            }
            if noise%13==0 {p.diamond(x+43,y+16,4,0xFF45605D);}
            if noise%29==1 {p.rounded(x+15,y+16,24,19,4,0xFF243B40);p.rect(x+18,y+19,19,1,0xFF38555A);}
        }
    }}
    // Connected roadway surface.
    let nodes=&g.path_points;
    for w in nodes.windows(2) {
        p.line(w[0].x,w[0].y+4.0,w[1].x,w[1].y+4.0,55.0,0xFF0E1B23);
        p.line(w[0].x,w[0].y,w[1].x,w[1].y,54.0,0xFF42514F);
        p.line(w[0].x,w[0].y,w[1].x,w[1].y,46.0,0xFF344446);
        p.line(w[0].x,w[0].y,w[1].x,w[1].y,29.0,0xFF3A4D4C);
    }
    for pt in nodes.iter().skip(1).take(nodes.len()-2) {p.circle(pt.x,pt.y,26.0,0xFF344446);p.circle(pt.x,pt.y,18.0,0xFF3A4D4C);}
    // Embedded road guide lamps.
    for row in 0..ROWS {for col in 0..COLS {
        if game::on_path(col,row) {
            let t=game::tile_center(col,row);
            p.rounded(t.x as i32-4,t.y as i32-2,8,4,2,0xFF67817C);
            if col%3==0 {p.rounded(t.x as i32-3,t.y as i32-2,6,3,1,0xFFB3CDAC);}
        }
    }}
    let entry=game::tile_center(0,2);
    p.rounded(68,entry.y as i32-28,6,56,2,MINT);
    p.label("IN",79,entry.y as i32-47,10,MINT);
    let end=game::tile_center(14,8);
    p.rect(961,end.y as i32-31,6,62,0xFFEFB37B);
    p.label("CORE",909,end.y as i32+31,10,GOLD);
    // Hover and selection ranges are below towers and creeps.
    if let Some(idx)=g.selected.and_then(|i|g.towers.get(i)) {
        let pos=idx.pos();p.circle(pos.x,pos.y,idx.range(),with_alpha(idx.kind.color(),17));
        p.ring(pos.x,pos.y,idx.range(),2.0,with_alpha(idx.kind.color(),115));
    }
    if let Some((mx,my))=mouse {
        if let Some((col,row))=game::tile_at(mx as f32,my as f32) {
            let pos=game::tile_center(col,row);
            if let Some(kind)=g.build {
                let valid=g.can_place(col,row)&&g.gold>=kind.price();
                let accent=if valid {MINT}else{0xFFEC7286};
                p.rounded((BOARD_X+col as f32*TILE+3.0) as i32,(BOARD_Y+row as f32*TILE+3.0) as i32,54,54,10,with_alpha(accent,72));
                p.circle(pos.x,pos.y,kind.base_range(),with_alpha(accent,13));
                p.ring(pos.x,pos.y,kind.base_range(),2.0,with_alpha(accent,90));
                tower_sprite(p,pos,kind,-1.5708,1,true);
            } else if let Some(idx)=g.tower_at(col,row) {
                let t=&g.towers[idx];p.ring(pos.x,pos.y,t.range(),1.0,with_alpha(t.kind.color(),65));
            }
        }
    }
    for (i,t) in g.towers.iter().enumerate() {
        let pos=t.pos();tower_sprite(p,pos,t.kind,t.facing,t.level,false);
        if g.selected==Some(i) {p.ring(pos.x,pos.y,31.0,2.0,MINT);}
        if t.firing>0.0 {p.glow(pos.x,pos.y,30.0,t.kind.color());}
    }
    for e in &g.enemies {enemy_sprite(p,e);}
    for bullet in &g.projectiles {
        let c=bullet.kind.color();p.glow(bullet.pos.x,bullet.pos.y,12.0,c);
        p.circle(bullet.pos.x,bullet.pos.y,if bullet.kind==TowerKind::Cannon {8.0}else{5.0},c);
        p.circle(bullet.pos.x-1.0,bullet.pos.y-1.0,2.0,WHITE);
    }
    for effect in &g.effects {
        let a=(effect.time/effect.max_time).clamp(0.0,1.0);
        if effect.radius<=0.0 {
            p.line(effect.from.x,effect.from.y,effect.at.x,effect.at.y,10.0,with_alpha(effect.color,(a*90.0) as u8));
            p.line(effect.from.x,effect.from.y,effect.at.x,effect.at.y,3.0,with_alpha(WHITE,(a*200.0) as u8));
            p.circle(effect.at.x,effect.at.y,13.0,with_alpha(effect.color,(a*160.0) as u8));
        } else {
            let r=effect.radius*(1.1-0.3*a);
            p.ring(effect.at.x,effect.at.y,r,2.0,with_alpha(effect.color,(a*170.0) as u8));
            p.circle(effect.at.x,effect.at.y,r*0.56,with_alpha(effect.color,(a*42.0) as u8));
        }
    }
    // Subtle scan border and board footer.
    p.rect(68,766,900,1,0xFF395454);
    p.label("SOUTHERN APPROACH   //   LIVE TERRAIN SCAN",69,777,10,MUTED);
    let status=if g.status_timer>0.0 {&g.status} else if g.phase==Phase::Ready {"Grid operational. Deploy towers, then launch the next wave."} else {"Hostile movement detected. Reinforce exposed bends."};
    p.label(status,69,794,13,if g.status_timer>0.0 {MINT}else{WHITE});
}
fn tower_sprite(p:&mut Painter<'_>,pos:Vec2,kind:TowerKind,facing:f32,level:u8,ghost:bool){
    let c=kind.color();let x=pos.x;let y=pos.y;
    let alpha=if ghost {150}else{255};
    p.circle(x,y+5.0,25.0,with_alpha(0xFF06141C,130));
    p.circle(x,y,24.0,with_alpha(0xFF081B26,alpha));
    p.ring(x,y,24.0,3.0,with_alpha(0xFF78919B,alpha));
    p.ring(x,y,19.0,3.0,with_alpha(c,(alpha as f32*0.66) as u8));
    let dx=facing.cos();let dy=facing.sin();
    match kind {
        TowerKind::Bolt=>{
            p.line(x,y,x+dx*28.0,y+dy*28.0,11.0,0xFF1C3C3E);
            p.line(x,y,x+dx*31.0,y+dy*31.0,6.0,c);
            p.diamond(x as i32,y as i32,13,0xFF244B4A);
            p.circle(x,y,7.0,c);p.circle(x,y,3.0,WHITE);
        }
        TowerKind::Cannon=>{
            p.line(x,y,x+dx*28.0,y+dy*28.0,17.0,0xFF192631);
            p.line(x+dx*10.0,y+dy*10.0,x+dx*32.0,y+dy*32.0,11.0,c);
            p.circle(x,y,14.0,0xFF3A3737);p.ring(x,y,12.0,3.0,c);
            p.circle(x,y,5.0,0xFFFFDAAE);
        }
        TowerKind::Frost=>{
            for k in 0..4 {let a=std::f32::consts::FRAC_PI_2*k as f32;
                p.diamond((x+a.cos()*18.0) as i32,(y+a.sin()*18.0) as i32,9,c);
            }
            p.circle(x,y,13.0,0xFF245478);p.diamond(x as i32,y as i32,10,0xFFB7EDFF);
            p.circle(x,y,4.0,WHITE);
        }
        TowerKind::Prism=>{
            p.ring(x,y,16.0,3.0,c);p.diamond(x as i32,y as i32,17,0xFF40345B);
            p.diamond(x as i32,y as i32,11,c);p.diamond(x as i32,y as i32,5,WHITE);
        }
    }
    if level>1 && !ghost {
        for i in 0..(level-1) {p.circle(x-17.0+i as f32*11.0,y+29.0,3.0,GOLD);}
    }
}
fn enemy_sprite(p:&mut Painter<'_>,e:&game::Enemy){
    let x=e.pos.x;let y=e.pos.y;let c=e.kind.color();
    let radius=if e.kind==EnemyKind::Bulwark {19.0}else if e.kind==EnemyKind::Phantom {13.0}else{15.0};
    p.circle(x+2.0,y+5.0,radius+2.0,with_alpha(0xFF00121D,160));
    p.circle(x,y,radius,0xFF17242D);
    p.ring(x,y,radius,4.0,c);
    if e.kind==EnemyKind::Bulwark {p.diamond(x as i32,y as i32,12,0xFF83679B);p.circle(x,y,6.0,WHITE);}
    else if e.kind==EnemyKind::Phantom {p.diamond(x as i32,y as i32,10,c);p.circle(x,y,4.0,WHITE);}
    else {p.line(x-7.0,y,x+7.0,y,4.0,c);p.circle(x,y,5.0,c);p.circle(x,y,2.0,WHITE);}
    if e.slow_time>0.0 {p.ring(x,y,radius+5.0,2.0,0xFF91E4FF);}
    let bar_width=38;
    let left=x as i32-bar_width/2;
    let top=y as i32-radius as i32-11;
    p.rounded(left-1,top-1,bar_width+2,6,3,0xFF10202A);
    p.rect(left,top,(bar_width as f32*(e.hp/e.max_hp).clamp(0.0,1.0)) as i32,4,if e.hp<e.max_hp*0.33 {0xFFF07C78}else{c});
}
fn sidebar(p:&mut Painter<'_>,g:&Game,mouse:Option<(i32,i32)>){
    let mouse=mouse.unwrap_or((-999,-999));
    p.rounded(1032,129,374,730,18,0xFF0A131E);
    p.rounded(1038,135,362,718,15,PANEL);
    p.label("DEFENSE SYSTEMS",1056,151,17,WHITE);
    p.label("SELECT A UNIT TO DEPLOY",1056,179,10,MUTED);
    for (i,k) in TowerKind::ALL.iter().enumerate() {
        let y=208+i as i32*97;
        let selected=g.build==Some(*k);
        let hover=within(mouse.0,mouse.1,1050,y,340,87);
        let backing=if selected {0xFF25453F}else if hover {0xFF253A46}else{0xFF1B303D};
        p.rounded(1050,y,340,87,11,backing);
        if selected {p.rect(1050,y+12,3,63,MINT);}
        p.rounded(1061,y+13,63,61,9,0xFF12232E);
        tower_sprite(p,Vec2::new(1092.0,y as f32+43.0),*k,-1.2,1,false);
        p.label(&format!("0{}",i+1),1129,y+14,9,if selected{MINT}else{MUTED});
        p.label(k.name(),1148,y+12,14,WHITE);
        p.label(k.role(),1148,y+39,10,MUTED);
        let cost_col=if g.gold>=k.price(){GOLD}else{0xFFE98590};
        p.label(&format!("{} CR",k.price()),1148,y+60,11,cost_col);
        p.label("[",1352,y+12,10,MUTED);p.label("]",1371,y+12,10,MUTED);
        p.label(&(i+1).to_string(),1360,y+12,10,MINT);
    }
    // Information and controls panel.
    p.rect(1051,605,338,1,LINE);
    if let Some(t)=g.selected.and_then(|i|g.towers.get(i)) {
        p.label("SELECTED / UNIT DATA",1055,613,10,MINT);
        p.label(t.kind.name(),1055,633,17,WHITE);
        p.label(&format!("LVL {}   DMG {:.0}   RNG {:.0}",t.level,t.damage(),t.range()),1055,656,11,MUTED);
        let max=t.level>=4;
        let cost=t.upgrade_cost();
        let can_up=!max&&g.gold>=cost;
        let hover_upgrade=within(mouse.0,mouse.1,1053,686,193,66);
        p.rounded(1053,686,193,66,10,if can_up {if hover_upgrade{0xFF2F7C69}else{0xFF246451}}else{0xFF263B43});
        p.label(if max {"MAX LEVEL"}else{"UPGRADE [U]"},1067,695,14,if can_up{WHITE}else{MUTED});
        p.label(if max {"All systems online"}else{"Boost damage & range"},1067,721,10,if can_up{0xFFC3F8E8}else{MUTED});
        if !max {p.label(&format!("{}",cost),1198,697,12,GOLD);}
        let hover_sell=within(mouse.0,mouse.1,1252,686,134,66);
        p.rounded(1252,686,134,66,10,if hover_sell{0xFF56404A}else{0xFF392E3A});
        p.label("SELL [X]",1265,695,13,0xFFFFB4B0);
        p.label(&format!("+{} credits",t.sell_value()),1265,721,10,MUTED);
    }else{
        p.label("FIELD INTEL",1055,617,11,MINT);
        p.label("Route bends create kill zones.",1055,643,13,WHITE);
        p.label("Build beside the road. Upgrade towers",1055,671,11,MUTED);
        p.label("to handle armored and faster threats.",1055,693,11,MUTED);
        p.label("Right-click to cancel placement.",1055,723,10,0xFF91ACBB);
    }
    let ready=g.phase==Phase::Ready;
    let btn=if ready {if within(mouse.0,mouse.1,1052,773,339,65){0xFF3FBE92}else{0xFF2C9C79}}else{0xFF273B47};
    p.rounded(1052,773,340,65,11,btn);
    let main=if g.phase==Phase::Ready {if g.wave==0 {"LAUNCH WAVE 01"}else{"LAUNCH NEXT WAVE"}}
             else if g.phase==Phase::Running {"DEFENSES ENGAGED"}
             else {"MISSION ENDED"};
    p.centered(main,1223,784,17,if ready{0xFFECFFF4}else{MUTED});
    p.centered(if ready {"[SPACE]  DEPLOY INCOMING HOSTILES"}else if g.phase==Phase::Running {"Wave in progress"}else{"[R] RESTART MISSION"},1223,810,10,if ready {0xFFD3F4E4}else{MUTED});
}
fn footer(p:&mut Painter<'_>,g:&Game){
    p.rect(41,856,955,1,LINE);
    p.label("LEFT CLICK",46,870,10,MINT);
    p.label("to build / inspect",136,870,10,MUTED);
    p.label("1 - 4",335,870,10,MINT);
    p.label("select tower",388,870,10,MUTED);
    p.label("P",550,870,10,MINT);
    p.label("pause",571,870,10,MUTED);
    p.label("TAB",674,870,10,MINT);
    p.label("speed",713,870,10,MUTED);
    p.label("ESC",798,870,10,MINT);
    p.label("cancel",845,870,10,MUTED);
    if g.phase==Phase::Running {
        let pending=g.spawn_left as usize+g.enemies.len();
        p.label(&format!("{} threats",pending),907,870,10,GOLD);
    }
}
fn end_overlay(p:&mut Painter<'_>,g:&Game){
    p.rect(0,108,1440,792,with_alpha(0xFF030A13,205));
    p.rounded(393,272,654,332,23,0xFF182B36);
    p.rounded(400,279,640,318,19,0xFF0F1D2A);
    let won=g.phase==Phase::Won;
    let accent=if won {MINT}else{0xFFF08A86};
    p.rect(442,314,557,3,accent);
    p.centered(if won {"GRID SECURED"}else{"CORE BREACHED"},719,343,32,accent);
    p.centered(if won {"All 20 hostile waves neutralized."}else{"The defense perimeter has fallen."},720,409,17,WHITE);
    p.centered(&format!("WAVES {:02} / 20       ELIMINATIONS {}       CREDITS {}",g.wave,g.kills,g.gold),720,461,13,MUTED);
    p.rounded(557,518,326,59,10,if won {0xFF226B58}else{0xFF734C51});
    p.centered("RESTART MISSION  [R]",720,529,17,WHITE);
}
fn paused_overlay(p:&mut Painter<'_>){
    p.rect(38,129,962,684,with_alpha(0xFF081421,165));
    p.rounded(324,372,388,147,18,0xFF1C3440);
    p.centered("SIMULATION PAUSED",518,399,23,WHITE);
    p.centered("Press P to resume",518,451,13,MINT);
}
