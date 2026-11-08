use std::{fs, path::{Path, PathBuf}, process::Command};
use crate::json::{self, Value};
use crate::{bail, cli::Options, config::Config, tools, Result};

#[derive(Debug)]
struct Metadata { target_directory: PathBuf, packages: Vec<Package>, workspace_members: Vec<String> }
#[derive(Debug)]
struct Package { id: String, name: String, version: String, manifest_path: PathBuf, targets: Vec<Target>, matawin_dependency: Option<String> }
#[derive(Debug)]
struct Target { name: String, kind: Vec<String>, crate_types: Vec<String> }
fn metadata_from_json(raw: &[u8]) -> Result<Metadata> {
    let doc = json::parse(raw)?;
    fn strs(obj: &Value, key: &str) -> Result<Vec<String>> {
        obj.field_array(key)?.iter().map(|v| Ok(v.string().ok_or("expected JSON string array")?.to_string())).collect()
    }
    let packages = doc.field_array("packages")?.iter().map(|p| -> Result<Package> {
        let targets = p.field_array("targets")?.iter().map(|t| -> Result<Target> {
            Ok(Target { name: t.field_str("name")?.into(), kind: strs(t,"kind")?, crate_types: strs(t,"crate_types")? })
        }).collect::<Result<Vec<_>>>()?;
        // Discover the direct dependency alias, if any. Cargo's feature
        // forwarding uses the *dependency key*, not necessarily its package name.
        let matawin_dependency = p.field_array("dependencies")?.iter().find_map(|dep| {
            if dep.get("name")?.string()? != "mata-win" { return None; }
            Some(dep.get("rename").and_then(Value::string).unwrap_or("mata-win").to_owned())
        });
        Ok(Package { id:p.field_str("id")?.into(), name:p.field_str("name")?.into(),
            version:p.field_str("version")?.into(), manifest_path:PathBuf::from(p.field_str("manifest_path")?),
            targets, matawin_dependency })
    }).collect::<Result<Vec<_>>>()?;
    Ok(Metadata { target_directory: PathBuf::from(doc.field_str("target_directory")?), packages,
        workspace_members: strs(&doc,"workspace_members")? })
}

#[derive(Debug)]
pub struct Project {
    pub root: PathBuf,
    pub package_root: PathBuf,
    pub target_dir: PathBuf,
    pub package: String,
    pub version: String,
    pub bin: Option<String>,
    pub lib: Option<String>,
    pub name: String,
    /// Direct Cargo dependency key for `mata_win`, including any renamed alias.
    pub matawin_dependency: Option<String>,
}

impl Project {
    pub fn discover(root: &Path, config: &Config, opts: &Options) -> Result<Self> {
        let mut cmd = Command::new(tools::require_program("cargo")?);
        cmd.args(["metadata", "--format-version", "1", "--no-deps", "--manifest-path"])
            .arg(root.join("Cargo.toml"));
        if opts.offline { cmd.arg("--offline"); }
        if opts.verbose { eprintln!("+ {cmd:?}"); }
        let result = cmd.output()?;
        if !result.status.success() {
            return bail(format!("cargo metadata failed:\n{}", String::from_utf8_lossy(&result.stderr)));
        }
        let metadata = metadata_from_json(&result.stdout)?;
        let members: Vec<&Package> = metadata.packages.iter()
            .filter(|p| metadata.workspace_members.contains(&p.id)).collect();
        let requested = opts.package.as_deref().or(config.app.package.as_deref());
        let package = match requested {
            Some(name) => members.iter().copied().find(|p| p.name == name)
                .ok_or_else(|| format!("package '{name}' is not a workspace member"))?,
            None if members.len() == 1 => members[0],
            None => return bail("workspace has multiple packages; set app.package in Mata.toml or use -p NAME"),
        };
        let binary_names: Vec<&Target> = package.targets.iter().filter(|t| t.kind.iter().any(|k| k == "bin")).collect();
        let binary = match config.app.bin.as_deref() {
            Some(name) => Some(binary_names.iter().find(|t| t.name == name)
                .ok_or_else(|| format!("bin target '{name}' not found in package {}", package.name))?.name.clone()),
            None if binary_names.len() == 1 => Some(binary_names[0].name.clone()),
            None => None,
        };
        let lib = package.targets.iter().find(|t| t.crate_types.iter().any(|x| x == "cdylib"))
            .map(|t| t.name.clone());
        let package_root = package.manifest_path.parent().ok_or("package manifest has no parent")?.to_path_buf();
        let name = config.app.name.clone().unwrap_or_else(|| package.name.clone());
        let version = config.app.version.clone().unwrap_or_else(|| package.version.clone());
        Ok(Self { root: root.into(), package_root, target_dir: metadata.target_directory, package: package.name.clone(), version, bin: binary, lib, name, matawin_dependency: package.matawin_dependency.clone() })
    }
    pub fn bundle_root(&self, opts: &Options) -> PathBuf {
        opts.output.as_ref().map(|p| tools::resolve_output(&self.root, p))
            .unwrap_or_else(|| self.target_dir.join("mata/dist"))
    }
    pub fn cargo_command(&self, opts: &Options) -> Result<Command> {
        let mut cmd = Command::new(tools::require_program("cargo")?);
        cmd.arg("build").args(["-p", &self.package]);
        if opts.release { cmd.arg("--release"); }
        if opts.offline { cmd.arg("--offline"); }
        if opts.no_default_features { cmd.arg("--no-default-features"); }
        for feature in &opts.features { cmd.args(["--features", feature]); }
        cmd.current_dir(&self.root);
        Ok(cmd)
    }
}

pub fn init(root: &Path) -> Result<()> {
    let path = root.join("Mata.toml");
    if path.exists() { return bail(format!("{} already exists; refusing to overwrite", path.display())); }
    let content = r#"# Mata application configuration. Configure [app].package if you have a Cargo workspace.
[app]
# package = "your-cargo-package"
# bin = "your-bin-name"
# name = "My Application"
# version = "0.1.0"
# maintainer = "App Developer <you@example.com>"
# license = "MIT"
# assets = "assets"

[android]
application_id = "com.example.mataapp"
# NativeActivity needs a cdylib with ANativeActivity_onCreate exported.
# For android-activity with GameActivity, set activity = "com.google.androidgamesdk.GameActivity"
activity = "android.app.NativeActivity"
min_sdk = 26
target_sdk = 35
compile_sdk = 35
abis = ["arm64-v8a"]
# lib_name = "your_rust_library"
# java_sources = ["android/java"] # e.g. MataMediaSessionBridge.java
# manifest = "android/AndroidManifest.xml"
# permissions = ["android.permission.INTERNET"]

[linux]
# files = ["desktop/myapp.desktop"]

[windows]
# files = []
# icon = "icons/icon.ico"
"#;
    fs::write(&path, content)?;
    println!("Created {}", path.display());
    Ok(())
}

/// Configure an existing Cargo project without replacing application source files.
/// Existing Android source is preserved; only the requested activity and Cargo
/// dependency feature are changed. Newly created lib.rs is a minimal event loop.
pub fn android_init(root: &Path, mode: crate::cli::AndroidInit) -> Result<()> {
    use crate::toml_simple;
    let manifest_path = root.join("Cargo.toml");
    let mut cargo = fs::read_to_string(&manifest_path)?;
    if !toml_simple::table_exists(&cargo,"package") {
        return bail("android init requires a Cargo package manifest, not a virtual workspace root");
    }
    let mata_path = root.join("Mata.toml");
    if !mata_path.is_file() { init(root)?; }
    let activity = match mode {
        crate::cli::AndroidInit::Native => "android.app.NativeActivity",
        crate::cli::AndroidInit::Game => "com.google.androidgamesdk.GameActivity",
    };
    let mut mata = fs::read_to_string(&mata_path)?;
    mata = toml_simple::set(&mata,"android","activity",&format!("\"{activity}\""));
    // Preserve all unrelated Cargo sections/comments. Update the two required
    // values without adding a dependency to mata-bundler itself.
    // In existing crates with [lib] crate-type arrays, add cdylib if missing.
    let existing = section_value(&cargo,"lib","crate-type");
    let types = match existing {
        Some(v) => {
            if !v.trim_start().starts_with('[') || !v.trim_end().ends_with(']') {
                return bail("[lib] crate-type must be an array");
            }
            if v.contains("cdylib") { v } else {
                let inner=v.trim(); format!("[{}, \"cdylib\"]",inner[1..inner.len()-1].trim().trim_end_matches(','))
            }
        },
        None => "[\"cdylib\"]".into(),
    };
    cargo = toml_simple::set(&cargo,"lib","crate-type",&types);
    let feature = match mode { crate::cli::AndroidInit::Native => "native-activity", crate::cli::AndroidInit::Game => "game-activity" };
    let table = "target.'cfg(target_os = \"android\")'.dependencies";
    let dep=section_value(&cargo,table,"android-activity");
    let new_dep= match dep {
        None => format!("{{ version = \"0.6.1\", default-features = false, features = [\"{feature}\"] }}"),
        Some(mut original) => {
            if !original.trim().starts_with('{') { return bail("android-activity must use an inline table (manual update required)"); }
            // Rewrite only the android-activity inline table and preserve unrelated keys.
            // Feature list retains any feature other than mutually exclusive activity backends.
            let mut retained:Vec<String>=Vec::new();
            if let Some(begin)=original.find("features") {
                if let Some(open)=original[begin..].find('[') {
                    let open=begin+open;
                    if let Some(close)=original[open..].find(']') {
                        let close=close+open;
                        for part in original[open+1..close].split(',') {
                            let p=part.trim().trim_matches('"');
                            if !p.is_empty() && p!="native-activity" && p!="game-activity" {retained.push(p.to_owned());}
                        }
                    }
                }
            }
            retained.push(feature.to_owned());
            let list = retained.iter().map(|f| format!("\"{f}\"")).collect::<Vec<_>>().join(", ");
            // Parse comma-delimited inline keys only outside arrays and strings.
            let body=original.trim().trim_start_matches('{').trim_end_matches('}').trim();
            let mut parts:Vec<String>=Vec::new();let mut depth=0i32;let mut quoted=false;let mut start=0;
            for (i,ch) in body.char_indices() {
                if ch=='"' {quoted=!quoted;}
                if !quoted {if ch=='[' || ch=='{' {depth+=1;} if ch==']' || ch=='}' {depth-=1;}}
                if ch==',' && depth==0 && !quoted {parts.push(body[start..i].trim().to_owned());start=i+1;}
            }
            parts.push(body[start..].trim().to_owned());
            parts.retain(|p| !p.starts_with("features") && !p.starts_with("default-features") && !p.is_empty());
            parts.push("default-features = false".into());parts.push(format!("features = [{list}]"));
            original=format!("{{ {} }}",parts.join(", "));
            original
        }
    };
    cargo = toml_simple::set(&cargo,table,"android-activity",&new_dep);
    let lib_path = section_value(&cargo,"lib","path")
        .map(|p| root.join(p.trim().trim_matches('"').trim_matches('\'')))
        .unwrap_or_else(||root.join("src/lib.rs"));
    if !lib_path.exists() {
        if let Some(parent)=lib_path.parent(){fs::create_dir_all(parent)?;}
        fs::write(&lib_path,r#"// Android application entry point. Replace with your Mata startup code.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub fn android_main(app: android_activity::AndroidApp) {
    use android_activity::{MainEvent, PollEvent};
    loop {
        let mut destroyed = false;
        app.poll_events(None, |event| {
            if let PollEvent::Main(MainEvent::Destroy) = event { destroyed = true; }
        });
        if destroyed { break; }
    }
}
"#)?;
        println!("Created {}",lib_path.display());
    } else { println!("Preserved {}",lib_path.display()); }
    fs::write(&manifest_path,cargo)?; fs::write(&mata_path,mata)?;
    println!("Configured Android activity: {activity}");
    Ok(())
}
fn section_value(src:&str,section:&str,key:&str)->Option<String> {
    let mut inside=false;
    for line in src.lines(){
        let l=line.trim();
        if l.starts_with('[') && l.ends_with(']') {inside=&l[1..l.len()-1]==section;continue;}
        if inside {if let Some((k,v))=l.split_once('=') {if k.trim()==key {return Some(v.trim().split('#').next()?.trim().to_owned());}}}
    }
    None
}

#[cfg(test)]
mod init_tests {
    use super::*;
    use crate::cli::AndroidInit;
    #[test]
    fn initialize_and_switch_activity_without_overwriting_source() {
        let id = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("mata-bundler-init-{}-{id}", std::process::id()));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("Cargo.toml"), "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n").unwrap();
        android_init(&root, AndroidInit::Native).unwrap();
        let native_manifest = fs::read_to_string(root.join("Cargo.toml")).unwrap();
        assert!(native_manifest.contains("native-activity"));
        let lib = root.join("src/lib.rs");
        fs::write(&lib, "// keep me\n").unwrap();
        android_init(&root, AndroidInit::Game).unwrap();
        assert_eq!(fs::read_to_string(&lib).unwrap(), "// keep me\n");
        let game_manifest = fs::read_to_string(root.join("Cargo.toml")).unwrap();
        assert!(game_manifest.contains("game-activity"));
        assert!(!game_manifest.contains("native-activity"));
        let config = crate::config::load(&root).unwrap();
        assert_eq!(config.android.activity, "com.google.androidgamesdk.GameActivity");
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod telemetry_dependency_tests {
    use super::*;

    #[test]
    fn discovers_matawin_dependency_alias() {
        let raw = br#"{
          "target_directory":"C:/build",
          "workspace_members":["a"],
          "packages":[{
            "id":"a", "name":"app", "version":"1.0.0",
            "manifest_path":"C:/project/Cargo.toml",
            "targets":[{"name":"app","kind":["bin"],"crate_types":["bin"]}],
            "dependencies":[{"name":"mata-win","rename":"window_api"}]
          }]
        }"#;
        let md = metadata_from_json(raw).unwrap();
        assert_eq!(md.packages[0].matawin_dependency.as_deref(), Some("window_api"));
    }

    #[test]
    fn does_not_assume_matawin_when_missing() {
        let raw = br#"{
          "target_directory":"C:/build",
          "workspace_members":["a"],
          "packages":[{
            "id":"a", "name":"app", "version":"1.0.0",
            "manifest_path":"C:/project/Cargo.toml",
            "targets":[{"name":"app","kind":["bin"],"crate_types":["bin"]}],
            "dependencies":[{"name":"mata-ui","rename":null}]
          }]
        }"#;
        let md = metadata_from_json(raw).unwrap();
        assert_eq!(md.packages[0].matawin_dependency, None);
    }
}
