//! Responsive video gallery using MataWin and the optional mata-video backend.
//!
//! By default, searches `workspace_manifest_dir/assets_ignored/videos`, then
//! `workspace_manifest_dir/videos`. Supply a directory as the first argument
//! to override either location. Files are found recursively and sorted.
//!
//! ```text
//! cargo run -p mata-ui --no-default-features --features x11,fonts,video \
//!     --example video_gallery -- /path/to/videos
//! ```
//!
//! On Windows omit `x11`; Linux video decoding requires GStreamer and the
//! appropriate codec plugins. The four visible thumbnails display paused first
//! frames, not a precomputed thumbnail database. Only five decoders (one main,
//! four previews) are kept open at any time. Embedded audio is not played.

mod support;

use std::{
    error::Error,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use mata_ui::{
    widgets::{Button, Rect, Video, WidgetEvent, WidgetExt, WidgetWindow},
    Window,
};
use support::{collect_files, workspace_manifest_dir};

const PAGE_SIZE: usize = 4;
const WINDOW_SIZE: (u32, u32) = (1180, 780);
const MARGIN: u32 = 16;
const GAP: u32 = 10;
const SEEK_SECONDS: f64 = 5.0;
const CONTROLS: [&str; 7] = ["Previous", "Pause", "Stop", "-5s", "+5s", "Next", "Loop: Off"];

fn is_video_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.to_ascii_lowercase())
            .as_deref(),
        Some("mp4" | "m4v" | "mov" | "mkv" | "webm" | "avi" | "wmv" | "mpg" | "mpeg" | "ogv" | "ts")
    )
}

fn gallery_directory() -> PathBuf {
    if let Some(argument) = std::env::args_os().nth(1) {
        return PathBuf::from(argument);
    }
    let workspace = workspace_manifest_dir();
    let ignored = workspace.join("assets_ignored").join("videos");
    if ignored.is_dir() {
        ignored
    } else {
        workspace.join("videos")
    }
}

fn filename(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn shorten(name: &str, max_chars: usize) -> String {
    let mut chars = name.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

fn format_time(seconds: f64) -> String {
    let total = if seconds.is_finite() && seconds >= 0.0 {
        seconds.floor().min(u64::MAX as f64) as u64
    } else {
        0
    };
    if total >= 3600 {
        format!("{}:{:02}:{:02}", total / 3600, (total / 60) % 60, total % 60)
    } else {
        format!("{}:{:02}", total / 60, total % 60)
    }
}

fn update_label(button: &Button, desired: &str) {
    if button.label() != desired {
        button.set_label(desired);
    }
}

fn rect(x: u32, y: u32, width: u32, height: u32) -> Rect {
    Rect::new(x as i32, y as i32, width.max(1), height.max(1))
}

#[derive(Clone, Copy)]
struct Layout {
    main: Rect,
    status: Rect,
    controls: [Rect; 7],
    preview: [Rect; PAGE_SIZE],
    selection: [Rect; PAGE_SIZE],
    page_nav: [Rect; 2],
}

impl Layout {
    fn new(width: u32, height: u32) -> Self {
        // Use saturating arithmetic so minimized/tiny client sizes cannot
        // underflow. Normal gallery layouts are designed for >= 600 x 480.
        let width = width.max(600);
        let height = height.max(480);
        let mut controls = [rect(0, 0, 1, 1); 7];
        let mut preview = [rect(0, 0, 1, 1); PAGE_SIZE];
        let mut selection = [rect(0, 0, 1, 1); PAGE_SIZE];
        let page_nav;
        let main;
        let status;
        let transport_width;
        let transport_y;
        let transport_x;

        if width >= 950 {
            // A wide preview rail on the right; transport stays under player.
            let sidebar_width = (width / 4).clamp(245, 320);
            let sidebar_x = width - MARGIN - sidebar_width;
            let main_width = sidebar_x - MARGIN * 2;
            main = rect(MARGIN, MARGIN, main_width, height.saturating_sub(166));
            status = rect(MARGIN, height - 139, main_width, 38);
            transport_x = MARGIN;
            transport_y = height - 81;
            transport_width = main_width;

            let available = height.saturating_sub(100);
            let tile_height = available.saturating_sub(GAP * 3) / PAGE_SIZE as u32;
            for slot in 0..PAGE_SIZE {
                let top = MARGIN + slot as u32 * (tile_height + GAP);
                preview[slot] = rect(sidebar_x, top, sidebar_width, tile_height.saturating_sub(34));
                selection[slot] = rect(sidebar_x, top + tile_height.saturating_sub(32), sidebar_width, 32);
            }
            let half = (sidebar_width - GAP) / 2;
            page_nav = [
                rect(sidebar_x, height - 58, half, 40),
                rect(sidebar_x + half + GAP, height - 58, sidebar_width - half - GAP, 40),
            ];
        } else {
            // Compact width: filmstrip under the player, transport at bottom.
            let main_height = (height * 52 / 100).min(height.saturating_sub(260).max(150));
            main = rect(MARGIN, MARGIN, width - 2 * MARGIN, main_height);
            let status_y = MARGIN + main.height + GAP;
            status = rect(MARGIN, status_y, width - 2 * MARGIN, 35);
            let strip_top = status_y + status.height + GAP;
            let strip_width = (width - 2 * MARGIN - 3 * GAP) / PAGE_SIZE as u32;
            let nav_y = height - 94;
            let strip_height = nav_y.saturating_sub(strip_top + 42).max(36);
            for slot in 0..PAGE_SIZE {
                let x = MARGIN + slot as u32 * (strip_width + GAP);
                preview[slot] = rect(x, strip_top, strip_width, strip_height);
                selection[slot] = rect(x, strip_top + strip_height + 4, strip_width, 32);
            }
            let half = (width - 2 * MARGIN - GAP) / 2;
            page_nav = [
                rect(MARGIN, nav_y, half, 30),
                rect(MARGIN + half + GAP, nav_y, half, 30),
            ];
            transport_x = MARGIN;
            transport_y = height - 52;
            transport_width = width - 2 * MARGIN;
        }

        let button_gap = 6;
        let cell = transport_width.saturating_sub(button_gap * 6) / 7;
        for (index, item) in controls.iter_mut().enumerate() {
            let x = transport_x + index as u32 * (cell + button_gap);
            let remaining = transport_x + transport_width - x;
            let w = if index == 6 { remaining } else { cell };
            *item = rect(x, transport_y, w, 42);
        }

        Self { main, status, controls, preview, selection, page_nav }
    }
}

struct Gallery {
    files: Vec<PathBuf>,
    current: usize,
    page: usize,
    player: Video,
    previews: [Option<Video>; PAGE_SIZE],
    looping: bool,
    error_message: Option<String>,
}

impl Gallery {
    fn new(files: Vec<PathBuf>) -> Result<Self, Box<dyn Error>> {
        // Some containers can be recognized by extension but lack an installed
        // native decoder. Pick the first stream that can actually be opened.
        let mut opened = None;
        for (index, path) in files.iter().enumerate() {
            match Video::from_file(path, true) {
                Ok(video) => { opened = Some((index, video)); break; }
                Err(error) => eprintln!("Skipping {}: {error}", path.display()),
            }
        }
        let (current, player) = opened.ok_or("none of the video files could be decoded")?;
        player.set_keep_aspect_ratio(true);
        player.set_looping(false);
        Ok(Self {
            files, current, page: current / PAGE_SIZE, player,
            previews: std::array::from_fn(|_| None),
            looping: false, error_message: None,
        })
    }

    fn page_count(&self) -> usize {
        self.files.len().div_ceil(PAGE_SIZE)
    }

    fn select(&mut self, index: usize) -> Result<bool, Box<dyn Error>> {
        if index >= self.files.len() { return Ok(false); }
        // `set_file` validates/decodes a frame before replacing decoder state.
        // If it fails, the previously selected video remains available.
        self.player.set_autoplay(true);
        self.player.set_file(&self.files[index])?;
        self.player.set_looping(self.looping);
        self.current = index;
        let moved_page = self.page != index / PAGE_SIZE;
        self.page = index / PAGE_SIZE;
        self.error_message = None;
        Ok(moved_page)
    }

    fn step(&mut self, delta: isize) -> Result<bool, Box<dyn Error>> {
        let len = self.files.len() as isize;
        let target = (self.current as isize + delta).rem_euclid(len) as usize;
        self.select(target)
    }

    fn change_page(&mut self, delta: isize) {
        self.page = (self.page as isize + delta).rem_euclid(self.page_count() as isize) as usize;
    }

    fn seek_relative(&self, delta: f64) -> Result<(), Box<dyn Error>> {
        let requested = (self.player.position() + delta).max(0.0);
        let target = self.player.duration().map_or(requested, |d| requested.min(d));
        self.player.seek(target)?;
        Ok(())
    }

    fn refresh_previews(&mut self, window: &WidgetWindow, layout: &Layout, tiles: &[Button]) {
        for (slot, tile) in tiles.iter().enumerate() {
            if let Some(old) = self.previews[slot].take() {
                window.remove_video(old.id());
            }
            let index = self.page * PAGE_SIZE + slot;
            if let Some(path) = self.files.get(index) {
                match Video::from_file(path, false) {
                    Ok(preview) => {
                        preview.set_bounds(layout.preview[slot]);
                        preview.set_keep_aspect_ratio(true);
                        window.add_video(&preview);
                        self.previews[slot] = Some(preview);
                    }
                    Err(error) => {
                        eprintln!("No preview for {}: {error}", path.display());
                    }
                }
            }
        }
        self.update_tiles(tiles);
        window.request_redraw();
    }

    fn update_tiles(&self, tiles: &[Button]) {
        for (slot, tile) in tiles.iter().enumerate() {
            let index = self.page * PAGE_SIZE + slot;
            if let Some(path) = self.files.get(index) {
                let prefix = if self.current == index { "▶ " } else { "" };
                let label = format!("{}{}. {}", prefix, index + 1, shorten(&filename(path), 21));
                update_label(tile, &label);
                tile.set_enabled(true);
            } else {
                update_label(tile, "—");
                tile.set_enabled(false);
            }
        }
    }

    fn refresh_status(&self, status: &Button, controls: &[Button]) {
        let playing = self.player.is_playing();
        update_label(&controls[1], if playing { "Pause" } else { "Play" });
        update_label(&controls[6], if self.looping { "Loop: On" } else { "Loop: Off" });

        let current_name = filename(&self.files[self.current]);
        let duration = self.player.duration().map(format_time).unwrap_or_else(|| "?:??".into());
        let elapsed = format_time(self.player.position());
        let state = if playing { "Playing" } else { "Paused" };
        let summary = match &self.error_message {
            Some(error) => shorten(error, 130),
            None => format!("{state}  •  {elapsed} / {duration}  •  {current_name}"),
        };
        update_label(status, &summary);
    }

    fn update_title(&self, window: &WidgetWindow) {
        let current_name = filename(&self.files[self.current]);
        window.set_title(&format!(
            "MataWin Video Gallery — {} / {} — {} (page {}/{})",
            self.current + 1, self.files.len(),
            shorten(&current_name, 55), self.page + 1, self.page_count()
        ));
    }
}

fn apply_layout(
    window: &WidgetWindow,
    gallery: &Gallery,
    controls: &[Button],
    status: &Button,
    tiles: &[Button],
    nav: &[Button; 2],
    layout: Layout,
) {
    gallery.player.set_bounds(layout.main);
    status.set_bounds(layout.status);
    for (button, bounds) in controls.iter().zip(layout.controls) {
        button.set_bounds(bounds);
    }
    for (index, tile) in tiles.iter().enumerate() {
        tile.set_bounds(layout.selection[index]);
        if let Some(preview) = &gallery.previews[index] {
            preview.set_bounds(layout.preview[index]);
        }
    }
    for (button, bounds) in nav.iter().zip(layout.page_nav) {
        button.set_bounds(bounds);
    }
    window.request_redraw();
}

fn main() -> Result<(), Box<dyn Error>> {
    let dir = gallery_directory();
    if !dir.is_dir() {
        return Err(format!("video directory not found: {}", dir.display()).into());
    }
    let files = collect_files(&dir, is_video_file)?;
    if files.is_empty() {
        return Err(format!("no video files found in {}", dir.display()).into());
    }

    let window = WidgetWindow::from_window(Window::new("MataWin Video Gallery", WINDOW_SIZE))?;
    window.set_background(0xff171b24);
    let mut gallery = Gallery::new(files)?;
    let mut layout = Layout::new(window.size().0, window.size().1);
    gallery.player.set_bounds(layout.main);
    window.add_video(&gallery.player);

    let controls: Vec<Button> = (0..CONTROLS.len())
        .map(|index| Button::new(&window, layout.controls[index], CONTROLS[index]))
        .collect::<Result<_, _>>()?;
    let tiles: Vec<Button> = (0..PAGE_SIZE)
        .map(|index| Button::new(&window, layout.selection[index], ""))
        .collect::<Result<_, _>>()?;
    let nav: [Button; 2] = [
        Button::new(&window, layout.page_nav[0], "◀ Previous page")?,
        Button::new(&window, layout.page_nav[1], "Next page ▶")?,
    ];
    let status = Button::new(&window, layout.status, "")?;
    status.set_enabled(false);

    gallery.refresh_previews(&window, &layout, &tiles);
    let many_pages = gallery.page_count() > 1;
    nav[0].set_enabled(many_pages);
    nav[1].set_enabled(many_pages);
    gallery.refresh_status(&status, &controls);
    gallery.update_title(&window);

    let mut events = Vec::with_capacity(24);
    let mut last_status = Instant::now();
    loop {
        window.poll_events_into(&mut events);
        let size = window.size();
        let new_layout = Layout::new(size.0, size.1);
        if size != (0, 0) && (layout.main != new_layout.main || layout.preview != new_layout.preview) {
            layout = new_layout;
            apply_layout(&window, &gallery, &controls, &status, &tiles, &nav, layout);
        }

        let mut page_changed = false;
        let mut selection_changed = false;
        let mut refresh = false;
        for event in &events {
            match event {
                WidgetEvent::Close => return Ok(()),
                WidgetEvent::ButtonClicked(id) => {
                    let action: Result<(), Box<dyn Error>> = if *id == controls[0].id() {
                        gallery.step(-1).map(|page| { page_changed |= page; selection_changed = true; })
                    } else if *id == controls[1].id() {
                        if gallery.player.is_playing() { gallery.player.pause(); }
                        else { gallery.player.play(); }
                        Ok(())
                    } else if *id == controls[2].id() {
                        gallery.player.stop().map_err(Into::into)
                    } else if *id == controls[3].id() {
                        gallery.seek_relative(-SEEK_SECONDS)
                    } else if *id == controls[4].id() {
                        gallery.seek_relative(SEEK_SECONDS)
                    } else if *id == controls[5].id() {
                        gallery.step(1).map(|page| { page_changed |= page; selection_changed = true; })
                    } else if *id == controls[6].id() {
                        gallery.looping = !gallery.looping;
                        gallery.player.set_looping(gallery.looping);
                        Ok(())
                    } else if *id == nav[0].id() {
                        gallery.change_page(-1);
                        page_changed = true;
                        Ok(())
                    } else if *id == nav[1].id() {
                        gallery.change_page(1);
                        page_changed = true;
                        Ok(())
                    } else if let Some(slot) = tiles.iter().position(|tile| tile.id() == *id) {
                        let index = gallery.page * PAGE_SIZE + slot;
                        gallery.select(index).map(|page| { page_changed |= page; selection_changed = true; })
                    } else {
                        Ok(())
                    };
                    if let Err(error) = action {
                        let message = error.to_string();
                        eprintln!("Video gallery: {message}");
                        gallery.error_message = Some(message);
                    }
                    refresh = true;
                }
                WidgetEvent::VideoError(id, error) if *id == gallery.player.id() => {
                    gallery.player.pause();
                    gallery.error_message = Some(error.clone());
                    eprintln!("Video playback error: {error}");
                    refresh = true;
                }
                WidgetEvent::Resized(_, _) => refresh = true,
                _ => {}
            }
        }

        if page_changed {
            gallery.refresh_previews(&window, &layout, &tiles);
        } else if selection_changed {
            gallery.update_tiles(&tiles);
        }
        if page_changed || selection_changed {
            gallery.update_title(&window);
        }
        if refresh || last_status.elapsed() >= Duration::from_millis(250) {
            gallery.refresh_status(&status, &controls);
            last_status = Instant::now();
        }
        window.present();
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_is_case_insensitive() {
        assert!(is_video_file(Path::new("video.MKV")));
        assert!(is_video_file(Path::new("movie.mp4")));
        assert!(!is_video_file(Path::new("cover.png")));
    }

    #[test]
    fn trim_and_time_are_stable() {
        assert_eq!(shorten("abc", 4), "abc");
        assert_eq!(shorten("abcdef", 4), "abcd…");
        assert_eq!(format_time(61.9), "1:01");
        assert_eq!(format_time(3661.0), "1:01:01");
    }

    #[test]
    fn layout_has_nonempty_surfaces() {
        for (w, h) in [(600, 480), (900, 640), (1180, 780), (1600, 900)] {
            let layout = Layout::new(w, h);
            assert!(layout.main.width > 0 && layout.main.height > 0);
            assert!(layout.preview.iter().all(|rect| rect.width > 0 && rect.height > 0));
            assert!(layout.controls.iter().all(|rect| rect.width > 0 && rect.height > 0));
        }
    }
}
