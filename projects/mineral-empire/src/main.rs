#[cfg(not(target_os="android"))]
fn main() { mineral_empire::run_desktop(); }
#[cfg(target_os="android")]
fn main() {}
