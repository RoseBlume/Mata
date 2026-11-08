//! Dependency-free reader/developer for uncompressed FITS IMAGE HDUs.
//! This intentionally does not handle tile-compressed FITS tables / .fz files.

use std::{collections::HashMap, fs::File, io::{self, BufReader, Read, Seek, SeekFrom, Write}, path::Path, thread};

const BLOCK: u64 = 2880;
const MAX_PIXELS: usize = 100_000_000;
const MAX_HEADER_BLOCKS: usize = 128;

/// Distinguishes camera image pixels from a visualization of numeric FITS tables.
#[derive(Clone, Debug)]
pub enum FitsKind {
    Image,
    /// Visualized TESS cotrending basis vectors: X is sampled cadence order;
    /// Y is the basis vector index. This is NOT a sky image.
    CbvHeatmap { cadences: usize, vectors: Vec<String>, extension: String },
}
impl FitsKind {
    pub fn is_table(&self)->bool { matches!(self, Self::CbvHeatmap{..}) }
}

#[derive(Clone, Debug)]
pub struct FitsImage {
    pub width: usize,
    pub height: usize,
    /// FITS plane order. Either 1 or 3 image planes, not packed RGB.
    pub planes: Vec<Vec<f32>>,
    pub bitpix: i32,
    pub name: String,
    pub filter: String,
    pub bayer: Option<Bayer>,
    pub exposure_s: f32,
    pub wavelength_nm: Option<f32>,
    pub filter_kind: FilterKind,
    pub cube_is_spectral: bool,
    pub kind: FitsKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bayer { Rggb, Bggr, Grbg, Gbrg }
impl Bayer {
    pub fn next(self) -> Self { match self { Self::Rggb=>Self::Bggr, Self::Bggr=>Self::Grbg, Self::Grbg=>Self::Gbrg, Self::Gbrg=>Self::Rggb } }
    pub fn label(self) -> &'static str { match self { Self::Rggb=>"RGGB",Self::Bggr=>"BGGR",Self::Grbg=>"GRBG",Self::Gbrg=>"GBRG" } }
    pub fn from_header(s: &str) -> Option<Self> {
        let upper = s.trim().to_ascii_uppercase();
        match upper.as_str() { "RGGB"=>Some(Self::Rggb), "BGGR"=>Some(Self::Bggr), "GRBG"=>Some(Self::Grbg), "GBRG"=>Some(Self::Gbrg), _=>None }
    }
    /// Coordinates are in the FITS array's native bottom-row-first orientation.
    pub fn shifted(self, dx:usize, dy:usize)->Self{
        let a=self.channel(dx&1,dy&1);
        let b=self.channel((dx+1)&1,dy&1);
        let c=self.channel(dx&1,(dy+1)&1);
        match (a,b,c) {
            (0,1,1)=>Self::Rggb,(2,1,1)=>Self::Bggr,
            (1,0,2)=>Self::Grbg,(1,2,0)=>Self::Gbrg,
            _=>self,
        }
    }
    fn channel(self, x: usize, y_fits: usize) -> usize {
        let index = (y_fits & 1) * 2 + (x & 1);
        let channels = match self {
            Self::Rggb => [0,1,1,2], Self::Bggr => [2,1,1,0],
            Self::Grbg => [1,0,2,1], Self::Gbrg => [1,2,0,1],
        };
        channels[index]
    }
}

/// Conventional filter names; a wavelength/color assignment is not a measured RGB color.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum FilterKind { Red, Green, Blue, HAlpha, Oiii, Sii, Luminance, Other }
impl FilterKind {
    pub fn recognize(name:&str)->Self {
        let cleaned:String=name.to_ascii_uppercase().chars().filter(|c|c.is_ascii_alphanumeric()).collect();
        match cleaned.as_str() {
            "R"|"RED"|"SR"|"RPRIME"|"JOHNSONR"=>Self::Red,
            "G"|"GREEN"|"SG"|"GPRIME"|"JOHNSONV"=>Self::Green,
            "B"|"BLUE"|"SB"|"BPRIME"|"JOHNSONB"=>Self::Blue,
            "HA"|"HALPHA"|"HALPHA656"|"HALPHA6563"|"H6563"|"H656"|"HΑ"=>Self::HAlpha,
            "OIII"|"O3"|"OXYGENIII"|"OXYGEN3"|"O5007"|"OIII5007"=>Self::Oiii,
            "SII"|"S2"|"SULFURII"|"SULPHURII"|"S672"|"SII672"=>Self::Sii,
            "L"|"LUM"|"LUMINANCE"|"CLEAR"|"NONE"=>Self::Luminance,
            _ if cleaned.starts_with("HALPHA") || cleaned.starts_with("HA656")
                || cleaned.starts_with("H656") => Self::HAlpha,
            _ if cleaned.starts_with("OIII") || cleaned.starts_with("O3")
                || cleaned.starts_with("OXYGENIII") => Self::Oiii,
            _ if cleaned.starts_with("SII") || cleaned.starts_with("S2")
                || cleaned.starts_with("SULFURII") => Self::Sii,
            _ if cleaned.starts_with("RBESSEL") || cleaned.starts_with("SDSSR")=>Self::Red,
            _ if cleaned.starts_with("GBESSEL") || cleaned.starts_with("SDSSG")=>Self::Green,
            _ if cleaned.starts_with("BBESSEL") || cleaned.starts_with("SDSSB")=>Self::Blue,
            _=>Self::Other,
        }
    }
    pub fn wavelength_nm(self)->Option<f32>{match self{
        Self::Red=>Some(640.),Self::Green=>Some(540.),Self::Blue=>Some(460.),
        Self::HAlpha=>Some(656.28),Self::Oiii=>Some(500.7),Self::Sii=>Some(671.6),
        _=>None,
    }}
    pub fn label(self)->&'static str {match self{
        Self::Red=>"R",Self::Green=>"G",Self::Blue=>"B",Self::HAlpha=>"H-alpha",
        Self::Oiii=>"OIII",Self::Sii=>"SII",Self::Luminance=>"Luminance",Self::Other=>"Unknown",
    }}
}

// 24 hand-designed color families x 16 grading variations = 384 selectable
// presets, plus six classic maps and Custom. Generated in constant space.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum FalseColorPalette { Nebula, Viridis, Plasma, Inferno, IceFire, Heat, Catalog(u16), Custom }

pub const PALETTE_FAMILIES: [(&str,[[u8;3];3]);24] = [
    ("Orion",      [[11,18,57],[27,125,185],[251,177,83]]),
    ("Lagoon",     [[10,22,61],[9,140,153],[177,233,187]]),
    ("Rosette",    [[46,12,73],[185,47,140],[255,206,144]]),
    ("Andromeda",  [[13,22,52],[78,85,162],[238,205,182]]),
    ("Aurora",     [[9,24,60],[48,192,123],[191,255,174]]),
    ("Solar",      [[45,5,19],[210,52,26],[255,233,96]]),
    ("Lunar",      [[12,22,41],[106,142,169],[245,247,251]]),
    ("Infrared",   [[24,8,45],[144,35,83],[244,185,88]]),
    ("Ultraviolet",[[26,8,68],[120,62,215],[175,204,255]]),
    ("Radio",      [[17,16,61],[55,90,188],[105,227,242]]),
    ("Emerald",    [[3,41,35],[40,155,98],[214,249,126]]),
    ("Cobalt",     [[9,20,80],[39,92,221],[198,221,255]]),
    ("Amber",      [[46,16,3],[202,103,24],[255,238,167]]),
    ("Coral",      [[60,10,22],[225,91,85],[255,221,153]]),
    ("Arctic",     [[5,31,59],[60,172,210],[211,246,252]]),
    ("Volcanic",   [[45,8,5],[205,42,7],[255,192,49]]),
    ("Forest",     [[12,35,18],[66,127,47],[204,229,121]]),
    ("Ocean",      [[5,28,62],[24,112,177],[143,232,234]]),
    ("Amethyst",   [[33,12,66],[124,54,171],[235,176,247]]),
    ("Copper",     [[40,26,22],[164,86,55],[240,186,125]]),
    ("Rose Gold",  [[51,22,45],[184,99,113],[255,213,164]]),
    ("Neon",       [[18,10,60],[185,22,218],[151,255,90]]),
    ("Teal Gold",  [[10,36,47],[23,174,152],[252,209,97]]),
    ("Spectral",   [[21,12,112],[21,188,148],[251,78,95]]),
];
pub const PALETTE_VARIANTS: [&str;16] = [
    "Classic","Soft","Vivid","Deep","Pastel","Muted","Solarized","Noir",
    "Bright","Moonlit","Warm","Cool","Dusk","Dawn","Reverse","Cinema",
];
pub const PRESET_COUNT: usize = 6 + PALETTE_FAMILIES.len()*PALETTE_VARIANTS.len() + 1;

impl FalseColorPalette {
    pub fn from_index(i:usize)->Self {match i {
        0=>Self::Nebula,1=>Self::Viridis,2=>Self::Plasma,3=>Self::Inferno,
        4=>Self::IceFire,5=>Self::Heat,
        n if n < PRESET_COUNT-1 => Self::Catalog((n-6) as u16),
        _=>Self::Custom,
    }}
    pub fn index(self)->usize {match self{
        Self::Nebula=>0,Self::Viridis=>1,Self::Plasma=>2,Self::Inferno=>3,
        Self::IceFire=>4,Self::Heat=>5,Self::Catalog(n)=>6+n as usize,
        Self::Custom=>PRESET_COUNT-1,
    }}
    pub fn next(self)->Self{Self::from_index((self.index()+1)%PRESET_COUNT)}
    pub fn prev(self)->Self{Self::from_index((self.index()+PRESET_COUNT-1)%PRESET_COUNT)}
    pub fn label(self)->String{match self{
        Self::Nebula=>"Nebula".into(),Self::Viridis=>"Viridis".into(),Self::Plasma=>"Plasma".into(),
        Self::Inferno=>"Inferno".into(),Self::IceFire=>"Ice-fire".into(),Self::Heat=>"Heat".into(),
        Self::Catalog(n)=>{let i=n as usize;format!("{} / {}",PALETTE_FAMILIES[i/16].0,PALETTE_VARIANTS[i%16])},
        Self::Custom=>"Custom".into(),
    }}
}
#[derive(Clone,Copy,Debug)]
pub struct FalseColorSettings {
    pub palette: FalseColorPalette,
    pub hue_shift_deg: f32,
    pub star_whitening: f32,
    pub shadows_hue:f32, pub midtones_hue:f32, pub highlights_hue:f32,
}
impl Default for FalseColorSettings {fn default()->Self{Self{
    palette:FalseColorPalette::Nebula,hue_shift_deg:0.,star_whitening:0.72,
    shadows_hue:245.,midtones_hue:184.,highlights_hue:30.,
}}}

fn invalid(message: impl Into<String>) -> io::Error { io::Error::new(io::ErrorKind::InvalidData, message.into()) }
fn round_block(n: u64) -> io::Result<u64> { n.checked_add(BLOCK-1).map(|n| n/BLOCK*BLOCK).ok_or_else(||invalid("FITS size overflow")) }

fn read_header(file: &mut BufReader<File>) -> io::Result<Option<HashMap<String,String>>> {
    let mut result = HashMap::new();
    let mut first = true;
    for _ in 0..MAX_HEADER_BLOCKS {
        let mut block = [0u8; BLOCK as usize];
        match file.read_exact(&mut block) {
            Ok(())=>{}, Err(e) if e.kind()==io::ErrorKind::UnexpectedEof && first=>return Ok(None),
            Err(e)=>return Err(e),
        }
        first = false;
        for card in block.chunks_exact(80) {
            if !card.iter().all(|b| b.is_ascii()) { return Err(invalid("non-ASCII FITS header")); }
            let key = String::from_utf8_lossy(&card[..8]).trim().to_string();
            if key == "END" { return Ok(Some(result)); }
            if card[8]==b'=' && card[9]==b' ' {
                let content = String::from_utf8_lossy(&card[10..]);
                // Do not treat '/' inside quoted FITS values as a comment marker.
                let mut quoted=false;
                let mut value=String::new();
                for c in content.chars() {
                    if c=='\'' { quoted=!quoted; }
                    if c=='/' && !quoted { break; }
                    value.push(c);
                }
                result.insert(key,value.trim().trim_matches('\'').trim().to_string());
            }
        }
    }
    Err(invalid("FITS header exceeds safety limit"))
}
fn number(h:&HashMap<String,String>, k:&str, default:i64)->io::Result<i64>{
    match h.get(k) {Some(v)=>v.parse::<i64>().map_err(|_|invalid(format!("invalid {k}: {v}"))),None=>Ok(default)}
}
fn real(h:&HashMap<String,String>, k:&str, default:f64)->io::Result<f64>{
    match h.get(k){Some(v)=>v.replace('D',"E").replace('d',"E").parse::<f64>().map_err(|_|invalid(format!("invalid {k}: {v}"))),None=>Ok(default)}
}

/// Binary-table TFORM field format. We skip unsupported column types while
/// retaining their byte width to find following columns correctly.
fn table_form(form:&str)->Option<(usize,char,usize)> {
    let form=form.trim().to_ascii_uppercase();
    let digits=form.bytes().take_while(|c|c.is_ascii_digit()).count();
    let repeat=if digits==0 {1usize}else{form[..digits].parse::<usize>().ok()?};
    let kind=form[digits..].chars().next()?;
    let elem=match kind { 'L'|'A'|'B'=>1usize,'I'=>2,'J'|'E'=>4,'K'|'D'|'C'|'P'=>8,'M'|'Q'=>16,
        'X'=>0,_=>return None };
    let bytes=if kind=='X' {repeat.checked_add(7)?/8} else {repeat.checked_mul(elem)?};
    Some((repeat,kind,bytes))
}
#[derive(Clone)]
struct CbvColumn { name:String, offset:usize, code:char, scale:f64, zero:f64 }
/// Read scalar float CBV columns from the BINTABLE without loading every time
/// sample. The output is an explicitly labeled numeric heatmap, not camera RGB.
fn read_cbv_table(reader:&mut BufReader<File>, header:&HashMap<String,String>, start:u64)->io::Result<Option<FitsImage>>{
    let nrows=number(header,"NAXIS2",0)?;
    let row_bytes=number(header,"NAXIS1",0)?;
    let nfields=number(header,"TFIELDS",0)?;
    if nrows<=0 || row_bytes<=0 || nfields<=0 || nfields>512 || row_bytes>1_048_576 {
        return Ok(None);
    }
    let nrows=nrows as usize;
    let row_bytes=row_bytes as usize;
    let mut offset=0usize;
    let mut vectors=Vec::<CbvColumn>::new();
    for index in 1..=nfields as usize {
        let Some(form)=header.get(&format!("TFORM{index}")) else {return Ok(None)};
        let Some((repeat,code,byte_len))=table_form(form) else {return Ok(None)};
        let label=header.get(&format!("TTYPE{index}")).map(|s|s.trim().to_ascii_uppercase()).unwrap_or_default();
        if label.starts_with("VECTOR_") && label[7..].chars().all(|c|c.is_ascii_digit())
            && repeat==1 && matches!(code,'E'|'D') && vectors.len()<32 {
            let scale=real(header,&format!("TSCAL{index}"),1.)?;
            let zero=real(header,&format!("TZERO{index}"),0.)?;
            vectors.push(CbvColumn{name:label,offset,code,scale,zero});
        }
        offset=offset.checked_add(byte_len).ok_or_else(||invalid("BINTABLE offset overflow"))?;
        if offset>row_bytes {return Ok(None);}
    }
    if vectors.is_empty() {return Ok(None);}
    let width=nrows.min(2048).max(1);
    let mut channels=vec![vec![f32::NAN;width];vectors.len()];
    let mut buf=vec![0u8;row_bytes];
    for x in 0..width {
        // Uniform decimation preserves the first and last cadence, avoiding
        // massive allocations when tables have hundreds of thousands of rows.
        let row=if width==1 {0}else{x*(nrows-1)/(width-1)};
        let pos=start.checked_add((row as u64).checked_mul(row_bytes as u64)
            .ok_or_else(||invalid("BINTABLE row offset overflow"))?)
            .ok_or_else(||invalid("BINTABLE position overflow"))?;
        reader.seek(SeekFrom::Start(pos))?;
        reader.read_exact(&mut buf)?;
        for (ch,col) in vectors.iter().enumerate(){
            let off=col.offset;
            let val=match col.code {
                'E'=>f32::from_be_bytes(buf[off..off+4].try_into().unwrap()) as f64,
                'D'=>f64::from_be_bytes(buf[off..off+8].try_into().unwrap()),
                _=>unreachable!(),
            };
            let value=val*col.scale+col.zero;
            if value.is_finite(){channels[ch][x]=value as f32;}
        }
    }
    // Convert each vector to a robust, sign-preserving [0,1] visualization,
    // centered at 0.5. Units are not comparable across differently-scaled CBVs.
    // Repeating each vector in height makes the matrix legible when fit to window.
    const STRIPE:usize=18;
    let height=vectors.len()*STRIPE;
    let mut image=vec![f32::NAN;width*height];
    for (ch,values) in channels.iter().enumerate() {
        let mut finite:Vec<f32>=values.iter().copied().filter(|v|v.is_finite()).collect();
        if finite.is_empty(){continue;}
        finite.sort_unstable_by(f32::total_cmp);
        let mid=finite[finite.len()/2];
        let lo=finite[finite.len()/20];
        let hi=finite[(finite.len()-1)*19/20];
        let scale=(mid-lo).abs().max((hi-mid).abs()).max(1e-12);
        for x in 0..width {
            let v=values[x];
            let normalized=if v.is_finite() {0.5+0.5*((v-mid)/scale).clamp(-1.,1.)} else {f32::NAN};
            for band_y in 0..STRIPE {
                image[(ch*STRIPE+band_y)*width+x]=normalized;
            }
        }
    }
    Ok(Some(FitsImage{
        width,height,planes:vec![image],bitpix:8,
        name:header.get("OBJECT").cloned().unwrap_or_default(),
        filter:String::new(),filter_kind:FilterKind::Other,bayer:None,
        exposure_s:1.,wavelength_nm:None,cube_is_spectral:false,
        kind:FitsKind::CbvHeatmap{cadences:nrows,
            vectors:vectors.into_iter().map(|c|c.name).collect(),
            extension:header.get("EXTNAME").cloned().unwrap_or_else(||"CBV".into())},
    }))
}

pub fn read_fits(path:&Path)->io::Result<FitsImage>{
    let file = File::open(path)?;
    let len = file.metadata()?.len();
    let mut reader=BufReader::with_capacity(256*1024,file);
    while reader.stream_position()? < len {
        let hdr = match read_header(&mut reader)? {Some(h)=>h,None=>break};
        let bitpix = number(&hdr,"BITPIX",0)? as i32;
        let bps = match bitpix {8=>1u64,16=>2,32|-32=>4,64|-64=>8,_=>return Err(invalid(format!("unsupported BITPIX {bitpix}")))};
        let naxis=number(&hdr,"NAXIS",0)?;
        if !(0..=999).contains(&naxis) {return Err(invalid("invalid FITS NAXIS"));}
        let mut dims=Vec::new();
        let mut elems=if naxis==0 {0u64}else{1};
        for i in 1..=naxis {
            let v=number(&hdr,&format!("NAXIS{i}"),-1)?;
            if v < 0 {return Err(invalid("invalid NAXISn"));}
            let dim=v as u64;
            elems=elems.checked_mul(dim).ok_or_else(||invalid("FITS data size overflow"))?;
            dims.push(dim);
        }
        let pcount=number(&hdr,"PCOUNT",0)?;
        let gcount=number(&hdr,"GCOUNT",1)?;
        if pcount<0 || gcount<0 {return Err(invalid("negative PCOUNT or GCOUNT"));}
        let data_bytes=elems.checked_add(pcount as u64)
            .and_then(|x|x.checked_mul(gcount as u64))
            .and_then(|x|x.checked_mul(bps)).ok_or_else(||invalid("FITS data size overflow"))?;
        let start=reader.stream_position()?;
        let end=start.checked_add(round_block(data_bytes)?).ok_or_else(||invalid("FITS data size overflow"))?;
        if end>len {return Err(invalid("truncated FITS HDU"));}
        let extension=hdr.get("XTENSION").map(String::as_str).unwrap_or("IMAGE").to_ascii_uppercase();
        let is_image=extension=="IMAGE" && pcount==0 && gcount==1
            && ((dims.len()==2) || (dims.len()==3 && matches!(dims[2],1|3)))
            && dims[0]>0 && dims[1]>0;
        if extension=="BINTABLE" {
            if let Some(table)=read_cbv_table(&mut reader,&hdr,start)? {return Ok(table);}
        }
        if is_image {
            let w=usize::try_from(dims[0]).map_err(|_|invalid("FITS width too large"))?;
            let h=usize::try_from(dims[1]).map_err(|_|invalid("FITS height too large"))?;
            let count=w.checked_mul(h).ok_or_else(||invalid("FITS image size overflow"))?;
            let plane_count=if dims.len()==3 {dims[2] as usize}else{1};
            if count.checked_mul(plane_count).is_none_or(|n|n>MAX_PIXELS) {return Err(invalid("FITS image exceeds 100-million-sample safety limit"));}
            let bscale=real(&hdr,"BSCALE",1.)?;
            let bzero=real(&hdr,"BZERO",0.)?;
            let blank=hdr.get("BLANK").and_then(|s|s.parse::<i64>().ok());
            let mut planes=Vec::with_capacity(plane_count);
            let mut buf=[0u8;8];
            for _ in 0..plane_count {
                let mut pixels=vec![f32::NAN;count];
                for y in 0..h { for x in 0..w {
                    reader.read_exact(&mut buf[..bps as usize])?;
                    let (raw, int_value): (f64,Option<i64>) = match bitpix {
                        8=>{let v=buf[0] as i64;(v as f64,Some(v))},
                        16=>{let v=i16::from_be_bytes(buf[..2].try_into().unwrap()) as i64;(v as f64,Some(v))},
                        32=>{let v=i32::from_be_bytes(buf[..4].try_into().unwrap()) as i64;(v as f64,Some(v))},
                        64=>{let v=i64::from_be_bytes(buf);(v as f64,Some(v))},
                        -32=>(f32::from_be_bytes(buf[..4].try_into().unwrap()) as f64,None),
                        -64=>(f64::from_be_bytes(buf),None),
                        _=>unreachable!(),
                    };
                    // FITS starts on the bottom row, so reverse Y for on-screen display.
                    let i=(h-1-y)*w+x;
                    if int_value.is_some() && blank==int_value {continue;}
                    let v=raw*bscale+bzero;
                    if v.is_finite(){pixels[i]=v as f32;}
                }}
                planes.push(pixels);
            }
            let bayer=["BAYERPAT", "BAYERPATTERN", "COLORTYP", "CFA"].iter()
                .find_map(|key|hdr.get(*key).and_then(|s|Bayer::from_header(s)))
                .map(|pattern|pattern.shifted(number(&hdr,"XBAYROFF",0).unwrap_or(0).rem_euclid(2) as usize,
                                              number(&hdr,"YBAYROFF",0).unwrap_or(0).rem_euclid(2) as usize));
            let filter=hdr.get("FILTER").or_else(||hdr.get("FILTER1")).cloned().unwrap_or_default();
            let filter_kind=FilterKind::recognize(&filter);
            let exposure_s=real(&hdr,"EXPTIME",real(&hdr,"EXPOSURE",1.)?)?.max(0.000001).min(1e9) as f32;
            let wavelength_nm=hdr.get("WAVELEN").or_else(||hdr.get("WAVELENGTH"))
                .or_else(||hdr.get("WAVECENT"))
                .and_then(|v|v.replace('D',"E").parse::<f64>().ok())
                .and_then(|v|{
                    let unit=hdr.get("WAVEUNIT").or_else(||hdr.get("WAVELUNT"))
                        .map(|s|s.to_ascii_lowercase()).unwrap_or_default();
                    let nm=if unit.contains("ang") || unit=="a" {v*0.1}
                        else if unit=="m" {v*1e9}
                        else if unit=="um" || unit.contains("micron") {v*1000.}
                        else if v>3000. && v<10000. {v*0.1} // FITS often uses Angstroms
                        else if v>0.0000003 && v<0.000001 {v*1e9}
                        else {v};
                    (nm>=350. && nm<=850.).then_some(nm as f32)
                });
            let ctype3=hdr.get("CTYPE3").map(|v|v.to_ascii_uppercase()).unwrap_or_default();
            let cube_is_spectral=["WAVE","FREQ","VELO","ENER","AWAV","VRAD"].iter().any(|k|ctype3.contains(k));
            return Ok(FitsImage {
                width:w,height:h,planes,bitpix,
                name:hdr.get("OBJECT").cloned().unwrap_or_default(),
                filter,filter_kind,bayer,exposure_s,wavelength_nm,cube_is_spectral,
                kind:FitsKind::Image,
            });
        }
        reader.seek(SeekFrom::Start(end))?;
    }
    Err(invalid("no supported IMAGE HDU or TESS VECTOR_* BINTABLE found"))
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Mode { Gray, FalseColor, StructureTint, FilterTint, Bayer(Bayer), CubeRgb, AssignedRgb, Sho, Hoo }
impl Mode {
    pub fn label(self)->&'static str {match self{
        Self::Gray=>"MONO", Self::FalseColor=>"ARTISTIC LUT",
        Self::StructureTint=>"ESTIMATED TINT (ARTISTIC)", Self::FilterTint=>"WAVELENGTH TINT (ARTISTIC)",
        Self::Bayer(_)=>"DEBAYER RGB",Self::CubeRgb=>"CUBE RGB",Self::AssignedRgb=>"FILTER RGB",
        Self::Sho=>"SHO (MAPPED)",Self::Hoo=>"HOO (MAPPED)",
    }}
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Stretch { Linear, Asinh, Log, Sqrt }
impl Stretch {
    pub fn next(self)->Self{match self{Self::Linear=>Self::Asinh,Self::Asinh=>Self::Log,Self::Log=>Self::Sqrt,Self::Sqrt=>Self::Linear}}
    pub fn label(self)->&'static str{match self{Self::Linear=>"Linear",Self::Asinh=>"Asinh",Self::Log=>"Log",Self::Sqrt=>"Sqrt"}}
}
#[derive(Clone,Copy,Debug)]
pub struct Adjust {
    pub exposure:f32, pub gamma:f32, pub stretch:Stretch,
    pub saturation:f32, pub neutralize_background:bool,
    pub black_point:f32, pub white_point:f32, pub rgb_gain:[f32;3],
}
impl Default for Adjust {fn default()->Self{Self{
    exposure:0.,gamma:1.,stretch:Stretch::Asinh,saturation:1.12,neutralize_background:true,
    black_point:0., white_point:1., rgb_gain:[1.,1.,1.],
}}}
#[derive(Clone)]
pub struct FilterSlot {
    pub label:String,pub width:usize,pub height:usize,pub pixels:Vec<f32>,pub exposure_s:f32,
}
#[derive(Default)]
pub struct Slots {
    pub red:Option<FilterSlot>,pub green:Option<FilterSlot>,pub blue:Option<FilterSlot>,
    pub ha:Option<FilterSlot>,pub oiii:Option<FilterSlot>,pub sii:Option<FilterSlot>,
}
impl Slots {
    pub fn assign(&mut self, image:&FitsImage, channel:usize,label:&str){
        let item=Self::slot(image,label);
        match channel{0=>self.red=Some(item),1=>self.green=Some(item),_=>self.blue=Some(item)}
    }
    pub fn assign_filter(&mut self,image:&FitsImage,label:&str){
        let slot=Self::slot(image,label);
        match image.filter_kind {
            FilterKind::Red=>self.red=Some(slot),FilterKind::Green=>self.green=Some(slot),
            FilterKind::Blue=>self.blue=Some(slot),FilterKind::HAlpha=>self.ha=Some(slot),
            FilterKind::Oiii=>self.oiii=Some(slot),FilterKind::Sii=>self.sii=Some(slot),_=>{},
        }
    }
    fn slot(image:&FitsImage,label:&str)->FilterSlot {FilterSlot{
        label:label.to_string(),width:image.width,height:image.height,
        pixels:image.planes[0].clone(),exposure_s:image.exposure_s,
    }}
    fn same(a:&FilterSlot,b:&FilterSlot)->bool{a.width==b.width && a.height==b.height}
    pub fn ready(&self)->bool{match (&self.red,&self.green,&self.blue){
        (Some(r),Some(g),Some(b))=>Self::same(r,g)&&Self::same(g,b),_=>false,
    }}
    pub fn ready_hoo(&self)->bool{match (&self.ha,&self.oiii){
        (Some(a),Some(b))=>Self::same(a,b),_=>false,
    }}
    pub fn ready_sho(&self)->bool{match (&self.sii,&self.ha,&self.oiii){
        (Some(s),Some(h),Some(o))=>Self::same(s,h)&&Self::same(h,o),_=>false,
    }}
    pub fn slots_for(&self,mode:Mode)->Option<[&FilterSlot;3]>{
        match mode {
            Mode::AssignedRgb=>Some([self.red.as_ref()?,self.green.as_ref()?,self.blue.as_ref()?]),
            Mode::Hoo=>Some([self.ha.as_ref()?,self.oiii.as_ref()?,self.oiii.as_ref()?]),
            Mode::Sho=>Some([self.sii.as_ref()?,self.ha.as_ref()?,self.oiii.as_ref()?]),
            _=>None,
        }
    }
}

/// Quantile levels computed from a bounded, regular sample rather than sorting
/// all pixels of a possibly giant astronomical image.
fn levels(values:&[f32])->(f32,f32){
    let step=(values.len()/65536).max(1);
    let mut samples:Vec<f32>=values.iter().step_by(step).copied().filter(|v|v.is_finite()).collect();
    if samples.is_empty(){return (0.,1.);}
    samples.sort_unstable_by(|a,b|a.total_cmp(b));
    let low=samples[samples.len()/200]; // 0.5 percentile
    let hi=samples[(samples.len()-1)*995/1000]; // 99.5 percentile
    if hi>low {(low,hi)}else {(low,low+1.)}
}
// A 16-bit lookup for expensive asinh/log/gamma transforms. Quantization
// error is below one screen intensity step; table rebuilt only on adjustment.
const TONE_SIZE:usize=65536;
fn tone_value(v:f32,adj:Adjust)->u8{
    let mut x=((v-adj.black_point)/(adj.white_point-adj.black_point).max(0.01)).clamp(0.,1.);
    x=(x*2f32.powf(adj.exposure)).clamp(0.,1.);
    x=match adj.stretch {Stretch::Linear=>x,
        Stretch::Asinh=>(x*12.).asinh()/12f32.asinh(),
        Stretch::Log=>(1.+100.*x).ln()/101f32.ln(),
        Stretch::Sqrt=>x.sqrt(),};
    (x.powf(1./adj.gamma.max(0.1))*255.).round() as u8
}
fn tone_lut(adj:Adjust)->Vec<u8>{
    (0..TONE_SIZE).map(|i|tone_value(i as f32/(TONE_SIZE-1) as f32,adj)).collect()
}
#[inline(always)] fn stretch_lookup(v:f32,tone:&[u8])->u8{
    if !v.is_finite(){return 0;}
    tone[(v.clamp(0.,1.)*((TONE_SIZE-1) as f32)) as usize]
}
#[inline(always)] fn map_stretch_fast(v:f32,limits:(f32,f32),tone:&[u8])->u8 {
    if !v.is_finite(){return 0;}
    stretch_lookup((v-limits.0)/(limits.1-limits.0).max(1e-12),tone)
}
fn map_stretch(v:f32,limits:(f32,f32),adj:Adjust)->u8 {
    if !v.is_finite(){return 0;}
    tone_value((v-limits.0)/(limits.1-limits.0).max(1e-12),adj)
}
fn color3(r:u8,g:u8,b:u8)->u32{0xff00_0000|((r as u32)<<16)|((g as u32)<<8)|(b as u32)}

fn srgb_to_linear(x:f32)->f32{
    let x=(x/255.).clamp(0.,1.);
    if x<=0.04045{x/12.92}else{((x+0.055)/1.055).powf(2.4)}
}
fn linear_to_srgb(x:f32)->u8{
    let x=x.clamp(0.,1.);
    let s=if x<=0.0031308{12.92*x}else{1.055*x.powf(1./2.4)-0.055};
    (s*255.).round().clamp(0.,255.) as u8
}
fn hue_shift(rgb:[u8;3],shift_deg:f32)->[u8;3]{
    let r=rgb[0] as f32/255.;let g=rgb[1] as f32/255.;let b=rgb[2] as f32/255.;
    let max=r.max(g.max(b));let min=r.min(g.min(b));let d=max-min;
    let mut h=if d<=1e-9{0.}else if max==r{60.*((g-b)/d).rem_euclid(6.)}
        else if max==g{60.*(((b-r)/d)+2.)}else{60.*(((r-g)/d)+4.)};
    let s=if max<=1e-9{0.}else{d/max};let v=max;
    h=(h+shift_deg).rem_euclid(360.);
    if s<=1e-9{return [rgb[0],rgb[1],rgb[2]];}
    let c=v*s;let x=c*(1.-(((h/60.).rem_euclid(2.))-1.).abs());let m=v-c;
    let (rr,gg,bb)=if h<60.{(c,x,0.)}else if h<120.{(x,c,0.)}else if h<180.{(0.,c,x)}
        else if h<240.{(0.,x,c)}else if h<300.{(x,0.,c)}else{(c,0.,x)};
    [((rr+m)*255.).round() as u8,((gg+m)*255.).round() as u8,((bb+m)*255.).round() as u8]
}
fn palette_stops(palette:FalseColorPalette)->&'static [(f32,[u8;3])] {
    match palette {
        FalseColorPalette::Nebula=>&[
            (0.00,[0,0,0]),(0.13,[7,6,28]),(0.34,[14,77,176]),
            (0.58,[48,183,170]),(0.82,[246,167,57]),(1.00,[255,244,230]),
        ],
        FalseColorPalette::Viridis=>&[
            (0.00,[0,0,0]),(0.15,[40,17,76]),(0.35,[44,113,142]),
            (0.58,[33,144,141]),(0.80,[94,201,98]),(1.00,[253,231,37]),
        ],
        FalseColorPalette::Plasma=>&[
            (0.00,[0,0,0]),(0.12,[13,8,76]),(0.34,[126,3,168]),
            (0.60,[204,71,120]),(0.82,[248,149,64]),(1.00,[240,249,33]),
        ],
        FalseColorPalette::Inferno=>&[
            (0.00,[0,0,0]),(0.10,[8,5,30]),(0.32,[87,15,109]),
            (0.58,[187,55,84]),(0.82,[249,142,8]),(1.00,[252,255,164]),
        ],
        FalseColorPalette::IceFire=>&[
            (0.00,[0,0,0]),(0.16,[18,38,92]),(0.38,[37,124,192]),
            (0.58,[208,234,245]),(0.78,[235,120,72]),(1.00,[255,244,231]),
        ],
        FalseColorPalette::Custom|FalseColorPalette::Catalog(_)=>&[],
        FalseColorPalette::Heat=>&[
            (0.00,[0,0,0]),(0.18,[60,0,0]),(0.42,[160,16,0]),
            (0.66,[232,112,0]),(0.84,[255,214,64]),(1.00,[255,247,232]),
        ],
    }
}
fn hsv_color(h:f32,s:f32,v:f32)->[u8;3]{
    let hue=h.rem_euclid(360.);
    let c=v*s;let x=c*(1.-((hue/60.).rem_euclid(2.)-1.).abs());let m=v-c;
    let (r,g,b)=if hue<60.{(c,x,0.)}else if hue<120.{(x,c,0.)}else if hue<180.{(0.,c,x)}
        else if hue<240.{(0.,x,c)}else if hue<300.{(x,0.,c)}else{(c,0.,x)};
    [((r+m)*255.).round() as u8,((g+m)*255.).round() as u8,((b+m)*255.).round() as u8]
}
/// Palette stop display color; also used by the editable sidebar preview.
pub fn palette_color(settings:FalseColorSettings,t:f32)->u32{
    let [r,g,b]=sample_palette(settings,t);
    color3(r,g,b)
}
fn sample_palette(settings:FalseColorSettings,t:f32)->[u8;3]{
    let t=t.clamp(0.,1.);
    let custom_stops=[
        (0.,[0,0,0]),
        (0.12,hsv_color(settings.shadows_hue,0.95,0.28)),
        (0.34,hsv_color(settings.shadows_hue,0.90,0.78)),
        (0.56,hsv_color(settings.midtones_hue,0.88,0.86)),
        (0.85,hsv_color(settings.highlights_hue,0.85,1.)),
        (1.,[255,249,240]),
    ];
    let catalog_stops = if let FalseColorPalette::Catalog(n)=settings.palette {
        let (name,tones)=PALETTE_FAMILIES[n as usize/16];
        let _=name;
        let variation=n as usize%16;
        let mut points=tones;
        let hue_offset=match variation {9=>-12.,10=>18.,11=>-18.,12=>25.,13=>-25.,_=>0.};
        for color in &mut points {
            let mut rgb=if hue_offset!=0. {hue_shift(*color,hue_offset)}else{*color};
            let factor=match variation {1=>0.8,2=>1.22,3=>0.72,4=>0.68,5=>0.65,6=>1.05,
                7=>0.55,8=>1.28,9=>0.86,10=>1.08,11=>1.02,12=>0.82,13=>1.14,
                14=>1.0,15=>1.1,_=>1.};
            let gray=rgb[0] as f32*0.2126+rgb[1] as f32*0.7152+rgb[2] as f32*0.0722;
            for ch in &mut rgb {
                let mut v=gray+(*ch as f32-gray)*factor;
                if variation==4 {v=v*0.63+88.;}
                if variation==5 {v=v*0.75+30.;}
                if variation==6 {v=v*0.86+23.;}
                if variation==7 {v=v*0.64;}
                if variation==8 {v=v*1.14+8.;}
                if variation==15{v=v*0.86+11.;}
                *ch=v.round().clamp(0.,255.) as u8;
            }
            *color=rgb;
        }
        if variation==14{points.reverse();}
        [ (0.,[0,0,0]),(0.12,points[0]),(0.52,points[1]),(0.84,points[2]),(1.,[255,247,234]) ]
    } else {[(0.,[0,0,0]);5]};
    let stops:&[(f32,[u8;3])]=match settings.palette {
        FalseColorPalette::Custom=>&custom_stops,
        FalseColorPalette::Catalog(_)=>&catalog_stops,
        _=>palette_stops(settings.palette),
    };
    for pair in stops.windows(2){
        let (left,a)=pair[0];let (right,b)=pair[1];
        if t<=right{
            let f=((t-left)/(right-left).max(1e-6)).clamp(0.,1.);
            let mut out=[0u8;3];
            for i in 0..3 {
                let la=srgb_to_linear(a[i] as f32);
                let lb=srgb_to_linear(b[i] as f32);
                out[i]=linear_to_srgb(la+(lb-la)*f);
            }
            return if settings.hue_shift_deg.abs()>0.01 {hue_shift(out,settings.hue_shift_deg)} else {out};
        }
    }
    let last=stops.last().map(|(_,c)|*c).unwrap_or([255,255,255]);
    if settings.hue_shift_deg.abs()>0.01 {hue_shift(last,settings.hue_shift_deg)} else {last}
}
fn false_color(raw:&[f32],w:usize,h:usize,x:usize,y:usize,lim:(f32,f32),adj:Adjust,star_whitening:f32,lut:&[[u8;3];256],tone:&[u8],fast_curves:&[[f32;2];256])->u32{
    let i=y*w+x;
    let current_byte=map_stretch_fast(raw[i],lim,tone);
    let current=current_byte as f32/255.;
    if current_byte==0{return color3(0,0,0);}
    let at=|xx:usize,yy:usize| ->f32 {let v=raw[yy*w+xx];if v.is_finite(){v}else{raw[i]}};
    let sx=x.saturating_sub(2);let ex=(x+2).min(w-1);
    let sy=y.saturating_sub(2);let ey=(y+2).min(h-1);
    let surround=(at(sx,y)+at(ex,y)+at(x,sy)+at(x,ey))*0.25;
    let range=(lim.1-lim.0).max(1e-8);
    let compact=((raw[i]-surround)/range*5.0).clamp(0.,1.)*current;
    let detail=((raw[i]-surround).abs()/range*3.2).clamp(0.,1.);
    let palette_t=(fast_curves[current_byte as usize][0]+(0.08*detail)).clamp(0.,1.);
    let base=lut[(palette_t*255.).round().clamp(0.,255.) as usize];
    let whiten=(compact*star_whitening).clamp(0.,0.97)*fast_curves[current_byte as usize][1];
    let channel=|j:usize| ->u8 {((base[j] as f32*(1.-whiten))+255.*whiten).round().clamp(0.,255.) as u8};
    let c=saturate_rgb([channel(0),channel(1),channel(2)],adj.saturation);
    color3(c[0],c[1],c[2])
}

/// Approximate visible-light wavelength-to-display-RGB mapping. The
/// response is intentionally artistic (not a CIE/sensor spectral integration).
fn wavelength_color(nm:f32)->[f32;3]{
    let w=nm.clamp(380.,780.);
    let (r,g,b)=if w<440. {(-(w-440.)/60.,0.,1.)}
        else if w<490. {(0.,(w-440.)/50.,1.)}
        else if w<510. {(0.,1.,-(w-510.)/20.)}
        else if w<580. {((w-510.)/70.,1.,0.)}
        else if w<645. {(1.,(645.-w)/65.,0.)}
        else {(1.,0.,0.)};
    let falloff=if w<420. {0.45+0.55*(w-380.)/40.}
        else if w>700. {0.5+0.5*(780.-w)/80.}else{1.};
    [r*falloff,g*falloff,b*falloff]
}
fn filter_color(img:&FitsImage)->[f32;3]{
    let nm=img.wavelength_nm.or_else(||img.filter_kind.wavelength_nm());
    nm.map(wavelength_color).unwrap_or([1.,0.72,0.42])
}
fn tint(v:u8,c:[f32;3])->u32{
    let x=v as f32/255.;
    let white=x.powf(5.)*0.52; // almost-saturated stars remain pale rather than solid red
    let mix=|i:usize| ->u8 {((c[i]*(1.-white)+white)*x*255.).round().clamp(0.,255.) as u8};
    color3(mix(0),mix(1),mix(2))
}
/// Artistic, structure-aware colorization of a single broadband exposure:
/// bright compact peaks tend toward near-white, diffuse background is toned.
/// It CANNOT determine actual stellar temperatures or nebular line intensities.
fn structure_tint(raw:&[f32],w:usize,h:usize,x:usize,y:usize,lim:(f32,f32),adj:Adjust)->u32{
    let i=y*w+x;
    let current=map_stretch(raw[i],lim,adj) as f32/255.;
    if current<0.002{return color3(0,0,0);}
    // Neighbour estimate on a 5-pixel cross detects point-like structure;
    // never use this mask as a measured spectral classification.
    let at=|xx:usize,yy:usize| ->f32 {let v=raw[yy*w+xx];if v.is_finite(){v}else{raw[i]}};
    let sx=x.saturating_sub(2);let ex=(x+2).min(w-1);
    let sy=y.saturating_sub(2);let ey=(y+2).min(h-1);
    let surround=(at(sx,y)+at(ex,y)+at(x,sy)+at(x,ey))*0.25;
    let range=(lim.1-lim.0).max(1e-8);
    let compact=((raw[i]-surround)/range*4.0).clamp(0.,1.)*current;
    let t=current;
    // HOO-inspired palette, emphatically not a reconstruction of H-alpha/OIII.
    let base=[0.25+0.75*t.powf(0.32),0.18+0.63*(1.-t).powf(0.5),0.38+0.48*(1.-t).powf(0.33)];
    let k=(compact*1.8).clamp(0.,0.95);
    let channel=|j:usize| ->u8 {((base[j]*(1.-k)+k)*t*255.).clamp(0.,255.) as u8};
    color3(channel(0),channel(1),channel(2))
}

fn structure_tint_fast(raw:&[f32],w:usize,h:usize,x:usize,y:usize,lim:(f32,f32),tone:&[u8],tint_curves:&[[f32;3];256])->u32{
    let i=y*w+x;
    let level=map_stretch_fast(raw[i],lim,tone);
    let current=level as f32/255.;
    if level==0{return color3(0,0,0);}
    let at=|xx:usize,yy:usize|->f32{let v=raw[yy*w+xx];if v.is_finite(){v}else{raw[i]}};
    let sx=x.saturating_sub(2);let ex=(x+2).min(w-1);
    let sy=y.saturating_sub(2);let ey=(y+2).min(h-1);
    let surround=(at(sx,y)+at(ex,y)+at(x,sy)+at(x,ey))*0.25;
    let range=(lim.1-lim.0).max(1e-8);
    let compact=((raw[i]-surround)/range*4.).clamp(0.,1.)*current;
    let base=tint_curves[level as usize];
    let k=(compact*1.8).clamp(0.,0.95);
    let channel=|j:usize|->u8{((base[j]*(1.-k)+k)*current*255.).clamp(0.,255.) as u8};
    color3(channel(0),channel(1),channel(2))
}
pub fn bayer_confidence(raw:&[f32],w:usize,h:usize,bayer:Bayer)->f32{
    if w<4 || h<4 || raw.is_empty(){return 0.;}
    let mut bins=[Vec::new(),Vec::new(),Vec::new(),Vec::new()];
    let step=(raw.len()/24000).max(1);
    for i in (0..raw.len()).step_by(step){
        let v=raw[i];if !v.is_finite(){continue;}
        let x=i%w;let y=i/w;
        let idx=((h-1-y)&1)*2+(x&1);
        bins[idx].push(v);
    }
    let med=|values:&mut Vec<f32>|->f32{
        if values.is_empty(){return 0.;}
        values.sort_unstable_by(f32::total_cmp);
        values[values.len()/2]
    };
    let mut tile=[0f32;4];
    for i in 0..4 { tile[i]=med(&mut bins[i]); }
    let spread=(tile[0]+tile[1]+tile[2]+tile[3]).abs()/4.;
    let contrast=((tile[0]-tile[3]).abs()+(tile[1]-tile[2]).abs())/(spread.max(1e-6)*2.);
    let greens=match bayer {
        Bayer::Rggb|Bayer::Bggr => [tile[1],tile[2]],
        Bayer::Grbg|Bayer::Gbrg => [tile[0],tile[3]],
    };
    let green_match=(greens[0]-greens[1]).abs()/(greens[0].abs().max(greens[1].abs()).max(1e-6));
    (contrast*(1.-green_match).clamp(0.,1.)*4.0).clamp(0.,1.)
}
fn bayer_channel_levels(raw:&[f32],w:usize,h:usize,bayer:Bayer)->[(f32,f32);3]{
    let mut channel_samples=[Vec::new(),Vec::new(),Vec::new()];
    let step=(raw.len()/25000).max(1);
    for i in (0..raw.len()).step_by(step){
        let val=raw[i];if !val.is_finite(){continue;}
        let x=i%w;let y=i/w;
        channel_samples[bayer.channel(x,h-1-y)].push(val);
    }
    let mut out=[(0f32,1f32);3];
    for c in 0..3 {
        out[c]=if channel_samples[c].len()>16 { levels(&channel_samples[c]) } else { (0.,1.) };
    }
    out
}
/// Edge-directed green reconstruction; where source color is red/blue,
/// choose the smoother of horizontal/vertical green gradients. Uses native
/// top-down pixel indices but FITS CFA parity remains bottom-row-first.
fn green_at(raw:&[f32],w:usize,h:usize,x:usize,y:usize,bayer:Bayer)->f32{
    let here=raw[y*w+x];
    if bayer.channel(x,h-1-y)==1{return here;}
    let neighbour=|xx:isize,yy:isize|->Option<f32>{
        if xx<0 || yy<0 || xx>=w as isize || yy>=h as isize {return None;}
        let v=raw[yy as usize*w+xx as usize];v.is_finite().then_some(v)
    };
    let x=x as isize;let y=y as isize;
    let l=neighbour(x-1,y);let r=neighbour(x+1,y);
    let t=neighbour(x,y-1);let b=neighbour(x,y+1);
    let hv=match(l,r){(Some(a),Some(z))=>Some(((a+z)*0.5,(a-z).abs()
        +(2.*here-neighbour(x-2,y).unwrap_or(here)-neighbour(x+2,y).unwrap_or(here)).abs()*0.5)),
        (Some(v),_)|(_,Some(v))=>Some((v,f32::MAX/4.)),_=>None};
    let vv=match(t,b){(Some(a),Some(z))=>Some(((a+z)*0.5,(a-z).abs()
        +(2.*here-neighbour(x,y-2).unwrap_or(here)-neighbour(x,y+2).unwrap_or(here)).abs()*0.5)),
        (Some(v),_)|(_,Some(v))=>Some((v,f32::MAX/4.)),_=>None};
    match(hv,vv){
        (Some((a,ga)),Some((b,gb)))=>{
            if ga<gb*0.7{a}else if gb<ga*0.7{b}
            else{(a*0.5+b*0.5)}
        },
        (Some((v,_)),_)|(_,Some((v,_)))=>v,
        _=>here,
    }
}
/// Reconstruct red/blue as interpolated (channel - green) differences; this
/// reduces color zippering and chromatic halos versus raw bilinear averaging.
fn interp_bayer(raw:&[f32],w:usize,h:usize,x:usize,y:usize,bayer:Bayer,channel:usize)->f32{
    if channel==1{return green_at(raw,w,h,x,y,bayer);}
    let here=raw[y*w+x];
    if bayer.channel(x,h-1-y)==channel{return here;}
    let base=green_at(raw,w,h,x,y,bayer);
    if !base.is_finite(){return f32::NAN;}
    let mut sum=0f64;let mut weights=0f64;
    for radius in 1isize..=2isize {
        for dy in -radius..=radius {for dx in -radius..=radius {
            if dy.abs().max(dx.abs())!=radius{continue;}
            let xx=x as isize+dx;let yy=y as isize+dy;
            if xx<0||yy<0||xx>=w as isize||yy>=h as isize{continue;}
            let xx=xx as usize;let yy=yy as usize;
            if bayer.channel(xx,h-1-yy)!=channel{continue;}
            let v=raw[yy*w+xx];
            let g=green_at(raw,w,h,xx,yy,bayer);
            if !v.is_finite()||!g.is_finite(){continue;}
            let weight=1./((dx*dx+dy*dy) as f64).sqrt();
            sum+=(v-g) as f64*weight;weights+=weight;
        }}
        if weights>0.{break;}
    }
    if weights==0.{base}else{base+(sum/weights) as f32}
}

// Avoid recalculating the same edge-directed green interpolation for
// every neighboring red/blue sample (large speedup for raw CFA frames).
fn interp_bayer_fast(raw:&[f32],green:&[f32],w:usize,h:usize,x:usize,y:usize,bayer:Bayer,channel:usize)->f32{
    let i=y*w+x;
    if channel==1{return green[i];}
    if bayer.channel(x,h-1-y)==channel{return raw[i];}
    let base=green[i];
    if !base.is_finite(){return f32::NAN;}
    let mut sum=0f32;let mut weight_sum=0f32;
    for radius in 1isize..=2isize {
        for dy in -radius..=radius {for dx in -radius..=radius {
            if dy.abs().max(dx.abs())!=radius{continue;}
            let xx=x as isize+dx;let yy=y as isize+dy;
            if xx<0||yy<0||xx>=w as isize||yy>=h as isize{continue;}
            let xx=xx as usize;let yy=yy as usize;
            if bayer.channel(xx,h-1-yy)!=channel{continue;}
            let j=yy*w+xx;let v=raw[j];let g=green[j];
            if !v.is_finite() || !g.is_finite(){continue;}
            let weight=match dx*dx+dy*dy {1=>1.,2=>0.70710677,4=>0.5,5=>0.4472136,8=>0.35355338,_=>1.};
            sum+=(v-g)*weight;weight_sum+=weight;
        }}
        if weight_sum>0.{break;}
    }
    if weight_sum==0.{base}else{base+sum/weight_sum}
}

/// Develop into MataWin 0xAARRGGBB; measured RGB pathways share a luminance
/// stretch rather than stretching each color to a separate white point.
/// This preserves more chromatic information than the old independent 0.5/99.5
/// percentile normalization. Optional black-level subtraction suppresses
/// different sky-background offsets, not actual color differences.
pub fn generate_bayer_green(image:&FitsImage,bayer:Bayer)->Vec<f32>{
    let (w,h)=(image.width,image.height);
    let raw=&image.planes[0];
    let count=w*h;
    let mut green=vec![0f32;count];
    let workers=thread::available_parallelism().map(|n|n.get()).unwrap_or(1).min(12).min((count/320_000).max(1));
    let rows_per=((h+workers-1)/workers).max(1);
    thread::scope(|scope|{
        for (part,chunk) in green.chunks_mut(rows_per*w).enumerate(){
            let ystart=part*rows_per;
            scope.spawn(move||{
                for (dy,row) in chunk.chunks_mut(w).enumerate(){
                    let y=ystart+dy;
                    for x in 0..w{row[x]=green_at(raw,w,h,x,y,bayer);}
                }
            });
        }
    });
    green
}
pub fn develop(image:&FitsImage, mode:Mode, adj:Adjust, slots:&Slots, false_color_settings:FalseColorSettings)->io::Result<Vec<u32>>{
    develop_with_green(image,mode,adj,slots,false_color_settings,None)
}
/// Optional cached green plane allows repeated debayer adjustments without
/// recomputing expensive edge-directed interpolation each time.
pub fn develop_with_green(image:&FitsImage, mode:Mode, adj:Adjust, slots:&Slots, false_color_settings:FalseColorSettings,cached_green:Option<&[f32]>)->io::Result<Vec<u32>>{
    let count=image.width.checked_mul(image.height).ok_or_else(||invalid("image dimensions overflow"))?;
    if image.planes.is_empty(){return Err(invalid("image has no planes"));}
    if mode==Mode::CubeRgb && image.planes.len()!=3{return Err(invalid("not a 3-plane cube"));}
    let assigned=slots.slots_for(mode);
    if matches!(mode,Mode::AssignedRgb|Mode::Sho|Mode::Hoo) && assigned.is_none(){return Err(invalid("required filter frames missing"));}
    if let Some(filters)=assigned {
        if filters.iter().any(|f|f.width!=image.width || f.height!=image.height){
            return Err(invalid("filter frames must have identical dimensions and be aligned"));
        }
    }
    let raw=&image.planes[0];
    let raw_levels=levels(raw);
    let mono=if adj.neutralize_background || raw_levels.1<=0. {raw_levels}else{(0.,raw_levels.1)};
    let measured=matches!(mode,Mode::AssignedRgb|Mode::Sho|Mode::Hoo|Mode::CubeRgb|Mode::Bayer(_));
    let mut low=[0f32;3];let mut high=[1f32;3];let mut gain=[1f32;3];
    if measured && !matches!(mode,Mode::Bayer(_)) {
        for c in 0..3 {
            let (pixels,exptime):(&[f32],f32)=if let Some(filters)=assigned {
                (&filters[c].pixels,filters[c].exposure_s)
            }else{(&image.planes[c],image.exposure_s)};
            let (a,b)=levels(pixels);
            gain[c]=1./exptime.max(0.000001);
            low[c]=if adj.neutralize_background {a*gain[c]}else{0.};
            high[c]=((b-a).max(1e-6))*gain[c];
        }
    }
    let mut ordered=high;ordered.sort_by(f32::total_cmp);
    let shared_high=ordered[1].max(0.000001);
    let tone=tone_lut(adj);
    let lut:[[u8;3];256]=std::array::from_fn(|i|sample_palette(false_color_settings,i as f32/255.));
    let curves:[[f32;2];256]=std::array::from_fn(|i|{
        let x=i as f32/255.;[x.powf(0.88),x.powf(0.35)]
    });
    let tint_curves:[[f32;3];256]=std::array::from_fn(|i|{
        let x=i as f32/255.;
        [0.25+0.75*x.powf(0.32),0.18+0.63*(1.-x).sqrt(),0.38+0.48*(1.-x).powf(0.33)]
    });
    let bayer_gains=if let Mode::Bayer(bayer)=mode {
        bayer_balance(raw,image.width,image.height,bayer,adj.neutralize_background)
    }else{[1.;3]};
    let bayer_levels=if let Mode::Bayer(bayer)=mode {
        bayer_channel_levels(raw,image.width,image.height,bayer)
    }else{[(mono.0,mono.1);3]};
    let bayer_shared_high=if matches!(mode,Mode::Bayer(_)) {
        let mut spans=[0f32;3];
        for ch in 0..3 { spans[ch]=((bayer_levels[ch].1-bayer_levels[ch].0).max(1e-6))*bayer_gains[ch]; }
        spans.sort_by(f32::total_cmp);
        spans[1].max(1e-6)
    }else{1.};
    let w=image.width;let h=image.height;
    let workers=thread::available_parallelism().map(|n|n.get()).unwrap_or(1).min(12).min((count/320_000).max(1));
    let owned_green=if let Mode::Bayer(bayer)=mode {
        if cached_green.is_some_and(|c|c.len()==count){None}
        else{Some(generate_bayer_green(image,bayer))}
    }else{None};
    let green=cached_green.filter(|g|g.len()==count)
        .or_else(||owned_green.as_deref()).unwrap_or(&[]);
    let mut out=vec![0xff00_0000;count];
    let rows_per=((h+workers-1)/workers).max(1);
    let filter_tint=filter_color(image);
    thread::scope(|scope|{
        for (part,chunk) in out.chunks_mut(rows_per*w).enumerate(){
            let ystart=part*rows_per;
            let tone=&tone;
            let lut=&lut;
            let curves=&curves;
            let tint_curves=&tint_curves;
            scope.spawn(move||{
                for (dy,row) in chunk.chunks_mut(w).enumerate(){
                    let y=ystart+dy;
                    for (x,dst) in row.iter_mut().enumerate(){
                        let i=y*w+x;
                        *dst=match mode {
                            Mode::Gray=>{let v=map_stretch_fast(raw[i],mono,tone);color3(v,v,v)},
                            Mode::FalseColor=>false_color(raw,w,h,x,y,mono,adj,false_color_settings.star_whitening,lut,tone,curves),
                            Mode::StructureTint=>saturate_pixel(structure_tint_fast(raw,w,h,x,y,mono,tone,tint_curves),adj.saturation),
                            Mode::FilterTint=>saturate_pixel(tint(map_stretch_fast(raw[i],mono,tone),filter_tint),adj.saturation),
                            Mode::CubeRgb|Mode::AssignedRgb|Mode::Sho|Mode::Hoo=>{
                                let mut c=[0u8;3];
                                for ch in 0..3 {
                                    let pix=if let Some(filters)=assigned {filters[ch].pixels[i]} else {image.planes[ch][i]};
                                    let v=if adj.neutralize_background {(pix*gain[ch]-low[ch])/shared_high}
                                        else {pix*gain[ch]/shared_high};
                                    c[ch]=stretch_lookup(v*adj.rgb_gain[ch],tone);
                                }
                                let c=saturate_rgb(c,adj.saturation);color3(c[0],c[1],c[2])
                            },
                            Mode::Bayer(bayer)=>{
                                let mut c=[0u8;3];
                                for ch in 0..3 {
                                    let rawch=interp_bayer_fast(raw,green,w,h,x,y,bayer,ch);
                                    let level=bayer_levels[ch];
                                    let corrected=(rawch-level.0).max(0.)*bayer_gains[ch];
                                    let val=corrected/bayer_shared_high;
                                    c[ch]=stretch_lookup(val*adj.rgb_gain[ch],tone);
                                }
                                let c=saturate_rgb(c,(adj.saturation*1.12).min(3.0));
                                color3(c[0],c[1],c[2])
                            },
                        };
                    }
                }
            });
        }
    });
    Ok(out)
}
fn stretch_normalized(v:f32,adj:Adjust)->u8{
    if !v.is_finite(){return 0;}
    let mut x=((v-adj.black_point)/(adj.white_point-adj.black_point).max(0.01)).clamp(0.,1.);
    x=(x*2f32.powf(adj.exposure)).clamp(0.,1.);
    x=match adj.stretch {
        Stretch::Linear=>x,Stretch::Asinh=>(x*12.).asinh()/12f32.asinh(),
        Stretch::Log=>(1.+100.*x).ln()/101f32.ln(),Stretch::Sqrt=>x.sqrt(),
    };
    (x.powf(1./adj.gamma.max(0.1))*255.).round() as u8
}
fn saturate_pixel(argb:u32,saturation:f32)->u32{
    let rgb=[((argb>>16)&255) as u8,((argb>>8)&255) as u8,(argb&255) as u8];
    let c=saturate_rgb(rgb,saturation);
    color3(c[0],c[1],c[2])
}
fn saturate_rgb(rgb:[u8;3],saturation:f32)->[u8;3]{
    let [r,g,b]=rgb.map(|c|c as f32);
    let l=r*0.2126+g*0.7152+b*0.0722;
    let sat=saturation.clamp(0.,3.);
    [r,g,b].map(|v|(l+(v-l)*sat).round().clamp(0.,255.) as u8)
}
fn bayer_balance(raw:&[f32],w:usize,h:usize,bayer:Bayer,enabled:bool)->[f32;3]{
    if !enabled || w==0 || h==0{return [1.;3];}
    let mut channel_samples=[Vec::new(),Vec::new(),Vec::new()];
    let step=(raw.len()/20000).max(1);
    for i in (0..raw.len()).step_by(step){
        let val=raw[i];if !val.is_finite(){continue;}
        let x=i%w;let y=i/w;
        channel_samples[bayer.channel(x,h-1-y)].push(val);
    }
    let mut range=[1f32;3];
    for c in 0..3 {if channel_samples[c].len()>20{
        let (black,white)=levels(&channel_samples[c]);range[c]=(white-black).max(1e-6);
    }}
    let reference=range[1];
    // Only a partial gray-world correction: fully normalizing all channels
    // would bleach colored nebulosity and filter response differences.
    [0usize,1,2].map(|c|((reference/range[c]).sqrt()).clamp(0.65,1.55))
}

/// Portable 24-bit BMP export with no external encoders/dependencies.
pub fn save_bmp(path:&Path,width:usize,height:usize,pixels:&[u32])->io::Result<()>{
    if width.checked_mul(height)!=Some(pixels.len()) || width==0 || height==0{return Err(invalid("invalid image size"));}
    let row=((width.checked_mul(3).ok_or_else(||invalid("BMP overflow"))?+3)/4)*4;
    let image_bytes=row.checked_mul(height).ok_or_else(||invalid("BMP overflow"))?;
    let full=54usize.checked_add(image_bytes).ok_or_else(||invalid("BMP overflow"))?;
    let total=u32::try_from(full).map_err(|_|invalid("BMP larger than 4 GiB"))?;
    let mut f=File::create(path)?;
    f.write_all(b"BM")?;f.write_all(&total.to_le_bytes())?;f.write_all(&[0;4])?;f.write_all(&54u32.to_le_bytes())?;
    f.write_all(&40u32.to_le_bytes())?;
    f.write_all(&i32::try_from(width).map_err(|_|invalid("BMP width too large"))?.to_le_bytes())?;
    f.write_all(&i32::try_from(height).map_err(|_|invalid("BMP height too large"))?.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?;f.write_all(&24u16.to_le_bytes())?;
    f.write_all(&0u32.to_le_bytes())?;f.write_all(&(image_bytes as u32).to_le_bytes())?;
    f.write_all(&[0;16])?;
    let mut line=vec![0u8;row];
    for y in (0..height).rev(){
        for (x,&argb) in pixels[y*width..(y+1)*width].iter().enumerate(){
            line[3*x]=(argb&255) as u8;line[3*x+1]=((argb>>8)&255) as u8;line[3*x+2]=((argb>>16)&255) as u8;
        }
        f.write_all(&line)?;
    }
    f.flush()
}

#[cfg(test)] mod tests {
    use super::*;
    use std::{env, fs};
    fn card(k:&str,v:&str)->String{format!("{k:<8}= {v:<70}")}
    fn header(cards:&[String])->Vec<u8>{
        let mut s=cards.join("");s.push_str(&format!("{:<80}","END"));
        let mut b=s.into_bytes();b.resize(((b.len()+2879)/2880)*2880,b' ');b
    }
    #[test] fn loads_scaled_signed_pixels_and_blank(){
        let p=env::temp_dir().join(format!("mata_win-fits-test-{}.fits",std::process::id()));
        let mut bytes=header(&[card("SIMPLE","T"),card("BITPIX","16"),card("NAXIS","2"),card("NAXIS1","2"),card("NAXIS2","2"),card("BSCALE","2"),card("BZERO","32768"),card("BLANK","-32768")]);
        for n in [0i16,10,-32768,20]{bytes.extend_from_slice(&n.to_be_bytes());}
        bytes.resize(bytes.len()+2872,0);fs::write(&p,bytes).unwrap();
        let image=read_fits(&p).unwrap();let _=fs::remove_file(&p);
        assert_eq!((image.width,image.height),(2,2));
        assert!(image.planes[0][0].is_nan());
        assert_eq!(image.planes[0][1],32808.);
        assert_eq!(image.planes[0][2],32768.);
        assert_eq!(image.planes[0][3],32788.);
    }
    #[test] fn loads_tess_cbv_binary_table_as_heatmap(){
        let p=env::temp_dir().join(format!("mata_win-cbv-test-{}.fits",std::process::id()));
        let mut bytes=header(&[
            card("SIMPLE","T"),card("BITPIX","8"),card("NAXIS","0"),card("EXTEND","T"),
        ]);
        bytes.extend(header(&[
            card("XTENSION","'BINTABLE'"),card("BITPIX","8"),card("NAXIS","2"),
            card("NAXIS1","20"),card("NAXIS2","8"),card("PCOUNT","0"),card("GCOUNT","1"),
            card("TFIELDS","4"),
            card("TTYPE1","'TIME'"),card("TFORM1","'1D'"),
            card("TTYPE2","'VECTOR_1'"),card("TFORM2","'1E'"),
            card("TTYPE3","'VECTOR_2'"),card("TFORM3","'1E'"),
            card("TTYPE4","'CADENCENO'"),card("TFORM4","'1J'"),
            card("EXTNAME","'CBV.SINGLE_SCALE.1.1'"),
        ]));
        let start=bytes.len();
        for i in 0..8 {
            bytes.extend_from_slice(&(1200.0f64 + i as f64).to_be_bytes());
            bytes.extend_from_slice(&(i as f32 * 0.1).to_be_bytes());
            bytes.extend_from_slice(&((7-i) as f32 * 0.1).to_be_bytes());
            bytes.extend_from_slice(&(1000i32+i).to_be_bytes());
        }
        assert_eq!(bytes.len()-start,160);
        bytes.resize(start+2880,0);
        fs::write(&p,bytes).unwrap();
        let image=read_fits(&p).unwrap();
        let _=fs::remove_file(&p);
        assert_eq!((image.width,image.height),(8,36));
        match &image.kind {
            FitsKind::CbvHeatmap{cadences,vectors,extension}=>{
                assert_eq!(*cadences,8);
                assert_eq!(vectors.as_slice(),["VECTOR_1","VECTOR_2"]);
                assert!(extension.starts_with("CBV"));
            },
            _=>panic!("expected CBV table"),
        }
        let data=&image.planes[0];
        assert!((data[0]-data[17]).abs()<1e-5);
        assert!((data[0]-data[18*8]).abs()>0.1);
        assert!(data.iter().all(|n|n.is_finite()));
    }
    #[test] fn filters_and_bayer_offsets(){
        assert_eq!(FilterKind::recognize("OIII"),FilterKind::Oiii);
        assert_eq!(FilterKind::recognize("OIII 3nm"),FilterKind::Oiii);
        assert_eq!(FilterKind::recognize("H-alpha 7nm"),FilterKind::HAlpha);
        assert_eq!(FilterKind::recognize("H-alpha"),FilterKind::HAlpha);
        assert_eq!(FilterKind::recognize("SII"),FilterKind::Sii);
        assert_eq!(Bayer::Rggb.shifted(1,0),Bayer::Grbg);
        assert_eq!(Bayer::Rggb.shifted(0,1),Bayer::Gbrg);
        assert_eq!(Bayer::Rggb.shifted(1,1),Bayer::Bggr);
    }
    #[test] fn estimator_stays_opaque(){
        let img=FitsImage{width:3,height:3,planes:vec![vec![0.,2.,0.,2.,25.,2.,0.,2.,0.]],
            bitpix:-32,name:"star".into(),filter:"".into(),bayer:None,exposure_s:1.,
            wavelength_nm:None,filter_kind:FilterKind::Other,cube_is_spectral:false,kind:FitsKind::Image};
        for mode in [Mode::Gray,Mode::FalseColor,Mode::StructureTint,Mode::FilterTint]{
            let out=develop(&img,mode,Adjust::default(),&Slots::default(),FalseColorSettings::default()).unwrap();
            assert_eq!(out.len(),9);
            assert!(out.iter().all(|px|px>>24==255));
        }
    }
    #[test] fn demosaic_all_patterns_reaches_all_channels(){
        for bayer in [Bayer::Rggb,Bayer::Bggr,Bayer::Grbg,Bayer::Gbrg] {
            let raw=vec![20f32;16];
            for c in 0..3 {assert_eq!(interp_bayer(&raw,4,4,2,2,bayer,c),20.);}
        }
    }
    #[test] fn ghostbar_black_white_and_palette_changes_are_live(){
        let img=FitsImage{width:4,height:4,planes:vec![vec![0.,0.2,0.4,0.6,0.8,1.,0.7,0.5,0.3,0.1,0.9,0.7,0.5,0.4,0.3,1.]],
            bitpix:-32,name:"test".into(),filter:"".into(),bayer:None,exposure_s:1.,
            wavelength_nm:None,filter_kind:FilterKind::Other,cube_is_spectral:false,kind:FitsKind::Image};
        let base=develop(&img,Mode::FalseColor,Adjust::default(),&Slots::default(),FalseColorSettings::default()).unwrap();
        let mut settings=FalseColorSettings::default();
        settings.palette=FalseColorPalette::Custom;
        settings.midtones_hue=35.;
        let custom=develop(&img,Mode::FalseColor,Adjust::default(),&Slots::default(),settings).unwrap();
        assert_ne!(base,custom);
        assert_ne!(palette_color(settings,0.5),palette_color(FalseColorSettings::default(),0.5));
        let mut adj=Adjust::default();adj.black_point=0.2;adj.white_point=0.8;
        let changed=develop(&img,Mode::Gray,adj,&Slots::default(),settings).unwrap();
        let original=develop(&img,Mode::Gray,Adjust::default(),&Slots::default(),settings).unwrap();
        assert_ne!(changed,original);
    }
    #[test] fn palette_catalog_is_complete_and_unique(){
        assert_eq!(PRESET_COUNT,391);
        let mut names=std::collections::HashSet::new();
        for i in 0..PRESET_COUNT {
            let palette=FalseColorPalette::from_index(i);
            assert_eq!(palette.index(),i);
            assert!(names.insert(palette.label()));
            let settings=FalseColorSettings{palette,..FalseColorSettings::default()};
            for t in [0.,0.2,0.5,0.8,1.] {
                assert_eq!(palette_color(settings,t)>>24,255);
            }
        }
    }
    #[test] fn lut_stretch_is_close_to_reference(){
        for kind in [Stretch::Linear,Stretch::Asinh,Stretch::Log,Stretch::Sqrt]{
            let a=Adjust{stretch:kind,..Adjust::default()};
            let lut=tone_lut(a);
            for i in 0..10000 {
                let x=i as f32/9999.;
                let expected=tone_value(x,a);
                let actual=stretch_lookup(x,&lut);
                assert!((expected as i16-actual as i16).abs()<=1);
            }
        }
    }
    #[test] fn bayer_cached_and_uncached_outputs_match(){
        let (w,h)=(16,12);
        let raw=(0..w*h).map(|i|(i%23) as f32 + (i%3) as f32*4.).collect();
        let img=FitsImage{width:w,height:h,planes:vec![raw],bitpix:-32,
            name:"test".into(),filter:String::new(),bayer:Some(Bayer::Rggb),
            exposure_s:1.,wavelength_nm:None,filter_kind:FilterKind::Other,cube_is_spectral:false,kind:FitsKind::Image};
        let cache=generate_bayer_green(&img,Bayer::Rggb);
        let adj=Adjust::default();let pal=FalseColorSettings::default();let slots=Slots::default();
        let cold=develop(&img,Mode::Bayer(Bayer::Rggb),adj,&slots,pal).unwrap();
        let warm=develop_with_green(&img,Mode::Bayer(Bayer::Rggb),adj,&slots,pal,Some(&cache)).unwrap();
        assert_eq!(cold,warm);
    }

}
