//! # MIVL
//!
//! An interpreted, QML-inspired UI description language. All parsing, evaluation,
//! property bindings, and event dispatch are implemented in the Rust standard library.
//! Native rendering is available directly through `mata-ui` / `mata-win` (no feature flag).
//!
//! This is its own language, not a binary-compatible implementation of Qt QML or JavaScript.
//!
//! ```
//! use mivl::{Engine, Value};
//! let mut ui = Engine::parse(r#"
//!     Window {
//!         property int count: 1
//!         Text { id: heading; text: "Count: " + count }
//!         Button { id: increase; text: "Add"; onClicked: { count += 1 } }
//!     }
//! "#).unwrap();
//! assert_eq!(ui.get("heading.text").unwrap(), Value::String("Count: 1".into()));
//! ui.emit("increase", "clicked").unwrap();
//! assert_eq!(ui.get("heading.text").unwrap(), Value::String("Count: 2".into()));
//! ```

mod ast;
mod lexer;
mod parser;
mod runtime;
mod text_layout;
mod bridge;

pub mod native;

pub use ast::{Expr, Node, Operator, Property, Statement};
pub use parser::parse;
pub use runtime::{Engine, Value, LayoutRect};
pub use mivl_macros::command;
pub use bridge::{FrontendHandle, CommandFuture, install_default_commands, clear_default_commands};
mod commands;
pub use commands::{FromCommandArg, IntoCommandResult};

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub message: String,
    pub line: usize,
    pub column: usize,
}
impl Error {
    pub(crate) fn new(message: impl Into<String>, line: usize, column: usize) -> Self {
        Self { message: message.into(), line, column }
    }
    pub(crate) fn runtime(message: impl Into<String>) -> Self { Self::new(message, 0, 0) }
    /// Construct a command registration / invocation error.
    pub fn command(message: impl Into<String>) -> Self { Self::new(message, 0, 0) }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 { write!(f, "{}", self.message) }
        else { write!(f, "{}:{}: {}", self.line, self.column, self.message) }
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

/// A compiled-in MIVL document with the same runtime semantics as `Engine`.
///
/// Generated component types created by [`include_modules!`] wrap this type.
/// They provide [`CompiledWindow::engine_mut`] for registering host commands
/// before the native window starts (and before `onCompleted` is emitted).
#[derive(Debug, Clone)]
pub struct CompiledWindow {
    engine: Engine,
}

impl CompiledWindow {
    /// Parse the statically embedded MIVL source into a new independent window.
    pub fn new(source: &str) -> Result<Self> {
        Ok(Self { engine: Engine::parse(source)? })
    }
    pub fn engine(&self) -> &Engine { &self.engine }
    pub fn engine_mut(&mut self) -> &mut Engine { &mut self.engine }
    pub fn into_engine(self) -> Engine { self.engine }
    pub fn get(&self, path: &str) -> Result<Value> { self.engine.get(path) }
    pub fn set(&mut self, path: &str, value: impl Into<Value>) -> Result<()> {
        self.engine.set(path, value)
    }
    pub fn emit(&mut self, target_id: &str, signal: &str) -> Result<()> {
        self.engine.emit(target_id, signal)
    }
    pub fn frontend_handle(&self)->FrontendHandle {self.engine.frontend_handle()}
    pub fn pump_messages(&mut self)->Result<usize> {self.engine.pump_messages()}
    pub fn set_listener_enabled(&mut self,id:&str,enabled:bool)->Result<()> {self.engine.set_listener_enabled(id,enabled)}
    pub fn register_command<F>(&mut self, name: &str, callback: F) -> Result<()>
    where F: FnMut(&[Value]) -> Result<Value> + 'static {
        self.engine.register_command(name, callback)
    }
    /// Run the current document using Mata's retained native widget backend.
    /// Commands should be registered before calling `run()`.
    pub fn run(self) -> Result<()> {
        let mut app = native::NativeApp::new(self.engine)?;
        app.run()
    }
}

/// Bring generated `.mivl` components into scope, like Slint's
/// `slint::include_modules!();`.
///
/// The consumer's `build.rs` must call `mivl_build::compile("ui/main.mivl")`
/// or `mivl_build::compile_files(&[...])`, which writes
/// `$OUT_DIR/mivl_generated.rs`. Generated component type names are derived
/// from the source file names (`main_window.mivl` -> `MainWindow`).
///
/// ```ignore
/// mivl::include_modules!();
/// let mut window = MainWindow::new()?;
/// window.set("root.title", "Hello from Rust")?;
/// window.run()?;
/// ```
#[macro_export]
macro_rules! include_modules {
    () => {
        include!(concat!(env!("OUT_DIR"), "/mivl_generated.rs"));
    };
}

/// Register decorated Rust functions from any module.
///
/// Register commands for *new engines created on this thread*:
/// `mivl::register_commands![resources::add_numbers, greet];`
///
/// Register explicitly on an existing engine:
/// `mivl::register_commands![in &mut engine; resources::add_numbers, greet]?;`
///
/// Each function must have `#[mivl::command]`. The default form replaces the
/// thread-local default registrar list; it does not mutate existing engines.
#[macro_export]
macro_rules! register_commands {
    (in $engine:expr; $($($path:ident)::+),+ $(,)?) => {{
        (|| -> $crate::Result<()> {
            $( $($path)::+::register($engine)?; )+
            Ok(())
        })()
    }};
    ($($($path:ident)::+),+ $(,)?) => {{
        $crate::install_default_commands(&[$($($path)::+::register as fn(&mut $crate::Engine)->$crate::Result<()>),+]);
    }};
}
