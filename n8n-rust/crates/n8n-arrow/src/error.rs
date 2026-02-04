//! Error types for Arrow operations.

use thiserror::Error;

/// Errors that can occur during Arrow operations.
#[derive(Error, Debug)]
pub enum ArrowError {
    #[error("Arrow error: {0}")]
    Arrow(#[from] arrow::error::ArrowError),

    #[error("Schema mismatch: {0}")]
    SchemaMismatch(String),

    #[error("Conversion error: {0}")]
    ConversionError(String),

    #[error("IPC error: {0}")]
    IpcError(String),

    #[error("Flight error: {0}")]
    FlightError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Invalid data: {0}")]
    InvalidData(String),
}

impl From<serde_json::Error> for ArrowError {
    fn from(e: serde_json::Error) -> Self {
        ArrowError::SerializationError(e.to_string())
    }
}
