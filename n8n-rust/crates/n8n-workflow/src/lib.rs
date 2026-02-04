//! # n8n-workflow
//!
//! Core workflow types and execution model for n8n-rust.
//! This crate provides the fundamental data structures that mirror
//! the TypeScript n8n workflow definitions.

pub mod connection;
pub mod data;
pub mod error;
pub mod execution;
pub mod node;
pub mod workflow;

pub use connection::*;
pub use data::*;
pub use error::*;
pub use execution::*;
pub use node::*;
pub use workflow::*;
