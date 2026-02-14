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
pub mod bridge;
pub mod wire_bridge;
pub mod interface_gateway;
pub mod impact_gate;
pub mod free_will;

#[cfg(feature = "postgres")]
pub mod pg_store;

pub use types::*;
pub use envelope::*;
pub use crew_router::CrewRouter;
pub use ladybug_router::LadybugRouter;
pub use executors::{CrewAgentExecutor, LadybugResonateExecutor, LadybugCollapseExecutor};
pub use interface_gateway::{InterfaceGateway, InterfaceDefinition, InterfaceProtocol, ImpactLevel};
pub use impact_gate::{ImpactGate, GateDecision, RoleDefinition};
pub use free_will::{FreeWillPipeline, ModificationProposal, ModificationType, ModificationLimits};

// Re-export the shared substrate types from ladybug-contract
pub use ladybug_contract as kernel;
