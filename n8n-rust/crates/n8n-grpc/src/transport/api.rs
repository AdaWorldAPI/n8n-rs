//! REST API handlers for n8n-compatible endpoints.
//!
//! Provides 1:1 mapping to n8n's REST API endpoints for workflows
//! and executions.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use n8n_core::{ExecutionStorage, WorkflowStorage, MemoryExecutionStorage, MemoryWorkflowStorage};
use n8n_workflow::{Connection, ExecutionStatus, Node, Run, Workflow, WorkflowExecuteMode, WorkflowSettings};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use tokio::sync::RwLock;

/// API state containing storage backends.
#[derive(Clone)]
pub struct ApiState {
    pub workflows: Arc<MemoryWorkflowStorage>,
    pub executions: Arc<ExecutionStore>,
}

/// Extended execution store that tracks execution metadata.
pub struct ExecutionStore {
    inner: MemoryExecutionStorage,
    /// Map of execution ID -> (workflow_id, workflow_name)
    execution_metadata: RwLock<HashMap<String, ExecutionMetadata>>,
}

#[derive(Clone)]
pub struct ExecutionMetadata {
    workflow_id: String,
    workflow_name: String,
}

impl ExecutionStore {
    pub fn new() -> Self {
        Self {
            inner: MemoryExecutionStorage::new(),
            execution_metadata: RwLock::new(HashMap::new()),
        }
    }

    pub async fn save_execution(&self, id: &str, workflow_id: &str, workflow_name: &str, run: &Run) -> Result<(), n8n_core::ExecutionEngineError> {
        self.execution_metadata.write().await.insert(
            id.to_string(),
            ExecutionMetadata {
                workflow_id: workflow_id.to_string(),
                workflow_name: workflow_name.to_string(),
            },
        );
        self.inner.save_execution(id, run).await
    }

    pub async fn get_execution(&self, id: &str) -> Result<Option<(Run, Option<ExecutionMetadata>)>, n8n_core::ExecutionEngineError> {
        let run = self.inner.get_execution(id).await?;
        let metadata = self.execution_metadata.read().await.get(id).cloned();
        Ok(run.map(|r| (r, metadata)))
    }

    pub async fn delete_execution(&self, id: &str) -> Result<bool, n8n_core::ExecutionEngineError> {
        self.execution_metadata.write().await.remove(id);
        self.inner.delete_execution(id).await
    }

    pub async fn list_all_executions(&self) -> Result<Vec<(String, Run, Option<ExecutionMetadata>)>, n8n_core::ExecutionEngineError> {
        // Get all metadata
        let metadata = self.execution_metadata.read().await.clone();
        let mut results = Vec::new();

        for (id, meta) in metadata.iter() {
            if let Some(run) = self.inner.get_execution(id).await? {
                results.push((id.clone(), run, Some(meta.clone())));
            }
        }

        Ok(results)
    }
}

impl Default for ExecutionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ApiState {
    pub fn new(
        workflows: Arc<MemoryWorkflowStorage>,
        executions: Arc<ExecutionStore>,
    ) -> Self {
        Self { workflows, executions }
    }
}

// ============================================================================
// Workflow API Types
// ============================================================================

/// Request body for creating/updating a workflow.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRequest {
    pub name: String,
    #[serde(default)]
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub connections: HashMap<String, HashMap<String, Vec<Vec<Connection>>>>,
    #[serde(default)]
    pub settings: Option<serde_json::Value>,
    #[serde(default)]
    pub static_data: Option<serde_json::Value>,
}

/// Workflow response matching n8n's API format.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowResponse {
    pub id: String,
    pub name: String,
    pub active: bool,
    pub nodes: Vec<Node>,
    pub connections: HashMap<String, HashMap<String, Vec<Vec<Connection>>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub static_data: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<&Workflow> for WorkflowResponse {
    fn from(w: &Workflow) -> Self {
        Self {
            id: w.id.clone(),
            name: w.name.clone(),
            active: w.active,
            nodes: w.nodes.clone(),
            connections: w.connections.clone(),
            settings: serde_json::to_value(&w.settings).ok(),
            static_data: w.static_data.as_ref().and_then(|d| serde_json::to_value(d).ok()),
            created_at: w.created_at.unwrap_or_else(Utc::now),
            updated_at: w.updated_at.unwrap_or_else(Utc::now),
        }
    }
}

/// List workflows query parameters.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListWorkflowsQuery {
    #[serde(default)]
    pub active: Option<bool>,
    #[serde(default)]
    pub tags: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub cursor: Option<String>,
}

/// Paginated list response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResponse<T> {
    pub data: Vec<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

// ============================================================================
// Execution API Types
// ============================================================================

/// Request body for creating an execution.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionRequest {
    pub workflow_id: String,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
}

/// Execution response matching n8n's API format.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionResponse {
    pub id: String,
    pub workflow_id: Option<String>,
    pub finished: bool,
    pub mode: String,
    pub status: String,
    pub started_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stopped_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait_till: Option<DateTime<Utc>>,
    pub data: ExecutionDataResponse,
}

/// Execution data in response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionDataResponse {
    pub result_data: ResultDataResponse,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_data: Option<serde_json::Value>,
}

/// Result data in response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultDataResponse {
    pub run_data: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_node_executed: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<serde_json::Value>,
}

impl ExecutionResponse {
    fn from_run(id: String, run: &Run, workflow_id: Option<String>) -> Self {
        let finished = run.status.is_finished();

        Self {
            id,
            workflow_id,
            finished,
            mode: run.mode.as_str().to_string(),
            status: run.status.as_str().to_string(),
            started_at: run.started_at,
            stopped_at: run.finished_at,
            wait_till: run.wait_till,
            data: ExecutionDataResponse {
                result_data: ResultDataResponse {
                    run_data: serde_json::to_value(&run.data.result_data.run_data).unwrap_or_default(),
                    last_node_executed: run.data.result_data.last_node_executed.clone(),
                    error: run.data.result_data.error.as_ref().map(|e| {
                        serde_json::json!({
                            "message": e.message
                        })
                    }),
                },
                execution_data: None,
            },
        }
    }
}

/// List executions query parameters.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListExecutionsQuery {
    #[serde(default)]
    pub workflow_id: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub finished: Option<bool>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub cursor: Option<String>,
}

// ============================================================================
// API Error Type
// ============================================================================

/// API error response.
#[derive(Debug, Serialize)]
pub struct ApiError {
    pub code: u16,
    pub message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = StatusCode::from_u16(self.code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status, Json(self)).into_response()
    }
}

// ============================================================================
// Workflow Handlers
// ============================================================================

/// GET /workflows - List all workflows.
pub async fn list_workflows(
    State(state): State<ApiState>,
    Query(query): Query<ListWorkflowsQuery>,
) -> Result<Json<ListResponse<WorkflowResponse>>, ApiError> {
    let workflows = state.workflows.list_workflows().await
        .map_err(|e| ApiError {
            code: 500,
            message: e.to_string(),
        })?;

    let mut data: Vec<WorkflowResponse> = workflows
        .iter()
        .filter(|w| {
            if let Some(active) = query.active {
                if w.active != active {
                    return false;
                }
            }
            true
        })
        .map(WorkflowResponse::from)
        .collect();

    // Apply limit
    if let Some(limit) = query.limit {
        data.truncate(limit);
    }

    Ok(Json(ListResponse {
        data,
        next_cursor: None,
    }))
}

/// POST /workflows - Create a new workflow.
pub async fn create_workflow(
    State(state): State<ApiState>,
    Json(request): Json<WorkflowRequest>,
) -> Result<(StatusCode, Json<WorkflowResponse>), ApiError> {
    // Parse settings from JSON or use defaults
    let settings: WorkflowSettings = request.settings
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let workflow = Workflow {
        id: Uuid::new_v4().to_string(),
        name: request.name,
        active: false,
        nodes: request.nodes,
        connections: request.connections,
        settings,
        static_data: None, // TODO: Convert from serde_json::Value to DataObject
        pin_data: None,
        description: None,
        version_id: None,
        created_at: Some(Utc::now()),
        updated_at: Some(Utc::now()),
    };

    state.workflows.save_workflow(&workflow).await
        .map_err(|e| ApiError {
            code: 500,
            message: e.to_string(),
        })?;

    Ok((StatusCode::CREATED, Json(WorkflowResponse::from(&workflow))))
}

/// GET /workflows/:id - Get a workflow by ID.
pub async fn get_workflow(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<WorkflowResponse>, ApiError> {
    let workflow = state.workflows.get_workflow(&id).await
        .map_err(|e| ApiError {
            code: 500,
            message: e.to_string(),
        })?
        .ok_or_else(|| ApiError {
            code: 404,
            message: format!("Workflow {} not found", id),
        })?;

    Ok(Json(WorkflowResponse::from(&workflow)))
}

/// PUT /workflows/:id - Update a workflow.
pub async fn update_workflow(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Json(request): Json<WorkflowRequest>,
) -> Result<Json<WorkflowResponse>, ApiError> {
    let existing = state.workflows.get_workflow(&id).await
        .map_err(|e| ApiError {
            code: 500,
            message: e.to_string(),
        })?
        .ok_or_else(|| ApiError {
            code: 404,
            message: format!("Workflow {} not found", id),
        })?;

    // Parse settings from JSON or keep existing
    let settings: WorkflowSettings = request.settings
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or(existing.settings);

    let workflow = Workflow {
        id,
        name: request.name,
        active: existing.active,
        nodes: request.nodes,
        connections: request.connections,
        settings,
        static_data: existing.static_data, // Keep existing static data
        pin_data: existing.pin_data,
        description: existing.description,
        version_id: existing.version_id,
        created_at: existing.created_at,
        updated_at: Some(Utc::now()),
    };

    state.workflows.save_workflow(&workflow).await
        .map_err(|e| ApiError {
            code: 500,
            message: e.to_string(),
        })?;

    Ok(Json(WorkflowResponse::from(&workflow)))
}

/// DELETE /workflows/:id - Delete a workflow.
pub async fn delete_workflow(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    state.workflows.delete_workflow(&id).await
        .map_err(|e| ApiError {
            code: 500,
            message: e.to_string(),
        })?;

    Ok(StatusCode::NO_CONTENT)
}

/// POST /workflows/:id/activate - Activate a workflow.
pub async fn activate_workflow(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<WorkflowResponse>, ApiError> {
    let mut workflow = state.workflows.get_workflow(&id).await
        .map_err(|e| ApiError {
            code: 500,
            message: e.to_string(),
        })?
        .ok_or_else(|| ApiError {
            code: 404,
            message: format!("Workflow {} not found", id),
        })?;

    workflow.active = true;
    workflow.updated_at = Some(Utc::now());

    state.workflows.save_workflow(&workflow).await
        .map_err(|e| ApiError {
            code: 500,
            message: e.to_string(),
        })?;

    Ok(Json(WorkflowResponse::from(&workflow)))
}

/// POST /workflows/:id/deactivate - Deactivate a workflow.
pub async fn deactivate_workflow(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<WorkflowResponse>, ApiError> {
    let mut workflow = state.workflows.get_workflow(&id).await
        .map_err(|e| ApiError {
            code: 500,
            message: e.to_string(),
        })?
        .ok_or_else(|| ApiError {
            code: 404,
            message: format!("Workflow {} not found", id),
        })?;

    workflow.active = false;
    workflow.updated_at = Some(Utc::now());

    state.workflows.save_workflow(&workflow).await
        .map_err(|e| ApiError {
            code: 500,
            message: e.to_string(),
        })?;

    Ok(Json(WorkflowResponse::from(&workflow)))
}

// ============================================================================
// Execution Handlers
// ============================================================================

/// GET /executions - List executions.
pub async fn list_executions(
    State(state): State<ApiState>,
    Query(query): Query<ListExecutionsQuery>,
) -> Result<Json<ListResponse<ExecutionResponse>>, ApiError> {
    let executions = state.executions.list_all_executions().await
        .map_err(|e| ApiError {
            code: 500,
            message: e.to_string(),
        })?;

    let mut data: Vec<ExecutionResponse> = executions
        .into_iter()
        .filter(|(_, run, metadata)| {
            if let Some(ref wf_id) = query.workflow_id {
                if let Some(meta) = metadata {
                    if &meta.workflow_id != wf_id {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            if let Some(finished) = query.finished {
                if run.status.is_finished() != finished {
                    return false;
                }
            }
            if let Some(ref status) = query.status {
                if run.status.as_str() != status {
                    return false;
                }
            }
            true
        })
        .map(|(id, run, metadata)| {
            ExecutionResponse::from_run(id, &run, metadata.map(|m| m.workflow_id))
        })
        .collect();

    // Apply limit
    if let Some(limit) = query.limit {
        data.truncate(limit);
    }

    Ok(Json(ListResponse {
        data,
        next_cursor: None,
    }))
}

/// POST /executions - Create and start an execution.
pub async fn create_execution(
    State(state): State<ApiState>,
    Json(request): Json<ExecutionRequest>,
) -> Result<(StatusCode, Json<ExecutionResponse>), ApiError> {
    // Get the workflow
    let workflow = state.workflows.get_workflow(&request.workflow_id).await
        .map_err(|e| ApiError {
            code: 500,
            message: e.to_string(),
        })?
        .ok_or_else(|| ApiError {
            code: 404,
            message: format!("Workflow {} not found", request.workflow_id),
        })?;

    let mode = request.mode.as_deref().unwrap_or("manual");
    let execute_mode = WorkflowExecuteMode::from_str(mode).unwrap_or_default();

    // Create the execution
    let execution_id = Uuid::new_v4().to_string();
    let run = Run::new(execute_mode);

    state.executions.save_execution(
        &execution_id,
        &request.workflow_id,
        &workflow.name,
        &run
    ).await
        .map_err(|e| ApiError {
            code: 500,
            message: e.to_string(),
        })?;

    // Note: In a full implementation, this would actually execute the workflow
    // For now, we just create the execution record

    Ok((
        StatusCode::CREATED,
        Json(ExecutionResponse::from_run(execution_id, &run, Some(request.workflow_id)))
    ))
}

/// GET /executions/:id - Get an execution by ID.
pub async fn get_execution(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<ExecutionResponse>, ApiError> {
    let (run, metadata) = state.executions.get_execution(&id).await
        .map_err(|e| ApiError {
            code: 500,
            message: e.to_string(),
        })?
        .ok_or_else(|| ApiError {
            code: 404,
            message: format!("Execution {} not found", id),
        })?;

    Ok(Json(ExecutionResponse::from_run(id, &run, metadata.map(|m| m.workflow_id))))
}

/// DELETE /executions/:id - Delete an execution.
pub async fn delete_execution(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    state.executions.delete_execution(&id).await
        .map_err(|e| ApiError {
            code: 500,
            message: e.to_string(),
        })?;

    Ok(StatusCode::NO_CONTENT)
}

/// POST /executions/:id/stop - Stop a running execution.
pub async fn stop_execution(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<ExecutionResponse>, ApiError> {
    let (mut run, metadata) = state.executions.get_execution(&id).await
        .map_err(|e| ApiError {
            code: 500,
            message: e.to_string(),
        })?
        .ok_or_else(|| ApiError {
            code: 404,
            message: format!("Execution {} not found", id),
        })?;

    if run.status.is_finished() {
        return Err(ApiError {
            code: 400,
            message: "Execution is already finished".to_string(),
        });
    }

    run.finish(ExecutionStatus::Canceled);

    let workflow_id = metadata.as_ref().map(|m| m.workflow_id.clone()).unwrap_or_default();
    let workflow_name = metadata.as_ref().map(|m| m.workflow_name.clone()).unwrap_or_default();

    state.executions.save_execution(&id, &workflow_id, &workflow_name, &run).await
        .map_err(|e| ApiError {
            code: 500,
            message: e.to_string(),
        })?;

    Ok(Json(ExecutionResponse::from_run(id, &run, Some(workflow_id))))
}

/// POST /executions/:id/retry - Retry a failed execution.
pub async fn retry_execution(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<ExecutionResponse>), ApiError> {
    let (_, metadata) = state.executions.get_execution(&id).await
        .map_err(|e| ApiError {
            code: 500,
            message: e.to_string(),
        })?
        .ok_or_else(|| ApiError {
            code: 404,
            message: format!("Execution {} not found", id),
        })?;

    // Create a new execution as a retry
    let new_execution_id = Uuid::new_v4().to_string();
    let retry = Run::new(WorkflowExecuteMode::Retry);

    let workflow_id = metadata.as_ref().map(|m| m.workflow_id.clone()).unwrap_or_default();
    let workflow_name = metadata.as_ref().map(|m| m.workflow_name.clone()).unwrap_or_default();

    state.executions.save_execution(&new_execution_id, &workflow_id, &workflow_name, &retry).await
        .map_err(|e| ApiError {
            code: 500,
            message: e.to_string(),
        })?;

    Ok((
        StatusCode::CREATED,
        Json(ExecutionResponse::from_run(new_execution_id, &retry, Some(workflow_id)))
    ))
}

// ============================================================================
// Router Setup
// ============================================================================

use axum::{Router, routing::{get as axum_get, post as axum_post}};

/// Create the n8n-compatible REST API router.
pub fn create_api_router(state: ApiState) -> Router {
    Router::new()
        // Workflow endpoints
        .route("/api/v1/workflows", axum_get(list_workflows).post(create_workflow))
        .route("/api/v1/workflows/:id", axum_get(get_workflow).put(update_workflow).delete(delete_workflow))
        .route("/api/v1/workflows/:id/activate", axum_post(activate_workflow))
        .route("/api/v1/workflows/:id/deactivate", axum_post(deactivate_workflow))
        // Execution endpoints
        .route("/api/v1/executions", axum_get(list_executions).post(create_execution))
        .route("/api/v1/executions/:id", axum_get(get_execution).delete(delete_execution))
        .route("/api/v1/executions/:id/stop", axum_post(stop_execution))
        .route("/api/v1/executions/:id/retry", axum_post(retry_execution))
        .with_state(state)
}
