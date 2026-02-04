//! Error types for the n8n workflow system.

use thiserror::Error;

/// Primary error type for workflow operations.
#[derive(Error, Debug, Clone)]
pub enum WorkflowError {
    #[error("Node not found: {0}")]
    NodeNotFound(String),

    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Invalid workflow: {0}")]
    InvalidWorkflow(String),

    #[error("Execution error: {0}")]
    ExecutionError(String),

    #[error("Expression evaluation error: {0}")]
    ExpressionError(String),

    #[error("Credential error: {0}")]
    CredentialError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Node operation error in '{node}': {message}")]
    NodeOperationError { node: String, message: String },

    #[error("API error in '{node}': {message}")]
    NodeApiError {
        node: String,
        message: String,
        status_code: Option<u16>,
    },
}

/// Error context for node execution.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecutionErrorContext {
    pub node_name: Option<String>,
    pub item_index: Option<usize>,
    pub run_index: Option<usize>,
    pub description: Option<String>,
    pub cause: Option<String>,
}

impl Default for ExecutionErrorContext {
    fn default() -> Self {
        Self {
            node_name: None,
            item_index: None,
            run_index: None,
            description: None,
            cause: None,
        }
    }
}

/// Execution error with full context.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecutionError {
    pub message: String,
    pub context: ExecutionErrorContext,
    pub stack: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl ExecutionError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            context: ExecutionErrorContext::default(),
            stack: None,
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn with_node(mut self, node: impl Into<String>) -> Self {
        self.context.node_name = Some(node.into());
        self
    }

    pub fn with_item_index(mut self, index: usize) -> Self {
        self.context.item_index = Some(index);
        self
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.context.description = Some(desc.into());
        self
    }
}

impl std::fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(ref node) = self.context.node_name {
            write!(f, " (node: {})", node)?;
        }
        Ok(())
    }
}

impl std::error::Error for ExecutionError {}
