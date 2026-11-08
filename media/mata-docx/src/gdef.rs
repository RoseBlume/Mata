//! Bounds-checked OpenType GDEF data needed by GSUB/GPOS lookup filtering
//! and precise ligature cursor placement. Does not require platform APIs.
use crate::opentype::Face;
fn u16be(d:&[u8], p:usize)->Option<u16>{let s=d.get(p..p.checked_add(2)?)?;Some(u16::from_be_bytes([s[0],s[1]]))}
fn u32be(d:&[u8], p:usize)->Option<u32>{let s=d.get(p..p.checked_add(4)?)?;Some(u32::from_be_bytes([s[0],s[1],s[2],s[3]]))}
fn coverage(d:&[u8], off:usize, glyph:u16)->bool{
    let Some(t)=d.get(off..)else{return false};
    match u16be(t,0){
        Some(1)=>{let n=u16be(t,2).unwrap_or(0) as usize;
            if n>65535||4usize.saturating_add(n.saturating_mul(2))>t.len(){return false;}
            let mut lo=0;let mut hi=n;
            while lo<hi{let mid=(lo+hi)/2;let item=u16be(t,4+mid*2).unwrap_or(u16::MAX);
                if item<glyph{lo=mid+1;}else{hi=mid;}}
            lo<n&&u16be(t,4+lo*2)==Some(glyph)
        },
        Some(2)=>{let n=u16be(t,2).unwrap_or(0) as usize;
            if n>65535||4usize.saturating_add(n.saturating_mul(6))>t.len(){return false;}
            (0..n).any(|i|{
                let a=u16be(t,4+i*6).unwrap_or(0);
                let b=u16be(t,6+i*6).unwrap_or(0);
                a<=glyph&&glyph<=b
            })
        },_=>false
    }
}
fn class(d:&[u8], off:usize, glyph:u16)->u16{
    if off==0{return 0;}
    let Some(t)=d.get(off..)else{return 0};
    match u16be(t,0){
        Some(1)=>{let first=u16be(t,2).unwrap_or(0);let n=u16be(t,4).unwrap_or(0) as usize;
            if (glyph as usize)>=first as usize && (glyph as usize)<first as usize+n{
                u16be(t,6+(glyph as usize-first as usize)*2).unwrap_or(0)
            }else{0}
        },
        Some(2)=>{let n=u16be(t,2).unwrap_or(0) as usize;
            if n>65535||4usize.saturating_add(n.saturating_mul(6))>t.len(){return 0;}
            // GDEF format 2 records are ordered by starting glyph.
            let mut lo=0;let mut hi=n;
            while lo<hi{let mid=(lo+hi)/2;
                if u16be(t,4+mid*6).unwrap_or(u16::MAX)<=glyph{lo=mid+1;}else{hi=mid;}}
            if lo==0{return 0;}
            let at=4+(lo-1)*6;
            if glyph<=u16be(t,at+2).unwrap_or(0){u16be(t,at+4).unwrap_or(0)}else{0}
        },_=>0
    }
}
/// A GDEF caret may be an outline coordinate or a TrueType point index.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum CaretValue { Coordinate(i16), PointIndex(u16) }
/// A lookup's original flag bits and optional mark-filtering set index.
#[derive(Debug,Clone,Copy,PartialEq,Eq,Default)]
pub struct LookupFlags {pub bits:u16,pub mark_filtering_set:Option<u16>}
impl LookupFlags{
    /// Determines whether the font instructs the shaper to skip this glyph.
    pub fn ignores(&self,face:&Face,glyph:u16)->bool{
        let Some(d)=face.table(*b"GDEF")else{return false};
        let glyph_class=class(d,u16be(d,4).unwrap_or(0) as usize,glyph);
        match glyph_class{
            1 if self.bits&0x0002!=0=>true,
            2 if self.bits&0x0004!=0=>true,
            3 if self.bits&0x0008!=0=>true,
            3=>{
                if self.bits&0x0010!=0 {
                    let Some(set)=self.mark_filtering_set else{return true};
                    let Some(off)=u32be(d,12).map(|n|n as usize)else{return true};
                    let Some(t)=d.get(off..)else{return true};
                    if u16be(t,0)!=Some(1)||set as usize>=u16be(t,2).unwrap_or(0) as usize{return true;}
                    let Some(coverage_offset)=u32be(t,4+set as usize*4)else{return true};
                    return !coverage(t,coverage_offset as usize,glyph);
                }
                let typ=self.bits>>8;
                typ!=0 && class(d,u16be(d,10).unwrap_or(0) as usize,glyph)!=typ
            },_=>false
        }
    }
}
/// Retrieve lookup flags for a GSUB/GPOS subtable at an absolute table offset.
/// Extension lookups use the outer extension-subtable offset here.
pub fn flags_for_subtable(table:&[u8],subtable_offset:usize)->LookupFlags{
    let Some(list)=u16be(table,8).map(|v|v as usize)else{return LookupFlags::default()};
    let count=u16be(table,list).unwrap_or(0) as usize;
    if count>4096{return LookupFlags::default();}
    for i in 0..count{
        let Some(off)=u16be(table,list+2+i*2)else{break};
        let Some(base)=list.checked_add(off as usize)else{continue};
        let n=u16be(table,base+4).unwrap_or(0) as usize;
        if n>1024{continue;}
        for j in 0..n{
            let Some(offset)=u16be(table,base+6+j*2)else{break};
            if base.checked_add(offset as usize)!=Some(subtable_offset){continue;}
            let bits=u16be(table,base+2).unwrap_or(0);
            let mark_filtering_set=if bits&0x0010!=0{u16be(table,base+6+n*2)}else{None};
            return LookupFlags{bits,mark_filtering_set};
        }
    }
    LookupFlags::default()
}
/// Read GDEF LigCaretList coordinates/point indices for caret movement inside
/// a ligature. Device-table offsets are retained as design coordinates only.
pub fn ligature_carets(face:&Face,glyph:u16)->Vec<CaretValue>{
    let Some(d)=face.table(*b"GDEF")else{return Vec::new()};
    let Some(at)=u16be(d,8).map(|v|v as usize).filter(|v|*v>0)else{return Vec::new()};
    let Some(list)=d.get(at..)else{return Vec::new()};
    let Some(cov)=u16be(list,0)else{return Vec::new()};
    if !coverage(list,cov as usize,glyph){return Vec::new();}
    // The LigCaretList coverage is sorted by glyph ID.
    let n=u16be(list,2).unwrap_or(0) as usize;
    if n>4096{return Vec::new();}
    let Some(covtab)=list.get(cov as usize..)else{return Vec::new()};
    let index=match u16be(covtab,0){
        Some(1)=>{(0..u16be(covtab,2).unwrap_or(0) as usize).find(|i|u16be(covtab,4+i*2)==Some(glyph))},
        Some(2)=>{let n=u16be(covtab,2).unwrap_or(0) as usize;
            (0..n).find_map(|i|{
                let at=4+i*6;
                let a=u16be(covtab,at)?;let b=u16be(covtab,at+2)?;
                if a<=glyph&&glyph<=b{u16be(covtab,at+4).map(|start|start as usize+(glyph-a) as usize)}else{None}
            })
        },_=>None
    };
    let Some(index)=index.filter(|i|*i<n)else{return Vec::new()};
    let Some(off)=u16be(list,4+index*2).map(|v|v as usize)else{return Vec::new()};
    let Some(lig)=list.get(off..)else{return Vec::new()};
    let count=u16be(lig,0).unwrap_or(0) as usize;
    if count>256{return Vec::new();}
    (0..count).filter_map(|i|{
        let off=u16be(lig,2+i*2)? as usize;let caret=lig.get(off..)?;
        match u16be(caret,0)?{
            1|3=>Some(CaretValue::Coordinate(i16::from_be_bytes(caret.get(2..4)?.try_into().ok()?))),
            2=>Some(CaretValue::PointIndex(u16be(caret,2)?)),_=>None
        }
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_gdef_class_and_coverage(){
        // ClassDef format 1 with two consecutive glyph classes.
        let classes=[0,1, 0,5, 0,2, 0,1, 0,3];
        assert_eq!(class(&classes,0,5),0); // null offset is not a table
        let mut table=vec![0];table.extend_from_slice(&classes);
        assert_eq!(class(&table,1,5),1);
        assert_eq!(class(&table,1,6),3);
        assert_eq!(class(&table,1,7),0);
        let coverage_table=[0,1, 0,2, 0,5, 0,9];
        assert!(coverage(&coverage_table,0,5));
        assert!(coverage(&coverage_table,0,9));
        assert!(!coverage(&coverage_table,0,6));
    }
    #[test]
    fn flags_find_original_lookup_header(){
        // Header (GSUB 1.0) with LookupList offset 10.
        let mut table=vec![0u8;40];
        table[8..10].copy_from_slice(&10u16.to_be_bytes());
        table[10..12].copy_from_slice(&1u16.to_be_bytes());
        table[12..14].copy_from_slice(&4u16.to_be_bytes());
        table[14..16].copy_from_slice(&4u16.to_be_bytes());
        table[16..18].copy_from_slice(&0x0008u16.to_be_bytes());
        table[18..20].copy_from_slice(&1u16.to_be_bytes());
        table[20..22].copy_from_slice(&8u16.to_be_bytes());
        assert_eq!(flags_for_subtable(&table,22).bits,0x0008);
        assert_eq!(flags_for_subtable(&table,33),LookupFlags::default());
    }
}
