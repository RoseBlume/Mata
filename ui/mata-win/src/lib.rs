//! Minimal cross-platform native window creation, input, and software framebuffers.
//!
//! This crate contains *no* widgets, fonts, image/audio/video decoders, UI
//! layout, or UI painting. Use `mata-ui` for controls and text rendering.

pub mod buffer;
pub mod events;
pub mod window;

pub use buffer::Framebuffer;
pub use window::Window;
#[cfg(target_os = "android")]
pub use window::AndroidApp;
