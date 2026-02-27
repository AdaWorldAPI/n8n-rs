//! ThinkingMode dispatch — route queries by NARS inference type.
//!
//! Each thinking mode maps to a ladybug-rs query strategy and a neo4j-rs
//! Cypher pattern. The mode is the bridge between human intent and
//! machine strategy.
//!
//! ```text
//! User intent → ThinkingMode → QueryPlan
//!   "Find X"  → Deduction    → CamExact (direct lookup)
//!   "What is" → Induction    → CamWide (pattern scan)
//!   "Why did" → Abduction    → DnTreeFull (deep traversal)
//!   "Update"  → Revision     → BundleInto (learning)
//!   "Connect" → Synthesis    → BundleAcross (multi-path)
//! ```
//!
//! # Neo4j-rs Cypher mapping
//!
//! Each mode also maps to a Cypher query pattern:
//! - **Deduction**: `MATCH (n) WHERE n.id = $id` → direct CAM lookup
//! - **Induction**: `MATCH (n)-[r]->(m) WHERE n.type = $type` → wide scan
//! - **Abduction**: `MATCH path = (n)-[*1..5]->(m)` → DN-tree traversal
//! - **Revision**: `MERGE (n) SET n.confidence = $new` → bundle with learning
//! - **Synthesis**: `MATCH (a), (b) RETURN collect(a)` → multi-path bundle

use serde::{Deserialize, Serialize};

// =============================================================================
// THINKING MODE
// =============================================================================

/// A thinking mode determines how ladybug-rs processes a query.
///
/// Set by the user (via UI), by crewai-rust agents (via ThinkingTemplate),
/// or auto-detected from the Cypher pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingMode {
    /// Which NARS inference type to use.
    pub inference_type: InferenceType,

    /// CAM top-k: how many candidates to retrieve from the CAM index.
    /// Higher = more results, slower. Default: 10.
    pub cam_top_k: usize,

    /// Beam width for DN-tree traversal.
    /// Higher = more thorough search. Default: 4.
    pub beam_width: usize,

    /// Learning rate for Revision mode.
    /// How much to update the bundled fingerprint. Default: 0.1.
    pub learning_rate: f64,

    /// BTSP gate probability for Revision mode.
    /// Probability of activating burst timing-dependent plasticity. Default: 0.5.
    pub btsp_gate_prob: f64,

    /// Winner-k for BNN (Binary Neural Network) in Synthesis mode.
    /// How many winners to keep in competitive learning. Default: 3.
    pub bnn_winner_k: usize,

    /// Maximum depth for DN-tree traversal (Abduction mode).
    pub max_depth: usize,

    /// Whether to allow early exit in search (vs exhaustive).
    pub allow_early_exit: bool,
}

impl Default for ThinkingMode {
    fn default() -> Self {
        Self {
            inference_type: InferenceType::Deduction,
            cam_top_k: 10,
            beam_width: 4,
            learning_rate: 0.1,
            btsp_gate_prob: 0.5,
            bnn_winner_k: 3,
            max_depth: 5,
            allow_early_exit: true,
        }
    }
}

// =============================================================================
// INFERENCE TYPE
// =============================================================================

/// NARS inference type — determines the reasoning strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InferenceType {
    /// Direct lookup: "I know X, find X" → exact CAM search
    Deduction,
    /// Pattern matching: "Things like X" → wide CAM scan
    Induction,
    /// Root cause: "Why did X happen?" → full DN-tree traversal
    Abduction,
    /// Update belief: "X changed" → bundle_into with learning rate
    Revision,
    /// Cross-domain: "Connect X and Y" → multi-path bundle
    Synthesis,
}

// =============================================================================
// QUERY PLAN
// =============================================================================

/// Physical query plan produced by thinking mode dispatch.
///
/// This is what ladybug-rs actually executes. The executor maps these
/// to BindSpace operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueryPlan {
    /// Exact CAM search — direct lookup by fingerprint.
    /// Used for Deduction: "Find this specific thing."
    CamExact {
        top_k: usize,
        beam: usize,
    },

    /// Wide CAM scan — broader search with windowed access.
    /// Used for Induction: "Find things like this."
    CamWide {
        top_k: usize,
        window: usize,
    },

    /// Full DN-tree traversal — deep graph walk.
    /// Used for Abduction: "Explain why this happened."
    DnTreeFull {
        beam: usize,
        no_early_exit: bool,
    },

    /// Bundle into existing node — update with learning.
    /// Used for Revision: "Update my understanding."
    BundleInto {
        lr: f64,
        btsp: f64,
    },

    /// Bundle across multiple paths — synthesis.
    /// Used for Synthesis: "Connect these concepts."
    BundleAcross {
        winner_k: usize,
    },
}

// =============================================================================
// DISPATCH
// =============================================================================

/// Route a query to the appropriate plan based on thinking mode.
///
/// This is the central dispatch function. It takes a ThinkingMode
/// (which encodes the user's intent) and produces a QueryPlan
/// (which encodes the machine's strategy).
pub fn route_by_thinking_mode(mode: &ThinkingMode) -> QueryPlan {
    match mode.inference_type {
        InferenceType::Deduction => QueryPlan::CamExact {
            top_k: mode.cam_top_k,
            beam: 1,
        },
        InferenceType::Induction => QueryPlan::CamWide {
            top_k: mode.cam_top_k,
            window: 64,
        },
        InferenceType::Abduction => QueryPlan::DnTreeFull {
            beam: mode.beam_width,
            no_early_exit: !mode.allow_early_exit,
        },
        InferenceType::Revision => QueryPlan::BundleInto {
            lr: mode.learning_rate,
            btsp: mode.btsp_gate_prob,
        },
        InferenceType::Synthesis => QueryPlan::BundleAcross {
            winner_k: mode.bnn_winner_k,
        },
    }
}

/// Auto-detect thinking mode from a Cypher query pattern.
///
/// Heuristic: look at the query structure to guess the best mode.
pub fn detect_from_cypher(cypher: &str) -> ThinkingMode {
    let upper = cypher.to_uppercase();

    let inference_type = if upper.contains("MERGE") || upper.contains("SET") {
        InferenceType::Revision
    } else if upper.contains("[*") || upper.contains("path =") || upper.contains("PATH =") {
        InferenceType::Abduction
    } else if upper.contains("COLLECT(") || (upper.contains("MATCH") && upper.matches("MATCH").count() > 1) {
        InferenceType::Synthesis
    } else if upper.contains("WHERE") && upper.contains("RETURN") && !upper.contains("[") {
        InferenceType::Deduction
    } else {
        InferenceType::Induction
    };

    ThinkingMode {
        inference_type,
        ..Default::default()
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_mode() {
        let mode = ThinkingMode::default();
        assert_eq!(mode.inference_type, InferenceType::Deduction);
        assert_eq!(mode.cam_top_k, 10);
    }

    #[test]
    fn test_route_deduction() {
        let mode = ThinkingMode {
            inference_type: InferenceType::Deduction,
            cam_top_k: 5,
            ..Default::default()
        };
        let plan = route_by_thinking_mode(&mode);
        match plan {
            QueryPlan::CamExact { top_k, beam } => {
                assert_eq!(top_k, 5);
                assert_eq!(beam, 1);
            }
            _ => panic!("Expected CamExact"),
        }
    }

    #[test]
    fn test_route_abduction() {
        let mode = ThinkingMode {
            inference_type: InferenceType::Abduction,
            beam_width: 8,
            allow_early_exit: false,
            ..Default::default()
        };
        let plan = route_by_thinking_mode(&mode);
        match plan {
            QueryPlan::DnTreeFull { beam, no_early_exit } => {
                assert_eq!(beam, 8);
                assert!(no_early_exit);
            }
            _ => panic!("Expected DnTreeFull"),
        }
    }

    #[test]
    fn test_detect_from_cypher_revision() {
        let mode = detect_from_cypher("MERGE (n:System {name: 'X'}) SET n.status = 'active'");
        assert_eq!(mode.inference_type, InferenceType::Revision);
    }

    #[test]
    fn test_detect_from_cypher_abduction() {
        let mode = detect_from_cypher("MATCH path = (a)-[*1..5]->(b) RETURN path");
        assert_eq!(mode.inference_type, InferenceType::Abduction);
    }

    #[test]
    fn test_detect_from_cypher_deduction() {
        let mode = detect_from_cypher("MATCH (n:System) WHERE n.name = 'Predator' RETURN n");
        assert_eq!(mode.inference_type, InferenceType::Deduction);
    }

    #[test]
    fn test_detect_from_cypher_synthesis() {
        let mode = detect_from_cypher("MATCH (a:System), (b:Stakeholder) RETURN collect(a)");
        assert_eq!(mode.inference_type, InferenceType::Synthesis);
    }
}
