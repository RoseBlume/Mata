use mata_3d::Vec3;
use std::collections::{BTreeMap,HashSet};

pub const ELEMENTS:&str="H He Li Be B C N O F Ne Na Mg Al Si P S Cl Ar K Ca Sc Ti V Cr Mn Fe Co Ni Cu Zn Ga Ge As Se Br Kr Rb Sr Y Zr Nb Mo Tc Ru Rh Pd Ag Cd In Sn Sb Te I Xe Cs Ba La Ce Pr Nd Pm Sm Eu Gd Tb Dy Ho Er Tm Yb Lu Hf Ta W Re Os Ir Pt Au Hg Tl Pb Bi Po At Rn Fr Ra Ac Th Pa U Np Pu Am Cm Bk Cf Es Fm Md No Lr Rf Db Sg Bh Hs Mt Ds Rg Cn Nh Fl Mc Lv Ts Og";

pub fn atomic_number(s:&str)->Option<usize>{ELEMENTS.split_whitespace().position(|e|e==s).map(|x|x+1)}
pub fn electrons(s:&str)->Option<i32>{Some(match s {
    "H"=>1,"He"=>2,"Li"|"Na"|"K"|"Rb"|"Cs"=>1,"Be"|"Mg"|"Ca"|"Sr"|"Ba"=>2,
    "B"|"Al"|"Ga"=>3,"C"|"Si"|"Ge"|"Sn"|"Pb"=>4,
    "N"|"P"|"As"|"Sb"|"Bi"=>5,"O"|"S"|"Se"|"Te"=>6,
    "F"|"Cl"|"Br"|"I"=>7,"Ne"|"Ar"|"Kr"|"Xe"=>8,_=>return None,
})}
fn preferred_valences(s:&str)->&'static [u8]{match s {
    "H"|"F"|"Cl"|"Br"|"I"=>&[1], "O"=>&[2],"S"|"Se"|"Te"=>&[2,4,6],
    "N"=>&[3],"P"|"As"=>&[3,5],"C"|"Si"|"Ge"=>&[4],"B"|"Al"=>&[3],_=>&[]
}}

/// Atom-site identity and crystallographic fields for PDB / mmCIF structures.
/// Atom serials are preserved for reading connectivity; writers renumber them
/// to satisfy each output format's record conventions.
#[derive(Clone, Debug, PartialEq)]
pub struct AtomSite {
    pub atom_name: String,
    pub residue_name: String,
    pub chain_id: String,
    pub residue_number: String,
    pub insertion_code: String,
    pub alternate_location: String,
    pub occupancy: f32,
    pub b_factor: f32,
    pub hetero: bool,
    pub serial: String,
}

#[derive(Clone,Debug,PartialEq)]
pub struct Atom { pub symbol:String, pub charge:i8, pub pos:Vec3, pub partial_charge:Option<f32>, pub mol2_type:Option<String>, pub site:Option<AtomSite> }
impl Atom {pub fn new(s:&str)->Self{Self{symbol:s.into(),charge:0,pos:Vec3::default(),partial_charge:None,mol2_type:None,site:None}}}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Bond {pub a:usize,pub b:usize, /// 1 single, 2 double, 3 triple, 4 aromatic.
    pub order:u8 }
#[derive(Clone,Debug)]
pub struct Molecule {
    pub name:String, pub atoms:Vec<Atom>,pub bonds:Vec<Bond>,pub from_file:bool,
    /// Preview-only indices of modeled atoms, never saved in source structures.
    pub estimated_atoms:Vec<usize>,
    /// Whether canonical amino-acid bond-order templates may supplement the imported data.
    pub apply_templates:bool
}
impl Molecule {
    pub fn new(name:&str)->Self{Self{name:name.into(),atoms:Vec::new(),bonds:Vec::new(),from_file:false,estimated_atoms:Vec::new(),apply_templates:true}}
    pub fn atom(&mut self,s:&str)->usize {let idx=self.atoms.len();self.atoms.push(Atom::new(s));idx}
    pub fn bond(&mut self,a:usize,b:usize,order:u8){self.bonds.push(Bond{a,b,order});}
    pub fn bond_order_sum(&self,i:usize)->i32 {self.bonds.iter().filter(|b|b.a==i||b.b==i).map(|b|if b.order==4 {1}else{b.order as i32}).sum()}
    pub fn degree(&self,i:usize)->usize{self.bonds.iter().filter(|b|b.a==i||b.b==i).count()}
    pub fn lone_pairs(&self,i:usize)->u8 {
        let a=&self.atoms[i];let Some(e)=electrons(&a.symbol) else{return 0};
        // Aromatic bonds share delocalized pi electrons. For the common neutral
        // sp2 C/N ring atoms an aromatic edge contributes one sigma electron.
        // File-based organic structures often omit hydrogen atoms. Use the
        // same explicitly flagged implicit-H estimate as the VSEPR report so
        // carbon centers do not gain spurious lone-pair dots.
        let used=self.bond_order_sum(i) + crate::vsepr::implicit_hydrogens(self,i) as i32;
        ((e-used-a.charge as i32).max(0)/2).min(5) as u8
    }
    pub fn total_charge(&self)->i32 {self.atoms.iter().map(|a|a.charge as i32).sum()}
    pub fn formula(&self)->String {
        let mut counts=BTreeMap::<String,usize>::new();for a in &self.atoms {*counts.entry(a.symbol.clone()).or_default()+=1;}
        let mut result=String::new();
        if counts.contains_key("C") {append_count(&mut result,"C",counts.remove("C").unwrap());if let Some(h)=counts.remove("H"){append_count(&mut result,"H",h);}}
        for (s,n) in counts {append_count(&mut result,&s,n);}
        if self.total_charge()!=0 {let q=self.total_charge();result.push('^');if q.abs()!=1{result.push_str(&q.abs().to_string());}result.push(if q>0 {'+'}else{'-'});}
        result
    }
    pub fn verify(&self)->Result<(),String>{
        if self.atoms.len()>100_000 {return Err("Maximum 100,000 atoms per structure".into());}
        if self.bonds.len()>400_000{return Err("Maximum 400,000 bonds per structure".into());}
        for (i,a) in self.atoms.iter().enumerate(){if atomic_number(&a.symbol).is_none(){return Err(format!("Unknown element {} at atom {}",a.symbol,i+1));}}
        let mut seen=HashSet::new();
        for b in &self.bonds {if b.a>=self.atoms.len()||b.b>=self.atoms.len()||b.a==b.b||!(1..=4).contains(&b.order) {return Err("Invalid bond endpoints or bond type".into());}if !seen.insert((b.a.min(b.b),b.a.max(b.b))){return Err("Duplicate bond".into());}}
        Ok(())
    }
}
fn append_count(s:&mut String,element:&str,n:usize){s.push_str(element);if n!=1{s.push_str(&n.to_string());}}

#[derive(Debug,Clone)]
pub struct Formula { pub counts:Vec<(String,usize)>,pub charge:i32 }
impl Formula {pub fn get(&self,s:&str)->usize{self.counts.iter().find(|(e,_)|e==s).map(|x|x.1).unwrap_or(0)} pub fn total(&self)->usize{self.counts.iter().map(|x|x.1).sum()} }

pub fn parse_formula(text:&str)->Result<Formula,String>{
    let mut s=text.trim().replace(' ', "");
    if s.is_empty(){return Err("Enter a chemical formula (for example C2H6O)".into());}
    let mut charge=0;
    if s.ends_with('+')||s.ends_with('-') {
        let sign=if s.ends_with('+'){1}else{-1};s.pop();
        if let Some(p)=s.rfind('^') {let digits=s[p+1..].to_string();if digits.is_empty()||digits.chars().all(|c|c.is_ascii_digit()) {
            charge=sign*if digits.is_empty(){1}else{digits.parse::<i32>().map_err(|_|"Invalid charge")?};s.truncate(p);
        }else{return Err("Invalid charge suffix".into());}}
        else {charge=sign;}
    }
    // A single wrapping bracket also permits common [NH4]+ notation.
    if s.starts_with('[')&&s.ends_with(']') {s=s[1..s.len()-1].to_string();}
    let chars:Vec<char>=s.chars().collect();let mut index=0;
    fn read_count(c:&[char],pos:&mut usize)->Result<usize,String>{let start=*pos;while *pos<c.len()&&c[*pos].is_ascii_digit(){*pos+=1;}if *pos==start{return Ok(1)}let n:String=c[start..*pos].iter().collect();let value=n.parse::<usize>().map_err(|_|"Invalid atom count")?;if value==0||value>512{return Err("Counts must be between 1 and 512".into());}Ok(value)}
    fn group(c:&[char],pos:&mut usize,nesting:usize)->Result<BTreeMap<String,usize>,String>{
        if nesting>6{return Err("Formula nesting too deep".into());}
        let mut counts=BTreeMap::new();
        while *pos<c.len(){let ch=c[*pos];
            if ch==')'{if nesting==0{return Err("Unexpected ')'".into());}*pos+=1;return Ok(counts);}
            if ch=='(' { *pos+=1;let inner=group(c,pos,nesting+1)?;let n=read_count(c,pos)?;for (s,v) in inner {let entry=counts.entry(s).or_insert(0usize);*entry=entry.checked_add(v.checked_mul(n).ok_or("Too many atoms")?).ok_or("Too many atoms")?;} }
            else if ch.is_ascii_uppercase(){let mut symbol=String::new();symbol.push(ch);*pos+=1;if *pos<c.len()&&c[*pos].is_ascii_lowercase(){symbol.push(c[*pos]);*pos+=1;}if atomic_number(&symbol).is_none(){return Err(format!("Unknown element {symbol}"));}let n=read_count(c,pos)?;*counts.entry(symbol).or_insert(0usize)+=n;}
            else{return Err(format!("Unexpected symbol '{}'",ch));}
        }
        if nesting>0 {return Err("Unclosed '('".into());}
        Ok(counts)
    }
    let map=group(&chars,&mut index,0)?;
    if index!=chars.len(){return Err("Unexpected trailing text".into());}
    let counts:Vec<_>=map.into_iter().collect();
    if counts.iter().map(|x|x.1).sum::<usize>()>512{return Err("Maximum 512 atoms".into());}
    if counts.is_empty(){return Err("No atoms specified".into());}
    if charge.abs()>8{return Err("Charge magnitude is too large".into());}
    Ok(Formula{counts,charge})
}

fn template(f:&Formula)->Vec<Molecule>{
    let matches=|elements:&[(&str,usize)],q:i32|f.charge==q&&f.counts.len()==elements.len()&&elements.iter().all(|(s,n)|f.get(s)==*n);
    let mut result=Vec::new();
    // Small ionic and resonance systems cannot safely be inferred from neutral valences.
    if f.total()==1 {
        let mut m=Molecule::new(&format!("{} atom",f.counts[0].0));
        let id=m.atom(&f.counts[0].0);m.atoms[id].charge=f.charge as i8;result.push(m);
    }
    if matches(&[("H",3),("O",1)],1){let mut m=Molecule::new("Hydronium ion");let o=m.atom("O");m.atoms[o].charge=1;for _ in 0..3 {let h=m.atom("H");m.bond(o,h,1);}result.push(m);}
    if matches(&[("H",1),("O",1)],-1){let mut m=Molecule::new("Hydroxide ion");let o=m.atom("O");m.atoms[o].charge=-1;let h=m.atom("H");m.bond(o,h,1);result.push(m);}
    if matches(&[("C",1),("N",1)],-1){let mut m=Molecule::new("Cyanide ion");let c=m.atom("C");let n=m.atom("N");m.atoms[c].charge=-1;m.bond(c,n,3);result.push(m);}
    if matches(&[("N",1),("O",2)],-1){for double in 0..2 {let mut m=Molecule::new(&format!("Nitrite resonance {}",double+1));let n=m.atom("N");for i in 0..2{let o=m.atom("O");if i!=double{m.atoms[o].charge=-1;}m.bond(n,o,if i==double{2}else{1});}result.push(m);}}
    if matches(&[("C",1),("O",3)],-2){for double in 0..3 {let mut m=Molecule::new(&format!("Carbonate resonance {}",double+1));let c=m.atom("C");for i in 0..3{let o=m.atom("O");if i!=double{m.atoms[o].charge=-1;}m.bond(c,o,if i==double{2}else{1});}result.push(m);}}
    if matches(&[("O",4),("S",1)],-2){for d1 in 0..4 {for d2 in d1+1..4 {let mut m=Molecule::new(&format!("Sulfate resonance {}-{}",d1+1,d2+1));let s=m.atom("S");for i in 0..4{let o=m.atom("O");let double=i==d1||i==d2;if !double{m.atoms[o].charge=-1;}m.bond(s,o,if double{2}else{1});}result.push(m);}}}
    if matches(&[("O",4),("P",1)],-3){for double in 0..4 {let mut m=Molecule::new(&format!("Phosphate resonance {}",double+1));let p=m.atom("P");for i in 0..4{let o=m.atom("O");if i!=double{m.atoms[o].charge=-1;}m.bond(p,o,if i==double{2}else{1});}result.push(m);}}
    if matches(&[("C",2),("H",3),("O",2)],-1){for double in 0..2 {let mut m=Molecule::new(&format!("Acetate resonance {}",double+1));let methyl=m.atom("C");let carbonyl=m.atom("C");m.bond(methyl,carbonyl,1);for _ in 0..3{let h=m.atom("H");m.bond(methyl,h,1);}for i in 0..2{let o=m.atom("O");if i!=double{m.atoms[o].charge=-1;}m.bond(carbonyl,o,if i==double{2}else{1});}result.push(m);}}
    if matches(&[("H",4),("N",1)],1){let mut m=Molecule::new("Ammonium ion");let n=m.atom("N");m.atoms[n].charge=1;for _ in 0..4 {let h=m.atom("H");m.bond(n,h,1);}result.push(m);}
    if matches(&[("N",1),("O",3)],-1){for double in 0..3 {let mut m=Molecule::new(&format!("Nitrate resonance {}",double+1));let n=m.atom("N");m.atoms[n].charge=1;for j in 0..3 {let o=m.atom("O");if j!=double{m.atoms[o].charge=-1;}m.bond(n,o,if j==double{2}else{1});}result.push(m);}}
    if matches(&[("O",3)],0){for shift in 0..2 {let mut m=Molecule::new(&format!("Ozone resonance {}",shift+1));let a=m.atom("O");let b=m.atom("O");let c=m.atom("O");m.atoms[b].charge=1;m.atoms[if shift==0 {a}else{c}].charge=-1;m.bond(a,b,if shift==0{1}else{2});m.bond(b,c,if shift==0{2}else{1});result.push(m);}}
    if matches(&[("C",1),("O",1)],0){let mut m=Molecule::new("Carbon monoxide");let c=m.atom("C");let o=m.atom("O");m.bond(c,o,3);m.atoms[c].charge=-1;m.atoms[o].charge=1;result.push(m);}
    if matches(&[("C",6),("H",6)],0){let mut m=Molecule::new("Benzene (aromatic)");let mut c=Vec::new();for _ in 0..6{c.push(m.atom("C"));}for i in 0..6{m.bond(c[i],c[(i+1)%6],4);}for i in 0..6{let h=m.atom("H");m.bond(c[i],h,1);}result.push(m);}
    if matches(&[("C",5),("H",5),("N",1)],0){let mut m=Molecule::new("Pyridine (aromatic)");for s in ["N","C","C","C","C","C"] {m.atom(s);}for i in 0..6 {m.bond(i,(i+1)%6,4);}for i in 1..6{let h=m.atom("H");m.bond(i,h,1);}result.push(m);}
    if matches(&[("C",2),("H",6),("O",1)],0){
        let mut ethanol=Molecule::new("Ethanol");let c1=ethanol.atom("C");let c2=ethanol.atom("C");let o=ethanol.atom("O");
        ethanol.bond(c1,c2,1);ethanol.bond(c2,o,1);
        for (center,n) in [(c1,3),(c2,2),(o,1)]{for _ in 0..n{let h=ethanol.atom("H");ethanol.bond(center,h,1);}}
        result.push(ethanol);
        let mut ether=Molecule::new("Dimethyl ether");let c1=ether.atom("C");let o=ether.atom("O");let c2=ether.atom("C");
        ether.bond(c1,o,1);ether.bond(o,c2,1);
        for center in [c1,c2]{for _ in 0..3{let h=ether.atom("H");ether.bond(center,h,1);}}
        result.push(ether);
    }
    if matches(&[("C",2),("H",4),("O",2)],0){
        let mut acid=Molecule::new("Acetic acid");let methyl=acid.atom("C");let carboxyl=acid.atom("C");let carbonyl=acid.atom("O");let hydroxyl=acid.atom("O");
        acid.bond(methyl,carboxyl,1);acid.bond(carboxyl,carbonyl,2);acid.bond(carboxyl,hydroxyl,1);
        for _ in 0..3{let h=acid.atom("H");acid.bond(methyl,h,1);}let h=acid.atom("H");acid.bond(hydroxyl,h,1);result.push(acid);
        let mut ester=Molecule::new("Methyl formate");let formyl=ester.atom("C");let oxo=ester.atom("O");let ester_o=ester.atom("O");let methyl=ester.atom("C");
        ester.bond(formyl,oxo,2);ester.bond(formyl,ester_o,1);ester.bond(ester_o,methyl,1);
        let h=ester.atom("H");ester.bond(formyl,h,1);
        for _ in 0..3{let h=ester.atom("H");ester.bond(methyl,h,1);}result.push(ester);
    }
    if matches(&[("H",2),("O",2)],0){let mut m=Molecule::new("Hydrogen peroxide");let o=m.atom("O");let o2=m.atom("O");m.bond(o,o2,1);let h=m.atom("H");let h2=m.atom("H");m.bond(o,h,1);m.bond(o2,h2,1);result.push(m);}
    if matches(&[("H",2),("S",1)],0){let mut m=Molecule::new("Hydrogen sulfide");let s=m.atom("S");for _ in 0..2{let h=m.atom("H");m.bond(s,h,1);}result.push(m);}
    if matches(&[("O",2),("S",1)],0){let mut m=Molecule::new("Sulfur dioxide (expanded octet)");let s=m.atom("S");for _ in 0..2{let o=m.atom("O");m.bond(s,o,2);}result.push(m);}
    if matches(&[("O",3),("S",1)],0){let mut m=Molecule::new("Sulfur trioxide (expanded octet)");let s=m.atom("S");for _ in 0..3{let o=m.atom("O");m.bond(s,o,2);}result.push(m);}
    if matches(&[("F",6),("S",1)],0){let mut m=Molecule::new("Sulfur hexafluoride");let s=m.atom("S");for _ in 0..6{let o=m.atom("F");m.bond(s,o,1);}result.push(m);}
    if matches(&[("Cl",5),("P",1)],0){let mut m=Molecule::new("Phosphorus pentachloride");let p=m.atom("P");for _ in 0..5{let cl=m.atom("Cl");m.bond(p,cl,1);}result.push(m);}
    result
}

fn graph_key(m:&Molecule)->String {
    // Exact graph canonicalization by signatures + bounded permutations of
    // indistinguishable nodes (small generated graphs only).
    let n=m.atoms.len();if n>20{return format!("{:?}{:?}",m.atoms,m.bonds);}
    let mut node_colors:Vec<String>=m.atoms.iter().map(|a|a.symbol.clone()).collect();
    for _ in 0..n {let next=(0..n).map(|i|{
        let mut edges=m.bonds.iter().filter_map(|b|if b.a==i{Some((b.order,node_colors[b.b].clone()))}else if b.b==i{Some((b.order,node_colors[b.a].clone()))}else{None}).collect::<Vec<_>>();edges.sort();format!("{}{:?}",m.atoms[i].symbol,edges)
    }).collect::<Vec<_>>();
        let mut ranked=next.clone();ranked.sort();ranked.dedup();let colors=next.iter().map(|s|ranked.binary_search(s).unwrap().to_string()).collect::<Vec<_>>();if colors==node_colors{break;}node_colors=colors;
    }
    let mut edges:Vec<_>=m.bonds.iter().map(|b|{let p=&node_colors[b.a];let q=&node_colors[b.b];format!("{}-{}:{}",p.min(q),p.max(q),b.order)}).collect();edges.sort();format!("{:?}|{}",node_colors.iter().fold(BTreeMap::<String,usize>::new(),|mut a,s|{*a.entry(s.clone()).or_default()+=1;a}),edges.join(","))
}

pub fn propose(formula:&str)->Result<Vec<Molecule>,String>{
    let f=parse_formula(formula)?;if f.total()>80{return Err("Formula too large for automatic assembly (up to 80 atoms). Import a MOL/SDF/MOL2 structure instead.".into());}
    let mut results=template(&f);
    // For neutral, commonplace covalent systems, enumerate bounded heavy-atom
    // trees with bond orders, and complete open valences with hydrogens.
    if f.charge==0&&f.counts.iter().all(|(s,_)|!preferred_valences(s).is_empty()) {
        let mut heavy=Vec::<String>::new();let hydrogens=f.get("H");
        for (s,count) in &f.counts {if s!="H"{for _ in 0..*count{heavy.push(s.clone());}}}
        if heavy.is_empty(){if hydrogens==2 {let mut m=Molecule::new("Dihydrogen");m.atom("H");m.atom("H");m.bond(0,1,1);results.push(m);} }
        else if heavy.len()<=5 {let mut seen=HashSet::new();for m in &results{seen.insert(graph_key(m));}
            // Build all connected attachment trees across all next-element choices.
            let mut budget=0;
            fn walk(cur:&Molecule,remaining:&mut Vec<String>,n_h:usize,results:&mut Vec<Molecule>,seen:&mut HashSet<String>,budget:&mut usize){
                if *budget>100_000||results.len()>=20{return;}
                *budget+=1;
                if remaining.is_empty(){
                    let mut caps=Vec::new();for i in 0..cur.atoms.len(){let b=cur.bond_order_sum(i) as u8;
                        let Some(v)=preferred_valences(&cur.atoms[i].symbol).iter().copied().find(|v|*v>=b&&(*v-b) as usize<=n_h) else{return;};caps.push((v-b) as usize);
                    }
                    if caps.iter().sum::<usize>()!=n_h{return;}
                    let mut m=cur.clone();for (i,count) in caps.into_iter().enumerate(){for _ in 0..count{let h=m.atom("H");m.bond(i,h,1);}}
                    let key=graph_key(&m);if seen.insert(key){m.name=format!("Connectivity variant {}",results.len()+1);results.push(m);}return;
                }
                let mut elements=HashSet::new();for ix in 0..remaining.len(){if !elements.insert(remaining[ix].clone()){continue;}
                    let symbol=remaining.remove(ix);
                    if cur.atoms.is_empty(){let mut next=cur.clone();next.atom(&symbol);walk(&next,remaining,n_h,results,seen,budget);}
                    else {for p in 0..cur.atoms.len(){let capacity=preferred_valences(&cur.atoms[p].symbol).last().copied().unwrap_or(0) as i32-cur.bond_order_sum(p);
                        for order in 1..=3 {if capacity<order || !preferred_valences(&symbol).iter().any(|v|*v>=order as u8){continue;}
                            let mut next=cur.clone();let j=next.atom(&symbol);next.bond(p,j,order as u8);walk(&next,remaining,n_h,results,seen,budget);
                        }
                    }}
                    remaining.insert(ix,symbol);if *budget>100_000||results.len()>=20{return;}
                }
            }
            let mut base=Molecule::new("");walk(&base,&mut heavy,hydrogens,&mut results,&mut seen,&mut budget);
        }
    }
    if results.is_empty(){return Err(format!("No confident covalent assembly for {formula}. Supply a MOL, SDF, or MOL2 structure for unusual valence/ionic/complex molecules."));}
    for m in &mut results {if m.atoms.iter().all(|a|a.pos==Vec3::default()){layout3d(m);}}
    Ok(results)
}

/// A simple local VSEPR-based coordinate generator, not a force-field optimizer.
/// Imported coordinates are never replaced.
pub fn layout3d(m:&mut Molecule){
    if m.atoms.is_empty(){return;}
    let heavy:Vec<usize>=(0..m.atoms.len()).filter(|&i|m.atoms[i].symbol!="H").collect();
    let aromatic=m.bonds.iter().any(|b|b.order==4);
    if aromatic&&heavy.len()==6&&m.bonds.iter().filter(|b|b.order==4).count()==6 {
        for (i,&id) in heavy.iter().enumerate(){let t=std::f32::consts::TAU*i as f32/6.0;m.atoms[id].pos=Vec3::new(t.cos()*1.40,t.sin()*1.40,0.0);}
        for b in m.bonds.clone(){if m.atoms[b.a].symbol=="H"||m.atoms[b.b].symbol=="H" {let (h,c)=if m.atoms[b.a].symbol=="H"{(b.a,b.b)}else{(b.b,b.a)};m.atoms[h].pos=m.atoms[c].pos*1.77;}}
        return;
    }
    let root=heavy.first().copied().unwrap_or(0);
    let mut visited=vec![false;m.atoms.len()];visited[root]=true;m.atoms[root].pos=Vec3::default();let mut queue=std::collections::VecDeque::new();queue.push_back(root);
    while let Some(center)=queue.pop_front(){
        let parent=m.bonds.iter().filter_map(|b|if b.a==center&&visited[b.b]{Some(b.b)}else if b.b==center&&visited[b.a]{Some(b.a)}else{None}).next();
        let mut children:Vec<_>=m.bonds.iter().filter_map(|b|if b.a==center&&!visited[b.b]{Some(b.b)}else if b.b==center&&!visited[b.a]{Some(b.a)}else{None}).collect();
        children.sort_by_key(|&x|m.atoms[x].symbol=="H");
        let dirs=steric_vectors(m,center,children.len(),parent.is_some());
        let fallback=Vec3::new(0.0,0.0,1.0);
        let back=parent.map(|p|(m.atoms[p].pos-m.atoms[center].pos).normalized()).unwrap_or(fallback);
        let axis=if back.y.abs()<0.9{Vec3::new(0.0,1.0,0.0)}else{Vec3::new(1.0,0.0,0.0)};
        let r=axis.cross(back).normalized();let u=back.cross(r).normalized();
        for (k,ch) in children.into_iter().enumerate(){let d=dirs.get(k).copied().unwrap_or(Vec3::new(0.0,0.0,-1.0));
            let direction=(r*d.x+u*d.y+back*d.z).normalized();
            let bond=m.bonds.iter().find(|b|(b.a==center&&b.b==ch)||(b.b==center&&b.a==ch)).map(|b|b.order).unwrap_or(1);
            let length=if m.atoms[ch].symbol=="H"{1.0}else if bond>=3{1.2}else if bond==2{1.30}else{1.50};
            m.atoms[ch].pos=m.atoms[center].pos+direction*length;
            visited[ch]=true;queue.push_back(ch);
        }
    }
    // basic recentering improves rotation around molecule centroid
    let centroid=m.atoms.iter().fold(Vec3::default(),|v,a|v+a.pos)/m.atoms.len() as f32;
    for a in &mut m.atoms {a.pos=a.pos-centroid;}
}
fn steric_vectors(m:&Molecule,center:usize,n:usize,has_parent:bool)->Vec<Vec3>{
    if !has_parent && m.atoms[center].symbol=="O" && m.degree(center)==2 && m.lone_pairs(center)==2 {
        // Approximately 104.5 degrees for the water molecular shape.
        let h=52.25_f32.to_radians();
        return vec![Vec3::new(h.sin(),0.0,h.cos()),Vec3::new(-h.sin(),0.0,h.cos())].into_iter().take(n).collect();
    }
    if !has_parent && m.atoms[center].symbol=="N" && m.degree(center)==3 && m.lone_pairs(center)==1 {
        // Three pyramidal N-H bonds at ~107 degrees, lone pair opposite.
        let axial=0.372_f32;let radial=(1.0-axial*axial).sqrt();
        return (0..n).map(|i|{let t=std::f32::consts::TAU*i as f32/3.0;Vec3::new(radial*t.cos(),radial*t.sin(),axial)}).collect();
    }
    let steric=(m.degree(center)+m.lone_pairs(center) as usize).min(6);
    let all:Vec<Vec3>=match steric {
        0|1=>vec![Vec3::new(0.0,0.0,-1.0)],
        2=>vec![Vec3::new(0.0,0.0,1.0),Vec3::new(0.0,0.0,-1.0)],
        3=> (0..3).map(|i|{let t=std::f32::consts::TAU*i as f32/3.0;Vec3::new(t.sin(),0.0,t.cos())}).collect(),
        4=>vec![Vec3::new(1.0,1.0,1.0).normalized(),Vec3::new(-1.0,-1.0,1.0).normalized(),Vec3::new(-1.0,1.0,-1.0).normalized(),Vec3::new(1.0,-1.0,-1.0).normalized()],
        5=>vec![Vec3::new(0.0,0.0,1.0),Vec3::new(0.0,0.0,-1.0),Vec3::new(1.0,0.0,0.0),Vec3::new(-0.5,0.866,0.0),Vec3::new(-0.5,-0.866,0.0)],
        _=>vec![Vec3::new(0.0,0.0,1.0),Vec3::new(0.0,0.0,-1.0),Vec3::new(1.0,0.0,0.0),Vec3::new(-1.0,0.0,0.0),Vec3::new(0.0,1.0,0.0),Vec3::new(0.0,-1.0,0.0)]
    };
    let skip=if has_parent{1}else{0};
    // Put lone pairs at the end of the local geometry slots. For lone-pair
    // centers with a parent, the first slot aligns with the parent bond.
    all.into_iter().skip(skip).take(n).collect()
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn parsed(){assert_eq!(parse_formula("Al2(SO4)3").unwrap().get("O"),12);assert_eq!(parse_formula("NO3^-").unwrap().charge,-1);assert!(parse_formula("Xx5").is_err());}
    #[test] fn ethanol_ether(){let v=propose("C2H6O").unwrap();assert!(v.len()>=2,"{:?}",v.iter().map(|m|&m.name).collect::<Vec<_>>());}
    #[test] fn common(){for f in ["H2O","CO2","NH3","CH4","C2H2","C2H4","C6H6","NO3^-"]{let v=propose(f).unwrap();assert!(!v.is_empty(),"{f}");for m in v{assert_eq!(m.formula(),f.replace("^-","^-"));m.verify().unwrap();}}}
    #[test] fn lone_pairs(){let w=propose("H2O").unwrap();assert_eq!(w[0].lone_pairs(0),2);}
}
