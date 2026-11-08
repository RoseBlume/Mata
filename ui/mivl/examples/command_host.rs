//! cargo run --example command_host 
//! (requires the existing Mata input patches described in INSTALL_INPUT.md)
use mivl::{command, native::NativeApp, Engine, Value};

#[command]
fn greet(name:String) -> String {format!("Hello from Rust, {name}!")}

#[command]
fn add_numbers(a:i32, b:i32) -> i32 {a+b}

fn main()->Result<(), Box<dyn std::error::Error>> {
    let source=include_str!("commands.mivl");
    let mut engine=Engine::parse(source)?;
    register_greet(&mut engine)?;
    register_add_numbers(&mut engine)?;
    engine.register_command("log_message", |args| {
        println!("MIVL: {}",args[0].display());
        Ok(Value::Null)
    })?;
    let mut app=NativeApp::new(engine)?;
    app.run()?;
    Ok(())
}
