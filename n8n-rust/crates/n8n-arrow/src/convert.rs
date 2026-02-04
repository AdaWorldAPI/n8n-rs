//! Conversion between n8n types and Arrow arrays.

use crate::error::ArrowError;
use crate::schema;
use arrow_array::{
    Array, ArrayRef, BooleanArray, Float64Array, Int64Array, RecordBatch, StringArray,
    TimestampMillisecondArray, UInt32Array,
};
use n8n_workflow::{
    DataObject, ExecutionStatus, GenericValue, Node, NodeExecutionData, Run, RunData, TaskData,
    Workflow, WorkflowExecuteMode,
};
use std::collections::HashMap;
use std::sync::Arc;

/// Convert a vector of NodeExecutionData to an Arrow RecordBatch.
pub fn node_execution_data_to_batch(
    data: &[NodeExecutionData],
) -> Result<RecordBatch, ArrowError> {
    let schema = Arc::new(schema::node_execution_data_schema());

    let json_data: Vec<String> = data
        .iter()
        .map(|d| serde_json::to_string(&d.json).unwrap_or_default())
        .collect();

    let binary_keys: Vec<Option<Vec<Option<String>>>> = data
        .iter()
        .map(|d| {
            d.binary.as_ref().map(|b| {
                b.keys().map(|k| Some(k.clone())).collect()
            })
        })
        .collect();

    let has_error: Vec<bool> = data.iter().map(|d| d.error.is_some()).collect();

    let error_messages: Vec<Option<String>> = data
        .iter()
        .map(|d| d.error.as_ref().map(|e| e.message.clone()))
        .collect();

    let paired_item_indices: Vec<Option<Vec<Option<u32>>>> = data
        .iter()
        .map(|d| {
            d.paired_item.as_ref().map(|items| {
                items.iter().map(|p| Some(p.item as u32)).collect()
            })
        })
        .collect();

    let json_array = Arc::new(StringArray::from(json_data)) as ArrayRef;
    let has_error_array = Arc::new(BooleanArray::from(has_error)) as ArrayRef;
    let error_message_array = Arc::new(StringArray::from(error_messages)) as ArrayRef;

    // Create list arrays for binary keys and paired items
    let binary_keys_array = create_string_list_array(&binary_keys)?;
    let paired_item_array = create_u32_list_array(&paired_item_indices)?;

    RecordBatch::try_new(
        schema,
        vec![
            json_array,
            binary_keys_array,
            has_error_array,
            error_message_array,
            paired_item_array,
        ],
    )
    .map_err(ArrowError::from)
}

/// Convert a Run's task data to Arrow RecordBatch.
pub fn run_data_to_batch(run_data: &RunData) -> Result<RecordBatch, ArrowError> {
    let schema = Arc::new(schema::task_data_schema());

    let mut node_names = Vec::new();
    let mut run_indices = Vec::new();
    let mut start_times = Vec::new();
    let mut execution_times = Vec::new();
    let mut statuses = Vec::new();
    let mut error_messages = Vec::new();
    let mut output_counts = Vec::new();

    for (node_name, tasks) in run_data {
        for (run_idx, task) in tasks.iter().enumerate() {
            node_names.push(node_name.clone());
            run_indices.push(run_idx as u32);
            start_times.push(task.start_time);
            execution_times.push(task.execution_time);
            statuses.push(format!("{:?}", task.execution_status).to_lowercase());
            error_messages.push(task.error.as_ref().map(|e| e.message.clone()));

            let count = task
                .data
                .as_ref()
                .map(|d| {
                    d.values()
                        .flat_map(|outputs| outputs.iter().map(|o| o.len()))
                        .sum::<usize>()
                })
                .unwrap_or(0);
            output_counts.push(count as u32);
        }
    }

    RecordBatch::try_new(
        schema,
        vec![
            Arc::new(StringArray::from(node_names)) as ArrayRef,
            Arc::new(UInt32Array::from(run_indices)) as ArrayRef,
            Arc::new(TimestampMillisecondArray::from(start_times)) as ArrayRef,
            Arc::new(Int64Array::from(execution_times)) as ArrayRef,
            Arc::new(StringArray::from(statuses)) as ArrayRef,
            Arc::new(StringArray::from(error_messages)) as ArrayRef,
            Arc::new(UInt32Array::from(output_counts)) as ArrayRef,
        ],
    )
    .map_err(ArrowError::from)
}

/// Convert a Run to an execution summary RecordBatch.
pub fn run_to_summary_batch(
    execution_id: &str,
    workflow_id: &str,
    run: &Run,
) -> Result<RecordBatch, ArrowError> {
    let schema = Arc::new(schema::execution_summary_schema());

    let duration = run
        .finished_at
        .map(|f| (f - run.started_at).num_milliseconds());

    let error_msg = run.data.result_data.error.as_ref().map(|e| e.message.clone());

    RecordBatch::try_new(
        schema,
        vec![
            Arc::new(StringArray::from(vec![execution_id])) as ArrayRef,
            Arc::new(StringArray::from(vec![workflow_id])) as ArrayRef,
            Arc::new(StringArray::from(vec![format!("{:?}", run.status).to_lowercase()])) as ArrayRef,
            Arc::new(StringArray::from(vec![format!("{:?}", run.mode).to_lowercase()])) as ArrayRef,
            Arc::new(TimestampMillisecondArray::from(vec![run.started_at.timestamp_millis()])) as ArrayRef,
            Arc::new(TimestampMillisecondArray::from(vec![run.finished_at.map(|f| f.timestamp_millis())])) as ArrayRef,
            Arc::new(Int64Array::from(vec![duration])) as ArrayRef,
            Arc::new(UInt32Array::from(vec![run.data.result_data.run_data.len() as u32])) as ArrayRef,
            Arc::new(StringArray::from(vec![error_msg])) as ArrayRef,
        ],
    )
    .map_err(ArrowError::from)
}

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

/// Convert Arrow RecordBatch to NodeExecutionData.
pub fn batch_to_node_execution_data(
    batch: &RecordBatch,
) -> Result<Vec<NodeExecutionData>, ArrowError> {
    let json_col = batch
        .column(0)
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| ArrowError::ConversionError("Expected string column for json".into()))?;

    let mut result = Vec::with_capacity(batch.num_rows());

    for i in 0..batch.num_rows() {
        let json_str = json_col.value(i);
        let json: DataObject = serde_json::from_str(json_str)?;
        result.push(NodeExecutionData::new(json));
    }

    Ok(result)
}

// Helper functions for creating list arrays
fn create_string_list_array(
    data: &[Option<Vec<Option<String>>>],
) -> Result<ArrayRef, ArrowError> {
    use arrow_array::{builder::StringBuilder, ListArray};
    use arrow_buffer::OffsetBuffer;

    let mut values_builder = StringBuilder::new();
    let mut offsets = vec![0i32];

    for item in data {
        match item {
            Some(strings) => {
                for s in strings {
                    match s {
                        Some(v) => values_builder.append_value(v),
                        None => values_builder.append_null(),
                    }
                }
                offsets.push(offsets.last().unwrap() + strings.len() as i32);
            }
            None => {
                offsets.push(*offsets.last().unwrap());
            }
        }
    }

    let values = values_builder.finish();
    let offset_buffer = OffsetBuffer::new(offsets.into());
    let nulls = data.iter().map(|d| d.is_some()).collect::<Vec<_>>();

    let list_array = ListArray::try_new(
        Arc::new(arrow_schema::Field::new("item", arrow_schema::DataType::Utf8, true)),
        offset_buffer,
        Arc::new(values),
        Some(arrow_buffer::NullBuffer::from(nulls)),
    )?;

    Ok(Arc::new(list_array))
}

fn create_u32_list_array(
    data: &[Option<Vec<Option<u32>>>],
) -> Result<ArrayRef, ArrowError> {
    use arrow_array::{builder::UInt32Builder, ListArray};
    use arrow_buffer::OffsetBuffer;

    let mut values_builder = UInt32Builder::new();
    let mut offsets = vec![0i32];

    for item in data {
        match item {
            Some(nums) => {
                for n in nums {
                    match n {
                        Some(v) => values_builder.append_value(*v),
                        None => values_builder.append_null(),
                    }
                }
                offsets.push(offsets.last().unwrap() + nums.len() as i32);
            }
            None => {
                offsets.push(*offsets.last().unwrap());
            }
        }
    }

    let values = values_builder.finish();
    let offset_buffer = OffsetBuffer::new(offsets.into());
    let nulls = data.iter().map(|d| d.is_some()).collect::<Vec<_>>();

    let list_array = ListArray::try_new(
        Arc::new(arrow_schema::Field::new("item", arrow_schema::DataType::UInt32, true)),
        offset_buffer,
        Arc::new(values),
        Some(arrow_buffer::NullBuffer::from(nulls)),
    )?;

    Ok(Arc::new(list_array))
}
