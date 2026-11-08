//! Small std-only OpenType metric and glyph shaping subsystem.
//!
//! Supports sfnt/OTF parsing, Unicode cmap formats 4/12, hmtx, legacy kern,
//! several GSUB and GPOS positioning lookups, HVAR metric interpolation and
//! fallback across caller-provided fonts. It is not a complete complex-script
//! engine: additional contextual formats, AAT, full Indic syllable shaping,
//! TrueType and CFF2 outlines are decoded in native Rust; advanced shaping remains incomplete.
use std::collections::{BTreeMap,BTreeSet};
use std::io::{self,Error,ErrorKind};
use crate::unicode::{self,Direction,Script};
fn bad(s:&str)->io::Error{Error::new(ErrorKind::InvalidData,s)}
fn u16be(b:&[u8],at:usize)->Option<u16>{Some(u16::from_be_bytes([*b.get(at)?,*b.get(at+1)?]))}
fn i16be(b:&[u8],at:usize)->Option<i16>{Some(i16::from_be_bytes([*b.get(at)?,*b.get(at+1)?]))}
fn u32be(b:&[u8],at:usize)->Option<u32>{Some(u32::from_be_bytes([*b.get(at)?,*b.get(at+1)?,*b.get(at+2)?,*b.get(at+3)?]))}
fn sub(b:&[u8],offset:usize)->Option<&[u8]>{b.get(offset..)}
fn tag_eq(data:&[u8],at:usize,tag:&[u8;4])->bool{data.get(at..at+4)==Some(tag.as_slice())}
fn sfnt_offset(data:&[u8],index:usize)->io::Result<usize>{
    if !data.starts_with(b"ttcf"){
        if index!=0{return Err(bad("ordinary OpenType font has only face index zero"));}
        return Ok(0);
    }
    let count=u32be(data,8).unwrap_or(0) as usize;
    if count==0||count>4096||index>=count{return Err(bad("invalid TTC collection face index"));}
    let position=12usize.checked_add(index.checked_mul(4).ok_or_else(||bad("TTC index overflow"))?).ok_or_else(||bad("TTC index overflow"))?;
    u32be(data,position).map(|offset|offset as usize).ok_or_else(||bad("truncated TTC offset table"))
}


#[derive(Debug,Clone,Copy)]pub struct VariationAxis{
    pub tag:[u8;4],pub minimum:f32,pub default:f32,pub maximum:f32,
}
#[derive(Debug,Clone)]pub struct Face{
    pub family:String,pub weight:u16,pub italic:bool,pub units_per_em:u16,pub ascent:i16,pub descent:i16,pub line_gap:i16,
    bytes:Vec<u8>,tables:BTreeMap<[u8;4],(usize,usize)>,number_hmetrics:usize,number_glyphs:usize,
    variation_values:BTreeMap<[u8;4],f32>,
}
impl Face{
    pub fn new(family:impl Into<String>,bytes:Vec<u8>)->io::Result<Self>{
        Self::new_with_index(family,bytes,0)
    }
    /// Read one face from a normal sfnt or an indexed TrueType Collection.
    pub fn new_with_index(family:impl Into<String>,bytes:Vec<u8>,font_index:usize)->io::Result<Self>{
        let at=sfnt_offset(&bytes,font_index)?;
        if at.checked_add(12).map(|n|n>bytes.len()).unwrap_or(true) ||
            !(bytes[at..].starts_with(&[0,1,0,0])||bytes[at..].starts_with(b"OTTO")||bytes[at..].starts_with(b"true")){
            return Err(bad("unsupported OpenType sfnt header"));
        }
        let count=u16be(&bytes,at+4).unwrap_or(0) as usize;
        if count>4096||at.saturating_add(12).saturating_add(count.saturating_mul(16))>bytes.len(){return Err(bad("invalid sfnt directory"));}
        let mut tables=BTreeMap::new();
        for i in 0..count{
            let at=at+12+i*16;let mut name=[0u8;4];name.copy_from_slice(&bytes[at..at+4]);
            let offset=u32be(&bytes,at+8).unwrap_or(0) as usize;
            let len=u32be(&bytes,at+12).unwrap_or(0) as usize;
            if offset.checked_add(len).map(|end|end<=bytes.len())!=Some(true){return Err(bad("font table exceeds file size"));}
            if tables.insert(name,(offset,len)).is_some(){return Err(bad("duplicate OpenType table"));}
        }
        let table=|key:[u8;4]|->io::Result<&[u8]>{
            let (start,size)=tables.get(&key).ok_or_else(||bad("required OpenType table missing"))?;
            Ok(&bytes[*start..*start+*size])
        };
        let head=table(*b"head")?;let hhea=table(*b"hhea")?;let maxp=table(*b"maxp")?;let cmap=table(*b"cmap")?;let hmtx=table(*b"hmtx")?;
        if head.len()<54||hhea.len()<36||maxp.len()<6||cmap.len()<4{return Err(bad("truncated required OpenType table"));}
        let upem=u16be(head,18).unwrap_or(0);let number_hmetrics=u16be(hhea,34).unwrap_or(0) as usize;
        let number_glyphs=u16be(maxp,4).unwrap_or(0) as usize;
        if upem==0||number_hmetrics==0||number_glyphs==0||number_hmetrics>number_glyphs||hmtx.len()<number_hmetrics*4+(number_glyphs-number_hmetrics)*2{
            return Err(bad("invalid OpenType glyph or horizontal metrics"));
        }
        let os2=table(*b"OS/2").ok();
        let weight=os2.and_then(|t|u16be(t,4)).unwrap_or(400);
        let italic=u16be(head,44).map(|flags|flags&2!=0).unwrap_or(false);
        Ok(Self{family:family.into(),weight,italic,units_per_em:upem,ascent:i16be(hhea,4).unwrap_or(0),descent:i16be(hhea,6).unwrap_or(0),
            line_gap:i16be(hhea,8).unwrap_or(0),bytes,tables,number_hmetrics,number_glyphs,
            variation_values:BTreeMap::new()})
    }
    /// Variable-font axis metadata from the fvar table. This is metadata:
    /// outline deltas (gvar/CFF2) are not evaluated by this metrics reader.
    pub fn variation_axes(&self)->Vec<VariationAxis>{
        let Some(t)=self.table(*b"fvar")else{return Vec::new();};
        let base=u16be(t,4).unwrap_or(0) as usize;
        let count=u16be(t,8).unwrap_or(0) as usize;
        let size=u16be(t,10).unwrap_or(0) as usize;
        if base<16||size<20||count>512{return Vec::new();}
        let mut result=Vec::new();
        for i in 0..count{
            let Some(at)=i.checked_mul(size).and_then(|v|base.checked_add(v))else{break;};
            let Some(bytes)=t.get(at..at+20)else{break;};
            let mut tag=[0u8;4];tag.copy_from_slice(&bytes[..4]);
            let read=|at:usize|u32be(bytes,at).map(|v|(v as i32 as f32)/65536.0).unwrap_or(0.0);
            result.push(VariationAxis{tag,minimum:read(4),default:read(8),maximum:read(12)});
        }
        result
    }
    /// Set a named font-axis design coordinate. Values are clipped to the
    /// fvar range. Rasterizers must receive the *same* coordinates to render
    /// matching outlines. This method updates metrics but not glyph contours.
    pub fn set_variation(&mut self,tag:[u8;4],value:f32)->io::Result<()> {
        let Some(axis)=self.variation_axes().into_iter().find(|a|a.tag==tag)else{
            return Err(bad("unknown variable-font axis"));
        };
        if !value.is_finite(){return Err(bad("non-finite variable-font coordinate"));}
        self.variation_values.insert(tag,value.clamp(axis.minimum,axis.maximum));
        Ok(())
    }
    pub fn variations(&self)->&BTreeMap<[u8;4],f32>{&self.variation_values}
    pub fn advance_units_instance(&self,glyph:u16)->f32 {
        let base=self.advance_units(glyph) as f32;
        if self.variation_values.is_empty(){return base;}
        let Some(hvar)=self.table(*b"HVAR")else{return base;};
        let delta=crate::variations::hvar_advance_delta(hvar,&self.variation_axes(),
            &self.variation_values,self.table(*b"avar"),glyph).unwrap_or(0.0);
        (base+delta).max(0.0)
    }
    /// hhea ascent, descent and line gap after the MVAR instance deltas.
    /// Does not invent interpolated outlines; the rasterizer must use the
    /// same variation coordinates as the shaping/metrics engine.
    pub fn vertical_metrics_instance(&self)->(f32,f32,f32){
        let base=(self.ascent as f32,self.descent as f32,self.line_gap as f32);
        if self.variation_values.is_empty(){return base;}
        let Some(mvar)=self.table(*b"MVAR")else{return base;};
        let axes=self.variation_axes();
        let avar=self.table(*b"avar");
        let adjust=|tag:[u8;4]|crate::variations::mvar_metric_delta(mvar,&axes,&self.variation_values,avar,tag).unwrap_or(0.0);
        (base.0+adjust(*b"hasc"),base.1+adjust(*b"hdsc"),base.2+adjust(*b"hlgp"))
    }
    /// Unhinted TrueType or CFF2 outline for a selected variable-font instance.
    /// This supports `glyf`/`gvar` and CFF2 interpolation and recursive
    /// composite glyphs, including point attachments, or CFF2 variable outlines.
    /// Consumers may rasterize the returned quadratic contours themselves.
    pub fn outline_instance(&self,glyph:u16)->io::Result<crate::glyph_variations::GlyphOutline>{
        if let Some(table)=self.table(*b"CFF2"){
            return crate::cff2::outline(table,glyph,&self.variation_axes(),&self.variation_values,self.table(*b"avar"));
        }
        let get=|tag:[u8;4]|self.table(tag).ok_or_else(||bad("TrueType outline table missing"));
        crate::glyph_variations::outline(get(*b"head")?,get(*b"loca")?,get(*b"glyf")?,
            self.table(*b"gvar"),glyph,&self.variation_axes(),&self.variation_values,
            self.table(*b"avar"))
    }
    /// Apply font-supplied TrueType bytecode to the variation-adjusted glyph
    /// outline at the requested pixels-per-em. Unsupported instruction opcodes
    /// produce `io::ErrorKind::Unsupported`, never an unhinted result presented
    /// as hinted. CFF2 outlines do not use the TrueType instruction set.
    pub fn hinted_glyph_instance(&self,glyph:u16,pixels_per_em:f32,
            limits:crate::truetype_hinting::HintLimits)
            ->io::Result<crate::truetype_hinting::HintedGlyph>{
        if self.table(*b"CFF2").is_some(){return Err(io::Error::new(ErrorKind::Unsupported,
            "TrueType bytecode hinting is not applicable to CFF2 outlines"));}
        let get=|tag:[u8;4]|self.table(tag).ok_or_else(||bad("hinting outline table missing"));
        let outline=self.outline_instance(glyph)?;
        let min_x=outline.points.iter().map(|p|p.x).reduce(f32::min).unwrap_or(0.);
        let max_y=outline.points.iter().map(|p|p.y).reduce(f32::max).unwrap_or(0.);
        let glyph_index=glyph as usize;
        let lsb=self.table(*b"hmtx").and_then(|data|{
            let offset=if glyph_index<self.number_hmetrics{glyph_index*4+2}
                else{self.number_hmetrics*4+(glyph_index-self.number_hmetrics)*2};
            i16be(data,offset)
        }).unwrap_or(0) as f32;
        let origin=min_x-lsb;
        let horizontal_advance=self.advance_units_instance(glyph);
        let top=self.ascent as f32;
        let bottom=top-self.units_per_em as f32;
        let phantoms=[(origin,0.),(origin+horizontal_advance,0.),(0.,top.max(max_y)),(0.,bottom)];
        crate::truetype_hinting::hint_outline_with_metrics(&outline,get(*b"head")?,get(*b"loca")?,get(*b"glyf")?,glyph,
            self.table(*b"fpgm"),self.table(*b"prep"),self.table(*b"cvt "),self.table(*b"maxp"),
            pixels_per_em,self.units_per_em,Some(phantoms),limits)
    }
    /// Return a hinted outline without the phantom-point metrics.
    pub fn outline_hinted_instance(&self,glyph:u16,pixels_per_em:f32,
            limits:crate::truetype_hinting::HintLimits)
            ->io::Result<crate::glyph_variations::GlyphOutline>{
        Ok(self.hinted_glyph_instance(glyph,pixels_per_em,limits)?.outline)
    }
    /// Rasterize the TrueType bytecode-hinted outline at this pixel size.
    pub fn rasterize_hinted_glyph_instance(&self,glyph:u16,pixels_per_em:f32,
            limits:crate::truetype_hinting::HintLimits)
            ->io::Result<crate::glyph_variations::OutlineBitmap>{
        let hinted=self.hinted_glyph_instance(glyph,pixels_per_em,limits)?;
        // A normal unrotated scaler activates dropout control only when
        // SCANCTRL enables the PPEM threshold. Other scan-conversion modes
        // continue through the regular coverage rasterizer.
        let control=hinted.scan_control;
        let threshold=(control&0xff) as f32;
        let dropout=(control&0x100)!=0 && (threshold==255.0||pixels_per_em<=threshold)
            && (control&0x800)==0;
        if dropout && matches!(hinted.scan_type,0|4){
            hinted.outline.rasterize_with_dropout(pixels_per_em,self.units_per_em,
                hinted.advance_width_units,hinted.scan_type)
        }else{
            hinted.outline.rasterize(pixels_per_em,self.units_per_em,hinted.advance_width_units)
        }
    }
    /// Read the UTF-8 SVG document stored for an OpenType SVG color glyph.
    pub fn svg_glyph_document(&self,glyph:u16)
            ->io::Result<Option<crate::svg_color::SvgGlyphDocument>>{
        crate::svg_color::glyph_document(self,glyph)
    }
    /// Inspect the encoded sbix image stored for a glyph, including raw JPEG
    /// or TIFF payloads that the std-only renderer cannot yet decode.
    pub fn sbix_image(&self,glyph:u16,pixels_per_em:f32)
            ->io::Result<Option<crate::bitmap_fonts::EmbeddedSbit>>{
        crate::bitmap_fonts::sbix_image(self,glyph,pixels_per_em)
    }
    /// Read ligature carets from GDEF for accurate cluster cursor positions.
    pub fn ligature_carets(&self,glyph:u16)->Vec<crate::gdef::CaretValue>{
        crate::gdef::ligature_carets(self,glyph)
    }
    /// Render OpenType COLR v0/CPAL layers at current variation coordinates.
    /// Pixels are packed 0xRRGGBBAA, unlike grayscale outline coverage.
    pub fn rasterize_colr_glyph_instance(&self,glyph:u16,pixels_per_em:f32,
            palette:usize,foreground:u32)->io::Result<Option<crate::colr::ColorBitmap>>{
        if let Some(colr)=crate::colr::rasterize(self,glyph,pixels_per_em,palette,foreground)?{
            return Ok(Some(colr));
        }
        if let Some(bitmap)=crate::bitmap_fonts::rasterize(self,glyph,pixels_per_em)?{
            return Ok(Some(bitmap));
        }
        match crate::svg_color::rasterize_with_foreground(self,glyph,pixels_per_em,foreground){
            Ok(Some(bitmap))=>return Ok(Some(bitmap)),
            Ok(None)=>{},
            Err(e) if e.kind()==ErrorKind::Unsupported=>{
                if let Some(bitmap)=crate::ebdt::rasterize(self,glyph,pixels_per_em)?{
                    return Ok(Some(bitmap));
                }
                return Err(e);
            },
            Err(e)=>return Err(e),
        }
        crate::ebdt::rasterize(self,glyph,pixels_per_em)
    }
    /// Rasterize one glyph ID using the instance-adjusted font outline.
    /// This handles glyphs created by GSUB, unlike scalar-only font APIs.
    pub fn rasterize_glyph_instance(&self,glyph:u16,pixels_per_em:f32)
        ->io::Result<crate::glyph_variations::OutlineBitmap>{
        let outline=self.outline_instance(glyph)?;
        outline.rasterize(pixels_per_em,self.units_per_em,self.advance_units_instance(glyph))
    }
    pub fn table(&self,key:[u8;4])->Option<&[u8]>{
        let (at,len)=*self.tables.get(&key)?;self.bytes.get(at..at+len)
    }
    pub fn glyph(&self,ch:char)->u16{
        let Some(cmap)=self.table(*b"cmap")else{return 0;};
        let n=u16be(cmap,2).unwrap_or(0) as usize;let code=ch as u32;
        let mut preferred=Vec::new();
        for i in 0..n.min(128){
            let at=4+i*8;let platform=u16be(cmap,at).unwrap_or(0);let encoding=u16be(cmap,at+2).unwrap_or(0);
            let off=u32be(cmap,at+4).unwrap_or(u32::MAX) as usize;
            let Some(t)=sub(cmap,off)else{continue;};let fmt=u16be(t,0).unwrap_or(0);
            if (platform==0 || (platform==3&&encoding==10))&&fmt==12{preferred.insert(0,t);}
            else if (platform==0||platform==3)&&fmt==4{preferred.push(t);}
        }
        for t in preferred{
            match u16be(t,0){
                Some(12) if t.len()>=16=>{
                    let groups=u32be(t,12).unwrap_or(0) as usize;
                    if groups>100_000 || t.len()<16+groups.saturating_mul(12){continue;}
                    let mut lo=0;let mut hi=groups;
                    while lo<hi{let mid=(lo+hi)/2;let at=16+mid*12;
                        let (start,end)=(u32be(t,at).unwrap_or(0),u32be(t,at+4).unwrap_or(0));
                        if code<start{hi=mid;}else if code>end{lo=mid+1;}else{
                            return (u32be(t,at+8).unwrap_or(0).wrapping_add(code-start)).try_into().ok().filter(|g:&u16|(*g as usize)<self.number_glyphs).unwrap_or(0);
                        }
                    }
                },
                Some(4) if code<=0xffff=>{
                    let segments=u16be(t,6).unwrap_or(0) as usize/2;
                    let start_base=16+2*segments;let delta_base=start_base+2*segments;
                    let offset_base=delta_base+2*segments;
                    if segments>8192||t.len()<offset_base+2*segments{continue;}
                    let mut lo=0;let mut hi=segments;
                    while lo<hi{let mid=(lo+hi)/2;
                        if code>u16be(t,14+mid*2).unwrap_or(0) as u32{lo=mid+1;}else{hi=mid;}
                    }
                    if lo<segments&&code>=u16be(t,start_base+lo*2).unwrap_or(u16::MAX) as u32{
                        let delta=i16be(t,delta_base+lo*2).unwrap_or(0) as u16;
                        let offset=u16be(t,offset_base+lo*2).unwrap_or(0) as usize;
                        let glyph=if offset==0{(code as u16).wrapping_add(delta)}else{
                            let at=offset_base+lo*2+offset+2*(code as usize-u16be(t,start_base+lo*2).unwrap_or(0) as usize);
                            let mapped=u16be(t,at).unwrap_or(0);
                            if mapped==0{0}else{mapped.wrapping_add(delta)}
                        };
                        if (glyph as usize)<self.number_glyphs{return glyph;}
                    }
                },_=>{}
            }
        }0
    }
    /// Unicode variation sequences from cmap format 14, such as VS15/VS16
    /// presentation selectors. A zero result means use the ordinary cmap.
    pub fn glyph_with_selector(&self,base:char,selector:char)->u16{
        let Some(cmap)=self.table(*b"cmap")else{return self.glyph(base);};
        let count=u16be(cmap,2).unwrap_or(0) as usize;
        for i in 0..count.min(128){
            let Some(start)=u32be(cmap,4+i*8+4).map(|v|v as usize)else{continue;};
            let Some(table)=sub(cmap,start)else{continue;};
            if u16be(table,0)!=Some(14){continue;}
            let size=u32be(table,2).unwrap_or(0) as usize;
            if size<10||size>table.len(){continue;}
            let table=&table[..size];
            let record_count=u32be(table,6).unwrap_or(0) as usize;
            if record_count>100_000{continue;}
            for r in 0..record_count{
                let at=10+r*11;
                let Some(bytes)=table.get(at..at+11)else{break;};
                let vs=((bytes[0] as u32)<<16)|((bytes[1] as u32)<<8)|bytes[2] as u32;
                if vs!=selector as u32{continue;}
                let Some(mapping)=u32be(table,at+7).map(|v|v as usize).filter(|v|*v>0)else{break;};
                let Some(n)=u32be(table,mapping).map(|v|v as usize)else{break;};
                if n>200_000{break;}
                let mut low=0;let mut high=n;
                while low<high{
                    let mid=(low+high)/2;
                    let Some(row)=mapping.checked_add(4+mid*5).and_then(|x|table.get(x..x+5))else{break;};
                    let code=((row[0] as u32)<<16)|((row[1] as u32)<<8)|row[2] as u32;
                    if code<base as u32{low=mid+1;}
                    else if code>base as u32{high=mid;}
                    else {let id=u16::from_be_bytes([row[3],row[4]]);
                        if (id as usize)<self.number_glyphs{return id;}
                        break;
                    }
                }
            }
        }
        self.glyph(base)
    }
    pub fn advance_units(&self,glyph:u16)->u16{
        let Some(t)=self.table(*b"hmtx")else{return 0;};
        let index=(glyph as usize).min(self.number_hmetrics.saturating_sub(1));u16be(t,index*4).unwrap_or(0)
    }
    pub fn kerning_units(&self,left:u16,right:u16)->i16{
        let Some(table)=self.table(*b"kern")else{return 0;};
        if table.len()<4{return 0;}
        let num=u16be(table,2).unwrap_or(0) as usize;let mut at=4;
        for _ in 0..num.min(100){
            if at+6>table.len(){break;}
            let len=u16be(table,at+2).unwrap_or(0) as usize;let coverage=u16be(table,at+4).unwrap_or(0);
            if len<14||at+len>table.len(){break;}
            if coverage & 0xff == 0 && coverage&0x0100==0{
                let records=u16be(table,at+6).unwrap_or(0) as usize;
                let key=((left as u32)<<16)|right as u32;
                let mut low=0;let mut high=records;
                while low<high {let mid=(low+high)/2;let offset=at+14+mid*6;
                    if offset+6>at+len{break;}
                    let pair=u32be(table,offset).unwrap_or(0);
                    if pair<key{low=mid+1;}else{high=mid;}
                }
                let offset=at+14+low*6;
                if low<records&&offset+6<=at+len&&u32be(table,offset)==Some(key){return i16be(table,offset+4).unwrap_or(0);}
            }
            at+=len;
        }0
    }
}
fn coverage(data:&[u8],offset:usize,glyph:u16)->Option<usize>{
    let t=sub(data,offset)?;let n=u16be(t,2)? as usize;if n>65535{return None;}
    match u16be(t,0)?{
        1=>{
            for i in 0..n{if u16be(t,4+i*2)==Some(glyph){return Some(i);}}None
        },
        2=>{
            for i in 0..n{let at=4+i*6;let a=u16be(t,at)?;let b=u16be(t,at+2)?;
                if glyph>=a&&glyph<=b{return Some(u16be(t,at+4)? as usize+(glyph-a) as usize);}
            }None
        },_=>None
    }
}
fn classdef(data:&[u8],offset:usize,glyph:u16)->usize{
    let Some(t)=sub(data,offset)else{return 0;};
    match u16be(t,0){
        Some(1)=>{
            let start=u16be(t,2).unwrap_or(0);let n=u16be(t,4).unwrap_or(0);
            if glyph<start||glyph>=start.saturating_add(n){0}else{u16be(t,6+2*(glyph-start) as usize).unwrap_or(0) as usize}
        },
        Some(2)=>{
            let n=u16be(t,2).unwrap_or(0) as usize;
            for i in 0..n.min(65535){let at=4+i*6;
                if let (Some(a),Some(b),Some(class))=(u16be(t,at),u16be(t,at+2),u16be(t,at+4)){
                    if glyph>=a&&glyph<=b{return class as usize;}
                }else{break;}
            }0
        },_=>0
    }
}
fn script_at(text:&str,at:usize)->Script{
    let ch=text.get(at..).and_then(|s|s.chars().next());
    let raw=ch.map(unicode::script).unwrap_or(Script::Latin);
    if raw!=Script::Common{return raw;}
    // Punctuation and combining signs are resolved from the surrounding
    // strong script, not treated as Latin merely because they share a font.
    if let Some(previous)=text.get(..at){
        for c in previous.chars().rev(){let script=unicode::script(c);
            if script!=Script::Common{return script;}}
    }
    if let Some(next)=text.get(at..){
        for c in next.chars(){let script=unicode::script(c);
            if script!=Script::Common{return script;}}
    }
    Script::Latin
}
fn script_tag(script:Script)->[u8;4]{match script{
    Script::Arabic=>*b"arab",Script::Hebrew=>*b"hebr",Script::Devanagari=>*b"deva",Script::Han=>*b"hani",
    Script::Hangul=>*b"hang",Script::Cyrillic=>*b"cyrl",
    Script::Bengali=>*b"beng",Script::Gurmukhi=>*b"guru",Script::Gujarati=>*b"gujr",
    Script::Oriya=>*b"orya",Script::Tamil=>*b"taml",Script::Telugu=>*b"telu",
    Script::Kannada=>*b"knda",Script::Malayalam=>*b"mlym",Script::Thai=>*b"thai",
    Script::Lao=>*b"lao ",Script::Khmer=>*b"khmr",Script::Myanmar=>*b"mymr",
    Script::Syriac=>*b"syrc",_=>*b"latn",
}}
/// OpenType 1.1 FeatureVariations chooses the first condition set matching
/// normalized fvar/avar coordinates. Returned offsets reference alternate
/// FeatureTables within the FeatureTableSubstitution subtable.
fn substituted_features(table:&[u8],face:&Face)->BTreeMap<usize,usize>{
    let mut substitutions=BTreeMap::new();
    if u16be(table,0)!=Some(1)||u16be(table,2).unwrap_or(0)<1{return substitutions;}
    let Some(root)=u32be(table,10).map(|n|n as usize).filter(|n|*n>0)else{return substitutions;};
    let Some(t)=sub(table,root)else{return substitutions;};
    if u16be(t,0)!=Some(1){return substitutions;}
    let count=u32be(t,4).unwrap_or(0) as usize;
    if count>32768{return substitutions;}
    let axes=face.variation_axes();
    let coords=crate::variations::normalized(&axes,face.variations(),face.table(*b"avar"));
    for i in 0..count{
        let Some(cond_off)=u32be(t,8+8*i).map(|n|n as usize)else{break;};
        let Some(subst_off)=u32be(t,12+8*i).map(|n|n as usize)else{break;};
        let Some(cond)=sub(t,cond_off)else{continue;};
        let n=u16be(cond,0).unwrap_or(0) as usize;
        if n>1024{continue;}
        let mut matches=true;
        for j in 0..n{
            let Some(off)=u32be(cond,2+j*4).map(|n|n as usize)else{matches=false;break;};
            let Some(c)=sub(cond,off)else{matches=false;break;};
            if u16be(c,0)!=Some(1){matches=false;break;}
            let axis=u16be(c,2).unwrap_or(u16::MAX) as usize;
            let (Some(&v),Some(min),Some(max))=(coords.get(axis),i16be(c,4),i16be(c,6))else{matches=false;break;};
            if v<min as f32/16384.0||v>max as f32/16384.0{matches=false;break;}
        }
        if !matches{continue;}
        let Some(alt)=sub(t,subst_off)else{break;};
        if u16be(alt,0)!=Some(1){break;}
        let n=u16be(alt,4).unwrap_or(0) as usize;
        for j in 0..n.min(8192){
            let (Some(index),Some(off))=(u16be(alt,6+j*6),u32be(alt,8+j*6))else{break;};
            let Some(at)=subst_off.checked_add(off as usize)else{continue;};
            if sub(t,at).is_some(){substitutions.insert(index as usize,root+at);}
        }
        // First matching record wins, even if no requested feature changed.
        break;
    }
    substitutions
}
fn script_tags(script:Script)->Vec<[u8;4]>{
    let modern=match script{
        Script::Devanagari=>Some(*b"dev2"),Script::Bengali=>Some(*b"bng2"),
        Script::Gurmukhi=>Some(*b"gur2"),Script::Gujarati=>Some(*b"gjr2"),
        Script::Oriya=>Some(*b"ory2"),Script::Tamil=>Some(*b"tml2"),
        Script::Telugu=>Some(*b"tel2"),Script::Kannada=>Some(*b"knd2"),
        Script::Malayalam=>Some(*b"mlm2"),Script::Myanmar=>Some(*b"mym2"),_=>None,
    };
    let mut tags=Vec::new();if let Some(tag)=modern{tags.push(tag);}tags.push(script_tag(script));
    tags.extend([*b"DFLT",*b"latn"]);tags
}
/// Lookup offsets selected by ScriptList->LangSys->FeatureList->LookupList.
fn lookups(face:&Face,table:&[u8],script:Script,wanted:&[[u8;4]],language:Option<[u8;4]>)->Vec<(u16,usize)> {
    let script_list=u16be(table,4).unwrap_or(0) as usize;
    let feature_list=u16be(table,6).unwrap_or(0) as usize;
    let lookup_list=u16be(table,8).unwrap_or(0) as usize;
    let Some(scripts)=sub(table,script_list)else{return Vec::new();};
    let Some(features)=sub(table,feature_list)else{return Vec::new();};
    let Some(lookup_table)=sub(table,lookup_list)else{return Vec::new();};
    let mut script_off=None;let count=u16be(scripts,0).unwrap_or(0) as usize;
    for target in script_tags(script){
        for i in 0..count.min(256){let at=2+i*6;
            if tag_eq(scripts,at,&target){script_off=u16be(scripts,at+4).map(|x|x as usize);break;}
        }
        if script_off.is_some(){break;}
    }
    let Some(st)=script_off.and_then(|at|sub(scripts,at))else{return Vec::new();};
    let default_lang=u16be(st,0).unwrap_or(0) as usize;
    // A Script table can omit DefaultLangSys. Use its first declared
    // language system rather than accidentally treating the Script header
    // itself as a LangSys table.
    let mut lang_off=None;
    if let Some(language)=language{
        let count=u16be(st,2).unwrap_or(0) as usize;
        for i in 0..count.min(256){
            let at=4+i*6;
            if tag_eq(st,at,&language){
                lang_off=u16be(st,at+4).map(|n|n as usize).filter(|n|*n>0);
                break;
            }
        }
    }
    // If the requested language isn't present, use DefaultLangSys, or the
    // first named language system when the script defines no default.
    let lang_off=lang_off.or_else(||if default_lang!=0{Some(default_lang)}else{
        (u16be(st,2).unwrap_or(0)>0).then(||u16be(st,8).unwrap_or(0) as usize)
    }).unwrap_or(0);
    if lang_off==0{return Vec::new();}
    let Some(lang)=sub(st,lang_off)else{return Vec::new();};
    // OpenType feature execution order is semantically important for scripts
    // that form reph, half-forms and post-base ligatures. The FeatureList is
    // sorted alphabetically, NOT by shaping order: keep the caller's order.
    let mut requested_features=BTreeSet::new();
    let required=u16be(lang,2).unwrap_or(0xffff);
    if required!=0xffff{requested_features.insert(required as usize);}
    let n=u16be(lang,4).unwrap_or(0) as usize;
    for i in 0..n.min(1024){if let Some(f)=u16be(lang,6+i*2){requested_features.insert(f as usize);}}
    let nfeatures=u16be(features,0).unwrap_or(0) as usize;
    let mut ordered_features=Vec::new();
    if required!=0xffff && (required as usize)<nfeatures {ordered_features.push(required as usize);}
    for tag in wanted{
        for &i in &requested_features{
            if i>=nfeatures{continue;}
            if tag_eq(features,2+i*6,tag) && !ordered_features.contains(&i){ordered_features.push(i);}
        }
    }
    let mut lookup_ids=Vec::new();
    let variants=substituted_features(table,face);
    for i in ordered_features{
        let record=2+i*6;
        let at=u16be(features,record+4).unwrap_or(0) as usize;
        let Some(feat)=variants.get(&i).and_then(|off|sub(table,*off)).or_else(||sub(features,at))else{continue;};let size=u16be(feat,2).unwrap_or(0) as usize;
        for j in 0..size.min(1024){if let Some(id)=u16be(feat,4+j*2){
            if !lookup_ids.contains(&(id as usize)){lookup_ids.push(id as usize);}
        }}
    }
    let mut out=Vec::new();
    for i in lookup_ids{
        let n=u16be(lookup_table,0).unwrap_or(0) as usize;
        if i>=n{continue;}
        let Some(offset)=u16be(lookup_table,2+i*2)else{continue;};
        let at=offset as usize;let Some(subtable)=sub(lookup_table,at)else{continue;};
        let ty=u16be(subtable,0).unwrap_or(0);let n=u16be(subtable,4).unwrap_or(0) as usize;
        for j in 0..n.min(2048){if let Some(off)=u16be(subtable,6+j*2){
            let Some(absolute)=lookup_list.checked_add(at).and_then(|v|v.checked_add(off as usize))else{continue;};
            if absolute<table.len(){out.push((ty,absolute));}
        }}
    }
    out
}
#[derive(Debug,Clone)]pub struct Glyph{
    pub glyph_id:u16,pub cluster:usize,pub face_index:usize,
    pub advance_pt:f32,pub x_offset_pt:f32,pub y_offset_pt:f32,
}
#[derive(Debug,Clone)]pub struct ShapedRun{pub glyphs:Vec<Glyph>,pub direction:Direction,pub width_pt:f32}
#[derive(Debug,Clone,Copy)]pub struct ShapeOptions{pub font_size_pt:f32,pub direction:Direction,pub ligatures:bool,pub kerning:bool,pub weight:u16,pub italic:bool,
    /// OpenType four-byte LangSys tag, e.g. `*b"URD "` (Urdu).
    /// Falls back to DefaultLangSys when the requested tag is absent.
    pub language:Option<[u8;4]>}
impl Default for ShapeOptions{fn default()->Self{Self{font_size_pt:11.0,direction:Direction::Auto,ligatures:true,kerning:true,weight:400,italic:false,language:None}}}
#[derive(Debug,Default,Clone)]pub struct FontCollection{faces:Vec<Face>}
impl FontCollection{
    pub fn new()->Self{Self::default()}
    pub fn add(&mut self,family:impl Into<String>,bytes:Vec<u8>)->io::Result<usize>{
        let face=Face::new(family,bytes)?;let index=self.faces.len();self.faces.push(face);Ok(index)
    }
    pub fn faces(&self)->&[Face]{&self.faces}
    /// Select a named variation axis for an installed font face.
    pub fn set_variation(&mut self,face_index:usize,tag:[u8;4],value:f32)->io::Result<()> {
        self.faces.get_mut(face_index).ok_or_else(||bad("invalid font face index"))?.set_variation(tag,value)
    }

    pub fn add_index(&mut self,family:impl Into<String>,bytes:Vec<u8>,index:usize)->io::Result<usize>{
        let face=Face::new_with_index(family,bytes,index)?;
        let at=self.faces.len();self.faces.push(face);Ok(at)
    }
    pub fn add_collection(&mut self,family:impl Into<String>,bytes:Vec<u8>)->io::Result<Vec<usize>>{
        let count=if bytes.starts_with(b"ttcf"){u32be(&bytes,8).unwrap_or(0) as usize}else{1};
        if count==0||count>256{return Err(bad("invalid number of TTC faces"));}
        let name=family.into();let mut faces=Vec::new();
        for i in 0..count{faces.push(Face::new_with_index(name.clone(),bytes.clone(),i)?);}
        let mut indices=Vec::new();for face in faces{indices.push(self.faces.len());self.faces.push(face);}
        Ok(indices)
    }
    pub fn select(&self,ch:char,preferred:Option<&str>)->Option<usize>{
        if let Some(name)=preferred{
            if let Some((i,_))=self.faces.iter().enumerate().find(|(_,f)|f.family.eq_ignore_ascii_case(name)&&f.glyph(ch)!=0){return Some(i);}
        }
        self.faces.iter().position(|f|f.glyph(ch)!=0)
    }
    /// Select a single font for an entire grapheme cluster, preventing a
    /// combining mark from drifting to an unrelated fallback face.
    pub fn select_cluster(&self,cluster:&str,preferred:Option<&str>)->Option<usize>{
        self.select_cluster_style(cluster,preferred,400,false)
    }
    pub fn select_cluster_style(&self,cluster:&str,preferred:Option<&str>,weight:u16,italic:bool)->Option<usize>{
        let supports=|face:&Face|cluster.chars().all(|c|{
            face.glyph(c)!=0 || matches!(c,'\u{200d}'|'\u{fe0f}'|'\u{fe0e}')
        });
        let mut matches=self.faces.iter().enumerate().filter(|(_,f)|supports(f)).collect::<Vec<_>>();
        matches.sort_by_key(|(_,f)|{
            let family_penalty=if preferred.map(|name|f.family.eq_ignore_ascii_case(name)).unwrap_or(true){0u32}else{100_000};
            family_penalty+(f.weight.abs_diff(weight)) as u32+
                if f.italic==italic{0}else{1000}
        });
        matches.first().map(|(i,_)|*i)
    }
    pub fn shape(&self,text:&str,preferred:Option<&str>,options:ShapeOptions)->ShapedRun{
        let mut glyphs=Vec::new();let font_size=options.font_size_pt.max(1.0);
        let scalars=text.char_indices().collect::<Vec<_>>();
        let clusters=unicode::graphemes(text);
        let mut owners=BTreeMap::new();
        for cluster in &clusters{
            let index=self.select_cluster_style(cluster.text,preferred,options.weight,options.italic);
            for (byte,_) in text[cluster.byte_range.0..cluster.byte_range.1].char_indices(){
                owners.insert(byte+cluster.byte_range.0,index);
            }
        }
        for (index,&(byte,c)) in scalars.iter().enumerate(){
            // Presentation selectors and ZWJ control how a grapheme is shaped;
            // they are not separately rendered glyphs with an advance.
            if matches!(c,'\u{fe0e}'|'\u{fe0f}'|'\u{200d}') {continue;}
            let face_index=owners.get(&byte).copied().flatten().or_else(||self.select(c,preferred)).unwrap_or(usize::MAX);
            let (id,advance)=if let Some(face)=self.faces.get(face_index){
                let contextual=arabic_form(&scalars,index);
                let id=contextual.map(|c|face.glyph(c)).filter(|id|*id!=0).unwrap_or_else(||{
                    if let Some(next)=scalars.get(index+1).map(|(_,next)|*next).filter(|c|matches!(c,'\u{fe0e}'|'\u{fe0f}')){
                        face.glyph_with_selector(c,next)
                    }else{face.glyph(c)}
                });
                let size=face.advance_units_instance(id)/face.units_per_em as f32*font_size;
                (id,if is_mark(c){0.0}else{size})
            }else{(0,if c.is_whitespace(){font_size*0.33}else if matches!(unicode::script(c),Script::Han|Script::Hangul){font_size}else{font_size*0.54})};
            glyphs.push(Glyph{glyph_id:id,cluster:byte,face_index,advance_pt:advance,x_offset_pt:0.0,y_offset_pt:0.0});
        }
        // GSUB runs never cross a font boundary, avoiding mixed font glyph IDs.
        if options.ligatures{
            let mut start=0;while start<glyphs.len(){
                let idx=glyphs[start].face_index;
                let script=script_at(text,glyphs[start].cluster);
                let mut end=start+1;
                while end<glyphs.len() && glyphs[end].face_index==idx && script_at(text,glyphs[end].cluster)==script{end+=1;}
                if crate::indic::syllabic_script(script){
                    let stop=crate::indic::syllable_end(text,glyphs[start].cluster);
                    end=(start+1..end).find(|&i|glyphs[i].cluster>=stop).unwrap_or(end);
                }
                if let Some(face)=self.faces.get(idx){gsub(face,&mut glyphs,start,end,script,font_size,options.language);}
                let mut j=start+1;
                while j<glyphs.len()&&glyphs[j].face_index==idx&&script_at(text,glyphs[j].cluster)==script{j+=1;}
                if crate::indic::syllabic_script(script){
                    let stop=crate::indic::syllable_end(text,glyphs[start].cluster);
                    j=(start+1..j).find(|&i|glyphs[i].cluster>=stop).unwrap_or(j);
                }
                start=j;
            }
        }
        // Reorder Indic pre-base signs after syllable substitutions and before
        // mark attachment/positioning. Source cluster offsets remain stable.
        reorder_indic_prebase(text,&mut glyphs);
        // Selected GPOS SinglePos records adjust the initial pen position and
        // glyph advance before pair adjustments and mark attachment.
        for glyph in &mut glyphs{
            let Some(face)=self.faces.get(glyph.face_index)else{continue};
            let script=script_at(text,glyph.cluster);
            if let Some((dx,dy,da,_))=gpos_single(face,glyph.glyph_id,script,options.language){
                let factor=font_size/face.units_per_em as f32;
                glyph.x_offset_pt+=dx as f32*factor;
                glyph.y_offset_pt-=dy as f32*factor;
                glyph.advance_pt+=da as f32*factor;
            }
        }
        // Attach combining marks to their base, rather than treating each
        // mark as a zero-width glyph aligned at the next pen position.
        for i in 1..glyphs.len(){
            let idx=glyphs[i].face_index;
            let Some(face)=self.faces.get(idx)else{continue;};
            let Some(ch)=text.get(glyphs[i].cluster..).and_then(|s|s.chars().next())else{continue;};
            if !is_mark(ch){continue;}
            let mut base=i-1;
            while base>0 && glyphs[base].face_index==idx {
                let before=text.get(glyphs[base].cluster..).and_then(|s|s.chars().next());
                if !before.map(is_mark).unwrap_or(false){break;}
                base-=1;
            }
            if glyphs[base].face_index!=idx{continue;}
            let script=script_at(text,glyphs[base].cluster);
            if let Some((dx,dy))=gpos_mark(face,glyphs[base].glyph_id,glyphs[i].glyph_id,script,options.language){
                let factor=font_size/face.units_per_em as f32;
                let between=glyphs[base..i].iter().map(|g|g.advance_pt).sum::<f32>();
                glyphs[i].x_offset_pt=dx as f32*factor-between;
                glyphs[i].y_offset_pt=-(dy as f32)*factor;
            }
        }
        if options.kerning{
            for i in 0..glyphs.len().saturating_sub(1){
                if glyphs[i].face_index!=glyphs[i+1].face_index{continue;}
                let Some(face)=self.faces.get(glyphs[i].face_index)else{continue;};
                let script=script_at(text,glyphs[i].cluster);
                let pair=gpos_pair(face,glyphs[i].glyph_id,glyphs[i+1].glyph_id,script,options.language).unwrap_or_else(||face.kerning_units(glyphs[i].glyph_id,glyphs[i+1].glyph_id));
                glyphs[i].advance_pt+=pair as f32/face.units_per_em as f32*font_size;
            }
        }
        // Cursive joining uses the actual advance and pen offsets after
        // SinglePos and PairPos. A right glyph's entry anchor is aligned with
        // the preceding glyph's exit anchor in font design coordinates.
        for i in 0..glyphs.len().saturating_sub(1){
            let idx=glyphs[i].face_index;
            if idx!=glyphs[i+1].face_index{continue;}
            let Some(face)=self.faces.get(idx)else{continue;};
            let script=script_at(text,glyphs[i].cluster);
            if let Some((dx,dy))=gpos_cursive(face,glyphs[i].glyph_id,glyphs[i+1].glyph_id,script,options.language){
                let factor=font_size/face.units_per_em as f32;
                let desired=dx as f32*factor+glyphs[i].x_offset_pt-glyphs[i+1].x_offset_pt;
                glyphs[i].advance_pt=desired;
                glyphs[i+1].y_offset_pt=glyphs[i].y_offset_pt-dy as f32*factor;
            }
        }
        // Apply nested ContextPos/ChainContextPos lookup records before the
        // visual-order pass, while glyph indices are still logical.
        let mut start=0;
        while start<glyphs.len(){
            let idx=glyphs[start].face_index;
            let script=script_at(text,glyphs[start].cluster);
            let mut end=start+1;
            while end<glyphs.len() && glyphs[end].face_index==idx && script_at(text,glyphs[end].cluster)==script{end+=1;}
            if let Some(face)=self.faces.get(idx){
                gpos_context(face,&mut glyphs,start,end,script,font_size,options.language);
            }
            start=end;
        }
        let direction=unicode::base_direction(text,options.direction);
        {
            // Resolve visual runs even when the paragraph begins LTR and
            // contains embedded RTL script. Bidi is not limited to RTL pages.
            let order=unicode::visual_order(text,direction);let clusters=unicode::graphemes(text);
            let mut rank=BTreeMap::new();for (i,cluster) in order.into_iter().enumerate(){rank.insert(clusters[cluster].byte_range.0,i);}
            for glyph in &mut glyphs{
                if let Some((cluster,_))=clusters.iter().enumerate().find(|(_,c)|c.byte_range.0<=glyph.cluster&&glyph.cluster<c.byte_range.1){
                    glyph.cluster=clusters[cluster].byte_range.0;
                }
            }
            glyphs.sort_by_key(|g|rank.get(&g.cluster).copied().unwrap_or(usize::MAX));
        }
        let width_pt=glyphs.iter().map(|g|g.advance_pt).sum();ShapedRun{glyphs,direction,width_pt}
    }
}
/// Place pre-base vowel signs before the base glyph in an Indic syllable.
/// Glyphs keep their original source byte-cluster for caret positions and
/// bidi reordering. GSUB is applied first because many fonts substitute
/// the consonant/vowel-sign pair before the visual-order move.
fn reorder_indic_prebase(text:&str,glyphs:&mut Vec<Glyph>){
    let prebase=|c:char|matches!(c as u32,
        0x093f|0x09bf|0x0a3f|0x0abf|0x0b3f|0x0bbf|0x0c3f|0x0cbf|0x0d3f);
    for (start,end) in crate::indic::syllable_ranges(text){
        if !text[start..end].chars().any(prebase){continue;}
        let (Some(first),Some(last))=(glyphs.iter().position(|g|g.cluster>=start&&g.cluster<end),
            glyphs.iter().rposition(|g|g.cluster>=start&&g.cluster<end))else{continue;};
        if last<=first{continue;}
        let mut at=first+1;
        while at<=last && at<glyphs.len(){
            let mark=text.get(glyphs[at].cluster..).and_then(|s|s.chars().next()).map(prebase).unwrap_or(false);
            if mark{
                let glyph=glyphs.remove(at);
                glyphs.insert(first,glyph);
                // This mark was moved to the syllable start, so the next
                // glyph remains at the current original position.
            }else{at+=1;}
        }
    }
}
fn is_mark(c:char)->bool{
    let code=c as u32;
    crate::unicode::matches_property(c,"M") || matches!(code,0x200c|0x200d)
}
fn gsub(face:&Face,glyphs:&mut Vec<Glyph>,start:usize,end:usize,script:Script,size:f32,language:Option<[u8;4]>){
    let Some(data)=face.table(*b"GSUB")else{return;};
    let matches=lookups(face,data,script,&[*b"locl",*b"ccmp",*b"nukt",*b"akhn",*b"rphf",*b"rkrf",*b"pref",*b"blwf",*b"half",*b"pstf",*b"vatu",*b"cjct",*b"init",*b"medi",*b"fina",*b"isol",*b"rlig",*b"rclt",*b"calt",*b"liga",*b"clig",*b"pres",*b"abvs",*b"blws",*b"psts",*b"haln"],language);
    let mut bound=end.min(glyphs.len());
    for (mut ty,absolute) in matches{
        let Some(mut t)=sub(data,absolute)else{continue;};
        if ty==7 { // GSUB ExtensionSubst
            if u16be(t,0)!=Some(1){continue;}
            ty=u16be(t,2).unwrap_or(0);
            let Some(off)=u32be(t,4)else{continue;};let Some(subtable)=sub(t,off as usize)else{continue;};t=subtable;
        }
        let flags=crate::gdef::flags_for_subtable(data,absolute);
        match ty{
            2=>{ // MultipleSubst, format 1: one glyph becomes a sequence.
                if u16be(t,0)!=Some(1){continue;}
                let cov=u16be(t,2).unwrap_or(0) as usize;
                let count=u16be(t,4).unwrap_or(0) as usize;
                let mut i=start;
                while i<bound {
                    if flags.ignores(face,glyphs[i].glyph_id){i+=1;continue;}
                    let Some(index)=coverage(t,cov,glyphs[i].glyph_id)else{i+=1;continue;};
                    if index>=count{i+=1;continue;}
                    let Some(offset)=u16be(t,6+index*2)else{i+=1;continue;};
                    let Some(sequence)=sub(t,offset as usize)else{i+=1;continue;};
                    let length=u16be(sequence,0).unwrap_or(0) as usize;
                    if length==0||length>128{i+=1;continue;}
                    let ids=(0..length).map(|j|u16be(sequence,2+j*2)).collect::<Option<Vec<_>>>();
                    let Some(ids)=ids.filter(|ids|ids.iter().all(|id|(*id as usize)<face.number_glyphs))else{i+=1;continue;};
                    let old=glyphs[i].clone();
                    let new=ids.into_iter().map(|id|Glyph{glyph_id:id,advance_pt:face.advance_units_instance(id)/face.units_per_em as f32*size,..old.clone()}).collect::<Vec<_>>();
                    glyphs.splice(i..i+1,new);
                    bound=bound.saturating_add(length-1);
                    i+=length;
                }
            },
            3=>{ // AlternateSubst, format 1: deterministic first alternate.
                if u16be(t,0)!=Some(1){continue;}
                let cov=u16be(t,2).unwrap_or(0) as usize;
                let count=u16be(t,4).unwrap_or(0) as usize;
                for glyph in &mut glyphs[start..bound]{
                    if flags.ignores(face,glyph.glyph_id){continue;}
                    let Some(index)=coverage(t,cov,glyph.glyph_id)else{continue;};
                    if index>=count{continue;}
                    let Some(offset)=u16be(t,6+index*2)else{continue;};
                    let Some(alt)=sub(t,offset as usize)else{continue;};
                    if u16be(alt,0).unwrap_or(0)==0{continue;}
                    if let Some(id)=u16be(alt,2).filter(|id|(*id as usize)<face.number_glyphs){
                        glyph.glyph_id=id;glyph.advance_pt=face.advance_units_instance(id)/face.units_per_em as f32*size;
                    }
                }
            },
            1=>{
                let fmt=u16be(t,0).unwrap_or(0);let cov=u16be(t,2).unwrap_or(0) as usize;
                for glyph in &mut glyphs[start..bound]{
                    if flags.ignores(face,glyph.glyph_id){continue;}
                    if let Some(index)=coverage(t,cov,glyph.glyph_id){
                        let replaced=match fmt{
                            1=>Some(glyph.glyph_id.wrapping_add(i16be(t,4).unwrap_or(0) as u16)),
                            2=>u16be(t,6+index*2),_=>None,
                        };
                        if let Some(id)=replaced{if (id as usize)<face.number_glyphs{
                            glyph.glyph_id=id;glyph.advance_pt=face.advance_units_instance(id)/face.units_per_em as f32*size;
                        }}
                    }
                }
            },
            4=>{
                if u16be(t,0)!=Some(1){continue;}
                let cov=u16be(t,2).unwrap_or(0) as usize;
                let sets=u16be(t,4).unwrap_or(0) as usize;
                let mut i=start;
                while i<bound{
                    if flags.ignores(face,glyphs[i].glyph_id){i+=1;continue;}
                    let Some(index)=coverage(t,cov,glyphs[i].glyph_id)else{i+=1;continue;};
                    if index>=sets{i+=1;continue;}
                    let Some(set_off)=u16be(t,6+index*2)else{i+=1;continue;};
                    let Some(set)=sub(t,set_off as usize)else{i+=1;continue;};
                    let count=u16be(set,0).unwrap_or(0) as usize;
                    let mut best:Option<(u16,Vec<usize>)>=None;
                    for j in 0..count.min(2048){
                        let Some(off)=u16be(set,2+j*2)else{continue;};
                        let Some(lig)=sub(set,off as usize)else{continue;};
                        let (Some(id),Some(length))=(u16be(lig,0),u16be(lig,2))else{continue;};
                        let length=length as usize;
                        if length<2||length>128||id as usize>=face.number_glyphs{continue;}
                        let mut positions=vec![i];let mut next=i+1;
                        let mut matched=true;
                        for k in 1..length{
                            while next<bound&&flags.ignores(face,glyphs[next].glyph_id){next+=1;}
                            if next>=bound||u16be(lig,4+(k-1)*2)!=Some(glyphs[next].glyph_id){matched=false;break;}
                            positions.push(next);next+=1;
                        }
                        if matched&&best.as_ref().map(|(_,items)|length>items.len()).unwrap_or(true){
                            best=Some((id,positions));
                        }
                    }
                    if let Some((id,positions))=best{
                        glyphs[i].glyph_id=id;
                        glyphs[i].advance_pt=face.advance_units_instance(id)/face.units_per_em as f32*size;
                        // Preserve ignored GDEF marks in the run; remove only
                        // the matched ligature components, in reverse order.
                        for &at in positions.iter().skip(1).rev(){glyphs.remove(at);}
                        bound-=positions.len()-1;
                    }
                    i+=1;
                }
            },
            5|6|8=>{
                if ty==8{
                    // ReverseChainSingleSubst is defined right-to-left even
                    // when the surrounding text direction is left-to-right.
                    for i in (start..bound).rev(){
                        if i<glyphs.len()&&!flags.ignores(face,glyphs[i].glyph_id){apply_substitution_at(face,glyphs,i,&mut bound,ty,t,size,0);}
                    }
                }else{
                    let mut i=start;
                    while i<bound && i<glyphs.len(){
                        if !flags.ignores(face,glyphs[i].glyph_id){apply_substitution_at(face,glyphs,i,&mut bound,ty,t,size,0);}
                        i+=1;
                    }
                }
            },
            _=>{}
        }
    }
}
/// Resolve a lookup by its *lookup-list index*, as required by contextual
/// SubstLookupRecords. The feature-selected lookup list cannot be used here:
/// it is a filtered list and its indices differ from the font's lookup list.
fn lookup_by_index(table:&[u8],id:usize)->Vec<(u16,usize)>{
    let list=u16be(table,8).unwrap_or(0) as usize;
    let Some(list_table)=sub(table,list)else{return Vec::new()};
    let count=u16be(list_table,0).unwrap_or(0) as usize;
    if id>=count||count>4096{return Vec::new();}
    let Some(base)=u16be(list_table,2+id*2).map(|n|n as usize)else{return Vec::new()};
    let Some(lookup)=sub(list_table,base)else{return Vec::new()};
    let ty=u16be(lookup,0).unwrap_or(0);
    let n=u16be(lookup,4).unwrap_or(0) as usize;
    let mut out=Vec::new();
    for i in 0..n.min(1024){
        let Some(offset)=u16be(lookup,6+i*2).map(|v|v as usize)else{continue};
        let Some(abs)=list.checked_add(base).and_then(|n|n.checked_add(offset))else{continue};
        let Some(mut t)=sub(table,abs)else{continue};
        if ty==7{
            if u16be(t,0)!=Some(1){continue;}
            let kind=u16be(t,2).unwrap_or(0);
            let Some(a)=u32be(t,4).and_then(|off|abs.checked_add(off as usize))else{continue};
            t=sub(table,a).unwrap_or_default();
            if !t.is_empty(){out.push((kind,a));}
        }else{out.push((ty,abs));}
    }
    out
}
/// Apply a contextual lookup to precisely one input glyph. All offsets are
/// checked; the depth/length limits protect against self-referential fonts.
fn apply_substitution_at(face:&Face,glyphs:&mut Vec<Glyph>,at:usize,bound:&mut usize,
                         kind:u16,t:&[u8],size:f32,depth:usize)->bool{
    if depth>=8||at>=*bound||*bound>glyphs.len(){return false;}
    let format=u16be(t,0).unwrap_or(0);
    if matches!(kind,5|6){return apply_context_at(face,glyphs,at,bound,kind,t,size,depth+1);}
    let Some(index)=u16be(t,2).and_then(|off|coverage(t,off as usize,glyphs[at].glyph_id))else{return false};
    match kind{
        1=>{
            let id=match format{
                1=>glyphs[at].glyph_id.wrapping_add(i16be(t,4).unwrap_or(0) as u16),
                2=>u16be(t,6+index*2).unwrap_or(u16::MAX),_=>return false,
            };
            if id as usize>=face.number_glyphs{return false;}
            glyphs[at].glyph_id=id;
            glyphs[at].advance_pt=face.advance_units_instance(id)*size/face.units_per_em as f32;
            true
        },
        2 if format==1=>{
            let Some(seq)=u16be(t,6+index*2).and_then(|off|sub(t,off as usize))else{return false};
            let count=u16be(seq,0).unwrap_or(0) as usize;
            if count==0||count>64||glyphs.len().saturating_add(count)>100_000{return false;}
            let Some(ids)=(0..count).map(|i|u16be(seq,2+i*2)).collect::<Option<Vec<_>>>()else{return false};
            if ids.iter().any(|id|*id as usize>=face.number_glyphs){return false;}
            let original=glyphs[at].clone();
            glyphs.splice(at..at+1,ids.into_iter().map(|id|Glyph{
                glyph_id:id,advance_pt:face.advance_units_instance(id)*size/face.units_per_em as f32,..original.clone()
            }));
            *bound+=count-1;true
        },
        3 if format==1=>{
            let Some(alt)=u16be(t,6+index*2).and_then(|off|sub(t,off as usize))else{return false};
            let Some(id)=u16be(alt,2).filter(|id|(*id as usize)<face.number_glyphs)else{return false};
            glyphs[at].glyph_id=id;
            glyphs[at].advance_pt=face.advance_units_instance(id)*size/face.units_per_em as f32;
            true
        },
        4 if format==1=>{
            let Some(set)=u16be(t,6+index*2).and_then(|off|sub(t,off as usize))else{return false};
            let count=u16be(set,0).unwrap_or(0) as usize;
            let mut best=None;
            for i in 0..count.min(512){
                let Some(lig)=u16be(set,2+i*2).and_then(|off|sub(set,off as usize))else{continue};
                let (Some(id),Some(n))=(u16be(lig,0),u16be(lig,2))else{continue};
                let n=n as usize;
                if n<2||at+n>*bound||id as usize>=face.number_glyphs{continue;}
                if (1..n).all(|j|u16be(lig,4+2*(j-1))==Some(glyphs[at+j].glyph_id))
                    && best.map(|(_,old)|n>old).unwrap_or(true){best=Some((id,n));}
            }
            let Some((id,n))=best else{return false};
            glyphs[at].glyph_id=id;
            glyphs[at].advance_pt=face.advance_units_instance(id)*size/face.units_per_em as f32;
            glyphs.drain(at+1..at+n);
            *bound-=n-1;true
        },
        5|6=>apply_context_at(face,glyphs,at,bound,kind,t,size,depth+1),
        8 if format==1=>{
            // Reverse chaining contextual single substitution; all coverage
            // offsets must match in their specified backtrack/lookahead order.
            let mut cursor=4usize;
            let back=u16be(t,cursor).unwrap_or(0) as usize;cursor+=2;
            if back>256{return false;}
            for j in 0..back{
                if at<=j{return false;}
                let Some(off)=u16be(t,cursor+2*j)else{return false};
                if coverage(t,off as usize,glyphs[at-j-1].glyph_id).is_none(){return false;}
            }
            cursor+=back*2;
            let ahead=u16be(t,cursor).unwrap_or(0) as usize;cursor+=2;
            if ahead>256{return false;}
            for j in 0..ahead{
                if at+j+1>=*bound{return false;}
                let Some(off)=u16be(t,cursor+2*j)else{return false};
                if coverage(t,off as usize,glyphs[at+j+1].glyph_id).is_none(){return false;}
            }
            cursor+=ahead*2;
            let count=u16be(t,cursor).unwrap_or(0) as usize;cursor+=2;
            if index>=count{return false;}
            let Some(id)=u16be(t,cursor+index*2).filter(|v|(*v as usize)<face.number_glyphs)else{return false};
            glyphs[at].glyph_id=id;
            glyphs[at].advance_pt=face.advance_units_instance(id)*size/face.units_per_em as f32;
            true
        },
        _=>false,
    }
}
/// Interpret a SequenceContext or ChainedSequenceContext rule. Format 1
/// compares glyph IDs; format 2 compares class definitions. Backtrack entries
/// are stored in nearest-first order by OpenType. All counts are bounded.
fn context_rule_records(rule:&[u8],kind:u16,class_based:bool,definitions:(&[u8],&[u8],&[u8]),
                        glyphs:&[Glyph],at:usize,bound:usize)->Option<(usize,Vec<(usize,usize)>)>{
    let mut cursor=0usize;
    let read_count=|cursor:&mut usize|->Option<usize>{
        let n=u16be(rule,*cursor)? as usize;*cursor=(*cursor).checked_add(2)?;
        if n>256{None}else{Some(n)}
    };
    let matches=|actual:u16,expected:u16,definition:&[u8]|->bool{
        if class_based{classdef(definition,0,actual)==expected as usize}else{actual==expected}
    };
    if kind==6{
        let n=read_count(&mut cursor)?;
        if n>at{return None;}
        for j in 0..n{
            let expected=u16be(rule,cursor.checked_add(2*j)?)?;
            if !matches(glyphs[at-1-j].glyph_id,expected,definitions.0){return None;}
        }
        cursor=cursor.checked_add(n.checked_mul(2)?)?;
    }
    let n=read_count(&mut cursor)?;
    if n==0||at.checked_add(n)?>bound{return None;}
    for j in 1..n{
        let expected=u16be(rule,cursor.checked_add(2*(j-1))?)?;
        if !matches(glyphs[at+j].glyph_id,expected,definitions.1){return None;}
    }
    cursor=cursor.checked_add((n-1).checked_mul(2)?)?;
    if kind==6{
        let ahead=read_count(&mut cursor)?;
        if at.checked_add(n)?.checked_add(ahead)?>bound{return None;}
        for j in 0..ahead{
            let expected=u16be(rule,cursor.checked_add(2*j)?)?;
            if !matches(glyphs[at+n+j].glyph_id,expected,definitions.2){return None;}
        }
        cursor=cursor.checked_add(ahead.checked_mul(2)?)?;
    }
    let records=read_count(&mut cursor)?;
    let mut out=Vec::with_capacity(records);
    for j in 0..records{
        let base=cursor.checked_add(j.checked_mul(4)?)?;
        let position=u16be(rule,base)? as usize;
        let lookup=u16be(rule,base+2)? as usize;
        if position>=n{return None;}
        out.push((position,lookup));
    }
    Some((n,out))
}
/// GSUB context/chaining formats 1 (glyph rules), 2 (class rules), and
/// 3 (coverage rules). SubstLookupRecord indices address the original input
/// sequence, not the filtered list of enabled feature lookups.
fn context_match(glyphs:&[Glyph],at:usize,bound:usize,kind:u16,t:&[u8])->Option<(usize,Vec<(usize,usize)>)>{
    let format=u16be(t,0)?;
    let cov=u16be(t,2)? as usize;
    let first=coverage(t,cov,glyphs.get(at)?.glyph_id)?;
    if matches!(format,1|2){
        let class_based=format==2;
        let (set_count_at,sets_at,definitions)=if kind==5{
            if class_based{
                let input=t.get(u16be(t,4)? as usize..)?;
                (6usize,8usize,(&[][..],input,&[][..]))
            }else{(4usize,6usize,(&[][..],&[][..],&[][..]))}
        }else if class_based{
            let back=t.get(u16be(t,4)? as usize..)?;
            let input=t.get(u16be(t,6)? as usize..)?;
            let ahead=t.get(u16be(t,8)? as usize..)?;
            (10usize,12usize,(back,input,ahead))
        }else{(4usize,6usize,(&[][..],&[][..],&[][..]))};
        let set_index=if class_based{classdef(definitions.1,0,glyphs[at].glyph_id)}else{first};
        let sets=u16be(t,set_count_at)? as usize;
        if set_index>=sets||sets>4096{return None;}
        let offset=u16be(t,sets_at.checked_add(set_index.checked_mul(2)?)?)? as usize;
        if offset==0{return None;}
        let set=sub(t,offset)?;
        let count=u16be(set,0)? as usize;
        if count>4096{return None;}
        for j in 0..count{
            let off=u16be(set,2+j*2)? as usize;
            if off==0{continue;}
            if let Some(rule)=sub(set,off){
                if let Some(found)=context_rule_records(rule,kind,class_based,definitions,glyphs,at,bound){
                    return Some(found);
                }
            }
        }
        return None;
    }
    if format!=3{return None;}
    let mut pos=2usize;
    if kind==6{
        let back=u16be(t,pos)? as usize;pos+=2;
        if back>256||back>at{return None;}
        for j in 0..back{
            let off=u16be(t,pos.checked_add(j.checked_mul(2)?)?)? as usize;
            if coverage(t,off,glyphs[at-1-j].glyph_id).is_none(){return None;}
        }
        pos=pos.checked_add(back.checked_mul(2)?)?;
    }
    let n=u16be(t,pos)? as usize;pos+=2;
    if n==0||n>256||at.checked_add(n)?>bound{return None;}
    for j in 0..n{
        let off=u16be(t,pos.checked_add(j.checked_mul(2)?)?)? as usize;
        if coverage(t,off,glyphs[at+j].glyph_id).is_none(){return None;}
    }
    pos=pos.checked_add(n.checked_mul(2)?)?;
    if kind==6{
        let ahead=u16be(t,pos)? as usize;pos+=2;
        if ahead>256||at.checked_add(n)?.checked_add(ahead)?>bound{return None;}
        for j in 0..ahead{
            let off=u16be(t,pos.checked_add(j.checked_mul(2)?)?)? as usize;
            if coverage(t,off,glyphs[at+n+j].glyph_id).is_none(){return None;}
        }
        pos=pos.checked_add(ahead.checked_mul(2)?)?;
    }
    let count=u16be(t,pos)? as usize;pos+=2;
    if count>256{return None;}
    let mut out=Vec::with_capacity(count);
    for j in 0..count{
        let base=pos.checked_add(j.checked_mul(4)?)?;
        let sequence=u16be(t,base)? as usize;
        let lookup=u16be(t,base+2)? as usize;
        if sequence>=n{return None;}
        out.push((sequence,lookup));
    }
    Some((n,out))
}
fn apply_context_at(face:&Face,glyphs:&mut Vec<Glyph>,at:usize,bound:&mut usize,
                    kind:u16,t:&[u8],size:f32,depth:usize)->bool{
    if depth>=8||!matches!(kind,5|6){return false;}
    let Some((_input_count,records))=context_match(glyphs,at,*bound,kind,t)else{return false;};
    let Some(table)=face.table(*b"GSUB")else{return false};
    let mut result=false;
    for (index,lookup) in records{
        if at+index>=*bound{continue;}
        for (ty,off) in lookup_by_index(table,lookup){
            let Some(subtable)=sub(table,off)else{continue};
            if apply_substitution_at(face,glyphs,at+index,bound,ty,subtable,size,depth+1){result=true;break;}
        }
    }
    result
}


/// A GPOS lookup list uses ExtensionPos (type 9), unlike GSUB's type 7.
fn gpos_lookup_by_index(table:&[u8],id:usize)->Vec<(u16,usize)>{
    let list=u16be(table,8).unwrap_or(0) as usize;
    let Some(root)=sub(table,list)else{return Vec::new()};
    let count=u16be(root,0).unwrap_or(0) as usize;
    if count>4096 || id>=count{return Vec::new();}
    let Some(lookup_off)=u16be(root,2+id*2).map(|v|v as usize)else{return Vec::new()};
    let Some(lookup)=sub(root,lookup_off)else{return Vec::new()};
    let kind=u16be(lookup,0).unwrap_or(0);
    let n=u16be(lookup,4).unwrap_or(0) as usize;
    let mut out=Vec::new();
    for i in 0..n.min(1024){
        let Some(off)=u16be(lookup,6+i*2).map(|v|v as usize)else{continue};
        let Some(abs)=list.checked_add(lookup_off).and_then(|v|v.checked_add(off))else{continue};
        let Some(t)=sub(table,abs)else{continue};
        if kind==9{
            if u16be(t,0)!=Some(1){continue;}
            let ty=u16be(t,2).unwrap_or(0);
            let Some(ext)=u32be(t,4).and_then(|v|abs.checked_add(v as usize))else{continue};
            if sub(table,ext).is_some(){out.push((ty,ext));}
        }else{out.push((kind,abs));}
    }
    out
}
fn add_gpos_value(glyph:&mut Glyph,value:(i16,i16,i16,i16),factor:f32){
    glyph.x_offset_pt+=value.0 as f32*factor;
    glyph.y_offset_pt-=value.1 as f32*factor;
    glyph.advance_pt+=value.2 as f32*factor;
}
/// Apply a single GPOS lookup at an exact logical glyph index. A nested
/// ContextPos lookup may invoke more contextual lookups but recursion is
/// bounded to avoid malicious cyclic font tables.
fn gpos_at(face:&Face,all:&mut [Glyph],at:usize,end:usize,
           kind:u16,t:&[u8],full:&[u8],factor:f32,depth:usize)->bool{
    if depth>=8||at>=end||end>all.len(){return false;}
    let fmt=u16be(t,0).unwrap_or(0);
    if matches!(kind,7|8){
        let mapped=if kind==7{5}else{6};
        let Some((_,records))=context_match(all,at,end,mapped,t)else{return false;};
        let mut done=false;
        for (index,lookup) in records{
            let Some(target)=at.checked_add(index).filter(|v|*v<end)else{continue;};
            for (ty,off) in gpos_lookup_by_index(full,lookup){
                if let Some(next)=sub(full,off){
                    if gpos_at(face,all,target,end,ty,next,full,factor,depth+1){done=true;break;}
                }
            }
        }
        return done;
    }
    let Some(index)=u16be(t,2).and_then(|v|coverage(t,v as usize,all[at].glyph_id))else{return false;};
    match kind{
        1=>{
            let format=u16be(t,4).unwrap_or(0);
            let value=match fmt{
                1=>value_xy_adv(t,6,format),
                2=>{
                    if index>=u16be(t,6).unwrap_or(0) as usize{None}
                    else{value_xy_adv(t,8+index*value_record_size(format),format)}
                },
                _=>None,
            };
            if let Some(value)=value{add_gpos_value(&mut all[at],value,factor);true}else{false}
        },
        2 if at+1<end && all[at].face_index==all[at+1].face_index=>{
            let f1=u16be(t,4).unwrap_or(0);let f2=u16be(t,6).unwrap_or(0);
            let (sz1,sz2)=(value_record_size(f1),value_record_size(f2));
            let pair=match fmt{
                1=>{
                    let count=u16be(t,8).unwrap_or(0) as usize;
                    if index>=count{None}else{
                        let pairset=u16be(t,10+index*2).and_then(|off|sub(t,off as usize));
                        pairset.and_then(|pairs|{
                            let n=u16be(pairs,0)? as usize;
                            let stride=2usize.checked_add(sz1)?.checked_add(sz2)?;
                            (0..n.min(65535)).find_map(|i|{
                                let off=2usize.checked_add(i.checked_mul(stride)?)?;
                                if u16be(pairs,off)==Some(all[at+1].glyph_id){
                                    Some((value_xy_adv(pairs,off+2,f1)?,value_xy_adv(pairs,off+2+sz1,f2)?))
                                }else{None}
                            })
                        })
                    }
                },
                2=>{
                    let a=classdef(t,u16be(t,8).unwrap_or(0) as usize,all[at].glyph_id);
                    let b=classdef(t,u16be(t,10).unwrap_or(0) as usize,all[at+1].glyph_id);
                    let n=u16be(t,12).unwrap_or(0) as usize;
                    let m=u16be(t,14).unwrap_or(0) as usize;
                    if a>=n||b>=m{None}else{
                        let off=a.checked_mul(m).and_then(|v|v.checked_add(b))
                            .and_then(|v|v.checked_mul(sz1+sz2)).and_then(|v|v.checked_add(16));
                        off.and_then(|off|Some((value_xy_adv(t,off,f1)?,value_xy_adv(t,off+sz1,f2)?)))
                    }
                },
                _=>None,
            };
            if let Some((left,right))=pair{
                add_gpos_value(&mut all[at],left,factor);
                add_gpos_value(&mut all[at+1],right,factor);
                true
            }else{false}
        },
        _=>false,
    }
}
/// Apply font-defined contextual positioning for the calt/kern/dist/mark
/// feature sets. Scalar-based positioning and direct mark/kerning operations
/// are handled earlier; only ContextPos/ChainContextPos subtables are visited
/// here to avoid double-applying their noncontextual lookups.
fn gpos_context(face:&Face,glyphs:&mut [Glyph],start:usize,end:usize,script:Script,size:f32,language:Option<[u8;4]>){
    let Some(table)=face.table(*b"GPOS")else{return;};
    let factor=size/face.units_per_em as f32;
    let selected=lookups(face,table,script,&[*b"calt",*b"kern",*b"dist",*b"mark",*b"mkmk",*b"curs",*b"abvm",*b"blwm"],language);
    for (mut ty,off) in selected{
        let Some(mut t)=sub(table,off)else{continue;};
        if ty==9{
            if u16be(t,0)!=Some(1){continue;}
            ty=u16be(t,2).unwrap_or(0);
            let Some(v)=u32be(t,4).and_then(|delta|sub(t,delta as usize))else{continue;};
            t=v;
        }
        if !matches!(ty,7|8){continue;}
        for at in start..end{
            gpos_at(face,glyphs,at,end,ty,t,table,factor,0);
        }
    }
}

fn value_record_size(format:u16)->usize{format.count_ones() as usize*2}
fn xadvance(t:&[u8],offset:usize,format:u16)->Option<i16>{
    let mut at=offset;for bit in 0..16{
        if format&(1<<bit)!=0{
            let value=i16be(t,at)?;
            if bit==2{return Some(value);}
            at+=2;
        }
    }Some(0)
}
fn gpos_pair(face:&Face,left:u16,right:u16,script:Script,language:Option<[u8;4]>)->Option<i16>{
    let data=face.table(*b"GPOS")?;
    let matches=lookups(face,data,script,&[*b"kern",*b"dist"],language);
    for (mut ty,off) in matches{
        let mut t=sub(data,off)?;
        if ty==9 { // ExtensionPos
            if u16be(t,0)!=Some(1){continue;}
            ty=u16be(t,2).unwrap_or(0);t=sub(t,u32be(t,4)? as usize)?;
        }
        if ty!=2 {continue;}
        let format=u16be(t,0)?;let cov=u16be(t,2)? as usize;
        if coverage(t,cov,left).is_none(){continue;}
        let v1=u16be(t,4)?;let v2=u16be(t,6)?;
        let sz1=value_record_size(v1);let sz2=value_record_size(v2);
        if format==1{
            let coverage_index=coverage(t,cov,left)?;
            let count=u16be(t,8)? as usize;if coverage_index>=count{continue;}
            let pairset=sub(t,u16be(t,10+coverage_index*2)? as usize)?;
            let pairs=u16be(pairset,0)? as usize;
            let row=2+sz1+sz2;
            for i in 0..pairs.min(65535){let at=2+i*row;
                if u16be(pairset,at)==Some(right){return xadvance(pairset,at+2,v1);}
            }
        }else if format==2{
            let first_class=classdef(t,u16be(t,8)? as usize,left);
            let second_class=classdef(t,u16be(t,10)? as usize,right);
            let classes1=u16be(t,12)? as usize;let classes2=u16be(t,14)? as usize;
            if first_class>=classes1||second_class>=classes2{continue;}
            let row=sz1+sz2;
            let at=16+(first_class*classes2+second_class).checked_mul(row)?;
            return xadvance(t,at,v1);
        }
    }
    None
}

/// Parse all four basic GPOS ValueRecord fields. Format bits beyond bit 3
/// are offset-to-device/variation records and are skipped by this reader.
fn value_xy_adv(t:&[u8],at:usize,format:u16)->Option<(i16,i16,i16,i16)>{
    let mut values=[0i16;4];let mut cursor=at;
    for bit in 0..16{
        if format&(1u16<<bit)!=0{
            let val=i16be(t,cursor)?;cursor+=2;
            if bit<4{values[bit]=val;}
        }
    }
    Some((values[0],values[1],values[2],values[3]))
}
fn gpos_single(face:&Face,glyph:u16,script:Script,language:Option<[u8;4]>)->Option<(i16,i16,i16,i16)>{
    let data=face.table(*b"GPOS")?;
    let mut result=(0i16,0i16,0i16,0i16);let mut applied=false;
    for (mut kind,off) in lookups(face,data,script,&[*b"dist",*b"palt",*b"halt",*b"vpal",*b"vhal",*b"cpsp",*b"case"],language){
        let mut t=sub(data,off)?;
        if kind==9{
            if u16be(t,0)!=Some(1){continue;}
            kind=u16be(t,2)?;t=sub(t,u32be(t,4)? as usize)?;
        }
        if kind!=1{continue;}
        let format=u16be(t,0)?;
        let Some(index)=coverage(t,u16be(t,2)? as usize,glyph)else{continue;};
        let value_format=u16be(t,4)?;
        let value=match format{
            1=>value_xy_adv(t,6,value_format),
            2=>{
                if index>=u16be(t,6)? as usize{continue;}
                value_xy_adv(t,8+index*value_record_size(value_format),value_format)
            },_=>None,
        };
        if let Some((x,y,advance,height))=value{
            result.0=result.0.saturating_add(x);
            result.1=result.1.saturating_add(y);
            result.2=result.2.saturating_add(advance);
            result.3=result.3.saturating_add(height);
            applied=true;
        }
    }
    if applied{Some(result)}else{None}
}
/// CursivePos type 3, aligning the left glyph's exit and right glyph's entry
/// anchor by adjusting horizontal advance and vertical placement.
fn gpos_cursive(face:&Face,left:u16,right:u16,script:Script,language:Option<[u8;4]>)->Option<(i16,i16)>{
    let data=face.table(*b"GPOS")?;
    for (mut kind,off) in lookups(face,data,script,&[*b"curs"],language){
        let mut t=sub(data,off)?;
        if kind==9{
            if u16be(t,0)!=Some(1){continue;}
            kind=u16be(t,2)?;t=sub(t,u32be(t,4)? as usize)?;
        }
        if kind!=3||u16be(t,0)!=Some(1){continue;}
        let cov=u16be(t,2)? as usize;
        let Some(li)=coverage(t,cov,left)else{continue};
        let Some(ri)=coverage(t,cov,right)else{continue};
        let count=u16be(t,4)? as usize;
        if li>=count||ri>=count{continue;}
        let Some(exit)=u16be(t,6+li*4+2).filter(|off|*off!=0)else{continue};
        let Some(entry)=u16be(t,6+ri*4).filter(|off|*off!=0)else{continue};
        let (ex,ey)=anchor_xy(t,exit as usize)?;
        let (ix,iy)=anchor_xy(t,entry as usize)?;
        return Some((ex.saturating_sub(ix),ey.saturating_sub(iy)));
    }
    None
}

/// Return an attachment anchor in font units. Anchor formats 2 and 3
/// retain x/y coordinates; device/variation adjustments are not inferred.
fn anchor_xy(table:&[u8],offset:usize)->Option<(i16,i16)>{
    let data=sub(table,offset)?;
    match u16be(data,0)?{1|2|3=>Some((i16be(data,2)?,i16be(data,4)?)),_=>None}
}
/// GPOS mark-to-base and mark-to-mark attachment. Offsets are expressed
/// relative to the base glyph origin, in design units.
fn gpos_mark(face:&Face,base:u16,mark:u16,script:Script,language:Option<[u8;4]>)->Option<(i16,i16)>{
    let data=face.table(*b"GPOS")?;
    for (mut ty,offset) in lookups(face,data,script,&[*b"mark",*b"mkmk",*b"abvm",*b"blwm"],language){
        let mut t=sub(data,offset)?;
        if ty==9{
            if u16be(t,0)!=Some(1){continue;}
            ty=u16be(t,2)?;
            t=sub(t,u32be(t,4)? as usize)?;
        }
        if !matches!(ty,4|5|6)||u16be(t,0)!=Some(1){continue;}
        let Some(mark_index)=coverage(t,u16be(t,2)? as usize,mark)else{continue;};
        let Some(base_index)=coverage(t,u16be(t,4)? as usize,base)else{continue;};
        let classes=u16be(t,6)? as usize;
        if classes==0||classes>256{continue;}
        let marks=sub(t,u16be(t,8)? as usize)?;
        let bases=sub(t,u16be(t,10)? as usize)?;
        if mark_index>=u16be(marks,0)? as usize || base_index>=u16be(bases,0)? as usize{continue;}
        let mark_record=2+mark_index*4;
        let class=u16be(marks,mark_record)? as usize;
        if class>=classes{continue;}
        let mark_offset=u16be(marks,mark_record+2)? as usize;
        let base_offset=if ty==5{
            // LigatureArray -> LigatureAttach -> ComponentRecord[] -> Anchor.
            let lig=u16be(bases,2+base_index*2).and_then(|off|sub(bases,off as usize));
            let Some(lig)=lig else{continue};
            let n=u16be(lig,0).unwrap_or(0) as usize;
            if n==0||n>256{continue;}
            // Without the original ligature component association, the final
            // component is the most useful conventional mark attachment site.
            let off=u16be(lig,2+((n-1)*classes+class)*2).unwrap_or(0) as usize;
            if off==0{0}else{
                // Different anchor owner than MarkToBase: handle below.
                let mark_anchor=anchor_xy(marks,mark_offset);
                let lig_anchor=anchor_xy(lig,off);
                if let (Some((mx,my)),Some((bx,by)))=(mark_anchor,lig_anchor){
                    return Some((bx.saturating_sub(mx),by.saturating_sub(my)));
                }
                0
            }
        }else{u16be(bases,2+(base_index*classes+class)*2).unwrap_or(0) as usize};
        if mark_offset==0||base_offset==0{continue;}
        let (mx,my)=anchor_xy(marks,mark_offset)?;
        let (bx,by)=anchor_xy(bases,base_offset)?;
        return Some((bx.saturating_sub(mx),by.saturating_sub(my)));
    }
    None
}

/// Unicode joining controls and legacy Arabic presentation form fallback.
/// Modern fonts with contextual GSUB should still have their Arabic features
/// applied; this lookup is a compatibility path for fonts encoding forms in cmap.
fn joining_type(ch:char)->u8{
    let n=ch as u32;let rows=crate::arabic_data::JOIN_TYPES;
    let mut low=0;let mut high=rows.len();
    while low<high{let i=(low+high)/2;let (start,end,value)=rows[i];
        if n<start{high=i;}else if n>end{low=i+1;}else{return value;}}
    0
}
fn arabic_form(scalars:&[(usize,char)],index:usize)->Option<char>{
    let ch=scalars.get(index)?.1;
    if unicode::script(ch)!=Script::Arabic{return None;}
    let entry=crate::arabic_data::ARABIC_FORMS.binary_search_by_key(&(ch as u32),|(scalar,_)|*scalar).ok()?;
    let shapes=crate::arabic_data::ARABIC_FORMS[entry].1;
    let current=joining_type(ch);
    let prev=(0..index).rev().find_map(|i|{let value=joining_type(scalars[i].1);if value==5{None}else{Some(value)}});
    let next=((index+1)..scalars.len()).find_map(|i|{let value=joining_type(scalars[i].1);if value==5{None}else{Some(value)}});
    // 1=dual, 2=right, 3=left, 4=join-causing, 5=transparent.
    let from_prev=matches!(prev,Some(1|3|4))&&matches!(current,1|2|4);
    let to_next=matches!(current,1|3|4)&&matches!(next,Some(1|2|4));
    let choice=match (from_prev,to_next){(true,true)=>3,(false,true)=>2,(true,false)=>1,(false,false)=>0};
    char::from_u32(if shapes[choice]!=0{shapes[choice]}else{shapes[0]})
}
