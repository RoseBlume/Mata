//! Unicode grapheme segmentation, normalization, script/bidi helpers.
//!
//! The bundled tables are generated from Unicode 15.1.0. All four Unicode
//! normalization forms are supported via canonical/compatibility decomposition,
//! canonical combining reordering and canonical composition, including Hangul.
//! Grapheme boundaries implement UAX #29 GB3–GB13 including emoji ZWJ and
//! Indic conjunct sequences. The bidi reordering engine supports common RTL,
//! numeric and neutral text; the more unusual directional isolate/override
//! interactions and paired-bracket resolution remain approximate.
use crate::ucd_tables as data;
#[derive(Debug,Clone,Copy,PartialEq,Eq)]pub enum Direction{LeftToRight,RightToLeft,Auto}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]pub enum Script{Latin,Arabic,Hebrew,Devanagari,Bengali,Gurmukhi,Gujarati,Oriya,Tamil,Telugu,Kannada,Malayalam,Thai,Lao,Khmer,Myanmar,Syriac,Han,Hangul,Cyrillic,Common}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]pub enum Normalization{NFC,NFD,NFKC,NFKD}
#[derive(Debug,Clone,Copy)]pub struct Grapheme<'a>{pub text:&'a str,pub byte_range:(usize,usize)}
pub const UNICODE_VERSION:&str=data::UNICODE_VERSION;
fn range_property(c:u32,ranges:&[(u32,u32,u8)])->u8{
    let mut lo=0;let mut hi=ranges.len();
    while lo<hi{let mid=(lo+hi)/2;let (a,b,val)=ranges[mid];
        if c<a{hi=mid;}else if c>b{lo=mid+1;}else{return val;}
    }0
}
fn binary_contains(c:u32,ranges:&[(u32,u32)])->bool{
    let mut lo=0;let mut hi=ranges.len();
    while lo<hi{let mid=(lo+hi)/2;let (a,b)=ranges[mid];
        if c<a{hi=mid;}else if c>b{lo=mid+1;}else{return true;}
    }false
}
fn gcb(c:char)->u8{range_property(c as u32,data::GCB_RANGES)}
fn incb(c:char)->u8{range_property(c as u32,data::INCB_RANGES)}
fn pictographic(c:char)->bool{binary_contains(c as u32,data::EXTENDED_PICTOGRAPHIC)}
fn break_here(chars:&[(usize,char)],i:usize)->bool{
    let left=chars[i-1].1;let right=chars[i].1;
    let (a,b)=(gcb(left),gcb(right));
    if a==1&&b==2{return false;} // CR x LF
    if matches!(a,1|2|3)||matches!(b,1|2|3){return true;}
    if a==9&&matches!(b,9|10|12|13){return false;}
    if matches!(a,12|10)&&matches!(b,10|11){return false;}
    if matches!(a,13|11)&&b==11{return false;}
    if matches!(b,4|5|8){return false;} // Extend, ZWJ, SpacingMark
    if a==7{return false;} // Prepend
    // GB9c: Indic conjunct, consonant followed by linker/extend sequence.
    if incb(right)==1{
        let mut at=i;let mut linker=false;
        while at>0{
            at-=1;match incb(chars[at].1){3=>linker=true,2=>{},1=>{
                if linker{return false;}break;
            },_=>break}
        }
    }
    // GB11: Extended pictographic (Extend*) ZWJ x Extended pictographic.
    if pictographic(right)&&a==5{
        let mut at=i-1;
        while at>0{at-=1;if gcb(chars[at].1)==4{continue;}
            if pictographic(chars[at].1){return false;}break;
        }
    }
    // GB12/13: pairs of regional indicators form one flag grapheme.
    if a==6&&b==6{
        let mut count=0;let mut at=i;
        while at>0&&gcb(chars[at-1].1)==6{count+=1;at-=1;}
        if count%2==1{return false;}
    }
    true
}
pub fn graphemes(text:&str)->Vec<Grapheme<'_>>{
    let chars:Vec<(usize,char)>=text.char_indices().collect();let mut output=Vec::new();
    if chars.is_empty(){return output;}
    let mut start=0;
    for i in 1..chars.len(){if break_here(&chars,i){
        output.push(Grapheme{text:&text[chars[start].0..chars[i].0],byte_range:(chars[start].0,chars[i].0)});
        start=i;
    }}
    output.push(Grapheme{text:&text[chars[start].0..],byte_range:(chars[start].0,text.len())});output
}
fn ccc(ch:char)->u8{
    match data::CCC.binary_search_by_key(&(ch as u32),|(c,_)|*c){
        Ok(i)=>data::CCC[i].1,Err(_)=>0,
    }
}
fn decompose(code:u32,compatibility:bool,out:&mut Vec<char>){
    // Hangul decomposition (UAX #15 D1–D3).
    if (0xac00..=0xd7a3).contains(&code){
        let s=code-0xac00;let l=0x1100+s/588;let v=0x1161+(s%588)/28;let t=s%28;
        if let Some(c)=char::from_u32(l){out.push(c);}
        if let Some(c)=char::from_u32(v){out.push(c);}
        if t!=0{if let Some(c)=char::from_u32(0x11a7+t){out.push(c);}}
        return;
    }
    let mut entry=None;
    if compatibility{
        if let Ok(i)=data::COMPATIBILITY_DECOMP.binary_search_by_key(&code,|(n,_)|*n){entry=Some(data::COMPATIBILITY_DECOMP[i].1);}
    }
    if entry.is_none(){if let Ok(i)=data::CANONICAL_DECOMP.binary_search_by_key(&code,|(n,_)|*n){entry=Some(data::CANONICAL_DECOMP[i].1);}}
    if let Some(children)=entry{for &child in children{decompose(child,compatibility,out);}}
    else if let Some(ch)=char::from_u32(code){out.push(ch);}
}
fn compose(a:u32,b:u32)->Option<u32>{
    // Hangul canonical recomposition.
    if (0x1100..=0x1112).contains(&a)&&(0x1161..=0x1175).contains(&b){
        return Some(0xac00+(a-0x1100)*588+(b-0x1161)*28);
    }
    if (0xac00..=0xd7a3).contains(&a)&&(a-0xac00)%28==0&&(0x11a8..=0x11c2).contains(&b){return Some(a+b-0x11a7);}
    let key=(a,b);
    data::COMPOSITION.binary_search_by_key(&key,|(first,second,_)|(*first,*second)).ok().map(|i|data::COMPOSITION[i].2)
}
/// Normalize using Unicode 15.1.0 canonical/compatibility tables.
pub fn normalize(text:&str,form:Normalization)->String{
    let compatibility=matches!(form,Normalization::NFKC|Normalization::NFKD);
    let mut data=Vec::new();for c in text.chars(){decompose(c as u32,compatibility,&mut data);}
    // Stable canonical combining class reordering, constrained by starters.
    for i in 1..data.len(){let class=ccc(data[i]);if class==0{continue;}
        let mut j=i;while j>0&&ccc(data[j-1])>class{
            data.swap(j-1,j);j-=1;
        }
    }
    if matches!(form,Normalization::NFD|Normalization::NFKD){return data.into_iter().collect();}
    if data.is_empty(){return String::new();}
    let mut out=vec![data[0]];let mut starter=0;let mut previous_class=ccc(data[0]);
    for ch in data.into_iter().skip(1){
        let current_class=ccc(ch);
        if let Some(joined)=compose(out[starter] as u32,ch as u32){
            if previous_class==0||previous_class<current_class{
                if let Some(value)=char::from_u32(joined){out[starter]=value;continue;}
            }
        }
        if current_class==0{starter=out.len();}
        out.push(ch);previous_class=current_class;
    }
    out.into_iter().collect()
}
pub fn nfc(text:&str)->String{normalize(text,Normalization::NFC)}
pub fn nfd(text:&str)->String{normalize(text,Normalization::NFD)}
pub fn nfkc(text:&str)->String{normalize(text,Normalization::NFKC)}
pub fn nfkd(text:&str)->String{normalize(text,Normalization::NFKD)}
/// Backwards-compatible aliases from the initial Stage 5 implementation.
pub fn nfc_common(text:&str)->String{nfc(text)}
pub fn nfd_common(text:&str)->String{nfd(text)}
pub fn script(c:char)->Script{let n=c as u32;match n{
    0x0590..=0x05ff=>Script::Hebrew,0x0700..=0x074f=>Script::Syriac,
    0x0600..=0x06ff|0x0750..=0x08ff|0xfb50..=0xfdff|0xfe70..=0xfeff=>Script::Arabic,
    0x0900..=0x097f=>Script::Devanagari,
    0x0980..=0x09ff=>Script::Bengali,0x0a00..=0x0a7f=>Script::Gurmukhi,
    0x0a80..=0x0aff=>Script::Gujarati,0x0b00..=0x0b7f=>Script::Oriya,
    0x0b80..=0x0bff=>Script::Tamil,0x0c00..=0x0c7f=>Script::Telugu,
    0x0c80..=0x0cff=>Script::Kannada,0x0d00..=0x0d7f=>Script::Malayalam,
    0x0e00..=0x0e7f=>Script::Thai,0x0e80..=0x0eff=>Script::Lao,
    0x1780..=0x17ff=>Script::Khmer,0x1000..=0x109f=>Script::Myanmar,
    0x3400..=0x9fff|0xf900..=0xfaff=>Script::Han,
    0x1100..=0x11ff|0x3130..=0x318f|0xac00..=0xd7af=>Script::Hangul,
    0x0400..=0x052f=>Script::Cyrillic,
    0x0041..=0x005a|0x0061..=0x007a|0x00c0..=0x024f=>Script::Latin,
    _=>Script::Common,
}}
fn bidi_class(c:char)->u8{range_property(c as u32,data::BIDI_RANGES)}
pub fn strong_direction(c:char)->Option<Direction>{match bidi_class(c){1=>Some(Direction::LeftToRight),2|3=>Some(Direction::RightToLeft),_=>None}}
pub fn base_direction(text:&str,preferred:Direction)->Direction{
    match preferred{Direction::Auto=>text.chars().find_map(strong_direction).unwrap_or(Direction::LeftToRight),v=>v}
}
/// Approximate UAX #9 visual cluster order. Resolves common weak/neutral runs,
/// embedding levels and numbers while keeping each grapheme intact.
pub fn visual_order(text:&str,preferred:Direction)->Vec<usize>{
    let clusters=graphemes(text);let base=base_direction(text,preferred);
    let base_level=if base==Direction::RightToLeft{1}else{0};
    let mut levels=vec![base_level;clusters.len()];let mut classes=Vec::new();
    // (resolved level, directional override, isolate marker). Isolate markers
    // prevent PDF from accidentally closing an outer isolate scope.
    let mut stack=vec![(base_level,0u8,false)];
    for (i,g) in clusters.iter().enumerate(){
        let class=g.text.chars().map(bidi_class).find(|v|*v!=9).unwrap_or(9);
        classes.push(class);
        let (level,override_class,_)=*stack.last().unwrap();levels[i]=level;
        match class{
            15|16|17|18=>{
                let rtl=matches!(class,16|18);
                let next=if rtl{(level+1)|1}else{(level+2)&!1};
                if next<=125&&stack.len()<126{stack.push((next,if class==17{1}else if class==18{2}else{0},false));}
            },
            19=>{if stack.len()>1&&!stack.last().map(|v|v.2).unwrap_or(false){stack.pop();}},
            20|21|22=>{
                let rtl=if class==22 {
                    let mut depth=0i32;let mut direction=None;
                    for following in clusters.iter().skip(i+1){
                        let c=following.text.chars().next().unwrap_or(' ');
                        let t=bidi_class(c);
                        if matches!(t,20|21|22){depth+=1;continue;}
                        if t==23{if depth==0{break;}depth-=1;continue;}
                        if depth==0{if let Some(d)=strong_direction(c){direction=Some(d);break;}}
                    }
                    direction.unwrap_or(base)==Direction::RightToLeft
                }else{class==21};
                let next=if rtl{(level+1)|1}else{(level+2)&!1};
                if next<=125&&stack.len()<126{stack.push((next,0,true));}
            },
            23=>{
                while stack.len()>1{if let Some((_l,_o,isolate))=stack.pop(){if isolate{break;}}}
            },
            _=>{if override_class!=0{classes[i]=override_class;}}
        }
    }
    // N0: assign bracket direction based on strong text contained in a pair.
    // The pairs below cover ordinary Latin, math, CJK and quotation brackets.
    let matching=|c:char|match c{
        '('=>Some(')'), '['=>Some(']'), '{'=>Some('}'), '<'=>Some('>'),
        '«'=>Some('»'),'‹'=>Some('›'),'（'=>Some('）'),'【'=>Some('】'),
        '「'=>Some('」'),'『'=>Some('』'),_=>None,
    };
    for i in 0..clusters.len(){
        if classes[i]!=14{continue;}
        let Some(close)=clusters[i].text.chars().next().and_then(matching)else{continue;};
        let mut depth=0usize;
        for j in i+1..clusters.len(){
            let ch=clusters[j].text.chars().next().unwrap_or(' ');
            if matching(ch)==Some(close){depth+=1;}
            if ch!=close{continue;}
            if depth>0{depth-=1;continue;}
            let inner=classes[i+1..j].iter().copied().find(|v|matches!(v,1|2|3));
            if let Some(strong)=inner{classes[i]=strong;classes[j]=strong;}
            break;
        }
    }
    // W1-W3 (NSM, EN after AL, AL -> R).
    let mut previous=if base_level==1{2}else{1};let mut last_strong=previous;
    for class in &mut classes{
        if *class==9{*class=previous;}
        if *class==4 && last_strong==3 {*class=5;}
        if *class==3{*class=2;}
        if matches!(*class,1|2|3){last_strong=*class;}
        previous=*class;
    }
    // W4-W6: separators/terminators between numbers.
    for i in 0..classes.len(){
        let prev=if i>0{classes[i-1]}else{0};let next=classes.get(i+1).copied().unwrap_or(0);
        if (classes[i]==6&&prev==4&&next==4)||(classes[i]==7&&((prev==4&&next==4)||(prev==5&&next==5))){classes[i]=prev;}
        else if classes[i]==8 && (prev==4||next==4){classes[i]=4;}
        else if matches!(classes[i],6|7|8){classes[i]=14;}
    }
    // W7: numbers following a strong L take L direction.
    let mut last=if base_level==1{2}else{1};
    for class in &mut classes {if matches!(*class,1|2){last=*class;}if *class==4&&last==1{*class=1;}}
    // Resolve neutrals between strong neighboring directions.
    for i in 0..classes.len(){
        if matches!(classes[i],11|12|13|14|10){
            let preceding=classes[..i].iter().rev().copied().find(|c|matches!(c,1|2|4|5));
            let following=classes[i+1..].iter().copied().find(|c|matches!(c,1|2|4|5));
            let dir=|c:u8|if c==1{1}else{2};
            classes[i]=match (preceding,following){(Some(a),Some(b)) if dir(a)==dir(b)=>dir(a),_=>if levels[i]%2==0{1}else{2}};
        }
    }
    for (i,c) in classes.iter().copied().enumerate(){
        if levels[i]%2==0{
            if c==2{levels[i]+=1;}else if matches!(c,4|5){levels[i]+=2;}
        }else if matches!(c,1|4|5){levels[i]+=1;}
    }
    let mut order=(0..clusters.len()).collect::<Vec<_>>();
    let max=*levels.iter().max().unwrap_or(&0);
    let min_odd=levels.iter().copied().filter(|l|l%2==1).min();
    if let Some(low)=min_odd{
        for level in (low..=max).rev(){let mut start=0;
            while start<order.len(){
                if levels[order[start]]<level{start+=1;continue;}
                let mut end=start+1;while end<order.len()&&levels[order[end]]>=level{end+=1;}
                order[start..end].reverse();start=end;
            }
        }
    }
    order
}
pub fn visual_text(text:&str,direction:Direction)->String{
    let clusters=graphemes(text);
    visual_order(text,direction).into_iter().filter_map(|i|{
        let text=clusters[i].text;
        if text.chars().all(|c|matches!(bidi_class(c),15..=23|10)){None}else{Some(text)}
    }).collect()
}
/// Basic Unicode line break opportunities used by the layout engine.
/// UAX #14 dictionary segmentation and language-specific hyphenation still
/// require a separate dictionary component.
pub fn line_breaks(text:&str)->Vec<usize>{crate::textbreak::line_breaks(text)}

/// Word-break boundary indices in UTF-8 bytes.
pub fn word_boundaries(text:&str)->Vec<usize>{crate::textbreak::word_boundaries(text)}
/// Sentence-break boundary indices in UTF-8 bytes.
pub fn sentence_boundaries(text:&str)->Vec<usize>{crate::textbreak::sentence_boundaries(text)}

/// Unicode general-category abbreviation (such as `Lu`, `Mn`, `Po`).
pub fn general_category(c:char)->&'static str{
    let n=c as u32;let rows=crate::ucd_regex::GENERAL_CATEGORY;
    let mut lo=0;let mut hi=rows.len();
    while lo<hi{let i=(lo+hi)/2;let (from,to,k)=rows[i];
        if n<from{hi=i;}else if n>to{lo=i+1;}
        else{return crate::ucd_regex::GENERAL_CATEGORY_NAMES[k as usize];}
    }
    "Cn"
}
/// Check a PCRE2 Unicode property, including UCD General_Category and all
/// Script/Script_Extensions aliases in the bundled Unicode 16.0 script tables.
pub fn property_name_valid(property:&str)->bool{
    let raw=property.trim().trim_start_matches('^');
    let (kind,value)=raw.split_once('=').or_else(||raw.split_once(':'))
        .map(|(k,v)|(Some(k),v)).unwrap_or((None,raw));
    let value=value.chars().filter(|c|!matches!(c,'_'|'-'|' '| '\t'))
        .flat_map(char::to_lowercase).collect::<String>();
    let kind=kind.map(|k|k.chars().filter(|c|!matches!(c,'_'|'-'|' '))
        .flat_map(char::to_lowercase).collect::<String>());
    if let Some(k)=kind.as_deref(){
        match k {
            "sc"|"script"|"scx"|"scriptextensions"=>return crate::pcre_script_data::script_name_id(&value).is_some(),
            "gc"|"generalcategory"=>return category_valid(&value),
            _=>return false,
        }
    }
    category_valid(&value)||crate::pcre_script_data::script_name_id(&value).is_some()
}
fn category_valid(value:&str)->bool{
    let names=["l","letter","alphabetic","m","mark","n","number","p","punctuation",
        "s","symbol","z","separator","c","other","whitespace","emoji",
        "extendedpictographic","any","assigned","xan","xwd","xps","xsp","xuc"];
    names.contains(&value)||crate::ucd_regex::GENERAL_CATEGORY_NAMES
        .iter().any(|c|c.eq_ignore_ascii_case(value))
}
/// Match Unicode properties with script extensions and loose alias matching.
pub fn matches_property(ch:char,property:&str)->bool{
    let raw=property.trim().trim_start_matches('^');
    let (kind,value)=raw.split_once('=').or_else(||raw.split_once(':'))
        .map(|(k,v)|(Some(k),v)).unwrap_or((None,raw));
    let value=value.chars().filter(|c|!matches!(c,'_'|'-'|' '| '\t'))
        .flat_map(char::to_lowercase).collect::<String>();
    let kind=kind.map(|k|k.chars().filter(|c|!matches!(c,'_'|'-'|' '))
        .flat_map(char::to_lowercase).collect::<String>());
    if !matches!(kind.as_deref(),None|Some("gc"|"generalcategory")) {
        if !matches!(kind.as_deref(),Some("sc"|"script"|"scx"|"scriptextensions")){return false;}
    }
    if matches!(kind.as_deref(),None|Some("sc"|"script"|"scx"|"scriptextensions")) {
        if let Some(id)=crate::pcre_script_data::script_name_id(&value) {
            let (basic,extensions)=crate::pcre_script_data::scripts(ch as u32);
            return if matches!(kind.as_deref(),Some("sc"|"script")) {basic==id}
                else{extensions.map(|sets|sets.contains(&id)).unwrap_or(basic==id)};
        }
    }
    let category=general_category(ch).to_ascii_lowercase();
    if value==category{return true;}
    if value.len()==1&&matches!(value.as_str(),"l"|"m"|"n"|"p"|"s"|"z"|"c"){
        return category.starts_with(&value);
    }
    match value.as_str(){
        "any"=>true,"assigned"=>category!="cn",
        "letter"|"alphabetic"=>ch.is_alphabetic(),
        "mark"=>category.starts_with('m'),"number"=>category.starts_with('n'),
        "punctuation"=>category.starts_with('p'),"symbol"=>category.starts_with('s'),
        "separator"=>category.starts_with('z'),"other"=>category.starts_with('c'),
        "whitespace"|"xps"|"xsp"=>ch.is_whitespace(),
        "emoji"|"extendedpictographic"=>pictographic(ch),
        "xan"=>ch.is_alphanumeric(),
        "xwd"=>ch.is_alphanumeric()||ch=='_'||category.starts_with('m'),
        "xuc"=>ch=='\u{200c}'||ch=='\u{200d}'||ch=='_'||ch.is_alphanumeric(),
        _=>false,
    }
}
