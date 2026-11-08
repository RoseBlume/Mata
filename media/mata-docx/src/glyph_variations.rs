//! Native TrueType `glyf` outlines with OpenType `gvar` point interpolation.
//!
//! No rasterizer or native ABI is required. Coordinates are unhinted font
//! design units. Composite glyphs support XY-offset components and 2x2
//! transforms and point-to-point component attachment. Hinting remains separate.
use std::collections::BTreeMap;
use std::io::{self,Error,ErrorKind};
use crate::opentype::VariationAxis;
use crate::variations::normalized;

fn invalid(s:&str)->io::Error{Error::new(ErrorKind::InvalidData,s)}
fn u16be(data:&[u8],at:usize)->Option<u16>{let v=data.get(at..at.checked_add(2)?)?;Some(u16::from_be_bytes([v[0],v[1]]))}
fn i16be(data:&[u8],at:usize)->Option<i16>{u16be(data,at).map(|n|n as i16)}
fn u32be(data:&[u8],at:usize)->Option<u32>{let v=data.get(at..at.checked_add(4)?)?;Some(u32::from_be_bytes([v[0],v[1],v[2],v[3]]))}
fn f2(data:&[u8],at:usize)->Option<f32>{Some(i16be(data,at)? as f32/16384.0)}
fn read_tuple(data:&[u8],cursor:&mut usize,axes:usize)->Option<Vec<f32>>{
    let bytes=axes.checked_mul(2)?;
    data.get(*cursor..(*cursor).checked_add(bytes)?)?;
    let tuple=(0..axes).map(|i|f2(data,*cursor+i*2)).collect::<Option<Vec<_>>>()?;
    *cursor+=bytes;Some(tuple)
}

#[derive(Debug,Clone,Copy,PartialEq)]pub struct OutlinePoint{pub x:f32,pub y:f32,pub on_curve:bool}
#[derive(Debug,Clone,Default)]pub struct GlyphOutline{
    pub points:Vec<OutlinePoint>,
    /// Inclusive last point index of each contour.
    pub contours:Vec<usize>,
}
#[derive(Clone,Copy)]struct Component{id:u16,dx:f32,dy:f32,m:[f32;4],attach:Option<(usize,usize)>}

enum GlyphData{Simple(GlyphOutline),Composite(Vec<Component>)}
fn raw_glyph<'a>(head:&[u8],loca:&[u8],glyf:&'a [u8],glyph:u16)->io::Result<&'a [u8]>{
    let format=i16be(head,50).ok_or_else(||invalid("truncated head"))?;
    let index=glyph as usize;
    let (a,b)=match format{
        0=>(u16be(loca,index*2).map(|v|v as usize*2),u16be(loca,(index+1)*2).map(|v|v as usize*2)),
        1=>(u32be(loca,index*4).map(|v|v as usize),u32be(loca,(index+1)*4).map(|v|v as usize)),
        _=>return Err(invalid("unsupported loca format")),
    };
    let (a,b)=(a.ok_or_else(||invalid("loca truncated"))?,b.ok_or_else(||invalid("loca truncated"))?);
    if a>b{return Err(invalid("loca offsets not monotonic"));}
    glyf.get(a..b).ok_or_else(||invalid("glyf offset outside table"))
}
fn parse_glyph(glyph:&[u8])->io::Result<GlyphData>{
    if glyph.is_empty(){return Ok(GlyphData::Simple(GlyphOutline::default()));}
    let count=i16be(glyph,0).ok_or_else(||invalid("truncated glyf header"))?;
    if count<0{
        let mut components=Vec::new();let mut at=10;
        loop{
            let flags=u16be(glyph,at).ok_or_else(||invalid("truncated composite flags"))?;
            let id=u16be(glyph,at+2).ok_or_else(||invalid("truncated composite glyph id"))?;
            at+=4;
            // For XY arguments values are signed offsets. For point matching
            // arguments are unsigned indexes into the assembled parent and
            // unshifted component point arrays (OpenType glyf specification).
            let is_xy=flags&0x0002!=0;
            let (a,b)=if flags&1!=0{
                let a=u16be(glyph,at).ok_or_else(||invalid("composite args truncated"))?;
                let b=u16be(glyph,at+2).ok_or_else(||invalid("composite args truncated"))?;
                at+=4;(a,b)
            }else{
                let a=*glyph.get(at).ok_or_else(||invalid("composite args truncated"))? as u16;
                let b=*glyph.get(at+1).ok_or_else(||invalid("composite args truncated"))? as u16;
                at+=2;(a,b)
            };
            let attach=if is_xy{None}else{Some((a as usize,b as usize))};
            let (dx,dy)=if is_xy{
                if flags&1!=0{(a as i16 as f32,b as i16 as f32)}
                else{(a as u8 as i8 as f32,b as u8 as i8 as f32)}
            }else{(0.0,0.0)};
            let mut m=[1.,0.,0.,1.];
            if flags&8!=0{
                let s=f2(glyph,at).ok_or_else(||invalid("composite scale truncated"))?;
                m=[s,0.,0.,s];at+=2;
            }else if flags&64!=0{
                m[0]=f2(glyph,at).ok_or_else(||invalid("composite scale truncated"))?;
                m[3]=f2(glyph,at+2).ok_or_else(||invalid("composite scale truncated"))?;at+=4;
            }else if flags&128!=0{
                for (j,slot) in m.iter_mut().enumerate(){*slot=f2(glyph,at+2*j).ok_or_else(||invalid("composite matrix truncated"))?;}
                at+=8;
            }
            // Offset scaling in composite glyphs is explicit in modern fonts.
            // 0x0800: scaled offsets; 0x1000: unscaled offsets.
            let (dx,dy)=if is_xy && flags&0x0800!=0 && flags&0x1000==0{
                (m[0]*dx+m[1]*dy,m[2]*dx+m[3]*dy)
            }else{(dx,dy)};
            components.push(Component{id,dx,dy,m,attach});
            if components.len()>256{return Err(invalid("composite component limit exceeded"));}
            if flags&32==0{break;}
        }
        return Ok(GlyphData::Composite(components));
    }
    let count=count as usize;
    if count>8192{return Err(invalid("glyf contour limit exceeded"));}
    let mut ends=Vec::with_capacity(count);
    for i in 0..count{ends.push(u16be(glyph,10+i*2).ok_or_else(||invalid("glyf endpoints truncated"))? as usize);}
    if !ends.windows(2).all(|v|v[0]<v[1]){return Err(invalid("unordered contour endpoints"));}
    let points=ends.last().map(|n|n+1).unwrap_or(0);
    if points>65535{return Err(invalid("glyf point limit exceeded"));}
    let mut at=10+2*count;
    let instruction_bytes=u16be(glyph,at).ok_or_else(||invalid("glyf instructions truncated"))? as usize;
    at=at.checked_add(2+instruction_bytes).ok_or_else(||invalid("glyf size overflow"))?;
    if at>glyph.len(){return Err(invalid("glyf instructions outside table"));}
    let mut flags=Vec::with_capacity(points);
    while flags.len()<points{
        let flag=*glyph.get(at).ok_or_else(||invalid("glyf flags truncated"))?;at+=1;
        let repeat=if flag&8!=0{let n=*glyph.get(at).ok_or_else(||invalid("glyf flag repeat truncated"))? as usize;at+=1;n}else{0};
        if flags.len()+repeat+1>points{return Err(invalid("glyf flag repeat exceeds points"));}
        flags.extend(std::iter::repeat(flag).take(repeat+1));
    }
    let mut x=0i32;let mut xs=Vec::with_capacity(points);
    for flag in &flags{
        let delta=if flag&2!=0{
            let n=*glyph.get(at).ok_or_else(||invalid("glyf x bytes truncated"))? as i32;at+=1;
            if flag&16!=0{n}else{-n}
        }else if flag&16!=0{0}else{let n=i16be(glyph,at).ok_or_else(||invalid("glyf x words truncated"))? as i32;at+=2;n};
        x=x.checked_add(delta).ok_or_else(||invalid("glyf x coordinate overflow"))?;xs.push(x as f32);
    }
    let mut y=0i32;let mut out=Vec::with_capacity(points);
    for (i,flag) in flags.iter().enumerate(){
        let delta=if flag&4!=0{
            let n=*glyph.get(at).ok_or_else(||invalid("glyf y bytes truncated"))? as i32;at+=1;
            if flag&32!=0{n}else{-n}
        }else if flag&32!=0{0}else{let n=i16be(glyph,at).ok_or_else(||invalid("glyf y words truncated"))? as i32;at+=2;n};
        y=y.checked_add(delta).ok_or_else(||invalid("glyf y coordinate overflow"))?;
        out.push(OutlinePoint{x:xs[i],y:y as f32,on_curve:flag&1!=0});
    }
    Ok(GlyphData::Simple(GlyphOutline{points:out,contours:ends}))
}

fn packed_points(data:&[u8],at:&mut usize)->Option<Option<Vec<usize>>>{
    let first=*data.get(*at)?;*at+=1;
    if first==0{return Some(None);}
    let count=if first&0x80!=0{let b=*data.get(*at)?;*at+=1;(((first&0x7f) as usize)<<8)|b as usize}else{first as usize};
    if count>65535{return None;}
    let mut result=Vec::with_capacity(count);let mut previous=0usize;
    while result.len()<count{
        let control=*data.get(*at)?;*at+=1;
        let run=(control as usize&0x7f)+1;
        if result.len()+run>count{return None;}
        for _ in 0..run{
            let delta=if control&0x80!=0{let v=u16be(data,*at)? as usize;*at+=2;v}
                else{let v=*data.get(*at)? as usize;*at+=1;v};
            previous=previous.checked_add(delta)?;
            result.push(previous);
        }
    }
    Some(Some(result))
}
fn packed_deltas(data:&[u8],at:&mut usize,count:usize)->Option<Vec<f32>>{
    let mut out=Vec::with_capacity(count);
    while out.len()<count{
        let control=*data.get(*at)?;*at+=1;
        let run=(control as usize&0x3f)+1;
        if out.len()+run>count{return None;}
        for _ in 0..run{
            let v=if control&0x80!=0{0.0}
            else if control&0x40!=0{let n=i16be(data,*at)? as f32;*at+=2;n}
            else{let n=*data.get(*at)? as i8 as f32;*at+=1;n};
            out.push(v);
        }
    }
    Some(out)
}

fn tuple_scalar(coords:&[f32],peak:&[f32],region:Option<(&[f32],&[f32])>)->f32{
    let mut scalar=1.0f32;
    for (i,&value) in coords.iter().enumerate(){
        let p=peak[i];
        if p==0.0{continue;}
        let (start,end)=region.map(|(s,e)|(s[i],e[i])).unwrap_or((p.min(0.),p.max(0.)));
        if start>p || p>end || value<start || value>end || value*p<=0.0{return 0.0;}
        if value<p{scalar*=if p==start{1.0}else{(value-start)/(p-start)};}
        else if value>p{scalar*=if p==end{1.0}else{(end-value)/(end-p)};}
    }
    scalar.max(0.0)
}
fn interpolated_axis(coords:&[f32],touch:&[Option<f32>],ends:&[usize])->Vec<f32>{
    let mut values=touch.iter().map(|v|v.unwrap_or(0.0)).collect::<Vec<_>>();
    let mut first=0;
    for &last in ends{
        if last<first || last>=touch.len(){break;}
        let marked=(first..=last).filter(|i|touch[*i].is_some()).collect::<Vec<_>>();
        if marked.len()==1{for v in &mut values[first..=last]{*v=touch[marked[0]].unwrap_or(0.0);}}
        else if marked.len()>1{
            for i in 0..marked.len(){
                let a=marked[i];let b=marked[(i+1)%marked.len()];
                let xa=coords[a];let xb=coords[b];let da=touch[a].unwrap_or(0.0);let db=touch[b].unwrap_or(0.0);
                let mut j=if a==last{first}else{a+1};
                while j!=b{
                    let x=coords[j];
                    values[j]=if xa==xb{if da==db{da}else{0.0}}
                    else if xa<xb{
                        if x<=xa{da}else if x>=xb{db}else{da+(db-da)*(x-xa)/(xb-xa)}
                    }else if x<=xb{db}else if x>=xa{da}else{db+(da-db)*(x-xb)/(xa-xb)};
                    j=if j==last{first}else{j+1};
                }
            }
        }
        first=last+1;
    }
    values
}

fn gvar_deltas(gvar:&[u8],glyph:u16,axes:&[VariationAxis],values:&BTreeMap<[u8;4],f32>,avar:Option<&[u8]>,
               point_count:usize,base_xy:&[(f32,f32)],contours:&[usize])->io::Result<Vec<(f32,f32)>>{
    let mut result=vec![(0.0,0.0);point_count];
    let axis_count=u16be(gvar,4).ok_or_else(||invalid("gvar truncated"))? as usize;
    let shared_count=u16be(gvar,6).ok_or_else(||invalid("gvar truncated"))? as usize;
    let shared_at=u32be(gvar,8).ok_or_else(||invalid("gvar truncated"))? as usize;
    let glyph_count=u16be(gvar,12).ok_or_else(||invalid("gvar truncated"))? as usize;
    let flags=u16be(gvar,14).ok_or_else(||invalid("gvar truncated"))?;
    let base=u32be(gvar,16).ok_or_else(||invalid("gvar truncated"))? as usize;
    if u16be(gvar,0)!=Some(1)||axis_count!=axes.len()||axis_count>64||glyph as usize>=glyph_count||shared_count>4096{return Err(invalid("invalid gvar header"));}
    let idx=glyph as usize;
    let short=flags&1==0;
    let (start,end)=if short{
        (u16be(gvar,20+idx*2).map(|x|x as usize*2),u16be(gvar,22+idx*2).map(|x|x as usize*2))
    }else{(u32be(gvar,20+idx*4).map(|x|x as usize),u32be(gvar,24+idx*4).map(|x|x as usize))};
    let (start,end)=(start.ok_or_else(||invalid("gvar offsets truncated"))?,end.ok_or_else(||invalid("gvar offsets truncated"))?);
    if end<start{return Err(invalid("gvar offsets reversed"));}
    if start==end{return Ok(result);}
    let glyphdata=gvar.get(base.checked_add(start).ok_or_else(||invalid("gvar size overflow"))?..base.checked_add(end).ok_or_else(||invalid("gvar size overflow"))?).ok_or_else(||invalid("gvar glyph variation outside table"))?;
    let tuple_count=u16be(glyphdata,0).ok_or_else(||invalid("gvar tuple header truncated"))?;
    let n=(tuple_count&0x0fff) as usize;
    let data_offset=u16be(glyphdata,2).ok_or_else(||invalid("gvar data offset truncated"))? as usize;
    let coords=normalized(axes,values,avar);
    let mut descriptors=Vec::new();let mut cursor=4usize;
    for _ in 0..n{
        let size=u16be(glyphdata,cursor).ok_or_else(||invalid("tuple size truncated"))? as usize;
        let index=u16be(glyphdata,cursor+2).ok_or_else(||invalid("tuple index truncated"))?;cursor+=4;
        let peak=if index&0x8000!=0{read_tuple(glyphdata,&mut cursor,axis_count)}else{
            let tuple_idx=(index&0x0fff) as usize;
            if tuple_idx>=shared_count{None}else{
                let mut off=shared_at+tuple_idx*axis_count*2;
                read_tuple(gvar,&mut off,axis_count)
            }
        }.ok_or_else(||invalid("tuple peak outside table"))?;
        let intermediate=if index&0x4000!=0{
            let first=read_tuple(glyphdata,&mut cursor,axis_count).ok_or_else(||invalid("tuple start truncated"))?;
            let last=read_tuple(glyphdata,&mut cursor,axis_count).ok_or_else(||invalid("tuple end truncated"))?;
            Some((first,last))
        }else{None};
        descriptors.push((size,index,peak,intermediate));
    }
    if cursor>data_offset{return Err(invalid("tuple headers overlap data"));}
    let mut data_cursor=data_offset;
    let shared_points=if tuple_count&0x8000!=0{
        packed_points(glyphdata,&mut data_cursor).ok_or_else(||invalid("invalid gvar shared point numbers"))?
    }else{None};
    for (size,index,peak,region) in descriptors{
        let tuple_end=data_cursor.checked_add(size).ok_or_else(||invalid("tuple size overflow"))?;
        let tuple=glyphdata.get(data_cursor..tuple_end).ok_or_else(||invalid("tuple data outside gvar"))?;
        data_cursor=tuple_end;
        let scalar=tuple_scalar(&coords,&peak,region.as_ref().map(|(s,e)|(s.as_slice(),e.as_slice())));
        if scalar==0.0{continue;}
        let mut at=0;
        let points=if index&0x2000!=0{packed_points(tuple,&mut at).ok_or_else(||invalid("private gvar points malformed"))?}
            else{shared_points.clone()};
        let count=points.as_ref().map_or(point_count,Vec::len);
        if count>point_count{return Err(invalid("gvar point count exceeds glyph"));}
        let x=packed_deltas(tuple,&mut at,count).ok_or_else(||invalid("gvar X deltas malformed"))?;
        let y=packed_deltas(tuple,&mut at,count).ok_or_else(||invalid("gvar Y deltas malformed"))?;
        let mut sparse_x=vec![None;point_count];let mut sparse_y=vec![None;point_count];
        for k in 0..count{
            let p=points.as_ref().map_or(k,|v|v[k]);
            if p>=point_count{return Err(invalid("gvar point index exceeds glyph"));}
            // Duplicate point indexes can occur and add cumulatively.
            sparse_x[p]=Some(sparse_x[p].unwrap_or(0.0)+x[k]);
            sparse_y[p]=Some(sparse_y[p].unwrap_or(0.0)+y[k]);
        }
        let (interp_x,interp_y)=if contours.is_empty(){
            (sparse_x.iter().map(|v|v.unwrap_or(0.0)).collect::<Vec<_>>(),sparse_y.iter().map(|v|v.unwrap_or(0.0)).collect::<Vec<_>>())
        }else{
            (interpolated_axis(&base_xy.iter().map(|p|p.0).collect::<Vec<_>>(),&sparse_x,contours),
             interpolated_axis(&base_xy.iter().map(|p|p.1).collect::<Vec<_>>(),&sparse_y,contours))
        };
        for j in 0..point_count{result[j].0+=interp_x[j]*scalar;result[j].1+=interp_y[j]*scalar;}
    }
    Ok(result)
}

/// Decode the instance outline, including composite component transforms and
/// `gvar` sparse point interpolation. No TrueType instruction hinting occurs.
pub(crate) fn outline(head:&[u8],loca:&[u8],glyf:&[u8],gvar:Option<&[u8]>,
    glyph:u16,axes:&[VariationAxis],values:&BTreeMap<[u8;4],f32>,avar:Option<&[u8]>) -> io::Result<GlyphOutline>{
    fn recurse(head:&[u8],loca:&[u8],glyf:&[u8],gvar:Option<&[u8]>,glyph:u16,axes:&[VariationAxis],
        values:&BTreeMap<[u8;4],f32>,avar:Option<&[u8]>,depth:usize)->io::Result<GlyphOutline>{
        if depth>12{return Err(invalid("composite glyph recursion limit exceeded"));}
        match parse_glyph(raw_glyph(head,loca,glyf,glyph)?)?{
            GlyphData::Simple(mut o)=>{
                if !values.is_empty(){if let Some(data)=gvar{
                    let points=o.points.len()+4;
                    let coords=o.points.iter().map(|p|(p.x,p.y)).chain(std::iter::repeat((0.,0.)).take(4)).collect::<Vec<_>>();
                    let delta=gvar_deltas(data,glyph,axes,values,avar,points,&coords,&o.contours)?;
                    for (p,(dx,dy)) in o.points.iter_mut().zip(delta){p.x+=dx;p.y+=dy;}
                }}
                Ok(o)
            },
            GlyphData::Composite(mut comps)=>{
                if !values.is_empty(){if let Some(data)=gvar{
                    let coords=comps.iter().map(|c|(c.dx,c.dy)).chain(std::iter::repeat((0.,0.)).take(4)).collect::<Vec<_>>();
                    let delta=gvar_deltas(data,glyph,axes,values,avar,comps.len()+4,&coords,&[])?;
                    for (c,(dx,dy)) in comps.iter_mut().zip(delta){c.dx+=dx;c.dy+=dy;}
                }}
                let mut whole=GlyphOutline::default();
                for c in comps{
                    let mut part=recurse(head,loca,glyf,gvar,c.id,axes,values,avar,depth+1)?;
                    for p in &mut part.points{
                        let (x,y)=(p.x,p.y);
                        p.x=c.m[0]*x+c.m[1]*y;
                        p.y=c.m[2]*x+c.m[3]*y;
                    }
                    let (shift_x,shift_y)=if let Some((parent,child))=c.attach{
                        let anchor=whole.points.get(parent).ok_or_else(||invalid("invalid parent composite anchor point"))?;
                        let moved=part.points.get(child).ok_or_else(||invalid("invalid child composite anchor point"))?;
                        (anchor.x-moved.x+c.dx,anchor.y-moved.y+c.dy)
                    }else{(c.dx,c.dy)};
                    for p in &mut part.points{p.x+=shift_x;p.y+=shift_y;}
                    let offset=whole.points.len();
                    if offset+part.points.len()>65535{return Err(invalid("composite point limit exceeded"));}
                    whole.contours.extend(part.contours.into_iter().map(|v|v+offset));
                    whole.points.extend(part.points);
                }
                Ok(whole)
            },
        }
    }
    recurse(head,loca,glyf,gvar,glyph,axes,values,avar,0)
}

/// Rasterized, unhinted glyph in rrggbbaa order. The bitmap is scaled from
/// the `gvar`-interpolated outline; it does not depend on native rasterizers.
#[derive(Debug,Clone)]pub struct OutlineBitmap{
    pub width:u32,pub height:u32,pub pixels:Vec<u32>,
    /// Left edge relative to glyph origin, and top edge above baseline.
    pub bearing_x:i32,pub bearing_y:i32,pub advance_px:f32,
}
impl GlyphOutline{
    /// Flatten each contour's TrueType quadratic off-curve points to line
    /// segments in pixel coordinates. Handles implicit on-curve midpoints.
    fn flattened(&self,scale:f32)->io::Result<Vec<Vec<(f32,f32)>>>{
        let mut result=Vec::new();let mut first=0usize;
        for &last in &self.contours{
            if last>=self.points.len()||first>last{return Err(invalid("invalid quadratic contour endpoints"));}
            let source=&self.points[first..=last];first=last+1;
            if source.is_empty(){continue;}
            let point=|p:OutlinePoint|(p.x*scale,-p.y*scale);
            let midpoint=|a:(f32,f32),b:(f32,f32)|((a.0+b.0)/2.0,(a.1+b.1)/2.0);
            let first_on=source[0].on_curve;
            let start=if first_on{point(source[0])}
                else if source.last().map(|p|p.on_curve).unwrap_or(false){point(*source.last().unwrap())}
                else{midpoint(point(source[0]),point(*source.last().unwrap()))};
            let mut path=vec![start];let mut current=start;let mut pending=None;
            let mut index=if first_on{1}else{0};
            let steps=source.len();
            for _ in 0..steps{
                let p=source[index%source.len()];index+=1;
                let next=point(p);
                if p.on_curve{
                    if let Some(control)=pending.take(){
                        flatten_quadratic(current,control,next,&mut path);
                    }else{path.push(next);}
                    current=next;
                }else if let Some(control)=pending.replace(next){
                    let mid=midpoint(control,next);
                    flatten_quadratic(current,control,mid,&mut path);
                    current=mid;
                }
            }
            if let Some(control)=pending{flatten_quadratic(current,control,start,&mut path);}
            else{path.push(start);}
            if path.len()>2{result.push(path);}
        }
        Ok(result)
    }
    /// Add conservative TrueType scan-converter dropout coverage to a hinted
    /// outline. This supports the simple- and smart-include-stubs modes, not
    /// the platform-specific excluded-stub scan conversion variants.
    /// No native rasterizer or FFI is used. The glyph is already constrained
    /// by the ordinary rasterizer's dimension and work limits.
    pub fn rasterize_with_dropout(&self,pixels_per_em:f32,units_per_em:u16,
            advance_units:f32,scan_type:u16)->io::Result<OutlineBitmap>{
        let mut bitmap=self.rasterize(pixels_per_em,units_per_em,advance_units)?;
        if !matches!(scan_type,0|4) || bitmap.width==0||bitmap.height==0 {
            return Ok(bitmap);
        }
        let paths=self.flattened(pixels_per_em/units_per_em as f32)?;
        let segments=paths.iter().map(|p|p.len()).sum::<usize>();
        if (bitmap.width as usize).saturating_mul(bitmap.height as usize)
            .saturating_mul(segments)>50_000_000{
            return Err(invalid("TrueType dropout scan work limit"));
        }
        let (width,height)=(bitmap.width as usize,bitmap.height as usize);
        let (left,top)=(bitmap.bearing_x as f32,-bitmap.bearing_y as f32);
        // A half-open row/column crossing with signed winding transitions.
        // Only subpixel filled intervals are considered and no existing
        // antialiased coverage is overwritten.
        for horizontal in [true,false]{
            let rows=if horizontal{height}else{width};
            let cross_extent=if horizontal{width}else{height};
            for row in 0..rows{
                let scan=if horizontal{top+row as f32+0.5}else{left+row as f32+0.5};
                let mut crossings:Vec<(f32,i32)>=Vec::new();
                for path in &paths{for edge in path.windows(2){
                    let(a,b)=(edge[0],edge[1]);
                    let (a0,a1,b0,b1)=if horizontal{(a.0,a.1,b.0,b.1)}
                        else{(a.1,a.0,b.1,b.0)};
                    if (a1<=scan&&b1>scan)||(b1<=scan&&a1>scan){
                        let t=(scan-a1)/(b1-a1);
                        let cross=a0+t*(b0-a0);
                        if cross.is_finite(){crossings.push((cross,if b1>a1{1}else{-1}));}
                    }
                }}
                if crossings.len()>65_536{return Err(invalid("TrueType scan crossing limit"));}
                crossings.sort_by(|a,b|a.0.total_cmp(&b.0));
                let mut winding=0i32;
                for pair in crossings.windows(2){
                    winding+=pair[0].1;
                    if winding==0{continue;}
                    let (begin,end)=(pair[0].0,pair[1].0);
                    if end<=begin||end-begin>=1.0{continue;}
                    let local=if horizontal{begin-left}else{begin-top};
                    let index=if scan_type==0{local.floor()}else{((begin+end)*0.5
                        -if horizontal{left}else{top}-0.5).round()};
                    if index<0.0||index>=cross_extent as f32{continue;}
                    let ix=index as usize;
                    let pos=if horizontal{row*width+ix}else{ix*width+row};
                    if bitmap.pixels[pos]==0 {bitmap.pixels[pos]=255;}
                }
            }
        }
        Ok(bitmap)
    }
    /// Build an antialiased software bitmap at a requested pixel em size.
    /// Curves use adaptive quadratic subdivision; coverage uses four samples
    /// per pixel and the TrueType non-zero winding fill rule.
    pub fn rasterize(&self,pixels_per_em:f32,units_per_em:u16,advance_units:f32)->io::Result<OutlineBitmap>{
        if !pixels_per_em.is_finite()||pixels_per_em<=0.0||pixels_per_em>4096.0||units_per_em==0{
            return Err(invalid("invalid font raster scale"));
        }
        let scale=pixels_per_em/units_per_em as f32;
        let paths=self.flattened(scale)?;
        let advance_px=advance_units*scale;
        if paths.is_empty(){return Ok(OutlineBitmap{width:0,height:0,pixels:Vec::new(),bearing_x:0,bearing_y:0,advance_px});}
        let (mut min_x,mut max_x,mut min_y,mut max_y)=(f32::MAX,f32::MIN,f32::MAX,f32::MIN);
        for path in &paths{for &(x,y) in path{
            min_x=min_x.min(x);max_x=max_x.max(x);min_y=min_y.min(y);max_y=max_y.max(y);
        }}
        let left=min_x.floor() as i32-1;let right=max_x.ceil() as i32+1;
        let top=min_y.floor() as i32-1;let bottom=max_y.ceil() as i32+1;
        let width=right.saturating_sub(left) as usize;
        let height=bottom.saturating_sub(top) as usize;
        if width>4096||height>4096||width.checked_mul(height).map(|n|n>16_777_216).unwrap_or(true){
            return Err(invalid("interpolated glyph bitmap limit exceeded"));
        }
        let segments=paths.iter().map(|p|p.len()).sum::<usize>();
        if width.checked_mul(height).and_then(|n|n.checked_mul(segments))
            .map(|work|work>50_000_000).unwrap_or(true){
            return Err(invalid("interpolated glyph raster work limit exceeded"));
        }
        let mut pixels=vec![0u32;width*height];
        for y in 0..height {for x in 0..width{
            let mut covered=0u32;
            for (dx,dy) in [(0.25,0.25),(0.75,0.25),(0.25,0.75),(0.75,0.75)]{
                let (px,py)=(left as f32+x as f32+dx,top as f32+y as f32+dy);
                let mut winding=0i32;
                for path in &paths{for edge in path.windows(2){
                    let ((x0,y0),(x1,y1))=(edge[0],edge[1]);
                    if y0<=py && y1>py{
                        if (x1-x0)*(py-y0)-(px-x0)*(y1-y0)>0.0{winding+=1;}
                    }else if y1<=py && y0>py{
                        if (x1-x0)*(py-y0)-(px-x0)*(y1-y0)<0.0{winding-=1;}
                    }
                }}
                if winding!=0{covered+=1;}
            }
            pixels[y*width+x]=(covered*255/4) as u32;
        }}
        Ok(OutlineBitmap{width:width as u32,height:height as u32,pixels,
            bearing_x:left,bearing_y:-top,advance_px})
    }
}
fn flatten_quadratic(a:(f32,f32),control:(f32,f32),b:(f32,f32),path:&mut Vec<(f32,f32)>){
    // Bezier deviation from the chord controls sample density.
    let deviation=((control.0-(a.0+b.0)*0.5).powi(2)+(control.1-(a.1+b.1)*0.5).powi(2)).sqrt();
    let steps=(deviation*2.0).sqrt().ceil().clamp(2.0,64.0) as usize;
    for i in 1..=steps{
        let t=i as f32/steps as f32;let m=1.0-t;
        path.push((m*m*a.0+2.0*m*t*control.0+t*t*b.0,
                   m*m*a.1+2.0*m*t*control.1+t*t*b.1));
    }
}
