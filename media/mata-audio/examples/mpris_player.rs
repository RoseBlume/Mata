//! cargo run --features mpris --example mpris_player -- /path/to/music.flac
#[cfg(target_os = "linux")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use mata_audio::{
        playback::AudioSource,
        media_controls::{MediaController, MediaMetadata, mpris::MprisSession},
    };
    let file = std::env::args().nth(1).ok_or("provide an audio file path")?;
    let source = AudioSource::open(&file, false)?;
    let name = std::path::Path::new(&file)
        .file_stem().and_then(|stem| stem.to_str()).unwrap_or("Audio");
    let controller = MediaController::new(source, MediaMetadata {
        title: name.to_owned(), ..Default::default()
    });
    let _session = MprisSession::new("mata", "Mata Audio", controller.clone())?;
    controller.play();
    println!("Publishing org.mpris.MediaPlayer2.mata; press Enter to quit");
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    controller.stop();
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("The MPRIS example is for Linux only.");
}
