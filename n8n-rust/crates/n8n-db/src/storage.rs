//! DB-backed implementations of n8n-core storage traits.
//!
//! This module bridges the gap between n8n-core's in-memory storage
//! interfaces and n8n-db's PostgreSQL repositories, providing production-ready
//! persistence for workflows and executions.

use async_trait::async_trait;
use sqlx::PgPool;

use n8n_core::error::ExecutionEngineError;
use n8n_core::storage::{ExecutionStorage, WorkflowStorage};
use n8n_workflow::{ExecutionStatus, Run, Workflow, WorkflowExecuteMode};

use crate::entities::{
    ExecutionData, ExecutionEntity, InsertExecution, InsertWorkflow,
    UpdateWorkflow, WorkflowEntity,
};
use crate::error::DbError;
use crate::repositories::{ExecutionRepository, WorkflowRepository};

// =============================================================================
// Error Conversion
// =============================================================================

/// Convert a DbError into an ExecutionEngineError::Storage.
fn db_err(e: DbError) -> ExecutionEngineError {
    ExecutionEngineError::Storage(e.to_string())
}

/// Convert a serde_json::Error into an ExecutionEngineError::Storage.
fn json_err(e: serde_json::Error) -> ExecutionEngineError {
    ExecutionEngineError::Storage(format!("JSON serialization error: {e}"))
}

// =============================================================================
// Workflow Conversions
// =============================================================================

/// Convert a `WorkflowEntity` (DB row) into a domain `Workflow`.
fn entity_to_workflow(entity: &WorkflowEntity) -> Result<Workflow, ExecutionEngineError> {
    // WorkflowEntity.connections is serde_json::Value; deserialize into WorkflowConnections.
    let connections = serde_json::from_value(entity.connections.clone()).map_err(json_err)?;

    Ok(Workflow {
        id: entity.id.clone(),
        name: entity.name.clone(),
        description: entity.description.clone(),
        active: entity.active,
        nodes: entity.nodes.clone(),
        connections,
        settings: entity.settings.clone().unwrap_or_default(),
        static_data: entity
            .static_data
            .as_ref()
            .and_then(|v| serde_json::from_value(v.clone()).ok()),
        pin_data: entity
            .pin_data
            .as_ref()
            .and_then(|v| serde_json::from_value(v.clone()).ok()),
        version_id: Some(entity.version_id.clone()),
        created_at: Some(entity.created_at),
        updated_at: Some(entity.updated_at),
    })
}

/// Convert a domain `Workflow` into an `InsertWorkflow` for creating a new DB row.
fn workflow_to_insert(workflow: &Workflow) -> Result<InsertWorkflow, ExecutionEngineError> {
    let nodes = serde_json::to_value(&workflow.nodes).map_err(json_err)?;
    let connections = serde_json::to_value(&workflow.connections).map_err(json_err)?;
    let settings = serde_json::to_value(&workflow.settings)
        .ok()
        .filter(|v| !v.is_null());
    let static_data = workflow
        .static_data
        .as_ref()
        .and_then(|d| serde_json::to_value(d).ok());
    let pin_data = workflow
        .pin_data
        .as_ref()
        .and_then(|d| serde_json::to_value(d).ok());

    Ok(InsertWorkflow {
        id: workflow.id.clone(),
        name: workflow.name.clone(),
        description: workflow.description.clone(),
        nodes,
        connections,
        settings,
        static_data,
        meta: None,
        pin_data,
        version_id: workflow
            .version_id
            .clone()
            .unwrap_or_else(crate::entities::generate_version_id),
        parent_folder_id: None,
    })
}

/// Build an `UpdateWorkflow` from a domain `Workflow`.
fn workflow_to_update(workflow: &Workflow) -> Result<UpdateWorkflow, ExecutionEngineError> {
    let nodes = serde_json::to_value(&workflow.nodes).map_err(json_err)?;
    let connections = serde_json::to_value(&workflow.connections).map_err(json_err)?;

    Ok(UpdateWorkflow {
        name: Some(workflow.name.clone()),
        description: Some(workflow.description.clone()),
        active: Some(workflow.active),
        nodes: Some(nodes),
        connections: Some(connections),
        settings: Some(
            serde_json::to_value(&workflow.settings)
                .ok()
                .filter(|v| !v.is_null()),
        ),
        static_data: Some(
            workflow
                .static_data
                .as_ref()
                .and_then(|d| serde_json::to_value(d).ok()),
        ),
        pin_data: Some(
            workflow
                .pin_data
                .as_ref()
                .and_then(|d| serde_json::to_value(d).ok()),
        ),
        ..Default::default()
    })
}

// =============================================================================
// Execution Conversions
// =============================================================================

/// Convert an `ExecutionEntity` + `ExecutionData` back into a domain `Run`.
fn entity_to_run(
    entity: &ExecutionEntity,
    exec_data: &ExecutionData,
) -> Result<Run, ExecutionEngineError> {
    let data = exec_data.parse_data().map_err(json_err)?;
    let mode = WorkflowExecuteMode::from_str(&entity.mode).unwrap_or_default();
    let status = ExecutionStatus::from_str(&entity.status).unwrap_or_default();

    Ok(Run {
        data,
        mode,
        started_at: entity.started_at.unwrap_or(entity.created_at),
        finished_at: entity.stopped_at,
        status,
        wait_till: entity.wait_till,
    })
}

// =============================================================================
// SqlxWorkflowStorage
// =============================================================================

/// PostgreSQL-backed implementation of `WorkflowStorage`.
///
/// Uses [`WorkflowRepository`] under the hood to perform all DB operations,
/// converting between the domain `Workflow` type and the persistence
/// `WorkflowEntity` type transparently.
#[derive(Clone)]
pub struct SqlxWorkflowStorage {
    repo: WorkflowRepository,
}

impl SqlxWorkflowStorage {
    /// Create a new storage backed by the given connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: WorkflowRepository::new(pool),
        }
    }

    /// Create from an existing repository.
    pub fn from_repo(repo: WorkflowRepository) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl WorkflowStorage for SqlxWorkflowStorage {
    async fn get_workflow(&self, id: &str) -> Result<Option<Workflow>, ExecutionEngineError> {
        match self.repo.find_by_id(id).await.map_err(db_err)? {
            Some(entity) => Ok(Some(entity_to_workflow(&entity)?)),
            None => Ok(None),
        }
    }

    async fn save_workflow(&self, workflow: &Workflow) -> Result<(), ExecutionEngineError> {
        // Try to update first; if the workflow doesn't exist yet, create it.
        let existing = self.repo.find_by_id(&workflow.id).await.map_err(db_err)?;

        if existing.is_some() {
            let update = workflow_to_update(workflow)?;
            self.repo
                .update(&workflow.id, &update)
                .await
                .map_err(db_err)?;
        } else {
            let insert = workflow_to_insert(workflow)?;
            self.repo.create(&insert).await.map_err(db_err)?;
        }

        Ok(())
    }

    async fn delete_workflow(&self, id: &str) -> Result<bool, ExecutionEngineError> {
        self.repo.delete(id).await.map_err(db_err)
    }

    async fn list_workflows(&self) -> Result<Vec<Workflow>, ExecutionEngineError> {
        let entities = self.repo.find_all(false).await.map_err(db_err)?;
        entities.iter().map(entity_to_workflow).collect()
    }
}

// =============================================================================
// SqlxExecutionStorage
// =============================================================================

/// PostgreSQL-backed implementation of `ExecutionStorage`.
///
/// Uses [`ExecutionRepository`] under the hood. Execution data is stored
/// across two tables (`execution_entity` for metadata and `execution_data`
/// for the full serialized run), following the same pattern as n8n's
/// TypeScript backend.
#[derive(Clone)]
pub struct SqlxExecutionStorage {
    repo: ExecutionRepository,
}

impl SqlxExecutionStorage {
    /// Create a new storage backed by the given connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: ExecutionRepository::new(pool),
        }
    }

    /// Create from an existing repository.
    pub fn from_repo(repo: ExecutionRepository) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl ExecutionStorage for SqlxExecutionStorage {
    async fn get_execution(&self, id: &str) -> Result<Option<Run>, ExecutionEngineError> {
        let with_data = self.repo.find_by_id_with_data(id).await.map_err(db_err)?;

        match with_data {
            Some(ewd) => {
                if let Some(ref data) = ewd.data {
                    Ok(Some(entity_to_run(&ewd.execution, data)?))
                } else {
                    // Execution exists but has no data blob; cannot reconstruct Run.
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    async fn save_execution(&self, id: &str, run: &Run) -> Result<(), ExecutionEngineError> {
        let status_str = run.status.as_str().to_string();
        let mode_str = run.mode.as_str().to_string();
        let finished = run.status.is_finished();

        // Upsert the execution entity.
        let existing = self.repo.find_by_id(id).await.map_err(db_err)?;

        if existing.is_none() {
            let insert = InsertExecution {
                id: id.to_string(),
                workflow_id: None, // Caller can set this via the repository directly if needed.
                mode: mode_str.clone(),
                status: status_str.clone(),
            };
            self.repo.create(&insert).await.map_err(db_err)?;
        }

        // Update the execution metadata.
        let update = crate::entities::UpdateExecution {
            finished: Some(finished),
            status: Some(status_str),
            started_at: Some(run.started_at),
            stopped_at: run.finished_at,
            ..Default::default()
        };
        self.repo.update(id, &update).await.map_err(db_err)?;

        // Serialize and save execution data.
        let data_json = serde_json::to_string(&run.data).map_err(json_err)?;
        let exec_data = ExecutionData {
            execution_id: id.to_string(),
            data: data_json,
            workflow_data: serde_json::json!({}),
            workflow_version_id: None,
        };
        self.repo.save_data(&exec_data).await.map_err(db_err)?;

        Ok(())
    }

    async fn delete_execution(&self, id: &str) -> Result<bool, ExecutionEngineError> {
        self.repo.delete(id).await.map_err(db_err)
    }

    async fn list_executions(
        &self,
        workflow_id: &str,
    ) -> Result<Vec<(String, Run)>, ExecutionEngineError> {
        let entities = self
            .repo
            .find_by_workflow(workflow_id, 100)
            .await
            .map_err(db_err)?;

        let mut results = Vec::with_capacity(entities.len());
        for entity in &entities {
            let data = self.repo.get_data(&entity.id).await.map_err(db_err)?;
            if let Some(ref data) = data {
                let run = entity_to_run(entity, data)?;
                results.push((entity.id.clone(), run));
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use n8n_workflow::{Run, RunExecutionData, WorkflowExecuteMode};

    #[test]
    fn test_entity_to_workflow_roundtrip() {
        let entity = WorkflowEntity::new("Test Workflow");
        let workflow = entity_to_workflow(&entity).expect("conversion should succeed");
        assert_eq!(workflow.id, entity.id);
        assert_eq!(workflow.name, entity.name);
        assert_eq!(workflow.active, entity.active);
    }

    #[test]
    fn test_workflow_to_insert_roundtrip() {
        let workflow = Workflow::new("Roundtrip Test");
        let insert = workflow_to_insert(&workflow).expect("conversion should succeed");
        assert_eq!(insert.id, workflow.id);
        assert_eq!(insert.name, workflow.name);
    }

    #[test]
    fn test_entity_to_run_roundtrip() {
        let entity = ExecutionEntity::new("wf-1", WorkflowExecuteMode::Manual);
        let run_data = RunExecutionData::new();
        let data_str = serde_json::to_string(&run_data).unwrap();
        let exec_data = ExecutionData {
            execution_id: entity.id.clone(),
            data: data_str,
            workflow_data: serde_json::json!({}),
            workflow_version_id: None,
        };

        let run = entity_to_run(&entity, &exec_data).expect("conversion should succeed");
        assert_eq!(run.mode, WorkflowExecuteMode::Manual);
    }
}
