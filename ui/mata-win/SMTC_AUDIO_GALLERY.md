# Audio gallery: Windows System Media Transport Controls (SMTC)

The `audio_gallery` example uses MataWin's existing audio widget and native
`mata-audio` playback source. On Windows, the optional `smtc` feature connects
this source to Windows' system media controls; it does not use Android MediaSession.

From the Cargo workspace root, with the unified `mata-audio` crate installed
as a sibling of `mata-win`:

```powershell
cargo run -p mata-win --example audio_gallery --features smtc
```

Without the integration:

```powershell
cargo run -p mata-win --example audio_gallery
```

The existing `audio_player` example is also updated, and accepts the same feature.
Audio is discovered recursively under `<workspace>/assets_ignored/audio`.

## Expected behavior

- Play, Pause, Stop and seeking in Windows operate on the same streamed
  `AudioSource` as the MataWin buttons; Windows receives the current duration
  and position from `mata-audio`.
- Next and Previous are dispatched to the MataWin event loop via a channel.
  This is important because Windows may invoke SMTC handlers from non-UI threads.
- Track changes from MataWin buttons, Windows controls, or automatic playlist
  advance replace SMTC's source and metadata.
- Title is derived from the filename. `Artist - Title.ext` filenames populate
  artist and title separately; the containing directory is used as album.
- The SMTC session is dropped before the native window is destroyed. Failure
  to register SMTC prints a warning and playback continues without OS controls.
- Non-Windows builds do not compile or register SMTC and retain regular playback.

## Design notes

SMTC is opt-in: `smtc = ["audio", "mata-audio/smtc"]`.
This code requires a version of `mata-audio` which exports
`media_controls::{MediaController, MediaMetadata}` and
`media_controls::smtc::SmtcSession`.

Album artwork is left unset: the gallery doesn't yet read embedded cover art.
The Windows SMTC backend accepts artwork URLs when a reliable metadata source
is added later.

## Validate

```powershell
cargo check -p mata-win --example audio_gallery --features smtc
cargo check -p mata-win --example audio_player --features smtc
cargo check -p mata-win --example audio_gallery
```

Build/run results have not been verified here: this environment lacks Rust/Cargo.
