//! Dependency-free WASM browser audio via the Web Audio API.
//!
//! Compile `mata-audio` for `wasm32-unknown-unknown` with
//! `--no-default-features --features web-audio`. Host imports are implemented
//! by `wasm/host/mata-web.js`. Playback uses native AudioBufferSourceNode
//! scheduling; recording uses `getUserMedia` and AudioWorklet when permitted.
//! Audio operations must be initiated after a browser user gesture.

#[link(wasm_import_module="env")]
unsafe extern "C" {
    fn mata_web_audio_unlock() -> i32;
    fn mata_web_audio_play(data:*const f32, len:u32, channels:u32, rate:u32, volume:f32) -> u32;
    fn mata_web_audio_stop(id:u32);
    fn mata_web_audio_volume(id:u32, volume:f32);
    fn mata_web_audio_stream_new(channels:u32, rate:u32, volume:f32) -> u32;
    fn mata_web_audio_stream_write(id:u32, data:*const f32, len:u32) -> i32;
    fn mata_web_audio_stream_pause(id:u32, paused:i32);
    fn mata_web_audio_stream_close(id:u32);
    fn mata_web_audio_microphone_new() -> u32;
    fn mata_web_audio_microphone_status(id:u32) -> i32;
    fn mata_web_audio_microphone_read(id:u32, data:*mut f32, len:u32) -> u32;
    fn mata_web_audio_microphone_close(id:u32);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioError { InvalidFormat, BrowserUnavailable, BufferTooLarge }

fn valid(samples: &[f32], channels:u32, rate:u32)->Result<u32,AudioError> {
    if !(channels==1 || channels==2) || !(8_000..=192_000).contains(&rate) || samples.len()%channels as usize!=0 {
        return Err(AudioError::InvalidFormat);
    }
    u32::try_from(samples.len()).map_err(|_|AudioError::BufferTooLarge)
}
/// Call in direct response to a click, key press, or touch gesture.
/// Browser policies may prevent playback before it is called.
pub fn unlock_audio()->bool {unsafe{mata_web_audio_unlock()!=0}}

/// A one-shot PCM sound. Dropping the handle leaves the sound playing;
/// use `stop()` when you want to interrupt it.
pub struct Sound {id:u32}
impl Sound {
    pub fn play_f32(samples:&[f32],channels:u32,rate:u32,volume:f32)->Result<Self,AudioError> {
        let len=valid(samples,channels,rate)?;
        let id=unsafe{mata_web_audio_play(samples.as_ptr(),len,channels,rate,volume.clamp(0.0,4.0))};
        if id==0 {Err(AudioError::BrowserUnavailable)} else {Ok(Self{id})}
    }
    pub fn stop(&self){unsafe{mata_web_audio_stop(self.id);}}
    pub fn set_volume(&self,value:f32){unsafe{mata_web_audio_volume(self.id,value.clamp(0.0,4.0));}}
}

/// Queues interleaved F32 PCM chunks for continuous browser playback.
/// Call `write_f32` regularly to avoid underruns. Web Audio performs the
/// sample-rate conversion when required.
pub struct AudioStream {id:u32, channels:u32, sample_rate:u32}
impl AudioStream {
    pub fn new(channels:u32,sample_rate:u32,volume:f32)->Result<Self,AudioError> {
        valid(&[],channels,sample_rate)?;
        let id=unsafe{mata_web_audio_stream_new(channels,sample_rate,volume.clamp(0.0,4.0))};
        if id==0{Err(AudioError::BrowserUnavailable)}else{Ok(Self{id,channels,sample_rate})}
    }
    pub fn write_f32(&mut self,samples:&[f32])->Result<(),AudioError> {
        let len=valid(samples,self.channels,self.sample_rate)?;
        if unsafe{mata_web_audio_stream_write(self.id,samples.as_ptr(),len)}==0 {Err(AudioError::BrowserUnavailable)} else {Ok(())}
    }
    pub fn pause(&self){unsafe{mata_web_audio_stream_pause(self.id,1);}}
    pub fn resume(&self){unsafe{mata_web_audio_stream_pause(self.id,0);}}
}
impl Drop for AudioStream {fn drop(&mut self){unsafe{mata_web_audio_stream_close(self.id);}}}

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum MicrophoneStatus { Pending, Ready, Denied }
/// Requests microphone access asynchronously. Requires HTTPS or localhost,
/// user consent, and browser AudioWorklet support. Calls to `read` return
/// mono, normalized samples collected from the AudioWorklet.
pub struct Microphone {id:u32}
impl Microphone {
    pub fn request()->Result<Self,AudioError> {
        let id=unsafe{mata_web_audio_microphone_new()};
        if id==0 {Err(AudioError::BrowserUnavailable)} else {Ok(Self{id})}
    }
    pub fn status(&self)->MicrophoneStatus {
        match unsafe{mata_web_audio_microphone_status(self.id)} {1=>MicrophoneStatus::Ready,-1=>MicrophoneStatus::Denied,_=>MicrophoneStatus::Pending}
    }
    pub fn read(&mut self,destination:&mut[f32])->usize {
        let n=destination.len().min(u32::MAX as usize) as u32;
        unsafe{mata_web_audio_microphone_read(self.id,destination.as_mut_ptr(),n) as usize}
    }
}
impl Drop for Microphone {fn drop(&mut self){unsafe{mata_web_audio_microphone_close(self.id);}}}
