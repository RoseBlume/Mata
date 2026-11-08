use std::{error::Error as StdError, fmt, io};

#[derive(Debug)]
pub enum SqliteError {
    Io(io::Error),
    InvalidHeader(String),
    Corrupt(String),
    Sql(String),
    Type(String),
    QueryReturnedNoRows,
}

impl fmt::Display for SqliteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::InvalidHeader(s) => write!(f, "invalid SQLite header: {s}"),
            Self::Corrupt(s) => write!(f, "corrupt SQLite database: {s}"),
            Self::Sql(s) => write!(f, "SQL error: {s}"),
            Self::Type(s) => write!(f, "SQLite value conversion error: {s}"),
            Self::QueryReturnedNoRows => write!(f, "query returned no rows"),
        }
    }
}
impl StdError for SqliteError {}
impl From<io::Error> for SqliteError { fn from(v: io::Error) -> Self { Self::Io(v) } }
