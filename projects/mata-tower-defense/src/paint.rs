//! Small CPU raster renderer built on mata_win::Framebuffer and mata_fonts.
//! No textures, game engine, UI framework, or downloadable assets required.
use std::{collections::HashMap,fs};
use mata_win::Framebuffer;
use mata_fonts::decoders::{Character,ttf::Ttf};

pub fn rgb(r:u8,g:u8,b:u8)->u32 {0xFF000000|((r as u32)<<16)|((g as u32)<<8)|b as u32}
pub fn with_alpha(color:u32,alpha:u8)->u32 {(color&0x00FF_FFFF)|((alpha as u32)<<24)}
pub fn mix(a:u32,b:u32,t:f32)->u32 {
    let t=t.clamp(0.0,1.0);
    let ch=|sh: u32| {let x=((a>>sh)&255u32) as f32;let y=((b>>sh)&255u32) as f32;(x+(y-x)*t).round() as u32};
    0xFF00_0000|(ch(16)<<16)|(ch(8)<<8)|ch(0)
}


/// Mapping between the 1440x900 logical game coordinates and the current
/// *physical client-area pixels*. Maintain the aspect ratio: stretching the old
/// 1440x900 framebuffer is what made text and sprites look smeared on resize.
/// Rounding the viewport to whole pixels also makes the rendered edges and
/// hit-test edges agree across fractional Windows DPI scales.
#[derive(Clone,Copy,Debug)]
pub struct Viewport {
    pub x:i32,
    pub y:i32,
    pub width:u32,
    pub height:u32,
    sx:f32,
    sy:f32,
}
impl Viewport {
    pub fn new(client:(u32,u32), canvas:(u32,u32))->Option<Self> {
        let (w,h)=client;
        let (cw,ch)=canvas;
        if w==0 || h==0 || cw==0 || ch==0 {return None;}
        let scale=(w as f64/cw as f64).min(h as f64/ch as f64);
        let width=((cw as f64*scale).round() as u32).clamp(1,w);
        let height=((ch as f64*scale).round() as u32).clamp(1,h);
        Some(Self {
            x:((w-width)/2) as i32,
            y:((h-height)/2) as i32,
            width,height,
            sx:width as f32/cw as f32,
            sy:height as f32/ch as f32,
        })
    }
    #[inline] fn xx(self,x:f32)->i32 {self.x+(x*self.sx).round() as i32}
    #[inline] fn yy(self,y:f32)->i32 {self.y+(y*self.sy).round() as i32}
    // Native dimensions have at most one pixel of aspect-ratio rounding.
    #[inline] fn rr(self,r:f32)->f32 {r*((self.sx+self.sy)*0.5)}
    #[inline] fn font_size(self,pt:u16)->u16 {((pt as f32*self.sy).round() as i32).clamp(1,128) as u16}

    /// A raw native client pixel is converted into canvas coordinates only if
    /// it falls inside the same image rectangle used by Painter.
    pub fn pointer(self,x:i32,y:i32,canvas:(u32,u32))->Option<(i32,i32)> {
        let rx=x-self.x;
        let ry=y-self.y;
        if rx<0 || ry<0 || rx>=self.width as i32 || ry>=self.height as i32 {return None;}
        Some((
            ((rx as f64+0.5)*canvas.0 as f64/self.width as f64).floor() as i32,
            ((ry as f64+0.5)*canvas.1 as f64/self.height as f64).floor() as i32,
        ))
    }
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

/// Paint logical game coordinates directly into the native-resolution buffer.
/// All operations (including glyph rasterization) are rerun at the current
/// window size; no old framebuffer or bitmap is ever resized after rendering.
pub struct Painter<'a> {
    pub fb:&'a mut Framebuffer,
    pub font:&'a mut Font,
    viewport:Viewport,
}
impl<'a> Painter<'a> {
    pub fn new(fb:&'a mut Framebuffer,font:&'a mut Font,viewport:Viewport)->Self {
        Self {fb,font,viewport}
    }
    #[inline] pub fn pixel(&mut self,x:i32,y:i32,color:u32){
        self.rect(x,y,1,1,color);
    }
    #[inline] fn raw_pixel(&mut self,x:i32,y:i32,color:u32) {
        put_pixel(self.fb,x,y,color)
    }
    #[inline] pub fn rect(&mut self,x:i32,y:i32,w:i32,h:i32,color:u32){
        let v=self.viewport;
        let left=v.xx(x as f32);let top=v.yy(y as f32);
        let right=v.xx(x as f32+w as f32);let bottom=v.yy(y as f32+h as f32);
        self.raw_rect(left,top,right-left,bottom-top,color);
    }
    fn raw_rect(&mut self,x:i32,y:i32,w:i32,h:i32,color:u32){
        let min_x=x.max(0).min(self.fb.width as i32);
        let max_x=x.saturating_add(w).max(0).min(self.fb.width as i32);
        let min_y=y.max(0).min(self.fb.height as i32);
        let max_y=y.saturating_add(h).max(0).min(self.fb.height as i32);
        if min_x>=max_x||min_y>=max_y{return;}
        if color>>24==255 {
            for yy in min_y..max_y {let from=yy as usize*self.fb.width as usize+min_x as usize;let end=from+(max_x-min_x) as usize;self.fb.pixels[from..end].fill(color);}
        } else {
            for yy in min_y..max_y {for xx in min_x..max_x {self.raw_pixel(xx,yy,color);}}
        }
    }
    pub fn vgradient(&mut self,x:i32,y:i32,w:i32,h:i32,top:u32,bottom:u32){
        if h<=0{return;}
        // Interpolate in physical rows, without transforming hundreds of tiny
        // logical one-pixel strips (which can disappear when scaled down).
        let v=self.viewport;
        let x0=v.xx(x as f32);let x1=v.xx((x+w) as f32);
        let y0=v.yy(y as f32);let y1=v.yy((y+h) as f32);
        let height=y1-y0;
        if height<=0{return;}
        for yy in y0.max(0)..y1.min(self.fb.height as i32) {
            let t=(yy-y0) as f32/height as f32;
            self.raw_rect(x0,yy,x1-x0,1,mix(top,bottom,t));
        }
    }
    pub fn rounded(&mut self,x:i32,y:i32,w:i32,h:i32,r:i32,color:u32){
        let v=self.viewport;
        let left=v.xx(x as f32);let top=v.yy(y as f32);
        let right=v.xx((x+w) as f32);let bottom=v.yy((y+h) as f32);
        self.raw_rounded(left,top,right-left,bottom-top,v.rr(r as f32).round() as i32,color);
    }
    fn raw_rounded(&mut self,x:i32,y:i32,w:i32,h:i32,r:i32,color:u32){
        if w<=0||h<=0{return;}
        let r=r.max(0).min(w/2).min(h/2);
        if r==0 {self.raw_rect(x,y,w,h,color);return;}
        let min_y=y.max(0);let max_y=y.saturating_add(h).min(self.fb.height as i32);
        for yy in min_y..max_y {
            let dy=if yy<y+r {y+r-yy}else if yy>=y+h-r {yy-(y+h-r-1)}else{0};
            let inset=if dy>0 {let a=(r*r-dy*dy).max(0) as f32;r-(a.sqrt() as i32)}else{0};
            self.raw_rect(x+inset,yy,w-2*inset,1,color);
        }
    }
    pub fn outline(&mut self,x:i32,y:i32,w:i32,h:i32,r:i32,thick:i32,color:u32){
        self.rounded(x,y,w,h,r,color);
        self.rounded(x+thick,y+thick,w-2*thick,h-2*thick,(r-thick).max(0),0xFF101926);
    }
    pub fn circle(&mut self,cx:f32,cy:f32,r:f32,color:u32){
        let v=self.viewport;
        self.raw_circle(v.xx(cx) as f32,v.yy(cy) as f32,v.rr(r),color);
    }
    fn raw_circle(&mut self,cx:f32,cy:f32,r:f32,color:u32){
        if r<=0.0{return;}
        let x0=((cx-r).floor() as i32).max(0);let x1=((cx+r).ceil() as i32).min(self.fb.width as i32-1);
        let y0=((cy-r).floor() as i32).max(0);let y1=((cy+r).ceil() as i32).min(self.fb.height as i32-1);
        let rr=r*r;
        for y in y0..=y1 {let dy=y as f32+0.5-cy;let dx2=rr-dy*dy;if dx2<0.0{continue;}
            let d=dx2.sqrt();let left=((cx-d).floor() as i32).max(x0);let right=((cx+d).ceil() as i32).min(x1);
            self.raw_rect(left,y,right-left+1,1,color);
        }
    }
    pub fn ring(&mut self,cx:f32,cy:f32,r:f32,thick:f32,color:u32){
        let v=self.viewport;
        self.raw_ring(v.xx(cx) as f32,v.yy(cy) as f32,v.rr(r),v.rr(thick),color);
    }
    fn raw_ring(&mut self,cx:f32,cy:f32,r:f32,thick:f32,color:u32){
        if r<=0.0||thick<=0.0{return;}
        let x0=((cx-r).floor() as i32).max(0);let x1=((cx+r).ceil() as i32).min(self.fb.width as i32-1);
        let y0=((cy-r).floor() as i32).max(0);let y1=((cy+r).ceil() as i32).min(self.fb.height as i32-1);
        let outer=r*r;let inner=(r-thick).max(0.0).powi(2);
        for y in y0..=y1 {for x in x0..=x1 {
            let d=(x as f32+0.5-cx).powi(2)+(y as f32+0.5-cy).powi(2);
            if d<=outer&&d>=inner {self.raw_pixel(x,y,color);}
        }}
    }
    pub fn line(&mut self,ax:f32,ay:f32,bx:f32,by:f32,width:f32,color:u32){
        let v=self.viewport;
        self.raw_line(v.xx(ax) as f32,v.yy(ay) as f32,v.xx(bx) as f32,v.yy(by) as f32,v.rr(width),color);
    }
    fn raw_line(&mut self,ax:f32,ay:f32,bx:f32,by:f32,width:f32,color:u32){
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
            if d<=half*half {self.raw_pixel(x,y,color);}
        }}
    }
    pub fn diamond(&mut self,cx:i32,cy:i32,r:i32,color:u32) {
        if r<0{return;}
        // Re-rasterize the diamond at the native resolution; repeatedly
        // projecting one-logical-pixel scanlines can leave gaps on shrink.
        let v=self.viewport;
        let px=v.xx(cx as f32);
        let py=v.yy(cy as f32);
        let rx=(r as f32*v.sx).max(0.5);
        let ry=(r as f32*v.sy).max(0.5);
        let x0=(px as f32-rx).floor() as i32;
        let x1=(px as f32+rx).ceil() as i32;
        let y0=(py as f32-ry).floor().max(0.0) as i32;
        let y1=(py as f32+ry).ceil().min(self.fb.height as f32) as i32;
        for yy in y0..y1 {
            let spread=(1.0-((yy as f32+0.5-py as f32)/ry).abs()).max(0.0);
            let left=(px as f32-rx*spread).floor().max(x0 as f32) as i32;
            let right=(px as f32+rx*spread).ceil().min(x1 as f32) as i32;
            self.raw_rect(left,yy,(right-left).max(1),1,color);
        }
    }
    pub fn glow(&mut self,cx:f32,cy:f32,r:f32,color:u32){
        for i in (1..=5).rev(){self.circle(cx,cy,r*(i as f32)/5.0,with_alpha(color,7));}
    }
    pub fn label(&mut self,txt:&str,x:i32,y:i32,pt:u16,color:u32){
        let v=self.viewport;
        self.raw_label(txt,v.xx(x as f32),v.yy(y as f32),v.font_size(pt),color);
    }
    fn raw_label(&mut self,txt:&str,x:i32,y:i32,pt:u16,color:u32){
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
            for ch in txt.chars(){let rows=bitmap(ch);for (row,bits) in rows.into_iter().enumerate(){for col in 0..5 {if bits&(1<<(4-col))!=0 {self.raw_rect(pen+col*size,y+row as i32*size,size,size,color);}}}pen+=6*size;}
        }
    }
    pub fn centered(&mut self,txt:&str,cx:i32,y:i32,pt:u16,color:u32){
        let v=self.viewport;
        let size=v.font_size(pt);
        let width=self.font.measure(txt,size);
        self.raw_label(txt,v.xx(cx as f32)-width/2,v.yy(y as f32),size,color);
    }
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
