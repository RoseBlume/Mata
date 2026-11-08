//! Local VSEPR classification for main-group central atoms.
//!
//! Each distinct neighboring atom is one bonding domain, regardless of
//! single/double/triple/aromatic bond order. Nonbonding pairs contribute
//! separate domains. Imported organic structures may omit hydrogens, so a
//! cautious default-valence estimate is shown explicitly in the results.
//! These are idealized classifications, not measured bond angles.
use crate::chemistry::{electrons, Molecule};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VseprCenter {
    /// Zero-based atom index, corresponding to the order in MOL/SDF/MOL2.
    pub atom_index: usize,
    pub symbol: String,
    /// Number of explicit bonds plus any estimated implicit hydrogens.
    pub bonded_atoms: usize,
    pub explicit_bonds: usize,
    pub inferred_hydrogens: usize,
    pub lone_pairs: usize,
    pub electron_domains: usize,
    /// Standard AXmEn notation, e.g. AX2E2 for water.
    pub axe_notation: String,
    pub electron_geometry: &'static str,
    pub molecular_geometry: &'static str,
    /// Indicative angle(s), not measured from 3D coordinates.
    pub approximate_angles: &'static str,
}

impl VseprCenter {
    pub fn label(&self) -> String {
        format!("{}{}", self.symbol, self.atom_index + 1)
    }
}

/// Returns each central atom for which a conventional VSEPR shape can be
/// assigned. The index in each result is the original atom number minus one.
/// Unusual valences, radicals, metals and unsupported environments are omitted
/// instead of assigning a fabricated geometry.
pub fn centers(m: &Molecule) -> Vec<VseprCenter> {
    (0..m.atoms.len())
        .filter_map(|atom_index| classify(m, atom_index))
        .collect()
}

pub fn classify(m: &Molecule, atom_index: usize) -> Option<VseprCenter> {
    let atom = m.atoms.get(atom_index)?;
    let valence_electrons = electrons(&atom.symbol)?;
    let explicit_bonds = m.degree(atom_index);
    let inferred_hydrogens = implicit_hydrogens(m, atom_index);
    let bonded_atoms = explicit_bonds + inferred_hydrogens;
    // A terminal atom with only one attachment isn't a VSEPR central atom.
    if bonded_atoms < 2 { return None; }
    let bonding_electrons = m.bond_order_sum(atom_index) + inferred_hydrogens as i32;
    let remainder = valence_electrons - bonding_electrons - atom.charge as i32;
    if remainder < 0 { return None; }
    // An unpaired electron means an open-shell/radical environment. Aromatic
    // ring pi electrons are delocalized, so they are an expected exception.
    let is_aromatic = m.bonds.iter().any(|bond| bond.order == 4 && (bond.a == atom_index || bond.b == atom_index));
    if remainder % 2 != 0 && !is_aromatic { return None; }
    let lone_pairs = (remainder / 2) as usize;
    let electron_domains = bonded_atoms + lone_pairs;
    if !(2..=6).contains(&electron_domains) { return None; }
    let (electron_geometry, molecular_geometry, approximate_angles) = geometry(bonded_atoms, lone_pairs)?;
    let axe_notation = if lone_pairs == 0 {
        format!("AX{bonded_atoms}")
    } else if lone_pairs == 1 {
        format!("AX{bonded_atoms}E")
    } else {
        format!("AX{bonded_atoms}E{lone_pairs}")
    };
    Some(VseprCenter {
        atom_index, symbol: atom.symbol.clone(), bonded_atoms, explicit_bonds,
        inferred_hydrogens, lone_pairs, electron_domains, axe_notation,
        electron_geometry, molecular_geometry, approximate_angles,
    })
}

/// Estimate implicit hydrogens only for imported neutral main-group atoms.
/// Explicitly generated molecules already contain all their hydrogens. The
/// imported H estimate is presented in the UI because it may be inappropriate
/// for radicals, specialized atom types, or organometallic species.
pub(crate) fn implicit_hydrogens(m: &Molecule, i: usize) -> usize {
    let atom = &m.atoms[i];
    if !m.from_file || atom.charge != 0 { return 0; }
    let aromatic = m.bonds.iter().any(|b| b.order == 4 && (b.a == i || b.b == i));
    let used = m.bond_order_sum(i);
    let target = match atom.symbol.as_str() {
        // Aromatic carbon has three sigma-bonded neighbors at most; the
        // fourth valence electron participates in the delocalized pi system.
        "C" if aromatic => 3,
        "C" | "Si" => 4,
        // Aromatic nitrogens may be pyridinic, pyrrolic or charged; avoid
        // silently choosing a hydrogenation state for those atom types.
        "N" if !aromatic => 3,
        "O" if !aromatic => 2,
        "B" if !aromatic => 3,
        "P" if !aromatic && used <= 3 => 3,
        "S" if !aromatic && used <= 2 => 2,
        _ => return 0,
    };
    if used < 0 || used > target { return 0; }
    ((target - used) as usize).min(4)
}

fn geometry(x: usize, e: usize) -> Option<(&'static str, &'static str, &'static str)> {
    Some(match (x, e) {
        (2, 0) => ("Linear", "Linear", "180 deg"),
        (3, 0) => ("Trigonal planar", "Trigonal planar", "120 deg"),
        (2, 1) => ("Trigonal planar", "Bent", "Less than 120 deg"),
        (4, 0) => ("Tetrahedral", "Tetrahedral", "109.5 deg"),
        (3, 1) => ("Tetrahedral", "Trigonal pyramidal", "About 107 deg"),
        (2, 2) => ("Tetrahedral", "Bent", "About 104.5 deg"),
        (5, 0) => ("Trigonal bipyramidal", "Trigonal bipyramidal", "90 / 120 / 180 deg"),
        (4, 1) => ("Trigonal bipyramidal", "Seesaw", "Near 90 / 120 / 180 deg"),
        (3, 2) => ("Trigonal bipyramidal", "T-shaped", "Near 90 / 180 deg"),
        (2, 3) => ("Trigonal bipyramidal", "Linear", "180 deg"),
        (6, 0) => ("Octahedral", "Octahedral", "90 / 180 deg"),
        (5, 1) => ("Octahedral", "Square pyramidal", "Near 90 / 180 deg"),
        (4, 2) => ("Octahedral", "Square planar", "90 / 180 deg"),
        (3, 3) => ("Octahedral", "T-shaped", "Near 90 / 180 deg"),
        (2, 4) => ("Octahedral", "Linear", "180 deg"),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chemistry::propose;

    fn only_center(formula: &str, symbol: &str) -> VseprCenter {
        let variants = propose(formula).unwrap();
        centers(&variants[0]).into_iter().find(|c| c.symbol == symbol).unwrap()
    }

    #[test]
    fn common_molecules() {
        let cases = [
            ("H2O", "O", "AX2E2", "Bent", "Tetrahedral"),
            ("NH3", "N", "AX3E", "Trigonal pyramidal", "Tetrahedral"),
            ("CH4", "C", "AX4", "Tetrahedral", "Tetrahedral"),
            ("CO2", "C", "AX2", "Linear", "Linear"),
            ("NO3^-", "N", "AX3", "Trigonal planar", "Trigonal planar"),
            ("CO3^2-", "C", "AX3", "Trigonal planar", "Trigonal planar"),
            ("SF6", "S", "AX6", "Octahedral", "Octahedral"),
            ("PCl5", "P", "AX5", "Trigonal bipyramidal", "Trigonal bipyramidal"),
        ];
        for (formula, atom, axe, shape, electron_shape) in cases {
            let c = only_center(formula, atom);
            assert_eq!(c.axe_notation, axe, "{formula}");
            assert_eq!(c.molecular_geometry, shape, "{formula}");
            assert_eq!(c.electron_geometry, electron_shape, "{formula}");
        }
    }

    #[test]
    fn separate_centers_for_multiple_atom_molecule() {
        let variants = propose("C2H6O").unwrap();
        assert!(variants.iter().all(|m| centers(m).len() == 3));
        let ethanol = &variants[0];
        let result = centers(ethanol);
        assert_eq!(result.iter().filter(|c| c.symbol == "C").count(), 2);
        assert_eq!(result.iter().filter(|c| c.symbol == "O").count(), 1);
        assert_eq!(result.iter().find(|c| c.symbol == "O").unwrap().molecular_geometry, "Bent");
    }

    #[test]
    fn double_and_triple_bonds_are_single_domains() {
        let mut m = Molecule::new("Test");
        let center = m.atom("C");
        for _ in 0..2 { let o = m.atom("O"); m.bond(center, o, 2); }
        assert_eq!(classify(&m, center).unwrap().axe_notation, "AX2");
        let acetylene = propose("C2H2").unwrap();
        for c in centers(&acetylene[0]) { if c.symbol == "C" { assert_eq!(c.axe_notation, "AX2"); } }
    }

    #[test]
    fn imported_implicit_hydrogens() {
        let mut m = Molecule::new("Heavy atoms only ethanol");
        m.from_file = true;
        let c1 = m.atom("C"); let c2 = m.atom("C"); let o = m.atom("O");
        m.bond(c1, c2, 1); m.bond(c2, o, 1);
        let list = centers(&m);
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].inferred_hydrogens, 3);
        assert_eq!(list[0].molecular_geometry, "Tetrahedral");
        assert_eq!(list[2].inferred_hydrogens, 1);
        assert_eq!(list[2].molecular_geometry, "Bent");
        assert_eq!(m.lone_pairs(c1), 0);
        assert_eq!(m.lone_pairs(o), 2);
    }

    #[test]
    fn aromatic_carbon_is_trigonal_planar_even_with_implicit_h() {
        let mut m = Molecule::new("benzene heavy atoms");
        m.from_file = true;
        for _ in 0..6 { m.atom("C"); }
        for i in 0..6 { m.bond(i, (i+1)%6, 4); }
        for c in centers(&m) {
            assert_eq!(c.inferred_hydrogens, 1);
            assert_eq!(c.axe_notation, "AX3");
            assert_eq!(c.molecular_geometry, "Trigonal planar");
        }
    }

    #[test]
    fn geometry_table_supported_variants() {
        for (x,e) in [(2,0),(3,0),(2,1),(4,0),(3,1),(2,2),(5,0),(4,1),(3,2),(2,3),(6,0),(5,1),(4,2),(3,3),(2,4)] {
            assert!(geometry(x,e).is_some(), "AX{x}E{e}");
        }
        assert!(geometry(7,0).is_none());
    }
}
