//! Data types for workflow execution data.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Generic value type that can hold any JSON-compatible value.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum GenericValue {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Array(Vec<GenericValue>),
    Object(DataObject),
}

impl Default for GenericValue {
    fn default() -> Self {
        GenericValue::Null
    }
}

impl From<bool> for GenericValue {
    fn from(v: bool) -> Self {
        GenericValue::Bool(v)
    }
}

impl From<i64> for GenericValue {
    fn from(v: i64) -> Self {
        GenericValue::Integer(v)
    }
}

impl From<f64> for GenericValue {
    fn from(v: f64) -> Self {
        GenericValue::Float(v)
    }
}

impl From<String> for GenericValue {
    fn from(v: String) -> Self {
        GenericValue::String(v)
    }
}

impl From<&str> for GenericValue {
    fn from(v: &str) -> Self {
        GenericValue::String(v.to_string())
    }
}

impl<T: Into<GenericValue>> From<Vec<T>> for GenericValue {
    fn from(v: Vec<T>) -> Self {
        GenericValue::Array(v.into_iter().map(Into::into).collect())
    }
}

impl From<DataObject> for GenericValue {
    fn from(v: DataObject) -> Self {
        GenericValue::Object(v)
    }
}

/// A map of string keys to generic values (equivalent to IDataObject in TS).
pub type DataObject = HashMap<String, GenericValue>;

/// Binary data descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryData {
    /// Base64 encoded data or file reference ID.
    pub data: String,
    /// MIME type of the binary data.
    pub mime_type: String,
    /// Original file name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    /// File extension.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_extension: Option<String>,
    /// Human-readable file size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_size: Option<String>,
    /// Actual size in bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    /// Reference ID for stored binary data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// File type category.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_type: Option<BinaryFileType>,
}

/// Categories of binary file types.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BinaryFileType {
    Text,
    Json,
    Image,
    Audio,
    Video,
    Pdf,
    Html,
    Other,
}

/// Paired item data for tracking data lineage through node executions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairedItemData {
    /// Index of the item in the source node's output.
    pub item: usize,
    /// Index of the input connection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<usize>,
    /// Name of the source node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_overwrite: Option<String>,
}

/// Individual execution data item flowing through nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeExecutionData {
    /// Primary JSON data payload.
    pub json: DataObject,
    /// Binary data attachments keyed by name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary: Option<HashMap<String, BinaryData>>,
    /// Error from node execution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<crate::ExecutionError>,
    /// Data lineage tracking.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paired_item: Option<Vec<PairedItemData>>,
    /// Evaluation metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evaluation_data: Option<DataObject>,
}

impl NodeExecutionData {
    /// Create new execution data with just JSON.
    pub fn new(json: DataObject) -> Self {
        Self {
            json,
            binary: None,
            error: None,
            paired_item: None,
            evaluation_data: None,
        }
    }

    /// Create from a raw JSON value.
    pub fn from_json_value(value: serde_json::Value) -> Result<Self, serde_json::Error> {
        let json: DataObject = serde_json::from_value(value)?;
        Ok(Self::new(json))
    }

    /// Add binary data.
    pub fn with_binary(mut self, key: impl Into<String>, data: BinaryData) -> Self {
        self.binary
            .get_or_insert_with(HashMap::new)
            .insert(key.into(), data);
        self
    }

    /// Add paired item data for lineage tracking.
    pub fn with_paired_item(mut self, item: usize, input: Option<usize>) -> Self {
        self.paired_item
            .get_or_insert_with(Vec::new)
            .push(PairedItemData {
                item,
                input,
                source_overwrite: None,
            });
        self
    }
}

impl Default for NodeExecutionData {
    fn default() -> Self {
        Self::new(DataObject::new())
    }
}

/// Node parameter value types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum NodeParameterValue {
    String(String),
    Number(f64),
    Boolean(bool),
    Array(Vec<NodeParameterValue>),
    Object(HashMap<String, NodeParameterValue>),
    Expression(String), // Expressions start with "="
}

impl Default for NodeParameterValue {
    fn default() -> Self {
        NodeParameterValue::String(String::new())
    }
}

/// Node parameters map.
pub type NodeParameters = HashMap<String, NodeParameterValue>;

/// Pinned data for workflow testing.
pub type PinData = HashMap<String, Vec<NodeExecutionData>>;
