//! # n8n-contract
//!
//! Unified execution contract for collaboration between:
//! - **n8n-rs** — workflow orchestration engine
//! - **crewai-rust** — AI agent delegation (crew.* steps)
//! - **ladybug-rs** — cognitive database / CAM operations (lb.* steps)
//!
//! This crate is the **single source of truth** for the types and routing
//! logic that bridge these three systems.  It provides:
//!
//! - [`types`] — `UnifiedStep`, `UnifiedExecution`, `DataEnvelope`, `StepStatus`
//! - [`envelope`] — conversion helpers between n8n items and envelopes
//! - [`crew_router`] — HTTP client that delegates `crew.*` steps to crewai-rust
//! - [`ladybug_router`] — HTTP client that delegates `lb.*` steps to ladybug-rs
//! - [`pg_store`] — (feature `postgres`) persistence of executions/steps
//! - [`executors`] — `NodeExecutor` adapters so the n8n engine can route to crew/ladybug

pub mod types;
pub mod envelope;
pub mod crew_router;
pub mod ladybug_router;
pub mod executors;

#[cfg(feature = "postgres")]
pub mod pg_store;

pub use types::*;
pub use envelope::*;
pub use crew_router::CrewRouter;
pub use ladybug_router::LadybugRouter;
pub use executors::{CrewAgentExecutor, LadybugResonateExecutor, LadybugCollapseExecutor};
