use std::{env, fs, path::{Path, PathBuf}, process::Command};
use crate::{bail, cli::Options, config::{self, Config}, project::Project, icon_layout, tools, Result};

fn abi_to_target(abi: &str) -> Result<(&'static str, &'static str)> {
    match abi {
        "arm64-v8a" => Ok(("aarch64-linux-android", "aarch64-linux-android")),
        "armeabi-v7a" => Ok(("armv7-linux-androideabi", "armv7a-linux-androideabi")),
        "x86_64" => Ok(("x86_64-linux-android", "x86_64-linux-android")),
        "x86" => Ok(("i686-linux-android", "i686-linux-android")),
        _ => bail(format!("unsupported Android ABI '{abi}' (arm64-v8a, armeabi-v7a, x86_64, x86)")),
    }
}

pub fn build(project: &Project, config: &Config, opts: &Options, dev: bool) -> Result<PathBuf> {
    let application_id = config::required_id(config)?;
    let lib = project.lib.as_deref().ok_or("Android requires a Cargo [lib] with crate-type = [\"cdylib\"] (see README.md)")?;
    if config.android.min_sdk < 21 || config.android.min_sdk > config.android.target_sdk || config.android.target_sdk > config.android.compile_sdk {
        return bail("Android SDK levels must satisfy 21 <= min_sdk <= target_sdk <= compile_sdk");
    }
    if dev && opts.unsigned { return bail("Android development requires debug APK signing"); }
    let sdk = tools::android_sdk()?;
    let ndk = tools::newest_child(&sdk.join("ndk"), "")
        .map_err(|e| format!("Android NDK missing ({e}). Install with sdkmanager --install \"ndk;27.2.12479018\""))?;
    let gradle = find_gradle(&project.root)?;
    let work = project.target_dir.join("mata/android");
    let main = work.join("app/src/main");
    let jni_libs = main.join("jniLibs");
    if jni_libs.exists() { fs::remove_dir_all(&jni_libs)?; }
    tools::ensure_dir(&jni_libs)?;

    let abis = if !opts.abis.is_empty() { opts.abis.clone() }
        else if dev { vec![device_abi(opts)?] }
        else { config.android.abis.clone() };
    if abis.is_empty() { return bail("no Android ABIs configured"); }
    for abi in &abis {
        let (target, cc_prefix) = abi_to_target(abi)?;
        // rustup target add is idempotent, only invoked if the target is missing.
        install_rust_target(target, opts)?;
        let clang_dir = ndk.join("toolchains/llvm/prebuilt");
        let host_bin = choose_host_prebuilt(&clang_dir)?.join("bin");
        let linker = host_bin.join(format!("{}{}-clang{}", cc_prefix, config.android.min_sdk,
            if cfg!(windows) { ".cmd" } else { "" }));
        if !linker.is_file() { return bail(format!("NDK compiler missing: {}", linker.display())); }
        let mut cargo = project.cargo_command(opts)?;
        cargo.args(["--lib", "--target", target]);
        let linker_var = format!("CARGO_TARGET_{}_LINKER", target.to_ascii_uppercase().replace('-', "_"));
        cargo.env(linker_var, &linker).env("ANDROID_NDK_HOME", &ndk).env("ANDROID_HOME", &sdk);
        // cc-rs and some native build scripts need an explicit compiler/archiver.
        cargo.env(format!("CC_{}", target.replace('-', "_")), &linker);
        cargo.env(format!("AR_{}", target.replace('-', "_")), host_bin.join(exe("llvm-ar")));
        tools::run(&mut cargo, opts.verbose)?;
        let output = project.target_dir.join(target).join(if opts.release { "release" } else { "debug" })
            .join(format!("lib{}.so", lib.replace('-', "_")));
        if !output.is_file() { return bail(format!("Cargo succeeded but Android shared library is missing: {}", output.display())); }
        let lib_name = config.android.lib_name.as_deref().unwrap_or(lib);
        let dest = jni_libs.join(abi).join(format!("lib{}.so", lib_name.replace('-', "_")));
        tools::ensure_dir(dest.parent().ok_or("invalid native library destination")?)?;
        fs::copy(&output, &dest)?;
        println!("Android {} -> {}", abi, dest.display());
    }
    if let Some(extra_libs) = &config.android.native_libs {
        tools::copy_tree(&config::resolve(&project.root, extra_libs), &jni_libs)?;
    }
    prepare_gradle(project, config, opts, &sdk, application_id, &work, &main)?;
    let task = if opts.release { "assembleRelease" } else { "assembleDebug" };
    let mut cmd = Command::new(gradle);
    cmd.arg("--project-dir").arg(&work).arg(task).env("ANDROID_HOME", &sdk).env("ANDROID_SDK_ROOT", &sdk);
    if opts.offline { cmd.arg("--offline"); }
    tools::run(&mut cmd, opts.verbose)?;
    let name = if opts.release {
        if opts.unsigned { "app-release-unsigned.apk" } else { "app-release.apk" }
    } else { "app-debug.apk" };
    let generated = work.join("app/build/outputs/apk").join(if opts.release { "release" } else { "debug" }).join(name);
    if !generated.is_file() { return bail(format!("Gradle build succeeded but expected APK not found: {}", generated.display())); }
    let dir = project.bundle_root(opts);
    tools::ensure_dir(&dir)?;
    let output = dir.join(format!("{}-{}-{}{}.apk", project.package, project.version, if opts.release { "release" } else { "debug" }, if opts.unsigned { "-unsigned" } else { "" }));
    fs::copy(&generated, &output)?;
    Ok(output)
}

fn choose_host_prebuilt(path: &Path) -> Result<PathBuf> {
    for candidate in if cfg!(windows) {
        &["windows-x86_64"][..]
    } else if cfg!(target_os = "macos") {
        &["darwin-x86_64", "darwin-arm64"][..]
    } else { &["linux-x86_64"][..] } {
        let p = path.join(candidate);
        if p.is_dir() { return Ok(p); }
    }
    tools::newest_child(path, "").map_err(|e| format!("no Android NDK LLVM prebuilt host toolchain: {e}").into())
}
fn exe(name: &str) -> String { format!("{name}{}", if cfg!(windows) { ".exe" } else { "" }) }

fn install_rust_target(target: &str, opts: &Options) -> Result<()> {
    let mut cmd = Command::new(tools::require_program("rustup")?);
    cmd.args(["target", "list", "--installed"]);
    let output = cmd.output()?;
    if !output.status.success() { return bail("rustup target list --installed failed"); }
    if !String::from_utf8_lossy(&output.stdout).lines().any(|s| s == target) {
        if opts.offline { return bail(format!("Rust target {target} is not installed and --offline forbids downloading it")); }
        println!("Installing Rust Android target {target} ...");
        let mut add = Command::new(tools::require_program("rustup")?);
        add.args(["target", "add", target]);
        tools::run(&mut add, opts.verbose)?;
    }
    Ok(())
}

fn find_gradle(root: &Path) -> Result<PathBuf> {
    let script = if cfg!(windows) { "gradlew.bat" } else { "gradlew" };
    for path in [root.join("android").join(script), root.join(script)] {
        if path.is_file() { return Ok(path); }
    }
    tools::require_program("gradle").map_err(|_| "Gradle not found. Install Gradle 8.11.1+ or place gradlew and its wrapper files in your project or android/ directory".into())
}

fn groovy_string(s: &str) -> String { format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'").replace(['\r','\n'], " ")) }
fn prepare_gradle(project: &Project, config: &Config, opts: &Options, sdk: &Path, app_id: &str, work: &Path, main: &Path) -> Result<()> {
    let lib = config.android.lib_name.as_deref().or(project.lib.as_deref()).ok_or("missing Android library name")?.replace('-', "_");
    let game = config.android.activity == "com.google.androidgamesdk.GameActivity";
    let custom_activity = config.android.activity != "android.app.NativeActivity";
    let java_enabled = game || custom_activity || !config.android.java_sources.is_empty();
    fs::write(work.join("settings.gradle"), "pluginManagement { repositories { google(); mavenCentral(); gradlePluginPortal() } }\ndependencyResolutionManagement { repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS); repositories { google(); mavenCentral() } }\nrootProject.name = 'mata-generated-android'\ninclude ':app'\n")?;
    fs::write(work.join("gradle.properties"), "org.gradle.jvmargs=-Xmx2048m -Dfile.encoding=UTF-8\nandroid.useAndroidX=true\n")?;
    tools::ensure_dir(&work.join("app"))?;
    // Only environmental variables contain signing secrets; they never enter generated files.
    let signing = if opts.release && !opts.unsigned {
        let store = env::var("MATA_ANDROID_KEYSTORE").map_err(|_| "release APK signing requires MATA_ANDROID_KEYSTORE, MATA_ANDROID_KEY_ALIAS, MATA_ANDROID_KEYSTORE_PASSWORD and MATA_ANDROID_KEY_PASSWORD, or pass --unsigned")?;
        let _alias = env::var("MATA_ANDROID_KEY_ALIAS")?;
        let _store_pass = env::var("MATA_ANDROID_KEYSTORE_PASSWORD")?;
        let _key_pass = env::var("MATA_ANDROID_KEY_PASSWORD")?;
        if !Path::new(&store).is_file() { return bail("MATA_ANDROID_KEYSTORE does not point to a readable file"); }
        true
    } else { false };
    let signing_block = if signing { "signingConfigs { release { storeFile file(System.getenv('MATA_ANDROID_KEYSTORE')); storePassword System.getenv('MATA_ANDROID_KEYSTORE_PASSWORD'); keyAlias System.getenv('MATA_ANDROID_KEY_ALIAS'); keyPassword System.getenv('MATA_ANDROID_KEY_PASSWORD') } }" } else { "" };
    let release_signing = if signing { "signingConfig signingConfigs.release" } else { "" };
    let game_dep = if game { "dependencies { implementation 'androidx.games:games-activity:4.4.0' }" } else { "" };
    let gradle = format!("plugins {{ id 'com.android.application' version '8.9.2' }}\nandroid {{\n    namespace {}\n    compileSdk {}\n    defaultConfig {{\n        applicationId {}\n        minSdk {}\n        targetSdk {}\n        versionCode {}\n        versionName {}\n    }}\n    {}\n    buildTypes {{ release {{ minifyEnabled false; {} }} }}\n}}\n{}\n",
        groovy_string(app_id), config.android.compile_sdk, groovy_string(app_id), config.android.min_sdk,
        config.android.target_sdk, config.android.version_code, groovy_string(&project.version), signing_block, release_signing, game_dep);
    fs::write(work.join("app/build.gradle"), gradle)?;
    // Keep local.properties only in target output; Android Gradle also accepts ANDROID_HOME.
    let props = sdk.display().to_string().replace('\\', "\\\\").replace(':', "\\:");
    fs::write(work.join("local.properties"), format!("sdk.dir={props}\n"))?;
    let (has_icon, has_round_icon)=icon_layout::has_android(&project.root);
    if let Some(manifest) = &config.android.manifest {
        let path=config::resolve(&project.root, manifest);
        let mut xml=fs::read_to_string(&path)?;
        if (has_icon || has_round_icon) && !xml.contains("<application") {
            return bail(format!("custom Android manifest {} has no <application> element",path.display()));
        }
        if let Some(at)=xml.find("<application") {
            if let Some(end_offset)=xml[at..].find('>') {
                let end=at+end_offset;
                let opening=&xml[at..end];
                let mut extra=String::new();
                if has_icon && !opening.contains("android:icon=") {extra.push_str(" android:icon=\"@mipmap/ic_launcher\"");}
                if has_round_icon && !opening.contains("android:roundIcon=") {extra.push_str(" android:roundIcon=\"@mipmap/ic_launcher_round\"");}
                xml.insert_str(end,&extra);
            }
        }
        fs::write(main.join("AndroidManifest.xml"),xml)?;
    } else {
        let perms = config.android.permissions.iter().map(|p| format!("    <uses-permission android:name=\"{}\" />\n", config::xml_escape(p))) .collect::<String>();
        let manifest = format!(r#"<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android">
{perms}    <application android:label="{}" android:hasCode="{}" android:extractNativeLibs="true"{}{}>
        <activity android:name="{}" android:exported="true" android:configChanges="orientation|keyboardHidden|screenSize|uiMode">
            <meta-data android:name="android.app.lib_name" android:value="{}" />
            <intent-filter>
                <action android:name="android.intent.action.MAIN" />
                <category android:name="android.intent.category.LAUNCHER" />
            </intent-filter>
        </activity>
    </application>
</manifest>
"#, config::xml_escape(&project.name), if java_enabled { "true" } else { "false" },
            if has_icon { " android:icon=\"@mipmap/ic_launcher\"" } else { "" },
            if has_round_icon { " android:roundIcon=\"@mipmap/ic_launcher_round\"" } else { "" },
            config::xml_escape(&config.android.activity), config::xml_escape(&lib));
        fs::write(main.join("AndroidManifest.xml"), manifest)?;
    }
    for (name, path) in [("res", config.android.resources.as_ref()), ("assets", config.android.assets.as_ref())] {
        let dest = main.join(name);
        if dest.exists() { fs::remove_dir_all(&dest)?; }
        if let Some(p) = path { tools::copy_tree(&config::resolve(&project.root, p), &dest)?; }
    }
    icon_layout::install_android(&project.root, main)?;
    if let Some(assets) = &config.app.assets {
        tools::copy_tree(&config::resolve(&project.root, assets), &main.join("assets"))?;
    }
    let java_root = main.join("java");
    if java_root.exists() { fs::remove_dir_all(&java_root)?; }
    for src in &config.android.java_sources {
        let path = config::resolve(&project.root, src);
        if !path.exists() { return bail(format!("Java source not found: {}", path.display())); }
        if path.is_file() {
            tools::copy_tree(&path, &java_root.join(path.file_name().ok_or("Java source without file name")?))?;
        } else { tools::copy_tree(&path, &java_root)?; }
    }
    Ok(())
}

fn adb_command(opts: &Options) -> Result<Command> {
    let adb = tools::require_program("adb")?;
    let mut cmd = Command::new(adb);
    if let Some(serial) = &opts.device { cmd.arg("-s").arg(serial); }
    Ok(cmd)
}
fn available_devices() -> Result<Vec<String>> {
    let output = Command::new(tools::require_program("adb")?).arg("devices").output()?;
    if !output.status.success() { return bail("adb devices failed"); }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().skip(1).filter_map(|s| {
        let mut parts = s.split_whitespace();
        let id = parts.next()?;
        (parts.next() == Some("device")).then_some(id.to_owned())
    }).collect())
}
fn device_serial(opts: &Options) -> Result<String> {
    let devices = available_devices()?;
    if let Some(device) = &opts.device {
        return if devices.contains(device) { Ok(device.clone()) }
        else { bail(format!("device '{device}' is not connected and authorized")) };
    }
    match devices.as_slice() {
        [device] => Ok(device.clone()),
        [] => bail("no Android device/emulator connected. Start emulator or authorize USB debugging"),
        _ => bail(format!("multiple Android devices found: {}. Use --device SERIAL", devices.join(", "))),
    }
}
fn device_abi(opts: &Options) -> Result<String> {
    let serial = device_serial(opts)?;
    let output = Command::new(tools::require_program("adb")?)
        .args(["-s", &serial, "shell", "getprop", "ro.product.cpu.abi"]).output()?;
    if !output.status.success() { return bail("unable to query device CPU ABI through ADB"); }
    let abi = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    abi_to_target(&abi)?;
    Ok(abi)
}

pub fn install_and_launch(apk: &Path, config: &Config, opts: &Options) -> Result<()> {
    let serial = device_serial(opts)?;
    let app_id = config::required_id(config)?;
    let mut install = adb_command(opts)?;
    // Supply the resolved serial even when user didn't specify it.
    if opts.device.is_none() { install.arg("-s").arg(&serial); }
    install.args(["install", "-r", "-t"]).arg(apk);
    tools::run(&mut install, opts.verbose)?;
    let mut launch = adb_command(opts)?;
    if opts.device.is_none() { launch.arg("-s").arg(&serial); }
    launch.args(["shell", "am", "start", "-n"])
        .arg(format!("{app_id}/{}", config.android.activity));
    tools::run(&mut launch, opts.verbose)?;
    println!("Installed and launched {app_id} on {serial}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn abi_mappings() {
        assert_eq!(abi_to_target("arm64-v8a").unwrap().0, "aarch64-linux-android");
        assert_eq!(abi_to_target("x86_64").unwrap().0, "x86_64-linux-android");
        assert!(abi_to_target("mips").is_err());
    }
}
