# mata-bundler

A Cargo-based build, packaging, and development CLI for Mata applications. The crate installs the executable **`mata`**.

## Quick start

The `mata-bundler` crate has **zero third-party Cargo dependencies** and does not need `mata-image` or another sibling crate. It uses Rust 1.88+ and the standard library only. No crates are downloaded when compiling the bundler itself. The application being built can still have its own dependencies.

**Platform tools are separate from Rust crate dependencies.** Creating MSI/NSIS installers still invokes WiX/NSIS; Windows uses its built-in PowerShell to download, verify and unpack missing installer toolchains. RPM output now requires a host `rpmbuild`. Android builds need the usual Android SDK/NDK, Gradle and Java tools. All of these are runtime/build tools, not `mata-bundler` Cargo dependencies.

```sh
cargo install --path .
cd /path/to/your/mata-app
mata init
mata dev                     # debug compile, run, watch source files
mata monitor                 # release compile, profile, and generate graphs
mata build                   # release compile and package for the builder's OS
```

### Command reference

| Command | Behavior |
|---|---|
| `mata build` | **Release** build for the builder OS: Linux `.deb` and `.rpm`; Windows `.msi` and NSIS `.exe` |
| `mata build --bundle dpkg` | Build a Debian package (file extension `.deb`) |
| `mata build --bundle rpm` | Build an RPM package |
| `mata build --bundle msi,nsis` | Build Windows MSI and NSIS installers |
| `mata build --bundle dpkg,rpm` | Build both Linux formats |
| `mata build --bundle msi,nsis,dpkg,rpm` | Compile as needed for **both** Windows and Linux; build all four formats |
| `mata build --bundle zip` | Build a portable Windows ZIP |
| `mata dev` | **Debug** compile and launch the host application; rebuild and restart when files change |
| `mata monitor` | **Release** build; run one monitoring session; generate change-only CPU/GPU/FPS/memory/network graphs with in-app FPS telemetry (no elevation) |
| `mata android init --native` | Set up **NativeActivity** and add the matching Rust Android feature |
| `mata android init --game` | Set up **GameActivity** and add the matching Rust Android feature |
| `mata android build` | Build Android APK for configured ABIs |
| `mata android dev` | Build, install and launch on a connected phone/emulator; watch and redeploy |
| `mata doctor` | Show installed and cached host tools (including WiX/NSIS) without downloading |
| `mata icon icon.png` | Generate `icons/icon.ico`, desktop PNG icon sizes, and Android `mipmap-*` launcher icons |

The `--bundle` flag is **build-only** and accepts comma-separated formats (or may be repeated). Each format implies its operating system. When formats span Windows and Linux, `mata build` runs separate Cargo builds for those target OSes and then creates the selected packages. Explicit `mata linux build` and `mata windows build` still work, and `--target TRIPLE` can override the target triple.

### Automatic platform detection and build profiles

```sh
mata build             # cargo build --release (default)
mata build --debug     # cargo build, using debug output and profile
mata dev               # cargo build, debug profile (default)
mata dev --release     # cargo build --release, then run and watch
mata monitor           # cargo build --release, then monitor and report
mata monitor --debug   # cargo build, then monitor the debug binary
```

`mata windows build`, `mata linux build`, and `mata android build` also default to release. `--debug` and `--release` explicitly select the profile; when both are provided, the last flag wins. Android release builds require release signing environment variables or `--unsigned`, while Android `dev` always uses debug signing.

- **Linux host:** produces `.deb` directly using the standard library; `.rpm` invokes a host-installed `rpmbuild` (the previous Rust `rpm` crate is removed).
- **Windows host:** produces `.msi` (WiX) and `.exe` (NSIS) installers. Missing WiX/NSIS toolchains are retrieved from pinned verified releases and cached per-user (similar to Tauri). You can request `--bundle zip` to avoid both.
- **Android:** use `mata android build` to produce an APK. Plain `mata build` does not infer Android just because an Android phone is attached.

Note that packaging and cross-*compiling* are distinct steps. For example, `mata build --bundle msi,nsis` on Linux selects `x86_64-pc-windows-gnu` when building from an x86-64 Linux host. You **must** have the Rust Windows target's standard library and a working MinGW cross-linker installed. Cross-compiling Linux programs from Windows similarly requires a Linux linker/sysroot. A `--bundle` option cannot make a missing cross toolchain work automatically. Any per-target linker configuration in `.cargo/config.toml` is honored by Cargo.

Supported automatic cross triple inference is limited to x86-64, i686, and AArch64 GNU targets. Supply `--target` for non-default architectures. ARM64 Windows builds in particular may require a different toolchain than the inferred GNU triple; use an explicitly supported target. On Windows, GNU and MSVC target binaries can both be built if the corresponding linkers exist, though the installer toolchain must also support the selected architecture.



### Process monitoring

`mata monitor` runs until the application exits, Ctrl+C is pressed, or `exit` is entered. By default it samples once per second and stores separate SVG graphs and change-only CSVs in `target/mata/monitor/<package>-<timestamp>/`. `--poll-ms`, `--debug`, `--out`, `--fps-file`, workspace `-p`, and Cargo feature flags are supported. CPU and memory are per-process; GPU is per-process if available. FPS on Windows is captured automatically for apps using the updated `mata-win`: the monitor enables the dependency's `telemetry` feature for its release (or `--debug`) build, and MataWin counts successful framebuffer presentations from `Window::new` onward. For other renderers or Linux, use `integrations/mata_fps.rs` manually. No Windows ETW tracing, PresentMon, or administrator rights are needed. Uninstrumented apps correctly show FPS N/A. Network is system-wide unless the app writes network counters. No flamegraph is generated. No new Rust crates are required. See **[MONITOR.md](MONITOR.md)** for details, integration snippets, and capture limitations.

### Windows installer install scope, shortcuts, and icons

Both Windows installer formats now generate two installation variants when selected:

| Output | Default install path | Administrator permissions |
| --- | --- | --- |
| `<package>-<version>-windows.msi` / `-setup.exe` | `%LOCALAPPDATA%\Programs\<Application Name>` | Not required (per-user) |
| `<package>-<version>-windows-all-users.msi` / `-all-users-setup.exe` | `%ProgramFiles%\<Application Name>` (or the x86 Program Files directory for an x86 target) | Required (per-machine) |

Both variants install a **Start-menu entry**, register Add/Remove Programs metadata, and remove their own shortcuts when uninstalled. The regular per-user installer does not require administrator rights; the all-users installer requests elevation.

**Desktop shortcut selection is made by the person installing the app**, not by the developer in `Mata.toml`:

- **NSIS**: optional `Desktop shortcut` checkbox on the installer component-selection page, unchecked initially, for both per-user and all-users setups.
- **MSI (WiX 3 / WiX 4)**: an optional `Desktop shortcut` feature selectable on the installer feature-selection screen (`WixUI_FeatureTree`), unchecked initially, for both per-user and all-users MSIs. The shortcut can also be enabled for an unattended installation with `ADDLOCAL=Complete,DesktopShortcutFeature`.

Interactive MSI feature selection uses the **WiX UI extension**. WiX 3 requires `WixUIExtension` when linking (bundler passes it automatically). For WiX 4, first install the extension with `wix extension add WixToolset.UI.wixext`; the bundler passes `-ext WixToolset.UI.wixext` when building. The `wixl` cross-platform MSI compiler does not support this dialog set; for MSIs requiring interactive shortcut selection, use WiX 3 or WiX 4. NSIS does not need an additional extension.

Do not add `windows.desktop_shortcut` to `Mata.toml`; this obsolete build-time option now causes a descriptive error instead of silently overriding the installer's choices.

The following project layout is detected automatically:

```text
icons/
  icon.ico
  icon-16x16.png
  icon-24x24.png
  icon-32x32.png
  icon-48x48.png
  icon-64x64.png
  icon-128x128.png
  icon-256x256.png
  icon-512x512.png
  android/
    mipmap-mdpi/ic_launcher.png
    mipmap-mdpi/ic_launcher_round.png
    mipmap-hdpi/...
    mipmap-xhdpi/...
    mipmap-xxhdpi/...
    mipmap-xxxhdpi/...
```

An uppercase `ICONS` directory also works, including when cross-building on case-sensitive filesystems. All paths are relative to the project root, not the bundler crate root. Icons are picked by target OS:

| Target | Automatic treatment |
| --- | --- |
| Windows | `icons/icon.ico` is bundled, used for NSIS setup/uninstall icons, Start menu/Desktop shortcut icons, and Add/Remove Programs icon metadata (MSI/NSIS). The icon of the `.msi` *file* in Explorer is controlled by the system association. |
| Linux | Each available `icons/icon-{SIZE}x{SIZE}.png` is copied to `/usr/share/icons/hicolor/{SIZE}x{SIZE}/apps/<package>.png` in DEB/RPM packages, and an application entry is written to `/usr/share/applications/<package>.desktop`. |
| Android | Copies existing PNGs from `icons/android/mipmap-*/` into generated `app/src/main/res/mipmap-*/`; adds `android:icon` and `android:roundIcon` to the manifest automatically unless your custom manifest already specifies them. |

No icon generation is performed during packaging; run `mata icon` beforehand if you want to generate the standard assets. Missing icon sizes are simply skipped. An optional `[windows] icon = "path/to/custom.ico"` still overrides automatic ICO discovery for Windows only.

The WiX generator marks files under Local AppData as 32-bit MSI *components* with HKCU registry key paths, even for x64 executables, avoiding ICE80/ICE38. System-wide x64 MSI components are installed under ProgramFiles64Folder and marked 64-bit. Both scopes get a per-scope Start menu folder and use the correct HKCU/HKLM registry keypaths. WiX v4 uses `Bitness`, WiX v3 uses `Win64`.

**Testing limitation:** Windows-specific WiX/NSIS installers need final compilation and install/uninstall testing on Windows. Rust crate dependency count remains zero; WiX, NSIS, and Android toolchains are external build tools rather than added Rust crates.

## Icon generation

Generate all common icon assets from a single PNG source:

```sh
mata icon icon.png
```

This command decodes 8-bit, non-interlaced PNG images with a built-in, dependency-free decoder (gray/RGB/indexed/gray-alpha/RGBA), then writes:

- `icons/icon.ico` for Windows
- `icons/icon-16x16.png`, `icon-24x24.png`, `icon-32x32.png`, `icon-48x48.png`, `icon-64x64.png`, `icon-128x128.png`, `icon-256x256.png`, and `icon-512x512.png` for desktop/Linux usage
- `icons/android/mipmap-mdpi/ic_launcher.png`
- `icons/android/mipmap-hdpi/ic_launcher.png`
- `icons/android/mipmap-xhdpi/ic_launcher.png`
- `icons/android/mipmap-xxhdpi/ic_launcher.png`
- `icons/android/mipmap-xxxhdpi/ic_launcher.png`

It also writes matching `ic_launcher_round.png` files in each Android mipmap directory.

The source icon should ideally be a square 8-bit PNG with transparency and at least 512×512 pixels. JPEG, 16-bit PNG and interlaced PNG inputs are not supported by the built-in decoder. Generated PNG and ZIP files use uncompressed DEFLATE/stored entries, which are standards-compliant but larger than compression-oriented formats.

## Android initialization

From a Rust app's Cargo project root:

```sh
mata android init --native
# OR
mata android init --game
```

The command creates `Mata.toml` if absent, sets the Android activity to `android.app.NativeActivity` or `com.google.androidgamesdk.GameActivity`, ensures `[lib] crate-type` contains `cdylib`, and adds or adjusts `android-activity` (with `native-activity` **or** `game-activity`) under `target.'cfg(target_os = "android")'.dependencies` in your Cargo.toml. It preserves unrelated Cargo manifest fields and existing `src/lib.rs`. It creates a minimal `src/lib.rs` with `android_main` **only when missing**. An existing Android entry point must remain compatible with the selected mode; the bundler does not rewrite existing GUI/application Rust code.

The initial application ID is the placeholder `com.example.mataapp` in `Mata.toml`; change it to your unique package name before publishing. Likewise, the generated Android Rust event loop is a scaffold and **does not render an interface**; replace it with your Mata window startup code. A minimal example is in `examples/hello-mata/`.

Build or deploy:

```sh
mata android build
mata android dev
mata android dev --device emulator-5554
```

Android requires the SDK, NDK, JDK 17+, Gradle 8.11.1+ (or a project Gradle wrapper), Rust Android targets and ADB for dev. Gradle's generated application project is stored beneath `target/mata/android`. The output APK is under `target/mata/dist`. `mata android dev` detects one connected, authorized device, selects its ABI, builds the native cdylib, installs the APK, and launches it again after changes. Ctrl+C stops watching. Debug builds use debug signing; release uses the environment variables listed below or `--unsigned`.

```sh
export MATA_ANDROID_KEYSTORE=/path/to/signing.jks
export MATA_ANDROID_KEY_ALIAS=your-key-alias
export MATA_ANDROID_KEYSTORE_PASSWORD=your-store-password
export MATA_ANDROID_KEY_PASSWORD=your-key-password
mata android build
```

`mata android build --unsigned` creates a release APK that still needs signing prior to installation. Use `mata android build --debug` for a debug APK.

Mata's Android `MediaSession` Java bridge may be supplied via `[android] java_sources = ["android/java"]`. Set `[android] activity` to your custom Java class if using one. For a native shared library that should be loaded automatically, `android.app.lib_name` is generated from the Cargo library or `[android] lib_name`.

## Desktop packaging details

### Linux `.deb` and `.rpm` packaging

Debian packages are written directly as an `ar` archive containing `debian-binary`, `control.tar.gz`, and `data.tar.gz`. Files are added with correct Linux executable permissions, including when building on Windows. The RPM bundle is generated using a temporary spec file passed to **`rpmbuild`**; install your distribution's `rpm-build` package if selecting `--bundle rpm`. Both formats install the executable to `/usr/bin/<name>`, with optional app data under `/usr/share/<name>/`.

**Limitations:** No automatic system-library dependency inference, package signing, or distribution repository publishing is performed. Prebuilt dynamically linked binaries can depend on system libraries not contained in the package. Make sure runtime dependencies are installed on your target distribution. Bundle file names include package version and architecture.

### Windows installers

* **MSI:** generate WiX XML and use WiX 4+ `wix build`, WiX 3 `candle`/`light`, or Unix `wixl`. On Windows, Tauri's pinned WiX 3.14 toolchain is downloaded and verified automatically if nothing is installed/cached. WiX inputs, outputs, object files and `<File Source>` paths are converted from Windows extended-length `\\?\` paths into ordinary Win32 paths to avoid `CNDL0284`.
* **NSIS:** generate an `.nsi` script, run `makensis`, and package the executable and assets with an uninstaller. On Windows, a missing NSIS toolchain is downloaded and cached from a pinned verified NSIS 3.13 release. On Linux/macOS, `makensis` must be installed.
* **ZIP:** Rust-only portable archive, with no NSIS/WiX requirement.

#### Tool selection and cache

Explicitly configured tools take priority, followed by the user tool cache, then installed tools. Windows will download a missing compiler into `%LOCALAPPDATA%\mata-bundler`; on Linux this defaults to `~/.cache/mata-bundler` and on macOS to `~/Library/Caches/mata-bundler`.

| Environment variable | Behavior |
|---|---|
| `MATA_BUNDLER_TOOLS_DIR` | Override the directory containing downloaded installer toolchains |
| `MATA_BUNDLER_WIX_PATH` | Full path or directory for WiX 4+ `wix` |
| `MATA_BUNDLER_WIX3_PATH` | Full path or directory for WiX 3 `candle`; `light` must be alongside it |
| `MATA_BUNDLER_WIXL_PATH` | Full path or directory for `wixl` |
| `MATA_BUNDLER_NSIS_PATH` | Full path or directory for `makensis` |
| `NSIS_PATH` | Alternative Windows NSIS directory/executable |

`mata doctor` reports available tools without downloading. `mata build --offline` uses only tools that are already installed or cached. Toolchain ZIPs are hash-verified (SHA-1 for the pinned NSIS release, SHA-256 for WiX) and unpacked using Windows PowerShell/.NET, without HTTP, hash or archive Rust crates. The tool archive is validated before moving it into the cache.

The MSI and NSIS files are not automatically code-signed. Windows may show publisher warnings for unsigned installers. For reliable distribution, sign them in your release process. Installer validation on real Windows machines is recommended.

## Configuration (`Mata.toml`)

The documented Mata.toml keys are supported with ordinary TOML scalar strings, numbers, and string arrays. This dependency-free parser does not implement every TOML 1.0 extension (for example, multiline quoted strings or dotted shorthand for Mata fields):

```toml
[app]
package = "mata-player"
name = "Mata Player"
maintainer = "Publisher Name"
license = "MIT"
assets = "assets"

[android]
application_id = "com.example.mataplayer"
activity = "android.app.NativeActivity"
min_sdk = 26
target_sdk = 35
compile_sdk = 35
abis = ["arm64-v8a", "x86_64"]
# java_sources = ["android/java"]
# manifest = "android/AndroidManifest.xml"

[linux]
# target = "aarch64-unknown-linux-gnu"
# files = ["desktop/mata-player.desktop"]

[windows]
# target = "x86_64-pc-windows-gnu"
# files = []
```

When building a Cargo workspace with multiple packages, set `[app] package` or use `-p NAME`. For a Cargo package containing multiple desktop binaries, set `[app] bin`. `mata build` defaults to Cargo's release profile, and `mata dev` defaults to debug; `--debug` and `--release` override those defaults. Outputs default to `target/mata/dist/`, or set `--out PATH`. Repeated `--features`, `--no-default-features`, `--offline`, and `--verbose` are supported.

## Development mode

`mata dev` runs a polling watcher (default 650 ms) for Rust and UI sources, config, manifest, and common asset formats. Edits trigger a rebuild; successful builds restart the desktop process. `mata android dev` rebuilds and redeploys APKs. These are **compile/restart workflows**, not in-process hot-reload. The bundler does not supply the Mata GUI itself; your application must initialize the window.

## Verification

```sh
cargo check --all-targets
cargo test --all-targets
cargo fmt --check
```

The test suite includes CLI parsing for single- and mixed-OS `--bundle` selections, Android init flags, architecture inference, and MSI version handling. Integration tests for installer output still require the relevant target compilers and/or real devices.
