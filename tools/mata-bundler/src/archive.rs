//! ZIP (stored), POSIX tar and gzip/deflate (stored blocks), all std-only.
//! Archives are valid and interoperable; using stored blocks trades size for a
//! compact, dependency-free bundler and fully streaming file I/O.
use std::{fs, io::{self, Read, Write, Seek}, path::Path};
use crate::{bail, tools, Result};

pub fn crc32(bytes:&[u8])->u32 { !crc_update(!0,bytes) }
fn crc_update(mut crc:u32,bytes:&[u8])->u32 {
    for &byte in bytes { crc^=byte as u32;for _ in 0..8 {crc=(crc>>1)^((crc&1).wrapping_neg()&0xedb88320);} }
    crc
}
fn write_u16<W:Write>(w:&mut W,x:u16)->io::Result<()> {w.write_all(&x.to_le_bytes())}
fn write_u32<W:Write>(w:&mut W,x:u32)->io::Result<()> {w.write_all(&x.to_le_bytes())}

pub fn zip_dir(root:&Path,dest:&Path)->Result<()> {
    let mut out=fs::File::create(dest)?;
    let mut index=Vec::new();
    for source in tools::collect_files(root)? {
        let rel=tools::safe_archive_name(source.strip_prefix(root)?)?;
        let name=rel.as_bytes();if name.len()>u16::MAX as usize {return bail("ZIP filename too long");}
        let size=fs::metadata(&source)?.len();if size>u32::MAX as u64{return bail("ZIP64 (>4 GiB) is unsupported by dependency-free ZIP writer");}
        let offset=out.stream_position()?;if offset>u32::MAX as u64{return bail("ZIP64 file offsets are unsupported");}
        let mut file=fs::File::open(&source)?;let mut buf=[0u8;64*1024];let mut crc=!0u32;
        loop {let n=file.read(&mut buf)?;if n==0{break;}crc=crc_update(crc,&buf[..n]);}
        crc=!crc;
        write_u32(&mut out,0x04034b50)?;write_u16(&mut out,20)?;write_u16(&mut out,0x0800)?;
        write_u16(&mut out,0)?;write_u16(&mut out,0)?;write_u16(&mut out,0)?;
        write_u32(&mut out,crc)?;write_u32(&mut out,size as u32)?;write_u32(&mut out,size as u32)?;
        write_u16(&mut out,name.len() as u16)?;write_u16(&mut out,0)?;out.write_all(name)?;
        io::copy(&mut fs::File::open(&source)?,&mut out)?;
        index.push((rel,size as u32,crc,offset as u32));
    }
    if index.len()>u16::MAX as usize {return bail("ZIP archive has too many files");}
    let center_offset=out.stream_position()?;if center_offset>u32::MAX as u64{return bail("ZIP64 central directory is unsupported");}
    for (name,size,crc,offset) in &index {
        let bytes=name.as_bytes();write_u32(&mut out,0x02014b50)?;write_u16(&mut out,20)?;write_u16(&mut out,20)?;
        write_u16(&mut out,0x0800)?;write_u16(&mut out,0)?;write_u16(&mut out,0)?;write_u16(&mut out,0)?;
        write_u32(&mut out,*crc)?;write_u32(&mut out,*size)?;write_u32(&mut out,*size)?;
        write_u16(&mut out,bytes.len() as u16)?;write_u16(&mut out,0)?;write_u16(&mut out,0)?;
        write_u16(&mut out,0)?;write_u16(&mut out,0)?;write_u32(&mut out,0)?;write_u32(&mut out,*offset)?;
        out.write_all(bytes)?;
    }
    let center_size=out.stream_position()? - center_offset;
    if center_size>u32::MAX as u64{return bail("ZIP64 central directory is unsupported");}
    write_u32(&mut out,0x06054b50)?;write_u16(&mut out,0)?;write_u16(&mut out,0)?;
    write_u16(&mut out,index.len() as u16)?;write_u16(&mut out,index.len() as u16)?;
    write_u32(&mut out,center_size as u32)?;write_u32(&mut out,center_offset as u32)?;write_u16(&mut out,0)?;
    Ok(())
}

pub fn zlib_stored(raw:&[u8])->Vec<u8> {
    let mut out=Vec::with_capacity(raw.len()+32+raw.len()/65535*5);
    out.extend_from_slice(&[0x78,0x01]);
    stored_blocks(&mut out,raw).expect("Vec<u8> writes cannot fail");
    let mut s1=1u32;let mut s2=0u32;
    for chunk in raw.chunks(5552) { for &b in chunk {s1=(s1+b as u32)%65521;s2=(s2+s1)%65521;} }
    out.extend_from_slice(&((s2<<16)|s1).to_be_bytes());out
}
fn stored_blocks<W:Write>(out:&mut W, data:&[u8])->io::Result<()> {
    if data.is_empty(){out.write_all(&[1,0,0,255,255])?;return Ok(());}
    for (i,chunk) in data.chunks(65535).enumerate() {
        let last=i==(data.len()-1)/65535;
        let len=chunk.len() as u16;
        out.write_all(&[if last {1}else{0}])?;
        out.write_all(&len.to_le_bytes())?;out.write_all(&(!len).to_le_bytes())?;out.write_all(chunk)?;
    }
    Ok(())
}

fn gzip_file(source:&Path,dest:&Path)->Result<()> {
    let mut input=fs::File::open(source)?;let mut out=fs::File::create(dest)?;
    out.write_all(&[0x1f,0x8b,8,0,0,0,0,0,0,3])?;
    let mut crc=!0u32;let mut count=0u32;
    let mut buf=[0u8;65535];let mut n=input.read(&mut buf)?;
    if n==0 {out.write_all(&[1,0,0,255,255])?;}
    while n!=0 {
        let mut next=[0u8;65535];let n2=input.read(&mut next)?;
        let len=n as u16;
        out.write_all(&[if n2==0 {1} else {0}])?;
        out.write_all(&len.to_le_bytes())?;out.write_all(&(!len).to_le_bytes())?;
        out.write_all(&buf[..n])?;
        crc=crc_update(crc,&buf[..n]);count=count.wrapping_add(n as u32);
        buf=next;n=n2;
    }
    out.write_all(&(!crc).to_le_bytes())?;out.write_all(&count.to_le_bytes())?;
    Ok(())
}

fn write_octal(h:&mut [u8],num:u64)->Result<()> {
    let text=format!("{:o}",num);
    if text.len()+1>h.len(){return bail("tar metadata numeric field overflow");}
    h.fill(b'0');let start=h.len()-1-text.len();h[start..start+text.len()].copy_from_slice(text.as_bytes());h[h.len()-1]=0;Ok(())
}
fn tar_header(name:&str,size:u64,mode:u64,typeflag:u8)->Result<[u8;512]> {
    let mut h=[0u8;512];
    // USTAR names longer than 100 bytes are stored using GNU longname entries.
    let bytes=name.as_bytes();h[..bytes.len().min(100)].copy_from_slice(&bytes[..bytes.len().min(100)]);
    write_octal(&mut h[100..108],mode)?;write_octal(&mut h[108..116],0)?;write_octal(&mut h[116..124],0)?;
    write_octal(&mut h[124..136],size)?;write_octal(&mut h[136..148],0)?;
    h[148..156].fill(b' ');h[156]=typeflag;h[257..263].copy_from_slice(b"ustar\0");
    h[263..265].copy_from_slice(b"00");
    let sum:u32=h.iter().map(|b|*b as u32).sum();
    let v=format!("{sum:06o}\0 ");h[148..156].copy_from_slice(v.as_bytes());
    Ok(h)
}
fn pad<W:Write>(out:&mut W,len:u64)->io::Result<()> {
    let remainder=(512-len%512)%512;if remainder>0 {out.write_all(&[0u8;512][..remainder as usize])?;}Ok(())
}
pub fn tar_gz_dir(root:&Path,dest:&Path)->Result<()> {
    let temp=dest.with_extension("tar.tmp");
    let result=(||->Result<()> {
        let mut out=fs::File::create(&temp)?;
        for source in tools::collect_files(root)? {
            let name=tools::safe_archive_name(source.strip_prefix(root)?)?;
            let mode=if source.strip_prefix(root)?.starts_with("usr/bin"){0o755}else{0o644};
            let size=fs::metadata(&source)?.len();
            if name.as_bytes().len()>100 {
                let len=name.len() as u64+1;
                out.write_all(&tar_header("././@LongLink",len,0o644,b'L')?)?;
                out.write_all(name.as_bytes())?;out.write_all(&[0])?;pad(&mut out,len)?;
            }
            out.write_all(&tar_header(&name,size,mode,b'0')?)?;
            io::copy(&mut fs::File::open(&source)?,&mut out)?;pad(&mut out,size)?;
        }
        out.write_all(&[0u8;1024])?;drop(out);
        gzip_file(&temp,dest)
    })();
    let _=fs::remove_file(temp);result
}
#[cfg(test)]mod tests {
    use super::*;
    #[test]fn compressed_data_has_expected_headers(){assert_eq!(crc32(b"123456789"),0xcbf43926);assert_eq!(&zlib_stored(b"x")[..2],&[0x78,1]);}
    #[test]fn creates_zip_with_local_and_central_headers() {
        let root=std::env::temp_dir().join(format!("mata-zip-test-{}",std::process::id()));
        let _=fs::remove_dir_all(&root);fs::create_dir_all(&root).unwrap();
        fs::write(root.join("file.txt"),"hello").unwrap();
        let dest=root.with_extension("zip");
        zip_dir(&root,&dest).unwrap();
        let bytes=fs::read(&dest).unwrap();
        assert_eq!(&bytes[..4],b"PK\x03\x04");
        assert!(bytes.windows(4).any(|x|x==b"PK\x01\x02"));
        let _=fs::remove_file(dest);let _=fs::remove_dir_all(root);
    }
}
