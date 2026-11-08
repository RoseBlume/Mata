use mata_win::Framebuffer;
use crate::font;

pub const BG:u32=0xff0b1220;
pub const PANEL:u32=0xff121f31;
pub const PANEL2:u32=0xff18283c;
pub const STROKE:u32=0xff2b3c54;
pub const WHITE:u32=0xffeff7ff;
pub const MUTED:u32=0xff97adc6;
pub const BLUE:u32=0xff34c8ce;
pub const GOLD:u32=0xffffbc68;
#[derive(Clone,Copy)] pub struct Rect {pub x:i32,pub y:i32,pub w:i32,pub h:i32}
impl Rect{pub const fn new(x:i32,y:i32,w:i32,h:i32)->Self{Self{x,y,w,h}} pub fn hit(self,x:i32,y:i32)->bool{x>=self.x&&y>=self.y&&x<self.x+self.w&&y<self.y+self.h}}

pub fn fill(f:&mut Framebuffer,r:Rect,color:u32){
 let x0=r.x.max(0).min(f.width as i32);let x1=(r.x+r.w).max(0).min(f.width as i32);
 let y0=r.y.max(0).min(f.height as i32);let y1=(r.y+r.h).max(0).min(f.height as i32);
 for y in y0..y1 {let row=y as usize*f.width as usize;f.pixels[row+x0 as usize..row+x1 as usize].fill(color);}
}
pub fn stroke(f:&mut Framebuffer,r:Rect,c:u32){fill(f,Rect::new(r.x,r.y,r.w,1),c);fill(f,Rect::new(r.x,r.y+r.h-1,r.w,1),c);fill(f,Rect::new(r.x,r.y,1,r.h),c);fill(f,Rect::new(r.x+r.w-1,r.y,1,r.h),c)}
pub fn circle(f:&mut Framebuffer,x:i32,y:i32,r:i32,color:u32){
 if r<=0{return;}let x0=(x-r).max(0);let x1=(x+r+1).min(f.width as i32);let y0=(y-r).max(0);let y1=(y+r+1).min(f.height as i32);
 for yy in y0..y1 {let dy=yy-y;let row=yy as usize*f.width as usize;for xx in x0..x1{let dx=xx-x;if dx*dx+dy*dy<=r*r{f.pixels[row+xx as usize]=color;}}}
}
pub fn line(f:&mut Framebuffer,ax:f32,ay:f32,bx:f32,by:f32,width:i32,color:u32){
 let dx=bx-ax;let dy=by-ay;let length=dx.abs().max(dy.abs()) as usize+1;
 let length=length.min(4000);for i in 0..=length {let t=i as f32/(length.max(1) as f32);circle(f,(ax+dx*t) as i32,(ay+dy*t) as i32,width.max(1),color);}
}
pub fn line_dashed(f:&mut Framebuffer,ax:f32,ay:f32,bx:f32,by:f32,width:i32,color:u32){
 let dx=bx-ax;let dy=by-ay;let length=dx.abs().max(dy.abs()) as usize+1;
 let length=length.min(4000);for i in 0..=length{if (i/7)%2==0{let t=i as f32/(length.max(1) as f32);circle(f,(ax+dx*t) as i32,(ay+dy*t) as i32,width.max(1),color);}}
}

// Fallback font for environments without a usable TrueType system font.
// The preferred rasterizer is mata_fonts; these fixed glyphs remain for recovery.
fn glyph(c:char)->[u8;7]{match c.to_ascii_uppercase(){
'A'=>[14,17,17,31,17,17,17], 'B'=>[30,17,17,30,17,17,30], 'C'=>[14,17,16,16,16,17,14],
'D'=>[28,18,17,17,17,18,28], 'E'=>[31,16,16,30,16,16,31], 'F'=>[31,16,16,30,16,16,16],
'G'=>[14,17,16,23,17,17,14], 'H'=>[17,17,17,31,17,17,17], 'I'=>[31,4,4,4,4,4,31],
'J'=>[7,2,2,2,18,18,12], 'K'=>[17,18,20,24,20,18,17], 'L'=>[16,16,16,16,16,16,31],
'M'=>[17,27,21,21,17,17,17], 'N'=>[17,25,21,19,17,17,17], 'O'=>[14,17,17,17,17,17,14],
'P'=>[30,17,17,30,16,16,16], 'Q'=>[14,17,17,17,21,18,13], 'R'=>[30,17,17,30,20,18,17],
'S'=>[15,16,16,14,1,1,30], 'T'=>[31,4,4,4,4,4,4], 'U'=>[17,17,17,17,17,17,14],
'V'=>[17,17,17,17,17,10,4], 'W'=>[17,17,17,21,21,21,10], 'X'=>[17,17,10,4,10,17,17],
'Y'=>[17,17,10,4,4,4,4], 'Z'=>[31,1,2,4,8,16,31],
'0'=>[14,17,19,21,25,17,14], '1'=>[4,12,4,4,4,4,14], '2'=>[14,17,1,2,4,8,31],
'3'=>[30,1,1,14,1,1,30], '4'=>[2,6,10,18,31,2,2], '5'=>[31,16,30,1,1,17,14],
'6'=>[6,8,16,30,17,17,14], '7'=>[31,1,2,4,8,8,8], '8'=>[14,17,17,14,17,17,14],
'9'=>[14,17,17,15,1,2,12],
'.'=>[0,0,0,0,0,12,12], ':'=>[0,12,12,0,12,12,0], '-'=>[0,0,0,31,0,0,0],
'+'=>[0,4,4,31,4,4,0], '/'=>[1,2,2,4,8,8,16], '\\'=>[16,8,8,4,2,2,1],
'('=>[2,4,8,8,8,4,2], ')'=>[8,4,2,2,2,4,8], '['=>[14,8,8,8,8,8,14],
']'=>[14,2,2,2,2,2,14], '='=>[0,31,0,31,0,0,0], '^'=>[4,10,17,0,0,0,0],
'_'=>[0,0,0,0,0,0,31], '#'=>[10,31,10,10,31,10,0], ','=>[0,0,0,0,0,4,8],
' '=>[0;7], '>'=>[16,8,4,2,4,8,16], '<'=>[1,2,4,8,4,2,1],
'!'=>[4,4,4,4,4,0,4], '?'=>[14,17,1,2,4,0,4], '%'=>[17,2,4,8,17,0,0],
'*'=>[0,10,4,31,4,10,0], '@'=>[14,17,23,21,23,16,14],
_=>[31,17,5,5,5,17,31]}}
pub fn text(f:&mut Framebuffer,mut x:i32,y:i32,s:&str,scale:i32,color:u32){
 if font::text(f,x,y,s,scale,color,None){return;}
 let scale=scale.clamp(1,5);for ch in s.chars(){for (yy,bits) in glyph(ch).into_iter().enumerate(){for xx in 0..5{if bits&(1<<(4-xx))!=0{fill(f,Rect::new(x+xx as i32*scale,y+yy as i32*scale,scale,scale),color);}}}x+=6*scale;}
}
pub fn text_clip(f:&mut Framebuffer,x:i32,y:i32,s:&str,scale:i32,color:u32,max_chars:usize){
 let display=s.chars().take(max_chars).collect::<String>();
 // The width limit protects panel borders even for proportionally spaced text.
 if !font::text(f,x,y,&display,scale,color,Some((max_chars as i32).saturating_mul(6*scale.clamp(1,5)))) {
   text(f,x,y,&display,scale,color);
 }
}
pub fn centered(f:&mut Framebuffer,cx:i32,y:i32,s:&str,scale:i32,c:u32){
 let width=font::width(s,scale).unwrap_or(s.chars().count() as i32*scale.clamp(1,5)*6);
 text(f,cx-width/2,y,s,scale,c);
}
pub fn element_color(s:&str)->u32{match s {
 "H"=>0xffe8effa,"C"=>0xff778293,"N"=>0xff428dff,"O"=>0xffff555d,
 "F"=>0xff76e3ae,"Cl"=>0xff55cb86,"Br"=>0xffbb7554,"I"=>0xffb77bea,
 "S"=>0xffffd369,"P"=>0xffff9853,"B"=>0xffe19aa5,"Si"=>0xffbaaba1,
 "Fe"=>0xffdf835b,"Cu"=>0xffd98256,"Au"=>0xffffc842,"Ag"=>0xffb8d2e1,
 "Na"|"K"|"Li"=>0xffb18bfb,"Mg"|"Ca"=>0xff90ca7b,"Xe"|"Ne"|"Ar"|"He"=>0xff7dcadf,
 _=>0xffa8c4d4
}}
