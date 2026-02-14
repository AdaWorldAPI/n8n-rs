//! Bridge between n8n-rs local contract types and `ladybug-contract` V1 types.
//!
//! The V1 types in `ladybug-contract` are the canonical wire-format types shared
//! across all three runtimes. This module provides `From` conversions so that
//! n8n-rs can emit / ingest the canonical format while keeping its richer local
//! types (with UUID constructors, lifecycle helpers, etc.).

use ladybug_contract::{
    V1DataEnvelope, V1EnvelopeMetadata, V1StepDelegationRequest, V1StepDelegationResponse,
    V1StepStatus, V1UnifiedStep,
};

use crate::types::{
    DataEnvelope, EnvelopeMetadata, StepDelegationRequest, StepDelegationResponse, StepStatus,
    UnifiedStep,
};

// ============================================================================
// StepStatus ↔ V1StepStatus
// ============================================================================

impl From<StepStatus> for V1StepStatus {
    fn from(s: StepStatus) -> Self {
        match s {
            StepStatus::Pending => V1StepStatus::Pending,
            StepStatus::Running => V1StepStatus::Running,
            StepStatus::Completed => V1StepStatus::Completed,
            StepStatus::Failed => V1StepStatus::Failed,
            StepStatus::Skipped => V1StepStatus::Skipped,
        }
    }
}

impl From<V1StepStatus> for StepStatus {
    fn from(s: V1StepStatus) -> Self {
        match s {
            V1StepStatus::Pending => StepStatus::Pending,
            V1StepStatus::Running => StepStatus::Running,
            V1StepStatus::Completed => StepStatus::Completed,
            V1StepStatus::Failed => StepStatus::Failed,
            V1StepStatus::Skipped => StepStatus::Skipped,
        }
    }
}

// ============================================================================
// UnifiedStep ↔ V1UnifiedStep
// ============================================================================

impl From<&UnifiedStep> for V1UnifiedStep {
    fn from(s: &UnifiedStep) -> Self {
        V1UnifiedStep {
            step_id: s.step_id.clone(),
            execution_id: s.execution_id.clone(),
            step_type: s.step_type.clone(),
            name: s.name.clone(),
            status: s.status.into(),
            sequence: s.sequence,
            input: s.input.clone(),
            output: s.output.clone(),
            error: s.error.clone(),
            started_at: s.started_at,
            finished_at: s.finished_at,
            reasoning: s.reasoning.clone(),
            confidence: s.confidence,
            alternatives: s.alternatives.clone(),
        }
    }
}

impl From<V1UnifiedStep> for UnifiedStep {
    fn from(v: V1UnifiedStep) -> Self {
        UnifiedStep {
            step_id: v.step_id,
            execution_id: v.execution_id,
            step_type: v.step_type,
            name: v.name,
            status: v.status.into(),
            sequence: v.sequence,
            input: v.input,
            output: v.output,
            error: v.error,
            started_at: v.started_at,
            finished_at: v.finished_at,
            reasoning: v.reasoning,
            confidence: v.confidence,
            alternatives: v.alternatives,
        }
    }
}

// ============================================================================
// DataEnvelope ↔ V1DataEnvelope
// ============================================================================

impl From<&DataEnvelope> for V1DataEnvelope {
    fn from(e: &DataEnvelope) -> Self {
        V1DataEnvelope {
            data: e.data.clone(),
            metadata: V1EnvelopeMetadata {
                source_step: e.metadata.source_step.clone(),
                confidence: e.metadata.confidence,
                epoch: e.metadata.epoch,
                version: e.metadata.version.clone(),
                dominant_layer: e.metadata.dominant_layer,
                layer_activations: e.metadata.layer_activations.clone(),
                nars_frequency: e.metadata.nars_frequency,
                calibration_error: e.metadata.calibration_error,
            },
        }
    }
}

impl From<V1DataEnvelope> for DataEnvelope {
    fn from(v: V1DataEnvelope) -> Self {
        DataEnvelope {
            data: v.data,
            metadata: EnvelopeMetadata {
                source_step: v.metadata.source_step,
                confidence: v.metadata.confidence,
                epoch: v.metadata.epoch,
                version: v.metadata.version,
                dominant_layer: v.metadata.dominant_layer,
                layer_activations: v.metadata.layer_activations,
                nars_frequency: v.metadata.nars_frequency,
                calibration_error: v.metadata.calibration_error,
            },
        }
    }
}

// ============================================================================
// StepDelegationRequest ↔ V1StepDelegationRequest
// ============================================================================

impl From<&StepDelegationRequest> for V1StepDelegationRequest {
    fn from(r: &StepDelegationRequest) -> Self {
        V1StepDelegationRequest {
            step: (&r.step).into(),
            input: (&r.input).into(),
        }
    }
}

impl From<V1StepDelegationRequest> for StepDelegationRequest {
    fn from(v: V1StepDelegationRequest) -> Self {
        StepDelegationRequest {
            step: v.step.into(),
            input: v.input.into(),
        }
    }
}

// ============================================================================
// StepDelegationResponse ↔ V1StepDelegationResponse
// ============================================================================

impl From<&StepDelegationResponse> for V1StepDelegationResponse {
    fn from(r: &StepDelegationResponse) -> Self {
        V1StepDelegationResponse {
            output: (&r.output).into(),
            step: r.step.as_ref().map(|s| s.into()),
        }
    }
}

impl From<V1StepDelegationResponse> for StepDelegationResponse {
    fn from(v: V1StepDelegationResponse) -> Self {
        StepDelegationResponse {
            output: v.output.into(),
            step: v.step.map(|s| s.into()),
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_step_status_roundtrip() {
        for status in [
            StepStatus::Pending,
            StepStatus::Running,
            StepStatus::Completed,
            StepStatus::Failed,
            StepStatus::Skipped,
        ] {
            let v1: V1StepStatus = status.into();
            let back: StepStatus = v1.into();
            assert_eq!(back, status);
        }
    }

    #[test]
    fn test_unified_step_roundtrip() {
        let step = UnifiedStep::new("exec-1", "n8n.set", "Set Values", 0);
        let v1: V1UnifiedStep = (&step).into();
        let back: UnifiedStep = v1.into();
        assert_eq!(back.step_id, step.step_id);
        assert_eq!(back.step_type, "n8n.set");
        assert_eq!(back.name, "Set Values");
    }

    #[test]
    fn test_envelope_roundtrip() {
        let env = DataEnvelope::new(json!({"key": "value"}), "step-1");
        let v1: V1DataEnvelope = (&env).into();
        let back: DataEnvelope = v1.into();
        assert_eq!(back.metadata.source_step, "step-1");
        assert_eq!(back.data["key"], "value");
    }

    #[test]
    fn test_delegation_roundtrip() {
        let step = UnifiedStep::new("e1", "lb.resonate", "Resonate", 0);
        let input = DataEnvelope::new(json!({"query": "search term"}), "trigger");
        let req = StepDelegationRequest { step, input };
        let v1: V1StepDelegationRequest = (&req).into();
        let back: StepDelegationRequest = v1.into();
        assert_eq!(back.step.step_type, "lb.resonate");
    }
}
