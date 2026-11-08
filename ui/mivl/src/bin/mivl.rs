use std::{env,fs,process};
use mivl::{Engine,native::NativeApp};
fn main() {
    if let Err(error) = start() { eprintln!("mivl: {error}"); process::exit(1); }
}
fn start() -> Result<(),Box<dyn std::error::Error>> {
    let path = env::args().nth(1).ok_or("usage: mivl <file.mivl>")?;
    let source = fs::read_to_string(path)?;
    let mut app = NativeApp::new(Engine::parse(&source)?)?;
    app.run()?;
    Ok(())
}
