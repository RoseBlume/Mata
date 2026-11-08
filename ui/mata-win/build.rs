use std::{env, path::PathBuf, process::Command};
fn main() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") { return; }
    println!("cargo:rerun-if-changed=src/window/macos/native.m");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let object = out.join("mata_macos_window.o");
    let target = env::var("TARGET").expect("TARGET");
    let compiler = env::var("CC").unwrap_or_else(|_| "clang".to_string());
    let status = Command::new(&compiler)
        .args(["-target", &target, "-fobjc-arc", "-fmodules", "-O2", "-fPIC", "-c", "src/window/macos/native.m", "-o"])
        .arg(&object).status().expect("Apple clang is needed to compile the native macOS backend");
    assert!(status.success(), "macOS Objective-C window backend failed to compile");
    let archive = out.join("libmata_macos_window.a");
    let status = Command::new("ar").arg("crs").arg(&archive).arg(&object).status().expect("ar");
    assert!(status.success(), "macOS backend archive failed");
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=mata_macos_window");
    for name in ["AppKit", "Foundation", "QuartzCore", "Metal"] {
        println!("cargo:rustc-link-lib=framework={name}");
    }
}
