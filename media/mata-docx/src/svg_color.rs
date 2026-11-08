//! OpenType `SVG ` color-glyph table, including gzip-compressed documents.
//! Provides validated glyph XML plus a bounded software rasterizer for local
//! SVG shapes, transforms, paint servers, styles, and clipping. No scripts, remote resources or native SVG
//! engines are used. Unsupported SVG features return `Unsupported` rather
//! than being silently dropped.
use std::io::{self,Error,ErrorKind};
use std::collections::BTreeMap;
use crate::{opentype::Face,colr::{ColorBitmap,blend_over},xml::{self,Element,Node}};
fn bad(s:&'static str)->io::Error{Error::new(ErrorKind::InvalidData,s)}
fn unsupported(s:&'static str)->io::Error{Error::new(ErrorKind::Unsupported,s)}
fn be16(b:&[u8],at:usize)->Option<u16>{let v=b.get(at..at.checked_add(2)?)?;Some(u16::from_be_bytes([v[0],v[1]]))}
fn be32(b:&[u8],at:usize)->Option<u32>{let v=b.get(at..at.checked_add(4)?)?;Some(u32::from_be_bytes([v[0],v[1],v[2],v[3]]))}
/// A decoded SVG document shared by a contiguous glyph ID range.
#[derive(Debug,Clone)]pub struct SvgGlyphDocument {pub first_glyph:u16,pub last_glyph:u16,pub xml:String}
fn decompress_gzip(data:&[u8])->io::Result<Vec<u8>>{
 if data.len()<18||data.get(0..3)!=Some(&[0x1f,0x8b,8]){return Err(bad("invalid gzip OpenType SVG document"));}
 let flags=data[3];if flags&0xE0!=0{return Err(bad("gzip reserved flags set"));}
 let mut at=10usize;
 if flags&4!=0{let n=be16(data,at).ok_or_else(||bad("gzip extra field truncated"))? as usize;at+=2;
  at=at.checked_add(n).filter(|n|*n<=data.len()).ok_or_else(||bad("gzip extra field overflow"))?;}
 for mask in [8,16]{if flags&mask!=0{loop{let b=*data.get(at).ok_or_else(||bad("gzip name/comment truncated"))?;at+=1;
  if b==0{break;}if at>65536{return Err(bad("gzip metadata too long"));}}}}
 if flags&2!=0{
  let crc=data.get(at..at+2).ok_or_else(||bad("gzip header checksum truncated"))?;
  let expected=u16::from_le_bytes([crc[0],crc[1]]);
  if (crate::zip::crc32(&data[..at]) as u16)!=expected{return Err(bad("gzip header CRC16 mismatch"));}
  at+=2;
 }
 let len=data.len();if at>len-8{return Err(bad("gzip payload missing"));}
 let expected=be32(data,len-4).unwrap_or(0) as usize;
 if expected>4*1024*1024{return Err(bad("OpenType SVG uncompressed size limit"));}
 let decoded=crate::zip::inflate(&data[at..len-8],expected)?;
 let crc=be32(data,len-8).ok_or_else(||bad("gzip checksum missing"))?;
 if crate::zip::crc32(&decoded)!=crc{return Err(bad("OpenType SVG gzip CRC mismatch"));}
 Ok(decoded)
}
/// Decode the SVG document for a glyph. Invalid XML and external entities are
/// rejected; callers should apply additional security policy before showing SVG
/// in a general-purpose renderer.
pub fn glyph_document(face:&Face,glyph:u16)->io::Result<Option<SvgGlyphDocument>>{
 let Some(svg)=face.table(*b"SVG ")else{return Ok(None)};
 if be16(svg,0)!=Some(0){return Err(bad("unsupported SVG table version"));}
 let offset=be32(svg,2).ok_or_else(||bad("truncated SVG table header"))? as usize;
 let n=be16(svg,offset).ok_or_else(||bad("truncated SVG document list"))? as usize;
 if n>65_536{return Err(bad("SVG glyph record limit exceeded"));}
 let mut previous=0u16;
 for i in 0..n{
  let at=offset.checked_add(2+i*12).ok_or_else(||bad("SVG record offset overflow"))?;
  let first=be16(svg,at).ok_or_else(||bad("truncated SVG glyph record"))?;
  let last=be16(svg,at+2).ok_or_else(||bad("truncated SVG glyph record"))?;
  if last<first||(i!=0&&first<=previous){return Err(bad("overlapping or unsorted SVG glyph ranges"));}
  previous=last;if glyph<first||glyph>last{continue;}
  let begin=offset.checked_add(be32(svg,at+4).ok_or_else(||bad("SVG document offset truncated"))? as usize).ok_or_else(||bad("SVG document overflow"))?;
  let len=be32(svg,at+8).ok_or_else(||bad("SVG document length truncated"))? as usize;
  if len>4*1024*1024{return Err(bad("SVG document too large"));}
  let data=svg.get(begin..begin.checked_add(len).ok_or_else(||bad("SVG length overflow"))?).ok_or_else(||bad("SVG document outside table"))?;
  let decoded=if data.starts_with(&[0x1f,0x8b]){decompress_gzip(data)?}else{data.to_vec()};
  let text=String::from_utf8(decoded).map_err(|_|bad("OpenType SVG document is not UTF-8"))?;
  let limits=xml::Limits{max_bytes:4*1024*1024,max_depth:96,max_nodes:100_000,max_attributes:128,max_value_bytes:256*1024};
  let root=xml::parse_with_limits(&text,limits)?;
  if root.local()!="svg"{return Err(bad("OpenType SVG root element missing"));}
  return Ok(Some(SvgGlyphDocument{first_glyph:first,last_glyph:last,xml:text}));
 }
 Ok(None)
}
// Accept only local, fragment-only SVG references. Never fetch external URLs.
fn local_reference(value:&str)->io::Result<&str>{
    let value=value.trim();
    let candidate=if let Some(fragment)=value.strip_prefix('#') {
        fragment
    }else if let Some(inner)=value.strip_prefix("url(").and_then(|s|s.strip_suffix(')')) {
        let inner=inner.trim().trim_matches(|c|c=='\"'||c=='\'');
        inner.strip_prefix('#').ok_or_else(||bad("SVG reference must be local"))?
    }else{
        return Err(bad("SVG reference must be local"));
    };
    if candidate.is_empty() || candidate.len()>256 ||
       !candidate.chars().all(|c|c.is_alphanumeric()||matches!(c,'_'|'-'|'.'|':')) {
        return Err(bad("invalid local SVG reference"));
    }
    Ok(candidate)
}

// Iterative walk avoids recursion in a font-controlled XML document.
fn find_id<'a>(root:&'a Element,id:&str)->Option<&'a Element>{
    let mut pending=vec![root];
    while let Some(node)=pending.pop(){
        if node.attr("id")==Some(id){return Some(node);}
        for child in node.children.iter().rev(){
            if let Node::Element(element)=child{pending.push(element);}
        }
    }
    None
}

fn number(s:&str)->io::Result<f32>{
 let s=s.trim().trim_end_matches("px");let v=s.parse::<f32>().map_err(|_|bad("invalid SVG numeric value"))?;
 if !v.is_finite()||v.abs()>1_000_000.{return Err(bad("SVG numeric limit exceeded"));}Ok(v)
}
fn numbers(s:&str)->io::Result<Vec<f32>>{
 s.split(|c:char|c.is_ascii_whitespace()||c==',').filter(|s|!s.is_empty()).map(number).collect()
}
fn property<'a>(node:&'a Element,name:&str)->Option<&'a str>{
 if let Some(v)=node.attr("style").and_then(|value|value.split(';').rev()
     .filter_map(|kv|kv.split_once(':')).find(|(k,_)|k.trim()==name).map(|(_,v)|v.trim())){
     return Some(v);
 }
 node.attr(name)
}
/// Apply a bounded subset of SVG embedded stylesheet selectors before rendering.
/// Presentation attributes have lower priority than these rules and inline
/// `style` declarations. External @import, stylesheets and scripting are not
/// fetched or executed.
fn apply_embedded_styles(root:&mut Element)->io::Result<()> {
    #[derive(Clone)] struct Rule{selector:String,declarations:String,weight:u8,order:usize}
    let mut style_nodes=Vec::new();root.descendants("style",&mut style_nodes);
    if style_nodes.len()>32{return Err(bad("too many SVG style elements"));}
    let mut rules:Vec<Rule>=Vec::new();
    for style in style_nodes {
        let value=style.text();
        if value.len()>256*1024{return Err(bad("SVG stylesheet size limit"));}
        // CSS comments must not be mistaken for part of a selector.
        let mut without_comments=String::with_capacity(value.len());
        let mut tail=value.as_str();
        while let Some(start)=tail.find("/*"){
            without_comments.push_str(&tail[..start]);
            let suffix=&tail[start+2..];
            let end=suffix.find("*/").ok_or_else(||bad("unterminated SVG CSS comment"))?;
            tail=&suffix[end+2..];
        }
        without_comments.push_str(tail);
        let mut remaining=without_comments.as_str();
        while let Some(open)=remaining.find('{') {
            let selectors=remaining[..open].trim();
            let after=&remaining[open+1..];
            let close=after.find('}').ok_or_else(||bad("unclosed SVG stylesheet block"))?;
            let body=after[..close].trim();
            if selectors.starts_with('@'){
                return Err(unsupported("SVG stylesheet at-rules are not supported"));
            }
            for selector in selectors.split(',') {
                let selector=selector.trim();
                if selector.is_empty(){continue;}
                let (weight,ident)=if selector==":root" {(0,"root")}
                    else if let Some(id)=selector.strip_prefix('#') {(3,id)}
                    else if let Some(class)=selector.strip_prefix('.') {(2,class)}
                    else {(1,selector)};
                if ident.is_empty() || !ident.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'_'||b==b'-') {
                    return Err(unsupported("SVG complex stylesheet selector"));
                }
                if rules.len()>1024{return Err(bad("too many SVG stylesheet rules"));}
                rules.push(Rule{selector:selector.to_string(),declarations:body.to_string(),weight,order:rules.len()});
            }
            remaining=&after[close+1..];
        }
    }
    rules.sort_by_key(|r|(r.weight,r.order));
    fn walk(node:&mut Element,rules:&[Rule],depth:usize)->io::Result<()> {
        if depth>96{return Err(bad("SVG stylesheet traversal depth"));}
        let mut inherited=String::new();
        for rule in rules {
            let matches=match rule.weight{
                3=>node.attr("id")==rule.selector.strip_prefix('#'),
                2=>node.attr("class").map(|classes|classes.split_ascii_whitespace()
                    .any(|class|Some(class)==rule.selector.strip_prefix('.'))).unwrap_or(false),
                0=>node.local()=="svg",
                _=>node.local()==rule.selector,
            };
            if matches {
                if inherited.len()+rule.declarations.len()>65_536{
                    return Err(bad("SVG computed stylesheet size limit"));
                }
                inherited.push_str(&rule.declarations);
                inherited.push(';');
            }
        }
        if !inherited.is_empty(){
            if let Some(inline)=node.attrs.get("style") {inherited.push_str(inline);}
            node.attrs.insert("style".to_string(),inherited);
        }
        for child in &mut node.children {
            if let Node::Element(e)=child {walk(e,rules,depth+1)?;}
        }
        Ok(())
    }
    walk(root,&rules,0)
}
fn color(s:&str)->io::Result<u32>{
 let t=s.trim();let s=t.to_ascii_lowercase();
 if let Some(params)=s.strip_prefix("rgb(").and_then(|v|v.strip_suffix(')')) {
     let components=params.split(|c:char|c==','||c.is_ascii_whitespace())
       .filter(|v|!v.is_empty()).collect::<Vec<_>>();
     if components.len()!=3{return Err(bad("invalid SVG rgb() value"));}
     let mut rgb=0u32;
     for channel in components {
       let n=if channel.ends_with('%') {
         (percent(channel)?*255.0).round()
       }else {number(channel)?};
       if n<0.0||n>255.0{return Err(bad("invalid SVG rgb() channel"));}
       rgb=(rgb<<8)|(n.round() as u32);
     }
     return Ok((rgb<<8)|255);
 }
 let rgb=match s.as_str(){"black"=>0x000000ff,"white"=>0xffffffff,
  "red"=>0xff0000ff,"green"=>0x008000ff,"blue"=>0x0000ffff,
  "yellow"=>0xffff00ff,"gray"|"grey"=>0x808080ff,"none"=>0,
  _ if s.starts_with('#')=>{
   let h=&s[1..];match h.len(){3=>{let mut v=0u32;for c in h.chars(){let n=c.to_digit(16).ok_or_else(||bad("invalid SVG hex color"))? as u32;
      v=(v<<8)|((n<<4)|n);}v<<8|255},
    6=>u32::from_str_radix(h,16).map_err(|_|bad("invalid SVG hex color"))?<<8|255,
    8=>u32::from_str_radix(h,16).map_err(|_|bad("invalid SVG hex color"))?,
    _=>return Err(unsupported("unsupported SVG color notation"))}},
  _=>return Err(unsupported("unsupported SVG CSS paint")),
 };Ok(rgb)
}
fn path_tokens(input:&str)->io::Result<Vec<String>>{
 let data=input.as_bytes();let mut at=0;let mut tokens=Vec::new();
 while at<data.len(){let c=data[at] as char;
  if c.is_ascii_whitespace()||c==','{at+=1;continue;}
  if c.is_ascii_alphabetic(){tokens.push(c.to_string());at+=1;}
  else{let first=at;if matches!(c,'+'|'-'){at+=1;}
    let mut digits=0;
    while at<data.len()&&(data[at] as char).is_ascii_digit(){at+=1;digits+=1;}
    if data.get(at)==Some(&b'.'){at+=1;while at<data.len()&&(data[at] as char).is_ascii_digit(){at+=1;digits+=1;}}
    if digits==0{return Err(bad("invalid SVG path number"));}
    if matches!(data.get(at),Some(b'e'|b'E')){let save=at;at+=1;
      if matches!(data.get(at),Some(b'+'|b'-')){at+=1;}
      let exp=at;while at<data.len()&&(data[at] as char).is_ascii_digit(){at+=1;}
      if exp==at{at=save;}}
    tokens.push(input[first..at].to_string());}
  if tokens.len()>65_536{return Err(bad("SVG path too long"));}
 }
 Ok(tokens)
}
/// Convert an SVG elliptical-arc endpoint form to a bounded polyline.
fn arc_points(from:(f32,f32),to:(f32,f32),mut rx:f32,mut ry:f32,
              rotation:f32,large:bool,sweep:bool)->io::Result<Vec<(f32,f32)>> {
    rx=rx.abs();ry=ry.abs();
    if rx<0.00001||ry<0.00001||from==to{return Ok(vec![to]);}
    let phi=rotation.to_radians();let (sin,cos)=phi.sin_cos();
    let dx=(from.0-to.0)*0.5;let dy=(from.1-to.1)*0.5;
    let (xp,yp)=(cos*dx+sin*dy,-sin*dx+cos*dy);
    let lambda=xp*xp/(rx*rx)+yp*yp/(ry*ry);
    if lambda>1.0 {let k=lambda.sqrt();rx*=k;ry*=k;}
    let top=(rx*rx*ry*ry-rx*rx*yp*yp-ry*ry*xp*xp).max(0.0);
    let bottom=rx*rx*yp*yp+ry*ry*xp*xp;
    let factor=if bottom<1e-12{0.0}else{(top/bottom).sqrt()}
       * if large==sweep{-1.0}else{1.0};
    let (cxp,cyp)=(factor*rx*yp/ry,-factor*ry*xp/rx);
    let center=((from.0+to.0)*0.5+cos*cxp-sin*cyp,
                (from.1+to.1)*0.5+sin*cxp+cos*cyp);
    let (ux,uy)=((xp-cxp)/rx,(yp-cyp)/ry);
    let (vx,vy)=((-xp-cxp)/rx,(-yp-cyp)/ry);
    let begin=uy.atan2(ux);
    let mut sweep_angle=(ux*vy-uy*vx).atan2(ux*vx+uy*vy);
    if !sweep&&sweep_angle>0.0 {sweep_angle-=std::f32::consts::TAU;}
    if sweep&&sweep_angle<0.0 {sweep_angle+=std::f32::consts::TAU;}
    let sections=((sweep_angle.abs()*rx.max(ry)/3.0).ceil() as usize).clamp(1,512);
    let mut result=Vec::with_capacity(sections);
    for step in 1..=sections {
        let a=begin+sweep_angle*step as f32/sections as f32;
        let (sa,ca)=a.sin_cos();
        result.push((center.0+cos*rx*ca-sin*ry*sa,
                     center.1+sin*rx*ca+cos*ry*sa));
    }
    if let Some(last)=result.last_mut(){*last=to;}
    Ok(result)
}
fn parse_path(d:&str)->io::Result<Vec<Vec<(f32,f32)>>> {
 let tokens=path_tokens(d)?;let(mut at,mut cmd)=(0usize,' ');
 let(mut x,mut y,mut sx,mut sy)=(0f32,0f32,0f32,0f32);
 let(mut last_cubic,mut last_quad)=(None,None);
 let mut paths=Vec::new();let mut path=Vec::new();
 while at<tokens.len(){
  if tokens[at].len()==1 && tokens[at].chars().next().unwrap().is_ascii_alphabetic(){
   cmd=tokens[at].chars().next().unwrap();at+=1;
  }
  if cmd==' '{return Err(bad("SVG path starts without command"));}
  let relative=cmd.is_ascii_lowercase();let kind=cmd.to_ascii_uppercase();
  if kind=='Z'{
    if path.len()>1{path.push((sx,sy));paths.push(std::mem::take(&mut path));}
    x=sx;y=sy;cmd=' ';last_cubic=None;last_quad=None;continue;
  }
  let n=match kind{'M'|'L'|'T'=>2,'H'|'V'=>1,'Q'|'S'=>4,'C'=>6,'A'=>7,
       _=>return Err(unsupported("unsupported SVG path command"))};
  if at+n>tokens.len(){return Err(bad("SVG path missing operands"));}
  let mut args=Vec::with_capacity(n);
  for _ in 0..n{args.push(number(&tokens[at])?);at+=1;}
  let point=|a:f32,b:f32|if relative{(x+a,y+b)}else{(a,b)};
  match kind{
   'M'=>{
     if path.len()>1{paths.push(std::mem::take(&mut path));}
     (x,y)=point(args[0],args[1]);sx=x;sy=y;path.push((x,y));
     cmd=if relative{'l'}else{'L'};last_cubic=None;last_quad=None;
   },
   'L'=>{(x,y)=point(args[0],args[1]);path.push((x,y));last_cubic=None;last_quad=None;},
   'H'=>{x=if relative{x+args[0]}else{args[0]};path.push((x,y));last_cubic=None;last_quad=None;},
   'V'=>{y=if relative{y+args[0]}else{args[0]};path.push((x,y));last_cubic=None;last_quad=None;},
   'Q'|'T'=>{
      let control=if kind=='Q'{point(args[0],args[1])}
          else{let old=last_quad.unwrap_or((x,y));(2.0*x-old.0,2.0*y-old.1)};
      let end=if kind=='Q'{point(args[2],args[3])}else{point(args[0],args[1])};
      let (px,py)=(x,y);
      for i in 1..=20 {let t=i as f32/20.;let a=1.-t;
         path.push((a*a*px+2.*a*t*control.0+t*t*end.0,
                    a*a*py+2.*a*t*control.1+t*t*end.1));}
      (x,y)=end;last_quad=Some(control);last_cubic=None;
   },
   'C'|'S'=>{
      let (c1,c2,end)=if kind=='C'{(point(args[0],args[1]),point(args[2],args[3]),point(args[4],args[5]))}
      else{let old=last_cubic.unwrap_or((x,y));((2.*x-old.0,2.*y-old.1),point(args[0],args[1]),point(args[2],args[3]))};
      let (px,py)=(x,y);
      for i in 1..=32 {let t=i as f32/32.;let a=1.-t;
          path.push((a*a*a*px+3.*a*a*t*c1.0+3.*a*t*t*c2.0+t*t*t*end.0,
                     a*a*a*py+3.*a*a*t*c1.1+3.*a*t*t*c2.1+t*t*t*end.1));}
      (x,y)=end;last_cubic=Some(c2);last_quad=None;
   },
   'A'=>{
      let (large,sweep)=(args[3]==1.0,args[4]==1.0);
      if (args[3]!=0.0&&args[3]!=1.0)||(args[4]!=0.0&&args[4]!=1.0){
          return Err(bad("SVG arc flags must be 0 or 1"));
      }
      let end=point(args[5],args[6]);
      path.extend(arc_points((x,y),end,args[0],args[1],args[2],large,sweep)?);
      (x,y)=end;last_cubic=None;last_quad=None;
   },
   _=>unreachable!(),
  }
  if path.len()>65536{return Err(bad("SVG path point count exceeded"));}
 }
 if path.len()>1 {paths.push(path);}
 Ok(paths)
}
/// SVG 2-D affine transform in row-major `(a,b,c,d,e,f)` form.
#[derive(Clone,Copy,Debug)]struct Affine{a:f32,b:f32,c:f32,d:f32,e:f32,f:f32}
impl Affine {
 fn identity()->Self{Self{a:1.,b:0.,c:0.,d:1.,e:0.,f:0.}}
 fn then(self,o:Self)->Self{Self{
    a:self.a*o.a+self.c*o.b,b:self.b*o.a+self.d*o.b,
    c:self.a*o.c+self.c*o.d,d:self.b*o.c+self.d*o.d,
    e:self.a*o.e+self.c*o.f+self.e,f:self.b*o.e+self.d*o.f+self.f,
 }}
 fn inverse(self)->io::Result<Self>{
    let det=self.a*self.d-self.b*self.c;
    if !det.is_finite()||det.abs()<1e-8{return Err(bad("SVG non-invertible transform"));}
    let a=self.d/det;let b=-self.b/det;let c=-self.c/det;let d=self.a/det;
    Ok(Self{a,b,c,d,e:-a*self.e-c*self.f,f:-b*self.e-d*self.f})
 }
 fn apply(self,p:(f32,f32))->(f32,f32){
    (self.a*p.0+self.c*p.1+self.e,self.b*p.0+self.d*p.1+self.f)
 }
}
fn transforms(spec:&str)->io::Result<Affine>{
 let mut t=Affine::identity();let mut rest=spec.trim();let mut operations=0usize;
 while !rest.is_empty(){
  operations+=1;if operations>64{return Err(bad("too many SVG transforms"));}
  let at=rest.find('(').ok_or_else(||bad("SVG transform missing opening parenthesis"))?;
  let close=rest[at+1..].find(')').ok_or_else(||bad("SVG transform missing close"))?+at+1;
  let name=rest[..at].trim();let args=numbers(&rest[at+1..close])?;
  let op=match(name,args.as_slice()){
   ("translate",[x])=>Affine{e:*x,..Affine::identity()},
   ("translate",[x,y])=>Affine{e:*x,f:*y,..Affine::identity()},
   ("scale",[x])=>Affine{a:*x,d:*x,..Affine::identity()},
   ("scale",[x,y])=>Affine{a:*x,d:*y,..Affine::identity()},
   ("matrix",[a,b,c,d,e,f])=>Affine{a:*a,b:*b,c:*c,d:*d,e:*e,f:*f},
   ("rotate",[deg])=>{let(si,co)=deg.to_radians().sin_cos();
       Affine{a:co,b:si,c:-si,d:co,e:0.,f:0.}},
   ("rotate",[deg,x,y])=>{let(si,co)=deg.to_radians().sin_cos();
       Affine{a:co,b:si,c:-si,d:co,e:*x-*x*co+*y*si,f:*y-*x*si-*y*co}},
   ("skewX",[deg])=>Affine{c:deg.to_radians().tan(),..Affine::identity()},
   ("skewY",[deg])=>Affine{b:deg.to_radians().tan(),..Affine::identity()},
   _=>return Err(unsupported("unsupported SVG transform expression")),
  };
  if ![op.a,op.b,op.c,op.d,op.e,op.f].iter().all(|v|v.is_finite()&&v.abs()<1e6){return Err(bad("SVG transform overflow"));}
  t=t.then(op);rest=rest[close+1..].trim_start_matches(|c:char|c.is_ascii_whitespace()||c==',');
 }
 Ok(t)
}
#[derive(Clone)]
struct SvgShape {
    // Each clip rule is a union of child shapes. Nested clip rules intersect.
    clips:Vec<Vec<(Vec<Vec<(f32,f32)>>,bool)>>,
    paths:Vec<Vec<(f32,f32)>>,
    fill:Paint,
    stroke:Paint,
    stroke_width:f32,
    evenodd:bool,
    clip_evenodd:bool,
}
/// Gradients are static and strictly local to the OpenType SVG document.
#[derive(Clone)]
struct Gradient {
    radial:bool,user_space:bool,
    coords:[f32;5],
    spread:u8,
    inverse_transform:Affine,
    stops:Vec<(f32,u32)>,
}
#[derive(Clone)]enum Paint {Solid(u32),Gradient(Gradient,f32)}
impl Paint {
 fn sample(&self,p:(f32,f32),bounds:[f32;4])->u32 {
    match self {
       Self::Solid(c)=>*c,
       Self::Gradient(grad,opacity)=>{
          let (x,y)=if grad.user_space{p}else{
             ((p.0-bounds[0])/bounds[2].max(0.001),
              (p.1-bounds[1])/bounds[3].max(0.001))
          };
          // SVG gradientTransform maps gradient space into user space.
          // Sample in its inverse to avoid incorrect rotated gradients.
          let (x,y)=grad.inverse_transform.apply((x,y));
          let [x1,y1,x2,y2,r]=grad.coords;
          let mut t=if grad.radial{
              // A radial gradient is parameterized along a ray originating at
              // its focal point, not simply by distance from the circle center.
              let (fx,fy)=(x2,y2);
              let (dx,dy)=(x-fx,y-fy);
              let (ox,oy)=(fx-x1,fy-y1);
              let a=dx*dx+dy*dy;
              if a<1e-10 {0.0} else {
                  let b=2.0*(dx*ox+dy*oy);
                  let c=ox*ox+oy*oy-r*r;
                  let discriminant=(b*b-4.0*a*c).max(0.0);
                  let hit=(-b+discriminant.sqrt())/(2.0*a);
                  if hit>1e-10 {1.0/hit}else{1.0}
              }
          }else{
              let (dx,dy)=(x2-x1,y2-y1);
              ((x-x1)*dx+(y-y1)*dy)/(dx*dx+dy*dy).max(1e-8)
          };
          t=match grad.spread{
             1=>t.rem_euclid(1.0),
             2=>{let u=t.rem_euclid(2.0);if u>1.0{2.0-u}else{u}},
             _=>t.clamp(0.0,1.0)
          };
          if !t.is_finite() {return 0;}
          let first=grad.stops.first().copied().unwrap_or((0.0,0));
          if t<=first.0{return alpha(first.1,*opacity);}
          for window in grad.stops.windows(2){
             let(a,b)=(window[0],window[1]);
             if t<=b.0{
                let f=((t-a.0)/(b.0-a.0).max(1e-6)).clamp(0.0,1.0);
                let mut c=0u32;
                for shift in [24u32,16,8,0]{
                   let av=(a.1>>shift)&255;let bv=(b.1>>shift)&255;
                   let value=(av as f32+(bv as f32-av as f32)*f).round() as u32;
                   c|=value.min(255)<<shift;
                }
                return alpha(c,*opacity);
             }
          }
          alpha(grad.stops.last().map(|s|s.1).unwrap_or(0),*opacity)
       }
    }
 }
 fn visible(&self)->bool{match self{Self::Solid(c)=>c&255!=0,Self::Gradient(_,opacity)=>*opacity>0.0}}
 fn with_opacity(self,opacity:f32)->Self{
    match self{Self::Solid(c)=>Self::Solid(alpha(c,opacity)),
         Self::Gradient(g,o)=>Self::Gradient(g,o*opacity)}
 }
}
fn percent(s:&str)->io::Result<f32>{
 let t=s.trim();if let Some(v)=t.strip_suffix('%'){Ok(number(v)?/100.0)}else{number(t)}
}
fn gradients(root:&Element)->io::Result<BTreeMap<String,Gradient>>{
    let mut nodes=Vec::new();root.descendants("linearGradient",&mut nodes);
    root.descendants("radialGradient",&mut nodes);
    if nodes.len()>256{return Err(bad("SVG gradient count exceeded"));}
    let mut definitions=BTreeMap::new();
    for node in nodes {
        if let Some(id)=node.attr("id") {
            if id.is_empty()||id.len()>256||definitions.insert(id.to_string(),node).is_some(){
                return Err(bad("SVG duplicate or invalid gradient ID"));
            }
        }
    }
    fn resolve(id:&str,definitions:&BTreeMap<String,&Element>,
               cache:&mut BTreeMap<String,Gradient>,active:&mut Vec<String>)->io::Result<Gradient>{
        if let Some(existing)=cache.get(id){return Ok(existing.clone());}
        if active.len()>32||active.iter().any(|a|a==id){
            return Err(bad("cyclic SVG gradient reference"));
        }
        let node=*definitions.get(id).ok_or_else(||bad("SVG gradient reference not found"))?;
        active.push(id.to_string());
        let parent=if let Some(href)=node.attr("href") {
            let linked=local_reference(href)?;
            Some(resolve(linked,definitions,cache,active)?)
        }else{None};
        active.pop();
        let radial=node.local()=="radialGradient";
        if parent.as_ref().is_some_and(|p|p.radial!=radial){
            return Err(unsupported("SVG cross-type gradient inheritance"));
        }
        let mut coords=parent.as_ref().map(|p|p.coords)
            .unwrap_or(if radial{[0.5,0.5,0.5,0.5,0.5]}else{[0.,0.,1.,0.,0.]});
        for (index,name) in (if radial{["cx","cy","fx","fy","r"]}
             else{["x1","y1","x2","y2","r"]}).iter().enumerate(){
            if let Some(value)=node.attr(name){coords[index]=percent(value)?;}
        }
        // When no focal center is given, SVG uses the circle center.
        if radial && parent.is_none() {
            if node.attr("fx").is_none(){coords[2]=coords[0];}
            if node.attr("fy").is_none(){coords[3]=coords[1];}
        }
        let user_space=match node.attr("gradientUnits"){
            None=>parent.as_ref().map(|p|p.user_space).unwrap_or(false),
            Some("userSpaceOnUse")=>true,Some("objectBoundingBox")=>false,
            _=>return Err(unsupported("unknown SVG gradient units")),
        };
        let spread=match node.attr("spreadMethod"){
            None=>parent.as_ref().map(|p|p.spread).unwrap_or(0),
            Some("pad")=>0,Some("repeat")=>1,Some("reflect")=>2,
            _=>return Err(unsupported("unknown SVG gradient spread")),
        };
        let inverse_transform=match node.attr("gradientTransform"){
            Some(value)=>transforms(value)?.inverse()?,
            None=>parent.as_ref().map(|p|p.inverse_transform).unwrap_or(Affine::identity()),
        };
        let mut stops=Vec::new();
        for item in node.elements("stop") {
            if stops.len()>=256{return Err(bad("SVG gradient stop limit"));}
            let offset=percent(item.attr("offset").unwrap_or("0"))?.clamp(0.,1.);
            let rgb=color(property(item,"stop-color").unwrap_or("black"))?;
            stops.push((offset,alpha(rgb,opacity(property(item,"stop-opacity"))?)));
        }
        if stops.is_empty(){
            stops=parent.as_ref().map(|p|p.stops.clone()).unwrap_or_default();
        }
        if stops.is_empty(){return Err(bad("SVG gradient missing color stops"));}
        let mut previous=0.0;
        for stop in &mut stops {stop.0=stop.0.max(previous);previous=stop.0;}
        let result=Gradient{radial,user_space,coords,spread,inverse_transform,stops};
        cache.insert(id.to_string(),result.clone());
        Ok(result)
    }
    let mut cache=BTreeMap::new();
    for id in definitions.keys(){resolve(id,&definitions,&mut cache,&mut Vec::new())?;}
    Ok(cache)
}
fn parse_paint(value:&str, defs:&BTreeMap<String,Gradient>,foreground:u32)->io::Result<Paint>{
 let value=value.trim();
 if value.eq_ignore_ascii_case("currentColor"){
     return Ok(Paint::Solid(foreground));
 }
 if let Some(id)=value.strip_prefix("url(#").and_then(|s|s.strip_suffix(')')){
    let gradient=defs.get(id).ok_or_else(||bad("unknown local SVG gradient"))?;
    return Ok(Paint::Gradient(gradient.clone(),1.0));
 }
 Ok(Paint::Solid(color(value)?))
}
fn alpha(color:u32,opacity:f32)->u32{
    let a=((color&255) as f32*opacity.clamp(0.0,1.0)).round() as u32;
    (color&!255)|a.min(255)
}
fn opacity(s:Option<&str>)->io::Result<f32>{
    let Some(text)=s else{return Ok(1.0)};
    let value=if let Some(percent)=text.trim().strip_suffix('%'){
        number(percent)?/100.0
    }else{number(text)?};
    Ok(value.clamp(0.0,1.0))
}
fn point_segment_distance(p:(f32,f32),a:(f32,f32),b:(f32,f32))->f32{
    let dx=b.0-a.0;let dy=b.1-a.1;
    let length=dx*dx+dy*dy;
    let t=if length<1e-8{0.0}else{
        ((p.0-a.0)*dx+(p.1-a.1)*dy).clamp(0.0,length)/length
    };
    let px=p.0-(a.0+t*dx);let py=p.1-(a.1+t*dy);
    (px*px+py*py).sqrt()
}
fn inside_evenodd(paths:&[Vec<(f32,f32)>],x:f32,y:f32)->bool{
    let mut intersections=0usize;
    for path in paths {
        if path.len()<2{continue;}
        for i in 0..path.len(){
            let (a,b)=(path[i],path[(i+1)%path.len()]);
            if (a.1>y)!=(b.1>y)&&x<(b.0-a.0)*(y-a.1)/(b.1-a.1)+a.0 {
                intersections+=1;
            }
        }
    }
    intersections%2!=0
}
fn shapes(node:&Element,fill:Paint,stroke:Paint,stroke_width:f32,opacity_factor:f32,
          out:&mut Vec<SvgShape>,depth:usize,
          parent:Affine,defs:&BTreeMap<String,Gradient>,foreground:u32,
          root:&Element,clips:&[Vec<(Vec<Vec<(f32,f32)>>,bool)>],active:&mut Vec<String>)->io::Result<()>{
 if depth>48{return Err(bad("SVG group nesting limit exceeded"));}
 if matches!(node.local(),"script"|"foreignObject"|"image"|"filter"|"text"|"a"){
  return Err(unsupported("active/external SVG content disabled in font glyph"));}
 let transform=parent.then(match node.attr("transform") {
    Some(value)=>transforms(value)?,None=>Affine::identity(),
 });
 let current=match property(node,"fill"){
    Some(value)=>parse_paint(value,defs,foreground)?,None=>fill};
 let current_stroke=match property(node,"stroke"){
    Some(value)=>parse_paint(value,defs,foreground)?,None=>stroke};
 let current_width=match property(node,"stroke-width"){
    Some(value)=>number(value)?.max(0.0),None=>stroke_width,
 };
 let group_opacity=opacity_factor*opacity(property(node,"opacity"))?;
 let fill_opacity=group_opacity*opacity(property(node,"fill-opacity"))?;
 let stroke_opacity=group_opacity*opacity(property(node,"stroke-opacity"))?;
 if property(node,"display")==Some("none") || matches!(property(node,"visibility"),Some("hidden"|"collapse")){
     return Ok(());
 }
 if property(node,"mask").is_some()||property(node,"filter").is_some(){
     return Err(unsupported("SVG glyph masks/filters are not supported"));
 }
 let evenodd=property(node,"fill-rule")==Some("evenodd");
 let clip_evenodd=property(node,"clip-rule").unwrap_or("nonzero")=="evenodd";
 let mut local_clips=clips.to_vec();
 if let Some(value)=property(node,"clip-path"){
     if value.trim()!="none" {
         let id=local_reference(value)?;
         if active.len()>32||active.iter().any(|a|a==id){return Err(bad("cyclic SVG clip-path reference"));}
         let clip=find_id(root,id).ok_or_else(||bad("unknown SVG clip-path reference"))?;
         if clip.local()!="clipPath" {return Err(bad("SVG clip reference is not clipPath"));}
         if matches!(clip.attr("clipPathUnits"),Some("objectBoundingBox")){
             return Err(unsupported("object-bounding-box SVG clip coordinates"));
         }
         active.push(id.to_string());
         let mut rendered=Vec::new();
         let clip_transform=transform.then(match clip.attr("transform"){
             Some(value)=>transforms(value)?,None=>Affine::identity()});
         let mut outcome=Ok(());
         for child in &clip.children{
             if let Node::Element(e)=child {
                 if let Err(error)=shapes(&e,Paint::Solid(0xffffffff),Paint::Solid(0),1.,1.,
                     &mut rendered,depth+1,clip_transform,defs,foreground,root,&[],active){
                     outcome=Err(error);break;
                 }
             }
         }
         active.pop();outcome?;
         if rendered.len()>256{return Err(bad("SVG clip-path shape limit"));}
         let rule=rendered.into_iter().map(|shape|(shape.paths,shape.clip_evenodd)).collect();
         local_clips.push(rule);
         if local_clips.len()>24{return Err(bad("SVG nested clip-path limit"));}
     }
 }
 // Local <use> references are expanded as geometry, with their own x/y and
 // transform, but never retrieve resources or run XML scripting.
 if node.local()=="use"{
     let href=node.attr("href").ok_or_else(||bad("SVG use missing href"))?;
     let id=local_reference(href)?;
     if active.len()>32||active.iter().any(|a|a==id){return Err(bad("cyclic SVG use reference"));}
     let target=find_id(root,id).ok_or_else(||bad("unknown SVG use reference"))?;
     let x=number(node.attr("x").unwrap_or("0"))?;
     let y=number(node.attr("y").unwrap_or("0"))?;
     active.push(id.to_string());
     let placement=transform.then(Affine{e:x,f:y,..Affine::identity()});
     let result=shapes(target,current,current_stroke,current_width,group_opacity,
         out,depth+1,placement,defs,foreground,root,&local_clips,active);
     active.pop();return result;
 }
 let mut paths=Vec::new();
 match node.local(){
  "svg"|"g"|"symbol"|"clipPath"=>{},
  "rect"=>{let x=number(node.attr("x").unwrap_or("0"))?;let y=number(node.attr("y").unwrap_or("0"))?;
    let w=number(node.attr("width").unwrap_or("0"))?;let h=number(node.attr("height").unwrap_or("0"))?;
    paths.push(vec![(x,y),(x+w,y),(x+w,y+h),(x,y+h),(x,y)]);},
  "circle"|"ellipse"=>{let x=number(node.attr("cx").unwrap_or("0"))?;let y=number(node.attr("cy").unwrap_or("0"))?;
    let r=if node.local()=="circle"{number(node.attr("r").unwrap_or("0"))?}else{0.};
    let rx=if r!=0.{r}else{number(node.attr("rx").unwrap_or("0"))?};
    let ry=if r!=0.{r}else{number(node.attr("ry").unwrap_or("0"))?};
    paths.push((0..=64).map(|i|{let a=std::f32::consts::TAU*i as f32/64.;(x+rx*a.cos(),y+ry*a.sin())}).collect());},
  "polygon"|"polyline"=>{let v=numbers(node.attr("points").unwrap_or(""))?;if v.len()%2!=0{return Err(bad("invalid SVG polyline point count"));}
    let mut p=v.chunks_exact(2).map(|a|(a[0],a[1])).collect::<Vec<_>>();
    if node.local()=="polygon"{if let Some(&first)=p.first(){p.push(first);}}
    paths.push(p);},
  "line"=>{let x1=number(node.attr("x1").unwrap_or("0"))?;
    let y1=number(node.attr("y1").unwrap_or("0"))?;
    let x2=number(node.attr("x2").unwrap_or("0"))?;
    let y2=number(node.attr("y2").unwrap_or("0"))?;
    paths.push(vec![(x1,y1),(x2,y2)]);},
  "path"=>paths=parse_path(node.attr("d").unwrap_or(""))?,
  "defs"|"title"|"desc"|"metadata"|"style"=>return Ok(()),
  "linearGradient"|"radialGradient"|"stop"=>return Ok(()),
  _=>return Err(unsupported("SVG glyph element not supported")),
 }
 if !paths.is_empty() {
     for contour in &mut paths {for vertex in contour { *vertex=transform.apply(*vertex); }}
     let area_scale=(transform.a*transform.d-transform.b*transform.c).abs().sqrt();
     if !area_scale.is_finite()||area_scale>1e6{return Err(bad("SVG stroke scale overflow"));}
     out.push(SvgShape {clips:local_clips.clone(),paths,fill:current.clone().with_opacity(fill_opacity),
         stroke:current_stroke.clone().with_opacity(stroke_opacity),
         stroke_width:current_width*area_scale,evenodd,clip_evenodd});
 }
 for child in &node.children{if let Node::Element(element)=child{shapes(element,current.clone(),current_stroke.clone(),current_width,
    group_opacity,out,depth+1,transform,defs,foreground,root,&local_clips,active)?;}}
 Ok(())
}
fn winding(paths:&[Vec<(f32,f32)>],x:f32,y:f32)->bool{
 let mut n=0i32;
 for path in paths {
  if path.len()<2{continue;}
  for i in 0..path.len() {
    let ((ax,ay),(bx,by))=(path[i],path[(i+1)%path.len()]);
    if ay<=y&&by>y{
      if (bx-ax)*(y-ay)-(x-ax)*(by-ay)>0.0{n+=1;}
    }else if by<=y&&ay>y{
      if (bx-ax)*(y-ay)-(x-ax)*(by-ay)<0.0{n-=1;}
    }
  }
 }n!=0
}
/// Bounded static SVG glyph rasterizer: transformed paths, elliptical arcs,
/// smooth Bézier curves, local linear/radial gradients and filled/stroked shapes.
/// External resources, filters and scripts are deliberately not executed.
pub fn rasterize(face:&Face,glyph:u16,ppem:f32)->io::Result<Option<ColorBitmap>>{
    rasterize_with_foreground(face,glyph,ppem,0x000000ff)
}
/// Render an OpenType SVG glyph, resolving `currentColor` to the document's
/// requested foreground instead of always using black.
pub fn rasterize_with_foreground(face:&Face,glyph:u16,ppem:f32,foreground:u32)
        ->io::Result<Option<ColorBitmap>>{
 let Some(document)=glyph_document(face,glyph)?else{return Ok(None)};
 if !ppem.is_finite()||ppem<=0.||ppem>2048.{return Err(bad("SVG glyph ppem outside limits"));}
 let mut root=xml::parse(&document.xml)?;
 apply_embedded_styles(&mut root)?;
 let default=[0.,0.,face.units_per_em as f32,face.units_per_em as f32];
 let vb=match root.attr("viewBox"){Some(v)=>{let values=numbers(v)?;
  if values.len()!=4{return Err(bad("invalid SVG viewBox"));}[values[0],values[1],values[2],values[3]]},None=>default};
 if vb[2]<=0.||vb[3]<=0.{return Err(bad("invalid SVG glyph viewBox size"));}
 let width=ppem.ceil() as usize;let height=(ppem*vb[3]/vb[2]).ceil() as usize;
 if width==0||height==0||height>4096||width.saturating_mul(height)>4_194_304{return Err(bad("SVG glyph raster dimensions exceed limit"));}
 let target_id=format!("glyph{glyph}");
 // An SVG document may contain several glyph IDs. Never render another
 // glyph's artwork as a fallback when the requested glyph is absent.
 let named=find_id(&root,&target_id);
 if named.is_none(){
     fn contains_glyph(node:&Element)->bool{
         if node.attr("id").is_some_and(|v|v.strip_prefix("glyph").is_some_and(|tail|
             !tail.is_empty()&&tail.bytes().all(|b|b.is_ascii_digit()))){return true;}
         node.children.iter().any(|child|matches!(child,Node::Element(e) if contains_glyph(e)))
     }
     if contains_glyph(&root){return Ok(None);}
 }
 let glyph_root=named.unwrap_or(&root);
 let defs=gradients(&root)?;
 let mut paths=Vec::new();let mut active=Vec::new();
 shapes(glyph_root,Paint::Solid(foreground),Paint::Solid(0),1.0,1.0,
         &mut paths,0,Affine::identity(),&defs,foreground,&root,&[],&mut active)?;
 let mut pixels=vec![0u32;width*height];
 let edges=paths.iter().map(|shape|shape.paths.iter().map(Vec::len).sum::<usize>()+
     shape.clips.iter().flatten().map(|(paths,_)|paths.iter().map(Vec::len).sum::<usize>()).sum::<usize>()).sum::<usize>();
 let work=width.saturating_mul(height).saturating_mul(edges).saturating_mul(4);
 if work>60_000_000{return Err(bad("SVG glyph raster work limit exceeded"));}
 for shape in paths {
  let scale_x=width as f32/vb[2];let scale_y=height as f32/vb[3];
  let geom=shape.paths.into_iter().map(|p|p.into_iter().map(|(x,y)|
    ((x-vb[0])*scale_x,(y-vb[1])*scale_y)).collect())
      .collect::<Vec<Vec<(f32,f32)>>>();
  let clip_geometry=shape.clips.iter().map(|rule|rule.iter().map(|(paths,evenodd)|{
      let pixels=paths.iter().map(|path|path.iter().map(|&(x,y)|
          ((x-vb[0])*scale_x,(y-vb[1])*scale_y)).collect()).collect();
      (pixels,*evenodd)
  }).collect::<Vec<(Vec<Vec<(f32,f32)>>,bool)>>()).collect::<Vec<_>>();
  let stroke_width=shape.stroke_width*(scale_x.abs()+scale_y.abs())*0.5;
  let bounds={
     let mut r=[f32::INFINITY,f32::INFINITY,f32::NEG_INFINITY,f32::NEG_INFINITY];
     for contour in &geom {for &(gx,gy) in contour {
       r[0]=r[0].min(vb[0]+gx/scale_x);
       r[1]=r[1].min(vb[1]+gy/scale_y);
       r[2]=r[2].max(vb[0]+gx/scale_x);
       r[3]=r[3].max(vb[1]+gy/scale_y);
     }}
     [r[0],r[1],r[2]-r[0],r[3]-r[1]]
  };
  for y in 0..height{for x in 0..width{
   let mut fill_hits=0u32;let mut stroke_hits=0u32;
   for (ox,oy) in [(0.25,0.25),(0.75,0.25),(0.25,0.75),(0.75,0.75)]{
    let sample=(x as f32+ox,y as f32+oy);
    let visible=clip_geometry.iter().all(|group|group.iter().any(|(p,evenodd)|{
        if *evenodd {inside_evenodd(p,sample.0,sample.1)}
        else{winding(p,sample.0,sample.1)}
    }));
    if !visible{continue;}
    let inside=if shape.evenodd{inside_evenodd(&geom,sample.0,sample.1)}
              else{winding(&geom,sample.0,sample.1)};
    if shape.fill.visible()&&inside{fill_hits+=1;}
    if shape.stroke.visible()&&stroke_width>0.0 {
      let mut covered=false;
      for contour in &geom {
       for segment in contour.windows(2){
        if point_segment_distance(sample,segment[0],segment[1])<=stroke_width*0.5 {
            covered=true;break;
        }
       }
       if covered{break;}
      }
      if covered{stroke_hits+=1;}
    }
   }
   let idx=y*width+x;
   let sample_svg=(vb[0]+(x as f32+0.5)/scale_x,
                   vb[1]+(y as f32+0.5)/scale_y);
   if fill_hits>0{
     let c=shape.fill.sample(sample_svg,bounds);
     let a=((c&255)*fill_hits+2)/4;
     pixels[idx]=blend_over(pixels[idx],(c&!255)|a);
   }
   if stroke_hits>0{
     let c=shape.stroke.sample(sample_svg,bounds);
     let a=((c&255)*stroke_hits+2)/4;
     pixels[idx]=blend_over(pixels[idx],(c&!255)|a);
   }
  }}
 }
 let advance=face.advance_units_instance(glyph)*ppem/face.units_per_em as f32;
 Ok(Some(ColorBitmap{width:width as u32,height:height as u32,pixels,
   bearing_x:(vb[0]*ppem/vb[2]).round() as i32,bearing_y:(-vb[1]*ppem/vb[3]).round() as i32,
   advance_px:advance}))
}

#[cfg(test)]
mod tests {
    use super::{parse_path,color};
    #[test]
    fn parse_closed_svg_triangle(){
        let paths=parse_path("M 0 0 L 10 0 L 10 10 Z").unwrap();
        assert_eq!(paths.len(),1);
        assert_eq!(paths[0].first(),Some(&(0.,0.)));
        assert_eq!(paths[0].last(),Some(&(0.,0.)));
    }
    #[test]
    fn svg_path_curves_and_arcs(){
        assert!(!parse_path("M0 0 C1 1 2 2 3 3 S4 4 5 5 A3 2 30 0 1 10 5 Z").unwrap().is_empty());
        assert!(!parse_path("M0 0 Q3 3 6 0 T12 0 Z").unwrap().is_empty());
    }
    #[test]
    fn stroke_segment_distance(){
        assert_eq!(super::point_segment_distance((2.,0.),(0.,0.),(4.,0.)),0.0);
        assert_eq!(super::point_segment_distance((2.,1.),(0.,0.),(4.,0.)),1.0);
    }
    #[test]
    fn svg_transform_matrix(){
        let t=super::transforms("translate(5 6) scale(2)").unwrap();
        assert_eq!(t.apply((1.,1.)),(7.,8.));
    }
    #[test]
    fn local_svg_gradient_interpolates_colors(){
        let root=crate::xml::parse(r##"<svg xmlns="http://www.w3.org/2000/svg"><defs>
         <linearGradient id="bar"><stop offset="0%" stop-color="#ff0000"/>
         <stop offset="100%" stop-color="#0000ff"/></linearGradient>
         </defs></svg>"##).unwrap();
        let defs=super::gradients(&root).unwrap();
        let p=super::parse_paint("url(#bar)",&defs,0x000000ff).unwrap();
        assert_eq!(p.sample((0.0,0.0),[0.0,0.0,1.0,1.0]),0xff0000ff);
        assert_eq!(p.sample((1.0,0.0),[0.0,0.0,1.0,1.0]),0x0000ffff);
        assert!(super::parse_paint("url(https://example.com/foo)",&defs,0x000000ff).is_err());
        assert_eq!(super::parse_paint("currentColor",&defs,0x336699ff).unwrap()
            .sample((0.,0.),[0.,0.,1.,1.]),0x336699ff);
    }
    #[test]
    fn svg_gradient_reference_inherits_stops(){
        let root=crate::xml::parse(r##"<svg xmlns="http://www.w3.org/2000/svg"
        xmlns:xlink="http://www.w3.org/1999/xlink">
        <linearGradient id="base"><stop offset="0" stop-color="#ff0000"/></linearGradient>
        <linearGradient id="copy" xlink:href="#base" x2="0.5"/>
        </svg>"##).unwrap();
        let defs=super::gradients(&root).unwrap();
        assert_eq!(defs["copy"].stops.len(),1);
        assert_eq!(defs["copy"].coords[2],0.5);
    }
    #[test]
    fn svg_gradient_transform_inverts_sample_coordinates(){
        let root=crate::xml::parse(r#"<svg xmlns="http://www.w3.org/2000/svg">
        <linearGradient id="a" gradientTransform="rotate(45)"><stop offset="0" stop-color="red"/></linearGradient>
        </svg>"#).unwrap();
        let definitions=super::gradients(&root).unwrap();
        assert!(definitions.get("a").is_some());
    }
    #[test]
    fn local_svg_use_clip_and_stylesheet(){
        let mut root=crate::xml::parse(r##"<svg xmlns="http://www.w3.org/2000/svg">
            <style>.t { fill: #ff0000; }</style>
            <defs>
                <path id="triangle" d="M 0 0 L 10 0 L 10 10 Z"/>
                <clipPath id="clip"><rect x="0" y="0" width="5" height="5"/></clipPath>
            </defs>
            <use href="#triangle" class="t" clip-path="url(#clip)"/>
        </svg>"##).unwrap();
        super::apply_embedded_styles(&mut root).unwrap();
        let defs=super::gradients(&root).unwrap();
        let mut out=Vec::new();let mut active=Vec::new();
        super::shapes(&root,super::Paint::Solid(0),super::Paint::Solid(0),1.0,1.0,
            &mut out,0,super::Affine::identity(),&defs,0,&root,&[],&mut active).unwrap();
        assert_eq!(out.len(),1);
        assert_eq!(out[0].clips.len(),1);
        assert_eq!(out[0].fill.sample((0.,0.),[0.,0.,10.,10.]),0xff0000ff);
    }
    #[test]
    fn local_svg_use_cycle_is_rejected(){
        let root=crate::xml::parse(r##"<svg xmlns="http://www.w3.org/2000/svg">
           <use id="loop" href="#loop"/>
        </svg>"##).unwrap();
        let mut out=Vec::new();let mut active=Vec::new();
        assert!(super::shapes(&root,super::Paint::Solid(0),super::Paint::Solid(0),1.0,1.0,
            &mut out,0,super::Affine::identity(),&std::collections::BTreeMap::new(),0,
            &root,&[],&mut active).is_err());
    }
    #[test]
    fn color_parsing_rejects_external_paint(){
        assert!(color("url(https://example.invalid/color)").is_err());
        assert_eq!(color("#ff00aa").unwrap(),0xff00aaff);
    }
}
