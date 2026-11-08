use std::path::PathBuf;
use crate::{bail, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform { Android, Windows, Linux }
impl Platform {
    pub fn name(self) -> &'static str {
        match self { Self::Android => "android", Self::Windows => "windows", Self::Linux => "linux" }
    }
    pub fn host() -> Result<Self> {
        if cfg!(target_os = "windows") { Ok(Self::Windows) }
        else if cfg!(target_os = "linux") { Ok(Self::Linux) }
        else { bail("desktop bundling is supported on Windows and Linux hosts; use 'mata android build' for Android") }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bundle { Dpkg, Rpm, Msi, Nsis, Zip }
impl Bundle {
    pub fn parse(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "dpkg" | "deb" => Ok(Self::Dpkg), "rpm" => Ok(Self::Rpm),
            "msi" => Ok(Self::Msi), "nsis" => Ok(Self::Nsis), "zip" => Ok(Self::Zip),
            _ => bail(format!("unsupported bundle '{s}' (choose msi,nsis,dpkg,rpm,zip)")),
        }
    }
    pub fn platform(self) -> Platform {
        match self { Self::Dpkg | Self::Rpm => Platform::Linux, _ => Platform::Windows }
    }
    pub fn name(self) -> &'static str {
        match self { Self::Dpkg => "dpkg", Self::Rpm => "rpm", Self::Msi => "msi", Self::Nsis => "nsis", Self::Zip => "zip" }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AndroidInit { Native, Game }

#[derive(Debug, Default, Clone)]
pub struct Options {
    pub release: bool,
    pub unsigned: bool,
    pub package: Option<String>,
    pub target: Option<String>,
    pub abis: Vec<String>,
    pub bundles: Vec<Bundle>,
    pub output: Option<PathBuf>,
    pub features: Vec<String>,
    pub no_default_features: bool,
    pub offline: bool,
    pub verbose: bool,
    pub project_dir: Option<PathBuf>,
    pub device: Option<String>,
    pub poll_ms: u64,
    pub fps_file: Option<PathBuf>,
    pub android_init: Option<AndroidInit>,
}
impl Options { pub fn with_defaults() -> Self { Self { poll_ms: 650, ..Self::default() } } }

pub enum Command { Build(Platform, Options), Dev(Platform, Options), Monitor(Platform, Options), Init(Options), AndroidInit(AndroidInit, Options), Icon(PathBuf, Options), Doctor(Options), Help }

pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Command> {
    let args: Vec<_> = args.into_iter().collect();
    if args.is_empty() || matches!(args[0].as_str(), "--help" | "-h" | "help") { return Ok(Command::Help); }
    let (command, explicit_platform, start) = match args[0].as_str() {
        "android" | "windows" | "linux" => {
            let platform = match args[0].as_str() { "android" => Platform::Android, "windows" => Platform::Windows, _ => Platform::Linux };
            match args.get(1).map(String::as_str) {
                Some("build") => ("build", Some(platform), 2),
                Some("dev") => ("dev", Some(platform), 2),
                Some("monitor") if platform != Platform::Android => ("monitor", Some(platform), 2),
                Some("init") if platform == Platform::Android => ("android-init", Some(platform), 2),
                _ => return bail("expected 'build' or 'dev' after platform, or 'android init'"),
            }
        }
        "build" => ("build", None, 1),
        "dev" => ("dev", None, 1),
        "monitor" => ("monitor", None, 1),
        "init" => ("init", None, 1),
        "doctor" => ("doctor", None, 1),
        "icon" => ("icon", None, 1),
        _ => return bail(format!("unknown command '{}' (try mata --help)", args[0])),
    };
    let mut opts = Options::with_defaults();
    // Production bundles should use Cargo's optimized release profile.
    // A dev session defaults to the debug profile. Both can be overridden.
    opts.release = command == "build" || command == "monitor";
    if command == "monitor" { opts.poll_ms = 1000; }
    let mut icon_source: Option<PathBuf> = None;
    let mut i = start;
    while i < args.len() {
        let arg = args[i].as_str();
        match arg {
            "--release" => opts.release = true,
            "--debug" => opts.release = false,
            "--unsigned" => opts.unsigned = true,
            "--offline" => opts.offline = true,
            "--verbose" | "-v" => opts.verbose = true,
            "--no-default-features" => opts.no_default_features = true,
            "--native" => {
                if opts.android_init.replace(AndroidInit::Native).is_some() { return bail("choose exactly one of --native or --game"); }
            }
            "--game" => {
                if opts.android_init.replace(AndroidInit::Game).is_some() { return bail("choose exactly one of --native or --game"); }
            }
            "--package" | "-p" | "--target" | "--abi" | "--bundle" | "--out" | "--features" | "--project-dir" | "--device" | "--poll-ms" | "--fps-file" => {
                let value = args.get(i + 1).ok_or_else(|| format!("missing value for {arg}"))?;
                i += 1;
                match arg {
                    "--package" | "-p" => opts.package = Some(value.clone()),
                    "--target" => opts.target = Some(value.clone()),
                    "--abi" => opts.abis.push(value.clone()),
                    "--bundle" => {
                        for name in value.split(',') {
                            if name.trim().is_empty() { return bail("--bundle contains an empty format"); }
                            let bundle = Bundle::parse(name)?;
                            if !opts.bundles.contains(&bundle) { opts.bundles.push(bundle); }
                        }
                    }
                    "--out" => opts.output = Some(PathBuf::from(value)),
                    "--features" => opts.features.push(value.clone()),
                    "--project-dir" => opts.project_dir = Some(PathBuf::from(value)),
                    "--device" => opts.device = Some(value.clone()),
                    "--poll-ms" => opts.poll_ms = value.parse().map_err(|_| "--poll-ms must be a positive integer")?,
                    "--fps-file" => opts.fps_file = Some(PathBuf::from(value)),
                    _ => unreachable!(),
                }
            }
            _ if !arg.starts_with('-') && command == "icon" && icon_source.is_none() => {
                icon_source = Some(PathBuf::from(arg));
            }
            _ => return bail(format!("unknown option: {arg}")),
        }
        i += 1;
    }
    if opts.poll_ms < 100 { return bail("--poll-ms must be at least 100"); }
    if opts.fps_file.is_some() && command != "monitor" { return bail("--fps-file is only supported by mata monitor"); }
    if opts.android_init.is_some() && command != "android-init" { return bail("--native and --game are only valid for 'mata android init'"); }
    if command == "android-init" {
        let mode = opts.android_init.ok_or("choose 'mata android init --native' or 'mata android init --game'")?;
        if !opts.bundles.is_empty() || opts.target.is_some() { return bail("android init does not take --bundle or --target"); }
        return Ok(Command::AndroidInit(mode, opts));
    }
    if command == "init" { return Ok(Command::Init(opts)); }
    if command == "icon" {
        let source = icon_source.ok_or("usage: mata icon <icon.png>")?;
        if opts.android_init.is_some() || !opts.bundles.is_empty() || opts.target.is_some() || !opts.abis.is_empty() || opts.device.is_some() || opts.unsigned {
            return bail("mata icon only accepts the image path plus common options like --project-dir or --verbose");
        }
        return Ok(Command::Icon(source, opts));
    }
    if command == "doctor" { return Ok(Command::Doctor(opts)); }
    let mut platform = match explicit_platform { Some(p) => p, None => Platform::host()? };
    if !opts.bundles.is_empty() {
        if command != "build" { return bail("--bundle is only for mata build"); }
        let requested = opts.bundles[0].platform();
        if let Some(explicit) = explicit_platform {
            if opts.bundles.iter().any(|bundle| bundle.platform() != explicit) {
                return bail("--bundle conflicts with the explicitly selected platform");
            }
        }
        platform = requested;
    }
    if opts.unsigned && platform != Platform::Android { return bail("--unsigned is Android-only"); }
    if opts.unsigned && !opts.release { return bail("--unsigned requires --release"); }
    if !opts.abis.is_empty() && platform != Platform::Android { return bail("--abi is Android-only"); }
    if opts.target.is_some() && platform == Platform::Android { return bail("use --abi for Android targets"); }
    if opts.device.is_some() && (platform != Platform::Android || command != "dev") { return bail("--device is for mata android dev"); }
    if command == "monitor" && platform == Platform::Android { return bail("mata monitor supports Windows and Linux applications (not Android)"); }
    if command == "dev" && opts.release && platform == Platform::Android { return bail("mata android dev uses debug signing; omit --release"); }
    match command {
        "build" => Ok(Command::Build(platform, opts)),
        "dev" => Ok(Command::Dev(platform, opts)),
        "monitor" => Ok(Command::Monitor(platform, opts)),
        _ => unreachable!(),
    }
}

pub fn selected_bundles(platform: Platform, opts: &Options) -> Vec<Bundle> {
    if !opts.bundles.is_empty() { return opts.bundles.clone(); }
    match platform {
        Platform::Linux => vec![Bundle::Dpkg, Bundle::Rpm],
        Platform::Windows => vec![Bundle::Msi, Bundle::Nsis],
        Platform::Android => Vec::new(),
    }
}

pub const HELP: &str = r#"Mata Bundler — build and package Mata applications

  mata init                         Create Mata.toml
  mata android init --native        Configure and scaffold NativeActivity app
  mata android init --game          Configure and scaffold GameActivity app
  mata build [options]              Release build by default; host OS bundles
  mata build --bundle dpkg,rpm      Linux installers (from any host with cross toolchain)
  mata build --bundle msi,nsis      Windows installers (from any host with cross toolchain)
  mata build --bundle msi,rpm       Build both Windows MSI and Linux RPM
  mata build --bundle zip           Portable Windows ZIP
  mata dev [options]                Debug build by default; launch and watch for changes
  mata monitor [options]            Release build, profile until exit/app ends, save graphs
  mata android build [options]      Build Android APK
  mata android dev [options]        Install APK on device and watch/redeploy
  mata doctor                       List installed and cached toolchains
  mata icon <icon.png>              Generate desktop and Android icon assets

Options:
  --bundle FORMATS                 Comma-separated: msi,nsis,dpkg,rpm,zip (build only)
  mata icon writes icons/icon.ico, desktop PNG sizes, and Android mipmaps.
  --target TRIPLE                  Explicit desktop Rust target triple
  -p, --package NAME               Workspace Cargo package
  --project-dir PATH               Cargo project directory
  --release                         Force optimized release build (default for build/monitor)
  --debug                           Force debug build (default for dev)
  --fps-file PATH                   Optional application-written FPS value file for monitor
  --abi ABI                         Android ABI (repeatable)
  --device SERIAL                  ADB serial (Android dev)
  --out DIRECTORY                  Default target/mata/dist
  --features LIST                  Cargo features, comma-separated
  --no-default-features            Disable default Cargo features
  --offline                        Cargo/Gradle offline; skip installer tool downloads
  --unsigned                       Unsigned Android release APK
  --poll-ms NUMBER                 Dev interval (650ms); monitor sampling interval (1000ms)
  -v, --verbose                    Print commands

Linux package creation uses Rust libraries (no dpkg-deb/rpmbuild).
On Windows, missing WiX 3 or NSIS tools download to a verified user cache.
On Linux/macOS, MSI requires wixl or wix and NSIS requires makensis.
Cross compilation needs a working Rust target linker.
Monitor reports default to target/mata/monitor and require host-platform binaries.
Windows monitor builds automatically enable mata_win/telemetry to count
successful MataWin frame presentations. Other renderers can report FPS via
MATA_MONITOR_FPS_FILE. See MONITOR.md.
No PresentMon, ETW access, or external profiler is used.
"#;

#[cfg(test)]
mod tests {
    use super::*;
    fn p(s: &[&str]) -> Result<Command> { parse(s.iter().map(|s| s.to_string())) }
    #[test] fn android_development() {
        let Command::Dev(Platform::Android, opts) = p(&["android", "dev", "--device", "emulator-5554"]).unwrap() else { panic!() };
        assert_eq!(opts.device.as_deref(), Some("emulator-5554"));
    }
    #[test] fn android_init_modes() {
        assert!(matches!(p(&["android", "init", "--native"]).unwrap(), Command::AndroidInit(AndroidInit::Native, _)));
        assert!(matches!(p(&["android", "init", "--game"]).unwrap(), Command::AndroidInit(AndroidInit::Game, _)));
        assert!(p(&["android", "init"]).is_err());
        assert!(p(&["android", "init", "--native", "--game"]).is_err());
        assert!(p(&["build", "--game"]).is_err());
    }
    #[test] fn bundle_selection() {
        let Command::Build(Platform::Windows, options) = p(&["build", "--bundle", "msi,nsis"]).unwrap() else { panic!() };
        assert_eq!(options.bundles, vec![Bundle::Msi, Bundle::Nsis]);
        assert!(matches!(p(&["build", "--bundle", "dpkg,rpm"]).unwrap(), Command::Build(Platform::Linux, _)));
        assert!(matches!(p(&["build", "--bundle", "msi,rpm"]).unwrap(), Command::Build(Platform::Windows, _)));
        assert!(p(&["android", "build", "--bundle", "rpm"]).is_err());
        assert!(p(&["dev", "--bundle", "rpm"]).is_err());
    }
    #[test] fn monitor_defaults() {
        let Command::Monitor(_, _) = p(&["monitor"]).unwrap() else { panic!("expected host monitor"); };
    }
    #[test] fn monitor_options() {
        let Command::Monitor(_, opts) = p(&["monitor", "--poll-ms", "250", "--fps-file", "fps.txt"]).unwrap() else { panic!() };
        assert!(opts.release); assert_eq!(opts.poll_ms,250); assert_eq!(opts.fps_file.unwrap(),PathBuf::from("fps.txt"));
        let Command::Monitor(_, opts) = p(&["monitor", "--debug"]).unwrap() else { panic!() };
        assert!(!opts.release);
        assert!(p(&["android","monitor"]).is_err());
        assert!(p(&["dev","--fps-file","x"]).is_err());
    }
    #[test] fn command_profile_defaults_and_overrides() {
        for args in [&["build"][..], &["windows", "build"][..], &["android", "build"][..]] {
            let Command::Build(_, opts) = p(args).unwrap() else { panic!("expected build") };
            assert!(opts.release, "{args:?} should use release");
        }
        for args in [&["dev"][..], &["windows", "dev"][..], &["android", "dev"][..]] {
            let Command::Dev(_, opts) = p(args).unwrap() else { panic!("expected dev") };
            assert!(!opts.release, "{args:?} should use debug");
        }
        let Command::Build(_, opts) = p(&["build", "--debug"]).unwrap() else { panic!() };
        assert!(!opts.release);
        let Command::Dev(_, opts) = p(&["dev", "--release"]).unwrap() else { panic!() };
        assert!(opts.release);
        let Command::Build(_, opts) = p(&["build", "--release", "--debug"]).unwrap() else { panic!() };
        assert!(!opts.release);
        let Command::Build(_, opts) = p(&["build", "--debug", "--release"]).unwrap() else { panic!() };
        assert!(opts.release);
        assert!(p(&["android", "dev", "--release"]).is_err());
        let Command::Build(_, opts) = p(&["android", "build", "--unsigned"]).unwrap() else { panic!() };
        assert!(opts.unsigned && opts.release);
    }
    #[test] fn invalid_options() {
        assert!(p(&["dev", "--device", "foo"]).is_err());
        assert!(p(&["dev", "--poll-ms", "0"]).is_err());
    }
    #[test] fn icon_command() {
        let Command::Icon(path, _) = p(&["icon", "icon.png"]).unwrap() else { panic!() };
        assert_eq!(path, PathBuf::from("icon.png"));
    }
}
