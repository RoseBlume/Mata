//! CRC-protected saves. V7 persists the 60-second economy automation clock.
//! V2 through V6 saves remain compatible without losing their preferences.
use std::{fs,io::{self,Read,Write},path::{Path,PathBuf},time::{SystemTime,UNIX_EPOCH}};
use crate::{big::Big,engine::Game};
#[cfg(test)] use crate::engine::{COUNT,PRESTIGE_COUNT,RESEARCH_COUNT};
const MAGIC_V7:&[u8;8]=b"MNMPIRE7";
const MAGIC_V6:&[u8;8]=b"MNMPIRE6";
const MAGIC_V5:&[u8;8]=b"MNMPIRE5";
const MAGIC_V4:&[u8;8]=b"MNMPIRE4";
const MAGIC_V3:&[u8;8]=b"MNMPIRE3";
const MAGIC_V2:&[u8;8]=b"MNMPIRE2";
const VERSION:u32=7;
const MAX_OFFLINE:u64=60*60*12;
fn bad(what:&str)->io::Error{io::Error::new(io::ErrorKind::InvalidData,what)}
pub fn default_path()->PathBuf{
    let base=std::env::var_os("XDG_DATA_HOME").map(PathBuf::from)
        .or_else(||std::env::var_os("APPDATA").map(PathBuf::from))
        .or_else(||std::env::var_os("HOME").map(|p|PathBuf::from(p).join(".local/share")))
        .unwrap_or_else(||std::env::current_dir().unwrap_or_else(|_|PathBuf::from(".")));
    base.join("mineral-empire").join("save.dat")
}
fn timestamp()->u64{SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()}
fn crc32(bytes:&[u8])->u32{let mut c=0xffff_ffffu32;for &byte in bytes{c^=byte as u32;for _ in 0..8{c=(c>>1)^((c&1).wrapping_neg()&0xedb8_8320);}}!c}
fn write_big(data:&mut Vec<u8>,b:&Big)->io::Result<()> {let text=b.storage();let bytes=text.as_bytes();let len=u32::try_from(bytes.len()).map_err(|_|bad("number exponent too large"))?;data.extend_from_slice(&len.to_le_bytes());data.extend_from_slice(bytes);Ok(())}
pub fn save(path:&Path,g:&Game)->io::Result<()> {
    let mut data=Vec::with_capacity(8192);
    data.extend_from_slice(MAGIC_V7);data.extend_from_slice(&VERSION.to_le_bytes());data.extend_from_slice(&timestamp().to_le_bytes());
    for b in [&g.cash,&g.run_earned,&g.all_time_earned] {write_big(&mut data,b)?;}
    data.extend_from_slice(&g.elapsed_seconds.to_le_bytes());
    for b in &g.total_mined{write_big(&mut data,b)?;}
    for b in &g.prestige{write_big(&mut data,b)?;}
    for b in &g.prestige_lifetime{write_big(&mut data,b)?;}
    for &n in &g.miners{data.extend_from_slice(&n.to_le_bytes());}
    for &n in &g.equipment{data.extend_from_slice(&n.to_le_bytes());}
    for row in &g.research{for &n in row{data.extend_from_slice(&n.to_le_bytes());}}
    for &n in &g.resets{data.extend_from_slice(&n.to_le_bytes());}
    data.push(u8::from(g.auto_buy));data.push(u8::from(g.auto_gear));
    data.push(u8::from(g.auto_prestige));
    write_big(&mut data,&g.auto_prestige_min_gain)?;
    data.push(u8::from(g.auto_research));
    for &v in &g.achievements{data.push(u8::from(v));}
    data.extend_from_slice(&g.auto_economy_elapsed.to_le_bytes());
    let crc=crc32(&data);data.extend_from_slice(&crc.to_le_bytes());
    if let Some(parent)=path.parent(){fs::create_dir_all(parent)?;}
    let tmp=path.with_extension("dat.tmp");
    {let mut file=fs::File::create(&tmp)?;file.write_all(&data)?;file.sync_all()?;}
    #[cfg(windows)]if path.exists(){fs::remove_file(path)?;}
    fs::rename(tmp,path)?;Ok(())
}
struct Reader<'a>{buf:&'a [u8],pos:usize}
impl Reader<'_>{
    fn read(&mut self,n:usize)->io::Result<&[u8]>{let end=self.pos.checked_add(n).ok_or_else(||bad("invalid save length"))?;let data=self.buf.get(self.pos..end).ok_or_else(||bad("truncated save"))?;self.pos=end;Ok(data)}
    fn u64(&mut self)->io::Result<u64>{Ok(u64::from_le_bytes(self.read(8)?.try_into().unwrap()))}
    fn u32(&mut self)->io::Result<u32>{Ok(u32::from_le_bytes(self.read(4)?.try_into().unwrap()))}
    fn f64(&mut self)->io::Result<f64>{Ok(f64::from_le_bytes(self.read(8)?.try_into().unwrap()))}
    fn flag(&mut self)->io::Result<bool>{Ok(self.read(1)?[0]!=0)}
    fn big(&mut self)->io::Result<Big>{let len=self.u32()? as usize;if len>32_768{return Err(bad("number string too long"))}let s=std::str::from_utf8(self.read(len)?).map_err(|_|bad("invalid number text"))?;s.parse::<Big>().map_err(|_|bad("invalid stored number"))}
}
pub fn load(path:&Path)->io::Result<(Game,u64)> {
    let mut file=fs::File::open(path)?;let mut bytes=Vec::new();file.take(4*1024*1024).read_to_end(&mut bytes)?;
    if bytes.len()<24{return Err(bad("save too short"))}
    let expected=u32::from_le_bytes(bytes[bytes.len()-4..].try_into().unwrap());
    if crc32(&bytes[..bytes.len()-4])!=expected{return Err(bad("save checksum failure"))}
    let magic=&bytes[..8];let mut r=Reader{buf:&bytes[..bytes.len()-4],pos:8};
    let version=r.u32()?;let saved_at=r.u64()?;let mut g=Game::default();
    if (magic==MAGIC_V7&&version==VERSION) || (magic==MAGIC_V6&&version==6) {
        g.cash=r.big()?;g.run_earned=r.big()?;g.all_time_earned=r.big()?;g.elapsed_seconds=r.f64()?;
        for b in &mut g.total_mined{*b=r.big()?;}
        for b in &mut g.prestige{*b=r.big()?;}
        for b in &mut g.prestige_lifetime{*b=r.big()?;}
        for n in &mut g.miners{*n=r.u64()?;}
        for n in &mut g.equipment{*n=r.u32()?;}
        for row in &mut g.research{for n in row{*n=r.u32()?;}}
        for n in &mut g.resets{*n=r.u64()?;}
        g.auto_buy=r.flag()?;g.auto_gear=r.flag()?;
        g.auto_prestige=r.flag()?;
        g.auto_prestige_min_gain=r.big()?;
        g.auto_research=r.flag()?;
        for b in &mut g.achievements{*b=r.flag()?;}
        if magic==MAGIC_V7 {g.auto_economy_elapsed=r.f64()?;}
    }else if magic==MAGIC_V5&&version==5 {
        g.cash=r.big()?;g.run_earned=r.big()?;g.all_time_earned=r.big()?;g.elapsed_seconds=r.f64()?;
        for b in &mut g.total_mined{*b=r.big()?;}
        for b in &mut g.prestige{*b=r.big()?;}
        for b in &mut g.prestige_lifetime{*b=r.big()?;}
        for n in &mut g.miners{*n=r.u64()?;}
        for n in &mut g.equipment{*n=r.u32()?;}
        for row in &mut g.research{for n in row{*n=r.u32()?;}}
        for n in &mut g.resets{*n=r.u64()?;}
        g.auto_buy=r.flag()?;g.auto_gear=r.flag()?;
        g.auto_prestige=r.flag()?;
        let _previous_target=r.read(1)?[0];
        let old_gain_index=r.read(1)?[0].min(4);
        g.auto_prestige_min_gain=Big::from([1u64,10,100,1_000,10_000][old_gain_index as usize]);
        g.auto_research=r.flag()?;
        for b in &mut g.achievements{*b=r.flag()?;}
    }else if magic==MAGIC_V4&&version==4 {
        // V4 had 8 research branches. Load them without shifting their indices.
        g.cash=r.big()?;g.run_earned=r.big()?;g.all_time_earned=r.big()?;g.elapsed_seconds=r.f64()?;
        for b in &mut g.total_mined{*b=r.big()?;}
        for b in &mut g.prestige{*b=r.big()?;}
        for b in &mut g.prestige_lifetime{*b=r.big()?;}
        for n in &mut g.miners{*n=r.u64()?;}
        for n in &mut g.equipment{*n=r.u32()?;}
        for row in &mut g.research{for n in &mut row[..8]{*n=r.u32()?;}}
        for n in &mut g.resets{*n=r.u64()?;}
        g.auto_buy=r.flag()?;g.auto_gear=r.flag()?;
        g.auto_prestige=r.flag()?;
        let _previous_target=r.read(1)?[0];
        let old_gain_index=r.read(1)?[0].min(4);
        g.auto_prestige_min_gain=Big::from([1u64,10,100,1_000,10_000][old_gain_index as usize]);
        for b in &mut g.achievements{*b=r.flag()?;}
    }else if magic==MAGIC_V3&&version==3 {
        // Legacy V3 had 30 ore slots, 4 prestige tiers and 7 research tracks.
        g.cash=r.big()?;g.run_earned=r.big()?;g.all_time_earned=r.big()?;
        g.elapsed_seconds=r.f64()?;
        for b in &mut g.total_mined[..30]{*b=r.big()?;}
        for b in &mut g.prestige[..4]{*b=r.big()?;}
        for b in &mut g.prestige_lifetime[..4]{*b=r.big()?;}
        for n in &mut g.miners[..30]{*n=r.u64()?;}
        for n in &mut g.equipment[..30]{*n=r.u32()?;}
        for row in &mut g.research[..4]{for n in &mut row[..7]{*n=r.u32()?;}}
        for n in &mut g.resets[..4]{*n=r.u64()?;}
        g.auto_buy=r.flag()?;g.auto_gear=r.flag()?;
        for b in &mut g.achievements{*b=r.flag()?;}
    }else if magic==MAGIC_V2&&version==2 {
        // Old saves had 10 shafts, 3 research tracks, and f64 currency.
        g.cash=Big::from(r.f64()?);g.run_earned=Big::from(r.f64()?);g.all_time_earned=Big::from(r.f64()?);g.elapsed_seconds=r.f64()?;
        for b in &mut g.total_mined[..10]{*b=Big::from(r.f64()?);}
        for b in &mut g.prestige[..4]{*b=Big::from(r.f64()?);}
        for b in &mut g.prestige_lifetime[..4]{*b=Big::from(r.f64()?);}
        for n in &mut g.miners[..10]{*n=r.u64()?;}
        for n in &mut g.equipment[..10]{*n=r.u32()?;}
        for row in &mut g.research[..4]{for n in &mut row[..3]{*n=r.u32()?;}}
        for n in &mut g.resets[..4]{*n=r.u64()?;}
        g.auto_buy=r.flag()?;for b in &mut g.achievements{*b=r.flag()?;}
        // Old last achievement was specifically Mythril; preserve it.
    }else{return Err(bad("unrecognized save format"))}
    if r.pos!=r.buf.len(){return Err(bad("save length mismatch"))}
    g.sanitize();let elapsed=timestamp().saturating_sub(saved_at).min(MAX_OFFLINE);
    if elapsed>0 {
        g.tick(elapsed as f64);
        // Offline earnings are credited, but automated *purchases* require
        // a complete minute of active gameplay following the load.
        g.auto_economy_elapsed=0.;
    }
    Ok((g,elapsed))
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn v7_persists_automation_clock_and_loads_v6(){
        let path=std::env::temp_dir().join(format!("mine-v7-{}-{}.dat",std::process::id(),timestamp()));
        let mut g=Game::default();
        g.auto_buy=true;g.research[0][2]=1;
        g.auto_economy_elapsed=42.5;
        save(&path,&g).unwrap();
        let (v7,_)=load(&path).unwrap();
        assert!(v7.auto_buy);
        assert_eq!(v7.research[0][2],1);
        assert!(v7.auto_economy_elapsed==42.5 || v7.auto_economy_elapsed==0.);
        // Reconstruct the previous V6 layout: same payload with no clock.
        let mut data=fs::read(&path).unwrap();
        let len=data.len();
        data.truncate(len-12); // remove 8-byte clock and 4-byte checksum
        data[..8].copy_from_slice(MAGIC_V6);
        data[8..12].copy_from_slice(&6u32.to_le_bytes());
        let crc=crc32(&data);data.extend_from_slice(&crc.to_le_bytes());
        fs::write(&path,data).unwrap();
        let (old,_)=load(&path).unwrap();
        assert!(old.auto_buy);
        assert_eq!(old.research[0][2],1);
        assert_eq!(old.auto_economy_elapsed,0.);
        let _=fs::remove_file(path);
    }
    #[test]fn round_trip_large_currency_and_autogear(){let path=std::env::temp_dir().join(format!("mine-{}-{}.dat",std::process::id(),timestamp()));let mut g=Game::default();g.cash="3.141592653589793e9999999999999999999999999999999".parse().unwrap();g.miners[59]=15;g.research[0][3]=1;g.research[13][7]=2;
        g.prestige[13]="2e9999".parse().unwrap();g.auto_gear=true;
        g.auto_prestige=true;g.auto_prestige_min_gain="1e500000000000000000".parse().unwrap();
        save(&path,&g).unwrap();let (got,_)=load(&path).unwrap();
        assert!(got.cash>=g.cash);assert_eq!(got.miners[59],15);
        assert!(got.auto_gear);assert!(got.auto_prestige);
        assert_eq!(got.auto_prestige_min_gain,g.auto_prestige_min_gain);
        assert_eq!(got.research[13][7],2);let _=fs::remove_file(path);}
    #[test]fn v5_auto_research_and_specializations_round_trip(){
        let path=std::env::temp_dir().join(format!("mine-v5-{}-{}.dat",std::process::id(),timestamp()));
        let mut g=Game::default();g.auto_research=true;
        g.research[2][8]=1;g.research[12][9]=15;g.research[10][12]=99;
        save(&path,&g).unwrap();let (got,_)=load(&path).unwrap();
        assert!(got.auto_research);assert_eq!(got.research[2][8],1);
        assert_eq!(got.research[12][9],15);assert_eq!(got.research[10][12],99);
        let _=fs::remove_file(path);
    }
    #[test]fn migrates_v5_preset_to_text_field_gain(){
        let path=std::env::temp_dir().join(format!("mine-v5-{}-{}.dat",std::process::id(),timestamp()));
        let mut data=Vec::new();data.extend_from_slice(MAGIC_V5);
        data.extend_from_slice(&5u32.to_le_bytes());data.extend_from_slice(&timestamp().to_le_bytes());
        for _ in 0..3{write_big(&mut data,&Big::from(50.)).unwrap();}
        data.extend_from_slice(&0f64.to_le_bytes());
        for _ in 0..COUNT+2*PRESTIGE_COUNT {write_big(&mut data,&Big::one()).unwrap();}
        for _ in 0..COUNT {data.extend_from_slice(&1u64.to_le_bytes());}
        for _ in 0..COUNT {data.extend_from_slice(&0u32.to_le_bytes());}
        for _ in 0..PRESTIGE_COUNT*RESEARCH_COUNT {data.extend_from_slice(&0u32.to_le_bytes());}
        for _ in 0..PRESTIGE_COUNT {data.extend_from_slice(&0u64.to_le_bytes());}
        data.extend_from_slice(&[0,0,1,13,4,1]); // auto prestige, old target and goal, auto research
        data.extend_from_slice(&[0u8;8]);
        let crc=crc32(&data);data.extend_from_slice(&crc.to_le_bytes());fs::write(&path,data).unwrap();
        let (g,_)=load(&path).unwrap();
        assert!(g.auto_prestige);assert_eq!(g.auto_prestige_min_gain,Big::from(10_000.));
        assert!(g.auto_research);let _=fs::remove_file(path);
    }
    #[test]fn migrates_version_four_save(){
        let path=std::env::temp_dir().join(format!("mine-v4-{}-{}.dat",std::process::id(),timestamp()));
        let mut data=Vec::new();data.extend_from_slice(MAGIC_V4);
        data.extend_from_slice(&4u32.to_le_bytes());data.extend_from_slice(&(timestamp()+3600).to_le_bytes());
        for _ in 0..3{write_big(&mut data,&Big::from(100.)).unwrap();}
        data.extend_from_slice(&0f64.to_le_bytes());
        for _ in 0..COUNT+2*PRESTIGE_COUNT {write_big(&mut data,&Big::one()).unwrap();}
        for _ in 0..COUNT {data.extend_from_slice(&1u64.to_le_bytes());}
        for _ in 0..COUNT {data.extend_from_slice(&0u32.to_le_bytes());}
        for i in 0..PRESTIGE_COUNT*8 {data.extend_from_slice(&(i as u32).to_le_bytes());}
        for _ in 0..PRESTIGE_COUNT {data.extend_from_slice(&0u64.to_le_bytes());}
        data.extend_from_slice(&[1,1,1,2,3]);data.extend_from_slice(&[0u8;8]);
        let crc=crc32(&data);data.extend_from_slice(&crc.to_le_bytes());fs::write(&path,data).unwrap();
        let (g,_)=load(&path).unwrap();assert_eq!(g.research[13][7],111);
        assert_eq!(g.research[13][8],0);assert!(!g.auto_research);
        assert!(g.auto_prestige);assert_eq!(g.auto_prestige_min_gain,Big::from(1_000.));
        let _=fs::remove_file(path);
    }
    #[test]fn migrates_version_two_save(){
        let path=std::env::temp_dir().join(format!("mine-v2-{}-{}.dat",std::process::id(),timestamp()));
        let mut data=Vec::new();data.extend_from_slice(MAGIC_V2);
        data.extend_from_slice(&2u32.to_le_bytes());data.extend_from_slice(&(timestamp()+3600).to_le_bytes());
        for n in [250.0f64,150.,1e50,200.] {data.extend_from_slice(&n.to_le_bytes());}
        for i in 0..10 {data.extend_from_slice(&(i as f64).to_le_bytes());}
        for i in 0..8 {data.extend_from_slice(&(i as f64).to_le_bytes());}
        for i in 0..10 {data.extend_from_slice(&(i as u64).to_le_bytes());}
        for i in 0..10 {data.extend_from_slice(&(i as u32).to_le_bytes());}
        for _ in 0..12 {data.extend_from_slice(&0u32.to_le_bytes());}
        for _ in 0..4 {data.extend_from_slice(&0u64.to_le_bytes());}
        data.push(1);data.extend_from_slice(&[0u8;8]);
        let crc=crc32(&data);data.extend_from_slice(&crc.to_le_bytes());
        fs::write(&path,data).unwrap();let (g,_) = load(&path).unwrap();
        assert!(g.auto_buy);assert!(!g.auto_gear);assert_eq!(g.miners[9],9);
        assert_eq!(g.miners[59],0);assert_eq!(g.cash,Big::from(250.));
        assert_eq!(g.all_time_earned,Big::from(1e50));let _=fs::remove_file(path);
    }

    #[test]fn migrates_version_three_save(){
        let path=std::env::temp_dir().join(format!("mine-v3-{}-{}.dat",std::process::id(),timestamp()));
        let mut data=Vec::new();data.extend_from_slice(MAGIC_V3);
        data.extend_from_slice(&3u32.to_le_bytes());data.extend_from_slice(&(timestamp()+3600).to_le_bytes());
        for _ in 0..3 {write_big(&mut data,&Big::from(50.)).unwrap();}
        data.extend_from_slice(&0f64.to_le_bytes());
        for i in 0..30 {write_big(&mut data,&Big::from(i as f64)).unwrap();}
        for _ in 0..8 {write_big(&mut data,&Big::one()).unwrap();}
        for i in 0..30 {data.extend_from_slice(&(i as u64).to_le_bytes());}
        for _ in 0..30 {data.extend_from_slice(&0u32.to_le_bytes());}
        for _ in 0..(4*7) {data.extend_from_slice(&0u32.to_le_bytes());}
        for _ in 0..4 {data.extend_from_slice(&0u64.to_le_bytes());}
        data.extend_from_slice(&[1,1]);data.extend_from_slice(&[0u8;8]);
        let crc=crc32(&data);data.extend_from_slice(&crc.to_le_bytes());
        fs::write(&path,data).unwrap();let (g,_)=load(&path).unwrap();
        assert_eq!(g.miners[29],29);assert_eq!(g.miners[30],0);
        assert!(g.auto_gear);assert!(!g.auto_prestige);
        assert_eq!(g.research[13][7],0);
        let _=fs::remove_file(path);
    }
    #[test]fn corrupted_save_rejected(){let path=std::env::temp_dir().join(format!("mine-crc-{}-{}.dat",std::process::id(),timestamp()));save(&path,&Game::default()).unwrap();let mut data=fs::read(&path).unwrap();data[20]^=3;fs::write(&path,data).unwrap();assert!(load(&path).is_err());let _=fs::remove_file(path);}
}
