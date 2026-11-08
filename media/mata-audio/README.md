# mata-audio — unified playback and decoders

`mata-audio` now contains **both** crates from the supplied archive:

- `mata_audio::audio_io`: lower-level host/device/stream I/O, including WASAPI on Windows, ALSA and optional PulseAudio on Linux, and the existing AAudio backend on Android. Previously published as `mata-playback`.
- `mata_audio::playback`: higher-level decoder, shared mixer, `AudioSource`, seeking and playback telemetry.
- `mata_audio::media_controls`: optional native Linux MPRIS 2 and Windows SMTC integration using the existing `AudioSource`. **Android MediaSession is deliberately not included.**

No external `mata-playback` Rust dependency, path dependency or second package is needed. The former low-level APIs now live in `mata_audio::audio_io`; downstream imports should be updated accordingly.

## Feature flags

| Features | Purpose |
| --- | --- |
| `default` | Full decoding + native I/O, ALSA/PulseAudio or WASAPI/AAudio, real-time tuning |
| `--no-default-features --features all-codecs,all-demuxers` | Decoder-only: no sound host or media desktop dependencies |
| `native-playback` | Built-in low-level audio I/O and the high-level `playback` module |
| `native-pulseaudio` | Optional PulseAudio host on Linux |
| `native-realtime` | Optional native backend real-time thread tuning |
| `cpal` | Alternative CPAL backend; to prefer CPAL use `--no-default-features --features cpal` |
| `mpris` | Linux desktop MPRIS2 service over D-Bus (zbus) |
| `smtc` | Windows 10/11 SMTC, through the existing `windows` crate |
| `mata-playback`, `pulseaudio`, `realtime` | Compatibility aliases for feature spellings (not separate crates) |

MPRIS and SMTC are **opt in**; they do not activate when ordinary playback begins. On unsupported platforms their respective modules are absent. If you enable `smtc` on Linux or `mpris` on Windows, the feature has no OS media-control implementation on that platform.

## Linux MPRIS2

Enable `mpris` and keep a `MprisSession` in scope while the application runs:

```rust,no_run
use mata_audio::{
    playback::AudioSource,
    media_controls::{MediaController, MediaMetadata, mpris::MprisSession},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let song = AudioSource::open("song.flac", false)?;
    let ctrl = MediaController::new(song, MediaMetadata {
        title: "Example Song".into(), artist: "Example Artist".into(),
        ..Default::default()
    });
    // To support Next/Previous, install callbacks which replace the active
    // source using ctrl.replace_source(next_source, next_metadata).
    let _session = MprisSession::new("mata", "Mata Music", ctrl.clone())?;
    ctrl.play();
    std::thread::park(); // Replace with your application's event loop.
}
```

The `org.mpris.MediaPlayer2.mata` service exports both `org.mpris.MediaPlayer2` and `org.mpris.MediaPlayer2.Player` on `/org/mpris/MediaPlayer2`. Play, pause, toggle, stop, position, relative seeking, absolute seeking, loop mode, volume, album metadata, artwork URIs, and optional next/previous commands are wired to `AudioSource`. Playlist callbacks need to be supplied by the application; no automatic playlist is invented. Properties-change signals are emitted during background state snapshots, with an explicit `Seeked` signal for D-Bus seek commands.

Requires an active session D-Bus. It cannot be started when no session bus exists; regular audio playback still works.

## Windows SMTC

Enable `smtc`; pass your **existing top-level HWND** as an `isize`. The HWND must belong to the current process and outlive the session. Initialize COM/WinRT on the window-owning thread using the host UI framework:

```rust,ignore
use mata_audio::{
    playback::AudioSource,
    media_controls::{MediaController, MediaMetadata, smtc::SmtcSession},
};

let source = AudioSource::open("song.wav", false)?;
let control = MediaController::new(source, MediaMetadata {
    title: "Song".into(), artist: "Artist".into(),
    ..Default::default()
});
// hwnd comes from your existing Windows / mata-win window.
let _session = unsafe { SmtcSession::for_window(hwnd, control.clone())? };
control.play();
```

The Windows backend uses `windows::core::factory` + `ISystemMediaTransportControlsInterop::GetForWindow` rather than `GetForCurrentView()` (which is not generally usable for classic Win32 desktop windows). It registers system play/pause/stop/next/previous, responds to timeline seeking, updates metadata, cover art URI and timeline/status, and unregisters its events on drop. Callbacks only queue commands into the existing mixer; they do not decode audio.

**Windows integration requires a real top-level HWND.** `mata-audio` deliberately does not create its own fake window or install a global hook.

## Example commands

```sh
cargo check -p mata-audio --no-default-features --features all-codecs,all-demuxers
cargo check -p mata-audio
cargo check -p mata-audio --features mpris     # Linux
cargo check -p mata-audio --features smtc      # Windows
cargo check -p mata-audio --no-default-features --features cpal
cargo run -p mata-audio --example native_beep
```

`AudioSource` uses the process-global mixer. For high-level playback see `src/playback.rs`; for device enumeration, capture, and raw streams see `audio_io`. Android AAudio remains supported, but **no Android MediaSession code has been added**.

## Verification status

The source was merged and checked for missing modules and Cargo manifest syntax in an environment without `rustc` / `cargo`. It **has not been compiled** on Linux, Windows or Android in this environment. Run the above Cargo checks on each target before shipping.
