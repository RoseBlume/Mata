//! Mata UI widgets, composed over MataWin native windows.
//!
//! Ordinary controls are software rendered by mata-ui itself. Input comes from
//! `mata_win::Window`, pixels go through `mata_win::Framebuffer`, and the
//! `fonts`, `audio`, and `image` features independently enable their Mata
//! ecosystem integrations. Native ARGB pixel drawing remains available with
//! image decoding disabled.
//! Optional native video decoding uses `mata-video` with the `video` feature.
//!
//! `webview` is a compatibility alias for `webview-servo`. Both use Servo
//! pixel rendering *inside* the Mata framebuffer. Disabled by default.
//! Android supports mata-ui software widgets; embedded Servo has not been
//! validated for Android with this backend.

#[cfg(all(feature = "webview", not(feature = "webview-servo"), target_os = "android"))]
compile_error!(
    "mata-ui's optional `webview` feature is not implemented on Android; disable `webview` when targeting Android"
);

/// Contains the catalog implementation.
///
/// Visibility: **public**.
pub mod catalog;
/// Retained visual content and nested layout groups.
pub mod content;
pub mod interaction;
/// Cross-platform operating-system color preference and asynchronous change monitor.
pub mod system_appearance;
pub mod qa;
pub use interaction::{MenuItem, AccessibilityNode, AccessibilityRole, Theme};
pub use system_appearance::{system_color_scheme, SystemColorScheme, SystemAppearanceMonitor};
pub use content::{WidgetContent, ContentGroup, ContentDirection};
/// Contains the font implementation.
///
/// Visibility: **public**.
#[cfg(feature = "fonts")]
#[cfg(target_os = "windows")]
#[path = "font_gdi.rs"]
pub mod font;
#[cfg(all(feature = "fonts", not(target_os = "windows")))]
#[path = "font.rs"]
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
/// Desktop clipboard access shared by text fields and HTML selection.
pub mod clipboard;
/// Contains the events implementation.
///
/// Visibility: **public**.
pub mod events;
/// Contains the geometry implementation.
///
/// Visibility: **public**.
pub mod geometry;
/// Simplified layout helpers for native applications.
pub mod layouts;
/// Contains the render implementation.
///
/// Visibility: **private**.
mod render;
mod text_selection;
#[cfg(all(feature = "fonts", not(target_os = "windows")))]
mod emoji;
#[cfg(any(feature = "webview", feature = "webview-servo"))]
/// Contains the webview implementation.
///
/// Visibility: **public**.
pub mod webview;

// pub use mata_doc_edit::{DocEdit, PagedDocument, DrawTool, PaperSize, Point, Stroke, PageLayout};
/// Contains the window implementation.
///
/// Visibility: **public**.
pub mod window;

// Native Save As selection; document serialization is handled by format crates.
#[cfg(target_os = "windows")]
pub mod file_dialog;
#[cfg(target_os = "linux")]
#[path = "file_dialog_linux.rs"]
pub mod file_dialog;
#[cfg(target_os = "linux")]
mod file_dialog_portal;
#[cfg(target_os = "linux")]
mod dbus_std;
#[cfg(target_os = "macos")]
#[path = "file_dialog_macos.rs"]
pub mod file_dialog;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub use file_dialog::{
    open_file_dialog, open_files_dialog, save_file_dialog,
    open_folder_dialog, open_folders_dialog, FileDialog,
};
#[cfg(target_os = "linux")]
pub use file_dialog::DialogBackend;

#[cfg(feature = "webview-servo")]
mod servo_backend;
#[cfg(feature = "webview-servo")]
use servo_backend as webview_platform;

pub use catalog::*;
pub use controls::{ Button, TextField };
pub use events::{ WidgetError, WidgetEvent, WidgetId, TouchPhase };
#[cfg(feature = "fonts")]
pub use font::FontRenderer;
// Rendering controls without fonts retains backgrounds and input handling.
#[cfg(not(feature = "fonts"))]
pub(crate) type FontRenderer = ();
pub use geometry::{ Anchors, Rect, ResizePolicy };
pub use layouts::{ AlignItems, FlexDirection, FlexItem, FlexLayout, GridLayout, Insets, JustifyContent };
#[cfg(feature = "audio")]
pub use media::{ AudioSource, AudioSourceState };
#[cfg(feature = "video")]
pub use mata_video::{ PixelFormat as VideoPixelFormat, VideoFrame, VideoInfo };
pub use style::*;
#[cfg(any(feature = "webview", feature = "webview-servo"))]
pub use webview::WebView;
pub use window::{NativeWindowHandle, WidgetWindow};

pub(crate) use events::next_id;
#[cfg(any(feature = "webview", feature = "webview-servo"))]
pub(crate) use events::push_event;
