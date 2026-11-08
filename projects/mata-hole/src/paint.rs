//! Small CPU raster renderer built on mata_win::Framebuffer and mata_fonts.
//! No textures, game engine, UI framework, or downloadable assets required.
use std::{collections::HashMap,fs};
use mata_win::Framebuffer;
use mata_fonts::decoders::{Character,ttf::Ttf};

pub fn rgb(r:u8,g:u8,b:u8)->u32 {0xFF000000|((r as u32)<<16)|((g as u32)<<8)|b as u32}
pub fn with_alpha(color:u32,alpha:u8)->u32 {(color&0x00FF_FFFF)|((alpha as u32)<<24)}
pub fn mix(a:u32,b:u32,t:f32)->u32 {
    let t=t.clamp(0.0,1.0);
    let ch=|sh: u32| {let x=((a>>sh)&255_u32) as f32;let y=((b>>sh)&255_u32) as f32;(x+(y-x)*t).round() as u32};
    0xFF00_0000|(ch(16)<<16)|(ch(8)<<8)|ch(0)
}

pub struct Font {face:Option<Ttf>,glyphs:HashMap<(u16,char),Character>}
impl Font {
    pub fn new()->Self {
        let candidates=[
            "C:\\Windows\\Fonts\\segoeui.ttf","C:\\Windows\\Fonts\\arial.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
            "/System/Library/Fonts/Supplemental/Arial.ttf",
        ];
        let face=candidates.iter().find_map(|p|fs::read(p).ok().and_then(|bytes|Ttf::from_vec(bytes).ok()));
        Self {face,glyphs:HashMap::new()}
    }
    fn glyph(&mut self,pt:u16,ch:char)->Option<&Character> {
        let key=(pt,ch);
        if !self.glyphs.contains_key(&key) {
            if let Some(face)=&self.face {if let Ok(g)=face.character(pt,ch){self.glyphs.insert(key,g);}}
        }
        self.glyphs.get(&key)
    }
    pub fn measure(&mut self,text:&str,pt:u16)->i32 {
        if self.face.is_none(){return text.chars().count() as i32*(pt as i32*4/7+2);}
        text.chars().map(|ch|self.glyph(pt,ch).map(|g|g.advance_width as i32).unwrap_or(pt as i32/2)).sum()
    }
}

#[inline]
fn put_pixel(fb:&mut Framebuffer,x:i32,y:i32,color:u32){
    if x<0||y<0||x>=fb.width as i32||y>=fb.height as i32 {return;}
    let idx=y as usize*fb.width as usize+x as usize;
    let a=color>>24;
    if a==255 {fb.pixels[idx]=color;return;}
    if a==0 {return;}
    let dst=fb.pixels[idx];let inv=255-a;
    let r=(((color>>16&255)*a+(dst>>16&255)*inv+127)/255)<<16;
    let g=(((color>>8&255)*a+(dst>>8&255)*inv+127)/255)<<8;
    let b=((color&255)*a+(dst&255)*inv+127)/255;
    fb.pixels[idx]=0xFF000000|r|g|b;
}

pub struct Painter<'a> {pub fb:&'a mut Framebuffer,pub font:&'a mut Font}
impl<'a> Painter<'a> {
    pub fn new(fb:&'a mut Framebuffer,font:&'a mut Font)->Self{Self{fb,font}}
    #[inline] pub fn pixel(&mut self,x:i32,y:i32,color:u32){
        put_pixel(self.fb,x,y,color)
    }
    pub fn rect(&mut self,x:i32,y:i32,w:i32,h:i32,color:u32){
        let min_x=x.max(0).min(self.fb.width as i32);
        let max_x=(x+w).max(0).min(self.fb.width as i32);
        let min_y=y.max(0).min(self.fb.height as i32);
        let max_y=(y+h).max(0).min(self.fb.height as i32);
        if min_x>=max_x||min_y>=max_y{return;}
        if color>>24==255 {
            for yy in min_y..max_y{let from=yy as usize*self.fb.width as usize+min_x as usize;let end=from+(max_x-min_x) as usize;self.fb.pixels[from..end].fill(color);}
        }else{
            for yy in min_y..max_y{for xx in min_x..max_x {self.pixel(xx,yy,color);}}
        }
    }
    pub fn vgradient(&mut self,x:i32,y:i32,w:i32,h:i32,top:u32,bottom:u32){
        if h<=0{return;}
        for dy in 0..h {self.rect(x,y+dy,w,1,mix(top,bottom,dy as f32/h as f32));}
    }
    pub fn rounded(&mut self,x:i32,y:i32,w:i32,h:i32,r:i32,color:u32){
        if w<=0||h<=0{return;}
        let r=r.max(0).min(w/2).min(h/2);
        if r==0 {self.rect(x,y,w,h,color);return;}
        let min_y=y.max(0);let max_y=(y+h).min(self.fb.height as i32);
        for yy in min_y..max_y {
            let dy=if yy<y+r {y+r-yy}else if yy>=y+h-r {yy-(y+h-r-1)}else{0};
            let inset=if dy>0 {let a=(r*r-dy*dy).max(0) as f32;r-(a.sqrt() as i32)}else{0};
            self.rect(x+inset,yy,w-2*inset,1,color);
        }
    }
    pub fn outline(&mut self,x:i32,y:i32,w:i32,h:i32,r:i32,thick:i32,color:u32){
        self.rounded(x,y,w,h,r,color);
        self.rounded(x+thick,y+thick,w-2*thick,h-2*thick,(r-thick).max(0),0xFF101926);
    }
    pub fn circle(&mut self,cx:f32,cy:f32,r:f32,color:u32){
        if r<=0.0{return;}
        // Oversized holes can cover the entire screen. Render that case in
        // one fill instead of scanning a circle whose boundary is off-screen.
        let max_x=(self.fb.width as f64-cx as f64).abs().max((cx as f64).abs());
        let max_y=(self.fb.height as f64-cy as f64).abs().max((cy as f64).abs());
        if f64::from(r)>max_x.hypot(max_y) {
            self.rect(0,0,self.fb.width as i32,self.fb.height as i32,color);
            return;
        }
        let x0=((cx-r).floor() as i32).max(0);let x1=((cx+r).ceil() as i32).min(self.fb.width as i32-1);
        let y0=((cy-r).floor() as i32).max(0);let y1=((cy+r).ceil() as i32).min(self.fb.height as i32-1);
        let rr=r*r;
        for y in y0..=y1 {let dy=y as f32+0.5-cy;let dx2=rr-dy*dy;if dx2<0.0{continue;}
            let d=dx2.sqrt();let left=((cx-d).floor() as i32).max(x0);let right=((cx+d).ceil() as i32).min(x1);
            self.rect(left,y,right-left+1,1,color);
        }
    }
    pub fn ring(&mut self,cx:f32,cy:f32,r:f32,thick:f32,color:u32){
        if r<=0.0||thick<=0.0{return;}
        // If every screen pixel is inside the ring's inner circle, there is
        // no visible boundary. This keeps giant-hole rendering inexpensive.
        let max_x=(self.fb.width as f64-cx as f64).abs().max((cx as f64).abs());
        let max_y=(self.fb.height as f64-cy as f64).abs().max((cy as f64).abs());
        if f64::from((r-thick).max(0.0))>max_x.hypot(max_y){return;}
        let x0=((cx-r).floor() as i32).max(0);let x1=((cx+r).ceil() as i32).min(self.fb.width as i32-1);
        let y0=((cy-r).floor() as i32).max(0);let y1=((cy+r).ceil() as i32).min(self.fb.height as i32-1);
        let outer=r*r;let inner=(r-thick).max(0.0).powi(2);
        for y in y0..=y1 {for x in x0..=x1 {
            let d=(x as f32+0.5-cx).powi(2)+(y as f32+0.5-cy).powi(2);
            if d<=outer&&d>=inner {self.pixel(x,y,color);}
        }}
    }
    pub fn line(&mut self,ax:f32,ay:f32,bx:f32,by:f32,width:f32,color:u32){
        let half=width.max(1.0)*0.5;
        let x0=(ax.min(bx)-half-1.0).floor().max(0.0) as i32;
        let x1=(ax.max(bx)+half+1.0).ceil().min(self.fb.width as f32) as i32;
        let y0=(ay.min(by)-half-1.0).floor().max(0.0) as i32;
        let y1=(ay.max(by)+half+1.0).ceil().min(self.fb.height as f32) as i32;
        let dx=bx-ax;let dy=by-ay;let len=(dx*dx+dy*dy).max(1.0);
        for y in y0..y1 {for x in x0..x1 {
            let t=(((x as f32+0.5-ax)*dx+(y as f32+0.5-ay)*dy)/len).clamp(0.0,1.0);
            let px=ax+t*dx;let py=ay+t*dy;
            let d=(x as f32+0.5-px).powi(2)+(y as f32+0.5-py).powi(2);
            if d<=half*half {self.pixel(x,y,color);}
        }}
    }
    pub fn diamond(&mut self,cx:i32,cy:i32,r:i32,color:u32) {
        for dy in -r..=r {self.rect(cx-(r-dy.abs()),cy+dy,2*(r-dy.abs())+1,1,color);}
    }
    pub fn glow(&mut self,cx:f32,cy:f32,r:f32,color:u32){
        for i in (1..=5).rev(){self.circle(cx,cy,r*(i as f32)/5.0,with_alpha(color,7));}
    }
    pub fn label(&mut self,txt:&str,x:i32,y:i32,pt:u16,color:u32){
        let mut pen=x;
        if self.font.face.is_some() {
            for ch in txt.chars() {
                if let Some(g)=self.font.glyph(pt,ch) {
                    let gx=pen+g.bearing_x;let gy=y+g.baseline as i32-g.bearing_y;
                    let width=g.width as usize;let height=g.height as usize;
                    let r=color>>16&255;let gg=color>>8&255;let b=color&255;
                    let ca=color>>24;
                    for dy in 0..height {for dx in 0..width {
                        let coverage=g.pixels[dy*width+dx]&255;
                        if coverage>0 {put_pixel(self.fb,gx+dx as i32,gy+dy as i32,(coverage*ca/255<<24)|(r<<16)|(gg<<8)|b);}
                    }}
                    pen+=g.advance_width as i32;
                } else {pen+=pt as i32/2;}
            }
        } else {
            let size=(pt as i32/7).max(1);
            for ch in txt.chars(){let rows=bitmap(ch);for (row,bits) in rows.into_iter().enumerate(){for col in 0..5 {if bits&(1<<(4-col))!=0 {self.rect(pen+col*size,y+row as i32*size,size,size,color);}}}pen+=6*size;}
        }
    }
    pub fn centered(&mut self,txt:&str,cx:i32,y:i32,pt:u16,color:u32){let w=self.font.measure(txt,pt);self.label(txt,cx-w/2,y,pt,color);}
}

// Minimal offline font fallback. Real font rasterization uses mata_fonts.
fn bitmap(ch:char)->[u8;7]{
    match ch.to_ascii_uppercase(){
    'A'=>[14,17,17,31,17,17,17], 'B'=>[30,17,17,30,17,17,30],
    'C'=>[14,17,16,16,16,17,14], 'D'=>[30,17,17,17,17,17,30],
    'E'=>[31,16,16,30,16,16,31], 'F'=>[31,16,16,30,16,16,16],
    'G'=>[14,17,16,23,17,17,14], 'H'=>[17,17,17,31,17,17,17],
    'I'=>[14,4,4,4,4,4,14], 'J'=>[7,2,2,2,18,18,12],
    'K'=>[17,18,20,24,20,18,17], 'L'=>[16,16,16,16,16,16,31],
    'M'=>[17,27,21,21,17,17,17], 'N'=>[17,25,21,19,17,17,17],
    'O'=>[14,17,17,17,17,17,14], 'P'=>[30,17,17,30,16,16,16],
    'Q'=>[14,17,17,17,21,18,13], 'R'=>[30,17,17,30,20,18,17],
    'S'=>[15,16,16,14,1,1,30], 'T'=>[31,4,4,4,4,4,4],
    'U'=>[17,17,17,17,17,17,14], 'V'=>[17,17,17,17,17,10,4],
    'W'=>[17,17,17,21,21,21,10], 'X'=>[17,17,10,4,10,17,17],
    'Y'=>[17,17,10,4,4,4,4], 'Z'=>[31,1,2,4,8,16,31],
    '0'=>[14,17,19,21,25,17,14], '1'=>[4,12,4,4,4,4,14],
    '2'=>[14,17,1,2,4,8,31], '3'=>[30,1,1,14,1,1,30],
    '4'=>[2,6,10,18,31,2,2], '5'=>[31,16,16,30,1,1,30],
    '6'=>[14,16,16,30,17,17,14], '7'=>[31,1,2,4,8,8,8],
    '8'=>[14,17,17,14,17,17,14], '9'=>[14,17,17,15,1,1,14],
    ' '=>[0;7], ':'=>[0,4,4,0,4,4,0], '.'=>[0,0,0,0,0,12,12],
    '/'=>[1,1,2,4,8,16,16], '-'=>[0,0,0,31,0,0,0],
    '+'=>[0,4,4,31,4,4,0], '!'=>[4,4,4,4,4,0,4],
    '%'=>[25,25,2,4,8,19,19], '>'=>[16,8,4,2,4,8,16],
    '<'=>[1,2,4,8,4,2,1], '='=>[0,31,0,31,0,0,0],
    '('=>[2,4,8,8,8,4,2], ')'=>[8,4,2,2,2,4,8],
    '['=>[14,8,8,8,8,8,14], ']'=>[14,2,2,2,2,2,14],
    '#'=>[10,10,31,10,31,10,10], '?'=>[14,17,1,2,4,0,4],
    _=>[0;7],
    }
}
