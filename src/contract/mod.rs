//! Unified execution contract module
//!
//! Shared types and adapters for cross-runtime workflow execution.
//! These types serialize to identical JSON across ada-n8n, crewai-rust,
//! and ladybug-rs — enabling each runtime to route steps to the others
//! without any cross-Cargo dependencies.

pub mod crew_router;
pub mod envelope;
pub mod pg_store;
pub mod types;

pub use crew_router::{CrewRouter, LadybugRouter};
pub use types::*;
