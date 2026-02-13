//! Expression evaluation system for n8n.
//!
//! This module provides the expression parser and evaluator for n8n's
//! expression syntax: `{{ $json.field }}`, `{{ $node.Name.json }}`, etc.

pub mod parser;
pub mod evaluator;
pub mod extensions;
pub mod variables;

pub use evaluator::*;
pub use parser::*;
pub use extensions::*;
pub use variables::*;

use n8n_workflow::NodeExecutionData;
use serde_json::Value;
use std::collections::HashMap;

/// Expression evaluation context.
#[derive(Debug, Clone)]
pub struct ExpressionContext<'a> {
    /// Current item being processed.
    pub item: &'a NodeExecutionData,
    /// Item index in current batch.
    pub item_index: usize,
    /// Run index for the current node.
    pub run_index: usize,
    /// Access to other nodes' data.
    pub node_data: &'a HashMap<String, Vec<Vec<NodeExecutionData>>>,
    /// Workflow variables.
    pub variables: &'a HashMap<String, Value>,
    /// Environment variables.
    pub env: &'a HashMap<String, String>,
    /// Execution metadata.
    pub execution_id: &'a str,
    /// Workflow ID.
    pub workflow_id: &'a str,
    /// Workflow name.
    pub workflow_name: &'a str,
    /// Current node name.
    pub node_name: &'a str,
}

impl<'a> ExpressionContext<'a> {
    /// Create a minimal context for testing.
    pub fn minimal(item: &'a NodeExecutionData) -> Self {
        static EMPTY_NODE_DATA: std::sync::OnceLock<HashMap<String, Vec<Vec<NodeExecutionData>>>> =
            std::sync::OnceLock::new();
        static EMPTY_VARIABLES: std::sync::OnceLock<HashMap<String, Value>> =
            std::sync::OnceLock::new();
        static EMPTY_ENV: std::sync::OnceLock<HashMap<String, String>> = std::sync::OnceLock::new();

        Self {
            item,
            item_index: 0,
            run_index: 0,
            node_data: EMPTY_NODE_DATA.get_or_init(HashMap::new),
            variables: EMPTY_VARIABLES.get_or_init(HashMap::new),
            env: EMPTY_ENV.get_or_init(HashMap::new),
            execution_id: "",
            workflow_id: "",
            workflow_name: "",
            node_name: "",
        }
    }
}

/// Result type for expression operations.
pub type ExpressionResult<T> = Result<T, ExpressionError>;

/// Expression evaluation error.
#[derive(Debug, thiserror::Error)]
pub enum ExpressionError {
    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Undefined variable: {0}")]
    UndefinedVariable(String),

    #[error("Property not found: {0}")]
    PropertyNotFound(String),

    #[error("Invalid index: {0}")]
    InvalidIndex(String),

    #[error("Type error: expected {expected}, got {actual}")]
    TypeError { expected: String, actual: String },

    #[error("Method not found: {0}")]
    MethodNotFound(String),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("Node not found: {0}")]
    NodeNotFound(String),

    #[error("Evaluation error: {0}")]
    EvaluationError(String),
}
