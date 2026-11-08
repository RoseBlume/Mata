//! Implements the `loader` module.
//!
//! This module documents both public API items and internal implementation details.

use super::*;

/// Type alias for create environment with options.
///
/// Visibility: **private**.
type CreateEnvironmentWithOptions = unsafe extern "system" fn(
    PCWSTR,
    PCWSTR,
    *mut c_void,
    *mut EnvironmentCompletedHandler
) -> HRESULT;

/// Loads webview2 create.
///
/// Visibility: **parent-module-visible**.
#[doc = "Loads webview2 create. Visibility: parent-module-visible."]
pub(super) fn load_webview2_create() -> Result<CreateEnvironmentWithOptions, WidgetError> {
    // The WebView2 Runtime and the SDK loader are distinct components.
    // Prefer the exact DLL deployed beside this executable by our build script
    // rather than loading an arbitrary same-named DLL from the process PATH.
    let executable = std::env::current_exe().map_err(|e|
        WidgetError(format!("cannot determine application directory for WebView2: {e}"))
    )?;
    let dll = executable.with_file_name("WebView2Loader.dll");
    let dll_wide: Vec<u16> = dll.as_os_str().encode_wide()
        .chain(std::iter::once(0)).collect();
    let module = (unsafe { LoadLibraryW(PCWSTR(dll_wide.as_ptr())) }).map_err(|e|
        WidgetError(format!(
            "cannot load Microsoft WebView2Loader.dll from {} ({e}). \
             This is an SDK loader DLL, separate from Edge and the WebView2 Runtime. \
             Run mata-document/scripts/install-webview2-loader.ps1 and rebuild.", dll.display()
        ))
    )?;

    let proc = (
        unsafe {
            GetProcAddress(module, PCSTR(b"CreateCoreWebView2EnvironmentWithOptions\0".as_ptr()))
        }
    ).ok_or_else(||
        WidgetError(
            "WebView2Loader.dll does not export CreateCoreWebView2EnvironmentWithOptions".into()
        )
    )?;

    Ok(unsafe { mem::transmute(proc) })
}
