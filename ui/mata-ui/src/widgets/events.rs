//! Widget identifiers, public events, errors, and the cross-widget event queue.

use std::sync::{ Mutex, OnceLock };
use std::sync::atomic::{ AtomicU64, Ordering };

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Stores the state and data for widget id.
///
/// Visibility: **public**.
pub struct WidgetId(
    /// Stores the numeric widget identifier.
    ///
    /// Visibility: **crate-visible field of a public tuple struct**.
    pub(crate) u64,
);
impl WidgetId {
    #[inline]
    #[doc = "Implements the get operation. Visibility: public."]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Defines the available widget event choices.
///
/// Visibility: **public**.
pub enum WidgetEvent {
    /// Native keystroke, for platform-independent custom input surfaces.
    KeyPressed(crate::events::Key),
    /// Native key release for modifier tracking.
    KeyReleased(crate::events::Key),
    /// Printable character for an unfocused standard TextField.
    TextInput(char),
    /// Represents the close variant.
    ///
    /// Visibility: inherited from the **public** enum.
    Close,
    /// Represents the resized variant.
    ///
    /// Visibility: inherited from the **public** enum.
    Resized(u32, u32),
    /// Mouse cursor position in widget-client pixels.
    PointerMoved(i32, i32),
    /// Primary mouse button pressed at a client-area position.
    PointerPressed(i32, i32),
    /// Primary mouse button released at a client-area position.
    PointerReleased(i32, i32),
    /// Secondary mouse button pressed at a client-area position.
    PointerRightPressed(i32, i32),
    /// Secondary mouse button released at a client-area position.
    PointerRightReleased(i32, i32),
    /// Right-click completed; optional target is the widget beneath the pointer.
    /// Applications can use this to display a context menu.
    ContextMenuRequested { widget: Option<WidgetId>, x: i32, y: i32 },
    /// An input method updated the not-yet-committed composition text.
    ImePreeditChanged(WidgetId, String),
    /// Activated an item in a software context menu.
    MenuItemActivated { owner: WidgetId, item: u64 },
    /// A selectable text widget's highlighted range changed.
    TextSelectionChanged(WidgetId),
    /// Wheel displacement at the current pointer position (positive Y scrolls up).
    Scrolled { x: i32, y: i32, dx: i32, dy: i32 },
    /// Native touch input using a stable contact-specific device ID.
    Touched { device_id: String, phase: TouchPhase, x: i32, y: i32 },
    /// Represents the button clicked variant.
    ///
    /// Visibility: inherited from the **public** enum.
    ButtonClicked(WidgetId),
    /// A live catalog widget changed itself in response to pointer input.
    CatalogChanged(WidgetId),
    /// Represents the text changed variant.
    ///
    /// Visibility: inherited from the **public** enum.
    TextChanged(WidgetId),
    /// Represents the text field activated variant.
    ///
    /// Visibility: inherited from the **public** enum.
    TextFieldActivated(WidgetId),
    #[cfg(feature = "video")]
    /// Video decoder/playback failure; video is paused after emission.
    VideoError(WidgetId, String),
    #[cfg(any(feature = "webview", feature = "webview-servo"))]
    /// Represents the web view ready variant.
    ///
    /// Visibility: inherited from the **public** enum.
    WebViewReady(WidgetId),
    #[cfg(any(feature = "webview", feature = "webview-servo"))]
    /// Represents the web view error variant.
    ///
    /// Visibility: inherited from the **public** enum.
    WebViewError(WidgetId, String),
}

/// Phase of a native touch contact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchPhase {
    Started,
    Moved,
    Ended,
    Cancelled,
}

#[derive(Debug, Clone)]
/// Stores the state and data for widget error.
///
/// Visibility: **public**.
pub struct WidgetError(
    /// Stores the human-readable widget error message.
    ///
    /// Visibility: **public**.
    pub String,
);
impl std::fmt::Display for WidgetError {
    #[doc = "Implements the fmt operation. Visibility: private/internal."]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for WidgetError {}

#[derive(Debug, Clone)]
/// Stores the state and data for queued event.
///
/// Visibility: **crate-visible**.
pub(crate) struct QueuedEvent {
    /// Stores host id.
    ///
    /// Visibility: **public**.
    pub host_id: u64,
    /// Stores event.
    ///
    /// Visibility: **public**.
    pub event: WidgetEvent,
}
/// Static storage used for next id.
///
/// Visibility: **private**.
static NEXT_ID: AtomicU64 = AtomicU64::new(1);
/// Static storage used for event queue.
///
/// Visibility: **private**.
static EVENT_QUEUE: OnceLock<Mutex<Vec<QueuedEvent>>> = OnceLock::new();
#[inline]
#[doc = "Implements the next id operation. Visibility: crate-visible."]
pub(crate) fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}
#[inline]
#[doc = "Pushes event. Visibility: crate-visible."]
pub(crate) fn push_event(host_id: u64, event: WidgetEvent) {
    EVENT_QUEUE.get_or_init(|| Mutex::new(Vec::with_capacity(32)))
        .lock()
        .unwrap()
        .push(QueuedEvent { host_id, event });
}
/// Implements drain events.
///
/// Visibility: **crate-visible**.
#[doc = "Implements the drain events operation. Visibility: crate-visible."]
pub(crate) fn drain_events(host_id: u64, out: &mut Vec<WidgetEvent>) {
    let mut q = EVENT_QUEUE.get_or_init(|| Mutex::new(Vec::with_capacity(32)))
        .lock()
        .unwrap();
    let mut i = 0;
    while i < q.len() {
        if q[i].host_id == host_id {
            out.push(q.swap_remove(i).event);
        } else {
            i += 1;
        }
    }
}

#[cfg(test)]
mod event_api_tests {
    use super::*;
    #[test]
    fn native_input_events_are_public_and_usable() {
        let all = [
            WidgetEvent::PointerMoved(1, 2),
            WidgetEvent::PointerPressed(3, 4),
            WidgetEvent::PointerReleased(5, 6),
            WidgetEvent::PointerRightPressed(3, 4),
            WidgetEvent::PointerRightReleased(5, 6),
            WidgetEvent::ContextMenuRequested { widget: None, x: 5, y: 6 },
            WidgetEvent::Scrolled { x: 1, y: 2, dx: 0, dy: -120 },
            WidgetEvent::Touched { device_id: "touch:1".into(), phase: TouchPhase::Started, x: 5, y: 6 },
        ];
        assert_eq!(all.len(), 8);
        assert_eq!(TouchPhase::Ended, TouchPhase::Ended);
    }
    #[test]
    fn key_release_event_can_be_cloned_and_compared() {
        let released = WidgetEvent::KeyReleased(crate::events::Key::LeftShift);
        assert_eq!(released, released.clone());
    }
}
