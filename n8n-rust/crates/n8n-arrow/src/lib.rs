//! # n8n-arrow
//!
//! Apache Arrow integration with zero-copy data transfer for n8n-rust.
//!
//! This crate provides:
//! - Conversion between n8n workflow data and Arrow RecordBatches
//! - Zero-copy data streaming via Arrow IPC
//! - DataFusion integration for SQL queries on workflow data
//! - Arrow Flight server/client for efficient network transfer

pub mod convert;
pub mod error;
pub mod flight;
pub mod ipc;
pub mod schema;

pub use convert::*;
pub use error::*;
pub use flight::*;
pub use ipc::*;
pub use schema::*;
