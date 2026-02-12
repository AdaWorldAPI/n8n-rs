//! Execution entities - matches n8n's ExecutionEntity and ExecutionData.
//!
//! Reference: packages/@n8n/db/src/entities/execution-entity.ts
//!            packages/@n8n/db/src/entities/execution-data.ts

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use n8n_workflow::{ExecutionStatus, WorkflowExecuteMode};

use super::generate_nano_id;

/// ExecutionEntity - workflow execution record.
///
/// Matches the TypeORM entity exactly.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ExecutionEntity {
    /// Primary key - nano ID.
    pub id: String,

    /// Whether execution is finished (deprecated - use status).
    pub finished: bool,

    /// How the execution was triggered.
    pub mode: String,

    /// Execution status.
    pub status: String,

    /// When execution record was created.
    pub created_at: DateTime<Utc>,

    /// When execution actually started running.
    #[sqlx(default)]
    pub started_at: Option<DateTime<Utc>>,

    /// When execution stopped (success or failure).
    #[sqlx(default)]
    pub stopped_at: Option<DateTime<Utc>>,

    /// Soft-delete timestamp.
    #[sqlx(default)]
    pub deleted_at: Option<DateTime<Utc>>,

    /// Reference to workflow (nullable if workflow deleted).
    #[sqlx(default)]
    pub workflow_id: Option<String>,

    /// ID of original execution if this is a retry.
    #[sqlx(default)]
    pub retry_of: Option<String>,

    /// ID of successful retry if this execution failed.
    #[sqlx(default)]
    pub retry_success_id: Option<String>,

    /// When a waiting execution should resume.
    #[sqlx(default)]
    pub wait_till: Option<DateTime<Utc>>,

    /// Where execution data is stored: 'db', 'fs', or 's3'.
    pub stored_at: String,
}

impl ExecutionEntity {
    /// Create a new execution record.
    pub fn new(workflow_id: &str, mode: WorkflowExecuteMode) -> Self {
        let now = Utc::now();
        Self {
            id: generate_nano_id(),
            finished: false,
            mode: mode.as_str().to_string(),
            status: ExecutionStatus::New.as_str().to_string(),
            created_at: now,
            started_at: None,
            stopped_at: None,
            deleted_at: None,
            workflow_id: Some(workflow_id.to_string()),
            retry_of: None,
            retry_success_id: None,
            wait_till: None,
            stored_at: "db".to_string(),
        }
    }

    /// Mark execution as started.
    pub fn start(&mut self) {
        self.started_at = Some(Utc::now());
        self.status = ExecutionStatus::Running.as_str().to_string();
    }

    /// Mark execution as completed with given status.
    pub fn complete(&mut self, status: ExecutionStatus) {
        self.stopped_at = Some(Utc::now());
        self.status = status.as_str().to_string();
        self.finished = status.is_finished();
    }

    /// Check if execution is in a terminal state.
    pub fn is_finished(&self) -> bool {
        self.finished
            || matches!(
                ExecutionStatus::from_str(&self.status),
                ExecutionStatus::Success
                    | ExecutionStatus::Error
                    | ExecutionStatus::Canceled
                    | ExecutionStatus::Crashed
            )
    }

    /// Get the parsed execution status.
    pub fn get_status(&self) -> ExecutionStatus {
        ExecutionStatus::from_str(&self.status)
    }

    /// Get the parsed execution mode.
    pub fn get_mode(&self) -> WorkflowExecuteMode {
        WorkflowExecuteMode::from_str(&self.mode)
    }
}

/// ExecutionData - separate table for large execution data.
///
/// This is kept separate from ExecutionEntity to optimize storage
/// and query performance for execution metadata.
///
/// Reference: packages/@n8n/db/src/entities/execution-data.ts
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ExecutionData {
    /// FK to execution_entity (also primary key).
    pub execution_id: String,

    /// Serialized IRunExecutionData as JSON text.
    /// We store as text rather than JSONB for consistency with n8n.
    pub data: String,

    /// Workflow snapshot at execution time.
    #[sqlx(json)]
    pub workflow_data: serde_json::Value,

    /// Workflow version at execution time.
    #[sqlx(default)]
    pub workflow_version_id: Option<String>,
}

impl ExecutionData {
    /// Create new execution data.
    pub fn new(
        execution_id: &str,
        data: &n8n_workflow::RunExecutionData,
        workflow_data: serde_json::Value,
        workflow_version_id: Option<String>,
    ) -> Result<Self, serde_json::Error> {
        Ok(Self {
            execution_id: execution_id.to_string(),
            data: serde_json::to_string(data)?,
            workflow_data,
            workflow_version_id,
        })
    }

    /// Parse the stored execution data.
    pub fn parse_data(&self) -> Result<n8n_workflow::RunExecutionData, serde_json::Error> {
        serde_json::from_str(&self.data)
    }
}

/// ExecutionMetadata - arbitrary key-value metadata for executions.
///
/// Reference: packages/@n8n/db/src/entities/execution-metadata.ts
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ExecutionMetadata {
    pub id: i32,
    pub execution_id: String,
    pub key: String,
    pub value: String,
}

impl ExecutionMetadata {
    /// Create new metadata entry (without id - auto-generated).
    pub fn new(execution_id: &str, key: &str, value: &str) -> Self {
        Self {
            id: 0, // Will be set by database
            execution_id: execution_id.to_string(),
            key: key.to_string(),
            value: value.to_string(),
        }
    }
}

/// Query filters for executions.
#[derive(Debug, Clone, Default)]
pub struct ExecutionFilters {
    pub workflow_id: Option<String>,
    pub status: Option<Vec<ExecutionStatus>>,
    pub mode: Option<Vec<WorkflowExecuteMode>>,
    pub finished: Option<bool>,
    pub started_after: Option<DateTime<Utc>>,
    pub started_before: Option<DateTime<Utc>>,
    pub include_deleted: bool,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

impl ExecutionFilters {
    pub fn for_workflow(workflow_id: &str) -> Self {
        Self {
            workflow_id: Some(workflow_id.to_string()),
            ..Default::default()
        }
    }

    pub fn running() -> Self {
        Self {
            status: Some(vec![ExecutionStatus::Running, ExecutionStatus::New]),
            finished: Some(false),
            ..Default::default()
        }
    }

    pub fn waiting() -> Self {
        Self {
            status: Some(vec![ExecutionStatus::Waiting]),
            ..Default::default()
        }
    }
}

/// Insert parameters for creating an execution.
#[derive(Debug, Clone)]
pub struct InsertExecution {
    pub id: String,
    pub workflow_id: Option<String>,
    pub mode: String,
    pub status: String,
}

impl From<&ExecutionEntity> for InsertExecution {
    fn from(e: &ExecutionEntity) -> Self {
        Self {
            id: e.id.clone(),
            workflow_id: e.workflow_id.clone(),
            mode: e.mode.clone(),
            status: e.status.clone(),
        }
    }
}

/// Update parameters for execution.
#[derive(Debug, Clone, Default)]
pub struct UpdateExecution {
    pub finished: Option<bool>,
    pub status: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub stopped_at: Option<DateTime<Utc>>,
    pub wait_till: Option<Option<DateTime<Utc>>>,
    pub retry_success_id: Option<String>,
}

/// Execution with its data joined.
#[derive(Debug, Clone)]
pub struct ExecutionWithData {
    pub execution: ExecutionEntity,
    pub data: Option<ExecutionData>,
    pub metadata: Vec<ExecutionMetadata>,
}
