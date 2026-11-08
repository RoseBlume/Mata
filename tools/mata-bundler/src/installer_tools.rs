//! Tauri-style installer tool selection: environment override, cached copy,
//! installed tool, then pinned and verified Windows toolchain download.
use std::{env, fs, path::{Path, PathBuf}};
#[cfg(windows)]
use std::{process::Command, time::{SystemTime, UNIX_EPOCH}};
use crate::{bail, cli::Options, tools, Result};

#[cfg(windows)]
const NSIS_URL: &str = "https://github.com/tauri-apps/binary-releases/releases/download/nsis-3.13/nsis-3.13.zip";
#[cfg(windows)]
const NSIS_SHA1: &str = "DB14F8AF2EA346B786C6AE19343F61AAF5E09B8E";
#[cfg(windows)]
const WIX_URL: &str = "https://github.com/wixtoolset/wix3/releases/download/wix3141rtm/wix314-binaries.zip";
#[cfg(windows)]
const WIX_SHA256: &str = "6ac824e1642d6f7277d0ed7ea09411a508f6116ba6fae0aa5f2c7daa2ff43d31";

#[derive(Debug)]
pub enum MsiTool {
    Wix4(PathBuf),
    Wix3 { candle: PathBuf, light: PathBuf },
    Wixl(PathBuf),
}
impl MsiTool {
    pub fn description(&self) -> String {
        match self {
            Self::Wix4(p) => format!("WiX 4+ ({})", p.display()),
            Self::Wix3 { candle, .. } => format!("WiX 3 ({})", candle.display()),
            Self::Wixl(p) => format!("wixl ({})", p.display()),
        }
    }
}
fn exe(name: &str) -> String { format!("{name}{}", if cfg!(windows) { ".exe" } else { "" }) }
fn hinted_executable(hint: &Path, name: &str) -> Option<PathBuf> {
    if hint.is_file() { return Some(hint.to_path_buf()); }
    [hint.join(exe(name)), hint.join("Bin").join(exe(name))]
        .into_iter().find(|p| p.is_file())
}
fn configured(var: &str, name: &str) -> Result<Option<PathBuf>> {
    match env::var_os(var) {
        None => Ok(None),
        Some(value) => {
            let dir = PathBuf::from(value);
            hinted_executable(&dir, name).map(Some).ok_or_else(||
                format!("{var} is set to {}, but '{name}' does not exist there", dir.display()).into())
        }
    }
}
pub fn cache_dir() -> PathBuf {
    if let Some(value) = env::var_os("MATA_BUNDLER_TOOLS_DIR") { return PathBuf::from(value); }
    #[cfg(windows)] {
        if let Some(dir) = env::var_os("LOCALAPPDATA").or_else(|| env::var_os("APPDATA")) {
            return PathBuf::from(dir).join("mata-bundler");
        }
    }
    #[cfg(target_os = "macos")] {
        if let Some(dir) = env::var_os("HOME") {
            return PathBuf::from(dir).join("Library/Caches/mata-bundler");
        }
    }
    if let Some(dir) = env::var_os("XDG_CACHE_HOME") {
        if !dir.is_empty() { return PathBuf::from(dir).join("mata-bundler"); }
    }
    if let Some(dir) = env::var_os("HOME").or_else(|| env::var_os("USERPROFILE")) {
        return PathBuf::from(dir).join(".cache/mata-bundler");
    }
    env::temp_dir().join("mata-bundler-tools")
}
fn nsis_in(dir: &Path) -> Option<PathBuf> {
    let p = hinted_executable(dir, "makensis")?;
    let root = if p.parent()?.file_name().is_some_and(|name| name == "Bin") {
        p.parent()?.parent()?
    } else { p.parent()? };
    if root.join("Include/MUI2.nsh").is_file() { Some(p) } else { None }
}
fn wix3_in(dir: &Path) -> Option<MsiTool> {
    Some(MsiTool::Wix3 {
        candle: hinted_executable(dir, "candle")?,
        light: hinted_executable(dir, "light")?,
    })
}

pub fn nsis(opts: &Options) -> Result<PathBuf> {
    if let Some(p) = configured("MATA_BUNDLER_NSIS_PATH", "makensis")? { return Ok(p); }
    #[cfg(windows)]
    if let Some(p) = configured("NSIS_PATH", "makensis")? { return Ok(p); }
    if let Some(p) = nsis_in(&cache_dir().join("NSIS-3.13")) { return Ok(p); }
    if let Some(p) = tools::program_path("makensis") { return Ok(p); }
    #[cfg(windows)] {
        for var in ["ProgramFiles(x86)", "ProgramFiles"] {
            if let Some(dir) = env::var_os(var) {
                if let Some(p) = nsis_in(&PathBuf::from(dir).join("NSIS")) { return Ok(p); }
            }
        }
        let dir = cache_dir().join("NSIS-3.13");
        download_archive(&dir, NSIS_URL, NSIS_SHA1, Hash::Sha1, Some("nsis-3.13"), opts.offline,
            &["makensis.exe", "Include/MUI2.nsh", "Include/FileFunc.nsh"])?;
        return nsis_in(&dir).ok_or_else(|| "downloaded NSIS is incomplete".into());
    }
    #[cfg(not(windows))] {
        let _ = opts;
        bail("NSIS requires makensis on this platform; set MATA_BUNDLER_NSIS_PATH to override")
    }
}

pub fn msi(opts: &Options) -> Result<MsiTool> {
    if let Some(p) = configured("MATA_BUNDLER_WIX_PATH", "wix")? { return Ok(MsiTool::Wix4(p)); }
    if let Some(candle) = configured("MATA_BUNDLER_WIX3_PATH", "candle")? {
        let light = hinted_executable(candle.parent().ok_or("invalid WiX 3 location")?, "light")
            .ok_or("MATA_BUNDLER_WIX3_PATH requires both candle and light")?;
        return Ok(MsiTool::Wix3 { candle, light });
    }
    if let Some(p) = configured("MATA_BUNDLER_WIXL_PATH", "wixl")? { return Ok(MsiTool::Wixl(p)); }
    #[cfg(windows)] {
        let cache = cache_dir().join("WixTools314");
        if let Some(tool) = wix3_in(&cache) { return Ok(tool); }
        if let Some(p) = tools::program_path("wix") { return Ok(MsiTool::Wix4(p)); }
        if let (Some(candle), Some(light)) = (tools::program_path("candle"), tools::program_path("light")) {
            return Ok(MsiTool::Wix3 { candle, light });
        }
        if let Some(p) = tools::program_path("wixl") { return Ok(MsiTool::Wixl(p)); }
        download_archive(&cache, WIX_URL, WIX_SHA256, Hash::Sha256, None, opts.offline,
            &["candle.exe", "light.exe", "wix.dll", "winterop.dll"])?;
        return wix3_in(&cache).ok_or_else(|| "downloaded WiX 3 is incomplete".into());
    }
    #[cfg(not(windows))] {
        let _ = opts;
        if let Some(p) = tools::program_path("wixl") { return Ok(MsiTool::Wixl(p)); }
        if let Some(p) = tools::program_path("wix") { return Ok(MsiTool::Wix4(p)); }
        bail("MSI packaging requires wixl or wix; specify MATA_BUNDLER_WIXL_PATH or MATA_BUNDLER_WIX_PATH")
    }
}

#[cfg(windows)]
enum Hash { Sha1, Sha256 }

#[cfg(windows)]
fn powershell(script:&str, vars:&[(&str,&str)]) -> Result<()> {
    // No interpolated user data in script: pass paths/URLs via child environment.
    let shell=tools::program_path("powershell.exe").or_else(||tools::program_path("pwsh.exe"))
        .ok_or("PowerShell 5+ is required for first-time toolchain downloads on Windows")?;
    let mut cmd=Command::new(shell);
    cmd.args(["-NoProfile","-NonInteractive","-ExecutionPolicy","Bypass","-Command",script]);
    for &(key,value) in vars {cmd.env(key,value);}
    let result=cmd.output()?;
    if !result.status.success(){return bail(format!("installer toolchain PowerShell command failed: {}",
        String::from_utf8_lossy(&result.stderr)));}
    Ok(())
}
#[cfg(windows)]
fn download_archive(dest:&Path,url:&str,digest:&str,hash:Hash,nested:Option<&str>,offline:bool,expected:&[&str])->Result<()> {
    if offline{return bail(format!("installer tools missing from {} and --offline prevents download",dest.display()));}
    let parent=dest.parent().ok_or("toolchain cache has no parent")?;
    fs::create_dir_all(parent)?;
    let stamp=SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let staging=parent.join(format!(".mata-{}-{stamp}",std::process::id()));
    fs::create_dir(&staging)?;
    let archive=staging.join("toolchain.zip");let unpack=staging.join("unpack");
    println!("Downloading installer toolchain {url}");
    let result=(||->Result<()> {
        powershell(r#"$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue';
            [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12;
            (New-Object Net.WebClient).DownloadFile($env:MATA_URL, $env:MATA_ZIP)"#,
            &[("MATA_URL",url),("MATA_ZIP",&archive.to_string_lossy())])?;
        if fs::metadata(&archive)?.len()>256*1024*1024{return bail("installer archive exceeds 256 MiB");}
        let algorithm=match hash {Hash::Sha1=>"SHA1",Hash::Sha256=>"SHA256"};
        powershell(r#"$ErrorActionPreference='Stop';
            $actual=(Get-FileHash -LiteralPath $env:MATA_ZIP -Algorithm $env:MATA_ALGORITHM).Hash;
            if($actual -ine $env:MATA_DIGEST){throw 'installer toolchain hash mismatch'};
            Add-Type -AssemblyName System.IO.Compression.FileSystem;
            [System.IO.Compression.ZipFile]::ExtractToDirectory($env:MATA_ZIP, $env:MATA_UNPACK)"#,
            &[("MATA_ZIP",&archive.to_string_lossy()),("MATA_DIGEST",digest),("MATA_ALGORITHM",algorithm),
              ("MATA_UNPACK",&unpack.to_string_lossy())])?;
        let inner=nested.map(|p|unpack.join(p)).unwrap_or_else(||unpack.clone());
        for file in expected{if !inner.join(file).is_file(){return bail(format!("tool archive missing {file}"));}}
        if dest.exists(){fs::remove_dir_all(dest)?;}
        fs::rename(inner,dest)?;Ok(())
    })();
    let _=fs::remove_dir_all(staging);result
}

pub fn doctor() {
    println!("Installer cache: {}", cache_dir().display());
    let offline = Options { offline: true, ..Options::with_defaults() };
    match nsis(&offline) {
        Ok(p) => println!("NSIS: {}", p.display()),
        Err(err) => println!("NSIS: unavailable ({err})"),
    }
    match msi(&offline) {
        Ok(tool) => println!("MSI: {}", tool.description()),
        Err(err) => println!("MSI: unavailable ({err})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detects_complete_wix3_installation() {
        let temp = env::temp_dir().join(format!("mata-wix-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).unwrap();
        fs::write(temp.join(exe("candle")), b"").unwrap();
        assert!(wix3_in(&temp).is_none());
        fs::write(temp.join(exe("light")), b"").unwrap();
        assert!(matches!(wix3_in(&temp), Some(MsiTool::Wix3 { .. })));
        fs::remove_dir_all(temp).unwrap();
    }
}
