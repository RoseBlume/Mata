//! Native AudioToolbox output test on macOS and iOS (requires app host for iOS).
//! On macOS: cargo run --release --example apple_sine
#![cfg(any(target_os="macos",target_os="ios"))]
use mata_audio::audio_io::{SampleFormat,traits::{HostTrait,DeviceTrait,StreamTrait}};
fn main(){
    let host=mata_audio::audio_io::default_host();
    let device=host.default_output_device().expect("Apple output device");
    let config=device.default_output_config().expect("default output config");
    assert_eq!(config.sample_format(),SampleFormat::F32);
    let rate=config.sample_rate() as f32;
    let channels=config.channels() as usize;
    let mut phase=0.0f32;
    let stream=device.build_output_stream(config.config(),move |data:&mut [f32],_| {
        for frame in data.chunks_exact_mut(channels) {
            let sample=phase.sin()*0.12;
            phase=(phase+std::f32::consts::TAU*440.0/rate).rem_euclid(std::f32::consts::TAU);
            frame.fill(sample);
        }
    },|e|eprintln!("Apple audio callback error: {e}"),None).expect("AudioQueue output");
    stream.play().expect("start AudioQueue");
    std::thread::sleep(std::time::Duration::from_secs(2));
    stream.pause().expect("pause AudioQueue");
}
