//! PubChem PC-Compound, PC-Compounds, and PC-Substance structure I/O.
//!
//! ASNT is ASN.1 *text*, not JSON. ASN/PC3D may be ASN.1 text or BER binary.
//! Only structure fields expressible in `Molecule` can survive import/export:
//! atom elements/formal charges, bonds, coordinates, and separate conformers.
//! Unsupported stereochemistry and descriptor data must not be mistaken for
//! structure coordinates or formal charge information.
use crate::chemistry::{atomic_number, Atom, Bond, Molecule, ELEMENTS};
use mata_3d::Vec3;
use std::collections::HashMap;

const MAX_FILE: usize = 128 * 1024 * 1024;
const MAX_CONFORMERS: usize = 4096;

#[derive(Debug, Clone)]
enum Value { Scalar(String), List(Vec<Value>), Field(String, Box<Value>) }
impl Value {
    fn field(&self, key:&str)->Option<&Self>{
        match self { Self::List(entries)=>entries.iter().find_map(|entry| match entry {Self::Field(name,v) if name==key=>Some(v.as_ref()), _=>None}), Self::Field(name,v) if name==key=>Some(v.as_ref()), _=>None }
    }
    fn items(&self)->&[Value]{match self {Self::List(xs)=>xs, _=>&[]}}
    fn as_text(&self)->Option<&str>{match self {Self::Scalar(s)=>Some(s), Self::Field(_,v)=>v.as_text(), _=>None}}
    fn as_i64(&self)->Option<i64>{let s=self.as_text()?;s.parse().ok().or_else(|| match s.to_ascii_lowercase().as_str(){"deposited"=>Some(0),"standardized"=>Some(1),"twod"=>Some(1),"threed"=>Some(2),"units-angstroms"=>Some(10),"units-nanometers"=>Some(11),"single"=>Some(1),"double"=>Some(2),"triple"=>Some(3),"quadruple"=>Some(4),"aromatic"=>Some(8),_=>None})}
    fn as_float(&self)->Option<f32>{self.as_text()?.parse::<f32>().ok().filter(|x|x.is_finite())}
}

#[derive(Clone,Debug,PartialEq)]
enum Token{Word(String), Open,Close,Comma,Assign}
fn tokenize(src:&str)->Result<Vec<Token>,String>{
    let b=src.as_bytes();let mut out=Vec::new();let mut i=0usize;
    while i<b.len(){
        if b[i].is_ascii_whitespace(){i+=1;continue;}
        if b[i]==b'-'&&b.get(i+1)==Some(&b'-') {while i<b.len()&&b[i]!=b'\n'{i+=1;}continue;}
        if b[i..].starts_with(b"::="){out.push(Token::Assign);i+=3;continue;}
        match b[i]{
            b'{'=>{out.push(Token::Open);i+=1;},b'}'=>{out.push(Token::Close);i+=1;},b','=>{out.push(Token::Comma);i+=1;},
            b'"'=>{i+=1;let mut s=String::new();let mut closed=false;
                while i<b.len(){if b[i]==b'"'{if b.get(i+1)==Some(&b'"'){s.push('"');i+=2;}else{i+=1;closed=true;break;}}else{let start=i;while i<b.len()&&b[i]!=b'"'{i+=1;}s.push_str(std::str::from_utf8(&b[start..i]).map_err(|_|"ASNT: invalid UTF-8 string")?);}}
                if !closed{return Err("ASNT: unterminated string".into());}out.push(Token::Word(s));
            },
            _=>{let start=i;while i<b.len()&&!b[i].is_ascii_whitespace()&&!matches!(b[i],b'{'|b'}'|b','|b'"')&&!b[i..].starts_with(b"::="){
                if b[i]==b'-'&&b.get(i+1)==Some(&b'-'){break;}i+=1;
            }
            if start==i{return Err("ASNT: unexpected character".into());}
            out.push(Token::Word(std::str::from_utf8(&b[start..i]).map_err(|_|"ASNT: invalid UTF-8 token")?.to_string()));
            }
        }
        if out.len()>3_000_000{return Err("ASNT: too many tokens".into());}
    }
    Ok(out)
}
struct TextParser{tokens:Vec<Token>,i:usize}
impl TextParser {
    fn value(&mut self, depth:usize)->Result<Value,String>{
        if depth>48{return Err("ASNT: nesting depth exceeded".into());}
        let cur=self.tokens.get(self.i).ok_or("ASNT: truncated value")?.clone();self.i+=1;
        match cur {
            Token::Open=>{
                let mut entries=Vec::new();
                while self.tokens.get(self.i)!=Some(&Token::Close){
                    if self.i>=self.tokens.len(){return Err("ASNT: missing }".into());}
                    if self.tokens.get(self.i)==Some(&Token::Comma){self.i+=1;continue;}
                    let first=self.value(depth+1)?;
                    entries.push(first);
                    if self.tokens.get(self.i)==Some(&Token::Comma){self.i+=1;}
                    if entries.len()>1_000_000{return Err("ASNT: too many sequence entries".into());}
                }
                self.i+=1;Ok(Value::List(entries))
            },
            Token::Word(s)=>{
                // A value starting with a word and immediately followed by a
                // value is an ASN.1 field or CHOICE: `atoms { ... }`, `id cid 12`.
                match self.tokens.get(self.i) {
                    Some(Token::Word(_))|Some(Token::Open)=>Ok(Value::Field(s,Box::new(self.value(depth+1)?))),
                    _=>Ok(Value::Scalar(s)),
                }
            },
            _=>Err("ASNT: expected value".into()),
        }
    }
    fn documents(mut self)->Result<Vec<(String,Value)>,String>{
        let mut result=Vec::new();
        while self.i<self.tokens.len(){
            let typename=match self.tokens.get(self.i){Some(Token::Word(t))=>t.clone(),_=>return Err("ASNT: expected record type".into())};self.i+=1;
            if self.tokens.get(self.i)!=Some(&Token::Assign){return Err("ASNT: expected ::= after record type".into());}self.i+=1;
            let value=self.value(0)?;
            result.push((typename,value));
        }
        Ok(result)
    }
}
fn values(v:Option<&Value>)->Result<Vec<i64>,String>{
    let v=v.ok_or("PubChem: missing integer vector")?;
    v.items().iter().map(|x|x.as_i64().ok_or_else(||format!("PubChem: invalid integer/enum {:?}",x))).collect()
}
fn floats(v:Option<&Value>)->Result<Vec<f32>,String>{
    let v=v.ok_or("PubChem: missing coordinate vector")?;
    v.items().iter().map(|x|x.as_float().ok_or_else(||"PubChem: invalid coordinate".to_string())).collect()
}
fn element(n:i64)->Result<&'static str,String>{
    if !(1..=118).contains(&n){return Err(format!("PubChem: unsupported atomic number {n}"));}
    Ok(ELEMENTS.split_whitespace().nth(n as usize-1).unwrap())
}
fn numeric_element(v:&Value)->Option<i64>{
    if let Some(n)=v.as_i64(){return Some(n);}
    let text=v.as_text()?.trim();
    let normalized={let mut chars=text.chars();let first=chars.next()?.to_ascii_uppercase();let mut s=first.to_string();s.push_str(&chars.as_str().to_ascii_lowercase());s};
    atomic_number(&normalized).map(|n|n as i64)
}
fn name_of_record(root:&Value,index:usize)->String{
    let id=root.field("id");
    let lookup=id.and_then(|id|id.field("id")).or(id);
    if let Some(id)=lookup{
        for typ in ["cid","sid","xid"] {if let Some(n)=id.field(typ).and_then(Value::as_i64){return format!("PubChem {typ} {n}");}}
    }
    format!("PubChem structure {}",index+1)
}
fn molecule_from_compound(root:&Value,index:usize)->Result<Vec<Molecule>,String>{
    let atoms=root.field("atoms").ok_or("PubChem: PC-Compound has no atoms")?;
    let ids=values(atoms.field("aid"))?;
    let elements=atoms.field("element").ok_or("PubChem: no element list")?;
    if ids.is_empty()||ids.len()!=elements.items().len(){return Err("PubChem: atom ID/element array length mismatch".into());}
    if ids.len()>100_000{return Err("PubChem: too many atoms".into());}
    let mut base=Molecule::new(&name_of_record(root,index));
    let mut by_id=HashMap::with_capacity(ids.len());
    for (i,(&id,el)) in ids.iter().zip(elements.items()).enumerate(){
        if id<=0||by_id.insert(id,i).is_some(){return Err("PubChem: invalid or duplicate atom identifier".into());}
        let z=numeric_element(el).ok_or("PubChem: unknown element")?;
        base.atoms.push(Atom::new(element(z)?));
    }
    if let Some(charges)=atoms.field("charge") {
        for entry in charges.items(){
            let id=entry.field("aid").and_then(Value::as_i64).ok_or("PubChem: charge missing aid")?;
            let q=entry.field("value").and_then(Value::as_i64).ok_or("PubChem: charge missing value")?;
            if !(-8..=8).contains(&q){return Err("PubChem: atom charge outside supported range".into());}
            let i=*by_id.get(&id).ok_or("PubChem: charge refers to unknown atom")?;base.atoms[i].charge=q as i8;
        }
    }
    if let Some(bonds)=root.field("bonds"){
        let a=values(bonds.field("aid1"))?;let b=values(bonds.field("aid2"))?;let o=values(bonds.field("order"))?;
        if a.len()!=b.len()||a.len()!=o.len(){return Err("PubChem: bond array length mismatch".into());}
        if a.len()>400_000{return Err("PubChem: too many bonds".into());}
        for ((x,y),ord) in a.iter().zip(&b).zip(&o){
            // In PubChem's PC-BondType 4 is QUADRUPLE, not aromatic. Never
            // silently mislabel it as aromatic (our model uses 4 for aromatic).
            if !(1..=3).contains(ord){return Err(format!("PubChem: bond type {ord} cannot be represented (4 means quadruple, not aromatic)"));}
            let i=*by_id.get(x).ok_or("PubChem: bond aid1 not present")?;
            let j=*by_id.get(y).ok_or("PubChem: bond aid2 not present")?;
            base.bonds.push(Bond{a:i,b:j,order:*ord as u8});
        }
    }
    base.verify()?;
    let mut result=Vec::new();
    if let Some(coords)=root.field("coords") {
        for coordset in coords.items(){
            let coord_ids=values(coordset.field("aid"))?;
            let ctype=coordset.field("type");
            let scale=if ctype.is_some_and(|v|v.items().iter().any(|n|n.as_i64()==Some(11))){10.0}else{1.0};
            if let Some(conformers)=coordset.field("conformers") {
                for (ix,conf) in conformers.items().iter().enumerate(){
                    if result.len()>=MAX_CONFORMERS{return Err("PubChem: too many conformers".into());}
                    let xs=floats(conf.field("x"))?;let ys=floats(conf.field("y"))?;
                    let zs=if conf.field("z").is_some(){floats(conf.field("z"))?}else{vec![0.0;xs.len()]};
                    if xs.len()!=coord_ids.len()||xs.len()!=ys.len()||xs.len()!=zs.len(){return Err("PubChem: conformer coordinates/atom IDs length mismatch".into());}
                    let mut m=base.clone();
                    for (j,id) in coord_ids.iter().enumerate(){
                        let at=*by_id.get(id).ok_or("PubChem: coordinates refer to unknown atom")?;
                        m.atoms[at].pos=Vec3::new(xs[j]*scale,ys[j]*scale,zs[j]*scale);
                    }
                    // Annotation 8 = aromatic; it is distinct from PC-BondType.
                    if let Some(style)=conf.field("style") {
                        let anns=values(style.field("annotation"))?;
                        let a1=values(style.field("aid1"))?;let a2=values(style.field("aid2"))?;
                        if anns.len()!=a1.len()||anns.len()!=a2.len(){return Err("PubChem: style annotation arrays differ in length".into());}
                        for ((kind,a),b) in anns.iter().zip(a1).zip(a2){if *kind==8{
                            let i=*by_id.get(&a).ok_or("PubChem: aromatic annotation atom missing")?;
                            let j=*by_id.get(&b).ok_or("PubChem: aromatic annotation atom missing")?;
                            if let Some(bond)=m.bonds.iter_mut().find(|bond|(bond.a==i&&bond.b==j)||(bond.a==j&&bond.b==i)){bond.order=4;}
                        }}
                    }
                    if !base.name.starts_with("PubChem structure")||conformers.items().len()>1||coords.items().len()>1 {m.name=format!("{} - conformer {}",base.name,ix+1);}
                    m.verify()?;result.push(m);
                }
            }
        }
    }
    if result.is_empty(){result.push(base);}
    Ok(result)
}
fn collect_compounds(root:&Value,out:&mut Vec<Molecule>)->Result<(),String>{
    if root.field("atoms").is_some(){
        let mut ms=molecule_from_compound(root,out.len())?;
        if out.len()+ms.len()>MAX_CONFORMERS{return Err("PubChem: too many structures".into());}
        out.append(&mut ms);
    }else{
        // Descend only via PC-Compounds / PC-Substance `compound` lists,
        // not arbitrary descriptor fields that could resemble structures.
        if let Some(v)=root.field("compound") {collect_compounds(v,out)?;}
        else {for v in root.items(){collect_compounds(v,out)?;}}
    }
    Ok(())
}

/// PubChem Protein/Gene/Target information records use `Record ::= ...`.
/// Unlike PC-Compound records, they describe biological annotations and may
/// include an amino-acid sequence, but do not carry atom coordinates.
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct ProteinRecord {
    pub record_type: String,
    pub accession: String,
    pub title: String,
    pub url: String,
    pub sequence: String,
    pub structure_links: Vec<(String,String)>,
}

fn collect_sections<'a>(v:&'a Value, title:&str, found:&mut Vec<&'a Value>) {
    if v.field("tT").and_then(|x|x.field("tOCHeading")).and_then(Value::as_text)==Some(title) {
        found.push(v);
    }
    match v {
        Value::List(xs)=>for x in xs {collect_sections(x,title,found)},
        Value::Field(_,inner)=>collect_sections(inner,title,found),
        Value::Scalar(_)=>{},
    }
}
fn collect_named_strings<'a>(v:&'a Value, field:&str, found:&mut Vec<&'a str>) {
    match v {
        Value::Field(name,inner)=>{
            if name==field {if let Some(s)=inner.as_text(){found.push(s);}}
            collect_named_strings(inner,field,found);
        },
        Value::List(xs)=>for x in xs {collect_named_strings(x,field,found)},
        Value::Scalar(_)=>{},
    }
}
/// Returns None for ordinary PC-Compound, BER, or unrelated ASN.1 files.
/// Returns Some only for text ASN.1 with a `Record` root.
pub fn read_protein_record(data:&[u8])->Result<Option<ProteinRecord>,String>{
    if data.len()>MAX_FILE{return Err("PubChem: file exceeds 128 MiB limit".into());}
    let Ok(text)=std::str::from_utf8(data) else{return Ok(None)};
    let text=text.trim_start_matches('\u{feff}').trim_start();
    // Avoid parsing unrelated ASN.1 structures or non-ASN plain text.
    if !text.starts_with("Record ::=") && !text.starts_with("Record::=") {return Ok(None);}
    let docs=TextParser{tokens:tokenize(text)?,i:0}.documents()?;
    if docs.len()!=1 || docs[0].0!="Record" {return Err("PubChem Record: expected exactly one Record document".into());}
    let root=&docs[0].1;
    let scalar=|key:&str|root.field(key).and_then(Value::as_text).unwrap_or("").to_string();
    let record_type=scalar("recordType");
    let accession=root.field("rR").and_then(|v|v.field("recordAccession"))
        .or_else(||root.field("recordAccession"))
        .and_then(Value::as_text).unwrap_or("").to_string();
    let title=scalar("recordTitle");
    let url=scalar("recordExternalURL");
    if record_type.is_empty() && accession.is_empty() && title.is_empty(){
        return Err("PubChem Record: no recognizable record fields".into());
    }
    let mut sequence=String::new();
    let mut sections=Vec::new();collect_sections(root,"Sequence",&mut sections);
    for section in sections {
        let mut strings=Vec::new();collect_named_strings(section,"string",&mut strings);
        for s in strings {
            let candidate=s.chars().filter(|c|!c.is_ascii_whitespace()).collect::<String>();
            if candidate.len()>sequence.len() && candidate.len()>=20 &&
                candidate.bytes().all(|b|matches!(b,b'A'|b'C'|b'D'|b'E'|b'F'|b'G'|b'H'|b'I'|b'K'|b'L'|b'M'|b'N'|b'P'|b'Q'|b'R'|b'S'|b'T'|b'V'|b'W'|b'Y'|b'X'|b'U'|b'O'|b'B'|b'Z'|b'J')) {
                sequence=candidate;
            }
        }
    }
    // Prefer accession-specific links from the record's reference block;
    // avoid displaying only database homepages if an exact record link exists.
    let mut urls=Vec::new();collect_named_strings(root,"uRL",&mut urls);
    let clean=urls.into_iter().filter(|s|s.starts_with("https://"))
        .map(|s|s.chars().filter(|c|*c!='\n' && *c!='\r').collect::<String>())
        .collect::<Vec<_>>();
    let mut structure_links=Vec::new();
    for (label,preferred,backup) in [
        ("PDB Structures","rcsb.org/pdb/protein/","rcsb.org"),
        ("NCBI Protein Structures","ncbi.nlm.nih.gov/structure?","ncbi.nlm.nih.gov/Structure/MMDB"),
        ("AlphaFold Structures","alphafold.ebi.ac.uk/entry/","alphafold.ebi.ac.uk"),
    ] {
        if let Some(url)=clean.iter().find(|s|s.contains(preferred))
            .or_else(||clean.iter().find(|s|s.contains(backup))) {
            structure_links.push((label.into(),url.clone()));
        }
    }
    Ok(Some(ProteinRecord{record_type,accession,title,url,sequence,structure_links}))
}

pub fn read_pubchem(data:&[u8])->Result<Vec<Molecule>,String>{
    if data.len()>MAX_FILE{return Err("PubChem: file exceeds 128 MiB limit".into());}
    if data.starts_with(&[0x1f,0x8b]) {return Err("PubChem: gzip compressed ASN files require decompression first".into());}
    let mut out=Vec::new();
    // ASCII text ASN.1 may be stored with any of .asn/.asnt/.pc3d.
    if let Ok(src)=std::str::from_utf8(data){
        let src=src.trim_start_matches('\u{feff}').trim_start();
        // NCBI ASNT may begin with `--` comments.
        let looks_asnt=src.starts_with("PC-")||src.starts_with("Record")||src.starts_with("--");
        if looks_asnt{
            for (typ,root) in (TextParser{tokens:tokenize(src)?,i:0}).documents()?{
                if !matches!(typ.as_str(),"PC-Compound"|"PC-Compounds"|"PC-Substance"|"PC-Substances") {
                    return Err(if typ=="Record" {"PubChem Record: this is an annotation/sequence record, not atom coordinates. Open with read_document() or import an actual PDB/mmCIF structure.".into()} else {format!("PubChem: unsupported root {typ}; expected PC-Compound(s)/PC-Substance(s)")});
                }
                collect_compounds(&root,&mut out)?;
            }
        }
    }
    if out.is_empty(){
        if let Ok(src)=std::str::from_utf8(data){
            if src.trim_start_matches('\u{feff}').trim_start().starts_with("PC-"){
                return Err("PubChem ASN.1 text: no PC-Compound structures found".into());
            }
        }
        out=read_binary(data)?;
    }
    if out.is_empty(){return Err("PubChem: no PC-Compound structures found".into());}
    for m in &out{m.verify()?;}Ok(out)
}

fn num_list(xs:impl IntoIterator<Item=i64>)->String{xs.into_iter().map(|n|n.to_string()).collect::<Vec<_>>().join(", ")}
fn real_list(xs:impl IntoIterator<Item=f32>)->String{xs.into_iter().map(|x|format!("{x:.7}")).collect::<Vec<_>>().join(", ")}
fn compound_text(m:&Molecule)->Result<String,String>{
    let ids=(1..=m.atoms.len()).map(|i|i as i64).collect::<Vec<_>>();
    let elements=m.atoms.iter().map(|a|atomic_number(&a.symbol).unwrap() as i64).collect::<Vec<_>>();
    let mut buf=String::from("{\n  id { type deposited },\n  atoms {\n    aid { ");
    buf.push_str(&num_list(ids.iter().copied()));
    buf.push_str(" },\n    element { ");buf.push_str(&num_list(elements));buf.push_str(" }");
    let charges=m.atoms.iter().enumerate().filter(|(_,a)|a.charge!=0).map(|(i,a)|format!("{{ aid {}, value {} }}",i+1,a.charge)).collect::<Vec<_>>();
    if !charges.is_empty(){buf.push_str(",\n    charge { ");buf.push_str(&charges.join(", "));buf.push_str(" }");}
    buf.push_str("\n  }");
    if !m.bonds.is_empty(){
        let a1=num_list(m.bonds.iter().map(|b|(b.a+1) as i64));
        let a2=num_list(m.bonds.iter().map(|b|(b.b+1) as i64));
        let orders=num_list(m.bonds.iter().map(|b|if b.order==4{1_i64}else{b.order as i64}));
        buf.push_str(",\n  bonds { aid1 { ");buf.push_str(&a1);
        buf.push_str(" }, aid2 { ");buf.push_str(&a2);
        buf.push_str(" }, order { ");buf.push_str(&orders);buf.push_str(" } }");
    }
    let has_z=m.atoms.iter().any(|a|a.pos.z.abs()>0.0001);
    buf.push_str(",\n  coords {\n    { type { ");buf.push_str(if has_z{"threed"}else{"twod"});
    buf.push_str(", units-angstroms },\n      aid { ");buf.push_str(&num_list(ids));
    buf.push_str(" },\n      conformers {\n        { x { ");
    buf.push_str(&real_list(m.atoms.iter().map(|a|a.pos.x)));
    buf.push_str(" },\n          y { ");buf.push_str(&real_list(m.atoms.iter().map(|a|a.pos.y)));buf.push_str(" }");
    if has_z{buf.push_str(",\n          z { ");buf.push_str(&real_list(m.atoms.iter().map(|a|a.pos.z)));buf.push_str(" }");}
    let arom=m.bonds.iter().filter(|b|b.order==4).collect::<Vec<_>>();
    if !arom.is_empty(){
        buf.push_str(",\n          style { annotation { ");buf.push_str(&vec!["aromatic";arom.len()].join(", "));
        buf.push_str(" }, aid1 { ");buf.push_str(&num_list(arom.iter().map(|b|(b.a+1) as i64)));
        buf.push_str(" }, aid2 { ");buf.push_str(&num_list(arom.iter().map(|b|(b.b+1) as i64)));
        buf.push_str(" } }");
    }
    buf.push_str(" }\n      }\n    }\n  }\n}");
    Ok(buf)
}
/// Export NCBI-compatible human-readable ASN.1 text. Arbitrary properties,
/// stereochemistry and parent record IDs are not retained by `Molecule`.
pub fn write_pubchem_text(structures:&[Molecule])->Result<String,String>{
    if structures.is_empty(){return Err("PubChem: nothing to export".into());}
    for m in structures{m.verify()?;}
    if structures.len()==1{return Ok(format!("PC-Compound ::= {}\n",compound_text(&structures[0])?));}
    let items=structures.iter().map(compound_text).collect::<Result<Vec<_>,_>>()?;
    Ok(format!("PC-Compounds ::= {{\n{}\n}}\n",items.join(",\n")))
}

// BER support is below. All TLVs are length-checked, bounded, and parsed
// without dependencies or arbitrary decompression/network I/O.
#[derive(Debug,Clone)]
struct Ber<'a>{tag:u8,body:&'a [u8],children:Vec<Ber<'a>>}
fn parse_ber<'a>(buf:&'a [u8],start:&mut usize,depth:usize)->Result<Ber<'a>,String>{
    if depth>48{return Err("PubChem BER: depth limit".into());}
    let tag=*buf.get(*start).ok_or("PubChem BER: truncated tag")?;*start+=1;
    if tag&0x1f==0x1f{return Err("PubChem BER: long-form tags are unsupported".into());}
    let b=*buf.get(*start).ok_or("PubChem BER: truncated length")?;*start+=1;
    let length=if b==0x80{None}else if b&0x80==0{Some(b as usize)}else{
        let n=(b&0x7f) as usize;
        if n==0||n>4||*start+n>buf.len(){return Err("PubChem BER: invalid length".into());}
        let mut len=0usize;for &digit in &buf[*start..*start+n]{len=len.checked_mul(256).and_then(|x|x.checked_add(digit as usize)).ok_or("PubChem BER: length overflow")?;}*start+=n;Some(len)
    };
    let body_start=*start;
    let mut children=Vec::new();
    if tag&0x20!=0{
        if let Some(n)=length{let end=(*start).checked_add(n).ok_or("PubChem BER: length overflow")?;if end>buf.len(){return Err("PubChem BER: truncated body".into());}while *start<end{children.push(parse_ber(buf,start,depth+1)?);if children.len()>1_000_000{return Err("PubChem BER: too many fields".into());}}if *start!=end{return Err("PubChem BER: invalid child length".into());}}
        else {loop{if buf.get(*start..*start+2)==Some(&[0,0][..]){*start+=2;break;}if *start>=buf.len(){return Err("PubChem BER: unterminated sequence".into());}children.push(parse_ber(buf,start,depth+1)?);}}
    }else {let n=length.ok_or("PubChem BER: primitive with indefinite length")?;*start=(*start).checked_add(n).ok_or("PubChem BER: length overflow")?;if *start>buf.len(){return Err("PubChem BER: truncated primitive".into());}}
    Ok(Ber{tag,body:&buf[body_start..*start],children})
}
impl Ber<'_>{
    fn unwrapped(&self)->&Self{if self.children.len()==1&&self.children[0].tag==0x30{&self.children[0]}else{self}}
    fn field(&self,n:u8)->Option<&Self>{self.unwrapped().children.iter().find(|x|x.tag&0xc0==0x80&&x.tag&31==n)}
    fn seq_items(&self)->&[Self]{&self.unwrapped().children}
    fn int(&self)->Result<i64,String>{
        if self.tag&0x20!=0 {if self.children.len()==1{return self.children[0].int();}return Err("PubChem BER: expected integer primitive".into());}
        let b=self.body;if b.is_empty()||b.len()>8{return Err("PubChem BER: invalid integer length".into());}
        let mut n=if b[0]&0x80!=0{-1_i64}else{0};for &x in b{n=(n<<8)|(x as i64);}Ok(n)
    }
    fn real(&self)->Result<f32,String>{
        if self.tag&0x20!=0&&self.children.len()==1{return self.children[0].real();}
        let b=self.body;if b.is_empty(){return Ok(0.0);}
        if matches!(b[0],1|2|3){return std::str::from_utf8(&b[1..]).map_err(|_|"PubChem BER: REAL not UTF-8")?.parse::<f32>().map_err(|_|"PubChem BER: invalid decimal REAL".into());}
        if b[0]==0x40{return Ok(f32::INFINITY)}if b[0]==0x41{return Ok(f32::NEG_INFINITY)}
        if b[0]&0x80!=0{
            // BER binary REAL: S, base (2/8/16), scale factor, exponent length.
            let sign=if b[0]&0x40!=0{-1.0_f64}else{1.0};
            let base=match b[0]&0x30{0=>2.0_f64,0x10=>8.0,0x20=>16.0,_=>return Err("PubChem BER: invalid binary REAL base".into())};
            let scale=(b[0]>>2)&3;let mut offset=1usize;
            let exp_len=if b[0]&3==3{let n=*b.get(offset).ok_or("PubChem BER: missing exponent length")? as usize;offset+=1;n}else{(b[0]&3) as usize+1};
            if exp_len==0||exp_len>8||offset+exp_len>=b.len(){return Err("PubChem BER: invalid REAL exponent".into());}
            let exp_bytes=&b[offset..offset+exp_len];let mut exponent=if exp_bytes[0]&0x80!=0{-1_i64}else{0};for &x in exp_bytes{exponent=(exponent<<8)|(x as i64);}offset+=exp_len;
            let mut mantissa=0_f64;for &x in &b[offset..]{mantissa=mantissa*256.0+x as f64;}
            let v=sign*mantissa*base.powf(exponent as f64)*2_f64.powi(scale as i32);
            return if v.is_finite()&&v.abs()<=f32::MAX as f64 {Ok(v as f32)}else{Err("PubChem BER: invalid REAL".into())};
        }
        Err("PubChem BER: unknown REAL encoding".into())
    }
}
fn ber_ints(field:Option<&Ber<'_>>)->Result<Vec<i64>,String>{let v=field.ok_or("PubChem BER: missing integer sequence")?;v.seq_items().iter().map(|x|x.int()).collect()}
fn ber_reals(field:Option<&Ber<'_>>)->Result<Vec<f32>,String>{let v=field.ok_or("PubChem BER: missing REAL sequence")?;v.seq_items().iter().map(|x|x.real().and_then(|f|if f.is_finite(){Ok(f)}else{Err("PubChem BER: non-finite coordinate".into())})).collect()}
fn compound_binary(root:&Ber<'_>,index:usize)->Result<Vec<Molecule>,String>{
    let atoms=root.field(1).ok_or("PubChem BER: no atoms")?;
    let ids=ber_ints(atoms.field(0))?;let elements=ber_ints(atoms.field(1))?;
    if ids.is_empty()||ids.len()!=elements.len(){return Err("PubChem BER: atom arrays differ".into());}
    let mut m=Molecule::new(&format!("PubChem structure {}",index+1));let mut id_map=HashMap::new();
    for (i,(&id,&z)) in ids.iter().zip(&elements).enumerate(){if id<=0||id_map.insert(id,i).is_some(){return Err("PubChem BER: duplicate atom ID".into());}m.atoms.push(Atom::new(element(z)?));}
    if let Some(charges)=atoms.field(4){for pair in charges.seq_items(){let id=pair.field(0).ok_or("PubChem BER: charge ID missing")?.int()?;let q=pair.field(1).ok_or("PubChem BER: charge value missing")?.int()?;if !(-8..=8).contains(&q){return Err("PubChem BER: charge out of range".into());}let i=*id_map.get(&id).ok_or("PubChem BER: invalid charge ID")?;m.atoms[i].charge=q as i8;}}
    if let Some(bonds)=root.field(2){
        let a=ber_ints(bonds.field(0))?;let b=ber_ints(bonds.field(1))?;let o=ber_ints(bonds.field(2))?;
        if a.len()!=b.len()||a.len()!=o.len(){return Err("PubChem BER: bond arrays differ".into());}
        for ((&i,&j),&order) in a.iter().zip(&b).zip(&o){if !(1..=3).contains(&order){return Err(format!("PubChem BER: unsupported bond type {order}"));}m.bonds.push(Bond{a:*id_map.get(&i).ok_or("PubChem BER: invalid bond aid1")?,b:*id_map.get(&j).ok_or("PubChem BER: invalid bond aid2")?,order:order as u8});}
    }
    m.verify()?;
    let mut result=Vec::new();
    if let Some(coords)=root.field(4){for group in coords.seq_items(){
        let atom_ids=ber_ints(group.field(1))?;
        let typ=if let Some(t)=group.field(0){ber_ints(Some(t))?}else{vec![]};
        let scale=if typ.contains(&11){10.0}else{1.0};
        if let Some(conformers)=group.field(2){for c in conformers.seq_items(){
            if result.len()>=MAX_CONFORMERS{return Err("PubChem BER: too many conformers".into());}
            let x=ber_reals(c.field(0))?;let y=ber_reals(c.field(1))?;let z=if c.field(2).is_some(){ber_reals(c.field(2))?}else{vec![0.0;x.len()]};
            if x.len()!=atom_ids.len()||x.len()!=y.len()||x.len()!=z.len(){return Err("PubChem BER: conformer arrays differ".into());}
            let mut next=m.clone();
            for (j,id) in atom_ids.iter().enumerate(){let pos=*id_map.get(id).ok_or("PubChem BER: invalid coordinate atom ID")?;next.atoms[pos].pos=Vec3::new(x[j]*scale,y[j]*scale,z[j]*scale);}
            // PC-DrawAnnotations with annotation 8 = aromatic bond.
            if let Some(style)=c.field(3){let ann=ber_ints(style.field(0))?;let a=ber_ints(style.field(1))?;let b=ber_ints(style.field(2))?;if ann.len()!=a.len()||ann.len()!=b.len(){return Err("PubChem BER: annotation lengths differ".into());}for ((kind,aid1),aid2) in ann.iter().zip(a).zip(b){if *kind==8{let i=*id_map.get(&aid1).ok_or("PubChem BER: invalid aromatic aid")?;let j=*id_map.get(&aid2).ok_or("PubChem BER: invalid aromatic aid")?;if let Some(bond)=next.bonds.iter_mut().find(|t|(t.a==i&&t.b==j)||(t.a==j&&t.b==i)){bond.order=4;}}}}
            result.push(next);
        }}
    }}
    if result.is_empty(){result.push(m);}Ok(result)
}
fn looks_like_compound(node:&Ber<'_>)->bool{
    let Some(atoms)=node.field(1) else{return false};
    match (ber_ints(atoms.field(0)),ber_ints(atoms.field(1))){
        (Ok(ids),Ok(elements))=>!ids.is_empty()&&ids.len()==elements.len()&&elements.iter().all(|x|(1..=118).contains(x)),
        _=>false,
    }
}
fn decode_binary_record(record:&Ber<'_>,out:&mut Vec<Molecule>)->Result<(),String>{
    if looks_like_compound(record){out.extend(compound_binary(record,out.len())?);}
    else if let Some(compounds)=record.field(6){
        // PC-Substance.compound contains SEQUENCE OF PC-Compound.
        for comp in compounds.seq_items(){if looks_like_compound(comp){out.extend(compound_binary(comp,out.len())?);}}
    }else{for sub in record.seq_items(){if looks_like_compound(sub)||sub.field(6).is_some(){decode_binary_record(sub,out)?;}}}
    if out.len()>MAX_CONFORMERS{return Err("PubChem BER: too many structures".into());}
    Ok(())
}
fn read_binary(data:&[u8])->Result<Vec<Molecule>,String>{
    let mut pos=0usize;let mut out=Vec::new();
    while pos<data.len(){
        let tlv=parse_ber(data,&mut pos,0)?;
        if tlv.tag!=0x30{return Err("PubChem BER: expected SEQUENCE root; binary record not recognized".into());}
        // BER has no self-describing type names. Recognize PC-Compound by
        // checking its atom-array schema, and PC-Substance via field 6.
        decode_binary_record(&tlv,&mut out)?;
    }
    if out.is_empty(){return Err("PubChem BER: no supported PC-Compound records".into());}
    Ok(out)
}

fn tlv(tag:u8,content:&[u8])->Vec<u8>{
    let mut bytes=vec![tag];if content.len()<128{bytes.push(content.len() as u8);}else{
        let len=content.len() as u64;let n=if len<=255{1}else if len<=65535{2}else if len<=16_777_215{3}else{4};bytes.push(0x80|n);for i in (0..n).rev(){bytes.push((len>>(i*8)) as u8);}
    }bytes.extend_from_slice(content);bytes
}
fn join_tlv(tag:u8,items:impl IntoIterator<Item=Vec<u8>>)->Vec<u8>{let content=items.into_iter().flatten().collect::<Vec<_>>();tlv(tag,&content)}
fn i64_ber(n:i64)->Vec<u8>{let bytes=n.to_be_bytes();let mut i=0;while i<7 && ((bytes[i]==0 && bytes[i+1]&0x80==0)||(bytes[i]==255 && bytes[i+1]&0x80!=0)){i+=1;}tlv(2,&bytes[i..])}
fn f32_ber(n:f32)->Vec<u8>{if n==0.0{return tlv(9,&[]);}let mut payload=vec![3];payload.extend_from_slice(format!("{n:.9E}").as_bytes());tlv(9,&payload)}
fn tagged(tag:u8,inner:Vec<u8>)->Vec<u8>{tlv(0xa0|tag,&inner)}
fn field_seq(tag:u8,fields:Vec<Vec<u8>>)->Vec<u8>{tagged(tag,join_tlv(0x30,fields))}
fn field_ints(tag:u8,nums:impl IntoIterator<Item=i64>)->Vec<u8>{tagged(tag,join_tlv(0x30,nums.into_iter().map(i64_ber)))}
fn field_reals(tag:u8,nums:impl IntoIterator<Item=f32>)->Vec<u8>{tagged(tag,join_tlv(0x30,nums.into_iter().map(f32_ber)))}
fn compound_ber(m:&Molecule)->Vec<u8>{
    let mut atom_fields=vec![field_ints(0,(1..=m.atoms.len()).map(|i|i as i64)),field_ints(1,m.atoms.iter().map(|a|atomic_number(&a.symbol).unwrap() as i64))];
    let charges=m.atoms.iter().enumerate().filter(|(_,a)|a.charge!=0).map(|(i,a)|join_tlv(0x30,vec![tagged(0,i64_ber(i as i64+1)),tagged(1,i64_ber(a.charge as i64))])).collect::<Vec<_>>();
    if !charges.is_empty(){atom_fields.push(field_seq(4,charges));}
    let mut fields=vec![field_seq(0,vec![tagged(0,i64_ber(0))]),field_seq(1,atom_fields)];
    if !m.bonds.is_empty(){fields.push(field_seq(2,vec![field_ints(0,m.bonds.iter().map(|b|b.a as i64+1)),field_ints(1,m.bonds.iter().map(|b|b.b as i64+1)),field_ints(2,m.bonds.iter().map(|b|if b.order==4{1_i64}else{b.order as i64}))]));}
    let mut conformer=vec![field_reals(0,m.atoms.iter().map(|a|a.pos.x)),field_reals(1,m.atoms.iter().map(|a|a.pos.y)),field_reals(2,m.atoms.iter().map(|a|a.pos.z))];
    let arom=m.bonds.iter().filter(|b|b.order==4).collect::<Vec<_>>();
    if !arom.is_empty(){conformer.push(field_seq(3,vec![field_ints(0,arom.iter().map(|_|8)),field_ints(1,arom.iter().map(|b|b.a as i64+1)),field_ints(2,arom.iter().map(|b|b.b as i64+1))]));}
    let set=join_tlv(0x30,vec![field_ints(0,[2,10]),field_ints(1,(1..=m.atoms.len()).map(|i|i as i64)),field_seq(2,vec![join_tlv(0x30,conformer)])]);
    fields.push(field_seq(4,vec![set]));
    join_tlv(0x30,fields)
}
/// Write PC-Compound(s) as BER ASN.1. The writer emits definite lengths and
/// explicit context field tags for the supported NCBI-PCSubstance fields.
pub fn write_pubchem_binary(structures:&[Molecule])->Result<Vec<u8>,String>{
    if structures.is_empty(){return Err("PubChem: nothing to export".into());}
    for m in structures{m.verify()?;}
    if structures.len()==1{Ok(compound_ber(&structures[0]))}else{Ok(join_tlv(0x30,structures.iter().map(compound_ber)))}
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test] fn text_pubchem_sample_with_atom_numbers(){
        let sample=r#"PC-Compound ::= {
          id { type standardized, id cid 962 },
          atoms { aid { 1, 2, 3 }, element { o, h, h } },
          bonds { aid1 { 1, 1 }, aid2 { 2, 3 }, order { single, single } },
          coords { { type { threed, units-angstroms }, aid { 1, 2, 3 },
             conformers { { x { 0, -0.75, 0.75 }, y { 0, 0.5, 0.5 }, z { 0, 0, 0 } } } } }
        }"#;
        let parsed=read_pubchem(sample.as_bytes()).unwrap();
        assert_eq!(parsed.len(),1);assert_eq!(parsed[0].formula(),"H2O");
        assert_eq!(parsed[0].bonds.len(),2);assert_eq!(parsed[0].atoms[1].pos.x,-0.75);
    }
    #[test] fn text_binary_multiconformer_roundtrips(){
        let mut m=crate::chemistry::propose("C2H6O").unwrap().remove(0);
        m.atoms[0].pos=Vec3::new(1.0,2.0,3.0);
        m.atoms[0].charge=1;
        for bin in [false,true]{
            let bytes=if bin{write_pubchem_binary(&[m.clone(),m.clone()]).unwrap()}else{write_pubchem_text(&[m.clone(),m.clone()]).unwrap().into_bytes()};
            let got=read_pubchem(&bytes).unwrap();assert_eq!(got.len(),2);
            assert_eq!(got[0].atoms.len(),m.atoms.len());assert_eq!(got[0].bonds.len(),m.bonds.len());
            assert_eq!(got[0].atoms[0].charge,1);assert!((got[0].atoms[0].pos.z-3.0).abs()<0.001);
        }
    }
    #[test] fn incorrect_unsupported_bond_is_not_aromatic(){
        let sample=r#"PC-Compound ::= { atoms { aid {1,2}, element {6,6} }, bonds {aid1 {1},aid2 {2},order { quadruple }} }"#;
        assert!(read_pubchem(sample.as_bytes()).is_err());
    }
    #[test] fn text_substance_and_aromatic_annotation(){
        let sample=r#"PC-Substance ::= { sid {id 1, version 1}, source {db {name "demo", source-id str "demo"}}, compound {
          { atoms { aid {1,2}, element {c,c} },
            bonds {aid1 {1},aid2 {2},order {single}},
            coords {{type {threed},aid {1,2},conformers {{x {0,1},y {0,0},z {0,0},
              style {annotation {aromatic},aid1 {1},aid2 {2}}}}}} }
        } }"#;
        let parsed=read_pubchem(sample.as_bytes()).unwrap();
        assert_eq!(parsed.len(),1);
        assert_eq!(parsed[0].bonds[0].order,4);
    }
    #[test] fn uploaded_egfr_protein_record_is_metadata_not_ber() {
        let data=include_bytes!("../../samples/PROTEIN_PROTACXN_P00533.asnt");
        let record=read_protein_record(data).unwrap().expect("Record root should be recognized");
        assert_eq!(record.record_type,"Protein");
        assert_eq!(record.accession,"P00533");
        assert_eq!(record.title,"Epidermal growth factor receptor (human)");
        assert_eq!(record.sequence.len(),1210);
        assert!(record.sequence.starts_with("MRPSGTAGAALLALLAALC"));
        assert!(record.structure_links.iter().any(|(_,link)|link.contains("rcsb.org")));
        let error=read_pubchem(data).unwrap_err();
        assert!(error.contains("annotation/sequence record"),"{error}");
    }
    #[test] fn truncated_binary_is_error(){assert!(read_pubchem(&[0x30,0x82,0x7f]).is_err());}
    #[test] fn multi_conformer(){
        let asnt=r#"PC-Compound ::= { atoms {aid {1}, element { o }}, coords {{type {threed},aid {1},conformers {{x {1},y {2},z {3}}, {x {4},y {5},z {6}}}}}}"#;
        let m=read_pubchem(asnt.as_bytes()).unwrap();assert_eq!(m.len(),2);assert_eq!(m[1].atoms[0].pos.x,4.0);
    }
}
