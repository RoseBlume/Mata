//! Indic syllable boundaries used to constrain OpenType contextual lookups.
//!
//! This implements an orthographic-syllable recognizer for the nine principal
//! Indic blocks supported by mata-docx. It deliberately preserves source-byte
//! boundaries; glyph order is a separate shaping concern.
use crate::unicode::Script;
#[inline]pub(crate) fn indic(s:Script)->bool{matches!(s,Script::Devanagari|Script::Bengali|Script::Gurmukhi|Script::Gujarati|Script::Oriya|Script::Tamil|Script::Telugu|Script::Kannada|Script::Malayalam)}
/// Scripts with orthographic clusters that contextual GSUB substitutions
/// should not cross. Thai and Lao leading vowels are joined to their base.
#[inline]pub(crate) fn syllabic_script(s:Script)->bool{
 indic(s)||matches!(s,Script::Myanmar|Script::Khmer|Script::Thai|Script::Lao)
}
fn southeast_block(c:char)->Option<u32>{match c as u32{
 0x1000..=0x109f|0xaa60..=0xaa7f=>Some(0x1000),
 0x1780..=0x17ff|0x19e0..=0x19ff=>Some(0x1780),
 0x0e00..=0x0e7f=>Some(0x0e00),0x0e80..=0x0eff=>Some(0x0e80),
 _=>None}}
fn sea_base(c:char,region:u32)->bool{let n=c as u32;match region{
 0x1000=>matches!(n,0x1000..=0x102a|0x103f|0x1050..=0x1055|0xaa60..=0xaa76),
 0x1780=>matches!(n,0x1780..=0x17b3),
 0x0e00=>matches!(n,0x0e01..=0x0e2e|0x0e30|0x0e32|0x0e33|0x0e45),
 0x0e80=>matches!(n,0x0e81..=0x0eae|0x0eb0|0x0eb2|0x0eb3|0x0ec5),
 _=>false}}
fn sea_leading(c:char,region:u32)->bool{match region{
 0x0e00=>matches!(c as u32,0x0e40..=0x0e44),
 0x0e80=>matches!(c as u32,0x0ec0..=0x0ec4),
 0x1780=>matches!(c as u32,0x17c1..=0x17c5),
 _=>false}}
fn sea_mark(c:char,region:u32)->bool{let n=c as u32;match region{
 0x1000=>matches!(n,0x102b..=0x103e|0x1056..=0x1059|0x1060..=0x107d|0x1082..=0x108d|0x109a..=0x109d|0xaa7b..=0xaa7d),
 0x1780=>matches!(n,0x17b4..=0x17d3),
 0x0e00=>matches!(n,0x0e31|0x0e34..=0x0e3a|0x0e47..=0x0e4e),
 0x0e80=>matches!(n,0x0eb1|0x0eb4..=0x0ebc|0x0ec8..=0x0ece),
 _=>false}}
/// Identify Southeast Asian syllables conservatively. The algorithm keeps
/// consonant+virama/coeng+consonant clusters intact, attaches modifying
/// vowels/tones and preserves leading-vowel positions for Thai and Lao.
/// It does not perform language-specific word segmentation.
fn southeast_ranges(text:&str)->Vec<(usize,usize)>{
 let chars=text.char_indices().collect::<Vec<_>>();let mut result=Vec::new();let mut i=0;
 while i<chars.len(){
  let start=chars[i].0;let region=southeast_block(chars[i].1);
  let Some(region)=region else{
   let end=chars.get(i+1).map(|x|x.0).unwrap_or(text.len());result.push((start,end));i+=1;continue;
  };
  let mut j=i;let mut has_base=false;
  // Leading-vowel signs belong to the following base consonant.
  while j<chars.len() && sea_leading(chars[j].1,region){j+=1;}
  if j<chars.len()&&sea_base(chars[j].1,region){has_base=true;j+=1;}
  if !has_base{j=(i+1).max(j);}
  loop{
   if j>=chars.len(){break;}
   let c=chars[j].1;
   if sea_mark(c,region){
    j+=1;
    // Myanmar virama and Khmer coeng form a subjoined consonant.
    if matches!(c as u32,0x1039|0x17d2){
     if j<chars.len()&&sea_base(chars[j].1,region){j+=1;}
    }
    continue;
   }
   if matches!(c,'\u{200c}'|'\u{200d}') {j+=1;continue;}
   break;
  }
  let end=chars.get(j).map(|x|x.0).unwrap_or(text.len());result.push((start,end));i=j;
 }
 result
}
fn block(c:char)->Option<u32>{let n=c as u32;match n{
 0x0900..=0x097f=>Some(0x0900),0x0980..=0x09ff=>Some(0x0980),
 0x0a00..=0x0a7f=>Some(0x0a00),0x0a80..=0x0aff=>Some(0x0a80),
 0x0b00..=0x0b7f=>Some(0x0b00),0x0b80..=0x0bff=>Some(0x0b80),
 0x0c00..=0x0c7f=>Some(0x0c00),0x0c80..=0x0cff=>Some(0x0c80),
 0x0d00..=0x0d7f=>Some(0x0d00),_=>None}}
fn virama(c:char)->bool{matches!(c as u32,0x094d|0x09cd|0x0a4d|0x0acd|0x0b4d|0x0bcd|0x0c4d|0x0ccd|0x0d4d)}
fn nukta(c:char)->bool{matches!(c as u32,0x093c|0x09bc|0x0a3c|0x0abc|0x0b3c|0x0cbc|0x0c3c|0x0d3c)}
fn consonant(c:char)->bool{let Some(base)=block(c) else{return false;};let x=c as u32-base;
 // Most Brahmic consonant blocks follow the 0x15..0x39 layout. Tamil,
 // Malayalam and some extended letters use extra consonant ranges.
 (0x15..=0x39).contains(&x)||matches!(base,
 0x0900|0x0980|0x0a00|0x0a80|0x0b00|0x0b80|0x0c00|0x0c80|0x0d00) && matches!(x,0x58..=0x5f|0x7a..=0x7f)}
fn vowel(c:char)->bool{let Some(base)=block(c)else{return false;};let x=c as u32-base;(0x04..=0x14).contains(&x)}
fn modifier(c:char)->bool{nukta(c)||matches!(c,'\u{200c}'|'\u{200d}')||crate::unicode::matches_property(c,"M")}
/// Split Indic text into orthographic syllables without losing UTF-8 indices.
/// Non-Indic characters each form an independent group.
pub(crate) fn syllable_ranges(text:&str)->Vec<(usize,usize)>{
 let chars=text.char_indices().collect::<Vec<_>>();let mut ranges=Vec::new();let mut i=0;
 while i<chars.len(){let start=chars[i].0;let first=chars[i].1;let region=block(first);
  if region.is_none(){let end=chars.get(i+1).map(|p|p.0).unwrap_or(text.len());ranges.push((start,end));i+=1;continue;}
  let mut j=i+1;let mut last_consonant=consonant(first);
  while j<chars.len(){let c=chars[j].1;if block(c)!=region && !matches!(c,'\u{200c}'|'\u{200d}') {break;}
   if nukta(c){j+=1;continue;}
   if virama(c)&&last_consonant{j+=1;
    while j<chars.len()&&matches!(chars[j].1,'\u{200c}'|'\u{200d}') {j+=1;}
    if j<chars.len() && consonant(chars[j].1){j+=1;last_consonant=true;continue;}
    // A terminal virama belongs to the previous consonant's syllable.
    break;
   }
   if modifier(c)&&!virama(c){j+=1;continue;}
   if !last_consonant && vowel(first) && modifier(c){j+=1;continue;}
   break;
  }
  let end=chars.get(j).map(|p|p.0).unwrap_or(text.len());ranges.push((start,end));i=j;
 }
 ranges
}
/// Return the UTF-8 boundary of the orthographic syllable containing `byte`.
pub(crate) fn syllable_end(text:&str,byte:usize)->usize{
 let c=text.get(byte..).and_then(|s|s.chars().next());
 let ranges=if c.and_then(southeast_block).is_some(){southeast_ranges(text)}else{syllable_ranges(text)};
 ranges.into_iter().find(|&(a,b)|a<=byte&&byte<b).map(|(_,b)|b).unwrap_or(text.len())
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn join_devanagari_halant(){let s="क्\u{200d}ष न";let r=syllable_ranges(s);assert_eq!(&s[r[0].0..r[0].1],"क्\u{200d}ष");}
 #[test]fn split_consonants_without_virama(){let s="कख";let r=syllable_ranges(s);assert_eq!(r.len(),2);}
}
