//! Optional, non-destructive molecular-structure preview reconstruction.
//!
//! This is a geometry-based *visual aid*, not a protonation/rotamer solver.
//! It never changes the imported `Molecule` and does not model missing residues.
use crate::{Atom, Bond, Molecule, Vec3};
use crate::protein_inspect::{self, Residue};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReconstructionSettings {
    pub hydrogens: bool,
    pub bonds: bool,
    pub side_chains: bool,
    pub backbone: bool,
    pub highlight: bool,
}
impl Default for ReconstructionSettings {
    fn default() -> Self {
        Self { hydrogens: true, bonds: true, side_chains: true,
            backbone: false, highlight: true }
    }
}
impl ReconstructionSettings {
    fn user_file() -> Option<std::path::PathBuf> {
        #[cfg(windows)]
        let root=std::env::var_os("APPDATA").map(std::path::PathBuf::from);
        #[cfg(not(windows))]
        let root=std::env::var_os("XDG_CONFIG_HOME").map(std::path::PathBuf::from)
            .or_else(||std::env::var_os("HOME").map(|home|std::path::PathBuf::from(home).join(".config")));
        root.map(|path|path.join("mata-molecule").join("settings.conf"))
    }
    pub fn load() -> Self {
        let mut settings=Self::default();
        let Some(file)=Self::user_file() else{return settings};
        let Ok(content)=std::fs::read_to_string(file) else{return settings};
        for line in content.lines() {
            let Some((key,value))=line.split_once('=') else {continue};
            let flag=match value.trim(){"true"=>true,"false"=>false,_=>continue};
            match key.trim() {
                "hydrogens"=>settings.hydrogens=flag,
                "bonds"=>settings.bonds=flag,
                "side_chains"=>settings.side_chains=flag,
                "backbone"=>settings.backbone=flag,
                "highlight"=>settings.highlight=flag,
                _=>{},
            }
        }
        settings
    }
    pub fn save(self) {
        let Some(file)=Self::user_file() else{return};
        if let Some(parent)=file.parent() {if std::fs::create_dir_all(parent).is_err(){return;}}
        let text=format!("hydrogens={}\nbonds={}\nside_chains={}\nbackbone={}\nhighlight={}\n",
            self.hydrogens,self.bonds,self.side_chains,self.backbone,self.highlight);
        let _=std::fs::write(file,text);
    }
}
#[derive(Default, Clone, Copy, Debug)]
pub struct ReconstructionStats {
    pub hydrogen_atoms: usize,
    pub side_chain_atoms: usize,
    pub backbone_atoms: usize,
    pub bonds_added: usize,
    pub orders_assigned: usize,
}
#[derive(Clone, Debug)]
pub struct Reconstructed {
    pub molecule: Molecule,
    pub stats: ReconstructionStats,
}

fn unit(v: Vec3, fallback: Vec3) -> Vec3 {
    if v.len() > 1e-5 {v.normalized()} else {fallback}
}
fn direction(neighbors: &[Vec3], base: Vec3, salt: u32) -> Vec3 {
    let outward = neighbors.iter().fold(Vec3::default(), |p, &n| p + unit(base - n, Vec3::default()));
    let axis = unit(outward, Vec3::new(1.0, 0.0, 0.0));
    let reference = if axis.z.abs()<0.8 {Vec3::new(0.0,0.0,1.0)} else {Vec3::new(0.0,1.0,0.0)};
    let ortho=unit(axis.cross(reference),Vec3::new(0.0,1.0,0.0));
    let side=unit(axis.cross(ortho),Vec3::new(0.0,0.0,1.0));
    let phi=(salt as f32 * 2.399963).sin();
    unit(axis*0.70 + ortho*(0.53*phi) + side*(0.53*(salt as f32*2.399963).cos()),axis)
}
fn atom_symbol(name:&str)->&'static str {
    if name.starts_with("SE") {"Se"}
    else if name.starts_with('N') {"N"}
    else if name.starts_with('O') {"O"}
    else if name.starts_with('S') {"S"}
    else {"C"}
}
fn canonical(r:&Residue)->bool {r.is_polymer && r.one_letter!='X'}
fn add_atom(out:&mut Molecule, parent:usize, label:&str, pos:Vec3, symbol:&str) -> usize {
    let mut a=Atom::new(symbol);a.pos=pos;
    a.site=out.atoms[parent].site.clone().map(|mut s| {
        s.atom_name=label.to_string();s.serial=String::new();s.alternate_location=String::new();
        s.occupancy=0.0;s.b_factor=0.0;s
    });
    let idx=out.atoms.len();out.atoms.push(a);out.estimated_atoms.push(idx);idx
}
fn ensure_bond(m:&mut Molecule, edges:&mut HashMap<(usize,usize),usize>, a:usize,b:usize,order:u8,stats:&mut ReconstructionStats) {
    let key=(a.min(b),a.max(b));
    if let Some(&idx)=edges.get(&key) {
        let existing=&mut m.bonds[idx];
        // Preserve explicitly specified multiple bonds. Reassign a generic
        // single bond using a known canonical amino-acid template.
        if existing.order==1 && order>1 {existing.order=order;stats.orders_assigned+=1;}
    }else {
        edges.insert(key,m.bonds.len());
        m.bonds.push(Bond{a,b,order});stats.bonds_added+=1;
    }
}
fn named_atoms(m:&Molecule,r:&Residue)->HashMap<String,usize> {
    r.atoms.iter().filter_map(|&i|{
        let site=m.atoms.get(i)?.site.as_ref()?;
        Some((site.atom_name.trim().to_ascii_uppercase(),i))
    }).collect()
}
/// Build a preview once per setting or variant change. Original atom indices
/// are preserved; appended estimates are tracked in `estimated_atoms`.
pub fn reconstruct(source:&Molecule, settings:ReconstructionSettings)->Reconstructed {
    let mut out=source.clone();out.estimated_atoms.clear();out.apply_templates=settings.bonds;
    let mut stats=ReconstructionStats::default();
    let mut edge_indices=HashMap::with_capacity(out.bonds.len());
    for (idx,b) in out.bonds.iter().enumerate() {edge_indices.insert((b.a.min(b.b),b.a.max(b.b)),idx);}
    let mut connectivity=vec![Vec::<usize>::new();out.atoms.len()];
    for b in &out.bonds {
        if b.a<connectivity.len() && b.b<connectivity.len(){
            connectivity[b.a].push(b.b);connectivity[b.b].push(b.a);
        }
    }
    let residues=protein_inspect::residues(source);
    if residues.is_empty() {return Reconstructed{molecule:out,stats};}
    // Limit automatic reconstruction to known polymer residues, not ligands,
    // ions, solvent, or unrecognized modifications.
    let mut maps=Vec::with_capacity(residues.len());
    for r in &residues {
        let mut names=named_atoms(&out,r);
        if !canonical(r){maps.push(names);continue;}
        if settings.backbone {
            // Only infer a missing carbonyl oxygen when N, CA and C anchor a
            // local peptide plane. A gap in the polypeptide is never filled.
            if !names.contains_key("O") {
                if let (Some(&n),Some(&ca),Some(&c))=(names.get("N"),names.get("CA"),names.get("C")) {
                    let cpos=out.atoms[c].pos;
                    let orient=unit(unit(cpos-out.atoms[ca].pos,Vec3::new(1.0,0.0,0.0))
                        +unit(cpos-out.atoms[n].pos,Vec3::new(0.0,1.0,0.0)),Vec3::new(1.0,0.0,0.0));
                    let idx=add_atom(&mut out,c,"O",cpos+orient*1.23,"O");
                    names.insert("O".into(),idx);stats.backbone_atoms+=1;
                    connectivity.push(vec![c]);connectivity[c].push(idx);
                    ensure_bond(&mut out,&mut edge_indices,c,idx,if settings.bonds{2}else{1},&mut stats);
                }
            }
        }
        if settings.side_chains && names.contains_key("N") && names.contains_key("CA") && names.contains_key("C") {
            // Grow only template-defined heavy atoms attached to known local
            // anchors. Positions are *schematic* and not rotamer optimized.
            let expected=protein_inspect::template(&r.key.name);
            let mut used=HashSet::new();
            for _ in 0..expected.len() {
                let mut changed=false;
                for &(a,b,order) in &expected {
                    if used.contains(b) || names.contains_key(b) || matches!(b,"N"|"CA"|"C"|"O"|"OXT") {continue;}
                    let Some(&parent)=names.get(a) else{continue;};
                    let origin=out.atoms[parent].pos;
                    let neighbor_positions=connectivity[parent].iter().filter_map(|&neighbor|{
                        out.atoms.get(neighbor).map(|atom|atom.pos)
                    }).collect::<Vec<_>>();
                    let seed=b.bytes().fold(r.atoms[0] as u32,|v,c|v.wrapping_mul(37).wrapping_add(c as u32));
                    let vector=direction(&neighbor_positions,origin,seed);
                    let bond_length=if atom_symbol(b)=="O" {1.35} else if atom_symbol(b)=="N"{1.45}else if atom_symbol(b)=="S"{1.81}else{1.52};
                    let idx=add_atom(&mut out,parent,b,origin+vector*bond_length,atom_symbol(b));
                    connectivity.push(vec![parent]);connectivity[parent].push(idx);
                    ensure_bond(&mut out,&mut edge_indices,parent,idx,if settings.bonds{order}else{1},&mut stats);
                    names.insert(b.into(),idx);used.insert(b);stats.side_chain_atoms+=1;changed=true;
                }
                if !changed {break;}
            }
        }
        if settings.bonds || settings.side_chains || settings.backbone {
            for (an,bn,order) in protein_inspect::template(&r.key.name) {
                if let (Some(&a),Some(&b))=(names.get(an),names.get(bn)) {
                    // Do not connect atoms across a large geometric gap if
                    // both atoms were observed in the source structure.
                    if !out.estimated_atoms.contains(&a) && !out.estimated_atoms.contains(&b)
                        && (out.atoms[a].pos-out.atoms[b].pos).len()>2.25 {continue;}
                    if settings.bonds || out.estimated_atoms.contains(&a) || out.estimated_atoms.contains(&b) {
                        ensure_bond(&mut out,&mut edge_indices,a,b,order,&mut stats);
                    }
                }
            }
        }
        maps.push(names);
    }
    if settings.bonds {
        // Peptide bonds across adjacent, complete residues in the same chain.
        for i in 1..residues.len() {
            let (prev,next)=(&residues[i-1],&residues[i]);
            if !canonical(prev)||!canonical(next)||prev.key.chain!=next.key.chain {continue;}
            if let (Some(&c),Some(&n))=(maps[i-1].get("C"),maps[i].get("N")) {
                let distance=(out.atoms[c].pos-out.atoms[n].pos).len();
                if (0.9..1.9).contains(&distance){ensure_bond(&mut out,&mut edge_indices,c,n,1,&mut stats);}
            }
        }
    }
    if settings.hydrogens {
        // Conservative, neutral valence-only estimate. This is NOT a pH-aware
        // protonation model, particularly for termini, histidine or acidic groups.
        // Do not derive hydrogens from unknown ligands or unrecognized residues.
        let groups=protein_inspect::residues(&out);
        let mut adjacency=vec![Vec::<(usize,u8)>::new();out.atoms.len()];
        for b in &out.bonds {
            if b.a<adjacency.len() && b.b<adjacency.len(){
                adjacency[b.a].push((b.b,b.order));
                adjacency[b.b].push((b.a,b.order));
            }
        }
        for r in &groups {
            if !canonical(r){continue;}
            for &i in &r.atoms {
                if i>=out.atoms.len(){continue;}
                let symbol=out.atoms[i].symbol.clone();
                let Some(site)=out.atoms[i].site.as_ref() else{continue;};
                let name=site.atom_name.trim().to_string();
                let residue=site.residue_name.to_string();
                if matches!(name.as_str(),"O"|"OXT"|"C")
                    || ((residue=="ASP"||residue=="ASN") && name=="CG")
                    || ((residue=="GLU"||residue=="GLN") && name=="CD")
                    || (residue=="ARG" && name=="CZ")
                    || ((residue=="ASP"||residue=="GLU") && matches!(name.as_str(),"OD1"|"OD2"|"OE1"|"OE2")) {continue;}
                let target=match symbol.as_str() {"C"=>4.0,"N"=>3.0,"O"|"S"=>2.0,_=>continue};
                let mut used=0.0f32;let mut neighbors=Vec::new();
                for &(other,order) in &adjacency[i] {
                    used+=if order==4 {1.5}else{order as f32};
                    neighbors.push(out.atoms[other].pos);
                }
                // Never assign hydrogens to disconnected or otherwise
                // uncharacterized atoms.
                if neighbors.is_empty(){continue;}
                let count=(target-used).round().clamp(0.0,3.0) as usize;
                for j in 0..count {
                    let p=out.atoms[i].pos;
                    let salt=(i as u32).wrapping_mul(17).wrapping_add(j as u32*11);
                    let pos=p+direction(&neighbors,p,salt)*if symbol=="O"{0.96}else if symbol=="S"{1.33}else{1.01};
                    let label=format!("H{}{}",name,j+1);
                    let new=add_atom(&mut out,i,&label,pos,"H");
                    out.bonds.push(Bond{a:i,b:new,order:1});
                    stats.hydrogen_atoms+=1;stats.bonds_added+=1;
                }
            }
        }
    }
    if !settings.highlight {out.estimated_atoms.clear();}
    Reconstructed{molecule:out,stats}
}

#[cfg(test)] mod tests {
    use super::*;
    use crate::chemistry::AtomSite;
    fn residue_atom(m:&mut Molecule,name:&str,symbol:&str) {
        let mut a=Atom::new(symbol);a.pos=Vec3::new(m.atoms.len() as f32*1.3,0.0,0.0);
        a.site=Some(AtomSite{atom_name:name.into(),residue_name:"ALA".into(),chain_id:"A".into(),residue_number:"1".into(),insertion_code:String::new(),alternate_location:String::new(),occupancy:1.0,b_factor:0.0,hetero:false,serial:(m.atoms.len()+1).to_string()});m.atoms.push(a);
    }
    #[test] fn all_off_retains_imported_structure() {
        let mut m=Molecule::new("alanine");for (n,s) in [("N","N"),("CA","C"),("C","C"),("O","O")] {residue_atom(&mut m,n,s);}m.bond(0,1,1);m.bond(1,2,1);m.bond(2,3,2);
        let opts=ReconstructionSettings{hydrogens:false,bonds:false,side_chains:false,backbone:false,highlight:false};
        let reconstructed=reconstruct(&m,opts);
        assert_eq!(reconstructed.molecule.atoms,m.atoms);
        assert_eq!(reconstructed.molecule.bonds,m.bonds);
        assert!(reconstructed.molecule.estimated_atoms.is_empty());
    }
    #[test] fn reconstructed_sidechains_are_tagged_and_source_unchanged() {
        let mut m=Molecule::new("alanine");for (n,s) in [("N","N"),("CA","C"),("C","C"),("O","O")] {residue_atom(&mut m,n,s);}m.bond(0,1,1);m.bond(1,2,1);m.bond(2,3,2);
        let result=reconstruct(&m,ReconstructionSettings{hydrogens:false,bonds:true,side_chains:true,backbone:false,highlight:true});
        assert_eq!(m.atoms.len(),4);
        assert_eq!(result.molecule.atoms.len(),5);
        assert_eq!(result.molecule.atoms[4].site.as_ref().unwrap().atom_name,"CB");
        assert!(result.molecule.estimated_atoms.contains(&4));
        assert!(result.molecule.bonds.iter().any(|b|b.a==1&&b.b==4||b.a==4&&b.b==1));
    }
    #[test] fn estimated_oxygen_requires_backbone_toggle() {
        let mut m=Molecule::new("alanine");for (n,s) in [("N","N"),("CA","C"),("C","C")] {residue_atom(&mut m,n,s);}m.bond(0,1,1);m.bond(1,2,1);
        let off=reconstruct(&m,ReconstructionSettings{hydrogens:false,bonds:true,side_chains:false,backbone:false,highlight:true});
        let on=reconstruct(&m,ReconstructionSettings{hydrogens:false,bonds:true,side_chains:false,backbone:true,highlight:true});
        assert_eq!(off.molecule.atoms.len(),3);assert_eq!(on.molecule.atoms.len(),4);
        assert_eq!(on.stats.backbone_atoms,1);
    }
}

#[cfg(test)] mod toggle_tests {
    use super::*;
    use crate::chemistry::AtomSite;
    fn alanine() -> Molecule {
        let mut m=Molecule::new("alanine");
        for (name,element,pos) in [
            ("N","N",Vec3::new(-1.2,0.0,0.0)),
            ("CA","C",Vec3::new(0.0,0.0,0.0)),
            ("C","C",Vec3::new(1.45,0.0,0.0)),
            ("O","O",Vec3::new(2.0,1.05,0.0)),
            ("CB","C",Vec3::new(0.0,-1.45,0.0)),
        ] {
            let mut a=Atom::new(element);a.pos=pos;
            a.site=Some(AtomSite{atom_name:name.into(),residue_name:"ALA".into(),chain_id:"A".into(),residue_number:"1".into(),insertion_code:String::new(),alternate_location:String::new(),occupancy:1.0,b_factor:0.0,hetero:false,serial:String::new()});
            m.atoms.push(a);
        }
        m.bond(0,1,1);m.bond(1,2,1);m.bond(2,3,1);m.bond(1,4,1);
        m
    }
    #[test] fn bond_toggle_assigns_carbonyl_only_when_enabled() {
        let input=alanine();
        let mut flags=ReconstructionSettings{hydrogens:false,bonds:false,side_chains:false,backbone:false,highlight:true};
        assert_eq!(reconstruct(&input,flags).molecule.bonds[2].order,1);
        flags.bonds=true;
        let fixed=reconstruct(&input,flags);
        assert_eq!(fixed.molecule.bonds[2].order,2);
        assert_eq!(input.bonds[2].order,1);
    }
    #[test] fn highlight_toggle_does_not_change_added_atom_count() {
        let input=alanine();
        let mut flags=ReconstructionSettings{hydrogens:true,bonds:true,side_chains:false,backbone:false,highlight:true};
        let marked=reconstruct(&input,flags);
        flags.highlight=false;
        let normal=reconstruct(&input,flags);
        assert!(marked.stats.hydrogen_atoms>0);
        assert_eq!(marked.molecule.atoms.len(),normal.molecule.atoms.len());
        assert!(normal.molecule.estimated_atoms.is_empty());
        assert!(!marked.molecule.estimated_atoms.is_empty());
    }
}
