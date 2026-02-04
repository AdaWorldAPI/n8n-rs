//! Storage backends for workflow data.

use crate::error::ExecutionEngineError;
use async_trait::async_trait;
use n8n_workflow::{Run, Workflow};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Trait for workflow storage backends.
#[async_trait]
pub trait WorkflowStorage: Send + Sync {
    /// Get a workflow by ID.
    async fn get_workflow(&self, id: &str) -> Result<Option<Workflow>, ExecutionEngineError>;

    /// Save a workflow.
    async fn save_workflow(&self, workflow: &Workflow) -> Result<(), ExecutionEngineError>;

    /// Delete a workflow.
    async fn delete_workflow(&self, id: &str) -> Result<bool, ExecutionEngineError>;

    /// List all workflows.
    async fn list_workflows(&self) -> Result<Vec<Workflow>, ExecutionEngineError>;
}

/// Trait for execution storage backends.
#[async_trait]
pub trait ExecutionStorage: Send + Sync {
    /// Get an execution by ID.
    async fn get_execution(&self, id: &str) -> Result<Option<Run>, ExecutionEngineError>;

    /// Save an execution.
    async fn save_execution(&self, id: &str, run: &Run) -> Result<(), ExecutionEngineError>;

    /// Delete an execution.
    async fn delete_execution(&self, id: &str) -> Result<bool, ExecutionEngineError>;

    /// List executions for a workflow.
    async fn list_executions(
        &self,
        workflow_id: &str,
    ) -> Result<Vec<(String, Run)>, ExecutionEngineError>;
}

/// In-memory workflow storage (for testing and development).
pub struct MemoryWorkflowStorage {
    workflows: Arc<RwLock<HashMap<String, Workflow>>>,
}

impl MemoryWorkflowStorage {
    pub fn new() -> Self {
        Self {
            workflows: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for MemoryWorkflowStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl WorkflowStorage for MemoryWorkflowStorage {
    async fn get_workflow(&self, id: &str) -> Result<Option<Workflow>, ExecutionEngineError> {
        Ok(self.workflows.read().await.get(id).cloned())
    }

    async fn save_workflow(&self, workflow: &Workflow) -> Result<(), ExecutionEngineError> {
        self.workflows
            .write()
            .await
            .insert(workflow.id.clone(), workflow.clone());
        Ok(())
    }

    async fn delete_workflow(&self, id: &str) -> Result<bool, ExecutionEngineError> {
        Ok(self.workflows.write().await.remove(id).is_some())
    }

    async fn list_workflows(&self) -> Result<Vec<Workflow>, ExecutionEngineError> {
        Ok(self.workflows.read().await.values().cloned().collect())
    }
}

/// In-memory execution storage.
pub struct MemoryExecutionStorage {
    executions: Arc<RwLock<HashMap<String, Run>>>,
    workflow_executions: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

impl MemoryExecutionStorage {
    pub fn new() -> Self {
        Self {
            executions: Arc::new(RwLock::new(HashMap::new())),
            workflow_executions: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for MemoryExecutionStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExecutionStorage for MemoryExecutionStorage {
    async fn get_execution(&self, id: &str) -> Result<Option<Run>, ExecutionEngineError> {
        Ok(self.executions.read().await.get(id).cloned())
    }

    async fn save_execution(&self, id: &str, run: &Run) -> Result<(), ExecutionEngineError> {
        self.executions.write().await.insert(id.to_string(), run.clone());
        Ok(())
    }

    async fn delete_execution(&self, id: &str) -> Result<bool, ExecutionEngineError> {
        Ok(self.executions.write().await.remove(id).is_some())
    }

    async fn list_executions(
        &self,
        workflow_id: &str,
    ) -> Result<Vec<(String, Run)>, ExecutionEngineError> {
        let executions = self.executions.read().await;
        let workflow_execs = self.workflow_executions.read().await;

        let ids = workflow_execs.get(workflow_id).cloned().unwrap_or_default();

        Ok(ids
            .into_iter()
            .filter_map(|id| executions.get(&id).map(|r| (id, r.clone())))
            .collect())
    }
}
