//! Compile-time regression checks for the public mata-ui input surface MIVL uses.
//! The test must work without opening a native window.

use mata_ui::widgets::{TouchPhase, WidgetEvent};

#[test]
fn widget_events_used_by_mivl_exist_without_feature_flags() {
    let input = [
        WidgetEvent::PointerMoved(3, 4),
        WidgetEvent::PointerPressed(3, 4),
        WidgetEvent::PointerReleased(3, 4),
        WidgetEvent::Scrolled { x: 3, y: 4, dx: 0, dy: -120 },
        WidgetEvent::Touched {
            device_id: "contact-0".to_owned(), phase: TouchPhase::Moved, x: 3, y: 4,
        },
    ];
    assert_eq!(input.len(), 5);
    let _ = std::any::type_name::<mivl::native::NativeApp>();
}
