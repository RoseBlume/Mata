//! Bounded OpenType COLR v1 paint graph renderer.
//!
//! This renderer handles non-variable COLR v1 geometry and palette fills,
//! compositing in rrggbbaa. Variable COLR paint deltas are rejected rather
//! than silently interpreted at their default values.
use std::io::{self, Error, ErrorKind};
use crate::{colr::{ColorBitmap, layer_color}, opentype::Face};
fn bad(s:&str)->io::Error{Error::new(ErrorKind::InvalidData,s)}
fn u8_at(d:&[u8],o:usize)->io::Result<u8>{d.get(o).copied().ok_or_else(||bad("COLR v1 truncated field"))}
fn u16_at(d:&[u8],o:usize)->io::Result<u16>{let a=d.get(o..o.checked_add(2).ok_or_else(||bad("COLR offset overflow"))?).ok_or_else(||bad("COLR truncated u16"))?;Ok(u16::from_be_bytes([a[0],a[1]]))}
fn i16_at(d:&[u8],o:usize)->io::Result<i16>{Ok(u16_at(d,o)? as i16)}
fn u24_at(d:&[u8],o:usize)->io::Result<usize>{let a=d.get(o..o.checked_add(3).ok_or_else(||bad("COLR offset overflow"))?).ok_or_else(||bad("COLR truncated offset24"))?;Ok(((a[0] as usize)<<16)|((a[1] as usize)<<8)|a[2] as usize)}
fn u32_at(d:&[u8],o:usize)->io::Result<usize>{let a=d.get(o..o.checked_add(4).ok_or_else(||bad("COLR offset overflow"))?).ok_or_else(||bad("COLR truncated u32"))?;Ok(u32::from_be_bytes([a[0],a[1],a[2],a[3]]) as usize)}
fn f2_at(d:&[u8],o:usize)->io::Result<f32>{Ok(i16_at(d,o)? as f32/16384.0)}
fn fixed_at(d:&[u8],o:usize)->io::Result<f32>{Ok(u32_at(d,o)? as u32 as i32 as f32/65536.0)}
fn add(a:usize,b:usize,d:&[u8])->io::Result<usize>{let at=a.checked_add(b).ok_or_else(||bad("COLR offset overflow"))?;if at>=d.len(){return Err(bad("COLR offset outside table"));}Ok(at)}
#[derive(Clone,Copy,Debug)]
struct Affine{xx:f32,xy:f32,yx:f32,yy:f32,dx:f32,dy:f32}
impl Affine{
 fn identity()->Self{Self{xx:1.,xy:0.,yx:0.,yy:1.,dx:0.,dy:0.}}
 fn point(self,x:f32,y:f32)->(f32,f32){(self.xx*x+self.xy*y+self.dx,self.yx*x+self.yy*y+self.dy)}
 fn combine(self,b:Self)->Self{Self{xx:self.xx*b.xx+self.xy*b.yx,xy:self.xx*b.xy+self.xy*b.yy,yx:self.yx*b.xx+self.yy*b.yx,yy:self.yx*b.xy+self.yy*b.yy,dx:self.xx*b.dx+self.xy*b.dy+self.dx,dy:self.yx*b.dx+self.yy*b.dy+self.dy}}
 fn inverse(self)->Option<Self>{let d=self.xx*self.yy-self.xy*self.yx;if d.abs()<1e-8||!d.is_finite(){return None;}let xx=self.yy/d;let xy=-self.xy/d;let yx=-self.yx/d;let yy=self.xx/d;Some(Self{xx,xy,yx,yy,dx:-(xx*self.dx+xy*self.dy),dy:-(yx*self.dx+yy*self.dy)})}
}
#[derive(Clone)]struct Stop{offset:f32,color:u32}
#[derive(Clone)]struct ColorLine{extend:u8,stops:Vec<Stop>}
impl ColorLine{
 fn sample(&self,t:f32)->u32{
  if self.stops.is_empty()||!t.is_finite(){return 0;}
  if self.stops.len()==1{return self.stops[0].color;}
  let first=self.stops[0].offset;let last=self.stops[self.stops.len()-1].offset;
  let distance=last-first;
  let normalized=if distance<=1e-8{first}else{match self.extend{1=>first+(t-first).rem_euclid(distance),2=>{let u=(t-first).rem_euclid(distance*2.);first+if u>distance{distance*2.-u}else{u}},_=>t.clamp(first,last)}};
  if normalized<=first{return self.stops[0].color;}
  for pair in self.stops.windows(2){if normalized<=pair[1].offset{
   let a=((normalized-pair[0].offset)/(pair[1].offset-pair[0].offset).max(1e-8)).clamp(0.,1.);
   let mut result=0;for shift in [24,16,8,0]{let low=((pair[0].color>>shift)&255) as f32;let high=((pair[1].color>>shift)&255) as f32;result|=((low+(high-low)*a).round() as u32).min(255)<<shift;}return result;
  }}self.stops[self.stops.len()-1].color
 }
}
#[derive(Clone)]enum Fill{Solid(u32),Linear(ColorLine,[(f32,f32);3]),Radial(ColorLine,(f32,f32,f32),(f32,f32,f32)),Sweep(ColorLine,(f32,f32),f32,f32)}
impl Fill{
 fn sample(&self,x:f32,y:f32)->u32{match self{
  Self::Solid(c)=>*c,
  Self::Linear(c,[p0,p1,p2])=>{let dx=x-p0.0;let dy=y-p0.1;let v=(p1.0-p0.0,p1.1-p0.1);let w=(p2.0-p0.0,p2.1-p0.1);let denominator=v.0*w.1-v.1*w.0;let t=if denominator.abs()>1e-5{(dx*w.1-dy*w.0)/denominator}else{(dx*v.0+dy*v.1)/(v.0*v.0+v.1*v.1).max(1e-6)};c.sample(t)},
  Self::Radial(c,(x0,y0,r0),(x1,y1,r1))=>{let vx=x1-x0;let vy=y1-y0;let vr=r1-r0;let px=x-x0;let py=y-y0;let a=vx*vx+vy*vy-vr*vr;let b=-2.*(px*vx+py*vy+r0*vr);let cc=px*px+py*py-r0*r0;let t=if a.abs()<1e-6{if b.abs()<1e-6{0.}else{-cc/b}}else{let d=b*b-4.*a*cc;if d<0.{0.}else{let t1=(-b-d.sqrt())/(2.*a);let t2=(-b+d.sqrt())/(2.*a);if t1>=0.&&r0+vr*t1>=0.{t1}else{t2}}};c.sample(t)},
  Self::Sweep(c,(cx,cy),start,end)=>{let angle=(y-cy).atan2(x-cx).to_degrees().rem_euclid(360.);let t=(angle-start)/(end-start).max(1e-5);c.sample(t)},
 }}
}
#[derive(Clone)]struct Sprite{x:i32,y:i32,w:usize,h:usize,pixels:Vec<u32>}
impl Sprite{
 fn bounds(images:&[Self])->io::Result<Option<(i32,i32,usize,usize)>>{
  let mut minx=i64::MAX;let mut miny=i64::MAX;let mut maxx=i64::MIN;let mut maxy=i64::MIN;
  for im in images{if im.w==0||im.h==0{continue;}minx=minx.min(im.x as i64);miny=miny.min(im.y as i64);maxx=maxx.max(im.x as i64+im.w as i64);maxy=maxy.max(im.y as i64+im.h as i64);}
  if minx==i64::MAX{return Ok(None);}let w=maxx-minx;let h=maxy-miny;
  if w>4096||h>4096||w*h>16_777_216{return Err(bad("COLR v1 canvas exceeds 4096px / 16Mpx limit"));}
  Ok(Some((minx as i32,miny as i32,w as usize,h as usize)))
 }
 fn flatten(images:&[Self])->io::Result<Option<Self>>{
  let Some((x,y,w,h))=Self::bounds(images)? else{return Ok(None)};
  let mut output=Self{x,y,w,h,pixels:vec![0;w*h]};
  for im in images{for iy in 0..im.h{for ix in 0..im.w{
   let dx=im.x as i64+ix as i64-x as i64;let dy=im.y as i64+iy as i64-y as i64;
   if dx<0||dy<0||dx>=w as i64||dy>=h as i64{continue;}
   let at=dy as usize*w+dx as usize;
   output.pixels[at]=composite_pixel(output.pixels[at],im.pixels[iy*im.w+ix],3);
  }}}
  Ok(Some(output))
 }
}
struct Renderer<'a>{face:&'a Face,colr:&'a [u8],cpal:&'a [u8],palette:usize,foreground:u32,scale:f32,root:usize,layer_list:usize,nodes:usize,painted_pixels:usize,active:Vec<usize>}
impl<'a> Renderer<'a>{
 fn base_paint(&self,glyph:u16)->io::Result<Option<usize>>{
  if self.root==0{return Ok(None)};
  let count=u32_at(self.colr,self.root)?;if count>65536{return Err(bad("COLR v1 excessive base records"));}
  let mut low=0;let mut high=count;
  while low<high{let mid=(low+high)/2;let at=add(self.root,4+mid*6,self.colr)?;if u16_at(self.colr,at)?<glyph{low=mid+1}else{high=mid;}}
  if low>=count{return Ok(None);}let at=add(self.root,4+low*6,self.colr)?;
  if u16_at(self.colr,at)?!=glyph{return Ok(None);}
  Ok(Some(add(self.root,u32_at(self.colr,at+2)?,self.colr)?))
 }
 fn delta(&self,index:u32)->io::Result<f32>{
  let axes=self.face.variation_axes();
  let v=crate::variations::colr_delta(self.colr,&axes,self.face.variations(),self.face.table(*b"avar"),index)
    .ok_or_else(||bad("COLR invalid variation map or item store"))?;
  if !v.is_finite()||v.abs()>1_000_000.{return Err(bad("COLR variation delta out of range"));}Ok(v)
 }
 fn varied(&self,base:f32,index_base:usize,field:usize,unit:f32)->io::Result<f32>{
  let index=u32_at(self.colr,index_base)? as u32;
  let v=if index==u32::MAX{base}else{base+self.delta(index.checked_add(field as u32).ok_or_else(||bad("COLR variation index overflow"))?)?*unit};
  if !v.is_finite(){return Err(bad("COLR varied value is not finite"));}Ok(v)
 }
 fn clip_box(&self,glyph:u16)->io::Result<Option<[f32;4]>>{
  let list=u32_at(self.colr,22)?;
  if list==0{return Ok(None);}
  if u8_at(self.colr,list)?!=1{return Err(bad("COLR ClipList format not supported"));}
  let count=u32_at(self.colr,list+1)?;
  if count>65536{return Err(bad("COLR excessive clip records"));}
  for index in 0..count{
   let rec=add(list,5+index*7,self.colr)?;
   let start=u16_at(self.colr,rec)?;let end=u16_at(self.colr,rec+2)?;
   if glyph<start||glyph>end{continue;}
   let box_at=add(list,u24_at(self.colr,rec+4)?,self.colr)?;
   let format=u8_at(self.colr,box_at)?;
   if format!=1 && format!=2{return Err(bad("COLR unsupported clip box format"));}
   let mut b=[0.;4];
   for (i,v) in b.iter_mut().enumerate(){
     let original=i16_at(self.colr,box_at+1+i*2)? as f32;
     *v=if format==2{self.varied(original,box_at+9,i,1.)?}else{original};
   }
   if b[2]<b[0]||b[3]<b[1]{return Err(bad("COLR invalid clip rectangle"));}
   return Ok(Some(b));
  }
  Ok(None)
 }
 fn color_line(&self,off:usize,variable:bool)->io::Result<ColorLine>{
  let extend=u8_at(self.colr,off)?;let count=u16_at(self.colr,off+1)? as usize;
  if count>1024{return Err(bad("COLR excessive gradient stops"));}
  let mut stops=Vec::with_capacity(count);
  for i in 0..count{
   let at=add(off,3+i*if variable{10}else{6},self.colr)?;
   let offset=if variable{self.varied(f2_at(self.colr,at)?,at+6,0,1./16384.)?}else{f2_at(self.colr,at)?};
   let palette_index=u16_at(self.colr,at+2)?;
   let alpha=if variable{self.varied(f2_at(self.colr,at+4)?,at+6,1,1./16384.)?}else{f2_at(self.colr,at+4)?};
   let alpha=alpha.clamp(0.,1.);
   let color=layer_color(self.cpal,self.palette,palette_index,self.foreground).ok_or_else(||bad("COLR palette index outside CPAL"))?;
   let a=((color&255) as f32*alpha).round().clamp(0.,255.) as u32;
   stops.push(Stop{offset,color:(color&!255)|a});
  }
  stops.sort_by(|a,b|a.offset.total_cmp(&b.offset));
  Ok(ColorLine{extend,stops})
 }
 fn fill(&self,at:usize)->io::Result<Fill>{
  let format=u8_at(self.colr,at)?;
  match format{
   2|3=>{
    let index=u16_at(self.colr,at+1)?;
    let alpha=if format==3{self.varied(f2_at(self.colr,at+3)?,at+5,0,1./16384.)?}else{f2_at(self.colr,at+3)?};
    let color=layer_color(self.cpal,self.palette,index,self.foreground).ok_or_else(||bad("COLR palette index outside CPAL"))?;
    Ok(Fill::Solid((color&!255)|(((color&255) as f32*alpha.clamp(0.,1.)).round() as u32).min(255)))
   },
   4|5=>{
    let variable=format==5;
    let line=self.color_line(add(at,u24_at(self.colr,at+1)?,self.colr)?,variable)?;
    let mut points=[(0.,0.);3];
    for (i,p) in points.iter_mut().enumerate(){
     let x=i16_at(self.colr,at+4+i*4)? as f32;let y=i16_at(self.colr,at+6+i*4)? as f32;
     *p=if variable{(self.varied(x,at+16,i*2,1.)?,self.varied(y,at+16,i*2+1,1.)?)}else{(x,y)};
    }
    Ok(Fill::Linear(line,points))
   },
   6|7=>{
    let variable=format==7;
    let line=self.color_line(add(at,u24_at(self.colr,at+1)?,self.colr)?,variable)?;
    let mut vals=[0.;6];
    for (i,v) in vals.iter_mut().enumerate(){
     let raw=if i==2||i==5{u16_at(self.colr,at+4+i*2)? as f32}else{i16_at(self.colr,at+4+i*2)? as f32};
     *v=if variable{self.varied(raw,at+16,i,1.)?}else{raw};
    }
    if vals[2]<0.||vals[5]<0.{return Err(bad("COLR negative radial gradient radius"));}
    Ok(Fill::Radial(line,(vals[0],vals[1],vals[2]),(vals[3],vals[4],vals[5])))
   },
   8|9=>{
    let variable=format==9;
    let line=self.color_line(add(at,u24_at(self.colr,at+1)?,self.colr)?,variable)?;
    let mut vals=[0.;4];
    for (i,v) in vals.iter_mut().enumerate(){
     let raw=if i<2{i16_at(self.colr,at+4+i*2)? as f32}else{f2_at(self.colr,at+4+i*2)?};
     let unit=if i<2{1.}else{1./16384.};
     *v=if variable{self.varied(raw,at+12,i,unit)?}else{raw};
    }
    Ok(Fill::Sweep(line,(vals[0],vals[1]),(vals[2]+1.)*180.,(vals[3]+1.)*180.))
   },
   _=>Err(bad("COLR v1 unsupported fill format")),
  }
 }
 fn paint_glyph(&mut self,id:u16,fill:Fill,m:Affine)->io::Result<Vec<Sprite>>{
  let mask=self.face.rasterize_glyph_instance(id,self.scale*self.face.units_per_em as f32)?;
  if mask.width==0||mask.height==0{return Ok(Vec::new());}
  let (bx,by)=(mask.bearing_x as f32,-mask.bearing_y as f32);
  let corners=[m.point(bx,by),m.point(bx+mask.width as f32,by),m.point(bx,by+mask.height as f32),m.point(bx+mask.width as f32,by+mask.height as f32)];
  let minx=corners.iter().map(|p|p.0).fold(f32::INFINITY,f32::min).floor();let miny=corners.iter().map(|p|p.1).fold(f32::INFINITY,f32::min).floor();
  let maxx=corners.iter().map(|p|p.0).fold(f32::NEG_INFINITY,f32::max).ceil();let maxy=corners.iter().map(|p|p.1).fold(f32::NEG_INFINITY,f32::max).ceil();
  if ![minx,miny,maxx,maxy].iter().all(|v|v.is_finite()&&v.abs()<65536.){return Err(bad("COLR transformed glyph exceeds coordinates limit"));}
  let w=(maxx-minx).max(0.) as usize;let h=(maxy-miny).max(0.) as usize;
  if w>4096||h>4096||w.checked_mul(h).map(|n|n>16_777_216).unwrap_or(true){return Err(bad("COLR transformed glyph exceeds raster limit"));}
  let pixels=w.checked_mul(h).ok_or_else(||bad("COLR image dimensions overflow"))?;
  let budget=self.painted_pixels.checked_add(pixels).ok_or_else(||bad("COLR paint allocation overflow"))?;
  if budget>16_777_216{return Err(bad("COLR v1 paint graph exceeds 16 million raster samples"));}
  self.painted_pixels=budget;
  let Some(inv)=m.inverse()else{return Err(bad("COLR non-invertible paint transform"));};
  let mut output=Sprite{x:minx as i32,y:miny as i32,w,h,pixels:vec![0;w*h]};
  for y in 0..h{for x in 0..w{
   let (sx,sy)=inv.point(minx+x as f32+0.5,miny+y as f32+0.5);
   let px=sx-bx;let py=sy-by;
   let ix=px.floor() as i32;let iy=py.floor() as i32;
   if ix<0||iy<0||ix>=mask.width as i32||iy>=mask.height as i32{continue;}
   let coverage=mask.pixels[iy as usize*mask.width as usize+ix as usize].min(255);
   if coverage==0{continue;}
   let color=fill.sample(sx/self.scale,-sy/self.scale);
   output.pixels[y*w+x]=(color&!255)|(((color&255)*coverage+127)/255);
  }}
  Ok(vec![output])
 }
 fn render(&mut self,at:usize,m:Affine,depth:usize)->io::Result<Vec<Sprite>>{
  if depth>48||self.nodes>=8192{return Err(bad("COLR v1 paint graph exceeds recursion/node limits"));}
  if self.active.contains(&at){return Err(bad("cyclic COLR v1 paint graph"));}
  self.nodes+=1;self.active.push(at);
  let output=self.render_node(at,m,depth);
  self.active.pop();output
 }
 fn render_node(&mut self,at:usize,m:Affine,depth:usize)->io::Result<Vec<Sprite>>{
  let format=u8_at(self.colr,at)?;
  let child=|offset:usize|add(at,offset,self.colr);
  match format{
   1=>{
    let n=u8_at(self.colr,at+1)? as usize;let first=u32_at(self.colr,at+2)?;
    if self.layer_list==0{return Err(bad("COLR v1 missing LayerList"));}
    let total=u32_at(self.colr,self.layer_list)?;
    if first.checked_add(n).map(|v|v>total).unwrap_or(true){return Err(bad("COLR v1 layer index out of range"));}
    let mut layers=Vec::new();
    for i in first..first+n{
     let entry=add(self.layer_list,4+i*4,self.colr)?;
     let off=u32_at(self.colr,entry)?;
     layers.extend(self.render(add(self.layer_list,off,self.colr)?,m,depth+1)?);
    }Ok(layers)
   },
   10=>{
    let paint=child(u24_at(self.colr,at+1)?)?;let gid=u16_at(self.colr,at+4)?;
    if matches!(u8_at(self.colr,paint)?,2|3|4|5|6|7|8|9){let fill=self.fill(paint)?;self.paint_glyph(gid,fill,m)}
    else{
     // A PaintGlyph may clip an arbitrary child paint graph.
     let child_sprites=self.render(paint,m,depth+1)?;
     let Some(mut composite)=Sprite::flatten(&child_sprites)? else{return Ok(Vec::new());};
     let glyph_mask=self.paint_glyph(gid,Fill::Solid(0xffffffff),m)?;
     let Some(mask)=Sprite::flatten(&glyph_mask)? else{return Ok(Vec::new());};
     for y in 0..composite.h{for x in 0..composite.w{
      let mx=composite.x as i64+x as i64-mask.x as i64;let my=composite.y as i64+y as i64-mask.y as i64;
      let alpha=if mx>=0&&my>=0&&mx<mask.w as i64&&my<mask.h as i64{mask.pixels[my as usize*mask.w+mx as usize]&255}else{0};
      let at=y*composite.w+x;let a=(composite.pixels[at]&255)*alpha/255;composite.pixels[at]=(composite.pixels[at]&!255)|a;
     }}Ok(vec![composite])
    }
   },
   11=>{let gid=u16_at(self.colr,at+1)?;let Some(start)=self.base_paint(gid)? else{return Err(bad("COLR PaintColrGlyph references missing base glyph"))};self.render(start,m,depth+1)},
   12|13=>{
    let p=child(u24_at(self.colr,at+1)?)?;let t=child(u24_at(self.colr,at+4)?)?;
    let variable=format==13;let s=self.scale;
    let mut parts=[0.;6];
    for i in 0..6{
     let base=fixed_at(self.colr,t+i*4)?;
     parts[i]=if variable{self.varied(base,t+24,i,1./65536.)?}else{base};
    }
    let affine=Affine{xx:parts[0],yx:-parts[1],xy:-parts[2],yy:parts[3],dx:s*parts[4],dy:-s*parts[5]};
    self.render(p,m.combine(affine),depth+1)
   },
   14|15=>{
    let p=child(u24_at(self.colr,at+1)?)?;
    let x=i16_at(self.colr,at+4)? as f32;let y=i16_at(self.colr,at+6)? as f32;
    let (dx,dy)=if format==15{(self.varied(x,at+8,0,1.)?,self.varied(y,at+8,1,1.)?)}else{(x,y)};
    self.render(p,m.combine(Affine{dx:dx*self.scale,dy:-dy*self.scale,..Affine::identity()}),depth+1)
   },
   16|17|18|19|20|21|22|23=>{
    let p=child(u24_at(self.colr,at+1)?)?;
    let variable=format%2==1;
    let centered=matches!(format,18|19|22|23);
    let uniform=matches!(format,20|21|22|23);
    let (sx,sy,offset)=if uniform{
     let raw=f2_at(self.colr,at+4)?;
     (raw,raw,if centered{10}else{6})
    }else{
     (f2_at(self.colr,at+4)?,f2_at(self.colr,at+6)?,if centered{12}else{8})
    };
    let count=if uniform{1}else{2};
    let (sx,sy)=if variable{
     let x=self.varied(sx,at+offset,0,1./16384.)?;
     (x,if uniform{x}else{self.varied(sy,at+offset,1,1./16384.)?})
    }else{(sx,sy)};
    let (cx,cy)=if centered{
     let at_center=if uniform{6}else{8};
     let x=i16_at(self.colr,at+at_center)? as f32;
     let y=i16_at(self.colr,at+at_center+2)? as f32;
     if variable{(self.varied(x,at+offset,count,1.)?,self.varied(y,at+offset,count+1,1.)?)}else{(x,y)}
    }else{(0.,0.)};
    let cx=cx*self.scale;let cy=-cy*self.scale;
    let affine=Affine{xx:sx,yy:sy,dx:cx*(1.-sx),dy:cy*(1.-sy),..Affine::identity()};
    self.render(p,m.combine(affine),depth+1)
   },
   24|25|26|27=>{
    let p=child(u24_at(self.colr,at+1)?)?;
    let centered=format>=26;let variable=format%2==1;
    let raw=f2_at(self.colr,at+4)?;
    let index_offset=if centered{10}else{6};
    let angle=if variable{self.varied(raw,at+index_offset,0,1./16384.)?}else{raw};
    let angle=angle*std::f32::consts::PI;
    let (cx,cy)=if centered{
     let x=i16_at(self.colr,at+6)? as f32;
     let y=i16_at(self.colr,at+8)? as f32;
     let (x,y)=if variable{(self.varied(x,at+index_offset,1,1.)?,self.varied(y,at+index_offset,2,1.)?)}else{(x,y)};
     (x*self.scale,-y*self.scale)
    }else{(0.,0.)};
    let (sn,cs)=angle.sin_cos();let affine=Affine{xx:cs,xy:sn,yx:-sn,yy:cs,dx:cx-cs*cx-sn*cy,dy:cy+sn*cx-cs*cy};
    self.render(p,m.combine(affine),depth+1)
   },
   28|29|30|31=>{
    let p=child(u24_at(self.colr,at+1)?)?;
    let centered=format>=30;let variable=format%2==1;
    let index_offset=if centered{12}else{8};
    let (rawx,rawy)=(f2_at(self.colr,at+4)?,f2_at(self.colr,at+6)?);
    let (rawx,rawy)=if variable{(self.varied(rawx,at+index_offset,0,1./16384.)?,self.varied(rawy,at+index_offset,1,1./16384.)?)}else{(rawx,rawy)};
    let skewx=(rawx*std::f32::consts::PI).tan();let skewy=(rawy*std::f32::consts::PI).tan();
    if !skewx.is_finite()||!skewy.is_finite(){return Err(bad("COLR infinite skew"));}
    let (cx,cy)=if centered{
     let x=i16_at(self.colr,at+8)? as f32;
     let y=i16_at(self.colr,at+10)? as f32;
     let (x,y)=if variable{(self.varied(x,at+index_offset,2,1.)?,self.varied(y,at+index_offset,3,1.)?)}else{(x,y)};
     (x*self.scale,-y*self.scale)
    }else{(0.,0.)};
    let affine=Affine{xx:1.,xy:skewx,yx:-skewy,yy:1.,dx:-skewx*cy,dy:skewy*cx};
    self.render(p,m.combine(affine),depth+1)
   },
   32=>{
    let source=child(u24_at(self.colr,at+1)?)?;let mode=u8_at(self.colr,at+4)?;let backdrop=child(u24_at(self.colr,at+5)?)?;
    let bg=self.render(backdrop,m,depth+1)?;let fg=self.render(source,m,depth+1)?;
    let Some(composed)=composite(&bg,&fg,mode)? else{return Ok(Vec::new());};Ok(vec![composed])
   },
   _=>Err(bad("COLR v1 unsupported or variable paint format")),
  }
 }
}
fn srgb_to_linear(x:f32)->f32{if x<=0.04045{x/12.92}else{((x+0.055)/1.055).powf(2.4)}}
fn linear_to_srgb(x:f32)->f32{let x=x.clamp(0.,1.);if x<=0.0031308{x*12.92}else{1.055*x.powf(1./2.4)-0.055}}
fn lum(c:[f32;3])->f32{0.3*c[0]+0.59*c[1]+0.11*c[2]}
fn sat(c:[f32;3])->f32{c.iter().copied().fold(0.,f32::max)-c.iter().copied().fold(1.,f32::min)}
fn clip_color(mut c:[f32;3])->[f32;3]{
 let l=lum(c);let n=c.iter().copied().fold(f32::INFINITY,f32::min);let x=c.iter().copied().fold(f32::NEG_INFINITY,f32::max);
 if n<0.{for v in &mut c{*v=l+(*v-l)*l/(l-n).max(1e-8);}}
 if x>1.{for v in &mut c{*v=l+(*v-l)*(1.-l)/(x-l).max(1e-8);}}
 c
}
fn set_lum(c:[f32;3],l:f32)->[f32;3]{let d=l-lum(c);clip_color([c[0]+d,c[1]+d,c[2]+d])}
fn set_sat(mut c:[f32;3],s:f32)->[f32;3]{
 let mut indices=[0usize,1,2];indices.sort_by(|&a,&b|c[a].total_cmp(&c[b]));
 let (lo,mid,hi)=(indices[0],indices[1],indices[2]);
 if c[hi]>c[lo]{c[mid]=(c[mid]-c[lo])*s/(c[hi]-c[lo]);c[hi]=s;}else{c[mid]=0.;c[hi]=0.;}
 c[lo]=0.;c
}
fn blend_channels(back:[f32;3],front:[f32;3],mode:u8)->[f32;3]{
 if (24..=27).contains(&mode){return match mode{
 24=>set_lum(set_sat(front,sat(back)),lum(back)),
 25=>set_lum(set_sat(back,sat(front)),lum(back)),
 26=>set_lum(front,lum(back)),
 _=>set_lum(back,lum(front))};}
 let mut out=[0.;3];for i in 0..3{let b=back[i];let f=front[i];out[i]=match mode{
 13=>f+b-f*b,
 14=>if b<=0.5{2.*f*b}else{1.-2.*(1.-f)*(1.-b)},
 15=>f.min(b),16=>f.max(b),
 17=>if f>=1.{1.}else{(b/(1.-f)).min(1.)},
 18=>if f<=0.{0.}else{1.-((1.-b)/f).min(1.)},
 19=>if f<=0.5{2.*f*b}else{1.-2.*(1.-f)*(1.-b)},
 20=>if f<=0.5{b-(1.-2.*f)*b*(1.-b)}else{
     let d=if b<=0.25{((16.*b-12.)*b+4.)*b}else{b.sqrt()};b+(2.*f-1.)*(d-b)},
 21=>(b-f).abs(),22=>b+f-2.*b*f,23=>b*f,
 _=>f};}out
}
fn composite_pixel(dest:u32,src:u32,mode:u8)->u32{
 // Unrecognized composite modes are defined to produce COMPOSITE_CLEAR.
 if mode>27{return 0;}
 let ad=(dest&255) as f32/255.;let asrc=(src&255) as f32/255.;
 let read=|v:u32|-> [f32;3]{
  [srgb_to_linear(((v>>24)&255) as f32/255.),srgb_to_linear(((v>>16)&255) as f32/255.),srgb_to_linear(((v>>8)&255) as f32/255.)]};
 let d=read(dest);let s=read(src);
 let (fa,fb)=match mode{
 0=>(0.,0.),1=>(1.,0.),2=>(0.,1.),3=>(1.,1.-asrc),4=>(1.-ad,1.),
 5=>(ad,0.),6=>(0.,asrc),7=>(1.-ad,0.),8=>(0.,1.-asrc),
 9=>(ad,1.-asrc),10=>(1.-ad,asrc),11=>(1.-ad,1.-asrc),12=>(1.,1.),
 _=>(1.,1.-asrc)};
 let mut color=[0.;3];let alpha;
 if mode>=13 && mode<=27{
  let blended=blend_channels(d,s,mode);
  alpha=asrc+ad-asrc*ad;
  for i in 0..3{color[i]=(1.-ad)*asrc*s[i]+(1.-asrc)*ad*d[i]+asrc*ad*blended[i];}
 }else{
  alpha=(asrc*fa+ad*fb).min(1.);
  for i in 0..3{color[i]=(asrc*fa*s[i]+ad*fb*d[i]).min(1.);}
 }
 let mut packed=0u32;
 for (i,shift) in [24,16,8].iter().enumerate(){
  let c=if alpha>0.{color[i]/alpha}else{0.};
  packed|=((linear_to_srgb(c)*255.).round().clamp(0.,255.) as u32)<<shift;
 }
 packed|((alpha*255.).round().clamp(0.,255.) as u32)
}
fn composite(bg:&[Sprite],fg:&[Sprite],mode:u8)->io::Result<Option<Sprite>>{
 let background=Sprite::flatten(bg)?;let foreground=Sprite::flatten(fg)?;
 let images=[background.as_ref(),foreground.as_ref()].into_iter().flatten().cloned().collect::<Vec<_>>();
 let Some((left,top,w,h))=Sprite::bounds(&images)? else{return Ok(None)};
 let mut out=Sprite{x:left,y:top,w,h,pixels:vec![0;w*h]};
 for y in 0..h{for x in 0..w{
  let sample=|s:Option<&Sprite>|->u32{let Some(s)=s else{return 0};let px=left as i64+x as i64-s.x as i64;let py=top as i64+y as i64-s.y as i64;
   if px>=0&&py>=0&&px<s.w as i64&&py<s.h as i64{s.pixels[py as usize*s.w+px as usize]}else{0}};
  out.pixels[y*w+x]=composite_pixel(sample(background.as_ref()),sample(foreground.as_ref()),mode);
 }}Ok(Some(out))
}
/// Render a COLR v1 glyph, or return None if it has no BaseGlyphPaintRecord.
/// Variable paint deltas are resolved through COLR's ItemVariationStore and
/// DeltaSetIndexMap. Unsupported paint formats return InvalidData.
pub fn rasterize(face:&Face,glyph:u16,ppem:f32,palette:usize,foreground:u32)->io::Result<Option<ColorBitmap>>{
 let Some(colr)=face.table(*b"COLR")else{return Ok(None)};
 let Some(cpal)=face.table(*b"CPAL")else{return Ok(None)};
 if u16_at(colr,0)?!=1{return Ok(None);}
 if !ppem.is_finite()||ppem<=0.||ppem>4096.{return Err(bad("invalid COLR pixels per em"));}
 let root=u32_at(colr,14)?;let layer_list=u32_at(colr,18)?;
 let mut renderer=Renderer{face,colr,cpal,palette,foreground,scale:ppem/face.units_per_em as f32,root,layer_list,nodes:0,painted_pixels:0,active:Vec::new()};
 let Some(start)=renderer.base_paint(glyph)? else{return Ok(None)};
 let clip=renderer.clip_box(glyph)?;
 let sprites=renderer.render(start,Affine::identity(),0)?;
 let mut flattened=Sprite::flatten(&sprites)?;
 if let (Some(rect),Some(image))=(clip,flattened.as_mut()){
  let scale=renderer.scale;
  let (left,right)=((rect[0]*scale).floor() as i32,(rect[2]*scale).ceil() as i32);
  let (top,bottom)=(( -rect[3]*scale).floor() as i32,(-rect[1]*scale).ceil() as i32);
  for y in 0..image.h{for x in 0..image.w{
   let px=image.x.saturating_add(x as i32);let py=image.y.saturating_add(y as i32);
   if px<left||px>=right||py<top||py>=bottom{image.pixels[y*image.w+x]=0;}
  }}
 }
 let advance=face.advance_units_instance(glyph)*ppem/face.units_per_em as f32;
 Ok(Some(match flattened{
  Some(image)=>ColorBitmap{width:image.w as u32,height:image.h as u32,pixels:image.pixels,bearing_x:image.x,bearing_y:-image.y,advance_px:advance},
  None=>ColorBitmap{width:0,height:0,pixels:Vec::new(),bearing_x:0,bearing_y:0,advance_px:advance},
 }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn affine_roundtrip(){
        let a=Affine{xx:0.7,xy:-0.4,yx:0.3,yy:1.2,dx:12.,dy:-9.};
        let inverse=a.inverse().unwrap();
        let p=a.point(4.,8.);let back=inverse.point(p.0,p.1);
        assert!((back.0-4.).abs()<1e-4 && (back.1-8.).abs()<1e-4);
    }
    #[test] fn paint_alpha(){
        let c=Fill::Solid(0xff804080);
        assert_eq!(c.sample(7.,12.),0xff804080);
        assert_eq!(composite_pixel(0x00000000,0xff0000ff,3),0xff0000ff);
        assert_eq!(composite_pixel(0xff0000ff,0x00000000,3),0xff0000ff);
    }
    #[test] fn unknown_blend_mode_clears(){
        assert_eq!(composite_pixel(0xffff00ff,0x00ffffff,255),0);
    }
    #[test] fn gradient_modes(){
        let colors=ColorLine{extend:1,stops:vec![Stop{offset:0.,color:0xff0000ff},Stop{offset:1.,color:0x0000ffff}]};
        assert_eq!(colors.sample(0.2),colors.sample(1.2));
    }
}
