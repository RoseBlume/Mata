//! Procedural effects streamed via mata-audio's native audio_io module.
//! Direct dependencies are first-party Mata crates only.
//!
//! The audio callback only consumes preallocated voices and a bounded queue.
//! No assets, decoding, file I/O, or per-sample heap allocations.
use std::f32::consts::TAU;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundCue {
    Carbine, Cannon, Sniper, Shotgun, Minigun, Hit, BotDown, BotShot,
    Reload, ReloadDone, Jump, Dash, Step,
    Hurt, ShieldHit, Health, Shield, Ammo,
    WaveStart, WaveClear, Victory, Defeat, Menu,
}

#[derive(Clone, Copy)]
enum Wave { Sine, Triangle, Noise }

#[derive(Clone, Copy)]
struct Voice {
    wave: Wave,
    phase: f32,
    age: f32,
    duration: f32,
    start_hz: f32,
    end_hz: f32,
    gain: f32,
    noise: u32,
}

impl Voice {
    fn sample(&mut self, sample_rate: f32) -> f32 {
        let t = (self.age / self.duration).min(1.0);
        let frequency = self.start_hz + (self.end_hz - self.start_hz) * t;
        self.phase = (self.phase + frequency / sample_rate).fract();
        self.age += 1.0 / sample_rate;
        // Quick attack prevents abrupt clicks; squared decay has a soft tail.
        let attack = (self.age * 350.0).min(1.0);
        let env = (1.0 - t).max(0.0);
        let waveform = match self.wave {
            Wave::Sine => (self.phase * TAU).sin(),
            Wave::Triangle => 1.0 - 4.0 * (self.phase - 0.5).abs(),
            Wave::Noise => {
                self.noise ^= self.noise << 13;
                self.noise ^= self.noise >> 17;
                self.noise ^= self.noise << 5;
                ((self.noise >> 8) as f32 / 8_388_607.5) - 1.0
            }
        };
        waveform * self.gain * attack * env * env
    }
}

struct Mixer {
    sample_rate: f32,
    voices: Vec<Voice>,
    noise_seed: u32,
    hum_phase: f32,
}

impl Mixer {
    fn new(sample_rate: f32) -> Self {
        Self { sample_rate, voices: Vec::with_capacity(32), noise_seed: 0x6e65_6f6e, hum_phase: 0.0 }
    }

    fn add(&mut self, wave: Wave, from: f32, to: f32, length: f32, gain: f32) {
        const MAX_VOICES: usize = 32;
        if self.voices.len() == MAX_VOICES {
            self.voices.swap_remove(0);
        }
        self.noise_seed = self.noise_seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.voices.push(Voice {
            wave, phase: 0.0, age: 0.0,
            duration: length, start_hz: from, end_hz: to, gain,
            noise: self.noise_seed | 1,
        });
    }

    fn cue(&mut self, cue: SoundCue) {
        use SoundCue::*;
        match cue {
            Carbine => {
                self.add(Wave::Triangle, 1050.0, 260.0, 0.115, 0.23);
                self.add(Wave::Noise, 650.0, 110.0, 0.045, 0.075);
            }
            Cannon => {
                self.add(Wave::Sine, 330.0, 65.0, 0.32, 0.34);
                self.add(Wave::Noise, 280.0, 80.0, 0.16, 0.14);
            }
            Sniper => {
                self.add(Wave::Triangle, 930.0, 120.0, 0.31, 0.32);
                self.add(Wave::Noise, 660.0, 95.0, 0.26, 0.19);
            }
            Shotgun => {
                self.add(Wave::Noise, 410.0, 65.0, 0.29, 0.26);
                self.add(Wave::Sine, 180.0, 70.0, 0.34, 0.19);
            }
            Minigun => {
                self.add(Wave::Triangle, 390.0, 125.0, 0.075, 0.105);
                self.add(Wave::Noise, 310.0, 165.0, 0.045, 0.06);
            }
            Hit => self.add(Wave::Sine, 1100.0, 740.0, 0.09, 0.18),
            BotDown => {
                self.add(Wave::Noise, 530.0, 80.0, 0.23, 0.13);
                self.add(Wave::Triangle, 470.0, 140.0, 0.29, 0.19);
            }
            BotShot => self.add(Wave::Triangle, 420.0, 140.0, 0.12, 0.075),
            Reload => self.add(Wave::Noise, 900.0, 150.0, 0.10, 0.09),
            ReloadDone => self.add(Wave::Triangle, 620.0, 940.0, 0.13, 0.11),
            Jump => self.add(Wave::Sine, 210.0, 330.0, 0.12, 0.12),
            Dash => self.add(Wave::Noise, 450.0, 110.0, 0.22, 0.11),
            Step => self.add(Wave::Noise, 90.0, 50.0, 0.075, 0.042),
            Hurt => {
                self.add(Wave::Triangle, 260.0, 95.0, 0.22, 0.17);
                self.add(Wave::Noise, 100.0, 50.0, 0.11, 0.08);
            }
            ShieldHit => self.add(Wave::Sine, 960.0, 330.0, 0.16, 0.14),
            Health => {
                self.add(Wave::Sine, 470.0, 720.0, 0.25, 0.16);
                self.add(Wave::Sine, 700.0, 1040.0, 0.28, 0.075);
            }
            Shield => self.add(Wave::Triangle, 510.0, 920.0, 0.24, 0.13),
            Ammo => self.add(Wave::Sine, 630.0, 800.0, 0.16, 0.13),
            WaveStart => {
                self.add(Wave::Sine, 260.0, 440.0, 0.48, 0.17);
                self.add(Wave::Triangle, 390.0, 620.0, 0.48, 0.075);
            }
            WaveClear => self.add(Wave::Sine, 640.0, 960.0, 0.36, 0.18),
            Victory => {
                self.add(Wave::Sine, 440.0, 880.0, 0.80, 0.22);
                self.add(Wave::Triangle, 660.0, 990.0, 0.83, 0.11);
            }
            Defeat => self.add(Wave::Triangle, 340.0, 70.0, 0.85, 0.24),
            Menu => self.add(Wave::Sine, 560.0, 640.0, 0.09, 0.095),
        }
    }

    fn sample(&mut self, active: bool) -> f32 {
        let mut value = 0.0;
        for v in &mut self.voices {
            value += v.sample(self.sample_rate);
        }
        self.voices.retain(|v| v.age < v.duration);
        if active {
            // Quiet low-frequency synth drone, audible on start even if idle.
            self.hum_phase = (self.hum_phase + 58.0 / self.sample_rate).fract();
            value += (self.hum_phase * TAU).sin() * 0.008;
        }
        value.clamp(-0.9, 0.9)
    }
}

#[cfg(feature = "audio")]
mod output {
    use super::{Mixer, SoundCue};
    // mata-audio owns the audio backend. The game never depends on CPAL.
    use mata_audio::audio_io::{self, Device, SampleFormat, Stream, StreamConfig};
    use mata_audio::audio_io::traits::{DeviceTrait, HostTrait, StreamTrait};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, SyncSender, sync_channel},
    };

    pub struct Audio {
        _stream: Stream,
        send: SyncSender<SoundCue>,
        active: Arc<AtomicBool>,
        muted: Arc<AtomicBool>,
    }

    trait PcmSample: audio_io::SizedSample + Send + 'static {
        fn from_float(x: f32) -> Self;
    }
    impl PcmSample for f32 { fn from_float(x: f32) -> Self { x } }
    impl PcmSample for i16 { fn from_float(x: f32) -> Self { (x * 32767.0) as i16 } }
    impl PcmSample for u16 { fn from_float(x: f32) -> Self { ((x + 1.0) * 32767.5) as u16 } }

    fn build<T: PcmSample>(
        device: &Device,
        config: StreamConfig,
        rx: Receiver<SoundCue>,
        active: Arc<AtomicBool>,
        muted: Arc<AtomicBool>,
    ) -> Result<Stream, String> {
        let channels = config.channels as usize;
        let mut mixer = Mixer::new(config.sample_rate as f32);
        device.build_output_stream(
            config,
            move |buffer: &mut [T], _| {
                // No callback locks, file I/O, or dynamic buffer allocations.
                while let Ok(event) = rx.try_recv() { mixer.cue(event); }
                let play_ambience = active.load(Ordering::Relaxed);
                let silent = muted.load(Ordering::Relaxed);
                for frame in buffer.chunks_mut(channels) {
                    let sample = if silent { mixer.sample(false) * 0.0 }
                                 else { mixer.sample(play_ambience) };
                    for destination in frame { *destination = T::from_float(sample); }
                }
            },
            |err| eprintln!("[Neon Run audio] output stream error: {err}"),
            None,
        ).map_err(|err| format!("unable to open output stream: {err}"))
    }

    impl Audio {
        pub fn new() -> Result<Self, String> {
            let host = audio_io::default_host();
            let device = host.default_output_device()
                .ok_or("no default audio output device found")?;
            let format = device.default_output_config()
                .map_err(|err| format!("could not query audio device: {err}"))?;
            let sample_format = format.sample_format();
            let config = format.config();
            let rate = config.sample_rate;
            let channels = config.channels;
            let (send, rx) = sync_channel(128);
            let active = Arc::new(AtomicBool::new(true));
            let muted = Arc::new(AtomicBool::new(false));
            let stream = match sample_format {
                SampleFormat::F32 => build::<f32>(&device, config, rx, active.clone(), muted.clone()),
                SampleFormat::I16 => build::<i16>(&device, config, rx, active.clone(), muted.clone()),
                SampleFormat::U16 => build::<u16>(&device, config, rx, active.clone(), muted.clone()),
                other => return Err(format!("unsupported default PCM format {other:?}")),
            }?;
            // Start the stream explicitly on all supported backends.
            stream.play().map_err(|err| format!("could not start audio: {err}"))?;
            eprintln!("[Neon Run audio] active: {rate} Hz, {channels} channels, {sample_format:?}");
            Ok(Self { _stream: stream, send, active, muted })
        }
        pub fn cue(&self, event: SoundCue) {
            let _ = self.send.try_send(event);
        }
        pub fn set_ambience(&self, on: bool) {
            self.active.store(on, Ordering::Relaxed);
        }
        pub fn mute(&self, muted: bool) {
            self.muted.store(muted, Ordering::Relaxed);
        }
    }
}

#[cfg(feature = "audio")]
pub use output::Audio;

#[cfg(not(feature = "audio"))]
pub struct Audio;
#[cfg(not(feature = "audio"))]
impl Audio {
    pub fn new() -> Result<Self, String> { Err("audio was disabled at compile time".into()) }
    pub fn cue(&self, _: SoundCue) {}
    pub fn set_ambience(&self, _: bool) {}
    pub fn mute(&self, _: bool) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_events_generate_samples_and_finish() {
        use SoundCue::*;
        for cue in [Carbine,Cannon,Hit,BotDown,BotShot,Reload,ReloadDone,
                    Jump,Dash,Step,Hurt,ShieldHit,Health,Shield,Ammo,
                    WaveStart,WaveClear,Victory,Defeat,Menu] {
            let mut mixer = Mixer::new(44_100.0);
            mixer.cue(cue);
            let mut sound = false;
            for _ in 0..44_100 {
                let v = mixer.sample(false);
                assert!(v.is_finite());
                sound |= v.abs() > 0.0001;
            }
            assert!(sound, "silent cue: {cue:?}");
            assert!(mixer.voices.is_empty(), "voice did not expire: {cue:?}");
        }
    }
    #[test]
    fn polyphony_is_bounded() {
        let mut mixer = Mixer::new(48_000.0);
        for _ in 0..100 { mixer.cue(SoundCue::Cannon); }
        assert!(mixer.voices.len() <= 32);
    }
}
