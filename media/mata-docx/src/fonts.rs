//! OOXML embedded font obfuscation helpers (ECMA-376 Part 1, §17.8.1).
//! This is *obfuscation*, not encryption. Callers must check embedding rights
//! using the OS/2 `fsType` bits in the font and obey its license.
use std::io::{self, Error, ErrorKind};
fn invalid(message: &str) -> io::Error { Error::new(ErrorKind::InvalidInput,message) }

/// Parse a fontKey GUID into its 16 bytes, in the UUID's displayed byte order.
pub fn font_key(guid: &str) -> io::Result<[u8;16]> {
    let hex:String=guid.trim().trim_start_matches('{').trim_end_matches('}').chars().filter(|&c|c!='-').collect();
    if hex.len()!=32{return Err(invalid("font GUID must have 32 hexadecimal digits"));}
    let mut out=[0u8;16];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot=u8::from_str_radix(&hex[i*2..i*2+2],16).map_err(|_|invalid("invalid font GUID"))?;
    }
    Ok(out)
}
/// XOR the first 32 font bytes with the font key in reverse UUID order.
/// Applying this function twice restores the original bytes.
pub fn toggle_obfuscation(bytes: &mut [u8], guid: &str) -> io::Result<()> {
    if bytes.len()<32 { return Err(invalid("embedded OpenType font must contain at least 32 bytes")); }
    let key=font_key(guid)?;
    for i in 0..32 { bytes[i]^=key[15-(i%16)]; }
    Ok(())
}
/// Create an obfuscated `.odttf` payload from raw TrueType/OpenType bytes.
pub fn obfuscate(font: &[u8], guid: &str) -> io::Result<Vec<u8>> {
    let mut data=font.to_vec();toggle_obfuscation(&mut data,guid)?;Ok(data)
}
/// Restore the original OpenType bytes from a `.odttf` font.
pub fn deobfuscate(data: &[u8], guid: &str) -> io::Result<Vec<u8>> {obfuscate(data,guid)}

/// Read the `fsType` embedding flags from a TrueType/OpenType font's OS/2
/// table. Restricted embedding (bit 1) and bitmap-only embedding (bit 9) are
/// rejected. Use the license as the final authority, not this flag alone.
pub fn embedding_allowed(font:&[u8])->io::Result<bool> {
    fn be16(x:&[u8])->u16{u16::from_be_bytes([x[0],x[1]])}
    fn be32(x:&[u8])->u32{u32::from_be_bytes([x[0],x[1],x[2],x[3]])}
    if font.len()<12 {return Err(invalid("truncated sfnt font"));}
    let count=be16(&font[4..6]) as usize;
    let directory_end=12usize.checked_add(count.checked_mul(16).ok_or_else(||invalid("sfnt overflow"))?).ok_or_else(||invalid("sfnt overflow"))?;
    if directory_end>font.len(){return Err(invalid("truncated sfnt table directory"));}
    for i in 0..count {
        let record=&font[12+i*16..12+(i+1)*16];
        if &record[..4]==b"OS/2" {
            let offset=be32(&record[8..12]) as usize;
            let length=be32(&record[12..16]) as usize;
            if length<10 || offset.checked_add(length).map(|end|end>font.len()).unwrap_or(true){return Err(invalid("truncated OS/2 table"));}
            let fs_type=be16(&font[offset+8..offset+10]);
            return Ok(fs_type & 0x0202 == 0);
        }
    }
    Err(invalid("OS/2 embedding permissions table missing"))
}
