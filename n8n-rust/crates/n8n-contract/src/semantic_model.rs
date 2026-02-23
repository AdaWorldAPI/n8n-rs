//! Semantic Model Registry for A2A RAG orchestration.
//!
//! The semantic model bridges **structured relational data** (from n8n providers,
//! databases, APIs) with **BindSpace fingerprints** (16Kbit HDC vectors).
//!
//! LLMs can't reason about relational schema — they hallucinate joins, miss
//! foreign keys, invent columns. The semantic model provides the **context layer**
//! that turns blind-spot data into awareness.
//!
//! ## Architecture
//!
//! ```text
//! n8n providers → SemanticModel → fingerprint → BindSpace → resonance search
//!                     ↑                                        ↓
//!              neo4j-rs graph ←── explicit surface ←── RAG retrieval
//! ```
//!
//! ## Agent routing
//!
//! The registry maps data domains to agent capabilities. When an n8n workflow
//! produces structured output, the router checks which agents have the skills
//! to interpret that schema and routes accordingly.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::debug;

// ============================================================================
// SemanticField — describes one field in a semantic model
// ============================================================================

/// A single field within a semantic model entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticField {
    /// Field name (e.g., "customer_id", "order_total").
    pub name: String,

    /// Semantic type: "identifier", "measure", "dimension", "timestamp", "text", "embedding".
    pub semantic_type: String,

    /// Human-readable description for LLM context injection.
    pub description: String,

    /// Whether this field is a primary/foreign key.
    #[serde(default)]
    pub is_key: bool,

    /// Linked entity name for foreign key relationships.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links_to: Option<String>,
}

// ============================================================================
// SemanticEntity — describes one entity/table in a semantic model
// ============================================================================

/// A semantic entity (maps to a database table, API resource, or domain concept).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticEntity {
    /// Entity name (e.g., "Customer", "Order", "Product").
    pub name: String,

    /// Human-readable description.
    pub description: String,

    /// Provider source (e.g., "postgres.orders", "shopify.products", "hubspot.contacts").
    pub source: String,

    /// Fields in this entity.
    pub fields: Vec<SemanticField>,

    /// Domain tags for agent routing (e.g., ["ecommerce", "sales", "analytics"]).
    #[serde(default)]
    pub domain_tags: Vec<String>,
}

impl SemanticEntity {
    /// Get all key fields.
    pub fn key_fields(&self) -> Vec<&SemanticField> {
        self.fields.iter().filter(|f| f.is_key).collect()
    }

    /// Get foreign key relationships: (field_name, target_entity).
    pub fn relationships(&self) -> Vec<(&str, &str)> {
        self.fields
            .iter()
            .filter_map(|f| f.links_to.as_ref().map(|target| (f.name.as_str(), target.as_str())))
            .collect()
    }

    /// Generate a context string suitable for LLM injection.
    pub fn to_context_string(&self) -> String {
        let mut ctx = format!("Entity: {} ({})\n", self.name, self.description);
        ctx.push_str(&format!("Source: {}\n", self.source));
        ctx.push_str("Fields:\n");
        for f in &self.fields {
            let key_marker = if f.is_key { " [KEY]" } else { "" };
            let link = f
                .links_to
                .as_ref()
                .map(|t| format!(" → {}", t))
                .unwrap_or_default();
            ctx.push_str(&format!(
                "  - {} ({}): {}{}{}\n",
                f.name, f.semantic_type, f.description, key_marker, link
            ));
        }
        ctx
    }
}

// ============================================================================
// SemanticModel — a collection of related entities
// ============================================================================

/// A semantic model: a named collection of entities with relationships.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticModel {
    /// Model name (e.g., "ecommerce", "crm", "analytics").
    pub name: String,

    /// Model description.
    pub description: String,

    /// Version string.
    pub version: String,

    /// Entities in this model.
    pub entities: Vec<SemanticEntity>,
}

impl SemanticModel {
    /// Get an entity by name.
    pub fn get_entity(&self, name: &str) -> Option<&SemanticEntity> {
        self.entities.iter().find(|e| e.name == name)
    }

    /// Get all cross-entity relationships as (from_entity, field, to_entity) triples.
    pub fn all_relationships(&self) -> Vec<(&str, &str, &str)> {
        self.entities
            .iter()
            .flat_map(|e| {
                e.relationships()
                    .into_iter()
                    .map(move |(field, target)| (e.name.as_str(), field, target))
            })
            .collect()
    }

    /// Generate full context string for all entities.
    pub fn to_context_string(&self) -> String {
        let mut ctx = format!("Semantic Model: {} v{}\n{}\n\n", self.name, self.version, self.description);
        for entity in &self.entities {
            ctx.push_str(&entity.to_context_string());
            ctx.push('\n');
        }

        let rels = self.all_relationships();
        if !rels.is_empty() {
            ctx.push_str("Relationships:\n");
            for (from, field, to) in &rels {
                ctx.push_str(&format!("  {} .{} → {}\n", from, field, to));
            }
        }
        ctx
    }

    /// Collect all unique domain tags across entities.
    pub fn all_domain_tags(&self) -> Vec<String> {
        let mut tags: Vec<String> = self
            .entities
            .iter()
            .flat_map(|e| e.domain_tags.iter().cloned())
            .collect();
        tags.sort();
        tags.dedup();
        tags
    }
}

// ============================================================================
// AgentCapability — what an agent can do with a semantic model
// ============================================================================

/// Describes an agent's capability relative to semantic domains.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCapability {
    /// Agent identifier (slot address or name).
    pub agent_id: String,

    /// Domains this agent can handle (must match SemanticEntity domain_tags).
    pub domains: Vec<String>,

    /// Specific entity names this agent specializes in.
    #[serde(default)]
    pub entity_specializations: Vec<String>,

    /// Proficiency score (0.0–1.0) for routing priority.
    #[serde(default = "default_proficiency")]
    pub proficiency: f64,

    /// Step types this agent supports (e.g., ["crew.agent", "lb.resonate"]).
    #[serde(default)]
    pub step_types: Vec<String>,
}

fn default_proficiency() -> f64 {
    0.5
}

// ============================================================================
// SemanticModelRegistry — central registry for models + agent routing
// ============================================================================

/// Registry of semantic models and agent capabilities.
///
/// Used by the n8n-contract execution pipeline to:
/// 1. Inject schema context into LLM prompts (anti-hallucination)
/// 2. Route workflow data to the right agent based on domain
/// 3. Build knowledge graph edges for neo4j-rs
pub struct SemanticModelRegistry {
    /// Registered semantic models, keyed by model name.
    models: HashMap<String, SemanticModel>,

    /// Agent capabilities, keyed by agent_id.
    agents: HashMap<String, AgentCapability>,

    /// Reverse index: domain_tag → [agent_ids].
    domain_agents: HashMap<String, Vec<String>>,
}

impl SemanticModelRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
            agents: HashMap::new(),
            domain_agents: HashMap::new(),
        }
    }

    /// Register a semantic model.
    pub fn register_model(&mut self, model: SemanticModel) {
        debug!(model = %model.name, entities = model.entities.len(), "Registering semantic model");
        self.models.insert(model.name.clone(), model);
    }

    /// Register an agent's capabilities.
    pub fn register_agent(&mut self, cap: AgentCapability) {
        debug!(agent = %cap.agent_id, domains = ?cap.domains, "Registering agent capability");
        // Update reverse index.
        for domain in &cap.domains {
            self.domain_agents
                .entry(domain.clone())
                .or_default()
                .push(cap.agent_id.clone());
        }
        self.agents.insert(cap.agent_id.clone(), cap);
    }

    /// Get a registered model by name.
    pub fn get_model(&self, name: &str) -> Option<&SemanticModel> {
        self.models.get(name)
    }

    /// Get an agent's capabilities.
    pub fn get_agent(&self, agent_id: &str) -> Option<&AgentCapability> {
        self.agents.get(agent_id)
    }

    /// Find the best agent for a given domain, ranked by proficiency.
    pub fn route_by_domain(&self, domain: &str) -> Vec<(&str, f64)> {
        let mut candidates: Vec<(&str, f64)> = self
            .domain_agents
            .get(domain)
            .into_iter()
            .flat_map(|ids| ids.iter())
            .filter_map(|id| self.agents.get(id).map(|a| (a.agent_id.as_str(), a.proficiency)))
            .collect();

        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        candidates
    }

    /// Find agents that specialize in a specific entity.
    pub fn route_by_entity(&self, entity_name: &str) -> Vec<(&str, f64)> {
        let mut candidates: Vec<(&str, f64)> = self
            .agents
            .values()
            .filter(|a| a.entity_specializations.iter().any(|e| e == entity_name))
            .map(|a| (a.agent_id.as_str(), a.proficiency))
            .collect();

        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        candidates
    }

    /// Generate context string for all models matching given domains.
    ///
    /// Used to inject schema context into LLM prompts to prevent hallucination.
    pub fn context_for_domains(&self, domains: &[&str]) -> String {
        let mut ctx = String::new();
        for model in self.models.values() {
            let model_tags = model.all_domain_tags();
            if domains.iter().any(|d| model_tags.iter().any(|t| t == d)) {
                ctx.push_str(&model.to_context_string());
                ctx.push('\n');
            }
        }
        ctx
    }

    /// List all registered model names.
    pub fn model_names(&self) -> Vec<&str> {
        self.models.keys().map(|k| k.as_str()).collect()
    }

    /// List all registered agent IDs.
    pub fn agent_ids(&self) -> Vec<&str> {
        self.agents.keys().map(|k| k.as_str()).collect()
    }

    /// Total entity count across all models.
    pub fn entity_count(&self) -> usize {
        self.models.values().map(|m| m.entities.len()).sum()
    }
}

impl Default for SemanticModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ecommerce_model() -> SemanticModel {
        SemanticModel {
            name: "ecommerce".into(),
            description: "E-commerce platform data model".into(),
            version: "1.0".into(),
            entities: vec![
                SemanticEntity {
                    name: "Customer".into(),
                    description: "Customer accounts".into(),
                    source: "postgres.customers".into(),
                    fields: vec![
                        SemanticField {
                            name: "id".into(),
                            semantic_type: "identifier".into(),
                            description: "Primary key".into(),
                            is_key: true,
                            links_to: None,
                        },
                        SemanticField {
                            name: "email".into(),
                            semantic_type: "dimension".into(),
                            description: "Customer email address".into(),
                            is_key: false,
                            links_to: None,
                        },
                    ],
                    domain_tags: vec!["ecommerce".into(), "crm".into()],
                },
                SemanticEntity {
                    name: "Order".into(),
                    description: "Purchase orders".into(),
                    source: "postgres.orders".into(),
                    fields: vec![
                        SemanticField {
                            name: "id".into(),
                            semantic_type: "identifier".into(),
                            description: "Order primary key".into(),
                            is_key: true,
                            links_to: None,
                        },
                        SemanticField {
                            name: "customer_id".into(),
                            semantic_type: "identifier".into(),
                            description: "Foreign key to Customer".into(),
                            is_key: true,
                            links_to: Some("Customer".into()),
                        },
                        SemanticField {
                            name: "total".into(),
                            semantic_type: "measure".into(),
                            description: "Order total in cents".into(),
                            is_key: false,
                            links_to: None,
                        },
                    ],
                    domain_tags: vec!["ecommerce".into(), "sales".into()],
                },
            ],
        }
    }

    #[test]
    fn test_entity_relationships() {
        let model = make_ecommerce_model();
        let order = model.get_entity("Order").unwrap();
        let rels = order.relationships();
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0], ("customer_id", "Customer"));
    }

    #[test]
    fn test_model_all_relationships() {
        let model = make_ecommerce_model();
        let rels = model.all_relationships();
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0], ("Order", "customer_id", "Customer"));
    }

    #[test]
    fn test_model_domain_tags() {
        let model = make_ecommerce_model();
        let tags = model.all_domain_tags();
        assert!(tags.contains(&"ecommerce".to_string()));
        assert!(tags.contains(&"crm".to_string()));
        assert!(tags.contains(&"sales".to_string()));
    }

    #[test]
    fn test_registry_route_by_domain() {
        let mut reg = SemanticModelRegistry::new();
        reg.register_model(make_ecommerce_model());

        reg.register_agent(AgentCapability {
            agent_id: "agent-sales".into(),
            domains: vec!["sales".into(), "ecommerce".into()],
            entity_specializations: vec!["Order".into()],
            proficiency: 0.9,
            step_types: vec!["crew.agent".into()],
        });

        reg.register_agent(AgentCapability {
            agent_id: "agent-crm".into(),
            domains: vec!["crm".into()],
            entity_specializations: vec!["Customer".into()],
            proficiency: 0.85,
            step_types: vec!["crew.agent".into()],
        });

        let sales_agents = reg.route_by_domain("sales");
        assert_eq!(sales_agents.len(), 1);
        assert_eq!(sales_agents[0].0, "agent-sales");

        let ecom_agents = reg.route_by_domain("ecommerce");
        assert_eq!(ecom_agents.len(), 1);
        assert_eq!(ecom_agents[0].0, "agent-sales");

        let crm_agents = reg.route_by_domain("crm");
        assert_eq!(crm_agents.len(), 1);
        assert_eq!(crm_agents[0].0, "agent-crm");
    }

    #[test]
    fn test_registry_route_by_entity() {
        let mut reg = SemanticModelRegistry::new();

        reg.register_agent(AgentCapability {
            agent_id: "agent-a".into(),
            domains: vec!["sales".into()],
            entity_specializations: vec!["Order".into(), "Product".into()],
            proficiency: 0.8,
            step_types: vec![],
        });

        reg.register_agent(AgentCapability {
            agent_id: "agent-b".into(),
            domains: vec!["sales".into()],
            entity_specializations: vec!["Order".into()],
            proficiency: 0.95,
            step_types: vec![],
        });

        let order_agents = reg.route_by_entity("Order");
        assert_eq!(order_agents.len(), 2);
        // Highest proficiency first.
        assert_eq!(order_agents[0].0, "agent-b");
        assert_eq!(order_agents[1].0, "agent-a");
    }

    #[test]
    fn test_context_for_domains() {
        let mut reg = SemanticModelRegistry::new();
        reg.register_model(make_ecommerce_model());

        let ctx = reg.context_for_domains(&["sales"]);
        assert!(ctx.contains("Order"));
        assert!(ctx.contains("total"));
        assert!(ctx.contains("Relationships:"));

        let empty = reg.context_for_domains(&["nonexistent"]);
        assert!(empty.is_empty());
    }

    #[test]
    fn test_entity_context_string() {
        let model = make_ecommerce_model();
        let customer = model.get_entity("Customer").unwrap();
        let ctx = customer.to_context_string();
        assert!(ctx.contains("Customer"));
        assert!(ctx.contains("[KEY]"));
        assert!(ctx.contains("email"));
    }

    #[test]
    fn test_semantic_model_serde_roundtrip() {
        let model = make_ecommerce_model();
        let json = serde_json::to_string(&model).unwrap();
        let back: SemanticModel = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "ecommerce");
        assert_eq!(back.entities.len(), 2);
        assert_eq!(back.entities[1].fields.len(), 3);
        assert_eq!(
            back.entities[1].fields[1].links_to.as_deref(),
            Some("Customer")
        );
    }

    #[test]
    fn test_registry_counts() {
        let mut reg = SemanticModelRegistry::new();
        reg.register_model(make_ecommerce_model());
        assert_eq!(reg.model_names().len(), 1);
        assert_eq!(reg.entity_count(), 2);
        assert_eq!(reg.agent_ids().len(), 0);

        reg.register_agent(AgentCapability {
            agent_id: "a1".into(),
            domains: vec![],
            entity_specializations: vec![],
            proficiency: 0.5,
            step_types: vec![],
        });
        assert_eq!(reg.agent_ids().len(), 1);
    }
}
