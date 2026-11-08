# Mata Lockdown (Windows prototype)

A Windows-first, opt-in exam browser proof-of-concept using **only Rust std**, your
`mata-win` and `mata-ui` crates, and the official Microsoft `windows` crate.
The native browser is `mata-ui`'s WebView2 integration. No `wry`, `tao`,
`webview2-com`, `serde`, `tokio`, or other third-party Rust dependencies.

## Current functionality

- MataWin window, Mata UI controls and a WebView2 exam view.
- `exam.conf` parser with fail-closed validation of the initial HTTPS URL.
- Configurable hostname patterns and std-only unit tests for hostile URLs.
- A **companion source patch** to your Mata UI WebView2 backend that cancels
  disallowed top-level and iframe navigation, including redirects.
- Enumerates incompatible running apps with the Windows Tool Help API, both
  before and during sessions; displays warnings without terminating them.
- Displays a voluntary close/recheck screen; does **not** forcibly terminate apps.
- Explicit "Exit / End exam" control and normal close behavior.
- **Optional** Windows Filtering Platform dynamic-session rules that block **all**
  outgoing connections (IPv4 and IPv6) for exact, configured executable paths.
  On application exit or initialization failure the session closes and its rules vanish.
- No camera, microphone, recording, telemetry, admin persistence, hidden background service,
  unrequested registry changes, touchpad modifications or accessibility disabling.

## Limitations — do not treat this as a tamper-proof exam environment

The supplied Mata UI WebView2 backend did not expose navigation interception.
This package includes a patch adding top-level and iframe `NavigationStarting`
interceptors. **Apply the patch before compiling.** Subresource requests, downloads,
new-window requests, JavaScript network calls, popups, and browser permissions
are not yet covered by the allowlist, so the app must **not** be used as a
high-stakes exam security boundary. Build `NewWindowRequested` and
`WebResourceRequested` filtering and test it thoroughly before deployment.

WFP's app-path rules are **not hostname rules**, do not impact unlisted programs,
and may not revoke already-established connections. Running-app checks are advisory, even when
rechecked during the exam; they do not enforce process blocking. This prototype does not inhibit
Task View, Alt+Tab, virtual desktop gestures, or other OS features. Realistic
kiosk-style deployment needs an institution-managed Windows environment.

## Directory placement

Place this directory next to your existing `mata-win` and `mata-ui` crates:

```text
Mata/
├── mata-win/
│   └── Cargo.toml
├── mata-ui/
│   └── Cargo.toml
├── mata-fonts/         # your existing optional Mata dependency
├── mata-image/         # your existing optional Mata dependency
├── mata-audio/        # your existing optional Mata dependency
├── mata-video/        # your existing optional Mata dependency
└── mata-lockdown/
    ├── Cargo.toml
    ├── exam.conf
    └── src/
```

`mata-ui`'s manifest references other optional Mata crates by path; Cargo may
require their manifests to exist even when they are not enabled.

## Run

1. Apply the patch from the Mata workspace root (directory containing both
   `mata-ui` and `mata-lockdown`):

```powershell
git apply --check mata-lockdown/patches/mata-ui-navigation-guard.patch
git apply mata-lockdown/patches/mata-ui-navigation-guard.patch
```

2. Edit `exam.conf`. Replace `example.org` with the real institution domain,
   add other approved hosts and test login/CDN redirects.
3. Install the Microsoft Edge WebView2 Runtime (if missing). The archived
   Mata UI backend also expects Microsoft's **WebView2Loader.dll** next to the executable.
4. On Windows, run from the `mata-lockdown` folder:

```powershell
cargo test
cargo run --release
```

Or provide a different config file path as the first argument:

```powershell
cargo run --release -- C:\path\to\exam.conf
```

**The application is not tested/compiled here:** this environment does not
have a Rust toolchain or Windows system APIs. The crate should be built and
corrected against your exact local Mata crate versions before any deployment.

## Optional: app-specific WFP rules

In `exam.conf`:

```ini
require_network_filter=true
blocked_app_path=C:\Program Files\ExampleBrowser\browser.exe
```

Supply **existing full file paths** to exact executables. Launch with
appropriate WFP administration rights. If the rules cannot be applied, the
exam refuses to start. Rules are automatically cleaned up by the WFP dynamic
session upon exit. Test on a non-critical device first. Never target system,
accessibility, security, or management software.

Use this only to block configured apps' **entire outbound network access**;
there is no supported per-website rule in this prototype.

## Next security/quality milestones

1. Extend the included top-level/iframe navigation hook to capture all resource
   requests and new windows, and test real exam login flows and CDNs.
2. Handle file downloads, popup requests, permission prompts and protocol schemes.
3. Add an application-specific preflight/exam state machine that watches new
   incompatible processes and informs the examinee without killing them.
4. Add WebView2 runtime checks, better UI states, digital signatures and a
   test matrix for current Windows versions.
5. Use institution-managed Assigned Access / kiosk policies when a higher
   assurance environment is actually required.

The app is an opt-in prototype, not an enforcement or surveillance product.
