//! Neutral-atom electronic reference configurations and occupied subshells.
//! This data is not a molecular-orbital calculation; ions, crystal fields and
//! bonding can substantially change the actual electronic distribution.
use mata_molecule::{Molecule,chemistry::atomic_number};

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Subshell { pub principal:u8, pub kind:char, pub occupancy:u8 }

/// Neutral atomic ground-state reference configurations through Lr (Z=103).
/// The Madelung sequence is corrected for known anomalous d/f-block atoms.
/// Beyond Lr we deliberately report unsupported rather than guess.
pub fn configuration(symbol:&str)->Option<Vec<Subshell>> {
    let z=atomic_number(symbol)? as u8;
    if z>103 {return None;}
    const ORDER:&[(u8,char,u8)]=&[
        (1,'s',2),(2,'s',2),(2,'p',6),(3,'s',2),(3,'p',6),
        (4,'s',2),(3,'d',10),(4,'p',6),(5,'s',2),(4,'d',10),
        (5,'p',6),(6,'s',2),(4,'f',14),(5,'d',10),(6,'p',6),
        (7,'s',2),(5,'f',14),(6,'d',10),(7,'p',6),
    ];
    let mut left=z;
    let mut shells=Vec::new();
    for &(principal,kind,capacity) in ORDER {
        if left==0 {break;}
        let occupancy=left.min(capacity);
        shells.push(Subshell{principal,kind,occupancy});
        left-=occupancy;
    }
    // Source: standard free neutral-atom reference configurations, including
    // the deviations in the transition, lanthanide, and actinide series.
    // Transfer from one subshell into another without changing electron total.
    // For Pd and early f-block atoms, two electrons may be transferred.
    let corrections:&[(u8,char,u8,char,u8)]=match z {
        24|29 => &[(4,'s',3,'d',1)],
        41|42|44|45|47 => &[(5,'s',4,'d',1)],
        46 => &[(5,'s',4,'d',2)],
        57|58|64 => &[(4,'f',5,'d',1)],
        78|79 => &[(6,'s',5,'d',1)],
        89|91|92|93|96 => &[(5,'f',6,'d',1)],
        90 => &[(5,'f',6,'d',2)],
        103 => &[(6,'d',7,'p',1)],
        _=>&[],
    };
    for &(from_n,from_kind,to_n,to_kind,amount) in corrections {
        let from=shells.iter_mut().find(|s|s.principal==from_n&&s.kind==from_kind)?;
        from.occupancy=from.occupancy.checked_sub(amount)?;
        if let Some(to)=shells.iter_mut().find(|s|s.principal==to_n&&s.kind==to_kind) {
            to.occupancy+=amount;
        }else {
            shells.push(Subshell{principal:to_n,kind:to_kind,occupancy:amount});
        }
    }
    shells.retain(|s|s.occupancy!=0);
    debug_assert_eq!(shells.iter().map(|s|s.occupancy as usize).sum::<usize>(),z as usize);
    Some(shells)
}

pub fn configuration_label(symbol:&str)->String {
    configuration(symbol).map_or_else(||"NOT MODELED".into(),|shells|{
        shells.iter().map(|s|format!("{}{}{}",s.principal,s.kind,s.occupancy))
            .collect::<Vec<_>>().join(" ")
    })
}

/// For the dock: abbreviated neutral reference, so f-block atoms fit without
/// their entire xenon/radon cores obscuring the outer d/f configuration.
pub fn configuration_short_label(symbol:&str)->String {
    let Some(shells)=configuration(symbol) else {return "NOT MODELED".into()};
    let z=atomic_number(symbol).unwrap_or(0);
    let (core_z,core)=if z>86 {(86,"[Rn]")}
        else if z>54 {(54,"[Xe]")}
        else if z>36 {(36,"[Kr]")}
        else if z>18 {(18,"[Ar]")}
        else if z>10 {(10,"[Ne]")}
        else if z>2 {(2,"[He]")}
        else {(0,"")};
    if core_z==0 {return configuration_label(symbol)}
    // A correction could transfer an electron out of a presumed core for
    // exotic atoms. These neutral references do not do so, but validate it.
    let core_shells=configuration(match core_z{2=>"He",10=>"Ne",18=>"Ar",36=>"Kr",54=>"Xe",_=>"Rn"});
    let Some(core_shells)=core_shells else {return configuration_label(symbol)};
    for c in &core_shells {
        if shells.iter().find(|s|s.principal==c.principal&&s.kind==c.kind)
            .is_none_or(|s|s.occupancy<c.occupancy) {return configuration_label(symbol)}
    }
    let mut remainder=shells;
    for c in core_shells {
        if let Some(s)=remainder.iter_mut().find(|s|s.principal==c.principal&&s.kind==c.kind) {
            s.occupancy-=c.occupancy;
        }
    }
    let rest=remainder.iter().filter(|s|s.occupancy>0)
        .map(|s|format!("{}{}{}",s.principal,s.kind,s.occupancy))
        .collect::<Vec<_>>().join(" ");
    format!("{core} {rest}").trim().into()
}

/// Subshells relevant for depicting an atom's external electronic structure:
/// ns, np, (n-1)d and (n-2)f, including a completely filled d/f shell.
/// This is a visualization choice, not an assertion of molecular valence.
pub fn outer_subshells(symbol:&str)->Option<Vec<Subshell>> {
    let cfg=configuration(symbol)?;
    let n=cfg.iter().map(|s|s.principal).max()?;
    Some(cfg.into_iter().filter(|s|
        (s.principal==n && matches!(s.kind,'s'|'p')) ||
        (s.principal.checked_add(1)==Some(n) && s.kind=='d') ||
        (s.principal.checked_add(2)==Some(n) && s.kind=='f')
    ).collect())
}

/// Backwards-compatible summary of outermost n s/p electron counts.
pub fn valence_sp(symbol:&str)->Option<(u8,u8,u8)> {
    let cfg=configuration(symbol)?;
    let n=cfg.iter().map(|s|s.principal).max()?;
    let occupancy=|kind|cfg.iter().find(|s|s.principal==n&&s.kind==kind).map_or(0,|s|s.occupancy);
    Some((n,occupancy('s'),occupancy('p')))
}

/// Central atom and directly bonded atoms from the loaded graph only.
pub fn bonded_atoms(m:&Molecule,center:usize)->Vec<usize> {
    if center>=m.atoms.len(){return Vec::new();}
    let mut indices=vec![center];
    for b in &m.bonds {
        let other=if b.a==center {Some(b.b)} else if b.b==center {Some(b.a)}else{None};
        if let Some(index)=other.filter(|&i|i<m.atoms.len()&&!indices.contains(&i)){indices.push(index);}
    }
    indices
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn configurations_are_ground_state_neutral_references(){
        assert_eq!(configuration_label("H"),"1s1");
        assert_eq!(configuration_label("C"),"1s2 2s2 2p2");
        assert_eq!(configuration_label("N"),"1s2 2s2 2p3");
        assert_eq!(configuration_label("O"),"1s2 2s2 2p4");
        assert_eq!(configuration_label("S"),"1s2 2s2 2p6 3s2 3p4");
        assert_eq!(configuration_label("Cr"),"1s2 2s2 2p6 3s2 3p6 4s1 3d5");
        assert_eq!(configuration_label("Cu"),"1s2 2s2 2p6 3s2 3p6 4s1 3d10");
        assert_eq!(valence_sp("Se"),Some((4,2,4)));
        assert_eq!(valence_sp("H"),Some((1,1,0)));
        assert!(configuration("I").is_some());
        assert_eq!(configuration_short_label("Pd"),"[Kr] 4d10");
        assert_eq!(configuration_short_label("Ce"),"[Xe] 6s2 4f1 5d1");
        assert_eq!(configuration_short_label("Gd"),"[Xe] 6s2 4f7 5d1");
        assert_eq!(configuration_short_label("U"),"[Rn] 7s2 5f3 6d1");
        assert_eq!(configuration_short_label("Lr"),"[Rn] 7s2 5f14 7p1");
        assert_eq!(configuration("Rf"),None);
    }
    #[test] fn all_supported_configurations_conserve_electrons_and_shell_capacity(){
        const CAPACITY:[(char,u8);4]=[('s',2),('p',6),('d',10),('f',14)];
        for z in 1..=103 {
            let symbol=mata_molecule::chemistry::ELEMENTS.split_whitespace().nth(z-1).unwrap();
            let cfg=configuration(symbol).unwrap();
            assert_eq!(cfg.iter().map(|s|s.occupancy as usize).sum::<usize>(),z,"{symbol}");
            for s in cfg {
                let cap=CAPACITY.iter().find(|x|x.0==s.kind).unwrap().1;
                assert!(s.occupancy>0&&s.occupancy<=cap,"{symbol}: {s:?}");
            }
        }
    }
    #[test] fn relevant_orbitals_include_d_and_f(){
        let fe=outer_subshells("Fe").unwrap();
        assert!(fe.iter().any(|s|s.kind=='d' && s.principal==3 && s.occupancy==6));
        let ce=outer_subshells("Ce").unwrap();
        assert!(ce.iter().any(|s|s.kind=='f' && s.principal==4 && s.occupancy==1));
        let u=outer_subshells("U").unwrap();
        assert!(u.iter().any(|s|s.kind=='f'&&s.occupancy==3));
    }
    #[test] fn bonded_neighbors_use_imported_connectivity(){
        let mut m=Molecule::new("test");
        for s in ["C","O","H","N"] {m.atom(s);}
        m.bond(0,1,2);m.bond(0,2,1);
        assert_eq!(bonded_atoms(&m,0),vec![0,1,2]);
        assert_eq!(bonded_atoms(&m,3),vec![3]);
        assert!(bonded_atoms(&m,20).is_empty());
    }
}
