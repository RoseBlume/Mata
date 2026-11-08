//! Bounded OpenType HVAR item-variation-store evaluation.
//!
//! This computes *horizontal metrics* at user-defined variation coordinates.
//! It does not itself rasterize `gvar`/CFF2 outlines; rasterizer instances use
//! matching coordinates to avoid a visible metrics/outlines mismatch.
use std::collections::BTreeMap;
use crate::opentype::VariationAxis;

fn u16be(b:&[u8],i:usize)->Option<u16>{let s=b.get(i..i.checked_add(2)?)?;Some(u16::from_be_bytes([s[0],s[1]]))}
fn i16be(b:&[u8],i:usize)->Option<i16>{u16be(b,i).map(|v|v as i16)}
fn u32be(b:&[u8],i:usize)->Option<u32>{let s=b.get(i..i.checked_add(4)?)?;Some(u32::from_be_bytes([s[0],s[1],s[2],s[3]]))}
fn f2dot14(b:&[u8],i:usize)->Option<f32>{i16be(b,i).map(|v|v as f32/16384.0)}
fn mapping(avar:&[u8],axes:usize,index:usize,x:f32)->Option<f32>{
    if u16be(avar,0)!=Some(1)||u16be(avar,6)? as usize!=axes{return None;}
    let mut cursor=8;
    for axis in 0..axes{
        let count=u16be(avar,cursor)? as usize;cursor+=2;
        if count<2||count>4096{return None;}
        let size=count.checked_mul(4)?;
        avar.get(cursor..cursor.checked_add(size)?)?;
        if axis==index{
            let mut prev=(f2dot14(avar,cursor)?,f2dot14(avar,cursor+2)?);
            if x<=prev.0{return Some(prev.1);}
            for j in 1..count{
                let next=(f2dot14(avar,cursor+j*4)?,f2dot14(avar,cursor+j*4+2)?);
                if x<=next.0{
                    let d=next.0-prev.0;
                    return Some(if d.abs()<0.0001{next.1}else{prev.1+(x-prev.0)/d*(next.1-prev.1)});
                }
                prev=next;
            }
            return Some(prev.1);
        }
        cursor+=size;
    }
    None
}
pub(crate) fn normalized(axes:&[VariationAxis],values:&BTreeMap<[u8;4],f32>,avar:Option<&[u8]>)->Vec<f32>{
    axes.iter().enumerate().map(|(i,axis)|{
        let v=values.get(&axis.tag).copied().unwrap_or(axis.default).clamp(axis.minimum,axis.maximum);
        let n=if v<axis.default{
            (v-axis.default)/(axis.default-axis.minimum).max(0.00001)
        }else if v>axis.default{
            (v-axis.default)/(axis.maximum-axis.default).max(0.00001)
        }else{0.0};
        avar.and_then(|table|mapping(table,axes.len(),i,n)).unwrap_or(n)
    }).collect()
}
fn region_weight(table:&[u8],offset:usize,coords:&[f32])->Option<f32>{
    let mut multiplier=1.0f32;
    for (i,x) in coords.iter().enumerate(){
        let at=offset.checked_add(i.checked_mul(6)?)?;
        let (start,peak,end)=(f2dot14(table,at)?,f2dot14(table,at+2)?,f2dot14(table,at+4)?);
        if !(start<=peak && peak<=end){return None;}
        let factor=if peak==0.0{1.0}
            else if *x<start || *x>end{0.0}
            else if (*x-peak).abs()<1e-6{1.0}
            else if *x<peak{(*x-start)/(peak-start).max(0.00001)}
            else{(end-*x)/(end-peak).max(0.00001)};
        multiplier*=factor.clamp(0.0,1.0);
        if multiplier==0.0{break;}
    }
    Some(multiplier)
}

pub(crate) fn hvar_advance_delta(hvar:&[u8],axes:&[VariationAxis],values:&BTreeMap<[u8;4],f32>,
                                  avar:Option<&[u8]>,glyph:u16)->Option<f32>{
    if hvar.len()<20||u16be(hvar,0)!=Some(1)||axes.len()>64{return None;}
    let store_offset=u32be(hvar,4)? as usize;
    let map_offset=u32be(hvar,8)? as usize;
    let (outer,inner)=if map_offset==0{(0usize,glyph as usize)}else{
        let map=hvar.get(map_offset..)?;
        let format=u16be(map,0)?;
        let size=1+((format>>4)&3) as usize;
        let bits=1+(format&15) as usize;
        let count=u16be(map,2)? as usize;
        if count==0{return None;}
        let index=(glyph as usize).min(count-1);
        let begin=4+index.checked_mul(size)?;
        let mut value=0u32;
        for byte in map.get(begin..begin.checked_add(size)?)?{value=(value<<8)|*byte as u32;}
        let mask=(1u32.checked_shl(bits as u32)?).wrapping_sub(1);
        ((value>>bits) as usize,(value&mask) as usize)
    };
    if outer==0xffff&&inner==0xffff{return Some(0.0);}
    let store=hvar.get(store_offset..)?;
    item_variation_delta(store,axes,values,avar,outer,inner)
}

/// Resolve a single ItemVariationStore row by outer/inner delta-set indices.
/// HVAR and MVAR share this exact region-scalar representation.
fn item_variation_delta(store:&[u8],axes:&[VariationAxis],values:&BTreeMap<[u8;4],f32>,
                        avar:Option<&[u8]>,outer:usize,inner:usize)->Option<f32>{
    if u16be(store,0)!=Some(1){return None;}
    let regions_offset=u32be(store,2)? as usize;
    let count=u16be(store,6)? as usize;
    if count>16384||outer>=count{return None;}
    let item_offset=u32be(store,8+4*outer)? as usize;
    let item=store.get(item_offset..)?;
    let (item_count,short_count,region_count)=(u16be(item,0)? as usize,u16be(item,2)? as usize,u16be(item,4)? as usize);
    if inner>=item_count||short_count>region_count||region_count>16384{return None;}
    let region_list=store.get(regions_offset..)?;
    let axis_count=u16be(region_list,0)? as usize;
    let total_regions=u16be(region_list,2)? as usize;
    if axis_count!=axes.len()||total_regions>32767{return None;}
    let normalized=normalized(axes,values,avar);
    let row_size=short_count.checked_mul(2)?.checked_add(region_count-short_count)?;
    let row_offset=6usize.checked_add(region_count.checked_mul(2)?)?.checked_add(inner.checked_mul(row_size)?)?;
    let mut sum=0.0f32;
    let mut cursor=row_offset;
    for r in 0..region_count{
        let region=u16be(item,6+2*r)? as usize;
        if region>=total_regions{return None;}
        let weight=region_weight(region_list,4usize.checked_add(region.checked_mul(axis_count.checked_mul(6)?)?)?,&normalized)?;
        let delta=if r<short_count{
            let d=i16be(item,cursor)? as f32;cursor+=2;d
        }else{let d=*item.get(cursor)? as i8 as f32;cursor+=1;d};
        sum+=weight*delta;
    }
    if sum.is_finite(){Some(sum)}else{None}
}

/// MVAR changes ascent, descent and line gap for a named variable-font instance.
/// For example, `hasc`, `hdsc` and `hlgp` map to the hhea metrics.
pub(crate) fn mvar_metric_delta(mvar:&[u8],axes:&[VariationAxis],values:&BTreeMap<[u8;4],f32>,
                                avar:Option<&[u8]>,tag:[u8;4])->Option<f32>{
    if u16be(mvar,0)!=Some(1)||axes.len()>64{return None;}
    let record_size=u16be(mvar,6)? as usize;
    let record_count=u16be(mvar,8)? as usize;
    let offset=u16be(mvar,10)? as usize;
    if record_size<8||record_count>2048||offset==0{return None;}
    let mut indices=None;
    for i in 0..record_count{
        let at=12usize.checked_add(i.checked_mul(record_size)?)?;
        if mvar.get(at..at.checked_add(4)?)?==tag.as_slice(){
            indices=Some((u16be(mvar,at+4)? as usize,u16be(mvar,at+6)? as usize));
            break;
        }
    }
    let (outer,inner)=indices?;
    item_variation_delta(mvar.get(offset..)?,axes,values,avar,outer,inner)
}


/// Evaluate a COLR v1 variation index against its ItemVariationStore.
///
/// `index` is a zero-based entry in the DeltaSetIndexMap. If no map is
/// provided, the index itself is split into outer/inner sixteen-bit fields.
/// DeltaSetIndexMap formats 0 and 1 use byte-sized format/entry-format
/// headers, unlike the older HVAR advance-map representation.
pub(crate) fn colr_delta(colr:&[u8],axes:&[VariationAxis],values:&BTreeMap<[u8;4],f32>,
                         avar:Option<&[u8]>,index:u32)->Option<f32>{
    if index==u32::MAX{return Some(0.0);}
    let store_offset=u32be(colr,30)? as usize;
    if store_offset==0{return Some(0.0);}
    let map_offset=u32be(colr,26)? as usize;
    let (outer,inner)=if map_offset==0{
        ((index>>16) as usize,(index&65535) as usize)
    }else{
        let map=colr.get(map_offset..)?;
        let format=*map.first()?;
        let entry_format=*map.get(1)?;
        let (count,start)=match format{
            0=>(u16be(map,2)? as usize,4usize),
            1=>(u32be(map,2)? as usize,6usize),
            _=>return None,
        };
        if count==0||count>2_000_000{return None;}
        let entry_size=1+((entry_format>>4)&3) as usize;
        let inner_bits=1+(entry_format&15) as u32;
        if inner_bits>16{return None;}
        let position=(index as usize).min(count-1);
        let at=start.checked_add(position.checked_mul(entry_size)?)?;
        let mut raw=0u32;
        for byte in map.get(at..at.checked_add(entry_size)?)?{raw=(raw<<8)|*byte as u32;}
        let inner_mask=(1u32<<inner_bits)-1;
        ((raw>>inner_bits) as usize,(raw&inner_mask) as usize)
    };
    if outer==65535&&inner==65535{return Some(0.0);}
    item_variation_delta(colr.get(store_offset..)?,axes,values,avar,outer,inner)
}
