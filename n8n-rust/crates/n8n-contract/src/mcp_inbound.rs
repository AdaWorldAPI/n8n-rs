//! MCP Inbound Tool Registry — n8n integrations as MCP tool definitions.
//!
//! n8n's 400+ integrations become MCP tool definitions routed through
//! crewai-rust. Each tool definition IS the node type. No TypeScript wrapper.
//!
//! ```text
//! MCP Client → McpToolRegistry → lookup tool → crewai-rust routes
//!   ├── Tool "http_request"     → HttpRequestExecutor
//!   ├── Tool "slack_message"    → CrewAgentExecutor → crew.slack
//!   ├── Tool "neo4j_query"      → LadybugResonateExecutor → lb.cypher
//!   └── Tool "graph_traverse"   → LadybugCollapseExecutor → lb.traverse
//! ```

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

// =============================================================================
// MCP TOOL DEFINITION
// =============================================================================

/// An MCP tool definition — the contract between client and server.
///
/// Each tool maps to an n8n node type. When an MCP client calls a tool,
/// we route it through the appropriate executor (n8n, crew, or ladybug).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDefinition {
    /// Unique tool name (e.g., "http_request", "slack_send", "neo4j_query")
    pub name: String,

    /// Human-readable description
    pub description: String,

    /// JSON Schema for the tool's input parameters
    pub input_schema: serde_json::Value,

    /// Which routing domain handles this tool
    pub routing: ToolRouting,

    /// Required capabilities (auth, network, etc.)
    pub capabilities: Vec<String>,

    /// Whether this tool can modify state
    pub is_mutation: bool,
}

/// Where a tool's execution is routed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolRouting {
    /// Standard n8n node executor (n8n.* prefix)
    N8n { node_type: String },

    /// crewai-rust agent delegation (crew.* prefix)
    Crew { agent_type: String, task_template: Option<String> },

    /// ladybug-rs BindSpace operation (lb.* prefix)
    Ladybug { operation: LadybugOp },
}

/// Ladybug-rs operations exposed as MCP tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LadybugOp {
    /// Execute a Cypher query
    CypherQuery,
    /// Traverse the graph (BFS/DFS)
    Traverse,
    /// Search by fingerprint similarity (Hamming)
    Resonate,
    /// Read a node by address
    ReadNode,
    /// Write a node
    WriteNode,
    /// Bind two nodes via verb
    Bind,
    /// Get BindSpace statistics
    Stats,
}

// =============================================================================
// TOOL REGISTRY
// =============================================================================

/// Registry of all available MCP tools.
///
/// Populated at startup from n8n node definitions, crewai agent cards,
/// and ladybug-rs capabilities.
#[derive(Debug, Clone)]
pub struct McpToolRegistry {
    tools: HashMap<String, McpToolDefinition>,
}

impl McpToolRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Create a registry pre-populated with default tools.
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register_defaults();
        registry
    }

    /// Register a tool.
    pub fn register(&mut self, tool: McpToolDefinition) {
        self.tools.insert(tool.name.clone(), tool);
    }

    /// Look up a tool by name.
    pub fn get(&self, name: &str) -> Option<&McpToolDefinition> {
        self.tools.get(name)
    }

    /// List all registered tool names.
    pub fn list(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.tools.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }

    /// Number of registered tools.
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Register the default ladybug-rs and core n8n tools.
    fn register_defaults(&mut self) {
        // --- Ladybug-rs tools ---

        self.register(McpToolDefinition {
            name: "neo4j_query".to_string(),
            description: "Execute a Cypher query against the graph database".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Cypher query to execute" },
                    "params": { "type": "object", "description": "Query parameters" }
                },
                "required": ["query"]
            }),
            routing: ToolRouting::Ladybug { operation: LadybugOp::CypherQuery },
            capabilities: vec!["graph".to_string()],
            is_mutation: false,
        });

        self.register(McpToolDefinition {
            name: "graph_traverse".to_string(),
            description: "Traverse the graph from a starting node".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "start": { "type": "string", "description": "Starting node address (hex)" },
                    "depth": { "type": "integer", "description": "Max traversal depth" },
                    "direction": { "type": "string", "enum": ["outgoing", "incoming", "both"] }
                },
                "required": ["start"]
            }),
            routing: ToolRouting::Ladybug { operation: LadybugOp::Traverse },
            capabilities: vec!["graph".to_string()],
            is_mutation: false,
        });

        self.register(McpToolDefinition {
            name: "graph_resonate".to_string(),
            description: "Find similar nodes by fingerprint (Hamming distance)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Text to encode and search" },
                    "threshold": { "type": "number", "description": "Max Hamming distance (0.0-1.0)" },
                    "top_k": { "type": "integer", "description": "Max results to return" }
                },
                "required": ["query"]
            }),
            routing: ToolRouting::Ladybug { operation: LadybugOp::Resonate },
            capabilities: vec!["graph".to_string(), "simd".to_string()],
            is_mutation: false,
        });

        self.register(McpToolDefinition {
            name: "graph_stats".to_string(),
            description: "Get graph database statistics (node count, edge count, etc.)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
            routing: ToolRouting::Ladybug { operation: LadybugOp::Stats },
            capabilities: vec![],
            is_mutation: false,
        });

        // --- Core n8n tools ---

        self.register(McpToolDefinition {
            name: "http_request".to_string(),
            description: "Make an HTTP request to any URL".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "URL to request" },
                    "method": { "type": "string", "enum": ["GET", "POST", "PUT", "DELETE", "PATCH"] },
                    "headers": { "type": "object" },
                    "body": { "type": "string" }
                },
                "required": ["url", "method"]
            }),
            routing: ToolRouting::N8n { node_type: "n8n-nodes-base.httpRequest".to_string() },
            capabilities: vec!["network".to_string()],
            is_mutation: true,
        });

        self.register(McpToolDefinition {
            name: "code_execute".to_string(),
            description: "Execute a code snippet".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "code": { "type": "string", "description": "Code to execute" },
                    "language": { "type": "string", "enum": ["javascript", "python"] }
                },
                "required": ["code"]
            }),
            routing: ToolRouting::N8n { node_type: "n8n-nodes-base.code".to_string() },
            capabilities: vec!["execute".to_string()],
            is_mutation: true,
        });
    }
}

// =============================================================================
// TOOL CALL & RESULT
// =============================================================================

/// An inbound MCP tool call request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolCall {
    pub tool_name: String,
    pub arguments: serde_json::Value,
    pub call_id: Option<String>,
}

/// Result of an MCP tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolResult {
    pub content: serde_json::Value,
    pub is_error: bool,
    pub call_id: Option<String>,
}

impl McpToolResult {
    pub fn success(content: serde_json::Value) -> Self {
        Self { content, is_error: false, call_id: None }
    }

    pub fn error(message: &str) -> Self {
        Self {
            content: serde_json::json!({ "error": message }),
            is_error: true,
            call_id: None,
        }
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_defaults() {
        let reg = McpToolRegistry::with_defaults();
        assert!(reg.len() >= 6, "Should have at least 6 default tools");
        assert!(reg.get("neo4j_query").is_some());
        assert!(reg.get("graph_traverse").is_some());
        assert!(reg.get("http_request").is_some());
    }

    #[test]
    fn test_registry_list() {
        let reg = McpToolRegistry::with_defaults();
        let names = reg.list();
        assert!(names.contains(&"neo4j_query"));
        assert!(names.contains(&"graph_stats"));
    }

    #[test]
    fn test_tool_routing() {
        let reg = McpToolRegistry::with_defaults();
        let neo4j = reg.get("neo4j_query").unwrap();
        match &neo4j.routing {
            ToolRouting::Ladybug { operation: LadybugOp::CypherQuery } => {}
            other => panic!("Expected Ladybug CypherQuery, got {:?}", other),
        }
    }

    #[test]
    fn test_custom_tool_registration() {
        let mut reg = McpToolRegistry::new();
        reg.register(McpToolDefinition {
            name: "custom_tool".to_string(),
            description: "A custom tool".to_string(),
            input_schema: serde_json::json!({}),
            routing: ToolRouting::Crew {
                agent_type: "researcher".to_string(),
                task_template: None,
            },
            capabilities: vec![],
            is_mutation: false,
        });
        assert_eq!(reg.len(), 1);
        assert!(reg.get("custom_tool").is_some());
    }

    #[test]
    fn test_tool_result() {
        let ok = McpToolResult::success(serde_json::json!({"data": 42}));
        assert!(!ok.is_error);

        let err = McpToolResult::error("something broke");
        assert!(err.is_error);
    }
}
