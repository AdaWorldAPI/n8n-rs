//! Arrow data streaming service.

use n8n_arrow::{
    batch_to_ipc_bytes, ipc_bytes_to_batches, node_execution_data_to_batch,
    run_data_to_batch, workflow_connections_to_batch, workflow_nodes_to_batch, ArrowError,
    WorkflowFlightService,
};
use n8n_core::{MemoryExecutionStorage, MemoryWorkflowStorage};
use n8n_workflow::NodeExecutionData;
use arrow_array::RecordBatch;
use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::BoxStream;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::{wrappers::ReceiverStream, Stream, StreamExt};
use tonic::Status;

/// Arrow data service for zero-copy streaming.
#[derive(Clone)]
pub struct ArrowDataService {
    workflows: Arc<MemoryWorkflowStorage>,
    executions: Arc<MemoryExecutionStorage>,
}

impl ArrowDataService {
    pub fn new(
        workflows: Arc<MemoryWorkflowStorage>,
        executions: Arc<MemoryExecutionStorage>,
    ) -> Self {
        Self {
            workflows,
            executions,
        }
    }

    /// Stream execution data as Arrow IPC.
    pub async fn stream_execution_data(
        &self,
        execution_id: &str,
        node_names: Option<Vec<String>>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes, Status>> + Send>>, Status> {
        let run = self
            .executions
            .get_execution(execution_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found(format!("Execution {} not found", execution_id)))?;

        let (tx, rx) = mpsc::channel(100);

        // Filter run data if node names specified
        let run_data = if let Some(names) = node_names {
            run.data
                .result_data
                .run_data
                .into_iter()
                .filter(|(k, _)| names.contains(k))
                .collect()
        } else {
            run.data.result_data.run_data
        };

        tokio::spawn(async move {
            // Convert to Arrow and stream
            match run_data_to_batch(&run_data) {
                Ok(batch) => {
                    match batch_to_ipc_bytes(&batch) {
                        Ok(bytes) => {
                            let _ = tx.send(Ok(bytes)).await;
                        }
                        Err(e) => {
                            let _ = tx.send(Err(Status::internal(e.to_string()))).await;
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(Err(Status::internal(e.to_string()))).await;
                }
            }
        });

        Ok(Box::pin(ReceiverStream::new(rx)))
    }

    /// Get node output as Arrow.
    pub async fn get_node_output_arrow(
        &self,
        execution_id: &str,
        node_name: &str,
        run_index: Option<usize>,
    ) -> Result<Bytes, Status> {
        let run = self
            .executions
            .get_execution(execution_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found(format!("Execution {} not found", execution_id)))?;

        let task_data = run
            .data
            .result_data
            .run_data
            .get(node_name)
            .ok_or_else(|| Status::not_found(format!("Node {} not found in execution", node_name)))?;

        let idx = run_index.unwrap_or(0);
        let task = task_data
            .get(idx)
            .ok_or_else(|| Status::not_found(format!("Run index {} not found", idx)))?;

        // Extract output data
        let output_data: Vec<NodeExecutionData> = task
            .data
            .as_ref()
            .and_then(|d| d.get("main"))
            .map(|outputs| outputs.iter().flatten().cloned().collect())
            .unwrap_or_default();

        let batch = node_execution_data_to_batch(&output_data)
            .map_err(|e| Status::internal(e.to_string()))?;

        batch_to_ipc_bytes(&batch).map_err(|e| Status::internal(e.to_string()))
    }

    /// Get workflow structure as Arrow.
    pub async fn get_workflow_arrow(&self, workflow_id: &str) -> Result<WorkflowArrowData, Status> {
        let workflow = self
            .workflows
            .get_workflow(workflow_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found(format!("Workflow {} not found", workflow_id)))?;

        let nodes_batch =
            workflow_nodes_to_batch(&workflow).map_err(|e| Status::internal(e.to_string()))?;

        let connections_batch = workflow_connections_to_batch(&workflow)
            .map_err(|e| Status::internal(e.to_string()))?;

        let nodes_bytes =
            batch_to_ipc_bytes(&nodes_batch).map_err(|e| Status::internal(e.to_string()))?;

        let connections_bytes =
            batch_to_ipc_bytes(&connections_batch).map_err(|e| Status::internal(e.to_string()))?;

        Ok(WorkflowArrowData {
            nodes: nodes_bytes,
            connections: connections_bytes,
        })
    }

    /// Store Arrow data as execution result.
    pub async fn put_arrow_data(
        &self,
        _execution_id: &str,
        data: Bytes,
    ) -> Result<PutArrowResult, Status> {
        let batches =
            ipc_bytes_to_batches(&data).map_err(|e| Status::internal(e.to_string()))?;

        let rows_written: usize = batches.iter().map(|b| b.num_rows()).sum();

        Ok(PutArrowResult {
            rows_written: rows_written as u64,
        })
    }
}

/// Workflow data in Arrow format.
pub struct WorkflowArrowData {
    pub nodes: Bytes,
    pub connections: Bytes,
}

/// Result of putting Arrow data.
pub struct PutArrowResult {
    pub rows_written: u64,
}

/// Implementation of WorkflowFlightService for Arrow Flight.
#[async_trait]
impl WorkflowFlightService for ArrowDataService {
    async fn get_execution_data(
        &self,
        execution_id: &str,
        _node_name: Option<&str>,
    ) -> Result<Vec<RecordBatch>, ArrowError> {
        let run = self
            .executions
            .get_execution(execution_id)
            .await
            .map_err(|e| ArrowError::FlightError(e.to_string()))?
            .ok_or_else(|| ArrowError::FlightError(format!("Execution {} not found", execution_id)))?;

        let batch = run_data_to_batch(&run.data.result_data.run_data)?;
        Ok(vec![batch])
    }

    async fn stream_execution_data(
        &self,
        execution_id: &str,
    ) -> Result<BoxStream<'static, Result<RecordBatch, ArrowError>>, ArrowError> {
        let run = self
            .executions
            .get_execution(execution_id)
            .await
            .map_err(|e| ArrowError::FlightError(e.to_string()))?
            .ok_or_else(|| ArrowError::FlightError(format!("Execution {} not found", execution_id)))?;

        let batch = run_data_to_batch(&run.data.result_data.run_data)?;

        let stream = futures::stream::once(async move { Ok(batch) });
        Ok(Box::pin(stream))
    }

    async fn put_execution_data(
        &self,
        _execution_id: &str,
        batches: Vec<RecordBatch>,
    ) -> Result<u64, ArrowError> {
        let rows: usize = batches.iter().map(|b| b.num_rows()).sum();
        Ok(rows as u64)
    }
}
