# N8N-RS Integration Execution Plan

> **Date**: 2026-02-15
> **Branch**: `claude/ada-rs-consolidation-6nvNm`
> **Scope**: What n8n-rs needs to build to integrate with the cognitive stack

---

## Role: N8N-RS IS the Orchestrator

N8n-rs is the workflow execution engine — self-orchestrating, self-modifying,
autopoietic. It uses ladybug-rs BindSpace as substrate and can receive delegated
tasks from crewai-rust. It does NOT depend on ada-rs.

```
N8n-rs provides:
├── FreeWillPipeline: 7-step modification evaluation         ← EXISTS
├── ImpactGate: 5-level impact classification                ← EXISTS
├── Autopoiesis: Self-modifying workflow topology             ← TO BUILD
├── Q-Value Routing: Learned route selection per node         ← TO BUILD
├── MUL Bridge: Connect to ladybug-rs MulSnapshot             ← TO BUILD
├── Workflow → GEL Compiler: Steps → FireflyFrames            ← TO BUILD
└── Structural Coupling: Co-evolution via blackboard          ← TO BUILD
```

---

## Phase 1: MUL Bridge (Priority: HIGHEST)

Wire ladybug-rs MulSnapshot into existing FreeWillPipeline and ImpactGate.

### New File: `crates/n8n-contract/src/mul_bridge.rs` (~200 LOC)

```rust
/// Bridge between ladybug-rs MUL and n8n-rs workflow evaluation.
///
/// MulSnapshot is received via Arrow Flight or passed directly.
/// All functions work WITHOUT MUL (backward compatible with None).
pub struct MulBridge;

impl MulBridge {
    /// Modify FreeWillPipeline thresholds based on MUL state
    pub fn adjust_limits(
        limits: &ModificationLimits,
        mul: Option<&MulSnapshot>,
    ) -> ModificationLimits;

    /// Modify ImpactGate thresholds based on MUL state
    pub fn adjust_evidence_threshold(
        base_threshold: f64,
        mul: Option<&MulSnapshot>,
    ) -> f64;
}
```

### Changes to `free_will.rs`

Add `mul_snapshot: Option<MulSnapshot>` parameter to `evaluate()`.
Apply modifier to thresholds: `effective_min = limits.min_evidence / modifier`.
When `mul_snapshot` is None, behavior is unchanged (backward compatible).

### Changes to `impact_gate.rs`

Add `mul_snapshot: Option<MulSnapshot>` to `decide()`.
When MUL gate is closed, deny Critical actions even with good evidence.
When modifier < 1.0, raise evidence threshold (harder to pass).

### Tests

- FreeWillPipeline with None MUL → same behavior as before
- FreeWillPipeline with modifier=0.5 → thresholds doubled (harder)
- FreeWillPipeline with modifier=1.5 → thresholds relaxed
- ImpactGate with gate_open=false → always deny Critical
- ImpactGate with MountStupid → deny everything

---

## Phase 2: Autopoiesis Module (Priority: HIGH)

### New File: `crates/n8n-contract/src/autopoiesis.rs` (~400 LOC)

```rust
/// Self-modifying workflow topology.
///
/// Three levels of self-modification:
/// 1. Self-Tuning: Adjust parameters (Q-values, thresholds)
/// 2. Self-Pruning/Growing: Add/remove workflow nodes
/// 3. Self-Replicating: Spawn child workflows
pub struct AutopoieticWorkflow {
    pub id: Uuid,
    pub nodes: Vec<WorkflowNode>,
    pub edges: Vec<WorkflowEdge>,
    pub q_values: NodeQValues,
    pub lifecycle: LifecycleStage,
    pub parent: Option<Uuid>,
    pub children: Vec<Uuid>,
    pub generation: u32,
}

pub enum LifecycleStage {
    Birth,        // Just created, exploring
    Infancy,      // Learning routes, high exploration
    Adolescence,  // Routes crystallizing, some pruning
    Maturity,     // Stable routes, low exploration
    Senescence,   // Routes degrading, may need replacement
}
```

### Key Operations

- `self_tune()`: Adjust Q-values based on outcome (α=0.1 learning rate)
- `self_prune()`: Remove nodes with consistently low Q-values (MUL-gated)
- `self_grow()`: Add nodes when resonance finds gaps (MUL-gated)
- `self_replicate()`: Spawn child workflow for sub-problem (MUL-gated)
- `advance_lifecycle()`: Stage transitions based on stability metrics

---

## Phase 3: Q-Value Routing (Priority: HIGH)

### New File: `crates/n8n-contract/src/q_routing.rs` (~250 LOC)

```rust
/// Q-value learned routing for workflow nodes.
///
/// Each node has 16 Q-values (one per possible output route).
/// Epsilon-greedy exploration with MUL-informed epsilon.
pub struct NodeQValues {
    pub values: HashMap<NodeId, [f32; 16]>,
    pub visit_counts: HashMap<NodeId, [u32; 16]>,
    pub epsilon: f32,          // Exploration rate (starts high, decays)
}

impl NodeQValues {
    /// Select route via epsilon-greedy with MUL adjustment
    pub fn select_route(
        &self,
        node: &NodeId,
        mul: Option<&MulSnapshot>,
    ) -> usize;

    /// Update Q-value from outcome
    pub fn update(
        &mut self,
        node: &NodeId,
        route: usize,
        reward: f32,
        alpha: f32,
    );

    /// Check if routes have crystallized (Q-values converged)
    pub fn is_crystallized(&self, node: &NodeId) -> bool;
}
```

### MUL-Informed Exploration

```rust
// When MUL says we're uncertain → explore MORE
let adjusted_epsilon = if let Some(mul) = mul_snapshot {
    if matches!(mul.dk_state, DKState::MountStupid) {
        0.9  // Almost random — we don't know enough
    } else if mul.allostatic_load > 0.8 {
        0.1  // Low exploration — we're depleted
    } else {
        self.epsilon * (2.0 - mul.modifier)  // Inverse of confidence
    }
} else {
    self.epsilon  // Default without MUL
};
```

---

## Phase 4: Workflow → GEL Compiler (Priority: MEDIUM)

### New File: `crates/n8n-contract/src/gel_compiler.rs` (~500 LOC)

```rust
/// Compile n8n-rs workflow definitions into GEL programs.
///
/// Each workflow step becomes one or more FireflyFrames.
/// The workflow DAG becomes a GEL program with FORK/JOIN.
pub struct WorkflowCompiler;

impl WorkflowCompiler {
    /// Compile a single workflow step to GEL frames
    pub fn compile_step(&self, step: &WorkflowStep) -> Vec<FireflyFrame>;

    /// Compile an entire workflow DAG to a GEL program
    pub fn compile_workflow(&self, workflow: &Workflow) -> Vec<FireflyFrame>;

    /// Map n8n-rs node types to GEL language prefixes
    fn map_node_type(&self, node_type: &str) -> LanguagePrefix;
}
```

### Mapping: Workflow Node Types → GEL Prefixes

```
n8n-rs Node Type    →  GEL Prefix    →  Typical Opcode
─────────────────      ──────────       ──────────────
data_transform         Memory (0x6)     bind, unbind, permute
condition_check        Control (0x7)    cmp, branch
api_call               Trap (0xF)       syscall
similarity_search      Lance (0x0)      resonate, hamming
inference_step         NARS (0x3)       deduce, revise
graph_query            Cypher (0x2)     match, traverse
causal_analysis        Causal (0x4)     see, do, imagine
parallel_split         Control (0x7)    fork
parallel_join          Control (0x7)    join
```

---

## Phase 5: Structural Coupling via Blackboard (Priority: MEDIUM)

### Integration with ladybug-rs BindSpace

Workflows co-evolve through shared state (stigmergy):

```rust
/// Blackboard operations for structural coupling.
///
/// Workflows don't message each other directly.
/// They read/write shared state in BindSpace.
pub struct BlackboardAccess {
    /// Read shared state from BindSpace blackboard prefix (0x0E)
    pub fn read_shared(&self, slot: u8) -> Option<CogRecord>;

    /// Write shared state to BindSpace blackboard
    pub fn write_shared(&mut self, slot: u8, record: CogRecord);

    /// Watch for changes on a blackboard slot (trigger on write)
    pub fn watch(&self, slot: u8) -> Receiver<CogRecord>;
}
```

### The blackboard lives in ladybug-rs BindSpace prefix 0x0E:
- 0x0E:00 — Global context (shared by all workflows)
- 0x0E:01-0x0E:0F — Per-workflow state
- 0x0E:10-0x0E:1F — Agent communication slots (crewai-rust)
- 0x0E:20-0x0E:FF — Application-specific

---

## Execution Timeline

```
Week 1: Phase 1 (MUL bridge) — wire MulSnapshot to FreeWill + ImpactGate
Week 2: Phase 2 (Autopoiesis) + Phase 3 (Q-routing)
Week 3: Phase 4 (GEL compiler) + Phase 5 (Blackboard)
```

---

## Verification Checklist

- [ ] `cargo check` — clean compile
- [ ] `cargo test` — all existing tests still pass (backward compatible)
- [ ] FreeWillPipeline with `mul_snapshot: None` → unchanged behavior
- [ ] FreeWillPipeline with MUL → thresholds modulated correctly
- [ ] ImpactGate with MUL → Critical actions gated when uncertain
- [ ] Q-value routing selects optimal route after learning
- [ ] Autopoietic workflow can self-tune, self-prune, self-replicate
- [ ] Workflow compiles to GEL frames and executes correctly
- [ ] Blackboard read/write works via BindSpace prefix 0x0E
- [ ] No ada-rs dependency anywhere in n8n-rs

---

## Dependency Map

```
ladybug-rs (substrate):
├── MulSnapshot type            ← n8n-rs imports this
├── BindSpace blackboard        ← n8n-rs reads/writes this
├── GEL FireflyFrame            ← n8n-rs compiles to this
└── Arrow Flight                ← n8n-rs communicates via this

crewai-rust (agency):
├── DelegationRequest           → n8n-rs receives tasks from this
├── DelegationResponse          ← n8n-rs returns results to this
└── Agent status updates        ← n8n-rs reports progress to this
```
