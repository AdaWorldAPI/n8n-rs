//! Workflow gRPC service implementation.

use n8n_arrow::{batch_to_ipc_bytes, run_data_to_batch, run_to_summary_batch};
use n8n_core::{
    ExecutionEngineError, ExecutionEvent, MemoryExecutionStorage, MemoryWorkflowStorage,
    RuntimeConfig, WorkflowEngine,
};
use n8n_workflow::{
    ExecutionStatus, Node, NodeExecutionData, Run, Workflow, WorkflowExecuteMode,
};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio_stream::{wrappers::ReceiverStream, Stream};
use tonic::{Request, Response, Status};

/// Workflow service state.
pub struct WorkflowServiceState {
    pub workflows: Arc<MemoryWorkflowStorage>,
    pub executions: Arc<MemoryExecutionStorage>,
    pub engine: Arc<WorkflowEngine>,
    pub running_executions: Arc<RwLock<HashMap<String, mpsc::Sender<()>>>>,
}

impl WorkflowServiceState {
    pub fn new() -> Self {
        Self {
            workflows: Arc::new(MemoryWorkflowStorage::new()),
            executions: Arc::new(MemoryExecutionStorage::new()),
            engine: Arc::new(WorkflowEngine::new(RuntimeConfig::default())),
            running_executions: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for WorkflowServiceState {
    fn default() -> Self {
        Self::new()
    }
}

/// gRPC workflow service implementation.
///
/// This provides both JSON and Arrow streaming responses.
#[derive(Clone)]
pub struct WorkflowGrpcService {
    state: Arc<WorkflowServiceState>,
}

impl WorkflowGrpcService {
    pub fn new(state: Arc<WorkflowServiceState>) -> Self {
        Self { state }
    }

    /// Create a workflow.
    pub async fn create_workflow(&self, workflow: Workflow) -> Result<Workflow, Status> {
        // Validate workflow
        workflow
            .validate()
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        // Save workflow
        self.state
            .workflows
            .save_workflow(&workflow)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(workflow)
    }

    /// Get a workflow by ID.
    pub async fn get_workflow(&self, id: &str) -> Result<Workflow, Status> {
        self.state
            .workflows
            .get_workflow(id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found(format!("Workflow {} not found", id)))
    }

    /// Update a workflow.
    pub async fn update_workflow(&self, workflow: Workflow) -> Result<Workflow, Status> {
        // Check workflow exists
        let _ = self.get_workflow(&workflow.id).await?;

        // Validate and save
        workflow
            .validate()
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.state
            .workflows
            .save_workflow(&workflow)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(workflow)
    }

    /// Delete a workflow.
    pub async fn delete_workflow(&self, id: &str) -> Result<bool, Status> {
        self.state
            .workflows
            .delete_workflow(id)
            .await
            .map_err(|e| Status::internal(e.to_string()))
    }

    /// List workflows.
    pub async fn list_workflows(&self) -> Result<Vec<Workflow>, Status> {
        self.state
            .workflows
            .list_workflows()
            .await
            .map_err(|e| Status::internal(e.to_string()))
    }

    /// Execute a workflow and return the result.
    pub async fn execute_workflow(
        &self,
        workflow_id: &str,
        input_data: Option<Vec<NodeExecutionData>>,
        mode: WorkflowExecuteMode,
    ) -> Result<ExecutionResult, Status> {
        let workflow = self.get_workflow(workflow_id).await?;

        let run = self
            .state
            .engine
            .execute(&workflow, mode, input_data)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let execution_id = uuid::Uuid::new_v4().to_string();

        // Save execution
        self.state
            .executions
            .save_execution(&execution_id, &run)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(ExecutionResult {
            execution_id,
            workflow_id: workflow_id.to_string(),
            run,
        })
    }

    /// Execute a workflow with streaming events.
    pub async fn execute_workflow_stream(
        &self,
        workflow_id: &str,
        input_data: Option<Vec<NodeExecutionData>>,
        mode: WorkflowExecuteMode,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ExecutionEventMessage, Status>> + Send>>, Status>
    {
        let workflow = self.get_workflow(workflow_id).await?;
        let execution_id = uuid::Uuid::new_v4().to_string();

        let (event_tx, event_rx) = mpsc::channel(100);
        let (cancel_tx, mut cancel_rx) = mpsc::channel::<()>(1);

        // Store cancel sender
        self.state
            .running_executions
            .write()
            .await
            .insert(execution_id.clone(), cancel_tx);

        let engine = self.state.engine.clone();
        let executions = self.state.executions.clone();
        let exec_id = execution_id.clone();

        // Spawn execution task
        tokio::spawn(async move {
            let (internal_tx, mut internal_rx) = mpsc::channel(100);

            let engine_handle = tokio::spawn(async move {
                engine
                    .execute_with_events(&workflow, mode, input_data, internal_tx)
                    .await
            });

            loop {
                tokio::select! {
                    Some(event) = internal_rx.recv() => {
                        let msg = match event {
                            ExecutionEvent::Started { execution_id, workflow_id } => {
                                ExecutionEventMessage::Started { execution_id, workflow_id }
                            }
                            ExecutionEvent::NodeStarted { node_name, run_index } => {
                                ExecutionEventMessage::NodeStarted { node_name, run_index }
                            }
                            ExecutionEvent::NodeFinished { node_name, run_index, task_data } => {
                                ExecutionEventMessage::NodeFinished {
                                    node_name,
                                    run_index,
                                    output_count: task_data.data.as_ref()
                                        .map(|d| d.values().flat_map(|v| v.iter().map(|i| i.len())).sum())
                                        .unwrap_or(0),
                                }
                            }
                            ExecutionEvent::Finished { result } => {
                                // Save execution
                                let _ = executions.save_execution(&exec_id, &result).await;
                                ExecutionEventMessage::Finished {
                                    status: result.status,
                                }
                            }
                            ExecutionEvent::Error { error } => {
                                ExecutionEventMessage::Error {
                                    message: error.message,
                                }
                            }
                        };

                        if event_tx.send(Ok(msg)).await.is_err() {
                            break;
                        }
                    }
                    _ = cancel_rx.recv() => {
                        engine_handle.abort();
                        let _ = event_tx.send(Ok(ExecutionEventMessage::Canceled)).await;
                        break;
                    }
                    else => break,
                }
            }
        });

        Ok(Box::pin(ReceiverStream::new(event_rx)))
    }

    /// Execute and stream results as Arrow IPC.
    pub async fn execute_workflow_arrow_stream(
        &self,
        workflow_id: &str,
        input_data: Option<Vec<NodeExecutionData>>,
        mode: WorkflowExecuteMode,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<bytes::Bytes, Status>> + Send>>, Status> {
        let result = self
            .execute_workflow(workflow_id, input_data, mode)
            .await?;

        let (tx, rx) = mpsc::channel(10);

        // Convert results to Arrow and stream
        tokio::spawn(async move {
            // Stream run data as Arrow batches
            if let Ok(batch) = run_data_to_batch(&result.run.data.result_data.run_data) {
                if let Ok(bytes) = batch_to_ipc_bytes(&batch) {
                    let _ = tx.send(Ok(bytes)).await;
                }
            }

            // Stream summary
            if let Ok(summary_batch) =
                run_to_summary_batch(&result.execution_id, &result.workflow_id, &result.run)
            {
                if let Ok(bytes) = batch_to_ipc_bytes(&summary_batch) {
                    let _ = tx.send(Ok(bytes)).await;
                }
            }
        });

        Ok(Box::pin(ReceiverStream::new(rx)))
    }

    /// Get execution by ID.
    pub async fn get_execution(&self, id: &str) -> Result<Run, Status> {
        self.state
            .executions
            .get_execution(id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found(format!("Execution {} not found", id)))
    }

    /// Cancel a running execution.
    pub async fn cancel_execution(&self, execution_id: &str) -> Result<bool, Status> {
        let mut running = self.state.running_executions.write().await;
        if let Some(cancel_tx) = running.remove(execution_id) {
            let _ = cancel_tx.send(()).await;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

/// Result of workflow execution.
pub struct ExecutionResult {
    pub execution_id: String,
    pub workflow_id: String,
    pub run: Run,
}

/// Streaming execution event message.
#[derive(Debug, Clone)]
pub enum ExecutionEventMessage {
    Started {
        execution_id: String,
        workflow_id: String,
    },
    NodeStarted {
        node_name: String,
        run_index: usize,
    },
    NodeFinished {
        node_name: String,
        run_index: usize,
        output_count: usize,
    },
    Finished {
        status: ExecutionStatus,
    },
    Error {
        message: String,
    },
    Canceled,
}

/// Serialization format for responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseFormat {
    /// JSON serialization.
    Json,
    /// Arrow IPC format (zero-copy).
    Arrow,
}
