//! Error types for the execution engine.

use n8n_workflow::WorkflowError;
use thiserror::Error;

/// Errors that can occur during workflow execution.
#[derive(Error, Debug)]
pub enum ExecutionEngineError {
    #[error("Workflow error: {0}")]
    Workflow(#[from] WorkflowError),

    #[error("Node execution error in '{node}': {message}")]
    NodeExecution { node: String, message: String },

    #[error("No start nodes found in workflow")]
    NoStartNodes,

    #[error("Execution was canceled")]
    Canceled,

    #[error("Execution timed out after {0} seconds")]
    Timeout(u64),

    #[error("Invalid execution state: {0}")]
    InvalidState(String),

    #[error("Node type not found: {0}")]
    NodeTypeNotFound(String),

    #[error("Missing input data for node '{0}'")]
    MissingInput(String),

    #[error("Credential error: {0}")]
    Credential(String),

    #[error("Expression evaluation error: {0}")]
    Expression(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<ExecutionEngineError> for n8n_workflow::ExecutionError {
    fn from(e: ExecutionEngineError) -> Self {
        n8n_workflow::ExecutionError::new(e.to_string())
    }
}
