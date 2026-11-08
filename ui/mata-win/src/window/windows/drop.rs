//! Implements the `drop` module.
//!
//! This module documents both public API items and internal implementation details.

use windows::Win32::UI::WindowsAndMessaging::{ DestroyIcon, DestroyWindow };

impl Drop for super::PlatformWindow {
    /// Performs the drop operation.
    ///
    /// Visibility: **private**.
    #[doc = "Implements the drop operation. Visibility: private/internal."]
    fn drop(&mut self) {
        super::POINTER_VISIBLE.with(|map| { map.borrow_mut().remove(&(self.hwnd.0 as isize)); });
        // Release device-dependent Direct2D resources before destroying the HWND.
        self.renderer.shutdown();

        unsafe {
            if let Some(icon) = self.icon.take() {
                let _ = DestroyIcon(icon);
            }

            if !self.hwnd.is_invalid() {
                let _ = DestroyWindow(self.hwnd);
            }
        }
    }
}
