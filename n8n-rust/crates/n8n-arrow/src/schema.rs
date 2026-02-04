//! Arrow schema definitions for n8n workflow data.

use arrow_schema::{DataType, Field, Schema, TimeUnit};
use std::sync::Arc;

/// Schema for node execution data items.
pub fn node_execution_data_schema() -> Schema {
    Schema::new(vec![
        Field::new("json", DataType::Utf8, false),
        Field::new("binary_keys", DataType::List(Arc::new(Field::new("item", DataType::Utf8, true))), true),
        Field::new("has_error", DataType::Boolean, false),
        Field::new("error_message", DataType::Utf8, true),
        Field::new("paired_item_indices", DataType::List(Arc::new(Field::new("item", DataType::UInt32, true))), true),
    ])
}

/// Schema for task data (node execution results).
pub fn task_data_schema() -> Schema {
    Schema::new(vec![
        Field::new("node_name", DataType::Utf8, false),
        Field::new("run_index", DataType::UInt32, false),
        Field::new("start_time", DataType::Timestamp(TimeUnit::Millisecond, None), false),
        Field::new("execution_time_ms", DataType::Int64, false),
        Field::new("status", DataType::Utf8, false),
        Field::new("error_message", DataType::Utf8, true),
        Field::new("output_count", DataType::UInt32, false),
    ])
}

/// Schema for workflow execution summary.
pub fn execution_summary_schema() -> Schema {
    Schema::new(vec![
        Field::new("execution_id", DataType::Utf8, false),
        Field::new("workflow_id", DataType::Utf8, false),
        Field::new("status", DataType::Utf8, false),
        Field::new("mode", DataType::Utf8, false),
        Field::new("started_at", DataType::Timestamp(TimeUnit::Millisecond, None), false),
        Field::new("finished_at", DataType::Timestamp(TimeUnit::Millisecond, None), true),
        Field::new("duration_ms", DataType::Int64, true),
        Field::new("node_count", DataType::UInt32, false),
        Field::new("error_message", DataType::Utf8, true),
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
        "node_execution_data" => Some(node_execution_data_schema()),
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
