//! Execution data structures and types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::data::{DataObject, NodeExecutionData, PinData};
use crate::error::ExecutionError;
use crate::node::Node;
use crate::workflow::WorkflowExecuteMode;

/// Execution status.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionStatus {
    #[default]
    New,
    Running,
    Success,
    Error,
    Waiting,
    Canceled,
    Crashed,
}

impl ExecutionStatus {
    pub fn is_finished(&self) -> bool {
        matches!(
            self,
            ExecutionStatus::Success
                | ExecutionStatus::Error
                | ExecutionStatus::Canceled
                | ExecutionStatus::Crashed
        )
    }

    pub fn is_error(&self) -> bool {
        matches!(
            self,
            ExecutionStatus::Error | ExecutionStatus::Crashed | ExecutionStatus::Canceled
        )
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ExecutionStatus::New => "new",
            ExecutionStatus::Running => "running",
            ExecutionStatus::Success => "success",
            ExecutionStatus::Error => "error",
            ExecutionStatus::Waiting => "waiting",
            ExecutionStatus::Canceled => "canceled",
            ExecutionStatus::Crashed => "crashed",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "new" => Some(ExecutionStatus::New),
            "running" => Some(ExecutionStatus::Running),
            "success" => Some(ExecutionStatus::Success),
            "error" => Some(ExecutionStatus::Error),
            "waiting" => Some(ExecutionStatus::Waiting),
            "canceled" => Some(ExecutionStatus::Canceled),
            "crashed" => Some(ExecutionStatus::Crashed),
            _ => None,
        }
    }
}

/// Task data connections - output data organized by connection type and index.
/// [connectionType][outputIndex] = Vec<NodeExecutionData>
pub type TaskDataConnections = HashMap<String, Vec<Vec<NodeExecutionData>>>;

/// Metadata about a task execution.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TaskMetadata {
    /// Sub-execution reference if this triggered another workflow.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_execution: Option<RelatedExecution>,
    /// Custom metadata.
    #[serde(flatten)]
    pub custom: DataObject,
}

/// Reference to a related execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedExecution {
    pub execution_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_id: Option<String>,
}

/// Result of executing a single node.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskData {
    /// Start time as Unix timestamp (milliseconds).
    pub start_time: i64,
    /// Execution duration in milliseconds.
    pub execution_time: i64,
    /// Execution status.
    #[serde(default)]
    pub execution_status: ExecutionStatus,
    /// Output data by connection type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<TaskDataConnections>,
    /// Input data override (for testing/debugging).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_override: Option<TaskDataConnections>,
    /// Error if execution failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ExecutionError>,
    /// Task metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<TaskMetadata>,
}

impl TaskData {
    pub fn new() -> Self {
        Self {
            start_time: chrono::Utc::now().timestamp_millis(),
            execution_time: 0,
            execution_status: ExecutionStatus::Running,
            data: None,
            input_override: None,
            error: None,
            metadata: None,
        }
    }

    pub fn with_output(mut self, connection_type: &str, output: Vec<Vec<NodeExecutionData>>) -> Self {
        self.data
            .get_or_insert_with(HashMap::new)
            .insert(connection_type.to_string(), output);
        self
    }

    pub fn with_error(mut self, error: ExecutionError) -> Self {
        self.error = Some(error);
        self.execution_status = ExecutionStatus::Error;
        self
    }

    pub fn finish(&mut self) {
        let now = chrono::Utc::now().timestamp_millis();
        self.execution_time = now - self.start_time;
        if self.error.is_none() {
            self.execution_status = ExecutionStatus::Success;
        }
    }
}

impl Default for TaskData {
    fn default() -> Self {
        Self::new()
    }
}

/// Run data - results indexed by node name and run index.
pub type RunData = HashMap<String, Vec<TaskData>>;

/// Source tracking for task data connections.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDataConnectionsSource {
    /// Source node name.
    pub previous_node: String,
    /// Index within the source node's output.
    pub previous_node_output: Option<usize>,
    /// Run index of the source node.
    pub previous_node_run: Option<usize>,
}

/// Data to be executed by a node.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteData {
    /// Node to execute.
    pub node: Node,
    /// Input data for the node.
    pub data: TaskDataConnections,
    /// Source of the input data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<Vec<TaskDataConnectionsSource>>,
    /// Task metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<TaskMetadata>,
}

/// Start node configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartNodeData {
    /// Node name to start from.
    pub name: String,
    /// Source information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_data: Option<TaskDataConnectionsSource>,
}

/// Destination node for partial execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DestinationNode {
    pub node_name: String,
    /// Include or exclude the destination node itself.
    pub mode: DestinationMode,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum DestinationMode {
    #[default]
    Inclusive,
    Exclusive,
}

/// Waiting execution state for wait nodes.
pub type WaitingForExecution = HashMap<String, HashMap<usize, TaskDataConnections>>;

/// Source tracking for waiting execution.
pub type WaitingForExecutionSource = HashMap<String, HashMap<usize, Vec<TaskDataConnectionsSource>>>;

/// Runtime execution context.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionContext {
    /// Schema version.
    pub version: u32,
    /// When the context was established.
    pub established_at: i64,
    /// How the execution was triggered.
    pub source: WorkflowExecuteMode,
    /// Parent execution ID for sub-workflows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_execution_id: Option<String>,
    /// Custom context data.
    #[serde(flatten)]
    pub custom: DataObject,
}

impl ExecutionContext {
    pub fn new(source: WorkflowExecuteMode) -> Self {
        Self {
            version: 1,
            established_at: chrono::Utc::now().timestamp_millis(),
            source,
            parent_execution_id: None,
            custom: DataObject::new(),
        }
    }
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self::new(WorkflowExecuteMode::Manual)
    }
}

/// Start data for execution.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StartData {
    /// Nodes to start from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_nodes: Option<Vec<StartNodeData>>,
    /// Destination for partial execution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination_node: Option<DestinationNode>,
    /// Original destination before modifications.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_destination_node: Option<DestinationNode>,
    /// Filter for which nodes can execute.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_node_filter: Option<Vec<String>>,
}

/// Result data from execution.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ResultData {
    /// Global execution error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ExecutionError>,
    /// Results indexed by node name.
    pub run_data: RunData,
    /// Pinned data used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin_data: Option<PinData>,
    /// Last node that executed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_node_executed: Option<String>,
    /// Execution metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
}

/// Internal execution data (execution state).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InternalExecutionData {
    /// Context data for flow and nodes.
    pub context_data: DataObject,
    /// Runtime context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_data: Option<ExecutionContext>,
    /// Stack of nodes to execute.
    pub node_execution_stack: Vec<ExecuteData>,
    /// Metadata by node name and run index.
    pub metadata: HashMap<String, Vec<TaskMetadata>>,
    /// Waiting execution state.
    pub waiting_execution: WaitingForExecution,
    /// Source tracking for waiting execution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waiting_execution_source: Option<WaitingForExecutionSource>,
}

/// Manual execution data.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ManualExecutionData {
    /// Nodes that have been modified.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dirty_node_names: Option<Vec<String>>,
    /// User who triggered the execution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}

/// Complete execution data (V1 schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunExecutionData {
    /// Schema version (always 1).
    pub version: u32,
    /// Start configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_data: Option<StartData>,
    /// Execution results.
    pub result_data: ResultData,
    /// Internal execution state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_data: Option<InternalExecutionData>,
    /// Parent execution reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_execution: Option<RelatedExecution>,
    /// Wait until timestamp.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait_till: Option<chrono::DateTime<chrono::Utc>>,
    /// Manual execution data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manual_data: Option<ManualExecutionData>,
}

impl RunExecutionData {
    pub fn new() -> Self {
        Self {
            version: 1,
            start_data: None,
            result_data: ResultData::default(),
            execution_data: Some(InternalExecutionData::default()),
            parent_execution: None,
            wait_till: None,
            manual_data: None,
        }
    }
}

impl Default for RunExecutionData {
    fn default() -> Self {
        Self::new()
    }
}

/// Complete execution run result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    /// Execution data.
    pub data: RunExecutionData,
    /// Execution mode.
    pub mode: WorkflowExecuteMode,
    /// Start time.
    pub started_at: chrono::DateTime<chrono::Utc>,
    /// Finish time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Overall status.
    pub status: ExecutionStatus,
    /// Wait until timestamp.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait_till: Option<chrono::DateTime<chrono::Utc>>,
}

impl Run {
    pub fn new(mode: WorkflowExecuteMode) -> Self {
        Self {
            data: RunExecutionData::new(),
            mode,
            started_at: chrono::Utc::now(),
            finished_at: None,
            status: ExecutionStatus::Running,
            wait_till: None,
        }
    }

    pub fn finish(&mut self, status: ExecutionStatus) {
        self.finished_at = Some(chrono::Utc::now());
        self.status = status;
    }

    pub fn has_error(&self) -> bool {
        self.status.is_error() || self.data.result_data.error.is_some()
    }
}
