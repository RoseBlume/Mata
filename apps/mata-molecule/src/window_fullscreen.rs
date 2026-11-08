//! OS-window fullscreen without additional Rust dependencies. The original
//! Windows placement and decorations are restored when F11 is pressed again.

#[cfg(target_os="windows")]
pub struct FullscreenState {
    original:Option<(isize,windows::Win32::UI::WindowsAndMessaging::WINDOWPLACEMENT)>,
}
#[cfg(target_os="windows")]
impl FullscreenState {
    pub fn new()->Self{Self{original:None}}
    pub fn toggle(&mut self,handle:*mut std::ffi::c_void)->Result<bool,String> {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::Graphics::Gdi::{GetMonitorInfoW,MonitorFromWindow,MONITORINFO,MONITOR_DEFAULTTONEAREST};
        use windows::Win32::UI::WindowsAndMessaging::{
            GetWindowLongPtrW,SetWindowLongPtrW,GWL_STYLE,WS_OVERLAPPEDWINDOW,
            GetWindowPlacement,SetWindowPlacement,WINDOWPLACEMENT,
            SetWindowPos,SWP_FRAMECHANGED,SWP_NOOWNERZORDER,SWP_NOZORDER,SWP_NOMOVE,SWP_NOSIZE,
        };
        let hwnd=HWND(handle);
        if hwnd.0.is_null(){return Err("MataWin did not expose a native window handle".into());}
        // SAFETY: handle belongs to our live Window on its UI thread.
        unsafe {
            if let Some((style,placement))=self.original.take(){
                SetWindowLongPtrW(hwnd,GWL_STYLE,style);
                SetWindowPlacement(hwnd,&placement).map_err(|e|format!("Restoring window placement: {e}"))?;
                SetWindowPos(hwnd,None,0,0,0,0,SWP_FRAMECHANGED|SWP_NOOWNERZORDER|SWP_NOZORDER|SWP_NOMOVE|SWP_NOSIZE)
                    .map_err(|e|format!("Restoring window decorations: {e}"))?;
                return Ok(false);
            }
            let mut placement=WINDOWPLACEMENT::default();
            placement.length=std::mem::size_of::<WINDOWPLACEMENT>() as u32;
            GetWindowPlacement(hwnd,&mut placement).map_err(|e|format!("Reading window placement: {e}"))?;
            let monitor=MonitorFromWindow(hwnd,MONITOR_DEFAULTTONEAREST);
            if monitor.0.is_null(){return Err("Could not find the window's display".into());}
            let mut info=MONITORINFO::default();
            info.cbSize=std::mem::size_of::<MONITORINFO>() as u32;
            if !GetMonitorInfoW(monitor,&mut info).as_bool(){return Err("Could not read monitor dimensions".into());}
            let style=GetWindowLongPtrW(hwnd,GWL_STYLE);
            SetWindowLongPtrW(hwnd,GWL_STYLE,style & !(WS_OVERLAPPEDWINDOW.0 as isize));
            let r=info.rcMonitor;
            if let Err(err)=SetWindowPos(hwnd,None,r.left,r.top,r.right-r.left,r.bottom-r.top,SWP_FRAMECHANGED|SWP_NOOWNERZORDER|SWP_NOZORDER) {
                SetWindowLongPtrW(hwnd,GWL_STYLE,style);
                return Err(format!("Entering fullscreen: {err}"));
            }
            self.original=Some((style,placement));
            Ok(true)
        }
    }
}

#[cfg(target_os="linux")]
pub struct FullscreenState { enabled:bool }
#[cfg(target_os="linux")]
impl FullscreenState {
    pub fn new()->Self{Self{enabled:false}}
    pub fn toggle(&mut self,_handle:*mut std::ffi::c_void)->Result<bool,String> {
        // EWMH message through wmctrl. This leaves the existing mata_win crate
        // unchanged; X11 compositors generally support _NET_WM_STATE_FULLSCREEN.
        // Wayland deliberately does not permit arbitrary external fullscreen
        // manipulation. A native mata_win fullscreen API is needed there.
        let command=if self.enabled{"remove,fullscreen"}else{"add,fullscreen"};
        let result=std::process::Command::new("wmctrl")
            .args(["-r","Mata Molecule Studio","-b",command]).status()
            .map_err(|e|format!("X11 fullscreen requires wmctrl or a mata_win fullscreen API: {e}"))?;
        if !result.success(){return Err("Window manager rejected fullscreen (Wayland does not support wmctrl)".into());}
        self.enabled=!self.enabled;
        Ok(self.enabled)
    }
}

#[cfg(not(any(target_os="windows",target_os="linux")))]
pub struct FullscreenState;
#[cfg(not(any(target_os="windows",target_os="linux")))]
impl FullscreenState {
    pub fn new()->Self{Self}
    pub fn toggle(&mut self,_handle:*mut std::ffi::c_void)->Result<bool,String>{Err("OS fullscreen is not implemented for this platform".into())}
}

/// Poll the physical F11 key without assuming that `mata_win::events::Key`
/// exposes an `F11` variant. The window must be foreground to prevent a
/// background Studio instance from responding to another application's keys.
#[cfg(target_os = "windows")]
pub fn native_f11_down(handle: *mut std::ffi::c_void) -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_F11};
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    if handle.is_null() { return false; }
    // SAFETY: Both Win32 functions only read the foreground HWND/key state;
    // no window pointers are dereferenced by application code.
    unsafe {
        GetForegroundWindow().0 == handle
            && GetAsyncKeyState(i32::from(VK_F11.0)) < 0
    }
}

/// A native F11 poll for non-Windows systems requires the windowing backend
/// to expose a physical function-key event. Ctrl+Shift+F is the portable
/// fallback until that is available in mata_win.
#[cfg(not(target_os = "windows"))]
pub fn native_f11_down(_handle: *mut std::ffi::c_void) -> bool { false }
