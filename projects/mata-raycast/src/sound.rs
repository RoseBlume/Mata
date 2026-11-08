//! Fire-and-forget short audio cues through mata-audio.
use std::path::Path;
use mata_audio::AudioSource;

pub struct Sounds {voices:Vec<AudioSource>,enabled:bool}
impl Sounds {
    pub fn new()->Self{Self{voices:Vec::new(),enabled:true}}
    pub fn toggle(&mut self){self.enabled=!self.enabled;}
    pub fn play(&mut self,cue:&str){
        if !self.enabled{return}
        self.voices.retain(|v|{let s=v.state();!s.stopped && s.position_seconds<s.duration_seconds.unwrap_or(2.0)});
        let path=Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/sfx").join(format!("{cue}.wav"));
        if let Ok(source)=AudioSource::open(&path,true){source.set_volume(0.27);self.voices.push(source);}
        if self.voices.len()>8{self.voices.drain(..self.voices.len()-8);}
    }
}
