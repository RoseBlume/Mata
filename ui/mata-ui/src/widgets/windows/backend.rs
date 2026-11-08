//! Implements the `backend` module.
//!
//! This module documents both public API items and internal implementation details.

use super::*;
use super::callbacks::{ env_release, release_com, ENV_COMPLETED_VTBL };
use super::loader::load_webview2_create;

/// Stores the state and data for web view.
///
/// Visibility: **public**.
pub struct WebView {
    /// Stores state.
    ///
    /// Visibility: **private**.
    state: Arc<Mutex<WebViewState>>,
}

impl WebView {
    /// Creates a new instance.
    ///
    /// Visibility: **public**.
    #[doc = "Creates a new value. Visibility: public."]
    pub fn new(parent: &WidgetWindow, id: WidgetId, bounds: Rect) -> Result<Self, WidgetError> {
        Self::new_with_native_parent(parent, id, bounds, parent.native_handle())
    }

    /// Creates a WebView2 controller inside an independent native child host.
    /// `bounds` must be relative to that host's client area.
    pub fn new_with_native_parent(
        parent: &WidgetWindow,
        id: WidgetId,
        bounds: Rect,
        native_parent: *mut c_void,
    ) -> Result<Self, WidgetError> {
        if native_parent.is_null() {
            return Err(WidgetError("WebView2 parent window is null".into()));
        }
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
        let create = load_webview2_create()?;
        let state = Arc::new(
            Mutex::new(WebViewState {
                bounds,
                visible: true,
                ..Default::default()
            })
        );

        let handler = Box::new(EnvironmentCompletedHandler {
            vtbl: &ENV_COMPLETED_VTBL,
            refs: AtomicU32::new(1),
            parent: HWND(native_parent),
            host_id: parent.id(),
            widget_id: id,
            state: state.clone(),
        });
        let raw_handler = Box::into_raw(handler);

        let hr = unsafe { create(PCWSTR::null(), PCWSTR::null(), ptr::null_mut(), raw_handler) };

        // Release the creator's reference. The loader owns another reference if
        // asynchronous creation started successfully.
        unsafe {
            env_release(raw_handler.cast());
        }

        if hr.0 < 0 {
            return Err(
                WidgetError(
                    format!(
                        "CreateCoreWebView2EnvironmentWithOptions failed: 0x{:08X}",
                        hr.0 as u32
                    )
                )
            );
        }

        Ok(Self { state })
    }

    /// Performs the navigate operation.
    ///
    /// Visibility: **public**.
    #[doc = "Implements the navigate operation. Visibility: public."]
    pub fn navigate(&self, uri: &str) -> Result<(), WidgetError> {
        let uri = wide(uri);
        let mut state = self.state.lock().unwrap();
        if state.webview == 0 {
            state.pending_uri = Some(uri);
            return Ok(());
        }
        let webview = state.webview as *mut CoreWebView2;
        drop(state);
        let hr = unsafe { ((*(*webview).vtbl).navigate)(webview, PCWSTR(uri.as_ptr())) };
        if hr.0 < 0 {
            Err(WidgetError(format!("ICoreWebView2::Navigate failed: 0x{:08X}", hr.0 as u32)))
        } else {
            Ok(())
        }
    }

    /// Sets bounds.
    ///
    /// Visibility: **public**.
    #[doc = "Sets bounds. Visibility: public."]
    pub fn set_bounds(&self, bounds: Rect) {
        let mut state = self.state.lock().unwrap();
        state.bounds = bounds;
        if state.controller == 0 {
            return;
        }
        let controller = state.controller as *mut Controller;
        drop(state);
        let rect = RECT {
            left: bounds.x,
            top: bounds.y,
            right: bounds.x.saturating_add(bounds.width as i32),
            bottom: bounds.y.saturating_add(bounds.height as i32),
        };
        unsafe {
            let _ = ((*(*controller).vtbl).put_bounds)(controller, rect);
        }
    }

    /// Shows or hides the native WebView2 controller without destroying its tab history.
    pub fn set_visible(&self, visible: bool) {
        let mut state = self.state.lock().unwrap();
        state.visible = visible;
        let controller = state.controller as *mut Controller;
        drop(state);
        if !controller.is_null() {
            unsafe { let _ = ((*(*controller).vtbl).put_is_visible)(controller, i32::from(visible)); }
        }
    }

    /// Performs the go back operation.
    ///
    /// Visibility: **public**.
    #[doc = "Implements the go back operation. Visibility: public."]
    pub fn go_back(&self) {
        let state = self.state.lock().unwrap();
        if state.webview == 0 {
            return;
        }
        let webview = state.webview as *mut CoreWebView2;
        drop(state);
        unsafe {
            let _ = ((*(*webview).vtbl).go_back)(webview);
        }
    }

    /// Performs the go forward operation.
    ///
    /// Visibility: **public**.
    #[doc = "Implements the go forward operation. Visibility: public."]
    pub fn go_forward(&self) {
        let state = self.state.lock().unwrap();
        if state.webview == 0 {
            return;
        }
        let webview = state.webview as *mut CoreWebView2;
        drop(state);
        unsafe {
            let _ = ((*(*webview).vtbl).go_forward)(webview);
        }
    }

    /// Performs the reload operation.
    ///
    /// Visibility: **public**.
    #[doc = "Implements the reload operation. Visibility: public."]
    pub fn reload(&self) {
        let state = self.state.lock().unwrap();
        if state.webview == 0 {
            return;
        }
        let webview = state.webview as *mut CoreWebView2;
        drop(state);
        unsafe {
            let _ = ((*(*webview).vtbl).reload)(webview);
        }
    }
}

impl Drop for WebView {
    /// Performs the drop operation.
    ///
    /// Visibility: **private**.
    #[doc = "Implements the drop operation. Visibility: private/internal."]
    fn drop(&mut self) {
        let mut state = self.state.lock().unwrap();
        state.closed = true;
        let webview = state.webview as *mut CoreWebView2;
        let controller = state.controller as *mut Controller;
        state.webview = 0;
        state.controller = 0;
        drop(state);

        unsafe {
            release_com(webview);
            release_com(controller);
        }
    }
}


// Browser views must not share the editor's full-window WebView2 controller
// parent: their HWNDs otherwise compete for sibling z-order and are hidden by
// the opaque editor. This Win32 child window provides a stable, separate host.
// It is positioned over the editor's browser viewport and owns no Rust crates.
#[link(name = "user32")]
unsafe extern "system" {
    fn CreateWindowExW(
        ex_style: u32, class_name: *const u16, window_name: *const u16,
        style: u32, x: i32, y: i32, width: i32, height: i32,
        parent: *mut c_void, menu: *mut c_void, instance: *mut c_void,
        parameter: *mut c_void,
    ) -> *mut c_void;
    fn SetWindowPos(
        window: *mut c_void, insert_after: *mut c_void,
        x: i32, y: i32, width: i32, height: i32, flags: u32,
    ) -> i32;
    fn ShowWindow(window: *mut c_void, command: i32) -> i32;
    fn DestroyWindow(window: *mut c_void) -> i32;
}

/// Native browser viewport, separate from the full-window editor WebView2.
/// All WebView2 tabs use this HWND as parent and draw at local (0,0).
pub struct OverlayHost {
    hwnd: *mut c_void,
}

impl OverlayHost {
    pub fn new(parent: &WidgetWindow, bounds: Rect) -> Result<Self, WidgetError> {
        const WS_CHILD: u32 = 0x4000_0000;
        const WS_VISIBLE: u32 = 0x1000_0000;
        const WS_CLIPSIBLINGS: u32 = 0x0400_0000;
        const WS_CLIPCHILDREN: u32 = 0x0200_0000;
        let class = wide("STATIC"); // built-in Win32 window class
        let hwnd = unsafe {
            CreateWindowExW(
                0, class.as_ptr(), class.as_ptr(),
                WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | WS_CLIPCHILDREN,
                bounds.x, bounds.y,
                bounds.width.min(i32::MAX as u32) as i32,
                bounds.height.min(i32::MAX as u32) as i32,
                parent.native_handle(), ptr::null_mut(), ptr::null_mut(), ptr::null_mut(),
            )
        };
        if hwnd.is_null() {
            return Err(WidgetError("CreateWindowExW for browser viewport failed".into()));
        }
        let host = Self { hwnd };
        host.bring_to_front();
        Ok(host)
    }

    pub fn native_handle(&self) -> *mut c_void { self.hwnd }

    pub fn set_bounds(&self, bounds: Rect) {
        const SWP_NOACTIVATE: u32 = 0x0010;
        const SWP_SHOWWINDOW: u32 = 0x0040;
        unsafe {
            let _ = SetWindowPos(
                self.hwnd, ptr::null_mut(), bounds.x, bounds.y,
                bounds.width.min(i32::MAX as u32) as i32,
                bounds.height.min(i32::MAX as u32) as i32,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
    }

    pub fn bring_to_front(&self) {
        // Reassert ordering after asynchronous WebView2 controller creation.
        const SWP_NOSIZE: u32 = 0x0001;
        const SWP_NOMOVE: u32 = 0x0002;
        const SWP_NOACTIVATE: u32 = 0x0010;
        unsafe {
            let _ = SetWindowPos(
                self.hwnd, ptr::null_mut(), 0, 0, 0, 0,
                SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE,
            );
        }
    }

    pub fn set_visible(&self, visible: bool) {
        unsafe { let _ = ShowWindow(self.hwnd, if visible { 5 } else { 0 }); }
        if visible { self.bring_to_front(); }
    }
}

impl Drop for OverlayHost {
    fn drop(&mut self) {
        if !self.hwnd.is_null() {
            unsafe { let _ = DestroyWindow(self.hwnd); }
        }
    }
}
