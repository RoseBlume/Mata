//! Small synthesized WAV stingers played through mata-audio. No sound assets required.
use std::{fs, io::Write, path::PathBuf};
use mata_audio::AudioSource;

pub struct Sounds {
    enabled:bool,
    clips: Vec<AudioSource>,
    sounds:[PathBuf;3],
}
impl Sounds {
    pub fn new(cache:&std::path::Path)->Self {
        let sounds= [cache.join("buy.wav"),cache.join("prestige.wav"),cache.join("upgrade.wav")];
        let notes=[(580.,780.,0.10),(330.,880.,0.36),(650.,920.,0.18)];
        for (path,(start,end,duration)) in sounds.iter().zip(notes) {
            if !path.is_file() {let _=write_tone(path,start,end,duration);}
        }
        Self{enabled:true,clips:Vec::new(),sounds}
    }
    pub fn enabled(&self)->bool {self.enabled}
    pub fn toggle(&mut self){self.enabled=!self.enabled;}
    pub fn play(&mut self,kind:usize){
        if !self.enabled{return;}
        self.clips.retain(|c| {let s=c.state();!s.stopped && s.position_seconds<s.duration_seconds.unwrap_or(3.)});
        if let Some(file)=self.sounds.get(kind){
            if let Ok(source)=AudioSource::open(file,true){source.set_volume(0.22);self.clips.push(source);}
        }
        if self.clips.len()>8{self.clips.drain(..self.clips.len()-8);}
    }
}
fn write_tone(path:&std::path::Path,start:f64,end:f64,duration:f64)->std::io::Result<()> {
    if let Some(parent)=path.parent(){fs::create_dir_all(parent)?;}
    let sample_rate=22_050u32;let count=(duration*sample_rate as f64) as u32;
    let bytes=count*2;
    let mut file=fs::File::create(path)?;
    file.write_all(b"RIFF")?;file.write_all(&(36+bytes).to_le_bytes())?;
    file.write_all(b"WAVEfmt ")?;file.write_all(&16u32.to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?;file.write_all(&1u16.to_le_bytes())?;
    file.write_all(&sample_rate.to_le_bytes())?;file.write_all(&(sample_rate*2).to_le_bytes())?;
    file.write_all(&2u16.to_le_bytes())?;file.write_all(&16u16.to_le_bytes())?;
    file.write_all(b"data")?;file.write_all(&bytes.to_le_bytes())?;
    for n in 0..count {
        let t=n as f64/sample_rate as f64;
        let alpha=t/duration;
        let frequency=start+(end-start)*alpha;
        let attack=(t/0.015).min(1.0);
        let decay=(1.0-alpha).max(0.0).powf(2.0);
        let sample=((std::f64::consts::TAU*frequency*t).sin()*attack*decay*9_000.0) as i16;
        file.write_all(&sample.to_le_bytes())?;
    }
    Ok(())
}
