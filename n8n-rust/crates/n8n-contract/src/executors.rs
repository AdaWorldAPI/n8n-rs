//! NodeExecutor adapters for crew and ladybug step delegation.
//!
//! These implement the [`n8n_core::NodeExecutor`] trait so the n8n workflow
//! engine can seamlessly route `crew.*` and `lb.*` node types to their
//! respective external services.

use crate::crew_router::CrewRouter;
use crate::envelope;
use crate::ladybug_router::LadybugRouter;
use crate::types::UnifiedStep;
use async_trait::async_trait;
use n8n_core::error::ExecutionEngineError;
use n8n_core::executor::NodeOutput;
use n8n_core::NodeExecutor;
use n8n_core::runtime::RuntimeContext;
use n8n_workflow::{Node, TaskDataConnections};
use tracing::{debug, warn};

// ============================================================================
// CrewAgentExecutor
// ============================================================================

/// Executor for `crew.agent` nodes.
///
/// Delegates to crewai-rust via HTTP and translates the response back into
/// n8n execution items.
pub struct CrewAgentExecutor {
    router: CrewRouter,
}

impl CrewAgentExecutor {
    pub fn new(router: CrewRouter) -> Self {
        Self { router }
    }
}

#[async_trait]
impl NodeExecutor for CrewAgentExecutor {
    fn node_type(&self) -> &str {
        "crew.agent"
    }

    async fn execute(
        &self,
        node: &Node,
        input: &TaskDataConnections,
        _context: &RuntimeContext,
    ) -> Result<NodeOutput, ExecutionEngineError> {
        let items = input
            .get("main")
            .and_then(|v| v.first())
            .cloned()
            .unwrap_or_default();

        // Build envelope from n8n items
        let input_envelope = envelope::from_n8n_output(&items, &node.name);

        // Build step
        let mut step = UnifiedStep::new("", &node.node_type, &node.name, 0);
        step.input = input_envelope.data.clone();

        // Merge node parameters into step input
        if let Some(role) = node.get_parameter("role") {
            if let n8n_workflow::NodeParameterValue::String(s) = role {
                step.input = serde_json::json!({
                    "items": input_envelope.data,
                    "role": s,
                });
            }
        }

        debug!(node = %node.name, "Delegating to crewai-rust");

        match self.router.execute(&step, &input_envelope).await {
            Ok(response) => {
                // Convert output envelope back to n8n items
                let output_items = envelope::to_n8n_items(&response.output);

                // Log decision trail if present
                if let Some(ref returned_step) = response.step {
                    if let Some(ref reasoning) = returned_step.reasoning {
                        debug!(node = %node.name, reasoning = %reasoning, "Agent reasoning");
                    }
                }

                Ok(vec![output_items])
            }
            Err(e) => {
                warn!(node = %node.name, error = %e, "CrewAgent delegation failed");
                Err(ExecutionEngineError::NodeExecution {
                    node: node.name.clone(),
                    message: format!("crew.agent delegation failed: {e}"),
                })
            }
        }
    }
}

// ============================================================================
// LadybugResonateExecutor
// ============================================================================

/// Executor for `lb.resonate` nodes (CAM resonance operation).
pub struct LadybugResonateExecutor {
    router: LadybugRouter,
}

impl LadybugResonateExecutor {
    pub fn new(router: LadybugRouter) -> Self {
        Self { router }
    }
}

#[async_trait]
impl NodeExecutor for LadybugResonateExecutor {
    fn node_type(&self) -> &str {
        "lb.resonate"
    }

    async fn execute(
        &self,
        node: &Node,
        input: &TaskDataConnections,
        _context: &RuntimeContext,
    ) -> Result<NodeOutput, ExecutionEngineError> {
        let items = input
            .get("main")
            .and_then(|v| v.first())
            .cloned()
            .unwrap_or_default();

        let input_envelope = envelope::from_n8n_output(&items, &node.name);
        let step = UnifiedStep::new("", "lb.resonate", &node.name, 0);

        debug!(node = %node.name, "Delegating to ladybug-rs resonate");

        match self.router.execute(&step, &input_envelope).await {
            Ok(response) => {
                let output_items = envelope::to_n8n_items(&response.output);
                Ok(vec![output_items])
            }
            Err(e) => {
                warn!(node = %node.name, error = %e, "Ladybug resonate failed");
                Err(ExecutionEngineError::NodeExecution {
                    node: node.name.clone(),
                    message: format!("lb.resonate delegation failed: {e}"),
                })
            }
        }
    }
}

// ============================================================================
// LadybugCollapseExecutor
// ============================================================================

/// Executor for `lb.collapse` nodes (CAM collapse operation).
pub struct LadybugCollapseExecutor {
    router: LadybugRouter,
}

impl LadybugCollapseExecutor {
    pub fn new(router: LadybugRouter) -> Self {
        Self { router }
    }
}

#[async_trait]
impl NodeExecutor for LadybugCollapseExecutor {
    fn node_type(&self) -> &str {
        "lb.collapse"
    }

    async fn execute(
        &self,
        node: &Node,
        input: &TaskDataConnections,
        _context: &RuntimeContext,
    ) -> Result<NodeOutput, ExecutionEngineError> {
        let items = input
            .get("main")
            .and_then(|v| v.first())
            .cloned()
            .unwrap_or_default();

        let input_envelope = envelope::from_n8n_output(&items, &node.name);
        let step = UnifiedStep::new("", "lb.collapse", &node.name, 0);

        debug!(node = %node.name, "Delegating to ladybug-rs collapse");

        match self.router.execute(&step, &input_envelope).await {
            Ok(response) => {
                let output_items = envelope::to_n8n_items(&response.output);
                Ok(vec![output_items])
            }
            Err(e) => {
                warn!(node = %node.name, error = %e, "Ladybug collapse failed");
                Err(ExecutionEngineError::NodeExecution {
                    node: node.name.clone(),
                    message: format!("lb.collapse delegation failed: {e}"),
                })
            }
        }
    }
}
