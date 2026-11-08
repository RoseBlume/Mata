//! Native Windows and desktop-integrated Linux file selectors.
//!
//! No picker-specific Rust dependency: the Windows implementation uses the
//! workspace's existing `windows` crate, while Linux invokes a desktop GUI
//! (`kdialog`, `zenity`, or `yad`) directly, without a shell.
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode { Open, Save }

pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "mol", "sdf", "mol2", "pdb", "ent", "cif", "mmcif", "asnt", "asn", "pc3d", "pc3", "mrc", "map", "ccp4", "nrrd", "tif", "tiff",
];

/// Match the file types accepted by `formats::read_structures` and
/// `formats::write_structures`. `All supported` comes first in the UI.
const FILTERS: &[(&str, &str, &str)] = &[
    ("All supported files", "*.mol *.sdf *.mol2 *.pdb *.ent *.cif *.mmcif *.asnt *.asn *.pc3d *.pc3 *.mrc *.map *.ccp4 *.nrrd *.tif *.tiff", "sdf"),
    ("Structure Data File (SDF)", "*.sdf", "sdf"),
    ("MDL MOL", "*.mol", "mol"),
    ("Tripos MOL2", "*.mol2", "mol2"),
    ("Protein Data Bank", "*.pdb *.ent", "pdb"),
    ("Macromolecular CIF", "*.cif *.mmcif", "cif"),
    ("PubChem ASN.1 text", "*.asnt", "asnt"),
    ("PubChem ASN.1 binary", "*.asn", "asn"),
    ("PubChem 3D", "*.pc3d *.pc3", "pc3d"),
    ("Microscopy MRC/CCP4 density", "*.mrc *.map *.ccp4", "mrc"),
    ("Microscopy NRRD volume", "*.nrrd", "nrrd"),
    ("Microscopy TIFF/OME-TIFF", "*.tif *.tiff", "tif"),
    ("All files", "*", "sdf"),
];

/// The picker can return Ok(None) for cancellation. For Save, append the
/// default extension only if none was explicitly supplied by the user.
fn preferred_filter_index(path: &Path) -> usize {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else { return 0; };
    FILTERS.iter().enumerate().skip(1).take(FILTERS.len() - 2).find(|(_, (_, patterns, _))| {
        patterns.split_whitespace().any(|pat| pat.strip_prefix("*.").is_some_and(|suffix| suffix.eq_ignore_ascii_case(ext)))
    }).map(|(index, _)| index).unwrap_or(0)
}

fn finalize_path(path: PathBuf, mode: Mode, filter_index: usize) -> PathBuf {
    if mode == Mode::Save && path.extension().is_none() {
        let extension = FILTERS.get(filter_index).unwrap_or(&FILTERS[0]).2;
        path.with_extension(extension)
    } else { path }
}

pub fn pick(mode: Mode, existing: &Path, owner: *mut std::ffi::c_void) -> Result<Option<PathBuf>, String> {
    #[cfg(target_os = "windows")]
    { return windows_picker(mode, existing, owner); }
    #[cfg(target_os = "linux")]
    { let _ = owner; return linux_picker(mode, existing); }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    { let _ = (mode, existing, owner); Err("File dialogs are supported on Windows and Linux".into()) }
}

#[cfg(target_os = "windows")]
fn windows_picker(mode: Mode, existing: &Path, owner: *mut std::ffi::c_void) -> Result<Option<PathBuf>, String> {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_YESNO, MB_ICONWARNING, IDYES};
    use windows::Win32::UI::Controls::Dialogs::{
        CommDlgExtendedError, GetOpenFileNameW, GetSaveFileNameW,
        OPENFILENAMEW, OFN_EXPLORER, OFN_FILEMUSTEXIST, OFN_PATHMUSTEXIST,
        OFN_NOCHANGEDIR, OFN_OVERWRITEPROMPT, OFN_HIDEREADONLY,
    };

    // Filters in Win32 are pairs of null-terminated UTF-16 strings, with a
    // final additional null terminator. Semicolons separate filename patterns.
    let mut filters = Vec::<u16>::new();
    // Only filename patterns use semicolon separators; labels keep spaces.
    for (name, patterns, _) in FILTERS {
        filters.extend(name.encode_utf16().chain([0]));
        filters.extend(patterns.replace(' ', ";").encode_utf16().chain([0]));
    }
    filters.push(0);

    // OPENFILENAMEW expects a writable buffer; 32,768 UTF-16 code units
    // handles long paths and non-ASCII filenames.
    let mut filename = vec![0u16; 32768];
    let seed = existing.as_os_str().encode_wide().collect::<Vec<_>>();
    if seed.len() < filename.len() {
        filename[..seed.len()].copy_from_slice(&seed);
    }
    let selected_filter = preferred_filter_index(existing);
    let title: Vec<u16> = match mode { Mode::Open => "Open molecule or microscopy volume", Mode::Save => "Save molecular structure as" }
        .encode_utf16().chain([0]).collect();
    // Do not set lpstrDefExt here. A static default would incorrectly force
    // e.g. `.sdf` after the user changes the filter to PC3D. Apply the actual
    // selected filter's extension *after* the dialog returns instead.
    // Zero initialization is valid for this C ABI structure (all pointer
    // fields and numeric/bitflag fields may be zero), including 32-bit Windows.
    let base: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    let mut dialog = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: HWND(owner),
        lpstrFilter: PCWSTR(filters.as_ptr()),
        nFilterIndex: (selected_filter + 1) as u32,
        lpstrFile: PWSTR(filename.as_mut_ptr()),
        nMaxFile: filename.len() as u32,
        lpstrTitle: PCWSTR(title.as_ptr()),
        Flags: OFN_EXPLORER | OFN_NOCHANGEDIR | OFN_PATHMUSTEXIST | OFN_HIDEREADONLY
            | if mode == Mode::Open { OFN_FILEMUSTEXIST } else { OFN_OVERWRITEPROMPT },
        ..base
    };

    // SAFETY: all pointers inside `dialog` refer to living UTF-16 buffers;
    // the `hwndOwner` belongs to the invoking mata_win Window. Win32 writes
    // at most nMaxFile UTF-16 units into lpstrFile.
    let success = unsafe { match mode {
        Mode::Open => GetOpenFileNameW(&mut dialog),
        Mode::Save => GetSaveFileNameW(&mut dialog),
    }}.as_bool();
    if !success {
        let error = unsafe { CommDlgExtendedError() };
        return if error.0 == 0 {
            Ok(None)
        } else {
            Err(format!("Windows file dialog error 0x{:04X}", error.0))
        };
    }
    let end = filename.iter().position(|c| *c == 0).unwrap_or(filename.len());
    if end == 0 { return Ok(None); }
    let path = PathBuf::from(std::ffi::OsString::from_wide(&filename[..end]));
    let final_path = finalize_path(path.clone(), mode, (dialog.nFilterIndex as usize).saturating_sub(1));
    if mode == Mode::Save && path != final_path && final_path.exists() {
        // Windows cannot confirm overwrites for an extension that is added
        // after the picker closes. Ask explicitly before replacing it.
        let prompt = format!("This file already exists:\n{}\n\nReplace it?", final_path.display());
        let prompt = prompt.encode_utf16().chain([0]).collect::<Vec<_>>();
        let caption = "Confirm overwrite".encode_utf16().chain([0]).collect::<Vec<_>>();
        // SAFETY: the UTF-16 strings remain alive for the entire modal call.
        let answer = unsafe { MessageBoxW(Some(HWND(owner)), PCWSTR(prompt.as_ptr()), PCWSTR(caption.as_ptr()), MB_YESNO | MB_ICONWARNING) };
        if answer != IDYES { return Ok(None); }
    }
    Ok(Some(final_path))
}

#[cfg(target_os = "linux")]
fn linux_picker(mode: Mode, existing: &Path) -> Result<Option<PathBuf>, String> {
    use std::process::Command;

    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_ascii_lowercase();
    let prefer_kde = desktop.contains("kde") || desktop.contains("plasma");
    let programs: &[&str] = if prefer_kde {
        &["kdialog", "zenity", "yad"]
    } else {
        &["zenity", "kdialog", "yad"]
    };
    let initial = if existing.is_absolute() { existing.to_path_buf() }
        else { std::env::current_dir().map_err(|e| format!("Cannot resolve current directory: {e}"))?.join(existing) };
    for program in programs {
        let mut cmd = Command::new(program);
        match *program {
            "kdialog" => {
                let action = if mode == Mode::Open { "--getopenfilename" } else { "--getsavefilename" };
                // KDE file patterns: `Label (*.mol *.sdf);;Label (*.pdb)`.
                let kde_filters = FILTERS.iter().map(|(name, patterns, _)| {
                    format!("{name} ({patterns})")
                }).collect::<Vec<_>>().join(";;");
                cmd.arg("--title").arg(if mode == Mode::Open { "Open molecule or microscopy volume" } else { "Save molecular structure as" })
                    .arg(action).arg(&initial).arg(kde_filters);
            },
            "zenity" => {
                cmd.arg("--file-selection")
                    .arg(if mode == Mode::Open { "--title=Open molecule" } else { "--title=Save molecule as" })
                    .arg(format!("--filename={}", initial.display()));
                if mode == Mode::Save { cmd.arg("--save").arg("--confirm-overwrite"); }
                for (name, patterns, _) in FILTERS {
                    cmd.arg(format!("--file-filter={name} | {patterns}"));
                }
            },
            "yad" => {
                cmd.arg("--file-selection")
                    .arg(if mode == Mode::Open { "--title=Open molecule" } else { "--title=Save molecule as" })
                    .arg(format!("--filename={}", initial.display()));
                if mode == Mode::Save { cmd.arg("--save").arg("--confirm-overwrite"); }
                for (name, patterns, _) in FILTERS {
                    cmd.arg(format!("--file-filter={name} | {patterns}"));
                }
            },
            _ => unreachable!(),
        }
        match cmd.output() {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(format!("Cannot launch {program}: {e}")),
            Ok(output) if output.status.success() => {
                let value = String::from_utf8(output.stdout)
                    .map_err(|_| format!("{program} returned a path that is not UTF-8"))?;
                let value = value.trim_end_matches(['\r', '\n']);
                if value.is_empty() { return Ok(None); }
                // If the user selected a filename without a suffix, carry
                // forward the last used format instead of changing it to SDF.
                // This also avoids overwriting an unrelated extension.
                let chosen = PathBuf::from(value);
                let finalized = finalize_path(chosen.clone(), mode, preferred_filter_index(existing));
                if mode == Mode::Save && finalized != chosen && finalized.exists() {
                    let title = format!("Overwrite existing file?\n{}", finalized.display());
                    let confirmed = match *program {
                        "kdialog" => Command::new("kdialog").arg("--warningyesno").arg(&title).status(),
                        "zenity" => Command::new("zenity").arg("--question").arg("--title=Confirm overwrite")
                            .arg(format!("--text={title}")).status(),
                        "yad" => Command::new("yad").arg("--question").arg("--title=Confirm overwrite")
                            .arg(format!("--text={title}")).status(),
                        _ => unreachable!(),
                    }.map_err(|e| format!("Cannot confirm overwrite: {e}"))?;
                    if !confirmed.success() { return Ok(None); }
                }
                return Ok(Some(finalized));
            },
            Ok(output) if matches!(output.status.code(), Some(1) | Some(252)) => return Ok(None),
            Ok(output) => {
                return Err(format!("{program} file picker failed: {}", String::from_utf8_lossy(&output.stderr).trim()));
            },
        }
    }
    Err("No graphical file picker available. Install Zenity, KDialog or YAD, or type a file path in the FILE field and press Enter to open it. Saving to the typed path is also supported.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn save_defaults_to_sdf_when_missing_extension() {
        assert_eq!(finalize_path(PathBuf::from("my molecule"), Mode::Save, 0), PathBuf::from("my molecule.sdf"));
        assert_eq!(finalize_path(PathBuf::from("new"), Mode::Save, 3), PathBuf::from("new.mol2"));
    }
    #[test]
    fn explicit_extensions_are_retained() {
        for ext in SUPPORTED_EXTENSIONS {
            let p = PathBuf::from(format!("a.{ext}"));
            assert_eq!(finalize_path(p.clone(), Mode::Save, 0), p);
        }
    }
    #[test]
    fn selects_filter_for_alias_extensions() {
        assert_eq!(preferred_filter_index(Path::new("protein.ent")), 4);
        assert_eq!(preferred_filter_index(Path::new("sample.mmcif")), 5);
        assert_eq!(preferred_filter_index(Path::new("xyz.PC3D")), 8);
        assert_eq!(preferred_filter_index(Path::new("xyz.PC3")), 8);
    }
    #[test]
    fn open_does_not_append_extension() {
        let p = PathBuf::from("anything");
        assert_eq!(finalize_path(p.clone(), Mode::Open, 4), p);
    }
    #[test]
    fn filter_list_matches_importers() {
        for ext in SUPPORTED_EXTENSIONS {
            assert!(FILTERS[0].1.split_whitespace().any(|p| p == format!("*.{ext}").as_str()), "missing filter for {ext}");
        }
    }
}

/// Select multiple molecular files in one native dialog. The existing `pick`
/// API stays unchanged for Save As and users wanting a single file.
pub fn pick_many(existing: &Path, owner:*mut std::ffi::c_void)->Result<Option<Vec<PathBuf>>,String>{
    #[cfg(target_os="windows")]
    {return windows_many(existing,owner);}
    #[cfg(target_os="linux")]
    {let _=owner;return linux_many(existing);}
    #[cfg(not(any(target_os="windows",target_os="linux")))]
    {let _=(existing,owner);Err("Multi-file picker available on Windows and Linux".into())}
}

#[cfg(target_os="windows")]
fn windows_many(existing:&Path,owner:*mut std::ffi::c_void)->Result<Option<Vec<PathBuf>>,String>{
    use std::os::windows::ffi::{OsStrExt,OsStringExt};
    use windows::core::{PCWSTR,PWSTR};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Controls::Dialogs::{GetOpenFileNameW,CommDlgExtendedError,
        OPENFILENAMEW,OFN_EXPLORER,OFN_NOCHANGEDIR,OFN_PATHMUSTEXIST,OFN_FILEMUSTEXIST,
        OFN_HIDEREADONLY,OFN_ALLOWMULTISELECT};
    let mut filters=Vec::<u16>::new();
    for (name,patterns,_) in FILTERS {
        filters.extend(name.encode_utf16().chain([0]));
        filters.extend(patterns.replace(' ',";").encode_utf16().chain([0]));
    }
    filters.push(0);
    let mut filename=vec![0u16;32768];
    let init=existing.as_os_str().encode_wide().collect::<Vec<_>>();
    if init.len()<filename.len(){filename[..init.len()].copy_from_slice(&init);}
    let title="Add molecular structure or microscopy files".encode_utf16().chain([0]).collect::<Vec<_>>();
    let base:OPENFILENAMEW=unsafe{std::mem::zeroed()};
    let mut dialog=OPENFILENAMEW{
        lStructSize:std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner:HWND(owner),lpstrFilter:PCWSTR(filters.as_ptr()),
        nFilterIndex:1,lpstrFile:PWSTR(filename.as_mut_ptr()),nMaxFile:filename.len() as u32,
        lpstrTitle:PCWSTR(title.as_ptr()),
        Flags:OFN_EXPLORER|OFN_NOCHANGEDIR|OFN_PATHMUSTEXIST|OFN_FILEMUSTEXIST|
            OFN_HIDEREADONLY|OFN_ALLOWMULTISELECT,..base
    };
    if !unsafe{GetOpenFileNameW(&mut dialog)}.as_bool(){
        let code=unsafe{CommDlgExtendedError()}.0;
        return if code==0{Ok(None)}else{Err(format!("Windows multi-file picker error 0x{code:04X}"))};
    }
    let values=filename.split(|&unit|unit==0).take_while(|part|!part.is_empty())
        .map(|part|PathBuf::from(std::ffi::OsString::from_wide(part)))
        .collect::<Vec<_>>();
    if values.is_empty(){return Ok(None);}
    if values.len()==1 {return Ok(Some(values));}
    let directory=&values[0];
    Ok(Some(values.iter().skip(1).map(|name|directory.join(name)).collect()))
}

#[cfg(target_os="linux")]
fn linux_many(existing:&Path)->Result<Option<Vec<PathBuf>>,String>{
    use std::process::Command;
    let desktop=std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_ascii_lowercase();
    let programs: &[&str]=if desktop.contains("kde")||desktop.contains("plasma"){
        &["kdialog","zenity","yad"]
    }else{&["zenity","kdialog","yad"]};
    let initial=if existing.is_absolute(){existing.to_path_buf()}else{
        std::env::current_dir().map_err(|e|e.to_string())?.join(existing)
    };
    for prog in programs {
        let mut cmd=Command::new(prog);
        match *prog {
            "kdialog"=>{
                let filters=FILTERS.iter().map(|(name,patterns,_)|format!("{name} ({patterns})"))
                    .collect::<Vec<_>>().join(";;");
                cmd.arg("--title").arg("Add molecular structures").arg("--getopenfilename")
                    .arg(&initial).arg(filters).arg("--multiple").arg("--separate-output");
            },
            "zenity"|"yad"=>{
                cmd.arg("--file-selection").arg("--multiple").arg("--separator=\n")
                    .arg("--title=Add molecular structures")
                    .arg(format!("--filename={}",initial.display()));
                for (name,patterns,_) in FILTERS {
                    cmd.arg(format!("--file-filter={name} | {patterns}"));
                }
            },
            _=>unreachable!(),
        }
        match cmd.output(){
            Err(err) if err.kind()==std::io::ErrorKind::NotFound=>continue,
            Err(err)=>return Err(format!("{prog}: {err}")),
            Ok(output) if output.status.success()=>{
                let result=String::from_utf8(output.stdout)
                    .map_err(|_|"File picker returned invalid UTF-8".to_string())?;
                let paths=result.lines().filter(|s|!s.trim().is_empty())
                    .map(|s|PathBuf::from(s.trim_end_matches('\r'))).collect::<Vec<_>>();
                return if paths.is_empty(){Ok(None)}else{Ok(Some(paths))};
            },
            Ok(output) if output.status.code()==Some(1)=>return Ok(None),
            Ok(output)=>return Err(format!("{prog} exited with {}",output.status)),
        }
    }
    Err("Install zenity, kdialog, or yad for a graphical multi-file picker".into())
}
