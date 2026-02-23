//! Chat Awareness Session Manager.
//!
//! The chat GUI is not a passive terminal — it's **agent 0x0C:00** in the
//! BindSpace, a first-class participant in the awareness loop.
//!
//! ## Architecture
//!
//! ```text
//! User Message
//!     ↓
//! IntentParser (classify: query / command / reflection / delegate)
//!     ↓
//! ContextBuilder (inject: semantic model + agent state + NARS evidence)
//!     ↓
//! CognitiveService.query_text() or .process_text()
//!     ↓
//! AwarenessRenderer (format: DK-gap, confidence, causal rung, flow state)
//!     ↓
//! Chat Response (with awareness cockpit metadata)
//! ```
//!
//! ## Awareness Cockpit
//!
//! Each response carries awareness metadata so the GUI can render:
//! - Confidence gauge (NARS expectation)
//! - DK-gap indicator (overconfidence warning)
//! - Active thinking style
//! - Flow state (FLOW / HOLD / BLOCK)
//! - Causal rung (See / Do / Imagine)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use tracing::debug;

// ============================================================================
// Intent Classification
// ============================================================================

/// Classified intent of a user message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatIntent {
    /// Direct question requiring resonance search.
    Query,
    /// Workflow command (create, run, modify).
    Command,
    /// Metacognitive reflection (how confident are you? why?).
    Reflection,
    /// Delegate to another agent or system.
    Delegate,
    /// Casual conversation / greeting.
    Conversation,
}

/// Result of intent parsing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedIntent {
    /// Primary intent classification.
    pub intent: ChatIntent,
    /// Confidence in classification (0.0–1.0).
    pub confidence: f64,
    /// Extracted keywords/entities.
    pub entities: Vec<String>,
    /// Semantic domains detected (for model routing).
    pub domains: Vec<String>,
    /// Target agent name if intent is Delegate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delegate_target: Option<String>,
}

/// Simple keyword-based intent parser.
///
/// In production, this would be replaced by a fine-tuned classifier or
/// the CognitiveService grammar triangle. For now, keyword matching
/// provides a working baseline.
pub fn parse_intent(message: &str) -> ParsedIntent {
    let lower = message.to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();

    // Delegate markers (check first — explicit @mention is highest priority)
    if lower.starts_with('@') || lower.contains("delegate to") || lower.contains("ask agent") {
        let target = words
            .iter()
            .find(|w| w.starts_with('@'))
            .map(|w| w.trim_start_matches('@').to_string());
        return ParsedIntent {
            intent: ChatIntent::Delegate,
            confidence: 0.8,
            entities: extract_entities(&words),
            domains: Vec::new(),
            delegate_target: target,
        };
    }

    // Reflection markers (check before generic query — has question mark too)
    let reflection_markers = ["confiden", "sure", "certain", "explain", "reasoning", "dk-gap"];
    if reflection_markers.iter().any(|m| lower.contains(m)) && lower.contains('?') {
        return ParsedIntent {
            intent: ChatIntent::Reflection,
            confidence: 0.7,
            entities: extract_entities(&words),
            domains: Vec::new(),
            delegate_target: None,
        };
    }

    // Command markers
    let command_markers = ["run", "execute", "create", "delete", "start", "stop", "deploy"];
    if command_markers.iter().any(|m| words.contains(m)) {
        return ParsedIntent {
            intent: ChatIntent::Command,
            confidence: 0.75,
            entities: extract_entities(&words),
            domains: Vec::new(),
            delegate_target: None,
        };
    }

    // Query (question mark or question words)
    let question_words = ["what", "how", "where", "when", "which", "who", "can", "does", "is"];
    if lower.contains('?') || question_words.iter().any(|q| words.first() == Some(q)) {
        return ParsedIntent {
            intent: ChatIntent::Query,
            confidence: 0.7,
            entities: extract_entities(&words),
            domains: Vec::new(),
            delegate_target: None,
        };
    }

    // Default: conversation
    ParsedIntent {
        intent: ChatIntent::Conversation,
        confidence: 0.5,
        entities: extract_entities(&words),
        domains: Vec::new(),
        delegate_target: None,
    }
}

fn extract_entities(words: &[&str]) -> Vec<String> {
    // Simple: collect capitalized words and @mentions as entities.
    words
        .iter()
        .filter(|w| {
            let first = w.chars().next().unwrap_or('a');
            first.is_uppercase() || w.starts_with('@') || w.starts_with('#')
        })
        .map(|w| w.to_string())
        .collect()
}

// ============================================================================
// Awareness Cockpit — metadata for GUI rendering
// ============================================================================

/// Awareness metadata attached to each chat response.
///
/// The GUI renders this as a cockpit display alongside the text response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwarenessCockpit {
    /// NARS expectation value (0.0–1.0). Higher = more confident.
    pub nars_expectation: f64,

    /// Dunning-Kruger gap: confidence - coherence.
    /// Positive = overconfident, negative = underconfident.
    pub dk_gap: f64,

    /// Current thinking style name.
    pub active_style: String,

    /// Flow state: "FLOW", "HOLD", or "BLOCK".
    pub flow_state: String,

    /// Causal rung: "see" (correlation), "do" (intervention), "imagine" (counterfactual).
    pub causal_rung: String,

    /// Cognitive cycle count.
    pub cycle: u64,

    /// Number of evidence items supporting this response.
    pub evidence_count: u32,

    /// Agent that produced this response.
    pub responding_agent: String,
}

impl Default for AwarenessCockpit {
    fn default() -> Self {
        Self {
            nars_expectation: 0.5,
            dk_gap: 0.0,
            active_style: "Analytical".into(),
            flow_state: "FLOW".into(),
            causal_rung: "see".into(),
            cycle: 0,
            evidence_count: 0,
            responding_agent: "chat:0x0C:00".into(),
        }
    }
}

// ============================================================================
// ChatMessage — message in a session
// ============================================================================

/// A single message in a chat session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Unique message ID.
    pub id: String,

    /// Role: "user", "assistant", "system", "agent".
    pub role: String,

    /// Message content.
    pub content: String,

    /// Timestamp.
    pub timestamp: DateTime<Utc>,

    /// Parsed intent (for user messages).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intent: Option<ParsedIntent>,

    /// Awareness cockpit (for assistant/agent responses).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub awareness: Option<AwarenessCockpit>,

    /// Which agent produced this (for multi-agent conversations).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
}

impl ChatMessage {
    /// Create a user message.
    pub fn user(content: impl Into<String>) -> Self {
        let content = content.into();
        let intent = Some(parse_intent(&content));
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role: "user".into(),
            content,
            timestamp: Utc::now(),
            intent,
            awareness: None,
            agent_id: None,
        }
    }

    /// Create an assistant response with awareness metadata.
    pub fn assistant(content: impl Into<String>, awareness: AwarenessCockpit) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role: "assistant".into(),
            content: content.into(),
            timestamp: Utc::now(),
            intent: None,
            awareness: Some(awareness),
            agent_id: Some("chat:0x0C:00".into()),
        }
    }

    /// Create a delegated agent response.
    pub fn agent(
        content: impl Into<String>,
        agent_id: impl Into<String>,
        awareness: AwarenessCockpit,
    ) -> Self {
        let agent_id = agent_id.into();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role: "agent".into(),
            content: content.into(),
            timestamp: Utc::now(),
            intent: None,
            awareness: Some(awareness),
            agent_id: Some(agent_id),
        }
    }
}

// ============================================================================
// ChatSession — awareness-enriched conversation
// ============================================================================

/// A chat session with awareness tracking.
///
/// Maintains conversation history, semantic context, and cognitive state
/// across multiple message exchanges.
pub struct ChatSession {
    /// Session ID.
    pub session_id: String,

    /// Messages in chronological order.
    pub messages: Vec<ChatMessage>,

    /// Session-level semantic context (accumulated domain tags).
    pub active_domains: Vec<String>,

    /// Session-level thinking style preference.
    pub preferred_style: Option<String>,

    /// Total cognitive cycles in this session.
    pub total_cycles: u64,

    /// Custom metadata.
    pub metadata: HashMap<String, String>,

    /// Session creation time.
    pub created_at: DateTime<Utc>,
}

impl ChatSession {
    /// Create a new chat session.
    pub fn new() -> Self {
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            messages: Vec::new(),
            active_domains: Vec::new(),
            preferred_style: None,
            total_cycles: 0,
            metadata: HashMap::new(),
            created_at: Utc::now(),
        }
    }

    /// Add a user message and return its parsed intent.
    pub fn add_user_message(&mut self, content: impl Into<String>) -> &ParsedIntent {
        let msg = ChatMessage::user(content);
        self.messages.push(msg);
        self.messages.last().unwrap().intent.as_ref().unwrap()
    }

    /// Add an assistant response with awareness metadata.
    pub fn add_response(&mut self, content: impl Into<String>, awareness: AwarenessCockpit) {
        self.total_cycles = awareness.cycle;
        let msg = ChatMessage::assistant(content, awareness);
        self.messages.push(msg);
    }

    /// Add a delegated agent response.
    pub fn add_agent_response(
        &mut self,
        content: impl Into<String>,
        agent_id: impl Into<String>,
        awareness: AwarenessCockpit,
    ) {
        self.total_cycles = awareness.cycle;
        let msg = ChatMessage::agent(content, agent_id, awareness);
        self.messages.push(msg);
    }

    /// Get the last N messages for context window.
    pub fn recent_messages(&self, n: usize) -> &[ChatMessage] {
        let start = self.messages.len().saturating_sub(n);
        &self.messages[start..]
    }

    /// Get all unique agents that have participated.
    pub fn participating_agents(&self) -> Vec<String> {
        let mut agents: Vec<String> = self
            .messages
            .iter()
            .filter_map(|m| m.agent_id.clone())
            .collect();
        agents.sort();
        agents.dedup();
        agents
    }

    /// Track active domains from message intents.
    pub fn update_domains(&mut self, domains: &[String]) {
        for d in domains {
            if !self.active_domains.contains(d) {
                debug!(session = %self.session_id, domain = %d, "New domain activated");
                self.active_domains.push(d.clone());
            }
        }
    }

    /// Message count.
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Average confidence across all assistant responses.
    pub fn avg_confidence(&self) -> f64 {
        let (sum, count) = self
            .messages
            .iter()
            .filter_map(|m| m.awareness.as_ref())
            .fold((0.0, 0u32), |(s, c), a| {
                (s + a.nars_expectation, c + 1)
            });
        if count > 0 {
            sum / count as f64
        } else {
            0.5
        }
    }
}

impl Default for ChatSession {
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

    #[test]
    fn test_parse_intent_query() {
        let intent = parse_intent("What is the total revenue for Q4?");
        assert_eq!(intent.intent, ChatIntent::Query);
    }

    #[test]
    fn test_parse_intent_command() {
        let intent = parse_intent("run the ETL workflow now");
        assert_eq!(intent.intent, ChatIntent::Command);
    }

    #[test]
    fn test_parse_intent_reflection() {
        let intent = parse_intent("how confident are you about that?");
        assert_eq!(intent.intent, ChatIntent::Reflection);
    }

    #[test]
    fn test_parse_intent_delegate() {
        let intent = parse_intent("@sales-agent check the pipeline");
        assert_eq!(intent.intent, ChatIntent::Delegate);
        assert_eq!(intent.delegate_target.as_deref(), Some("sales-agent"));
    }

    #[test]
    fn test_parse_intent_conversation() {
        let intent = parse_intent("hello there");
        assert_eq!(intent.intent, ChatIntent::Conversation);
    }

    #[test]
    fn test_chat_session_lifecycle() {
        let mut session = ChatSession::new();
        assert_eq!(session.message_count(), 0);

        // User asks a question
        let intent = session.add_user_message("What is the order total?");
        assert_eq!(intent.intent, ChatIntent::Query);
        assert_eq!(session.message_count(), 1);

        // Assistant responds with awareness
        let awareness = AwarenessCockpit {
            nars_expectation: 0.85,
            dk_gap: 0.05,
            active_style: "Analytical".into(),
            flow_state: "FLOW".into(),
            causal_rung: "see".into(),
            cycle: 42,
            evidence_count: 3,
            responding_agent: "chat:0x0C:00".into(),
        };
        session.add_response("The total is $1,234.56", awareness);
        assert_eq!(session.message_count(), 2);
        assert_eq!(session.total_cycles, 42);

        // Check avg confidence
        let avg = session.avg_confidence();
        assert!((avg - 0.85).abs() < 0.01);
    }

    #[test]
    fn test_chat_session_multi_agent() {
        let mut session = ChatSession::new();
        session.add_user_message("@analyst check the trends");

        let awareness = AwarenessCockpit::default();
        session.add_agent_response("Trends show 15% growth", "agent-analyst", awareness.clone());
        session.add_response("Summary of findings", awareness);

        let agents = session.participating_agents();
        assert!(agents.contains(&"agent-analyst".to_string()));
        // chat:0x0C:00 comes from add_response (assistant messages).
        assert!(agents.contains(&"chat:0x0C:00".to_string()));
    }

    #[test]
    fn test_awareness_cockpit_serde() {
        let cockpit = AwarenessCockpit {
            nars_expectation: 0.92,
            dk_gap: -0.03,
            active_style: "Divergent".into(),
            flow_state: "HOLD".into(),
            causal_rung: "do".into(),
            cycle: 100,
            evidence_count: 7,
            responding_agent: "chat:0x0C:00".into(),
        };

        let json = serde_json::to_string(&cockpit).unwrap();
        let back: AwarenessCockpit = serde_json::from_str(&json).unwrap();
        assert_eq!(back.nars_expectation, 0.92);
        assert_eq!(back.flow_state, "HOLD");
        assert_eq!(back.causal_rung, "do");
    }

    #[test]
    fn test_recent_messages() {
        let mut session = ChatSession::new();
        for i in 0..10 {
            session.add_user_message(format!("Message {}", i));
        }
        let recent = session.recent_messages(3);
        assert_eq!(recent.len(), 3);
        assert!(recent[0].content.contains("7"));
        assert!(recent[2].content.contains("9"));
    }

    #[test]
    fn test_domain_tracking() {
        let mut session = ChatSession::new();
        session.update_domains(&["sales".into(), "crm".into()]);
        session.update_domains(&["sales".into(), "analytics".into()]);
        // "sales" should not be duplicated.
        assert_eq!(session.active_domains.len(), 3);
        assert!(session.active_domains.contains(&"analytics".to_string()));
    }
}
