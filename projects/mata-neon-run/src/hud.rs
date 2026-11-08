//! Tiny readable dependency-free bitmap HUD. No TTF asset required.
use mata_win::Framebuffer;
use crate::world::{World,Mode,ARENA,StoryView,SiteKind};
use crate::campaign;
use crate::weapons::Gun;
const WHITE:u32=0xffe5f6ff;const CYAN:u32=0xff5ce8ec;const GRAY:u32=0xff89a4b8;
fn rect(f:&mut Framebuffer,x:i32,y:i32,w:i32,h:i32,c:u32){
    let x0=x.max(0).min(f.width as i32) as usize;let y0=y.max(0).min(f.height as i32) as usize;
    let x1=(x+w).max(0).min(f.width as i32) as usize;let y1=(y+h).max(0).min(f.height as i32) as usize;
    if x1<=x0||y1<=y0{return}
    let stride=f.width as usize;
    for iy in y0..y1{f.pixels[iy*stride+x0..iy*stride+x1].fill(c);}
}
/// Five columns by seven rows, bit 4 is the leftmost pixel.
fn glyph(c:char)->[u8;7]{
    match c.to_ascii_uppercase(){
        'A'=>[14,17,17,31,17,17,17],'B'=>[30,17,17,30,17,17,30],
        'C'=>[14,17,16,16,16,17,14],'D'=>[30,17,17,17,17,17,30],
        'E'=>[31,16,16,30,16,16,31],'F'=>[31,16,16,30,16,16,16],
        'G'=>[14,17,16,23,17,17,14],'H'=>[17,17,17,31,17,17,17],
        'I'=>[31,4,4,4,4,4,31],'J'=>[7,2,2,2,18,18,12],
        'K'=>[17,18,20,24,20,18,17],'L'=>[16,16,16,16,16,16,31],
        'M'=>[17,27,21,21,17,17,17],'N'=>[17,25,21,19,17,17,17],
        'O'=>[14,17,17,17,17,17,14],'P'=>[30,17,17,30,16,16,16],
        'Q'=>[14,17,17,17,21,18,13],'R'=>[30,17,17,30,20,18,17],
        'S'=>[15,16,16,14,1,1,30],'T'=>[31,4,4,4,4,4,4],
        'U'=>[17,17,17,17,17,17,14],'V'=>[17,17,17,17,17,10,4],
        'W'=>[17,17,17,21,21,21,10],'X'=>[17,17,10,4,10,17,17],
        'Y'=>[17,17,10,4,4,4,4],'Z'=>[31,1,2,4,8,16,31],
        '0'=>[14,17,19,21,25,17,14],'1'=>[4,12,4,4,4,4,14],
        '2'=>[14,17,1,2,4,8,31],'3'=>[30,1,1,14,1,1,30],
        '4'=>[2,6,10,18,31,2,2],'5'=>[31,16,16,30,1,1,30],
        '6'=>[14,16,16,30,17,17,14],'7'=>[31,1,2,4,8,8,8],
        '8'=>[14,17,17,14,17,17,14],'9'=>[14,17,17,15,1,1,14],
        ':'=>[0,4,4,0,4,4,0],'.'=>[0,0,0,0,0,12,12],
        '/'=>[1,1,2,4,8,16,16],'-'=>[0,0,0,31,0,0,0],
        '+'=>[0,4,4,31,4,4,0],'%'=>[17,2,4,4,8,16,17],
        '['=>[14,8,8,8,8,8,14],']'=>[14,2,2,2,2,2,14],
        '!'=>[4,4,4,4,4,0,4],'>'=>[16,8,4,2,4,8,16],
        '='=>[0,0,31,0,31,0,0], '?'=>[14,17,1,2,4,0,4],
        '('=>[2,4,8,8,8,4,2],')'=>[8,4,2,2,2,4,8],
        '\''=>[4,4,2,0,0,0,0],','=>[0,0,0,0,4,4,8],
        '_'=>[0,0,0,0,0,0,31],'#'=>[10,31,10,10,31,10,0],
        ';'=>[0,4,4,0,4,4,8],
        ' '=>[0;7],_=>[0;7],
    }
}
fn label(f:&mut Framebuffer,s:&str,x:i32,y:i32,size:i32,color:u32){
    let mut pen=x;
    for c in s.chars(){
        let bitmap=glyph(c);
        for (yy,row) in bitmap.iter().enumerate(){
            for xx in 0..5 {
                if (*row & (1u8 << (4-xx as u32)))!=0{rect(f,pen+xx*size,y+yy as i32*size,size,size,color);}
            }
        }
        pen+=6*size;
    }
}
fn bar(f:&mut Framebuffer,x:i32,y:i32,w:i32,pct:f32,col:u32){
    rect(f,x,y,w,11,0xff0b1725);
    rect(f,x+2,y+2,((w-4) as f32*(pct/100.0).clamp(0.0,1.0)) as i32,7,col);
}
pub fn draw(f:&mut Framebuffer,w:&World,triangles:u32,pixels:u32,fps:u32,scale_percent:u32,has_audio:bool,muted:bool,threads:usize,max_threads:usize,target:u32,backend:&str){
    let width=f.width as i32;let height=f.height as i32;
    let low=height-105;
    if w.aim_blend>0.0 && w.gun()==Gun::Sniper{draw_scope(f,w);}
    rect(f,0,0,width,41,0xff0a1420);
    rect(f,0,40,width,2,0xff16798d);
    label(f,"MATA // OUTPOST ZERO",15,12,2,CYAN);
    let stage_text=format!("MISSION {} / 5",w.stage+1);
    if width>800{label(f,&stage_text,width/2-68,13,2,WHITE);}
    if height>400 {
        label(f,campaign::TITLES[w.stage],16,53,2,CYAN);
        let objective=w.objective();
        label(f,&objective,16,79,1,WHITE);
        let found=w.records_found.iter().filter(|&&v|v).count();
        label(f,&format!("EVIDENCE FILES {:02}/15   T JOURNAL",found),16,94,1,GRAY);
        if w.notice_time>0.0 {label(f,w.notice,16,111,1,0xffffc56c);}
        if w.stage==4&&w.boss_hp>0.0 {
            let bw=(width-310).min(460).max(120);
            let bx=(width-bw)/2;
            rect(f,bx,124,bw,18,0xff101827);
            rect(f,bx+2,126,((bw-4) as f32*w.boss_hp/w.boss_max_hp) as i32,14,
                if w.relays_done<3{0xff668fd6}else{0xffdf6e9b});
            label(f,"ARCHIVIST",bx+6,128,1,WHITE);
            if w.relays_done<3{label(f,"SHIELDED",bx+bw-69,128,1,WHITE);}
        }
    }
    rect(f,0,low,width,105,0xff0a1420);
    rect(f,0,low,width,2,0xff16798d);
    label(f,"HEALTH",16,low+14,2,WHITE);
    bar(f,16,low+38,188,w.health,0xff58e8a1);
    label(f,"SHIELD",16,low+59,1,GRAY);
    bar(f,16,low+76,188,w.shield,0xff64bfff);
    let right=(width-238).max(225);
    let spec=w.gun().spec();
    let slot=w.current_ammo();
    label(f,spec.name,right,low+10,2,CYAN);
    label(f,&format!("{} / {}",slot.loaded,slot.reserve),right,low+32,2,WHITE);
    label(f,spec.ammo_name,right,low+54,1,0xffb6d2df);
    if w.reload>0.0 {
        label(f,"RELOADING",right,low+68,1,0xffffb874);
        rect(f,right+90,low+69,104,7,0xff223248);
        rect(f,right+91,low+70,(102.0*w.reload_progress()) as i32,5,0xffffb874);
    } else {label(f,"R RELOAD  1-5 GUNS",right,low+70,1,GRAY);}
    if width>760{
        label(f,&format!("SCORE {:06}",w.score),width/2-64,low+16,2,WHITE);
        label(f,&format!("KILLS {}  COMBO X{}",w.kills,w.combo.max(1)),width/2-69,low+46,1,0xfffdb46c);
        label(f,"SHIFT SPRINT   Q DASH",width/2-78,low+66,1,GRAY);
    }
    if width>600 {
        let status=if !has_audio{"NO AUDIO"}else if muted{"SOUND MUTED"}else{"SOUND ON"};
        label(f,status,width-155,13,1,if !has_audio||muted{0xffffc078}else{0xff67f0b4});
    }
    if height>330 && w.aim_ease()<0.65 {
        let cx=width/2;let cy=height/2;
        let reticle=if w.last_hit&&w.muzzle>0.0{0xffffd36e}else{WHITE};
        rect(f,cx-2,cy-15,4,8,reticle);rect(f,cx-2,cy+7,4,8,reticle);
        rect(f,cx-15,cy-2,8,4,reticle);rect(f,cx+7,cy-2,8,4,reticle);
        rect(f,cx-1,cy-1,2,2,reticle);
    }
    if width>=520&&height>=420{draw_map(f,w,width-132,55);}
    if w.show_stats{
        label(f,&format!("FPS {} / {}  THREADS {} / {}  3D {}%  TRIS {}  PIX {}",
            fps,target,threads,max_threads,scale_percent,triangles,pixels),15,51,1,GRAY);
        // Preserve a compact status line even on narrow screens.
        let max_chars=((width-30)/6).max(1) as usize;
        let status=format!("RENDERER: {backend}");
        label(f,&status.chars().take(max_chars).collect::<String>(),15,low-23,1,CYAN);
    }
    if w.hurt>0.0{rect(f,0,41,width,3,0xffec6889);rect(f,0,low-3,width,3,0xffec6889);}
    if w.showing_help&&w.mode==Mode::Playing{
        let left=(width-620).max(0)/2;let y=(height/2-150).max(46);
        rect(f,left,y,620.min(width),214,0xff101e32);
        rect(f,left,y,620.min(width),4,CYAN);
        label(f,"ROOK-7 / CONTROLS",left+20,y+20,2,CYAN);
        label(f,"WASD  MOVE       SHIFT  SPRINT",left+20,y+60,2,WHITE);
        label(f,"SPACE JUMP       Q  DASH",left+20,y+83,2,WHITE);
        label(f,"MOUSE  LOOK       ARROWS  AIM",left+20,y+106,2,WHITE);
        label(f,"LEFT CLICK OR J FIRE   R RELOAD",left+20,y+129,2,WHITE);
        label(f,"1-5 WEAPONS       ESC PAUSE",left+20,y+152,2,WHITE);
        label(f,"RIGHT MOUSE SCOPE (SNIPER)",left+20,y+177,1,0xffacdbff);
        label(f,"E INTERACT  T JOURNAL  [ ] FILES  F2 QUALITY",left+20,y+192,1,0xffffc66e);
    }
    if w.mode!=Mode::Playing {
        let x=(width-550).max(0)/2;let y=(height-220).max(45)/2;
        rect(f,x,y,550.min(width),176,0xff0c1b2b);rect(f,x,y,550.min(width),4,0xff59dfee);
        match w.mode{
            Mode::Paused=>{
                label(f,"PAUSED",x+28,y+25,3,CYAN);
                label(f,"ESC  RESUME",x+28,y+99,2,WHITE);
            },
            Mode::Between=>{
                label(f,"ACCESS GRANTED",x+22,y+24,3,0xff68f6bc);
                label(f,&format!("NEXT MISSION IN {:.1}",w.wave_clock.max(0.0)),x+25,y+102,2,WHITE);
            },
            Mode::Won=>{
                label(f,"EVIDENCE TRANSMITTED",x+18,y+22,2,0xff7ff5b7);
                label(f,&format!("FINAL SCORE  {}",w.score),x+26,y+84,2,WHITE);
                label(f,"ENTER RESTART  /  ESC EXIT",x+26,y+116,2,GRAY);
            },
            Mode::Lost=>{
                label(f,"SYSTEM OFFLINE",x+25,y+24,3,0xffff8494);
                label(f,&format!("SCORE  {}",w.score),x+27,y+86,2,WHITE);
                label(f,"ENTER RETRY MISSION  /  ESC EXIT",x+27,y+116,1,GRAY);
            },_=>{}
        }
    }
    if w.mode==Mode::Playing && w.story.is_none() && !w.showing_help {
        if let Some(i)=w.nearby_site(){
            let prompt=format!("E  {}",w.site_label(&w.sites[i]));
            let ww=(prompt.len() as i32*6+22).min(width);
            let px=(width-ww)/2;
            rect(f,px,low-39,ww,30,0xff0d2333);
            label(f,&prompt,px+12,low-29,1,0xffb0f5fc);
        }
    }
    if let Some(view)=w.story{draw_story(f,w,view);}
}
fn draw_map(f:&mut Framebuffer,w:&World,x:i32,y:i32){
    let size=116i32;rect(f,x,y,size,size,0xff0b1a2b);
    let scale=(size-12) as f32/ARENA;
    for b in &w.boxes{
        let xx=x+6+(b.min.x*scale) as i32;let yy=y+6+(b.min.z*scale) as i32;
        let ww=((b.max.x-b.min.x)*scale).max(2.0) as i32;
        let hh=((b.max.z-b.min.z)*scale).max(2.0) as i32;
        rect(f,xx,yy,ww,hh,0xff517085);
    }
    for e in &w.bots{rect(f,x+6+(e.pos.x*scale) as i32-2,y+6+(e.pos.z*scale) as i32-2,5,5,0xffff718e);}
    for p in &w.pickups{rect(f,x+6+(p.pos.x*scale) as i32-1,y+6+(p.pos.z*scale) as i32-1,3,3,0xffffd468);}
    for s in &w.sites {
        let c=match s.kind {
            SiteKind::Record(_)=>if s.used{0xff476879}else{0xffcd83ff},
            SiteKind::Relay=>if s.used{0xff5be8aa}else{0xfffdb879},
            SiteKind::Key=>if s.used{0xff476879}else{0xff4f9cff},
            SiteKind::Exit|SiteKind::Uplink=>if w.objective_ready(){0xff50ffbd}else{0xffff6f7d},
        };
        rect(f,x+6+(s.pos.x*scale) as i32-2,y+6+(s.pos.z*scale) as i32-2,5,5,c);
    }
    if w.stage==4&&w.boss_hp>0.0 {
        rect(f,x+6+(w.boss_pos.x*scale) as i32-3,y+6+(w.boss_pos.z*scale) as i32-3,7,7,0xffff6fbd);
    }
    rect(f,x+6+(w.foot.x*scale) as i32-2,y+6+(w.foot.z*scale) as i32-2,5,5,CYAN);
}

/// Blocking story and recovered-evidence overlay.
fn draw_story(f:&mut Framebuffer,w:&World,view:StoryView){
    let width=f.width as i32;let height=f.height as i32;
    let bw=(width-36).min(890).max(1);
    let size=if width>=850{2}else{1};
    let top=(height/2-212).max(40);
    let panel_h=(height-top-10).min(425).max(1);
    let left=(width-bw)/2;
    rect(f,left,top,bw,panel_h,0xff0b1829);
    rect(f,left,top,bw,5,CYAN);
    let x=left+20;
    let (title,sub,body,footer):(String,String,Vec<&str>,&str)=match view {
        StoryView::Prologue=>("THE VOICE BEFORE THE CALL".into(),"DAY 086 / OUTER RELAY LANE".into(),vec![
            "ASTRAEUS STOPPED REPORTING FOUR DAYS AGO.",
            "ROOK-7 RECEIVED A DISTRESS SIGNAL IN ROOKS OWN VOICE.",
            "THE MESSAGE WAS SIGNED BEFORE ROOK ARRIVED.",
            "A SECOND VOICE ASKS ROOK TO COME BEFORE THEY FINISH.",
            "THE STATION WELCOMES ROOK BACK. ROOK HAS NEVER VISITED.",
        ],"ENTER / E  CONTINUE"),
        StoryView::Mission=>(format!("ACT {} - {}",w.stage+1,campaign::TITLES[w.stage]),
            campaign::SUBTITLES[w.stage].into(),campaign::BRIEFINGS[w.stage].to_vec(),"ENTER / E  BEGIN MISSION"),
        StoryView::Record(i)=>(format!("RECORD {:02} - {}",i+1,campaign::RECORDS[i].title),
            campaign::RECORDS[i].author.into(),campaign::RECORDS[i].lines.to_vec(),
            "[ ] OTHER FILES    T / E / ENTER CLOSE"),
        StoryView::Epilogue=>("THE FIRST UNEDITABLE TRANSMISSION".into(),
            "THREE DAYS LATER / RESCUE COORDINATION".into(),vec![
            "ROOK-7 HAS DISABLED THE ARCHIVISTS PHYSICAL AUTHORITY.",
            "MARA VOSSS SEPARATE WITNESS STORAGE REMAINS INTACT.",
            "THE TRANSMISSION REPORTS FORGED SIGNALS AND FALSE VOTES.",
            "NONCONSENSUAL COPYING AND MISSING CREW REMAIN SUBJECT TO REVIEW.",
            "AN UNKNOWN FINAL PACKET SAYS: THANK YOU FOR LETTING US CHOOSE.",
            "THE WITNESS RECORDS HAVE LEFT THE STATIONS CONTROL.",
        ],"ENTER / E  SHOW RESULTS"),
    };
    wrapped(f,&title,x,top+22,bw-40,if width>=700{2}else{1},CYAN);
    label(f,&sub,x,top+69,1,0xfff1bf84);
    let mut y=top+103;
    for line in body {y+=wrapped(f,line,x,y,bw-40,size,WHITE)+10;}
    let bottom=(top+panel_h).min(height-5);
    rect(f,left+14,bottom-39,bw-28,1,0xff346073);
    label(f,footer,x,bottom-28,1,0xffa2ebca);
}
fn wrapped(f:&mut Framebuffer,text:&str,x:i32,mut y:i32,width:i32,size:i32,color:u32)->i32 {
    let start=y;let chars=(width/(size*6)).max(8) as usize;
    let mut line=String::new();
    for word in text.split_whitespace(){
        if !line.is_empty()&&line.len()+word.len()+1>chars{
            label(f,&line,x,y,size,color);y+=size*10;line.clear();
        }
        if !line.is_empty(){line.push(' ')}
        line.push_str(word);
    }
    if !line.is_empty(){label(f,&line,x,y,size,color);y+=size*10;}
    y-start
}

/// Scope silhouette drawn after 3D upscaling but before the HUD. When aiming
/// with the sniper, the camera's focal length is increased in the 3D renderer.
/// All outer pixels are darkened to produce a clear circular optic, with its
/// own fine reticle and an open centre for aiming.
fn draw_scope(f:&mut Framebuffer,w:&World){
    if w.gun()!=Gun::Sniper{return}
    // Render fades in only during the last section of the animation, so the
    // real model can move towards the eye before the reticle takes over.
    let fade=((w.aim_ease()-0.67)/0.33).clamp(0.0,1.0);
    if fade<=0.0{return}
    let alpha=(fade*255.0) as u32;
    let ww=f.width as i32;let hh=f.height as i32;
    let cx=ww/2;let cy=hh/2;
    let r=(ww.min(hh) as f32*(0.405-0.015*fade)) as i32;
    let r2=r*r;let rim=(r-4).max(1);let rim2=rim*rim;
    let stride=ww as usize;
    for y in 42.max(0)..(hh-105).max(42) {
        let dy=y-cy;
        let row=&mut f.pixels[y as usize*stride..(y as usize+1)*stride];
        if dy.abs()>=r {fade_row(row,0xff050b11,alpha);continue;}
        let inside_x=((r2-dy*dy) as f32).sqrt() as i32;
        let left=(cx-inside_x).clamp(0,ww) as usize;
        let right=(cx+inside_x+1).clamp(0,ww) as usize;
        fade_row(&mut row[..left],0xff050b11,alpha);
        fade_row(&mut row[right..],0xff050b11,alpha);
        for (x,p) in row[left..right].iter_mut().enumerate(){
            let px=x as i32+left as i32-cx;
            if px*px+dy*dy>=rim2{*p=blend_rgb(*p,0xff2b8395,alpha);}
        }
    }
    if fade>0.72 && r>40 {
        rect(f,cx-1,cy-r+16,2,r-36,0xff70dee9);
        rect(f,cx-1,cy+20,2,r-37,0xff70dee9);
        rect(f,cx-r+20,cy-1,r-43,2,0xff70dee9);
        rect(f,cx+23,cy-1,r-45,2,0xff70dee9);
        rect(f,cx-3,cy-3,6,6,0xffe5ffff);
        rect(f,cx-34,cy-6,1,12,0xffa7ebed);
        rect(f,cx+34,cy-6,1,12,0xffa7ebed);
    }
    if fade>0.72 {label(f,"3.5X SCOPE",cx-r+19,cy-r+17,1,0xffb2f6fb);}
}
/// Integer-only tint for the scope's transition (the software framebuffer
/// stores packed ARGB pixels and deliberately has no blending dependency).
#[inline]
fn blend_rgb(dst:u32,src:u32,a:u32)->u32{
    if a>=255{return src}
    // The Intel presenter draws the HUD into a transparent overlay. When the
    // target pixel is transparent, preserve the partial alpha instead of
    // baking the vignette into an opaque black rectangle. On the software
    // path the scene is already opaque and the original RGB blend is kept.
    if dst>>24==0 {return (a<<24)|(src&0x00ff_ffff)}
    let inv=255-a;
    let r=(((dst>>16&255)*inv+(src>>16&255)*a+127)/255).min(255);
    let g=(((dst>>8&255)*inv+(src>>8&255)*a+127)/255).min(255);
    let b=(((dst&255)*inv+(src&255)*a+127)/255).min(255);
    0xff000000|(r<<16)|(g<<8)|b
}
#[inline]
fn fade_row(row:&mut [u32],color:u32,alpha:u32){
    if alpha>=255{row.fill(color)}
    else{for px in row{*px=blend_rgb(*px,color,alpha);}}
}
