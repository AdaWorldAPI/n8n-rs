//! Envelope codec — n8n-specific conversions for DataEnvelope
//!
//! Converts between n8n item arrays, crewAI callback responses,
//! and the unified DataEnvelope format.

use serde_json::{json, Value};

use super::types::{DataEnvelope, EnvelopeMetadata};

impl DataEnvelope {
    /// Create an envelope from an n8n node output (JSON items array).
    ///
    /// n8n nodes produce `[{"json": {...}}, ...]` arrays.
    /// This wraps them into a DataEnvelope for cross-runtime transport.
    pub fn from_n8n_output(node_id: &str, items: &Value) -> Self {
        Self {
            step_id: node_id.to_string(),
            output_key: format!("{}.output", node_id),
            content_type: "application/json".to_string(),
            content: items.clone(),
            metadata: EnvelopeMetadata {
                agent_id: None,
                confidence: None,
                epoch: None,
                version: None,
            },
        }
    }

    /// Create an envelope from a crewAI callback response.
    ///
    /// crewAI returns `{"step_id": "...", "agent_id": "...", "result": ..., "confidence": ...}`
    /// when an agent task completes.
    pub fn from_crew_callback(response: &Value) -> Self {
        let step_id = response
            .get("step_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let agent_id = response
            .get("agent_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let confidence = response.get("confidence").and_then(|v| v.as_f64());

        let result = response.get("result").cloned().unwrap_or(json!(null));

        Self {
            step_id: step_id.clone(),
            output_key: format!("{}.result", step_id),
            content_type: "application/json".to_string(),
            content: result,
            metadata: EnvelopeMetadata {
                agent_id,
                confidence,
                epoch: None,
                version: None,
            },
        }
    }

    /// Convert this envelope's content to n8n items format for the next node.
    ///
    /// n8n expects `[{"json": {...}}, ...]` arrays. If the content is already
    /// an array of objects, wrap each in `{"json": ...}`. If it's a single
    /// object, wrap it as `[{"json": content}]`.
    pub fn to_n8n_items(&self) -> Value {
        match &self.content {
            Value::Array(arr) => {
                let items: Vec<Value> = arr
                    .iter()
                    .map(|item| {
                        // If already in {"json": ...} format, pass through
                        if item.is_object() && item.get("json").is_some() {
                            item.clone()
                        } else {
                            json!({"json": item})
                        }
                    })
                    .collect();
                Value::Array(items)
            }
            other => {
                json!([{"json": other}])
            }
        }
    }

    /// Create an empty pass-through envelope (used when a step is skipped).
    pub fn passthrough(step_id: &str, input: &DataEnvelope) -> Self {
        Self {
            step_id: step_id.to_string(),
            output_key: format!("{}.passthrough", step_id),
            content_type: input.content_type.clone(),
            content: input.content.clone(),
            metadata: EnvelopeMetadata {
                agent_id: None,
                confidence: None,
                epoch: None,
                version: None,
            },
        }
    }
}
