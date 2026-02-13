//! Workflow definition types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::connection::{WorkflowConnections, CONNECTION_MAIN};
use crate::data::{DataObject, PinData};
use crate::node::Node;

/// Workflow execution mode.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum WorkflowExecuteMode {
    #[default]
    Manual,
    Trigger,
    Webhook,
    Error,
    Wait,
    Scheduled,
    Worker,
    Retry,
    Internal,
}

impl WorkflowExecuteMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            WorkflowExecuteMode::Manual => "manual",
            WorkflowExecuteMode::Trigger => "trigger",
            WorkflowExecuteMode::Webhook => "webhook",
            WorkflowExecuteMode::Error => "error",
            WorkflowExecuteMode::Wait => "wait",
            WorkflowExecuteMode::Scheduled => "scheduled",
            WorkflowExecuteMode::Worker => "worker",
            WorkflowExecuteMode::Retry => "retry",
            WorkflowExecuteMode::Internal => "internal",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "manual" => Some(WorkflowExecuteMode::Manual),
            "trigger" => Some(WorkflowExecuteMode::Trigger),
            "webhook" => Some(WorkflowExecuteMode::Webhook),
            "error" => Some(WorkflowExecuteMode::Error),
            "wait" => Some(WorkflowExecuteMode::Wait),
            "scheduled" => Some(WorkflowExecuteMode::Scheduled),
            "worker" => Some(WorkflowExecuteMode::Worker),
            "retry" => Some(WorkflowExecuteMode::Retry),
            "internal" => Some(WorkflowExecuteMode::Internal),
            _ => None,
        }
    }
}

/// Workflow settings.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowSettings {
    /// Timezone for date/time operations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,

    /// ID of error handling workflow.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_workflow: Option<String>,

    /// Save data on error execution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save_data_error_execution: Option<SaveDataOption>,

    /// Save data on success execution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save_data_success_execution: Option<SaveDataOption>,

    /// Execution timeout in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_timeout: Option<u64>,

    /// Execution order version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_order: Option<ExecutionOrder>,

    /// Binary data storage mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary_mode: Option<BinaryMode>,

    /// Save manual executions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save_manual_executions: Option<bool>,

    /// Save execution progress.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save_execution_progress: Option<bool>,
}

/// Save data options.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SaveDataOption {
    All,
    None,
}

/// Execution order algorithm version.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExecutionOrder {
    #[serde(rename = "v0")]
    V0,
    #[serde(rename = "v1")]
    V1,
}

/// Binary data storage mode.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BinaryMode {
    Default,
    Stored,
    S3,
}

/// A workflow definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workflow {
    /// Unique workflow identifier.
    pub id: String,

    /// Workflow name.
    pub name: String,

    /// Optional description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Whether the workflow is active.
    #[serde(default)]
    pub active: bool,

    /// Workflow nodes.
    pub nodes: Vec<Node>,

    /// Node connections (DAG).
    pub connections: WorkflowConnections,

    /// Workflow settings.
    #[serde(default)]
    pub settings: WorkflowSettings,

    /// Persistent workflow data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub static_data: Option<DataObject>,

    /// Pinned node data for testing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin_data: Option<PinData>,

    /// Version identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_id: Option<String>,

    /// Creation timestamp.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,

    /// Last update timestamp.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl Workflow {
    /// Create a new empty workflow.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            description: None,
            active: false,
            nodes: Vec::new(),
            connections: WorkflowConnections::new(),
            settings: WorkflowSettings::default(),
            static_data: None,
            pin_data: None,
            version_id: None,
            created_at: Some(chrono::Utc::now()),
            updated_at: Some(chrono::Utc::now()),
        }
    }

    /// Add a node to the workflow.
    pub fn add_node(&mut self, node: Node) {
        self.nodes.push(node);
        self.updated_at = Some(chrono::Utc::now());
    }

    /// Get a node by name.
    pub fn get_node(&self, name: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.name == name)
    }

    /// Get a mutable node by name.
    pub fn get_node_mut(&mut self, name: &str) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|n| n.name == name)
    }

    /// Get a node by ID.
    pub fn get_node_by_id(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Connect two nodes.
    pub fn connect(
        &mut self,
        source: &str,
        target: &str,
        source_index: usize,
        target_index: usize,
    ) -> Result<(), crate::WorkflowError> {
        // Verify both nodes exist
        if self.get_node(source).is_none() {
            return Err(crate::WorkflowError::NodeNotFound(source.to_string()));
        }
        if self.get_node(target).is_none() {
            return Err(crate::WorkflowError::NodeNotFound(target.to_string()));
        }

        let conn = crate::Connection::new(target, CONNECTION_MAIN, target_index);

        self.connections
            .entry(source.to_string())
            .or_default()
            .entry(CONNECTION_MAIN.to_string())
            .or_default();

        let by_index = self
            .connections
            .get_mut(source)
            .unwrap()
            .get_mut(CONNECTION_MAIN)
            .unwrap();

        // Extend the vector if needed
        while by_index.len() <= source_index {
            by_index.push(Vec::new());
        }

        by_index[source_index].push(conn);
        self.updated_at = Some(chrono::Utc::now());

        Ok(())
    }

    /// Find all trigger nodes (entry points).
    pub fn get_trigger_nodes(&self) -> Vec<&Node> {
        self.nodes.iter().filter(|n| n.is_trigger()).collect()
    }

    /// Find start nodes (nodes with no incoming connections).
    pub fn get_start_nodes(&self) -> Vec<&Node> {
        let conns_by_dest = crate::connection::graph::map_connections_by_destination(&self.connections);

        self.nodes
            .iter()
            .filter(|n| !conns_by_dest.contains_key(&n.name))
            .collect()
    }

    /// Get all node names.
    pub fn node_names(&self) -> Vec<String> {
        self.nodes.iter().map(|n| n.name.clone()).collect()
    }

    /// Validate the workflow structure.
    pub fn validate(&self) -> Result<(), crate::WorkflowError> {
        // Check for empty workflow
        if self.nodes.is_empty() {
            return Err(crate::WorkflowError::InvalidWorkflow(
                "Workflow has no nodes".to_string(),
            ));
        }

        // Check for unique node names
        let mut names = std::collections::HashSet::new();
        for node in &self.nodes {
            if !names.insert(&node.name) {
                return Err(crate::WorkflowError::InvalidWorkflow(format!(
                    "Duplicate node name: {}",
                    node.name
                )));
            }
        }

        // Verify all connections reference existing nodes
        for (source, node_conns) in &self.connections {
            if self.get_node(source).is_none() {
                return Err(crate::WorkflowError::NodeNotFound(source.clone()));
            }

            for by_index in node_conns.values() {
                for connections_at_index in by_index {
                    for conn in connections_at_index {
                        if self.get_node(&conn.node).is_none() {
                            return Err(crate::WorkflowError::NodeNotFound(conn.node.clone()));
                        }
                    }
                }
            }
        }

        // Check for cycles (topological sort will fail if there are cycles)
        let names: Vec<_> = self.nodes.iter().map(|n| n.name.clone()).collect();
        crate::connection::graph::topological_sort(&names, &self.connections)?;

        Ok(())
    }
}

impl Default for Workflow {
    fn default() -> Self {
        Self::new("New Workflow")
    }
}

/// Builder for creating workflows.
pub struct WorkflowBuilder {
    workflow: Workflow,
}

impl WorkflowBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            workflow: Workflow::new(name),
        }
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.workflow.description = Some(desc.into());
        self
    }

    pub fn active(mut self, active: bool) -> Self {
        self.workflow.active = active;
        self
    }

    pub fn node(mut self, node: Node) -> Self {
        self.workflow.add_node(node);
        self
    }

    pub fn connect(
        mut self,
        source: &str,
        target: &str,
        source_index: usize,
        target_index: usize,
    ) -> Result<Self, crate::WorkflowError> {
        self.workflow
            .connect(source, target, source_index, target_index)?;
        Ok(self)
    }

    pub fn settings(mut self, settings: WorkflowSettings) -> Self {
        self.workflow.settings = settings;
        self
    }

    pub fn build(self) -> Result<Workflow, crate::WorkflowError> {
        self.workflow.validate()?;
        Ok(self.workflow)
    }

    pub fn build_unchecked(self) -> Workflow {
        self.workflow
    }
}
