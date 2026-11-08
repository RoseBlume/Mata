//! Bounded, unhinted CFF2 Type 2 charstring outline decoder.
//!
//! CFF2 curves are sampled into the existing glyph-outline type (on-curve
//! polygon vertices). VariationStore region weights are applied at `blend`.
//! The source contains no FFI and does not execute embedded font programs.
use crate::glyph_variations::{GlyphOutline,OutlinePoint};
use crate::opentype::VariationAxis;
use crate::variations::normalized;
use std::collections::BTreeMap;
use std::io::{self,Error,ErrorKind};
fn bad(s:&str)->io::Error{Error::new(ErrorKind::InvalidData,s)}
fn u16be(b:&[u8],i:usize)->Option<u16>{let b=b.get(i..i.checked_add(2)?)?;Some(u16::from_be_bytes([b[0],b[1]]))}
fn u32be(b:&[u8],i:usize)->Option<u32>{let b=b.get(i..i.checked_add(4)?)?;Some(u32::from_be_bytes([b[0],b[1],b[2],b[3]]))}
fn i16be(b:&[u8],i:usize)->Option<i16>{u16be(b,i).map(|x|x as i16)}
fn fixed(b:&[u8],i:usize)->Option<f32>{i16be(b,i).map(|n|n as f32 /16384.0)}
fn index<'a>(b:&'a[u8],offset:usize)->io::Result<(Vec<&'a [u8]>,usize)>{
    let n=u32be(b,offset).ok_or_else(||bad("truncated CFF2 INDEX count"))? as usize;
    if n>131072{return Err(bad("CFF2 INDEX exceeds object limit"));}
    if n==0{return Ok((Vec::new(),offset+4));}
    let size=*b.get(offset+4).ok_or_else(||bad("truncated CFF2 INDEX header"))? as usize;
    if !(1..=4).contains(&size){return Err(bad("invalid CFF2 INDEX offSize"));}
    let start=offset.checked_add(5).ok_or_else(||bad("CFF2 INDEX overflow"))?;
    let mut offsets=Vec::with_capacity(n+1);
    for i in 0..=n{
        let at=start.checked_add(i.checked_mul(size).ok_or_else(||bad("CFF2 INDEX overflow"))?).ok_or_else(||bad("CFF2 INDEX overflow"))?;
        let mut v=0usize;
        for byte in b.get(at..at+size).ok_or_else(||bad("truncated CFF2 INDEX offsets"))?{v=(v<<8)|*byte as usize;}
        offsets.push(v);
    }
    if offsets[0]!=1||!offsets.windows(2).all(|v|v[0]<=v[1]){return Err(bad("invalid CFF2 INDEX offsets"));}
    let base=start+(n+1)*size;
    let end=base.checked_add(offsets[n].saturating_sub(1)).ok_or_else(||bad("CFF2 INDEX overflow"))?;
    if end>b.len(){return Err(bad("CFF2 INDEX exceeds table"));}
    let mut objects=Vec::with_capacity(n);
    for i in 0..n{objects.push(&b[base+offsets[i]-1..base+offsets[i+1]-1]);}
    Ok((objects,end))
}
fn number(bytes:&[u8],at:&mut usize,dict:bool)->Option<f32>{
    let b=*bytes.get(*at)?;*at+=1;
    match b{
        32..=246=>Some((b as i32-139) as f32),
        247..=250=>{let q=*bytes.get(*at)?;*at+=1;Some(((b as i32-247)*256+q as i32+108) as f32)},
        251..=254=>{let q=*bytes.get(*at)?;*at+=1;Some((-(b as i32-251)*256-q as i32-108) as f32)},
        28=>{let v=i16be(bytes,*at)?;*at+=2;Some(v as f32)},
        29 if dict=>{let v=u32be(bytes,*at)? as i32;*at+=4;Some(v as f32)},
        255=>{let v=u32be(bytes,*at)? as i32;*at+=4;Some(v as f32/65536.0)},
        30 if dict=>{
            let mut result=String::new();let mut done=false;
            for _ in 0..32{let v=*bytes.get(*at)?;*at+=1;
                for n in [v>>4,v&15]{match n{
                    0..=9=>result.push((b'0'+n) as char),10=>result.push('.'),11=>result.push('E'),
                    12=>result.push_str("E-"),14=>result.push('-'),15=>{done=true;break;},_=>return None,
                }}
                if done{break;}
            }
            result.parse().ok()
        },_=>None,
    }
}
fn dict_operands(bytes:&[u8])->io::Result<BTreeMap<u16,Vec<f32>>>{
    let mut at=0;let mut stack=Vec::new();let mut out=BTreeMap::new();
    while at<bytes.len(){let byte=bytes[at];
        if byte>=28||byte==30{
            stack.push(number(bytes,&mut at,true).ok_or_else(||bad("invalid CFF2 DICT operand"))?);
            if stack.len()>513{return Err(bad("CFF2 DICT stack limit exceeded"));}
        }else{
            at+=1;let op=if byte==12{let next=*bytes.get(at).ok_or_else(||bad("truncated CFF2 DICT operator"))?;at+=1;0x0c00|next as u16}else{byte as u16};
            out.insert(op,std::mem::take(&mut stack));
        }
    }
    Ok(out)
}
fn offset_from(dict:&BTreeMap<u16,Vec<f32>>,key:u16,index:usize)->Option<usize>{
    let value=*dict.get(&key)?.get(index)?;
    if value.is_finite()&&value>=0.0 && value<100_000_000.0{Some(value as usize)}else{None}
}
fn fd_index(table:&[u8],offset:usize,glyph:usize)->Option<usize>{
    let format=*table.get(offset)?;
    match format{
        0=>table.get(offset+1+glyph).map(|n|*n as usize),
        3=>{let n=u16be(table,offset+1)? as usize;
            if n>32768{return None;}
            let mut result=None;
            for i in 0..n{let at=offset+3+i*3;let first=u16be(table,at)? as usize;
                if first>glyph{break;}result=table.get(at+2).map(|n|*n as usize);
            }result
        },
        4=>{let n=u32be(table,offset+1)? as usize;
            if n>32768{return None;}
            let mut result=None;
            for i in 0..n{let at=offset+5+i*6;let first=u32be(table,at)? as usize;
                if first>glyph{break;}result=u16be(table,at+4).map(|n|n as usize);
            }result
        },_=>None,
    }
}
fn subr_bias(n:usize)->i32{if n<1240{107}else if n<33900{1131}else{32768}}
/// Scalar weights for the selected VariationStore ItemVariationData region list.
fn region_weights(store:&[u8],variation_index:usize,coords:&[f32])->io::Result<Vec<f32>>{
    let item=store.get(2..).ok_or_else(||bad("truncated CFF2 VariationStore"))?;
    if u16be(item,0)!=Some(1){return Err(bad("invalid CFF2 ItemVariationStore"));}
    let regions_offset=u32be(item,2).ok_or_else(||bad("missing variation regions"))? as usize;
    let sub_count=u16be(item,6).unwrap_or(0) as usize;
    if variation_index>=sub_count||sub_count>16384{return Err(bad("CFF2 vsindex out of bounds"));}
    let sub_off=u32be(item,8+variation_index*4).ok_or_else(||bad("truncated CFF2 variation table"))? as usize;
    let sub=item.get(sub_off..).ok_or_else(||bad("CFF2 variation offset invalid"))?;
    let n=u16be(sub,4).ok_or_else(||bad("truncated CFF2 region count"))? as usize;
    if n>32768{return Err(bad("excessive CFF2 regions"));}
    let regions=item.get(regions_offset..).ok_or_else(||bad("invalid CFF2 region list"))?;
    let axes=u16be(regions,0).unwrap_or(0) as usize;
    let total=u16be(regions,2).unwrap_or(0) as usize;
    if axes!=coords.len()||total>32768{return Err(bad("CFF2 variation axis mismatch"));}
    let mut weights=Vec::with_capacity(n);
    for i in 0..n{
        let index=u16be(sub,6+i*2).ok_or_else(||bad("truncated CFF2 region index"))? as usize;
        if index>=total{return Err(bad("CFF2 region index out of bounds"));}
        let base=4+index*axes*6;
        let mut scalar=1.0f32;
        for (axis,&value) in coords.iter().enumerate(){let at=base+axis*6;
            let (a,p,b)=(fixed(regions,at),fixed(regions,at+2),fixed(regions,at+4));
            let (Some(a),Some(p),Some(b))=(a,p,b)else{return Err(bad("truncated CFF2 region"));};
            if !(a<=p && p<=b){return Err(bad("invalid CFF2 region axis bounds"));}
            let factor=if p==0.0{1.0}else if value<a||value>b{0.0}else if value==p{1.0}
                else if value<p{(value-a)/(p-a).max(0.00001)}else{(b-value)/(b-p).max(0.00001)};
            scalar*=factor.clamp(0.0,1.0);
        }
        weights.push(scalar);
    }
    Ok(weights)
}
struct Interpreter<'a>{
    stack:Vec<f32>,x:f32,y:f32,outline:GlyphOutline,open:bool,stems:usize,
    gsubrs:Vec<&'a[u8]>,lsubrs:Vec<&'a[u8]>,vstore:Option<&'a[u8]>,vsindex:usize,coords:Vec<f32>,
    operations:usize,
}
impl Interpreter<'_>{
    fn point(&mut self,x:f32,y:f32){self.outline.points.push(OutlinePoint{x,y,on_curve:true});}
    fn move_to(&mut self,dx:f32,dy:f32){
        if self.open{self.close();}
        self.x+=dx;self.y+=dy;self.point(self.x,self.y);self.open=true;
    }
    fn close(&mut self){if self.open && !self.outline.points.is_empty(){self.outline.contours.push(self.outline.points.len()-1);self.open=false;}}
    fn line_to(&mut self,dx:f32,dy:f32){self.x+=dx;self.y+=dy;self.point(self.x,self.y);}
    fn curve(&mut self,dx1:f32,dy1:f32,dx2:f32,dy2:f32,dx3:f32,dy3:f32){
        let (a,b)=(self.x,self.y);let (c,d)=(a+dx1,b+dy1);
        let (e,f)=(c+dx2,d+dy2);self.x=e+dx3;self.y=f+dy3;
        let detail=(((self.x-a).abs()+(self.y-b).abs()+dx1.abs()+dy1.abs()+dx2.abs()+dy2.abs())/64.0).ceil().clamp(6.0,64.0) as usize;
        for i in 1..=detail{let t=i as f32/detail as f32;let q=1.0-t;
            self.point(q*q*q*a+3.0*q*q*t*c+3.0*q*t*t*e+t*t*t*self.x,
                q*q*q*b+3.0*q*q*t*d+3.0*q*t*t*f+t*t*t*self.y);
        }
    }
    fn evaluate(&mut self,bytes:&[u8],depth:usize)->io::Result<()>{
        if depth>16{return Err(bad("CFF2 subroutine recursion limit exceeded"));}
        let mut at=0;
        while at<bytes.len(){
            self.operations+=1;
            if self.operations>200_000||self.outline.points.len()>200_000{return Err(bad("CFF2 glyph complexity limit exceeded"));}
            let op=bytes[at];
            if op>=32 || op==28 || op==255{
                self.stack.push(number(bytes,&mut at,false).ok_or_else(||bad("invalid CFF2 operand"))?);
                if self.stack.len()>513{return Err(bad("CFF2 operand stack overflow"));}
                continue;
            }
            at+=1;
            match op{
                1|3|18|23=>{self.stems+=self.stack.len()/2;self.stack.clear();},
                19|20=>{self.stems+=self.stack.len()/2;self.stack.clear();
                    let skip=(self.stems+7)/8;at=at.checked_add(skip).ok_or_else(||bad("CFF2 mask overflow"))?;
                    if at>bytes.len(){return Err(bad("truncated CFF2 hintmask"));}
                },
                21=>{if self.stack.len()!=2{return Err(bad("invalid rmoveto stack"));}let s=std::mem::take(&mut self.stack);self.move_to(s[0],s[1]);},
                22|4=>{if self.stack.len()!=1{return Err(bad("invalid moveto stack"));}let v=self.stack.pop().unwrap();if op==22{self.move_to(v,0.0)}else{self.move_to(0.0,v)}},
                5=>{let s=std::mem::take(&mut self.stack);if s.len()%2!=0{return Err(bad("invalid rlineto operands"));}
                    for xy in s.chunks_exact(2){self.line_to(xy[0],xy[1]);}},
                6|7=>{let s=std::mem::take(&mut self.stack);let mut horizontal=op==6;
                    for v in s{if horizontal{self.line_to(v,0.0)}else{self.line_to(0.0,v)}horizontal=!horizontal;}},
                8=>{let s=std::mem::take(&mut self.stack);if s.len()%6!=0{return Err(bad("invalid rrcurveto operands"));}
                    for v in s.chunks_exact(6){self.curve(v[0],v[1],v[2],v[3],v[4],v[5]);}},
                24=>{let s=std::mem::take(&mut self.stack);if s.len()<8||(s.len()-2)%6!=0{return Err(bad("invalid rcurveline"));}
                    for v in s[..s.len()-2].chunks_exact(6){self.curve(v[0],v[1],v[2],v[3],v[4],v[5]);}
                    self.line_to(s[s.len()-2],s[s.len()-1]);},
                25=>{let s=std::mem::take(&mut self.stack);if s.len()<8||(s.len()-6)%2!=0{return Err(bad("invalid rlinecurve"));}
                    for v in s[..s.len()-6].chunks_exact(2){self.line_to(v[0],v[1]);}
                    let n=s.len();self.curve(s[n-6],s[n-5],s[n-4],s[n-3],s[n-2],s[n-1]);},
                26|27=>{let s=std::mem::take(&mut self.stack);let mut cursor=0;
                    let first=if s.len()%4==1{s[0]}else{0.0};if s.len()%4>1{return Err(bad("invalid vv/hh curve operands"));}
                    if s.len()%4==1{cursor=1;}
                    while cursor+4<=s.len(){let v=&s[cursor..cursor+4];
                        if op==26{self.curve(if cursor==1{first}else{0.0},v[0],v[1],v[2],0.0,v[3]);}
                        else{self.curve(v[0],if cursor==1{first}else{0.0},v[1],v[2],v[3],0.0);}
                        cursor+=4;
                    }
                },
                30|31=>{let s=std::mem::take(&mut self.stack);let mut cursor=0;let mut horizontal=op==31;
                    while cursor+4<=s.len(){let v=&s[cursor..cursor+4];let last=cursor+4==s.len();
                        let extra=if last&&s.len()%4==1{s.last().copied().unwrap_or(0.0)}else{0.0};
                        if horizontal{self.curve(v[0],0.0,v[1],v[2],extra,v[3]);}
                        else{self.curve(0.0,v[0],v[1],v[2],v[3],extra);}
                        horizontal=!horizontal;cursor+=4;
                    }
                    if cursor+usize::from(s.len()%4==1)!=s.len(){return Err(bad("invalid hv/vh curve operands"));}
                },
                10|29=>{let ix=self.stack.pop().ok_or_else(||bad("missing CFF2 subroutine operand"))? as i32;
                    let routines=if op==10{&self.lsubrs}else{&self.gsubrs};
                    let id=ix.checked_add(subr_bias(routines.len())).ok_or_else(||bad("CFF2 subroutine index overflow"))?;
                    if id<0{return Err(bad("invalid CFF2 subroutine index"));}
                    let routine=*routines.get(id as usize).ok_or_else(||bad("CFF2 subroutine index out of bounds"))?;
                    self.evaluate(routine,depth+1)?;
                },
                11=>return Ok(()),
                15=>{let n=self.stack.pop().ok_or_else(||bad("CFF2 vsindex operand missing"))?;
                    if n<0.0||n>16384.0{return Err(bad("CFF2 vsindex invalid"));}self.vsindex=n as usize;},
                16=>{let count=self.stack.pop().ok_or_else(||bad("CFF2 blend count missing"))?;
                    if !count.is_finite()||count<0.0||count>513.0||count.fract()!=0.0{
                        return Err(bad("CFF2 blend count invalid"));
                    }
                    let n=count as usize;
                    let weights=if let Some(store)=self.vstore{region_weights(store,self.vsindex,&self.coords)?}else{return Err(bad("CFF2 blend without variation store"));};
                    let consumed=n.checked_mul(weights.len()+1).ok_or_else(||bad("CFF2 blend overflow"))?;
                    if self.stack.len()<consumed{return Err(bad("CFF2 blend stack underflow"));}
                    let start=self.stack.len()-consumed;
                    for i in 0..n{let mut v=self.stack[start+i];
                        for (k,w) in weights.iter().enumerate(){v+=self.stack[start+n+k*n+i]*w;}
                        self.stack[start+i]=v;
                    }
                    self.stack.truncate(start+n);
                },
                12=>{let second=*bytes.get(at).ok_or_else(||bad("truncated CFF2 escape"))?;at+=1;
                    // Flex operators are needed by CFF2 outlines. Each emits
                    // two cubic segments, with depth/control omitted.
                    let s=std::mem::take(&mut self.stack);
                    match second{
                        35 if s.len()==13=>{self.curve(s[0],s[1],s[2],s[3],s[4],s[5]);self.curve(s[6],s[7],s[8],s[9],s[10],s[11]);},
                        34 if s.len()==7=>{self.curve(s[0],0.0,s[1],s[2],s[3],0.0);self.curve(s[4],0.0,s[5],-s[2],s[6],0.0);},
                        36 if s.len()==9=>{self.curve(s[0],s[1],s[2],s[3],s[4],0.0);
                            self.curve(s[5],0.0,s[6],s[7],s[8],-(s[1]+s[3]+s[7]));},
                        37 if s.len()==11=>{
                            let dx=s[0]+s[2]+s[4]+s[6]+s[8];let dy=s[1]+s[3]+s[5]+s[7]+s[9];
                            self.curve(s[0],s[1],s[2],s[3],s[4],s[5]);
                            self.curve(s[6],s[7],s[8],s[9],if dx.abs()>dy.abs(){s[10]}else{-dx},
                                if dx.abs()>dy.abs(){-dy}else{s[10]});
                        },_=>return Err(bad("unsupported CFF2 flex operator")),
                    }
                },
                _=>return Err(bad("unsupported CFF2 charstring operator")),
            }
        }
        Ok(())
    }
}
/// Decode a CFF2 glyph, applying blend deltas at the requested variable-font
/// coordinates. The output is a tessellated, unhinted outline.
pub(crate) fn outline(cff2:&[u8],glyph:u16,axes:&[VariationAxis],values:&BTreeMap<[u8;4],f32>,avar:Option<&[u8]>)->io::Result<GlyphOutline>{
    if cff2.get(..2)!=Some(&[2,0][..]){return Err(bad("invalid CFF2 version"));}
    let hdr=*cff2.get(2).ok_or_else(||bad("truncated CFF2 header"))? as usize;
    let top_len=u16be(cff2,3).ok_or_else(||bad("truncated CFF2 Top DICT length"))? as usize;
    let top=cff2.get(hdr..hdr.checked_add(top_len).ok_or_else(||bad("CFF2 offset overflow"))?).ok_or_else(||bad("truncated CFF2 Top DICT"))?;
    let dict=dict_operands(top)?;
    let charstrings=offset_from(&dict,17,0).ok_or_else(||bad("CFF2 CharStrings missing"))?;
    let (glyphs,_)=index(cff2,charstrings)?;
    let code=*glyphs.get(glyph as usize).ok_or_else(||bad("CFF2 glyph index out of bounds"))?;
    let (gsubrs,_) = index(cff2,hdr+top_len)?;
    let fd_array=offset_from(&dict,0x0c24,0).ok_or_else(||bad("CFF2 FDArray missing"))?;
    let (fd,_)=index(cff2,fd_array)?;
    let fd_num=if fd.len()==1{0}else{
        let sel=offset_from(&dict,0x0c25,0).ok_or_else(||bad("CFF2 FDSelect missing"))?;
        fd_index(cff2,sel,glyph as usize).ok_or_else(||bad("invalid CFF2 FDSelect"))?
    };
    let face=dict_operands(fd.get(fd_num).ok_or_else(||bad("CFF2 FD index out of bounds"))?)?;
    let private_size=offset_from(&face,18,0).ok_or_else(||bad("CFF2 Private DICT missing"))?;
    let private_at=offset_from(&face,18,1).ok_or_else(||bad("CFF2 Private DICT missing"))?;
    let private=cff2.get(private_at..private_at.checked_add(private_size).ok_or_else(||bad("CFF2 private overflow"))?).ok_or_else(||bad("truncated CFF2 private data"))?;
    let private_dict=dict_operands(private)?;
    let lsubrs=if let Some(offset)=offset_from(&private_dict,19,0){index(cff2,private_at+offset)?.0}else{Vec::new()};
    let vstore=offset_from(&dict,24,0).and_then(|at|cff2.get(at..));
    let vsindex=offset_from(&private_dict,22,0).unwrap_or(0);
    let mut vm=Interpreter{stack:Vec::new(),x:0.0,y:0.0,outline:GlyphOutline::default(),open:false,stems:0,
        gsubrs,lsubrs,vstore,vsindex,coords:normalized(axes,values,avar),operations:0};
    vm.evaluate(code,0)?;vm.close();
    Ok(vm.outline)
}
