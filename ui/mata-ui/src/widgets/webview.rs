//! Optional public WebView wrapper and resize integration with [`WidgetWindow`].
//!
//! This module is available with `webview` (native) or `webview-servo` (embedded).

use std::{ cell::Cell, rc::Rc, sync::{ Arc, Mutex } };

use super::{
    events::{ next_id, WidgetError, WidgetId },
    geometry::{ Anchors, Rect, ResizePolicy, ResizeState },
    style::{ StyleHandle, Stylable },
    webview_platform,
    window::WidgetWindow,
};

/// A browser widget rendered by either the native or Servo backend.
pub struct WebView {
    /// Stores id.
    ///
    /// Visibility: **private**.
    id: WidgetId,
    /// Stores inner.
    ///
    /// Visibility: **private**.
    inner: Rc<webview_platform::WebView>,
    /// Stores resize.
    ///
    /// Visibility: **private**.
    resize: Arc<Mutex<ResizeState>>,
    /// Stores host size.
    ///
    /// Visibility: **private**.
    host_size: Rc<Cell<(u32, u32)>>,
    /// Stores style.
    ///
    /// Visibility: **private**.
    style: StyleHandle,
}
impl WebView {
    /// Creates a new instance.
    ///
    /// Visibility: **public**.
    #[doc = "Creates a new value. Visibility: public."]
    pub fn new(parent: &WidgetWindow, bounds: Rect) -> Result<Self, WidgetError> {
        let id = WidgetId(next_id());
        let inner = Rc::new(webview_platform::WebView::new(parent, id, bounds)?);
        let resize = Arc::new(Mutex::new(ResizeState::new(bounds, parent.size())));
        let style = StyleHandle::new();
        parent.register_webview(&inner, &resize, &style);
        Ok(Self { id, inner, resize, host_size: parent.host_size.clone(), style })
    }
    #[inline]
    #[doc = "Implements the id operation. Visibility: public."]
    pub const fn id(&self) -> WidgetId {
        self.id
    }
    #[inline]
    #[doc = "Implements the navigate operation. Visibility: public."]
    pub fn navigate(&self, uri: &str) -> Result<(), WidgetError> {
        self.inner.navigate(uri)
    }
    #[inline]
    /// Shows/hides the browser while retaining its history.
    pub fn set_visible(&self, visible: bool) {
        self.inner.set_visible(visible)
    }

    #[inline]
    #[doc = "Implements the go back operation. Visibility: public."]
    pub fn go_back(&self) {
        self.inner.go_back()
    }
    #[inline]
    #[doc = "Implements the go forward operation. Visibility: public."]
    pub fn go_forward(&self) {
        self.inner.go_forward()
    }
    #[inline]
    #[doc = "Implements the reload operation. Visibility: public."]
    pub fn reload(&self) {
        self.inner.reload()
    }
    /// Current page URL including navigations initiated by the page.
    pub fn url(&self) -> Option<String> { self.inner.url() }

    /// Current document title, if supplied by the page.
    pub fn page_title(&self) -> Option<String> { self.inner.page_title() }

    /// Reports Servo's back/forward history state.
    pub fn can_go_back(&self) -> bool { self.inner.can_go_back() }
    pub fn can_go_forward(&self) -> bool { self.inner.can_go_forward() }

    /// Sets page zoom; 1.0 is 100%.
    pub fn set_zoom(&self, scale: f32) -> Result<(), WidgetError> {
        if !scale.is_finite() || !(0.1..=10.0).contains(&scale) {
            return Err(WidgetError("Servo zoom must be between 0.1 and 10.0".into()));
        }
        self.inner.set_zoom(scale);
        Ok(())
    }

    /// Evaluates JavaScript in the current page asynchronously.
    /// The callback executes when Servo processes its event loop.
    pub fn evaluate_javascript(
        &self,
        source: &str,
        callback: impl FnOnce(Result<servo::JSValue, servo::JavaScriptEvaluationError>) + 'static,
    ) {
        self.inner.evaluate_javascript(source, callback)
    }

    /// Sets bounds.
    ///
    /// Visibility: **public**.
    #[doc = "Sets bounds. Visibility: public."]
    pub fn set_bounds(&self, bounds: Rect) {
        let mut resize = self.resize.lock().unwrap();
        resize.reset(bounds, self.host_size.get());
        drop(resize);
        let computed = self.style.computed(bounds, self.host_size.get(), None);
        self.inner.set_bounds(computed.bounds)
    }
    /// Performs the bounds operation.
    ///
    /// Visibility: **public**.
    #[doc = "Implements the bounds operation. Visibility: public."]
    pub fn bounds(&self) -> Rect {
        let base = self.resize.lock().unwrap().resolve(self.host_size.get());
        self.style.computed(base, self.host_size.get(), None).bounds
    }
    /// Sets resize policy.
    ///
    /// Visibility: **public**.
    #[doc = "Sets resize policy. Visibility: public."]
    pub fn set_resize_policy(&self, policy: ResizePolicy) {
        let mut resize = self.resize.lock().unwrap();
        let bounds = resize.resolve(self.host_size.get());
        resize.set_policy(policy, bounds, self.host_size.get())
    }
    /// Sets anchors.
    ///
    /// Visibility: **public**.
    #[doc = "Sets anchors. Visibility: public."]
    pub fn set_anchors(&self, anchors: Anchors) {
        self.set_resize_policy(ResizePolicy::Anchored(anchors))
    }
    /// Performs the style handle operation.
    ///
    /// Visibility: **public**.
    #[doc = "Implements the style handle operation. Visibility: public."]
    pub fn style_handle(&self) -> StyleHandle {
        self.style.clone()
    }
}
impl Stylable for WebView {
    #[doc = "Implements the style handle operation. Visibility: private/internal."]
    fn style_handle(&self) -> StyleHandle {
        WebView::style_handle(self)
    }
}


