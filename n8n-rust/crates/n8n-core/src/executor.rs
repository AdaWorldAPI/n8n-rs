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
        input: &TaskDataConnections,
        _context: &RuntimeContext,
    ) -> Result<NodeOutput, ExecutionEngineError> {
        // Pass through input items if provided, otherwise emit a single empty item
        let items = input
            .get("main")
            .and_then(|v| v.first())
            .filter(|items| !items.is_empty())
            .cloned()
            .unwrap_or_else(|| vec![NodeExecutionData::default()]);
        Ok(vec![items])
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

/// HTTP Request node - makes real HTTP requests using reqwest.
pub struct HttpRequestExecutor;

impl HttpRequestExecutor {
    /// Extract a string parameter from node parameters with a default.
    fn get_string_param<'a>(node: &'a Node, key: &str, default: &'a str) -> String {
        node.parameters
            .get(key)
            .and_then(|v| {
                if let n8n_workflow::NodeParameterValue::String(s) = v {
                    Some(s.clone())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| default.to_string())
    }

    /// Extract a number parameter from node parameters with a default.
    fn get_number_param(node: &Node, key: &str, default: f64) -> f64 {
        node.parameters
            .get(key)
            .and_then(|v| {
                if let n8n_workflow::NodeParameterValue::Number(n) = v {
                    Some(*n)
                } else {
                    None
                }
            })
            .unwrap_or(default)
    }

    /// Convert a serde_json::Value into a GenericValue.
    fn json_to_generic(value: serde_json::Value) -> n8n_workflow::GenericValue {
        match value {
            serde_json::Value::Null => n8n_workflow::GenericValue::Null,
            serde_json::Value::Bool(b) => n8n_workflow::GenericValue::Bool(b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    n8n_workflow::GenericValue::Integer(i)
                } else if let Some(f) = n.as_f64() {
                    n8n_workflow::GenericValue::Float(f)
                } else {
                    n8n_workflow::GenericValue::Null
                }
            }
            serde_json::Value::String(s) => n8n_workflow::GenericValue::String(s),
            serde_json::Value::Array(arr) => {
                n8n_workflow::GenericValue::Array(arr.into_iter().map(Self::json_to_generic).collect())
            }
            serde_json::Value::Object(obj) => {
                let map: DataObject = obj
                    .into_iter()
                    .map(|(k, v)| (k, Self::json_to_generic(v)))
                    .collect();
                n8n_workflow::GenericValue::Object(map)
            }
        }
    }

    /// Build a reqwest::Client with the configured timeout.
    fn build_client(timeout_ms: u64) -> Result<reqwest::Client, ExecutionEngineError> {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(timeout_ms))
            .build()
            .map_err(|e| ExecutionEngineError::Internal(format!("Failed to build HTTP client: {}", e)))
    }

    /// Build the request from node parameters.
    fn build_request(
        client: &reqwest::Client,
        node: &Node,
    ) -> Result<reqwest::RequestBuilder, ExecutionEngineError> {
        let url = Self::get_string_param(node, "url", "");
        if url.is_empty() {
            return Err(ExecutionEngineError::NodeExecution {
                node: node.name.clone(),
                message: "URL parameter is required for HTTP Request node".to_string(),
            });
        }

        let method_str = Self::get_string_param(node, "method", "GET").to_uppercase();
        let method = match method_str.as_str() {
            "GET" => reqwest::Method::GET,
            "POST" => reqwest::Method::POST,
            "PUT" => reqwest::Method::PUT,
            "PATCH" => reqwest::Method::PATCH,
            "DELETE" => reqwest::Method::DELETE,
            "HEAD" => reqwest::Method::HEAD,
            "OPTIONS" => reqwest::Method::OPTIONS,
            other => {
                return Err(ExecutionEngineError::NodeExecution {
                    node: node.name.clone(),
                    message: format!("Unsupported HTTP method: {}", other),
                });
            }
        };

        let mut request = client.request(method.clone(), &url);

        // Apply request headers
        if let Some(n8n_workflow::NodeParameterValue::Object(headers)) = node.parameters.get("headers") {
            for (key, val) in headers {
                if let n8n_workflow::NodeParameterValue::String(v) = val {
                    request = request.header(key.as_str(), v.as_str());
                }
            }
        }

        // Apply request body for methods that support it
        if matches!(method, reqwest::Method::POST | reqwest::Method::PUT | reqwest::Method::PATCH) {
            if let Some(body_param) = node.parameters.get("body") {
                match body_param {
                    n8n_workflow::NodeParameterValue::String(s) => {
                        // If no Content-Type header was set, try to detect JSON
                        let has_content_type = node
                            .parameters
                            .get("headers")
                            .and_then(|h| {
                                if let n8n_workflow::NodeParameterValue::Object(map) = h {
                                    map.keys().any(|k| k.to_lowercase() == "content-type").then_some(true)
                                } else {
                                    None
                                }
                            })
                            .is_some();

                        if !has_content_type {
                            // Auto-detect: if body looks like JSON, set content-type
                            let trimmed = s.trim();
                            if (trimmed.starts_with('{') && trimmed.ends_with('}'))
                                || (trimmed.starts_with('[') && trimmed.ends_with(']'))
                            {
                                request = request.header("Content-Type", "application/json");
                            }
                        }
                        request = request.body(s.clone());
                    }
                    n8n_workflow::NodeParameterValue::Object(map) => {
                        // Convert NodeParameterValue::Object to serde_json::Value for JSON body
                        let json_val = Self::param_object_to_json(map);
                        request = request.json(&json_val);
                    }
                    _ => {}
                }
            }
        }

        Ok(request)
    }

    /// Convert a NodeParameterValue Object map to serde_json::Value.
    fn param_object_to_json(
        map: &HashMap<String, n8n_workflow::NodeParameterValue>,
    ) -> serde_json::Value {
        let obj: serde_json::Map<String, serde_json::Value> = map
            .iter()
            .map(|(k, v)| (k.clone(), Self::param_value_to_json(v)))
            .collect();
        serde_json::Value::Object(obj)
    }

    /// Convert a single NodeParameterValue to serde_json::Value.
    fn param_value_to_json(val: &n8n_workflow::NodeParameterValue) -> serde_json::Value {
        match val {
            n8n_workflow::NodeParameterValue::String(s) => serde_json::Value::String(s.clone()),
            n8n_workflow::NodeParameterValue::Number(n) => {
                serde_json::Value::Number(serde_json::Number::from_f64(*n).unwrap_or_else(|| serde_json::Number::from(0)))
            }
            n8n_workflow::NodeParameterValue::Boolean(b) => serde_json::Value::Bool(*b),
            n8n_workflow::NodeParameterValue::Array(arr) => {
                serde_json::Value::Array(arr.iter().map(Self::param_value_to_json).collect())
            }
            n8n_workflow::NodeParameterValue::Object(map) => Self::param_object_to_json(map),
            n8n_workflow::NodeParameterValue::Expression(s) => serde_json::Value::String(s.clone()),
        }
    }

    /// Check whether the fullResponse option is enabled.
    fn is_full_response(node: &Node) -> bool {
        // Check in options.fullResponse or options.response.fullResponse
        if let Some(n8n_workflow::NodeParameterValue::Object(options)) = node.parameters.get("options") {
            if let Some(n8n_workflow::NodeParameterValue::Boolean(full)) = options.get("fullResponse") {
                return *full;
            }
            // n8n also nests under response sub-object
            if let Some(n8n_workflow::NodeParameterValue::Object(resp_opts)) = options.get("response") {
                if let Some(n8n_workflow::NodeParameterValue::Boolean(full)) = resp_opts.get("fullResponse") {
                    return *full;
                }
            }
        }
        false
    }

    /// Process the HTTP response into a DataObject.
    async fn process_response(
        node: &Node,
        response: reqwest::Response,
    ) -> Result<DataObject, ExecutionEngineError> {
        let status_code = response.status().as_u16() as i64;
        let full_response = Self::is_full_response(node);

        // Collect response headers
        let mut resp_headers = DataObject::new();
        for (name, value) in response.headers().iter() {
            if let Ok(v) = value.to_str() {
                resp_headers.insert(
                    name.as_str().to_string(),
                    n8n_workflow::GenericValue::String(v.to_string()),
                );
            }
        }

        let response_format = Self::get_string_param(node, "responseFormat", "autodetect");

        // Determine how to parse the response body
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_lowercase();

        let body_value = match response_format.as_str() {
            "json" => {
                // Force parse as JSON
                let text = response.text().await.map_err(|e| {
                    ExecutionEngineError::NodeExecution {
                        node: node.name.clone(),
                        message: format!("Failed to read response body: {}", e),
                    }
                })?;
                match serde_json::from_str::<serde_json::Value>(&text) {
                    Ok(json_val) => Self::json_to_generic(json_val),
                    Err(_) => {
                        // If JSON parsing fails, return as string
                        n8n_workflow::GenericValue::String(text)
                    }
                }
            }
            "text" => {
                let text = response.text().await.map_err(|e| {
                    ExecutionEngineError::NodeExecution {
                        node: node.name.clone(),
                        message: format!("Failed to read response body: {}", e),
                    }
                })?;
                n8n_workflow::GenericValue::String(text)
            }
            _ => {
                // "autodetect" or unspecified - detect from Content-Type
                let text = response.text().await.map_err(|e| {
                    ExecutionEngineError::NodeExecution {
                        node: node.name.clone(),
                        message: format!("Failed to read response body: {}", e),
                    }
                })?;
                if content_type.contains("application/json") || content_type.contains("+json") {
                    match serde_json::from_str::<serde_json::Value>(&text) {
                        Ok(json_val) => Self::json_to_generic(json_val),
                        Err(_) => n8n_workflow::GenericValue::String(text),
                    }
                } else {
                    n8n_workflow::GenericValue::String(text)
                }
            }
        };

        let mut result = DataObject::new();

        if full_response {
            // Full response mode: include statusCode, headers, body
            result.insert(
                "statusCode".to_string(),
                n8n_workflow::GenericValue::Integer(status_code),
            );
            result.insert(
                "headers".to_string(),
                n8n_workflow::GenericValue::Object(resp_headers),
            );
            result.insert("body".to_string(), body_value);
        } else {
            // Default mode: merge body into the result directly if it's an object,
            // otherwise put it in a "data" field
            match body_value {
                n8n_workflow::GenericValue::Object(obj) => {
                    for (k, v) in obj {
                        result.insert(k, v);
                    }
                }
                other => {
                    result.insert("data".to_string(), other);
                }
            }
            // Always include statusCode at the top level for convenience
            result.insert(
                "statusCode".to_string(),
                n8n_workflow::GenericValue::Integer(status_code),
            );
        }

        Ok(result)
    }
}

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

        // Extract timeout (default 10000ms = 10 seconds)
        let timeout_ms = Self::get_number_param(node, "timeout", 10000.0) as u64;

        // Build a shared client for all items in this execution
        let client = Self::build_client(timeout_ms)?;

        let mut output = Vec::new();
        let cancel_token = context.cancellation_token();

        for (_idx, _item) in items.iter().enumerate() {
            // Check for cancellation before each request
            if context.is_canceled() {
                return Err(ExecutionEngineError::Canceled);
            }

            // Build the request
            let request = Self::build_request(&client, node)?;

            // Execute the request with cancellation support
            let response = tokio::select! {
                result = request.send() => {
                    result.map_err(|e| {
                        if e.is_timeout() {
                            ExecutionEngineError::NodeExecution {
                                node: node.name.clone(),
                                message: format!("HTTP request timed out after {}ms", timeout_ms),
                            }
                        } else if e.is_connect() {
                            ExecutionEngineError::NodeExecution {
                                node: node.name.clone(),
                                message: format!("Failed to connect: {}", e),
                            }
                        } else {
                            ExecutionEngineError::NodeExecution {
                                node: node.name.clone(),
                                message: format!("HTTP request failed: {}", e),
                            }
                        }
                    })?
                }
                _ = cancel_token.cancelled() => {
                    return Err(ExecutionEngineError::Canceled);
                }
            };

            // Check for HTTP error status codes (4xx/5xx) - log but don't fail
            // n8n by default does not error on non-2xx unless configured to do so
            let should_error_on_status = node
                .parameters
                .get("options")
                .and_then(|v| {
                    if let n8n_workflow::NodeParameterValue::Object(opts) = v {
                        opts.get("neverError").and_then(|ne| {
                            if let n8n_workflow::NodeParameterValue::Boolean(b) = ne {
                                Some(*b)
                            } else {
                                None
                            }
                        })
                    } else {
                        None
                    }
                })
                // By default, n8n does NOT throw on non-2xx (neverError = true behavior)
                .unwrap_or(true);

            let status = response.status();
            if !should_error_on_status && status.is_client_error() || status.is_server_error() {
                if !should_error_on_status {
                    return Err(ExecutionEngineError::NodeExecution {
                        node: node.name.clone(),
                        message: format!("HTTP request returned status {}", status.as_u16()),
                    });
                }
            }

            // Process the response
            let result = Self::process_response(node, response).await?;
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
