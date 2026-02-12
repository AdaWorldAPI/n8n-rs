//! Unified execution contract types
//!
//! These types are shared across all 3 repos (ada-n8n, crewai-rust, ladybug-rs).
//! Keep them identical — they must serialize to identical JSON across all repos.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ═══════════════════════════════════════════════════════════════════════════
// Step Status
// ═══════════════════════════════════════════════════════════════════════════

/// Status of a unified execution step
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
}

impl std::fmt::Display for StepStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StepStatus::Pending => write!(f, "pending"),
            StepStatus::Running => write!(f, "running"),
            StepStatus::Completed => write!(f, "completed"),
            StepStatus::Failed => write!(f, "failed"),
            StepStatus::Skipped => write!(f, "skipped"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Unified Step
// ═══════════════════════════════════════════════════════════════════════════

/// A single step in a unified execution.
///
/// Represents work done by any runtime: n8n node, crewAI agent task,
/// or ladybug enrichment step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedStep {
    /// Unique step identifier (UUID)
    pub step_id: String,

    /// Parent execution ID
    pub execution_id: String,

    /// Step type with runtime prefix: "n8n.httpRequest", "crew.agent", "lb.index"
    pub step_type: String,

    /// Which runtime executed this step: "n8n", "crewai", "ladybug"
    pub runtime: String,

    /// Human-readable step name
    pub name: String,

    /// Step status
    pub status: StepStatus,

    /// Input parameters / configuration for this step
    pub input: Value,

    /// Output data from this step
    pub output: Value,

    /// Error message if status == Failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// When step started
    pub started_at: DateTime<Utc>,

    /// When step finished
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,

    /// Execution order / sequence number
    pub sequence: i32,
}

// ═══════════════════════════════════════════════════════════════════════════
// Unified Execution
// ═══════════════════════════════════════════════════════════════════════════

/// A unified execution record spanning one or more runtimes.
///
/// Tracks the full lifecycle of a workflow execution that may involve
/// n8n nodes, crewAI agents, and ladybug enrichment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedExecution {
    /// Unique execution identifier (UUID)
    pub execution_id: String,

    /// Primary runtime that initiated this execution: "n8n", "crewai", "ladybug"
    pub runtime: String,

    /// Workflow or crew definition name
    pub workflow_name: String,

    /// Execution status (mirrors StepStatus for the overall execution)
    pub status: StepStatus,

    /// Trigger that started this execution (e.g., "webhook", "schedule", "manual")
    pub trigger: String,

    /// Input data provided to the execution
    pub input: Value,

    /// Final output data from the execution
    pub output: Value,

    /// When execution started
    pub started_at: DateTime<Utc>,

    /// When execution finished
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,

    /// Total number of steps
    pub step_count: i32,
}

// ═══════════════════════════════════════════════════════════════════════════
// Envelope Metadata
// ═══════════════════════════════════════════════════════════════════════════

/// Metadata attached to a data envelope for provenance tracking.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EnvelopeMetadata {
    /// Agent ID if produced by a crewAI agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,

    /// Confidence score (0.0–1.0) if produced by an AI agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,

    /// Epoch / version counter for ladybug indexing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epoch: Option<i64>,

    /// Schema version for forward-compatibility
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

// ═══════════════════════════════════════════════════════════════════════════
// Data Envelope
// ═══════════════════════════════════════════════════════════════════════════

/// A data envelope wrapping step output for cross-runtime transport.
///
/// Every step produces a DataEnvelope that the next step consumes.
/// This is the lingua franca between n8n nodes, crewAI agents, and
/// ladybug enrichment steps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataEnvelope {
    /// Step ID that produced this envelope
    pub step_id: String,

    /// Output key (e.g., "node_id.output", "agent.result")
    pub output_key: String,

    /// MIME content type (e.g., "application/json", "text/plain")
    pub content_type: String,

    /// The actual payload data
    pub content: Value,

    /// Provenance metadata
    pub metadata: EnvelopeMetadata,
}
