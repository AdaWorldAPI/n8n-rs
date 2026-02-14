//! Binary Wire Protocol Bridge for n8n-rs.
//!
//! Converts between n8n-rs's local types and CogPacket binary format.
//! This replaces JSON serialization for **all internal communication**.
//!
//! External consumers (REST API, webhooks) still receive JSON, but every
//! internal delegation — n8n→ladybug, n8n→crewai, crewai→ladybug — uses
//! binary CogPackets routed by 8+8 addressing and 4096-opcode dispatch.
//!
//! ```text
//! External (JSON/YAML)        Internal (Binary CogPacket)
//! ════════════════════        ═══════════════════════════
//!     REST API  ──►  wire_bridge::ingest()  ──►  CogPacket
//!     YAML Def  ──►       │                          │
//!                          │    ┌────────────────────┘
//!                          │    ▼
//!                       CognitiveKernel.process_packet()
//!                               │
//!                               ▼
//!                       CogPacket (response)
//!                               │
//!                    wire_bridge::emit()  ──►  JSON response
//! ```

use ladybug_contract::container::Container;
use ladybug_contract::nars::TruthValue;
use ladybug_contract::wire::{self, CogPacket};

use crate::types::{
    DataEnvelope, EnvelopeMetadata, StepDelegationRequest, StepDelegationResponse,
    StepStatus, UnifiedStep,
};

// =============================================================================
// INGESTION — External → Binary
// =============================================================================

/// Convert a StepDelegationRequest to a CogPacket for internal routing.
///
/// The step_type prefix routes to the correct 8+8 address space:
/// - `crew.*` → 0x0C (Agents domain)
/// - `lb.*`   → 0x05 (Causal) for resonate, 0x80+ (Node) for collapse
/// - `n8n.*`  → 0x0F (A2A domain)
pub fn ingest(request: &StepDelegationRequest) -> CogPacket {
    let step_type = &request.step.step_type;

    // Determine source/target addresses from step_type
    let (source_prefix, opcode) = route_step_type(step_type);
    let source_addr = (source_prefix as u16) << 8;
    let target_addr = source_addr | 0x01;

    // Hash the input data to a Container
    let content_hash = hash_json_to_u64(&request.input.data);
    let content = Container::random(content_hash);

    let mut pkt = CogPacket::request(opcode, source_addr, target_addr, content);

    // Pack metadata into header
    pkt.set_cycle(request.input.metadata.epoch as u64);

    // Pack confidence as NARS truth value
    let conf = request.input.metadata.confidence as f32;
    if conf > 0.0 {
        pkt.set_truth_value(&TruthValue::new(1.0, conf));
    }

    // Pack dominant layer
    if let Some(layer) = request.input.metadata.dominant_layer {
        pkt.set_layer(layer);
    }

    // Pack layer activations as satisfaction scores
    if let Some(ref activations) = request.input.metadata.layer_activations {
        for (i, &a) in activations.iter().enumerate().take(10) {
            pkt.set_satisfaction(i as u8, a);
        }
    }

    // Pack NARS frequency
    if let Some(freq) = request.input.metadata.nars_frequency {
        let tv = pkt.truth_value();
        pkt.set_truth_value(&TruthValue::new(freq as f32, tv.confidence));
    }

    pkt.set_flags(pkt.flags() | wire::FLAG_DELEGATION);
    pkt.update_checksum();
    pkt
}

/// Batch-ingest a workflow's steps into a vector of CogPackets.
///
/// This is used when n8n executes an entire workflow — each step becomes
/// a CogPacket that can be routed through the binary substrate in parallel.
pub fn ingest_workflow(steps: &[(UnifiedStep, DataEnvelope)]) -> Vec<CogPacket> {
    steps
        .iter()
        .map(|(step, input)| {
            let request = StepDelegationRequest {
                step: step.clone(),
                input: input.clone(),
            };
            ingest(&request)
        })
        .collect()
}

// =============================================================================
// EMISSION — Binary → External
// =============================================================================

/// Convert a CogPacket response back to a StepDelegationResponse.
///
/// This is the egress path — binary → JSON for external consumers.
pub fn emit(response: &CogPacket, original_step: &UnifiedStep) -> StepDelegationResponse {
    let tv = response.truth_value();
    let sat = response.satisfaction_array();

    let mut step = original_step.clone();
    step.status = if response.is_error() {
        StepStatus::Failed
    } else {
        StepStatus::Completed
    };
    step.confidence = Some(tv.confidence as f64);

    let metadata = EnvelopeMetadata {
        source_step: step.step_id.clone(),
        confidence: tv.confidence as f64,
        epoch: response.cycle() as i64,
        version: Some(format!("wire-v{}", wire::WIRE_VERSION)),
        dominant_layer: Some(response.layer()),
        layer_activations: Some(sat.to_vec()),
        nars_frequency: Some(tv.frequency as f64),
        calibration_error: None,
    };

    let output = DataEnvelope {
        data: serde_json::json!({
            "opcode": response.opcode(),
            "cycle": response.cycle(),
            "crystallized": response.flags() & wire::FLAG_CRYSTALLIZED != 0,
            "validated": response.flags() & wire::FLAG_VALIDATED != 0,
            "rung": response.rung(),
            "source_addr": format!("{:#06x}", response.source_addr()),
            "target_addr": format!("{:#06x}", response.target_addr()),
        }),
        metadata,
    };

    StepDelegationResponse {
        output,
        step: Some(step),
    }
}

// =============================================================================
// WORKFLOW PACKET CREATION
// =============================================================================

/// Create a CogPacket for an n8n workflow orchestration event.
///
/// n8n-rs sits at L7 (Orchestration) — it coordinates multi-agent
/// pipelines across crew agents and ladybug cognitive operations.
pub fn pack_orchestration_event(
    workflow_name: &str,
    node_count: usize,
    confidence: f64,
) -> CogPacket {
    let content_hash = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        workflow_name.hash(&mut h);
        h.finish()
    };

    let content = Container::random(content_hash);

    // n8n lives in the A2A domain (0x0F)
    let source_addr = 0x0F00u16;
    let target_addr = 0x0F01u16;

    let mut pkt = CogPacket::request(
        wire::wire_ops::ROUTE,
        source_addr,
        target_addr,
        content,
    );

    pkt.set_layer(6); // L7 Orchestration (0-indexed = 6)
    pkt.set_truth_value(&TruthValue::new(1.0, confidence as f32));
    pkt.set_fan_out(node_count.min(255) as u8);

    pkt.update_checksum();
    pkt
}

/// Create a CogPacket for delegating from n8n to crewai-rust.
///
/// This replaces the HTTP JSON round-trip in CrewRouter.execute().
pub fn pack_crew_delegation(
    step: &UnifiedStep,
    input: &DataEnvelope,
) -> CogPacket {
    let request = StepDelegationRequest {
        step: step.clone(),
        input: input.clone(),
    };
    ingest(&request)
}

/// Create a CogPacket for delegating from n8n to ladybug-rs.
///
/// This replaces the HTTP JSON round-trip in LadybugRouter.execute().
pub fn pack_ladybug_delegation(
    step: &UnifiedStep,
    input: &DataEnvelope,
) -> CogPacket {
    let request = StepDelegationRequest {
        step: step.clone(),
        input: input.clone(),
    };
    ingest(&request)
}

// =============================================================================
// HELPERS
// =============================================================================

/// Route step_type to (prefix, opcode).
fn route_step_type(step_type: &str) -> (u8, u16) {
    match step_type.split('.').next() {
        Some("crew") => (0x0C, wire::wire_ops::DELEGATE),
        Some("lb") => {
            if step_type.contains("resonate") {
                (0x05, wire::wire_ops::RESONATE)
            } else if step_type.contains("collapse") {
                (0x05, wire::wire_ops::COLLAPSE)
            } else {
                (0x05, wire::wire_ops::EXECUTE)
            }
        }
        Some("n8n") => (0x0F, wire::wire_ops::EXECUTE),
        _ => (0x0F, wire::wire_ops::EXECUTE),
    }
}

/// Hash JSON value to u64 for Container seeding.
fn hash_json_to_u64(value: &serde_json::Value) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let s = serde_json::to_string(value).unwrap_or_default();
    s.hash(&mut h);
    h.finish()
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_request(step_type: &str) -> StepDelegationRequest {
        StepDelegationRequest {
            step: UnifiedStep {
                step_id: "test-1".into(),
                execution_id: "exec-1".into(),
                step_type: step_type.into(),
                name: "TestStep".into(),
                status: StepStatus::Pending,
                sequence: 0,
                input: serde_json::Value::Null,
                output: serde_json::Value::Null,
                error: None,
                started_at: None,
                finished_at: None,
                reasoning: None,
                confidence: Some(0.9),
                alternatives: None,
            },
            input: DataEnvelope {
                data: serde_json::json!({"query": "test"}),
                metadata: EnvelopeMetadata {
                    source_step: "trigger".into(),
                    confidence: 0.9,
                    epoch: 42,
                    version: None,
                    dominant_layer: Some(5),
                    layer_activations: None,
                    nars_frequency: None,
                    calibration_error: None,
                },
            },
        }
    }

    #[test]
    fn test_ingest_crew_step() {
        let request = make_test_request("crew.agent");
        let pkt = ingest(&request);
        assert!(pkt.verify_magic());
        assert_eq!(pkt.opcode(), wire::wire_ops::DELEGATE);
        assert_eq!(pkt.source_prefix(), 0x0C);
        assert!(pkt.is_delegation());
    }

    #[test]
    fn test_ingest_lb_resonate_step() {
        let request = make_test_request("lb.resonate");
        let pkt = ingest(&request);
        assert!(pkt.verify_magic());
        assert_eq!(pkt.opcode(), wire::wire_ops::RESONATE);
        assert_eq!(pkt.source_prefix(), 0x05);
    }

    #[test]
    fn test_ingest_lb_collapse_step() {
        let request = make_test_request("lb.collapse");
        let pkt = ingest(&request);
        assert!(pkt.verify_magic());
        assert_eq!(pkt.opcode(), wire::wire_ops::COLLAPSE);
        assert_eq!(pkt.source_prefix(), 0x05);
    }

    #[test]
    fn test_ingest_n8n_step() {
        let request = make_test_request("n8n.set");
        let pkt = ingest(&request);
        assert!(pkt.verify_magic());
        assert_eq!(pkt.opcode(), wire::wire_ops::EXECUTE);
        assert_eq!(pkt.source_prefix(), 0x0F);
    }

    #[test]
    fn test_emit_response() {
        let content = Container::random(42);
        let mut response = CogPacket::response(wire::wire_ops::EXECUTE, 0x8001, 0x0C00, content);
        response.set_layer(4);
        response.set_truth_value(&TruthValue::new(0.85, 0.92));
        response.set_flags(response.flags() | wire::FLAG_VALIDATED);
        response.update_checksum();

        let step = UnifiedStep::new("exec-1", "crew.agent", "Research", 0);
        let delegation_response = emit(&response, &step);
        assert_eq!(delegation_response.step.unwrap().status, StepStatus::Completed);
        assert!(delegation_response.output.metadata.confidence > 0.9);
        assert_eq!(delegation_response.output.metadata.dominant_layer, Some(4));
    }

    #[test]
    fn test_pack_orchestration_event() {
        let pkt = pack_orchestration_event("research-pipeline", 5, 0.95);
        assert!(pkt.verify_magic());
        assert_eq!(pkt.opcode(), wire::wire_ops::ROUTE);
        assert_eq!(pkt.layer(), 6); // L7 Orchestration
        assert_eq!(pkt.source_prefix(), 0x0F);
        assert_eq!(pkt.fan_out(), 5);
    }

    #[test]
    fn test_ingest_workflow_batch() {
        let steps = vec![
            (
                UnifiedStep::new("e1", "crew.agent", "Agent1", 0),
                DataEnvelope::new(serde_json::json!({"a": 1}), "trigger"),
            ),
            (
                UnifiedStep::new("e1", "lb.resonate", "Resonate1", 1),
                DataEnvelope::new(serde_json::json!({"b": 2}), "agent1"),
            ),
        ];

        let packets = ingest_workflow(&steps);
        assert_eq!(packets.len(), 2);
        assert_eq!(packets[0].opcode(), wire::wire_ops::DELEGATE);
        assert_eq!(packets[1].opcode(), wire::wire_ops::RESONATE);
    }

    #[test]
    fn test_roundtrip_ingest_emit() {
        let request = make_test_request("crew.agent");
        let pkt = ingest(&request);

        // Simulate a response based on the request packet
        let content = Container::random(99);
        let mut response = CogPacket::response(
            pkt.opcode(),
            pkt.target_addr(),
            pkt.source_addr(),
            content,
        );
        response.set_layer(pkt.layer());
        response.set_truth_value(&TruthValue::new(0.95, 0.88));
        response.set_flags(response.flags() | wire::FLAG_VALIDATED);
        response.update_checksum();

        let result = emit(&response, &request.step);
        assert_eq!(result.step.as_ref().unwrap().status, StepStatus::Completed);
        assert!(result.output.metadata.nars_frequency.unwrap() > 0.9);
    }
}
