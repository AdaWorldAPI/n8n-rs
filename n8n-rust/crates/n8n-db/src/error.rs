//! Database error types.

use thiserror::Error;

/// Database operation errors.
#[derive(Error, Debug)]
pub enum DbError {
    /// Entity not found.
    #[error("Entity not found")]
    NotFound,

    /// Duplicate key violation.
    #[error("Duplicate key: {0}")]
    DuplicateKey(String),

    /// Foreign key violation.
    #[error("Foreign key violation: {0}")]
    ForeignKeyViolation(String),

    /// Invalid data.
    #[error("Invalid data: {0}")]
    InvalidData(String),

    /// Connection error.
    #[error("Connection error: {0}")]
    ConnectionError(String),

    /// Query error.
    #[error("Query error: {0}")]
    QueryError(String),

    /// Migration error.
    #[error("Migration error: {0}")]
    MigrationError(String),

    /// Transaction error.
    #[error("Transaction error: {0}")]
    TransactionError(String),

    /// Serialization error.
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// SQLx error.
    #[error("Database error: {0}")]
    SqlxError(#[from] sqlx::Error),
}

impl DbError {
    /// Check if this is a not found error.
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound)
    }

    /// Check if this is a duplicate key error.
    pub fn is_duplicate(&self) -> bool {
        matches!(self, Self::DuplicateKey(_))
    }
}

/// Result type for database operations.
pub type DbResult<T> = Result<T, DbError>;
