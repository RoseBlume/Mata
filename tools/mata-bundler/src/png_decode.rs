//! PNG decoder (8-bit non-interlaced grayscale/RGB/indexed/gray+alpha/RGBA).
//! Implements RFC 1950/1951 inflate in safe Rust with no external crates.
use crate::{archive, bail, Result};
struct Bits<'a> { buf: &'a [u8], byte:usize, acc:u64, count:u8 }
impl<'a> Bits<'a> {
    fn new(buf:&'a [u8])->Self {Self{buf,byte:0,acc:0,count:0}}
    fn fill(&mut self,n:u8){while self.count<n && self.byte<self.buf.len(){self.acc|=(self.buf[self.byte] as u64)<<self.count;self.byte+=1;self.count+=8;}}
    fn peek(&mut self,n:u8)->u16 {self.fill(n);(self.acc&((1u64<<n)-1)) as u16}
    fn take(&mut self,n:u8)->Result<u16>{self.fill(n);if self.count<n{return bail("truncated DEFLATE data");}let v=self.peek(n);self.acc>>=n;self.count-=n;Ok(v)}
    fn align(&mut self){let leftover=self.count%8;self.acc>>=leftover;self.count-=leftover;}
}
struct Huff { table:Vec<(u16,u8)>,bits:u8 }
impl Huff {
    fn new(lengths:&[u8])->Result<Self>{
        let max=*lengths.iter().max().unwrap_or(&0);
        if max==0 || max>15{return bail("invalid Huffman table");}
        let mut counts=[0u16;16];for &len in lengths{if len>0{counts[len as usize]+=1;}}
        let mut available=1i32;
        for count in counts.iter().skip(1) {available=available*2-*count as i32;if available<0{return bail("oversubscribed Huffman tree");}}
        let mut codes=[0u16;16];let mut next=0u16;
        for bits in 1..=15 {next=(next+counts[bits-1])<<1;codes[bits]=next;}
        let mut table=vec![(0u16,0u8);1usize<<max];
        for (symbol,&len) in lengths.iter().enumerate(){
            if len==0{continue;}
            let code=codes[len as usize];codes[len as usize]+=1;
            let rev=code.reverse_bits()>>(16-len);
            for i in 0..(1usize<<(max-len)){
                table[rev as usize | (i<<len)]=(symbol as u16,len);
            }
        }
        Ok(Self{table,bits:max})
    }
    fn symbol(&self,r:&mut Bits<'_>)->Result<usize>{
        let (s,n)=self.table[r.peek(self.bits) as usize];if n==0{return bail("invalid DEFLATE code");}
        r.take(n)?;Ok(s as usize)
    }
}
const LEN_BASE:[usize;29]=[3,4,5,6,7,8,9,10,11,13,15,17,19,23,27,31,35,43,51,59,67,83,99,115,131,163,195,227,258];
const LEN_EXTRA:[u8;29]=[0,0,0,0,0,0,0,0,1,1,1,1,2,2,2,2,3,3,3,3,4,4,4,4,5,5,5,5,0];
const DIST_BASE:[usize;30]=[1,2,3,4,5,7,9,13,17,25,33,49,65,97,129,193,257,385,513,769,1025,1537,2049,3073,4097,6145,8193,12289,16385,24577];
const DIST_EXTRA:[u8;30]=[0,0,0,0,1,1,2,2,3,3,4,4,5,5,6,6,7,7,8,8,9,9,10,10,11,11,12,12,13,13];
fn tables(r:&mut Bits<'_>,kind:u16)->Result<(Huff,Huff)>{
    if kind==1 {
        let mut lens=vec![0u8;288];for (i,x) in lens.iter_mut().enumerate(){*x=if i<144{8}else if i<256{9}else if i<280{7}else{8};}
        return Ok((Huff::new(&lens)?,Huff::new(&[5u8;32])?));
    }
    let hlit=r.take(5)? as usize+257;let hdist=r.take(5)? as usize+1;let hclen=r.take(4)? as usize+4;
    if hlit>286 || hdist>32 {return bail("invalid DEFLATE dynamic table size");}
    const ORDER:[usize;19]=[16,17,18,0,8,7,9,6,10,5,11,4,12,3,13,2,14,1,15];
    let mut cl=[0u8;19];for i in 0..hclen{cl[ORDER[i]]=r.take(3)? as u8;}
    let decoder=Huff::new(&cl)?;let mut lens=Vec::with_capacity(hlit+hdist);
    while lens.len()<hlit+hdist{
        match decoder.symbol(r)? {
            n@0..=15=>lens.push(n as u8),
            16=>{let last=*lens.last().ok_or("repeat with no preceding DEFLATE length")?;let n=r.take(2)? as usize+3; lens.extend(std::iter::repeat_n(last,n));}
            17=>{let n=r.take(3)? as usize+3;lens.extend(std::iter::repeat_n(0u8,n));}
            18=>{let n=r.take(7)? as usize+11;lens.extend(std::iter::repeat_n(0u8,n));}
            _=>return bail("invalid DEFLATE code length"),
        }
        if lens.len()>hlit+hdist{return bail("DEFLATE code length overflow");}
    }
    let (literal,dist)=lens.split_at(hlit);
    // Empty distance table is permitted when the block has only literals.
    let distance = if dist.iter().all(|&len|len==0) { Huff::new(&[1])? } else { Huff::new(dist)? };
    Ok((Huff::new(literal)?,distance))
}
fn inflate(data:&[u8],max_size:usize)->Result<Vec<u8>> {
    if data.len()<6{return bail("truncated zlib stream");}
    let cmf=data[0];let flg=data[1];
    if cmf&15!=8 || cmf>>4>7 || ((cmf as u16)*256+flg as u16)%31!=0 || flg&0x20!=0 {
        return bail("unsupported zlib header");
    }
    let mut r=Bits::new(&data[2..data.len()-4]);let mut out=Vec::new();
    loop {
        let last=r.take(1)?!=0;let kind=r.take(2)?;
        if kind==0 {
            r.align();let len=r.take(16)? as usize;let nlen=r.take(16)?;
            if (len as u16)!=!nlen{return bail("invalid DEFLATE stored block length");}
            if len>max_size.saturating_sub(out.len()){return bail("PNG pixel buffer exceeds limit");}
            for _ in 0..len {out.push(r.take(8)? as u8);}
        } else if kind==1 || kind==2 {
            let (lit,dist)=tables(&mut r,kind)?;
            loop {
                let code=lit.symbol(&mut r)?;
                if code<256 {if out.len()==max_size{return bail("PNG pixel buffer exceeds limit");}out.push(code as u8);}
                else if code==256 {break;}
                else if code<=285 {
                    let i=code-257;let length=LEN_BASE[i]+r.take(LEN_EXTRA[i])? as usize;
                    let d=dist.symbol(&mut r)?;if d>=30{return bail("invalid DEFLATE distance");}
                    let offset=DIST_BASE[d]+r.take(DIST_EXTRA[d])? as usize;
                    if offset==0 || offset>out.len() || length>max_size.saturating_sub(out.len()) {return bail("invalid DEFLATE back-reference");}
                    for _ in 0..length {let v=out[out.len()-offset];out.push(v);}
                } else {return bail("invalid DEFLATE literal/length symbol");}
            }
        } else {return bail("reserved DEFLATE block type");}
        if last{break;}
    }
    let mut s1=1u32;let mut s2=0u32;
    for chunk in out.chunks(5552){for &b in chunk{s1=(s1+b as u32)%65521;s2=(s2+s1)%65521;}}
    let adler=u32::from_be_bytes(data[data.len()-4..].try_into()?);
    if adler!=((s2<<16)|s1) {return bail("invalid zlib Adler-32 checksum");}
    Ok(out)
}
fn be32(s:&[u8])->Result<u32>{Ok(u32::from_be_bytes(s.try_into()?))}
fn paeth(a:u8,b:u8,c:u8)->u8{
    let p=a as i32+b as i32-c as i32;
    let x=(p-a as i32).abs();let y=(p-b as i32).abs();let z=(p-c as i32).abs();
    if x<=y && x<=z{a}else if y<=z{b}else{c}
}
pub fn read_png(data:&[u8])->Result<(usize,usize,Vec<u32>)>{
    if !data.starts_with(b"\x89PNG\r\n\x1a\n"){return bail("icon input must be a PNG image");}
    let mut pos=8;let mut width=0usize;let mut height=0usize;let mut color=0u8;
    let mut palette=Vec::new();let mut transparency=Vec::new();let mut idat=Vec::new();
    let mut has_ihdr=false;let mut ended=false;
    while pos+12<=data.len() {
        let len=be32(&data[pos..pos+4])? as usize;pos+=4;
        if len>data.len().saturating_sub(pos+8){return bail("truncated PNG chunk");}
        let kind: &[u8;4] = data[pos..pos+4].try_into()?;pos+=4;let bytes=&data[pos..pos+len];pos+=len;
        let expected=be32(&data[pos..pos+4])?;pos+=4;
        let mut to_crc=Vec::with_capacity(len+4);to_crc.extend_from_slice(kind);to_crc.extend_from_slice(bytes);
        if archive::crc32(&to_crc)!=expected{return bail("PNG chunk CRC mismatch");}
        match kind {
            b"IHDR"=>{if has_ihdr || len!=13{return bail("invalid PNG IHDR");}has_ihdr=true;
                width=be32(&bytes[..4])? as usize;height=be32(&bytes[4..8])? as usize;
                if width==0 || height==0 || width.checked_mul(height).is_none_or(|n|n>16_777_216){return bail("PNG dimensions out of range");}
                if bytes[8]!=8 || bytes[10]!=0 || bytes[11]!=0 || bytes[12]!=0 {
                    return bail("PNG icons currently require 8-bit, non-interlaced, standard compression");
                }
                color=bytes[9];if !matches!(color,0|2|3|4|6){return bail("unsupported PNG color model");}
            }
            b"PLTE"=>palette=bytes.chunks_exact(3).map(|c|(c[0],c[1],c[2])).collect(),
            b"tRNS"=>transparency=bytes.to_vec(),
            b"IDAT"=>{if idat.len().saturating_add(len)>80*1024*1024{return bail("PNG compressed data too large");}idat.extend_from_slice(bytes);}
            b"IEND"=>{ended=true;break;}
            _ => {if kind[0]&0x20==0{return bail("unknown critical PNG chunk");}}
        }
    }
    if !has_ihdr || !ended{return bail("incomplete PNG");}
    let bpp=match color {0|3=>1,2=>3,4=>2,6=>4,_=>unreachable!()};
    let row=width.checked_mul(bpp).ok_or("PNG row too wide")?;
    let total=height.checked_mul(row+1).ok_or("PNG image too large")?;
    let raw=inflate(&idat,total)?;if raw.len()!=total{return bail("PNG decompressed size does not match dimensions");}
    let mut prev=vec![0u8;row];let mut cur=vec![0u8;row];let mut pixels=Vec::with_capacity(width*height);
    for y in 0..height {
        let base=y*(row+1);let filter=raw[base];
        for x in 0..row {
            let a=if x>=bpp{cur[x-bpp]}else{0};let b=prev[x];let c=if x>=bpp{prev[x-bpp]}else{0};
            cur[x]=raw[base+1+x].wrapping_add(match filter {0=>0,1=>a,2=>b,3=>((a as u16+b as u16)/2) as u8,4=>paeth(a,b,c),_=>return bail("invalid PNG scanline filter")});
        }
        for px in cur.chunks_exact(bpp) {
            let (r,g,b,a)=match color {
                0=>(px[0],px[0],px[0],255),2=>(px[0],px[1],px[2],255),
                3=>{let &(r,g,b)=palette.get(px[0] as usize).ok_or("PNG palette index out of range")?;(r,g,b,transparency.get(px[0] as usize).copied().unwrap_or(255))},
                4=>(px[0],px[0],px[0],px[1]),6=>(px[0],px[1],px[2],px[3]),_=>unreachable!(),
            };
            pixels.push(((r as u32)<<24)|((g as u32)<<16)|((b as u32)<<8)|(a as u32));
        }
        std::mem::swap(&mut prev,&mut cur);
    }
    Ok((width,height,pixels))
}
#[cfg(test)]mod tests {
    use super::*;
    #[test]fn decodes_stored_zlib(){assert_eq!(inflate(&archive::zlib_stored(b"hello"),100).unwrap(),b"hello");}
    #[test]fn decodes_fixed_huffman_zlib(){
        let bytes = [0x78,0x9c,0xab,0xa8,0x18,0x05,0xa3,0x60,0x14,0x0c,0x77,0x00,0x00,0xaa,0xf4,0xd4,0xd0];
        assert_eq!(inflate(&bytes,1000).unwrap(),vec![b'x';1000]);
    }
    #[test]fn decodes_dynamic_or_fixed_huffman_zlib(){
        let bytes = [0x78,0x9c,0xf3,0x48,0xcd,0xc9,0xc9,0x57,0x28,0xcf,0x2f,0xca,0x49,0xf1,0x18,0x65,0x8e,0x32,0x47,0x99,0xa3,0x4c,0x72,0x99,0x00,0x79,0xd6,0xa7,0x80];
        assert_eq!(inflate(&bytes,1100).unwrap(),b"Hello world".repeat(100));
    }
}
