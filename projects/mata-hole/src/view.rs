//! Pixel-perfect ARGB software rendering. Drawing happens at the current
//! framebuffer dimensions, so resizing does not upsample fonts or sprites.
use crate::game::{Game,Kind,Phase,Prop,V,WORLD_W,WORLD_H};
use crate::paint::{Painter,with_alpha,mix};
const INK:u32=0xFF07131C;
const PANEL:u32=0xFF11222D;
const WHITE:u32=0xFFF2F8FF;
const DIM:u32=0xFF8FA6B9;
const TEAL:u32=0xFF65EDD5;
const PINK:u32=0xFFFF94BB;
const GOLD:u32=0xFFFFD07C;

pub struct View {pub camera:V,pub zoom:f32,pub width:i32,pub height:i32}
impl View {
    pub fn new(at:V)->Self{Self{camera:at,zoom:1.0,width:1280,height:800}}
    pub fn update(&mut self,game:&Game,dt:f32,width:i32,height:i32){
        self.width=width.max(1);self.height=height.max(1);
        let goal=game.holes[0].pos;
        let ease=1.0-(-dt*8.0).exp();
        self.camera=self.camera+(goal-self.camera)*ease;
        let target_zoom=(self.height as f32/800.0).clamp(0.55,1.70)*
                        (1.0/(1.0+(game.holes[0].radius-23.5).max(0.0)/260.0));
        self.zoom+=(target_zoom-self.zoom)*(1.0-(-dt*3.0).exp());
    }
    pub fn to_screen(&self,p:V)->(f32,f32){
        ((p.x-self.camera.x)*self.zoom+self.width as f32*0.5,
         (p.y-self.camera.y)*self.zoom+self.height as f32*0.5)
    }
    fn visible(&self,p:V,r:f32)->bool {
        let(x,y)=self.to_screen(p);let r=r*self.zoom;
        x>=-r-40.0&&x<=self.width as f32+r+40.0&&y>=-r-40.0&&y<=self.height as f32+r+40.0
    }
    pub fn draw(&self,p:&mut Painter<'_>,g:&Game,mouse:Option<(i32,i32)>){
        p.fb.clear(0xFF243E48);
        self.city(p);
        for prop in &g.props {if prop.alive&&self.visible(prop.pos,prop.kind.radius()*1.9){self.prop(p,prop);}}
        // Holes drawn above props as portals cut into the map.
        for (i,h) in g.holes.iter().enumerate(){
            if !h.alive() {continue;}
            if !self.visible(h.pos,h.radius){continue;}
            let(x,y)=self.to_screen(h.pos);let r=h.radius*self.zoom;
            p.circle(x+3.0,y+6.0,r+5.0,with_alpha(INK,150));
            p.circle(x,y,r+5.0,with_alpha(h.color,60));
            p.ring(x,y,r+2.0,7.0_f32.max(5.0*self.zoom),h.color);
            p.circle(x,y,r-3.0,0xFF040911);
            p.ring(x,y,r*0.73,2.0,with_alpha(h.color,95));
            p.circle(x-r*0.24,y-r*0.22,r*0.10,with_alpha(h.color,110));
            let name=if i==0{"YOU"}else{h.name};
            let size=(12.0*self.zoom).clamp(11.0,20.0) as u16;
            p.centered(name,x as i32,(y-r-31.0) as i32,size,WHITE);
            if i==0 {p.ring(x,y,r+10.0,2.0,with_alpha(WHITE,120));}
        }
        for fx in &g.particles {if self.visible(fx.pos,2.0) {
            let(x,y)=self.to_screen(fx.pos);
            p.circle(x,y,(fx.life*8.0*self.zoom).clamp(1.0,4.0),with_alpha(fx.color,(fx.life*350.0).clamp(0.0,240.0) as u8));
        }}
        self.hud(p,g,mouse);
    }
    fn city(&self,p:&mut Painter<'_>){
        // Bound world-grid iteration to the actual city. Very large holes can
        // drive zoom toward zero; unbounded road-marking loops would otherwise
        // iterate over millions of invisible off-map segments.
        let left=(self.camera.x-self.width as f32/2.0/self.zoom-180.0).max(-180.0);
        let right=(self.camera.x+self.width as f32/2.0/self.zoom+180.0).min(WORLD_W+180.0);
        let top=(self.camera.y-self.height as f32/2.0/self.zoom-180.0).max(-180.0);
        let bottom=(self.camera.y+self.height as f32/2.0/self.zoom+180.0).min(WORLD_H+180.0);
        let ix0=(left/360.0).floor().max(0.0) as i32;
        let ix1=(right/360.0).ceil().min(11.0) as i32;
        let iy0=(top/350.0).floor().max(0.0) as i32;
        let iy1=(bottom/350.0).ceil().min(9.0) as i32;
        // Parks and paving between the road grid.
        for ix in ix0..=ix1{for iy in iy0..=iy1{
            let x=ix as f32*360.0+56.0;let y=iy as f32*350.0+56.0;
            let sx=self.sx(x);let sy=self.sy(y);
            let w=(248.0*self.zoom).ceil() as i32;let h=(238.0*self.zoom).ceil() as i32;
            if w<1||h<1{continue;}
            let park=(ix+iy*3)%7==0;
            p.rect(sx-5,sy-5,w+10,h+10,0xFF607680);
            p.rect(sx,sy,w,h,if park {0xFF385D55}else if (ix+iy)%2==0{0xFF416071}else{0xFF395763});
            // patterned slabs
            for k in 1..5 {let v=sx+(w*k)/5;p.rect(v,sy,1,h,with_alpha(WHITE,16));}
            for k in 1..5 {let v=sy+(h*k)/5;p.rect(sx,v,w,1,with_alpha(WHITE,16));}
            if park {
                p.rect(sx+w/2-2,sy,4,h,0xFF718A71);
                p.rect(sx,sy+h/2-2,w,4,0xFF718A71);
            }
        }}
        // Road system. Wide lanes, lane markers and crossings.
        for ix in ix0..=ix1 {
            let x=ix as f32*360.0;
            let sx=self.sx(x-53.0);let sw=(106.0*self.zoom).ceil() as i32;
            p.rect(sx,0,sw,self.height,0xFF1B303C);
            let center=self.sx(x);
            let mut segment=(top/70.0).floor() as i32;
            while segment as f32*70.0<bottom {
                let y=self.sy(segment as f32*70.0+12.0);
                p.rect(center-1,y,2,(33.0*self.zoom).ceil() as i32,0xFF68838A);segment+=1;
            }
            for iy in iy0..=iy1 {
                let cy=iy as f32*350.0;
                for k in -2..=2 {
                    let yy=self.sy(cy+k as f32*10.0);
                    p.rect(self.sx(x-50.0),yy,(100.0*self.zoom) as i32,(3.0*self.zoom).ceil() as i32,with_alpha(WHITE,70));
                }
            }
        }
        for iy in iy0..=iy1 {
            let y=iy as f32*350.0;let sy=self.sy(y-53.0);
            p.rect(0,sy,self.width,(106.0*self.zoom).ceil() as i32,0xFF213542);
            let center=self.sy(y);
            let mut segment=(left/70.0).floor() as i32;
            while segment as f32*70.0<right {
                let x=self.sx(segment as f32*70.0+12.0);
                p.rect(x,center-1,(33.0*self.zoom).ceil() as i32,2,0xFF8B8580);segment+=1;
            }
            for ix in ix0..=ix1 {
                let cx=ix as f32*360.0;
                for k in -2..=2 {
                    let xx=self.sx(cx+k as f32*10.0);
                    p.rect(xx,self.sy(y-50.0),(3.0*self.zoom).ceil() as i32,(100.0*self.zoom) as i32,with_alpha(WHITE,75));
                }
            }
        }
        // The city has an explicit world boundary.
        if left<0.0 {p.rect(self.sx(0.0)-5,0,5,self.height,TEAL);}
        if right>WORLD_W {p.rect(self.sx(WORLD_W),0,5,self.height,TEAL);}
        if top<0.0 {p.rect(0,self.sy(0.0)-5,self.width,5,TEAL);}
        if bottom>WORLD_H {p.rect(0,self.sy(WORLD_H),self.width,5,TEAL);}
    }
    #[inline] fn sx(&self,x:f32)->i32 {((x-self.camera.x)*self.zoom+self.width as f32*0.5).round() as i32}
    #[inline] fn sy(&self,y:f32)->i32 {((y-self.camera.y)*self.zoom+self.height as f32*0.5).round() as i32}
    fn prop(&self,p:&mut Painter<'_>,a:&Prop){
        let(x,y)=self.to_screen(a.pos);let r=(a.kind.radius()*self.zoom).max(2.0);
        let x=x.round() as i32;let y=y.round() as i32;
        match a.kind {
            Kind::Snack=>{
                let c=if a.tint%2==0{GOLD}else{0xFFFF918E};
                p.circle(x as f32+2.0,y as f32+3.0,r,with_alpha(INK,95));
                p.rounded(x-r as i32,y-r as i32,(r*2.0) as i32,(r*2.0) as i32,2,c);
                p.rect(x-2,y-r as i32+2,4,2,WHITE);
            }
            Kind::Cone=>{
                p.circle(x as f32+2.0,y as f32+3.0,r+3.0,with_alpha(INK,90));
                p.rect(x-r as i32,y-r as i32/3,(r*2.0) as i32,(r*0.65) as i32,0xFF1E2834);
                p.diamond(x,y,r as i32,0xFFFFA348);
                p.rect(x-3,y-2,6,3,WHITE);
            }
            Kind::Bin=>{
                p.rounded(x-r as i32+2,y-r as i32+4,(r*2.0) as i32,(r*2.0) as i32,3,with_alpha(INK,120));
                p.rounded(x-r as i32,y-r as i32,(r*2.0) as i32,(r*2.0) as i32,4,0xFF496D82);
                p.rect(x-r as i32+3,y-r as i32+3,(r*2.0) as i32-6,(r*2.0) as i32-6,0xFF264252);
                p.rect(x-4,y-3,8,6,TEAL);
            }
            Kind::Bench=>{
                p.rounded(x-r as i32+4,y-r as i32/2+5,(r*2.0) as i32,(r*1.05) as i32,3,with_alpha(INK,130));
                p.rect(x-r as i32,y-r as i32/2,(r*2.0) as i32,(r*0.43) as i32,0xFFD99E69);
                p.rect(x-r as i32,y+2,(r*2.0) as i32,(r*0.43) as i32,0xFFB97855);
                p.rect(x-r as i32+4,y-2,(r*2.0) as i32-8,2,0xFFFFD8AC);
            }
            Kind::Tree=>{
                p.circle(x as f32+4.0,y as f32+6.0,r,with_alpha(INK,110));
                p.circle(x as f32,y as f32,r,if a.tint%3==0{0xFF438D75}else{0xFF36A27C});
                p.circle(x as f32-r*0.30,y as f32-r*0.26,r*0.70,0xFF65CA8C);
                p.circle(x as f32+r*0.35,y as f32-r*0.05,r*0.52,0xFF248B70);
                p.circle(x as f32-r*0.22,y as f32-r*0.30,r*0.17,0xFFACE7B0);
            }
            Kind::Car=>{
                let w=(r*1.95) as i32;let h=(r*1.06) as i32;
                p.rounded(x-w/2+5,y-h/2+6,w,h,5,with_alpha(INK,140));
                let car=[0xFFFFD27C,0xFFED8A99,0xFF8CCCEF,0xFFDBE4F0,0xFFAAB9FD,0xFF7BD7BF][a.tint as usize%6];
                p.rounded(x-w/2,y-h/2,w,h,5,car);
                p.rounded(x-w/4,y-h/2+3,w/2,h-6,3,0xFF294A61);
                p.rect(x-w/2+3,y-h/2+3,4,3,0xFFFFFFCB);
                p.rect(x+w/2-7,y+h/2-6,4,3,0xFFFF626D);
                p.rect(x-w/2-1,y-h/2+2,3,4,0xFF172834);
            }
            Kind::House=>{
                let w=(r*1.82) as i32;let h=(r*1.55) as i32;
                p.rounded(x-w/2+12,y-h/2+17,w,h,5,with_alpha(INK,180));
                p.rect(x-w/2+6,y-h/2+9,w,h,0xFF263D4E);
                let roof=[0xFF7D8EA1,0xFF8D7F9C,0xFF6696A3,0xFF9F8C83,0xFF728B83][a.tint as usize%5];
                p.rect(x-w/2,y-h/2,w,h,roof);
                p.rect(x-w/2,y-h/2,w,6,0xFFBAC9D3);
                p.rect(x-w/2,y-h/2,6,h,0xFFACC3D2);
                p.rect(x-w/2+10,y-h/2+10,w-20,h-20,mix(roof,INK,0.18));
                let bx=x-w/2+16;let by=y-h/2+18;
                for j in 0..2 {for i in 0..3 {
                    p.rounded(bx+i*(w-37)/3,by+j*(h-39)/2,13,13,2,0xFF234358);
                    p.rect(bx+2+i*(w-37)/3,by+3+j*(h-39)/2,7,3,0xFF9BD6D8);
                }}
                p.rect(x+w/4,y+h/4,17,12,0xFF3B5867);
            }
        }
    }
    fn hud(&self,p:&mut Painter<'_>,g:&Game,mouse:Option<(i32,i32)>){
        let w=self.width;let h=self.height;
        // Top left: compact brand, score and progression.
        p.rounded(18,16,285,117,15,with_alpha(INK,220));
        p.rounded(24,23,35,35,11,0xFF342C55);
        p.circle(41.0,40.0,12.0,0xFF9672F4);p.circle(41.0,40.0,8.0,INK);
        p.label("MATA HOLE",70,22,23,WHITE);
        p.label("CITY EATER  /  ARCADE ARENA",71,53,11,DIM);
        p.label(&format!("{:05} PTS",g.holes[0].score),30,80,22,GOLD);
        p.label(&format!("SIZE {:.0}",g.holes[0].radius),190,91,12,TEAL);
        // Top center: round timer.
        let tw=168;let cx=w/2;
        p.rounded(cx-tw/2,18,tw,65,15,with_alpha(INK,215));
        let secs=g.remaining().ceil() as u32;
        p.centered(&format!("{:01}:{:02}",secs/60,secs%60),cx,24,29,if secs<25{PINK}else{WHITE});
        p.centered("ROUND TIME",cx,57,10,DIM);
        // Leaderboard, anchored to the top right.
        let right=w-226;
        p.rounded(right,16,208,208,15,with_alpha(INK,220));
        p.label("LEADERBOARD",right+16,26,15,WHITE);
        let standings=g.standings();
        for (rank,idx) in standings.iter().enumerate() {
            let h0=&g.holes[*idx];let y=57+rank as i32*26;
            if *idx==0{p.rounded(right+7,y-3,194,23,5,0xFF28364B);}
            p.circle((right+20) as f32,(y+8) as f32,5.0,h0.color);
            p.label(&format!("{}.",rank+1),right+31,y,11,DIM);
            p.label(h0.name,right+51,y,13,if *idx==0{TEAL}else{WHITE});
            let sc=format!("{}",h0.score);let x=right+192-p.font.measure(&sc,11);
            p.label(&sc,x,y+1,11,GOLD);
        }
        // Growth progress at lower left.
        let bottom=(h-91).max(166);
        p.rounded(18,bottom,321,72,13,with_alpha(INK,220));
        if let Some((next,need))=g.next_unlock() {
            p.label(&format!("NEXT: {}",next.name()),32,bottom+11,13,WHITE);
            let prev=match next{Kind::Cone=>23.5,Kind::Bin=>23.0,Kind::Bench=>29.0,Kind::Tree=>36.0,Kind::Car=>44.0,Kind::House=>60.0,Kind::Snack=>23.5};
            let value=((g.holes[0].radius-prev)/(need-prev).max(1.0)).clamp(0.0,1.0);
            p.rounded(32,bottom+42,291,12,6,0xFF2A3D4B);
            p.rounded(32,bottom+42,(291.0*value).round() as i32,12,6,TEAL);
            p.label(&format!("{:.0} / {:.0}",g.holes[0].radius,need),242,bottom+14,11,GOLD);
        } else {
            p.label("UNLIMITED GROWTH",32,bottom+10,16,TEAL);
            p.label("KEEP SWALLOWING TO GET BIGGER",32,bottom+37,11,WHITE);
        }
        // Energy bar / controls.
        let hud_y=(h-88).max(165);
        let bx=(w/2-151).max(349);
        p.rounded(bx,hud_y,302,69,14,with_alpha(INK,215));
        p.label("BOOST  [SHIFT]",bx+15,hud_y+9,12,WHITE);
        p.rect(bx+15,hud_y+35,267,12,0xFF2B4050);
        p.rect(bx+15,hud_y+35,(267.0*g.holes[0].energy) as i32,12,0xFFBA9CFF);
        p.label("WASD / ARROWS / MOUSE TO MOVE",bx+15,hud_y+54,10,DIM);
        // Mini map bottom right.
        let mx=w-203;let my=h-183;
        if mx>0&&my>225 {
            p.rounded(mx-10,my-25,195,183,12,with_alpha(INK,212));
            p.label("CITY OVERVIEW",mx,my-17,11,WHITE);
            p.rect(mx,my,175,137,0xFF314956);
            for ix in 1..10 {p.rect(mx+(ix*175/10),my,1,137,with_alpha(WHITE,24));}
            for iy in 1..8 {p.rect(mx,my+(iy*137/8),175,1,with_alpha(WHITE,24));}
            for (i,h0) in g.holes.iter().enumerate(){if h0.alive(){
                let xx=mx+(h0.pos.x/WORLD_W*175.0) as i32;
                let yy=my+(h0.pos.y/WORLD_H*137.0) as i32;
                p.circle(xx as f32,yy as f32,if i==0{5.0}else{3.0},h0.color);
            }}
        }
        if g.combo>1 && g.combo_timer>0.0 {
            p.centered(&format!("{}x CHAIN!",g.combo.min(99)),w/2,100,25,if g.combo>=8{GOLD}else{TEAL});
        }
        if g.last_eaten_timer>0.0 {if let Some(kind)=g.last_eaten {
            p.centered(&format!("+ {}",kind.name()),w/2,132,15,WHITE);
        }}
        if g.holes[0].respawn>0.0 {
            p.rounded(w/2-195,h/2+96,390,77,15,with_alpha(INK,230));
            p.centered("SWALLOWED BY ANOTHER HOLE",w/2,h/2+104,17,PINK);
            p.centered(&format!("RESPAWN IN {:.1}S",g.holes[0].respawn),w/2,h/2+138,15,WHITE);
        }
        // Cursor hint for mouse steering.
        if let Some((x,y))=mouse {if g.phase==Phase::Playing&&!g.paused {
            let dx=x-w/2;let dy=y-h/2;
            if dx*dx+dy*dy>36*36 {p.ring(x as f32,y as f32,10.0,2.0,with_alpha(WHITE,140));}
        }}
        if g.phase==Phase::Title{self.overlay(p,g,0);}
        if g.paused&&g.phase==Phase::Playing{self.overlay(p,g,1);}
        if g.phase==Phase::Finished{self.overlay(p,g,2);}
    }
    fn overlay(&self,p:&mut Painter<'_>,g:&Game,kind:u8){
        let w=self.width;let h=self.height;
        p.rect(0,0,w,h,with_alpha(INK,198));
        let pw=570.min(w-26);let ph=344.min(h-24);let x=(w-pw)/2;let y=(h-ph)/2;
        p.rounded(x,y,pw,ph,22,0xFF162734);
        p.rounded(x+8,y+8,pw-16,ph-16,18,0xFF203642);
        let title=match kind {0=>"MATA HOLE",1=>"PAUSED",_=>"ROUND OVER"};
        p.centered(title,w/2,y+29,39,WHITE);
        p.centered("SWALLOW THE CITY. OUTGROW EVERYONE.",w/2,y+86,14,TEAL);
        if kind==0{
            p.centered("CONSUME STREET OBJECTS TO GROW",w/2,y+132,17,WHITE);
            p.centered("BIG ENOUGH? EAT CARS, BUILDINGS AND RIVALS.",w/2,y+160,12,DIM);
            p.centered("MOVE: MOUSE / WASD / ARROW KEYS    BOOST: SHIFT",w/2,y+190,12,DIM);
            p.centered("YOU HAVE 2 MINUTES 30 SECONDS.",w/2,y+216,12,DIM);
        } else if kind==1 {
            p.centered("PRESS SPACE TO RESUME",w/2,y+157,20,WHITE);
        } else {
            let place=g.standings().iter().position(|&i|i==0).unwrap_or(0)+1;
            p.centered(&format!("{} WINS!",g.holes[g.winner].name),w/2,y+128,24,GOLD);
            p.centered(&format!("YOUR PLACE: #{}    SCORE: {}",place,g.holes[0].score),w/2,y+164,16,WHITE);
            p.centered(&format!("{} OBJECTS SWALLOWED",g.player_eaten),w/2,y+198,13,DIM);
        }
        p.rounded(w/2-174,y+ph-78,348,52,13,0xFF6F5CE2);
        p.centered(if kind==0{"START DEVOURING  [ENTER]"}else if kind==1{"RESUME  [SPACE]"}else{"PLAY AGAIN  [ENTER]"},w/2,y+ph-65,19,WHITE);
    }
}
