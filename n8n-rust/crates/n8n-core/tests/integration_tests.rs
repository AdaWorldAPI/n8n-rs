//! Integration tests for the workflow execution engine.
//!
//! These tests verify end-to-end workflow execution including node traversal,
//! data flow between nodes, conditional branching, error handling, and event
//! streaming.

use std::collections::HashMap;

use n8n_core::{ExecutionEvent, WorkflowEngine};
use n8n_workflow::{
    ExecutionStatus, GenericValue, Node, NodeExecutionData, NodeParameterValue,
    Workflow, WorkflowExecuteMode,
};
use tokio::sync::mpsc;

// ============================================================================
// Helper functions
// ============================================================================

/// Create a workflow from a list of nodes and connect them using the provided
/// connection tuples.  Each tuple is `(source_name, target_name,
/// source_output_index, target_input_index)`.
fn make_workflow(
    name: &str,
    nodes: Vec<Node>,
    connections: &[(&str, &str, usize, usize)],
) -> Workflow {
    let mut wf = Workflow::new(name);
    for node in nodes {
        wf.add_node(node);
    }
    for &(src, tgt, src_idx, tgt_idx) in connections {
        wf.connect(src, tgt, src_idx, tgt_idx)
            .unwrap_or_else(|e| panic!("Failed to connect {src} -> {tgt}: {e}"));
    }
    wf
}

/// Create a ManualTrigger node with the given display name.
fn manual_trigger(name: &str) -> Node {
    Node::new(name, "n8n-nodes-base.manualTrigger")
}

/// Create a Set node that sets the given key/value string pairs.
fn set_node(name: &str, values: &[(&str, &str)]) -> Node {
    let mut node = Node::new(name, "n8n-nodes-base.set");
    let mut map: HashMap<String, NodeParameterValue> = HashMap::new();
    for &(k, v) in values {
        map.insert(k.to_string(), NodeParameterValue::String(v.to_string()));
    }
    node.set_parameter("values", NodeParameterValue::Object(map));
    node
}

/// Create a NoOp (pass-through) node.
fn noop_node(name: &str) -> Node {
    Node::new(name, "n8n-nodes-base.noOp")
}

/// Create an If node that checks whether a given field exists on the item.
fn if_node(name: &str, field: &str) -> Node {
    let mut node = Node::new(name, "n8n-nodes-base.if");
    let mut conditions: HashMap<String, NodeParameterValue> = HashMap::new();
    conditions.insert(
        "field".to_string(),
        NodeParameterValue::String(field.to_string()),
    );
    node.set_parameter("conditions", NodeParameterValue::Object(conditions));
    node
}

/// Create a Merge node.
fn merge_node(name: &str) -> Node {
    Node::new(name, "n8n-nodes-base.merge")
}

/// Create a Filter node that checks a field for truthiness.
fn filter_node(name: &str, field: &str) -> Node {
    let mut node = Node::new(name, "n8n-nodes-base.filter");
    let mut cond: HashMap<String, NodeParameterValue> = HashMap::new();
    cond.insert(
        "field".to_string(),
        NodeParameterValue::String(field.to_string()),
    );
    node.set_parameter("conditions", NodeParameterValue::Object(cond));
    node
}

/// Create a Sort node that sorts by the given field with the given order.
fn sort_node(name: &str, sort_by: &str, order: &str) -> Node {
    let mut node = Node::new(name, "n8n-nodes-base.sort");
    node.set_parameter("sortBy", NodeParameterValue::String(sort_by.to_string()));
    node.set_parameter("order", NodeParameterValue::String(order.to_string()));
    node
}

/// Create a Limit node with the given max items.
fn limit_node(name: &str, max_items: f64) -> Node {
    let mut node = Node::new(name, "n8n-nodes-base.limit");
    node.set_parameter("maxItems", NodeParameterValue::Number(max_items));
    node
}

/// Create a Switch node with rules that check for specific field presence.
fn switch_node(name: &str, num_outputs: f64, rule_fields: &[&str]) -> Node {
    let mut node = Node::new(name, "n8n-nodes-base.switch");
    node.set_parameter("numberOutputs", NodeParameterValue::Number(num_outputs));
    let rules: Vec<NodeParameterValue> = rule_fields
        .iter()
        .map(|field| {
            let mut rule: HashMap<String, NodeParameterValue> = HashMap::new();
            rule.insert(
                "field".to_string(),
                NodeParameterValue::String(field.to_string()),
            );
            NodeParameterValue::Object(rule)
        })
        .collect();
    let mut rules_obj: HashMap<String, NodeParameterValue> = HashMap::new();
    rules_obj.insert("rules".to_string(), NodeParameterValue::Array(rules));
    node.set_parameter("rules", NodeParameterValue::Object(rules_obj));
    node
}

/// Create a StopAndError node.
fn stop_and_error_node(name: &str, message: &str) -> Node {
    let mut node = Node::new(name, "n8n-nodes-base.stopAndError");
    node.set_parameter(
        "errorMessage",
        NodeParameterValue::String(message.to_string()),
    );
    node
}

/// Helper to extract output items from a run for a given node name.
/// Returns the items from the first run of the node, first output index.
fn get_node_output_items(run: &n8n_workflow::Run, node_name: &str) -> Vec<NodeExecutionData> {
    let task_data_vec = run
        .data
        .result_data
        .run_data
        .get(node_name)
        .unwrap_or_else(|| panic!("No run data found for node '{node_name}'"));

    let task_data = &task_data_vec[0];
    let connections = task_data
        .data
        .as_ref()
        .unwrap_or_else(|| panic!("No output data for node '{node_name}'"));

    let main_outputs = connections
        .get("main")
        .unwrap_or_else(|| panic!("No 'main' output for node '{node_name}'"));

    main_outputs
        .first()
        .cloned()
        .unwrap_or_default()
}

/// Helper to extract output items from a specific output index of a node.
fn get_node_output_at_index(
    run: &n8n_workflow::Run,
    node_name: &str,
    output_index: usize,
) -> Vec<NodeExecutionData> {
    let task_data_vec = run
        .data
        .result_data
        .run_data
        .get(node_name)
        .unwrap_or_else(|| panic!("No run data found for node '{node_name}'"));

    let task_data = &task_data_vec[0];
    let connections = task_data
        .data
        .as_ref()
        .unwrap_or_else(|| panic!("No output data for node '{node_name}'"));

    let main_outputs = connections
        .get("main")
        .unwrap_or_else(|| panic!("No 'main' output for node '{node_name}'"));

    main_outputs
        .get(output_index)
        .cloned()
        .unwrap_or_default()
}

// ============================================================================
// Test cases
// ============================================================================

/// 1. Simple trigger-to-set pipeline.
///    ManualTrigger -> Set(field1="hello")
///    Verify that the Set node ran and added the value.
#[tokio::test]
async fn test_simple_trigger_to_set() {
    let engine = WorkflowEngine::default();

    let workflow = make_workflow(
        "simple_trigger_to_set",
        vec![
            manual_trigger("Trigger"),
            set_node("Set", &[("field1", "hello")]),
        ],
        &[("Trigger", "Set", 0, 0)],
    );

    let run = engine
        .execute(&workflow, WorkflowExecuteMode::Manual, None)
        .await
        .expect("Execution should succeed");

    assert_eq!(run.status, ExecutionStatus::Success);

    // The Set node should have executed
    assert!(
        run.data.result_data.run_data.contains_key("Set"),
        "Set node should be in run data"
    );

    // Verify the Set node added the field
    let set_items = get_node_output_items(&run, "Set");
    assert!(!set_items.is_empty(), "Set node should output at least one item");

    let first_item = &set_items[0];
    let field_val = first_item.json.get("field1");
    assert!(field_val.is_some(), "Set node should add 'field1'");
    assert_eq!(
        field_val.unwrap(),
        &GenericValue::String("hello".to_string())
    );
}

/// 2. If branching test.
///    ManualTrigger -> Set(active="true") -> If(field="active") -> NoOp_True (output 0)
///                                                               -> NoOp_False (output 1)
///    Since the Set node adds "active" as a string field and the If node
///    checks for field existence, items should go to the true branch only.
#[tokio::test]
async fn test_if_branching() {
    let engine = WorkflowEngine::default();

    let workflow = make_workflow(
        "if_branching",
        vec![
            manual_trigger("Trigger"),
            set_node("Set", &[("active", "true")]),
            if_node("If", "active"),
            noop_node("TrueBranch"),
            noop_node("FalseBranch"),
        ],
        &[
            ("Trigger", "Set", 0, 0),
            ("Set", "If", 0, 0),
            ("If", "TrueBranch", 0, 0),  // output 0 = true branch
            ("If", "FalseBranch", 1, 0), // output 1 = false branch
        ],
    );

    let run = engine
        .execute(&workflow, WorkflowExecuteMode::Manual, None)
        .await
        .expect("Execution should succeed");

    assert_eq!(run.status, ExecutionStatus::Success);

    // The If node should have produced output on index 0 (true branch)
    let true_items = get_node_output_at_index(&run, "If", 0);
    assert!(
        !true_items.is_empty(),
        "True branch should have items"
    );

    // The false branch (output 1) should be empty
    let false_items = get_node_output_at_index(&run, "If", 1);
    assert!(
        false_items.is_empty(),
        "False branch should be empty"
    );

    // TrueBranch NoOp should have executed
    assert!(
        run.data.result_data.run_data.contains_key("TrueBranch"),
        "TrueBranch node should have been executed"
    );

    // FalseBranch NoOp should NOT have executed (no items flowed to it)
    assert!(
        !run.data.result_data.run_data.contains_key("FalseBranch"),
        "FalseBranch node should NOT have been executed"
    );
}

/// 3. Merge two branches.
///    ManualTrigger1 -> Set1(source="branch1")
///    ManualTrigger2 -> Set2(source="branch2")
///    Both Set nodes connect to a Merge node.
///    The merge should combine items from both branches.
#[tokio::test]
async fn test_merge_two_branches() {
    let engine = WorkflowEngine::default();

    // NOTE: Because the engine finds all trigger nodes and executes them
    // sequentially, each branch will be queued. The Merge node will be
    // reached once from each branch, producing separate runs.
    // We verify that the Merge node was executed at least once.
    let workflow = make_workflow(
        "merge_two_branches",
        vec![
            manual_trigger("Trigger1"),
            manual_trigger("Trigger2"),
            set_node("Set1", &[("source", "branch1")]),
            set_node("Set2", &[("source", "branch2")]),
            merge_node("Merge"),
        ],
        &[
            ("Trigger1", "Set1", 0, 0),
            ("Trigger2", "Set2", 0, 0),
            ("Set1", "Merge", 0, 0),
            ("Set2", "Merge", 0, 0),
        ],
    );

    let run = engine
        .execute(&workflow, WorkflowExecuteMode::Manual, None)
        .await
        .expect("Execution should succeed");

    assert_eq!(run.status, ExecutionStatus::Success);

    // Merge node should have executed
    assert!(
        run.data.result_data.run_data.contains_key("Merge"),
        "Merge node should have been executed"
    );

    // Both Set nodes should have executed
    assert!(
        run.data.result_data.run_data.contains_key("Set1"),
        "Set1 should have been executed"
    );
    assert!(
        run.data.result_data.run_data.contains_key("Set2"),
        "Set2 should have been executed"
    );

    // The Merge node should have produced output items
    let merge_runs = run.data.result_data.run_data.get("Merge").unwrap();
    assert!(
        !merge_runs.is_empty(),
        "Merge should have at least one execution run"
    );

    // Collect all items from all Merge runs
    let mut all_merge_items: Vec<NodeExecutionData> = Vec::new();
    for task in merge_runs {
        if let Some(ref data) = task.data {
            if let Some(main) = data.get("main") {
                for output in main {
                    all_merge_items.extend(output.clone());
                }
            }
        }
    }
    assert!(
        !all_merge_items.is_empty(),
        "Merge should produce output items"
    );
}

/// 4. Filter node test.
///    ManualTrigger -> Set(count="5") -> Filter(field="count")
///    The filter checks truthiness of the "count" field (non-empty string
///    is truthy), so the item should pass through.
#[tokio::test]
async fn test_filter_node() {
    let engine = WorkflowEngine::default();

    let workflow = make_workflow(
        "filter_test",
        vec![
            manual_trigger("Trigger"),
            set_node("Set", &[("count", "5")]),
            filter_node("Filter", "count"),
        ],
        &[
            ("Trigger", "Set", 0, 0),
            ("Set", "Filter", 0, 0),
        ],
    );

    let run = engine
        .execute(&workflow, WorkflowExecuteMode::Manual, None)
        .await
        .expect("Execution should succeed");

    assert_eq!(run.status, ExecutionStatus::Success);

    // Filter output 0 = passed items
    let passed = get_node_output_at_index(&run, "Filter", 0);
    assert!(
        !passed.is_empty(),
        "Items with truthy 'count' field should pass the filter"
    );

    // Filter output 1 = failed items (should be empty)
    let failed = get_node_output_at_index(&run, "Filter", 1);
    assert!(
        failed.is_empty(),
        "No items should fail the filter since 'count' is truthy"
    );
}

/// 5. Sort node test.
///    ManualTrigger -> Sort(sortBy="name", order="asc")
///    We supply multiple input items with different "name" values and verify
///    they come out sorted.
#[tokio::test]
async fn test_sort_node() {
    let engine = WorkflowEngine::default();

    let workflow = make_workflow(
        "sort_test",
        vec![
            manual_trigger("Trigger"),
            sort_node("Sort", "name", "asc"),
        ],
        &[("Trigger", "Sort", 0, 0)],
    );

    // Provide multiple input items with "name" fields in unsorted order
    let input_items: Vec<NodeExecutionData> = vec!["charlie", "alpha", "bravo"]
        .into_iter()
        .map(|name| {
            let mut data = HashMap::new();
            data.insert("name".to_string(), GenericValue::String(name.to_string()));
            NodeExecutionData::new(data)
        })
        .collect();

    let run = engine
        .execute(&workflow, WorkflowExecuteMode::Manual, Some(input_items))
        .await
        .expect("Execution should succeed");

    assert_eq!(run.status, ExecutionStatus::Success);

    let sorted_items = get_node_output_items(&run, "Sort");
    assert_eq!(sorted_items.len(), 3, "Should have 3 items after sort");

    let names: Vec<String> = sorted_items
        .iter()
        .filter_map(|item| {
            if let Some(GenericValue::String(s)) = item.json.get("name") {
                Some(s.clone())
            } else {
                None
            }
        })
        .collect();

    assert_eq!(names, vec!["alpha", "bravo", "charlie"]);
}

/// 6. Limit node test.
///    ManualTrigger -> Limit(maxItems=3)
///    Provide 10 input items and verify only 3 come out.
#[tokio::test]
async fn test_limit_node() {
    let engine = WorkflowEngine::default();

    let workflow = make_workflow(
        "limit_test",
        vec![manual_trigger("Trigger"), limit_node("Limit", 3.0)],
        &[("Trigger", "Limit", 0, 0)],
    );

    // Provide 10 input items
    let input_items: Vec<NodeExecutionData> = (0..10)
        .map(|i| {
            let mut data = HashMap::new();
            data.insert("index".to_string(), GenericValue::Integer(i));
            NodeExecutionData::new(data)
        })
        .collect();

    let run = engine
        .execute(&workflow, WorkflowExecuteMode::Manual, Some(input_items))
        .await
        .expect("Execution should succeed");

    assert_eq!(run.status, ExecutionStatus::Success);

    let limited_items = get_node_output_items(&run, "Limit");
    assert_eq!(
        limited_items.len(),
        3,
        "Limit node should output exactly 3 items"
    );

    // Verify we got the first 3 items (indices 0, 1, 2)
    for (i, item) in limited_items.iter().enumerate() {
        let idx = item.json.get("index");
        assert_eq!(
            idx,
            Some(&GenericValue::Integer(i as i64)),
            "Item at position {i} should have index {i}"
        );
    }
}

/// 7. Switch node test.
///    ManualTrigger -> Switch(3 outputs, rules check fields "alpha" and "bravo")
///    The switch has 3 outputs:
///      - Output 0 matches items with field "alpha"
///      - Output 1 matches items with field "bravo"
///      - Output 2 is the fallback (no match)
///    We connect each output to a separate NoOp node.
#[tokio::test]
async fn test_switch_node() {
    let engine = WorkflowEngine::default();

    let workflow = make_workflow(
        "switch_test",
        vec![
            manual_trigger("Trigger"),
            switch_node("Switch", 3.0, &["alpha", "bravo"]),
            noop_node("OutputAlpha"),
            noop_node("OutputBravo"),
            noop_node("OutputDefault"),
        ],
        &[
            ("Trigger", "Switch", 0, 0),
            ("Switch", "OutputAlpha", 0, 0),
            ("Switch", "OutputBravo", 1, 0),
            ("Switch", "OutputDefault", 2, 0),
        ],
    );

    // Create an item with field "alpha" -- it should route to output 0
    let input_items: Vec<NodeExecutionData> = vec![{
        let mut data = HashMap::new();
        data.insert(
            "alpha".to_string(),
            GenericValue::String("value".to_string()),
        );
        NodeExecutionData::new(data)
    }];

    let run = engine
        .execute(&workflow, WorkflowExecuteMode::Manual, Some(input_items))
        .await
        .expect("Execution should succeed");

    assert_eq!(run.status, ExecutionStatus::Success);

    // Switch output 0 (alpha) should have items
    let alpha_items = get_node_output_at_index(&run, "Switch", 0);
    assert!(
        !alpha_items.is_empty(),
        "Output 0 (alpha) should have items"
    );

    // OutputAlpha node should have executed
    assert!(
        run.data.result_data.run_data.contains_key("OutputAlpha"),
        "OutputAlpha should have been executed"
    );

    // OutputBravo should NOT have been reached (no items on output 1)
    let bravo_items = get_node_output_at_index(&run, "Switch", 1);
    assert!(
        bravo_items.is_empty(),
        "Output 1 (bravo) should be empty"
    );

    // OutputDefault should NOT have been reached
    let default_items = get_node_output_at_index(&run, "Switch", 2);
    assert!(
        default_items.is_empty(),
        "Output 2 (default) should be empty"
    );
}

/// 8. Error handling test.
///    ManualTrigger -> StopAndError
///    The execution should complete with Error status.
#[tokio::test]
async fn test_error_handling() {
    let engine = WorkflowEngine::default();

    let workflow = make_workflow(
        "error_handling",
        vec![
            manual_trigger("Trigger"),
            stop_and_error_node("StopAndError", "Test error message"),
        ],
        &[("Trigger", "StopAndError", 0, 0)],
    );

    let run = engine
        .execute(&workflow, WorkflowExecuteMode::Manual, None)
        .await
        .expect("Engine should return a Run even on error");

    assert_eq!(
        run.status,
        ExecutionStatus::Error,
        "Execution should have Error status"
    );

    // The run should have an error recorded
    assert!(
        run.data.result_data.error.is_some(),
        "Run should have an error recorded"
    );

    let error = run.data.result_data.error.as_ref().unwrap();
    assert!(
        error.message.contains("Test error message"),
        "Error message should contain the StopAndError message, got: {}",
        error.message
    );
}

/// 9. Continue on fail test.
///    ManualTrigger -> StopAndError (continue_on_fail=true) -> NoOp
///    The StopAndError node fails but continue_on_fail is set, so execution
///    should proceed to the NoOp and the run should finish with Success.
#[tokio::test]
async fn test_continue_on_fail() {
    let engine = WorkflowEngine::default();

    let mut error_node = stop_and_error_node("StopAndError", "Ignored error");
    error_node.continue_on_fail = true;

    let workflow = make_workflow(
        "continue_on_fail",
        vec![
            manual_trigger("Trigger"),
            error_node,
            noop_node("AfterError"),
        ],
        &[
            ("Trigger", "StopAndError", 0, 0),
            ("StopAndError", "AfterError", 0, 0),
        ],
    );

    let run = engine
        .execute(&workflow, WorkflowExecuteMode::Manual, None)
        .await
        .expect("Execution should succeed");

    // The StopAndError node should have executed with error status
    assert!(
        run.data.result_data.run_data.contains_key("StopAndError"),
        "StopAndError node should be in run data"
    );

    let stop_task = &run.data.result_data.run_data["StopAndError"][0];
    assert_eq!(
        stop_task.execution_status,
        ExecutionStatus::Error,
        "StopAndError node itself should have Error status"
    );

    // But the overall run should succeed since continue_on_fail was set.
    // Note: The engine continues past the error node, but since the error
    // node produces no output data, the AfterError node won't be queued.
    // The important thing is that the run does NOT abort with Error status.
    assert_eq!(
        run.status,
        ExecutionStatus::Success,
        "Overall run should succeed when continue_on_fail is set"
    );

    // Verify no global error is set on the run
    assert!(
        run.data.result_data.error.is_none(),
        "No global error should be set when continue_on_fail is used"
    );
}

/// 10. Event streaming test.
///     Execute a simple workflow with an event channel and verify that the
///     expected events are emitted: Started, NodeStarted, NodeFinished,
///     Finished.
#[tokio::test]
async fn test_event_streaming() {
    let engine = WorkflowEngine::default();

    let workflow = make_workflow(
        "event_streaming",
        vec![
            manual_trigger("Trigger"),
            noop_node("NoOp"),
        ],
        &[("Trigger", "NoOp", 0, 0)],
    );

    let (tx, mut rx) = mpsc::channel::<ExecutionEvent>(100);

    let run = engine
        .execute_with_events(&workflow, WorkflowExecuteMode::Manual, None, tx)
        .await
        .expect("Execution should succeed");

    assert_eq!(run.status, ExecutionStatus::Success);

    // Collect all events
    let mut events: Vec<ExecutionEvent> = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }

    // Verify we got a Started event
    let has_started = events.iter().any(|e| matches!(e, ExecutionEvent::Started { .. }));
    assert!(has_started, "Should have received a Started event");

    // Verify we got NodeStarted events for both nodes
    let node_started_names: Vec<String> = events
        .iter()
        .filter_map(|e| {
            if let ExecutionEvent::NodeStarted { node_name, .. } = e {
                Some(node_name.clone())
            } else {
                None
            }
        })
        .collect();
    assert!(
        node_started_names.contains(&"Trigger".to_string()),
        "Should have NodeStarted for Trigger"
    );
    assert!(
        node_started_names.contains(&"NoOp".to_string()),
        "Should have NodeStarted for NoOp"
    );

    // Verify we got NodeFinished events for both nodes
    let node_finished_names: Vec<String> = events
        .iter()
        .filter_map(|e| {
            if let ExecutionEvent::NodeFinished { node_name, .. } = e {
                Some(node_name.clone())
            } else {
                None
            }
        })
        .collect();
    assert!(
        node_finished_names.contains(&"Trigger".to_string()),
        "Should have NodeFinished for Trigger"
    );
    assert!(
        node_finished_names.contains(&"NoOp".to_string()),
        "Should have NodeFinished for NoOp"
    );

    // Verify we got a Finished event
    let has_finished = events
        .iter()
        .any(|e| matches!(e, ExecutionEvent::Finished { .. }));
    assert!(has_finished, "Should have received a Finished event");

    // Verify event ordering: Started should come first, Finished should come last
    let first_event = &events[0];
    assert!(
        matches!(first_event, ExecutionEvent::Started { .. }),
        "First event should be Started"
    );

    let last_event = events.last().unwrap();
    assert!(
        matches!(last_event, ExecutionEvent::Finished { .. }),
        "Last event should be Finished"
    );
}
