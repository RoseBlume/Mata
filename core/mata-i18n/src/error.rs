use std::fmt;
use std::io;

#[derive(Debug)]
pub enum I18nError {
    Locale(crate::LocaleError),
    Io(io::Error),
    Resource { line: usize, message: String },
    Message { key: String, position: usize, message: String },
    Json { position: usize, message: String },
    MissingKey(String),
    MissingArgument(String),
    InvalidArgument { name: String, message: String },
}
impl fmt::Display for I18nError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Locale(e) => write!(f, "{e}"),
            Self::Io(e) => write!(f, "I/O: {e}"),
            Self::Resource { line, message } => write!(f, "resource line {line}: {message}"),
            Self::Message { key, position, message } => write!(f, "message '{key}' at byte {position}: {message}"),
            Self::Json { position, message } => write!(f, "JSON at byte {position}: {message}"),
            Self::MissingKey(key) => write!(f, "missing translation for '{key}'"),
            Self::MissingArgument(key) => write!(f, "missing message argument '{key}'"),
            Self::InvalidArgument { name, message } => write!(f, "invalid argument '{name}': {message}"),
        }
    }
}
impl std::error::Error for I18nError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self { Self::Io(e) => Some(e), Self::Locale(e) => Some(e), _ => None }
    }
}
impl From<io::Error> for I18nError { fn from(e: io::Error) -> Self { Self::Io(e) } }
impl From<crate::LocaleError> for I18nError { fn from(e: crate::LocaleError) -> Self { Self::Locale(e) } }
