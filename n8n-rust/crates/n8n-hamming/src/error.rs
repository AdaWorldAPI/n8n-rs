//! Error types for Hamming vector operations.

use thiserror::Error;

/// Errors that can occur during Hamming vector operations.
#[derive(Error, Debug, Clone)]
pub enum HammingError {
    #[error("Invalid vector size: expected {expected} bytes, got {actual}")]
    InvalidSize { expected: usize, actual: usize },

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Invalid hex string: {0}")]
    InvalidHex(String),
}
