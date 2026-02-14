//! Unified execution contract types.
//!
//! These types are the **SOURCE OF TRUTH**.  crewai-rust and ladybug-rs must
//! copy or depend on these definitions exactly.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ============================================================================
// StepStatus
// ============================================================================

/// Status of a single step within a unified execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
}

impl Default for StepStatus {
    fn default() -> Self {
        Self::Pending
    }
}

// ============================================================================
// UnifiedStep
// ============================================================================

/// A single step in a unified execution.
///
/// Each step maps to one n8n node *or* one crew/ladybug delegation.
/// The `step_type` prefix determines routing:
/// - `n8n.*`  → handled by the n8n execution engine
/// - `crew.*` → delegated to crewai-rust via [`CrewRouter`]
/// - `lb.*`   → delegated to ladybug-rs via [`LadybugRouter`]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedStep {
    /// Unique step identifier (UUID).
    pub step_id: String,

    /// Parent execution identifier.
    pub execution_id: String,

    /// Step type with routing prefix (e.g. `crew.agent`, `lb.resonate`, `n8n.set`).
    pub step_type: String,

    /// Human-readable step name (maps to n8n node name).
    pub name: String,

    /// Current status.
    #[serde(default)]
    pub status: StepStatus,

    /// Ordering within the execution (0-based).
    pub sequence: i32,

    /// Input data for this step (arbitrary JSON).
    #[serde(default = "Value::default")]
    pub input: Value,

    /// Output data produced by this step.
    #[serde(default = "Value::default")]
    pub output: Value,

    /// Error message if status == Failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// When the step started executing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,

    /// When the step finished executing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,

    // ----- Decision Trail (crew.agent steps) -----

    /// Reasoning trace from the AI agent (crew.* steps).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,

    /// Agent confidence score (0.0–1.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,

    /// Alternative outputs considered by the agent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alternatives: Option<Value>,
}

impl UnifiedStep {
    /// Create a new pending step.
    pub fn new(
        execution_id: impl Into<String>,
        step_type: impl Into<String>,
        name: impl Into<String>,
        sequence: i32,
    ) -> Self {
        Self {
            step_id: uuid::Uuid::new_v4().to_string(),
            execution_id: execution_id.into(),
            step_type: step_type.into(),
            name: name.into(),
            status: StepStatus::Pending,
            sequence,
            input: Value::Null,
            output: Value::Null,
            error: None,
            started_at: None,
            finished_at: None,
            reasoning: None,
            confidence: None,
            alternatives: None,
        }
    }

    /// Mark this step as running.
    pub fn mark_running(&mut self) {
        self.status = StepStatus::Running;
        self.started_at = Some(Utc::now());
    }

    /// Mark this step as completed with output.
    pub fn mark_completed(&mut self, output: Value) {
        self.status = StepStatus::Completed;
        self.output = output;
        self.finished_at = Some(Utc::now());
    }

    /// Mark this step as failed.
    pub fn mark_failed(&mut self, error: impl Into<String>) {
        self.status = StepStatus::Failed;
        self.error = Some(error.into());
        self.finished_at = Some(Utc::now());
    }

    /// Returns true if this step should be routed to crewai-rust.
    pub fn is_crew(&self) -> bool {
        self.step_type.starts_with("crew.")
    }

    /// Returns true if this step should be routed to ladybug-rs.
    pub fn is_ladybug(&self) -> bool {
        self.step_type.starts_with("lb.")
    }

    /// Returns true if this step is a standard n8n node.
    pub fn is_n8n(&self) -> bool {
        self.step_type.starts_with("n8n.")
    }
}

// ============================================================================
// UnifiedExecution
// ============================================================================

/// A complete unified execution spanning n8n, crew, and ladybug steps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedExecution {
    /// Unique execution identifier (UUID).
    pub execution_id: String,

    /// Workflow name (human-readable).
    pub workflow_name: String,

    /// Overall execution status.
    #[serde(default)]
    pub status: StepStatus,

    /// When the execution started.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,

    /// When the execution finished.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,

    /// Steps in execution order.
    #[serde(default)]
    pub steps: Vec<UnifiedStep>,

    // ----- Fork tracking (ladybug what-if spectator) -----

    /// Fork identifier for what-if branching.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fork_id: Option<String>,

    /// Parent execution this was forked from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fork_parent: Option<String>,
}

impl UnifiedExecution {
    /// Create a new pending execution.
    pub fn new(workflow_name: impl Into<String>) -> Self {
        Self {
            execution_id: uuid::Uuid::new_v4().to_string(),
            workflow_name: workflow_name.into(),
            status: StepStatus::Pending,
            started_at: None,
            finished_at: None,
            steps: Vec::new(),
            fork_id: None,
            fork_parent: None,
        }
    }

    /// Create a forked execution from a parent.
    pub fn fork(parent_id: impl Into<String>, workflow_name: impl Into<String>) -> Self {
        let mut exec = Self::new(workflow_name);
        exec.fork_id = Some(uuid::Uuid::new_v4().to_string());
        exec.fork_parent = Some(parent_id.into());
        exec
    }

    /// Mark this execution as running.
    pub fn mark_running(&mut self) {
        self.status = StepStatus::Running;
        self.started_at = Some(Utc::now());
    }

    /// Mark this execution as completed.
    pub fn mark_completed(&mut self) {
        self.status = StepStatus::Completed;
        self.finished_at = Some(Utc::now());
    }

    /// Mark this execution as failed.
    pub fn mark_failed(&mut self) {
        self.status = StepStatus::Failed;
        self.finished_at = Some(Utc::now());
    }
}

// ============================================================================
// DataEnvelope
// ============================================================================

/// Metadata attached to a data envelope flowing between steps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvelopeMetadata {
    /// Source step that produced this envelope.
    pub source_step: String,

    /// Agent confidence score (0.0–1.0).
    #[serde(default)]
    pub confidence: f64,

    /// Monotonic epoch counter for ordering.
    #[serde(default)]
    pub epoch: i64,

    /// Schema version tag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    // --- 10-Layer Cognitive Awareness (backward-compatible) ---

    /// Dominant cognitive layer (0-9 → L1-L10) that produced this output.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub dominant_layer: Option<u8>,

    /// 10-layer activation snapshot: [f32; 10] for cross-agent awareness.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub layer_activations: Option<Vec<f32>>,

    /// NARS frequency from L9 validation.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub nars_frequency: Option<f64>,

    /// Calibration error (Brier score) from MetaCognition.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub calibration_error: Option<f64>,
}

/// A data envelope that flows between steps in a unified execution.
///
/// This is the standard wire format passed between n8n nodes, crew agents,
/// and ladybug operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataEnvelope {
    /// Payload data (arbitrary JSON, typically an array of items).
    pub data: Value,

    /// Envelope metadata.
    pub metadata: EnvelopeMetadata,
}

impl DataEnvelope {
    /// Create a new envelope from a step output.
    pub fn new(data: Value, source_step: impl Into<String>) -> Self {
        Self {
            data,
            metadata: EnvelopeMetadata {
                source_step: source_step.into(),
                confidence: 1.0,
                epoch: Utc::now().timestamp_millis(),
                version: None,
                dominant_layer: None,
                layer_activations: None,
                nars_frequency: None,
                calibration_error: None,
            },
        }
    }
}

// ============================================================================
// Router Request / Response
// ============================================================================

/// Request body sent to crewai-rust or ladybug-rs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepDelegationRequest {
    /// The step to execute.
    pub step: UnifiedStep,

    /// Input envelope.
    pub input: DataEnvelope,
}

/// Response body returned by crewai-rust or ladybug-rs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepDelegationResponse {
    /// Output envelope.
    pub output: DataEnvelope,

    /// Updated step (with status, reasoning, confidence, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<UnifiedStep>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step_status_serde_roundtrip() {
        let json = serde_json::to_string(&StepStatus::Completed).unwrap();
        assert_eq!(json, "\"completed\"");

        let back: StepStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(back, StepStatus::Completed);
    }

    #[test]
    fn test_unified_step_routing_prefixes() {
        let crew = UnifiedStep::new("e1", "crew.agent", "Research", 0);
        assert!(crew.is_crew());
        assert!(!crew.is_ladybug());
        assert!(!crew.is_n8n());

        let lb = UnifiedStep::new("e1", "lb.resonate", "Resonate", 1);
        assert!(lb.is_ladybug());

        let n8n = UnifiedStep::new("e1", "n8n.set", "Set", 2);
        assert!(n8n.is_n8n());
    }

    #[test]
    fn test_backward_compat_deserialize_without_new_fields() {
        // Old JSON without reasoning/confidence/alternatives/fork_id/fork_parent
        let old_step_json = r#"{
            "step_id": "abc",
            "execution_id": "e1",
            "step_type": "crew.agent",
            "name": "Research",
            "status": "completed",
            "sequence": 0,
            "input": null,
            "output": {"result": "done"}
        }"#;

        let step: UnifiedStep = serde_json::from_str(old_step_json).unwrap();
        assert_eq!(step.status, StepStatus::Completed);
        assert!(step.reasoning.is_none());
        assert!(step.confidence.is_none());
        assert!(step.alternatives.is_none());

        let old_exec_json = r#"{
            "execution_id": "e1",
            "workflow_name": "test",
            "status": "completed",
            "steps": []
        }"#;

        let exec: UnifiedExecution = serde_json::from_str(old_exec_json).unwrap();
        assert!(exec.fork_id.is_none());
        assert!(exec.fork_parent.is_none());
    }

    #[test]
    fn test_new_fields_serialize_roundtrip() {
        let mut step = UnifiedStep::new("e1", "crew.agent", "Research", 0);
        step.reasoning = Some("Used web search to find latest papers".into());
        step.confidence = Some(0.92);
        step.alternatives = Some(serde_json::json!(["approach A", "approach B"]));

        let json = serde_json::to_string(&step).unwrap();
        let back: UnifiedStep = serde_json::from_str(&json).unwrap();
        assert_eq!(back.reasoning.as_deref(), Some("Used web search to find latest papers"));
        assert_eq!(back.confidence, Some(0.92));
        assert!(back.alternatives.is_some());
    }

    #[test]
    fn test_fork_execution() {
        let exec = UnifiedExecution::fork("parent-123", "forked-workflow");
        assert!(exec.fork_id.is_some());
        assert_eq!(exec.fork_parent.as_deref(), Some("parent-123"));
    }

    #[test]
    fn test_data_envelope_serde() {
        let env = DataEnvelope::new(serde_json::json!({"key": "value"}), "step-1");
        let json = serde_json::to_string(&env).unwrap();
        let back: DataEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(back.metadata.source_step, "step-1");
        assert_eq!(back.metadata.confidence, 1.0);
    }

    #[test]
    fn test_delegation_request_serde() {
        let step = UnifiedStep::new("e1", "crew.agent", "Research", 0);
        let input = DataEnvelope::new(serde_json::json!({"query": "rust"}), "trigger");
        let req = StepDelegationRequest { step, input };

        let json = serde_json::to_string(&req).unwrap();
        let back: StepDelegationRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.step.step_type, "crew.agent");
    }
}
