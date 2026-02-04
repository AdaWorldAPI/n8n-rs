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
        registry.register(Arc::new(SetExecutor));
        registry.register(Arc::new(CodeExecutor));
        registry.register(Arc::new(IfExecutor));
        registry.register(Arc::new(MergeExecutor));
        registry.register(Arc::new(NoOpExecutor));
        registry.register(Arc::new(HttpRequestExecutor));

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
        node: &Node,
        input: &TaskDataConnections,
        context: &RuntimeContext,
    ) -> Result<NodeOutput, ExecutionEngineError> {
        let main_input = input.get("main").and_then(|v| v.first());
        let items = main_input.cloned().unwrap_or_else(|| vec![NodeExecutionData::default()]);

        let mut output = Vec::new();

        for item in items {
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
