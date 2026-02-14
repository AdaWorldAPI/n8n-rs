//! RBAC Impact Gates — controlling the "free will" boundary.
//!
//! Impact gates determine what cognitive operations are permitted based on:
//! 1. The role of the requesting entity (user, agent, kernel)
//! 2. The impact level of the operation (observe → critical)
//! 3. The current satisfaction state of the cognitive stack
//! 4. NARS truth value evidence (confidence must meet threshold)
//!
//! This implements the n8n-rs responsibility: gating what ladybug-rs is
//! allowed to self-modify. crewai-rust agents propose changes; n8n-rs
//! gates them by impact classification; ladybug-rs executes within limits.
//!
//! ```text
//! Impact Classification:
//!
//!   OBSERVE ───── Read-only queries, search, introspection
//!       │         Max: unlimited.  No side effects.
//!       ▼
//!   INTERNAL ──── Cache updates, memory writes, layer activation
//!       │         Max: rate-limited. Internal state only.
//!       ▼
//!   MODERATE ──── Notifications, external API calls, agent delegation
//!       │         Max: per-minute budget. Reversible effects.
//!       ▼
//!   SIGNIFICANT ─ Deployments, payments, data mutations
//!       │         Max: per-hour budget. Requires confidence > 0.8.
//!       ▼
//!   CRITICAL ──── Self-modification, architectural changes, weight updates
//!                 Max: per-day budget. Requires confidence > 0.95.
//!                 NARS evidence threshold: confidence * frequency > 0.9.
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ladybug_contract::nars::TruthValue;

use crate::interface_gateway::ImpactLevel;

// =============================================================================
// ROLE DEFINITIONS
// =============================================================================

/// A role in the RBAC system with its maximum allowed impact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleDefinition {
    /// Role identifier (e.g. "viewer", "executor", "system_architect").
    pub role_id: String,

    /// Human-readable name.
    pub name: String,

    /// Maximum impact level this role can authorize.
    pub max_impact: ImpactLevel,

    /// Per-minute budget for operations above Internal.
    pub budget_per_minute: u32,

    /// Per-hour budget for Significant+ operations.
    pub budget_per_hour: u32,

    /// Per-day budget for Critical operations.
    pub budget_per_day: u32,

    /// Minimum NARS confidence required for this role's max impact.
    pub min_confidence: f32,
}

// =============================================================================
// GATE DECISION
// =============================================================================

/// Result of an impact gate check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateDecision {
    /// Operation is allowed.
    Allow,

    /// Operation denied — impact exceeds role's maximum.
    DenyImpact,

    /// Operation denied — insufficient NARS evidence.
    DenyEvidence,

    /// Operation denied — budget exhausted.
    DenyBudget,

    /// Operation denied — satisfaction gate not met (cognitive stack unhealthy).
    DenySatisfaction,
}

// =============================================================================
// IMPACT GATE ENGINE
// =============================================================================

/// The Impact Gate engine — RBAC-based cognitive operation gating.
///
/// This is the core safety mechanism that controls what the cognitive
/// substrate (ladybug-rs) is allowed to do when operating autonomously.
#[derive(Debug, Clone)]
pub struct ImpactGate {
    /// Registered roles.
    roles: HashMap<String, RoleDefinition>,

    /// Budget counters: role → (minute_count, hour_count, day_count).
    budgets: HashMap<String, (u32, u32, u32)>,

    /// Minimum satisfaction score (0.0–1.0) required across the 10-layer stack.
    /// If average satisfaction drops below this, Critical operations are blocked.
    min_stack_satisfaction: f32,
}

impl ImpactGate {
    /// Create a new impact gate with default roles.
    pub fn new() -> Self {
        let mut gate = Self {
            roles: HashMap::new(),
            budgets: HashMap::new(),
            min_stack_satisfaction: 0.3,
        };
        gate.register_default_roles();
        gate
    }

    /// Register a role definition.
    pub fn register_role(&mut self, role: RoleDefinition) {
        self.budgets.insert(role.role_id.clone(), (0, 0, 0));
        self.roles.insert(role.role_id.clone(), role);
    }

    /// Check whether an operation should be allowed.
    ///
    /// This is the primary gate function. It checks:
    /// 1. Role's maximum impact level
    /// 2. NARS evidence (confidence × frequency)
    /// 3. Budget availability
    /// 4. Cognitive stack satisfaction (for Critical ops)
    pub fn check(
        &self,
        role_id: &str,
        impact: ImpactLevel,
        truth_value: &TruthValue,
        stack_satisfaction: &[f32],
    ) -> GateDecision {
        // Look up role
        let role = match self.roles.get(role_id) {
            Some(r) => r,
            None => return GateDecision::DenyImpact, // unknown role = deny
        };

        // 1. Impact level check
        if impact > role.max_impact {
            return GateDecision::DenyImpact;
        }

        // 2. Evidence check (for Significant+ operations)
        if impact >= ImpactLevel::Significant {
            let evidence = truth_value.frequency * truth_value.confidence;
            if truth_value.confidence < role.min_confidence {
                return GateDecision::DenyEvidence;
            }
            if impact >= ImpactLevel::Critical && evidence < 0.9 {
                return GateDecision::DenyEvidence;
            }
        }

        // 3. Budget check
        if let Some(&(minute, hour, day)) = self.budgets.get(role_id) {
            if impact >= ImpactLevel::Moderate && minute >= role.budget_per_minute {
                return GateDecision::DenyBudget;
            }
            if impact >= ImpactLevel::Significant && hour >= role.budget_per_hour {
                return GateDecision::DenyBudget;
            }
            if impact >= ImpactLevel::Critical && day >= role.budget_per_day {
                return GateDecision::DenyBudget;
            }
        }

        // 4. Satisfaction check (for Critical operations)
        if impact >= ImpactLevel::Critical && !stack_satisfaction.is_empty() {
            let avg: f32 =
                stack_satisfaction.iter().sum::<f32>() / stack_satisfaction.len() as f32;
            if avg < self.min_stack_satisfaction {
                return GateDecision::DenySatisfaction;
            }
        }

        GateDecision::Allow
    }

    /// Record that an operation was performed (increment budget counters).
    pub fn record_operation(&mut self, role_id: &str, impact: ImpactLevel) {
        if let Some(budget) = self.budgets.get_mut(role_id) {
            if impact >= ImpactLevel::Moderate {
                budget.0 += 1; // minute counter
            }
            if impact >= ImpactLevel::Significant {
                budget.1 += 1; // hour counter
            }
            if impact >= ImpactLevel::Critical {
                budget.2 += 1; // day counter
            }
        }
    }

    /// Reset minute-level budget counters (call every minute).
    pub fn reset_minute_budgets(&mut self) {
        for budget in self.budgets.values_mut() {
            budget.0 = 0;
        }
    }

    /// Reset hour-level budget counters (call every hour).
    pub fn reset_hour_budgets(&mut self) {
        for budget in self.budgets.values_mut() {
            budget.1 = 0;
        }
    }

    /// Reset day-level budget counters (call daily).
    pub fn reset_day_budgets(&mut self) {
        for budget in self.budgets.values_mut() {
            budget.2 = 0;
        }
    }

    /// Get a role by ID.
    pub fn get_role(&self, role_id: &str) -> Option<&RoleDefinition> {
        self.roles.get(role_id)
    }

    /// List all role IDs.
    pub fn role_ids(&self) -> Vec<&str> {
        self.roles.keys().map(|s| s.as_str()).collect()
    }

    /// Register the default role hierarchy.
    fn register_default_roles(&mut self) {
        let defaults = vec![
            RoleDefinition {
                role_id: "viewer".into(),
                name: "Viewer".into(),
                max_impact: ImpactLevel::Observe,
                budget_per_minute: 1000,
                budget_per_hour: 10000,
                budget_per_day: 100000,
                min_confidence: 0.0,
            },
            RoleDefinition {
                role_id: "operator".into(),
                name: "Operator".into(),
                max_impact: ImpactLevel::Internal,
                budget_per_minute: 500,
                budget_per_hour: 5000,
                budget_per_day: 50000,
                min_confidence: 0.5,
            },
            RoleDefinition {
                role_id: "executor".into(),
                name: "Executor".into(),
                max_impact: ImpactLevel::Moderate,
                budget_per_minute: 100,
                budget_per_hour: 1000,
                budget_per_day: 10000,
                min_confidence: 0.6,
            },
            RoleDefinition {
                role_id: "agent_operator".into(),
                name: "Agent Operator".into(),
                max_impact: ImpactLevel::Moderate,
                budget_per_minute: 200,
                budget_per_hour: 2000,
                budget_per_day: 20000,
                min_confidence: 0.7,
            },
            RoleDefinition {
                role_id: "cognitive_operator".into(),
                name: "Cognitive Operator".into(),
                max_impact: ImpactLevel::Significant,
                budget_per_minute: 50,
                budget_per_hour: 500,
                budget_per_day: 5000,
                min_confidence: 0.8,
            },
            RoleDefinition {
                role_id: "cognitive_admin".into(),
                name: "Cognitive Admin".into(),
                max_impact: ImpactLevel::Significant,
                budget_per_minute: 20,
                budget_per_hour: 200,
                budget_per_day: 2000,
                min_confidence: 0.85,
            },
            RoleDefinition {
                role_id: "system_architect".into(),
                name: "System Architect".into(),
                max_impact: ImpactLevel::Critical,
                budget_per_minute: 5,
                budget_per_hour: 20,
                budget_per_day: 50,
                min_confidence: 0.95,
            },
            // Autonomous agent role — ladybug-rs operating with "free will"
            RoleDefinition {
                role_id: "autonomous_kernel".into(),
                name: "Autonomous Kernel".into(),
                max_impact: ImpactLevel::Critical,
                budget_per_minute: 2,
                budget_per_hour: 10,
                budget_per_day: 20,
                min_confidence: 0.98,
            },
        ];

        for role in defaults {
            self.register_role(role);
        }
    }
}

impl Default for ImpactGate {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_viewer_observe_allowed() {
        let gate = ImpactGate::new();
        let tv = TruthValue::new(1.0, 0.5);
        let sat = [0.5; 10];

        assert_eq!(
            gate.check("viewer", ImpactLevel::Observe, &tv, &sat),
            GateDecision::Allow
        );
    }

    #[test]
    fn test_viewer_moderate_denied() {
        let gate = ImpactGate::new();
        let tv = TruthValue::new(1.0, 0.9);
        let sat = [0.5; 10];

        assert_eq!(
            gate.check("viewer", ImpactLevel::Moderate, &tv, &sat),
            GateDecision::DenyImpact
        );
    }

    #[test]
    fn test_executor_moderate_allowed() {
        let gate = ImpactGate::new();
        let tv = TruthValue::new(1.0, 0.9);
        let sat = [0.5; 10];

        assert_eq!(
            gate.check("executor", ImpactLevel::Moderate, &tv, &sat),
            GateDecision::Allow
        );
    }

    #[test]
    fn test_system_architect_critical_allowed() {
        let gate = ImpactGate::new();
        let tv = TruthValue::new(0.98, 0.97);
        let sat = [0.5; 10];

        assert_eq!(
            gate.check("system_architect", ImpactLevel::Critical, &tv, &sat),
            GateDecision::Allow
        );
    }

    #[test]
    fn test_critical_low_evidence_denied() {
        let gate = ImpactGate::new();
        // frequency * confidence = 0.5 * 0.5 = 0.25 < 0.9
        let tv = TruthValue::new(0.5, 0.5);
        let sat = [0.5; 10];

        assert_eq!(
            gate.check("system_architect", ImpactLevel::Critical, &tv, &sat),
            GateDecision::DenyEvidence
        );
    }

    #[test]
    fn test_critical_low_satisfaction_denied() {
        let gate = ImpactGate::new();
        let tv = TruthValue::new(0.99, 0.99);
        // Very low satisfaction across all layers
        let sat = [0.1; 10];

        assert_eq!(
            gate.check("system_architect", ImpactLevel::Critical, &tv, &sat),
            GateDecision::DenySatisfaction
        );
    }

    #[test]
    fn test_budget_exhaustion() {
        let mut gate = ImpactGate::new();
        let tv = TruthValue::new(1.0, 0.9);
        let sat = [0.5; 10];

        // Exhaust the executor's per-minute budget (100)
        for _ in 0..100 {
            gate.record_operation("executor", ImpactLevel::Moderate);
        }

        assert_eq!(
            gate.check("executor", ImpactLevel::Moderate, &tv, &sat),
            GateDecision::DenyBudget
        );

        // Reset and it should work again
        gate.reset_minute_budgets();
        assert_eq!(
            gate.check("executor", ImpactLevel::Moderate, &tv, &sat),
            GateDecision::Allow
        );
    }

    #[test]
    fn test_autonomous_kernel_strict_limits() {
        let gate = ImpactGate::new();

        // The autonomous kernel needs very high evidence
        let low_tv = TruthValue::new(0.9, 0.9);
        let sat = [0.5; 10];
        // confidence 0.9 < min_confidence 0.98
        assert_eq!(
            gate.check("autonomous_kernel", ImpactLevel::Significant, &low_tv, &sat),
            GateDecision::DenyEvidence
        );

        // With sufficient evidence, allowed
        let high_tv = TruthValue::new(0.99, 0.99);
        assert_eq!(
            gate.check("autonomous_kernel", ImpactLevel::Critical, &high_tv, &sat),
            GateDecision::Allow
        );
    }

    #[test]
    fn test_unknown_role_denied() {
        let gate = ImpactGate::new();
        let tv = TruthValue::new(1.0, 1.0);
        let sat = [1.0; 10];

        assert_eq!(
            gate.check("nonexistent", ImpactLevel::Observe, &tv, &sat),
            GateDecision::DenyImpact
        );
    }

    #[test]
    fn test_default_role_count() {
        let gate = ImpactGate::new();
        assert_eq!(gate.role_ids().len(), 8);
    }
}
