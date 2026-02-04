//! JSON compatibility layer for gRPC services.
//!
//! This module provides JSON serialization as a fallback for clients
//! that don't support Arrow/Flight streaming.

use bytes::Bytes;
use n8n_workflow::{
    ExecutionStatus, Node, NodeExecutionData, Run, RunData, TaskData, Workflow,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Format for data transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DataFormat {
    #[default]
    Json,
    Arrow,
    ArrowFlight,
}

impl DataFormat {
    /// Parse from content-type header.
    pub fn from_content_type(content_type: &str) -> Self {
        match content_type {
            "application/vnd.apache.arrow.stream" => DataFormat::Arrow,
            "application/vnd.apache.arrow.flight" => DataFormat::ArrowFlight,
            _ => DataFormat::Json,
        }
    }

    /// Get the content-type header value.
    pub fn content_type(&self) -> &'static str {
        match self {
            DataFormat::Json => "application/json",
            DataFormat::Arrow => "application/vnd.apache.arrow.stream",
            DataFormat::ArrowFlight => "application/vnd.apache.arrow.flight",
        }
    }
}

/// JSON-serializable workflow response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowJson {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub active: bool,
    pub nodes: Vec<NodeJson>,
    pub connections: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

impl From<&Workflow> for WorkflowJson {
    fn from(w: &Workflow) -> Self {
        Self {
            id: w.id.clone(),
            name: w.name.clone(),
            description: w.description.clone(),
            active: w.active,
            nodes: w.nodes.iter().map(NodeJson::from).collect(),
            connections: serde_json::to_value(&w.connections).unwrap_or_default(),
            settings: serde_json::to_value(&w.settings).ok(),
            created_at: w.created_at.map(|t| t.to_rfc3339()),
            updated_at: w.updated_at.map(|t| t.to_rfc3339()),
        }
    }
}

/// JSON-serializable node.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeJson {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub type_version: u32,
    pub position: [f64; 2],
    pub disabled: bool,
    pub parameters: serde_json::Value,
}

impl From<&Node> for NodeJson {
    fn from(n: &Node) -> Self {
        Self {
            id: n.id.clone(),
            name: n.name.clone(),
            node_type: n.node_type.clone(),
            type_version: n.type_version,
            position: n.position,
            disabled: n.disabled,
            parameters: serde_json::to_value(&n.parameters).unwrap_or_default(),
        }
    }
}

/// JSON-serializable execution response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionJson {
    pub execution_id: String,
    pub workflow_id: String,
    pub status: String,
    pub mode: String,
    pub started_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    pub run_data: HashMap<String, Vec<TaskDataJson>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorJson>,
}

impl ExecutionJson {
    pub fn from_run(execution_id: &str, workflow_id: &str, run: &Run) -> Self {
        Self {
            execution_id: execution_id.to_string(),
            workflow_id: workflow_id.to_string(),
            status: format!("{:?}", run.status).to_lowercase(),
            mode: format!("{:?}", run.mode).to_lowercase(),
            started_at: run.started_at.to_rfc3339(),
            finished_at: run.finished_at.map(|t| t.to_rfc3339()),
            run_data: run
                .data
                .result_data
                .run_data
                .iter()
                .map(|(k, v)| (k.clone(), v.iter().map(TaskDataJson::from).collect()))
                .collect(),
            error: run.data.result_data.error.as_ref().map(ErrorJson::from),
        }
    }
}

/// JSON-serializable task data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskDataJson {
    pub start_time: i64,
    pub execution_time: i64,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<HashMap<String, Vec<Vec<NodeExecutionDataJson>>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorJson>,
}

impl From<&TaskData> for TaskDataJson {
    fn from(t: &TaskData) -> Self {
        Self {
            start_time: t.start_time,
            execution_time: t.execution_time,
            status: format!("{:?}", t.execution_status).to_lowercase(),
            data: t.data.as_ref().map(|d| {
                d.iter()
                    .map(|(k, v)| {
                        (
                            k.clone(),
                            v.iter()
                                .map(|items| items.iter().map(NodeExecutionDataJson::from).collect())
                                .collect(),
                        )
                    })
                    .collect()
            }),
            error: t.error.as_ref().map(ErrorJson::from),
        }
    }
}

/// JSON-serializable node execution data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeExecutionDataJson {
    pub json: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary: Option<HashMap<String, BinaryDataJson>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorJson>,
}

impl From<&NodeExecutionData> for NodeExecutionDataJson {
    fn from(d: &NodeExecutionData) -> Self {
        Self {
            json: serde_json::to_value(&d.json).unwrap_or_default(),
            binary: d.binary.as_ref().map(|b| {
                b.iter()
                    .map(|(k, v)| (k.clone(), BinaryDataJson::from(v)))
                    .collect()
            }),
            error: d.error.as_ref().map(ErrorJson::from),
        }
    }
}

/// JSON-serializable binary data reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BinaryDataJson {
    pub mime_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

impl From<&n8n_workflow::BinaryData> for BinaryDataJson {
    fn from(b: &n8n_workflow::BinaryData) -> Self {
        Self {
            mime_type: b.mime_type.clone(),
            file_name: b.file_name.clone(),
            file_size: b.file_size.clone(),
            id: b.id.clone(),
        }
    }
}

/// JSON-serializable error.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorJson {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl From<&n8n_workflow::ExecutionError> for ErrorJson {
    fn from(e: &n8n_workflow::ExecutionError) -> Self {
        Self {
            message: e.message.clone(),
            node_name: e.context.node_name.clone(),
            description: e.context.description.clone(),
        }
    }
}

/// Serialize data to the requested format.
pub fn serialize_response<T: Serialize>(data: &T, format: DataFormat) -> Result<Bytes, String> {
    match format {
        DataFormat::Json => {
            serde_json::to_vec(data)
                .map(Bytes::from)
                .map_err(|e| e.to_string())
        }
        DataFormat::Arrow | DataFormat::ArrowFlight => {
            // For Arrow formats, caller should use dedicated Arrow functions
            Err("Use Arrow-specific functions for Arrow format".to_string())
        }
    }
}

/// Deserialize data from the given format.
pub fn deserialize_request<T: for<'de> Deserialize<'de>>(
    data: &[u8],
    format: DataFormat,
) -> Result<T, String> {
    match format {
        DataFormat::Json => {
            serde_json::from_slice(data).map_err(|e| e.to_string())
        }
        DataFormat::Arrow | DataFormat::ArrowFlight => {
            Err("Use Arrow-specific functions for Arrow format".to_string())
        }
    }
}

/// Content negotiation helper.
pub struct ContentNegotiation;

impl ContentNegotiation {
    /// Determine the best format based on Accept header.
    pub fn select_format(accept_header: Option<&str>) -> DataFormat {
        let accept = accept_header.unwrap_or("application/json");

        if accept.contains("application/vnd.apache.arrow.flight") {
            DataFormat::ArrowFlight
        } else if accept.contains("application/vnd.apache.arrow.stream") {
            DataFormat::Arrow
        } else {
            DataFormat::Json
        }
    }

    /// Check if the client prefers Arrow format.
    pub fn prefers_arrow(accept_header: Option<&str>) -> bool {
        let format = Self::select_format(accept_header);
        matches!(format, DataFormat::Arrow | DataFormat::ArrowFlight)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_from_content_type() {
        assert_eq!(DataFormat::from_content_type("application/json"), DataFormat::Json);
        assert_eq!(
            DataFormat::from_content_type("application/vnd.apache.arrow.stream"),
            DataFormat::Arrow
        );
    }

    #[test]
    fn test_content_negotiation() {
        assert_eq!(
            ContentNegotiation::select_format(None),
            DataFormat::Json
        );
        assert_eq!(
            ContentNegotiation::select_format(Some("application/vnd.apache.arrow.stream")),
            DataFormat::Arrow
        );
    }
}
