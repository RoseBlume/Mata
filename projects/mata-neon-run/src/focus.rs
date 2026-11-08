//! Portable focus approximation available through the MataWin public API.
//! For reliable Alt+Tab/background detection even with a stationary pointer,
//! MataWin should eventually expose keyboard/window activation state.
use mata_win::Window;

pub fn active(window: &Window) -> bool {
    window.cursor_focused()
}
