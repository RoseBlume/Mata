//! Implements the `show` module.
//!
//! This module documents both public API items and internal implementation details.

use crate::events::WindowEvent;

impl super::PlatformWindow {
    /// Unmaps the surface by committing a null buffer.
    #[doc = "Implements the hide operation. Visibility: public."]
    pub fn hide(&self) {
        if self.hidden.get() {
            return;
        }

        self.surface.attach(None, 0, 0);
        self.surface.commit();
        self.hidden.set(true);
        let _ = self.connection.flush();
    }


    /// Performs the maximize operation.
    ///
    /// Visibility: **public**.
    #[doc = "Implements the maximize operation. Visibility: public."]
    pub fn maximize(&self) {
        self.toplevel.set_maximized();
        self.surface.commit();
        let _ = self.connection.flush();
    }

    /// Performs the minimize operation.
    ///
    /// Visibility: **public**.
    #[doc = "Implements the minimize operation. Visibility: public."]
    pub fn minimize(&self) {
        self.toplevel.set_minimized();
        self.surface.commit();
        super::push_event(WindowEvent::Minimized);
        let _ = self.connection.flush();
    }

    /// Performs the restore operation.
    ///
    /// Visibility: **public**.
    #[doc = "Implements the restore operation. Visibility: public."]
    pub fn restore(&self) {
        self.toplevel.unset_maximized();
        self.surface.commit();
        let _ = self.connection.flush();
    }

    /// Wayland intentionally does not allow normal clients to choose absolute
    /// top-level screen coordinates. The compositor is responsible for placement,
    /// so this method is a no-op retained for API compatibility.
    #[doc = "Implements the reposition operation. Visibility: public."]
    pub fn reposition(&self, _x: i32, _y: i32) {}

    /// Requests a new preferred content size.
    ///
    /// XDG shell has no Win32-style "set the outer window to exactly W×H" request.
    /// Changing the buffer size is the client-side size hint. If resizing has been
    /// disabled, the min/max constraints are also updated to lock this size.
    #[doc = "Implements the resize operation. Visibility: public."]
    pub fn resize(&self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }

        let decorated = {
            let state=self.state.borrow();
            state.decorations_enabled && !state.server_decorations
        };
        let surface_height = height.saturating_add(
            if decorated { super::decorations::HEIGHT } else { 0 });
        self.state.borrow_mut().client_size = (width, surface_height);
        if !self.resizable.get() {
            self.toplevel.set_min_size(width as i32, surface_height as i32);
            self.toplevel.set_max_size(width as i32, surface_height as i32);
        }

        self.surface.commit();
        super::push_event(WindowEvent::Resized(width, height));
        let _ = self.connection.flush();
    }

    /// Marks the surface visible again. Actual mapping occurs on the next
    /// `present`, when a real buffer is attached and committed.
    #[doc = "Implements the show operation. Visibility: public."]
    pub fn show(&self) {
        self.hidden.set(false);
    }

    /// Enable or disable window decorations. Prefers compositor-owned controls,
    /// falling back to the internal title bar when necessary.
    #[doc = "Sets decorations. Visibility: public."]
    pub fn set_decorations(&self, enabled: bool) {
        self.decorations_enabled.set(enabled);
        self.decoration_dirty.set(true);
        let mut state = self.state.borrow_mut();
        let old_chrome = state.decorations_enabled && !state.server_decorations;
        state.decorations_enabled = enabled;
        let new_chrome = enabled && !state.server_decorations;
        if old_chrome != new_chrome {
            state.client_size.1 = if new_chrome {
                state.client_size.1.saturating_add(super::decorations::HEIGHT)
            } else {
                state.client_size.1.saturating_sub(super::decorations::HEIGHT)
            };
        }
        if let Some(decoration) = self.decoration.as_ref() {
            decoration.set_mode(if enabled {
                super::zxdg_toplevel_decoration_v1::Mode::ServerSide
            } else {
                super::zxdg_toplevel_decoration_v1::Mode::ClientSide
            });
        }
        if !self.resizable.get() {
            let (width,height) = state.client_size;
            self.toplevel.set_min_size(width as i32,height as i32);
            self.toplevel.set_max_size(width as i32,height as i32);
        }
        let chrome=if new_chrome {super::decorations::HEIGHT} else {0};
        super::push_event(WindowEvent::Resized(state.client_size.0,
            state.client_size.1.saturating_sub(chrome)));
        drop(state);
        self.surface.commit();
        let _=self.connection.flush();
    }

    /// Configure the maximize button in the client-side fallback.
    #[doc = "Sets maximizable. Visibility: public."]
    pub fn set_maximizable(&self, enabled: bool) {
        self.maximizable.set(enabled);
        self.decoration_dirty.set(true);
        self.state.borrow_mut().maximizable = enabled;
    }

    /// Configure the minimize button in the client-side fallback.
    #[doc = "Sets minimizable. Visibility: public."]
    pub fn set_minimizable(&self, enabled: bool) {
        self.minimizable.set(enabled);
        self.decoration_dirty.set(true);
        self.state.borrow_mut().minimizable = enabled;
    }

    /// Enables or disables interactive resizing by changing XDG min/max size hints.
    #[doc = "Sets resizable. Visibility: public."]
    pub fn set_resizable(&self, enabled: bool) {
        self.resizable.set(enabled);

        if enabled {
            self.toplevel.set_min_size(0, 0);
            self.toplevel.set_max_size(0, 0);
        } else {
            // client_size is the total Wayland surface size. When the compositor
            // does not draw decorations it already includes the client-side title bar.
            let (width, surface_height) = self.state.borrow().client_size;
            self.toplevel.set_min_size(width as i32, surface_height as i32);
            self.toplevel.set_max_size(width as i32, surface_height as i32);
        }

        self.surface.commit();
        let _ = self.connection.flush();
    }
}
