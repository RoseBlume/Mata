//! Pure-Rust TrueType bytecode execution for `glyf` outlines.
//!
//! All coordinates in the interpreter are 26.6 pixel units, represented by
//! `f32` to retain subpixel projection-vector movements. The public outline
//! converts them back to font design units for the shared rasterizer.
//!
//! Unrecognized instructions fail closed (`Unsupported`) instead of silently
//! producing a falsely hinted glyph. Execution, storage, stack, recursion and
//! point counts are bounded for untrusted font files.
use std::collections::BTreeMap;
use std::io::{self, Error, ErrorKind};
use crate::glyph_variations::{GlyphOutline, OutlinePoint};

fn bad(s: &'static str)->io::Error { Error::new(ErrorKind::InvalidData,s) }
fn unsupported(op:u8)->io::Error { Error::new(ErrorKind::Unsupported,format!("unsupported TrueType opcode 0x{op:02x}")) }
fn u16be(b:&[u8],at:usize)->Option<u16>{let b=b.get(at..at.checked_add(2)?)?;Some(u16::from_be_bytes([b[0],b[1]]))}
fn u32be(b:&[u8],at:usize)->Option<u32>{let b=b.get(at..at.checked_add(4)?)?;Some(u32::from_be_bytes([b[0],b[1],b[2],b[3]]))}

fn next_code(code:&[u8],pc:usize)->io::Result<usize>{
    let op=*code.get(pc).ok_or_else(||bad("TrueType instruction pointer out of bounds"))?;
    let begin=pc+1;
    let count=match op{
        0x40|0x41=>{let n=*code.get(begin).ok_or_else(||bad("truncated NPUSH"))? as usize;
            return begin.checked_add(1+n*(if op==0x41{2}else{1})).filter(|e|*e<=code.len()).ok_or_else(||bad("truncated NPUSH data"));},
        0xB0..=0xB7=>(op-0xB0+1) as usize,
        0xB8..=0xBF=>2*(op-0xB8+1) as usize,
        _=>0
    };
    begin.checked_add(count).filter(|e|*e<=code.len()).ok_or_else(||bad("truncated TrueType PUSH"))
}
fn push_bytes(code:&[u8],pc:usize, stack:&mut Vec<i32>)->io::Result<usize>{
    let op=code[pc];let begin=pc+1;
    let (at,n,wide)=match op{
        0x40|0x41=>(begin+1,*code.get(begin).ok_or_else(||bad("truncated NPUSH"))? as usize,op==0x41),
        0xB0..=0xB7=>(begin,(op-0xB0+1) as usize,false),
        0xB8..=0xBF=>(begin,(op-0xB8+1) as usize,true),
        _=>return Err(bad("not a push opcode")),
    };
    if n>255||stack.len().saturating_add(n)>4096{return Err(bad("TrueType stack limit exceeded"));}
    let end=at.checked_add(n*(if wide{2}else{1})).ok_or_else(||bad("TrueType push overflow"))?;
    let source=code.get(at..end).ok_or_else(||bad("truncated TrueType push operands"))?;
    if wide{for word in source.chunks_exact(2){stack.push(i16::from_be_bytes([word[0],word[1]]) as i32);}}
    else{for &value in source{stack.push(i32::from(value));}}
    Ok(end)
}
#[derive(Debug,Clone,Copy,Default)]struct Vec2{x:f32,y:f32}
impl Vec2{
    fn dot(self, other:Self)->f32 {self.x*other.x+self.y*other.y}
    fn normalized(self)->io::Result<Self>{let n=(self.x*self.x+self.y*self.y).sqrt();
        if !n.is_finite()||n<1e-6{Err(bad("zero TrueType vector"))}else{Ok(Self{x:self.x/n,y:self.y/n})}}
}
#[derive(Debug,Clone)]struct Point{x:f32,y:f32,touch_x:bool,touch_y:bool,on_curve:bool}
impl Point{fn xy(&self)->Vec2{Vec2{x:self.x,y:self.y}}}
#[derive(Debug,Clone,Copy)]enum Round{Grid,Half,Double,Up,Down,Off,Super(u8,f32)}
#[derive(Debug,Clone)]struct State{
    projection:Vec2,freedom:Vec2,dual:Vec2,zp:[usize;3],rp:[usize;3],loop_value:usize,
    round:Round,minimum:f32,cut_in:f32,single_width:f32,single_width_cut_in:f32,delta_base:i32,delta_shift:i32,
    auto_flip:bool,disable_grid_fit:bool,ignore_prep_settings:bool,
    scan_control:u16,scan_type:u16,
}
impl Default for State{
    fn default()->Self{let x=Vec2{x:1.0,y:0.0};Self{projection:x,freedom:x,dual:x,
        zp:[1,1,1],rp:[0;3],loop_value:1,round:Round::Grid,minimum:1.0,
        cut_in:17.0/16.0,single_width:0.0,single_width_cut_in:0.0,delta_base:9,delta_shift:3,
        auto_flip:true,disable_grid_fit:false,ignore_prep_settings:false,scan_control:0,scan_type:2}}
}
/// Resource caps for font-supplied bytecode.
#[derive(Debug,Clone,Copy)]pub struct HintLimits{
    pub max_instructions:usize,pub max_stack:usize,pub max_call_depth:usize,
    pub max_storage:usize,pub max_twilight_points:usize,
}
impl Default for HintLimits{
    fn default()->Self{Self{max_instructions:200_000,max_stack:4096,max_call_depth:32,
        max_storage:65_536,max_twilight_points:4096}}
}
struct Vm{
    state:State,stack:Vec<i32>,storage:Vec<i32>,cvt:Vec<f32>,
    zone:[Vec<Point>;2],original:[Vec<Point>;2],contours:Vec<usize>,
    functions:BTreeMap<i32,Vec<u8>>,definitions:BTreeMap<u8,Vec<u8>>,
    limits:HintLimits,remaining:usize,ppem:f32,scale:f32,
}
impl Vm{
    fn new(cvt:&[u8],maxp:Option<&[u8]>,ppem:f32,scale:f32,limits:HintLimits)->io::Result<Self>{
        if cvt.len()%2!=0||cvt.len()/2>limits.max_storage{return Err(bad("invalid TrueType CVT"));}
        let max_storage=maxp.and_then(|x|u16be(x,18)).unwrap_or(1024) as usize;
        let max_twilight=maxp.and_then(|x|u16be(x,16)).unwrap_or(0) as usize;
        if max_storage>limits.max_storage||max_twilight>limits.max_twilight_points{
            return Err(bad("TrueType maxp exceeds interpreter resource limit"));
        }
        let twilight=vec![Point{x:0.,y:0.,touch_x:false,touch_y:false,on_curve:true};max_twilight];
        Ok(Self{state:State::default(),stack:Vec::new(),storage:vec![0;max_storage],
            cvt:cvt.chunks_exact(2).map(|b|i16::from_be_bytes([b[0],b[1]]) as f32*scale).collect(),
            zone:[twilight.clone(),Vec::new()],original:[twilight,Vec::new()],contours:Vec::new(),
            functions:BTreeMap::new(),definitions:BTreeMap::new(),limits,
            remaining:limits.max_instructions,ppem,scale})
    }
    fn pop(&mut self)->io::Result<i32>{self.stack.pop().ok_or_else(||bad("TrueType operand stack underflow"))}
    fn idx(&mut self)->io::Result<usize>{usize::try_from(self.pop()?).map_err(|_|bad("negative TrueType index"))}
    fn push(&mut self,n:i32)->io::Result<()>{if self.stack.len()>=self.limits.max_stack{return Err(bad("TrueType stack overflow"));}self.stack.push(n);Ok(())}
    fn coordinate(&self,zone:usize,index:usize,original:bool)->io::Result<Vec2>{
        let zones=if original{&self.original}else{&self.zone};
        zones.get(zone).and_then(|v|v.get(index)).map(|p|p.xy()).ok_or_else(||bad("TrueType point index outside zone"))
    }
    fn projected(&self,zone:usize,index:usize,original:bool)->io::Result<f32>{
        Ok(self.coordinate(zone,index,original)?.dot(if original{self.state.dual}else{self.state.projection}))
    }
    fn move_point(&mut self,zone:usize,index:usize,target:f32)->io::Result<()>{
        let old=self.projected(zone,index,false)?;
        let v=self.state.freedom;let dot=v.dot(self.state.projection);
        if dot.abs()<1e-6{return Err(bad("perpendicular freedom and projection vectors"));}
        let delta=(target-old)/dot;
        let p=self.zone.get_mut(zone).and_then(|z|z.get_mut(index)).ok_or_else(||bad("TrueType point outside zone"))?;
        p.x+=delta*v.x;p.y+=delta*v.y;
        if v.x.abs()>1e-6{p.touch_x=true;}if v.y.abs()>1e-6{p.touch_y=true;}
        if !p.x.is_finite()||!p.y.is_finite()||p.x.abs()>1_000_000.||p.y.abs()>1_000_000.{return Err(bad("TrueType point displacement overflow"));}
        Ok(())
    }
    fn round(&self,x:f32)->f32{
        match self.state.round{
            Round::Grid=>x.round(),Round::Half=>(x-0.5).round()+0.5,
            Round::Double=>(x*2.0).round()/2.0,Round::Up=>x.ceil(),
            Round::Down=>x.floor(),Round::Off=>x,
            Round::Super(n,grid)=>{
                let period=match (n>>6)&3{0=>grid*0.5,1=>grid,2=>grid*2.0,_=>grid};
                let phase=grid*((n>>4)&3) as f32/4.0;
                let threshold=if n&15==0 {period-1.0/64.0}
                              else{period*((n&15) as f32-4.0)/8.0};
                if x<0.0 {-(((-x-phase+threshold)/period).floor()*period+phase)}
                else {((x-phase+threshold)/period).floor()*period+phase}
            },
        }
    }
    fn zone(&self,n:i32)->io::Result<usize>{match n{0=>Ok(0),1=>Ok(1),_=>Err(bad("TrueType invalid zone"))}}
    fn skip_branch(&self,code:&[u8],mut pc:usize,stop_on_else:bool)->io::Result<usize>{
        let mut nested=0usize;
        while pc<code.len(){let op=code[pc];
            if op==0x58{nested+=1;}else if op==0x59{if nested==0{return Ok(pc+1)}nested-=1;}
            else if op==0x1B&&nested==0&&stop_on_else{return Ok(pc+1);}
            pc=next_code(code,pc)?;
        }
        Err(bad("unbalanced TrueType conditional"))
    }
    fn scan_definition(&self,code:&[u8],mut pc:usize)->io::Result<(usize,usize)>{
        let begin=pc;let mut nested=0;
        while pc<code.len(){match code[pc]{0x2C|0x89=>nested+=1,0x2D=>{
            if nested==0{return Ok((begin,pc));}nested-=1;},_=>{}}
            pc=next_code(code,pc)?;}
        Err(bad("unclosed TrueType function definition"))
    }
    fn exec(&mut self,code:&[u8],depth:usize)->io::Result<()>{
        if depth>self.limits.max_call_depth{return Err(bad("TrueType call depth exceeded"));}
        let(mut pc,mut steps)=(0usize,0usize);
        while pc<code.len(){
            if self.remaining==0{return Err(bad("TrueType instruction budget exhausted"));}
            self.remaining-=1;steps+=1;
            if steps>self.limits.max_instructions{return Err(bad("TrueType instruction limit reached"));}
            let start=pc;let op=code[pc];pc+=1;
            if matches!(op,0x40|0x41|0xB0..=0xBF){pc=push_bytes(code,start,&mut self.stack)?;
                if self.stack.len()>self.limits.max_stack{return Err(bad("TrueType stack limit exceeded"));}
                continue;}
            match op{
                0x00|0x01=>{let v=if op==0{Vec2{x:0.,y:1.}}else{Vec2{x:1.,y:0.}};
                    self.state.projection=v;self.state.freedom=v;self.state.dual=v;},
                0x02|0x03=>{let v=if op==0x02{Vec2{x:0.,y:1.}}else{Vec2{x:1.,y:0.}};self.state.projection=v;self.state.dual=v;},
                0x04|0x05=>{self.state.freedom=if op==0x04{Vec2{x:0.,y:1.}}else{Vec2{x:1.,y:0.}};},
                0x06..=0x09|0x86|0x87=>{
                    let b=self.idx()?;let a=self.idx()?;
                    let za=self.state.zp[1];let zb=self.state.zp[2];
                    let pa=self.coordinate(za,a,false)?;let pb=self.coordinate(zb,b,false)?;
                    let mut v=Vec2{x:pb.x-pa.x,y:pb.y-pa.y};
                    if op&1!=0{v=Vec2{x:-v.y,y:v.x};}
                    v=v.normalized()?;
                    if matches!(op,0x06|0x07|0x86|0x87){self.state.projection=v;self.state.dual=v;}
                    else{self.state.freedom=v;}
                },
                0x0A|0x0B=>{let y=self.pop()? as f32/16384.;let x=self.pop()? as f32/16384.;
                    let v=Vec2{x,y}.normalized()?;
                    if op==0x0A{self.state.projection=v;self.state.dual=v;}else{self.state.freedom=v;}
                },
                0x0C|0x0D=>{let v=if op==0x0C{self.state.projection}else{self.state.freedom};
                    self.push((v.x*16384.).round() as i32)?;self.push((v.y*16384.).round() as i32)?;},
                0x0E=>self.state.freedom=self.state.projection,
                0x0F=>{
                    // ISECT: the intersection of lines a0-a1 and b0-b1.
                    let b1=self.idx()?;let b0=self.idx()?;let a1=self.idx()?;let a0=self.idx()?;let target=self.idx()?;
                    let a=self.coordinate(self.state.zp[1],a0,false)?;
                    let av=self.coordinate(self.state.zp[1],a1,false)?;
                    let b=self.coordinate(self.state.zp[0],b0,false)?;
                    let bv=self.coordinate(self.state.zp[0],b1,false)?;
                    let (ux,uy)=(av.x-a.x,av.y-a.y);let(vx,vy)=(bv.x-b.x,bv.y-b.y);
                    let denom=ux*vy-uy*vx;
                    let result=if denom.abs()<0.00001{
                        Vec2{x:(a.x+av.x+b.x+bv.x)/4.,y:(a.y+av.y+b.y+bv.y)/4.}
                    }else{let t=((b.x-a.x)*vy-(b.y-a.y)*vx)/denom;
                        Vec2{x:a.x+t*ux,y:a.y+t*uy}};
                    let p=self.zone[self.state.zp[2]].get_mut(target).ok_or_else(||bad("ISECT target out of bounds"))?;
                    p.x=result.x;p.y=result.y;p.touch_x=true;p.touch_y=true;
                },
                0x10..=0x12=>{let n=self.idx()?;self.state.rp[(op-0x10) as usize]=n;},
                0x13..=0x15=>{let n=self.pop()?;let z=self.zone(n)?;self.state.zp[(op-0x13) as usize]=z;},
                0x16=>{let n=self.pop()?;self.state.zp=[self.zone(n)?;3];},
                0x17=>{let n=self.idx()?;if n==0||n>4096{return Err(bad("invalid TrueType loop count"));}self.state.loop_value=n;},
                0x18=>self.state.round=Round::Grid,
                0x19=>self.state.round=Round::Half,
                0x1A=>self.state.minimum=self.pop()? as f32/64.,
                0x1B=>pc=self.skip_branch(code,pc,false)?,
                0x1C=>{let delta=self.pop()? as isize;pc=start.checked_add_signed(delta).filter(|p|*p<=code.len()).ok_or_else(||bad("TrueType jump outside program"))?;},
                0x1D=>self.state.cut_in=self.pop()? as f32/64.,
                0x1E=>self.state.single_width_cut_in=self.pop()? as f32/64.,
                0x1F=>self.state.single_width=self.pop()? as f32*self.scale,
                0x20=>{let v=*self.stack.last().ok_or_else(||bad("TrueType DUP underflow"))?;self.push(v)?;},
                0x21=>{self.pop()?;},0x22=>self.stack.clear(),
                0x23=>{let n=self.stack.len();if n<2{return Err(bad("TrueType SWAP underflow"));}self.stack.swap(n-1,n-2);},
                0x24=>self.push(self.stack.len() as i32)?,
                0x25|0x26=>{let n=self.idx()?;if n==0||n>self.stack.len(){return Err(bad("TrueType indexed stack access"));}
                    let pos=self.stack.len()-n;let v=if op==0x25{self.stack[pos]}else{self.stack.remove(pos)};self.push(v)?;},
                0x27=>{let b=self.idx()?;let a=self.idx()?;
                    let va=self.projected(self.state.zp[1],a,false)?;
                    let vb=self.projected(self.state.zp[0],b,false)?;
                    let mid=(va+vb)/2.;self.move_point(self.state.zp[1],a,mid)?;self.move_point(self.state.zp[0],b,mid)?;},
                0x29=>{let p=self.idx()?;let z=self.state.zp[0];let point=self.zone[z].get_mut(p).ok_or_else(||bad("TrueType point outside zone"))?;
                    point.touch_x=false;point.touch_y=false;},
                0x2A=>{let function=self.pop()?;let count=self.idx()?;
                    if count>4096{return Err(bad("TrueType LOOPCALL count exceeded"));}
                    let body=self.functions.get(&function).ok_or_else(||bad("undefined TrueType function"))?.clone();
                    for _ in 0..count{self.exec(&body,depth+1)?;}
                },
                0x28=>return Err(unsupported(op)), // reserved
                0x2B=>{let function=self.pop()?;let body=self.functions.get(&function).ok_or_else(||bad("undefined TrueType function"))?.clone();
                    self.exec(&body,depth+1)?;},
                0x2C|0x89=>{let function=self.pop()?;if function<0||function>65535{return Err(bad("TrueType function ID out of range"));}
                    let (from,to)=self.scan_definition(code,pc)?;let body=code[from..to].to_vec();
                    if op==0x2C{self.functions.insert(function,body);}else{
                        let id=u8::try_from(function).map_err(|_|bad("TrueType instruction ID out of range"))?;
                        self.definitions.insert(id,body);
                    }
                    if self.functions.len()+self.definitions.len()>2048{return Err(bad("TrueType function count exceeded"));}
                    pc=to+1;
                },
                0x2D=>return Ok(()),
                0x2E|0x2F=>{let p=self.idx()?;let z=self.state.zp[0];let old=self.projected(z,p,false)?;
                    self.move_point(z,p,if op&1!=0{self.round(old)}else{old})?;self.state.rp[0]=p;self.state.rp[1]=p;},
                0x30|0x31=>self.interpolate_untouched(op==0x31)?,
                0x32|0x33=>{let reference=self.state.rp[if op&1==0{2}else{1}];
                    let zone=self.state.zp[if op&1==0{1}else{0}];let original=self.projected(zone,reference,true)?;
                    let current=self.projected(zone,reference,false)?;let shift=current-original;
                    for _ in 0..self.state.loop_value{let p=self.idx()?;let z=self.state.zp[2];
                        let value=self.projected(z,p,false)?;self.move_point(z,p,value+shift)?;}
                    self.state.loop_value=1;},
                0x34..=0x37=>{
                    // SHC/SHZ: shift an entire contour or zone relative to rp1/rp2.
                    let target=self.idx()?;
                    let reference=self.state.rp[if op&1==0{2}else{1}];
                    let reference_zone=self.state.zp[if op&1==0{1}else{0}];
                    let shift=self.projected(reference_zone,reference,false)?
                        -self.projected(reference_zone,reference,true)?;
                    let zone=self.state.zp[2];
                    if op<=0x35{
                        let first=if target==0{0}else{self.contours.get(target-1).copied()
                            .ok_or_else(||bad("SHC contour outside glyph"))?+1};
                        let end=*self.contours.get(target).ok_or_else(||bad("SHC contour outside glyph"))?;
                        for i in first..=end{
                            if i==reference&&zone==reference_zone{continue;}
                            let v=self.projected(zone,i,false)?;self.move_point(zone,i,v+shift)?;
                        }
                    }else{
                        let dest=match target{0|1=>target,_=>return Err(bad("SHZ zone outside font"))};
                        let count=self.zone[dest].len();
                        for i in 0..count{
                            if i==reference&&dest==reference_zone{continue;}
                            let v=self.projected(dest,i,false)?;self.move_point(dest,i,v+shift)?;
                        }
                    }
                },
                0x38=>{let amount=self.pop()? as f32/64.;
                    for _ in 0..self.state.loop_value{let p=self.idx()?;let z=self.state.zp[2];let value=self.projected(z,p,false)?;self.move_point(z,p,value+amount)?;}
                    self.state.loop_value=1;},
                0x39=>{let z0=self.state.zp[0];let z1=self.state.zp[1];
                    let rp1=self.state.rp[1];let rp2=self.state.rp[2];
                    let original_first=self.projected(z0,rp1,true)?;let original_last=self.projected(z1,rp2,true)?;
                    let first=self.projected(z0,rp1,false)?;let last=self.projected(z1,rp2,false)?;
                    for _ in 0..self.state.loop_value{let p=self.idx()?;let z=self.state.zp[2];
                        let original=self.projected(z,p,true)?;
                        let ratio=if (original_last-original_first).abs()<1e-6{0.}
                            else{(original-original_first)/(original_last-original_first)};
                        self.move_point(z,p,first+ratio*(last-first))?;}
                    self.state.loop_value=1;},
                0x3A|0x3B=>{let distance=self.pop()? as f32/64.;let p=self.idx()?;
                    let original=self.projected(self.state.zp[0],self.state.rp[0],false)?;
                    self.move_point(self.state.zp[1],p,original+distance)?;
                    self.state.rp[1]=self.state.rp[0];self.state.rp[2]=p;
                    if op&1!=0{self.state.rp[0]=p;}
                },
                0x3C=>{let reference=self.projected(self.state.zp[0],self.state.rp[0],false)?;
                    for _ in 0..self.state.loop_value{let p=self.idx()?;self.move_point(self.state.zp[1],p,reference)?;}
                    self.state.loop_value=1;},
                0x3D=>self.state.round=Round::Double,
                0x3E|0x3F=>{let cvt_index=self.idx()?;let p=self.idx()?;
                    let mut target=*self.cvt.get(cvt_index).ok_or_else(||bad("TrueType CVT index outside table"))?;
                    let zone=self.state.zp[0];
                    if op&1!=0 {
                        let current=self.projected(zone,p,false)?;
                        if zone==1&&(current-target).abs()>self.state.cut_in {target=current;}
                        target=self.round(target);
                    }
                    self.move_point(zone,p,target)?;self.state.rp[0]=p;self.state.rp[1]=p;},
                0x42=>{let value=self.pop()?;let index=self.idx()?;let slot=self.storage.get_mut(index).ok_or_else(||bad("TrueType storage index outside table"))?;*slot=value;},
                0x43=>{let index=self.idx()?;let v=*self.storage.get(index).ok_or_else(||bad("TrueType storage index outside table"))?;self.push(v)?;},
                0x44|0x70=>{let value=self.pop()?;let index=self.idx()?;
                    let slot=self.cvt.get_mut(index).ok_or_else(||bad("TrueType CVT index outside table"))?;
                    *slot=if op==0x44{value as f32/64.}else{value as f32*self.scale};},
                0x45=>{let index=self.idx()?;let v=*self.cvt.get(index).ok_or_else(||bad("TrueType CVT index outside table"))?;
                    self.push((v*64.).round() as i32)?;},
                0x46|0x47=>{let p=self.idx()?;let z=self.state.zp[2];
                    let v=self.projected(z,p,op&1!=0)?;self.push((v*64.).round() as i32)?;},
                0x48=>{let value=self.pop()? as f32/64.;let p=self.idx()?;self.move_point(self.state.zp[2],p,value)?;},
                0x49|0x4A=>{let b=self.idx()?;let a=self.idx()?;
                    let original=op==0x4A;let pa=self.projected(self.state.zp[0],a,original)?;
                    let pb=self.projected(self.state.zp[1],b,original)?;
                    self.push(((pa-pb)*64.).round() as i32)?;},
                0x4B|0x4C=>self.push(self.ppem.round() as i32)?,
                0x4D=>self.state.auto_flip=true,0x4E=>self.state.auto_flip=false,
                0x4F=>{let _=self.pop()?;},
                0x50..=0x55|0x5A|0x5B=>{let b=self.pop()?;let a=self.pop()?;
                    let result=match op{0x50=>a<b,0x51=>a<=b,0x52=>a>b,0x53=>a>=b,
                        0x54=>a==b,0x55=>a!=b,0x5A=>a!=0&&b!=0,_=>a!=0||b!=0};
                    self.push(if result{1}else{0})?;},
                0x56|0x57=>{let v=self.pop()? as f32/64.;let odd=self.round(v).abs() as i32%2!=0;
                    self.push(if odd==(op==0x56){1}else{0})?;},
                0x58=>{if self.pop()?==0{pc=self.skip_branch(code,pc,true)?;}},
                0x59=>{},
                0x5C=>{let n=self.pop()?;self.push(if n==0{1}else{0})?;},
                0x5E=>self.state.delta_base=self.pop()?,
                0x5F=>{let n=self.pop()?;if !(0..=6).contains(&n){return Err(bad("TrueType invalid delta shift"));}self.state.delta_shift=n;},
                0x60..=0x63|0x8B|0x8C=>{let b=self.pop()?;let a=self.pop()?;
                    let n=match op{0x60=>a.checked_add(b),0x61=>a.checked_sub(b),
                        0x62=>{if b==0{return Err(bad("TrueType division by zero"));}a.checked_mul(64).map(|x|x/b)},
                        0x63=>a.checked_mul(b).map(|x|x/64),0x8B=>Some(a.max(b)),_=>Some(a.min(b))};
                    self.push(n.ok_or_else(||bad("TrueType arithmetic overflow"))?)?;},
                0x64..=0x67=>{let n=self.pop()?;self.push(match op{0x64=>n.checked_abs().ok_or_else(||bad("TrueType ABS overflow"))?,
                    0x65=>n.checked_neg().ok_or_else(||bad("TrueType NEG overflow"))?,
                    0x66=>n.div_euclid(64)*64,_=>{let q=n.div_euclid(64);if n%64==0{q*64}else{(q+1)*64}}})?;},
                0x68..=0x6F=>{let v=self.pop()? as f32/64.;let result=if op<0x6C{self.round(v)}else{v};
                    self.push((result*64.).round() as i32)?;},
                0x71..=0x75|0x5D=>{let count=self.idx()?;if count>1024{return Err(bad("TrueType delta count exceeded"));}
                    for _ in 0..count{let arg=self.pop()?;let id=self.idx()?;
                        let nibble=arg&15;let adjust=if nibble>=8{nibble-7}else{nibble-8};
                        let base=self.state.delta_base+((arg>>4)&15)+match op{0x71|0x74=>16,0x72|0x75=>32,_=>0};
                        if self.ppem.round() as i32!=base{continue;}
                        let delta=adjust as f32/((1<<self.state.delta_shift) as f32);
                        if matches!(op,0x73..=0x75){let slot=self.cvt.get_mut(id).ok_or_else(||bad("TrueType CVT delta index"))?;*slot+=delta;}
                        else{let z=self.state.zp[0];let p=self.projected(z,id,false)?;self.move_point(z,id,p+delta)?;}
                    }
                },
                0x76|0x77=>{let n=self.pop()? as u8;
                    self.state.round=Round::Super(n,if op==0x77 {std::f32::consts::FRAC_1_SQRT_2}else{1.0});},
                0x78|0x79=>{let offset=self.pop()? as isize;let condition=self.pop()?;
                    if (op==0x78&&condition!=0)||(op==0x79&&condition==0){
                        pc=start.checked_add_signed(offset).filter(|p|*p<=code.len()).ok_or_else(||bad("TrueType conditional jump outside code"))?;
                    }
                },
                0x7A=>self.state.round=Round::Off,
                0x7C=>self.state.round=Round::Up,
                0x7D=>self.state.round=Round::Down,
                0x7E|0x7F=>{let _=self.pop()?;},
                0x85=>self.state.scan_control=self.pop()? as u16,
                0x8D=>{let n=self.pop()?;if !(0..=7).contains(&n){return Err(bad("invalid TrueType SCANTYPE"));}
                    self.state.scan_type=n as u16;},
                0x80=>{for _ in 0..self.state.loop_value{
                    let index=self.idx()?;
                    let p=self.zone[self.state.zp[0]].get_mut(index)
                        .ok_or_else(||bad("TrueType FLIPPT index"))?;
                    p.on_curve=!p.on_curve;
                  }self.state.loop_value=1;},
                0x81|0x82=>{let high=self.idx()?;let low=self.idx()?;
                    if high<low||high-low>65_535{return Err(bad("invalid FLIPRG range"));}
                    for index in low..=high{
                        let p=self.zone[self.state.zp[0]].get_mut(index)
                            .ok_or_else(||bad("TrueType FLIPRG index"))?;
                        p.on_curve=op==0x81;
                    }
                },
                0x88=>{let selector=self.pop()?;
                    // v35 represents vertical phantom points and grayscale.
                    // No rotation, LCD subpixel mode or vertical stretching
                    // is claimed by this software rasterizer.
                    let mut result=if selector&1!=0 {35}else{0};
                    if selector&16!=0{result|=0x800;}
                    if selector&32!=0{result|=0x1000;}
                    self.push(result)?;
                },
                0x8A=>{let n=self.stack.len();if n<3{return Err(bad("TrueType ROLL underflow"));}
                    self.stack[n-3..n].rotate_left(1);},
                0x8E=>{let selector=self.pop()?;let value=self.pop()?;
                    match (selector,value){
                        (1,0|1)=>self.state.disable_grid_fit=value==1,
                        (2,0|2)=>self.state.ignore_prep_settings=value==2,
                        (3,0|4)=>{}, // ClearType-specific rasterizer mode is not used.
                        _=>return Err(bad("invalid TrueType INSTCTRL selector/value")),
                    }
                },
                0xC0..=0xFF=>self.relative_move(op)?,
                _=>{if let Some(body)=self.definitions.get(&op).cloned(){self.exec(&body,depth+1)?;}
                    else{return Err(unsupported(op));}}
            }
        }
        Ok(())
    }
    fn relative_move(&mut self,op:u8)->io::Result<()>{
        let cvt=if op>=0xE0{Some(self.idx()?)}else{None};let p=self.idx()?;
        let reference=self.state.rp[0];let z0=self.state.zp[0];let z1=self.state.zp[1];
        let current=self.projected(z0,reference,false)?;
        let original=self.projected(z1,p,true)?-self.projected(z0,reference,true)?;
        let mut distance=if let Some(id)=cvt{
            let mut value=*self.cvt.get(id).ok_or_else(||bad("TrueType MIRP CVT index"))?;
            if self.state.auto_flip&&original*value<0.0 {value=-value;}
            if (value.abs()-self.state.single_width.abs()).abs()
                < self.state.single_width_cut_in {
                value=self.state.single_width.copysign(value);
            }
            // The control-value cut-in is only consulted for rounded MIRP.
            if op&0x04!=0 && z0==z1 && (value-original).abs()>self.state.cut_in {
                original
            } else {value}
        }else{
            if (original.abs()-self.state.single_width.abs()).abs()
                < self.state.single_width_cut_in {
                self.state.single_width.copysign(original)
            }else{original}
        };
        if op&0x04!=0{distance=self.round(distance);}
        if op&0x08!=0&&distance.abs()<self.state.minimum{distance=self.state.minimum.copysign(distance);}
        self.move_point(z1,p,current+distance)?;
        self.state.rp[1]=reference;self.state.rp[2]=p;
        if op&0x10!=0{self.state.rp[0]=p;}
        Ok(())
    }
    fn interpolate_untouched(&mut self,x_axis:bool)->io::Result<()>{
        let mut start=0;
        for &end in &self.contours{
            if end>=self.zone[1].len()||end<start{return Err(bad("TrueType contour boundary invalid"));}
            let touched=(start..=end).filter(|&i|if x_axis{self.zone[1][i].touch_x}else{self.zone[1][i].touch_y}).collect::<Vec<_>>();
            if !touched.is_empty(){
                if (end-start+1).saturating_mul(touched.len())>4_000_000{
                    return Err(bad("TrueType IUP work limit exceeded"));
                }
                for i in start..=end{
                    if touched.contains(&i){continue;}
                    let prev=touched.iter().rev().copied().find(|&j|j<i).unwrap_or(*touched.last().unwrap());
                    let next=touched.iter().copied().find(|&j|j>i).unwrap_or(touched[0]);
                    let o=if x_axis{self.original[1][i].x}else{self.original[1][i].y};
                    let (a,b)=(if x_axis{self.original[1][prev].x}else{self.original[1][prev].y},
                        if x_axis{self.original[1][next].x}else{self.original[1][next].y});
                    let (aa,bb)=(if x_axis{self.zone[1][prev].x}else{self.zone[1][prev].y},
                        if x_axis{self.zone[1][next].x}else{self.zone[1][next].y});
                    let value=if (b-a).abs()<0.00001{o+aa-a}else if o<=a.min(b){o+if a<b{aa-a}else{bb-b}}
                        else if o>=a.max(b){o+if a>b{aa-a}else{bb-b}}
                        else{aa+(o-a)*(bb-aa)/(b-a)};
                    if x_axis{self.zone[1][i].x=value;}else{self.zone[1][i].y=value;}
                }
            }
            start=end+1;
        }
        Ok(())
    }
}
fn instructions<'a>(head:&[u8],loca:&[u8],glyf:&'a [u8],glyph:u16)->io::Result<&'a [u8]>{
    let i=glyph as usize;let format=u16be(head,50).ok_or_else(||bad("invalid head table"))?;
    let (from,to)=match format{
        0=>(u16be(loca,2*i).map(|v|v as usize*2),u16be(loca,2*i+2).map(|v|v as usize*2)),
        1=>(u32be(loca,4*i).map(|v|v as usize),u32be(loca,4*i+4).map(|v|v as usize)),
        _=>return Err(bad("invalid loca format")),
    };
    let glyph=glyf.get(from.ok_or_else(||bad("truncated loca"))?..to.ok_or_else(||bad("truncated loca"))?).ok_or_else(||bad("glyf offset out of range"))?;
    if glyph.len()<10{return Ok(&[]);}
    let contour_count=u16be(glyph,0).unwrap_or(0) as i16;
    let at=if contour_count>=0{
        let count=contour_count as usize;
        let ends=10usize.checked_add(count*2).ok_or_else(||bad("glyf contours overflow"))?;
        ends
    }else{
        let mut at=10usize;let mut flags=0u16;
        for _ in 0..256{
            flags=u16be(glyph,at).ok_or_else(||bad("composite instructions truncated"))?;
            let _=u16be(glyph,at+2).ok_or_else(||bad("composite glyph ID truncated"))?;
            at+=4;if flags&1!=0{at+=4;}else{at+=2;}
            if flags&8!=0{at+=2;}else if flags&64!=0{at+=4;}else if flags&128!=0{at+=8;}
            if at>glyph.len(){return Err(bad("composite operands truncated"));}
            if flags&0x20==0{break;}
        }
        if flags&0x20!=0{return Err(bad("composite component limit exceeded"));}
        if flags&0x100==0{return Ok(&[]);}at
    };
    let size=u16be(glyph,at).ok_or_else(||bad("glyph instruction size truncated"))? as usize;
    glyph.get(at+2..at+2+size).ok_or_else(||bad("glyph instructions truncated"))
}
/// A hinted outline and metrics derived from the four phantom points.
#[derive(Debug,Clone)]
pub struct HintedGlyph {
    pub outline:GlyphOutline,
    /// Pixel size independent advance width after glyph instruction execution.
    pub advance_width_units:f32,
    /// Updated phantom-point locations in unscaled font design units.
    pub phantom_points:[(f32,f32);4],
    /// Font-requested scan conversion options. Kept separate from the outline,
    /// so a renderer can explicitly choose simple/smart dropout behavior.
    pub scan_control:u16,
    pub scan_type:u16,
}
/// Execute TrueType font and glyph instructions. The input outline should have
/// already received any `gvar` deltas; output coordinates stay in design units.
/// An unsupported bytecode instruction returns `Unsupported`.
pub fn hint_outline_with_metrics(
    outline:&GlyphOutline,head:&[u8],loca:&[u8],glyf:&[u8],glyph:u16,
    fpgm:Option<&[u8]>,prep:Option<&[u8]>,cvt:Option<&[u8]>,maxp:Option<&[u8]>,
    ppem:f32,units_per_em:u16,phantoms:Option<[(f32,f32);4]>,limits:HintLimits,
)->io::Result<HintedGlyph>{
    if !ppem.is_finite()||ppem<1.0||ppem>4096.0||units_per_em==0{return Err(bad("invalid TrueType hinting scale"));}
    if outline.points.len()>65_535{return Err(bad("hinting point limit exceeded"));}
    let scale=ppem/units_per_em as f32;
    let mut vm=Vm::new(cvt.unwrap_or(&[]),maxp,ppem,scale,limits)?;
    if let Some(global)=fpgm{if global.len()>1_048_576{return Err(bad("TrueType font program too large"));}vm.exec(global,0)?;vm.stack.clear();}
    if let Some(pre)=prep{if pre.len()>1_048_576{return Err(bad("TrueType prep program too large"));}vm.exec(pre,0)?;vm.stack.clear();}
    // INSTCTRL selector 2 means the glyph program must see the default
    // graphics parameters, not the modifications performed by prep. This
    // does not discard prep's CVT, storage, or function definitions.
    if vm.state.ignore_prep_settings {
        let disabled=vm.state.disable_grid_fit;
        vm.state=State::default();
        vm.state.disable_grid_fit=disabled;
    }
    // Graphics state and CVT after prep are inherited by every glyph.
    let mut input=outline.points.iter().map(|p|Point{x:p.x*scale,y:p.y*scale,touch_x:false,touch_y:false,on_curve:p.on_curve}).collect::<Vec<_>>();
    // TrueType appends four phantom points after the contour points, used to
    // hint bearings, horizontal advances and vertical metrics. Glyph programs
    // may reference them even though they are not part of visible contours.
    let fallback=[(0.,0.),(units_per_em as f32,0.),(0.,0.),(0.,-(units_per_em as f32))];
    for (x,y) in phantoms.unwrap_or(fallback){
        input.push(Point{x:x*scale,y:y*scale,touch_x:false,touch_y:false,on_curve:true});
    }
    vm.original[1]=input.clone();vm.zone[1]=input;vm.contours=outline.contours.clone();
    vm.state.rp=[0;3];vm.state.zp=[1;3];vm.state.loop_value=1;
    if !vm.state.disable_grid_fit{
        let program=instructions(head,loca,glyf,glyph)?;
        vm.exec(program,0)?;
    }
    let result=vm.zone[1].iter().take(outline.points.len()).map(|p|OutlinePoint{
        x:p.x/scale,y:p.y/scale,on_curve:p.on_curve}).collect();
    let mut ph=[(0.,0.);4];
    for (i,position) in ph.iter_mut().enumerate(){
        let p=vm.zone[1].get(outline.points.len()+i).ok_or_else(||bad("missing TrueType phantom point"))?;
        *position=(p.x/scale,p.y/scale);
    }
    let advance_width_units=ph[1].0-ph[0].0;
    Ok(HintedGlyph{outline:GlyphOutline{points:result,contours:outline.contours.clone()},
        advance_width_units,phantom_points:ph,
        scan_control:vm.state.scan_control,scan_type:vm.state.scan_type})
}
/// Convenience wrapper returning only the hinted glyph contour geometry.
pub fn hint_outline(
    outline:&GlyphOutline,head:&[u8],loca:&[u8],glyf:&[u8],glyph:u16,
    fpgm:Option<&[u8]>,prep:Option<&[u8]>,cvt:Option<&[u8]>,maxp:Option<&[u8]>,
    ppem:f32,units_per_em:u16,phantoms:Option<[(f32,f32);4]>,limits:HintLimits,
)->io::Result<GlyphOutline>{
    Ok(hint_outline_with_metrics(outline,head,loca,glyf,glyph,fpgm,prep,cvt,maxp,
        ppem,units_per_em,phantoms,limits)?.outline)
}
