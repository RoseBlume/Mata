//! Linux file picker: XDG Desktop Portal (D-Bus) first, then graphical
//! helpers when a portal is unavailable. A user cancellation is final.
//!
//! All calls are synchronous: invoke from a UI action, not a render callback.
//! Cancellations return `Ok(None)`; unavailable backends return `Unsupported`.
use std::{env, io, path::PathBuf, process::Command};
use super::window::WidgetWindow;
use super::file_dialog_portal;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind { Open, Save, Folder }

/// Linux dialog provider. `Auto` tries XDG Desktop Portal first and falls back
/// to a desktop-compatible command helper only if the portal is unavailable.
/// `Portal` forces D-Bus and never launches another dialog.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DialogBackend { #[default] Auto, Portal, Zenity, KDialog, Yad, Qarma }

#[derive(Clone, Debug)]
pub(super) struct FileFilter { pub(super) name: String, pub(super) extensions: Vec<String> }

pub struct FileDialog<'a> {
    pub(super) window: &'a WidgetWindow,
    pub(super) kind: Kind,
    pub(super) filters: Vec<FileFilter>,
    pub(super) extension: Option<String>,
    pub(super) title: Option<String>,
    pub(super) initial: Option<PathBuf>,
    pub(super) suggested_name: Option<String>,
    backend: DialogBackend,
    show_hidden: bool,
    pub(super) portal_parent: Option<String>,
}
fn dialog(window: &WidgetWindow, kind: Kind) -> FileDialog<'_> {
    FileDialog { window, kind, filters: Vec::new(), extension: None,
        title: None, initial: None, suggested_name: None, backend: DialogBackend::Auto,
        show_hidden: false, portal_parent: None }
}
/// Select one file by calling `.show()`, or several using `.show_files()`.
pub fn open_files_dialog(window: &WidgetWindow) -> FileDialog<'_> { dialog(window, Kind::Open) }
/// Alias for the single-file picker.
pub fn open_file_dialog(window: &WidgetWindow) -> FileDialog<'_> { open_files_dialog(window) }
/// Select a save destination; no file is created automatically.
pub fn save_file_dialog(window: &WidgetWindow) -> FileDialog<'_> { dialog(window, Kind::Save) }
/// Select an existing directory.
pub fn open_folder_dialog(window: &WidgetWindow) -> FileDialog<'_> { dialog(window, Kind::Folder) }
/// Select multiple existing directories if the provider supports it.
pub fn open_folders_dialog(window: &WidgetWindow) -> FileDialog<'_> { open_folder_dialog(window) }

impl FileDialog<'_> {
    /// Replace all filters with a group of extensions (e.g. ["rs", "toml"]).
    pub fn filter_extensions<I, S>(mut self, items: I) -> Self
    where I: IntoIterator<Item=S>, S: AsRef<str> {
        self.filters.clear();
        self.add_filter_in_place("Supported files", items);
        self
    }
    /// Add a named file-type filter alongside any existing filters.
    pub fn add_filter<I, S>(mut self, name: impl Into<String>, items: I) -> Self
    where I: IntoIterator<Item=S>, S: AsRef<str> {
        self.add_filter_in_place(name, items); self
    }
    fn add_filter_in_place<I,S>(&mut self, name: impl Into<String>, items: I)
    where I: IntoIterator<Item=S>, S: AsRef<str> {
        self.filters.push(FileFilter { name: name.into(),
            extensions: items.into_iter().map(|s| s.as_ref().trim_start_matches('.').to_owned())
                .filter(|s| !s.is_empty()).collect() });
    }
    pub fn extension(mut self, value: impl AsRef<str>) -> Self {
        self.extension = Some(value.as_ref().trim_start_matches('.').to_owned()); self
    }
    pub fn title(mut self, value: impl Into<String>) -> Self { self.title=Some(value.into());self }
    pub fn initial_path(mut self, value: impl Into<PathBuf>) -> Self { self.initial=Some(value.into());self }
    pub fn initial_directory(self, value: impl Into<PathBuf>) -> Self { self.initial_path(value) }
    pub fn suggested_name(mut self, value: impl Into<String>) -> Self { self.suggested_name=Some(value.into());self }
    pub fn backend(mut self, value: DialogBackend) -> Self { self.backend=value;self }
    /// Overrides the portal window identifier (e.g. `x11:1a00007` or a valid
    /// exported Wayland handle `wayland:...`). Empty string disables parenting.
    /// By default X11 is detected from the native window, while Wayland
    /// currently opens the chooser without a parent (no xdg-foreign export).
    pub fn portal_parent_window(mut self, value: impl Into<String>) -> Self {
        self.portal_parent = Some(value.into()); self
    }
    /// Ask a GTK-based provider to display hidden files. Other providers have
    /// native hidden-file controls and may ignore this hint.
    pub fn show_hidden(mut self, value: bool) -> Self { self.show_hidden=value;self }
    pub fn show(self) -> io::Result<Option<PathBuf>> {
        Ok(self.run(false)?.and_then(|v|v.into_iter().next()))
    }
    pub fn show_files(self) -> io::Result<Option<Vec<PathBuf>>> {
        if self.kind != Kind::Open { return Err(io::Error::new(io::ErrorKind::InvalidInput,
            "show_files requires an open-file dialog")); }
        self.run(true)
    }
    pub fn show_folders(self) -> io::Result<Option<Vec<PathBuf>>> {
        if self.kind != Kind::Folder { return Err(io::Error::new(io::ErrorKind::InvalidInput,
            "show_folders requires a folder dialog")); }
        self.run(true)
    }
    fn run(self, multiple: bool) -> io::Result<Option<Vec<PathBuf>>> {
        self.validate()?;
        // Do not reopen a chooser when the portal returned Cancel, or when a
        // request was already accepted and later failed. Fallback only on an
        // unavailable/unsupported portal before the dialog is actually shown.
        if matches!(self.backend, DialogBackend::Auto | DialogBackend::Portal) {
            match file_dialog_portal::run(&self, multiple) {
                Ok(paths) => return self.finish(paths),
                Err(file_dialog_portal::PortalError::Unavailable(error))
                    if self.backend == DialogBackend::Auto => {
                    // Best-effort helpers for desktop sessions without a portal.
                    // If no helper exists, retain the useful D-Bus error context.
                    if let Ok(provider) = self.choose_backend(multiple) {
                        return self.run_helper(provider, multiple);
                    }
                    return Err(error);
                }
                Err(error) => return Err(error.into_io()),
            }
        }
        let provider = self.choose_backend(multiple)?;
        self.run_helper(provider, multiple)
    }
    fn finish(&self, paths: Option<Vec<PathBuf>>) -> io::Result<Option<Vec<PathBuf>>> {
        let Some(mut paths) = paths else { return Ok(None); };
        if self.kind == Kind::Save {
            if let Some(ext) = &self.extension {
                if let Some(path) = paths.first_mut() {
                    if path.extension().is_none() { path.set_extension(ext); }
                }
            }
        }
        Ok(Some(paths))
    }
    fn run_helper(&self, provider: DialogBackend, multiple: bool) -> io::Result<Option<Vec<PathBuf>>> {
        let result = match provider {
            DialogBackend::KDialog => self.kdialog(multiple).output()?,
            DialogBackend::Zenity | DialogBackend::Qarma | DialogBackend::Yad =>
                self.gtk_helper(provider, multiple).output()?,
            DialogBackend::Auto | DialogBackend::Portal => unreachable!(),
        };
        if !result.status.success() {
            if result.status.code() == Some(1) { return Ok(None); }
            return Err(io::Error::other(format!("dialog failed ({}): {}",
                result.status, String::from_utf8_lossy(&result.stderr))));
        }
        let stdout = String::from_utf8(result.stdout).map_err(|err|
            io::Error::new(io::ErrorKind::InvalidData, err))?;
        let paths = stdout.trim_end_matches(['\r','\n','\0']).split('\n')
            .filter(|s| !s.is_empty()).map(PathBuf::from).collect::<Vec<_>>();
        if paths.is_empty() { return Ok(None); }
        self.finish(Some(paths))
    }
    fn validate(&self) -> io::Result<()> {
        for ext in self.filters.iter().flat_map(|filter| &filter.extensions)
            .chain(self.extension.iter()) {
            if !valid_extension(ext) { return Err(io::Error::new(io::ErrorKind::InvalidInput,
                format!("invalid extension: {ext:?}"))); }
        }
        if self.filters.iter().any(|f| f.name.contains(['\n','\0','|'])) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput,"invalid filter name"));
        }
        if self.portal_parent.as_ref().is_some_and(|s| s.contains('\0') ||
            (!s.is_empty() && !s.starts_with("x11:") && !s.starts_with("wayland:"))) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput,
                "portal parent window must be an x11: or wayland: handle"));
        }
        if self.title.as_ref().is_some_and(|s|s.contains('\0'))
            || self.suggested_name.as_ref().is_some_and(|s|s.contains(['\0','/'])) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput,"invalid dialog title or filename"));
        }
        Ok(())
    }
    fn choose_backend(&self, multiple: bool) -> io::Result<DialogBackend> {
        let supports = |b| !(multiple && self.kind == Kind::Folder && b == DialogBackend::KDialog);
        if !matches!(self.backend, DialogBackend::Auto | DialogBackend::Portal) {
            if !supports(self.backend) { return Err(io::Error::new(io::ErrorKind::Unsupported,
                "kdialog does not support multi-folder selection")); }
            if available(executable(self.backend)) { return Ok(self.backend); }
            return Err(io::Error::new(io::ErrorKind::Unsupported, "requested file-dialog provider is not installed"));
        }
        let desktop = env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_uppercase();
        let candidates = if desktop.contains("KDE") || desktop.contains("PLASMA") {
            [DialogBackend::KDialog, DialogBackend::Zenity, DialogBackend::Yad, DialogBackend::Qarma]
        } else {
            [DialogBackend::Zenity, DialogBackend::Yad, DialogBackend::Qarma, DialogBackend::KDialog]
        };
        candidates.into_iter().find(|&b| supports(b) && available(executable(b)))
            .ok_or_else(|| io::Error::new(io::ErrorKind::Unsupported,
                "no compatible graphical file dialog: install zenity, kdialog, yad or qarma"))
    }
    fn initial_filename(&self) -> Option<PathBuf> {
        let mut path=self.initial.clone().unwrap_or_default();
        if let Some(name) = &self.suggested_name { path.push(name); }
        if path.as_os_str().is_empty() { None } else { Some(path) }
    }
    fn gtk_helper(&self, provider: DialogBackend, multiple: bool) -> Command {
        let mut cmd=Command::new(executable(provider));
        if provider == DialogBackend::Yad { cmd.arg("--file"); }
        else { cmd.arg("--file-selection"); }
        if self.kind==Kind::Save {cmd.args(["--save","--confirm-overwrite"]);}
        if self.kind==Kind::Folder {cmd.arg("--directory");}
        if multiple {cmd.arg("--multiple").arg("--separator=\n");}
        if self.show_hidden {cmd.arg("--show-hidden");}
        if let Some(title)=&self.title {cmd.arg("--title").arg(title);}
        if let Some(initial)=self.initial_filename() {cmd.arg("--filename").arg(initial);}
        if self.kind != Kind::Folder {
            for filter in &self.filters {
                if !filter.extensions.is_empty() {
                    cmd.arg("--file-filter").arg(format!("{} | {}", filter.name,
                        filter.extensions.iter().map(|e|format!("*.{e}")).collect::<Vec<_>>().join(" ")));
                }
            }
        }
        cmd
    }
    fn kdialog(&self, multiple: bool) -> Command {
        let mut cmd=Command::new("kdialog");
        cmd.arg(match self.kind {
            Kind::Save => "--getsavefilename",
            Kind::Folder => "--getexistingdirectory",
            Kind::Open => "--getopenfilename",
        });
        cmd.arg(self.initial_filename().map(|p|p.to_string_lossy().into_owned()).unwrap_or_default());
        if self.kind != Kind::Folder {
            let filters=self.filters.iter().filter(|f|!f.extensions.is_empty()).map(|f|format!("{}|{}",
                f.extensions.iter().map(|e|format!("*.{e}")).collect::<Vec<_>>().join(" "),f.name))
                .collect::<Vec<_>>().join("\n");
            cmd.arg(filters);
        }
        if multiple {cmd.args(["--multiple","--separate-output"]);}
        if let Some(title)=&self.title {cmd.arg("--title").arg(title);}
        cmd
    }
}
fn executable(backend: DialogBackend) -> &'static str {
    match backend {
        DialogBackend::Zenity => "zenity", DialogBackend::KDialog => "kdialog",
        DialogBackend::Yad => "yad", DialogBackend::Qarma => "qarma",
        DialogBackend::Auto | DialogBackend::Portal => "",
    }
}
fn available(program:&str)->bool {
    env::var_os("PATH").is_some_and(|path|env::split_paths(&path).any(|dir|dir.join(program).is_file()))
}
fn valid_extension(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c|c.is_ascii_alphanumeric() || c=='_' || c=='-')
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn extension_validation() {
        assert!(valid_extension("jpeg"));
        assert!(!valid_extension("*.sh"));
        assert!(!valid_extension("png;*.exe"));
    }
}
