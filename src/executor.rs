//! Unified workflow step executor
//!
//! Routes workflow steps to the appropriate runtime based on step type prefix:
//! - `n8n.*` → local n8n handler (existing logic)
//! - `crew.*` → crewAI HTTP service (via CrewRouter)
//! - `lb.*` → ladybug enrichment service (optional, pass-through if absent)
//!
//! Also writes execution records to PostgreSQL (if postgres feature is enabled).

#[cfg(feature = "postgres")]
use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{error, info, warn};

use crate::config::AppState;
use crate::contract::crew_router::{CrewRouter, LadybugRouter};
use crate::contract::types::*;

// ═══════════════════════════════════════════════════════════════════════════
// Workflow Definition Types (extended n8n format)
// ═══════════════════════════════════════════════════════════════════════════

/// A workflow node from the JSON definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowNode {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    #[serde(default)]
    pub parameters: Value,
    #[serde(default)]
    pub name: Option<String>,
}

/// Connection target in the workflow graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionTarget {
    pub node: String,
}

/// Full workflow definition (n8n-compatible with crew.* extensions).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    #[serde(default)]
    pub nodes: Vec<WorkflowNode>,
    #[serde(default)]
    pub connections: std::collections::HashMap<String, ConnectionOutputs>,
}

/// Connection outputs from a node (n8n format: `{"main": [[{"node": "..."}]]}`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionOutputs {
    #[serde(default)]
    pub main: Vec<Vec<ConnectionTarget>>,
}

// ═══════════════════════════════════════════════════════════════════════════
// Workflow Executor
// ═══════════════════════════════════════════════════════════════════════════

/// Configuration for the unified workflow executor.
pub struct ExecutorConfig {
    /// crewAI HTTP endpoint (from CREWAI_ENDPOINT env var)
    pub crewai_endpoint: Option<String>,
    /// Ladybug HTTP endpoint (from LADYBUG_ENDPOINT env var)
    pub ladybug_endpoint: Option<String>,
}

/// Unified workflow executor that dispatches steps to the appropriate runtime.
pub struct WorkflowExecutor {
    state: AppState,
    crew_router: Option<CrewRouter>,
    lb_router: Option<LadybugRouter>,
    #[cfg(feature = "postgres")]
    pg_store: Option<Arc<crate::contract::pg_store::PgStore>>,
}

impl WorkflowExecutor {
    /// Create a new WorkflowExecutor with the given configuration.
    pub fn new(state: AppState, config: ExecutorConfig) -> Self {
        let crew_router = config.crewai_endpoint.map(|endpoint| {
            info!("CrewAI routing enabled: {}", endpoint);
            CrewRouter::new(endpoint, state.http_client.clone())
        });

        let lb_router = config.ladybug_endpoint.map(|endpoint| {
            info!("Ladybug routing enabled: {}", endpoint);
            LadybugRouter::new(endpoint, state.http_client.clone())
        });

        Self {
            state,
            crew_router,
            lb_router,
            #[cfg(feature = "postgres")]
            pg_store: None,
        }
    }

    /// Set the PostgreSQL store for execution recording.
    #[cfg(feature = "postgres")]
    pub fn with_pg_store(mut self, store: Arc<crate::contract::pg_store::PgStore>) -> Self {
        self.pg_store = Some(store);
        self
    }

    /// Execute a single workflow step, routing to the appropriate runtime.
    pub async fn execute_step(
        &self,
        step: &UnifiedStep,
        input: &DataEnvelope,
    ) -> Result<DataEnvelope> {
        let runtime_prefix = step.step_type.split('.').next().unwrap_or("n8n");

        match runtime_prefix {
            "crew" => self.execute_crew_step(step, input).await,
            "lb" => self.execute_lb_step(step, input).await,
            _ => self.execute_n8n_step(step, input).await,
        }
    }

    /// Execute a crew.* step via the CrewRouter.
    async fn execute_crew_step(
        &self,
        step: &UnifiedStep,
        input: &DataEnvelope,
    ) -> Result<DataEnvelope> {
        match &self.crew_router {
            Some(router) => router.execute_crew_step(step, input).await,
            None => {
                warn!(
                    step_id = %step.step_id,
                    "No crewAI endpoint configured — skipping crew step"
                );
                Ok(DataEnvelope::passthrough(&step.step_id, input))
            }
        }
    }

    /// Execute an lb.* step via the LadybugRouter (optional enrichment).
    async fn execute_lb_step(
        &self,
        step: &UnifiedStep,
        input: &DataEnvelope,
    ) -> Result<DataEnvelope> {
        match &self.lb_router {
            Some(router) => router.execute_lb_step(step, input).await,
            None => {
                // Ladybug is optional enrichment — pass through if not configured
                Ok(DataEnvelope::passthrough(&step.step_id, input))
            }
        }
    }

    /// Execute an n8n.* step locally using the existing handler logic.
    async fn execute_n8n_step(
        &self,
        step: &UnifiedStep,
        input: &DataEnvelope,
    ) -> Result<DataEnvelope> {
        info!(
            step_id = %step.step_id,
            step_type = %step.step_type,
            "Executing n8n step locally"
        );

        // For n8n steps, delegate to the existing handler logic via HTTP client
        // to the local service, or execute inline based on step type
        let n8n_type = step
            .step_type
            .strip_prefix("n8n.")
            .unwrap_or(&step.step_type);

        match n8n_type {
            "webhook" => {
                // Webhook triggers are entry points — pass through
                Ok(DataEnvelope::passthrough(&step.step_id, input))
            }
            "httpRequest" => {
                // Execute HTTP request based on step parameters
                let url = step
                    .input
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let method = step
                    .input
                    .get("method")
                    .and_then(|v| v.as_str())
                    .unwrap_or("GET");

                let response = match method.to_uppercase().as_str() {
                    "POST" => {
                        let body = step.input.get("body").cloned().unwrap_or(json!({}));
                        self.state
                            .http_client
                            .post(url)
                            .json(&body)
                            .send()
                            .await?
                    }
                    "PUT" => {
                        let body = step.input.get("body").cloned().unwrap_or(json!({}));
                        self.state
                            .http_client
                            .put(url)
                            .json(&body)
                            .send()
                            .await?
                    }
                    _ => self.state.http_client.get(url).send().await?,
                };

                let result: Value = response.json().await.unwrap_or(json!(null));
                Ok(DataEnvelope::from_n8n_output(&step.step_id, &json!([result])))
            }
            _ => {
                // Default: pass through for unsupported n8n node types
                info!(
                    step_id = %step.step_id,
                    n8n_type = %n8n_type,
                    "Unsupported n8n node type — passing through"
                );
                Ok(DataEnvelope::passthrough(&step.step_id, input))
            }
        }
    }

    /// Execute a full workflow definition.
    ///
    /// Parses the workflow JSON, resolves the execution graph, and executes
    /// each step in sequence (following connections). Writes execution records
    /// to PostgreSQL if the postgres feature is enabled.
    pub async fn execute_workflow(
        &self,
        workflow: &WorkflowDefinition,
        workflow_name: &str,
        trigger: &str,
        initial_input: Value,
    ) -> Result<DataEnvelope> {
        let execution_id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();

        // Create execution record
        let _execution = UnifiedExecution {
            execution_id: execution_id.clone(),
            runtime: "n8n".to_string(),
            workflow_name: workflow_name.to_string(),
            status: StepStatus::Running,
            trigger: trigger.to_string(),
            input: initial_input.clone(),
            output: json!({}),
            started_at: now,
            finished_at: None,
            step_count: 0,
        };

        #[cfg(feature = "postgres")]
        if let Some(store) = &self.pg_store {
            if let Err(e) = store.write_execution(&_execution).await {
                error!("Failed to write execution record: {}", e);
            }
        }

        // Build execution order from connections
        let ordered_nodes = self.resolve_execution_order(workflow);

        // Execute steps in sequence
        let mut current_envelope =
            DataEnvelope::from_n8n_output("trigger", &json!([initial_input]));
        let mut step_count = 0;

        for node in &ordered_nodes {
            step_count += 1;

            let runtime = match node.node_type.split('.').next() {
                Some("crew") => "crewai",
                Some("lb") => "ladybug",
                _ => "n8n",
            };

            let step = UnifiedStep {
                step_id: uuid::Uuid::new_v4().to_string(),
                execution_id: execution_id.clone(),
                step_type: node.node_type.clone(),
                runtime: runtime.to_string(),
                name: node.name.clone().unwrap_or_else(|| node.id.clone()),
                status: StepStatus::Running,
                input: node.parameters.clone(),
                output: json!({}),
                error: None,
                started_at: Utc::now(),
                finished_at: None,
                sequence: step_count,
            };

            #[cfg(feature = "postgres")]
            if let Some(store) = &self.pg_store {
                if let Err(e) = store.write_step(&step).await {
                    error!("Failed to write step record: {}", e);
                }
            }

            match self.execute_step(&step, &current_envelope).await {
                Ok(output) => {
                    let mut completed_step = step.clone();
                    completed_step.status = StepStatus::Completed;
                    completed_step.output = output.content.clone();
                    completed_step.finished_at = Some(Utc::now());

                    #[cfg(feature = "postgres")]
                    if let Some(store) = &self.pg_store {
                        if let Err(e) = store.write_step(&completed_step).await {
                            error!("Failed to update step record: {}", e);
                        }
                    }

                    current_envelope = output;
                }
                Err(e) => {
                    error!(
                        step_id = %step.step_id,
                        node_id = %node.id,
                        "Step failed: {}",
                        e
                    );

                    let mut failed_step = step.clone();
                    failed_step.status = StepStatus::Failed;
                    failed_step.error = Some(e.to_string());
                    failed_step.finished_at = Some(Utc::now());

                    #[cfg(feature = "postgres")]
                    if let Some(store) = &self.pg_store {
                        if let Err(e) = store.write_step(&failed_step).await {
                            error!("Failed to update step record: {}", e);
                        }
                        if let Err(e) = store.finish_execution(
                            &execution_id,
                            StepStatus::Failed,
                            &json!({"error": failed_step.error}),
                            step_count,
                        ).await {
                            error!("Failed to update execution record: {}", e);
                        }
                    }

                    return Err(e);
                }
            }
        }

        // Finish execution
        #[cfg(feature = "postgres")]
        if let Some(store) = &self.pg_store {
            if let Err(e) = store
                .finish_execution(
                    &execution_id,
                    StepStatus::Completed,
                    &current_envelope.content,
                    step_count,
                )
                .await
            {
                error!("Failed to finish execution record: {}", e);
            }
        }

        info!(
            execution_id = %execution_id,
            steps = step_count,
            "Workflow execution completed"
        );

        Ok(current_envelope)
    }

    /// Resolve the execution order from the workflow connection graph.
    ///
    /// Performs a topological traversal starting from nodes that have no
    /// incoming connections (trigger nodes), following the connection graph.
    fn resolve_execution_order(&self, workflow: &WorkflowDefinition) -> Vec<WorkflowNode> {
        if workflow.nodes.is_empty() {
            return vec![];
        }

        // Build a map of node_id -> node
        let node_map: std::collections::HashMap<&str, &WorkflowNode> = workflow
            .nodes
            .iter()
            .map(|n| (n.id.as_str(), n))
            .collect();

        // Find all nodes that are targets of connections (have incoming edges)
        let mut has_incoming: std::collections::HashSet<&str> =
            std::collections::HashSet::new();
        for outputs in workflow.connections.values() {
            for output_group in &outputs.main {
                for target in output_group {
                    has_incoming.insert(&target.node);
                }
            }
        }

        // Start nodes: nodes with no incoming connections
        let start_nodes: Vec<&str> = workflow
            .nodes
            .iter()
            .filter(|n| !has_incoming.contains(n.id.as_str()))
            .map(|n| n.id.as_str())
            .collect();

        // BFS traversal
        let mut ordered = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();

        for start in start_nodes {
            queue.push_back(start);
        }

        while let Some(node_id) = queue.pop_front() {
            if visited.contains(node_id) {
                continue;
            }
            visited.insert(node_id);

            if let Some(node) = node_map.get(node_id) {
                ordered.push((*node).clone());
            }

            // Follow connections
            if let Some(outputs) = workflow.connections.get(node_id) {
                for output_group in &outputs.main {
                    for target in output_group {
                        if !visited.contains(target.node.as_str()) {
                            queue.push_back(&target.node);
                        }
                    }
                }
            }
        }

        ordered
    }
}
