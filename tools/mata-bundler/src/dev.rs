use std::{collections::BTreeMap, fs, path::{Path, PathBuf}, process::{Child, Command}, thread, time::{Duration, SystemTime}};
use crate::{android, cli::{Options, Platform}, config, desktop, project::Project, tools, Result};

#[derive(Debug, PartialEq, Eq)]
struct FileStamp { len: u64, modified: Option<SystemTime> }
type Snapshot = BTreeMap<PathBuf, FileStamp>;

pub fn run(platform: Platform, opts: &Options) -> Result<()> {
    let root = tools::project_root(opts.project_dir.as_deref())?;
    crate::interrupt::install()?;
    let mut process: Option<Child> = None;
    let mut assets_watch: Vec<PathBuf> = Vec::new();
    let interval = Duration::from_millis(opts.poll_ms);
    let mut first = true;
    let mut old_snapshot = Snapshot::new();
    println!("Mata dev: {} (Ctrl+C to exit)", platform.name());
    while !crate::interrupt::requested() {
        let next_snapshot = snapshot(&root, &assets_watch)?;
        let changed = first || next_snapshot != old_snapshot;
        if changed {
            // A short debounce allows editors using atomic rename/write to settle.
            if !first { thread::sleep(Duration::from_millis(200)); }
            if let Some(mut child) = process.take() { let _ = child.kill(); let _ = child.wait(); }
            // Record the change before compiling, not after: edits made during a
            // long build will trigger another pass instead of being swallowed.
            old_snapshot = next_snapshot;
            let config = config::load(&root);
            let result = config.and_then(|cfg| {
                assets_watch = cfg.app.assets.iter()
                    .chain(cfg.android.assets.iter()).chain(cfg.android.resources.iter())
                    .chain(cfg.android.java_sources.iter()).chain(cfg.android.manifest.iter())
                    .map(|p| config::resolve(&root, p)).collect();
                old_snapshot = snapshot(&root, &assets_watch)?;
                let project = Project::discover(&root, &cfg, opts)?;
                match platform {
                    Platform::Android => {
                        let path = android::build(&project, &cfg, opts, true)?;
                        android::install_and_launch(&path, &cfg, opts)
                    }
                    Platform::Linux | Platform::Windows => {
                        let binary = desktop::build(&project, &cfg, platform, opts)?;
                        println!("Launching {} ...", binary.display());
                        let child = Command::new(&binary).current_dir(&project.package_root).spawn()?;
                        process = Some(child);
                        Ok(())
                    }
                }
            });
            if let Err(e) = result { eprintln!("mata dev: build/deploy failed: {e}\nFix the source and save to retry."); }
            first = false;
        }
        if let Some(child) = process.as_mut() {
            if let Some(status) = child.try_wait()? {
                println!("App exited ({status}). Watching for file changes ...");
                process = None;
            }
        }
        if crate::interrupt::requested() { break; }
        thread::sleep(interval);
    }
    if let Some(mut child) = process { let _ = child.kill(); let _ = child.wait(); }
    println!("Mata dev stopped");
    Ok(())
}

fn interesting(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if matches!(name, "Cargo.lock" | "Cargo.toml" | "Mata.toml" | "build.rs" | "gradle.properties" | "settings.gradle" | "build.gradle") { return true; }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    matches!(ext.as_str(), "rs" | "toml" | "slint" | "html" | "css" | "js" | "ts" | "json" | "java" | "kt" | "xml" | "gradle" | "properties" | "png" | "jpg" | "jpeg" | "webp" | "svg" | "ttf" | "otf" | "wav" | "mp3" | "ogg")
}
fn walk(dir: &Path, snapshot: &mut Snapshot, depth: usize) -> Result<()> {
    if depth > 24 { return Ok(()); }
    if !dir.exists() { return Ok(()); }
    if dir.is_file() {
        if interesting(dir) { let meta = fs::metadata(dir)?; snapshot.insert(dir.into(), FileStamp { len: meta.len(), modified: meta.modified().ok() }); }
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        if ft.is_symlink() { continue; }
        let path = entry.path();
        if ft.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if matches!(name.as_ref(), "target" | ".git" | ".gradle" | ".idea" | ".vscode" | "node_modules" | "build" | "dist" | "assets_ignored") { continue; }
            walk(&path, snapshot, depth + 1)?;
        } else if ft.is_file() && interesting(&path) {
            let meta = entry.metadata()?;
            snapshot.insert(path, FileStamp { len: meta.len(), modified: meta.modified().ok() });
        }
    }
    Ok(())
}
fn snapshot(root: &Path, configured_assets: &[PathBuf]) -> Result<Snapshot> {
    let mut result = Snapshot::new();
    walk(root, &mut result, 0)?;
    for path in configured_assets {
        // If an explicitly configured assets directory lives in assets_ignored,
        // monitor it even though the normal repository walk skips that directory.
        walk(path, &mut result, 0)?;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn recognizes_source_changes() {
        assert!(interesting(Path::new("app.slint")));
        assert!(interesting(Path::new("Cargo.toml")));
        assert!(!interesting(Path::new("artifact.zip")));
    }
}
