//! Node types and definitions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::data::{NodeParameterValue, NodeParameters};

/// Error handling behavior for nodes.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum OnError {
    #[default]
    StopWorkflow,
    ContinueRegularOutput,
    ContinueErrorOutput,
}

/// A workflow node instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    /// Unique identifier for this node instance.
    pub id: String,
    /// Display name (unique within workflow).
    pub name: String,
    /// Node type identifier (e.g., "n8n-nodes-base.httpRequest").
    #[serde(rename = "type")]
    pub node_type: String,
    /// Version of the node type.
    pub type_version: u32,
    /// Position in the workflow canvas [x, y].
    pub position: [f64; 2],
    /// Whether the node is disabled.
    #[serde(default)]
    pub disabled: bool,
    /// Node configuration parameters.
    #[serde(default)]
    pub parameters: NodeParameters,
    /// Credential references keyed by credential type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credentials: Option<HashMap<String, NodeCredentialRef>>,
    /// Continue execution even if this node fails.
    #[serde(default)]
    pub continue_on_fail: bool,
    /// Retry execution on failure.
    #[serde(default)]
    pub retry_on_fail: bool,
    /// Maximum retry attempts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tries: Option<u32>,
    /// Delay between retries in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait_between_tries: Option<u64>,
    /// Always output data even if empty.
    #[serde(default)]
    pub always_output_data: bool,
    /// Execute only once regardless of input items.
    #[serde(default)]
    pub execute_once: bool,
    /// Error handling behavior.
    #[serde(default)]
    pub on_error: OnError,
    /// Notes/comments for this node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// Webhook ID if this is a webhook node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhook_id: Option<String>,
}

impl Node {
    /// Create a new node with default settings.
    pub fn new(name: impl Into<String>, node_type: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            node_type: node_type.into(),
            type_version: 1,
            position: [0.0, 0.0],
            disabled: false,
            parameters: NodeParameters::new(),
            credentials: None,
            continue_on_fail: false,
            retry_on_fail: false,
            max_tries: None,
            wait_between_tries: None,
            always_output_data: false,
            execute_once: false,
            on_error: OnError::default(),
            notes: None,
            webhook_id: None,
        }
    }

    /// Set a parameter value.
    pub fn set_parameter(&mut self, key: impl Into<String>, value: NodeParameterValue) {
        self.parameters.insert(key.into(), value);
    }

    /// Get a parameter value.
    pub fn get_parameter(&self, key: &str) -> Option<&NodeParameterValue> {
        self.parameters.get(key)
    }

    /// Check if this node is a trigger node.
    pub fn is_trigger(&self) -> bool {
        self.node_type.ends_with("Trigger")
            || self.node_type.contains(".trigger")
            || self.node_type == "n8n-nodes-base.manualTrigger"
    }
}

/// Reference to a credential.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeCredentialRef {
    /// Credential ID.
    pub id: String,
    /// Credential name.
    pub name: String,
}

/// Node type connection configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeConnectionConfig {
    /// Connection type name.
    #[serde(rename = "type")]
    pub connection_type: String,
    /// Display label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Whether this connection is required.
    #[serde(default)]
    pub required: bool,
    /// Maximum number of connections.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_connections: Option<usize>,
}

/// Node property definition for configuration UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeProperty {
    /// Property name/key.
    pub name: String,
    /// Display name.
    pub display_name: String,
    /// Property type.
    #[serde(rename = "type")]
    pub property_type: NodePropertyType,
    /// Default value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<NodeParameterValue>,
    /// Description/help text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether the property is required.
    #[serde(default)]
    pub required: bool,
    /// Options for select/multiOptions types.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<NodePropertyOption>>,
    /// Placeholder text for inputs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
}

/// Node property types.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum NodePropertyType {
    String,
    Number,
    Boolean,
    Options,
    MultiOptions,
    Collection,
    FixedCollection,
    Json,
    Color,
    DateTime,
    ResourceLocator,
    ResourceMapper,
    Filter,
    AssignmentCollection,
    Credentials,
    Notice,
    Button,
}

/// Option for select properties.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePropertyOption {
    /// Option display name.
    pub name: String,
    /// Option value.
    pub value: NodeParameterValue,
    /// Description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Node type description - declarative metadata about node capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeTypeDescription {
    /// Unique node type identifier.
    pub name: String,
    /// Display name.
    pub display_name: String,
    /// Node group/category.
    pub group: Vec<String>,
    /// Description text.
    pub description: String,
    /// Node version(s).
    pub version: NodeVersion,
    /// Icon identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Input connections.
    pub inputs: Vec<NodeConnectionConfig>,
    /// Output connections.
    pub outputs: Vec<NodeConnectionConfig>,
    /// Default input/output names.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_input_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_output_name: Option<String>,
    /// Node properties.
    pub properties: Vec<NodeProperty>,
    /// Credential requirements.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credentials: Option<Vec<NodeCredentialDescription>>,
    /// Whether this is a trigger node.
    #[serde(default)]
    pub trigger: bool,
    /// Whether this is a polling node.
    #[serde(default)]
    pub polling: bool,
}

/// Node version can be single or multiple.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum NodeVersion {
    Single(u32),
    Multiple(Vec<u32>),
}

impl NodeVersion {
    pub fn latest(&self) -> u32 {
        match self {
            NodeVersion::Single(v) => *v,
            NodeVersion::Multiple(vs) => *vs.iter().max().unwrap_or(&1),
        }
    }
}

/// Credential description for node type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeCredentialDescription {
    /// Credential type name.
    pub name: String,
    /// Whether required.
    #[serde(default)]
    pub required: bool,
    /// Display conditions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_options: Option<serde_json::Value>,
}
