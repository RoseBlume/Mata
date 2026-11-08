use std::path::{Path, PathBuf};

use crate::toml_simple::{self, Val};

use crate::{bail, Result};

#[derive(Debug, Default, Clone)]
pub struct Config {
    pub app: App,
    pub android: Android,
    pub windows: Desktop,
    pub linux: Desktop,
}

#[derive(Debug, Default, Clone)]
pub struct App {
    pub name: Option<String>,
    pub package: Option<String>,
    pub bin: Option<String>,
    pub version: Option<String>,
    pub maintainer: Option<String>,
    pub license: Option<String>,
    pub assets: Option<PathBuf>,
    pub files: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct Android {
    pub application_id: Option<String>,
    pub version_code: u32,
    pub min_sdk: u32,
    pub target_sdk: u32,
    pub compile_sdk: u32,
    pub abis: Vec<String>,
    pub activity: String,
    pub lib_name: Option<String>,
    pub manifest: Option<PathBuf>,
    pub java_sources: Vec<PathBuf>,
    pub resources: Option<PathBuf>,
    pub assets: Option<PathBuf>,
    pub native_libs: Option<PathBuf>,
    pub permissions: Vec<String>,
}

impl Default for Android {
    fn default() -> Self {
        Self {
            application_id: None,
            version_code: 1,
            min_sdk: 26,
            target_sdk: 35,
            compile_sdk: 35,
            abis: vec!["arm64-v8a".to_owned()],
            activity: "android.app.NativeActivity".to_owned(),
            lib_name: None,
            manifest: None,
            java_sources: Vec::new(),
            resources: Some("android/res".into()),
            assets: Some("android/assets".into()),
            native_libs: Some("android/jniLibs".into()),
            permissions: Vec::new(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct Desktop {
    pub target: Option<String>,
    pub files: Vec<PathBuf>,
    /// Windows only: ICO path, relative to the Mata project root.
    pub icon: Option<PathBuf>,
}

pub fn load(root: &Path) -> Result<Config> {
    let path = root.join("Mata.toml");
    if !path.is_file() { return Ok(Config::default()); }
    let values = toml_simple::values(&std::fs::read_to_string(&path)?)?;
    let mut cfg = Config::default();
    fn str_field(map: &std::collections::BTreeMap<String, Val>, key: &str) -> Result<Option<String>> {
        match map.get(key) { None => Ok(None), Some(Val::String(s)) => Ok(Some(s.clone())), _ => bail(format!("{key} must be a TOML string")) }
    }
    fn array_field(map: &std::collections::BTreeMap<String, Val>, key: &str) -> Result<Vec<String>> {
        match map.get(key) { None => Ok(vec![]), Some(Val::Strings(a)) => Ok(a.clone()), _ => bail(format!("{key} must be a string array")) }
    }
    fn number_field(map: &std::collections::BTreeMap<String, Val>, key: &str, default: u32) -> Result<u32> {
        match map.get(key) { None => Ok(default), Some(Val::Integer(n)) => Ok(u32::try_from(*n)?), _ => bail(format!("{key} must be an integer")) }
    }
    fn path_field(map: &std::collections::BTreeMap<String, Val>, key: &str) -> Result<Option<PathBuf>> { Ok(str_field(map,key)?.map(PathBuf::from)) }
    fn paths_field(map: &std::collections::BTreeMap<String, Val>, key: &str) -> Result<Vec<PathBuf>> { Ok(array_field(map,key)?.into_iter().map(PathBuf::from).collect()) }
    cfg.app.name = str_field(&values,"app.name")?;
    cfg.app.package = str_field(&values,"app.package")?;
    cfg.app.bin = str_field(&values,"app.bin")?;
    cfg.app.version = str_field(&values,"app.version")?;
    cfg.app.maintainer = str_field(&values,"app.maintainer")?;
    cfg.app.license = str_field(&values,"app.license")?;
    cfg.app.assets = path_field(&values,"app.assets")?;
    cfg.app.files = paths_field(&values,"app.files")?;
    cfg.android.application_id = str_field(&values,"android.application_id")?;
    cfg.android.version_code = number_field(&values,"android.version_code",cfg.android.version_code)?;
    cfg.android.min_sdk = number_field(&values,"android.min_sdk",cfg.android.min_sdk)?;
    cfg.android.target_sdk = number_field(&values,"android.target_sdk",cfg.android.target_sdk)?;
    cfg.android.compile_sdk = number_field(&values,"android.compile_sdk",cfg.android.compile_sdk)?;
    if values.contains_key("android.abis") {cfg.android.abis = array_field(&values,"android.abis")?;}
    if let Some(v) = str_field(&values,"android.activity")? {cfg.android.activity = v;}
    cfg.android.lib_name = str_field(&values,"android.lib_name")?;
    cfg.android.manifest = path_field(&values,"android.manifest")?;
    cfg.android.java_sources = paths_field(&values,"android.java_sources")?;
    if values.contains_key("android.resources") {cfg.android.resources = path_field(&values,"android.resources")?;}
    if values.contains_key("android.assets") {cfg.android.assets = path_field(&values,"android.assets")?;}
    if values.contains_key("android.native_libs") {cfg.android.native_libs = path_field(&values,"android.native_libs")?;}
    cfg.android.permissions = array_field(&values,"android.permissions")?;
    cfg.windows.target = str_field(&values,"windows.target")?;
    cfg.windows.files = paths_field(&values,"windows.files")?;
    cfg.windows.icon = path_field(&values,"windows.icon")?;
    if values.contains_key("windows.desktop_shortcut") {
        return bail("windows.desktop_shortcut was removed: installers now ask whether to create a desktop shortcut during installation");
    }
    cfg.linux.target = str_field(&values,"linux.target")?;
    cfg.linux.files = paths_field(&values,"linux.files")?;
    Ok(cfg)
}

pub fn resolve(root: &Path, value: &Path) -> PathBuf {
    if value.is_absolute() { value.to_owned() } else { root.join(value) }
}

pub fn required_id(config: &Config) -> Result<&str> {
    let value = config.android.application_id.as_deref().ok_or_else(||
        "Android builds require [android] application_id = \"com.example.app\" in Mata.toml"
    )?;
    if value.split('.').count() < 2 || !value.split('.').all(valid_segment) {
        return bail("android.application_id must contain dot-separated Java identifier components");
    }
    Ok(value)
}

fn valid_segment(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
        .replace('"', "&quot;").replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn package_name_validation() {
        let mut cfg = Config::default();
        cfg.android.application_id = Some("com.mata.player".into());
        assert!(required_id(&cfg).is_ok());
        cfg.android.application_id = Some("com.bad-name".into());
        assert!(required_id(&cfg).is_err());
    }
    #[test]
    fn xml_metacharacters() {
        assert_eq!(xml_escape("A&B <\"C\">"), "A&amp;B &lt;&quot;C&quot;&gt;");
    }
}
