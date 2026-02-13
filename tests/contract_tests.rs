//! Tests for the unified execution contract
//!
//! - DataEnvelope round-trip: n8n items → envelope → n8n items
//! - Workflow JSON parsing with mixed n8n + crew step types
//! - Crew step routing with mock HTTP endpoint
//! - Unknown step types (lb.*) pass through gracefully
//! - PgStore write/read (same tests as crewai-rust — both repos should pass)

use ada_n8n::contract::types::*;
use chrono::Utc;
use serde_json::json;

// ═══════════════════════════════════════════════════════════════════════════
// DataEnvelope round-trip tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_envelope_from_n8n_output() {
    let items = json!([
        {"name": "Alice", "score": 95},
        {"name": "Bob", "score": 87}
    ]);

    let envelope = DataEnvelope::from_n8n_output("node-1", &items);

    assert_eq!(envelope.step_id, "node-1");
    assert_eq!(envelope.output_key, "node-1.output");
    assert_eq!(envelope.content_type, "application/json");
    assert_eq!(envelope.content, items);
    assert!(envelope.metadata.agent_id.is_none());
    assert!(envelope.metadata.confidence.is_none());
}

#[test]
fn test_envelope_to_n8n_items_array() {
    // Array of plain objects → each wrapped in {"json": ...}
    let items = json!([
        {"name": "Alice"},
        {"name": "Bob"}
    ]);
    let envelope = DataEnvelope::from_n8n_output("node-1", &items);
    let n8n_items = envelope.to_n8n_items();

    assert!(n8n_items.is_array());
    let arr = n8n_items.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["json"]["name"], "Alice");
    assert_eq!(arr[1]["json"]["name"], "Bob");
}

#[test]
fn test_envelope_to_n8n_items_already_wrapped() {
    // Items already in n8n format → pass through
    let items = json!([
        {"json": {"name": "Alice"}},
        {"json": {"name": "Bob"}}
    ]);
    let envelope = DataEnvelope::from_n8n_output("node-1", &items);
    let n8n_items = envelope.to_n8n_items();

    assert!(n8n_items.is_array());
    let arr = n8n_items.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["json"]["name"], "Alice");
}

#[test]
fn test_envelope_to_n8n_items_single_object() {
    // Single object → wrapped as [{"json": obj}]
    let obj = json!({"result": "success"});
    let envelope = DataEnvelope::from_n8n_output("node-1", &obj);
    let n8n_items = envelope.to_n8n_items();

    assert!(n8n_items.is_array());
    let arr = n8n_items.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["json"]["result"], "success");
}

#[test]
fn test_envelope_roundtrip_n8n_items() {
    // n8n output → envelope → n8n items → envelope → should match
    let original_items = json!([
        {"name": "Alice", "score": 95},
        {"name": "Bob", "score": 87}
    ]);

    let envelope1 = DataEnvelope::from_n8n_output("node-1", &original_items);
    let n8n_items = envelope1.to_n8n_items();

    // n8n items should have the original data wrapped
    let envelope2 = DataEnvelope::from_n8n_output("node-2", &n8n_items);
    let final_items = envelope2.to_n8n_items();

    // Already-wrapped items pass through unchanged
    assert_eq!(n8n_items, final_items);
}

#[test]
fn test_envelope_from_crew_callback() {
    let response = json!({
        "step_id": "crew-step-1",
        "agent_id": "researcher",
        "result": {"summary": "Found 42 papers on quantum computing"},
        "confidence": 0.92
    });

    let envelope = DataEnvelope::from_crew_callback(&response);

    assert_eq!(envelope.step_id, "crew-step-1");
    assert_eq!(envelope.output_key, "crew-step-1.result");
    assert_eq!(envelope.content_type, "application/json");
    assert_eq!(
        envelope.content["summary"],
        "Found 42 papers on quantum computing"
    );
    assert_eq!(
        envelope.metadata.agent_id.as_deref(),
        Some("researcher")
    );
    assert_eq!(envelope.metadata.confidence, Some(0.92));
}

#[test]
fn test_envelope_from_crew_callback_minimal() {
    let response = json!({
        "result": "plain text output"
    });

    let envelope = DataEnvelope::from_crew_callback(&response);

    assert_eq!(envelope.step_id, "unknown");
    assert_eq!(envelope.content, json!("plain text output"));
    assert!(envelope.metadata.agent_id.is_none());
    assert!(envelope.metadata.confidence.is_none());
}

#[test]
fn test_envelope_passthrough() {
    let original = DataEnvelope::from_n8n_output("node-1", &json!({"data": "test"}));
    let passthrough = DataEnvelope::passthrough("node-2", &original);

    assert_eq!(passthrough.step_id, "node-2");
    assert_eq!(passthrough.output_key, "node-2.passthrough");
    assert_eq!(passthrough.content, original.content);
}

// ═══════════════════════════════════════════════════════════════════════════
// Contract type serialization tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_step_status_serialization() {
    assert_eq!(
        serde_json::to_string(&StepStatus::Pending).unwrap(),
        "\"pending\""
    );
    assert_eq!(
        serde_json::to_string(&StepStatus::Running).unwrap(),
        "\"running\""
    );
    assert_eq!(
        serde_json::to_string(&StepStatus::Completed).unwrap(),
        "\"completed\""
    );
    assert_eq!(
        serde_json::to_string(&StepStatus::Failed).unwrap(),
        "\"failed\""
    );
    assert_eq!(
        serde_json::to_string(&StepStatus::Skipped).unwrap(),
        "\"skipped\""
    );
}

#[test]
fn test_step_status_deserialization() {
    let pending: StepStatus = serde_json::from_str("\"pending\"").unwrap();
    assert_eq!(pending, StepStatus::Pending);

    let completed: StepStatus = serde_json::from_str("\"completed\"").unwrap();
    assert_eq!(completed, StepStatus::Completed);
}

#[test]
fn test_unified_step_json_roundtrip() {
    let step = UnifiedStep {
        step_id: "step-001".to_string(),
        execution_id: "exec-001".to_string(),
        step_type: "crew.agent".to_string(),
        runtime: "crewai".to_string(),
        name: "Research Agent".to_string(),
        status: StepStatus::Completed,
        input: json!({"task": "research quantum computing"}),
        output: json!({"summary": "42 papers found"}),
        error: None,
        started_at: Utc::now(),
        finished_at: Some(Utc::now()),
        sequence: 1,
    };

    let json_str = serde_json::to_string(&step).unwrap();
    let deserialized: UnifiedStep = serde_json::from_str(&json_str).unwrap();

    assert_eq!(deserialized.step_id, "step-001");
    assert_eq!(deserialized.step_type, "crew.agent");
    assert_eq!(deserialized.runtime, "crewai");
    assert_eq!(deserialized.status, StepStatus::Completed);
    assert!(deserialized.error.is_none());
}

#[test]
fn test_unified_execution_json_roundtrip() {
    let exec = UnifiedExecution {
        execution_id: "exec-001".to_string(),
        runtime: "n8n".to_string(),
        workflow_name: "research_pipeline".to_string(),
        status: StepStatus::Running,
        trigger: "webhook".to_string(),
        input: json!({"topic": "quantum computing"}),
        output: json!({}),
        started_at: Utc::now(),
        finished_at: None,
        step_count: 0,
    };

    let json_str = serde_json::to_string(&exec).unwrap();
    let deserialized: UnifiedExecution = serde_json::from_str(&json_str).unwrap();

    assert_eq!(deserialized.execution_id, "exec-001");
    assert_eq!(deserialized.runtime, "n8n");
    assert_eq!(deserialized.workflow_name, "research_pipeline");
    assert_eq!(deserialized.status, StepStatus::Running);
    assert!(deserialized.finished_at.is_none());
}

#[test]
fn test_data_envelope_json_roundtrip() {
    let envelope = DataEnvelope {
        step_id: "step-001".to_string(),
        output_key: "step-001.output".to_string(),
        content_type: "application/json".to_string(),
        content: json!({"data": [1, 2, 3]}),
        metadata: EnvelopeMetadata {
            agent_id: Some("researcher".to_string()),
            confidence: Some(0.95),
            epoch: Some(42),
            version: Some("1.0".to_string()),
        },
    };

    let json_str = serde_json::to_string(&envelope).unwrap();
    let deserialized: DataEnvelope = serde_json::from_str(&json_str).unwrap();

    assert_eq!(deserialized.step_id, "step-001");
    assert_eq!(deserialized.metadata.agent_id.as_deref(), Some("researcher"));
    assert_eq!(deserialized.metadata.confidence, Some(0.95));
    assert_eq!(deserialized.metadata.epoch, Some(42));
}

#[test]
fn test_envelope_metadata_defaults() {
    let meta = EnvelopeMetadata::default();
    assert!(meta.agent_id.is_none());
    assert!(meta.confidence.is_none());
    assert!(meta.epoch.is_none());
    assert!(meta.version.is_none());

    // Serialization should skip None fields
    let json_str = serde_json::to_string(&meta).unwrap();
    assert_eq!(json_str, "{}");
}

// ═══════════════════════════════════════════════════════════════════════════
// Workflow JSON parsing tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_parse_mixed_workflow() {
    use ada_n8n::executor::WorkflowDefinition;

    let workflow_json = json!({
        "nodes": [
            {
                "id": "webhook-trigger",
                "type": "n8n.webhook",
                "parameters": {"path": "/api/research", "method": "POST"}
            },
            {
                "id": "research-crew",
                "type": "crew.agent",
                "name": "Research Crew",
                "parameters": {
                    "crew_yaml": "research_crew.yaml",
                    "agents": ["researcher", "analyst"],
                    "process": "sequential"
                }
            },
            {
                "id": "lb-index",
                "type": "lb.index",
                "name": "Index Results",
                "parameters": {
                    "collection": "research_results"
                }
            },
            {
                "id": "notify-slack",
                "type": "n8n.slack",
                "parameters": {"channel": "#results"}
            }
        ],
        "connections": {
            "webhook-trigger": {"main": [[{"node": "research-crew"}]]},
            "research-crew": {"main": [[{"node": "lb-index"}]]},
            "lb-index": {"main": [[{"node": "notify-slack"}]]}
        }
    });

    let workflow: WorkflowDefinition = serde_json::from_value(workflow_json).unwrap();

    assert_eq!(workflow.nodes.len(), 4);

    // Verify step types
    assert_eq!(workflow.nodes[0].node_type, "n8n.webhook");
    assert_eq!(workflow.nodes[1].node_type, "crew.agent");
    assert_eq!(workflow.nodes[2].node_type, "lb.index");
    assert_eq!(workflow.nodes[3].node_type, "n8n.slack");

    // Verify connections
    assert!(workflow.connections.contains_key("webhook-trigger"));
    assert!(workflow.connections.contains_key("research-crew"));
    assert!(workflow.connections.contains_key("lb-index"));

    let crew_targets = &workflow.connections["research-crew"].main[0];
    assert_eq!(crew_targets[0].node, "lb-index");
}

#[test]
fn test_parse_crew_research_pipeline_file() {
    use ada_n8n::executor::WorkflowDefinition;

    let workflow_str = include_str!("../workflows/crew_research_pipeline.json");
    let workflow: WorkflowDefinition = serde_json::from_str(workflow_str).unwrap();

    assert_eq!(workflow.nodes.len(), 4);

    // Check that we have mixed step types
    let types: Vec<&str> = workflow.nodes.iter().map(|n| n.node_type.as_str()).collect();
    assert!(types.contains(&"n8n.webhook"));
    assert!(types.contains(&"crew.agent"));
    assert!(types.contains(&"lb.index"));
    assert!(types.contains(&"n8n.slack"));
}

#[test]
fn test_parse_pure_n8n_workflow() {
    use ada_n8n::executor::WorkflowDefinition;

    // Ensure pure n8n workflows still parse correctly
    let workflow_json = json!({
        "nodes": [
            {
                "id": "trigger",
                "type": "n8n.webhook",
                "parameters": {"path": "/test"}
            },
            {
                "id": "http-call",
                "type": "n8n.httpRequest",
                "parameters": {"url": "https://api.example.com/data"}
            }
        ],
        "connections": {
            "trigger": {"main": [[{"node": "http-call"}]]}
        }
    });

    let workflow: WorkflowDefinition = serde_json::from_value(workflow_json).unwrap();
    assert_eq!(workflow.nodes.len(), 2);
    assert_eq!(workflow.nodes[0].node_type, "n8n.webhook");
    assert_eq!(workflow.nodes[1].node_type, "n8n.httpRequest");
}

// ═══════════════════════════════════════════════════════════════════════════
// Runtime detection from step type
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_runtime_detection_from_step_type() {
    fn detect_runtime(step_type: &str) -> &str {
        match step_type.split('.').next() {
            Some("crew") => "crewai",
            Some("lb") => "ladybug",
            _ => "n8n",
        }
    }

    assert_eq!(detect_runtime("n8n.webhook"), "n8n");
    assert_eq!(detect_runtime("n8n.httpRequest"), "n8n");
    assert_eq!(detect_runtime("n8n.slack"), "n8n");
    assert_eq!(detect_runtime("crew.agent"), "crewai");
    assert_eq!(detect_runtime("crew.task"), "crewai");
    assert_eq!(detect_runtime("lb.index"), "ladybug");
    assert_eq!(detect_runtime("lb.enrich"), "ladybug");
    assert_eq!(detect_runtime("unknown"), "n8n"); // default
}

// ═══════════════════════════════════════════════════════════════════════════
// Crew step routing tests (mock)
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_lb_step_passthrough_when_no_endpoint() {
    use ada_n8n::executor::{ExecutorConfig, WorkflowExecutor};
    use ada_n8n::config::{AppState, Config};

    let config = Config {
        host: "127.0.0.1".to_string(),
        port: 0,
        protocol: "http".to_string(),
        webhook_url: "http://localhost/".to_string(),
        mcp_url: "http://localhost/mcp".to_string(),
        point_url: "http://localhost/point".to_string(),
        xai_url: "http://localhost/xai".to_string(),
        redis_url: String::new(),
        redis_token: String::new(),
        xai_key: String::new(),
        basic_auth_user: None,
        basic_auth_password: None,
        timezone: "UTC".to_string(),
        crewai_endpoint: None,
        ladybug_endpoint: None,
        database_url: None,
    };

    let state = AppState::new(config);
    let executor = WorkflowExecutor::new(
        state,
        ExecutorConfig {
            crewai_endpoint: None,
            ladybug_endpoint: None,
        },
    );

    let step = UnifiedStep {
        step_id: "lb-step-1".to_string(),
        execution_id: "exec-1".to_string(),
        step_type: "lb.index".to_string(),
        runtime: "ladybug".to_string(),
        name: "Index".to_string(),
        status: StepStatus::Running,
        input: json!({}),
        output: json!({}),
        error: None,
        started_at: Utc::now(),
        finished_at: None,
        sequence: 1,
    };

    let input_envelope = DataEnvelope::from_n8n_output("prev-node", &json!([{"data": "test"}]));

    // lb.* steps should pass through when no ladybug endpoint is configured
    let result = executor.execute_step(&step, &input_envelope).await.unwrap();
    assert_eq!(result.step_id, "lb-step-1");
    assert_eq!(result.output_key, "lb-step-1.passthrough");
    assert_eq!(result.content, input_envelope.content);
}

#[tokio::test]
async fn test_crew_step_passthrough_when_no_endpoint() {
    use ada_n8n::executor::{ExecutorConfig, WorkflowExecutor};
    use ada_n8n::config::{AppState, Config};

    let config = Config {
        host: "127.0.0.1".to_string(),
        port: 0,
        protocol: "http".to_string(),
        webhook_url: "http://localhost/".to_string(),
        mcp_url: "http://localhost/mcp".to_string(),
        point_url: "http://localhost/point".to_string(),
        xai_url: "http://localhost/xai".to_string(),
        redis_url: String::new(),
        redis_token: String::new(),
        xai_key: String::new(),
        basic_auth_user: None,
        basic_auth_password: None,
        timezone: "UTC".to_string(),
        crewai_endpoint: None,
        ladybug_endpoint: None,
        database_url: None,
    };

    let state = AppState::new(config);
    let executor = WorkflowExecutor::new(
        state,
        ExecutorConfig {
            crewai_endpoint: None,
            ladybug_endpoint: None,
        },
    );

    let step = UnifiedStep {
        step_id: "crew-step-1".to_string(),
        execution_id: "exec-1".to_string(),
        step_type: "crew.agent".to_string(),
        runtime: "crewai".to_string(),
        name: "Research".to_string(),
        status: StepStatus::Running,
        input: json!({"task": "research"}),
        output: json!({}),
        error: None,
        started_at: Utc::now(),
        finished_at: None,
        sequence: 1,
    };

    let input_envelope = DataEnvelope::from_n8n_output("trigger", &json!([{"topic": "AI"}]));

    // crew.* steps should pass through (with warning) when no crewAI endpoint
    let result = executor.execute_step(&step, &input_envelope).await.unwrap();
    assert_eq!(result.step_id, "crew-step-1");
    assert_eq!(result.content, input_envelope.content);
}

#[tokio::test]
async fn test_n8n_webhook_step_passthrough() {
    use ada_n8n::executor::{ExecutorConfig, WorkflowExecutor};
    use ada_n8n::config::{AppState, Config};

    let config = Config {
        host: "127.0.0.1".to_string(),
        port: 0,
        protocol: "http".to_string(),
        webhook_url: "http://localhost/".to_string(),
        mcp_url: "http://localhost/mcp".to_string(),
        point_url: "http://localhost/point".to_string(),
        xai_url: "http://localhost/xai".to_string(),
        redis_url: String::new(),
        redis_token: String::new(),
        xai_key: String::new(),
        basic_auth_user: None,
        basic_auth_password: None,
        timezone: "UTC".to_string(),
        crewai_endpoint: None,
        ladybug_endpoint: None,
        database_url: None,
    };

    let state = AppState::new(config);
    let executor = WorkflowExecutor::new(
        state,
        ExecutorConfig {
            crewai_endpoint: None,
            ladybug_endpoint: None,
        },
    );

    let step = UnifiedStep {
        step_id: "webhook-1".to_string(),
        execution_id: "exec-1".to_string(),
        step_type: "n8n.webhook".to_string(),
        runtime: "n8n".to_string(),
        name: "Trigger".to_string(),
        status: StepStatus::Running,
        input: json!({"path": "/test"}),
        output: json!({}),
        error: None,
        started_at: Utc::now(),
        finished_at: None,
        sequence: 0,
    };

    let input_envelope = DataEnvelope::from_n8n_output("start", &json!([{"data": "hello"}]));

    // n8n.webhook should pass through (it's a trigger, not an action)
    let result = executor.execute_step(&step, &input_envelope).await.unwrap();
    assert_eq!(result.step_id, "webhook-1");
}

// ═══════════════════════════════════════════════════════════════════════════
// PgStore tests (require DATABASE_URL — skipped if not set)
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(feature = "postgres")]
mod pg_tests {
    use super::*;

    fn get_database_url() -> Option<String> {
        std::env::var("DATABASE_URL").ok()
    }

    #[tokio::test]
    async fn test_pg_store_write_read_execution() {
        use ada_n8n::contract::pg_store::PgStore;

        let Some(database_url) = get_database_url() else {
            eprintln!("Skipping PgStore test — DATABASE_URL not set");
            return;
        };

        let store = PgStore::new(&database_url).await.unwrap();

        let exec = UnifiedExecution {
            execution_id: format!("test-exec-{}", uuid::Uuid::new_v4()),
            runtime: "n8n".to_string(),
            workflow_name: "test_workflow".to_string(),
            status: StepStatus::Running,
            trigger: "test".to_string(),
            input: json!({"test": true}),
            output: json!({}),
            started_at: Utc::now(),
            finished_at: None,
            step_count: 0,
        };

        // Write
        store.write_execution(&exec).await.unwrap();

        // Read back
        let read_back = store.read_execution(&exec.execution_id).await.unwrap();
        assert!(read_back.is_some());
        let read_back = read_back.unwrap();
        assert_eq!(read_back.execution_id, exec.execution_id);
        assert_eq!(read_back.runtime, "n8n");
        assert_eq!(read_back.status, StepStatus::Running);

        // Update via finish
        store
            .finish_execution(
                &exec.execution_id,
                StepStatus::Completed,
                &json!({"result": "done"}),
                3,
            )
            .await
            .unwrap();

        let updated = store.read_execution(&exec.execution_id).await.unwrap().unwrap();
        assert_eq!(updated.status, StepStatus::Completed);
        assert_eq!(updated.step_count, 3);
        assert!(updated.finished_at.is_some());
    }

    #[tokio::test]
    async fn test_pg_store_write_read_steps() {
        use ada_n8n::contract::pg_store::PgStore;

        let Some(database_url) = get_database_url() else {
            eprintln!("Skipping PgStore test — DATABASE_URL not set");
            return;
        };

        let store = PgStore::new(&database_url).await.unwrap();
        let exec_id = format!("test-exec-{}", uuid::Uuid::new_v4());

        // Create parent execution first
        let exec = UnifiedExecution {
            execution_id: exec_id.clone(),
            runtime: "n8n".to_string(),
            workflow_name: "test_workflow".to_string(),
            status: StepStatus::Running,
            trigger: "test".to_string(),
            input: json!({}),
            output: json!({}),
            started_at: Utc::now(),
            finished_at: None,
            step_count: 2,
        };
        store.write_execution(&exec).await.unwrap();

        // Write two steps
        let step1 = UnifiedStep {
            step_id: format!("step-{}", uuid::Uuid::new_v4()),
            execution_id: exec_id.clone(),
            step_type: "n8n.webhook".to_string(),
            runtime: "n8n".to_string(),
            name: "Trigger".to_string(),
            status: StepStatus::Completed,
            input: json!({}),
            output: json!({"triggered": true}),
            error: None,
            started_at: Utc::now(),
            finished_at: Some(Utc::now()),
            sequence: 1,
        };

        let step2 = UnifiedStep {
            step_id: format!("step-{}", uuid::Uuid::new_v4()),
            execution_id: exec_id.clone(),
            step_type: "crew.agent".to_string(),
            runtime: "crewai".to_string(),
            name: "Research Agent".to_string(),
            status: StepStatus::Completed,
            input: json!({"task": "research"}),
            output: json!({"summary": "results"}),
            error: None,
            started_at: Utc::now(),
            finished_at: Some(Utc::now()),
            sequence: 2,
        };

        store.write_step(&step1).await.unwrap();
        store.write_step(&step2).await.unwrap();

        // Read steps
        let steps = store.read_steps(&exec_id).await.unwrap();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].sequence, 1);
        assert_eq!(steps[1].sequence, 2);
        assert_eq!(steps[0].step_type, "n8n.webhook");
        assert_eq!(steps[1].step_type, "crew.agent");
        assert_eq!(steps[1].runtime, "crewai");
    }
}
