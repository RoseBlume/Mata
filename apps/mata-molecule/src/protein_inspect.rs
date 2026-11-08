//! Residue-centric inspection for PDB and mmCIF models. No extra dependencies.
//! Atom identity is taken from the imported coordinate model; chemistry templates
//! improve bond orders only where a recognized canonical residue allows it.
use crate::chemistry::{Bond, Molecule};
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResidueKey {
    pub chain: String,
    pub number: String,
    pub insertion: String,
    pub name: String,
}
impl ResidueKey {
    pub fn display(&self) -> String {
        format!("{} {}{} {}", if self.chain.is_empty() {"?"} else {&self.chain},
            self.number,self.insertion,self.name)
    }
}
#[derive(Clone, Debug)]
pub struct Residue {
    pub key: ResidueKey,
    pub atoms: Vec<usize>,
    pub one_letter: char,
    pub is_polymer: bool,
}

pub fn one_letter(s: &str) -> char {
    match s.trim().to_ascii_uppercase().as_str() {
        "ALA"=>'A',"ARG"=>'R',"ASN"=>'N',"ASP"=>'D',"CYS"=>'C',
        "GLU"=>'E',"GLN"=>'Q',"GLY"=>'G',"HIS"=>'H',"ILE"=>'I',
        "LEU"=>'L',"LYS"=>'K',"MET"=>'M',"PHE"=>'F',"PRO"=>'P',
        "SER"=>'S',"THR"=>'T',"TRP"=>'W',"TYR"=>'Y',"VAL"=>'V',
        "MSE"=>'M',_=>'X',
    }
}
pub fn full_name(s: &str) -> &'static str {
    match s.trim().to_ascii_uppercase().as_str() {
        "ALA"=>"Alanine", "ARG"=>"Arginine", "ASN"=>"Asparagine", "ASP"=>"Aspartate",
        "CYS"=>"Cysteine", "GLU"=>"Glutamate", "GLN"=>"Glutamine", "GLY"=>"Glycine",
        "HIS"=>"Histidine", "ILE"=>"Isoleucine", "LEU"=>"Leucine", "LYS"=>"Lysine",
        "MET"=>"Methionine", "PHE"=>"Phenylalanine", "PRO"=>"Proline", "SER"=>"Serine",
        "THR"=>"Threonine", "TRP"=>"Tryptophan", "TYR"=>"Tyrosine", "VAL"=>"Valine",
        "MSE"=>"Selenomethionine", _=>"Unclassified residue / ligand",
    }
}
/// First-seen PDB/mmCIF coordinate order is preserved for sequence browsing.
/// Chain+residue number+insertion code form distinct residue identities.
pub fn residues(m: &Molecule) -> Vec<Residue> {
    let mut groups=Vec::<Residue>::new();
    let mut positions=HashMap::<ResidueKey,usize>::new();
    for (idx,a) in m.atoms.iter().enumerate(){
        let Some(site)=a.site.as_ref() else { continue };
        let key=ResidueKey{chain:site.chain_id.clone(),number:site.residue_number.clone(),
            insertion:site.insertion_code.clone(),name:site.residue_name.clone()};
        let row=match positions.get(&key) {Some(&i)=>i,None=>{
            let i=groups.len();let aa=one_letter(&key.name);
            groups.push(Residue{key:key.clone(),atoms:Vec::new(),one_letter:aa,
                is_polymer:!site.hetero && aa!='X'});
            positions.insert(key,i);i
        }};
        groups[row].atoms.push(idx);
    }
    groups
}

/// Canonical side-chain heavy-atom bonds (PDB atom-name pairs). The normal
/// residue inspector uses observed atoms only; optional reconstruction uses
/// these names to create **explicitly flagged estimates**. Ring bonds use 4.
/// In protein chains the terminal N/C valences differ from free amino acids.
pub fn template(name:&str)->Vec<(&'static str,&'static str,u8)> {
    let mut pairs:Vec<(&'static str,&'static str,u8)>=vec![
        ("N","CA",1),("CA","C",1),("C","O",2),("C","OXT",1),
    ];
    if name!="GLY" {pairs.push(("CA","CB",1));}
    let edges:&[(&str,&str,u8)]=match name {
        "VAL"=>&[("CB","CG1",1),("CB","CG2",1)],
        "LEU"=>&[("CB","CG",1),("CG","CD1",1),("CG","CD2",1)],
        "ILE"=>&[("CB","CG1",1),("CB","CG2",1),("CG1","CD1",1)],
        "SER"=>&[("CB","OG",1)],"THR"=>&[("CB","OG1",1),("CB","CG2",1)],
        "CYS"=>&[("CB","SG",1)],
        "MET"=>&[("CB","CG",1),("CG","SD",1),("SD","CE",1)],
        "MSE"=>&[("CB","CG",1),("CG","SE",1),("SE","CE",1)],
        "ASN"=>&[("CB","CG",1),("CG","OD1",2),("CG","ND2",1)],
        "ASP"=>&[("CB","CG",1),("CG","OD1",2),("CG","OD2",1)],
        "GLN"=>&[("CB","CG",1),("CG","CD",1),("CD","OE1",2),("CD","NE2",1)],
        "GLU"=>&[("CB","CG",1),("CG","CD",1),("CD","OE1",2),("CD","OE2",1)],
        "LYS"=>&[("CB","CG",1),("CG","CD",1),("CD","CE",1),("CE","NZ",1)],
        "ARG"=>&[("CB","CG",1),("CG","CD",1),("CD","NE",1),
                 ("NE","CZ",1),("CZ","NH1",2),("CZ","NH2",1)],
        "PRO"=>&[("CB","CG",1),("CG","CD",1),("CD","N",1)],
        "PHE"|"TYR"=>&[("CB","CG",1),("CG","CD1",4),("CG","CD2",4),
                 ("CD1","CE1",4),("CD2","CE2",4),("CE1","CZ",4),("CE2","CZ",4)],
        "HIS"=>&[("CB","CG",1),("CG","ND1",4),("ND1","CE1",4),
                 ("CE1","NE2",4),("NE2","CD2",4),("CD2","CG",4)],
        "TRP"=>&[("CB","CG",1),("CG","CD1",4),("CD1","NE1",4),
                 ("NE1","CE2",4),("CE2","CD2",4),("CD2","CG",4),
                 ("CD2","CE3",4),("CE3","CZ3",4),("CZ3","CH2",4),
                 ("CH2","CZ2",4),("CZ2","CE2",4)],
        _=>&[],
    };
    pairs.extend_from_slice(edges);
    if name=="TYR" {pairs.push(("CZ","OH",1));}
    pairs
}

pub struct ResidueStructure {
    pub molecule: Molecule,
    pub recovered_orders: bool,
    pub has_missing_hydrogens: bool,
}
/// Extract exactly the observed atoms, preserving coordinates, residue metadata,
/// source connectivity and any existing non-standard bonds. Recognized residue
/// templates can repair incorrect *distance-inferred* single bond orders.
pub fn extract(m:&Molecule,r:&Residue)->ResidueStructure {
    let mut result=Molecule::new(&format!("{} - {}",r.key.display(),full_name(&r.key.name)));
    result.from_file=true;result.apply_templates=m.apply_templates;
    let mut map=HashMap::new();
    let mut names=HashMap::<String,usize>::new();
    for &old in &r.atoms {
        let id=result.atoms.len();map.insert(old,id);
        let atom=m.atoms[old].clone();
        if let Some(site)=&atom.site { names.insert(site.atom_name.trim().to_ascii_uppercase(),id); }
        result.atoms.push(atom);
        if m.estimated_atoms.binary_search(&old).is_ok() {result.estimated_atoms.push(id);}
    }
    let mut indexed=BTreeMap::<(usize,usize),u8>::new();
    for b in &m.bonds {
        if let (Some(&a),Some(&c))=(map.get(&b.a),map.get(&b.b)) {
            indexed.insert((a.min(c),a.max(c)),b.order);
        }
    }
    let canonical=one_letter(&r.key.name)!='X';
    let mut repaired=false;
    if canonical && m.apply_templates {
        for (an,bn,order) in template(&r.key.name) {
            if let (Some(&a),Some(&b))=(names.get(an),names.get(bn)) {
                let edge=(a.min(b),a.max(b));
                if indexed.insert(edge,order)!=Some(order){repaired=true;}
            }
        }
    }
    result.bonds=indexed.into_iter().map(|((a,b),order)|Bond{a,b,order}).collect();
    ResidueStructure{has_missing_hydrogens:canonical && !result.atoms.iter().any(|a|a.symbol=="H"),
        molecule:result,recovered_orders:repaired}
}

#[cfg(test)] mod tests {
    use super::*;
    use crate::chemistry::{Atom,AtomSite};
    use mata_3d::Vec3;
    fn residue_atom(m:&mut Molecule,name:&str,res:&str,num:&str,chain:&str){
        let mut a=Atom::new(if name=="N"{"N"}else if name=="O"{"O"}else{"C"});
        a.pos=Vec3::new(m.atoms.len() as f32,0.0,0.0);
        a.site=Some(AtomSite{atom_name:name.into(),residue_name:res.into(),chain_id:chain.into(),residue_number:num.into(),insertion_code:"".into(),
        alternate_location:"".into(),occupancy:1.0,b_factor:0.0,hetero:false,serial:(m.atoms.len()+1).to_string()});
        m.atoms.push(a);
    }
    #[test] fn grouping_does_not_merge_chains(){
        let mut m=Molecule::new("protein");
        residue_atom(&mut m,"N","ALA","1","A");residue_atom(&mut m,"CA","ALA","1","A");
        residue_atom(&mut m,"N","ALA","1","B");
        let found=residues(&m);assert_eq!(found.len(),2);assert_eq!(found[0].atoms.len(),2);
    }
    #[test] fn fixes_carbonyl_and_preserves_source_positions(){
        let mut m=Molecule::new("protein");
        for name in ["N","CA","C","O","CB"]{residue_atom(&mut m,name,"ALA","14","A");}
        m.bond(2,3,1);
        let r=extract(&m,&residues(&m)[0]);
        assert!(r.recovered_orders);
        assert!(r.molecule.bonds.iter().any(|b|((b.a==2&&b.b==3)||(b.a==3&&b.b==2))&&b.order==2));
        assert_eq!(r.molecule.atoms[3].pos,m.atoms[3].pos);
    }
    #[test] fn aromatic_phenyl_ring(){
        let mut m=Molecule::new("ring");
        for name in ["CG","CD1","CD2","CE1","CE2","CZ"] {residue_atom(&mut m,name,"PHE","3","A");}
        let r=extract(&m,&residues(&m)[0]);
        assert_eq!(r.molecule.bonds.iter().filter(|b|b.order==4).count(),6);
    }
}

#[cfg(test)] mod pdb_integration_tests {
    use super::*;
    #[test] fn real_two_chain_protein_supports_residue_inspection(){
        let pdb=include_str!("../../samples/protein-two-chains.pdb");
        let models=crate::macromolecule::read_pdb(pdb).expect("valid PDB model");
        let m=&models[0];
        assert_eq!(m.atoms.len(),6818);
        let list=residues(m);
        let chain_a=list.iter().filter(|r|r.key.chain=="A"&&r.is_polymer).count();
        let chain_b=list.iter().filter(|r|r.key.chain=="B"&&r.is_polymer).count();
        assert_eq!((chain_a,chain_b),(435,435));
        let selected=list.iter().find(|r|r.key.chain=="B"&&r.key.name=="TYR").unwrap();
        let residue=extract(m,selected);
        assert!(residue.molecule.atoms.iter().any(|a|a.site.as_ref().is_some_and(|s|s.atom_name=="OH")));
        assert!(residue.molecule.bonds.iter().any(|b|b.order==4));
    }
}

/// A coordinate-derived backbone dihedral, in degrees (-180..=180).
/// These are geometric torsions, not a DSSP secondary-structure assignment.
fn torsion(a:crate::Vec3,b:crate::Vec3,c:crate::Vec3,d:crate::Vec3)->Option<f32>{
    let u=a-b;let v=c-b;let w=d-c;
    let n1=u.cross(v);let n2=v.cross(w);
    if n1.len()<1e-5 || n2.len()<1e-5 || v.len()<1e-5{return None;}
    let left=n1.normalized();let right=n2.normalized();
    Some(right.cross(left).dot(v.normalized()).atan2(left.dot(right)).to_degrees())
}
fn atom_pos(m:&Molecule,r:&Residue,name:&str)->Option<crate::Vec3>{
    r.atoms.iter().filter_map(|&i|m.atoms.get(i))
        .find(|a|a.site.as_ref().is_some_and(|s|s.atom_name.trim()==name))
        .map(|a|a.pos)
}
/// Phi and psi for the selected protein-bound residue. Chain breaks and missing
/// backbone atoms are reported as `None` instead of invented torsion values.
pub fn phi_psi(m:&Molecule,res:&[Residue],index:usize)->(Option<f32>,Option<f32>){
    let Some(curr)=res.get(index) else{return (None,None)};
    if !curr.is_polymer{return (None,None)}
    let (n,ca,c)=(atom_pos(m,curr,"N"),atom_pos(m,curr,"CA"),atom_pos(m,curr,"C"));
    let mut phi=None;let mut psi=None;
    if let (Some(prev),Some(n),Some(ca),Some(c))=(index.checked_sub(1).and_then(|i|res.get(i)),n,ca,c){
        if prev.key.chain==curr.key.chain && prev.is_polymer {
            if let Some(pc)=atom_pos(m,prev,"C") {
                if (pc-n).len()<1.9 {phi=torsion(pc,n,ca,c);}
            }
        }
    }
    if let (Some(next),Some(n),Some(ca),Some(c))=(res.get(index+1),n,ca,c){
        if next.key.chain==curr.key.chain && next.is_polymer {
            if let Some(nn)=atom_pos(m,next,"N") {
                if (c-nn).len()<1.9 {psi=torsion(n,ca,c,nn);}
            }
        }
    }
    (phi,psi)
}
/// A rough region of the Ramachandran map. This is deliberately not labeled
/// as a secondary structure assignment: true helix/sheet detection also
/// requires patterns across several residues and hydrogen-bond geometry.
pub fn backbone_region(phi:Option<f32>,psi:Option<f32>)->&'static str{
    match (phi,psi) {
        (Some(p),Some(q)) if (-100.0..=-30.0).contains(&p)&&(-85.0..=-5.0).contains(&q)=>"Alpha-helical region",
        (Some(p),Some(q)) if (-180.0..=-90.0).contains(&p)&&(q>=80.0||q<=-130.0)=>"Extended beta region",
        (Some(p),Some(q)) if (-110.0..=-45.0).contains(&p)&&(90.0..=180.0).contains(&q)=>"Polyproline-II region",
        (Some(_),Some(_))=>"Other / turn-like region",
        _=>"Backbone angles unavailable",
    }
}
#[cfg(test)] mod torsion_tests {
    use super::*;
    use crate::Vec3;
    #[test] fn torsion_needs_four_noncollinear_positions(){
        assert!(torsion(Vec3::default(),Vec3::default(),Vec3::default(),Vec3::default()).is_none());
        let a=Vec3::new(0.0,0.0,0.0);let b=Vec3::new(1.0,0.0,0.0);
        let c=Vec3::new(1.0,1.0,0.0);let d=Vec3::new(1.0,1.0,1.0);
        assert!((torsion(a,b,c,d).unwrap().abs()-90.0).abs()<0.1);
    }
}

/// Chemical drawing only: restore standard heavy-atom bond multiplicity for
/// recognizable residue atom-name pairs. Imported non-single bonds remain
/// authoritative. Unknown groups retain the imported/inferred single bonds;
/// this never adds bonds or invents missing atoms.
pub fn display_bond_order(m:&Molecule,b:&Bond)->u8 {
    if b.order!=1 {return b.order;}
    let Some(a)=m.atoms.get(b.a).and_then(|a|a.site.as_ref()) else{return b.order;};
    let Some(z)=m.atoms.get(b.b).and_then(|a|a.site.as_ref()) else{return b.order;};
    if a.chain_id!=z.chain_id || a.residue_number!=z.residue_number
        || a.insertion_code!=z.insertion_code || a.residue_name!=z.residue_name {
        return b.order;
    }
    template(a.residue_name.as_str()).iter()
        .find_map(|&(start,end,order)| {
            ((a.atom_name.trim()==start && z.atom_name.trim()==end)
             ||(a.atom_name.trim()==end && z.atom_name.trim()==start))
                 .then_some(order)
        }).unwrap_or(b.order)
}

#[cfg(test)]
mod bond_display_tests {
    use super::*;
    #[test]
    fn recognizes_standard_carbonyl_from_single_bond() {
        let mut m=Molecule::new("aspartate");
        for atom in ["CG","OD1"] {
            let i=m.atom(if atom=="OD1"{"O"}else{"C"});
            m.atoms[i].site=Some(crate::chemistry::AtomSite{
                atom_name:atom.into(),residue_name:"ASP".into(),chain_id:"A".into(),
                residue_number:"8".into(),insertion_code:String::new(),alternate_location:String::new(),
                occupancy:1.0,b_factor:0.0,hetero:false,serial:(i+1).to_string(),
            });
        }
        m.bond(0,1,1);
        assert_eq!(display_bond_order(&m,&m.bonds[0]),2);
    }
}
