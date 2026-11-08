#![cfg_attr(not(windows), allow(dead_code))]

mod config;
#[cfg(windows)] mod process;
#[cfg(windows)] mod network;

#[cfg(not(windows))]
fn main() {
    eprintln!("Mata Lockdown currently requires Windows. Run `cargo test` for std-only policy tests.");
}

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{env, thread, time::{Duration, Instant}};
    use mata_ui::widgets::{Anchors, Button, Rect, Text, WebView, WidgetEvent, WidgetWindow};
    use mata_win::Window;
    use config::Config;

    let config_path = env::args_os().nth(1).map(std::path::PathBuf::from)
        .unwrap_or_else(|| "exam.conf".into());
    let cfg = Config::load(&config_path)?;
    let host = WidgetWindow::from_window(Window::new("Mata Lockdown | Exam Preflight", (1100, 760)))?;
    host.set_background(0xff101820);
    let banner = Text::new("Mata Lockdown - Exam Readiness");
    banner.set_font_size(25.0);
    host.add_content(Rect::new(24, 20, 1000, 44), banner.clone());
    let status = Text::new("Checking running applications...");
    status.set_font_size(17.0);
    host.add_content(Rect::new(24, 69, 1000, 90), status.clone());
    let start = Button::new(&host, Rect::new(24, 164, 175, 40), "Start exam")?;
    let recheck = Button::new(&host, Rect::new(214, 164, 160, 40), "Recheck")?;
    let leave = Button::new(&host, Rect::new(390, 164, 190, 40), "Exit / End exam")?;
    start.set_anchors(Anchors::TOP_LEFT);
    recheck.set_anchors(Anchors::TOP_LEFT);
    leave.set_anchors(Anchors::TOP_LEFT);
    let mut view: Option<WebView> = None;
    let mut network_guard: Option<network::AppNetworkGuard> = None;
    let mut active = false;
    let mut events = Vec::with_capacity(32);
    let mut last_scan = Instant::now() - Duration::from_secs(10);
    let mut safe_to_start = false;
    loop {
        host.poll_events_into(&mut events);
        for evt in &events {
            match evt {
                WidgetEvent::Close => return Ok(()),
                WidgetEvent::ButtonClicked(id) if *id == leave.id() => return Ok(()),
                WidgetEvent::ButtonClicked(id) if *id == recheck.id() && !active => {
                    last_scan = Instant::now() - Duration::from_secs(10);
                }
                WidgetEvent::ButtonClicked(id) if *id == start.id() && !active && safe_to_start => {
                    // Do not start if the network requirement fails, and do not kill applications.
                    if cfg.require_network_filter {
                        match network::AppNetworkGuard::new(&cfg.blocked_app_paths) {
                            Ok(guard) => network_guard = Some(guard),
                            Err(error) => {
                                status.set_text(&format!("Cannot apply network rules: {error}. Exam not started."));
                                continue;
                            }
                        }
                    }
                    match WebView::new(&host, Rect::new(0, 220, 1100, 540)) {
                        Ok(webview) => {
                            webview.set_anchors(Anchors::ALL);
                            // Requires the companion Mata UI navigation-guard patch.
                            // Covers top-level and iframe redirects, but not all subresources.
                            let navigation_config = cfg.clone();
                            webview.set_navigation_guard(move |url| navigation_config.allows_url(url));
                            if let Err(e) = webview.navigate(&cfg.start_url) {
                                status.set_text(&format!("Browser navigation failed: {e}"));
                                drop(network_guard.take());
                                continue;
                            }
                            view = Some(webview);
                            active = true;
                            start.set_enabled(false);
                            recheck.set_enabled(false);
                            banner.set_text("Mata Lockdown - Exam Session");
                            status.set_text("Exam browser active. Exit / End exam restores all temporary WFP rules.");
                        }
                        Err(e) => {
                            drop(network_guard.take());
                            status.set_text(&format!("Unable to initialize WebView2: {e}"));
                        }
                    }
                }
                WidgetEvent::WebViewError(_, message) => {
                    status.set_text(&format!("WebView2 error: {message}"));
                }
                _ => {}
            }
        }
        if last_scan.elapsed() >= Duration::from_secs(2) {
            match process::incompatible_processes(&cfg.blocked_processes) {
                Ok(found) => {
                    if !active {
                        safe_to_start = found.is_empty();
                        start.set_enabled(safe_to_start);
                        if found.is_empty() {
                            status.set_text(&format!("Ready. Approved starting site: {}. Navigation checks active with Mata UI patch.", cfg.start_url));
                        } else {
                            status.set_text(&format!("Close these applications voluntarily, then recheck: {}", found.join(", ")));
                        }
                    } else if !found.is_empty() {
                        status.set_text(&format!("Incompatible app detected: {}. Please close it before continuing.", found.join(", ")));
                    } else {
                        status.set_text("Exam browser active. Exit / End exam restores temporary WFP rules.");
                    }
                }
                Err(e) => {
                    if !active {
                        safe_to_start = false;
                        start.set_enabled(false);
                    }
                    status.set_text(&format!("Process check failed: {e}"));
                }
            }
            last_scan = Instant::now();
        }
        // Keep both guards alive until the session ends or the app exits.
        let _ = (&view, &network_guard);
        host.present();
        thread::sleep(Duration::from_millis(12));
    }
}
