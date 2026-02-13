//! Workflow entity - matches n8n's WorkflowEntity.
//!
//! Reference: packages/@n8n/db/src/entities/workflow-entity.ts

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use n8n_workflow::{Connection, Node, WorkflowSettings};

use super::generate_nano_id;

/// WorkflowEntity - main workflow storage.
///
/// Matches the TypeORM entity exactly, including column names and types.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct WorkflowEntity {
    /// Primary key - nano ID (21 chars).
    pub id: String,

    /// Workflow name - must be unique, 1-128 characters.
    pub name: String,

    /// Optional description.
    #[sqlx(default)]
    pub description: Option<String>,

    /// Whether workflow is active (deprecated - use active_version_id).
    pub active: bool,

    /// Soft-delete flag.
    pub is_archived: bool,

    /// Workflow nodes as JSON.
    #[sqlx(json)]
    pub nodes: Vec<Node>,

    /// Node connections as JSON.
    #[sqlx(json)]
    pub connections: serde_json::Value,

    /// Workflow settings as JSON.
    #[sqlx(json)]
    pub settings: Option<WorkflowSettings>,

    /// Workflow-wide persistent data.
    #[sqlx(json)]
    pub static_data: Option<serde_json::Value>,

    /// Frontend metadata.
    #[sqlx(json)]
    pub meta: Option<WorkflowMeta>,

    /// Pinned node data for testing.
    #[sqlx(json)]
    pub pin_data: Option<serde_json::Value>,

    /// Current version ID.
    pub version_id: String,

    /// Active version FK to workflow_history.
    #[sqlx(default)]
    pub active_version_id: Option<String>,

    /// Version counter for optimistic locking.
    pub version_counter: i32,

    /// Count of trigger nodes (excludes error/disabled).
    pub trigger_count: i32,

    /// Parent folder FK.
    #[sqlx(default)]
    pub parent_folder_id: Option<String>,

    /// Creation timestamp.
    pub created_at: DateTime<Utc>,

    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

impl WorkflowEntity {
    /// Create a new workflow with defaults.
    pub fn new(name: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: generate_nano_id(),
            name: name.into(),
            description: None,
            active: false,
            is_archived: false,
            nodes: Vec::new(),
            connections: serde_json::json!({}),
            settings: None,
            static_data: None,
            meta: None,
            pin_data: None,
            version_id: super::generate_version_id(),
            active_version_id: None,
            version_counter: 1,
            trigger_count: 0,
            parent_folder_id: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Count trigger nodes in the workflow.
    pub fn count_triggers(&self) -> i32 {
        self.nodes
            .iter()
            .filter(|n| {
                !n.disabled
                    && n.node_type.contains("Trigger")
                    && !n.node_type.contains("errorTrigger")
            })
            .count() as i32
    }
}

/// Frontend metadata for workflows.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_creation_version: Option<String>,
}

/// WorkflowHistory - version tracking.
///
/// Reference: packages/@n8n/db/src/entities/workflow-history.ts
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct WorkflowHistory {
    /// Version ID (36 chars).
    pub version_id: String,

    /// Parent workflow ID.
    pub workflow_id: String,

    /// Nodes at this version.
    #[sqlx(json)]
    pub nodes: Vec<Node>,

    /// Connections at this version.
    #[sqlx(json)]
    pub connections: serde_json::Value,

    /// Comma-separated author IDs.
    #[sqlx(default)]
    pub authors: Option<String>,

    /// Workflow name at this version.
    #[sqlx(default)]
    pub name: Option<String>,

    /// Description at this version.
    #[sqlx(default)]
    pub description: Option<String>,

    /// Whether this was auto-saved.
    pub autosaved: bool,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl WorkflowHistory {
    pub fn from_workflow(workflow: &WorkflowEntity, authors: &[&str]) -> Self {
        let now = Utc::now();
        Self {
            version_id: super::generate_version_id(),
            workflow_id: workflow.id.clone(),
            nodes: workflow.nodes.clone(),
            connections: workflow.connections.clone(),
            authors: if authors.is_empty() {
                None
            } else {
                Some(authors.join(","))
            },
            name: Some(workflow.name.clone()),
            description: workflow.description.clone(),
            autosaved: false,
            created_at: now,
            updated_at: now,
        }
    }
}

/// SharedWorkflow - workflow access control.
///
/// Reference: packages/@n8n/db/src/entities/shared-workflow.ts
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SharedWorkflow {
    pub workflow_id: String,
    pub project_id: String,
    pub role: WorkflowSharingRole,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Workflow sharing roles.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "varchar", rename_all = "snake_case")]
#[serde(rename_all = "camelCase")]
pub enum WorkflowSharingRole {
    #[sqlx(rename = "workflow:owner")]
    #[serde(rename = "workflow:owner")]
    Owner,
    #[sqlx(rename = "workflow:editor")]
    #[serde(rename = "workflow:editor")]
    Editor,
    #[sqlx(rename = "workflow:viewer")]
    #[serde(rename = "workflow:viewer")]
    Viewer,
}

impl Default for WorkflowSharingRole {
    fn default() -> Self {
        Self::Viewer
    }
}

/// WorkflowTagMapping - junction table for workflow-tag relationships.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct WorkflowTagMapping {
    pub workflow_id: String,
    pub tag_id: String,
}

/// Insert parameters for creating a workflow.
#[derive(Debug, Clone)]
pub struct InsertWorkflow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub nodes: serde_json::Value,
    pub connections: serde_json::Value,
    pub settings: Option<serde_json::Value>,
    pub static_data: Option<serde_json::Value>,
    pub meta: Option<serde_json::Value>,
    pub pin_data: Option<serde_json::Value>,
    pub version_id: String,
    pub parent_folder_id: Option<String>,
}

impl From<&WorkflowEntity> for InsertWorkflow {
    fn from(w: &WorkflowEntity) -> Self {
        Self {
            id: w.id.clone(),
            name: w.name.clone(),
            description: w.description.clone(),
            nodes: serde_json::to_value(&w.nodes).unwrap_or_default(),
            connections: w.connections.clone(),
            settings: w.settings.as_ref().and_then(|s| serde_json::to_value(s).ok()),
            static_data: w.static_data.clone(),
            meta: w.meta.as_ref().and_then(|m| serde_json::to_value(m).ok()),
            pin_data: w.pin_data.clone(),
            version_id: w.version_id.clone(),
            parent_folder_id: w.parent_folder_id.clone(),
        }
    }
}

/// Update parameters for workflow.
#[derive(Debug, Clone, Default)]
pub struct UpdateWorkflow {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub active: Option<bool>,
    pub is_archived: Option<bool>,
    pub nodes: Option<serde_json::Value>,
    pub connections: Option<serde_json::Value>,
    pub settings: Option<Option<serde_json::Value>>,
    pub static_data: Option<Option<serde_json::Value>>,
    pub meta: Option<Option<serde_json::Value>>,
    pub pin_data: Option<Option<serde_json::Value>>,
    pub version_id: Option<String>,
    pub active_version_id: Option<Option<String>>,
    pub parent_folder_id: Option<Option<String>>,
}
