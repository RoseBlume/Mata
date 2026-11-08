//! Read-only process enumeration. Never terminates or suspends anything.
use windows::Win32::{
    Foundation::{CloseHandle, HANDLE},
    System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW,
        PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    },
};
use std::{collections::BTreeSet, mem};

struct Snapshot(HANDLE);
impl Drop for Snapshot {
    fn drop(&mut self) { unsafe { let _ = CloseHandle(self.0); } }
}

pub fn incompatible_processes(names: &[String]) -> windows::core::Result<Vec<String>> {
    if names.is_empty() { return Ok(Vec::new()); }
    let snapshot = Snapshot(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)? });
    let mut entry = PROCESSENTRY32W { dwSize: mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
    let mut found = BTreeSet::new();
    if unsafe { Process32FirstW(snapshot.0, &mut entry) }.is_err() { return Ok(Vec::new()); }
    loop {
        let end = entry.szExeFile.iter().position(|&x| x == 0).unwrap_or(entry.szExeFile.len());
        let name = String::from_utf16_lossy(&entry.szExeFile[..end]).to_ascii_lowercase();
        if names.iter().any(|blocked| blocked.eq_ignore_ascii_case(&name)) {
            found.insert(name);
        }
        if unsafe { Process32NextW(snapshot.0, &mut entry) }.is_err() { break; }
    }
    Ok(found.into_iter().collect())
}
