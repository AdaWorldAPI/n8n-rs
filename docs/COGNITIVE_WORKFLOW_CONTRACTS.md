# Cognitive Workflow Contracts -- n8n-rs API Surface

> **Date**: 2026-02-15
> **Scope**: Exact API contracts that n8n-rs provides and consumes.
> **Dependency**: ladybug-rs (substrate). Ada-rs is NOT required.
> **Principle**: n8n-rs is self-sufficient for cognitive workflow orchestration.
>   These contracts define the boundaries between n8n-rs, ladybug-rs, and
>   crewai-rust. Every function signature, struct field, and RPC action is
>   specified here for implementors.

---

## 1. Contracts n8n-rs PROVIDES

These are the APIs that n8n-rs implements. Other systems (crewai-rust,
ladybug-rs, external clients) call these contracts.

### 1.1 WorkflowExecution

The core execution contract. Manages workflow step execution, routing,
and topology changes.

```rust
/// Execute a single step in a workflow.
///
/// This is the primary execution entry point. The step is:
/// 1. Routed by prefix (n8n.*, crew.*, lb.*)
/// 2. Executed by the appropriate backend
/// 3. Q-values consulted for downstream routing
/// 4. MUL snapshot evaluated at step boundary
///
/// Returns the completed step with output and routing decision.
pub async fn execute_step(
    &mut self,
    workflow_id: u64,
    step: &mut UnifiedStep,
    input: DataEnvelope,
    mul_snapshot: &MulSnapshot,
) -> Result<StepExecutionResult, WorkflowError> { ... }

/// Route from a completed step to the next downstream node.
///
/// Uses Q-value routing with MUL-informed epsilon-greedy selection.
/// Returns the index of the selected downstream node and the
/// exploration/exploitation flag.
pub fn route(
    &self,
    workflow_id: u64,
    current_node_index: usize,
    mul_snapshot: &MulSnapshot,
    mode: StrategicMode,
) -> RouteDecision { ... }

/// Propose and apply a topology change.
///
/// The change goes through FreeWillPipeline evaluation. If approved,
/// the topology is modified and topology_generation is incremented.
pub fn topology_change(
    &mut self,
    workflow_id: u64,
    change: TopologyChange,
    mul_snapshot: &MulSnapshot,
) -> Result<TopologyChangeResult, WorkflowError> { ... }
```

**Supporting types**:

```rust
/// Result of executing a single step.
pub struct StepExecutionResult {
    /// The completed step (status, output, timing).
    pub step: UnifiedStep,
    /// Q-value routing decision for next step.
    pub route_decision: RouteDecision,
    /// Updated MUL snapshot after step execution.
    pub mul_after: MulSnapshot,
    /// Whether crystallization was triggered.
    pub crystallized: bool,
}

/// A routing decision from Q-value selection.
pub struct RouteDecision {
    /// Index of selected downstream node.
    pub downstream_index: usize,
    /// Whether this was exploration (random) or exploitation (best Q).
    pub explored: bool,
    /// The epsilon value used for selection.
    pub epsilon: f32,
    /// Q-value of the selected route.
    pub q_value: f32,
}

/// A topology change proposal.
pub enum TopologyChange {
    /// Remove an edge between two nodes.
    PruneEdge { from: usize, to: usize },
    /// Add an edge between two nodes.
    GrowEdge { from: usize, to: usize },
    /// Deactivate a node (soft delete).
    DeactivateNode { index: usize },
    /// Spawn a child workflow specialized for a pattern.
    Replicate { pattern_fingerprint: u64 },
}

/// Result of a topology change operation.
pub struct TopologyChangeResult {
    /// Whether the change was approved by FreeWillPipeline.
    pub approved: bool,
    /// Gate decision from FreeWillPipeline.
    pub decision: GateDecision,
    /// New topology generation (if approved).
    pub new_generation: Option<u64>,
    /// Child workflow ID (for Replicate changes).
    pub child_id: Option<u64>,
    /// Denial reason (if denied).
    pub denial_reason: Option<String>,
}
```

### 1.2 FreeWillPipeline

The gating contract for self-modification proposals. This is the immune
system's enforcement mechanism.

```rust
/// Evaluate a self-modification proposal against YAML limits and MUL state.
///
/// This is the 7-step pipeline:
///   1. Type check (allowed_types)
///   2. Scope check (max_scope)
///   3. Reversibility check (allow_irreversible)
///   4. Evidence check (min_evidence / modifier)
///   5. Satisfaction check (min_satisfaction / modifier)
///   6. Rate limit check (max_per_hour * modifier)
///   7. RBAC gate (ImpactGate.check_with_mul)
///
/// All 7 steps must pass for approval.
pub fn evaluate_with_mul(
    &mut self,
    proposal: &ModificationProposal,
    mul_snapshot: &MulSnapshot,
) -> ProposalResult { ... }

/// Backward-compatible evaluate without MUL (uses modifier=1.0).
pub fn evaluate(
    &mut self,
    proposal: &ModificationProposal,
) -> ProposalResult { ... }

/// Create a ModificationProposal from raw parameters.
///
/// Helper for programmatic proposal creation (vs. CogPacket extraction).
pub fn propose_modification(
    modification_type: ModificationType,
    evidence: TruthValue,
    satisfaction: &[f32],
    scope: u32,
    reversible: bool,
    justification: String,
) -> ModificationProposal { ... }

/// Apply an approved modification to the workflow.
///
/// Pre-condition: proposal must have been approved by evaluate_with_mul().
/// Increments rate counters, records operation in ImpactGate,
/// and returns the response CogPacket.
pub fn apply(
    &mut self,
    proposal: &ModificationProposal,
) -> CogPacket { ... }

/// Extract a proposal from a CogPacket (ladybug-rs wire protocol).
pub fn extract_proposal(packet: &CogPacket) -> ModificationProposal { ... }

/// Process a CogPacket through the full pipeline: extract -> evaluate -> respond.
pub fn process_packet(
    &mut self,
    packet: &CogPacket,
) -> ProposalResult { ... }

/// Get current modification limits.
pub fn limits(&self) -> &ModificationLimits { ... }

/// Update modification limits from YAML reload.
pub fn set_limits(&mut self, limits: ModificationLimits) { ... }

/// Reset hourly rate counters.
pub fn reset_hourly(&mut self) { ... }

/// Reset daily rate counters.
pub fn reset_daily(&mut self) { ... }
```

### 1.3 ImpactGate

The RBAC-based operation gating contract. Controls what operations are
permitted based on role, impact level, evidence, and MUL state.

```rust
/// Classify the impact level of a workflow operation.
///
/// Maps operation types to ImpactLevel for RBAC gating:
///   Observe:     read-only queries, introspection
///   Internal:    cache updates, Q-value changes
///   Moderate:    external API calls, agent delegation
///   Significant: topology changes, crystallization
///   Critical:    self-modification, architectural restructuring
pub fn classify_impact(operation: &str) -> ImpactLevel { ... }

/// Check evidence against role requirements and MUL state.
///
/// Returns true if the evidence meets the threshold for this
/// role + impact combination, adjusted by the MUL modifier.
pub fn check_evidence(
    &self,
    role_id: &str,
    impact: ImpactLevel,
    truth_value: &TruthValue,
    mul_snapshot: &MulSnapshot,
) -> bool { ... }

/// Full gate decision with MUL integration.
///
/// Checks role, impact, evidence, budget, satisfaction, and MUL gate.
pub fn check_with_mul(
    &self,
    role_id: &str,
    impact: ImpactLevel,
    truth_value: &TruthValue,
    stack_satisfaction: &[f32],
    mul_snapshot: &MulSnapshot,
) -> GateDecision { ... }

/// Backward-compatible check without MUL.
pub fn check(
    &self,
    role_id: &str,
    impact: ImpactLevel,
    truth_value: &TruthValue,
    stack_satisfaction: &[f32],
) -> GateDecision { ... }

/// Record that an operation was performed (increment budget counters).
pub fn record_operation(&mut self, role_id: &str, impact: ImpactLevel) { ... }

/// Register a new role definition.
pub fn register_role(&mut self, role: RoleDefinition) { ... }

/// Budget reset functions.
pub fn reset_minute_budgets(&mut self) { ... }
pub fn reset_hour_budgets(&mut self) { ... }
pub fn reset_day_budgets(&mut self) { ... }
```

### 1.4 AutopoieticWorkflow

The self-orchestrating workflow contract. Manages lifecycle, replication,
and topology evolution.

```rust
/// Spawn a child workflow specialized for a sub-pattern.
///
/// Pre-conditions:
///   - Parent level must be SelfReplicating
///   - MUL: gate_open, modifier > 0.7, DK = Plateau
///   - FreeWillPipeline must approve (ImpactLevel::Significant)
///
/// The child inherits:
///   - Parent topology (cloned)
///   - Parent crystallized Q-values (warm start)
///   - Independent MUL state (starts at MountStupid)
///   - Independent lifecycle (born_at = now)
///
/// Returns the child workflow ID.
pub fn spawn_child(
    &mut self,
    pattern_fingerprint: u64,
    mul_snapshot: &MulSnapshot,
    pipeline: &mut FreeWillPipeline,
) -> Result<u64, WorkflowError> { ... }

/// Prune a node from the workflow (soft deactivation).
///
/// Pre-conditions:
///   - Workflow level must be SelfPruning or higher
///   - MUL: gate_open, modifier > 0.5
///   - Q-values for this node must be below threshold
///   - FreeWillPipeline must approve (ImpactLevel::Moderate)
///
/// The node is marked inactive but not deleted (reversible).
pub fn prune_node(
    &mut self,
    node_index: usize,
    mul_snapshot: &MulSnapshot,
    pipeline: &mut FreeWillPipeline,
) -> Result<bool, WorkflowError> { ... }

/// Replicate the entire workflow with specialized Q-values.
///
/// Creates a full clone with Q-values biased toward the specified
/// pattern. Used for Level 3 self-replication when the parent
/// detects a consistent routing split.
pub fn replicate(
    &self,
    specialization: ReplicationSpec,
    mul_snapshot: &MulSnapshot,
    pipeline: &mut FreeWillPipeline,
) -> Result<AutopoieticWorkflow, WorkflowError> { ... }

/// Advance the lifecycle phase based on current state.
///
/// Phase transitions are MUL-gated:
///   Birth -> Infancy: automatic after first execution
///   Infancy -> Adolescence: DK >= Slope AND modifier > 0.5
///   Adolescence -> Maturity: DK = Plateau AND modifier > 0.7
///   Any -> Senescence: idle_cycles > threshold AND homeostasis = Apathy
pub fn advance_lifecycle(&mut self, mul_snapshot: &MulSnapshot) { ... }

/// Get the current lifecycle phase.
pub fn phase(&self) -> LifecyclePhase { ... }

/// Get topology generation counter.
pub fn topology_generation(&self) -> u64 { ... }

/// Get lineage information (parent, children, birth).
pub fn lineage(&self) -> LineageInfo { ... }
```

**Supporting types**:

```rust
/// Specification for workflow replication.
pub struct ReplicationSpec {
    /// Fingerprint of the pattern this child specializes in.
    pub pattern_fingerprint: u64,
    /// Q-value bias: increase Q-values for routes matching this pattern.
    pub q_bias: f32,
    /// Name suffix for the child (appended to parent name).
    pub name_suffix: String,
}

/// Lineage information for a workflow.
pub struct LineageInfo {
    pub workflow_id: u64,
    pub parent: Option<u64>,
    pub children: Vec<u64>,
    pub topology_generation: u64,
    pub born_at: u64,
    pub total_cycles: u64,
    pub phase: LifecyclePhase,
    pub level: SelfOrchestrationLevel,
}
```

---

## 2. Contracts n8n-rs CONSUMES (from ladybug-rs)

These are the APIs that n8n-rs calls on ladybug-rs. n8n-rs is the
consumer; ladybug-rs is the provider.

### 2.1 MulSnapshot

n8n-rs reads MUL state from ladybug-rs via the MulSnapshot struct.
This is the primary contract between the two systems for metacognitive
gating.

```rust
/// MUL snapshot -- consumed by n8n-rs from ladybug-rs.
///
/// n8n-rs NEVER constructs this directly. It is always received from
/// ladybug-rs MetaUncertaintyLayer.evaluate() or unpacked from
/// CogRecord W64-W65.
pub struct MulSnapshot {
    /// Trust texture from Layer 1.
    pub trust_texture: TrustTexture,

    /// Dunning-Kruger position from Layer 2.
    pub dk_position: DKPosition,

    /// Homeostasis state from Layer 6.
    pub homeostasis_state: HomeostasisState,

    /// False flow severity from Layer 5.
    pub false_flow_severity: FalseFlowSeverity,

    /// Multiplicative confidence modifier (0.0-1.0).
    /// Product of dk_factor * trust_factor * complexity_factor * flow_factor.
    pub free_will_modifier: FreeWillModifier,

    /// Whether the MUL gate (Layer 7) is open.
    /// If false, all operations above Observe are blocked.
    pub gate_open: bool,

    /// Reason the gate is blocked (if gate_open is false).
    pub gate_block_reason: Option<GateBlockReason>,

    /// Accumulated deviation from identity set-point (0.0-1.0).
    pub allostatic_load: f32,
}

// n8n-rs reads these fields:
//   mul.gate_open              -> decides whether to allow modification
//   mul.free_will_modifier     -> modulates thresholds
//   mul.dk_position            -> determines orchestration level transitions
//   mul.allostatic_load        -> limits exploration rate
//   mul.homeostasis_state      -> triggers lifecycle phase changes
//   mul.false_flow_severity    -> triggers loop disruption
```

### 2.2 BindSpace

n8n-rs interacts with the ladybug-rs BindSpace for blackboard stigmergy,
crystallized knowledge storage, and workflow state persistence.

```rust
/// Read a fingerprint from a BindSpace address.
///
/// n8n-rs reads from:
///   prefix 0x0E (blackboard): shared workflow state
///   prefix 0x0F (n8n domain): workflow-specific state
pub async fn read(
    &self,
    prefix: u8,
    slot: u8,
) -> Option<Fingerprint> { ... }

/// Write a fingerprint to a BindSpace address.
///
/// n8n-rs writes to:
///   prefix 0x0E (blackboard): publish workflow outputs for coupling
///   prefix 0x0F (n8n domain): store workflow-specific state
pub async fn write(
    &mut self,
    prefix: u8,
    slot: u8,
    fingerprint: Fingerprint,
) -> Result<(), BindSpaceError> { ... }

/// Search for fingerprints similar to a query within a prefix range.
///
/// Used for:
///   - Structural coupling: find other workflows' outputs
///   - Knowledge retrieval: find crystallized route knowledge
///   - Pattern matching: find similar execution contexts
///
/// Returns matches sorted by Hamming distance (ascending).
pub async fn resonance_search(
    &self,
    query: Fingerprint,
    prefix: u8,
    max_distance: u32,
    limit: usize,
) -> Vec<ResonanceMatch> { ... }

/// A resonance search match result.
pub struct ResonanceMatch {
    pub prefix: u8,
    pub slot: u8,
    pub fingerprint: Fingerprint,
    pub hamming_distance: u32,
}
```

### 2.3 CogRecord

n8n-rs stores workflow node state as CogRecord containers. Each workflow
node maps to a CogRecord with the following word allocation:

```
CogRecord Word Layout for Workflow Nodes:

  W0:     DN address (node_id hash)
  W1:     Node type (trigger=0, action=1, condition=2, ...)
  W8:     Execution state (HOLD=0, FLOW=1, BLOCK=2)
  W12-15: Thinking style weights (10 x f32, packed)
  W16-31: Verb edges (TRIGGERS, FEEDS, GUARDS connections)
  W32-39: Q-values (16 x f32, 2 per word)
  W48-55: Health metrics (throughput, latency, error rate)
  W64-65: MUL state (packed, see MUL Snapshot Wire Format)
  W66-79: Reserved
  W80-127: Content (step parameters, serialized JSON)
```

```rust
/// Store workflow node state as a CogRecord container.
pub fn store_node_state(
    node: &WorkflowStrategicNode,
) -> CogRecord { ... }

/// Load workflow node state from a CogRecord container.
pub fn load_node_state(
    record: &CogRecord,
) -> WorkflowStrategicNode { ... }
```

### 2.4 GEL (Generic Execution Language)

n8n-rs compiles workflow steps to GEL frames for execution on the
ladybug-rs runtime.

```rust
/// Compile a workflow step into a GEL FireflyFrame.
pub fn compile_to_frame(
    step: &UnifiedStep,
    lane_id: &str,
) -> FireflyFrame { ... }

/// Submit a frame to the GEL runtime for execution.
pub async fn submit_frame(
    frame: FireflyFrame,
) -> Result<FrameHandle, GelError> { ... }

/// Wait for a frame to complete and retrieve its result.
pub async fn await_frame(
    handle: FrameHandle,
) -> Result<FrameResult, GelError> { ... }

/// Submit multiple frames as a fan-out group.
/// Returns when all frames complete (fan-in barrier).
pub async fn submit_fan_out(
    frames: Vec<FireflyFrame>,
    lane_id: &str,
) -> Result<Vec<FrameResult>, GelError> { ... }
```

---

## 3. Contracts n8n-rs PROVIDES to crewai-rust

crewai-rust delegates task execution to n8n-rs workflows. These contracts
define that delegation boundary.

### 3.1 Task Execution

crewai-rust agents create tasks that n8n-rs executes as workflows.

```rust
/// Execute a workflow on behalf of a crewai-rust agent.
///
/// The step_type prefix determines routing:
///   "n8n.*"  -> n8n-rs handles directly
///   "crew.*" -> delegated back to crewai-rust (round-trip)
///   "lb.*"   -> delegated to ladybug-rs
///
/// crewai-rust calls this via:
///   UnifiedStep { step_type: "n8n.workflow_execute", ... }
///   StepDelegationRequest { step, input: DataEnvelope }
pub async fn execute_workflow(
    &mut self,
    request: StepDelegationRequest,
) -> Result<StepDelegationResponse, WorkflowError> { ... }

/// Execute a single step within a running workflow.
///
/// Called by crewai-rust's unified execution contract when the
/// step_type prefix is "n8n.*".
pub async fn execute_single_step(
    &mut self,
    step: &mut UnifiedStep,
    input: DataEnvelope,
) -> Result<DataEnvelope, WorkflowError> { ... }
```

### 3.2 Status Reporting

n8n-rs reports workflow execution progress to crewai-rust.

```rust
/// Get the current status of a workflow execution.
///
/// Returns the full execution state including all step statuses,
/// current MUL snapshot, and topology generation.
pub fn get_execution_status(
    &self,
    execution_id: &str,
) -> Option<ExecutionStatus> { ... }

/// Subscribe to execution status updates (streaming).
///
/// Returns a stream of status events for real-time monitoring.
pub async fn subscribe_status(
    &self,
    execution_id: &str,
) -> BoxStream<'static, ExecutionEvent> { ... }

/// Execution status snapshot.
pub struct ExecutionStatus {
    pub execution_id: String,
    pub workflow_name: String,
    pub status: StepStatus,
    pub steps_completed: usize,
    pub steps_total: usize,
    pub current_step: Option<String>,
    pub mul_snapshot: MulSnapshot,
    pub topology_generation: u64,
    pub started_at: Option<DateTime<Utc>>,
    pub elapsed_ms: u64,
}

/// Execution lifecycle events for streaming updates.
pub enum ExecutionEvent {
    StepStarted { step_id: String, step_type: String },
    StepCompleted { step_id: String, output_size: usize },
    StepFailed { step_id: String, error: String },
    RouteDecision { from_node: String, to_node: String, explored: bool },
    TopologyChanged { change: TopologyChange, generation: u64 },
    Crystallized { node_id: String, route: usize, quality: f32 },
    ExecutionCompleted { total_steps: usize, elapsed_ms: u64 },
    ExecutionFailed { error: String },
}
```

### 3.3 Result Delivery

n8n-rs delivers final workflow results to crewai-rust in the unified
envelope format.

```rust
/// Get the final result of a completed workflow execution.
///
/// Returns a DataEnvelope containing the final output data and
/// metadata (confidence, layer activations, NARS frequency, etc.).
pub fn get_result(
    &self,
    execution_id: &str,
) -> Option<DataEnvelope> { ... }

/// Get results from all completed steps in an execution.
///
/// Useful for crewai-rust agents that need intermediate outputs
/// (e.g., reasoning traces from agent steps, Q-value decisions).
pub fn get_step_results(
    &self,
    execution_id: &str,
) -> Vec<(String, DataEnvelope)> { ... }
```

---

## 4. Arrow Flight DoAction RPC Catalog

All autopoietic workflow operations are exposed as Arrow Flight DoAction
RPCs. This enables zero-copy, high-performance access to the full n8n-rs
cognitive workflow API.

### 4.1 Workflow Execution Actions

```
DoAction("workflow.execute")
  Description: Execute a complete workflow from trigger to completion.
  Input:  {
      workflow_id: u64,
      input: JSON,           // initial input data
      mode: StrategicMode?,  // optional: override default mode
  }
  Output: {
      execution_id: String,
      status: "completed" | "failed",
      output: JSON,
      steps_completed: u32,
      elapsed_ms: u64,
      mul_snapshot: MulSnapshot,
  }
  Impact: Moderate

DoAction("workflow.step")
  Description: Execute a single step in an ongoing execution.
  Input:  {
      execution_id: String,
      step_index: u32,
      input: JSON,
  }
  Output: {
      step_id: String,
      status: "completed" | "failed",
      output: JSON,
      route_decision: RouteDecision,
      mul_after: MulSnapshot,
  }
  Impact: Internal

DoAction("workflow.route")
  Description: Get the routing decision for a node without executing.
  Input:  {
      workflow_id: u64,
      node_index: u32,
      mode: StrategicMode?,
  }
  Output: {
      downstream_index: u32,
      explored: bool,
      epsilon: f32,
      q_value: f32,
      all_q_values: [f32; 16],
  }
  Impact: Observe
```

### 4.2 Autopoietic Lifecycle Actions

```
DoAction("workflow.spawn")
  Description: Spawn a child workflow specialized for a sub-pattern.
  Input:  {
      parent_id: u64,
      pattern_fingerprint: u64,
      name_suffix: String,
      q_bias: f32,
  }
  Output: {
      child_id: u64,
      approved: bool,
      decision: GateDecision,
      denial_reason?: String,
  }
  Impact: Significant

DoAction("workflow.prune")
  Description: Prune an unused edge or deactivate a node.
  Input:  {
      workflow_id: u64,
      change_type: "edge" | "node",
      from_index?: u32,
      to_index?: u32,
      node_index?: u32,
  }
  Output: {
      approved: bool,
      decision: GateDecision,
      new_generation: u64?,
      denial_reason?: String,
  }
  Impact: Moderate

DoAction("workflow.replicate")
  Description: Full workflow replication with specialized Q-values.
  Input:  {
      source_id: u64,
      pattern_fingerprint: u64,
      name_suffix: String,
      q_bias: f32,
  }
  Output: {
      new_id: u64,
      approved: bool,
      topology_generation: u64,
      nodes_count: u32,
      decision: GateDecision,
  }
  Impact: Significant
```

### 4.3 Q-Value and Crystallization Actions

```
DoAction("workflow.q_update")
  Description: Update a node's Q-value after observing a routing outcome.
  Input:  {
      workflow_id: u64,
      node_index: u32,
      route_taken: u32,
      outcome_quality: f32,  // 0.0 = bad, 1.0 = good
  }
  Output: {
      previous_q: f32,
      new_q: f32,
      learning_rate_used: f32,
      crystallization_triggered: bool,
  }
  Impact: Internal

DoAction("workflow.crystallize_route")
  Description: Force crystallization of a converged route.
  Input:  {
      workflow_id: u64,
      node_index: u32,
      route_index: u32,
  }
  Output: {
      spo_key: String,
      bindspace_prefix: u8,
      bindspace_slot: u8,
      q_variance: f32,
      crystallized: bool,
  }
  Impact: Significant
```

### 4.4 FreeWill Pipeline Actions

```
DoAction("freewill.evaluate")
  Description: Evaluate a modification proposal through the 7-step pipeline.
  Input:  {
      modification_type: ModificationType,
      source_layer: u8,
      evidence: { frequency: f32, confidence: f32 },
      satisfaction: [f32; 10],
      scope: u32,
      reversible: bool,
      justification: String,
  }
  Output: {
      approved: bool,
      decision: GateDecision,
      effective_min_evidence: f32,
      effective_min_satisfaction: f32,
      modifier_used: f32,
      denial_reason?: String,
  }
  Impact: Observe (evaluation itself is read-only)

DoAction("freewill.propose")
  Description: Create a modification proposal from parameters.
  Input:  {
      modification_type: ModificationType,
      evidence: { frequency: f32, confidence: f32 },
      scope: u32,
      reversible: bool,
      justification: String,
  }
  Output: {
      proposal_id: String,
      impact_level: ImpactLevel,
      modification_type: ModificationType,
  }
  Impact: Observe

DoAction("freewill.apply")
  Description: Apply an approved modification.
  Input:  {
      proposal_id: String,
      workflow_id: u64,
  }
  Output: {
      applied: bool,
      topology_generation: u64?,
      response_packet_size: u32,
  }
  Impact: Critical
```

### 4.5 Impact Gate Actions

```
DoAction("impact.classify")
  Description: Classify the impact level of an operation.
  Input:  {
      operation: String,         // e.g., "modify_routing", "read_state"
      modification_type?: ModificationType,
  }
  Output: {
      impact_level: ImpactLevel,
      description: String,
  }
  Impact: Observe

DoAction("impact.decide")
  Description: Run a full gate decision with MUL integration.
  Input:  {
      role_id: String,
      impact: ImpactLevel,
      evidence: { frequency: f32, confidence: f32 },
      satisfaction: [f32; 10],
  }
  Output: {
      decision: GateDecision,
      effective_threshold: f32,
      modifier_used: f32,
      gate_open: bool,
      budget_remaining: {
          minute: u32,
          hour: u32,
          day: u32,
      },
  }
  Impact: Observe
```

### 4.6 Topology and Lineage Query Actions

```
DoAction("workflow.get_topology")
  Description: Get the full topology of an autopoietic workflow.
  Input:  { workflow_id: u64 }
  Output: {
      nodes: [{
          node_id: String,
          node_type: String,
          q_values: [f32; 16],
          downstream: [u32],
          execution_count: u64,
          mul_snapshot: MulSnapshot,
      }],
      topology_generation: u64,
      level: SelfOrchestrationLevel,
      phase: LifecyclePhase,
  }
  Impact: Observe

DoAction("workflow.get_lineage")
  Description: Get lineage information for a workflow.
  Input:  { workflow_id: u64 }
  Output: {
      workflow_id: u64,
      parent: u64?,
      children: [u64],
      topology_generation: u64,
      born_at: u64,
      total_cycles: u64,
      phase: LifecyclePhase,
      level: SelfOrchestrationLevel,
  }
  Impact: Observe

DoAction("workflow.get_q_values")
  Description: Get Q-values for a specific node.
  Input:  { workflow_id: u64, node_index: u32 }
  Output: {
      q_values: [f32; 16],
      best_route: u32,
      convergence_variance: f32,
      crystallized_routes: [u32],
      total_updates: u64,
  }
  Impact: Observe
```

---

## 5. Error Contract

All contracts use a unified error type hierarchy.

```rust
/// Workflow execution errors.
#[derive(Debug, Error)]
pub enum WorkflowError {
    /// Workflow not found.
    #[error("Workflow {0} not found")]
    NotFound(u64),

    /// Step execution failed.
    #[error("Step '{step_id}' failed: {message}")]
    StepFailed {
        step_id: String,
        message: String,
    },

    /// Modification denied by FreeWillPipeline.
    #[error("Modification denied: {decision:?} -- {reason}")]
    ModificationDenied {
        decision: GateDecision,
        reason: String,
    },

    /// MUL gate is closed (system cannot proceed).
    #[error("MUL gate closed: {reason:?}")]
    GateClosed {
        reason: GateBlockReason,
    },

    /// Topology change would create invalid state.
    #[error("Invalid topology change: {0}")]
    InvalidTopology(String),

    /// Lifecycle phase does not permit this operation.
    #[error("Operation requires {required:?} but workflow is in {current:?}")]
    PhaseRestriction {
        required: SelfOrchestrationLevel,
        current: SelfOrchestrationLevel,
    },

    /// Rate limit exceeded.
    #[error("Rate limit exceeded: {budget_type} ({used}/{max})")]
    RateLimited {
        budget_type: String,
        used: u32,
        max: u32,
    },

    /// GEL frame execution error.
    #[error("GEL frame error: {0}")]
    GelError(String),

    /// BindSpace operation error.
    #[error("BindSpace error: {0}")]
    BindSpaceError(String),

    /// Serialization/deserialization error.
    #[error("Serialization error: {0}")]
    SerdeError(String),
}
```

---

## 6. Wire Format Summary

### 6.1 CogPacket Encoding for Workflow Operations

```
CogPacket fields used by n8n-rs:
  opcode:     Wire operation (EXECUTE, DELEGATE, RESONATE, INTEGRATE, etc.)
  source_addr: 0x0F00 (n8n-rs domain)
  target_addr: Varies by operation:
               0x0F00 = n8n-rs internal
               0x0C00 = crewai-rust delegation
               0x0500 = ladybug-rs cognitive
  layer:       Source cognitive layer (0-9)
  rung:        Impact level (0-4) or ModificationType (0-7)
  truth_value: NARS evidence (frequency, confidence)
  satisfaction: 10-layer satisfaction array
  fan_out:     Scope of modification (for INTEGRATE packets)
  flags:       FLAG_DELEGATION, FLAG_VALIDATED, FLAG_ERROR, FLAG_CRYSTALLIZED
  content:     Container with serialized payload
```

### 6.2 DataEnvelope Wire Format

```json
{
    "data": { /* arbitrary JSON payload */ },
    "metadata": {
        "source_step": "step-uuid",
        "confidence": 0.92,
        "epoch": 1739635200000,
        "version": "1.0",
        "dominant_layer": 4,
        "layer_activations": [0.2, 0.3, 0.5, 0.8, 0.9, 0.7, 0.6, 0.4, 0.3, 0.2],
        "nars_frequency": 0.95,
        "calibration_error": 0.05
    }
}
```

### 6.3 Arrow Flight Serialization

All DoAction inputs and outputs are serialized as JSON in the
`Action.body` bytes field. The action type string identifies the
operation. Results are returned as `arrow_flight::Result` with JSON
body.

```
Request:
  Action {
      type: "workflow.execute",
      body: b'{"workflow_id": 42, "input": {"query": "test"}}',
  }

Response:
  arrow_flight::Result {
      body: b'{"execution_id": "abc-123", "status": "completed", ...}',
  }
```

---

## 7. Dependency Graph

```
n8n-rs
  |
  +-- PROVIDES to crewai-rust:
  |     execute_workflow()
  |     execute_single_step()
  |     get_execution_status()
  |     subscribe_status()
  |     get_result()
  |     get_step_results()
  |
  +-- PROVIDES to external clients (via Arrow Flight):
  |     workflow.execute, workflow.step, workflow.route
  |     workflow.spawn, workflow.prune, workflow.replicate
  |     workflow.q_update, workflow.crystallize_route
  |     freewill.evaluate, freewill.propose, freewill.apply
  |     impact.classify, impact.decide
  |     workflow.get_topology, workflow.get_lineage, workflow.get_q_values
  |
  +-- CONSUMES from ladybug-rs:
  |     MulSnapshot (metacognitive state)
  |     BindSpace (blackboard, resonance search)
  |     CogRecord (node state persistence)
  |     GEL (frame compilation and execution)
  |     CogPacket (binary wire protocol)
  |     TruthValue (NARS evidence)
  |
  +-- DOES NOT DEPEND ON:
        ada-rs (consciousness awareness -- optional enhancement only)

Dependency direction:
  n8n-rs --> ladybug-rs (required)
  crewai-rust --> n8n-rs (for workflow execution)
  n8n-rs --> crewai-rust (for crew.* step delegation, via CrewRouter)
  ada-rs --> ladybug-rs (separate chain, not required by n8n-rs)
```

---

## 8. Implementation Checklist

```
Phase 1 -- Foundation (n8n-rs changes):
  [ ] Add evaluate_with_mul() to FreeWillPipeline        (~30 LOC)
  [ ] Add check_with_mul() to ImpactGate                 (~20 LOC)
  [ ] Define NodeQValues struct with select_route/update  (~80 LOC)
  [ ] Define AutopoieticWorkflow struct                   (~60 LOC)
  [ ] Define WorkflowStrategicNode struct                 (~40 LOC)

Phase 2 -- Routing (n8n-rs changes):
  [ ] Implement Q-value routing in execute_step()         (~50 LOC)
  [ ] Implement route crystallization logic               (~40 LOC)
  [ ] Implement topology_change() with FreeWillPipeline   (~60 LOC)
  [ ] Implement lifecycle phase transitions               (~40 LOC)

Phase 3 -- Replication (n8n-rs changes):
  [ ] Implement spawn_child()                             (~50 LOC)
  [ ] Implement prune_node()                              (~30 LOC)
  [ ] Implement replicate()                               (~60 LOC)

Phase 4 -- GEL Integration:
  [ ] Implement compile_to_frame()                        (~30 LOC)
  [ ] Implement fan-out / fan-in for parallel steps       (~50 LOC)
  [ ] Implement frame result unpacking                    (~30 LOC)

Phase 5 -- Arrow Flight Actions:
  [ ] Register all DoAction handlers in do_action()       (~200 LOC)
  [ ] JSON serialization for action inputs/outputs        (~100 LOC)

Phase 6 -- Tests:
  [ ] Unit tests for Q-value routing                      (~100 LOC)
  [ ] Unit tests for FreeWillPipeline with MUL            (~100 LOC)
  [ ] Unit tests for lifecycle transitions                (~80 LOC)
  [ ] Integration test: full autopoietic loop             (~150 LOC)
  [ ] Integration test: workflow replication               (~100 LOC)

TOTAL: ~1,530 LOC new + ~50 LOC changes to existing code
```

---

*This contract specification defines every API boundary for n8n-rs
cognitive workflows. The contracts are designed for implementation
without ada-rs. n8n-rs consumes MUL state from ladybug-rs and provides
workflow execution, self-modification gating, and autopoietic lifecycle
management to crewai-rust and external clients via Arrow Flight.*
