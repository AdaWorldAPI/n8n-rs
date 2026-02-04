//! Runtime context and configuration for workflow execution.

use n8n_workflow::{ExecutionContext, WorkflowExecuteMode};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Runtime configuration for the execution engine.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Default execution timeout in seconds.
    pub default_timeout: u64,
    /// Maximum concurrent node executions.
    pub max_concurrency: usize,
    /// Whether to save execution progress.
    pub save_progress: bool,
    /// Default timezone.
    pub timezone: String,
    /// Binary data storage mode.
    pub binary_mode: BinaryStorageMode,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            default_timeout: 300,     // 5 minutes
            max_concurrency: 10,
            save_progress: true,
            timezone: "UTC".to_string(),
            binary_mode: BinaryStorageMode::Memory,
        }
    }
}

/// Binary data storage mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryStorageMode {
    Memory,
    FileSystem,
    S3,
}

/// Runtime context shared across node executions.
#[derive(Clone)]
pub struct RuntimeContext {
    /// Execution context.
    pub execution_context: ExecutionContext,
    /// Runtime configuration.
    pub config: RuntimeConfig,
    /// Shared state storage.
    state: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    /// Cancellation token.
    cancel_token: tokio_util::sync::CancellationToken,
}

impl RuntimeContext {
    /// Create a new runtime context.
    pub fn new(mode: WorkflowExecuteMode, config: RuntimeConfig) -> Self {
        Self {
            execution_context: ExecutionContext::new(mode),
            config,
            state: Arc::new(RwLock::new(HashMap::new())),
            cancel_token: tokio_util::sync::CancellationToken::new(),
        }
    }

    /// Get a value from shared state.
    pub async fn get_state(&self, key: &str) -> Option<serde_json::Value> {
        self.state.read().await.get(key).cloned()
    }

    /// Set a value in shared state.
    pub async fn set_state(&self, key: String, value: serde_json::Value) {
        self.state.write().await.insert(key, value);
    }

    /// Check if execution is canceled.
    pub fn is_canceled(&self) -> bool {
        self.cancel_token.is_cancelled()
    }

    /// Cancel the execution.
    pub fn cancel(&self) {
        self.cancel_token.cancel();
    }

    /// Get the cancellation token for async operations.
    pub fn cancellation_token(&self) -> tokio_util::sync::CancellationToken {
        self.cancel_token.clone()
    }

    /// Wait for cancellation.
    pub async fn wait_for_cancellation(&self) {
        self.cancel_token.cancelled().await;
    }
}
