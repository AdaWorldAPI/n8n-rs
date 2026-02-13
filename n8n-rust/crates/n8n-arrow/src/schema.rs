//! Arrow schema definitions for n8n workflow data.

use arrow_schema::{DataType, Field, Schema, TimeUnit};
use std::collections::BTreeMap;

use n8n_workflow::{GenericValue, NodeExecutionData};

/// Schema for node execution data items - dynamic schema inferred from DataObject keys.
///
/// Each key in the DataObject becomes a column with a type determined by the
/// first non-null value seen for that key across all items.
pub fn infer_node_execution_data_schema(items: &[NodeExecutionData]) -> Schema {
    if items.is_empty() {
        return Schema::empty();
    }

    // Collect all keys and infer their types from the superset of all items.
    // Use BTreeMap for deterministic column ordering.
    let mut key_types: BTreeMap<String, DataType> = BTreeMap::new();

    for item in items {
        for (key, value) in &item.json {
            // Only update if we haven't seen a concrete type yet (i.e. the key is
            // absent or was previously Null).
            let current = key_types.get(key);
            let needs_update = match current {
                None => true,
                Some(DataType::Null) => true,
                _ => false,
            };
            if needs_update {
                let dt = generic_value_to_data_type(value);
                key_types.insert(key.clone(), dt);
            }
        }
    }

    // If we ended up with no keys at all, return a single-column schema to
    // avoid empty batches.
    if key_types.is_empty() {
        return Schema::new(vec![Field::new("_empty", DataType::Null, true)]);
    }

    let fields: Vec<Field> = key_types
        .into_iter()
        .map(|(name, dt)| Field::new(name, dt, true))
        .collect();

    Schema::new(fields)
}

/// Map a GenericValue to the corresponding Arrow DataType.
pub fn generic_value_to_data_type(value: &GenericValue) -> DataType {
    match value {
        GenericValue::Null => DataType::Null,
        GenericValue::Bool(_) => DataType::Boolean,
        GenericValue::Integer(_) => DataType::Int64,
        GenericValue::Float(_) => DataType::Float64,
        GenericValue::String(_) => DataType::Utf8,
        // Complex types are JSON-serialized to strings.
        GenericValue::Array(_) => DataType::Utf8,
        GenericValue::Object(_) => DataType::Utf8,
    }
}

/// Schema for task data (node execution results).
pub fn task_data_schema() -> Schema {
    Schema::new(vec![
        Field::new("node_name", DataType::Utf8, false),
        Field::new("run_index", DataType::Int32, false),
        Field::new(
            "start_time",
            DataType::Timestamp(TimeUnit::Millisecond, None),
            false,
        ),
        Field::new("execution_time_ms", DataType::Int64, false),
        Field::new("status", DataType::Utf8, false),
        Field::new("output_items_count", DataType::Int32, false),
        Field::new("error_message", DataType::Utf8, true),
    ])
}

/// Schema for workflow execution summary.
pub fn execution_summary_schema() -> Schema {
    Schema::new(vec![
        Field::new("execution_id", DataType::Utf8, false),
        Field::new("workflow_id", DataType::Utf8, false),
        Field::new("status", DataType::Utf8, false),
        Field::new("mode", DataType::Utf8, false),
        Field::new(
            "started_at",
            DataType::Timestamp(TimeUnit::Millisecond, None),
            false,
        ),
        Field::new(
            "finished_at",
            DataType::Timestamp(TimeUnit::Millisecond, None),
            true,
        ),
        Field::new("total_nodes", DataType::Int32, false),
        Field::new("error_message", DataType::Utf8, true),
        Field::new("last_node_executed", DataType::Utf8, true),
    ])
}

/// Schema for binary data references.
pub fn binary_data_schema() -> Schema {
    Schema::new(vec![
        Field::new("key", DataType::Utf8, false),
        Field::new("mime_type", DataType::Utf8, false),
        Field::new("file_name", DataType::Utf8, true),
        Field::new("file_size_bytes", DataType::UInt64, true),
        Field::new("data_id", DataType::Utf8, true),
        // Inline data for small files (zero-copy when possible)
        Field::new("inline_data", DataType::Binary, true),
    ])
}

/// Schema for Hamming fingerprints.
pub fn hamming_fingerprint_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        // 1250 bytes = 10,000 bits packed
        Field::new("fingerprint", DataType::FixedSizeBinary(1250), false),
        Field::new("metadata_json", DataType::Utf8, true),
    ])
}

/// Schema for similarity search results.
pub fn similarity_result_schema() -> Schema {
    Schema::new(vec![
        Field::new("query_id", DataType::Utf8, false),
        Field::new("match_id", DataType::Utf8, false),
        Field::new("hamming_distance", DataType::UInt32, false),
        Field::new("similarity", DataType::Float64, false),
    ])
}

/// Schema for workflow nodes.
pub fn workflow_node_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("name", DataType::Utf8, false),
        Field::new("type", DataType::Utf8, false),
        Field::new("type_version", DataType::UInt32, false),
        Field::new("position_x", DataType::Float64, false),
        Field::new("position_y", DataType::Float64, false),
        Field::new("disabled", DataType::Boolean, false),
        Field::new("parameters_json", DataType::Utf8, false),
    ])
}

/// Schema for workflow connections.
pub fn workflow_connection_schema() -> Schema {
    Schema::new(vec![
        Field::new("source_node", DataType::Utf8, false),
        Field::new("source_index", DataType::UInt32, false),
        Field::new("target_node", DataType::Utf8, false),
        Field::new("target_index", DataType::UInt32, false),
        Field::new("connection_type", DataType::Utf8, false),
    ])
}

/// Get the schema for a given data type.
pub fn get_schema(data_type: &str) -> Option<Schema> {
    match data_type {
        "task_data" => Some(task_data_schema()),
        "execution_summary" => Some(execution_summary_schema()),
        "binary_data" => Some(binary_data_schema()),
        "hamming_fingerprint" => Some(hamming_fingerprint_schema()),
        "similarity_result" => Some(similarity_result_schema()),
        "workflow_node" => Some(workflow_node_schema()),
        "workflow_connection" => Some(workflow_connection_schema()),
        _ => None,
    }
}
