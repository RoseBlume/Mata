//! Optional native OS media sessions backed by mata-audio's `AudioSource`.
//!
//! `mpris` exposes a real D-Bus MPRIS service on Linux; `smtc` publishes
//! Windows System Media Transport Controls for an existing top-level HWND.
//! Media commands operate on the same audio source used by the shared mixer.
//! Creating a session is explicit: ordinary playback never requires a D-Bus
//! daemon, WinRT, or a running desktop environment.

use crate::playback::AudioSource;
use std::sync::{Arc, RwLock};

type Action = Arc<dyn Fn() + Send + Sync + 'static>;

/// Public metadata sent to the desktop. Album artwork must be a file:// URI
/// for MPRIS; other valid URI schemes may be used by supporting clients.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MediaMetadata {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub art_url: Option<String>,
    pub url: Option<String>,
    pub track_number: Option<i32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaState { Playing, Paused, Stopped }

#[derive(Clone)]
struct PlaylistCallbacks { next: Option<Action>, previous: Option<Action> }

/// Thread-safe bridge between OS transport controls and an actual AudioSource.
/// Update the active source and its metadata together when changing tracks.
#[derive(Clone)]
pub struct MediaController {
    inner: Arc<Inner>,
}

struct Inner {
    source: RwLock<AudioSource>,
    metadata: RwLock<MediaMetadata>,
    callbacks: RwLock<PlaylistCallbacks>,
}

impl MediaController {
    pub fn new(source: AudioSource, metadata: MediaMetadata) -> Self {
        Self { inner: Arc::new(Inner {
            source: RwLock::new(source),
            metadata: RwLock::new(metadata),
            callbacks: RwLock::new(PlaylistCallbacks { next: None, previous: None }),
        }) }
    }
    pub fn source(&self) -> AudioSource { self.inner.source.read().unwrap().clone() }
    pub fn metadata(&self) -> MediaMetadata { self.inner.metadata.read().unwrap().clone() }
    pub fn set_metadata(&self, metadata: MediaMetadata) { *self.inner.metadata.write().unwrap() = metadata; }
    pub fn replace_source(&self, source: AudioSource, metadata: MediaMetadata) {
        *self.inner.source.write().unwrap() = source;
        *self.inner.metadata.write().unwrap() = metadata;
    }
    pub fn set_next_handler(&self, handler: impl Fn() + Send + Sync + 'static) {
        self.inner.callbacks.write().unwrap().next = Some(Arc::new(handler));
    }
    pub fn set_previous_handler(&self, handler: impl Fn() + Send + Sync + 'static) {
        self.inner.callbacks.write().unwrap().previous = Some(Arc::new(handler));
    }
    pub fn can_next(&self) -> bool { self.inner.callbacks.read().unwrap().next.is_some() }
    pub fn can_previous(&self) -> bool { self.inner.callbacks.read().unwrap().previous.is_some() }
    pub fn next(&self) {
        let f = self.inner.callbacks.read().unwrap().next.clone();
        if let Some(f) = f { f(); }
    }
    pub fn previous(&self) {
        let f = self.inner.callbacks.read().unwrap().previous.clone();
        if let Some(f) = f { f(); }
    }
    pub fn play(&self) { self.source().play(); }
    pub fn pause(&self) { self.source().pause(); }
    pub fn stop(&self) { self.source().stop(); }
    pub fn toggle(&self) { self.source().toggle_playback(); }
    pub fn set_volume(&self, volume: f64) { self.source().set_volume(volume.max(0.0).min(1.0) as f32); }
    pub fn volume(&self) -> f64 { self.source().volume() as f64 }
    pub fn position(&self) -> f64 { self.source().position() }
    pub fn duration(&self) -> Option<f64> { self.source().duration() }
    pub fn seek(&self, seconds: f64) {
        if seconds.is_finite() && seconds >= 0.0 {
            let end = self.duration().unwrap_or(f64::MAX);
            self.source().seek(seconds.min(end));
        }
    }
    pub fn state(&self) -> MediaState {
        let s = self.source().state();
        if s.playing { MediaState::Playing }
        else if s.stopped { MediaState::Stopped }
        else { MediaState::Paused }
    }
    pub fn track_id(&self) -> u64 { self.source().id() }
}

#[cfg(all(feature = "mpris", target_os = "linux"))]
pub mod mpris;
#[cfg(all(feature = "smtc", target_os = "windows"))]
pub mod smtc;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metadata_defaults_are_empty() {
        let m = MediaMetadata::default();
        assert!(m.art_url.is_none());
        assert_eq!(m.title, "");
    }
}
