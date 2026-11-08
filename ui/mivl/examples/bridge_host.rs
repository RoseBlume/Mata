// Run with: cargo run --example bridge_host 
use mivl::{command, Engine};
use std::{thread, time::Duration};

#[command]
fn greet(name: String) -> String {format!("Hello {name}")}

mod resources {
    use mivl::command;
    #[command]
    pub async fn multiply(a: i32,b: i32)->i32 {a*b}
}
fn main()->mivl::Result<()> {
    // Exact requested syntax. Applies to new engines created on this thread.
    mivl::register_commands![resources::multiply, greet];
    let engine=Engine::parse(include_str!("bridge.mivl"))?;
    let handle=engine.frontend_handle();
    thread::spawn(move|| {
        let _=handle.emit("startup","MIVL frontend is connected");
        for i in 0..10 {
            thread::sleep(Duration::from_millis(500));
            let _=handle.emit("counter",1);
            let _=handle.emit("notification",format!("Background notification #{i}"));
        }
    });
    mivl::native::NativeApp::new(engine)?.run()
}
