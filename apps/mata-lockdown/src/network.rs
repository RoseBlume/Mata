//! Optional WFP rules. Only explicitly configured executable paths are blocked.
//! The engine uses a dynamic session: dropping it removes its filters.
use std::{ffi::c_void, path::Path, ptr};
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::HANDLE,
        NetworkManagement::WindowsFilteringPlatform::{
            FwpmEngineOpen0, FwpmEngineClose0, FwpmGetAppIdFromFileName0,
            FwpmFilterAdd0, FwpmFreeMemory0,
            FWPM_SESSION0, FWPM_SESSION_FLAG_DYNAMIC, FWPM_FILTER0,
            FWPM_FILTER_CONDITION0, FWPM_LAYER_ALE_AUTH_CONNECT_V4,
            FWPM_LAYER_ALE_AUTH_CONNECT_V6, FWPM_SUBLAYER_UNIVERSAL,
            FWPM_CONDITION_ALE_APP_ID, FWP_MATCH_EQUAL,
            FWP_CONDITION_VALUE0, FWP_CONDITION_VALUE0_0,
            FWP_BYTE_BLOB, FWP_BYTE_BLOB_TYPE, FWP_ACTION_BLOCK,
            FWPM_ACTION0,
        },
    },
};

pub struct AppNetworkGuard { engine: HANDLE }

impl AppNetworkGuard {
    /// Blocks outbound IPv4 and IPv6 connections by explicitly named .exe paths.
    /// This is NOT website-level filtering and requires WFP privileges.
    pub fn new(paths: &[String]) -> Result<Self, String> {
        if paths.is_empty() { return Err("no executable paths configured".into()); }
        for path in paths {
            if !Path::new(path).is_absolute() || !Path::new(path).is_file() {
                return Err(format!("blocked_app_path must be an existing absolute file path: {path}"));
            }
        }
        let session = FWPM_SESSION0 { flags: FWPM_SESSION_FLAG_DYNAMIC, ..Default::default() };
        let mut engine = HANDLE::default();
        // RPC_C_AUTHN_WINNT = 10. Only local WFP engine is targeted.
        let result = unsafe { FwpmEngineOpen0(PCWSTR::null(), 10, None, Some(&session), &mut engine) };
        if result != 0 { return Err(format!("FwpmEngineOpen0 returned {result} (administrator permission may be required)")); }
        let guard = Self { engine };
        for path in paths {
            guard.add_app(path)?;
        }
        Ok(guard)
    }

    fn add_app(&self, path: &str) -> Result<(), String> {
        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let mut blob: *mut FWP_BYTE_BLOB = ptr::null_mut();
        let rc = unsafe { FwpmGetAppIdFromFileName0(PCWSTR(wide.as_ptr()), &mut blob) };
        if rc != 0 { return Err(format!("FwpmGetAppIdFromFileName0({path}): {rc}")); }
        if blob.is_null() { return Err("WFP returned empty AppId".into()); }
        // Always release WFP-allocated memory, including on a filter error.
        let result = (|| {
            let mut condition = FWPM_FILTER_CONDITION0 {
                fieldKey: FWPM_CONDITION_ALE_APP_ID,
                matchType: FWP_MATCH_EQUAL,
                conditionValue: FWP_CONDITION_VALUE0 {
                    r#type: FWP_BYTE_BLOB_TYPE,
                    Anonymous: FWP_CONDITION_VALUE0_0 { byteBlob: blob },
                },
            };
            for layer in [FWPM_LAYER_ALE_AUTH_CONNECT_V4, FWPM_LAYER_ALE_AUTH_CONNECT_V6] {
                let filter = FWPM_FILTER0 {
                    layerKey: layer,
                    subLayerKey: FWPM_SUBLAYER_UNIVERSAL,
                    numFilterConditions: 1,
                    filterCondition: &mut condition,
                    action: FWPM_ACTION0 { r#type: FWP_ACTION_BLOCK, ..Default::default() },
                    ..Default::default()
                };
                let rc = unsafe { FwpmFilterAdd0(self.engine, &filter, None, None) };
                if rc != 0 { return Err(format!("FwpmFilterAdd0({path}): {rc}")); }
            }
            Ok(())
        })();
        let mut memory = blob.cast::<c_void>();
        unsafe { FwpmFreeMemory0(&mut memory); }
        result
    }
}

impl Drop for AppNetworkGuard {
    fn drop(&mut self) {
        // Dynamic-session policy objects are removed on close/process death.
        unsafe { let _ = FwpmEngineClose0(self.engine); }
    }
}
