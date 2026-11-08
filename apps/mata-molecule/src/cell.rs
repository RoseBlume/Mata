//! Cell models and bounded microscopy volume import. Only std + existing Mata crates.
//! Procedural organelles are educational illustrations, NOT segmentations of data.
use std::{fs,path::Path};
use mata_3d::Vec3;

const MAX_VOXELS: usize = 64_000_000;
const MAX_FILE_BYTES: u64 = 384 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrganelleKind { Membrane, Nucleus, Nucleolus, Mitochondrion, EndoplasmicReticulum, Golgi, Lysosome, Vesicle, Chloroplast, Vacuole, CellWall }
impl OrganelleKind {
    pub fn label(self)->&'static str { match self {
        Self::Membrane=>"Cell membrane",Self::Nucleus=>"Nucleus",Self::Nucleolus=>"Nucleolus",
        Self::Mitochondrion=>"Mitochondrion",Self::EndoplasmicReticulum=>"Endoplasmic reticulum",
        Self::Golgi=>"Golgi apparatus",Self::Lysosome=>"Lysosome",Self::Vesicle=>"Vesicle",
        Self::Chloroplast=>"Chloroplast",Self::Vacuole=>"Central vacuole",Self::CellWall=>"Cell wall"} }
    pub fn function(self)->&'static str { match self {
        Self::Membrane=>"Lipid bilayer; regulates transport and cell signaling",
        Self::Nucleus=>"Contains chromosomes and supports transcription",
        Self::Nucleolus=>"Assembles ribosomal subunits",
        Self::Mitochondrion=>"Produces ATP through oxidative phosphorylation",
        Self::EndoplasmicReticulum=>"Protein and lipid synthesis, folding and transport",
        Self::Golgi=>"Modifies, sorts and packages cellular cargo",
        Self::Lysosome=>"Acidic recycling and breakdown compartment",
        Self::Vesicle=>"Intracellular transport and storage",
        Self::Chloroplast=>"Photosynthesis in plant and algal cells",
        Self::Vacuole=>"Storage and turgor support in plant cells",
        Self::CellWall=>"Structural support outside the plasma membrane"} }
    pub fn color(self)->u32 {match self {
        Self::Membrane=>0xff51b7e6,Self::Nucleus=>0xffa682f7,Self::Nucleolus=>0xffffd48c,
        Self::Mitochondrion=>0xffff9468,Self::EndoplasmicReticulum=>0xff62dac0,
        Self::Golgi=>0xffffbe6f,Self::Lysosome=>0xffed779d,Self::Vesicle=>0xffaccdea,
        Self::Chloroplast=>0xff72ce79,Self::Vacuole=>0xff75c6e9,Self::CellWall=>0xff9cbe87} }
}
#[derive(Clone,Debug)]
pub struct Organelle {
    pub name:String, pub kind:OrganelleKind, pub center:Vec3, pub radii:Vec3,
    pub visible:bool, pub estimated:bool,
}
#[derive(Clone,Debug)]
pub struct GeneratedCell {pub name:String,pub size_um:Vec3,pub organelles:Vec<Organelle>}
impl GeneratedCell {
    pub fn animal()->Self {
        let mut organelles=vec![
            org("Plasma membrane",OrganelleKind::Membrane,[0.,0.,0.],[9.,7.2,6.6]),
            org("Nucleus",OrganelleKind::Nucleus,[-1.6,0.6,0.2],[2.5,2.35,2.0]),
            org("Nucleolus",OrganelleKind::Nucleolus,[-1.3,0.9,0.4],[0.8,0.72,0.66]),
            org("Rough ER",OrganelleKind::EndoplasmicReticulum,[-3.65,-1.3,0.2],[1.3,2.1,0.8]),
            org("Smooth ER",OrganelleKind::EndoplasmicReticulum,[-1.5,-2.9,0.6],[2.2,0.9,0.7]),
            org("Golgi apparatus",OrganelleKind::Golgi,[2.3,1.2,0.0],[1.8,0.85,0.72]),
        ];
        for (i,p) in [[4.8,-1.6,-1.4],[4.3,2.8,0.6],[1.3,-3.1,-1.4],[-5.,2.3,1.6],[-0.2,3.7,1.0]].into_iter().enumerate() {
            organelles.push(org(&format!("Mitochondrion {}",i+1),OrganelleKind::Mitochondrion,p,[1.2,0.52,0.62]));
        }
        for (i,p) in [[3.2,-3.2,1.5],[-4.6,-2.3,-0.5],[2.7,3.9,-1.3]].into_iter().enumerate() {
            organelles.push(org(&format!("Lysosome {}",i+1),OrganelleKind::Lysosome,p,[0.46,0.46,0.46]));
        }
        for (i,p) in [[0.8,4.1,-1.2],[5.3,0.5,1.9],[-4.0,3.1,-0.9],[2.3,-1.3,2.3]].into_iter().enumerate() {
            organelles.push(org(&format!("Transport vesicle {}",i+1),OrganelleKind::Vesicle,p,[0.29,0.29,0.29]));
        }
        Self{name:"Illustrative animal cell".into(),size_um:Vec3::new(18.0,14.4,13.2),organelles}
    }
    pub fn plant()->Self {
        let mut organelles=vec![
            org("Cell wall",OrganelleKind::CellWall,[0.,0.,0.],[10.5,8.3,7.2]),
            org("Plasma membrane",OrganelleKind::Membrane,[0.,0.,0.],[10.0,7.8,6.8]),
            org("Central vacuole",OrganelleKind::Vacuole,[0.9,0.0,0.0],[5.3,4.8,4.0]),
            org("Nucleus",OrganelleKind::Nucleus,[-6.7,1.6,0.0],[1.8,1.8,1.5]),
            org("Nucleolus",OrganelleKind::Nucleolus,[-6.5,1.7,0.25],[0.55,0.5,0.48]),
            org("Golgi apparatus",OrganelleKind::Golgi,[-5.0,-2.3,2.2],[1.1,0.6,0.7]),
            org("Endoplasmic reticulum",OrganelleKind::EndoplasmicReticulum,[-6.5,-1.2,-1.8],[1.5,1.3,0.65]),
        ];
        for (i,p) in [[-4.2,4.9,-2.1],[5.8,4.0,2.0],[6.7,-3.3,-1.0],[-3.9,-4.5,1.5],[2.1,5.1,-2.1]].into_iter().enumerate() {
            organelles.push(org(&format!("Chloroplast {}",i+1),OrganelleKind::Chloroplast,p,[1.35,0.75,0.65]));
        }
        for (i,p) in [[-6.8,-3.8,-0.6],[6.6,0.9,-2.0]].into_iter().enumerate() {
            organelles.push(org(&format!("Mitochondrion {}",i+1),OrganelleKind::Mitochondrion,p,[0.9,0.52,0.55]));
        }
        Self{name:"Illustrative plant cell".into(),size_um:Vec3::new(21.0,16.6,14.4),organelles}
    }
}
fn org(name:&str,kind:OrganelleKind,center:[f32;3],radii:[f32;3])->Organelle {
    Organelle{name:name.into(),kind,center:Vec3::new(center[0],center[1],center[2]),radii:Vec3::new(radii[0],radii[1],radii[2]),visible:true,estimated:true}
}

#[derive(Clone,Debug)]
pub struct Volume {
    pub name:String,pub dims:[usize;3],pub spacing_um:[f32;3],
    pub calibrated:bool,pub samples:Vec<f32>,pub source_format:String,
    range:(f32,f32),
}
impl Volume {
    pub fn value(&self,x:usize,y:usize,z:usize)->f32 {
        self.samples[z*self.dims[0]*self.dims[1]+y*self.dims[0]+x]
    }
    pub fn dimensions_um(&self)->[f32;3] {[
        self.dims[0] as f32*self.spacing_um[0],self.dims[1] as f32*self.spacing_um[1],
        self.dims[2] as f32*self.spacing_um[2]]}
    /// Construct an in-memory volume, e.g. for synthetic educational datasets.
    /// Source calibration is unknown unless supplied by a file importer.
    pub fn from_voxels(dims:[usize;3],samples:Vec<f32>)->Result<Self,String> {
        if checked_voxels(dims)? != samples.len() {
            return Err("Voxel payload length does not match dimensions".into());
        }
        if samples.iter().any(|x|!x.is_finite()) {return Err("Non-finite voxel values".into());}
        let range=values_range(&samples);
        Ok(Self{name:"In-memory volume".into(),dims,spacing_um:[1.;3],calibrated:false,samples,source_format:"Generated".into(),range})
    }
    pub fn min_max(&self)->(f32,f32) {self.range}
    /// Suggested isodensity contour from a bounded sample of the actual voxels.
    /// It is a visual starting point, NOT an experimentally validated contour.
    /// Return normalized position in the data range (0..1).
    pub fn suggested_contour(&self) -> f32 {
        if self.samples.is_empty() { return 0.5; }
        let (lo, hi) = self.range;
        if (hi-lo).abs() < 1e-12 { return 0.5; }
        let stride = (self.samples.len() / 32768).max(1);
        let mut sample: Vec<f32> = self.samples.iter().step_by(stride)
            .copied().filter(|x| x.is_finite()).collect();
        if sample.is_empty() { return 0.5; }
        sample.sort_unstable_by(f32::total_cmp);
        let q = |p:f32| -> f32 {
            sample[(((sample.len()-1) as f32 * p) as usize).min(sample.len()-1)]
        };
        let median = q(0.5);
        let high_sample = q(0.99);
        let low_sample = q(0.90);
        // Binary masks generally have two flat levels; split those levels.
        let value = if (low_sample-high_sample).abs() < (hi-lo)*0.0001 {
            (median + high_sample.max(q(0.999)))*0.5
        } else {
            (low_sample+high_sample)*0.5
        };
        ((value-lo)/(hi-lo)).clamp(0.001,0.999)
    }
}
fn values_range(samples:&[f32])->(f32,f32) {
    let (mut lo,mut hi)=(f32::INFINITY,f32::NEG_INFINITY);
    for &v in samples {if v.is_finite(){lo=lo.min(v);hi=hi.max(v);}}
    if lo.is_finite(){(lo,hi)}else{(0.0,1.0)}
}
#[derive(Clone,Debug)]
pub enum CellData {Generated(GeneratedCell),Microscopy(Volume)}
impl CellData {pub fn name(&self)->&str {match self {Self::Generated(c)=>&c.name,Self::Microscopy(v)=>&v.name}}}

fn checked_voxels(dims:[usize;3])->Result<usize,String> {
    if dims.iter().any(|&d|d==0||d>65_536) {return Err("Invalid volume dimensions".into());}
    let n=dims.into_iter().try_fold(1usize,|a,b|a.checked_mul(b))
        .ok_or("Volume dimension overflow")?;
    if n>MAX_VOXELS {return Err(format!("Volume has {n} voxels; limit is {MAX_VOXELS} (downsample first)"));}
    Ok(n)
}
fn read_u32(data:&[u8],off:usize,big:bool)->Result<u32,String> {
    let slice=data.get(off..off+4).ok_or("Truncated header")?;
    let bytes:[u8;4]=slice.try_into().map_err(|_|"Truncated integer")?;
    Ok(if big {u32::from_be_bytes(bytes)} else {u32::from_le_bytes(bytes)})
}
fn read_f32(data:&[u8],off:usize,big:bool)->Result<f32,String> {
    Ok(f32::from_bits(read_u32(data,off,big)?))
}
fn parse_mrc(data:&[u8],name:String)->Result<Volume,String> {
    if data.len()<1024{return Err("MRC header is shorter than 1024 bytes".into());}
    let be=if &data[208..212]==b"MAP " {
        // Validate dimensions in both byte orders rather than relying on optional machine stamp.
        let le=[read_u32(data,0,false)?,read_u32(data,4,false)?,read_u32(data,8,false)?];
        if le.iter().all(|&v|v>0&&v<65_536){false}else{true}
    } else {return Err("Not an MRC/CCP4 MAP file".into())};
    let dims=[read_u32(data,0,be)? as usize,read_u32(data,4,be)? as usize,read_u32(data,8,be)? as usize];
    let n=checked_voxels(dims)?;
    let mode=read_u32(data,12,be)?;
    let bpp=match mode {0=>1usize,1|6=>2,2=>4,_=>return Err(format!("Unsupported MRC mode {mode}; supports 0, 1, 2 and 6"))};
    let nsymbt=read_u32(data,92,be)? as usize;
    let start=1024usize.checked_add(nsymbt).ok_or("MRC extended header overflow")?;
    let needed=n.checked_mul(bpp).and_then(|v|v.checked_add(start)).ok_or("MRC size overflow")?;
    if data.len()<needed{return Err("Truncated MRC voxel data".into());}
    let mx=read_u32(data,28,be)? as usize;let my=read_u32(data,32,be)? as usize;let mz=read_u32(data,36,be)? as usize;
    let extent=[read_f32(data,40,be)?,read_f32(data,44,be)?,read_f32(data,48,be)?];
    let mut spacing_um=[1.0;3];let mut calibrated=true;
    for i in 0..3 {let d=[mx,my,mz][i];if d>0 && extent[i].is_finite() && extent[i]>0.0 {spacing_um[i]=extent[i]/d as f32 * 1e-4;} else {calibrated=false;}}
    // MRC stores NX as fastest axis. Axis mapping in mapc/mapr/maps is preserved as
    // stored; reorienting crystallographic axes is outside this viewer's scope.
    let vox=&data[start..needed];let mut samples=Vec::with_capacity(n);
    for p in vox.chunks_exact(bpp){let v=match mode {
        0=>p[0] as i8 as f32,
        1=>{let a=[p[0],p[1]];(if be {i16::from_be_bytes(a)}else{i16::from_le_bytes(a)}) as f32},
        6=>{let a=[p[0],p[1]];(if be {u16::from_be_bytes(a)}else{u16::from_le_bytes(a)}) as f32},
        2=>{let a=[p[0],p[1],p[2],p[3]];f32::from_bits(if be{u32::from_be_bytes(a)}else{u32::from_le_bytes(a)})},
        _=>unreachable!()};samples.push(if v.is_finite(){v}else{0.0});}
    let range=values_range(&samples);
    Ok(Volume{name,dims,spacing_um,calibrated,samples,source_format:"MRC/CCP4".into(),range})
}
fn find_header_end(data:&[u8])->Option<(usize,usize)> {
    data.windows(2).position(|w|w==b"\n\n").map(|i|(i,i+2))
        .or_else(||data.windows(4).position(|w|w==b"\r\n\r\n").map(|i|(i,i+4)))
}
fn parse_nrrd(data:&[u8],name:String)->Result<Volume,String> {
    if !data.starts_with(b"NRRD000"){return Err("Not a NRRD file".into());}
    let (end,body)=find_header_end(data).ok_or("NRRD header terminator not found")?;
    let head=std::str::from_utf8(&data[..end]).map_err(|_|"NRRD header is not UTF-8")?;
    let mut fields=std::collections::HashMap::<String,String>::new();
    for line in head.lines(){if line.starts_with('#')||!line.contains(':'){continue;}
        if let Some((k,v))=line.split_once(':'){fields.insert(k.trim().to_ascii_lowercase(),v.trim().to_string());}}
    let dim=fields.get("dimension").ok_or("NRRD missing dimension")?.parse::<usize>().map_err(|_|"Invalid NRRD dimension")?;
    if dim!=3{return Err("Only single-channel 3D NRRD volumes are supported".into());}
    let sizes=fields.get("sizes").ok_or("NRRD missing sizes")?.split_whitespace().map(|s|s.parse::<usize>().map_err(|_|"Invalid NRRD size".to_string())).collect::<Result<Vec<_>,_>>()?;
    if sizes.len()!=3{return Err("Expected exactly 3 NRRD sizes".into());}
    let dims=[sizes[0],sizes[1],sizes[2]];let n=checked_voxels(dims)?;
    if fields.contains_key("data file"){return Err("Detached NRRD data is not supported; use an embedded .nrrd file".into());}
    let endian=fields.get("endian").map_or("little",String::as_str);
    let big=endian.eq_ignore_ascii_case("big");
    let kind=fields.get("type").ok_or("NRRD missing type")?.to_ascii_lowercase();
    let bpp=match kind.as_str(){"unsigned char"|"uchar"|"uint8"|"uint8_t"|"char"|"int8"|"int8_t"=>1,
        "short"|"short int"|"int16"|"int16_t"|"unsigned short"|"ushort"|"uint16"|"uint16_t"=>2,
        "float"|"float32"|"double"|"float64"=>if kind.starts_with("double")||kind=="float64"{8}else{4},
        _=>return Err(format!("Unsupported NRRD numeric type: {kind}"))};
    let encoding=fields.get("encoding").map_or("raw",String::as_str).to_ascii_lowercase();
    let mut samples=Vec::with_capacity(n);
    if encoding=="raw" {
        let bytes=n.checked_mul(bpp).ok_or("NRRD size overflow")?;
        let chunk=data.get(body..body+bytes).ok_or("Truncated NRRD pixel data")?;
        for p in chunk.chunks_exact(bpp){let v=match (kind.as_str(),bpp) {
            ("char"|"int8"|"int8_t",1)=>p[0] as i8 as f32,
            (_,1)=>p[0] as f32,
            ("short"|"short int"|"int16"|"int16_t",2)=> (if big{i16::from_be_bytes([p[0],p[1]])}else{i16::from_le_bytes([p[0],p[1]])}) as f32,
            (_,2)=> (if big{u16::from_be_bytes([p[0],p[1]])}else{u16::from_le_bytes([p[0],p[1]])}) as f32,
            (_,4)=>f32::from_bits(if big{u32::from_be_bytes(p.try_into().unwrap())}else{u32::from_le_bytes(p.try_into().unwrap())}),
            (_,8)=>f64::from_bits(if big{u64::from_be_bytes(p.try_into().unwrap())}else{u64::from_le_bytes(p.try_into().unwrap())}) as f32,
            _=>0.0};samples.push(if v.is_finite(){v}else{0.0});}
    }else if encoding=="ascii"||encoding=="text"||encoding=="txt" {
        let txt=std::str::from_utf8(&data[body..]).map_err(|_|"Invalid NRRD ASCII")?;
        for word in txt.split_whitespace().take(n){let v=word.parse::<f32>().map_err(|_|"Invalid NRRD voxel value")?;samples.push(v);}
        if samples.len()!=n{return Err("Truncated NRRD ASCII data".into());}
    }else{return Err(format!("Unsupported NRRD encoding '{encoding}'; save as raw or ascii"))}
    let mut spacing_um=[1.0;3];let mut calibrated=false;
    if let Some(spaces)=fields.get("spacings") {
        let vals=spaces.split_whitespace().map(|s|s.parse::<f32>()).collect::<Result<Vec<_>,_>>();
        if let Ok(v)=vals {if v.len()==3 && v.iter().all(|x|x.is_finite()&&*x>0.0){spacing_um.copy_from_slice(&v);calibrated=true;}}
    }
    if !calibrated {if let Some(directions)=fields.get("space directions") {
        let mut lengths=Vec::new();
        for v in directions.split(')').take(3){let Some(start)=v.find('(') else{continue;};
            let nums=v[start+1..].replace(','," ").split_whitespace().map(|x|x.parse::<f32>()).collect::<Result<Vec<_>,_>>();
            if let Ok(n)=nums {if n.len()==3 {lengths.push(n.iter().map(|x|x*x).sum::<f32>().sqrt());}}
        }
        if lengths.len()==3 && lengths.iter().all(|x|x.is_finite()&&*x>0.0) {spacing_um.copy_from_slice(&lengths);calibrated=true;}
    }}
    // NRRD spacings are in the header's declared space units, not inherently micrometres.
    // Without explicit units, retain voxel spacing numbers but never label measurements as µm.
    let mut units=Vec::<&str>::new();
    if let Some(unit_line)=fields.get("space units") {
        units=unit_line.split('"').enumerate().filter_map(|(i,s)| (i%2==1).then_some(s)).collect();
    }
    if units.len()==3 && calibrated {
        for i in 0..3 {
            let factor=match units[i].to_ascii_lowercase().as_str(){
                "um"|"µm"|"micrometer"|"micrometers"|"micron"|"microns"=>1.0,
                "nm"|"nanometer"|"nanometers"=>0.001,
                "mm"|"millimeter"|"millimeters"=>1000.0,
                "m"|"meter"|"meters"=>1_000_000.0,
                "angstrom"|"angstroms"|"a"=>0.0001,
                _=>{calibrated=false;1.0}
            };
            spacing_um[i]*=factor;
        }
    } else {calibrated=false;}
    let range=values_range(&samples);
    Ok(Volume{name,dims,spacing_um,calibrated,samples,source_format:"NRRD".into(),range})
}
fn parse_tiff(data:&[u8],name:String)->Result<Volume,String> {
    if data.len()<8{return Err("Truncated TIFF header".into());}
    let big=match &data[..2]{b"II"=>false,b"MM"=>true,_=>return Err("Not a TIFF file".into())};
    let u16_at=|at:usize|->Result<u16,String>{let s=data.get(at..at+2).ok_or("Truncated TIFF short")?;
        Ok(if big{u16::from_be_bytes([s[0],s[1]])}else{u16::from_le_bytes([s[0],s[1]])})};
    let u32_at=|at:usize|->Result<u32,String>{read_u32(data,at,big)};
    if u16_at(2)?!=42{return Err("Only classic TIFF (not BigTIFF) is currently supported".into());}
    let mut next=u32_at(4)? as usize;let mut pages=0usize;
    let mut samples=Vec::<f32>::new();let mut dimensions=None::<[usize;2]>;
    let mut desc=String::new();
    while next!=0 {
        if pages>=4096{return Err("TIFF exceeds 4096 pages".into());}
        if next+2>data.len(){return Err("Invalid TIFF IFD offset".into());}
        let count=u16_at(next)? as usize;
        if count>4096{return Err("TIFF has too many IFD tags".into());}
        let end=next.checked_add(2).and_then(|v|v.checked_add(count*12)).ok_or("TIFF offset overflow")?;
        if end+4>data.len(){return Err("Truncated TIFF IFD".into());}
        let mut tags=std::collections::HashMap::<u16,Vec<u32>>::new();
        for i in 0..count {
            let offset=next+2+i*12;
            let tag=u16_at(offset)?;let kind=u16_at(offset+2)?;
            let n=u32_at(offset+4)? as usize;
            let width=match kind {1|2=>1usize,3=>2,4=>4,_=>continue};
            let bytes=n.checked_mul(width).ok_or("TIFF tag allocation overflow")?;
            if bytes>16_000_000{return Err("Oversized TIFF metadata tag".into());}
            let place=if bytes<=4{offset+8}else{u32_at(offset+8)? as usize};
            let Some(_) = data.get(place..place.saturating_add(bytes)) else {return Err("Truncated TIFF tag value".into());};
            if tag==270 && kind==2 && desc.is_empty(){
                desc=String::from_utf8_lossy(&data[place..place+bytes]).trim_end_matches('\0').to_owned();
            }
            if [256,257,258,259,262,273,277,278,279,339,270].contains(&tag) {
                let values=if kind==2{Vec::new()}else{(0..n).map(|j|{
                    match width {1=>Ok(data[place+j] as u32),2=>Ok(u16_at(place+j*2)? as u32),4=>u32_at(place+j*4),_=>Ok(0)}
                }).collect::<Result<Vec<_>,String>>()?};
                tags.insert(tag,values);
            }
        }
        let scalar=|tag:u16,default:u32|->u32 {tags.get(&tag).and_then(|v|v.first().copied()).unwrap_or(default)};
        let w=scalar(256,0) as usize;let h=scalar(257,0) as usize;
        if w==0||h==0{return Err("TIFF missing image dimensions".into());}
        if let Some([a,b])=dimensions {if [w,h]!=[a,b]{return Err("Variable-sized TIFF pages are not supported".into());}}
        else {dimensions=Some([w,h]);}
        if scalar(259,1)!=1{return Err("Compressed TIFF is not supported by this dependency-free importer".into());}
        if scalar(277,1)!=1{return Err("Only grayscale single-channel TIFF pages are supported".into());}
        if !matches!(scalar(262,1),0|1){return Err("TIFF photometric interpretation is not grayscale".into());}
        let bits=scalar(258,8);if bits!=8&&bits!=16{return Err("TIFF supports only 8-bit or 16-bit grayscale pixels".into());}
        if scalar(339,1)!=1 {return Err("Signed/floating TIFF samples are not supported".into());}
        let bpp=(bits/8) as usize;
        let (strips,counts)=(tags.get(&273).ok_or("TIFF lacks StripOffsets")?,tags.get(&279).ok_or("TIFF lacks StripByteCounts")?);
        if strips.len()!=counts.len(){return Err("TIFF strip offset/count mismatch".into());}
        let page_len=w.checked_mul(h).and_then(|v|v.checked_mul(bpp)).ok_or("TIFF pixel count overflow")?;
        let combined=(pages+1).checked_mul(w).and_then(|v|v.checked_mul(h)).ok_or("TIFF volume overflow")?;
        if combined>MAX_VOXELS{return Err("TIFF stack exceeds 64 million voxels".into());}
        let mut bytes=Vec::with_capacity(page_len);
        for (&off,&len) in strips.iter().zip(counts){
            let part=data.get(off as usize..(off as usize).saturating_add(len as usize)).ok_or("TIFF strip exceeds file")?;
            let left=page_len.saturating_sub(bytes.len());bytes.extend_from_slice(&part[..part.len().min(left)]);
        }
        if bytes.len()!=page_len {return Err("Incomplete TIFF image strips".into());}
        let inverted=scalar(262,1)==0;
        if bpp==1 {for byte in bytes {samples.push((if inverted {255-byte}else{byte}) as f32);}}
        else {for p in bytes.chunks_exact(2) {let value=if big{u16::from_be_bytes([p[0],p[1]])}else{u16::from_le_bytes([p[0],p[1]])};samples.push((if inverted{u16::MAX-value}else{value}) as f32);}}
        pages+=1;next=u32_at(end)? as usize;
    }
    if pages==0{return Err("TIFF contains no images".into());}
    let [w,h]=dimensions.ok_or("TIFF has no image")?;let dims=[w,h,pages];checked_voxels(dims)?;
    // Only OME files with one Z stack (one channel, one time point) can be
    // interpreted unambiguously as a 3D density volume.
    if desc.contains("<OME") || desc.contains(":OME") {
        for key in ["SizeC","SizeT"] {if let Some(value)=xml_attribute(&desc,key) {
            if value.parse::<usize>().unwrap_or(0)!=1{return Err(format!("OME-TIFF {key}={value}; only one channel and one time point supported"));}
        }}
        if let Some(z)=xml_attribute(&desc,"SizeZ") {if z.parse::<usize>().unwrap_or(0)!=pages {
            return Err("OME-TIFF SizeZ disagrees with the number of pages".into());
        }}
    }
    let mut spacing_um=[1.0;3];let mut calibrated=true;
    for (i,key) in ["PhysicalSizeX","PhysicalSizeY","PhysicalSizeZ"].into_iter().enumerate(){
        let size=xml_attribute(&desc,key).and_then(|s|s.parse::<f32>().ok());
        let units=xml_attribute(&desc,&format!("{key}Unit")).unwrap_or("um");
        let scale=match units {"µm"|"um"=>1.0,"nm"=>0.001,"mm"=>1000.0,"m"=>1_000_000.0,_=>{calibrated=false;1.0}};
        if let Some(size)=size {if size.is_finite()&&size>0.0{spacing_um[i]=size*scale;}else{calibrated=false;}}
        else if i==2 && pages==1{spacing_um[2]=1.0;}else{calibrated=false;}
    }
    let range=values_range(&samples);
    Ok(Volume{name,dims,spacing_um,calibrated,samples,source_format:"TIFF/OME-TIFF grayscale".into(),range})
}
fn xml_attribute<'a>(text:&'a str,key:&str)->Option<&'a str> {
    let needle=format!("{key}=\"");let start=text.find(&needle)?+needle.len();
    let tail=&text[start..];Some(&tail[..tail.find('"')?])
}

/// Load only formats with an implemented decoder; never disguise an unsupported
/// microscopy format as an empty scene or claim automatic cell segmentation.
pub fn load_volume(path:&Path)->Result<Volume,String>{
    let meta=fs::metadata(path).map_err(|e|e.to_string())?;
    if meta.len()>MAX_FILE_BYTES{return Err("Microscopy file exceeds 384 MiB import limit".into());}
    let data=fs::read(path).map_err(|e|e.to_string())?;
    let name=path.file_name().map_or_else(||"Microscopy dataset".into(),|n|n.to_string_lossy().into_owned());
    let ext=path.extension().and_then(|e|e.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str(){"mrc"|"map"|"ccp4"=>parse_mrc(&data,name),"nrrd"=>parse_nrrd(&data,name),"tif"|"tiff"=>parse_tiff(&data,name),
        _=>Err("Supported microscopy formats: MRC, MAP, CCP4, raw/ascii NRRD, uncompressed grayscale TIFF/OME-TIFF".into())}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]fn cell_models_have_selection_metadata(){let c=GeneratedCell::animal();assert!(c.organelles.len()>12);assert!(c.organelles.iter().all(|o|o.estimated && o.visible));assert!(GeneratedCell::plant().organelles.iter().any(|o|o.kind==OrganelleKind::Chloroplast));}
    #[test]fn nrrd_raw_round_trip_read() {let mut data=b"NRRD0005\ntype: unsigned char\ndimension: 3\nsizes: 2 2 2\nencoding: raw\nspacings: 0.5 0.6 0.7\nspace units: \"um\" \"um\" \"um\"\n\n".to_vec();data.extend([1,2,3,4,5,6,7,8]);let v=parse_nrrd(&data,"test".into()).unwrap();assert_eq!(v.value(1,1,1),8.0);assert!(v.calibrated);assert_eq!(v.spacing_um,[0.5,0.6,0.7]);}
    #[test]fn mrc_float_read() {let mut d=vec![0u8;1024+8*4];for (o,v) in [(0,2u32),(4,2),(8,2),(12,2),(28,2),(32,2),(36,2)]{d[o..o+4].copy_from_slice(&v.to_le_bytes());}for o in [40,44,48]{d[o..o+4].copy_from_slice(&10000f32.to_le_bytes());}d[208..212].copy_from_slice(b"MAP ");for i in 0..8{d[1024+i*4..1028+i*4].copy_from_slice(&(i as f32).to_le_bytes());}let v=parse_mrc(&d,"test".into()).unwrap();assert_eq!(v.value(1,1,1),7.0);assert!(v.calibrated);}
    #[test]fn tiff_one_page_grayscale() {
        let mut data=vec![0u8;138];
        data[0..4].copy_from_slice(b"II*\0");data[4..8].copy_from_slice(&8u32.to_le_bytes());
        data[8..10].copy_from_slice(&9u16.to_le_bytes());
        for (i,(tag,ty,n,value)) in [(256u16,4u16,1u32,2u32),(257,4,1,2),(258,3,1,8),(259,3,1,1),(262,3,1,1),(273,4,1,134),(277,3,1,1),(278,4,1,2),(279,4,1,4)].into_iter().enumerate(){
            let off=10+i*12;data[off..off+2].copy_from_slice(&tag.to_le_bytes());data[off+2..off+4].copy_from_slice(&ty.to_le_bytes());
            data[off+4..off+8].copy_from_slice(&n.to_le_bytes());data[off+8..off+12].copy_from_slice(&value.to_le_bytes());
        }
        data[134..138].copy_from_slice(&[10,20,30,40]);
        let v=parse_tiff(&data,"micro.tif".into()).unwrap();assert_eq!(v.dims,[2,2,1]);assert_eq!(v.value(1,1,0),40.0);
    }
    #[test]fn mask_auto_contour_is_near_midpoint() {
        let mut values=vec![0.0;900]; values.extend(vec![1.0;100]);
        let mask=Volume::from_voxels([10,10,10],values).unwrap();
        assert!((mask.suggested_contour()-0.5).abs()<0.02);
    }
    #[test]fn density_contour_is_valid_for_signed_data() {
        let vals=(0..1000).map(|x| (x as f32-500.)/1000.).collect();
        let vol=Volume::from_voxels([10,10,10],vals).unwrap();
        assert!((0.0..1.0).contains(&vol.suggested_contour()));
    }
    #[test]fn malformed_volume_rejected(){assert!(parse_nrrd(b"NRRD0005\n\n","t".into()).is_err());assert!(checked_voxels([10000,10000,10000]).is_err());}
}
