//! Compiled workflow hot paths — JIT-compiled dispatch tables.
//!
//! When a workflow is activated, this module analyzes its DAG structure and
//! compiles the **static** portions into an optimized dispatch table:
//!
//! - Node routing: source → [(target_index, connection_type)] as flat arrays
//! - Executor dispatch: node_index → executor fn (direct, not HashMap lookup)
//! - Static parameters: baked as constants (not deserialized at runtime)
//!
//! Dynamic data (expressions, API responses, user input) still flows at runtime.
//!
//! # Architecture
//!
//! ```text
//! Workflow JSON/YAML (deploy time)
//!       │
//!       ▼
//! WorkflowHotPath::compile(&workflow, &registry)
//!       │  Topological sort → node index assignment
//!       │  Connection resolution → flat routing table
//!       │  Executor lookup → direct fn ptr (not HashMap)
//!       ▼
//! CompiledWorkflow {
//!     nodes: Vec<CompiledNode>,        // indexed by position
//!     routing: Vec<Vec<RouteEntry>>,   // routing[src_idx] → targets
//!     start_indices: Vec<usize>,       // entry points
//! }
//!       │
//!       ▼  (runtime)
//! execute_compiled() — walks routing table by index, no HashMap lookups
//! ```

use crate::executor::{NodeExecutor, NodeExecutorRegistry};
use n8n_workflow::{Node, NodeParameterValue, NodeParameters, Workflow};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// A compiled route entry: target node index + connection type + input index.
#[derive(Debug, Clone)]
pub struct RouteEntry {
    /// Index into `CompiledWorkflow::nodes`.
    pub target_idx: usize,
    /// Connection type at the target (e.g., "main").
    pub connection_type: String,
    /// Input index at the target node.
    pub input_index: usize,
}

/// A compiled node: the original node + its pre-resolved executor.
#[derive(Clone)]
pub struct CompiledNode {
    /// The original workflow node (still needed for parameters, credentials, etc.).
    pub node: Node,
    /// Index in the compiled workflow's node array.
    pub index: usize,
    /// Pre-resolved executor (Arc, not looked up per-execution).
    pub executor: Arc<dyn NodeExecutor>,
    /// Whether this node is disabled (skip at runtime).
    pub disabled: bool,
    /// Whether this is a trigger node (entry point).
    pub is_trigger: bool,
    /// Static parameter values that don't contain expressions.
    /// These are extracted once at compile time, not re-parsed per execution.
    pub static_params: NodeParameters,
}

/// A fully compiled workflow — all routing resolved to indices.
///
/// At runtime, the engine walks `routing[current_idx]` to find next nodes
/// instead of doing HashMap lookups by node name.
pub struct CompiledWorkflow {
    /// Workflow ID (for cache keying).
    pub workflow_id: String,
    /// Workflow name (metadata).
    pub workflow_name: String,
    /// All nodes, indexed by position.
    pub nodes: Vec<CompiledNode>,
    /// Routing table: `routing[source_node_idx]` → list of route entries.
    pub routing: Vec<Vec<RouteEntry>>,
    /// Indices of start/trigger nodes.
    pub start_indices: Vec<usize>,
    /// Name → index map (for initial lookups only, not used in hot path).
    name_to_idx: HashMap<String, usize>,
}

impl CompiledWorkflow {
    /// Compile a workflow into an optimized dispatch table.
    ///
    /// This resolves all node names to indices, pre-resolves executors,
    /// and builds flat routing arrays. Called once at activation time.
    pub fn compile(
        workflow: &Workflow,
        registry: &NodeExecutorRegistry,
    ) -> Result<Self, CompileError> {
        let node_count = workflow.nodes.len();

        // Step 1: Assign indices to nodes by name.
        let mut name_to_idx: HashMap<String, usize> = HashMap::with_capacity(node_count);
        let mut compiled_nodes: Vec<CompiledNode> = Vec::with_capacity(node_count);

        for (idx, node) in workflow.nodes.iter().enumerate() {
            // Resolve executor at compile time (not per-execution).
            let executor = registry.get(&node.node_type).ok_or_else(|| {
                CompileError::MissingExecutor {
                    node_name: node.name.clone(),
                    node_type: node.node_type.clone(),
                }
            })?;

            // Extract static parameters (those without `{{ }}` expressions).
            let static_params = extract_static_params(&node.parameters);

            let is_trigger = node.node_type.contains("Trigger")
                || node.node_type.contains("trigger")
                || node.node_type.contains("webhook");

            compiled_nodes.push(CompiledNode {
                node: node.clone(),
                index: idx,
                executor,
                disabled: node.disabled,
                is_trigger,
                static_params,
            });

            name_to_idx.insert(node.name.clone(), idx);
        }

        // Step 2: Build routing table (name-based connections → index-based).
        let mut routing: Vec<Vec<RouteEntry>> = vec![Vec::new(); node_count];

        for (source_name, node_conns) in &workflow.connections {
            let source_idx = match name_to_idx.get(source_name) {
                Some(&idx) => idx,
                None => {
                    warn!(source = source_name, "Connection references unknown source node, skipping");
                    continue;
                }
            };

            for (conn_type, by_index) in node_conns {
                for (_output_index, connections) in by_index.iter().enumerate() {
                    for conn in connections {
                        let target_idx = match name_to_idx.get(&conn.node) {
                            Some(&idx) => idx,
                            None => {
                                warn!(
                                    source = source_name,
                                    target = conn.node,
                                    "Connection references unknown target node, skipping"
                                );
                                continue;
                            }
                        };

                        routing[source_idx].push(RouteEntry {
                            target_idx,
                            connection_type: conn.connection_type.clone(),
                            input_index: conn.index,
                        });
                    }
                }
            }
        }

        // Step 3: Identify start nodes.
        let start_indices: Vec<usize> = compiled_nodes
            .iter()
            .filter(|cn| cn.is_trigger && !cn.disabled)
            .map(|cn| cn.index)
            .collect();

        // Fallback: nodes with no incoming connections.
        let start_indices = if start_indices.is_empty() {
            let has_incoming: std::collections::HashSet<usize> = routing
                .iter()
                .flat_map(|routes| routes.iter().map(|r| r.target_idx))
                .collect();

            compiled_nodes
                .iter()
                .filter(|cn| !cn.disabled && !has_incoming.contains(&cn.index))
                .map(|cn| cn.index)
                .collect()
        } else {
            start_indices
        };

        info!(
            workflow = workflow.name,
            nodes = node_count,
            routes = routing.iter().map(|r| r.len()).sum::<usize>(),
            starts = start_indices.len(),
            "Compiled workflow hot path"
        );

        Ok(Self {
            workflow_id: workflow.id.clone(),
            workflow_name: workflow.name.clone(),
            nodes: compiled_nodes,
            routing,
            start_indices,
            name_to_idx,
        })
    }

    /// Get a compiled node by index (hot path — no HashMap lookup).
    #[inline]
    pub fn node(&self, idx: usize) -> &CompiledNode {
        &self.nodes[idx]
    }

    /// Get outgoing routes from a node (hot path — direct array index).
    #[inline]
    pub fn routes_from(&self, idx: usize) -> &[RouteEntry] {
        &self.routing[idx]
    }

    /// Get start node indices.
    #[inline]
    pub fn start_nodes(&self) -> &[usize] {
        &self.start_indices
    }

    /// Look up a node index by name (fallback — not for hot path).
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.name_to_idx.get(name).copied()
    }

    /// Total number of compiled nodes.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Total number of route entries.
    pub fn route_count(&self) -> usize {
        self.routing.iter().map(|r| r.len()).sum()
    }
}

// ============================================================================
// Compiled workflow cache
// ============================================================================

/// Thread-safe cache of compiled workflows, keyed by workflow ID.
///
/// When a workflow is activated, it's compiled once and cached here.
/// Subsequent executions skip compilation entirely and use the cached
/// dispatch table directly.
pub struct CompiledWorkflowCache {
    cache: dashmap::DashMap<String, Arc<CompiledWorkflow>>,
}

impl CompiledWorkflowCache {
    /// Create a new empty cache.
    pub fn new() -> Self {
        Self {
            cache: dashmap::DashMap::new(),
        }
    }

    /// Compile and cache a workflow. Returns the compiled workflow.
    ///
    /// If already cached, returns the existing compiled workflow.
    /// To force recompilation (e.g., after workflow edit), call `invalidate` first.
    pub fn compile_and_cache(
        &self,
        workflow: &Workflow,
        registry: &NodeExecutorRegistry,
    ) -> Result<Arc<CompiledWorkflow>, CompileError> {
        // Check cache first.
        if let Some(existing) = self.cache.get(&workflow.id) {
            debug!(workflow = workflow.name, "Using cached compiled workflow");
            return Ok(existing.clone());
        }

        // Compile and insert.
        let compiled = Arc::new(CompiledWorkflow::compile(workflow, registry)?);
        self.cache.insert(workflow.id.clone(), compiled.clone());
        info!(
            workflow = workflow.name,
            nodes = compiled.node_count(),
            routes = compiled.route_count(),
            "Compiled and cached workflow hot path"
        );
        Ok(compiled)
    }

    /// Get a cached compiled workflow by ID.
    pub fn get(&self, workflow_id: &str) -> Option<Arc<CompiledWorkflow>> {
        self.cache.get(workflow_id).map(|v| v.clone())
    }

    /// Invalidate a cached workflow (e.g., after editing).
    pub fn invalidate(&self, workflow_id: &str) -> bool {
        self.cache.remove(workflow_id).is_some()
    }

    /// Number of cached workflows.
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Clear the entire cache.
    pub fn clear(&self) {
        self.cache.clear();
    }
}

impl Default for CompiledWorkflowCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors during workflow compilation.
#[derive(Debug, thiserror::Error)]
pub enum CompileError {
    #[error("No executor found for node '{node_name}' (type: {node_type})")]
    MissingExecutor {
        node_name: String,
        node_type: String,
    },

    #[error("Workflow has no start nodes")]
    NoStartNodes,

    #[error("Compilation error: {0}")]
    Other(String),
}

// ============================================================================
// Internal helpers
// ============================================================================

/// Extract parameters that are static (no `{{ }}` expressions).
/// Returns NodeParameters directly — no serde_json conversion.
fn extract_static_params(params: &NodeParameters) -> NodeParameters {
    params
        .iter()
        .filter(|(_, v)| !npv_contains_expression(v))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// Check if a NodeParameterValue contains an n8n `{{ }}` expression.
fn npv_contains_expression(value: &NodeParameterValue) -> bool {
    match value {
        NodeParameterValue::String(s) => s.contains("{{") && s.contains("}}"),
        NodeParameterValue::Expression(_) => true,
        NodeParameterValue::Array(arr) => arr.iter().any(npv_contains_expression),
        NodeParameterValue::Object(map) => map.values().any(npv_contains_expression),
        NodeParameterValue::Number(_) | NodeParameterValue::Boolean(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use n8n_workflow::{WorkflowBuilder, Node};

    #[test]
    fn test_compile_simple_workflow() {
        let workflow = WorkflowBuilder::new("test")
            .node(Node::new("Start", "n8n-nodes-base.manualTrigger"))
            .node(Node::new("Process", "n8n-nodes-base.set"))
            .connect("Start", "Process", 0, 0)
            .expect("connect failed")
            .build()
            .expect("build failed");

        let registry = NodeExecutorRegistry::new();
        let compiled = CompiledWorkflow::compile(&workflow, &registry).unwrap();

        assert_eq!(compiled.node_count(), 2);
        assert_eq!(compiled.start_nodes().len(), 1);
        assert_eq!(compiled.routes_from(compiled.start_nodes()[0]).len(), 1);
    }

    #[test]
    fn test_static_param_extraction() {
        let mut params: NodeParameters = HashMap::new();
        params.insert(
            "url".to_string(),
            NodeParameterValue::String("https://example.com".to_string()),
        );
        params.insert(
            "dynamic".to_string(),
            NodeParameterValue::String("{{ $json.url }}".to_string()),
        );

        let static_params = extract_static_params(&params);
        assert_eq!(static_params.len(), 1);
        assert!(static_params.contains_key("url"));
        assert!(!static_params.contains_key("dynamic"));
    }

    #[test]
    fn test_static_param_nan() {
        let mut params: NodeParameters = HashMap::new();
        params.insert(
            "threshold".to_string(),
            NodeParameterValue::Number(f64::NAN),
        );
        params.insert(
            "limit".to_string(),
            NodeParameterValue::Number(42.0),
        );
        params.insert(
            "infinity".to_string(),
            NodeParameterValue::Number(f64::INFINITY),
        );

        let static_params = extract_static_params(&params);
        // All numeric values survive — no serde_json conversion to lose NaN/Inf.
        assert_eq!(static_params.len(), 3);
        match &static_params["threshold"] {
            NodeParameterValue::Number(n) => assert!(n.is_nan()),
            other => panic!("expected Number, got {:?}", other),
        }
        match &static_params["limit"] {
            NodeParameterValue::Number(n) => assert_eq!(*n, 42.0),
            other => panic!("expected Number, got {:?}", other),
        }
        match &static_params["infinity"] {
            NodeParameterValue::Number(n) => assert!(n.is_infinite()),
            other => panic!("expected Number, got {:?}", other),
        }
    }
}
