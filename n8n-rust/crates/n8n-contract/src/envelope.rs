//! Conversion between n8n execution items and unified DataEnvelopes.

use crate::types::{DataEnvelope, EnvelopeMetadata};
use chrono::Utc;
use n8n_workflow::NodeExecutionData;
use serde_json::Value;

/// Convert n8n node output items into a `DataEnvelope`.
///
/// The items are serialized as a JSON array in `data`.
pub fn from_n8n_output(items: &[NodeExecutionData], source_step: &str) -> DataEnvelope {
    let data: Vec<Value> = items
        .iter()
        .filter_map(|item| serde_json::to_value(&item.json).ok())
        .collect();

    DataEnvelope {
        data: Value::Array(data),
        metadata: EnvelopeMetadata {
            source_step: source_step.to_string(),
            confidence: 1.0,
            epoch: Utc::now().timestamp_millis(),
            version: None,
        },
    }
}

/// Convert a crew agent callback response into a `DataEnvelope`.
///
/// The `output` value from the agent is wrapped with source metadata.
pub fn from_crew_callback(output: Value, source_step: &str, confidence: f64) -> DataEnvelope {
    DataEnvelope {
        data: output,
        metadata: EnvelopeMetadata {
            source_step: source_step.to_string(),
            confidence,
            epoch: Utc::now().timestamp_millis(),
            version: None,
        },
    }
}

/// Convert a `DataEnvelope` back into n8n execution items.
///
/// If the envelope data is an array, each element becomes one item.
/// Otherwise the entire payload is wrapped as a single item.
pub fn to_n8n_items(envelope: &DataEnvelope) -> Vec<NodeExecutionData> {
    match &envelope.data {
        Value::Array(arr) => arr
            .iter()
            .map(|v| {
                let json = match v {
                    Value::Object(map) => {
                        map.iter()
                            .map(|(k, v)| {
                                (k.clone(), json_value_to_generic(v))
                            })
                            .collect()
                    }
                    other => {
                        let mut m = std::collections::HashMap::new();
                        m.insert("data".to_string(), json_value_to_generic(other));
                        m
                    }
                };
                NodeExecutionData::new(json)
            })
            .collect(),
        Value::Null => vec![NodeExecutionData::default()],
        other => {
            let mut m = std::collections::HashMap::new();
            m.insert("data".to_string(), json_value_to_generic(other));
            vec![NodeExecutionData::new(m)]
        }
    }
}

/// Pass an envelope through unchanged (identity transform for chaining).
pub fn passthrough(envelope: DataEnvelope) -> DataEnvelope {
    envelope
}

/// Convert serde_json::Value to n8n GenericValue.
fn json_value_to_generic(v: &Value) -> n8n_workflow::GenericValue {
    match v {
        Value::Null => n8n_workflow::GenericValue::Null,
        Value::Bool(b) => n8n_workflow::GenericValue::Bool(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                n8n_workflow::GenericValue::Integer(i)
            } else {
                n8n_workflow::GenericValue::Float(n.as_f64().unwrap_or(0.0))
            }
        }
        Value::String(s) => n8n_workflow::GenericValue::String(s.clone()),
        Value::Array(arr) => {
            n8n_workflow::GenericValue::Array(arr.iter().map(json_value_to_generic).collect())
        }
        Value::Object(map) => {
            let obj = map
                .iter()
                .map(|(k, v)| (k.clone(), json_value_to_generic(v)))
                .collect();
            n8n_workflow::GenericValue::Object(obj)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_n8n_roundtrip() {
        let mut data = std::collections::HashMap::new();
        data.insert(
            "name".to_string(),
            n8n_workflow::GenericValue::String("test".to_string()),
        );
        data.insert(
            "count".to_string(),
            n8n_workflow::GenericValue::Integer(42),
        );
        let items = vec![NodeExecutionData::new(data)];

        let envelope = from_n8n_output(&items, "set-node");
        assert_eq!(envelope.metadata.source_step, "set-node");

        let back = to_n8n_items(&envelope);
        assert_eq!(back.len(), 1);
        assert!(back[0].json.contains_key("name"));
    }

    #[test]
    fn test_from_crew_callback() {
        let output = serde_json::json!({"analysis": "The market is bullish"});
        let envelope = from_crew_callback(output, "researcher", 0.85);
        assert_eq!(envelope.metadata.confidence, 0.85);
    }

    #[test]
    fn test_to_n8n_items_null() {
        let envelope = DataEnvelope::new(Value::Null, "empty");
        let items = to_n8n_items(&envelope);
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn test_passthrough() {
        let env = DataEnvelope::new(serde_json::json!({"x": 1}), "src");
        let passed = passthrough(env.clone());
        assert_eq!(
            serde_json::to_string(&passed.data).unwrap(),
            serde_json::to_string(&env.data).unwrap(),
        );
    }
}
