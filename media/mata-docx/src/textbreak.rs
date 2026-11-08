//! Unicode text segmentation on grapheme boundaries, using generated Unicode
//! Line_Break, Word_Break and Sentence_Break property tables.
//!
//! These routines implement the major default rules of UAX #14 and UAX #29.
//! Tailored, dictionary-based segmenters (Thai/Lao/Khmer) are external inputs.
use crate::{ucd_textbreak as data, unicode};

fn property(ch:char,table:&[(u32,u32,u8)],names:&[&'static str])->&'static str{
    let n=ch as u32;
    let mut low=0;let mut high=table.len();
    while low<high {let i=(low+high)/2;let (a,b,v)=table[i];
        if n<a{high=i;}else if n>b{low=i+1;}else{return names[v as usize];}
    }
    names[0]
}
fn lb(ch:char)->&'static str{property(ch,data::LINE_BREAK_RANGES,data::LINE_BREAK_NAMES)}
fn wb(ch:char)->&'static str{property(ch,data::WORD_BREAK_RANGES,data::WORD_BREAK_NAMES)}
fn sb(ch:char)->&'static str{property(ch,data::SENTENCE_BREAK_RANGES,data::SENTENCE_BREAK_NAMES)}
fn has(value:&str,choices:&[&str])->bool{choices.contains(&value)}

/// Default line-break opportunities, returned as UTF-8 byte offsets (end of
/// grapheme clusters). Offsets never split a grapheme or CRLF pair.
pub fn line_breaks(text:&str)->Vec<usize>{
    let gs=unicode::graphemes(text);
    if gs.is_empty(){return vec![0];}
    let cls=gs.iter().map(|g|{
        let first=g.text.chars().next().unwrap_or(' ');
        let mut p=lb(first);
        // LB1 maps ambiguous/CJ/unknown to ordinary alphabetic or NS.
        if has(p,&["AI","SA","SG","XX"]){p="AL";}
        if p=="CJ"{p="NS";}
        p
    }).collect::<Vec<_>>();
    let mut result=Vec::new();
    for i in 0..gs.len()-1 {
        let a=cls[i];let b=cls[i+1];
        let mut allowed=true;
        if a=="CR"&&b=="LF"{allowed=false;}
        else if has(a,&["BK","CR","LF","NL"]){allowed=true;}
        else if has(b,&["BK","CR","LF","NL"]){allowed=false;}
        else if has(b,&["CM","ZWJ"]){allowed=false;}
        else if a=="ZW"{allowed=true;}
        else if a=="WJ"||b=="WJ"{allowed=false;}
        else if b=="GL"||a=="GL"&&b!="SP"{allowed=false;}
        else if b=="SP"{allowed=false;}
        else if a=="SP"{allowed=true;}
        else if has(b,&["CL","CP","EX","IS","SY"]){allowed=false;}
        else if has(a,&["OP","QU"]){allowed=false;}
        else if a=="B2"&&b=="B2"{allowed=false;}
        else if b=="BA"||b=="HY"||b=="NS"{allowed=false;}
        else if a=="BB"{allowed=false;}
        else if has(a,&["AL","HL"])&&has(b,&["AL","HL"]){allowed=false;}
        else if has(a,&["AL","HL"])&&b=="NU"||a=="NU"&&has(b,&["AL","HL"]){allowed=false;}
        else if a=="NU"&&has(b,&["NU","IS","SY","PO","PR"]){allowed=false;}
        else if has(a,&["PR","PO"])&&b=="NU"{allowed=false;}
        else if a=="RI"&&b=="RI"{
            let mut run=1;let mut j=i;while j>0&&cls[j-1]=="RI"{run+=1;j-=1;}
            allowed=run%2==0;
        }
        else if a=="JL"&&has(b,&["JL","JV","H2","H3"])||has(a,&["JV","H2"])&&has(b,&["JV","JT"])||has(a,&["JT","H3"])&&b=="JT"{allowed=false;}
        else if a=="EB"&&b=="EM"{allowed=false;}
        else if a=="HY"||a=="BA"||a=="CB"||b=="CB"{allowed=true;}
        else if a=="ID"||b=="ID"{allowed=true;}
        else {allowed=false;}
        if allowed{result.push(gs[i].byte_range.1);}
    }
    result.push(text.len());result
}

fn wb_ignored(name:&str)->bool{has(name,&["Extend","Format","ZWJ"])}
fn word_letter(name:&str)->bool{has(name,&["ALetter","Hebrew_Letter"])}
fn word_mid_letter(name:&str)->bool{has(name,&["MidLetter","MidNumLet","Single_Quote"])}
fn word_mid_number(name:&str)->bool{has(name,&["MidNum","MidNumLet","Single_Quote"])}

/// UAX #29-style word boundaries in UTF-8 byte offsets. This preserves
/// apostrophes in words, decimal separators in numbers and flag pairs.
pub fn word_boundaries(text:&str)->Vec<usize>{
    let gs=unicode::graphemes(text);
    if gs.is_empty(){return vec![0];}
    let props=gs.iter().map(|g|wb(g.text.chars().next().unwrap_or(' '))).collect::<Vec<_>>();
    let mut out=vec![0];
    for i in 1..gs.len(){
        let a=props[i-1];let b=props[i];
        let previous=(0..i-1).rev().find(|&j|!wb_ignored(props[j])).map(|j|props[j]);
        let following=((i+1)..props.len()).find(|&j|!wb_ignored(props[j])).map(|j|props[j]);
        let keep= if a=="CR"&&b=="LF"{true}
            else if has(a,&["Newline","CR","LF"])||has(b,&["Newline","CR","LF"]){false}
            else if a=="ZWJ"&&unicode::graphemes(gs[i].text).len()==1&&b=="ZWJ"{true}
            else if wb_ignored(a)||wb_ignored(b){true}
            else if a=="WSegSpace"&&b=="WSegSpace"{true}
            else if word_letter(a)&&word_letter(b){true}
            else if word_letter(a)&&word_mid_letter(b)&&following.map(word_letter).unwrap_or(false){true}
            else if word_mid_letter(a)&&word_letter(b)&&previous.map(word_letter).unwrap_or(false){true}
            else if a=="Hebrew_Letter"&&b=="Single_Quote"{true}
            else if a=="Numeric"&&b=="Numeric"{true}
            else if word_letter(a)&&b=="Numeric"||a=="Numeric"&&word_letter(b){true}
            else if a=="Numeric"&&word_mid_number(b)&&following==Some("Numeric"){true}
            else if word_mid_number(a)&&b=="Numeric"&&previous==Some("Numeric"){true}
            else if a=="Katakana"&&b=="Katakana"{true}
            else if a=="ExtendNumLet"&&has(b,&["ALetter","Hebrew_Letter","Numeric","Katakana"])
                ||b=="ExtendNumLet"&&has(a,&["ALetter","Hebrew_Letter","Numeric","Katakana"]){true}
            else if a=="Regional_Indicator"&&b=="Regional_Indicator"{
                let mut n=1;let mut j=i-1;while j>0&&props[j-1]=="Regional_Indicator"{j-=1;n+=1;}
                n%2==1
            } else{false};
        if !keep{out.push(gs[i].byte_range.0);}
    }
    out.push(text.len());out
}

/// Sentence segment boundaries, with terminal punctuation and abbreviation
/// heuristics. Language-specific abbreviation dictionaries can post-process it.
pub fn sentence_boundaries(text:&str)->Vec<usize>{
    let gs=unicode::graphemes(text);
    if gs.is_empty(){return vec![0];}
    let props=gs.iter().map(|g|sb(g.text.chars().next().unwrap_or(' '))).collect::<Vec<_>>();
    let mut out=vec![0];
    for i in 0..gs.len()-1 {
        let a=props[i];let b=props[i+1];
        if a=="CR"&&b=="LF"{continue;}
        if has(a,&["CR","LF","Sep"]){out.push(gs[i].byte_range.1);continue;}
        if !has(a,&["ATerm","STerm"]){continue;}
        let after=((i+1)..gs.len()).find(|&j|!has(props[j],&["Extend","Format","Close","Sp"]));
        let Some(j)=after else{out.push(text.len());break;};
        if a=="ATerm"&&props[j]=="Numeric"{continue;}
        if a=="ATerm"&&props[j]=="Lower"{continue;}
        if has(props[j],&["Upper","OLetter","Lower","Numeric"]){
            let before=(0..i).rev().find(|&k|!has(props[k],&["Extend","Format","Sp"]));
            if a=="ATerm"&&before.map(|k|props[k])==Some("Upper")&&props[j]=="Upper"{continue;}
        }
        out.push(gs[j].byte_range.0);
    }
    if out.last().copied()!=Some(text.len()){out.push(text.len());}
    out.sort_unstable();out.dedup();out
}
