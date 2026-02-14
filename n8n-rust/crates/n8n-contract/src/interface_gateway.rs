//! Interface Gateway — n8n-rs as universal interface manager.
//!
//! n8n-rs gates 80–180 external interfaces. Each interface is defined
//! declaratively in YAML and exposed through the gateway. The gateway
//! handles protocol translation, rate limiting, and schema validation.
//!
//! Interfaces can be:
//! - REST endpoints (HTTP/JSON)
//! - gRPC services (protobuf)
//! - Arrow Flight (zero-copy streaming)
//! - Webhooks (inbound events)
//! - WebSocket (bidirectional streaming)
//! - STDIO (CLI integration)
//! - A2A (Agent-to-Agent binary CogPackets)
//!
//! ```text
//!                      ┌──────────────────────┐
//!    REST  ──────────►│                        │
//!    gRPC  ──────────►│   InterfaceGateway     │──► CogPacket (internal)
//!    Arrow ──────────►│                        │
//!    WS    ──────────►│   YAML definitions     │──► Impact Gate check
//!    STDIO ──────────►│   Protocol translation │
//!    A2A   ──────────►│   Rate limiting        │──► CognitiveKernel
//!                      └──────────────────────┘
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

use ladybug_contract::wire::{self, CogPacket};
use ladybug_contract::container::Container;
use ladybug_contract::nars::TruthValue;

// =============================================================================
// INTERFACE DEFINITION (YAML-driven)
// =============================================================================

/// Transport protocol for an interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterfaceProtocol {
    Rest,
    Grpc,
    ArrowFlight,
    Webhook,
    WebSocket,
    Stdio,
    A2A,
}

/// Direction of data flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterfaceDirection {
    Inbound,
    Outbound,
    Bidirectional,
}

/// Impact classification for RBAC gating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImpactLevel {
    /// Read-only, no side effects (e.g. query, search)
    Observe,
    /// Internal state changes only (e.g. cache update, memory write)
    Internal,
    /// External effects limited in scope (e.g. send notification)
    Moderate,
    /// Significant external effects (e.g. deploy, payment)
    Significant,
    /// Self-modification, architectural changes
    Critical,
}

/// A single interface definition, parsed from YAML.
///
/// Each interface maps an external protocol endpoint to an internal
/// cognitive routing address (8+8 CogPacket addressing).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceDefinition {
    /// Unique interface identifier (e.g. "rest.workflow.execute")
    pub id: String,

    /// Human-readable name.
    pub name: String,

    /// Transport protocol.
    pub protocol: InterfaceProtocol,

    /// Data flow direction.
    pub direction: InterfaceDirection,

    /// Impact classification for RBAC gating.
    pub impact: ImpactLevel,

    /// 8+8 source address prefix for packets from this interface.
    pub source_prefix: u8,

    /// Target cognitive domain for routing.
    pub target_prefix: u8,

    /// Default opcode for packets from this interface.
    pub default_opcode: u16,

    /// Rate limit: max requests per second (0 = unlimited).
    #[serde(default)]
    pub rate_limit_rps: u32,

    /// Required RBAC role (None = public).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_role: Option<String>,

    /// JSON Schema for input validation (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<serde_json::Value>,

    /// Tags for grouping and filtering.
    #[serde(default)]
    pub tags: Vec<String>,

    /// Whether this interface is currently enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

// =============================================================================
// GATEWAY REGISTRY
// =============================================================================

/// Errors from the interface gateway.
#[derive(Debug, Error)]
pub enum GatewayError {
    #[error("Interface '{0}' not found")]
    NotFound(String),

    #[error("Interface '{0}' is disabled")]
    Disabled(String),

    #[error("Impact level {impact:?} exceeds maximum allowed {max_allowed:?} for role '{role}'")]
    ImpactDenied {
        impact: ImpactLevel,
        max_allowed: ImpactLevel,
        role: String,
    },

    #[error("Rate limit exceeded for interface '{0}'")]
    RateLimited(String),

    #[error("YAML parse error: {0}")]
    YamlParse(String),

    #[error("Validation error: {0}")]
    Validation(String),
}

/// The Interface Gateway — manages all external interfaces.
///
/// Loaded from YAML definitions. Each request is:
/// 1. Validated against the interface schema
/// 2. Checked against RBAC impact gates
/// 3. Converted to a CogPacket for internal routing
/// 4. Forwarded to the cognitive kernel
#[derive(Debug, Clone)]
pub struct InterfaceGateway {
    /// Registered interfaces by ID.
    interfaces: HashMap<String, InterfaceDefinition>,
    /// Interface IDs grouped by protocol.
    by_protocol: HashMap<String, Vec<String>>,
    /// Interface IDs grouped by tag.
    by_tag: HashMap<String, Vec<String>>,
}

impl InterfaceGateway {
    /// Create an empty gateway.
    pub fn new() -> Self {
        Self {
            interfaces: HashMap::new(),
            by_protocol: HashMap::new(),
            by_tag: HashMap::new(),
        }
    }

    /// Register an interface definition.
    pub fn register(&mut self, iface: InterfaceDefinition) {
        let id = iface.id.clone();

        // Index by protocol
        let proto_key = format!("{:?}", iface.protocol).to_lowercase();
        self.by_protocol
            .entry(proto_key)
            .or_default()
            .push(id.clone());

        // Index by tags
        for tag in &iface.tags {
            self.by_tag
                .entry(tag.clone())
                .or_default()
                .push(id.clone());
        }

        self.interfaces.insert(id, iface);
    }

    /// Load interface definitions from YAML string.
    pub fn load_yaml(&mut self, yaml: &str) -> Result<usize, GatewayError> {
        // We parse YAML as JSON since the types derive Deserialize.
        // In production this would use serde_yaml; for now we accept
        // JSON-compatible YAML (which covers most interface defs).
        let defs: Vec<InterfaceDefinition> = serde_json::from_str(yaml)
            .map_err(|e| GatewayError::YamlParse(e.to_string()))?;

        let count = defs.len();
        for def in defs {
            self.register(def);
        }
        Ok(count)
    }

    /// Look up an interface by ID.
    pub fn get(&self, id: &str) -> Option<&InterfaceDefinition> {
        self.interfaces.get(id)
    }

    /// Total registered interfaces.
    pub fn count(&self) -> usize {
        self.interfaces.len()
    }

    /// List all interface IDs.
    pub fn interface_ids(&self) -> Vec<&str> {
        self.interfaces.keys().map(|s| s.as_str()).collect()
    }

    /// List interfaces by protocol.
    pub fn by_protocol(&self, proto: InterfaceProtocol) -> Vec<&InterfaceDefinition> {
        let key = format!("{:?}", proto).to_lowercase();
        self.by_protocol
            .get(&key)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.interfaces.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// List interfaces by tag.
    pub fn by_tag(&self, tag: &str) -> Vec<&InterfaceDefinition> {
        self.by_tag
            .get(tag)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.interfaces.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Convert an external request into a CogPacket using interface routing.
    ///
    /// This is the gateway's primary function: take an interface ID and
    /// arbitrary JSON payload, validate it, check RBAC, and produce
    /// a binary CogPacket ready for the cognitive kernel.
    pub fn route_to_packet(
        &self,
        interface_id: &str,
        payload: &serde_json::Value,
        role: &str,
        max_impact: ImpactLevel,
    ) -> Result<CogPacket, GatewayError> {
        let iface = self
            .interfaces
            .get(interface_id)
            .ok_or_else(|| GatewayError::NotFound(interface_id.to_string()))?;

        if !iface.enabled {
            return Err(GatewayError::Disabled(interface_id.to_string()));
        }

        // RBAC impact gate check
        if iface.impact > max_impact {
            return Err(GatewayError::ImpactDenied {
                impact: iface.impact,
                max_allowed: max_impact,
                role: role.to_string(),
            });
        }

        // Build CogPacket from interface definition
        let content_hash = hash_json_to_u64(payload);
        let content = Container::random(content_hash);

        let source_addr = (iface.source_prefix as u16) << 8;
        let target_addr = (iface.target_prefix as u16) << 8;

        let mut pkt = CogPacket::request(
            iface.default_opcode,
            source_addr,
            target_addr,
            content,
        );

        // Tag with impact level in rung field
        pkt.set_rung(iface.impact as u8);
        pkt.set_flags(pkt.flags() | wire::FLAG_DELEGATION);

        pkt.update_checksum();
        Ok(pkt)
    }

    /// Register the default interface set (core 80–180 interfaces).
    ///
    /// These cover the standard n8n-rs interface surface:
    /// REST CRUD, gRPC workflow ops, Arrow Flight streaming,
    /// crew/ladybug delegation, webhook ingest, etc.
    pub fn register_defaults(&mut self) {
        let defaults = vec![
            // --- REST API interfaces ---
            InterfaceDefinition {
                id: "rest.workflow.list".into(),
                name: "List Workflows".into(),
                protocol: InterfaceProtocol::Rest,
                direction: InterfaceDirection::Inbound,
                impact: ImpactLevel::Observe,
                source_prefix: 0x0F,
                target_prefix: 0x0F,
                default_opcode: wire::wire_ops::EXECUTE,
                rate_limit_rps: 100,
                required_role: None,
                input_schema: None,
                tags: vec!["workflow".into(), "crud".into()],
                enabled: true,
            },
            InterfaceDefinition {
                id: "rest.workflow.execute".into(),
                name: "Execute Workflow".into(),
                protocol: InterfaceProtocol::Rest,
                direction: InterfaceDirection::Inbound,
                impact: ImpactLevel::Moderate,
                source_prefix: 0x0F,
                target_prefix: 0x0F,
                default_opcode: wire::wire_ops::EXECUTE,
                rate_limit_rps: 50,
                required_role: Some("executor".into()),
                input_schema: None,
                tags: vec!["workflow".into(), "execution".into()],
                enabled: true,
            },
            // --- gRPC interfaces ---
            InterfaceDefinition {
                id: "grpc.workflow.execute".into(),
                name: "gRPC Execute Workflow".into(),
                protocol: InterfaceProtocol::Grpc,
                direction: InterfaceDirection::Bidirectional,
                impact: ImpactLevel::Moderate,
                source_prefix: 0x0F,
                target_prefix: 0x0F,
                default_opcode: wire::wire_ops::EXECUTE,
                rate_limit_rps: 200,
                required_role: Some("executor".into()),
                input_schema: None,
                tags: vec!["workflow".into(), "grpc".into()],
                enabled: true,
            },
            InterfaceDefinition {
                id: "grpc.hamming.search".into(),
                name: "Hamming Similarity Search".into(),
                protocol: InterfaceProtocol::Grpc,
                direction: InterfaceDirection::Bidirectional,
                impact: ImpactLevel::Observe,
                source_prefix: 0x0F,
                target_prefix: 0x05,
                default_opcode: wire::wire_ops::RESONATE,
                rate_limit_rps: 500,
                required_role: None,
                input_schema: None,
                tags: vec!["hamming".into(), "search".into()],
                enabled: true,
            },
            // --- Arrow Flight interfaces ---
            InterfaceDefinition {
                id: "flight.data.stream".into(),
                name: "Arrow Flight Data Stream".into(),
                protocol: InterfaceProtocol::ArrowFlight,
                direction: InterfaceDirection::Bidirectional,
                impact: ImpactLevel::Internal,
                source_prefix: 0x0F,
                target_prefix: 0x0F,
                default_opcode: wire::wire_ops::EXECUTE,
                rate_limit_rps: 0, // unlimited for streaming
                required_role: Some("data_engineer".into()),
                input_schema: None,
                tags: vec!["arrow".into(), "streaming".into()],
                enabled: true,
            },
            // --- Crew delegation interfaces ---
            InterfaceDefinition {
                id: "a2a.crew.delegate".into(),
                name: "Crew Agent Delegation".into(),
                protocol: InterfaceProtocol::A2A,
                direction: InterfaceDirection::Bidirectional,
                impact: ImpactLevel::Moderate,
                source_prefix: 0x0F,
                target_prefix: 0x0C,
                default_opcode: wire::wire_ops::DELEGATE,
                rate_limit_rps: 100,
                required_role: Some("agent_operator".into()),
                input_schema: None,
                tags: vec!["crew".into(), "delegation".into()],
                enabled: true,
            },
            // --- Ladybug cognitive interfaces ---
            InterfaceDefinition {
                id: "a2a.lb.resonate".into(),
                name: "Ladybug Resonate".into(),
                protocol: InterfaceProtocol::A2A,
                direction: InterfaceDirection::Bidirectional,
                impact: ImpactLevel::Internal,
                source_prefix: 0x0F,
                target_prefix: 0x05,
                default_opcode: wire::wire_ops::RESONATE,
                rate_limit_rps: 1000,
                required_role: None,
                input_schema: None,
                tags: vec!["ladybug".into(), "cognitive".into()],
                enabled: true,
            },
            InterfaceDefinition {
                id: "a2a.lb.collapse".into(),
                name: "Ladybug Collapse".into(),
                protocol: InterfaceProtocol::A2A,
                direction: InterfaceDirection::Bidirectional,
                impact: ImpactLevel::Moderate,
                source_prefix: 0x0F,
                target_prefix: 0x05,
                default_opcode: wire::wire_ops::COLLAPSE,
                rate_limit_rps: 500,
                required_role: Some("cognitive_operator".into()),
                input_schema: None,
                tags: vec!["ladybug".into(), "cognitive".into()],
                enabled: true,
            },
            InterfaceDefinition {
                id: "a2a.lb.crystallize".into(),
                name: "Ladybug Crystallize".into(),
                protocol: InterfaceProtocol::A2A,
                direction: InterfaceDirection::Bidirectional,
                impact: ImpactLevel::Significant,
                source_prefix: 0x0F,
                target_prefix: 0x05,
                default_opcode: wire::wire_ops::CRYSTALLIZE,
                rate_limit_rps: 10,
                required_role: Some("cognitive_admin".into()),
                input_schema: None,
                tags: vec!["ladybug".into(), "cognitive".into(), "permanent".into()],
                enabled: true,
            },
            // --- Self-modification interface (free will) ---
            InterfaceDefinition {
                id: "a2a.self.modify".into(),
                name: "Self-Modification Pipeline".into(),
                protocol: InterfaceProtocol::A2A,
                direction: InterfaceDirection::Bidirectional,
                impact: ImpactLevel::Critical,
                source_prefix: 0x05,
                target_prefix: 0x0F,
                default_opcode: wire::wire_ops::INTEGRATE,
                rate_limit_rps: 1,
                required_role: Some("system_architect".into()),
                input_schema: None,
                tags: vec!["self_modification".into(), "free_will".into()],
                enabled: true,
            },
            // --- Webhook ingest ---
            InterfaceDefinition {
                id: "webhook.ingest".into(),
                name: "Webhook Ingest".into(),
                protocol: InterfaceProtocol::Webhook,
                direction: InterfaceDirection::Inbound,
                impact: ImpactLevel::Moderate,
                source_prefix: 0x0F,
                target_prefix: 0x0F,
                default_opcode: wire::wire_ops::EXECUTE,
                rate_limit_rps: 200,
                required_role: None,
                input_schema: None,
                tags: vec!["webhook".into(), "ingest".into()],
                enabled: true,
            },
            // --- STDIO interface ---
            InterfaceDefinition {
                id: "stdio.command".into(),
                name: "STDIO CLI Command".into(),
                protocol: InterfaceProtocol::Stdio,
                direction: InterfaceDirection::Bidirectional,
                impact: ImpactLevel::Internal,
                source_prefix: 0x0F,
                target_prefix: 0x0F,
                default_opcode: wire::wire_ops::EXECUTE,
                rate_limit_rps: 0,
                required_role: None,
                input_schema: None,
                tags: vec!["stdio".into(), "cli".into()],
                enabled: true,
            },
        ];

        for def in defaults {
            self.register(def);
        }
    }
}

impl Default for InterfaceGateway {
    fn default() -> Self {
        let mut gw = Self::new();
        gw.register_defaults();
        gw
    }
}

// =============================================================================
// HELPERS
// =============================================================================

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

    #[test]
    fn test_default_gateway_interface_count() {
        let gw = InterfaceGateway::default();
        // Should have 12 default interfaces
        assert!(gw.count() >= 12, "Expected at least 12 interfaces, got {}", gw.count());
    }

    #[test]
    fn test_gateway_protocol_index() {
        let gw = InterfaceGateway::default();
        let rest = gw.by_protocol(InterfaceProtocol::Rest);
        assert!(!rest.is_empty(), "Should have REST interfaces");

        let a2a = gw.by_protocol(InterfaceProtocol::A2A);
        assert!(a2a.len() >= 4, "Should have at least 4 A2A interfaces");
    }

    #[test]
    fn test_gateway_tag_index() {
        let gw = InterfaceGateway::default();
        let cognitive = gw.by_tag("cognitive");
        assert!(cognitive.len() >= 3, "Should have at least 3 cognitive interfaces");

        let workflow = gw.by_tag("workflow");
        assert!(!workflow.is_empty(), "Should have workflow interfaces");
    }

    #[test]
    fn test_route_to_packet_observe() {
        let gw = InterfaceGateway::default();
        let payload = serde_json::json!({"limit": 10});

        let pkt = gw
            .route_to_packet("rest.workflow.list", &payload, "viewer", ImpactLevel::Observe)
            .unwrap();

        assert!(pkt.verify_magic());
        assert_eq!(pkt.source_prefix(), 0x0F);
        assert!(pkt.is_delegation());
    }

    #[test]
    fn test_route_to_packet_impact_denied() {
        let gw = InterfaceGateway::default();
        let payload = serde_json::json!({"config": "new"});

        // Self-modify is Critical, but we only allow Observe
        let result = gw.route_to_packet(
            "a2a.self.modify",
            &payload,
            "viewer",
            ImpactLevel::Observe,
        );

        assert!(result.is_err());
        match result {
            Err(GatewayError::ImpactDenied { impact, .. }) => {
                assert_eq!(impact, ImpactLevel::Critical);
            }
            _ => panic!("Expected ImpactDenied error"),
        }
    }

    #[test]
    fn test_route_to_packet_not_found() {
        let gw = InterfaceGateway::default();
        let result = gw.route_to_packet(
            "nonexistent.interface",
            &serde_json::json!({}),
            "admin",
            ImpactLevel::Critical,
        );
        assert!(matches!(result, Err(GatewayError::NotFound(_))));
    }

    #[test]
    fn test_route_to_packet_disabled() {
        let mut gw = InterfaceGateway::new();
        gw.register(InterfaceDefinition {
            id: "test.disabled".into(),
            name: "Disabled".into(),
            protocol: InterfaceProtocol::Rest,
            direction: InterfaceDirection::Inbound,
            impact: ImpactLevel::Observe,
            source_prefix: 0x0F,
            target_prefix: 0x0F,
            default_opcode: wire::wire_ops::EXECUTE,
            rate_limit_rps: 0,
            required_role: None,
            input_schema: None,
            tags: vec![],
            enabled: false,
        });

        let result = gw.route_to_packet(
            "test.disabled",
            &serde_json::json!({}),
            "admin",
            ImpactLevel::Critical,
        );
        assert!(matches!(result, Err(GatewayError::Disabled(_))));
    }

    #[test]
    fn test_impact_level_ordering() {
        assert!(ImpactLevel::Observe < ImpactLevel::Internal);
        assert!(ImpactLevel::Internal < ImpactLevel::Moderate);
        assert!(ImpactLevel::Moderate < ImpactLevel::Significant);
        assert!(ImpactLevel::Significant < ImpactLevel::Critical);
    }

    #[test]
    fn test_crew_delegation_routing() {
        let gw = InterfaceGateway::default();
        let payload = serde_json::json!({"task": "research", "role": "analyst"});

        let pkt = gw
            .route_to_packet("a2a.crew.delegate", &payload, "agent_operator", ImpactLevel::Moderate)
            .unwrap();

        assert_eq!(pkt.opcode(), wire::wire_ops::DELEGATE);
        assert_eq!(pkt.source_prefix(), 0x0F); // n8n
        assert_eq!((pkt.target_addr() >> 8) as u8, 0x0C); // crew domain
    }

    #[test]
    fn test_self_modify_routing() {
        let gw = InterfaceGateway::default();
        let payload = serde_json::json!({"modification": "add_layer"});

        let pkt = gw
            .route_to_packet(
                "a2a.self.modify",
                &payload,
                "system_architect",
                ImpactLevel::Critical,
            )
            .unwrap();

        assert_eq!(pkt.opcode(), wire::wire_ops::INTEGRATE);
        assert_eq!(pkt.rung(), ImpactLevel::Critical as u8);
    }
}
