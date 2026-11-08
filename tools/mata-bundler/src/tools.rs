use std::{env, ffi::OsStr, fs, path::{Path, PathBuf}, process::Command};
use crate::{bail, Result};

pub fn run(command: &mut Command, verbose: bool) -> Result<()> {
    if verbose { eprintln!("+ {command:?}"); }
    let result = command.status().map_err(|e| format!("failed to launch {command:?}: {e}"))?;
    if !result.success() { return bail(format!("command failed ({result}): {command:?}")); }
    Ok(())
}

pub fn require_program(program: &str) -> Result<PathBuf> {
    program_path(program).ok_or_else(|| format!("required program '{program}' not found in PATH").into())
}

pub fn program_path(program: &str) -> Option<PathBuf> {
    let executable = Path::new(program);
    if executable.is_absolute() && executable.is_file() { return Some(executable.to_owned()); }
    let paths = env::var_os("PATH")?;
    for dir in env::split_paths(&paths) {
        for suffix in if cfg!(windows) { vec![".exe", ".bat", ".cmd", ""] } else { vec![""] } {
            let p = dir.join(format!("{program}{suffix}"));
            if p.is_file() { return Some(p); }
        }
    }
    None
}

pub fn android_sdk() -> Result<PathBuf> {
    for var in ["ANDROID_HOME", "ANDROID_SDK_ROOT"] {
        if let Some(home) = env::var_os(var) {
            let path = PathBuf::from(home);
            if path.is_dir() { return Ok(path); }
        }
    }
    bail("set ANDROID_HOME (or ANDROID_SDK_ROOT) to the installed Android SDK directory")
}

pub fn newest_child(dir: &Path, prefix: &str) -> Result<PathBuf> {
    let mut children = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let p = entry?.path();
        if p.is_dir() && p.file_name().and_then(OsStr::to_str).unwrap_or("").starts_with(prefix) {
            children.push(p);
        }
    }
    children.sort_by(|a, b| version_key(b).cmp(&version_key(a)));
    children.into_iter().next().ok_or_else(|| format!("no usable installation in {}", dir.display()).into())
}

fn version_key(p: &Path) -> Vec<u32> {
    p.file_name().and_then(OsStr::to_str).unwrap_or("")
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty()).map(|s| s.parse().unwrap_or(0)).collect()
}

pub fn collect_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if !root.exists() { return Ok(files); }
    if fs::symlink_metadata(root)?.file_type().is_symlink() {
        return bail(format!("symlinks are not supported in packaged assets: {}", root.display()));
    }
    if root.is_file() { return Ok(vec![root.to_owned()]); }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let kind = entry.file_type()?;
        if kind.is_symlink() { return bail(format!("symlinks are not supported in packaged assets: {}", path.display())); }
        if kind.is_dir() { files.extend(collect_files(&path)?); }
        else if kind.is_file() { files.push(path); }
    }
    files.sort();
    Ok(files)
}

pub fn safe_archive_name(path: &Path) -> Result<String> {
    use std::path::Component;
    let mut segments = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(s) => segments.push(s.to_string_lossy().into_owned()),
            _ => return bail(format!("unsafe archive entry: {}", path.display())),
        }
    }
    if segments.is_empty() { return bail("archive entry cannot be empty"); }
    Ok(segments.join("/"))
}

pub fn ensure_dir(path: &Path) -> Result<()> { fs::create_dir_all(path)?; Ok(()) }

pub fn project_root(override_dir: Option<&Path>) -> Result<PathBuf> {
    let from = match override_dir { Some(p) => p.to_path_buf(), None => env::current_dir()? };
    let absolute = from.canonicalize()?;
    if override_dir.is_some() {
        if absolute.join("Cargo.toml").is_file() { return Ok(absolute); }
        return bail(format!("no Cargo.toml in {}", absolute.display()));
    }
    for p in absolute.ancestors() {
        if p.join("Mata.toml").exists() && p.join("Cargo.toml").is_file() { return Ok(p.into()); }
    }
    for p in absolute.ancestors() {
        if p.join("Cargo.toml").is_file() { return Ok(p.into()); }
    }
    bail("run mata from a Cargo project or specify --project-dir")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn archive_path_rejects_traversal() {
        assert!(safe_archive_name(Path::new("../secrets")).is_err());
        assert_eq!(safe_archive_name(Path::new("libs/arm64-v8a/app.so")).unwrap(), "libs/arm64-v8a/app.so");
    }
}

pub fn resolve_output(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() { path.to_owned() } else { root.join(path) }
}

pub fn copy_tree(source: &Path, dest: &Path) -> Result<()> {
    if !source.exists() { return Ok(()); }
    if source.is_file() {
        if let Some(parent) = dest.parent() { fs::create_dir_all(parent)?; }
        fs::copy(source, dest)?;
        return Ok(());
    }
    // Never follow symlinks into directories while bundling user assets.
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let dest_path = dest.join(entry.file_name());
        if file_type.is_symlink() { return bail(format!("asset symlinks are not supported: {}", entry.path().display())); }
        if file_type.is_dir() { copy_tree(&entry.path(), &dest_path)?; }
        else if file_type.is_file() { copy_tree(&entry.path(), &dest_path)?; }
    }
    Ok(())
}

pub fn doctor(opts: &crate::cli::Options) -> Result<()> {
    for program in ["cargo", "rustc", "rustup", "java", "gradle", "adb", "wix", "wixl", "makensis"] {
        match program_path(program) {
            Some(p) => println!("{:12} OK  {}", program, p.display()),
            None => println!("{:12} --  not found", program),
        }
    }
    match android_sdk() {
        Ok(p) => {
            println!("Android SDK  {}", p.display());
            if let Ok(ndk) = newest_child(&p.join("ndk"), "") { println!("Android NDK  {}", ndk.display()); }
        }
        Err(e) => println!("Android SDK  unavailable: {e}"),
    }
    crate::installer_tools::doctor();
    if opts.verbose { println!("Environment PATH: {:?}", env::var_os("PATH")); }
    Ok(())
}
