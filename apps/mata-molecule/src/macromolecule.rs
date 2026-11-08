//! PDB v3.x (fixed-column) and PDBx/mmCIF (atom_site loop) support.
//! No chemistry or CIF parsing dependencies beyond the Rust standard library.
//! Bond connectivity missing from a protein file is inferred from proximity.
use crate::chemistry::{atomic_number, Atom, AtomSite, Bond, Molecule};
use mata_3d::Vec3;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

fn field(line: &str, start: usize, end: usize) -> &str {
    line.get(start..end).unwrap_or("").trim()
}
fn cif_value(s: &str) -> &str {
    if s == "." || s == "?" { "" } else { s }
}
fn symbol_from(raw: &str, atom_name: &str) -> Result<String, String> {
    let clean = raw.trim();
    if !clean.is_empty() {
        let mut it = clean.chars();
        let first = it.next().unwrap().to_ascii_uppercase();
        let rest: String = it.collect::<String>().to_ascii_lowercase();
        let candidate = format!("{first}{rest}");
        if atomic_number(&candidate).is_some() { return Ok(candidate); }
    }
    // Old PDBs sometimes omit the element column; atom naming can be
    // ambiguous (e.g. "CA" means alpha-carbon, not calcium).
    let name = atom_name.trim_start_matches(|c: char| c.is_ascii_digit()).trim();
    let first = name.chars().next().ok_or("Atom is missing an element/name")?;
    let candidate = first.to_ascii_uppercase().to_string();
    if atomic_number(&candidate).is_some() { Ok(candidate) }
    else { Err(format!("Unknown element for atom '{atom_name}'")) }
}
fn charge(raw: &str) -> i8 {
    let s = raw.trim();
    if s.is_empty() || s == "." || s == "?" { return 0; }
    if let Ok(n) = s.parse::<i8>() { return n; }
    let mut ch = s.chars();
    match (ch.next(), ch.next(), ch.next()) {
        (Some(d), Some('+'), None) if d.is_ascii_digit() => d.to_digit(10).unwrap_or(0) as i8,
        (Some(d), Some('-'), None) if d.is_ascii_digit() => -(d.to_digit(10).unwrap_or(0) as i8),
        (Some('+'), Some(d), None) if d.is_ascii_digit() => d.to_digit(10).unwrap_or(0) as i8,
        (Some('-'), Some(d), None) if d.is_ascii_digit() => -(d.to_digit(10).unwrap_or(0) as i8),
        _ => 0,
    }
}
fn parse_f32(s: &str, what: &str) -> Result<f32, String> {
    let n = s.parse::<f32>().map_err(|_| format!("Invalid {what}: '{s}'"))?;
    if !n.is_finite() { return Err(format!("Non-finite {what}")); }
    Ok(n)
}
fn opt_f32(s: &str, default: f32) -> f32 {
    cif_value(s).parse::<f32>().ok().filter(|v| v.is_finite()).unwrap_or(default)
}
fn is_primary(alt: &str) -> bool {
    alt.is_empty() || alt == "." || alt == "?" || alt == "A" || alt == "1"
}

struct ModelBuild {
    molecule: Molecule,
    serial_to_atom: HashMap<String, usize>,
    raw_conect: Vec<(String, Vec<String>)>,
}
impl ModelBuild {
    fn new(name: &str) -> Self {
        Self { molecule: Molecule::new(name), serial_to_atom: HashMap::new(), raw_conect: Vec::new() }
    }
    fn add(&mut self, atom: Atom) -> Result<(), String> {
        if self.molecule.atoms.len() >= 100_000 { return Err("Structure exceeds 100,000 atoms".into()); }
        if let Some(s) = &atom.site {
            if !s.serial.is_empty() {
                if self.serial_to_atom.insert(s.serial.clone(), self.molecule.atoms.len()).is_some() {
                    return Err(format!("Duplicate atom ID {}", s.serial));
                }
            }
        }
        self.molecule.atoms.push(atom);
        Ok(())
    }
    fn connect_serials(&mut self) {
        let mut order = HashMap::<(usize, usize), u8>::new();
        for (src, targets) in &self.raw_conect {
            let Some(&a) = self.serial_to_atom.get(src) else { continue; };
            let mut repeated = HashMap::<usize, u8>::new();
            for target in targets {
                if let Some(&b) = self.serial_to_atom.get(target) {
                    if a != b { let c = repeated.entry(b).or_insert(0); *c = (*c + 1).min(3); }
                }
            }
            for (b, count) in repeated {
                let key = (a.min(b), a.max(b));
                let old = order.entry(key).or_insert(0);
                *old = (*old).max(count);
            }
        }
        self.molecule.bonds.extend(order.into_iter().map(|((a,b),order)| Bond {a,b,order}));
    }
    fn finish(mut self) -> Result<Molecule, String> {
        self.connect_serials();
        infer_missing_bonds(&mut self.molecule);
        self.molecule.from_file = true;
        self.molecule.verify()?;
        Ok(self.molecule)
    }
}

/// Reads ATOM/HETATM, MODEL/ENDMDL and CONECT. Alternate conformers B-Z are
/// skipped (A/blank selected); one Molecule is returned per coordinate model.
pub fn read_pdb(src: &str) -> Result<Vec<Molecule>, String> {
    let title = src.lines().find_map(|line| (field(line,0,6)=="HEADER").then(||field(line,10,50)))
        .filter(|s| !s.is_empty()).unwrap_or("PDB structure");
    let mut models = Vec::new();
    let mut current = ModelBuild::new(title);
    let mut global_conect = Vec::<(String, Vec<String>)>::new();
    let mut saw_model = false;
    let mut in_model = false;
    for (line_n, line) in src.lines().enumerate() {
        match field(line,0,6) {
            "MODEL" => {
                saw_model = true;
                if !current.molecule.atoms.is_empty() { models.push(current.finish()?); }
                current = ModelBuild::new(&format!("{title} / model {}",field(line,10,14)));
                in_model = true;
            }
            "ENDMDL" => {
                if !current.molecule.atoms.is_empty() { models.push(current.finish()?); }
                current = ModelBuild::new(title);
                in_model = false;
            }
            "ATOM" | "HETATM" => {
                if saw_model && !in_model { continue; }
                let alt = field(line,16,17);
                if !is_primary(alt) { continue; }
                let name = field(line,12,16);
                let element = symbol_from(field(line,76,78),name)?;
                let mut atom = Atom::new(&element);
                atom.pos = Vec3::new(parse_f32(field(line,30,38),"PDB X")?,
                    parse_f32(field(line,38,46),"PDB Y")?,
                    parse_f32(field(line,46,54),"PDB Z")?);
                atom.charge = charge(field(line,78,80));
                atom.site = Some(AtomSite {
                    atom_name: name.into(), residue_name: field(line,17,20).into(),
                    chain_id: field(line,21,22).into(), residue_number: field(line,22,26).into(),
                    insertion_code: field(line,26,27).into(), alternate_location: alt.into(),
                    occupancy: opt_f32(field(line,54,60),1.0), b_factor: opt_f32(field(line,60,66),0.0),
                    hetero: field(line,0,6)=="HETATM", serial: field(line,6,11).into(),
                });
                current.add(atom).map_err(|e| format!("PDB line {}: {e}",line_n+1))?;
            }
            "CONECT" => {
                let parts: Vec<&str> = (6..line.len()).step_by(5).map(|p|field(line,p,p+5)).filter(|s|!s.is_empty()).collect();
                if let Some((source, rest)) = parts.split_first() {
                    let edge = (source.to_string(), rest.iter().map(|s|(*s).into()).collect());
                    if in_model || !saw_model { current.raw_conect.push(edge); }
                    else { global_conect.push(edge); }
                }
            }
            _ => {}
        }
    }
    // Global CONECT following ENDMDL applies to every coordinate model.
    if !global_conect.is_empty() {
        for model in &mut models { // connect to original serials; inference already ran
            let map: HashMap<String,usize> = model.atoms.iter().enumerate().filter_map(|(i,a)|a.site.as_ref().map(|s|(s.serial.clone(),i))).collect();
            append_connections(model, &map, &global_conect);
        }
    }
    if !current.molecule.atoms.is_empty() { models.push(current.finish()?); }
    if models.is_empty() { return Err("PDB: no ATOM or HETATM records".into()); }
    for m in &models {m.verify()?;}
    Ok(models)
}
fn append_connections(m: &mut Molecule, ids: &HashMap<String,usize>, conns: &[(String,Vec<String>)]) {
    let mut seen:HashSet<_> = m.bonds.iter().map(|b|(b.a.min(b.b),b.a.max(b.b))).collect();
    for (src, targets) in conns {
        if let Some(&a)=ids.get(src) {for target in targets {
            if let Some(&b)=ids.get(target) {if a!=b && seen.insert((a.min(b),a.max(b))) {m.bond(a,b,1);}}
        }}
    }
}

fn site_or_default(atom: &Atom, i: usize) -> AtomSite {
    atom.site.clone().unwrap_or_else(|| AtomSite {
        atom_name: format!("{}{}",atom.symbol,i+1), residue_name: "MOL".into(),
        chain_id: "A".into(), residue_number: "1".into(), insertion_code: String::new(),
        alternate_location: String::new(), occupancy: 1.0, b_factor: 0.0,
        hetero: true, serial: (i+1).to_string(),
    })
}
fn ascii_field(s: &str, width: usize, label: &str) -> Result<(),String> {
    if !s.is_ascii() || s.len()>width || s.contains('\n') || s.contains('\r') {
        return Err(format!("PDB {label} '{s}' exceeds its {width}-character ASCII field; save as mmCIF"));
    }
    Ok(())
}
/// PDB loses aromatic bond order and mmCIF's arbitrarily long chain IDs;
/// reject unrepresentable identity fields rather than silently truncating.
pub fn write_pdb(molecules: &[Molecule]) -> Result<String,String> {
    let mut out = String::from("HEADER    GENERATED BY MATA MOLECULE STUDIO\n");
    for (model, mol) in molecules.iter().enumerate() {
        if mol.atoms.len()>99_999 { return Err("PDB serial limit: 99,999 atoms per model; use mmCIF".into()); }
        if molecules.len()>1 {out.push_str(&format!("MODEL     {:>4}\n",model+1));}
        for (i, atom) in mol.atoms.iter().enumerate() {
            let s = site_or_default(atom,i);
            ascii_field(&s.atom_name,4,"atom name")?;
            ascii_field(&s.residue_name,3,"residue name")?;
            ascii_field(&s.chain_id,1,"chain")?;
            ascii_field(&s.residue_number,4,"residue number")?;
            ascii_field(&s.insertion_code,1,"insertion code")?;
            ascii_field(&s.alternate_location,1,"alternate location")?;
            if s.occupancy.is_nan()||s.b_factor.is_nan() {return Err("Invalid PDB occupancy or B factor".into());}
            let charge = if atom.charge == 0 {"  ".into()}
                else if atom.charge.abs() <= 9 {format!("{}{}",atom.charge.abs(),if atom.charge>0 {'+'}else{'-'})}
                else {return Err("Formal charge cannot fit in PDB; save as mmCIF".into());};
            let seq = s.residue_number.parse::<i32>().map_err(|_|"Non-numeric PDB residue number; use mmCIF")?;
            if !(-999..=9999).contains(&seq) {return Err("PDB residue number out of range; use mmCIF".into());}
            let aname = if s.atom_name.len()<4 {format!(" {:<3}",s.atom_name)} else {s.atom_name.clone()};
            out.push_str(&format!("{:<6}{:>5} {aname:4}{:<1}{:>3} {:1}{:>4}{:<1}   {:>8.3}{:>8.3}{:>8.3}{:>6.2}{:>6.2}          {:>2}{charge}\n",
                if s.hetero {"HETATM"}else{"ATOM"},i+1,
                s.alternate_location,s.residue_name,s.chain_id,seq,s.insertion_code,
                atom.pos.x,atom.pos.y,atom.pos.z,s.occupancy,s.b_factor,atom.symbol));
        }
        let mut adj = vec![Vec::new();mol.atoms.len()];
        for b in &mol.bonds {
            // PDB CONECT is a connectivity format, not an aromatic/stereo representation.
            let repeat = if b.order<=3 {b.order} else {1};
            for _ in 0..repeat {adj[b.a].push(b.b+1);adj[b.b].push(b.a+1);}
        }
        for (i, bonds) in adj.iter().enumerate() {
            for chunk in bonds.chunks(4) {
                out.push_str(&format!("CONECT{:>5}",i+1));
                for &b in chunk {out.push_str(&format!("{b:>5}"));}
                out.push('\n');
            }
        }
        if molecules.len()>1 {out.push_str("ENDMDL\n");}
    }
    out.push_str("END\n");
    Ok(out)
}

fn covalent_radius(s: &str) -> Option<f32> {
    Some(match s {
        "H"=>0.31,"B"=>0.85,"C"=>0.76,"N"=>0.71,"O"=>0.66,"F"=>0.57,
        "Si"=>1.11,"P"=>1.07,"S"=>1.05,"Cl"=>1.02,"Se"=>1.20,
        "Br"=>1.20,"I"=>1.39,_=>return None,
    })
}
fn valence_limit(s: &str) -> usize {
    match s {"H"|"F"|"Cl"|"Br"|"I"=>1,"O"=>2,"N"=>4,"C"|"Si"=>4,"B"=>3,"S"|"P"|"Se"=>6,_=>0}
}
/// The fallback is approximate and does not reconstruct aromaticity or
/// formal bond orders. A spatial grid avoids an O(N^2) scan for proteins.
pub fn infer_missing_bonds(m: &mut Molecule) {
    if m.atoms.len()<2 {return;}
    let mut seen: HashSet<_> = m.bonds.iter().map(|b|(b.a.min(b.b),b.a.max(b.b))).collect();
    let mut used_valence = vec![0usize;m.atoms.len()];
    for b in &m.bonds {
        let order = if b.order == 4 {1} else {b.order as usize};
        used_valence[b.a] += order;
        used_valence[b.b] += order;
    }
    let mut grid:HashMap<(i32,i32,i32),Vec<usize>> = HashMap::new();
    const CELL:f32 = 2.6;
    for i in 0..m.atoms.len() {
        let a = &m.atoms[i];
        let Some(ra) = covalent_radius(&a.symbol) else {continue;};
        let cell = ((a.pos.x/CELL).floor() as i32,(a.pos.y/CELL).floor() as i32,(a.pos.z/CELL).floor() as i32);
        let mut candidates = Vec::new();
        for dx in -1..=1 {for dy in -1..=1 {for dz in -1..=1 {
            if let Some(bucket)=grid.get(&(cell.0+dx,cell.1+dy,cell.2+dz)) {
                for &j in bucket {
                    let b = &m.atoms[j];
                    if let Some(rb)=covalent_radius(&b.symbol) {
                        let v=a.pos-b.pos;
                        let dist=v.len();
                        let threshold=(ra+rb+0.35).min(2.50);
                        if dist>0.4 && dist<=threshold {candidates.push((dist,j));}
                    }
                }
            }
        }}}
        candidates.sort_by(|x,y|x.0.total_cmp(&y.0));
        for (_dist,j) in candidates {
            let key=(j,i);
            if seen.contains(&key) {continue;}
            if used_valence[i]>=valence_limit(&m.atoms[i].symbol) || used_valence[j]>=valence_limit(&m.atoms[j].symbol) {continue;}
            if m.bonds.len()>=400_000 {return;}
            seen.insert(key);used_valence[i]+=1;used_valence[j]+=1;
            m.bonds.push(Bond{a:j,b:i,order:1});
        }
        grid.entry(cell).or_default().push(i);
    }
}

// CIF tokenization: quotes, multiline semicolon blocks, # comments, and
// ordinary whitespace-delimited words. Newlines inside CIF values are retained.
fn cif_tokens(src: &str) -> Result<Vec<String>, String> {
    let mut output=Vec::new();
    let mut lines=src.lines().peekable();
    while let Some(line)=lines.next() {
        if line.starts_with(';') {
            let mut value=String::from(&line[1..]);
            let mut terminated=false;
            for inner in lines.by_ref() {
                if inner.starts_with(';') {terminated=true;break;}
                value.push('\n');value.push_str(inner);
            }
            if !terminated {return Err("mmCIF: unterminated semicolon text field".into());}
            output.push(value);
            continue;
        }
        let bytes=line.as_bytes();let mut i=0;
        while i<bytes.len() {
            if bytes[i].is_ascii_whitespace() {i+=1;continue;}
            if bytes[i]==b'#' {break;}
            if bytes[i]==b'\'' || bytes[i]==b'"' {
                let delim=bytes[i];i+=1;let start=i;
                while i<bytes.len() {
                    if bytes[i]==delim && (i+1==bytes.len()||bytes[i+1].is_ascii_whitespace()||bytes[i+1]==b'#') {break;}
                    i+=1;
                }
                if i>=bytes.len() {return Err("mmCIF: unterminated quoted string".into());}
                output.push(line[start..i].to_string());i+=1;
            } else {
                let start=i;
                while i<bytes.len()&&!bytes[i].is_ascii_whitespace() {if bytes[i]==b'#' {break;} i+=1;}
                if start==i {break;}
                output.push(line[start..i].to_string());
                if i<bytes.len()&&bytes[i]==b'#' {break;}
            }
        }
    }
    Ok(output)
}
fn tag_index(tags: &[String], col: &str) -> Option<usize> {
    tags.iter().position(|s|s.eq_ignore_ascii_case(col))
}
fn at<'a>(tags:&[String], row:&'a [String], col:&str) -> &'a str {
    tag_index(tags,col).and_then(|i|row.get(i)).map(String::as_str).unwrap_or(".")
}
fn first<'a>(tags:&[String], row:&'a [String], names:&[&str])-> &'a str {
    names.iter().map(|k|at(tags,row,k)).find(|s| !cif_value(s).is_empty()).unwrap_or(".")
}
fn quote_cif(s:&str)->String {
    if s.is_empty(){return ".".into();}
    if s.bytes().all(|x|x.is_ascii_alphanumeric() || b"-_.+".contains(&x))
        && !s.starts_with('_') && !s.starts_with('#') && !s.starts_with(';') {return s.into();}
    let text=s.replace('\r', " " ).replace('\n', " " );
    if !text.contains('\'') {format!("'{text}'")}
    else if !text.contains('"') {format!("\"{text}\"")}
    else {format!("\n;{text}\n;\n")} // multiline CIF text field handles both quote styles
}

type CifRow = (Arc<Vec<String>>,Vec<String>);
#[derive(Default)]
struct CifBlock {
    name:String,
    atom_rows: Vec<CifRow>,
    conn_rows: Vec<CifRow>,
    own_bond_rows: Vec<CifRow>,
}
fn finish_cif_block(block: CifBlock, out: &mut Vec<Molecule>) -> Result<(),String> {
    if block.atom_rows.is_empty(){return Ok(());}
    let mut builds: Vec<(String,ModelBuild)> = Vec::new();
    let mut model_index=HashMap::<String,usize>::new();
    for (tags,row) in &block.atom_rows {
        let model=at(tags,row,"_atom_site.pdbx_PDB_model_num");
        let model=if cif_value(model).is_empty(){"1"}else{model};
        let idx = if let Some(&idx)=model_index.get(model) {idx} else {
            let idx=builds.len();model_index.insert(model.into(),idx);
            builds.push((model.to_string(), ModelBuild::new(&format!("{} / model {model}",block.name))));idx
        };
        let alt=at(tags,row,"_atom_site.label_alt_id");
        if !is_primary(alt){continue;}
        let atom_name=first(tags,row,&["_atom_site.auth_atom_id","_atom_site.label_atom_id"]);
        let sym=symbol_from(at(tags,row,"_atom_site.type_symbol"),atom_name)?;
        let mut atom=Atom::new(&sym);
        atom.pos=Vec3::new(
            parse_f32(at(tags,row,"_atom_site.Cartn_x"),"mmCIF Cartn_x")?,
            parse_f32(at(tags,row,"_atom_site.Cartn_y"),"mmCIF Cartn_y")?,
            parse_f32(at(tags,row,"_atom_site.Cartn_z"),"mmCIF Cartn_z")?);
        atom.charge=charge(at(tags,row,"_atom_site.pdbx_formal_charge"));
        atom.site=Some(AtomSite{
            atom_name:cif_value(atom_name).into(),
            residue_name:cif_value(first(tags,row,&["_atom_site.auth_comp_id","_atom_site.label_comp_id"])).into(),
            chain_id:cif_value(first(tags,row,&["_atom_site.auth_asym_id","_atom_site.label_asym_id"])).into(),
            residue_number:cif_value(first(tags,row,&["_atom_site.auth_seq_id","_atom_site.label_seq_id"])).into(),
            insertion_code:cif_value(at(tags,row,"_atom_site.pdbx_PDB_ins_code")).into(),
            alternate_location:cif_value(alt).into(),
            occupancy:opt_f32(at(tags,row,"_atom_site.occupancy"),1.0),
            b_factor:opt_f32(at(tags,row,"_atom_site.B_iso_or_equiv"),0.0),
            hetero:at(tags,row,"_atom_site.group_PDB").eq_ignore_ascii_case("HETATM"),
            serial:cif_value(at(tags,row,"_atom_site.id")).into(),
        });
        builds[idx].1.add(atom)?;
    }
    for (_,build) in &mut builds {
        let ids=build.serial_to_atom.clone();
        let mut explicit=Vec::new();
        for (tags,row) in &block.own_bond_rows {
            let a=at(tags,row,"_mata_bond.atom_id_1");let b=at(tags,row,"_mata_bond.atom_id_2");
            if let (Some(&a),Some(&b))=(ids.get(a),ids.get(b)) {
                if a!=b {
                    let order=match at(tags,row,"_mata_bond.value_order").to_ascii_uppercase().as_str(){
                        "2"|"DOUB"=>2,"3"|"TRIP"=>3,"4"|"AROM"=>4,_=>1};
                    explicit.push(Bond{a,b,order});
                }
            }
        }
        for (tags,row) in &block.conn_rows {
            let kind=at(tags,row,"_struct_conn.conn_type_id");
            if !kind.starts_with("coval")&&!kind.starts_with("disulf") {continue;}
            let a=find_site_atom(&build.molecule,tags,row,1);
            let b=find_site_atom(&build.molecule,tags,row,2);
            if let (Some(a),Some(b))=(a,b) {if a!=b {explicit.push(Bond{a,b,order:1});}}
        }
        let mut seen=HashSet::new();
        for bond in explicit {
            if seen.insert((bond.a.min(bond.b),bond.a.max(bond.b))) {build.molecule.bonds.push(bond);}
        }
    }
    for (_,build) in builds {if !build.molecule.atoms.is_empty(){out.push(build.finish()?);}}
    Ok(())
}
fn find_site_atom(m:&Molecule,tags:&[String],row:&[String],partner:usize)->Option<usize> {
    let prefix=format!("_struct_conn.ptnr{partner}_");
    let atom=at(tags,row,&format!("{prefix}label_atom_id"));
    let residue=at(tags,row,&format!("{prefix}label_comp_id"));
    let chain=at(tags,row,&format!("{prefix}label_asym_id"));
    let seq=at(tags,row,&format!("{prefix}label_seq_id"));
    // Authentication identifiers often agree with the atom_site auth columns.
    let auth_atom=at(tags,row,&format!("_struct_conn.ptnr{partner}_auth_atom_id"));
    let auth_chain=at(tags,row,&format!("_struct_conn.ptnr{partner}_auth_asym_id"));
    let auth_seq=at(tags,row,&format!("_struct_conn.ptnr{partner}_auth_seq_id"));
    m.atoms.iter().position(|a|a.site.as_ref().is_some_and(|s| {
        (s.atom_name==cif_value(atom)||s.atom_name==cif_value(auth_atom))
        && (cif_value(residue).is_empty()||s.residue_name==residue)
        && (s.chain_id==cif_value(chain)||s.chain_id==cif_value(auth_chain))
        && (s.residue_number==cif_value(seq)||s.residue_number==cif_value(auth_seq))
    }))
}

/// Supports data blocks and loop_ categories with true CIF quoted/semicolon
/// values; reads all atom_site models, plus covalent struct_conn links.
pub fn read_mmcif(src: &str) -> Result<Vec<Molecule>,String> {
    let tokens=cif_tokens(src)?;
    let mut all=Vec::new();let mut current=CifBlock::default();let mut pos=0;
    while pos<tokens.len() {
        let token=&tokens[pos];
        if token.starts_with("data_") {
            finish_cif_block(std::mem::take(&mut current),&mut all)?;
            current.name=token[5..].to_string();pos+=1;
        } else if token.eq_ignore_ascii_case("loop_") {
            pos+=1;let mut tags=Vec::new();
            while pos<tokens.len()&&tokens[pos].starts_with('_') {tags.push(tokens[pos].clone());pos+=1;}
            if tags.is_empty(){return Err("mmCIF: loop_ without columns".into());}
            let head = tags[0].to_ascii_lowercase();
            let record=if head.starts_with("_atom_site."){1}
                else if head.starts_with("_struct_conn."){2}
                else if head.starts_with("_mata_bond."){3}else{0};
            let width=tags.len();
            let tags=Arc::new(tags);
            while pos<tokens.len() {
                let t=&tokens[pos];
                if t.eq_ignore_ascii_case("stop_"){pos+=1;break;}
                if t.eq_ignore_ascii_case("loop_")||t.starts_with("data_")||t.starts_with("save_")||t.starts_with('_'){break;}
                if pos+width>tokens.len(){return Err("mmCIF: incomplete loop row".into());}
                let row=tokens[pos..pos+width].to_vec();
                // Loop data can contain ?/. values; reject a malformed loop
                // instead of consuming the first column of the next category.
                match record {1=>current.atom_rows.push((tags.clone(),row)),
                    2=>current.conn_rows.push((tags.clone(),row)),
                    3=>current.own_bond_rows.push((tags.clone(),row)),_=>{}}
                pos+=width;
            }
        } else if token.starts_with('_') && pos+1<tokens.len() {pos+=2;}
        else {pos+=1;}
    }
    finish_cif_block(current,&mut all)?;
    if all.is_empty(){return Err("mmCIF: no _atom_site coordinate records found".into());}
    Ok(all)
}

/// Multiple molecules are represented as separate coordinate models. An
/// optional `_mata_bond` extension retains exact single/double/triple/aromatic
/// bond orders (generic mmCIF atom_site does not encode these).
pub fn write_mmcif(molecules:&[Molecule])->Result<String,String> {
    let mut out=String::from("data_mata_molecule_studio\n#\nloop_\n\
        _atom_site.group_PDB\n_atom_site.id\n_atom_site.type_symbol\n\
        _atom_site.label_atom_id\n_atom_site.label_alt_id\n\
        _atom_site.label_comp_id\n_atom_site.label_asym_id\n\
        _atom_site.label_entity_id\n_atom_site.label_seq_id\n\
        _atom_site.pdbx_PDB_ins_code\n_atom_site.Cartn_x\n\
        _atom_site.Cartn_y\n_atom_site.Cartn_z\n_atom_site.occupancy\n\
        _atom_site.B_iso_or_equiv\n_atom_site.pdbx_formal_charge\n\
        _atom_site.auth_seq_id\n_atom_site.auth_comp_id\n\
        _atom_site.auth_asym_id\n_atom_site.auth_atom_id\n\
        _atom_site.pdbx_PDB_model_num\n");
    let mut serial=1usize;
    let mut model_ids=Vec::new();
    for (model,m) in molecules.iter().enumerate() {
        let mut ids=Vec::new();
        for (i,a) in m.atoms.iter().enumerate() {
            let site=site_or_default(a,i);
            let id=serial.to_string();ids.push(id.clone());serial+=1;
            let atom_name=quote_cif(&site.atom_name);
            let comp=quote_cif(&site.residue_name);
            let chain=quote_cif(&site.chain_id);
            let seq=quote_cif(&site.residue_number);
            out.push_str(&format!("{} {} {} {} {} {} {} 1 {} {} {:.3} {:.3} {:.3} {:.2} {:.2} {} {} {} {} {} {}\n",
                if site.hetero{"HETATM"}else{"ATOM"},id,a.symbol,
                atom_name,quote_cif(&site.alternate_location),comp,chain,seq,
                quote_cif(&site.insertion_code),a.pos.x,a.pos.y,a.pos.z,site.occupancy,site.b_factor,
                if a.charge==0{"?".to_string()}else{a.charge.to_string()},seq,comp,chain,atom_name,model+1));
        }
        model_ids.push(ids);
    }
    out.push_str("#\n");
    if molecules.iter().any(|m|!m.bonds.is_empty()) {
        out.push_str("# Exact bond orders retained using a Mata extension.\nloop_\n\
            _mata_bond.atom_id_1\n_mata_bond.atom_id_2\n_mata_bond.value_order\n");
        for (m,ids) in molecules.iter().zip(&model_ids) {
            for b in &m.bonds {out.push_str(&format!("{} {} {}\n",ids[b.a],ids[b.b],b.order));}
        }
        out.push_str("#\n");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    const PDB:&str = "HEADER    SAMPLE PROTEIN\nATOM      1  N   GLY A   1      11.104  13.207   8.678  1.00 20.00           N  \nATOM      2  CA  GLY A   1      12.560  13.272   8.678  1.00 20.00           C  \nATOM      3  C   GLY A   1      13.100  14.700   8.678  1.00 20.00           C  \nCONECT    1    2\nCONECT    2    3\nEND\n";
    #[test] fn pdb_import_export(){
        let a=read_pdb(PDB).unwrap();assert_eq!(a.len(),1);
        assert_eq!(a[0].atoms.len(),3);assert_eq!(a[0].atoms[1].site.as_ref().unwrap().atom_name,"CA");
        assert_eq!(a[0].atoms[1].symbol,"C");assert_eq!(a[0].bonds.len(),2);
        let b=read_pdb(&write_pdb(&a).unwrap()).unwrap();
        assert_eq!(b[0].atoms.len(),3);assert_eq!(b[0].bonds.len(),2);
    }
    #[test] fn pdb_models(){
        let p="MODEL        1\nATOM      1  O   HOH A   1       1.000   0.000   0.000  1.00  0.00           O\nENDMDL\nMODEL        2\nATOM      1  O   HOH A   1       2.000   0.000   0.000  1.00  0.00           O\nENDMDL\nEND\n";
        let m=read_pdb(p).unwrap();assert_eq!(m.len(),2);
        assert_eq!(read_pdb(&write_pdb(&m).unwrap()).unwrap().len(),2);
    }
    #[test] fn mmcif_models_and_metadata() {
        let text="data_example\nloop_\n_atom_site.group_PDB\n_atom_site.id\n_atom_site.type_symbol\n_atom_site.label_atom_id\n_atom_site.label_comp_id\n_atom_site.label_asym_id\n_atom_site.label_seq_id\n_atom_site.Cartn_x\n_atom_site.Cartn_y\n_atom_site.Cartn_z\n_atom_site.pdbx_PDB_model_num\nATOM 1 C CA GLY A 1 0 0 0 1\nATOM 2 N N GLY A 1 1.40 0 0 1\nATOM 3 C CA GLY A 1 0 0 1 2\nATOM 4 N N GLY A 1 1.40 0 1 2\n#\n";
        let m=read_mmcif(text).unwrap();assert_eq!(m.len(),2);assert_eq!(m[0].atoms.len(),2);
        assert_eq!(m[0].bonds.len(),1);
        let round=read_mmcif(&write_mmcif(&m).unwrap()).unwrap();
        assert_eq!(round.len(),2);assert_eq!(round[0].bonds.len(),1);
        assert_eq!(round[1].atoms[0].site.as_ref().unwrap().residue_name,"GLY");
    }
    #[test] fn cif_quoted_values_and_multiline() {
        let t=cif_tokens("data_x\n_tag 'A B'\n_tag2 \"some value\" #comment\n_tag3\n;hello\nworld\n;\n").unwrap();
        assert!(t.contains(&"A B".into()));assert!(t.contains(&"some value".into()));
        assert!(t.iter().any(|s|s.contains("hello\nworld")));
    }
    #[test] fn mmcif_bond_order_roundtrip(){
        let mut m=Molecule::new("benzene fragment");m.atom("C");m.atom("C");m.bond(0,1,4);
        let parsed=read_mmcif(&write_mmcif(&[m]).unwrap()).unwrap();
        assert_eq!(parsed[0].bonds[0].order,4);
    }
}
