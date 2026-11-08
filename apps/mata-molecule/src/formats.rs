//! Import/export V2000 MOL, multi-record SDF, and TRIPOS MOL2.
//! Unsupported V3000 or special query/bond features produce errors rather
//! than silently changing the chemistry.
use crate::chemistry::{Molecule,Atom,Bond,atomic_number};
use mata_3d::Vec3;
use std::{fs,path::Path};

/// Structure files have atoms and coordinates; PubChem `Record` ASNT files
/// instead provide protein metadata and often an amino-acid sequence.
#[derive(Debug)]
pub enum OpenedDocument {
    Structures(Vec<Molecule>),
    ProteinRecord(crate::pubchem::ProteinRecord),
}

pub fn read_document<P:AsRef<Path>>(path:P)->Result<OpenedDocument,String>{
    let p=path.as_ref();
    let ext=p.extension().and_then(|e|e.to_str()).unwrap_or("").to_ascii_lowercase();
    if matches!(ext.as_str(),"asn"|"asnt"|"pc3d"|"pc3") {
        let data=fs::read(p).map_err(|e|format!("Open {}: {e}",p.display()))?;
        if let Some(record)=crate::pubchem::read_protein_record(&data)? {
            return Ok(OpenedDocument::ProteinRecord(record));
        }
    }
    Ok(OpenedDocument::Structures(read_structures(p)?))
}

pub fn read_structures<P:AsRef<Path>>(path:P)->Result<Vec<Molecule>,String>{
    let p=path.as_ref();
    let ext=p.extension().and_then(|e|e.to_str()).unwrap_or("").to_ascii_lowercase();
    let mut structures=if matches!(ext.as_str(),"asn"|"asnt"|"pc3d"|"pc3") {
        let bytes=fs::read(p).map_err(|e|format!("Open {}: {e}",p.display()))?;
        crate::pubchem::read_pubchem(&bytes)?
    } else {
        let source=fs::read_to_string(p).map_err(|e|format!("Open {}: {e}",p.display()))?;
        match ext.as_str(){
        "mol"=>vec![read_mol(&source)?],
        "sdf"=>{let mut mols=Vec::new();for block in source.split("$$$$") {if block.trim().is_empty(){continue;}mols.push(read_mol(block.trim_start_matches(|c|c=='\n'||c=='\r'))?);}mols},
        "mol2"=>read_mol2(&source)?,
        "pdb"|"ent"=>crate::macromolecule::read_pdb(&source)?,
        "cif"|"mmcif"=>crate::macromolecule::read_mmcif(&source)?,
        _=>return Err("Supported file extensions: .mol, .sdf, .mol2, .pdb, .ent, .cif, .mmcif, .asn, .asnt, .pc3d, .pc3".into()),
    }
    };
    if structures.is_empty(){return Err("File contained no structures".into());}
    for m in &mut structures {m.from_file=true;m.verify()?;}
    Ok(structures)
}

pub fn write_structures<P:AsRef<Path>>(path:P,structures:&[Molecule])->Result<(),String>{
    let p=path.as_ref();if structures.is_empty(){return Err("Nothing to save".into());}
    for mol in structures{mol.verify()?;}
    let ext=p.extension().and_then(|e|e.to_str()).unwrap_or("").to_ascii_lowercase();
    if matches!(ext.as_str(),"asn"|"asnt"|"pc3d"|"pc3") {
        let content=if ext=="asnt" {crate::pubchem::write_pubchem_text(structures)?.into_bytes()}
                    else{crate::pubchem::write_pubchem_binary(structures)?};
        return fs::write(p,content).map_err(|e|format!("Write {}: {e}",p.display()));
    }
    let result=match ext.as_str(){
        "mol"=>write_mol(&structures[0])?,
        "sdf"=>{let mut content=String::new();for m in structures{content.push_str(&write_mol(m)?);content.push_str("$$$$\n");}content},
        "mol2"=>{let mut content=String::new();for m in structures{content.push_str(&write_mol2(m)?);}content},
        "pdb"|"ent"=>crate::macromolecule::write_pdb(structures)?,
        "cif"|"mmcif"=>crate::macromolecule::write_mmcif(structures)?,
        _=>return Err("Use .mol, .sdf, .mol2, .pdb, .ent, .cif, .mmcif, .asn, .asnt, .pc3d, or .pc3 extension".into()),
    };
    fs::write(p,result).map_err(|e|format!("Write {}: {e}",p.display()))
}

fn coord(v:&str)->Result<f32,String>{v.parse::<f32>().map_err(|_|format!("Invalid coordinate '{v}'"))}
fn number(v:&str)->Result<usize,String>{v.parse::<usize>().map_err(|_|format!("Invalid index/count '{v}'"))}
fn read_mol(s:&str)->Result<Molecule,String>{
    let lines=s.lines().map(|s|s.trim_end_matches('\r')).collect::<Vec<_>>();
    if lines.len()<4{return Err("MOL: missing V2000 header".into());}
    if lines[3].contains("V3000"){return Err("MOL V3000 is not supported; please export V2000".into());}
    let counts=lines[3].split_whitespace().collect::<Vec<_>>();
    if counts.len()<2 {return Err("MOL: malformed counts line".into());}
    let na=number(counts[0])?;let nb=number(counts[1])?;
    if na>512||nb>4096{return Err("MOL: structure too large".into());}
    if lines.len()<4+na+nb{return Err("MOL: truncated atom or bond block".into());}
    let mut m=Molecule::new(lines[0].trim());
    for line in &lines[4..4+na] {
        let w=line.split_whitespace().collect::<Vec<_>>();if w.len()<4{return Err("MOL: incomplete atom line".into());}
        if atomic_number(w[3]).is_none(){return Err(format!("Unsupported MOL element '{}'",w[3]));}
        let mut a=Atom::new(w[3]);a.pos=Vec3::new(coord(w[0])?,coord(w[1])?,coord(w[2])?);
        if let Some(code)=w.get(5).and_then(|v|v.parse::<u8>().ok()){a.charge=match code{1=>3,2=>2,3=>1,5=>-1,6=>-2,7=>-3,_=>0};}
        m.atoms.push(a);
    }
    for line in &lines[4+na..4+na+nb]{let w=line.split_whitespace().collect::<Vec<_>>();if w.len()<3{return Err("MOL: incomplete bond line".into());}
        let a=number(w[0])?;let b=number(w[1])?;let order=w[2].parse::<u8>().map_err(|_|"MOL: invalid bond type")?;
        if a==0||b==0||a>na||b>na{return Err("MOL: bond atom index out of bounds".into());}if order>4||order==0{return Err("MOL: unsupported/query bond type".into());}
        m.bonds.push(Bond{a:a-1,b:b-1,order});
    }
    for line in &lines[4+na+nb..]{let w=line.split_whitespace().collect::<Vec<_>>();if w.len()>=4&&w[0]=="M"&&w[1]=="CHG" {
        let n=number(w[2])?;if w.len()<3+2*n {return Err("MOL: invalid M CHG record".into());}
        for p in 0..n {let i=number(w[3+2*p])?;if i==0||i>na{return Err("MOL: M CHG index out of range".into());}m.atoms[i-1].charge=w[4+2*p].parse::<i8>().map_err(|_|"MOL: invalid charge")?;}
    }}
    m.verify()?;Ok(m)
}
fn write_mol(m:&Molecule)->Result<String,String>{
    if m.atoms.len()>999||m.bonds.len()>999{return Err("MOL V2000 supports at most 999 atoms and 999 bonds".into());}
    let title=m.name.chars().take(80).collect::<String>();
    let mut text=format!("{title}\n  MataMolecule    3D\n\n{:>3}{:>3}  0  0  0  0            999 V2000\n",m.atoms.len(),m.bonds.len());
    for a in &m.atoms {text.push_str(&format!("{:>10.4}{:>10.4}{:>10.4} {:<3} 0  0  0  0  0  0  0  0  0  0  0  0\n",a.pos.x,a.pos.y,a.pos.z,a.symbol));}
    for b in &m.bonds{text.push_str(&format!("{:>3}{:>3}{:>3}  0  0  0  0\n",b.a+1,b.b+1,b.order));}
    // Formal charges as explicit M CHG records; atom charge columns remain 0.
    let charged=m.atoms.iter().enumerate().filter(|(_,a)|a.charge!=0).collect::<Vec<_>>();
    for group in charged.chunks(8){text.push_str(&format!("M  CHG{:>3}",group.len()));for (i,a) in group{text.push_str(&format!("{:>4}{:>4}",i+1,a.charge));}text.push('\n');}
    text.push_str("M  END\n");Ok(text)
}
fn read_mol2(src:&str)->Result<Vec<Molecule>,String>{
    let mut out=Vec::new();for (i,part) in src.split("@<TRIPOS>MOLECULE").enumerate(){if i==0 {continue;}
        let mut lines=part.lines();let name=lines.find(|line|!line.trim().is_empty()).unwrap_or("Untitled").trim();let mut m=Molecule::new(name);
        let mut section="";let mut atom_ids=std::collections::HashMap::<usize,usize>::new();
        for line in lines {let line=line.trim();if line.starts_with("@<TRIPOS>"){section=line;continue;}if line.is_empty()||line.starts_with('#'){continue;}
            let w=line.split_whitespace().collect::<Vec<_>>();match section {
                "@<TRIPOS>ATOM"=>{
                    if w.len()<6{return Err("MOL2: incomplete atom record".into());}
                    let raw_id=number(w[0])?;
                    let symbol=w[5].split('.').next().unwrap_or("");let symbol=match symbol {"Du"|"Any"=>return Err("MOL2: dummy/query atoms are unsupported".into()),_=>symbol};
                    let mut element=symbol.to_string();if atomic_number(&element).is_none(){let start=w[1].chars().take_while(|c|c.is_ascii_alphabetic()).collect::<String>();element=if atomic_number(&start).is_some(){start}else{start.chars().next().map(|c|c.to_string()).unwrap_or_default()};}
                    if atomic_number(&element).is_none(){return Err(format!("MOL2: unknown atom type {}",w[5]));}
                    let mut a=Atom::new(&element);a.pos=Vec3::new(coord(w[2])?,coord(w[3])?,coord(w[4])?);a.mol2_type=Some(w[5].to_string());
                    if let Some(raw)=w.get(8).and_then(|c|c.parse::<f32>().ok()) {
                        a.partial_charge=Some(raw);
                        let nearest=raw.round();if (raw-nearest).abs()<0.001 && nearest.abs()<=8.0 {a.charge=nearest as i8;}
                    }
                    if atom_ids.insert(raw_id,m.atoms.len()).is_some(){return Err("MOL2: duplicate atom ID".into());}m.atoms.push(a);
                },
                "@<TRIPOS>BOND"=>{
                    if w.len()<4{return Err("MOL2: incomplete bond record".into());}
                    let a=*atom_ids.get(&number(w[1])?).ok_or("MOL2: bond refers to missing atom")?;
                    let b=*atom_ids.get(&number(w[2])?).ok_or("MOL2: bond refers to missing atom")?;
                    let order=match w[3].to_ascii_lowercase().as_str(){"1"|"am"=>1,"2"=>2,"3"=>3,"ar"=>4,_=>return Err(format!("MOL2: unsupported bond type {}",w[3]))};m.bonds.push(Bond{a,b,order});
                },_=>{}
            }
        }
        if !m.atoms.is_empty(){m.verify()?;out.push(m);}
    }Ok(out)
}
fn infer_mol2_type(m:&Molecule,i:usize)->String {
    let a=&m.atoms[i];let aromatic=m.bonds.iter().any(|b|b.order==4&&(b.a==i||b.b==i));
    if aromatic{return format!("{}.ar",a.symbol);}
    let triple=m.bonds.iter().any(|b|b.order==3&&(b.a==i||b.b==i));
    let double=m.bonds.iter().any(|b|b.order==2&&(b.a==i||b.b==i));
    match a.symbol.as_str(){
        "C"=>format!("C.{}",if triple{"1"}else if double{"2"}else{"3"}),
        "N"=>format!("N.{}",if m.degree(i)==4{"4"}else if triple{"1"}else if double{"2"}else{"3"}),
        "O"=>if double{"O.2".into()}else{"O.3".into()},
        "S"=>if m.bond_order_sum(i)>=4{"S.o2".into()}else{"S.3".into()},
        "P"=>if m.degree(i)>=5{"P.5".into()}else{"P.3".into()},
        s=>s.to_string(),
    }
}
fn write_mol2(m:&Molecule)->Result<String,String>{
    let mut text=format!("@<TRIPOS>MOLECULE\n{}\n{} {} 0 0 0\nSMALL\nUSER_CHARGES\n\n@<TRIPOS>ATOM\n",m.name.replace('\n'," "),m.atoms.len(),m.bonds.len());
    for (i,a) in m.atoms.iter().enumerate(){let typ=a.mol2_type.clone().unwrap_or_else(||infer_mol2_type(m,i));
        text.push_str(&format!("{:>7} {:<7} {:>10.4} {:>10.4} {:>10.4} {:<7} 1 MOL {:>7.4}\n",i+1,format!("{}{}",a.symbol,i+1),a.pos.x,a.pos.y,a.pos.z,typ,a.partial_charge.unwrap_or(a.charge as f32)));
    }
    text.push_str("@<TRIPOS>BOND\n");for (i,b) in m.bonds.iter().enumerate(){text.push_str(&format!("{:>6} {:>5} {:>5} {}\n",i+1,b.a+1,b.b+1,if b.order==4{"ar".to_string()}else{b.order.to_string()}));}
    text.push('\n');Ok(text)
}

#[cfg(test)] mod tests {
 use super::*;use crate::chemistry::propose;
 #[test] fn formats_roundtrip(){for (ext,f) in [("mol","H2O"),("sdf","C2H6O"),("mol2","C6H6")]{let input=propose(f).unwrap();let path=std::env::temp_dir().join(format!("mata-molecule-test-{}.{}",std::process::id(),ext));write_structures(&path,&input).unwrap();let got=read_structures(&path).unwrap();std::fs::remove_file(&path).unwrap();assert_eq!(got.len(),if ext=="mol"{1}else{input.len()});assert_eq!(got[0].atoms.len(),input[0].atoms.len());assert_eq!(got[0].bonds.len(),input[0].bonds.len());}}
 #[test] fn charges_roundtrip(){let m=propose("NO3^-").unwrap();let src=write_mol(&m[0]).unwrap();let out=read_mol(&src).unwrap();assert_eq!(out.total_charge(),-1);}
}

#[cfg(test)] mod macromolecule_file_tests {
    use super::*;
    #[test] fn dispatch_pdb_and_mmcif() {
        let samples=include_str!("../../samples/glycine.pdb");
        let cif=include_str!("../../samples/glycine.cif");
        assert_eq!(crate::macromolecule::read_pdb(samples).unwrap()[0].atoms.len(),5);
        assert_eq!(crate::macromolecule::read_mmcif(cif).unwrap()[0].atoms.len(),5);
        for (ext,src) in [("pdb",samples),("mmcif",cif)] {
            let p=std::env::temp_dir().join(format!("mata-struct-{}-{}.{}",std::process::id(),ext,ext));
            std::fs::write(&p,src).unwrap();
            let imported=read_structures(&p).unwrap();
            write_structures(&p,&imported).unwrap();
            let again=read_structures(&p).unwrap();
            std::fs::remove_file(&p).unwrap();
            assert_eq!(again[0].atoms.len(),5);
            assert_eq!(again[0].atoms[0].site.as_ref().unwrap().residue_name,"GLY");
        }
    }
}

#[cfg(test)] mod pubchem_dispatch_tests {
    use super::*;
    #[test] fn protein_record_dispatch_keeps_sequence_separate_from_atoms() {
        let filename=std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../samples/PROTEIN_PROTACXN_P00533.asnt");
        match read_document(&filename).unwrap() {
            OpenedDocument::ProteinRecord(record)=>{
                assert_eq!(record.accession,"P00533");
                assert_eq!(record.sequence.len(),1210);
            },
            OpenedDocument::Structures(_)=>panic!("Protein annotation was mistaken for coordinates"),
        }
        let err=read_structures(filename).unwrap_err();
        assert!(err.contains("annotation/sequence record"),"{err}");
    }
    #[test] fn opens_pubchem_asnt_and_pc3_in_the_same_3d_viewer() {
        let bytes=include_bytes!("../../samples/water.asnt");
        let asnt_path=std::env::temp_dir().join(format!("mata-3d-test-{}.asnt",std::process::id()));
        let pc3_path=std::env::temp_dir().join(format!("mata-3d-test-{}.pc3",std::process::id()));
        for path in [&asnt_path,&pc3_path] {
            std::fs::write(path,bytes).unwrap();
            let models=match read_document(path).unwrap(){
                OpenedDocument::Structures(models)=>models,
                _=>panic!("PubChem compound misread as protein record"),
            };
            assert_eq!(models.len(),1);
            assert_eq!(models[0].atoms.len(),3);
            assert_eq!(models[0].bonds.len(),2);
            assert!(models[0].atoms[1].pos.x.abs()>0.7);
            std::fs::remove_file(path).unwrap();
        }
    }
    #[test] fn pc3_rejects_unrelated_binary_data() {
        let path=std::env::temp_dir().join(format!("mata-invalid-{}.pc3",std::process::id()));
        std::fs::write(&path,b"not a PubChem structure").unwrap();
        let err=read_structures(&path).unwrap_err();
        std::fs::remove_file(path).unwrap();
        assert!(!err.is_empty());
    }
    #[test] fn read_write_all_pubchem_extensions() {
        let input=crate::chemistry::propose("H2O").unwrap();
        for ext in ["asn","asnt","pc3d","pc3"] {
            let path=std::env::temp_dir().join(format!("mata-pubchem-{}-{}.{}",std::process::id(),ext,ext));
            write_structures(&path,&input).unwrap();
            let loaded=read_structures(&path).unwrap();
            std::fs::remove_file(&path).unwrap();
            assert_eq!(loaded.len(),input.len(),"{ext}");
            assert_eq!(loaded[0].atoms.len(),input[0].atoms.len(),"{ext}");
            assert_eq!(loaded[0].bonds.len(),input[0].bonds.len(),"{ext}");
        }
    }
}
