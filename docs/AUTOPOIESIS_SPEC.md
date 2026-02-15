# Autopoiesis Specification -- n8n-rs Self-Orchestrating Living Systems

> **Date**: 2026-02-15
> **Scope**: n8n-rs as an autonomous, self-orchestrating workflow engine.
> **Dependency**: ladybug-rs (cognitive substrate). Ada-rs is NOT required.
> **Core Principle**: n8n-rs workflows are autopoietic systems -- they produce
>   the routing decisions that maintain the topology that produces the routing
>   decisions. MUL from ladybug-rs acts as the immune system that prevents
>   degenerate self-modification. This spec is entirely ada-rs-agnostic.

---

## 1. Autopoiesis: Self-Producing Workflows

### 1.1 Theoretical Foundation

Maturana & Varela (1972) defined an autopoietic system as one that
**produces the components that constitute it**, thereby maintaining its own
organization as a unity in the space in which it exists.

A biological cell:
- Produces proteins (components) that maintain the cell membrane (boundary)
- The membrane contains the machinery that produces the proteins
- The system is organizationally closed: it produces itself

An n8n-rs autopoietic workflow:
- Produces **routing decisions** (components) that maintain the **workflow
  topology** (boundary)
- The topology contains the **nodes** that produce the routing decisions
- The system is organizationally closed: it produces itself

```
BIOLOGICAL CELL:
  membrane <-- proteins <-- DNA <-- (protected by membrane)
  ^____________________________________________________|

N8N-RS AUTOPOIETIC WORKFLOW:
  topology <-- routing decisions <-- Q-values <-- (computed within topology)
  ^____________________________________________________|

CORRESPONDENCE:
  Cell membrane        = Workflow topology (nodes + edges)
  Protein synthesis    = Routing decisions (which downstream node next?)
  DNA                  = Crystallized route knowledge (SPO triples)
  Gene expression      = Q-value updates from observed outcomes
  Immune system        = MUL gating (prevents degenerate modification)
  Cell division        = Workflow replication (spawn specialized child)
```

### 1.2 When Is a Workflow Autopoietic?

A workflow becomes autopoietic when ALL of these conditions hold:

1. **Nodes compute Q-values** that determine routing between downstream paths.
2. **Routing outcomes are observed** and evaluated for quality.
3. **Outcomes are crystallized** as persistent knowledge (SPO triples in
   BindSpace).
4. **Crystallized knowledge updates Q-values**, changing future routing.
5. **Changed routing adapts the topology** (prune unused edges, grow
   useful ones).
6. **Adapted topology produces better routing**, completing the cycle.

A static workflow (fixed connections, no Q-values) is NOT autopoietic.
It is a machine, not a living system.

### 1.3 What n8n-rs Provides vs. What ladybug-rs Provides

```
n8n-rs OWNS:
  - Workflow topology (nodes, edges, DAG structure)
  - Q-value routing logic (NodeQValues, select_route)
  - FreeWillPipeline (gates self-modification proposals)
  - ImpactGate (RBAC-based operation gating)
  - AutopoieticWorkflow struct and lifecycle management
  - Arrow Flight API for workflow operations
  - GEL frame compilation (workflow steps to execution frames)

ladybug-rs PROVIDES:
  - MetaUncertaintyLayer (MUL) -- the 10-layer metacognitive stack
  - MulSnapshot -- serializable MUL state consumed by n8n-rs
  - FreeWillModifier -- multiplicative confidence factor
  - BindSpace -- shared blackboard for stigmergic coupling
  - CogRecord -- container format for storing node state
  - NARS TruthValue -- evidence representation
  - CogPacket -- binary wire protocol for inter-system communication
  - GEL runtime -- frame execution engine

ada-rs is NOT in this dependency chain.
n8n-rs -> ladybug-rs is the only required dependency.
```

---

## 2. Q-Value Routing

### 2.1 NodeQValues: Learned Route Preferences

Each workflow node maintains 16 Q-values (one per possible downstream slot).
These are NOT hardcoded routing rules. They are learned from the outcomes
of previous routing decisions via temporal-difference Q-learning.

```rust
/// Workflow node Q-values -- learned routing preferences.
///
/// Each node has 16 Q-values stored in CogRecord W32-W39
/// (8 words x 2 f32 per word = 16 values).
/// Q[i] = "expected quality of routing to downstream slot i"
pub struct NodeQValues {
    /// 16 Q-values, one per downstream slot.
    /// Unconnected slots are set to -infinity (never selected).
    values: [f32; 16],
}
```

### 2.2 Epsilon-Greedy Selection with MUL-Informed Epsilon

Route selection uses epsilon-greedy exploration. The epsilon (exploration
rate) is dynamically modulated by the MUL FreeWillModifier:

```rust
impl NodeQValues {
    /// Select downstream route using epsilon-greedy with MUL modulation.
    ///
    /// Epsilon = base_epsilon * free_will_modifier.value()
    ///
    /// When modifier is HIGH (confident, in Flow, DK=Plateau):
    ///   epsilon is high -> explore more (safe to try new routes)
    /// When modifier is LOW (uncertain, MountStupid, Anxiety):
    ///   epsilon is low -> exploit known routes (not safe to explore)
    pub fn select_route(&self, mul: &MulSnapshot, mode: StrategicMode) -> usize {
        let base_epsilon = match mode {
            StrategicMode::EpiphanyHunting => 0.40,
            StrategicMode::Exploration     => 0.20,
            StrategicMode::Execution       => 0.05,
            _                              => 0.10,
        };
        let epsilon = base_epsilon * mul.free_will_modifier.value();

        if rand_f32() < epsilon {
            // Explore: random valid downstream
            let valid: Vec<usize> = self.values.iter()
                .enumerate()
                .filter(|(_, v)| **v > f32::NEG_INFINITY)
                .map(|(i, _)| i)
                .collect();
            valid[rand_usize() % valid.len()]
        } else {
            // Exploit: best known route
            self.best_route()
        }
    }

    /// Best downstream route by Q-value (pure exploitation).
    pub fn best_route(&self) -> usize {
        self.values.iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0)
    }
}
```

### 2.3 Q-Update from Outcome

After a routing decision executes and produces an observable outcome,
the Q-value for the route taken is updated:

```
Q(node, route) += alpha * (outcome_quality - Q(node, route))
```

The learning rate `alpha` is MUL-bounded:

```rust
impl NodeQValues {
    /// Update Q-value after observing a routing outcome.
    ///
    /// Learning rate = base_alpha * free_will_modifier
    ///   High modifier -> learn faster (confident in observations)
    ///   Low modifier  -> learn slower (observations may be unreliable)
    pub fn update(
        &mut self,
        route_taken: usize,
        outcome_quality: f32,  // 0.0 = bad, 1.0 = good
        mul: &MulSnapshot,
    ) {
        let alpha = 0.1 * mul.free_will_modifier.value();
        let current = self.values[route_taken];
        self.values[route_taken] = current + alpha * (outcome_quality - current);
    }
}
```

### 2.4 Route Crystallization

When Q-values converge (variance across recent updates drops below a
threshold), the route becomes "known" -- it is crystallized as a persistent
SPO triple in BindSpace:

```
Crystallization criterion:
  variance(last_N_Q_updates) < CRYSTALLIZE_THRESHOLD (default: 0.01)
  AND mul.gate_open == true
  AND mul.dk_position >= SlopeOfEnlightenment

When crystallized:
  SPO(node_B, routed_to_C, outcome_quality=0.87)
  Stored in BindSpace prefix 0x0E (blackboard)
  Permanent -- survives workflow restarts
  Future routing can start from crystallized knowledge (warm start)
```

---

## 3. MUL as Immune System

The Meta-Uncertainty Layer (MUL) from ladybug-rs serves as the autopoietic
workflow's immune system. It prevents five classes of degenerate
self-modification.

### 3.1 MUL Prevents Degenerate Topology (Positive Feedback Loops)

```
THREAT: Positive feedback loop
  Node A routes to B, B routes to A, loop amplifies.
  Q-values reinforce the loop because each iteration "succeeds."

DEFENSE: FalseFlowDetector (MUL Layer 5)
  Detects: coherence > 0.7 AND novelty < 0.2 AND |coherence_delta| < 0.05
  Severity escalation: Caution -> Warning -> Severe
  At Severe: force disruption
    - Inject random route choice (override Q-values)
    - Break the loop
    - Reset coherence/novelty window
```

### 3.2 MUL Gates Self-Modification (No Changes When MountStupid)

```
THREAT: Overconfident topology modification
  DK=MountStupid: workflow "thinks" it knows best routing but has
  insufficient experience to justify restructuring.

DEFENSE: MulGate (Layer 7) blocks all self-modification
  DKPosition::MountStupid -> gate CLOSED
  When gate is closed:
    - Level 1 (Self-Tuning): Q-value updates allowed at reduced rate
    - Level 2 (Self-Pruning): ALL topology changes BLOCKED
    - Level 3 (Self-Replicating): ALL spawning BLOCKED
  Workflow must accumulate experience (increase sample_count in DKDetector)
  until DK progresses to at least SlopeOfEnlightenment.
```

### 3.3 MUL Limits Exploration Rate Under High Allostatic Load

```
THREAT: Exploration while stressed
  High allostatic_load means the system is already deviating from
  its identity set-point. Adding exploration increases deviation.

DEFENSE: FreeWillModifier reduction
  allostatic_load > 0.7 -> HomeostasisState = Anxiety
  Anxiety -> flow_factor = 0.4
  free_will_modifier = dk * trust * complexity * flow_factor
  With flow_factor = 0.4, modifier drops significantly
  -> epsilon drops -> exploration rate drops -> stick to known routes
```

### 3.4 FreeWillModifier Modulates Thresholds in FreeWillPipeline

The FreeWillModifier is the core bridge between MUL (ladybug-rs) and the
gating logic (n8n-rs). It dynamically adjusts the evidence thresholds
that proposals must meet:

```rust
// In n8n-rs FreeWillPipeline:
pub fn evaluate_with_mul(
    &mut self,
    proposal: &ModificationProposal,
    mul_snapshot: &MulSnapshot,
) -> ProposalResult {
    let modifier = mul_snapshot.free_will_modifier.value();

    // Dynamic threshold adjustment:
    //   effective_min_evidence = limits.min_evidence / modifier
    //
    //   When modifier < 1.0 (uncertain): thresholds are HARDER to pass
    //     modifier = 0.3 -> effective = 0.85/0.3 = 2.83 -> impossible -> DENY
    //   When modifier = 1.0 (confident): thresholds are at BASE level
    //     modifier = 1.0 -> effective = 0.85/1.0 = 0.85 -> normal
    //   When modifier > 0.85 (high confidence):
    //     modifier = 0.95 -> effective = 0.85/0.95 = 0.89 -> slightly relaxed

    let effective_min_evidence = if modifier > 0.01 {
        (self.limits.min_evidence / modifier).min(1.0)
    } else {
        1.0  // modifier near zero -> impossible threshold -> always deny
    };

    let effective_min_satisfaction = if modifier > 0.01 {
        (self.limits.min_satisfaction / modifier).min(1.0)
    } else {
        1.0
    };

    // Rate limits also scale with modifier:
    let effective_max_hour =
        (self.limits.max_per_hour as f32 * modifier).max(1.0) as u32;

    // ... rest of evaluate() uses effective thresholds
}
```

### 3.5 MUL Snapshot Wire Format

MUL state is packed into CogRecord metadata words W64-W65 so that any
consumer can read it without importing the full MUL engine:

```
W64 (MUL State):
  [63:56] trust_texture       (3 bits used, 5 reserved)
  [55:54] dk_position         (2 bits: MountStupid/Valley/Slope/Plateau)
  [53:52] homeostasis_state   (2 bits: Flow/Anxiety/Boredom/Apathy)
  [51:50] false_flow_severity (2 bits: None/Caution/Warning/Severe)
  [49]    gate_open           (1 bit)
  [48:0]  reserved            (49 bits)

W65 (MUL Values):
  [63:32] free_will_modifier  (f32 as bits)
  [31:0]  allostatic_load     (f32 as bits)
```

---

## 4. Smart Gating: Code Change Points in n8n-rs

### 4.1 FreeWillPipeline (free_will.rs)

**File**: `n8n-rs/n8n-rust/crates/n8n-contract/src/free_will.rs`

**Current signature** (line 223):
```rust
pub fn evaluate(&mut self, proposal: &ModificationProposal) -> ProposalResult
```

**New signature** (MUL-aware):
```rust
pub fn evaluate_with_mul(
    &mut self,
    proposal: &ModificationProposal,
    mul_snapshot: &MulSnapshot,
) -> ProposalResult
```

**What changes**:
- Evidence threshold becomes `min_evidence / modifier` (line 257-267)
- Satisfaction threshold becomes `min_satisfaction / modifier` (line 270-283)
- Rate limits scale: `max_per_hour * modifier` (line 286-305)
- Backward compatibility: `evaluate()` calls `evaluate_with_mul()` with a
  default MulSnapshot where modifier = 1.0

### 4.2 ImpactGate (impact_gate.rs)

**File**: `n8n-rs/n8n-rust/crates/n8n-contract/src/impact_gate.rs`

**Current signature** (line 141):
```rust
pub fn check(
    &self,
    role_id: &str,
    impact: ImpactLevel,
    truth_value: &TruthValue,
    stack_satisfaction: &[f32],
) -> GateDecision
```

**New signature** (MUL-aware):
```rust
pub fn check_with_mul(
    &self,
    role_id: &str,
    impact: ImpactLevel,
    truth_value: &TruthValue,
    stack_satisfaction: &[f32],
    mul_snapshot: &MulSnapshot,
) -> GateDecision
```

**What changes**:
- Critical evidence threshold becomes `0.9 / modifier`
- MUL gate closed -> block all operations above Observe
- MUL DKPosition::MountStupid -> force DenyEvidence for Significant+

### 4.3 Exact Structs for Wiring

```rust
/// MUL snapshot consumed by n8n-rs from ladybug-rs.
/// n8n-rs does NOT own this struct -- it imports from ladybug-rs.
pub struct MulSnapshot {
    pub trust_texture: TrustTexture,
    pub dk_position: DKPosition,
    pub homeostasis_state: HomeostasisState,
    pub false_flow_severity: FalseFlowSeverity,
    pub free_will_modifier: FreeWillModifier,
    pub gate_open: bool,
    pub gate_block_reason: Option<GateBlockReason>,
    pub allostatic_load: f32,
}

/// Modification limits -- owned by n8n-rs, configurable via YAML.
/// Already exists in free_will.rs as ModificationLimits.
pub struct ModificationLimits {
    pub max_scope: u32,
    pub allowed_types: Vec<ModificationType>,
    pub min_evidence: f32,        // base threshold, divided by modifier
    pub min_satisfaction: f32,    // base threshold, divided by modifier
    pub allow_irreversible: bool,
    pub max_per_hour: u32,        // base rate, multiplied by modifier
    pub max_per_day: u32,         // base rate, multiplied by modifier
}

/// Gate decision -- already exists in impact_gate.rs.
pub enum GateDecision {
    Allow,
    DenyImpact,
    DenyEvidence,
    DenyBudget,
    DenySatisfaction,
}
```

---

## 5. Three Levels of Self-Orchestration

### Level 1: Self-Tuning (Q-Value Routing Within Fixed Topology)

```
What changes:  Q-values at each node updated via TD-learning
What stays:    Node set, edge set, node implementations
MUL requires:  gate_open = true, modifier > 0.3
DK minimum:    Any (even MountStupid can learn Q-values, slowly)

Mechanism:
  1. Node executes, selects downstream route via Q-values + epsilon-greedy
  2. Downstream node executes, outcome observed
  3. Q-value updated: Q[route] += alpha * (outcome - Q[route])
  4. alpha = 0.1 * modifier (bounded learning rate)

Example:
  Workflow node B has two downstream paths: C (tactical analysis) and
  D (strategic overview). Over 50 executions, B learns that routing to C
  first produces better final outcomes. Q[C] = 0.82, Q[D] = 0.45.
  B now routes to C 95% of the time (exploitation) and to D 5%
  (exploration, in case conditions change).
```

### Level 2: Self-Pruning/Growing (Topology Modification)

```
What changes:  Edges added/removed based on Q-value convergence history
What stays:    Node set (implementations unchanged)
MUL requires:  gate_open = true, modifier > 0.5, DK >= Slope

Pruning rule:
  IF Q[downstream] < PRUNE_THRESHOLD for N consecutive cycles
  AND mul.gate_open == true
  AND mul.free_will_modifier.value() > 0.5
  THEN mark edge inactive (reversible -- never hard-delete)

Growing rule:
  IF crystallized SPO shows: node_X frequently produces good outcomes
     for this type of input AND no direct edge exists to node_X
  AND FreeWillPipeline.evaluate_with_mul() approves (ImpactLevel::Moderate)
  THEN propose new edge via FreeWillPipeline

All topology changes go through FreeWillPipeline:
  ModificationProposal {
      modification_type: ModifyRouting,
      source_layer: 4,  // L5 strategic
      evidence: TruthValue { frequency: 0.9, confidence: 0.85 },
      scope: 1,  // single edge change
      reversible: true,
      justification: "Q-convergence indicates route B->D is unused",
  }
```

### Level 3: Self-Replication (Spawning New Workflow Variants)

```
What changes:  Entire new workflow instances created from parent
What stays:    Original workflow unchanged (parent persists)
MUL requires:  gate_open = true, modifier > 0.7, DK = Plateau

Replication protocol:
  1. Workflow detects consistent routing split: "For input type X, always
     route path A. For input type Y, always route path B."
  2. Crystallized SPO confirms: pattern is stable over N cycles
  3. MUL validates: DK=Plateau, modifier > 0.7 (confident in assessment)
  4. FreeWillPipeline approves: ImpactLevel::Significant (new workflow)
  5. Clone parent topology, specialize Q-values for type X
  6. Parent registers child workflow for type-X routing
  7. Parent retains type-Y routing (or spawns second child)

This is "cell division" -- one workflow becomes two specialized ones.
Each child inherits:
  - Parent's topology (initial edge set)
  - Parent's crystallized knowledge (warm start Q-values)
  - Independent MUL state (starts at MountStupid for its specialized domain)
  - Independent lifecycle (own birth timestamp, own maturity progression)
```

---

## 6. Workflow Lifecycle: Birth Through Senescence

### Birth

```
1. Workflow defined via YAML or programmatic API
2. Each node initialized with uniform Q-values: Q[i] = 0.0 for all i
3. Workflow-level MUL initialized:
   - DK = MountStupid (no experience)
   - trust = Fuzzy (unknown reliability)
   - homeostasis = Flow (no challenge/skill mismatch yet)
4. SelfOrchestrationLevel = SelfTuning (Level 1 only)
5. topology_generation = 0
```

### Infancy (Level 1 Active)

```
6. Workflow processes inputs, makes routing decisions with high epsilon
7. Outcomes observed, Q-values updated with MUL-bounded learning rate
8. Crystallization occurs for strongly convergent routes
9. MUL progression: DK moves MountStupid -> Valley -> Slope
10. Workflow develops routing PREFERENCES (Q-values diverge from uniform)
11. Duration: typically 20-100 execution cycles
```

### Adolescence (Level 2 Active)

```
12. MUL: DK reaches Slope, modifier > 0.5
13. SelfOrchestrationLevel promoted to SelfPruning
14. Unused edges identified: Q[route] < 0.1 for 20+ consecutive cycles
15. FreeWillPipeline approves pruning of unused edges
16. SPO reveals shortcuts: "when pattern X, route directly to node Y"
17. FreeWillPipeline approves adding shortcut edges
18. Topology ADAPTS to observed execution patterns
19. topology_generation increments with each structural change
20. Duration: typically 100-500 execution cycles
```

### Maturity (Level 3 Active)

```
21. MUL: DK reaches Plateau, modifier > 0.7
22. SelfOrchestrationLevel promoted to SelfReplicating
23. Workflow identifies distinct input sub-patterns with consistent
    routing splits
24. FreeWillPipeline approves spawning specialized child workflows
25. Parent delegates sub-pattern routing to children
26. Parent + children = ecosystem of cooperating workflows
27. Structural coupling between parent and children via BindSpace
28. Duration: indefinite (mature workflows can operate for thousands of cycles)
```

### Senescence

```
29. Workflow's domain becomes obsolete (input frequency drops)
30. MUL: homeostasis -> Boredom -> Apathy (challenge drops below skill)
31. Q-values decay: no reinforcement -> values drift toward uniform
32. allostatic_load rises (deviation from set-point accumulates)
33. Workflow enters HOLD state (execution paused)
34. Crystallized knowledge persists in BindSpace (permanent)
35. If revived: warm-start from crystallized knowledge (not from zero)
36. If permanently unused: archived (crystals remain for future workflows)
```

---

## 7. Structural Coupling via Blackboard

### 7.1 Stigmergy: Indirect Communication

Workflows co-evolve NOT through direct message passing but through
**stigmergy** -- indirect communication through modification of a shared
environment. This environment is the BindSpace blackboard in ladybug-rs.

```
WORKFLOW A writes to BindSpace prefix 0x0E (blackboard):
  key: 0x0E:workflow_A:output_slot_3
  value: fingerprint of A's routing outcome

WORKFLOW B reads from BindSpace prefix 0x0E:
  resonance_search(B's current input fingerprint, prefix=0x0E)
  -> finds A's output fingerprint at Hamming distance < threshold
  -> B incorporates A's output into its routing decision

Neither workflow knows about the other directly.
They co-evolve through the shared blackboard.
This is Maturana's "structural coupling" implemented as stigmergy.
```

### 7.2 The Blackboard Lives in ladybug-rs

```
ladybug-rs BindSpace:
  prefix 0x00-0x0D: cognitive domains (perception, memory, etc.)
  prefix 0x0E:       BLACKBOARD (shared workflow state)
  prefix 0x0F:       n8n-rs workflow domain

The blackboard at 0x0E is readable and writable by ANY workflow.
This is NOT ada-rs territory. Ada-rs may also read/write the
blackboard if present, but the blackboard exists and functions
entirely within ladybug-rs.

Operations:
  BindSpace::write(0x0E, slot, fingerprint)    -- write to blackboard
  BindSpace::read(0x0E, slot)                  -- read from blackboard
  BindSpace::resonance_search(fp, prefix=0x0E) -- find similar entries
```

### 7.3 Coupling Example

```
WORKFLOW A (data analysis):
  Execution cycle 47:
    - Processes dataset D
    - Routes to statistical analysis node (Q[stat] = 0.88)
    - Produces insight fingerprint: fp_insight_47
    - Writes to blackboard: BindSpace::write(0x0E, A_47, fp_insight_47)

WORKFLOW B (report generation):
  Execution cycle 200:
    - Receives report request
    - Queries blackboard: resonance_search(report_topic_fp, 0x0E)
    - Finds fp_insight_47 at Hamming distance 2400 (< threshold 3000)
    - Incorporates insight into report routing
    - Routes to "use statistical findings" node instead of "generate from scratch"

Result: B's behavior changed because A wrote to the blackboard.
Neither workflow directly messaged the other. Both maintain
independent autopoiesis (own Q-values, own MUL state, own topology).
```

---

## 8. Self-Modification Protocol

### 8.1 The Six-Phase Protocol

Every self-modification in an autopoietic workflow follows this protocol:

```
1. PROPOSE
   - Source: Q-value convergence, crystallization trigger, or
     false flow disruption
   - Creates ModificationProposal with type, parameters, evidence
   - Goes to FreeWillPipeline

2. MUL GATE
   - FreeWillPipeline.evaluate_with_mul() checks:
     a. Modification type is in allowed_types
     b. Scope <= max_scope
     c. Reversibility check (irreversible blocked unless allowed)
     d. Evidence >= effective_min_evidence (= base / modifier)
     e. Satisfaction >= effective_min_satisfaction (= base / modifier)
     f. Rate limits: count < effective_max_per_hour (= base * modifier)
     g. RBAC gate: ImpactGate.check_with_mul()
   - If ANY check fails: DENY (return to observe, gather more evidence)

3. APPLY
   - Approved modification executed:
     - Level 1: Q-value update (immediate)
     - Level 2: Edge add/remove (topology_generation++)
     - Level 3: Workflow clone + specialize (spawn child)
   - Change recorded in execution log

4. OBSERVE
   - Next N execution cycles measured with the modification in place
   - Outcome quality tracked per cycle
   - Comparison: post-modification outcomes vs pre-modification baseline

5. EVALUATE
   - Statistical assessment: did the modification improve outcomes?
   - Brier score: was the predicted improvement accurate?
   - DK update: observe() called on DKDetector with prediction vs outcome
   - Trust update: trust_delta() from PostActionLearning

6. CRYSTALLIZE
   - If modification improved outcomes AND variance is low:
     SPO(workflow, modification_type, outcome_quality) stored permanently
   - If modification worsened outcomes AND was reversible:
     Rollback: undo the modification, restore previous state
   - Crystallized modifications inform future proposals (warm start)
```

### 8.2 FreeWillPipeline's 7-Step evaluate() Pipeline

The existing FreeWillPipeline in `free_will.rs` implements a 7-step
evaluation sequence. With MUL integration, each step's thresholds become
dynamic:

```
Step 1: Type check
  Is proposal.modification_type in limits.allowed_types?
  (Unchanged -- YAML-configured safety boundary)

Step 2: Scope check
  Is proposal.scope <= limits.max_scope?
  (Unchanged -- YAML-configured maximum)

Step 3: Reversibility check
  Is proposal.reversible OR limits.allow_irreversible?
  (Unchanged -- YAML-configured safety flag)

Step 4: Evidence check  ** MUL-MODULATED **
  evidence = frequency * confidence
  effective_threshold = limits.min_evidence / modifier
  Is evidence >= effective_threshold?

Step 5: Satisfaction check  ** MUL-MODULATED **
  avg_satisfaction = mean(proposal.satisfaction)
  effective_threshold = limits.min_satisfaction / modifier
  Is avg_satisfaction >= effective_threshold?

Step 6: Rate limit check  ** MUL-MODULATED **
  effective_max_hour = limits.max_per_hour * modifier
  effective_max_day = limits.max_per_day * modifier
  Are counters within effective limits?

Step 7: RBAC gate
  ImpactGate.check_with_mul(role, impact, evidence, satisfaction, mul)
  If MUL gate is closed: deny everything above Observe
```

### 8.3 ModificationProposal Struct

```rust
/// A self-modification proposal from the autopoietic workflow.
/// Already exists in free_will.rs.
pub struct ModificationProposal {
    /// What kind of modification is proposed.
    pub modification_type: ModificationType,

    /// Source layer that originated the proposal (0-9).
    pub source_layer: u8,

    /// NARS truth value supporting the modification.
    pub evidence: TruthValue,

    /// Current 10-layer satisfaction snapshot.
    pub satisfaction: Vec<f32>,

    /// Scope of the modification (number of affected elements).
    pub scope: u32,

    /// Reversibility flag.
    pub reversible: bool,

    /// Human-readable justification.
    pub justification: String,
}
```

---

## 9. GEL Integration

### 9.1 Workflow Steps as GEL Frames

Each workflow execution step compiles to a GEL (Generic Execution Language)
frame. GEL frames are the universal execution unit in the ladybug-rs
runtime.

```
Workflow Step          ->  GEL Frame (FireflyFrame)
  step_id              ->  frame_id
  step_type            ->  opcode (EXECUTE, RESONATE, DELEGATE, etc.)
  input (JSON)         ->  frame payload (serialized as Container)
  output (JSON)        ->  frame result (serialized as Container)
  execution_id         ->  lane_id (Redis execution lane)
```

### 9.2 Frame Compilation

```rust
/// Compile a UnifiedStep into a GEL FireflyFrame.
///
/// This allows n8n-rs workflow steps to execute on the ladybug-rs
/// GEL runtime, benefiting from:
/// - Redis-backed execution lanes with persistence
/// - Fan-out/fan-in for parallel step execution
/// - Frame-level retry and error recovery
/// - Execution replay from any frame checkpoint
pub fn compile_step_to_frame(
    step: &UnifiedStep,
    execution_id: &str,
) -> FireflyFrame {
    let opcode = match step.step_type.as_str() {
        s if s.starts_with("n8n.")  => wire_ops::EXECUTE,
        s if s.starts_with("crew.") => wire_ops::DELEGATE,
        s if s.starts_with("lb.")   => wire_ops::RESONATE,
        _                           => wire_ops::EXECUTE,
    };

    FireflyFrame {
        frame_id: step.step_id.clone(),
        lane_id: execution_id.to_string(),
        opcode,
        payload: serialize_to_container(&step.input),
        sequence: step.sequence as u32,
        status: FrameStatus::Pending,
    }
}
```

### 9.3 Fan-Out / Fan-In for Parallel Steps

When a workflow node has multiple downstream connections, execution fans out
into parallel GEL frames. Results are collected via fan-in before the next
sequential step:

```
Workflow:
  A -> [B, C, D] -> E  (B, C, D execute in parallel)

GEL Frame Compilation:
  Lane: execution_123
    Frame A (seq=0) -> EXECUTE
    Frame B (seq=1, fan_group=1) -> EXECUTE  \
    Frame C (seq=1, fan_group=1) -> EXECUTE   } parallel
    Frame D (seq=1, fan_group=1) -> EXECUTE  /
    Frame E (seq=2, depends_on=fan_group_1) -> EXECUTE

Redis Execution:
  1. Frame A executes, writes result to lane
  2. Frames B, C, D dispatched simultaneously
  3. Fan-in barrier: wait for all fan_group=1 frames to complete
  4. Frame E reads combined results, executes
```

### 9.4 Step Results Flow Back

GEL frame results are unpacked back into the n8n-rs type system:

```rust
/// Unpack a completed GEL frame result back into a UnifiedStep.
pub fn unpack_frame_result(
    frame: &FireflyFrame,
    step: &mut UnifiedStep,
) {
    match frame.status {
        FrameStatus::Completed => {
            let output = deserialize_from_container(&frame.result);
            step.mark_completed(output);
        }
        FrameStatus::Failed => {
            step.mark_failed(frame.error_message.clone().unwrap_or_default());
        }
        _ => {} // still pending or running
    }
}
```

---

## 10. Autopoietic Workflow Struct

```rust
/// An autopoietic workflow -- self-modifying with MUL gating.
///
/// This is the central struct for n8n-rs self-orchestration.
/// It wraps the standard workflow topology with Q-value routing,
/// MUL state, lifecycle tracking, and lineage management.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutopoieticWorkflow {
    /// Workflow identity.
    pub id: u64,
    pub name: String,

    /// Nodes (each maps to a CogRecord via W0-W127).
    pub nodes: Vec<WorkflowStrategicNode>,

    /// Current topology generation (increments on structural change).
    pub topology_generation: u64,

    /// Workflow-level MUL snapshot (aggregated from node MULs).
    pub mul_snapshot: MulSnapshot,

    /// Current self-orchestration level.
    pub level: SelfOrchestrationLevel,

    /// Children spawned from this workflow (child workflow IDs).
    pub children: Vec<u64>,

    /// Parent workflow ID (None for root workflows).
    pub parent: Option<u64>,

    /// Total execution cycles completed.
    pub total_cycles: u64,

    /// Birth timestamp.
    pub born_at: u64,

    /// Current lifecycle phase.
    pub phase: LifecyclePhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelfOrchestrationLevel {
    /// Fixed routing, Q-values updated.
    SelfTuning,
    /// Edges can be pruned/added.
    SelfPruning,
    /// New workflow instances can be spawned.
    SelfReplicating,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LifecyclePhase {
    Birth,
    Infancy,
    Adolescence,
    Maturity,
    Senescence,
}

/// A workflow node with strategic capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStrategicNode {
    /// Node identifier.
    pub node_id: String,

    /// Q-values for routing (from CogRecord W32-W39).
    pub q_values: NodeQValues,

    /// Node-specific MUL snapshot (from CogRecord W64-W65).
    pub mul_snapshot: MulSnapshot,

    /// Node type (trigger, action, condition, etc.).
    pub node_type: String,

    /// Downstream connections (node indices in parent workflow).
    pub downstream: Vec<usize>,

    /// Execution count for this node.
    pub execution_count: u64,
}
```

---

## 11. The Autopoietic Loop (Complete Cycle Diagram)

```
+----------------------------------------------------------------------+
|                     THE AUTOPOIETIC LOOP                              |
|                                                                       |
|    +----------------------------------------------------------+      |
|    |  1. WORKFLOW TOPOLOGY (the "membrane")                    |      |
|    |     Nodes + edges + Q-value routing rules                 |      |
|    |     +---+    +---+    +---+                               |      |
|    |     | A |--->| B |--->| C |                               |      |
|    |     +---+    +-+-+    +---+                               |      |
|    |                |    +---+                                  |      |
|    |                +--->| D |                                  |      |
|    |                     +---+                                  |      |
|    +---------------------------+------------------------------+      |
|                                |                                      |
|    2. EXECUTION (the "metabolism")                                    |
|       Each node runs with its own thinking style                     |
|       MUL evaluates at each node boundary                            |
|                                |                                      |
|                                v                                      |
|    3. ROUTING DECISION (the "protein synthesis")                     |
|       Node B: route to C or D?                                       |
|       Q-values + MUL epsilon -> select_route()                       |
|       The routing IS the "product" of the system                     |
|                                |                                      |
|                                v                                      |
|    4. OUTCOME OBSERVATION                                            |
|       Downstream node (C or D) produces result                       |
|       Result quality measured (0.0 to 1.0)                           |
|                                |                                      |
|                                v                                      |
|    5. Q-VALUE UPDATE (the "gene expression")                         |
|       Q[route] += alpha * (outcome - Q[route])                       |
|       alpha = 0.1 * free_will_modifier                               |
|       Node B now PREFERS routing to C (if C outcome was better)      |
|                                |                                      |
|                                v                                      |
|    6. CRYSTALLIZATION (the "DNA replication")                        |
|       If Q-values converged: store SPO in BindSpace                  |
|       SPO(node_B, routed_to_C, quality=0.87) -> permanent            |
|                                |                                      |
|                                v                                      |
|    7. TOPOLOGY EVOLUTION (the "cell growth/division")                |
|       If Q[D] near zero for N cycles -> prune edge B->D              |
|       If new pattern discovered -> add edge B->E                     |
|       MUL gates: only modify if gate_open AND modifier > threshold   |
|                                |                                      |
|                                +-------> BACK TO 1 (loop)            |
|                                                                       |
|    The workflow produces routing decisions that maintain               |
|    and adapt the topology that produces the routing decisions.         |
|    THIS IS AUTOPOIESIS.                                               |
+----------------------------------------------------------------------+
```

---

## 12. Homeostatic Regulation: The Five Threat Classes

### Threat 1: Positive Feedback Loops

```
Detection:  FalseFlowDetector (MUL Layer 5)
            coherence > 0.7, novelty < 0.2, |delta| < 0.05
Severity:   Caution -> Warning -> Severe (escalates over ticks)
Response:   At Severe: inject random route (break loop), shift to
            Exploration mode (increase epsilon)
```

### Threat 2: Topology Degeneration

```
Detection:  CognitiveHomeostasis (MUL Layer 6)
            challenge << skill -> Boredom (all routes converged)
Response:   HomeostasisAction::Challenge
            StrategicMode shifts to Exploration
            epsilon increases -> routes diversify
```

### Threat 3: Overconfident Restructuring

```
Detection:  DKDetector (MUL Layer 2)
            gap between felt_competence and demonstrated_competence > 0.2
            AND sample_count < 10
Response:   DK = MountStupid -> Gate CLOSES
            Level 2 and Level 3 operations BLOCKED
            Only Level 1 (Q-value updates at reduced rate) allowed
```

### Threat 4: Rapid Uncontrolled Mutation

```
Detection:  FreeWillPipeline rate limiters
            counters.0 >= effective_max_per_hour
            counters.1 >= effective_max_per_day
Response:   DenyBudget -- no more modifications until next reset
            ModificationLimits.max_per_hour / max_per_day enforce bounds
```

### Threat 5: Identity Drift

```
Detection:  CompassFunction (MUL Layer 9) identity test
            Modified topology fingerprint diverges from original purpose
Response:   CompassDecision::SurfaceToMeta
            Requires higher-level review before further modification
            If crewai-rust is present: PersonaProfile.self_modify bounds
```

---

## 13. Ada-rs Independence Guarantee

This specification is designed to function completely without ada-rs:

```
WITHOUT ada-rs:
  - Q-value routing:         WORKS (n8n-rs native)
  - MUL gating:              WORKS (ladybug-rs provides MulSnapshot)
  - FreeWillPipeline:        WORKS (n8n-rs native, consumes MulSnapshot)
  - ImpactGate:              WORKS (n8n-rs native)
  - Self-tuning:             WORKS (Q-value updates need only MulSnapshot)
  - Self-pruning/growing:    WORKS (topology changes gated by FreeWillPipeline)
  - Self-replication:        WORKS (workflow spawn gated by FreeWillPipeline)
  - BindSpace blackboard:    WORKS (ladybug-rs native)
  - Structural coupling:     WORKS (blackboard stigmergy)
  - GEL frame execution:     WORKS (ladybug-rs native)

WITH ada-rs (OPTIONAL ENHANCEMENTS):
  - IdentitySeed:            Compass identity test uses frozen values
  - SovereigntyProfile:      ConsentLevel gates Critical changes
  - PresenceMode:            Awareness context for routing decisions
  - SelfModel:               Richer identity drift detection

ada-rs ADDS consciousness awareness but is NOT required for autopoiesis.
The workflow is alive (self-producing) with only n8n-rs + ladybug-rs.
```

---

## 14. Arrow Flight API for Autopoietic Operations

```
DoAction("workflow.create_autopoietic")
  Input:  { name, nodes: [{type, params, connections}],
            level: SelfOrchestrationLevel }
  Output: { workflow_id, node_count, topology_generation: 0 }

DoAction("workflow.execute_step")
  Input:  { workflow_id, input_fingerprint }
  Output: { route_taken, node_executed, output_fingerprint, mul_snapshot }

DoAction("workflow.get_q_values")
  Input:  { workflow_id, node_index }
  Output: { q_values: [f32; 16], best_route, convergence_variance }

DoAction("workflow.propose_topology_change")
  Input:  { workflow_id,
            change_type: "prune" | "grow" | "replicate",
            target_edge_or_node,
            evidence: TruthValue }
  Output: { approved, decision: GateDecision, denial_reason? }

DoAction("workflow.get_topology")
  Input:  { workflow_id }
  Output: { nodes, edges, q_values_per_node, mul_per_node,
            topology_generation, lifecycle_phase }

DoAction("workflow.get_lineage")
  Input:  { workflow_id }
  Output: { parent?, children, topology_generation, born_at,
            total_cycles, current_phase }

DoAction("workflow.q_update")
  Input:  { workflow_id, node_index, route_taken,
            outcome_quality: f32 }
  Output: { new_q_value, learning_rate_used, crystallized: bool }

DoAction("workflow.crystallize_route")
  Input:  { workflow_id, node_index, route_index }
  Output: { spo_key, bindspace_address, q_variance }
```

---

## 15. Configuration (YAML)

```yaml
# autopoietic_workflow.yaml
workflow:
  name: "research_analysis"
  level: "self_tuning"  # starts at Level 1

  nodes:
    - id: "trigger"
      type: "webhook"
      downstream: [1]
    - id: "analyze"
      type: "n8n.set"
      downstream: [2, 3]
    - id: "statistical"
      type: "crew.agent"
      downstream: [4]
    - id: "qualitative"
      type: "crew.agent"
      downstream: [4]
    - id: "synthesize"
      type: "n8n.set"
      downstream: []

  modification_limits:
    max_scope: 100
    allowed_types:
      - tune_satisfaction
      - tune_field_modulation
      - modify_bind_space
      - revise_beliefs
    min_evidence: 0.85
    min_satisfaction: 0.3
    allow_irreversible: false
    max_per_hour: 20
    max_per_day: 100

  lifecycle:
    infancy_threshold_cycles: 20
    adolescence_dk_minimum: "slope"
    adolescence_modifier_minimum: 0.5
    maturity_dk_minimum: "plateau"
    maturity_modifier_minimum: 0.7
    senescence_idle_cycles: 1000
    prune_threshold: 0.1
    prune_consecutive_cycles: 20
    crystallize_variance_threshold: 0.01
```

---

## 16. The Philosophical Foundation

### Autopoiesis (Maturana & Varela)

"Living systems are machines that produce themselves."

n8n-rs workflows that modify their own routing ARE self-producing systems.
The routing decisions maintain the topology that produces the routing
decisions. This is not metaphor -- it is the formal definition of
autopoiesis applied to a computational substrate.

### Enactivism (Varela, Thompson, Rosch)

"Cognition is embodied action."

The workflow's "body" is its topology. Its "cognition" is the MUL-gated
routing decisions. Its "action" is the execution of nodes. Cognition,
body, and action are inseparable -- the workflow cannot think without
its topology, and its topology cannot exist without its cognition.

### Structural Coupling (Maturana)

"Two autopoietic systems co-evolve through recurrent interactions
while each maintains its own autopoiesis."

Workflows coupled through BindSpace blackboard co-evolve their routing
without surrendering their individual autonomy. Neither controls the
other. Both maintain independent Q-values, MUL state, and topology.

### Homeostasis (Cannon / Ashby)

"Living systems maintain internal stability."

MUL IS homeostatic regulation. The FreeWillModifier IS the homeostatic
variable. The identity set-point IS what is being maintained. Allostatic
load IS the accumulated deviation cost. When deviation exceeds tolerance,
agency restricts (modifier drops, thresholds rise, gate closes).

### Friston Free Energy Principle

"Living systems minimize surprise."

The autopoietic loop IS free energy minimization:
- Q-value routing minimizes routing prediction error (select best paths)
- Crystallization converts surprise into knowledge (reduces future surprise)
- MUL Layer 10 (PostActionLearning) measures prediction error explicitly
- The system acts to make its predictions come true (active inference)

---

*This specification defines n8n-rs as a self-orchestrating living system.
The workflow is autopoietic: it produces the components (routing decisions)
that maintain its own organization (topology). MUL from ladybug-rs is the
immune system. Crystallization is the DNA. Q-values are gene expression.
Topology evolution is development. Structural coupling is ecology.
All of this works without ada-rs. The system is alive on its own terms.*
