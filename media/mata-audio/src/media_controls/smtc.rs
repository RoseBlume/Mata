//! Windows 10/11 SMTC integration for ordinary Win32 desktop HWNDs.
//! Caller must supply an existing top-level window belonging to this process.
use super::{MediaController, MediaMetadata, MediaState};
use std::{sync::{Arc, atomic::{AtomicBool, Ordering}}, thread, time::Duration};
use windows::{
    core::{HSTRING, factory},
    Foundation::{TimeSpan, TypedEventHandler, Uri},
    Media::{
        MediaPlaybackStatus, MediaPlaybackType, SystemMediaTransportControls,
        SystemMediaTransportControlsButton, SystemMediaTransportControlsButtonPressedEventArgs,
        PlaybackPositionChangeRequestedEventArgs, SystemMediaTransportControlsTimelineProperties,
    },
    Storage::Streams::RandomAccessStreamReference,
    Win32::{Foundation::HWND, System::{WinRT::ISystemMediaTransportControlsInterop, Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED}}},
};

fn time_span(seconds: f64) -> TimeSpan {
    let ticks = if seconds.is_finite() && seconds > 0.0 {
        (seconds * 10_000_000.0).min(i64::MAX as f64) as i64
    } else { 0 };
    TimeSpan { Duration: ticks }
}

fn update_timeline(smtc: &SystemMediaTransportControls, control: &MediaController) -> windows::core::Result<()> {
    let timeline = SystemMediaTransportControlsTimelineProperties::new()?;
    timeline.SetStartTime(TimeSpan { Duration: 0 })?;
    timeline.SetPosition(time_span(control.position()))?;
    timeline.SetEndTime(time_span(control.duration().unwrap_or(0.0)))?;
    smtc.UpdateTimelineProperties(&timeline)
}

fn update_metadata(smtc: &SystemMediaTransportControls, metadata: &MediaMetadata) -> windows::core::Result<()> {
    let updater = smtc.DisplayUpdater()?;
    updater.ClearAll()?;
    updater.SetType(MediaPlaybackType::Music)?;
    let music = updater.MusicProperties()?;
    music.SetTitle(&HSTRING::from(metadata.title.as_str()))?;
    music.SetArtist(&HSTRING::from(metadata.artist.as_str()))?;
    music.SetAlbumTitle(&HSTRING::from(metadata.album.as_str()))?;
    if let Some(url) = &metadata.art_url {
        let uri = Uri::CreateUri(&HSTRING::from(url))?;
        updater.SetThumbnail(&RandomAccessStreamReference::CreateFromUri(&uri)?)?;
    }
    updater.Update()?;
    Ok(())
}

fn sync_controls(smtc: &SystemMediaTransportControls, control: &MediaController) -> windows::core::Result<()> {
    smtc.SetPlaybackStatus(match control.state() {
        MediaState::Playing => MediaPlaybackStatus::Playing,
        MediaState::Paused => MediaPlaybackStatus::Paused,
        MediaState::Stopped => MediaPlaybackStatus::Stopped,
    })?;
    smtc.SetIsPlayEnabled(true)?;
    smtc.SetIsPauseEnabled(true)?;
    smtc.SetIsStopEnabled(true)?;
    smtc.SetIsNextEnabled(control.can_next())?;
    smtc.SetIsPreviousEnabled(control.can_previous())?;
    update_timeline(smtc, control)
}

/// RAII handle keeping SMTC registration and event subscriptions active.
/// Dropping it unregisters the handlers and disables the system controls.
pub struct SmtcSession {
    smtc: SystemMediaTransportControls,
    button_token: i64,
    position_token: i64,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}

impl SmtcSession {
    /// # Safety
    /// `hwnd` must be a valid, living top-level HWND owned by this process.
    /// Keep the corresponding window alive until this session is dropped.
    pub unsafe fn for_window(hwnd: isize, controller: MediaController) -> windows::core::Result<Self> {
        let factory: ISystemMediaTransportControlsInterop =
            factory::<SystemMediaTransportControls, ISystemMediaTransportControlsInterop>()?;
        let smtc: SystemMediaTransportControls = unsafe {
            factory.GetForWindow(HWND(hwnd as *mut _))?
        };
        smtc.SetIsEnabled(true)?;
        update_metadata(&smtc, &controller.metadata())?;
        sync_controls(&smtc, &controller)?;

        let buttons_control = controller.clone();
        let button_token = smtc.ButtonPressed(&TypedEventHandler::new(
            move |_, event: windows::core::Ref<'_, SystemMediaTransportControlsButtonPressedEventArgs>| {
                if let Some(e) = event.as_ref() {
                    match e.Button()? {
                        SystemMediaTransportControlsButton::Play => buttons_control.play(),
                        SystemMediaTransportControlsButton::Pause => buttons_control.pause(),
                        SystemMediaTransportControlsButton::Stop => buttons_control.stop(),
                        SystemMediaTransportControlsButton::Next => buttons_control.next(),
                        SystemMediaTransportControlsButton::Previous => buttons_control.previous(),
                        _ => {},
                    }
                }
                Ok(())
            },
        ))?;

        let seek_control = controller.clone();
        let position_token = match smtc.PlaybackPositionChangeRequested(&TypedEventHandler::new(
            move |_, event: windows::core::Ref<'_, PlaybackPositionChangeRequestedEventArgs>| {
                if let Some(e) = event.as_ref() {
                    seek_control.seek(e.RequestedPlaybackPosition()?.Duration as f64 / 10_000_000.0);
                }
                Ok(())
            },
        )) {
            Ok(token) => token,
            Err(e) => { let _ = smtc.RemoveButtonPressed(button_token); let _ = smtc.SetIsEnabled(false); return Err(e); }
        };

        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let worker_smtc = smtc.clone();
        let worker = thread::spawn(move || {
            let com_ready = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).is_ok() };
            let mut last_metadata = None;
            while !stopped.load(Ordering::Acquire) {
                let metadata = controller.metadata();
                if last_metadata.as_ref() != Some(&metadata) {
                    let _ = update_metadata(&worker_smtc, &metadata);
                    last_metadata = Some(metadata);
                }
                let _ = sync_controls(&worker_smtc, &controller);
                thread::sleep(Duration::from_millis(250));
            }
            if com_ready { unsafe { CoUninitialize(); } }
        });
        Ok(Self { smtc, button_token, position_token, stop, worker: Some(worker) })
    }

    pub fn sync_now(&self, control: &MediaController) -> windows::core::Result<()> {
        update_metadata(&self.smtc, &control.metadata())?;
        sync_controls(&self.smtc, control)
    }
}

impl Drop for SmtcSession {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() { let _ = worker.join(); }
        let _ = self.smtc.RemovePlaybackPositionChangeRequested(self.position_token);
        let _ = self.smtc.RemoveButtonPressed(self.button_token);
        let _ = self.smtc.SetIsEnabled(false);
    }
}
