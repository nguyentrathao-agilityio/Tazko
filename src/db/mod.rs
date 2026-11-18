mod database;
mod migrate;
mod registry;

use std::fmt;

pub use database::{Database, READERS};
pub use migrate::{
    CONTROL, Migration, Migrations, TENANT, migrate, open_connection, schema_version,
};
pub use registry::{Databases, valid_slug};

#[derive(Debug)]
pub enum DbError {
    Sqlite(rusqlite::Error),
    Io(std::io::Error),
    TooNew { found: i64, known: i64 },
    BadSlug(String),
    Missing(String),
    Closed,
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DbError::Sqlite(e) => write!(f, "database error: {e}"),
            DbError::Io(e) => write!(f, "file error: {e}"),
            DbError::TooNew { found, known } => write!(
                f,
                "the database is at schema version {found}, newer than this program knows ({known}); install a newer release"
            ),
            DbError::BadSlug(s) => write!(f, "{s:?} is not a valid company name in a path"),
            DbError::Missing(s) => write!(f, "the folder for company {s:?} does not exist"),
            DbError::Closed => write!(f, "the database writer has stopped"),
        }
    }
}

impl std::error::Error for DbError {}

impl From<rusqlite::Error> for DbError {
    fn from(e: rusqlite::Error) -> Self {
        DbError::Sqlite(e)
    }
}

impl From<std::io::Error> for DbError {
    fn from(e: std::io::Error) -> Self {
        DbError::Io(e)
    }
}

pub type DbResult<T> = Result<T, DbError>;
