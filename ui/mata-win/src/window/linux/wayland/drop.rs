//! Implements the `drop` module.
//!
//! This module documents both public API items and internal implementation details.

impl Drop for super::PlatformWindow {
    /// Performs the drop operation.
    ///
    /// Visibility: **private**.
    #[doc = "Implements the drop operation. Visibility: private/internal."]
    fn drop(&mut self) {
        self.renderer.shutdown();

        // Cursor resources stay mapped until the cursor surface is destroyed.
        if let Some(cursor) = self.state.borrow_mut().cursor_surface.take() {
            cursor.attach(None, 0, 0);
            cursor.commit();
            cursor.destroy();
        }
        // Explicitly unmap before destroying the role objects. Remaining Wayland
        // resources are also reclaimed when the connection itself is dropped.
        self.surface.attach(None, 0, 0);
        self.surface.commit();

        if let Some(decoration) = self.decoration.take() {
            decoration.destroy();
        }
        self.toplevel.destroy();
        self.xdg_surface.destroy();
        self.surface.destroy();

        let _ = self.connection.flush();
    }
}
