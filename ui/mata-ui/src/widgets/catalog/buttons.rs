//! Toggle, check, and switch widgets.

use std::sync::{ Arc, Mutex };

use super::base::{ Widget, WidgetExt, PaintState };

#[derive(Debug, Clone)]
/// Stores the state and data for toggle button.
///
/// Visibility: **public**.
pub struct ToggleButton {
    /// Stores widget.
    ///
    /// Visibility: **private**.
    widget: Widget,
    /// Stores label.
    ///
    /// Visibility: **private**.
    label: Arc<Mutex<String>>,
    /// Stores active.
    ///
    /// Visibility: **private**.
    active: Arc<Mutex<bool>>,
}
impl ToggleButton {
    /// Creates a new instance.
    ///
    /// Visibility: **public**.
    #[doc = "Creates a new value. Visibility: public."]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            widget: Widget::new(),
            label: Arc::new(Mutex::new(label.into())),
            active: Arc::new(Mutex::new(false)),
        }
    }
    /// Performs the active operation.
    ///
    /// Visibility: **public**.
    #[doc = "Implements the active operation. Visibility: public."]
    pub fn active(&self) -> bool {
        *self.active.lock().unwrap()
    }
    /// Sets active.
    ///
    /// Visibility: **public**.
    #[doc = "Sets active. Visibility: public."]
    pub fn set_active(&self, active: bool) {
        *self.active.lock().unwrap() = active;
        self.widget.invalidate();
    }
    /// Performs the label operation.
    ///
    /// Visibility: **public**.
    #[doc = "Implements the label operation. Visibility: public."]
    pub fn label(&self) -> String {
        self.label.lock().unwrap().clone()
    }
}
impl WidgetExt for ToggleButton {
    fn pointer_activate(&self, _x: f32, _y: f32) -> bool {
        self.set_active(!self.active());
        true
    }
    fn paint_state(&self) -> PaintState { PaintState {active:Some(self.active()),text:Some(self.label()),..PaintState::default()} }
    #[doc = "Implements the widget operation. Visibility: private/internal."]
    fn widget(&self) -> &Widget {
        &self.widget
    }
}

#[derive(Debug, Clone)]
/// Stores the state and data for check button.
///
/// Visibility: **public**.
pub struct CheckButton {
    /// Stores inner.
    ///
    /// Visibility: **private**.
    inner: ToggleButton,
}
impl CheckButton {
    #[doc = "Creates a new value. Visibility: public."]
    pub fn new(label: impl Into<String>) -> Self {
        Self { inner: ToggleButton::new(label) }
    }
    #[doc = "Implements the active operation. Visibility: public."]
    pub fn active(&self) -> bool {
        self.inner.active()
    }
    #[doc = "Sets active. Visibility: public."]
    pub fn set_active(&self, v: bool) {
        self.inner.set_active(v);
    }
}
impl WidgetExt for CheckButton {
    fn pointer_activate(&self, _x: f32, _y: f32) -> bool {
        self.set_active(!self.active());
        true
    }
    fn paint_state(&self) -> PaintState { PaintState {active:Some(self.active()),text:Some(self.inner.label()),..PaintState::default()} }
    #[doc = "Implements the widget operation. Visibility: private/internal."]
    fn widget(&self) -> &Widget {
        self.inner.widget()
    }
}

#[derive(Debug, Clone)]
/// Stores the state and data for switch.
///
/// Visibility: **public**.
pub struct Switch {
    /// Stores widget.
    ///
    /// Visibility: **private**.
    widget: Widget,
    /// Stores active.
    ///
    /// Visibility: **private**.
    active: Arc<Mutex<bool>>,
}
impl Default for Switch {
    #[doc = "Creates the default value. Visibility: private/internal."]
    fn default() -> Self {
        Self::new()
    }
}
impl Switch {
    #[doc = "Creates a new value. Visibility: public."]
    pub fn new() -> Self {
        Self { widget: Widget::new(), active: Arc::new(Mutex::new(false)) }
    }
    #[doc = "Implements the active operation. Visibility: public."]
    pub fn active(&self) -> bool {
        *self.active.lock().unwrap()
    }
    #[doc = "Sets active. Visibility: public."]
    pub fn set_active(&self, v: bool) {
        *self.active.lock().unwrap() = v;
        self.widget.invalidate();
    }
}
impl WidgetExt for Switch {
    fn pointer_activate(&self, _x: f32, _y: f32) -> bool {
        self.set_active(!self.active());
        true
    }
    fn paint_state(&self) -> PaintState { PaintState {active:Some(self.active()),..PaintState::default()} }
    #[doc = "Implements the widget operation. Visibility: private/internal."]
    fn widget(&self) -> &Widget {
        &self.widget
    }
}

#[cfg(test)]
mod pointer_tests {
    use super::*;
    #[test]
    fn click_toggles_real_widget_state() {
        let switch = Switch::new();
        assert!(!switch.active());
        assert!(switch.pointer_activate(0.5,0.5));
        assert!(switch.active());
        assert_eq!(switch.paint_state().active, Some(true));
        assert!(switch.pointer_activate(0.5,0.5));
        assert!(!switch.active());
        let check=CheckButton::new("demo");
        assert!(check.pointer_activate(0.5,0.5));
        assert!(check.active());
    }
}
