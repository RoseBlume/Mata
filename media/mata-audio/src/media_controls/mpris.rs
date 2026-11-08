//! Linux MPRIS2 service implemented on the session D-Bus.
//! Runs independently of audio callback / decoding threads.
use super::{MediaController, MediaMetadata, MediaState};
use std::{collections::HashMap, sync::{Arc, atomic::{AtomicBool, AtomicI64, Ordering}}, thread, time::Duration};
use zbus::{blocking::connection::Builder, zvariant::{ObjectPath, Value}};

const OBJECT_PATH: &str = "/org/mpris/MediaPlayer2";

fn micros(seconds: f64) -> i64 {
    if seconds.is_finite() && seconds > 0.0 {
        (seconds * 1_000_000.0).min(i64::MAX as f64) as i64
    } else { 0 }
}
fn track_path(id: u64) -> String { format!("/org/mpris/MediaPlayer2/Track/t{id}") }

struct Root { identity: String }

#[zbus::interface(name = "org.mpris.MediaPlayer2")]
impl Root {
    fn raise(&self) {}
    fn quit(&self) {}
    #[zbus(property)]
    fn can_quit(&self) -> bool { false }
    #[zbus(property)]
    fn can_raise(&self) -> bool { false }
    #[zbus(property)]
    fn has_track_list(&self) -> bool { false }
    #[zbus(property)]
    fn identity(&self) -> String { self.identity.clone() }
    #[zbus(property)]
    fn desktop_entry(&self) -> String { String::new() }
    #[zbus(property)]
    fn supported_uri_schemes(&self) -> Vec<String> { vec!["file".into()] }
    #[zbus(property)]
    fn supported_mime_types(&self) -> Vec<String> { Vec::new() }
    #[zbus(property)]
    fn fullscreen(&self) -> bool { false }
    #[zbus(property)]
    fn set_fullscreen(&self, _value: bool) {}
    #[zbus(property)]
    fn can_set_fullscreen(&self) -> bool { false }
}

struct Player { control: MediaController, pending_seek: Arc<AtomicI64> }

#[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
impl Player {
    fn next(&self) { self.control.next(); }
    fn previous(&self) { self.control.previous(); }
    fn pause(&self) { self.control.pause(); }
    fn play_pause(&self) { self.control.toggle(); }
    fn stop(&self) { self.control.stop(); }
    fn play(&self) { self.control.play(); }
    fn seek(&self, offset: i64) {
        let target = self.control.position() + offset as f64 / 1_000_000.0;
        self.control.seek(target.max(0.0));
        self.pending_seek.store(micros(target.max(0.0)), Ordering::Release);
    }
    fn set_position(&self, track_id: ObjectPath<'_>, position: i64) {
        if track_id.as_str() == track_path(self.control.track_id()) {
            self.control.seek((position.max(0)) as f64 / 1_000_000.0);
            self.pending_seek.store(position.max(0), Ordering::Release);
        }
    }
    fn open_uri(&self, _uri: &str) { /* No implicit file access. */ }
    #[zbus(property)]
    fn playback_status(&self) -> &'static str {
        match self.control.state() {
            MediaState::Playing => "Playing", MediaState::Paused => "Paused", MediaState::Stopped => "Stopped",
        }
    }
    #[zbus(property)]
    fn loop_status(&self) -> &'static str { if self.control.source().state().looping { "Track" } else { "None" } }
    #[zbus(property)]
    fn set_loop_status(&self, loop_status: &str) { self.control.source().set_looping(loop_status == "Track"); }
    #[zbus(property)]
    fn shuffle(&self) -> bool { false }
    #[zbus(property)]
    fn set_shuffle(&self, _enabled: bool) {}
    #[zbus(property)]
    fn rate(&self) -> f64 { 1.0 }
    #[zbus(property)]
    fn set_rate(&self, _rate: f64) {}
    #[zbus(property)]
    fn minimum_rate(&self) -> f64 { 1.0 }
    #[zbus(property)]
    fn maximum_rate(&self) -> f64 { 1.0 }
    #[zbus(property)]
    fn metadata(&self) -> HashMap<String, Value<'static>> {
        metadata_map(self.control.track_id(), &self.control.metadata(), self.control.duration())
    }
    #[zbus(property)]
    fn volume(&self) -> f64 { self.control.volume() }
    #[zbus(property)]
    fn set_volume(&self, volume: f64) { self.control.set_volume(volume); }
    #[zbus(property)]
    fn position(&self) -> i64 { micros(self.control.position()) }
    #[zbus(property)]
    fn can_go_next(&self) -> bool { self.control.can_next() }
    #[zbus(property)]
    fn can_go_previous(&self) -> bool { self.control.can_previous() }
    #[zbus(property)]
    fn can_play(&self) -> bool { true }
    #[zbus(property)]
    fn can_pause(&self) -> bool { true }
    #[zbus(property)]
    fn can_seek(&self) -> bool { self.control.duration().is_some() }
    #[zbus(property)]
    fn can_control(&self) -> bool { true }
}

fn metadata_map(id: u64, m: &MediaMetadata, duration: Option<f64>) -> HashMap<String, Value<'static>> {
    let mut map = HashMap::new();
    map.insert("mpris:trackid".into(), Value::new(ObjectPath::try_from(track_path(id)).expect("safe generated track path")));
    map.insert("xesam:title".into(), Value::new(m.title.clone()));
    if !m.artist.is_empty() { map.insert("xesam:artist".into(), Value::new(vec![m.artist.clone()])); }
    if !m.album.is_empty() { map.insert("xesam:album".into(), Value::new(m.album.clone())); }
    if let Some(art) = &m.art_url { map.insert("mpris:artUrl".into(), Value::new(art.clone())); }
    if let Some(url) = &m.url { map.insert("xesam:url".into(), Value::new(url.clone())); }
    if let Some(number) = m.track_number { map.insert("xesam:trackNumber".into(), Value::new(number)); }
    if let Some(length) = duration { map.insert("mpris:length".into(), Value::new(micros(length))); }
    map
}

/// Owns the registered DBus name. Dropping it unregisters the MPRIS service.
pub struct MprisSession {
    connection: zbus::blocking::Connection,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}

impl MprisSession {
    /// The name is org.mpris.MediaPlayer2.<instance>. `instance` must contain
    /// only ASCII letters, numbers or underscores and cannot start with a digit.
    pub fn new(instance: &str, identity: impl Into<String>, control: MediaController) -> zbus::Result<Self> {
        if instance.is_empty() || instance.starts_with(|c: char| c.is_ascii_digit()) ||
            !instance.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(zbus::Error::Failure("invalid MPRIS instance name".into()));
        }
        let name = format!("org.mpris.MediaPlayer2.{instance}");
        let pending_seek = Arc::new(AtomicI64::new(-1));
        let conn = Builder::session()?
            .name(name.as_str())?
            .serve_at(OBJECT_PATH, Root { identity: identity.into() })?
            .serve_at(OBJECT_PATH, Player { control: control.clone(), pending_seek: pending_seek.clone() })?
            .build()?;
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let worker_conn = conn.clone();
        let worker_seek = pending_seek.clone();
        let worker = thread::spawn(move || {
            // Poll only the UI-facing state; never touch the realtime audio callback.
            let mut previous: Option<(MediaState, u64, MediaMetadata, bool, bool, bool, u64, Option<u64>)> = None;
            while !stopped.load(Ordering::Acquire) {
                let snapshot = (
                    control.state(), control.track_id(), control.metadata(),
                    control.can_next(), control.can_previous(), control.source().state().looping,
                    control.volume().to_bits(), control.duration().map(f64::to_bits),
                );
                if previous.as_ref() != Some(&snapshot) {
                    let mut changed: HashMap<String, Value<'static>> = HashMap::new();
                    let status = match snapshot.0 { MediaState::Playing => "Playing", MediaState::Paused => "Paused", MediaState::Stopped => "Stopped" };
                    changed.insert("PlaybackStatus".into(), Value::new(status.to_string()));
                    changed.insert("Metadata".into(), Value::new(metadata_map(snapshot.1, &snapshot.2, control.duration())));
                    changed.insert("CanGoNext".into(), Value::new(snapshot.3));
                    changed.insert("CanGoPrevious".into(), Value::new(snapshot.4));
                    changed.insert("CanSeek".into(), Value::new(control.duration().is_some()));
                    changed.insert("LoopStatus".into(), Value::new((if snapshot.5 { "Track" } else { "None" }).to_string()));
                    changed.insert("Volume".into(), Value::new(control.volume()));
                    let _ = worker_conn.emit_signal(
                        None::<&str>, OBJECT_PATH, "org.freedesktop.DBus.Properties",
                        "PropertiesChanged", &(
                            "org.mpris.MediaPlayer2.Player", changed, Vec::<String>::new()
                        ),
                    );
                    previous = Some(snapshot);
                }
                let new_position = worker_seek.swap(-1, Ordering::AcqRel);
                if new_position >= 0 {
                    let _ = worker_conn.emit_signal(
                        None::<&str>, OBJECT_PATH, "org.mpris.MediaPlayer2.Player",
                        "Seeked", &(new_position,),
                    );
                }
                thread::sleep(Duration::from_millis(200));
            }
        });
        Ok(Self { connection: conn, stop, worker: Some(worker) })
    }
    pub fn connection(&self) -> &zbus::blocking::Connection { &self.connection }
}

impl Drop for MprisSession {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() { let _ = worker.join(); }
    }
}
