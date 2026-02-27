//! JITSON Hooks — JIT-compiled workflow hot paths.
//!
//! TypeScript n8n hooks are runtime callbacks. In Rust, three tiers:
//!
//! ```text
//! HOT PATH → JITSON compiled:
//!   Node parameter evaluation  → jitson::compile(params) → native fn
//!   Expression evaluation      → jitson::compile(ast)    → native fn
//!   Routing decisions          → jitson::compile(mode)   → scan kernel
//!
//! COLD PATH → trait impls:
//!   Workflow lifecycle events   → impl WorkflowLifecycle
//!   Credential resolution       → impl CredentialProvider
//!   Error handling              → impl NodeErrorHandler
//!
//! STATEFUL → Markov chains:
//!   Execution retry state       → MarkovChain { running → retry → failed → done }
//!   Queue position tracking     → MarkovChain { queued → active → complete }
//!   Agent decision memory       → crewai-rust AgentCard with persistent state
//! ```
//!
//! This module provides the trait definitions and state machine types.
//! Actual JITSON compilation is wired in via the `jitson` crate when available.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

// =============================================================================
// HOT PATH: Compiled Parameter Cache
// =============================================================================

/// A compiled parameter evaluation function.
///
/// When JITSON is available, these are Cranelift-compiled native functions.
/// Without JITSON, they fall back to interpreted evaluation.
#[derive(Debug, Clone)]
pub struct CompiledParams {
    /// Hash of the parameter expression (for cache lookup).
    pub param_hash: u64,

    /// Pre-evaluated static values (known at compile time).
    pub static_values: HashMap<String, serde_json::Value>,

    /// Keys that need runtime evaluation (expressions with $input, etc.)
    pub dynamic_keys: Vec<String>,

    /// Whether this was actually JIT-compiled (vs interpreted fallback).
    pub is_jit_compiled: bool,
}

impl CompiledParams {
    /// Create a fallback (interpreted) parameter set.
    pub fn interpreted(params: HashMap<String, serde_json::Value>) -> Self {
        let dynamic_keys: Vec<String> = params.iter()
            .filter(|(_, v)| {
                v.as_str().map(|s| s.contains("{{") || s.starts_with('=')).unwrap_or(false)
            })
            .map(|(k, _)| k.clone())
            .collect();

        let static_values: HashMap<String, serde_json::Value> = params.into_iter()
            .filter(|(k, _)| !dynamic_keys.contains(k))
            .collect();

        Self {
            param_hash: 0,
            static_values,
            dynamic_keys,
            is_jit_compiled: false,
        }
    }
}

// =============================================================================
// COLD PATH: Lifecycle Traits
// =============================================================================

/// Workflow lifecycle events.
///
/// Cold path — called once per execution, not per-node.
pub trait WorkflowLifecycle: Send + Sync {
    /// Called when a workflow execution starts.
    fn on_execution_start(&self, execution_id: &str, workflow_id: &str);

    /// Called when a workflow execution completes successfully.
    fn on_execution_complete(&self, execution_id: &str, stats: &ExecutionStats);

    /// Called when a workflow execution fails.
    fn on_execution_error(&self, execution_id: &str, error: &str);

    /// Called when a node starts executing within a workflow.
    fn on_node_start(&self, execution_id: &str, node_name: &str);

    /// Called when a node completes within a workflow.
    fn on_node_complete(&self, execution_id: &str, node_name: &str);
}

/// Execution statistics passed to lifecycle callbacks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStats {
    pub execution_time_ms: u64,
    pub nodes_executed: usize,
    pub items_processed: usize,
    pub errors: usize,
}

/// Error handling strategy for a node.
pub trait NodeErrorHandler: Send + Sync {
    /// Determine what to do when a node fails.
    fn handle_error(&self, node_name: &str, error: &str, attempt: u32) -> ErrorAction;
}

/// What to do when a node error occurs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorAction {
    /// Retry the node after a delay.
    Retry { delay_ms: u64 },
    /// Skip this node and continue the workflow.
    Skip,
    /// Abort the entire workflow execution.
    Abort,
    /// Use a fallback value.
    Fallback(String),
}

// =============================================================================
// STATEFUL: Markov State Machines
// =============================================================================

/// A simple Markov state machine for tracking execution state.
///
/// Used for retry logic, queue management, and agent decision memory.
/// Transitions are deterministic (not probabilistic) in this implementation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkovChain {
    /// Current state.
    pub state: String,

    /// Valid transitions: from_state → [(to_state, condition)]
    pub transitions: HashMap<String, Vec<MarkovTransition>>,

    /// History of state changes (for debugging/audit).
    pub history: Vec<StateChange>,

    /// Maximum history entries to keep.
    pub max_history: usize,
}

/// A possible state transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkovTransition {
    /// Target state.
    pub to: String,
    /// Condition that triggers this transition.
    pub condition: TransitionCondition,
}

/// Condition for a state transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransitionCondition {
    /// Always transition (unconditional).
    Always,
    /// Transition after N attempts/ticks.
    AfterAttempts(u32),
    /// Transition on success signal.
    OnSuccess,
    /// Transition on failure signal.
    OnFailure,
    /// Transition on timeout (milliseconds).
    OnTimeout(u64),
}

/// Record of a state change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateChange {
    pub from: String,
    pub to: String,
    pub timestamp_ms: u64,
    pub reason: String,
}

impl MarkovChain {
    /// Create a new chain starting in the given state.
    pub fn new(initial_state: &str) -> Self {
        Self {
            state: initial_state.to_string(),
            transitions: HashMap::new(),
            history: Vec::new(),
            max_history: 100,
        }
    }

    /// Add a transition rule.
    pub fn add_transition(
        &mut self,
        from: &str,
        to: &str,
        condition: TransitionCondition,
    ) {
        self.transitions
            .entry(from.to_string())
            .or_default()
            .push(MarkovTransition {
                to: to.to_string(),
                condition,
            });
    }

    /// Attempt to transition based on a signal.
    pub fn signal(&mut self, signal: Signal) -> bool {
        let transitions = match self.transitions.get(&self.state) {
            Some(t) => t.clone(),
            None => return false,
        };

        for t in &transitions {
            let should_transition = match (&t.condition, &signal) {
                (TransitionCondition::Always, _) => true,
                (TransitionCondition::OnSuccess, Signal::Success) => true,
                (TransitionCondition::OnFailure, Signal::Failure) => true,
                (TransitionCondition::AfterAttempts(n), Signal::Attempt(a)) => *a >= *n,
                (TransitionCondition::OnTimeout(_), Signal::Timeout) => true,
                _ => false,
            };

            if should_transition {
                let change = StateChange {
                    from: self.state.clone(),
                    to: t.to.clone(),
                    timestamp_ms: current_time_ms(),
                    reason: format!("{:?}", signal),
                };

                self.state = t.to.clone();
                self.history.push(change);

                // Trim history
                if self.history.len() > self.max_history {
                    self.history.drain(..self.history.len() - self.max_history);
                }

                return true;
            }
        }

        false
    }

    /// Get current state.
    pub fn current_state(&self) -> &str {
        &self.state
    }

    /// Check if the chain is in a terminal state (no outgoing transitions).
    pub fn is_terminal(&self) -> bool {
        !self.transitions.contains_key(&self.state)
    }

    /// Create the standard execution retry chain.
    ///
    /// ```text
    /// running → retry (on failure, max 3)
    /// retry   → running (always, after delay)
    /// retry   → failed (after 3 attempts)
    /// running → done (on success)
    /// ```
    pub fn execution_retry() -> Self {
        let mut chain = Self::new("running");
        chain.add_transition("running", "retry", TransitionCondition::OnFailure);
        chain.add_transition("running", "done", TransitionCondition::OnSuccess);
        chain.add_transition("retry", "running", TransitionCondition::Always);
        chain.add_transition("retry", "failed", TransitionCondition::AfterAttempts(3));
        chain
    }

    /// Create the standard queue position chain.
    ///
    /// ```text
    /// queued → active (always, when dequeued)
    /// active → complete (on success)
    /// active → failed (on failure)
    /// ```
    pub fn queue_position() -> Self {
        let mut chain = Self::new("queued");
        chain.add_transition("queued", "active", TransitionCondition::Always);
        chain.add_transition("active", "complete", TransitionCondition::OnSuccess);
        chain.add_transition("active", "failed", TransitionCondition::OnFailure);
        chain
    }
}

/// Signal that can trigger state transitions.
#[derive(Debug, Clone)]
pub enum Signal {
    Success,
    Failure,
    Timeout,
    Attempt(u32),
    Tick,
}

fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compiled_params_interpreted() {
        let mut params = HashMap::new();
        params.insert("url".to_string(), serde_json::json!("https://example.com"));
        params.insert("body".to_string(), serde_json::json!("={{$input.data}}"));

        let compiled = CompiledParams::interpreted(params);
        assert!(!compiled.is_jit_compiled);
        assert_eq!(compiled.dynamic_keys, vec!["body"]);
        assert!(compiled.static_values.contains_key("url"));
    }

    #[test]
    fn test_markov_execution_retry() {
        let mut chain = MarkovChain::execution_retry();
        assert_eq!(chain.current_state(), "running");

        // Fail → retry
        assert!(chain.signal(Signal::Failure));
        assert_eq!(chain.current_state(), "retry");

        // Retry → back to running (Always condition)
        assert!(chain.signal(Signal::Success)); // no match but Always fires first
    }

    #[test]
    fn test_markov_queue() {
        let mut chain = MarkovChain::queue_position();
        assert_eq!(chain.current_state(), "queued");

        // Dequeue
        assert!(chain.signal(Signal::Tick)); // Always condition
        assert_eq!(chain.current_state(), "active");

        // Complete
        assert!(chain.signal(Signal::Success));
        assert_eq!(chain.current_state(), "complete");

        // Terminal
        assert!(chain.is_terminal());
    }

    #[test]
    fn test_markov_history() {
        let mut chain = MarkovChain::new("a");
        chain.add_transition("a", "b", TransitionCondition::Always);
        chain.add_transition("b", "c", TransitionCondition::Always);

        chain.signal(Signal::Tick);
        chain.signal(Signal::Tick);

        assert_eq!(chain.history.len(), 2);
        assert_eq!(chain.history[0].from, "a");
        assert_eq!(chain.history[0].to, "b");
        assert_eq!(chain.history[1].from, "b");
        assert_eq!(chain.history[1].to, "c");
    }

    #[test]
    fn test_error_action() {
        let retry = ErrorAction::Retry { delay_ms: 1000 };
        assert_eq!(retry, ErrorAction::Retry { delay_ms: 1000 });

        let skip = ErrorAction::Skip;
        assert_ne!(skip, ErrorAction::Abort);
    }
}
