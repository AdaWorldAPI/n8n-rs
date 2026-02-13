//! Node executor trait and implementations.

use crate::error::ExecutionEngineError;
use crate::runtime::RuntimeContext;
use async_trait::async_trait;
use n8n_workflow::{DataObject, Node, NodeExecutionData, TaskDataConnections};
use std::collections::HashMap;
use std::sync::Arc;

/// Result of node execution.
pub type NodeOutput = Vec<Vec<NodeExecutionData>>;

/// Trait for executing nodes.
#[async_trait]
pub trait NodeExecutor: Send + Sync {
    /// Get the node type this executor handles.
    fn node_type(&self) -> &str;

    /// Execute the node with the given input data.
    async fn execute(
        &self,
        node: &Node,
        input: &TaskDataConnections,
        context: &RuntimeContext,
    ) -> Result<NodeOutput, ExecutionEngineError>;
}

/// Registry of node executors.
pub struct NodeExecutorRegistry {
    executors: HashMap<String, Arc<dyn NodeExecutor>>,
}

impl NodeExecutorRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            executors: HashMap::new(),
        };

        // Register built-in executors
        registry.register(Arc::new(ManualTriggerExecutor));
        registry.register(Arc::new(ScheduleTriggerExecutor));
        registry.register(Arc::new(WebhookTriggerExecutor));
        registry.register(Arc::new(SetExecutor));
        registry.register(Arc::new(CodeExecutor));
        registry.register(Arc::new(IfExecutor));
        registry.register(Arc::new(MergeExecutor));
        registry.register(Arc::new(NoOpExecutor));
        registry.register(Arc::new(HttpRequestExecutor));

        // P0 Flow Control nodes
        registry.register(Arc::new(SwitchExecutor));
        registry.register(Arc::new(FilterExecutor));
        registry.register(Arc::new(SortExecutor));
        registry.register(Arc::new(LimitExecutor));
        registry.register(Arc::new(RemoveDuplicatesExecutor));
        registry.register(Arc::new(AggregateExecutor));
        registry.register(Arc::new(SplitInBatchesExecutor));
        registry.register(Arc::new(WaitExecutor));
        registry.register(Arc::new(StopAndErrorExecutor));
        registry.register(Arc::new(ExecuteWorkflowExecutor));

        registry
    }

    /// Register a node executor.
    pub fn register(&mut self, executor: Arc<dyn NodeExecutor>) {
        self.executors
            .insert(executor.node_type().to_string(), executor);
    }

    /// Get an executor for a node type.
    pub fn get(&self, node_type: &str) -> Option<Arc<dyn NodeExecutor>> {
        self.executors.get(node_type).cloned()
    }
}

impl Default for NodeExecutorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Built-in Node Executors
// ============================================================================

/// Manual trigger node - entry point for manual executions.
pub struct ManualTriggerExecutor;

#[async_trait]
impl NodeExecutor for ManualTriggerExecutor {
    fn node_type(&self) -> &str {
        "n8n-nodes-base.manualTrigger"
    }

    async fn execute(
        &self,
        _node: &Node,
        _input: &TaskDataConnections,
        _context: &RuntimeContext,
    ) -> Result<NodeOutput, ExecutionEngineError> {
        // Manual trigger just passes through an empty item
        Ok(vec![vec![NodeExecutionData::default()]])
    }
}

/// Schedule trigger node - triggers workflow on a schedule (cron).
///
/// When executed within a workflow context, this provides the trigger data
/// that was captured when the schedule fired.
pub struct ScheduleTriggerExecutor;

#[async_trait]
impl NodeExecutor for ScheduleTriggerExecutor {
    fn node_type(&self) -> &str {
        "n8n-nodes-base.scheduleTrigger"
    }

    async fn execute(
        &self,
        node: &Node,
        _input: &TaskDataConnections,
        _context: &RuntimeContext,
    ) -> Result<NodeOutput, ExecutionEngineError> {
        // Extract schedule info from node parameters
        let cron_expression = node
            .parameters
            .get("cronExpression")
            .and_then(|v| {
                if let n8n_workflow::NodeParameterValue::String(s) = v {
                    Some(s.clone())
                } else {
                    None
                }
            });

        // Create trigger output with schedule metadata
        let mut trigger_data = DataObject::new();
        trigger_data.insert(
            "timestamp".to_string(),
            n8n_workflow::GenericValue::Integer(chrono::Utc::now().timestamp_millis()),
        );
        trigger_data.insert(
            "timezone".to_string(),
            n8n_workflow::GenericValue::String("UTC".to_string()),
        );

        if let Some(cron) = cron_expression {
            trigger_data.insert(
                "cronExpression".to_string(),
                n8n_workflow::GenericValue::String(cron),
            );
        }

        // Add date/time components
        let now = chrono::Utc::now();
        trigger_data.insert(
            "date".to_string(),
            n8n_workflow::GenericValue::String(now.format("%Y-%m-%d").to_string()),
        );
        trigger_data.insert(
            "time".to_string(),
            n8n_workflow::GenericValue::String(now.format("%H:%M:%S").to_string()),
        );
        trigger_data.insert(
            "dayOfWeek".to_string(),
            n8n_workflow::GenericValue::Integer(now.format("%u").to_string().parse().unwrap_or(1)),
        );
        trigger_data.insert(
            "hour".to_string(),
            n8n_workflow::GenericValue::Integer(now.format("%H").to_string().parse().unwrap_or(0)),
        );
        trigger_data.insert(
            "minute".to_string(),
            n8n_workflow::GenericValue::Integer(now.format("%M").to_string().parse().unwrap_or(0)),
        );

        Ok(vec![vec![NodeExecutionData::new(trigger_data)]])
    }
}

/// Webhook trigger node - triggers workflow when HTTP request is received.
///
/// When executed within a workflow context, this provides the request data
/// that was captured when the webhook was called.
pub struct WebhookTriggerExecutor;

#[async_trait]
impl NodeExecutor for WebhookTriggerExecutor {
    fn node_type(&self) -> &str {
        "n8n-nodes-base.webhook"
    }

    async fn execute(
        &self,
        node: &Node,
        input: &TaskDataConnections,
        _context: &RuntimeContext,
    ) -> Result<NodeOutput, ExecutionEngineError> {
        // Check if webhook data was provided in input (from webhook handler)
        if let Some(main_input) = input.get("main").and_then(|v| v.first()) {
            if !main_input.is_empty() {
                // Webhook data was provided by the webhook handler
                return Ok(vec![main_input.clone()]);
            }
        }

        // No webhook data provided - this is a manual trigger or test
        // Create default webhook data structure
        let http_method = node
            .parameters
            .get("httpMethod")
            .and_then(|v| {
                if let n8n_workflow::NodeParameterValue::String(s) = v {
                    Some(s.clone())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "GET".to_string());

        let path = node
            .parameters
            .get("path")
            .and_then(|v| {
                if let n8n_workflow::NodeParameterValue::String(s) = v {
                    Some(s.clone())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "/webhook".to_string());

        let mut webhook_data = DataObject::new();

        // Headers
        let mut headers = DataObject::new();
        headers.insert("content-type".to_string(), "application/json".into());
        headers.insert("user-agent".to_string(), "n8n-test".into());
        webhook_data.insert("headers".to_string(), n8n_workflow::GenericValue::Object(headers));

        // Params
        webhook_data.insert("params".to_string(), n8n_workflow::GenericValue::Object(DataObject::new()));

        // Query
        webhook_data.insert("query".to_string(), n8n_workflow::GenericValue::Object(DataObject::new()));

        // Body (empty for test)
        webhook_data.insert("body".to_string(), n8n_workflow::GenericValue::Object(DataObject::new()));

        // Webhook metadata
        webhook_data.insert(
            "webhookUrl".to_string(),
            n8n_workflow::GenericValue::String(format!("/webhook{}", path)),
        );
        webhook_data.insert(
            "httpMethod".to_string(),
            n8n_workflow::GenericValue::String(http_method),
        );
        webhook_data.insert(
            "executionMode".to_string(),
            n8n_workflow::GenericValue::String("test".to_string()),
        );

        Ok(vec![vec![NodeExecutionData::new(webhook_data)]])
    }
}

/// Set node - set values on items.
pub struct SetExecutor;

#[async_trait]
impl NodeExecutor for SetExecutor {
    fn node_type(&self) -> &str {
        "n8n-nodes-base.set"
    }

    async fn execute(
        &self,
        node: &Node,
        input: &TaskDataConnections,
        _context: &RuntimeContext,
    ) -> Result<NodeOutput, ExecutionEngineError> {
        let main_input = input.get("main").and_then(|v| v.first());
        let items = main_input.cloned().unwrap_or_default();

        // Get values to set from parameters
        let values = node.parameters.get("values");

        let output: Vec<NodeExecutionData> = items
            .into_iter()
            .map(|mut item| {
                if let Some(n8n_workflow::NodeParameterValue::Object(vals)) = values {
                    for (key, val) in vals {
                        if let n8n_workflow::NodeParameterValue::String(s) = val {
                            item.json.insert(key.clone(), s.clone().into());
                        }
                    }
                }
                item
            })
            .collect();

        Ok(vec![output])
    }
}

/// Code node - execute custom code (placeholder).
pub struct CodeExecutor;

#[async_trait]
impl NodeExecutor for CodeExecutor {
    fn node_type(&self) -> &str {
        "n8n-nodes-base.code"
    }

    async fn execute(
        &self,
        _node: &Node,
        input: &TaskDataConnections,
        _context: &RuntimeContext,
    ) -> Result<NodeOutput, ExecutionEngineError> {
        // Placeholder - just pass through input
        let main_input = input.get("main").and_then(|v| v.first());
        Ok(vec![main_input.cloned().unwrap_or_default()])
    }
}

/// If node - conditional branching.
pub struct IfExecutor;

#[async_trait]
impl NodeExecutor for IfExecutor {
    fn node_type(&self) -> &str {
        "n8n-nodes-base.if"
    }

    async fn execute(
        &self,
        node: &Node,
        input: &TaskDataConnections,
        _context: &RuntimeContext,
    ) -> Result<NodeOutput, ExecutionEngineError> {
        let main_input = input.get("main").and_then(|v| v.first());
        let items = main_input.cloned().unwrap_or_default();

        let mut true_output = Vec::new();
        let mut false_output = Vec::new();

        // Simple condition check (placeholder logic)
        for item in items {
            // Check condition based on parameters
            let condition_met = check_condition(node, &item);
            if condition_met {
                true_output.push(item);
            } else {
                false_output.push(item);
            }
        }

        // Output 0 = true branch, Output 1 = false branch
        Ok(vec![true_output, false_output])
    }
}

fn check_condition(node: &Node, item: &NodeExecutionData) -> bool {
    // Simplified condition checking
    // In a real implementation, this would evaluate the condition expression
    if let Some(n8n_workflow::NodeParameterValue::Object(conditions)) = node.parameters.get("conditions") {
        if let Some(n8n_workflow::NodeParameterValue::String(field)) = conditions.get("field") {
            return item.json.contains_key(field);
        }
    }
    true
}

/// Merge node - combine multiple inputs.
pub struct MergeExecutor;

#[async_trait]
impl NodeExecutor for MergeExecutor {
    fn node_type(&self) -> &str {
        "n8n-nodes-base.merge"
    }

    async fn execute(
        &self,
        _node: &Node,
        input: &TaskDataConnections,
        _context: &RuntimeContext,
    ) -> Result<NodeOutput, ExecutionEngineError> {
        // Collect all inputs and merge them
        let mut merged = Vec::new();

        if let Some(main_inputs) = input.get("main") {
            for input_items in main_inputs {
                merged.extend(input_items.clone());
            }
        }

        Ok(vec![merged])
    }
}

/// No-op node - pass through without modification.
pub struct NoOpExecutor;

#[async_trait]
impl NodeExecutor for NoOpExecutor {
    fn node_type(&self) -> &str {
        "n8n-nodes-base.noOp"
    }

    async fn execute(
        &self,
        _node: &Node,
        input: &TaskDataConnections,
        _context: &RuntimeContext,
    ) -> Result<NodeOutput, ExecutionEngineError> {
        let main_input = input.get("main").and_then(|v| v.first());
        Ok(vec![main_input.cloned().unwrap_or_default()])
    }
}

/// HTTP Request node (placeholder).
pub struct HttpRequestExecutor;

#[async_trait]
impl NodeExecutor for HttpRequestExecutor {
    fn node_type(&self) -> &str {
        "n8n-nodes-base.httpRequest"
    }

    async fn execute(
        &self,
        _node: &Node,
        input: &TaskDataConnections,
        context: &RuntimeContext,
    ) -> Result<NodeOutput, ExecutionEngineError> {
        let main_input = input.get("main").and_then(|v| v.first());
        let items = main_input.cloned().unwrap_or_else(|| vec![NodeExecutionData::default()]);

        let mut output = Vec::new();

        for _item in items {
            // Check for cancellation
            if context.is_canceled() {
                return Err(ExecutionEngineError::Canceled);
            }

            // Placeholder - in real implementation, make HTTP request
            let mut result = DataObject::new();
            result.insert("status".to_string(), 200i64.into());
            result.insert("body".to_string(), "{}".into());

            output.push(NodeExecutionData::new(result));
        }

        Ok(vec![output])
    }
}

// ============================================================================
// P0 Flow Control Nodes
// ============================================================================

/// Switch node - route items to different outputs based on conditions.
pub struct SwitchExecutor;

#[async_trait]
impl NodeExecutor for SwitchExecutor {
    fn node_type(&self) -> &str {
        "n8n-nodes-base.switch"
    }

    async fn execute(
        &self,
        node: &Node,
        input: &TaskDataConnections,
        _context: &RuntimeContext,
    ) -> Result<NodeOutput, ExecutionEngineError> {
        let main_input = input.get("main").and_then(|v| v.first());
        let items = main_input.cloned().unwrap_or_default();

        // Get number of outputs from parameters (default 4)
        let num_outputs = node
            .parameters
            .get("numberOutputs")
            .and_then(|v| {
                if let n8n_workflow::NodeParameterValue::Number(n) = v {
                    Some(*n as usize)
                } else {
                    None
                }
            })
            .unwrap_or(4);

        // Initialize output arrays
        let mut outputs: Vec<Vec<NodeExecutionData>> = vec![Vec::new(); num_outputs];

        // Get routing rules
        let rules = node.parameters.get("rules");

        for item in items {
            let output_index = evaluate_switch_rules(&item, rules, num_outputs);
            if output_index < num_outputs {
                outputs[output_index].push(item);
            }
        }

        Ok(outputs)
    }
}

fn evaluate_switch_rules(
    item: &NodeExecutionData,
    rules: Option<&n8n_workflow::NodeParameterValue>,
    num_outputs: usize,
) -> usize {
    // Simplified rule evaluation - in real implementation would use expression evaluator
    if let Some(n8n_workflow::NodeParameterValue::Object(rules_obj)) = rules {
        if let Some(n8n_workflow::NodeParameterValue::Array(rule_list)) = rules_obj.get("rules") {
            for (i, rule) in rule_list.iter().enumerate() {
                if i >= num_outputs - 1 {
                    break;
                }
                if let n8n_workflow::NodeParameterValue::Object(rule_obj) = rule {
                    if let Some(n8n_workflow::NodeParameterValue::String(field)) =
                        rule_obj.get("field")
                    {
                        if item.json.contains_key(field) {
                            return i;
                        }
                    }
                }
            }
        }
    }
    // Default to last output (fallback/else)
    num_outputs.saturating_sub(1)
}

/// Filter node - filter items based on conditions.
pub struct FilterExecutor;

#[async_trait]
impl NodeExecutor for FilterExecutor {
    fn node_type(&self) -> &str {
        "n8n-nodes-base.filter"
    }

    async fn execute(
        &self,
        node: &Node,
        input: &TaskDataConnections,
        _context: &RuntimeContext,
    ) -> Result<NodeOutput, ExecutionEngineError> {
        let main_input = input.get("main").and_then(|v| v.first());
        let items = main_input.cloned().unwrap_or_default();

        let mut passed = Vec::new();
        let mut failed = Vec::new();

        // Get filter conditions
        let conditions = node.parameters.get("conditions");

        for item in items {
            if evaluate_filter_condition(&item, conditions) {
                passed.push(item);
            } else {
                failed.push(item);
            }
        }

        // Output 0 = passed, Output 1 = failed (optional)
        Ok(vec![passed, failed])
    }
}

fn evaluate_filter_condition(
    item: &NodeExecutionData,
    conditions: Option<&n8n_workflow::NodeParameterValue>,
) -> bool {
    if let Some(n8n_workflow::NodeParameterValue::Object(cond_obj)) = conditions {
        // Simplified condition evaluation
        if let Some(n8n_workflow::NodeParameterValue::String(field)) = cond_obj.get("field") {
            if let Some(value) = item.json.get(field) {
                // Check if value is truthy
                return match value {
                    n8n_workflow::GenericValue::Null => false,
                    n8n_workflow::GenericValue::Bool(b) => *b,
                    n8n_workflow::GenericValue::Integer(n) => *n != 0,
                    n8n_workflow::GenericValue::Float(f) => *f != 0.0,
                    n8n_workflow::GenericValue::String(s) => !s.is_empty(),
                    n8n_workflow::GenericValue::Array(arr) => !arr.is_empty(),
                    n8n_workflow::GenericValue::Object(_) => true,
                };
            }
        }
    }
    true // Default to passing if no conditions
}

/// Sort node - sort items by a field.
pub struct SortExecutor;

#[async_trait]
impl NodeExecutor for SortExecutor {
    fn node_type(&self) -> &str {
        "n8n-nodes-base.sort"
    }

    async fn execute(
        &self,
        node: &Node,
        input: &TaskDataConnections,
        _context: &RuntimeContext,
    ) -> Result<NodeOutput, ExecutionEngineError> {
        let main_input = input.get("main").and_then(|v| v.first());
        let mut items = main_input.cloned().unwrap_or_default();

        // Get sort field and order
        let sort_field = node
            .parameters
            .get("sortBy")
            .and_then(|v| {
                if let n8n_workflow::NodeParameterValue::String(s) = v {
                    Some(s.clone())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "id".to_string());

        let descending = node
            .parameters
            .get("order")
            .and_then(|v| {
                if let n8n_workflow::NodeParameterValue::String(s) = v {
                    Some(s == "desc" || s == "descending")
                } else {
                    None
                }
            })
            .unwrap_or(false);

        // Sort items
        items.sort_by(|a, b| {
            let val_a = a.json.get(&sort_field);
            let val_b = b.json.get(&sort_field);

            let ord = compare_values(val_a, val_b);
            if descending {
                ord.reverse()
            } else {
                ord
            }
        });

        Ok(vec![items])
    }
}

fn compare_values(
    a: Option<&n8n_workflow::GenericValue>,
    b: Option<&n8n_workflow::GenericValue>,
) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(n8n_workflow::GenericValue::Integer(a)), Some(n8n_workflow::GenericValue::Integer(b))) => a.cmp(b),
        (Some(n8n_workflow::GenericValue::Float(a)), Some(n8n_workflow::GenericValue::Float(b))) => {
            a.partial_cmp(b).unwrap_or(Ordering::Equal)
        }
        (Some(n8n_workflow::GenericValue::String(a)), Some(n8n_workflow::GenericValue::String(b))) => a.cmp(b),
        _ => Ordering::Equal,
    }
}

/// Limit node - limit number of items.
pub struct LimitExecutor;

#[async_trait]
impl NodeExecutor for LimitExecutor {
    fn node_type(&self) -> &str {
        "n8n-nodes-base.limit"
    }

    async fn execute(
        &self,
        node: &Node,
        input: &TaskDataConnections,
        _context: &RuntimeContext,
    ) -> Result<NodeOutput, ExecutionEngineError> {
        let main_input = input.get("main").and_then(|v| v.first());
        let items = main_input.cloned().unwrap_or_default();

        // Get limit value
        let limit = node
            .parameters
            .get("maxItems")
            .and_then(|v| {
                if let n8n_workflow::NodeParameterValue::Number(n) = v {
                    Some(*n as usize)
                } else {
                    None
                }
            })
            .unwrap_or(10);

        let limited: Vec<NodeExecutionData> = items.into_iter().take(limit).collect();

        Ok(vec![limited])
    }
}

/// RemoveDuplicates node - remove duplicate items.
pub struct RemoveDuplicatesExecutor;

#[async_trait]
impl NodeExecutor for RemoveDuplicatesExecutor {
    fn node_type(&self) -> &str {
        "n8n-nodes-base.removeDuplicates"
    }

    async fn execute(
        &self,
        node: &Node,
        input: &TaskDataConnections,
        _context: &RuntimeContext,
    ) -> Result<NodeOutput, ExecutionEngineError> {
        let main_input = input.get("main").and_then(|v| v.first());
        let items = main_input.cloned().unwrap_or_default();

        // Get field to check for duplicates
        let compare_field = node
            .parameters
            .get("compareField")
            .and_then(|v| {
                if let n8n_workflow::NodeParameterValue::String(s) = v {
                    Some(s.clone())
                } else {
                    None
                }
            });

        let mut seen = std::collections::HashSet::new();
        let mut unique = Vec::new();

        for item in items {
            let key = if let Some(ref field) = compare_field {
                // Compare by specific field
                item.json
                    .get(field)
                    .map(|v| format!("{:?}", v))
                    .unwrap_or_default()
            } else {
                // Compare entire JSON
                format!("{:?}", item.json)
            };

            if seen.insert(key) {
                unique.push(item);
            }
        }

        Ok(vec![unique])
    }
}

/// Aggregate node - aggregate items into groups.
pub struct AggregateExecutor;

#[async_trait]
impl NodeExecutor for AggregateExecutor {
    fn node_type(&self) -> &str {
        "n8n-nodes-base.aggregate"
    }

    async fn execute(
        &self,
        node: &Node,
        input: &TaskDataConnections,
        _context: &RuntimeContext,
    ) -> Result<NodeOutput, ExecutionEngineError> {
        let main_input = input.get("main").and_then(|v| v.first());
        let items = main_input.cloned().unwrap_or_default();

        // Get aggregation settings
        let aggregate_all = node
            .parameters
            .get("aggregateAllItemData")
            .and_then(|v| {
                if let n8n_workflow::NodeParameterValue::Boolean(b) = v {
                    Some(*b)
                } else {
                    None
                }
            })
            .unwrap_or(true);

        if aggregate_all {
            // Aggregate all items into a single item with an array
            let all_data: Vec<n8n_workflow::GenericValue> = items
                .into_iter()
                .map(|item| n8n_workflow::GenericValue::Object(item.json))
                .collect();

            let mut result = DataObject::new();
            result.insert(
                "data".to_string(),
                n8n_workflow::GenericValue::Array(all_data),
            );

            Ok(vec![vec![NodeExecutionData::new(result)]])
        } else {
            // Group by field
            let group_field = node
                .parameters
                .get("groupByField")
                .and_then(|v| {
                    if let n8n_workflow::NodeParameterValue::String(s) = v {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| "id".to_string());

            let mut groups: std::collections::HashMap<String, Vec<NodeExecutionData>> =
                std::collections::HashMap::new();

            for item in items {
                let key = item
                    .json
                    .get(&group_field)
                    .map(|v| format!("{:?}", v))
                    .unwrap_or_else(|| "default".to_string());
                groups.entry(key).or_default().push(item);
            }

            let output: Vec<NodeExecutionData> = groups
                .into_iter()
                .map(|(key, group_items)| {
                    let items_data: Vec<n8n_workflow::GenericValue> = group_items
                        .into_iter()
                        .map(|item| n8n_workflow::GenericValue::Object(item.json))
                        .collect();

                    let mut result = DataObject::new();
                    result.insert("groupKey".to_string(), key.into());
                    result.insert("items".to_string(), n8n_workflow::GenericValue::Array(items_data));
                    NodeExecutionData::new(result)
                })
                .collect();

            Ok(vec![output])
        }
    }
}

/// SplitInBatches node - split items into batches for loop processing.
pub struct SplitInBatchesExecutor;

#[async_trait]
impl NodeExecutor for SplitInBatchesExecutor {
    fn node_type(&self) -> &str {
        "n8n-nodes-base.splitInBatches"
    }

    async fn execute(
        &self,
        node: &Node,
        input: &TaskDataConnections,
        _context: &RuntimeContext,
    ) -> Result<NodeOutput, ExecutionEngineError> {
        let main_input = input.get("main").and_then(|v| v.first());
        let items = main_input.cloned().unwrap_or_default();

        // Get batch size
        let batch_size = node
            .parameters
            .get("batchSize")
            .and_then(|v| {
                if let n8n_workflow::NodeParameterValue::Number(n) = v {
                    Some((*n as usize).max(1))
                } else {
                    None
                }
            })
            .unwrap_or(10);

        // Split into batches
        let batches: Vec<Vec<NodeExecutionData>> = items
            .chunks(batch_size)
            .map(|chunk| chunk.to_vec())
            .collect();

        // For workflow looping, we output one batch at a time
        // In a real implementation, this would integrate with the execution engine
        // to iterate through batches
        if batches.is_empty() {
            Ok(vec![vec![]])
        } else {
            // Output first batch, with metadata about remaining
            let mut first_batch = batches.into_iter().next().unwrap();

            // Add batch metadata to first item
            if let Some(first_item) = first_batch.first_mut() {
                first_item.json.insert(
                    "__batchIndex".to_string(),
                    n8n_workflow::GenericValue::Integer(0),
                );
            }

            Ok(vec![first_batch])
        }
    }
}

/// Wait node - pause execution for a specified time.
pub struct WaitExecutor;

#[async_trait]
impl NodeExecutor for WaitExecutor {
    fn node_type(&self) -> &str {
        "n8n-nodes-base.wait"
    }

    async fn execute(
        &self,
        node: &Node,
        input: &TaskDataConnections,
        context: &RuntimeContext,
    ) -> Result<NodeOutput, ExecutionEngineError> {
        let main_input = input.get("main").and_then(|v| v.first());
        let items = main_input.cloned().unwrap_or_default();

        // Get wait time in milliseconds
        let wait_ms = node
            .parameters
            .get("amount")
            .and_then(|v| {
                if let n8n_workflow::NodeParameterValue::Number(n) = v {
                    Some(*n as u64)
                } else {
                    None
                }
            })
            .unwrap_or(1000);

        let unit = node
            .parameters
            .get("unit")
            .and_then(|v| {
                if let n8n_workflow::NodeParameterValue::String(s) = v {
                    Some(s.clone())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "milliseconds".to_string());

        let duration_ms = match unit.as_str() {
            "seconds" => wait_ms * 1000,
            "minutes" => wait_ms * 60 * 1000,
            "hours" => wait_ms * 60 * 60 * 1000,
            _ => wait_ms,
        };

        // Respect cancellation during wait
        let sleep_duration = std::time::Duration::from_millis(duration_ms);
        tokio::select! {
            _ = tokio::time::sleep(sleep_duration) => {}
            _ = async {
                while !context.is_canceled() {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            } => {
                return Err(ExecutionEngineError::Canceled);
            }
        }

        Ok(vec![items])
    }
}

/// StopAndError node - stop execution and throw an error.
pub struct StopAndErrorExecutor;

#[async_trait]
impl NodeExecutor for StopAndErrorExecutor {
    fn node_type(&self) -> &str {
        "n8n-nodes-base.stopAndError"
    }

    async fn execute(
        &self,
        node: &Node,
        _input: &TaskDataConnections,
        _context: &RuntimeContext,
    ) -> Result<NodeOutput, ExecutionEngineError> {
        let error_message = node
            .parameters
            .get("errorMessage")
            .and_then(|v| {
                if let n8n_workflow::NodeParameterValue::String(s) = v {
                    Some(s.clone())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "Workflow stopped by StopAndError node".to_string());

        Err(ExecutionEngineError::NodeExecution {
            node: node.name.clone(),
            message: error_message,
        })
    }
}

/// ExecuteWorkflow node - execute another workflow (placeholder).
pub struct ExecuteWorkflowExecutor;

#[async_trait]
impl NodeExecutor for ExecuteWorkflowExecutor {
    fn node_type(&self) -> &str {
        "n8n-nodes-base.executeWorkflow"
    }

    async fn execute(
        &self,
        _node: &Node,
        input: &TaskDataConnections,
        _context: &RuntimeContext,
    ) -> Result<NodeOutput, ExecutionEngineError> {
        // Placeholder - would need access to workflow repository and recursive execution
        let main_input = input.get("main").and_then(|v| v.first());
        Ok(vec![main_input.cloned().unwrap_or_default()])
    }
}
