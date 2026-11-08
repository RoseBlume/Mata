//! Read the operating system's preferred light/dark appearance.
//!
//! A background monitor lets Mata apps respond to changes without blocking
//! the UI thread on D-Bus, the registry, or desktop helpers.

use std::{sync::{Arc, atomic::{AtomicBool, Ordering}, mpsc::{self, Receiver}}, time::Duration};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemColorScheme { Light, Dark }

impl SystemColorScheme {
    pub const fn is_dark(self) -> bool { matches!(self, Self::Dark) }
}

/// Returns `None` if the system does not expose a recognizable preference.
/// Applications may fall back to light mode in that case.
pub fn system_color_scheme() -> Option<SystemColorScheme> {
    #[cfg(target_os = "linux")]
    { linux::detect() }
    #[cfg(target_os = "windows")]
    { windows::detect() }
    #[cfg(target_os = "macos")]
    { macos::detect() }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    { None }
}

/// Poll on a worker thread, delivering a new value only when it changes.
/// `try_recv` never blocks the GUI event loop. Dropping the monitor causes
/// its worker to exit after the next poll interval.
pub struct SystemAppearanceMonitor {
    receiver: Receiver<Option<SystemColorScheme>>,
    stopped: Arc<AtomicBool>,
}
impl SystemAppearanceMonitor {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        let stopped = Arc::new(AtomicBool::new(false));
        let worker_stopped = Arc::clone(&stopped);
        std::thread::spawn(move || {
            let mut previous = None;
            while !worker_stopped.load(Ordering::Acquire) {
                let current = system_color_scheme();
                if previous != Some(current) {
                    if sender.send(current).is_err() { break; }
                    previous = Some(current);
                }
                std::thread::sleep(Duration::from_secs(2));
            }
        });
        Self { receiver, stopped }
    }

    /// Return the most recent changed preference, if any.
    pub fn poll(&self) -> Option<Option<SystemColorScheme>> {
        let mut latest = None;
        while let Ok(value) = self.receiver.try_recv() { latest = Some(value); }
        latest
    }
}
impl Default for SystemAppearanceMonitor {
    fn default() -> Self { Self::new() }
}
impl Drop for SystemAppearanceMonitor {
    fn drop(&mut self) { self.stopped.store(true, Ordering::Release); }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::SystemColorScheme as Scheme;
    use std::{env, fs, path::PathBuf, process::Command};
    use super::super::dbus_std::{Session, Writer, PORTAL_DEST, PORTAL_OBJECT};

    fn config_home() -> Option<PathBuf> {
        if let Some(path) = env::var_os("XDG_CONFIG_HOME").filter(|p| !p.is_empty()) {
            return Some(PathBuf::from(path));
        }
        env::var_os("HOME").map(|home| PathBuf::from(home).join(".config"))
    }

    fn portal() -> Option<Scheme> {
        let mut session=Session::connect(std::time::Duration::from_secs(3)).ok()?;
        let mut args=Writer::new();
        args.string("org.freedesktop.appearance");
        args.string("color-scheme");
        let reply=session.call(PORTAL_DEST,PORTAL_OBJECT,
            "org.freedesktop.portal.Settings","Read","ss",args).ok()?;
        if reply.signature.as_deref()!=Some("v") {return None;}
        match reply.reader().variant_u32().ok()? {
            1=>Some(Scheme::Dark),
            2=>Some(Scheme::Light),
            _=>None, // No preference; try desktop-specific sources.
        }
    }

    fn cosmic() -> Option<Scheme> {
        let file = config_home()?.join("cosmic/com.system76.CosmicTheme.Mode/v1/is_dark");
        match fs::read_to_string(file).ok()?.trim() {
            "true" => Some(Scheme::Dark),
            "false" => Some(Scheme::Light),
            _ => None,
        }
    }

    fn theme_name(text: &str) -> Option<Scheme> {
        let v = text.trim_matches(|c: char| c.is_whitespace() || c == '\'' || c == '"').to_ascii_lowercase();
        if v.contains("dark") { Some(Scheme::Dark) }
        else if v.contains("light") { Some(Scheme::Light) }
        else { None }
    }

    fn kde() -> Option<Scheme> {
        let ini = fs::read_to_string(config_home()?.join("kdeglobals")).ok()?;
        let mut section = "";
        let mut colors = None;
        for line in ini.lines().map(str::trim) {
            if line.starts_with('[') && line.ends_with(']') {
                section = &line[1..line.len()-1];
                continue;
            }
            let Some((key, value)) = line.split_once('=') else { continue };
            if section == "General" && key.trim() == "ColorScheme" {
                if let Some(theme) = theme_name(value) { return Some(theme) }
            }
            if section == "Colors:Window" && key.trim() == "BackgroundNormal" {
                colors = parse_rgb(value);
            }
        }
        colors.map(|(r,g,b)| if (r as u32 * 2126 + g as u32 * 7152 + b as u32 * 722) / 10000 < 125 {
            Scheme::Dark
        } else { Scheme::Light })
    }

    fn parse_rgb(value: &str) -> Option<(u8,u8,u8)> {
        let mut parts = value.split(',').map(|x|x.trim().parse::<u8>().ok());
        Some((parts.next()??, parts.next()??, parts.next()??))
    }

    fn gnome() -> Option<Scheme> {
        let output = Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "color-scheme"])
            .output().ok()?;
        if output.status.success() {
            let value = String::from_utf8_lossy(&output.stdout);
            if let Some(scheme) = theme_name(&value) { return Some(scheme); }
        }
        let output = Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "gtk-theme"])
            .output().ok()?;
        if output.status.success() {
            return theme_name(&String::from_utf8_lossy(&output.stdout));
        }
        None
    }

    fn gtk() -> Option<Scheme> {
        let home = config_home()?;
        for name in ["gtk-4.0/settings.ini", "gtk-3.0/settings.ini"] {
            if let Ok(content) = fs::read_to_string(home.join(name)) {
                for line in content.lines().map(str::trim) {
                    if let Some(value) = line.strip_prefix("gtk-application-prefer-dark-theme=") {
                        return match value.trim() { "1" => Some(Scheme::Dark), "0" => Some(Scheme::Light), _ => None };
                    }
                    if let Some(value) = line.strip_prefix("gtk-theme-name=") {
                        if let Some(scheme) = theme_name(value) { return Some(scheme); }
                    }
                }
            }
        }
        None
    }

    pub(super) fn detect() -> Option<Scheme> {
        portal().or_else(cosmic).or_else(kde).or_else(gnome).or_else(gtk)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn detects_common_theme_names() {
            assert_eq!(theme_name("'Adwaita-dark'"), Some(Scheme::Dark));
            assert_eq!(theme_name("BreezeLight"), Some(Scheme::Light));
            assert_eq!(theme_name("Breeze"), None);
        }
        #[test]
        fn parses_kde_rgb() {
            assert_eq!(parse_rgb("28, 30, 35"), Some((28,30,35)));
            assert_eq!(parse_rgb("nope"), None);
        }
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::SystemColorScheme as Scheme;
    use std::{ffi::c_void, iter, ptr};
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegGetValueW(hkey: *mut c_void, subkey: *const u16, value: *const u16,
            flags: u32, kind: *mut u32, data: *mut c_void, size: *mut u32) -> i32;
    }
    pub(super) fn detect() -> Option<Scheme> {
        let utf16 = |s: &str| s.encode_utf16().chain(iter::once(0)).collect::<Vec<u16>>();
        let subkey = utf16("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
        let name = utf16("AppsUseLightTheme");
        let mut value = 0u32;
        let mut size = std::mem::size_of::<u32>() as u32;
        let status = unsafe {
            // HKEY_CURRENT_USER = 0x80000001; RRF_RT_REG_DWORD = 0x18.
            RegGetValueW(0x8000_0001usize as *mut c_void, subkey.as_ptr(), name.as_ptr(),
                0x18, ptr::null_mut(), (&mut value as *mut u32).cast(), &mut size)
        };
        if status == 0 && size == 4 {
            Some(if value == 0 { Scheme::Dark } else { Scheme::Light })
        } else { None }
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::SystemColorScheme as Scheme;
    use std::process::Command;
    pub(super) fn detect() -> Option<Scheme> {
        let result = Command::new("defaults")
            .args(["read", "-g", "AppleInterfaceStyle"])
            .output().ok()?;
        if !result.status.success() {
            // The preference is absent on a standard light desktop.
            return Some(Scheme::Light);
        }
        if String::from_utf8_lossy(&result.stdout).trim().eq_ignore_ascii_case("Dark") {
            Some(Scheme::Dark)
        } else { Some(Scheme::Light) }
    }
}
