//! Generic native Windows Open/Save file pickers.
//!
//! These APIs select filesystem paths only. No content is read or written.
//! Use `.filter_extensions(vec!["docx", "txt"])` to restrict the visible
//! types, and `.extension("docx")` on Save to add a missing suffix.

#![cfg(target_os = "windows")]

use std::{
    ffi::{c_void, OsString},
    io,
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
    ptr,
};

use super::window::{NativeWindowHandle, WidgetWindow};

// ABI layout of the Win32 OPENFILENAMEW structure. It must stay in sync with
// comdlg32; the pointers below all refer to buffers kept alive through the call.
#[repr(C)]
struct OpenFileNameW {
    struct_size: u32,
    owner: *mut c_void,
    instance: *mut c_void,
    filter: *const u16,
    custom_filter: *mut u16,
    max_custom_filter: u32,
    filter_index: u32,
    file: *mut u16,
    max_file: u32,
    file_title: *mut u16,
    max_file_title: u32,
    initial_dir: *const u16,
    title: *const u16,
    flags: u32,
    file_offset: u16,
    file_extension: u16,
    default_extension: *const u16,
    custom_data: isize,
    hook: *mut c_void,
    template_name: *const u16,
    reserved: *mut c_void,
    dw_reserved: u32,
    flags_ex: u32,
}

#[link(name = "comdlg32")]
unsafe extern "system" {
    fn GetSaveFileNameW(dialog: *mut OpenFileNameW) -> i32;
    fn GetOpenFileNameW(dialog: *mut OpenFileNameW) -> i32;
    fn CommDlgExtendedError() -> u32;
}

#[repr(C)]
struct BrowseInfoW {
    owner: *mut c_void, root: *const c_void, display_name: *mut u16,
    title: *const u16, flags: u32, callback: Option<unsafe extern "system" fn(*mut c_void, u32, isize, isize) -> i32>,
    callback_data: isize, image: i32,
}
#[link(name="shell32")]
unsafe extern "system" {
    fn SHBrowseForFolderW(browse: *mut BrowseInfoW) -> *mut c_void;
    fn SHGetPathFromIDListW(pidl: *const c_void, path: *mut u16) -> i32;
}
#[link(name="ole32")]
unsafe extern "system" { fn CoTaskMemFree(ptr: *mut c_void); }
#[link(name="user32")]
unsafe extern "system" { fn SendMessageW(hwnd: *mut c_void, message: u32, wparam: usize, lparam: isize) -> isize; }

unsafe extern "system" fn folder_initialized(hwnd: *mut c_void, event: u32, _value: isize, data: isize) -> i32 {
    // BFFM_INITIALIZED -> BFFM_SETSELECTIONW, using an initial Unicode path.
    if event == 1 && data != 0 { unsafe { SendMessageW(hwnd, 0x0400 + 103, 1, data); } }
    0
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DialogKind {
    Open,
    Save,
    Folder,
}

/// A reusable, file-format-independent Windows file dialog builder.
///
/// `show()` returns `Ok(None)` if the user presses Cancel and `Ok(Some(PathBuf))`
/// if a file is selected. `show_files()` supports multi-selection for Open.
/// File I/O remains the caller's responsibility.
pub struct FileDialog<'a> {
    window: &'a WidgetWindow,
    kind: DialogKind,
    extensions: Vec<String>,
    named_filters: Vec<(String, Vec<String>)>,
    default_extension: Option<String>,
    initial_path: Option<PathBuf>,
    title: Option<String>,
    suggested_name: Option<String>,
}

/// Create an Open dialog, optionally restricted with `filter_extensions`.
///
/// ```no_run
/// # /* Requires a live WidgetWindow named window.
/// let picked = open_files_dialog(&window)
///     .filter_extensions(vec!["docx", "txt"])
///     .show()?;
/// # */
/// ```
pub fn open_files_dialog(window: &WidgetWindow) -> FileDialog<'_> {
    FileDialog::new(window, DialogKind::Open)
}

/// Create a Save dialog. `.extension("docx")` automatically adds `.docx`
/// when the user does not supply an extension. An explicitly typed extension
/// is not overwritten.
pub fn save_file_dialog(window: &WidgetWindow) -> FileDialog<'_> {
    FileDialog::new(window, DialogKind::Save)
}
/// Single-file alias for `open_files_dialog`.
pub fn open_file_dialog(window: &WidgetWindow) -> FileDialog<'_> {
    open_files_dialog(window)
}
/// Select an existing directory through the native Windows folder chooser.
pub fn open_folder_dialog(window: &WidgetWindow) -> FileDialog<'_> {
    FileDialog::new(window, DialogKind::Folder)
}
/// Windows' native folder chooser is single-folder only. Call `.show()`.
pub fn open_folders_dialog(window: &WidgetWindow) -> FileDialog<'_> {
    open_folder_dialog(window)
}

impl<'a> FileDialog<'a> {
    fn new(window: &'a WidgetWindow, kind: DialogKind) -> Self {
        Self { window, kind, extensions: Vec::new(), named_filters: Vec::new(), default_extension: None, initial_path: None, title: None, suggested_name: None }
    }

    /// Set visible file types. Accepts strings with or without a leading dot;
    /// e.g. `vec!["png", "jpg", ".jpeg"]`. An empty vector shows all files.
    pub fn filter_extensions<I, S>(mut self, extensions: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.extensions = extensions.into_iter().map(|extension| extension.as_ref().to_owned()).collect();
        self
    }

    /// Add an additional named dropdown filter (e.g. "Images", ["png", "jpg"]).
    pub fn add_filter<I, S>(mut self, name: impl Into<String>, extensions: I) -> Self
    where I: IntoIterator<Item = S>, S: AsRef<str> {
        self.named_filters.push((name.into(), extensions.into_iter()
            .map(|value|value.as_ref().to_owned()).collect()));
        self
    }

    /// Specify the extension appended when a saved filename lacks one.
    /// This does not replace an extension explicitly entered by the user.
    pub fn extension(mut self, extension: impl AsRef<str>) -> Self {
        self.default_extension = Some(extension.as_ref().to_owned());
        self
    }

    /// Set an initial filename and/or directory.
    pub fn initial_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.initial_path = Some(path.into());
        self
    }

    /// Alias for specifying an initial directory.
    pub fn initial_directory(self, path: impl Into<PathBuf>) -> Self { self.initial_path(path) }

    /// Suggest an editable filename in a save dialog.
    pub fn suggested_name(mut self, name: impl Into<String>) -> Self {
        self.suggested_name = Some(name.into()); self
    }

    /// Override the native dialog's title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Show the picker for a single file. Cancel returns `Ok(None)`.
    pub fn show(self) -> io::Result<Option<PathBuf>> {
        let files = self.run(false)?;
        Ok(files.and_then(|mut files| {
            if files.is_empty() { None } else { Some(files.remove(0)) }
        }))
    }

    /// Show an Open picker permitting multiple selected files. Save dialogs
    /// reject multi-select with `InvalidInput`.
    pub fn show_files(self) -> io::Result<Option<Vec<PathBuf>>> {
        if self.kind != DialogKind::Open {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "show_files requires an Open dialog"));
        }
        self.run(true)
    }

    /// Windows shell's folder picker does not offer multiple folder selection.
    pub fn show_folders(self) -> io::Result<Option<Vec<PathBuf>>> {
        Err(io::Error::new(io::ErrorKind::Unsupported,
            "Windows native folder picker supports one folder at a time; use show()"))
    }

    fn run(self, multiple: bool) -> io::Result<Option<Vec<PathBuf>>> {
        let owner = match self.window.os_handle() {
            NativeWindowHandle::Win32(hwnd) => hwnd.as_ptr(),
            NativeWindowHandle::Unsupported => return Err(io::Error::new(
                io::ErrorKind::Unsupported, "a Win32 window is required for the native file picker")),
        };
        if owner.is_null() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "the parent window handle is null"));
        }

        if self.kind == DialogKind::Folder {
            if multiple { return Err(io::Error::new(io::ErrorKind::Unsupported,
                "Windows native folder picker supports one folder at a time")); }
            return self.pick_folder(owner).map(|path|path.map(|p|vec![p]));
        }

        let mut extensions = Vec::<String>::new();
        for raw in &self.extensions {
            let extension = normalized_extension(raw)?;
            if !extensions.iter().any(|existing| existing.eq_ignore_ascii_case(&extension)) {
                extensions.push(extension);
            }
        }
        let default_extension = self.default_extension
            .as_deref().map(normalized_extension).transpose()?;
        let mut named = Vec::new();
        for (name, values) in &self.named_filters {
            if name.is_empty() || name.contains('\0') { return Err(io::Error::new(io::ErrorKind::InvalidInput,
                "invalid filter name")); }
            let validated = values.iter().map(|v| normalized_extension(v))
                .collect::<io::Result<Vec<_>>>()?;
            named.push((name.clone(),validated));
        }
        let filter = filter_buffer_named(&extensions, &named);
        let title = self.title.unwrap_or_else(|| match self.kind {
            DialogKind::Open => "Open File".to_owned(),
            DialogKind::Save => "Save File As".to_owned(),
            DialogKind::Folder => "Select Folder".to_owned(),
        });
        if title.contains('\0') {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "dialog title contains a NUL character"));
        }
        let wide_title = wide(std::ffi::OsStr::new(&title));
        let wide_default = default_extension.as_ref().map(|ext| wide(std::ffi::OsStr::new(ext)));

        // OPENFILENAMEW requires writable, UTF-16, NUL-terminated storage.
        // The large buffer also permits multiple Explorer-style selections.
        let mut file = vec![0u16; 32_768];
        if let Some(suggested) = &self.suggested_name {
            if suggested.contains(['\0', '/', '\\']) { return Err(io::Error::new(io::ErrorKind::InvalidInput,
                "invalid suggested file name")); }
            let wide_name = wide(std::ffi::OsStr::new(suggested));
            if wide_name.len() > file.len() { return Err(io::Error::new(io::ErrorKind::InvalidInput,
                "suggested filename is too long")); }
            file[..wide_name.len()].copy_from_slice(&wide_name);
        } else if let Some(path) = self.initial_path.as_deref().filter(|path| !path.is_dir()) {
            if let Some(name) = path.file_name() {
                let wide_name = wide(name);
                if wide_name.len() > file.len() {
                    return Err(io::Error::new(io::ErrorKind::InvalidInput, "suggested filename is too long"));
                }
                file[..wide_name.len()].copy_from_slice(&wide_name);
            }
        } else if self.kind == DialogKind::Save {
            let mut fallback = PathBuf::from("Document");
            if let Some(ref extension) = default_extension { fallback.set_extension(extension); }
            let wide_name = wide(fallback.as_os_str());
            file[..wide_name.len()].copy_from_slice(&wide_name);
        }

        let initial_dir = self.initial_path.as_deref().and_then(|path| {
            let directory = if path.is_dir() { path } else { path.parent()? };
            directory.is_dir().then(|| wide(directory.as_os_str()))
        }).or_else(|| {
            if self.kind == DialogKind::Save {
                std::env::var_os("USERPROFILE")
                    .map(PathBuf::from)
                    .map(|home| home.join("Documents"))
                    .filter(|folder| folder.is_dir())
                    .map(|folder| wide(folder.as_os_str()))
            } else { None }
        });

        // OFN_HIDEREADONLY | OFN_NOCHANGEDIR | OFN_PATHMUSTEXIST | OFN_EXPLORER
        const COMMON: u32 = 0x0000_0004 | 0x0000_0008 | 0x0000_0800 | 0x0008_0000;
        const FILE_MUST_EXIST: u32 = 0x0000_1000;
        const OVERWRITE_PROMPT: u32 = 0x0000_0002;
        const ALLOW_MULTISELECT: u32 = 0x0000_0200;
        let flags = COMMON | match self.kind {
            DialogKind::Open => FILE_MUST_EXIST | if multiple { ALLOW_MULTISELECT } else { 0 },
            DialogKind::Save => OVERWRITE_PROMPT,
            DialogKind::Folder => unreachable!(),
        };
        let mut dialog = OpenFileNameW {
            struct_size: std::mem::size_of::<OpenFileNameW>() as u32,
            owner,
            instance: ptr::null_mut(),
            filter: filter.as_ptr(),
            custom_filter: ptr::null_mut(),
            max_custom_filter: 0,
            filter_index: 1,
            file: file.as_mut_ptr(),
            max_file: file.len() as u32,
            file_title: ptr::null_mut(),
            max_file_title: 0,
            initial_dir: initial_dir.as_ref().map_or(ptr::null(), |buf| buf.as_ptr()),
            title: wide_title.as_ptr(),
            flags,
            file_offset: 0,
            file_extension: 0,
            default_extension: wide_default.as_ref().map_or(ptr::null(), |buf| buf.as_ptr()),
            custom_data: 0,
            hook: ptr::null_mut(),
            template_name: ptr::null(),
            reserved: ptr::null_mut(),
            dw_reserved: 0,
            flags_ex: 0,
        };

        // SAFETY: layout is ABI-compatible and all UTF-16 allocations, the
        // output buffer and the live window outlive the synchronous OS call.
        let accepted = unsafe { match self.kind {
            DialogKind::Open => GetOpenFileNameW(&mut dialog),
            DialogKind::Save => GetSaveFileNameW(&mut dialog),
            DialogKind::Folder => unreachable!(),
        }};
        if accepted == 0 {
            let error = unsafe { CommDlgExtendedError() };
            if error == 0 { return Ok(None); }
            return Err(io::Error::other(format!("native file dialog failed (0x{error:04X})")));
        }

        let mut selected = parse_selected_files(&file, multiple)?;
        // Don't modify the suffix of an explicitly selected filename.
        if self.kind == DialogKind::Save {
            if let Some(extension) = default_extension {
                if let Some(path) = selected.first_mut() {
                    append_missing_extension(path, &extension);
                }
            }
        }
        Ok(Some(selected))
    }
}

impl FileDialog<'_> {
    fn pick_folder(&self, owner: *mut c_void) -> io::Result<Option<PathBuf>> {
        // SHBrowseForFolderW supports the native shell folder tree, including
        // setting the starting folder; it does not select multiple folders.
        let title = self.title.as_deref().unwrap_or("Choose a folder");
        if title.contains('\0') { return Err(io::Error::new(io::ErrorKind::InvalidInput,"invalid dialog title")); }
        let title = wide(std::ffi::OsStr::new(title));
        let initial = self.initial_path.as_ref()
            .filter(|p|p.is_dir()).map(|p|wide(p.as_os_str()));
        let mut buffer = vec![0u16; 260]; // MAX_PATH for SHGetPathFromIDListW
        let mut info = BrowseInfoW { owner, root: ptr::null(), display_name: buffer.as_mut_ptr(),
            title:title.as_ptr(), flags:0x0001 | 0x0040 | 0x0010, // dirs, new UI, edit box
            callback:Some(folder_initialized),
            callback_data:initial.as_ref().map_or(0,|v|v.as_ptr() as isize), image:0 };
        let pidl = unsafe { SHBrowseForFolderW(&mut info) };
        if pidl.is_null() { return Ok(None); }
        // Shell PIDLs must always be released via CoTaskMemFree.
        let ok = unsafe { SHGetPathFromIDListW(pidl,buffer.as_mut_ptr()) };
        unsafe { CoTaskMemFree(pidl); }
        if ok == 0 { return Err(io::Error::other("failed to resolve selected folder path")); }
        let len = buffer.iter().position(|&c|c == 0).unwrap_or(buffer.len());
        Ok(Some(PathBuf::from(OsString::from_wide(&buffer[..len]))))
    }
}

fn append_missing_extension(path: &mut PathBuf, extension: &str) {
    if path.extension().is_none_or(|current| current.is_empty()) {
        path.set_extension(extension);
    }
}

fn normalized_extension(value: &str) -> io::Result<String> {
    let trimmed = value.trim().trim_start_matches('.');
    if trimmed.is_empty() || !trimmed.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-') {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("invalid file extension: {value:?}")));
    }
    Ok(trimmed.to_owned())
}

fn wide(value: &std::ffi::OsStr) -> Vec<u16> {
    value.encode_wide().chain(std::iter::once(0)).collect()
}

fn filter_buffer(extensions: &[String]) -> Vec<u16> { filter_buffer_named(extensions,&[]) }
fn filter_buffer_named(extensions: &[String], named: &[(String,Vec<String>)]) -> Vec<u16> {
    // Win32 requires a pair of display label and pattern strings followed
    // by two NULs. The extension list is deliberately caller-provided.
    let mut parts = Vec::new();
    if !extensions.is_empty() {
        let patterns = extensions.iter().map(|ext| format!("*.{ext}")).collect::<Vec<_>>().join(";");
        parts.push(format!("Supported Files ({patterns})"));
        parts.push(patterns);
    }
    for (name, extensions) in named {
        if !extensions.is_empty() {
            parts.push(name.clone());
            parts.push(extensions.iter().map(|e|format!("*.{e}")).collect::<Vec<_>>().join(";"));
        }
    }
    parts.push("All Files (*.*)".to_owned());
    parts.push("*.*".to_owned());
    let mut result = Vec::new();
    for part in parts {
        result.extend(part.encode_utf16());
        result.push(0);
    }
    result.push(0);
    result
}

fn parse_selected_files(file: &[u16], multiple: bool) -> io::Result<Vec<PathBuf>> {
    let first_end = file.iter().position(|ch| *ch == 0)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "dialog returned an unterminated path"))?;
    if first_end == 0 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "dialog returned an empty path"));
    }
    let first = PathBuf::from(OsString::from_wide(&file[..first_end]));
    if !multiple || file.get(first_end + 1).copied().unwrap_or(0) == 0 {
        return Ok(vec![first]);
    }
    let mut paths = Vec::new();
    let mut start = first_end + 1;
    while start < file.len() && file[start] != 0 {
        let end = file[start..].iter().position(|ch| *ch == 0)
            .map(|len| start + len)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "unterminated multiple-file selection"))?;
        let filename = PathBuf::from(OsString::from_wide(&file[start..end]));
        paths.push(first.join(filename));
        start = end + 1;
    }
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_validation() {
        assert_eq!(normalized_extension(".docx").unwrap(), "docx");
        assert_eq!(normalized_extension("png").unwrap(), "png");
        for invalid in ["", ".", "*.txt", "docx;exe", "a/b", "x\0y"] {
            assert!(normalized_extension(invalid).is_err(), "{invalid:?}");
        }
    }

    #[test]
    fn filters_are_double_nul_terminated() {
        let data = filter_buffer(&["docx".into(), "txt".into()]);
        assert!(data.ends_with(&[0, 0]));
        let decoded = String::from_utf16_lossy(&data);
        assert!(decoded.contains("*.docx;*.txt"));
    }

    #[test]
    fn save_extension_adds_only_if_missing() {
        let mut without = PathBuf::from("report");
        append_missing_extension(&mut without, "docx");
        assert_eq!(without, PathBuf::from("report.docx"));
        let mut explicit = PathBuf::from("report.txt");
        append_missing_extension(&mut explicit, "docx");
        assert_eq!(explicit, PathBuf::from("report.txt"));
    }

    #[test]
    fn selection_parses_single_path() {
        let path: Vec<u16> = "C:\\temp\\letter.docx\0\0".encode_utf16().collect();
        assert_eq!(parse_selected_files(&path, false).unwrap(), vec![PathBuf::from("C:\\temp\\letter.docx")]);
    }

    #[test]
    fn selection_parses_multiple_files() {
        let path: Vec<u16> = "C:\\temp\0one.docx\0two.txt\0\0".encode_utf16().collect();
        assert_eq!(parse_selected_files(&path, true).unwrap(), vec![PathBuf::from("C:\\temp\\one.docx"), PathBuf::from("C:\\temp\\two.txt")]);
    }
}
