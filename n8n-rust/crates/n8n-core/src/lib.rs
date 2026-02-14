//! # n8n-core
//!
//! Workflow execution engine for n8n-rust.
//!
//! This crate provides the core execution logic for running workflows,
//! including the stack-based execution model that enables:
//! - Resumable executions
//! - Wait nodes (pause and resume)
//! - Partial execution (test specific nodes)
//! - Error handling and retry logic

pub mod chess_workflow;
pub mod credentials;
pub mod engine;
pub mod error;
pub mod executor;
pub mod expression;
pub mod node_types;
pub mod runtime;
pub mod storage;

pub use credentials::{CredentialError, CredentialService, DecryptedCredentialData};
pub use engine::*;
pub use error::*;
pub use executor::*;
pub use expression::{
    ExpressionContext, ExpressionError, ExpressionEvaluator, ExpressionResult,
    parse, parse_template, resolve_parameter,
};
pub use runtime::*;
pub use storage::{
    ExecutionStorage, WorkflowStorage, MemoryExecutionStorage, MemoryWorkflowStorage,
};
