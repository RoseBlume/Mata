#![forbid(unsafe_code)]
//! Mata's own dependency-free, pure Rust SQLite-file engine.
//!
//! Supports a deliberately limited SQL subset used by StudyForge.
//! Not a drop-in implementation of the full SQLite virtual machine.
mod error;
mod pure;
pub use error::SqliteError;
pub use pure::{Connection, FromValue, MappedRows, OptionalExtension, Params, Result, Row, RowIndex, Statement, Transaction, Value};
