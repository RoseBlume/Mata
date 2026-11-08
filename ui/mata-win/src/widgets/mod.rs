//! MataWin-native widgets.
//!
//! Ordinary controls are software rendered by MataWin itself. Input comes from
//! `mata_win::Window`, pixels go through `mata_win::Framebuffer`, and the
//! `fonts`, `audio`, and `image` features independently enable their Mata
//! ecosystem integrations. Native ARGB pixel drawing remains available with
//! image decoding disabled.
//! Optional native video decoding uses `mata-video` with the `video` feature.
//!
//! The optional `webview` feature enables the sole platform-toolkit exception:
//! WebView2 on Windows and WebKitGTK 4.1 on Linux. It is disabled by default.
//! Android supports MataWin's software widgets; the optional `webview` backend
//! is currently desktop-only.

#[cfg(all(feature = "webview", target_os = "android"))]
compile_error!(
    "mata_win's optional `webview` feature is not implemented on Android; disable `webview` when targeting Android"
);

/// Contains the catalog implementation.
///
/// Visibility: **public**.
pub mod catalog;
/// Retained visual content and nested layout groups.
pub mod content;
pub use content::{WidgetContent, ContentGroup, ContentDirection};
/// Contains the font implementation.
///
/// Visibility: **public**.
#[cfg(feature = "fonts")]
pub mod font;
/// Contains the media implementation.
///
/// Visibility: **public**.
#[cfg(feature = "audio")]
pub mod media;
/// Contains the style implementation.
///
/// Visibility: **public**.
pub mod style;

/// Contains the controls implementation.
///
/// Visibility: **public**.
pub mod controls;
/// Contains the events implementation.
///
/// Visibility: **public**.
pub mod events;
/// Contains the geometry implementation.
///
/// Visibility: **public**.
pub mod geometry;
/// Contains the render implementation.
///
/// Visibility: **private**.
mod render;
#[cfg(feature = "webview")]
/// Contains the webview implementation.
///
/// Visibility: **public**.
pub mod webview;
/// Contains the window implementation.
///
/// Visibility: **public**.
pub mod window;

#[cfg(all(feature = "webview", target_os = "windows"))]
/// Contains the windows implementation.
///
/// Visibility: **crate-visible**.
pub(crate) mod windows;
#[cfg(all(feature = "webview", target_os = "linux"))]
/// Contains the linux implementation.
///
/// Visibility: **private**.
mod linux;

#[cfg(all(feature = "webview", target_os = "windows"))]
use windows as webview_platform;
#[cfg(all(feature = "webview", target_os = "linux"))]
use linux as webview_platform;

pub use catalog::*;
pub use controls::{ Button, TextField };
pub use events::{ WidgetError, WidgetEvent, WidgetId };
#[cfg(feature = "fonts")]
pub use font::FontRenderer;
// Rendering controls without fonts retains backgrounds and input handling.
#[cfg(not(feature = "fonts"))]
pub(crate) type FontRenderer = ();
pub use geometry::{ Anchors, Rect, ResizePolicy };
#[cfg(feature = "audio")]
pub use media::{ AudioSource, AudioSourceState };
#[cfg(feature = "video")]
pub use mata_video::{ PixelFormat as VideoPixelFormat, VideoFrame, VideoInfo };
pub use style::*;
#[cfg(feature = "webview")]
pub use webview::WebView;
pub use window::WidgetWindow;

pub(crate) use events::next_id;
#[cfg(feature = "webview")]
pub(crate) use events::push_event;
