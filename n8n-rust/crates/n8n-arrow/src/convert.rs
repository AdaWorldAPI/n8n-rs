//! Conversion between n8n types and Arrow arrays.
//!
//! Provides typed Arrow conversions that map n8n `GenericValue` variants to
//! native Arrow column types instead of wrapping everything in JSON strings.

use crate::error::ArrowError;
use crate::schema;
use arrow_array::{
    Array, ArrayRef, BooleanArray, Float64Array, Int32Array, Int64Array, NullArray, RecordBatch,
    StringArray, TimestampMillisecondArray, UInt32Array,
};
use arrow_schema::DataType;
use n8n_workflow::{DataObject, GenericValue, NodeExecutionData, Run, Workflow};
use std::collections::HashMap;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// run_to_summary_batch
// ---------------------------------------------------------------------------

/// Convert a Run to an execution summary RecordBatch.
///
/// Columns: execution_id, workflow_id, status, mode, started_at, finished_at,
///          total_nodes, error_message, last_node_executed.
pub fn run_to_summary_batch(
    execution_id: &str,
    workflow_id: &str,
    run: &Run,
) -> Result<RecordBatch, ArrowError> {
    let schema = Arc::new(schema::execution_summary_schema());

    let error_msg: Option<String> = run
        .data
        .result_data
        .error
        .as_ref()
        .map(|e| e.message.clone());

    let last_node: Option<String> = run.data.result_data.last_node_executed.clone();

    let total_nodes = run.data.result_data.run_data.len() as i32;

    RecordBatch::try_new(
        schema,
        vec![
            Arc::new(StringArray::from(vec![execution_id])) as ArrayRef,
            Arc::new(StringArray::from(vec![workflow_id])) as ArrayRef,
            Arc::new(StringArray::from(vec![run.status.as_str()])) as ArrayRef,
            Arc::new(StringArray::from(vec![run.mode.as_str()])) as ArrayRef,
            Arc::new(TimestampMillisecondArray::from(vec![
                run.started_at.timestamp_millis(),
            ])) as ArrayRef,
            Arc::new(TimestampMillisecondArray::from(vec![
                run.finished_at.map(|f| f.timestamp_millis()),
            ])) as ArrayRef,
            Arc::new(Int32Array::from(vec![total_nodes])) as ArrayRef,
            Arc::new(StringArray::from(vec![error_msg])) as ArrayRef,
            Arc::new(StringArray::from(vec![last_node])) as ArrayRef,
        ],
    )
    .map_err(ArrowError::from)
}

// ---------------------------------------------------------------------------
// run_data_to_batch
// ---------------------------------------------------------------------------

/// Convert a RunData HashMap to an Arrow RecordBatch of per-node task summaries.
///
/// Columns: node_name (Utf8), run_index (Int32), start_time (Timestamp ms),
///          execution_time_ms (Int64), status (Utf8), output_items_count (Int32),
///          error_message (Utf8 nullable).
pub fn run_data_to_batch(
    run_data: &HashMap<String, Vec<n8n_workflow::TaskData>>,
) -> Result<RecordBatch, ArrowError> {
    let schema = Arc::new(schema::task_data_schema());

    let mut node_names: Vec<String> = Vec::new();
    let mut run_indices: Vec<i32> = Vec::new();
    let mut start_times: Vec<i64> = Vec::new();
    let mut execution_times: Vec<i64> = Vec::new();
    let mut statuses: Vec<String> = Vec::new();
    let mut output_counts: Vec<i32> = Vec::new();
    let mut error_messages: Vec<Option<String>> = Vec::new();

    for (node_name, tasks) in run_data {
        for (run_idx, task) in tasks.iter().enumerate() {
            node_names.push(node_name.clone());
            run_indices.push(run_idx as i32);
            start_times.push(task.start_time);
            execution_times.push(task.execution_time);
            statuses.push(task.execution_status.as_str().to_string());
            error_messages.push(task.error.as_ref().map(|e| e.message.clone()));

            let count: usize = task
                .data
                .as_ref()
                .map(|d| {
                    d.values()
                        .flat_map(|outputs| outputs.iter().map(|o| o.len()))
                        .sum()
                })
                .unwrap_or(0);
            output_counts.push(count as i32);
        }
    }

    RecordBatch::try_new(
        schema,
        vec![
            Arc::new(StringArray::from(node_names)) as ArrayRef,
            Arc::new(Int32Array::from(run_indices)) as ArrayRef,
            Arc::new(TimestampMillisecondArray::from(start_times)) as ArrayRef,
            Arc::new(Int64Array::from(execution_times)) as ArrayRef,
            Arc::new(StringArray::from(statuses)) as ArrayRef,
            Arc::new(Int32Array::from(output_counts)) as ArrayRef,
            Arc::new(StringArray::from(error_messages)) as ArrayRef,
        ],
    )
    .map_err(ArrowError::from)
}

// ---------------------------------------------------------------------------
// node_execution_data_to_batch  (typed, schema-inferred)
// ---------------------------------------------------------------------------

/// Convert a slice of NodeExecutionData to an Arrow RecordBatch with typed columns.
///
/// The schema is inferred from the superset of all keys across all items.
/// Each `GenericValue` variant maps to a native Arrow type:
///
/// - `String`           -> `Utf8`
/// - `Integer(i64)`     -> `Int64`
/// - `Float(f64)`       -> `Float64`
/// - `Bool(bool)`       -> `Boolean`
/// - `Null`             -> `Null` (or `Utf8` if other rows have strings)
/// - `Object` / `Array` -> `Utf8` (JSON-serialized)
pub fn node_execution_data_to_batch(
    data: &[NodeExecutionData],
) -> Result<RecordBatch, ArrowError> {
    let inferred = schema::infer_node_execution_data_schema(data);
    if inferred.fields().is_empty() || data.is_empty() {
        // Return an empty batch with the inferred (possibly empty) schema.
        return RecordBatch::try_new_with_options(
            Arc::new(inferred),
            vec![],
            &arrow_array::RecordBatchOptions::new().with_row_count(Some(0)),
        )
        .map_err(ArrowError::from);
    }

    let schema = Arc::new(inferred);
    let num_rows = data.len();
    let num_cols = schema.fields().len();

    let mut columns: Vec<ArrayRef> = Vec::with_capacity(num_cols);

    for field in schema.fields() {
        let col_name = field.name();
        let col = build_column_for_field(field.data_type(), col_name, data, num_rows)?;
        columns.push(col);
    }

    RecordBatch::try_new(schema, columns).map_err(ArrowError::from)
}

/// Build a single Arrow column from the items' DataObject values for the given key.
fn build_column_for_field(
    data_type: &DataType,
    key: &str,
    items: &[NodeExecutionData],
    num_rows: usize,
) -> Result<ArrayRef, ArrowError> {
    match data_type {
        DataType::Utf8 => {
            let values: Vec<Option<String>> = items
                .iter()
                .map(|item| match item.json.get(key) {
                    Some(GenericValue::String(s)) => Some(s.clone()),
                    Some(GenericValue::Array(arr)) => {
                        Some(serde_json::to_string(arr).unwrap_or_default())
                    }
                    Some(GenericValue::Object(obj)) => {
                        Some(serde_json::to_string(obj).unwrap_or_default())
                    }
                    Some(GenericValue::Null) | None => None,
                    // If the inferred type is Utf8 but the actual value is a
                    // different primitive (e.g. a heterogeneous schema promoted
                    // a column to string), convert it.
                    Some(other) => Some(serde_json::to_string(other).unwrap_or_default()),
                })
                .collect();
            Ok(Arc::new(StringArray::from(values)) as ArrayRef)
        }
        DataType::Int64 => {
            let values: Vec<Option<i64>> = items
                .iter()
                .map(|item| match item.json.get(key) {
                    Some(GenericValue::Integer(n)) => Some(*n),
                    _ => None,
                })
                .collect();
            Ok(Arc::new(Int64Array::from(values)) as ArrayRef)
        }
        DataType::Float64 => {
            let values: Vec<Option<f64>> = items
                .iter()
                .map(|item| match item.json.get(key) {
                    Some(GenericValue::Float(f)) => Some(*f),
                    Some(GenericValue::Integer(n)) => Some(*n as f64),
                    _ => None,
                })
                .collect();
            Ok(Arc::new(Float64Array::from(values)) as ArrayRef)
        }
        DataType::Boolean => {
            let values: Vec<Option<bool>> = items
                .iter()
                .map(|item| match item.json.get(key) {
                    Some(GenericValue::Bool(b)) => Some(*b),
                    _ => None,
                })
                .collect();
            Ok(Arc::new(BooleanArray::from(values)) as ArrayRef)
        }
        DataType::Null => Ok(Arc::new(NullArray::new(num_rows)) as ArrayRef),
        _ => {
            // Fallback: serialize to JSON string.
            let values: Vec<Option<String>> = items
                .iter()
                .map(|item| match item.json.get(key) {
                    Some(GenericValue::Null) | None => None,
                    Some(v) => Some(serde_json::to_string(v).unwrap_or_default()),
                })
                .collect();
            Ok(Arc::new(StringArray::from(values)) as ArrayRef)
        }
    }
}

// ---------------------------------------------------------------------------
// batch_to_node_execution_data  (reverse conversion)
// ---------------------------------------------------------------------------

/// Convert an Arrow RecordBatch back to a vector of NodeExecutionData.
///
/// Each column is mapped back to the corresponding `GenericValue` variant based
/// on the column's Arrow DataType:
///
/// - `Utf8`    -> `GenericValue::String`
/// - `Int64`   -> `GenericValue::Integer`
/// - `Float64` -> `GenericValue::Float`
/// - `Boolean` -> `GenericValue::Bool`
/// - `Null`    -> skipped (key is absent)
///
/// Null values in any column are omitted from the resulting DataObject.
pub fn batch_to_node_execution_data(
    batch: &RecordBatch,
) -> Result<Vec<NodeExecutionData>, ArrowError> {
    let num_rows = batch.num_rows();
    let schema = batch.schema();
    let mut results: Vec<NodeExecutionData> = Vec::with_capacity(num_rows);

    for row in 0..num_rows {
        let mut obj = DataObject::new();

        for (col_idx, field) in schema.fields().iter().enumerate() {
            let col = batch.column(col_idx);
            if col.is_null(row) {
                continue;
            }

            let key = field.name().clone();
            let value = arrow_value_to_generic(col.as_ref(), row, field.data_type())?;
            if let Some(v) = value {
                obj.insert(key, v);
            }
        }

        results.push(NodeExecutionData::new(obj));
    }

    Ok(results)
}

/// Extract a GenericValue from an Arrow array at the given row index.
fn arrow_value_to_generic(
    col: &dyn Array,
    row: usize,
    data_type: &DataType,
) -> Result<Option<GenericValue>, ArrowError> {
    if col.is_null(row) {
        return Ok(None);
    }

    match data_type {
        DataType::Utf8 => {
            let arr = col
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| {
                    ArrowError::ConversionError("Expected StringArray for Utf8 column".into())
                })?;
            let s = arr.value(row);
            // Attempt to detect if this was a JSON-serialized object or array.
            if (s.starts_with('{') && s.ends_with('}')) || (s.starts_with('[') && s.ends_with(']'))
            {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(s) {
                    return Ok(Some(json_value_to_generic(parsed)));
                }
            }
            Ok(Some(GenericValue::String(s.to_string())))
        }
        DataType::Int64 => {
            let arr = col.as_any().downcast_ref::<Int64Array>().ok_or_else(|| {
                ArrowError::ConversionError("Expected Int64Array".into())
            })?;
            Ok(Some(GenericValue::Integer(arr.value(row))))
        }
        DataType::Float64 => {
            let arr = col
                .as_any()
                .downcast_ref::<Float64Array>()
                .ok_or_else(|| {
                    ArrowError::ConversionError("Expected Float64Array".into())
                })?;
            Ok(Some(GenericValue::Float(arr.value(row))))
        }
        DataType::Boolean => {
            let arr = col
                .as_any()
                .downcast_ref::<BooleanArray>()
                .ok_or_else(|| {
                    ArrowError::ConversionError("Expected BooleanArray".into())
                })?;
            Ok(Some(GenericValue::Bool(arr.value(row))))
        }
        DataType::Null => Ok(None),
        _ => {
            // Fallback: try to read as string.
            if let Some(arr) = col.as_any().downcast_ref::<StringArray>() {
                Ok(Some(GenericValue::String(arr.value(row).to_string())))
            } else {
                Ok(Some(GenericValue::Null))
            }
        }
    }
}

/// Convert a serde_json::Value to a GenericValue.
fn json_value_to_generic(val: serde_json::Value) -> GenericValue {
    match val {
        serde_json::Value::Null => GenericValue::Null,
        serde_json::Value::Bool(b) => GenericValue::Bool(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                GenericValue::Integer(i)
            } else if let Some(f) = n.as_f64() {
                GenericValue::Float(f)
            } else {
                GenericValue::Null
            }
        }
        serde_json::Value::String(s) => GenericValue::String(s),
        serde_json::Value::Array(arr) => {
            GenericValue::Array(arr.into_iter().map(json_value_to_generic).collect())
        }
        serde_json::Value::Object(map) => {
            let obj: DataObject = map
                .into_iter()
                .map(|(k, v)| (k, json_value_to_generic(v)))
                .collect();
            GenericValue::Object(obj)
        }
    }
}

// ---------------------------------------------------------------------------
// Workflow-level conversions (unchanged signatures)
// ---------------------------------------------------------------------------

/// Convert workflow nodes to Arrow RecordBatch.
pub fn workflow_nodes_to_batch(workflow: &Workflow) -> Result<RecordBatch, ArrowError> {
    let schema = Arc::new(schema::workflow_node_schema());

    let ids: Vec<_> = workflow.nodes.iter().map(|n| n.id.clone()).collect();
    let names: Vec<_> = workflow.nodes.iter().map(|n| n.name.clone()).collect();
    let types: Vec<_> = workflow.nodes.iter().map(|n| n.node_type.clone()).collect();
    let versions: Vec<_> = workflow.nodes.iter().map(|n| n.type_version).collect();
    let pos_x: Vec<_> = workflow.nodes.iter().map(|n| n.position[0]).collect();
    let pos_y: Vec<_> = workflow.nodes.iter().map(|n| n.position[1]).collect();
    let disabled: Vec<_> = workflow.nodes.iter().map(|n| n.disabled).collect();
    let params: Vec<_> = workflow
        .nodes
        .iter()
        .map(|n| serde_json::to_string(&n.parameters).unwrap_or_default())
        .collect();

    RecordBatch::try_new(
        schema,
        vec![
            Arc::new(StringArray::from(ids)) as ArrayRef,
            Arc::new(StringArray::from(names)) as ArrayRef,
            Arc::new(StringArray::from(types)) as ArrayRef,
            Arc::new(UInt32Array::from(versions)) as ArrayRef,
            Arc::new(Float64Array::from(pos_x)) as ArrayRef,
            Arc::new(Float64Array::from(pos_y)) as ArrayRef,
            Arc::new(BooleanArray::from(disabled)) as ArrayRef,
            Arc::new(StringArray::from(params)) as ArrayRef,
        ],
    )
    .map_err(ArrowError::from)
}

/// Convert workflow connections to Arrow RecordBatch.
pub fn workflow_connections_to_batch(workflow: &Workflow) -> Result<RecordBatch, ArrowError> {
    let schema = Arc::new(schema::workflow_connection_schema());

    let mut source_nodes = Vec::new();
    let mut source_indices = Vec::new();
    let mut target_nodes = Vec::new();
    let mut target_indices = Vec::new();
    let mut connection_types = Vec::new();

    for (source, node_conns) in &workflow.connections {
        for (conn_type, by_index) in node_conns {
            for (src_idx, connections) in by_index.iter().enumerate() {
                for conn in connections {
                    source_nodes.push(source.clone());
                    source_indices.push(src_idx as u32);
                    target_nodes.push(conn.node.clone());
                    target_indices.push(conn.index as u32);
                    connection_types.push(conn_type.clone());
                }
            }
        }
    }

    RecordBatch::try_new(
        schema,
        vec![
            Arc::new(StringArray::from(source_nodes)) as ArrayRef,
            Arc::new(UInt32Array::from(source_indices)) as ArrayRef,
            Arc::new(StringArray::from(target_nodes)) as ArrayRef,
            Arc::new(UInt32Array::from(target_indices)) as ArrayRef,
            Arc::new(StringArray::from(connection_types)) as ArrayRef,
        ],
    )
    .map_err(ArrowError::from)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use n8n_workflow::{GenericValue, NodeExecutionData};
    use std::collections::HashMap;

    fn make_item(pairs: Vec<(&str, GenericValue)>) -> NodeExecutionData {
        let mut obj = DataObject::new();
        for (k, v) in pairs {
            obj.insert(k.to_string(), v);
        }
        NodeExecutionData::new(obj)
    }

    #[test]
    fn test_typed_roundtrip_strings() {
        let items = vec![
            make_item(vec![("name", GenericValue::String("Alice".into()))]),
            make_item(vec![("name", GenericValue::String("Bob".into()))]),
        ];

        let batch = node_execution_data_to_batch(&items).unwrap();
        assert_eq!(batch.num_rows(), 2);
        assert_eq!(batch.num_columns(), 1);
        assert_eq!(
            *batch.schema().field(0).data_type(),
            DataType::Utf8
        );

        let recovered = batch_to_node_execution_data(&batch).unwrap();
        assert_eq!(recovered.len(), 2);
        assert_eq!(
            recovered[0].json.get("name"),
            Some(&GenericValue::String("Alice".into()))
        );
        assert_eq!(
            recovered[1].json.get("name"),
            Some(&GenericValue::String("Bob".into()))
        );
    }

    #[test]
    fn test_typed_roundtrip_integers() {
        let items = vec![
            make_item(vec![("count", GenericValue::Integer(42))]),
            make_item(vec![("count", GenericValue::Integer(100))]),
        ];

        let batch = node_execution_data_to_batch(&items).unwrap();
        assert_eq!(
            *batch.schema().field(0).data_type(),
            DataType::Int64
        );

        let recovered = batch_to_node_execution_data(&batch).unwrap();
        assert_eq!(
            recovered[0].json.get("count"),
            Some(&GenericValue::Integer(42))
        );
    }

    #[test]
    fn test_typed_roundtrip_floats() {
        let items = vec![make_item(vec![("price", GenericValue::Float(9.99))])];

        let batch = node_execution_data_to_batch(&items).unwrap();
        assert_eq!(
            *batch.schema().field(0).data_type(),
            DataType::Float64
        );

        let recovered = batch_to_node_execution_data(&batch).unwrap();
        assert_eq!(
            recovered[0].json.get("price"),
            Some(&GenericValue::Float(9.99))
        );
    }

    #[test]
    fn test_typed_roundtrip_booleans() {
        let items = vec![
            make_item(vec![("active", GenericValue::Bool(true))]),
            make_item(vec![("active", GenericValue::Bool(false))]),
        ];

        let batch = node_execution_data_to_batch(&items).unwrap();
        assert_eq!(
            *batch.schema().field(0).data_type(),
            DataType::Boolean
        );

        let recovered = batch_to_node_execution_data(&batch).unwrap();
        assert_eq!(
            recovered[0].json.get("active"),
            Some(&GenericValue::Bool(true))
        );
        assert_eq!(
            recovered[1].json.get("active"),
            Some(&GenericValue::Bool(false))
        );
    }

    #[test]
    fn test_typed_roundtrip_mixed_keys() {
        let items = vec![
            make_item(vec![
                ("name", GenericValue::String("Alice".into())),
                ("age", GenericValue::Integer(30)),
                ("score", GenericValue::Float(95.5)),
                ("active", GenericValue::Bool(true)),
            ]),
            make_item(vec![
                ("name", GenericValue::String("Bob".into())),
                ("age", GenericValue::Integer(25)),
                // score missing -> null
                ("active", GenericValue::Bool(false)),
            ]),
        ];

        let batch = node_execution_data_to_batch(&items).unwrap();
        assert_eq!(batch.num_rows(), 2);
        assert_eq!(batch.num_columns(), 4);

        let recovered = batch_to_node_execution_data(&batch).unwrap();
        assert_eq!(recovered.len(), 2);

        // Bob should not have "score"
        assert!(recovered[1].json.get("score").is_none());
        assert_eq!(
            recovered[1].json.get("name"),
            Some(&GenericValue::String("Bob".into()))
        );
    }

    #[test]
    fn test_typed_roundtrip_nested_object() {
        let mut inner = DataObject::new();
        inner.insert("x".to_string(), GenericValue::Integer(1));
        inner.insert("y".to_string(), GenericValue::Integer(2));

        let items = vec![make_item(vec![(
            "coords",
            GenericValue::Object(inner.clone()),
        )])];

        let batch = node_execution_data_to_batch(&items).unwrap();
        // Objects are serialized as Utf8
        assert_eq!(
            *batch.schema().field(0).data_type(),
            DataType::Utf8
        );

        let recovered = batch_to_node_execution_data(&batch).unwrap();
        // The recovered value should be an Object after JSON deserialization.
        match recovered[0].json.get("coords") {
            Some(GenericValue::Object(obj)) => {
                assert_eq!(obj.get("x"), Some(&GenericValue::Integer(1)));
                assert_eq!(obj.get("y"), Some(&GenericValue::Integer(2)));
            }
            other => panic!("Expected Object, got {:?}", other),
        }
    }

    #[test]
    fn test_typed_roundtrip_array_value() {
        let items = vec![make_item(vec![(
            "tags",
            GenericValue::Array(vec![
                GenericValue::String("a".into()),
                GenericValue::String("b".into()),
            ]),
        )])];

        let batch = node_execution_data_to_batch(&items).unwrap();
        assert_eq!(
            *batch.schema().field(0).data_type(),
            DataType::Utf8
        );

        let recovered = batch_to_node_execution_data(&batch).unwrap();
        match recovered[0].json.get("tags") {
            Some(GenericValue::Array(arr)) => {
                assert_eq!(arr.len(), 2);
                assert_eq!(arr[0], GenericValue::String("a".into()));
            }
            other => panic!("Expected Array, got {:?}", other),
        }
    }

    #[test]
    fn test_empty_input() {
        let items: Vec<NodeExecutionData> = vec![];
        let batch = node_execution_data_to_batch(&items).unwrap();
        assert_eq!(batch.num_rows(), 0);

        let recovered = batch_to_node_execution_data(&batch).unwrap();
        assert!(recovered.is_empty());
    }

    #[test]
    fn test_heterogeneous_superset_schema() {
        // Item 1 has "a", item 2 has "b" -- superset has both columns.
        let items = vec![
            make_item(vec![("a", GenericValue::Integer(1))]),
            make_item(vec![("b", GenericValue::String("hello".into()))]),
        ];

        let batch = node_execution_data_to_batch(&items).unwrap();
        assert_eq!(batch.num_columns(), 2);
        assert_eq!(batch.num_rows(), 2);

        let recovered = batch_to_node_execution_data(&batch).unwrap();
        // First item has "a" but not "b" (null).
        assert_eq!(
            recovered[0].json.get("a"),
            Some(&GenericValue::Integer(1))
        );
        assert!(recovered[0].json.get("b").is_none());
        // Second item has "b" but not "a" (null).
        assert_eq!(
            recovered[1].json.get("b"),
            Some(&GenericValue::String("hello".into()))
        );
        assert!(recovered[1].json.get("a").is_none());
    }
}
