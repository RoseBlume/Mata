//! Retained widgets, fonts, layout, image/audio/video UI and rendering for Mata.
//!
//! `mata-ui` depends on `mata_win` for its native window and framebuffer; the
//! dependency is one-way, so `mata_win` can be used by games without UI overhead.
//!
//! For source compatibility, the root reexports `Window`, `Framebuffer`, and
//! `events` from `mata_win`. They remain **defined and implemented** in mata_win.

pub use mata_win::{Framebuffer, Window, events};
#[cfg(target_os = "android")]
pub use mata_win::AndroidApp;

pub mod widgets;
pub use widgets::{WidgetWindow, NativeWindowHandle, Rect};
#[cfg(feature = "fonts")]
pub use widgets::FontRenderer;

/// Optional zero-dependency translation engine. Enable `mata-ui/i18n`.
#[cfg(feature = "i18n")]
pub use mata_i18n as i18n;
