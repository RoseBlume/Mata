//! Discover a conventional `icons/` tree without Cargo dependencies.
use std::{fs, path::{Path, PathBuf}};
use crate::{tools, Result};

pub const PNG_SIZES: &[u32] = &[16, 24, 32, 48, 64, 128, 256, 512];
pub const ANDROID_DENSITIES: &[&str] = &["mipmap-mdpi", "mipmap-hdpi", "mipmap-xhdpi", "mipmap-xxhdpi", "mipmap-xxxhdpi"];

pub fn directory(project_root: &Path) -> PathBuf {
    // The `mata icon` command outputs lowercase `icons`. Also accept an
    // existing uppercase `ICONS` tree on case-sensitive builders.
    let lowercase = project_root.join("icons");
    if lowercase.is_dir() { return lowercase; }
    let uppercase = project_root.join("ICONS");
    if uppercase.is_dir() { return uppercase; }
    lowercase
}

pub fn windows_icon(project_root: &Path) -> Option<PathBuf> {
    let icon = directory(project_root).join("icon.ico");
    icon.is_file().then_some(icon)
}

pub fn install_linux(project_root: &Path, stage: &Path, package: &str, app_name: &str) -> Result<()> {
    let dir=directory(project_root);
    for &size in PNG_SIZES {
        let source=dir.join(format!("icon-{size}x{size}.png"));
        if source.is_file() {
            let dest=stage.join(format!("usr/share/icons/hicolor/{size}x{size}/apps/{package}.png"));
            tools::ensure_dir(dest.parent().ok_or("invalid Linux icon destination")?)?;
            fs::copy(source, dest)?;
        }
    }
    // Name cannot inject additional desktop-entry keys/lines.
    let clean_name=app_name.replace(['\r','\n']," ").replace('\\', "\\\\");
    let desktop=format!("[Desktop Entry]\nType=Application\nName={clean_name}\nExec={package}\nIcon={package}\nTerminal=false\nCategories=Utility;\n");
    let path=stage.join("usr/share/applications").join(format!("{package}.desktop"));
    tools::ensure_dir(path.parent().ok_or("invalid desktop-entry destination")?)?;
    fs::write(path,desktop)?;
    Ok(())
}

/// Install only the pre-generated icons. Never resize or regenerate them during
/// packaging: the creator's original, per-density artwork is authoritative.
pub fn install_android(project_root: &Path, main: &Path) -> Result<(bool,bool)> {
    let root=directory(project_root).join("android");
    let mut normal=false;
    let mut round=false;
    for &density in ANDROID_DENSITIES {
        for (filename,seen) in [("ic_launcher.png", &mut normal), ("ic_launcher_round.png", &mut round)] {
            let source=root.join(density).join(filename);
            if !source.is_file() { continue; }
            let dest=main.join("res").join(density).join(filename);
            tools::ensure_dir(dest.parent().ok_or("invalid Android icon destination")?)?;
            fs::copy(source,dest)?;
            *seen=true;
        }
    }
    Ok((normal,round))
}

pub fn has_android(project_root: &Path) -> (bool,bool) {
    let root=directory(project_root).join("android");
    (
        ANDROID_DENSITIES.iter().any(|d|root.join(d).join("ic_launcher.png").is_file()),
        ANDROID_DENSITIES.iter().any(|d|root.join(d).join("ic_launcher_round.png").is_file()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detects_project_icon_layout() {
        let dir=std::env::temp_dir().join(format!("mata-icon-layout-{}",std::process::id()));
        if dir.exists(){fs::remove_dir_all(&dir).unwrap();}
        fs::create_dir_all(dir.join("ICONS/android/mipmap-hdpi")).unwrap();
        fs::write(dir.join("ICONS/icon.ico"),b"icon").unwrap();
        fs::write(dir.join("ICONS/icon-48x48.png"),b"png").unwrap();
        fs::write(dir.join("ICONS/android/mipmap-hdpi/ic_launcher.png"),b"png").unwrap();
        assert!(windows_icon(&dir).is_some());
        let linux=dir.join("linux");install_linux(&dir,&linux,"sample-app","Sample App").unwrap();
        assert!(linux.join("usr/share/icons/hicolor/48x48/apps/sample-app.png").is_file());
        let desktop=fs::read_to_string(linux.join("usr/share/applications/sample-app.desktop")).unwrap();
        assert!(desktop.contains("Exec=sample-app"));
        let android=dir.join("android");
        assert_eq!(install_android(&dir,&android).unwrap(),(true,false));
        assert!(android.join("res/mipmap-hdpi/ic_launcher.png").is_file());
        fs::remove_dir_all(dir).unwrap();
    }
}
