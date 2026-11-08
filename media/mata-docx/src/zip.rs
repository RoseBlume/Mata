//! ZIP/ZIP64 support for OPC packages, implemented entirely with `std`.
//! The reader supports STORE and raw DEFLATE, validates CRCs and length limits;
//! the writer uses an in-tree fixed-Huffman LZ77 DEFLATE compressor.
use std::collections::BTreeMap;
use std::io::{Error, ErrorKind, Result};

fn bad(s: &str) -> Error { Error::new(ErrorKind::InvalidData, s) }
fn at(d: &[u8], i: usize, n: usize) -> Result<&[u8]> {
    d.get(i..i.checked_add(n).ok_or_else(||bad("zip offset overflow"))?)
        .ok_or_else(||bad("truncated zip"))
}
fn u16at(d: &[u8], i: usize) -> Result<u16> { Ok(u16::from_le_bytes(at(d,i,2)?.try_into().unwrap())) }
fn u32at(d: &[u8], i: usize) -> Result<u32> { Ok(u32::from_le_bytes(at(d,i,4)?.try_into().unwrap())) }
fn u64at(d: &[u8], i: usize) -> Result<u64> { Ok(u64::from_le_bytes(at(d,i,8)?.try_into().unwrap())) }
fn put16(b: &mut Vec<u8>, n: u16) { b.extend(n.to_le_bytes()); }
fn put32(b: &mut Vec<u8>, n: u32) { b.extend(n.to_le_bytes()); }
fn put64(b: &mut Vec<u8>, n: u64) { b.extend(n.to_le_bytes()); }
fn to_usize(n: u64) -> Result<usize> { usize::try_from(n).map_err(|_|bad("ZIP64 size does not fit memory")) }
fn valid_name(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('/') && !name.contains('\\') &&
    !name.contains('\0') && !name.split('/').any(|c| matches!(c, ""|"."|".."))
}

pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &c in data {
        crc ^= c as u32;
        for _ in 0..8 { crc = (crc >> 1) ^ (0xedb8_8320u32 & (0u32.wrapping_sub(crc & 1))); }
    }
    !crc
}

/// Compression selection for authoring ZIP archives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression { Store, Deflate }

/// Create a ZIP/ZIP64 archive, compressing members whenever beneficial.
/// `encode_with` can disable compression for reproducible byte-level diagnostics.
pub fn encode(parts: &BTreeMap<String, Vec<u8>>) -> Result<Vec<u8>> {
    encode_with(parts, Compression::Deflate)
}
pub fn encode_with(parts: &BTreeMap<String, Vec<u8>>, compression: Compression) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in parts {
        if !valid_name(name) || name.len() > u16::MAX as usize { return Err(bad("unsafe or oversized zip name")); }
        // Very short entries cannot profit from Huffman/LZ77 and should not
        // allocate a 32-KiB hash table per member (important for ZIP64 tests).
        let compressed = if compression == Compression::Deflate && data.len() >= 48 { deflate_fixed(data) } else { Vec::new() };
        let use_deflate = compression == Compression::Deflate && compressed.len() < data.len();
        let payload: &[u8] = if use_deflate { &compressed } else { data };
        let usize64 = data.len() as u64;
        let csize64 = payload.len() as u64;
        let offset = out.len() as u64;
        let big_size = usize64 >= u32::MAX as u64 || csize64 >= u32::MAX as u64;
        let big_offset = offset >= u32::MAX as u64;
        let version = if big_size || big_offset { 45 } else { 20 };
        let mut lextra = Vec::new();
        if big_size { put16(&mut lextra, 1); put16(&mut lextra, 16); put64(&mut lextra, usize64); put64(&mut lextra, csize64); }
        put32(&mut out, 0x0403_4b50); put16(&mut out, version); put16(&mut out, 0x0800);
        put16(&mut out, if use_deflate {8} else {0}); put16(&mut out, 0); put16(&mut out, 0);
        put32(&mut out, crc32(data)); put32(&mut out, if big_size {u32::MAX} else {csize64 as u32});
        put32(&mut out, if big_size {u32::MAX} else {usize64 as u32});
        put16(&mut out, name.len() as u16); put16(&mut out, lextra.len() as u16);
        out.extend(name.as_bytes()); out.extend(&lextra); out.extend(payload);

        let mut cextra = Vec::new();
        if big_size || big_offset {
            let mut fields = Vec::new();
            if big_size {put64(&mut fields, usize64);put64(&mut fields, csize64);}
            if big_offset {put64(&mut fields, offset);}
            put16(&mut cextra, 1); put16(&mut cextra, fields.len() as u16); cextra.extend(fields);
        }
        put32(&mut central, 0x0201_4b50); put16(&mut central, version); put16(&mut central, version);
        put16(&mut central, 0x0800); put16(&mut central, if use_deflate {8} else {0});
        put16(&mut central, 0); put16(&mut central, 0); put32(&mut central, crc32(data));
        put32(&mut central, if big_size {u32::MAX} else {csize64 as u32});
        put32(&mut central, if big_size {u32::MAX} else {usize64 as u32});
        put16(&mut central, name.len() as u16); put16(&mut central, cextra.len() as u16);
        put16(&mut central, 0); put16(&mut central, 0); put16(&mut central, 0);
        put32(&mut central, 0); put32(&mut central, if big_offset {u32::MAX} else {offset as u32});
        central.extend(name.as_bytes()); central.extend(cextra);
    }
    let cd_offset = out.len() as u64;
    let cd_size = central.len() as u64;
    out.extend(central);
    let count = parts.len() as u64;
    if count >= u16::MAX as u64 || cd_offset >= u32::MAX as u64 || cd_size >= u32::MAX as u64 {
        let zip64_offset = out.len() as u64;
        put32(&mut out, 0x0606_4b50); put64(&mut out, 44);
        put16(&mut out, 45); put16(&mut out, 45); put32(&mut out, 0); put32(&mut out, 0);
        put64(&mut out, count); put64(&mut out, count); put64(&mut out, cd_size); put64(&mut out, cd_offset);
        put32(&mut out, 0x0706_4b50); put32(&mut out, 0); put64(&mut out, zip64_offset); put32(&mut out, 1);
    }
    put32(&mut out, 0x0605_4b50); put16(&mut out, 0); put16(&mut out, 0);
    put16(&mut out, count.min(u16::MAX as u64) as u16); put16(&mut out, count.min(u16::MAX as u64) as u16);
    put32(&mut out, cd_size.min(u32::MAX as u64) as u32);
    put32(&mut out, cd_offset.min(u32::MAX as u64) as u32); put16(&mut out, 0);
    Ok(out)
}

/// Read a ZIP archive with per-entry and total extracted-size protections.
/// Rejects encrypted, multi-disk, duplicate, traversal and malformed entries.
pub fn decode(data: &[u8]) -> Result<BTreeMap<String, Vec<u8>>> {
    let start = data.len().saturating_sub(65557);
    let eocd = (start..data.len().saturating_sub(3)).rev()
        .find(|&i| data[i..].starts_with(b"PK\x05\x06") && i+22 <= data.len()
            && u16at(data,i+20).ok().and_then(|n|i.checked_add(22+n as usize)) == Some(data.len()))
        .ok_or_else(||bad("zip end-of-central-directory not found"))?;
    if u16at(data,eocd+4)? != 0 || u16at(data,eocd+6)? != 0 {return Err(bad("multi-disk ZIP unsupported"));}
    let mut count = u16at(data,eocd+10)? as u64;
    let mut cd_size = u32at(data,eocd+12)? as u64;
    let mut cd_off = u32at(data,eocd+16)? as u64;
    let has_zip64=eocd>=20 && u32at(data,eocd-20).ok()==Some(0x0706_4b50);
    if has_zip64 || cd_size == u32::MAX as u64 || cd_off == u32::MAX as u64 {
        if !has_zip64 {return Err(bad("missing ZIP64 locator"));}
        if u32at(data,eocd-16)? != 0 || u32at(data,eocd-4)? != 1 {return Err(bad("multi-disk ZIP64 unsupported"));}
        let off=to_usize(u64at(data,eocd-12)?)?;
        if u32at(data,off)? != 0x0606_4b50 || u64at(data,off+4)? < 44 {return Err(bad("invalid ZIP64 directory"));}
        if u32at(data,off+16)? != 0 || u32at(data,off+20)? != 0 {return Err(bad("multi-disk ZIP64 unsupported"));}
        count=u64at(data,off+32)?;cd_size=u64at(data,off+40)?;cd_off=u64at(data,off+48)?;
    }
    if count > 100_000 {return Err(bad("too many archive members"));}
    let mut pos=to_usize(cd_off)?;
    let cd_end=pos.checked_add(to_usize(cd_size)?).filter(|p|*p<=data.len() && *p<=eocd).ok_or_else(||bad("invalid central directory bounds"))?;
    let mut parts=BTreeMap::new();
    let mut total=0usize;
    for _ in 0..count {
        if u32at(data,pos)? != 0x0201_4b50 {return Err(bad("invalid central directory header"));}
        let flags=u16at(data,pos+8)?;
        let method=u16at(data,pos+10)?;
        let crc=u32at(data,pos+16)?;
        let mut csize=u32at(data,pos+20)? as u64;
        let mut usize_=u32at(data,pos+24)? as u64;
        let nl=u16at(data,pos+28)? as usize;
        let extra=u16at(data,pos+30)? as usize;
        let comment=u16at(data,pos+32)? as usize;
        let mut local=u32at(data,pos+42)? as u64;
        let name=std::str::from_utf8(at(data,pos+46,nl)?).map_err(|_|bad("non-UTF8 filename"))?.to_owned();
        let extra_start=pos.checked_add(46+nl).ok_or_else(||bad("invalid central header"))?;
        let extra_data=at(data,extra_start,extra)?;
        pos=extra_start.checked_add(extra+comment).filter(|p|*p<=cd_end).ok_or_else(||bad("invalid central header bounds"))?;
        let mut ep=0;
        while ep<extra_data.len() {
            let tag=u16at(extra_data,ep)?;
            let size=u16at(extra_data,ep+2)? as usize;
            let part=at(extra_data,ep+4,size)?;
            if tag==1 {
                let mut ix=0;
                if usize_==u32::MAX as u64 {usize_=u64at(part,ix)?;ix+=8;}
                if csize==u32::MAX as u64 {csize=u64at(part,ix)?;ix+=8;}
                if local==u32::MAX as u64 {local=u64at(part,ix)?;}
            }
            ep=ep.checked_add(4+size).ok_or_else(||bad("ZIP extra overflow"))?;
        }
        if flags & 1 != 0 || flags & 0x40 != 0 {return Err(bad("encrypted ZIP entry"));}
        if flags & 0x0800 == 0 && !name.is_ascii() {return Err(bad("unsupported ZIP filename encoding"));}
        if name.ends_with('/') {continue;}
        if !valid_name(&name) {return Err(bad("unsafe ZIP part name"));}
        let size=to_usize(usize_)?;
        if size>128*1024*1024 {return Err(bad("ZIP part exceeds 128 MiB limit"));}
        total=total.checked_add(size).ok_or_else(||bad("ZIP expanded size overflow"))?;
        if total>512*1024*1024 {return Err(bad("ZIP expands beyond 512 MiB limit"));}
        let local=to_usize(local)?;
        if u32at(data,local)? != 0x0403_4b50 {return Err(bad("bad ZIP local file header"));}
        if u16at(data,local+8)? != method {return Err(bad("ZIP method mismatch"));}
        let lnl=u16at(data,local+26)? as usize;
        let lel=u16at(data,local+28)? as usize;
        if at(data,local+30,lnl)? != name.as_bytes() {return Err(bad("ZIP filename mismatch"));}
        let payload=local.checked_add(30+lnl+lel).ok_or_else(||bad("ZIP payload offset overflow"))?;
        let comp=at(data,payload,to_usize(csize)?)?;
        let raw=match method {0=>comp.to_vec(),8=>inflate(comp,size)?,_=>return Err(bad("unsupported ZIP compression"))};
        if raw.len()!=size || crc32(&raw)!=crc {return Err(bad("ZIP size or CRC mismatch"));}
        if parts.insert(name,raw).is_some() {return Err(bad("duplicate ZIP entry"));}
    }
    if pos != cd_end {return Err(bad("unexpected bytes at end of ZIP directory"));}
    Ok(parts)
}

struct BitWriter { data: Vec<u8>, buf: u64, bits: u32 }
impl BitWriter {
    fn new()->Self{Self{data:Vec::new(),buf:0,bits:0}}
    fn put(&mut self,value:u32,n:u32){
        self.buf |= (value as u64)<<self.bits;
        self.bits+=n;
        while self.bits>=8 {self.data.push(self.buf as u8);self.buf >>= 8;self.bits-=8;}
    }
    fn finish(mut self)->Vec<u8>{if self.bits>0{self.data.push(self.buf as u8);}self.data}
}
fn reversed(mut v:u32,n:u32)->u32 {let mut r=0;for _ in 0..n {r=(r<<1)|(v&1);v>>=1;}r}
fn fixed_symbol(writer:&mut BitWriter,sym:usize){
    let (code,len)=match sym {0..=143=>(sym as u32+0x30,8),144..=255=>(sym as u32-144+0x190,9),256..=279=>(sym as u32-256,7),_=>(sym as u32-280+0xc0,8)};
    writer.put(reversed(code,len),len);
}
fn write_match(writer:&mut BitWriter,length:usize,distance:usize){
    for i in 0..29{let limit=LB[i]+((1usize<<LE[i])-1);if length<=limit{
        fixed_symbol(writer,257+i);writer.put((length-LB[i]) as u32,LE[i] as u32);break;
    }}
    for i in 0..30{let limit=DB[i]+((1usize<<DE[i])-1);if distance<=limit{
        writer.put(reversed(i as u32,5),5);writer.put((distance-DB[i]) as u32,DE[i] as u32);break;
    }}
}
fn deflate_fixed(data:&[u8])->Vec<u8>{
    let mut w=BitWriter::new();w.put(0b011,3); // BFINAL=1, BTYPE=01 (fixed Huffman)
    const HASH:usize=32768;
    let mut heads=vec![usize::MAX;HASH];
    let mut previous=vec![usize::MAX;data.len().min(32768)];
    let hash=|i:usize| -> usize {((data[i] as usize*251+(data[i+1] as usize)*67+data[i+2] as usize)*31)&(HASH-1)};
    let mut i=0;
    while i<data.len(){
        let mut best_len=0usize;let mut best_dist=0usize;
        if i+3<=data.len() {
            let mut candidate=heads[hash(i)];let mut examined=0;
            while candidate!=usize::MAX && examined<48 && i>candidate && i-candidate<=32768 {
                let max=(data.len()-i).min(258);
                let mut size=0;
                while size<max && data[candidate+size]==data[i+size] {size+=1;}
                if size>best_len && size>=3 {best_len=size;best_dist=i-candidate;if size==258{break;}}
                candidate=previous[candidate%previous.len()];examined+=1;
            }
        }
        if best_len>=3 {write_match(&mut w,best_len,best_dist);}else{fixed_symbol(&mut w,data[i] as usize);}
        let advance=best_len.max(1);
        for p in i..i+advance {if p+3<=data.len(){let key=hash(p);let ring_index=p%previous.len();previous[ring_index]=heads[key];heads[key]=p;}}
        i+=advance;
    }
    fixed_symbol(&mut w,256);w.finish()
}

#[derive(Default)]
struct Bits<'a> { data: &'a [u8], pos: usize }
impl Bits<'_> {
    fn get(&mut self, len: usize) -> Result<usize> {
        if len > 24 || self.pos.checked_add(len).filter(|&end| end <= self.data.len()*8).is_none() {
            return Err(bad("truncated DEFLATE stream"));
        }
        let mut value = 0usize;
        for bit in 0..len {
            value |= (((self.data[self.pos/8] >> (self.pos%8)) & 1) as usize) << bit;
            self.pos += 1;
        }
        Ok(value)
    }
}
#[derive(Default, Clone)]
struct Node { left: Option<usize>, right: Option<usize>, symbol: Option<usize> }
struct Huffman { nodes: Vec<Node> }
impl Huffman {
    fn new(lengths: &[usize]) -> Result<Self> {
        let mut counts = [0usize;16];
        for &l in lengths { if l > 15 { return Err(bad("invalid huffman code length")); } if l != 0 { counts[l]+=1; } }
        let mut code = 0usize;
        let mut next = [0usize;16];
        for bits in 1..=15 { code = (code + counts[bits-1]) << 1; next[bits] = code; }
        let mut tree = Self { nodes: vec![Node::default()] };
        for (sym, &len) in lengths.iter().enumerate() {
            if len == 0 { continue; }
            let c = next[len]; next[len]+=1;
            if c >= (1 << len) { return Err(bad("oversubscribed huffman tree")); }
            let mut node = 0usize;
            // Huffman codes appear MSB first in the DEFLATE bit stream.
            for shift in (0..len).rev() {
                let edge = (c >> shift) & 1;
                let next_index = if edge == 0 { tree.nodes[node].left } else { tree.nodes[node].right };
                node = match next_index {
                    Some(v) => v,
                    None => {
                        let ix = tree.nodes.len(); tree.nodes.push(Node::default());
                        if edge == 0 { tree.nodes[node].left = Some(ix); } else { tree.nodes[node].right = Some(ix); }
                        ix
                    }
                };
            }
            if tree.nodes[node].symbol.replace(sym).is_some() { return Err(bad("overlapping huffman symbols")); }
        }
        Ok(tree)
    }
    fn read(&self, bits: &mut Bits<'_>) -> Result<usize> {
        let mut n = 0usize;
        for _ in 0..16 {
            if let Some(s) = self.nodes[n].symbol { return Ok(s); }
            let edge = bits.get(1)?;
            n = (if edge == 0 { self.nodes[n].left } else { self.nodes[n].right })
                .ok_or_else(||bad("invalid huffman code"))?;
        }
        Err(bad("huffman code too long"))
    }
}
const LB: [usize;29] = [3,4,5,6,7,8,9,10,11,13,15,17,19,23,27,31,35,43,51,59,67,83,99,115,131,163,195,227,258];
const LE: [usize;29] = [0,0,0,0,0,0,0,0,1,1,1,1,2,2,2,2,3,3,3,3,4,4,4,4,5,5,5,5,0];
const DB: [usize;30] = [1,2,3,4,5,7,9,13,17,25,33,49,65,97,129,193,257,385,513,769,1025,1537,2049,3073,4097,6145,8193,12289,16385,24577];
const DE: [usize;30] = [0,0,0,0,1,1,2,2,3,3,4,4,5,5,6,6,7,7,8,8,9,9,10,10,11,11,12,12,13,13];
pub(crate) fn inflate(data: &[u8], expected: usize) -> Result<Vec<u8>> {
    let mut b = Bits { data, pos:0 };
    let mut out = Vec::with_capacity(expected.min(1024*1024));
    loop {
        let last = b.get(1)? != 0;
        let typ = b.get(2)?;
        if typ == 0 {
            b.pos = (b.pos+7) & !7;
            let n = b.get(16)?;
            let inverse = b.get(16)?;
            if (n ^ inverse) != 0xffff { return Err(bad("bad stored deflate length")); }
            if out.len()+n > expected { return Err(bad("deflate size exceeds header")); }
            for _ in 0..n { out.push(b.get(8)? as u8); }
        } else if typ == 1 || typ == 2 {
            let (lit, dist) = if typ == 1 {
                let ll: Vec<usize> = (0..288).map(|v| if v <= 143 {8} else if v<=255 {9} else if v<=279 {7} else {8}).collect();
                (Huffman::new(&ll)?, Huffman::new(&[5;32])?)
            } else {
                let hlit = b.get(5)?+257;
                let hdist = b.get(5)?+1;
                let hclen = b.get(4)?+4;
                let order = [16,17,18,0,8,7,9,6,10,5,11,4,12,3,13,2,14,1,15];
                let mut lengths = [0usize;19];
                for i in 0..hclen { lengths[order[i]] = b.get(3)?; }
                let h = Huffman::new(&lengths)?;
                let mut codes = Vec::new();
                while codes.len() < hlit+hdist {
                    let symbol = h.read(&mut b)?;
                    match symbol {
                        0..=15 => codes.push(symbol),
                        16 => { let prev = *codes.last().ok_or_else(||bad("deflate repeat before first symbol"))?; let times=b.get(2)?+3; codes.extend(std::iter::repeat(prev).take(times)); }
                        17 => { let times=b.get(3)?+3; codes.extend(std::iter::repeat(0).take(times)); }
                        18 => { let times=b.get(7)?+11; codes.extend(std::iter::repeat(0).take(times)); }
                        _ => return Err(bad("invalid code length")),
                    }
                    if codes.len() > hlit+hdist { return Err(bad("excess code lengths")); }
                }
                (Huffman::new(&codes[..hlit])?, Huffman::new(&codes[hlit..])?)
            };
            loop {
                let symbol = lit.read(&mut b)?;
                match symbol {
                    0..=255 => { if out.len() == expected { return Err(bad("exceeded deflate uncompressed size")); } out.push(symbol as u8); }
                    256 => break,
                    257..=285 => {
                        let ix = symbol-257;
                        let length = LB[ix] + b.get(LE[ix])?;
                        let dsym = dist.read(&mut b)?;
                        if dsym >= 30 { return Err(bad("invalid deflate distance code")); }
                        let distance = DB[dsym] + b.get(DE[dsym])?;
                        if distance == 0 || distance > out.len() || out.len()+length > expected { return Err(bad("invalid deflate backreference")); }
                        for _ in 0..length { let v = out[out.len()-distance]; out.push(v); }
                    }
                    _ => return Err(bad("invalid deflate literal/length")),
                }
            }
        } else { return Err(bad("invalid deflate block type")); }
        if last { break; }
    }
    if out.len() != expected { return Err(bad("deflate size mismatch")); }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn zip_store_round_trip() {
        let mut files=BTreeMap::new(); files.insert("[Content_Types].xml".to_string(),b"hello".to_vec());
        files.insert("word/document.xml".to_string(),"Bonjour 🦀".as_bytes().to_vec());
        assert_eq!(decode(&encode(&files).unwrap()).unwrap(), files);
    }
    #[test] fn crc_known() { assert_eq!(crc32(b"123456789"),0xcbf43926); }
}
