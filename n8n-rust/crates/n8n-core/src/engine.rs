//! Workflow execution engine.
//!
//! The engine uses a stack-based execution model that enables:
//! - Resumable executions (save and restore state)
//! - Wait nodes (pause execution until a condition is met)
//! - Partial execution (test specific nodes)
//! - Error handling with configurable retry logic

use crate::error::ExecutionEngineError;
use crate::executor::{NodeExecutorRegistry, NodeOutput};
use crate::runtime::{RuntimeConfig, RuntimeContext};
use n8n_workflow::{
    connection::{graph, CONNECTION_MAIN},
    ExecuteData, ExecutionContext, ExecutionStatus, InternalExecutionData, Node,
    NodeExecutionData, ResultData, Run, RunData, RunExecutionData, StartData,
    StartNodeData, TaskData, TaskDataConnections, TaskDataConnectionsSource, Workflow,
    WorkflowExecuteMode,
};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Event emitted during workflow execution.
#[derive(Debug, Clone)]
pub enum ExecutionEvent {
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
        task_data: TaskData,
    },
    Finished {
        result: Run,
    },
    Error {
        error: n8n_workflow::ExecutionError,
    },
}

/// Workflow execution engine.
pub struct WorkflowEngine {
    /// Node executor registry.
    executors: Arc<NodeExecutorRegistry>,
    /// Runtime configuration.
    config: RuntimeConfig,
}

impl WorkflowEngine {
    /// Create a new workflow engine.
    pub fn new(config: RuntimeConfig) -> Self {
        Self {
            executors: Arc::new(NodeExecutorRegistry::new()),
            config,
        }
    }

    /// Create with custom executor registry.
    pub fn with_executors(executors: NodeExecutorRegistry, config: RuntimeConfig) -> Self {
        Self {
            executors: Arc::new(executors),
            config,
        }
    }

    /// Execute a workflow and return the result.
    pub async fn execute(
        &self,
        workflow: &Workflow,
        mode: WorkflowExecuteMode,
        input_data: Option<Vec<NodeExecutionData>>,
    ) -> Result<Run, ExecutionEngineError> {
        let (tx, _rx) = mpsc::channel(100);
        self.execute_with_events(workflow, mode, input_data, tx).await
    }

    /// Execute a workflow with event streaming.
    pub async fn execute_with_events(
        &self,
        workflow: &Workflow,
        mode: WorkflowExecuteMode,
        input_data: Option<Vec<NodeExecutionData>>,
        event_tx: mpsc::Sender<ExecutionEvent>,
    ) -> Result<Run, ExecutionEngineError> {
        // Validate workflow
        workflow.validate()?;

        // Create runtime context
        let context = RuntimeContext::new(mode, self.config.clone());

        // Initialize run
        let mut run = Run::new(mode);
        let execution_id = uuid::Uuid::new_v4().to_string();

        // Emit started event
        let _ = event_tx
            .send(ExecutionEvent::Started {
                execution_id: execution_id.clone(),
                workflow_id: workflow.id.clone(),
            })
            .await;

        // Find start nodes
        let start_nodes = self.find_start_nodes(workflow)?;

        // Initialize execution stack
        let mut stack = self.initialize_stack(workflow, &start_nodes, input_data)?;

        // Build connections by destination for parent lookups
        let connections_by_dest = graph::map_connections_by_destination(&workflow.connections);

        // Execute nodes from stack
        while let Some(execute_data) = stack.pop_front() {
            // Check for cancellation
            if context.is_canceled() {
                run.finish(ExecutionStatus::Canceled);
                return Err(ExecutionEngineError::Canceled);
            }

            let node = &execute_data.node;
            let node_name = node.name.clone();
            let run_index = run
                .data
                .result_data
                .run_data
                .get(&node_name)
                .map(|v| v.len())
                .unwrap_or(0);

            // Emit node started event
            let _ = event_tx
                .send(ExecutionEvent::NodeStarted {
                    node_name: node_name.clone(),
                    run_index,
                })
                .await;

            debug!(node = %node_name, run_index, "Executing node");

            // Execute the node
            let task_data = self
                .execute_node(&execute_data, &context, &event_tx)
                .await;

            // Store result
            run.data
                .result_data
                .run_data
                .entry(node_name.clone())
                .or_default()
                .push(task_data.clone());

            run.data.result_data.last_node_executed = Some(node_name.clone());

            // Emit node finished event
            let _ = event_tx
                .send(ExecutionEvent::NodeFinished {
                    node_name: node_name.clone(),
                    run_index,
                    task_data: task_data.clone(),
                })
                .await;

            // Handle errors
            if task_data.execution_status == ExecutionStatus::Error {
                if node.continue_on_fail {
                    warn!(node = %node_name, "Node failed but continue_on_fail is set");
                } else {
                    error!(node = %node_name, "Node execution failed");
                    run.data.result_data.error = task_data.error.clone();
                    run.finish(ExecutionStatus::Error);

                    let _ = event_tx
                        .send(ExecutionEvent::Error {
                            error: task_data
                                .error
                                .clone()
                                .unwrap_or_else(|| n8n_workflow::ExecutionError::new("Unknown error")),
                        })
                        .await;

                    return Ok(run);
                }
            }

            // Queue child nodes
            if let Some(output_data) = &task_data.data {
                self.queue_child_nodes(
                    workflow,
                    &node_name,
                    output_data,
                    run_index,
                    &mut stack,
                )?;
            }
        }

        // Execution completed successfully
        run.finish(ExecutionStatus::Success);
        info!(workflow_id = %workflow.id, "Workflow execution completed");

        let _ = event_tx
            .send(ExecutionEvent::Finished { result: run.clone() })
            .await;

        Ok(run)
    }

    /// Find start nodes in the workflow.
    fn find_start_nodes(&self, workflow: &Workflow) -> Result<Vec<String>, ExecutionEngineError> {
        // First try to find trigger nodes
        let triggers: Vec<_> = workflow
            .get_trigger_nodes()
            .into_iter()
            .map(|n| n.name.clone())
            .collect();

        if !triggers.is_empty() {
            return Ok(triggers);
        }

        // Fall back to nodes with no incoming connections
        let start_nodes: Vec<_> = workflow
            .get_start_nodes()
            .into_iter()
            .filter(|n| !n.disabled)
            .map(|n| n.name.clone())
            .collect();

        if start_nodes.is_empty() {
            return Err(ExecutionEngineError::NoStartNodes);
        }

        Ok(start_nodes)
    }

    /// Initialize the execution stack.
    fn initialize_stack(
        &self,
        workflow: &Workflow,
        start_nodes: &[String],
        input_data: Option<Vec<NodeExecutionData>>,
    ) -> Result<VecDeque<ExecuteData>, ExecutionEngineError> {
        let mut stack = VecDeque::new();

        let initial_data = input_data.unwrap_or_else(|| vec![NodeExecutionData::default()]);

        for node_name in start_nodes {
            let node = workflow
                .get_node(node_name)
                .ok_or_else(|| ExecutionEngineError::NodeExecution {
                    node: node_name.clone(),
                    message: "Start node not found".to_string(),
                })?;

            let mut data = TaskDataConnections::new();
            data.insert(CONNECTION_MAIN.to_string(), vec![initial_data.clone()]);

            stack.push_back(ExecuteData {
                node: node.clone(),
                data,
                source: None,
                metadata: None,
            });
        }

        Ok(stack)
    }

    /// Execute a single node.
    async fn execute_node(
        &self,
        execute_data: &ExecuteData,
        context: &RuntimeContext,
        _event_tx: &mpsc::Sender<ExecutionEvent>,
    ) -> TaskData {
        let mut task_data = TaskData::new();
        let node = &execute_data.node;

        // Get executor for this node type
        let executor = match self.executors.get(&node.node_type) {
            Some(e) => e,
            None => {
                // Try generic executor based on node type pattern
                if node.is_trigger() {
                    self.executors.get("n8n-nodes-base.manualTrigger").unwrap()
                } else {
                    task_data.execution_status = ExecutionStatus::Error;
                    task_data.error = Some(n8n_workflow::ExecutionError::new(format!(
                        "No executor found for node type: {}",
                        node.node_type
                    )));
                    task_data.finish();
                    return task_data;
                }
            }
        };

        // Execute with retry logic
        let max_tries = if node.retry_on_fail {
            node.max_tries.unwrap_or(3) as usize
        } else {
            1
        };

        let wait_between = node.wait_between_tries.unwrap_or(1000);

        for attempt in 0..max_tries {
            if attempt > 0 {
                debug!(
                    node = %node.name,
                    attempt,
                    "Retrying node execution"
                );
                tokio::time::sleep(tokio::time::Duration::from_millis(wait_between)).await;
            }

            match executor.execute(node, &execute_data.data, context).await {
                Ok(output) => {
                    task_data.data = Some(self.format_output(output));
                    task_data.execution_status = ExecutionStatus::Success;
                    task_data.finish();
                    return task_data;
                }
                Err(e) => {
                    if attempt == max_tries - 1 {
                        task_data.execution_status = ExecutionStatus::Error;
                        task_data.error =
                            Some(n8n_workflow::ExecutionError::new(e.to_string()).with_node(&node.name));
                    }
                }
            }
        }

        task_data.finish();
        task_data
    }

    /// Format node output into TaskDataConnections.
    fn format_output(&self, output: NodeOutput) -> TaskDataConnections {
        let mut result = TaskDataConnections::new();
        result.insert(CONNECTION_MAIN.to_string(), output);
        result
    }

    /// Queue child nodes for execution.
    fn queue_child_nodes(
        &self,
        workflow: &Workflow,
        source_node: &str,
        output_data: &TaskDataConnections,
        run_index: usize,
        stack: &mut VecDeque<ExecuteData>,
    ) -> Result<(), ExecutionEngineError> {
        // Get connections from this node
        if let Some(node_conns) = workflow.connections.get(source_node) {
            for (conn_type, by_index) in node_conns {
                // Get output data for this connection type
                let outputs = output_data.get(conn_type);

                for (output_index, connections) in by_index.iter().enumerate() {
                    // Get data for this output index
                    let data_for_output = outputs
                        .and_then(|o| o.get(output_index))
                        .cloned()
                        .unwrap_or_default();

                    // Skip if no data
                    if data_for_output.is_empty() {
                        continue;
                    }

                    for conn in connections {
                        // Get target node
                        let target_node = workflow.get_node(&conn.node).ok_or_else(|| {
                            ExecutionEngineError::NodeExecution {
                                node: conn.node.clone(),
                                message: "Target node not found".to_string(),
                            }
                        })?;

                        // Skip disabled nodes
                        if target_node.disabled {
                            continue;
                        }

                        // Build input data for target node
                        let mut input = TaskDataConnections::new();
                        input
                            .entry(conn.connection_type.clone())
                            .or_default()
                            .resize(conn.index + 1, Vec::new());
                        input.get_mut(&conn.connection_type).unwrap()[conn.index] =
                            data_for_output.clone();

                        let source = vec![TaskDataConnectionsSource {
                            previous_node: source_node.to_string(),
                            previous_node_output: Some(output_index),
                            previous_node_run: Some(run_index),
                        }];

                        stack.push_back(ExecuteData {
                            node: target_node.clone(),
                            data: input,
                            source: Some(source),
                            metadata: None,
                        });
                    }
                }
            }
        }

        Ok(())
    }

    /// Execute a partial workflow (from start nodes to destination).
    pub async fn execute_partial(
        &self,
        workflow: &Workflow,
        start_nodes: Vec<String>,
        destination_node: Option<String>,
        input_data: Option<Vec<NodeExecutionData>>,
    ) -> Result<Run, ExecutionEngineError> {
        // Validate start nodes exist
        for name in &start_nodes {
            if workflow.get_node(name).is_none() {
                return Err(ExecutionEngineError::NodeExecution {
                    node: name.clone(),
                    message: "Start node not found".to_string(),
                });
            }
        }

        // Execute with the specific start nodes
        let (tx, _rx) = mpsc::channel(100);
        let context = RuntimeContext::new(WorkflowExecuteMode::Manual, self.config.clone());

        let mut run = Run::new(WorkflowExecuteMode::Manual);

        // Initialize stack with specified start nodes
        let mut stack = self.initialize_stack(
            workflow,
            &start_nodes,
            input_data,
        )?;

        // Build allowed nodes set if destination specified
        let allowed_nodes: Option<std::collections::HashSet<String>> =
            destination_node.as_ref().map(|dest| {
                let conns_by_dest = graph::map_connections_by_destination(&workflow.connections);
                let mut allowed: std::collections::HashSet<_> =
                    graph::get_parent_nodes(&conns_by_dest, dest, None, None)
                        .into_iter()
                        .collect();
                allowed.insert(dest.clone());
                for start in &start_nodes {
                    allowed.insert(start.clone());
                }
                allowed
            });

        // Execute
        while let Some(execute_data) = stack.pop_front() {
            let node_name = execute_data.node.name.clone();

            // Skip if not in allowed nodes
            if let Some(ref allowed) = allowed_nodes {
                if !allowed.contains(&node_name) {
                    continue;
                }
            }

            // Stop at destination
            if destination_node.as_ref() == Some(&node_name) {
                // Execute destination node
                let task_data = self.execute_node(&execute_data, &context, &tx).await;
                run.data
                    .result_data
                    .run_data
                    .entry(node_name)
                    .or_default()
                    .push(task_data);
                break;
            }

            let run_index = run
                .data
                .result_data
                .run_data
                .get(&node_name)
                .map(|v| v.len())
                .unwrap_or(0);

            let task_data = self.execute_node(&execute_data, &context, &tx).await;

            run.data
                .result_data
                .run_data
                .entry(node_name.clone())
                .or_default()
                .push(task_data.clone());

            if task_data.execution_status != ExecutionStatus::Error {
                if let Some(output_data) = &task_data.data {
                    self.queue_child_nodes(workflow, &node_name, output_data, run_index, &mut stack)?;
                }
            }
        }

        run.finish(ExecutionStatus::Success);
        Ok(run)
    }
}

impl Default for WorkflowEngine {
    fn default() -> Self {
        Self::new(RuntimeConfig::default())
    }
}
