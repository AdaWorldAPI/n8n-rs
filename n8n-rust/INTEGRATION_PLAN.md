# n8n-rust Integration Plan & Status Report

> **Project**: Complete Rust transcoding of n8n workflow automation platform
> **Status**: Phase 1 Complete (Core Infrastructure)
> **Date**: 2026-02-12

---

## Executive Summary

This document outlines the complete integration plan for transcoding n8n from TypeScript to Rust while maintaining 1:1 API compatibility and adding high-performance features (gRPC, Arrow Flight, Hamming vectors).

### Current Progress

| Component | Status | Completion |
|-----------|--------|------------|
| Core Types (Workflow, Node, Connection) | ✅ Complete | 100% |
| Execution Engine | ✅ Complete | 100% |
| PostgreSQL Persistence | ✅ Complete | 100% |
| Multi-Transport (REST/gRPC/Flight/STDIO) | ✅ Complete | 100% |
| Arrow Zero-Copy Integration | ✅ Complete | 100% |
| Hamming Vector Similarity | ✅ Complete | 100% |
| Node Executors (Built-in) | 🟡 Partial | 15% |
| Node Connectors (Integrations) | 🔴 Not Started | 0% |
| Expression Evaluation | 🔴 Not Started | 0% |
| Credential Encryption | 🔴 Not Started | 0% |
| Webhook Handling | 🔴 Not Started | 0% |
| 1:1 REST API Surface | 🔴 Not Started | 0% |

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              n8n-rust                                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                         API Surface (1:1)                            │    │
│  │  REST /api/v1/* │ Webhooks /webhook/* │ gRPC │ Flight │ STDIO       │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                    │                                         │
│  ┌─────────────────────────────────┴─────────────────────────────────┐      │
│  │                        Orchestrator Layer                          │      │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌───────────┐ │      │
│  │  │  Scheduler  │  │   Queue     │  │  Workers    │  │  Hooks    │ │      │
│  │  └─────────────┘  └─────────────┘  └─────────────┘  └───────────┘ │      │
│  └───────────────────────────────────────────────────────────────────┘      │
│                                    │                                         │
│  ┌─────────────────────────────────┴─────────────────────────────────┐      │
│  │                      Execution Engine                              │      │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌───────────┐ │      │
│  │  │ Stack-based │  │ Expression  │  │  Context    │  │  Error    │ │      │
│  │  │  Executor   │  │  Evaluator  │  │  Manager    │  │  Handler  │ │      │
│  │  └─────────────┘  └─────────────┘  └─────────────┘  └───────────┘ │      │
│  └───────────────────────────────────────────────────────────────────┘      │
│                                    │                                         │
│  ┌─────────────────────────────────┴─────────────────────────────────┐      │
│  │                         Node Layer                                 │      │
│  │  ┌─────────────────────────────────────────────────────────────┐  │      │
│  │  │                    Node Registry                             │  │      │
│  │  │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌───────┐  │  │      │
│  │  │  │ Trigger │ │ Action  │ │  Flow   │ │   AI    │ │ Custom│  │  │      │
│  │  │  │  Nodes  │ │  Nodes  │ │  Nodes  │ │  Nodes  │ │ Nodes │  │  │      │
│  │  │  └─────────┘ └─────────┘ └─────────┘ └─────────┘ └───────┘  │  │      │
│  │  └─────────────────────────────────────────────────────────────┘  │      │
│  └───────────────────────────────────────────────────────────────────┘      │
│                                    │                                         │
│  ┌─────────────────────────────────┴─────────────────────────────────┐      │
│  │                       Data Layer                                   │      │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌───────────┐ │      │
│  │  │ PostgreSQL  │  │   Arrow     │  │  Hamming    │  │  Binary   │ │      │
│  │  │    (n8n-db) │  │  (n8n-arrow)│  │ (n8n-hamming│  │  Storage  │ │      │
│  │  └─────────────┘  └─────────────┘  └─────────────┘  └───────────┘ │      │
│  └───────────────────────────────────────────────────────────────────┘      │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Phase 1: Core Infrastructure ✅ COMPLETE

### 1.1 Workflow Types (n8n-workflow)

| Type | Status | File |
|------|--------|------|
| `Workflow` | ✅ | `workflow.rs` |
| `Node` | ✅ | `node.rs` |
| `Connection` | ✅ | `connection.rs` |
| `NodeExecutionData` | ✅ | `data.rs` |
| `RunExecutionData` | ✅ | `execution.rs` |
| `TaskData` | ✅ | `execution.rs` |
| `WorkflowSettings` | ✅ | `workflow.rs` |
| `ExecutionStatus` | ✅ | `execution.rs` |
| `WorkflowExecuteMode` | ✅ | `workflow.rs` |

### 1.2 Execution Engine (n8n-core)

| Component | Status | File |
|-----------|--------|------|
| `WorkflowEngine` | ✅ | `engine.rs` |
| `NodeExecutorRegistry` | ✅ | `executor.rs` |
| `RuntimeConfig` | ✅ | `runtime.rs` |
| `ExecutionContext` | ✅ | `runtime.rs` |
| Stack-based execution | ✅ | `engine.rs` |
| Partial execution | ✅ | `engine.rs` |
| Error handling | ✅ | `error.rs` |

### 1.3 Database Persistence (n8n-db)

| Entity | Status | Repository |
|--------|--------|------------|
| `WorkflowEntity` | ✅ | `WorkflowRepository` |
| `ExecutionEntity` | ✅ | `ExecutionRepository` |
| `ExecutionData` | ✅ | `ExecutionRepository` |
| `CredentialsEntity` | ✅ | `CredentialsRepository` |
| `User` | ✅ | `UserRepository` |
| `Project` | ✅ | `ProjectRepository` |
| `TagEntity` | ✅ | `TagRepository` |
| `Variable` | ✅ | `VariablesRepository` |
| `Setting` | ✅ | `SettingsRepository` |
| `WebhookEntity` | ✅ | `WebhookRepository` |

### 1.4 Transport Layer (n8n-grpc)

| Transport | Status | Format Support |
|-----------|--------|----------------|
| REST API | ✅ | JSON, Arrow IPC |
| gRPC | ✅ | Protobuf, Arrow |
| Arrow Flight | ✅ | Arrow IPC (zero-copy) |
| STDIO | ✅ | NDJSON, Binary |
| Content Negotiation | ✅ | All formats |
| Graceful Fallback | ✅ | Health-based |

### 1.5 High-Performance Features

| Feature | Status | Crate |
|---------|--------|-------|
| Arrow Zero-Copy | ✅ | `n8n-arrow` |
| Arrow Flight Streaming | ✅ | `n8n-arrow` |
| 10kbit Hamming Vectors | ✅ | `n8n-hamming` |
| SIMD POPCNT Distance | ✅ | `n8n-hamming` |
| XOR Binding/Unbinding | ✅ | `n8n-hamming` |

---

## Phase 2: Node Connectors 🔴 TODO

### 2.1 Node Categories

The original n8n has **400+ node types** across these categories:

| Category | Count | Priority | Status |
|----------|-------|----------|--------|
| **Core/Flow Control** | 25 | P0 | 🟡 Partial |
| **Trigger Nodes** | 45 | P0 | 🔴 TODO |
| **Data Transform** | 30 | P0 | 🔴 TODO |
| **HTTP/API** | 20 | P0 | 🔴 TODO |
| **Databases** | 25 | P1 | 🔴 TODO |
| **Cloud Services** | 50 | P1 | 🔴 TODO |
| **Communication** | 40 | P1 | 🔴 TODO |
| **AI/ML** | 35 | P2 | 🔴 TODO |
| **File/Storage** | 20 | P2 | 🔴 TODO |
| **Analytics** | 15 | P2 | 🔴 TODO |
| **Productivity** | 50 | P3 | 🔴 TODO |
| **Marketing** | 30 | P3 | 🔴 TODO |
| **Other** | 40 | P3 | 🔴 TODO |

### 2.2 P0 Core Nodes - Implementation Plan

```rust
// n8n-nodes-core crate structure
n8n-nodes-core/
├── src/
│   ├── lib.rs
│   ├── trigger/
│   │   ├── mod.rs
│   │   ├── manual_trigger.rs      // ✅ Done
│   │   ├── schedule_trigger.rs    // 🔴 TODO
│   │   ├── webhook_trigger.rs     // 🔴 TODO
│   │   ├── error_trigger.rs       // 🔴 TODO
│   │   └── start.rs               // 🔴 TODO
│   ├── flow/
│   │   ├── mod.rs
│   │   ├── if_node.rs             // ✅ Done
│   │   ├── switch_node.rs         // 🔴 TODO
│   │   ├── merge_node.rs          // ✅ Done
│   │   ├── split_node.rs          // 🔴 TODO
│   │   ├── loop_node.rs           // 🔴 TODO
│   │   ├── wait_node.rs           // 🔴 TODO
│   │   ├── no_op.rs               // 🔴 TODO
│   │   └── stop_and_error.rs      // 🔴 TODO
│   ├── data/
│   │   ├── mod.rs
│   │   ├── set_node.rs            // ✅ Done
│   │   ├── code_node.rs           // ✅ Done (basic)
│   │   ├── function_node.rs       // 🔴 TODO
│   │   ├── item_lists.rs          // 🔴 TODO
│   │   ├── filter_node.rs         // 🔴 TODO
│   │   ├── sort_node.rs           // 🔴 TODO
│   │   ├── limit_node.rs          // 🔴 TODO
│   │   ├── aggregate_node.rs      // 🔴 TODO
│   │   ├── compare_datasets.rs    // 🔴 TODO
│   │   ├── remove_duplicates.rs   // 🔴 TODO
│   │   ├── rename_keys.rs         // 🔴 TODO
│   │   ├── spreadsheet_file.rs    // 🔴 TODO
│   │   ├── xml_node.rs            // 🔴 TODO
│   │   ├── html_node.rs           // 🔴 TODO
│   │   ├── markdown_node.rs       // 🔴 TODO
│   │   ├── json_node.rs           // 🔴 TODO
│   │   └── crypto_node.rs         // 🔴 TODO
│   ├── http/
│   │   ├── mod.rs
│   │   ├── http_request.rs        // ✅ Done (basic)
│   │   ├── respond_webhook.rs     // 🔴 TODO
│   │   ├── graphql.rs             // 🔴 TODO
│   │   └── soap.rs                // 🔴 TODO
│   └── output/
│       ├── mod.rs
│       ├── execute_command.rs     // 🔴 TODO
│       ├── ssh.rs                 // 🔴 TODO
│       └── write_binary.rs        // 🔴 TODO
```

### 2.3 Node Trait Definition

```rust
// n8n-core/src/executor.rs (existing)

/// Node executor trait - all nodes must implement this.
#[async_trait]
pub trait NodeExecutor: Send + Sync {
    /// Node type identifier (e.g., "n8n-nodes-base.httpRequest").
    fn node_type(&self) -> &'static str;

    /// Node description for UI.
    fn description(&self) -> NodeTypeDescription;

    /// Execute the node.
    async fn execute(
        &self,
        context: &ExecutionContext,
        input: Vec<Vec<NodeExecutionData>>,
        node: &Node,
    ) -> Result<NodeOutput, ExecutionError>;

    /// Trigger function (for trigger nodes).
    async fn trigger(
        &self,
        _context: &ExecutionContext,
        _node: &Node,
    ) -> Result<TriggerResponse, ExecutionError> {
        Err(ExecutionError::NotATrigger)
    }

    /// Webhook handler (for webhook nodes).
    async fn webhook(
        &self,
        _context: &ExecutionContext,
        _node: &Node,
        _request: WebhookRequest,
    ) -> Result<WebhookResponse, ExecutionError> {
        Err(ExecutionError::NotAWebhook)
    }

    /// Poll function (for polling triggers).
    async fn poll(
        &self,
        _context: &ExecutionContext,
        _node: &Node,
    ) -> Result<Option<Vec<Vec<NodeExecutionData>>>, ExecutionError> {
        Err(ExecutionError::NotAPoller)
    }
}

/// Node type description matching n8n's INodeTypeDescription.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeTypeDescription {
    pub display_name: String,
    pub name: String,
    pub group: Vec<String>,
    pub version: NodeVersion,
    pub description: String,
    pub defaults: NodeDefaults,
    pub inputs: Vec<NodeConnectionConfig>,
    pub outputs: Vec<NodeConnectionConfig>,
    pub properties: Vec<NodeProperty>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credentials: Option<Vec<CredentialDescription>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhooks: Option<Vec<WebhookDescription>>,
}
```

### 2.4 Example Node Implementation

```rust
// n8n-nodes-core/src/http/http_request.rs

use n8n_core::{NodeExecutor, ExecutionContext, NodeOutput, ExecutionError};
use n8n_workflow::{Node, NodeExecutionData, DataObject};
use async_trait::async_trait;
use reqwest::Client;

pub struct HttpRequestNode {
    client: Client,
}

impl HttpRequestNode {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(300))
                .build()
                .unwrap(),
        }
    }
}

#[async_trait]
impl NodeExecutor for HttpRequestNode {
    fn node_type(&self) -> &'static str {
        "n8n-nodes-base.httpRequest"
    }

    fn description(&self) -> NodeTypeDescription {
        NodeTypeDescription {
            display_name: "HTTP Request".to_string(),
            name: "httpRequest".to_string(),
            group: vec!["transform".to_string()],
            version: NodeVersion::Single(4),
            description: "Makes HTTP requests".to_string(),
            defaults: NodeDefaults {
                name: "HTTP Request".to_string(),
                color: Some("#0033AA".to_string()),
            },
            inputs: vec![NodeConnectionConfig::main()],
            outputs: vec![NodeConnectionConfig::main()],
            properties: vec![
                NodeProperty::options("method", "Method", vec![
                    "GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"
                ]),
                NodeProperty::string("url", "URL"),
                NodeProperty::json("headers", "Headers"),
                NodeProperty::json("body", "Body"),
                NodeProperty::options("responseFormat", "Response Format", vec![
                    "autodetect", "json", "text", "file"
                ]),
            ],
            credentials: Some(vec![
                CredentialDescription::optional("httpBasicAuth"),
                CredentialDescription::optional("httpHeaderAuth"),
                CredentialDescription::optional("oAuth2Api"),
            ]),
            webhooks: None,
        }
    }

    async fn execute(
        &self,
        context: &ExecutionContext,
        input: Vec<Vec<NodeExecutionData>>,
        node: &Node,
    ) -> Result<NodeOutput, ExecutionError> {
        let mut results = Vec::new();

        for items in input {
            let mut output_items = Vec::new();

            for item in items {
                // Get parameters with expression resolution
                let method = context.get_param_string(node, "method", &item)?;
                let url = context.get_param_string(node, "url", &item)?;
                let headers = context.get_param_json(node, "headers", &item)?;
                let body = context.get_param_json(node, "body", &item)?;

                // Build request
                let mut request = match method.to_uppercase().as_str() {
                    "GET" => self.client.get(&url),
                    "POST" => self.client.post(&url),
                    "PUT" => self.client.put(&url),
                    "DELETE" => self.client.delete(&url),
                    "PATCH" => self.client.patch(&url),
                    "HEAD" => self.client.head(&url),
                    _ => return Err(ExecutionError::InvalidParameter(
                        format!("Unknown method: {}", method)
                    )),
                };

                // Add headers
                if let Some(headers) = headers.as_object() {
                    for (key, value) in headers {
                        if let Some(v) = value.as_str() {
                            request = request.header(key, v);
                        }
                    }
                }

                // Add body
                if !body.is_null() {
                    request = request.json(&body);
                }

                // Execute request
                let response = request.send().await
                    .map_err(|e| ExecutionError::NodeError(e.to_string()))?;

                let status = response.status().as_u16();
                let headers: DataObject = response.headers()
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").into()))
                    .collect();

                let body: serde_json::Value = response.json().await
                    .unwrap_or(serde_json::Value::Null);

                output_items.push(NodeExecutionData {
                    json: serde_json::json!({
                        "statusCode": status,
                        "headers": headers,
                        "body": body,
                    }).as_object().unwrap().clone().into_iter().collect(),
                    binary: None,
                    paired_item: Some(item.paired_item.clone().unwrap_or_default()),
                    ..Default::default()
                });
            }

            results.push(output_items);
        }

        Ok(NodeOutput::Data(results))
    }
}
```

---

## Phase 3: Expression Evaluation 🔴 TODO

### 3.1 Expression System

n8n uses a custom expression syntax: `{{ $json.field }}`, `{{ $node.Name.json }}`, etc.

```rust
// n8n-core/src/expression/mod.rs (TODO)

pub mod parser;
pub mod evaluator;
pub mod extensions;

use n8n_workflow::NodeExecutionData;

/// Expression evaluation context.
pub struct ExpressionContext<'a> {
    /// Current item being processed.
    pub item: &'a NodeExecutionData,
    /// Item index in current batch.
    pub item_index: usize,
    /// Run index for the current node.
    pub run_index: usize,
    /// Access to other nodes' data.
    pub node_data: &'a HashMap<String, Vec<Vec<NodeExecutionData>>>,
    /// Workflow variables.
    pub variables: &'a HashMap<String, serde_json::Value>,
    /// Execution metadata.
    pub execution: &'a ExecutionMetadata,
}

/// Expression evaluator.
pub struct ExpressionEvaluator {
    /// Registered extension methods.
    extensions: HashMap<String, Box<dyn Extension>>,
}

impl ExpressionEvaluator {
    /// Evaluate an expression string.
    pub fn evaluate(
        &self,
        expression: &str,
        context: &ExpressionContext,
    ) -> Result<serde_json::Value, ExpressionError> {
        // Parse expression
        let ast = parser::parse(expression)?;

        // Evaluate AST
        self.evaluate_ast(&ast, context)
    }

    /// Resolve expressions in a node parameter.
    pub fn resolve_parameter(
        &self,
        value: &serde_json::Value,
        context: &ExpressionContext,
    ) -> Result<serde_json::Value, ExpressionError> {
        match value {
            serde_json::Value::String(s) if s.contains("{{") => {
                self.resolve_string(s, context)
            }
            serde_json::Value::Object(obj) => {
                let mut result = serde_json::Map::new();
                for (k, v) in obj {
                    result.insert(k.clone(), self.resolve_parameter(v, context)?);
                }
                Ok(serde_json::Value::Object(result))
            }
            serde_json::Value::Array(arr) => {
                let result: Result<Vec<_>, _> = arr
                    .iter()
                    .map(|v| self.resolve_parameter(v, context))
                    .collect();
                Ok(serde_json::Value::Array(result?))
            }
            _ => Ok(value.clone()),
        }
    }
}
```

### 3.2 Built-in Variables

| Variable | Description | Status |
|----------|-------------|--------|
| `$json` | Current item's JSON data | 🔴 TODO |
| `$binary` | Current item's binary data | 🔴 TODO |
| `$node["Name"]` | Access another node's data | 🔴 TODO |
| `$input` | Input data reference | 🔴 TODO |
| `$execution` | Execution metadata | 🔴 TODO |
| `$workflow` | Workflow metadata | 🔴 TODO |
| `$vars` | Workflow variables | 🔴 TODO |
| `$env` | Environment variables | 🔴 TODO |
| `$now` | Current timestamp | 🔴 TODO |
| `$today` | Today's date | 🔴 TODO |
| `$jmespath()` | JMESPath queries | 🔴 TODO |

### 3.3 Extension Methods

| Category | Methods | Status |
|----------|---------|--------|
| String | `.toUpperCase()`, `.toLowerCase()`, `.trim()`, etc. | 🔴 TODO |
| Array | `.first()`, `.last()`, `.filter()`, `.map()`, etc. | 🔴 TODO |
| Number | `.round()`, `.floor()`, `.ceil()`, `.format()`, etc. | 🔴 TODO |
| Date | `.format()`, `.plus()`, `.minus()`, `.diff()`, etc. | 🔴 TODO |
| Object | `.keys()`, `.values()`, `.entries()`, `.merge()`, etc. | 🔴 TODO |

---

## Phase 4: Credential Management 🔴 TODO

### 4.1 Credential Encryption

```rust
// n8n-core/src/credentials/mod.rs (TODO)

pub mod encryption;
pub mod types;
pub mod resolver;

use aes_gcm::{Aes256Gcm, Key, Nonce};
use aes_gcm::aead::{Aead, KeyInit};

/// Credential encryption service.
pub struct CredentialEncryption {
    key: Key<Aes256Gcm>,
}

impl CredentialEncryption {
    /// Create from encryption key.
    pub fn new(encryption_key: &[u8]) -> Result<Self, CredentialError> {
        let key = Key::<Aes256Gcm>::from_slice(encryption_key);
        Ok(Self { key: *key })
    }

    /// Encrypt credential data.
    pub fn encrypt(&self, data: &serde_json::Value) -> Result<String, CredentialError> {
        let plaintext = serde_json::to_vec(data)?;
        let cipher = Aes256Gcm::new(&self.key);
        let nonce = Nonce::from_slice(&rand::random::<[u8; 12]>());

        let ciphertext = cipher.encrypt(nonce, plaintext.as_ref())
            .map_err(|_| CredentialError::EncryptionFailed)?;

        // Format: nonce || ciphertext, base64 encoded
        let mut result = nonce.to_vec();
        result.extend(ciphertext);
        Ok(base64::encode(&result))
    }

    /// Decrypt credential data.
    pub fn decrypt(&self, encrypted: &str) -> Result<serde_json::Value, CredentialError> {
        let data = base64::decode(encrypted)?;
        if data.len() < 12 {
            return Err(CredentialError::InvalidFormat);
        }

        let (nonce, ciphertext) = data.split_at(12);
        let cipher = Aes256Gcm::new(&self.key);
        let nonce = Nonce::from_slice(nonce);

        let plaintext = cipher.decrypt(nonce, ciphertext)
            .map_err(|_| CredentialError::DecryptionFailed)?;

        Ok(serde_json::from_slice(&plaintext)?)
    }
}
```

### 4.2 Credential Types

| Type | Auth Method | Status |
|------|-------------|--------|
| `httpBasicAuth` | Basic Auth header | 🔴 TODO |
| `httpHeaderAuth` | Custom header | 🔴 TODO |
| `oAuth1Api` | OAuth 1.0 | 🔴 TODO |
| `oAuth2Api` | OAuth 2.0 | 🔴 TODO |
| `apiKey` | API Key | 🔴 TODO |
| Service-specific | Varies | 🔴 TODO |

---

## Phase 5: Orchestrator Layer 🔴 TODO

### 5.1 Components

```rust
// n8n-core/src/orchestrator/mod.rs (TODO)

pub mod scheduler;
pub mod queue;
pub mod worker;
pub mod webhook_server;

/// Workflow orchestrator managing execution lifecycle.
pub struct Orchestrator {
    /// Execution queue (Redis-backed for distributed).
    queue: Arc<ExecutionQueue>,
    /// Worker pool for parallel execution.
    workers: WorkerPool,
    /// Scheduler for cron/interval triggers.
    scheduler: Scheduler,
    /// Webhook server for HTTP triggers.
    webhook_server: WebhookServer,
    /// Database context.
    db: DbContext,
}

impl Orchestrator {
    /// Start the orchestrator.
    pub async fn start(&self) -> Result<(), OrchestratorError> {
        // Start all components
        tokio::try_join!(
            self.scheduler.start(),
            self.workers.start(),
            self.webhook_server.start(),
            self.process_queue(),
        )?;
        Ok(())
    }

    /// Queue a workflow execution.
    pub async fn queue_execution(
        &self,
        workflow_id: &str,
        mode: WorkflowExecuteMode,
        data: Option<serde_json::Value>,
    ) -> Result<String, OrchestratorError> {
        // Create execution record
        let execution = ExecutionEntity::new(workflow_id, mode);
        let execution = self.db.executions.create(&execution.into()).await?;

        // Queue for processing
        self.queue.enqueue(ExecutionJob {
            execution_id: execution.id.clone(),
            workflow_id: workflow_id.to_string(),
            mode,
            data,
            priority: ExecutionPriority::Normal,
        }).await?;

        Ok(execution.id)
    }

    /// Process execution queue.
    async fn process_queue(&self) -> Result<(), OrchestratorError> {
        loop {
            if let Some(job) = self.queue.dequeue().await? {
                let worker = self.workers.get_available().await;
                worker.execute(job).await?;
            }
        }
    }
}
```

### 5.2 Scheduler

```rust
// n8n-core/src/orchestrator/scheduler.rs (TODO)

use cron::Schedule;
use std::str::FromStr;

/// Scheduler for time-based triggers.
pub struct Scheduler {
    /// Active schedules.
    schedules: DashMap<String, ScheduleEntry>,
    /// Database context.
    db: DbContext,
}

#[derive(Clone)]
struct ScheduleEntry {
    workflow_id: String,
    node_name: String,
    schedule: Schedule,
    next_run: DateTime<Utc>,
    timezone: chrono_tz::Tz,
}

impl Scheduler {
    /// Start the scheduler.
    pub async fn start(&self) -> Result<(), SchedulerError> {
        // Load active workflows with schedule triggers
        let active_workflows = self.db.workflows.find_active().await?;

        for workflow in active_workflows {
            self.register_workflow(&workflow).await?;
        }

        // Run scheduler loop
        self.run_loop().await
    }

    /// Register a workflow's schedules.
    pub async fn register_workflow(&self, workflow: &WorkflowEntity) -> Result<(), SchedulerError> {
        for node in &workflow.nodes {
            if node.node_type == "n8n-nodes-base.scheduleTrigger" {
                let cron_expr = node.parameters.get("rule")
                    .and_then(|v| v.as_str())
                    .unwrap_or("0 * * * *");

                let schedule = Schedule::from_str(cron_expr)
                    .map_err(|e| SchedulerError::InvalidCron(e.to_string()))?;

                let timezone = node.parameters.get("timezone")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(chrono_tz::UTC);

                let next_run = schedule.upcoming(timezone).next()
                    .ok_or(SchedulerError::NoUpcomingRuns)?;

                self.schedules.insert(
                    format!("{}:{}", workflow.id, node.name),
                    ScheduleEntry {
                        workflow_id: workflow.id.clone(),
                        node_name: node.name.clone(),
                        schedule,
                        next_run,
                        timezone,
                    },
                );
            }
        }

        Ok(())
    }

    /// Main scheduler loop.
    async fn run_loop(&self) -> Result<(), SchedulerError> {
        loop {
            let now = Utc::now();

            // Find due schedules
            for entry in self.schedules.iter() {
                if entry.next_run <= now {
                    // Trigger workflow
                    self.trigger_workflow(&entry.workflow_id, &entry.node_name).await?;

                    // Update next run
                    let mut entry = entry.clone();
                    entry.next_run = entry.schedule
                        .upcoming(entry.timezone)
                        .next()
                        .ok_or(SchedulerError::NoUpcomingRuns)?;
                    self.schedules.insert(entry.workflow_id.clone(), entry);
                }
            }

            // Sleep until next check
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}
```

### 5.3 Webhook Server

```rust
// n8n-core/src/orchestrator/webhook_server.rs (TODO)

use axum::{Router, routing::any};

/// Webhook server for HTTP triggers.
pub struct WebhookServer {
    /// Port to listen on.
    port: u16,
    /// Base path for webhooks.
    base_path: String,
    /// Database context.
    db: DbContext,
    /// Orchestrator reference.
    orchestrator: Arc<Orchestrator>,
}

impl WebhookServer {
    /// Start the webhook server.
    pub async fn start(&self) -> Result<(), WebhookError> {
        let router = Router::new()
            .route("/webhook/*path", any(Self::handle_webhook))
            .route("/webhook-test/*path", any(Self::handle_test_webhook))
            .route("/webhook-waiting/*path", any(Self::handle_waiting_webhook))
            .with_state(self.clone());

        let addr = format!("0.0.0.0:{}", self.port);
        let listener = tokio::net::TcpListener::bind(&addr).await?;

        tracing::info!("Webhook server listening on {}", addr);
        axum::serve(listener, router).await?;

        Ok(())
    }

    /// Handle incoming webhook.
    async fn handle_webhook(
        State(server): State<Self>,
        method: Method,
        Path(path): Path<String>,
        headers: HeaderMap,
        body: Bytes,
    ) -> Result<Response, WebhookError> {
        // Find matching webhook
        let webhooks = server.db.webhooks
            .find_by_path(&method.to_string(), &path)
            .await?;

        if webhooks.is_empty() {
            return Ok((StatusCode::NOT_FOUND, "Webhook not found").into_response());
        }

        // Execute each matching workflow
        for webhook in webhooks {
            let workflow = server.db.workflows
                .find_by_id(&webhook.workflow_id)
                .await?
                .ok_or(WebhookError::WorkflowNotFound)?;

            if !workflow.active {
                continue;
            }

            // Queue execution with webhook data
            server.orchestrator.queue_execution(
                &webhook.workflow_id,
                WorkflowExecuteMode::Webhook,
                Some(serde_json::json!({
                    "headers": headers_to_json(&headers),
                    "params": parse_query_params(&path),
                    "body": parse_body(&body, &headers),
                    "method": method.to_string(),
                    "path": path,
                })),
            ).await?;
        }

        Ok((StatusCode::OK, "Webhook received").into_response())
    }
}
```

---

## Phase 6: 1:1 REST API Surface 🔴 TODO

### 6.1 API Endpoints

The original n8n REST API must be replicated exactly:

| Endpoint | Method | Description | Status |
|----------|--------|-------------|--------|
| `/api/v1/workflows` | GET | List workflows | 🔴 TODO |
| `/api/v1/workflows` | POST | Create workflow | 🔴 TODO |
| `/api/v1/workflows/:id` | GET | Get workflow | 🔴 TODO |
| `/api/v1/workflows/:id` | PUT | Update workflow | 🔴 TODO |
| `/api/v1/workflows/:id` | DELETE | Delete workflow | 🔴 TODO |
| `/api/v1/workflows/:id/activate` | POST | Activate workflow | 🔴 TODO |
| `/api/v1/workflows/:id/deactivate` | POST | Deactivate workflow | 🔴 TODO |
| `/api/v1/workflows/:id/execute` | POST | Execute workflow | 🔴 TODO |
| `/api/v1/executions` | GET | List executions | 🔴 TODO |
| `/api/v1/executions/:id` | GET | Get execution | 🔴 TODO |
| `/api/v1/executions/:id` | DELETE | Delete execution | 🔴 TODO |
| `/api/v1/executions/:id/stop` | POST | Stop execution | 🔴 TODO |
| `/api/v1/executions/:id/retry` | POST | Retry execution | 🔴 TODO |
| `/api/v1/credentials` | GET | List credentials | 🔴 TODO |
| `/api/v1/credentials` | POST | Create credential | 🔴 TODO |
| `/api/v1/credentials/:id` | GET | Get credential | 🔴 TODO |
| `/api/v1/credentials/:id` | PATCH | Update credential | 🔴 TODO |
| `/api/v1/credentials/:id` | DELETE | Delete credential | 🔴 TODO |
| `/api/v1/credentials/test` | POST | Test credential | 🔴 TODO |
| `/api/v1/tags` | GET | List tags | 🔴 TODO |
| `/api/v1/tags` | POST | Create tag | 🔴 TODO |
| `/api/v1/tags/:id` | GET | Get tag | 🔴 TODO |
| `/api/v1/tags/:id` | PATCH | Update tag | 🔴 TODO |
| `/api/v1/tags/:id` | DELETE | Delete tag | 🔴 TODO |
| `/api/v1/users` | GET | List users | 🔴 TODO |
| `/api/v1/users/:id` | GET | Get user | 🔴 TODO |
| `/api/v1/variables` | GET | List variables | 🔴 TODO |
| `/api/v1/variables` | POST | Create variable | 🔴 TODO |
| `/api/v1/variables/:id` | DELETE | Delete variable | 🔴 TODO |
| `/api/v1/node-types` | GET | List node types | 🔴 TODO |
| `/api/v1/active-workflows` | GET | Get active workflows | 🔴 TODO |
| `/api/v1/settings` | GET | Get settings | 🔴 TODO |

### 6.2 Request/Response DTOs

```rust
// n8n-grpc/src/api/dto/mod.rs (TODO)

pub mod workflow;
pub mod execution;
pub mod credentials;
pub mod user;
pub mod tag;

// Example: Workflow DTOs matching n8n's @n8n/api-types

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowResponse {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub active: bool,
    pub nodes: Vec<Node>,
    pub connections: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<WorkflowSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub static_data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<TagResponse>>,
    pub version_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkflowRequest {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub connections: serde_json::Value,
    #[serde(default)]
    pub settings: Option<WorkflowSettings>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWorkflowRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nodes: Option<Vec<Node>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connections: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<WorkflowSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteWorkflowRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_nodes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination_node: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin_data: Option<serde_json::Value>,
}
```

---

## Phase 7: Integration Nodes 🔴 TODO

### 7.1 Priority Integration Categories

| Category | Example Services | Node Count | Priority |
|----------|-----------------|------------|----------|
| **Databases** | PostgreSQL, MySQL, MongoDB, Redis | 12 | P1 |
| **Cloud** | AWS S3, GCS, Azure Blob | 15 | P1 |
| **Communication** | Slack, Discord, Telegram, Email | 20 | P1 |
| **CRM** | Salesforce, HubSpot, Pipedrive | 15 | P2 |
| **Project Management** | Jira, Asana, Trello, Linear | 12 | P2 |
| **AI/ML** | OpenAI, Anthropic, Cohere | 10 | P2 |
| **Analytics** | Google Analytics, Mixpanel | 8 | P3 |
| **Marketing** | Mailchimp, SendGrid, Twilio | 12 | P3 |
| **E-commerce** | Shopify, Stripe, PayPal | 10 | P3 |
| **Social Media** | Twitter, LinkedIn, Facebook | 10 | P3 |

### 7.2 Node Implementation Strategy

```rust
// Strategy: Use macros for common patterns

// Macro for REST API-based nodes
macro_rules! rest_api_node {
    ($name:ident, $node_type:literal, $display_name:literal, $base_url:expr) => {
        pub struct $name {
            client: reqwest::Client,
        }

        #[async_trait]
        impl NodeExecutor for $name {
            fn node_type(&self) -> &'static str { $node_type }

            async fn execute(
                &self,
                context: &ExecutionContext,
                input: Vec<Vec<NodeExecutionData>>,
                node: &Node,
            ) -> Result<NodeOutput, ExecutionError> {
                // Common REST API execution logic
                execute_rest_api(self, context, input, node, $base_url).await
            }
        }
    };
}

// Usage
rest_api_node!(SlackNode, "n8n-nodes-base.slack", "Slack", "https://slack.com/api");
rest_api_node!(GithubNode, "n8n-nodes-base.github", "GitHub", "https://api.github.com");
```

---

## Timeline Estimate

| Phase | Components | Estimated Effort |
|-------|------------|------------------|
| Phase 1 | Core Infrastructure | ✅ Complete |
| Phase 2 | P0 Core Nodes (25) | 2-3 weeks |
| Phase 3 | Expression Evaluation | 2 weeks |
| Phase 4 | Credential Management | 1 week |
| Phase 5 | Orchestrator Layer | 2-3 weeks |
| Phase 6 | 1:1 REST API | 2 weeks |
| Phase 7 | P1 Integration Nodes (50) | 4-6 weeks |
| Phase 8 | P2 Integration Nodes (50) | 4-6 weeks |
| Phase 9 | P3 Integration Nodes (100+) | 6-8 weeks |
| **Total** | Full feature parity | ~6-8 months |

---

## Files Created/Modified

### Current Implementation (12,700+ lines)

```
n8n-rust/
├── Cargo.toml                           # Workspace config
├── README.md                            # Documentation
├── crates/
│   ├── n8n-workflow/                    # Core types (1,800 lines)
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── workflow.rs              # Workflow, Settings
│   │   │   ├── node.rs                  # Node, NodeType
│   │   │   ├── connection.rs            # Connection, Graph
│   │   │   ├── data.rs                  # NodeExecutionData
│   │   │   ├── execution.rs             # RunExecutionData, TaskData
│   │   │   └── error.rs                 # Workflow errors
│   │   └── Cargo.toml
│   ├── n8n-core/                        # Execution engine (1,500 lines)
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── engine.rs                # WorkflowEngine
│   │   │   ├── executor.rs              # NodeExecutor trait
│   │   │   ├── node_types.rs            # Built-in nodes
│   │   │   ├── runtime.rs               # RuntimeConfig
│   │   │   ├── storage.rs               # Memory storage
│   │   │   └── error.rs                 # Execution errors
│   │   └── Cargo.toml
│   ├── n8n-db/                          # PostgreSQL (3,700 lines)
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── entities/                # All 15 entities
│   │   │   ├── repositories/            # All 9 repositories
│   │   │   └── error.rs
│   │   ├── migrations/
│   │   │   └── 001_initial_schema.sql
│   │   └── Cargo.toml
│   ├── n8n-arrow/                       # Arrow integration (1,200 lines)
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── schema.rs                # Arrow schemas
│   │   │   ├── convert.rs               # Type conversion
│   │   │   ├── ipc.rs                   # IPC serialization
│   │   │   ├── flight.rs                # Arrow Flight
│   │   │   └── error.rs
│   │   └── Cargo.toml
│   ├── n8n-hamming/                     # Hamming vectors (800 lines)
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── vector.rs                # HammingVector
│   │   │   └── error.rs
│   │   ├── benches/
│   │   │   └── hamming_bench.rs
│   │   └── Cargo.toml
│   ├── n8n-grpc/                        # Transports (3,500 lines)
│   │   ├── proto/
│   │   │   └── n8n.proto
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── services/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── workflow_service.rs
│   │   │   │   ├── arrow_service.rs
│   │   │   │   ├── hamming_service.rs
│   │   │   │   └── json_compat.rs
│   │   │   └── transport/
│   │   │       ├── mod.rs
│   │   │       ├── stdio.rs
│   │   │       ├── rest.rs
│   │   │       └── negotiate.rs
│   │   ├── build.rs
│   │   └── Cargo.toml
│   └── n8n-server/                      # Server binary (300 lines)
│       ├── src/
│       │   └── main.rs
│       └── Cargo.toml
```

---

## Next Steps

1. **Immediate (This Week)**
   - [ ] Implement expression parser and evaluator
   - [ ] Add credential encryption service
   - [ ] Implement remaining P0 core nodes

2. **Short Term (Next 2 Weeks)**
   - [ ] Complete orchestrator layer (scheduler, queue, workers)
   - [ ] Implement webhook server
   - [ ] Add 1:1 REST API endpoints

3. **Medium Term (Next Month)**
   - [ ] Implement P1 integration nodes (databases, cloud)
   - [ ] Add OAuth2 credential support
   - [ ] Integration testing with original n8n

4. **Long Term (Next Quarter)**
   - [ ] Complete all integration nodes
   - [ ] Performance benchmarking vs TypeScript n8n
   - [ ] Production deployment documentation

---

## Testing Strategy

```rust
// Integration tests comparing Rust vs TypeScript execution
#[tokio::test]
async fn test_workflow_execution_parity() {
    // Load same workflow in both systems
    let workflow = load_test_workflow("complex_workflow.json");

    // Execute in TypeScript n8n
    let ts_result = execute_in_typescript_n8n(&workflow).await;

    // Execute in Rust n8n
    let rs_result = execute_in_rust_n8n(&workflow).await;

    // Compare results
    assert_eq!(ts_result.status, rs_result.status);
    assert_eq!(ts_result.output_data, rs_result.output_data);
}
```

---

## Performance Targets

| Metric | TypeScript n8n | Rust n8n Target | Improvement |
|--------|---------------|-----------------|-------------|
| Workflow execution startup | ~100ms | <10ms | 10x |
| Node execution overhead | ~5ms/node | <0.5ms/node | 10x |
| Memory per execution | ~50MB | <10MB | 5x |
| Large data transfer | JSON serialize | Arrow zero-copy | 100x |
| Concurrent executions | ~100 | ~10,000 | 100x |
| Cold start time | ~3s | <100ms | 30x |

---

*Document Version: 1.0*
*Last Updated: 2026-02-12*
*Branch: claude/n8n-rust-grpc-CAnmd*
