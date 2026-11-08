//! Approximate protein secondary-structure visualization from observed backbone
//! coordinates. These are phi/psi heuristics, NOT DSSP, STRIDE, or the source
//! file's experimentally annotated HELIX/SHEET records.
//! Missing backbone atoms stay missing; no invented residues or coordinates.
use mata_molecule::{Molecule, Vec3};
use mata_molecule::protein_inspect;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Secondary { Helix, Strand, Loop }

impl Secondary {
    pub fn color(self) -> u32 {
        match self {
            Self::Helix => 0xffe979b0,
            Self::Strand => 0xffffd36c,
            Self::Loop => 0xff63c9dd,
        }
    }
}

#[derive(Clone, Debug)]
pub struct BackboneNode {
    pub ca: Vec3,
    pub ca_index: usize,
    pub n: Option<Vec3>,
    pub c: Option<Vec3>,
    pub o: Option<Vec3>,
    pub chain: String,
    pub name: String,
    pub number: String,
    pub insertion: String,
    pub secondary: Secondary,
}

/// C-alpha positions and optional observed backbone coordinates, in source
/// sequence order. The caller supplies the display filter for isolated chains
/// and focused residue regions. Residues missing CA are not fabricated.
pub fn backbone_nodes(m: &Molecule, keep: impl Fn(usize) -> bool) -> Vec<BackboneNode> {
    let mut out = Vec::new();
    for residue in protein_inspect::residues(m) {
        if !residue.is_polymer { continue; }
        let mut ca = None;
        let mut n = None;
        let mut c = None;
        let mut o = None;
        for &idx in &residue.atoms {
            let Some(atom) = m.atoms.get(idx) else { continue; };
            let Some(site) = atom.site.as_ref() else { continue; };
            // An alternate-location atom not selected by the loader should not
            // silently replace the first observed atom of that name.
            match site.atom_name.trim() {
                "CA" if keep(idx) && ca.is_none() => ca = Some((idx, atom.pos)),
                "N" if n.is_none() => n = Some(atom.pos),
                "C" if c.is_none() => c = Some(atom.pos),
                "O" if o.is_none() => o = Some(atom.pos),
                _ => {}
            }
        }
        if let Some((ca_index, point)) = ca {
            out.push(BackboneNode {
                ca: point, ca_index, n, c, o,
                chain: residue.key.chain,
                name: residue.key.name,
                number: residue.key.number,
                insertion: residue.key.insertion,
                secondary: Secondary::Loop,
            });
        }
    }
    assign_secondary(&mut out);
    out
}

pub fn consecutive(a: &BackboneNode, b: &BackboneNode) -> bool {
    if a.chain != b.chain { return false; }
    let delta = (a.ca - b.ca).len();
    if !(2.1..=4.7).contains(&delta) { return false; }
    if let (Ok(na), Ok(nb)) = (a.number.trim().parse::<i64>(), b.number.trim().parse::<i64>()) {
        // Different insertion codes may share the same residue number.
        let delta = i128::from(nb) - i128::from(na);
        if !(0..=1).contains(&delta) { return false; }
        if delta==0 && a.insertion==b.insertion { return false; }
    }
    true
}

fn dihedral(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> Option<f32> {
    let u = a - b;
    let v = c - b;
    let w = d - c;
    let n1 = u.cross(v);
    let n2 = v.cross(w);
    if n1.len() < 1.0e-5 || n2.len() < 1.0e-5 || v.len() < 1.0e-5 { return None; }
    let x = n1.normalized();
    let y = n2.normalized();
    Some(y.cross(x).dot(v.normalized()).atan2(x.dot(y)).to_degrees())
}

fn phi_psi(residues: &[BackboneNode], i: usize) -> (Option<f32>, Option<f32>) {
    let cur = &residues[i];
    let mut phi = None;
    let mut psi = None;
    if i > 0 && consecutive(&residues[i-1], cur) {
        if let (Some(c_prev), Some(n), Some(c)) = (residues[i-1].c, cur.n, cur.c) {
            if (c_prev-n).len() < 1.9 { phi = dihedral(c_prev, n, cur.ca, c); }
        }
    }
    if i+1 < residues.len() && consecutive(cur, &residues[i+1]) {
        if let (Some(n), Some(c), Some(n_next)) = (cur.n, cur.c, residues[i+1].n) {
            if (c-n_next).len() < 1.9 { psi = dihedral(n, cur.ca, c, n_next); }
        }
    }
    (phi, psi)
}

/// Conservative Ramachandran-region estimate. Geometry alone does not identify
/// H-bonded beta sheets: the yellow strand ribbons are candidate extended
/// secondary structure, not a formal DSSP sheet assignment.
fn classify(phi: f32, psi: f32) -> Secondary {
    if (-105.0..=-35.0).contains(&phi) && (-85.0..=5.0).contains(&psi) {
        Secondary::Helix
    } else if (-180.0..=-75.0).contains(&phi)
        && ((65.0..=180.0).contains(&psi) || (-180.0..=-105.0).contains(&psi)) {
        Secondary::Strand
    } else { Secondary::Loop }
}

/// Eliminate isolated dihedrals that do not form a meaningful continuous
/// segment (>=3 helix residues, >=2 strand residues).
pub fn assign_secondary(nodes: &mut [BackboneNode]) {
    let raw: Vec<Secondary> = (0..nodes.len()).map(|i| {
        match phi_psi(nodes, i) {
            (Some(p), Some(q)) => classify(p,q),
            _ => Secondary::Loop,
        }
    }).collect();
    let mut i = 0;
    while i < nodes.len() {
        let mut j = i+1;
        while j < nodes.len() && raw[j] == raw[i] && consecutive(&nodes[j-1], &nodes[j]) {
            j+=1;
        }
        let required = match raw[i] { Secondary::Helix=>3, Secondary::Strand=>2, Secondary::Loop=>1 };
        if j-i >= required {
            for n in &mut nodes[i..j] { n.secondary = raw[i]; }
        }
        i=j;
    }
}

/// Smooth a backbone segment using 3D Catmull-Rom interpolation. The caller
/// must not pass across a chain break.
pub fn smooth_segment(a:Vec3,b:Vec3,c:Vec3,d:Vec3,t:f32)->Vec3 {
    let t2=t*t;
    let t3=t2*t;
    (b*2.0 + (c-a)*t + (a*2.0-b*5.0+c*4.0-d)*t2
        + (a*-1.0+b*3.0-c*3.0+d)*t3)*0.5
}

#[cfg(test)]
mod tests {
    use super::*;
    use mata_molecule::chemistry::{Atom,AtomSite};

    fn node(chain:&str,n:i32,x:f32)->BackboneNode {
        BackboneNode{ca:Vec3::new(x,0.0,0.0),ca_index:0,n:None,c:None,o:None,
            chain:chain.to_owned(),name:"ALA".into(),number:n.to_string(),insertion:String::new(),secondary:Secondary::Loop}
    }
    #[test]
    fn no_ribbon_across_chain_or_missing_residue() {
        assert!(consecutive(&node("A",1,0.0),&node("A",2,3.8)));
        assert!(!consecutive(&node("A",1,0.0),&node("B",2,3.8)));
        assert!(!consecutive(&node("A",1,0.0),&node("A",3,3.8)));
        assert!(!consecutive(&node("A",1,0.0),&node("A",2,8.8)));
    }
    #[test]
    fn ramanchandran_regions() {
        assert_eq!(classify(-63.0,-42.0),Secondary::Helix);
        assert_eq!(classify(-135.0,140.0),Secondary::Strand);
        assert_eq!(classify(70.0,15.0),Secondary::Loop);
    }
    #[test]
    fn curve_interpolates_endpoints() {
        let a=Vec3::new(-1.0,0.0,0.0);
        let b=Vec3::new(0.0,1.0,0.0);
        let c=Vec3::new(1.0,1.0,0.0);
        let d=Vec3::new(2.0,0.0,0.0);
        assert!((smooth_segment(a,b,c,d,0.0)-b).len()<1.0e-5);
        assert!((smooth_segment(a,b,c,d,1.0)-c).len()<1.0e-5);
    }
    #[test]
    fn pdb_fixture_has_real_backbone_and_continuous_segments() {
        let text=include_str!("../../samples/protein-two-chains.pdb");
        let models=mata_molecule::macromolecule::read_pdb(text).expect("PDB fixture");
        let nodes=backbone_nodes(&models[0],|_|true);
        assert_eq!(nodes.len(),870);
        assert!(nodes.windows(2).filter(|p|consecutive(&p[0],&p[1])).count()>850);
        assert!(nodes.iter().any(|p|p.secondary==Secondary::Strand));
        let breaks=nodes.windows(2).filter(|p|p[0].chain!=p[1].chain).collect::<Vec<_>>();
        assert_eq!(breaks.len(),1);
        assert!(!consecutive(&breaks[0][0],&breaks[0][1]));
    }
    #[test]
    fn backbone_uses_observed_ca_only() {
        let mut m=Molecule::new("test");
        for (i,name) in ["N","CA","C","O","CB"].iter().enumerate(){
            let mut a=Atom::new("C");a.pos=Vec3::new(i as f32,0.0,0.0);
            a.site=Some(AtomSite{atom_name:name.to_string(),residue_name:"ALA".into(),
                chain_id:"A".into(),residue_number:"1".into(),insertion_code:String::new(),
                alternate_location:String::new(),occupancy:1.0,b_factor:0.0,
                hetero:false,serial:(i+1).to_string()});m.atoms.push(a);
        }
        let nodes=backbone_nodes(&m,|_|true);
        assert_eq!(nodes.len(),1);
        assert_eq!(nodes[0].ca_index,1);
    }
}
