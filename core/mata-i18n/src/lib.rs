//! # mata-i18n
//!
//! Self-contained internationalization for Mata, implemented with Rust `std`
//! and **no external dependencies**. Bundles are UTF-8 `.lang`/`.properties`
//! files or nested-string JSON dictionaries. Built-in plural forms support
//! selected languages; unrecognized languages use a simple `one`/`other` rule.
//!
//! ```rust
//! use mata_i18n::{I18n, args};
//! let i18n = I18n::new("en-US")?;
//! i18n.add_resource("en", "greeting = Hello, {name}!\nfiles = {count, plural, one {# file} other {# files}}")?;
//! i18n.set_locale("en-CA")?;
//! assert_eq!(i18n.tr_with("greeting", &args! {"name" => "Mata"}), "Hello, Mata!");
//! assert_eq!(i18n.tr_with("files", &args! {"count" => 3}), "3 files");
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod catalog;
mod error;
mod format;
mod locale;
mod message;
mod resource;

pub use catalog::I18n;
pub use error::I18nError;
pub use format::{Date, DateStyle, format_currency, format_date, format_integer, format_number};
pub use locale::{Locale, LocaleError, TextDirection, negotiate, system_locale};
pub use message::{Args, Value};
