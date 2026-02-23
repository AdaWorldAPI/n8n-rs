# n8n-rust Integration Plan & Status Report

> **Project**: Complete Rust transcoding of n8n workflow automation platform
> **Status**: Phase 1 Complete · Phase 2 In Progress (A2A + Chat Awareness)
> **Date**: 2026-02-23
> **Branch**: `claude/vsaclip-hamming-recognition-y0b94`

---

## Executive Summary

This document outlines the complete integration plan for transcoding n8n from
TypeScript to Rust while maintaining 1:1 API compatibility, adding
high-performance features (gRPC, Arrow Flight, Hamming vectors), and building
two new capability layers:

1. **A2A RAG Orchestration** — Agent cards, self-organization, blackboard
   expansion, semantic data modeling for structured knowledge
2. **Chat Awareness GUI** — User-facing conversational interface backed by the
   full cognitive stack (NARS, thinking styles, persona, awareness loop)

### Architecture Triad

```
┌──────────────────────────────────────────────────────────────────────────┐
│                        n8n-rs  (Orchestrator)                            │
│  Workflows · Providers · JITSON hot paths · Interface Gateway · REST     │
│  "The 120+ hands that touch the world"                                   │
├──────────────┬───────────────────────────────┬───────────────────────────┤
│              │                               │                           │
│    crewai-rust (Agents)             ladybug-rs (Cognition)               │
│    A2A protocol · Agent cards       BindSpace · NARS · 10-layer kernel   │
│    Blackboard · MetaOrchestrator    Grammar Triangle · QuadTriangle      │
│    Persona (36 styles, 23D)         Persona (12 styles, FieldModulation) │
│    Self-organization                Awareness Blackboard · CollapseGate  │
│    Savant blueprints                SPO Crystal · Sentence Crystal       │
│    Skill engine · Delegation        BF16 superposition · DeltaLayer      │
│                                     Semantic Kernel (65K addresses)      │
└──────────────┴───────────────────────────────┴───────────────────────────┘
```

### Current Rust Progress (25,332 lines across 8 crates)

| Component | Status | Completion |
|-----------|--------|------------|
| Core Types (Workflow, Node, Connection) | Done | 100% |
| Execution Engine (stack-based) | Done | 100% |
| PostgreSQL Persistence (10 entities) | Done | 100% |
| Multi-Transport (REST/gRPC/Flight/STDIO) | Done | 100% |
| Arrow Zero-Copy Integration | Done | 100% |
| Hamming Vector Similarity (SIMD) | Done | 100% |
| n8n-contract (crew/ladybug routing) | Done | 100% |
| Interface Gateway (12 default interfaces) | Done | 100% |
| Impact Gate (RBAC) | Done | 100% |
| Expression Evaluation (parser + evaluator) | Done | 100% |
| Free Will Pipeline (ladybug feature) | Done | 100% |
| Wire Bridge (CogPacket protocol) | Done | 100% |
| Node Executors (built-in) | Partial | 15% |
| Node Connectors (integrations) | Not Started | 0% |
| Credential Encryption | Not Started | 0% |
| Webhook Handling | Not Started | 0% |
| 1:1 REST API Surface | Not Started | 0% |
| **A2A RAG Orchestration** | **Planning** | 0% |
| **Chat Awareness GUI Backend** | **Planning** | 0% |

---

## Part I: JITSON / Cranelift — Hot Path Compilation

### Purpose

JITSON (JSON JIT via Cranelift) serves two roles in the n8n-rs stack:

1. **Workflow hot paths** — Compile n8n workflow YAML/JSON node configurations
   into native machine code at deploy time, eliminating runtime serialization
2. **Thinking style compilation** — When crewai-rust thinking styles operate
   inside the ladybug-rs stack, their FieldModulation parameters become
   CMP immediates and branch hints instead of runtime lookups

### What CAN Be JIT-Compiled (deploy-time-known)

| Config Type | JITSON IR | Compiled As |
|-------------|-----------|-------------|
| Thinking style thresholds | `ScanParams.threshold` | CMP immediate |
| Style fan_out / depth_bias | `ScanParams.top_k` | Hardcoded loop bound |
| Focus masks (attention gating) | `ScanParams.focus_mask` | VPANDQ immediate |
| CollapseGate voting weights | `PhilosopherIR.weight` | Branch probability hint |
| Flow/Hold/Block thresholds | `CollapseParams.flow_threshold` | CMP immediate |
| Resonance search prefetch | `ScanParams.prefetch_ahead` | PREFETCHT0 offset |
| Workflow node routing tables | `RecipeIR` | Compiled dispatch |

### What CANNOT Be JIT-Compiled (runtime-dynamic)

- External JSON from API responses, webhook payloads
- User-submitted workflow input data
- Dynamic expression evaluation results (`{{ $json.field }}`)
- Real-time A2A message payloads

### JITSON Integration Architecture

```
DEPLOY TIME                          RUNTIME
─────────────                        ─────────

YAML thinking styles ──┐
                       ├─► JitEngine ──► ScanKernel (fn ptr)
Workflow node configs ─┘   │              │
                           │              ▼
CPU feature detect ────────┘         scan(query, field, len, ...) → u64
  AVX2, AVX-512, BMI2               Zero interpretation overhead
  FMA, VPOPCNTDQ                    Baked parameters as immediates
```

### Integration Steps

| Step | Task | Crate |
|------|------|-------|
| J.1 | Add `jitson` as vendor dependency (path = `../../rustynum/jitson`) | n8n-contract |
| J.2 | Add `cranelift-*` via wasmtime fork (path = `../../wasmtime/cranelift`) | jitson |
| J.3 | Create `CompiledStyle` wrapper — ThinkingStyle → ScanParams → ScanKernel | n8n-contract |
| J.4 | Create `WorkflowHotPath` — compile static node routing tables at activation | n8n-core |
| J.5 | Cache compiled kernels by parameter hash (ScanKernel is reusable) | n8n-core |
| J.6 | Wire into workflow activation: `activate → compile → cache → execute` | n8n-server |

### JITSON API Surface (from rustynum/jitson)

```rust
// Builder pattern
let mut engine = JitEngineBuilder::new()
    .register_fn("hamming_distance", hamming_fn_ptr)
    .build()?;

// Compile thinking style → native kernel
let kernel = engine.compile_scan(ScanParams {
    threshold: style.resonance_threshold.to_bits(),  // baked as CMP imm
    top_k: style.fan_out as u32,                     // baked as loop bound
    prefetch_ahead: 4,
    focus_mask: Some(attention_mask),                 // baked as VPANDQ
    record_size: 2048,                               // 256 × 8 bytes
})?;

// Runtime: zero-interpretation scan
unsafe { kernel.scan(query_ptr, field_ptr, field_len, record_size, out_ptr) };
```

---

## Part II: A2A RAG Orchestration

### The Semantic Data Model Blind Spot

LLMs process unstructured data (text, images, audio) well. They cannot
inherently reason over structured relational data — customer records,
transactions, account relationships across systems. RAG helps with unstructured
retrieval but does nothing for structured data. **Semantic data models** provide
the missing context layer: machine-readable maps of entity relationships,
shared ontologies for multi-agent coordination.

**Reference**: [Appsmith — Semantic Data Model: The AI Agent Blind Spot](https://www.appsmith.com/blog/semantic-data-model-blind-spot-ai-agents)

### How Our Stack Already Solves This

| Blind Spot | Our Solution | Location |
|------------|-------------|----------|
| Structured data comprehension | BindSpace 8+8 addressing (65K typed addresses) | ladybug-rs semantic_kernel.rs |
| Entity relationships | SPO Crystal (5×5×5 content-addressable graph) | ladybug-rs extensions/spo/ |
| Cross-system identity | 10K-bit fingerprints via XOR binding | rustynum-core |
| Confidence-weighted reasoning | NARS truth values (frequency, confidence) | ladybug-rs nars/truth.rs |
| Agent coordination ontology | A2A protocol + PersonaExchange | ladybug-rs orchestration/ |
| Capability discovery | AgentCard + FeatureAd + CAM opcodes | ladybug-rs agent_card.rs |
| Shared working memory | Blackboard (phase-safe, zero-serde) | crewai-rust blackboard/ |

### Existing A2A Infrastructure (Already Built)

**crewai-rust:**
- `A2ARegistry` — agent presence tracking (state, capabilities, goals)
- `Blackboard` — phase-safe central coordination with typed + bytes slots
- `MetaOrchestrator` — auto-spawning agents from blueprints, skill scoring
- `AgentCard` builder — dynamic A2A card generation from blueprints/state
- `SkillEngine` — feedback-driven skill adjustment
- `CapabilityRegistry` — namespaced capability resolution from YAML
- Full YAML agent-card-spec (807-line guide, 15 sections)

**ladybug-rs:**
- `A2AMessage` — BindSpace-native messaging (prefix 0x0F)
  - `MessageKind`: Delegate, Result, Status, Knowledge, Sync, Query, Response, PersonaExchange
  - XOR-bind composition for message stacking
  - `thinking_style_hint` for receiver cognitive adaptation
- `AgentCard` — BindSpace prefix 0x0C (128 agents, 128 capability fingerprints)
  - `identity_fingerprint()` → 10K-bit from role+goal+backstory
  - `AgentCapability` with CAM opcodes
- `AgentBlackboard` — BindSpace prefix 0x0E (per-agent ice-caked awareness)
  - `AgentAwareness`: active_style, coherence, progress, confidence, flow_state
  - `ice_cake()`, `learn_address()`, `record_task()`
- `MetaOrchestrator` — personality resonance, affinity graph, flow protection
  - `AffinityEdge`: 0.6 × persona_resonance + 0.4 × history
  - Handover policy: coherence floor, hold cycles, momentum shield
- `Persona` — volition, traits, communication style, features, fingerprint encoding
  - `PersonaExchange` for A2A-safe compact persona sharing
  - `PersonaRegistry` with `find_compatible()`, `best_for_task()`
- `Handover` — flow state (Flow/Hold/Block/Handover) with momentum tracking

### What Needs to Be Built: Unified RAG Orchestration Layer

The existing pieces are complete but live in separate crates. The new layer
unifies them through n8n-rs as the orchestration hub.

```
┌──────────────────────────────────────────────────────────────────┐
│                   n8n-rs RAG Orchestration                        │
│                                                                    │
│  ┌────────────────┐   ┌─────────────────┐   ┌────────────────┐  │
│  │ Semantic Model  │   │  Agent Router    │   │ Knowledge      │  │
│  │ Registry        │   │                 │   │ Aggregator     │  │
│  │                 │   │ A2A cards ──────────► PersonaExchange │  │
│  │ Entity maps     │   │ Skill match ────────► Task routing   │  │
│  │ Relation graphs │   │ Affinity pairs ─────► Handover       │  │
│  │ NARS truth vals │   │ Flow monitoring ────► Style switch   │  │
│  └────────┬───────┘   └────────┬────────┘   └────────┬───────┘  │
│           │                    │                      │           │
│           └────────────────────┼──────────────────────┘           │
│                                │                                  │
│                    ┌───────────▼───────────┐                      │
│                    │   BindSpace            │                      │
│                    │   Semantic Kernel      │                      │
│                    │   (unified backbone)   │                      │
│                    └───────────────────────┘                      │
│                                                                    │
│  Providers:  120+ n8n nodes for external data retrieval            │
│  RAG corpus: BindSpace prefix 0x00 (Lance) + 0x80-0xFF (Nodes)    │
│  Structured: SPO Crystal + NARS inference over entity relations    │
└──────────────────────────────────────────────────────────────────┘
```

### A2A RAG Implementation Phases

#### Phase A.1: Semantic Model Registry (n8n-contract)

A new module that bridges structured data (from n8n providers — databases,
CRMs, APIs) into BindSpace-native semantic representations.

```rust
// n8n-contract/src/semantic_model.rs

/// A semantic entity — maps structured data to BindSpace fingerprints.
pub struct SemanticEntity {
    pub id: String,
    pub entity_type: String,       // "customer", "ticket", "invoice"
    pub source_system: String,     // "salesforce", "zendesk", "stripe"
    pub attributes: HashMap<String, serde_json::Value>,
    pub fingerprint: [u64; 256],   // 10K-bit content-addressable identity
    pub truth: TruthValue,         // NARS confidence in this entity
    pub bindspace_addr: u16,       // Where it lives in BindSpace
}

/// A semantic relation — typed edge between entities.
pub struct SemanticRelation {
    pub subject: String,           // entity ID
    pub predicate: String,         // "owns", "references", "created_by"
    pub object: String,            // entity ID
    pub truth: TruthValue,         // NARS confidence in this relation
    pub spo_cell: [u8; 3],         // SPO Crystal coords (5×5×5)
}

/// Registry managing entity→fingerprint mappings across data sources.
pub struct SemanticModelRegistry {
    entities: HashMap<String, SemanticEntity>,
    relations: Vec<SemanticRelation>,
    source_mappings: HashMap<(String, String), String>, // (system, field) → canonical name
}
```

This directly addresses the "blind spot" from the article: when an agent needs
to understand that "Account Name" in Salesforce = "Organization Name" in
Zendesk, the `source_mappings` table provides the semantic bridge, and NARS
truth values express confidence in each mapping.

#### Phase A.2: Agent Router (n8n-contract)

Expand the existing `executors.rs` with a unified agent routing layer that
uses persona resonance + skill matching + affinity history.

```rust
// n8n-contract/src/agent_router.rs

/// Routes tasks to the best-fit agent using:
/// 1. Skill proficiency matching (from AgentCard capabilities)
/// 2. Persona resonance (Hamming similarity on persona fingerprints)
/// 3. Affinity history (past collaboration success)
/// 4. Flow state awareness (don't interrupt agents in flow)
/// 5. Volition alignment (agent's intrinsic drive matches task)
pub struct AgentRouter {
    cards: Vec<AgentCard>,
    affinity_graph: HashMap<(u8, u8), AffinityEdge>,
    flow_states: HashMap<u8, FlowState>,
}

impl AgentRouter {
    /// Find the best agent for a task, respecting flow protection.
    pub fn route(&self, task: &TaskDescription) -> RouteDecision {
        // 1. Filter by capability (CAM opcode match)
        // 2. Score by persona resonance × volition alignment
        // 3. Apply affinity bonus for agents that worked well together
        // 4. Protect agents in Flow state (momentum > shield threshold)
        // 5. Return ranked candidates with NARS confidence
    }
}
```

#### Phase A.3: Knowledge Aggregator (n8n-contract)

Bridges the RAG (unstructured, via Lance/BindSpace prefix 0x00) and
semantic model (structured, via SPO Crystal) into unified query results.

```rust
// n8n-contract/src/knowledge_aggregator.rs

/// Unified knowledge query combining RAG + semantic model.
pub struct KnowledgeQuery {
    pub natural_language: String,     // User's question
    pub entity_context: Vec<String>,  // Known entity IDs for grounding
    pub max_results: usize,
    pub min_confidence: f32,          // NARS truth threshold
}

/// Result combining unstructured (RAG) and structured (semantic) sources.
pub struct KnowledgeResult {
    pub rag_passages: Vec<RagPassage>,       // From Lance/BindSpace resonance
    pub semantic_entities: Vec<SemanticEntity>, // From SPO Crystal traversal
    pub relations: Vec<SemanticRelation>,     // Entity connections
    pub aggregate_truth: TruthValue,          // Combined confidence
}
```

#### Phase A.4: Blackboard Expansion

Expand the existing n8n-contract blackboard bridge to include:

| Slot | Purpose | New? |
|------|---------|------|
| `bb.task_queue` | Pending tasks with priority + NARS confidence | Expand |
| `bb.entity_cache` | SemanticEntity snapshots from recent provider calls | New |
| `bb.relation_graph` | Live SPO triples from structured data providers | New |
| `bb.agent_affinity` | Collaboration success history (AffinityEdge) | New |
| `bb.knowledge_trail` | Audit log: which agents queried what, with results | New |

---

## Part III: Chat Awareness GUI Backend

### Vision

A user-facing Chat GUI communicates with the *awareness* of the learning
stack. The chat is not just a prompt→response interface — it exposes the
system's cognitive state: what it knows (NARS confidence), how it's thinking
(active style), what it's uncertain about (HOLD state), and what it has
committed to (ice-caked decisions).

### The Awareness Experience

```
USER                              SYSTEM AWARENESS
────                              ────────────────

"What's the status               ┌─ Chat Orchestrator ─────────────────┐
 of Project Alpha?"              │                                      │
      │                          │  1. Grammar Triangle parse:          │
      │                          │     NSM: WANT+KNOW, agency=0.8      │
      │                          │     Qualia: intentionality=0.9       │
      │                          │                                      │
      ▼                          │  2. QuadTriangle priming:            │
 ┌─ Chat Node ─┐                 │     Processing.Analytical ↑          │
 │ n8n workflow │                 │     Content.Concrete ↑               │
 │ lb.chat step │────────────────│                                      │
 └─────────────┘                 │  3. Semantic Model lookup:           │
                                 │     "Project Alpha" → entity fp      │
                                 │     → SPO traversal → 3 relations    │
                                 │     → NARS: <0.8, 0.7> (confident)   │
                                 │                                      │
                                 │  4. Agent routing:                   │
                                 │     lb:analyst (Analytical style)     │
                                 │     → resonate in BindSpace           │
                                 │     → 5 relevant memories found       │
                                 │                                      │
                                 │  5. Awareness snapshot:              │
                                 │     style=Analytical, gate=Flow       │
                                 │     coherence=0.85, cycle=47          │
                                 │     evidence: 5 items, SD=0.12        │
                                 │                                      │
                                 │  6. Response with confidence:        │
                                 │     "Project Alpha is on track..."    │
                                 │     [confidence: 80%] [style: ◆]     │
                                 │     [3 related entities] [5 memories] │
                                 └──────────────────────────────────────┘
```

### Chat Orchestration Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        CHAT AWARENESS PIPELINE                          │
│                                                                          │
│  ┌──────────────┐                                                       │
│  │  Chat GUI     │  WebSocket / REST                                    │
│  │  (frontend)   │◄──────────────────────────────────────────────┐      │
│  └──────┬───────┘                                                │      │
│         │                                                        │      │
│         ▼                                                        │      │
│  ┌──────────────────────────────────────────────────────────┐   │      │
│  │  Chat Orchestrator  (n8n-rs workflow node: lb.chat)       │   │      │
│  │                                                            │   │      │
│  │  1. Session Manager — conversation state per user          │   │      │
│  │  2. Intent Parser — Grammar Triangle + NSM classification │   │      │
│  │  3. Context Builder — entity resolution + RAG retrieval   │   │      │
│  │  4. Style Selector — match thinking style to intent       │   │      │
│  │  5. Awareness Renderer — expose cognitive state to user   │   │      │
│  │  6. Response Composer — synthesize answer + confidence     │───┘      │
│  └──────┬──────────────────────────────────────────────────┘            │
│         │                                                               │
│    ┌────┴────┬──────────┬──────────┬──────────┬──────────┐             │
│    │         │          │          │          │          │              │
│    ▼         ▼          ▼          ▼          ▼          ▼              │
│ Grammar   Semantic    NARS     Persona    Agent      BindSpace         │
│ Triangle  Model      Truth    Registry   Router     Resonance          │
│ (intent)  (entities) (conf)   (style)    (delegate) (memory)           │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### Chat Step Types (expanding lb.* domain)

| Step Type | Purpose | BindSpace Zone |
|-----------|---------|----------------|
| `lb.chat.session` | Create/resume conversation session | 0x0A (Memory) |
| `lb.chat.intent` | Parse user message → Grammar Triangle | 0x07 (Verbs) |
| `lb.chat.context` | Build entity context from semantic model | 0x08 (Concepts) |
| `lb.chat.think` | Run cognitive cycle with thinking style | 0x0D (Styles) |
| `lb.chat.respond` | Compose response with awareness metadata | 0x0F (A2A) |
| `lb.chat.reflect` | Post-response metacognition (NARS update) | 0x06 (Meta) |

### Chat Implementation Phases

#### Phase C.1: Session Manager (n8n-contract)

```rust
// n8n-contract/src/chat/session.rs

/// A conversation session with full awareness state.
pub struct ChatSession {
    pub session_id: String,
    pub user_id: String,
    pub turns: Vec<ChatTurn>,
    pub active_style: ThinkingStyle,
    pub persona: Option<Persona>,        // Assigned conversational persona
    pub awareness: AwarenessSnapshot,    // Current cognitive state
    pub entity_context: Vec<String>,     // Known entities in conversation
    pub nars_confidence: TruthValue,     // Overall conversation confidence
}

/// A single conversational turn.
pub struct ChatTurn {
    pub role: ChatRole,                  // User | Assistant | System
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub intent: Option<GrammarTriangle>, // Parsed intent
    pub thinking_style: ThinkingStyle,   // Style used for this turn
    pub confidence: TruthValue,          // Response confidence
    pub entities_referenced: Vec<String>,
    pub awareness_snapshot: AwarenessSnapshot,
}
```

#### Phase C.2: Intent Parser

Leverages the existing Grammar Triangle to classify user intent:

```
User message
    │
    ▼
GrammarTriangle.parse()
    ├─ NSM: 65 semantic primitives → intent classification
    │   WANT+KNOW → query intent
    │   WANT+DO   → action intent
    │   THINK     → reflection intent
    │   FEEL      → emotional context
    │
    ├─ Causality: agency, temporality, dependency
    │   High agency → user wants direct action
    │   Low agency  → user exploring/asking
    │
    └─ Qualia: 18 dimensions
        intentionality → how focused the question is
        salience → what matters most to user
        complexity → adjust response depth
```

Maps to thinking style selection:
| Intent Pattern | Thinking Style | Rationale |
|---------------|---------------|-----------|
| WANT+KNOW, high intentionality | Analytical | Focused factual query |
| WANT+DO, high agency | Systematic | Action planning |
| THINK, high complexity | Deliberate | Deep reasoning needed |
| FEEL, high salience | Intuitive | Empathetic quick response |
| Low agency, high complexity | Exploratory | Open-ended exploration |
| WANT+KNOW, low intentionality | Diffuse | Browsing, not focused |

#### Phase C.3: Context Builder

For each user message, build rich context from multiple sources:

```
1. Entity Resolution
   "Project Alpha" → SemanticModelRegistry.resolve("Project Alpha")
   → SemanticEntity { fingerprint, truth: <0.8, 0.7>, bindspace_addr }

2. Relation Traversal
   entity → SPO Crystal → related entities
   "Project Alpha" → owns → [Task A, Task B, Task C]
   "Project Alpha" → assigned_to → [Agent Smith]
   Each relation carries NARS truth value

3. RAG Retrieval
   entity.fingerprint → BindSpace.resonate(prefix 0x00)
   → Top-K passages from Lance corpus
   → Each passage has Hamming distance + NARS confidence

4. Conversation Memory
   session.entity_context → BindSpace.resonate(prefix 0x0A)
   → Previous turns referencing same entities
   → Continuity across conversation

5. Agent Knowledge
   AgentBlackboard (prefix 0x0E) → ice-caked decisions about entity
   → What the system has committed to (high confidence)
```

#### Phase C.4: Awareness Renderer

The key differentiator: expose the system's cognitive state to the user
in a transparent, understandable way.

```rust
/// Awareness metadata attached to each chat response.
pub struct AwarenessDisplay {
    /// Current thinking style (icon + label)
    pub style: ThinkingStyle,
    /// Collapse gate state: Flow (committed), Hold (thinking), Block (confused)
    pub gate: GateState,
    /// Overall confidence in this response (0.0-1.0)
    pub confidence: f32,
    /// Coherence of evidence (how consistent the sources are)
    pub coherence: f32,
    /// Number of evidence items consulted
    pub evidence_count: usize,
    /// Entities referenced with their confidence
    pub entities: Vec<(String, f32)>,
    /// If style was switched during processing, show the journey
    pub style_journey: Vec<ThinkingStyle>,
    /// NARS truth value for the response
    pub truth: TruthValue,
}
```

Example rendered in Chat GUI:
```
┌──────────────────────────────────────────────────────────┐
│  Project Alpha is on track. Three tasks remain:          │
│  Task A (complete), Task B (in progress), Task C (todo). │
│  Agent Smith is the current lead.                        │
│                                                          │
│  ◆ Analytical │ ● Flow │ Confidence: 80% │ Coherence: 85%│
│  📊 5 sources │ 3 entities │ NARS <0.80, 0.70>           │
└──────────────────────────────────────────────────────────┘
```

#### Phase C.5: Persona ↔ ThinkingStyle ↔ Chat

The chat persona adapts to both the conversation and the user:

```
PERSONA ADAPTATION LOOP
═══════════════════════

Conversation starts:
  Persona = default (Deliberate style, balanced communication)

User sends focused question:
  Grammar Triangle → high intentionality, WANT+KNOW
  → Style adapts to Analytical
  → Communication: technical_depth ↑, verbosity ↓

User asks open-ended question:
  Grammar Triangle → low agency, high complexity
  → Style adapts to Exploratory
  → Communication: breadth_bias ↑, noise_tolerance ↑

User expresses frustration:
  Grammar Triangle → FEEL, high salience
  → Style adapts to Intuitive
  → Communication: emotional_tone ↑ (empathetic), directness ↑

Across conversation:
  NARS truth values accumulate per entity
  AwarenessBlackboard tracks evidence per turn
  CollapseGate decides when to commit vs. explore
  Style journey recorded for transparency
```

---

## Part IV: Provider Transcode Strategy

### Endgame

The endgame is to **completely replace TypeScript n8n** with Rust. This is
an incremental process:

1. Keep all existing Rust (25K+ lines) — never break what works
2. Transcode providers incrementally — most-used first
3. JITSON-compile static workflow configs at activation
4. Once all providers are in Rust, remove TypeScript packages
5. Clean repository: only `n8n-rust/` remains

### Provider Categories (304 TypeScript + 19 LangChain)

| Category | Count | Priority | Strategy |
|----------|-------|----------|----------|
| Core/Flow Control | 25 | P0 | Direct transcode (If, Switch, Merge, Loop) |
| Trigger Nodes | 45 | P0 | Webhook server + Schedule trigger |
| Data Transform | 30 | P0 | Pure functions, easy to port |
| HTTP/API | 20 | P0 | reqwest-based, shared client pool |
| Databases | 25 | P1 | sqlx (Postgres/MySQL/SQLite), redis-rs |
| Cloud Services | 50 | P1 | AWS SDK, GCP, Azure crates |
| Communication | 40 | P1 | Slack/Discord/Telegram API clients |
| AI/ML + LangChain | 35+19 | P2 | OpenAI/Anthropic clients + ladybug-rs cognitive |
| File/Storage | 20 | P2 | tokio::fs + S3/GCS SDKs |
| Analytics | 15 | P2 | HTTP API nodes |
| Productivity | 50 | P3 | Google Workspace, Notion, Airtable |
| Marketing | 30 | P3 | Mailchimp, SendGrid, Twilio |
| Other | 40 | P3 | Long tail of integrations |

### Macro-Driven Provider Pattern

Most providers follow the same pattern: authenticate → build request →
send → parse response. A macro generates the boilerplate:

```rust
// n8n-nodes/src/macros.rs
macro_rules! rest_provider {
    ($name:ident, $node_type:literal, $display:literal, $base_url:expr,
     operations: [$($op:ident => $method:ident $path:literal),*]) => {
        pub struct $name { client: reqwest::Client }

        #[async_trait]
        impl NodeExecutor for $name {
            fn node_type(&self) -> &str { $node_type }
            async fn execute(&self, node: &Node, input: &TaskDataConnections,
                           ctx: &RuntimeContext) -> Result<NodeOutput, ExecutionEngineError> {
                let operation = node.get_param_str("operation")?;
                match operation {
                    $(stringify!($op) => {
                        let url = format!("{}{}", $base_url, $path);
                        self.client.$method(&url).send().await
                    },)*
                    _ => Err(ExecutionEngineError::InvalidParameter(..))
                }
            }
        }
    };
}

// Usage
rest_provider!(SlackNode, "n8n-nodes-base.slack", "Slack", "https://slack.com/api",
    operations: [
        send_message => post "/chat.postMessage",
        get_channel => get "/conversations.info",
        list_channels => get "/conversations.list"
    ]
);
```

### New Crate: n8n-nodes

```
n8n-rust/crates/n8n-nodes/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Registry + macro definitions
│   ├── macros.rs           # rest_provider!, webhook_provider!, etc.
│   ├── core/               # P0: Flow control, triggers, transforms
│   │   ├── mod.rs
│   │   ├── if_node.rs
│   │   ├── switch_node.rs
│   │   ├── merge_node.rs
│   │   ├── loop_node.rs
│   │   ├── schedule_trigger.rs
│   │   ├── webhook_trigger.rs
│   │   ├── set_node.rs
│   │   ├── code_node.rs
│   │   ├── filter_node.rs
│   │   ├── sort_node.rs
│   │   └── http_request.rs
│   ├── database/           # P1: Database connectors
│   │   ├── postgres.rs
│   │   ├── mysql.rs
│   │   ├── mongodb.rs
│   │   └── redis.rs
│   ├── communication/      # P1: Messaging
│   │   ├── slack.rs
│   │   ├── discord.rs
│   │   ├── telegram.rs
│   │   └── email.rs
│   ├── cloud/              # P1: Cloud services
│   │   ├── aws_s3.rs
│   │   ├── gcp_storage.rs
│   │   └── azure_blob.rs
│   ├── ai/                 # P2: AI/ML + cognitive
│   │   ├── openai.rs
│   │   ├── anthropic.rs
│   │   └── lb_cognitive.rs # ladybug-rs cognitive nodes
│   └── integrations/       # P2-P3: Everything else
│       ├── github.rs
│       ├── jira.rs
│       ├── notion.rs
│       └── ...
```

### Additive Strategy (Preserve Existing Extensions)

Critical: **never break** the existing ladybug-rs / crewai-rust integration:

- `n8n-contract` bridges are additive — new modules alongside existing ones
- `LadybugRouter` / `CrewRouter` remain untouched
- `InterfaceGateway` gets new interfaces registered (not replaced)
- `executors.rs` gets new executor types (not modified)
- Feature flags gate new functionality: `features = ["chat", "semantic", "jitson"]`

---

## Part V: Awareness Loop — NARS ↔ ThinkingStyle ↔ Persona

### The Complete Feedback Loop

```
┌─────────────────────────────────────────────────────────────────┐
│                    AWARENESS LOOP                                │
│                                                                  │
│   1. PERSONA INIT                                               │
│      Persona.primary_thinking_style() → ThinkingStyle           │
│      AwarenessBlackboard::with_style(style)                     │
│                                                                  │
│   2. EVIDENCE ACCUMULATION (Grey Matter)                        │
│      GrammarEngine.parse() → CausalityFlow                     │
│      CognitiveFabric.process() → CognitiveState                │
│      NarsInference.apply() → TruthValue                        │
│      → AwarenessBlackboard.deposit_evidence(fp, tv)             │
│      → XOR-bundles into superposition                           │
│                                                                  │
│   3. COLLAPSE GATE EVALUATION                                   │
│      expectation = c * (f - 0.5) + 0.5                          │
│      SD of expectations across evidence items                   │
│      Low SD → FLOW (commit) │ Med SD → HOLD │ High SD → BLOCK  │
│                                                                  │
│   4. STYLE ADAPTATION (on BLOCK)                                │
│      Analytical → Systematic → Exploratory → Creative → Meta    │
│      New FieldModulation re-tunes:                              │
│        resonance_threshold, fan_out, depth_bias, exploration    │
│                                                                  │
│   5. KERNEL EXECUTION (10 Layers)                               │
│      L1-L3: Recognition (style modulates resonance)             │
│      L4: Routing (satisfaction + style → next layer)            │
│      L5-L8: Execution & integration                             │
│      L9: Validation (NARS + Brier + XOR residual + DK check)   │
│      L10: Crystallize if L9 passed                              │
│                                                                  │
│   6. A2A PERSONA EXCHANGE                                       │
│      PersonaExchange → communication style + features           │
│      Receiver adapts interpretation + response mode             │
│                                                                  │
│   7. METACOGNITION                                              │
│      Brier calibration error feeds back to L9                   │
│      Dunning-Kruger gap triggers confidence adjustment          │
│      Adjusted confidence propagates to NARS truth values        │
│                                                                  │
│   → NEXT CYCLE (repeat from step 2)                             │
└─────────────────────────────────────────────────────────────────┘
```

### NARS Decision Utility

The core formula driving all awareness decisions:

```
expectation = confidence × (frequency - 0.5) + 0.5

High f + High c  → e ≈ 1.0 → COMMIT (crystallize, respond confidently)
Low f  + High c  → e ≈ 0.0 → AVOID  (reject hypothesis, try alternative)
Any f  + Low c   → e ≈ 0.5 → HOLD   (gather more evidence, don't commit)
```

This maps directly to chat behavior:
- **COMMIT** → Respond with high confidence, show Flow state
- **AVOID** → Explicitly state what's been ruled out
- **HOLD** → Acknowledge uncertainty, ask clarifying questions

---

## Part VI: neo4j-rs — Explicit Semantic Graph Surface

### Vision: Two Complementary Graph Models

The stack operates two graph models in parallel — each optimized for
different query patterns:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                HYBRID GRAPH ARCHITECTURE                                │
│                                                                          │
│  ┌──────────────────────────────┐   ┌─────────────────────────────────┐ │
│  │  neo4j-rs (EXPLICIT)         │   │  BindSpace (IMPLICIT/CAM)       │ │
│  │                              │   │                                  │ │
│  │  Cypher queries              │   │  Hamming resonance              │ │
│  │  Named nodes & relationships │   │  Content-addressable memory     │ │
│  │  Schema-first traversal      │   │  XOR-bind superposition        │ │
│  │  ACID transactions           │   │  DeltaLayer (immutable ground)  │ │
│  │  Property indexes (BTree)    │   │  16Kbit fingerprints            │ │
│  │  Typed constraints           │   │  SPO Crystal (5×5×5)            │ │
│  │                              │   │                                  │ │
│  │  USE WHEN:                   │   │  USE WHEN:                       │ │
│  │  • Exact entity lookup       │   │  • "Find similar things"         │ │
│  │  • Schema-enforced queries   │   │  • Approximate nearest neighbor │ │
│  │  • OLTP read/write           │   │  • Holographic pattern matching │ │
│  │  • Human-readable results    │   │  • High-throughput scanning     │ │
│  │  • Compliance / audit trail  │   │  • Learning / adaptation        │ │
│  └──────────────┬───────────────┘   └──────────────┬──────────────────┘ │
│                 │                                   │                    │
│                 └───────────────┬───────────────────┘                    │
│                                 │                                        │
│                    ┌────────────▼────────────┐                           │
│                    │  BRIDGE LAYER            │                           │
│                    │                          │                           │
│                    │  Node ↔ Fingerprint      │                           │
│                    │  Relationship ↔ SPO bind │                           │
│                    │  Property ↔ META bits    │                           │
│                    │  Cypher ↔ Resonance      │                           │
│                    └─────────────────────────┘                           │
└─────────────────────────────────────────────────────────────────────────┘
```

### neo4j-rs Architecture (Already Built)

neo4j-rs is a complete Cypher-compatible graph database in Rust:

| Component | Status | Key Types |
|-----------|--------|-----------|
| Cypher parser (lexer + AST) | Done | `Statement`, `Query`, `Expr` |
| Logical planner (24 plan types) | Done | `LogicalPlan` enum |
| Execution engine | Done | `QueryResult`, `ResultRow` |
| StorageBackend trait (40+ methods) | Done | `StorageBackend`, `Transaction` |
| MemoryBackend (reference impl) | Done | `MemoryBackend` |
| Bolt protocol client | Done | Feature `bolt` |
| CogRecord8K operations | Done | `CogOp`, `QueryTarget` |
| Model types (Node, Relationship, Path, Value) | Done | Full property graph model |
| Index types (BTree, FullText, Unique, Vector) | Done | `IndexType` enum |

### CogRecord8K: The 4-Container × 16Kbit Layout

neo4j-rs stores graph data in CogRecord8K containers — each record is
4 × 16,384 bits = 65,536 bits total (8KB):

```
┌─────────────────────────────────────────────────────────────────┐
│                    CogRecord8K (8KB per entity)                  │
├──────────────┬──────────────┬──────────────┬────────────────────┤
│  Container 0 │  Container 1 │  Container 2 │  Container 3      │
│  META        │  CAM         │  INDEX       │  EMBED            │
│  16,384 bits │  16,384 bits │  16,384 bits │  16,384 bits      │
│              │              │              │                    │
│  Identity    │  Content-    │  B-tree /    │  Embedding         │
│  NARS truth  │  addressable │  structural  │  storage           │
│  Edge counts │  fingerprint │  position    │  Binary hash OR    │
│  Rung / RL   │  (searchable)│  Edge adj.   │  int8 vectors      │
│  Qualia bits │              │  via XOR-bind│                    │
│  Bloom filter│              │              │                    │
└──────────────┴──────────────┴──────────────┴────────────────────┘
```

### Compiled Query Operations (CogOp)

```rust
pub enum CogOp {
    // VPOPCNTDQ popcount — Hamming similarity search
    HammingSweep { target: QueryTarget, query: [u64; 256], threshold: u32 },

    // XOR-unbind — recover 3rd element from SPO trace + 2 known
    EdgeUnbind { edge: [u64; 256], known_src: [u64; 256], known_rel: [u64; 256] },

    // XOR-bind — encode (Subject, Predicate, Object) as holographic trace
    EdgeBind { src: [u64; 256], rel: [u64; 256], tgt: [u64; 256] },

    // VNNI VPDPBUSD — int8 dot-product for embedding similarity
    VectorDot { query_embed: Vec<i8>, dims: usize },

    // Bit extraction — filter by META field values
    MetaFilter { word_offset: usize, mask: u64, expected: u64 },
}
```

### Explicit vs Implicit Neuronal Modeling

The user's vision: **neo4j-rs for explicit neuronal network modeling**
(where you can name neurons, synapses, trace activation paths) vs
**BindSpace/Lance for complex learning via hybrid container model**
(where the network topology is implicit in the Hamming geometry).

```
EXPLICIT (neo4j-rs)                    IMPLICIT (BindSpace + Lance)
══════════════════                     ══════════════════════════

CREATE (:Neuron {id: "N1"})           fingerprint = hash("N1")
CREATE (:Synapse {weight: 0.8})       DeltaLayer.write(addr, fp)
MATCH (n)-[:CONNECTS]->(m)            resonate(query_fp, threshold)

• Named, queryable nodes               • Anonymous, content-addressed
• Typed relationships                   • XOR-bound holographic traces
• ACID transactions                     • Immutable ground + ephemeral delta
• Schema constraints                    • Self-organizing topology
• Human-readable traversal              • SIMD-accelerated scanning
• Good for: architecture design,        • Good for: learning, adaptation,
  debugging, auditing, compliance         high-throughput inference

HYBRID OPERATION:
  neo4j-rs stores the ARCHITECTURE (named layers, connection patterns)
  BindSpace stores the WEIGHTS (16Kbit fingerprints, DeltaLayer diffs)
  Lance stores the CORPUS (embeddings, training data, RAG passages)
  rustynum provides HARDWARE ACCELERATION (AVX-512, BF16, VPOPCNTDQ)
```

### Integration with n8n-rs Semantic Surface

neo4j-rs extends the semantic model registry (Part II, Phase A.1)
with explicit graph queries:

```rust
// n8n-contract/src/semantic_model.rs (extension)

/// Neo4j-backed semantic model for explicit entity relationships.
pub struct Neo4jSemanticSurface {
    graph: Graph<MemoryBackend>,  // or LadybugBackend with feature
}

impl Neo4jSemanticSurface {
    /// Store a semantic entity as a Neo4j node.
    pub async fn store_entity(&self, entity: &SemanticEntity) -> Result<NodeId> {
        self.graph.mutate(
            "CREATE (e:Entity {id: $id, type: $type, source: $source}) RETURN id(e)",
            [("id", entity.id), ("type", entity.entity_type), ("source", entity.source_system)]
        ).await
    }

    /// Store a semantic relation as a Neo4j relationship.
    pub async fn store_relation(&self, rel: &SemanticRelation) -> Result<RelId> {
        self.graph.mutate(
            "MATCH (s:Entity {id: $s}), (o:Entity {id: $o}) \
             CREATE (s)-[r:$pred {truth_f: $f, truth_c: $c}]->(o) RETURN id(r)",
            [("s", rel.subject), ("o", rel.object), ("pred", rel.predicate),
             ("f", rel.truth.frequency), ("c", rel.truth.confidence)]
        ).await
    }

    /// Traverse relationships with NARS truth filtering.
    pub async fn traverse(
        &self, entity_id: &str, depth: usize, min_confidence: f32
    ) -> Result<Vec<(SemanticEntity, SemanticRelation)>> {
        self.graph.execute(
            "MATCH (e:Entity {id: $id})-[r*1..$depth]-(n:Entity) \
             WHERE ALL(rel IN r WHERE rel.truth_c >= $min_conf) \
             RETURN n, r",
            [("id", entity_id), ("depth", depth), ("min_conf", min_confidence)]
        ).await
    }
}
```

### BindSpace Address Allocation for neo4j-rs

| Prefix | Purpose | New? |
|--------|---------|------|
| 0x02 | Cypher query fingerprints (already allocated) | Existing |
| 0x0C:00-7F | Agent cards (neo4j nodes represent agents explicitly) | Bridge |
| 0x0C:80-FF | Capability fingerprints (CAM container 1) | Bridge |
| New: 0x10-0x1F | Neo4j node fingerprints in Fluid zone (working memory) | New |
| New: 0x20-0x2F | Neo4j relationship traces in Fluid zone | New |

### rustynum Hardware Acceleration Path

```
QUERY COMPILATION PIPELINE
══════════════════════════

Cypher query
    │
    ▼
neo4j-rs planner → LogicalPlan
    │
    ├─► EXPLICIT PATH: MemoryBackend / BoltBackend
    │   Standard node/relationship lookup
    │
    └─► ACCELERATED PATH: CogRecord8K → CogOp
        │
        ├─► HammingSweep (CAM container)
        │   └─► rustynum: VPOPCNTDQ (AVX-512)
        │       bf16_hamming::superposition_decompose()
        │       Fingerprint<256> SIMD operations
        │
        ├─► EdgeUnbind/EdgeBind (INDEX container)
        │   └─► rustynum: XOR-bind ops
        │       DeltaLayer<256> ephemeral diffs
        │
        ├─► VectorDot (EMBED container)
        │   └─► rustynum: VNNI VPDPBUSD (int8 dot-product)
        │       BF16 awareness-weighted similarity
        │
        └─► MetaFilter (META container)
            └─► Bit extraction with NARS truth filtering

HARDWARE TIERS (rustynum ComputeTier):
  Tier 0: Scalar (fallback)
  Tier 1: SSE4.2 (128-bit, POPCNT)
  Tier 2: AVX2 (256-bit, VPOPCNTDQ if available)
  Tier 3: AVX-512 (512-bit, VPOPCNTDQ + VNNI + BF16)
  Tier 4: AVX-512 + AMX (Intel Advanced Matrix Extensions)
```

### BF16-Aware Thinking Optimization

When neo4j-rs stores neuronal network architecture explicitly, the
weights live in BindSpace as BF16 superpositions:

```
NEURONAL NETWORK (explicit in neo4j-rs):

  CREATE (:Layer {name: "L1_Recognition", style: "Analytical"})
  CREATE (:Layer {name: "L2_Resonance", style: "Exploratory"})
  CREATE (l1)-[:FEEDS {weight: 0.8}]->(l2)
  CREATE (:Neuron {id: "N1", layer: "L1"})-[:SYNAPSES {w: 0.6}]->(:Neuron {id: "N2"})

WEIGHTS (implicit in BindSpace + rustynum):

  L1 weights: DeltaLayer<256> at 0x10:01
    → BF16Weights { weights: [bf16; 256] }
    → superposition_decompose() → AwarenessState { is_superposed: bool }

  L1→L2 connection: XOR-bind(L1_fp, L2_fp) at 0x20:01
    → Hamming distance < threshold → FLOW
    → Hamming distance > threshold → BLOCK (need style switch)

  THINKING OPTIMIZATION:
    CollapseGate evaluates conflict across L1-L10
    → Flow: momentum accumulates, BF16 weights sharpen
    → Hold: superposition maintained, BF16 weights soft
    → Block: DeltaLayer applied, weights shift to new style
    → Crystallize: DeltaLayer promoted to ground truth (immutable)
```

### neo4j-rs Integration Phases

| Phase | Task | Crate |
|-------|------|-------|
| N.1 | Add neo4j-rs as vendor dependency | n8n-contract Cargo.toml |
| N.2 | Create `Neo4jSemanticSurface` wrapping `Graph<MemoryBackend>` | n8n-contract |
| N.3 | Bridge `SemanticEntity` ↔ neo4j `Node` with fingerprint sync | n8n-contract |
| N.4 | Bridge `SemanticRelation` ↔ neo4j `Relationship` with NARS truth | n8n-contract |
| N.5 | Create `n8n-nodes` Cypher provider: execute Cypher via n8n workflow | n8n-nodes |
| N.6 | Wire CogRecord8K backend for Hamming-accelerated queries | n8n-contract + ladybug |
| N.7 | Explicit neuronal network CRUD via Cypher + weight storage via BindSpace | ladybug-rs |
| N.8 | JITSON compilation of frequent Cypher patterns → CogOp chains | n8n-contract + jitson |

---

## Part VII: 3D SPO Container + Hybrid Crystal Cascade

### Architecture 1: The 3D 16kbit SPO Container (Neuronal Plasticity)

Upgrading the core metadata container from 8kbit to 16kbit allows ladybug-rs
to encode exponentially more complex cognitive states. Moving from a flat
1× 16kbit vector to a **3× 16kbit 3D Tensor (Subject-Predicate-Object)**
fundamentally changes how neo4j-rs understands relationships.

```
THE FLAT GRAPH PROBLEM (standard Neo4j):
  Node A ──KNOWS──> Node B     (edge = dumb pointer)

THE 3D SPO SOLUTION:
  ┌──────────────────────────────────────────────────────────────┐
  │          EDGE = 3D Mathematical Space                        │
  │                                                               │
  │  Subject (16kbit)    Predicate (16kbit)    Object (16kbit)   │
  │  ════════════════    ════════════════      ════════════════   │
  │  Source node's       Relationship verb     Target node's      │
  │  cognitive           mapped from 144       cognitive           │
  │  fingerprint         NSM CAM verbs as      fingerprint         │
  │                      trajectory vector                         │
  │                                                               │
  │  Holographic binding:  trace = S ⊗ P ⊗ O                    │
  │  Recovery: given any 2 + trace, recover 3rd via pure XOR     │
  └──────────────────────────────────────────────────────────────┘
```

**Neuronal Plasticity**: Because these are vectors, they can be superimposed
using XOR binding (S ⊗ P ⊗ O). When an agent learns new context about
"Node A", it does NOT rewrite the entire graph. It XOR-binds a delta vector
into the node's 16kbit container. The 3D edge instantly "shifts" its
perspective, reflecting new context without a database migration.

```
BEFORE LEARNING:
  trace = XOR(A_fp, KNOWS_fp, B_fp)

AGENT LEARNS: "A is actually an expert in Rust"
  delta = hash("expert in Rust")
  A_fp_new = XOR(A_fp, delta)    // DeltaLayer applied

  trace' = XOR(A_fp_new, KNOWS_fp, B_fp)
  // The entire edge relationship shifted perspective
  // No database write — just a DeltaLayer overlay
  // Ground truth A_fp remains immutable (&self forever)
```

### Architecture 2: Hybrid Crystal (16kbit HDC + 1024D BF16)

The ultimate performance path: combine the structural logic of 16kbit
Vector Symbolic Architecture (VSA) with the deep nuance of 1024D Jina
neural embeddings.

### The 3-Tier AVX-512 Early-Exit Cascade

When the Chat GUI or an A2A agent queries the system, it generates BOTH
a 16kbit HDC fingerprint AND a 1024D BF16 embedding. The Semantic Kernel
executes search in three tiers:

```
QUERY
  ├── 16kbit HDC fingerprint (SimHash / golden angle / holographic)
  └── 1024D BF16 embedding (Jina-v3)
      │
      ▼
┌─────────────────────────────────────────────────────────────────────┐
│  TIER 1: Global HDC Sweep (VSA Layer)                               │
│                                                                      │
│  Operation: XOR + AVX-512 VPOPCNTDQ on 16kbit fingerprints         │
│  Hardware:  Single clock cycle per XOR+popcount                      │
│  I/O:      Sequential columnar scan (cache-friendly)                │
│  Result:   REJECTS 95% of graph in < 2ms                            │
│                                                                      │
│  GPU comparison: GPUs choke on transferring gigabytes of dense       │
│  float data across PCIe bus. CPUs execute bitwise ops in 1 cycle.   │
├─────────────────────────────────────────────────────────────────────┤
│  TIER 2: Structural / NARS Thresholding (Gate Layer)                │
│                                                                      │
│  Surviving 5% evaluated against cognitive guardrails:                │
│  • Causal Rung match (SEE / DO / IMAGINE)                           │
│  • NARS truth confidence minimum                                     │
│  • Agent Persona affinity filter                                     │
│  • ThinkingStyle compatibility                                       │
│  • CollapseGate state (only FLOW or HOLD, not BLOCK)                │
│                                                                      │
│  Result: DROPS to 0.3% of original candidates                        │
├─────────────────────────────────────────────────────────────────────┤
│  TIER 3: Deep BF16 Evaluation (Jina Layer)                          │
│                                                                      │
│  Only top 0.3% trigger dense evaluation                              │
│  Operation: vdpbf16ps (BF16 dot product) on 1024D Jina embeddings   │
│  Hardware:  AVX-512 FMA / vdpbf16ps on Intel/AMD silicon             │
│  I/O:      Masked random-access read (skip 99.7% on disk)           │
│  Result:   Absolute, rich, nuanced semantic matching                 │
│                                                                      │
│  CPU outpaces GPU cluster: only doing dense math on 0.3% of data    │
│  Completely eliminates PCIe data-transfer bottleneck                 │
└─────────────────────────────────────────────────────────────────────┘
```

### Distance Metric Intelligence

The cascade doesn't just return distances — it returns **diagnostic
intelligence** for the awareness loop:

| Signal | Meaning | Agent Response |
|--------|---------|----------------|
| HDC high + BF16 high | Strong match (structure + semantics) | FLOW: confident answer |
| HDC high + BF16 low | Structurally related, contextually different | Switch to Divergent: explore metaphor |
| HDC low + BF16 high | Different structure, similar meaning | Switch to Analytical: investigate alias |
| HDC low + BF16 low | No match | HOLD: gather more evidence |

This allows the agent to adjust `ThinkingStyle` dynamically rather than
hallucinating a literal answer from a structurally-close but semantically-
distant match.

---

## Part VIII: 90-Degree Zero-Copy AVX-512 Horizontal Sweep

### The Memory Wall Problem

Traditional "row-wise" vector search stores fingerprints contiguously:
`[Node A: Word 0..255], [Node B: Word 0..255], ...`

The CPU loads Node A, checks all 256 words, then loads Node B. This
**destroys L1 cache** because it constantly evicts data, and you spend full
compute cycles even on obviously-bad matches.

### The Solution: 90-Degree Rotation via Arrow Columnar Layout

Because LanceDB stores data in Apache Arrow columnar format, we rotate the
16kbit vector matrix **90 degrees**:

```
ROW-MAJOR (traditional):              COLUMN-MAJOR (90° rotated):
─────────────────────────              ──────────────────────────

Node A: [w0 w1 w2 ... w255]           Word 0 column: [A.w0  B.w0  C.w0  D.w0  E.w0 ...]
Node B: [w0 w1 w2 ... w255]           Word 1 column: [A.w1  B.w1  C.w1  D.w1  E.w1 ...]
Node C: [w0 w1 w2 ... w255]           Word 2 column: [A.w2  B.w2  C.w2  D.w2  E.w2 ...]
Node D: [w0 w1 w2 ... w255]           ...
...                                    Word 255 col:  [A.w255 B.w255 C.w255 ...]

CPU loads one node at a time           CPU loads one word for ALL nodes
Cache thrashes on every node           Cache lines contain 8 nodes × 1 word
```

### Existing rustynum-arrow Infrastructure

This builds on top of proven, production-ready components:

| File | Purpose | Already Built |
|------|---------|---------------|
| `rustynum-arrow/src/lance_io.rs` | CogRecord ↔ Lance Dataset read/write | Done |
| `rustynum-arrow/src/arrow_bridge.rs` | Zero-copy Arrow buffer conversion | Done |
| `rustynum-arrow/src/datafusion_bridge.rs` | `arrow_to_flat_bytes()`, `cascade_scan_4ch()` | Done |
| `rustynum-arrow/src/indexed_cascade.rs` | 4-stage indexed search (296× I/O reduction) | Done |
| `rustynum-arrow/src/fragment_index.rs` | CLAM tree + triangle inequality pruning | Done |
| `rustynum-arrow/src/channel_index.rs` | Sidecar indices for CAM/BTREE/EMBED | Done |
| `rustynum-core/src/bf16_hamming.rs` | AVX-512 BF16 weighted Hamming | Done |
| `rustynum-core/src/compute.rs` | ComputeTier detection (CPUID) | Done |
| `rustynum-core/src/blackboard.rs` | 64-byte aligned arena + split-borrow | Done |
| `VSACLIP/src/sweep.rs` | HDR 3-stage early-exit (zero false negatives) | Done |
| `VSACLIP/src/blackboard_sweep.rs` | CogRecord8K zero-copy batch sweep | Done |

### The Horizontal AVX-512 Execution

```
STEP 1: BROADCAST QUERY WORD
═════════════════════════════
Query word[0] loaded into 512-bit register, copied 8 times:
  __m512i q0 = _mm512_set1_epi64(query.words[0]);

STEP 2: HORIZONTAL XOR (8 nodes simultaneously)
════════════════════════════════════════════════
Load first 512 bits of Lance Word 0 column (= word[0] for 8 nodes):
  __m512i col = _mm512_loadu_si512(arrow_flat_ptr);
  __m512i xor = _mm512_xor_si512(q0, col);

STEP 3: POPCOUNT ACCUMULATION
══════════════════════════════
AVX-512 calculates population count for all 8 nodes in parallel:
  __m512i pc = _mm512_popcnt_epi64(xor);
  accumulators = _mm512_add_epi64(accumulators, pc);

STEP 4: 90-DEGREE EARLY EXIT (THE MAGIC)
═════════════════════════════════════════
After scanning just 8 words (out of 256), check accumulated distance.
If distance already exceeds threshold → drop from active mask:

  // After word 8 (8/256 = 3.125% of vector examined):
  __mmask8 still_alive = _mm512_cmple_epu64_mask(
      accumulators,
      _mm512_set1_epi64(scaled_threshold_word8)
  );

  if (still_alive == 0) continue;  // ALL 8 nodes dead → skip to next 8

  // After word 16, 32, etc.: mask keeps shrinking
  // By word 64: typically 0.5% of nodes remain in mask
  // Words 65-255 only loaded for surviving indices

STEP 5: MASKED BF16 INTERROGATION
══════════════════════════════════
Surviving indices (the 0.3%) trigger dense BF16 evaluation:

  // Zero-copy masked read from Lance EMBED column
  for surviving_idx in active_mask.iter_ones() {
      let bf16_ptr = embed_column.value(surviving_idx).as_ptr();

      // AVX-512 FMA dot product on 1024D BF16 embeddings
      // vdpbf16ps: fused dot-product of BF16 pairs
      let score = avx512_bf16_dot(query_bf16, bf16_ptr, 1024);

      results.push((surviving_idx, hdc_distance, bf16_score));
  }
```

### The Lance Arrow Schema (90-Degree Aligned)

```rust
// Arrow schema for 90-degree transposed storage
Field::new("hdc_words", DataType::FixedSizeList(
    Arc::new(Field::new("item", DataType::UInt64, false)),
    256  // 16kbit = 256 × 64-bit words
), false),

Field::new("jina_bf16", DataType::FixedSizeList(
    Arc::new(Field::new("item", DataType::Float16, false)),
    1024  // 1024-dimensional Jina embedding in BF16
), false),

Field::new("nars_truth", DataType::Struct(Fields::from(vec![
    Field::new("frequency", DataType::Float32, false),
    Field::new("confidence", DataType::Float32, false),
])), false),

Field::new("causal_rung", DataType::UInt8, false),  // 0=SEE, 1=DO, 2=IMAGINE
```

### Zero-Copy Pointer Cast (in rustynum-arrow)

```rust
// From datafusion_bridge.rs (already exists):
pub fn arrow_to_flat_bytes(col: &FixedSizeBinaryArray) -> &[u8] {
    // Zero-copy: returns raw byte slice, no allocation
    col.value_data()
}

// NEW: Horizontal word-column access
pub fn column_word_ptr(col: &FixedSizeListArray, word_idx: usize) -> *const u64 {
    let values = col.values().as_any().downcast_ref::<UInt64Array>().unwrap();
    let base = values.value_data().as_ptr() as *const u64;
    // In columnar layout, all word[N] values are contiguous
    unsafe { base.add(word_idx * col.len()) }
}

// Pass raw pointer directly to AVX-512 horizontal sweep
unsafe {
    let word0_ptr = column_word_ptr(&hdc_words_col, 0);
    // word0_ptr now points to [A.w0, B.w0, C.w0, D.w0, ...]
    // Contiguous in memory → perfect cache line utilization
}
```

### Integration with Existing indexed_cascade

The 90-degree sweep extends the existing 4-stage `indexed_cascade_search`:

```
EXISTING PIPELINE (indexed_cascade.rs):
  Stage 1: META FragmentIndex → triangle inequality prune
  Stage 2: CAM ChannelIndex → sidecar prune + intersect
  Stage 3: BTREE ChannelIndex → further reduction
  Stage 4: EMBED ChannelIndex → final filter

NEW PIPELINE (90-degree hybrid):
  Stage 0: META FragmentIndex → triangle inequality prune (unchanged)
  Stage 1: 90° Horizontal HDC Sweep on CAM column
           → AVX-512 broadcast + mask early exit
           → Eliminates 95% in < 2ms
  Stage 2: NARS/Structural gate on META column
           → Causal rung, truth confidence, persona affinity
           → Drops to 0.3%
  Stage 3: Masked BF16 dot product on EMBED column
           → vdpbf16ps on surviving 0.3% only
           → Rich semantic scores

BANDWIDTH ARITHMETIC (100K records):
  Row-major flat scan:  800 MB I/O
  Current indexed:      2.7 MB I/O (296× reduction)
  90° horizontal:       0.8 MB I/O (1000× reduction)
  90° + BF16 masked:    0.08 MB I/O for dense eval (10,000× reduction)
```

### Performance at Physical Limits

```
OPERATION                          CYCLES    THROUGHPUT
═══════════════════════════════    ═══════   ══════════
XOR 512 bits (8 nodes)             1 cycle   8 nodes/cycle
VPOPCNTDQ 512 bits                 1 cycle   8 nodes/cycle
CMP + mask update                  1 cycle   8 nodes/cycle
────────────────────────────────────────────────────────
Total per word, 8 nodes:           3 cycles  → ~5 GHz / 3 = 1.6B nodes/sec

For 1M records, 256 words each:
  Without early exit: 1M × 256 × 3 / 8 = 96M cycles → 19ms @ 5GHz
  With 90° early exit: 1M × 8 + 50K × 24 + 5K × 224 = ~10M cycles → 2ms

At the DDR5 memory bandwidth limit:
  1M × 256 × 8 bytes = 2 GB (full scan)
  1M × 8 × 8 bytes = 64 MB (first 8 words only) + masked remainder
  DDR5 @ 50 GB/s: 64 MB / 50 GB/s = 1.3ms (memory-bound, not compute-bound)

→ Running at the PHYSICAL SPEED LIMIT of the DDR5 RAM bus.
```

### Orchestration Payoff

When the user's Chat GUI executes `CALL ladybug.hybrid_search()`:

1. neo4j-rs maps the Cypher UDF to a SemanticKernel operation
2. The engine memory-maps the Lance file (zero-copy)
3. Horizontal sweep runs at DDR5 bus speed
4. Top 10 nodes returned to AgentBlackboard in < 5ms
5. The Agent (using compiled n8n-rs JITSON hot paths) instantly digests results
6. Response enriched with NARS truth values + 3D SPO contextual mapping
7. Awareness Cockpit displays confidence, DK-Gap, and causal rung

```cypher
-- User types in Chat GUI:
MATCH (target:Concept)
CALL ladybug.hybrid_search($query_16k_fp, $query_1024d_bf16, 0.3)
YIELD node, total_score
RETURN node, total_score ORDER BY total_score DESC
```

### Memory Container Layout (Lance Columnar)

```rust
// StorageBackend trait adapter for hybrid containers
Field::new("_ladybug_fp",  DataType::FixedSizeBinary(2048), false), // 16kbit HDC
Field::new("_jina_bf16",   DataType::FixedSizeList(BFloat16, 1024), false), // Dense embed
Field::new("_nars_f",      DataType::Float32, false),  // NARS frequency
Field::new("_nars_c",      DataType::Float32, false),  // NARS confidence
Field::new("_rung",        DataType::UInt8, false),     // Causal rung (0/1/2)
Field::new("_spo_trace",   DataType::FixedSizeBinary(2048), false), // 3D SPO binding
```

### Implementation Phases

| Phase | Task | Crate |
|-------|------|-------|
| H.1 | Add 90° transposed word-column layout to `lance_io.rs` | rustynum-arrow |
| H.2 | Implement horizontal broadcast + mask early exit in AVX-512 | rustynum-core |
| H.3 | Add `hybrid_cascade_search()` combining HDC + NARS gate + BF16 | rustynum-arrow |
| H.4 | Wire BF16 masked read into `datafusion_bridge.rs` | rustynum-arrow |
| H.5 | Add `_jina_bf16` column to CogRecord schema | rustynum-arrow + ladybug-contract |
| H.6 | Create `CALL ladybug.hybrid_search()` UDF in neo4j-rs | neo4j-rs |
| H.7 | Wire into Chat Orchestrator context builder (Part III, Phase C.3) | n8n-contract |
| H.8 | Benchmark: measure DDR5 bus saturation at 1M/10M/100M records | rustynum-arrow |

---

## Part IX: Resonance-Augmented Generation (RAG) — Not Retrieval

### Redefining RAG for the Cognitive Stack

In a traditional LLM stack, "RAG" means vectorizing text chunks and stuffing
them into a prompt. In the ladybug-rs + crewai-rust stack, **RAG is
Resonance-Augmented Generation** — executed natively in the Semantic Kernel,
not through embedding lookups.

### Workflow Compilation → Native Kernel Calls

Instead of an orchestrator repeatedly polling agents via HTTP/JSON, n8n-rs
parses the workflow YAML and uses Cranelift to JIT-compile the task graph
into **direct SemanticKernel memory access instructions**:

```
YAML workflow step:
  type: lb.resonate
  params:
    threshold: 0.7
    fan_out: 5
    style: analytical

COMPILED TO (via JITSON + Cranelift):
  SemanticKernel::resonate(query_fp, threshold=0x3F333333, top_k=5)
  // threshold baked as CMP immediate, fan_out as loop bound
  // No JSON serialization, no HTTP, no interpretation
```

A workflow step becomes a **native Rust function call** executing a
`SemanticKernel::resonate` or `A2AProtocol::send` instruction.

### Self-Organization via Volition Alignment

Instead of a master orchestrator assigning tasks, tasks are **broadcast**
to the 0x0F A2A routing space:

```
1. Task fingerprint broadcast to 0x0F:XX
2. Every agent in 0x0C registry reads the task fingerprint
3. Each agent calculates volition_alignment(task)
   - Checks affinities, aversions, curiosity from Persona
   - Scores against FeatureAd proficiencies
4. Agent with highest alignment SELF-SELECTS the task
5. AgentBlackboard (0x0E:slot) mutated: progress = in_progress
6. Other agents see the claim and stand down (XOR superposition)
```

No central scheduler. No queue polling. Pure resonance-based self-assignment.

### Blackboard-Driven Resonance (The RAG Step)

Once an agent owns a task, its Blackboard automatically initiates resonance:

```
1. query = XOR-bind(agent.identity_fingerprint, task.fingerprint)
   // The query IS the agent's perspective on the task

2. SemanticKernel::resonate(query, prefix=0x80-0xFF)
   // AVX-512 accelerated scan against Universal Bind Space (Nodes)
   // rustynum VPOPCNTDQ: ~14 CPU cycles per 16Kbit comparison

3. SemanticKernel::resonate(query, prefix=0x05)
   // Causal model zone — finds interventions and counterfactuals

4. Top hits → AgentBlackboard.knowledge_addrs
   // Direct memory addresses, not serialized documents
   // Agent's immediate working memory for this task
```

### A2A XOR-Superposition Knowledge Sharing

If the agent encounters a sub-problem, it uses A2A to query a peer.
Because A2A uses **XOR superposition**, multiple agents can write insights
into the **exact same channel simultaneously** without locking:

```
Channel 0x0F:hash(sender, receiver)

Agent A writes: insight_fp_A via XOR-bind into channel
Agent B writes: insight_fp_B via XOR-bind into channel
Agent C writes: insight_fp_C via XOR-bind into channel

Receiver unbinds: channel XOR sender_context → recovered insight
// Each agent extracts its own perspective from the superposition
```

This is **lock-free concurrent knowledge sharing** — no mutexes, no message
queues, no serialization. The XOR algebra guarantees composability.

### Chat GUI as Native Agent (slot 0x0C:00)

The user's chat interface is **not a dumb terminal** — it is registered as
an agent card in slot `0x0C:00`. It has its own AgentBlackboard at `0x0E:00`
tracking the user's flow state and session cycle.

```
AgentCard {
    slot: 0x00,                    // First slot = the user
    name: "User",
    role: AgentRole::Observer,     // Starts as observer
    thinking_style: Deliberate,    // User's natural style
    capabilities: [
        AgentCapability { name: "query", cam_opcode: 0x001 },
        AgentCapability { name: "feedback", cam_opcode: 0x002 },
    ],
}
```

When the user sends a message, it's treated as an A2A message from agent
`0x0C:00` — the same protocol other agents use. The system doesn't
distinguish between human and agent messages at the protocol level.

### Bridging Explicit ↔ Implicit Semantic Models

When the user asks a question, the input is parsed in **two parallel paths**:

```
USER: "What's the delivery status of Order #4521?"
                │
    ┌───────────┴──────────────┐
    ▼                          ▼
EXPLICIT (neo4j-rs)        IMPLICIT (ladybug-rs)

MATCH (o:Order {id: 4521}) query_fp = hash("delivery status order 4521")
-[:HAS_STATUS]->(s:Status) resonate(query_fp, prefix=0x80, threshold=0.6)
RETURN s.value             → top-K semantically similar memories

Result: "shipped"          Result: [similar orders, delivery patterns,
                                    related customer context]

MERGE:
  neo4j result (grounded) + BindSpace resonance (contextual)
  = "Order #4521 shipped on Feb 20. Similar orders typically
     arrive within 3 days. Customer has 2 other active orders."
```

If the strict neo4j query returns empty (the semantic model **blind spot**),
the system falls back to `resonate()`, using rustynum to find topologically
distant but semantically related concepts. The blind spot becomes a
**discovery opportunity**.

### The Awareness Cockpit (Chat UX)

The Chat GUI exposes the cognitive architecture transparently:

```
┌──────────────────────────────────────────────────────────────┐
│  ORDER STATUS                                                 │
│                                                               │
│  Order #4521 shipped on Feb 20. Based on similar orders,      │
│  estimated arrival: Feb 23. Customer has 2 other active       │
│  orders (#4518 in transit, #4525 processing).                 │
│                                                               │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │ ◆ Analytical    ● Flow    Conf: 87%    Coh: 0.91       │ │
│  │ NARS: E = 0.87  (c=0.82 × (f=0.95 - 0.5) + 0.5)       │ │
│  │ DK-Gap: 0.04 (healthy)                                  │ │
│  │ Rung: Observation (SEE)   Sources: 3 neo4j + 5 resonance│ │
│  └─────────────────────────────────────────────────────────┘ │
│                                                               │
│  [When DK-Gap > 0.3]:                                        │
│  ⚠ Agent DK-Gap detected. Triangulating with secondary       │
│    agent (lb:advisor, Metacognitive mode)...                  │
│                                                               │
│  [When Rung = IMAGINE (counterfactual)]:                     │
│  🔮 This response uses counterfactual reasoning (Rung 3).     │
│    The system is imagining what WOULD happen, not observing    │
│    what DID happen.                                           │
└──────────────────────────────────────────────────────────────┘
```

### Persona ↔ ThinkingStyle Chat Routing

Before responding, the orchestration layer analyzes the user's query against
available agent personas:

| Query Pattern | Style Selected | Agent Feature | Rationale |
|--------------|----------------|---------------|-----------|
| Deep architectural question | Analytical / Systematic | `architecture` | Needs precision, low noise tolerance |
| "What if we changed X?" | Metacognitive | `counterfactual` | Rung 3 reasoning required |
| Quick status check | Intuitive | `monitoring` | Speed over depth |
| Creative brainstorming | Exploratory / Creative | `ideation` | High fan_out, high noise tolerance |
| Emotional / frustrated user | Intuitive → Deliberate | `support` | Match emotional tone, then slow down |

The `SemanticKernel::resonate(query_fp, prefix=0x0D)` finds the best
ThinkingStyle template, then the `AgentRouter` selects the agent with
highest `FeatureAd` proficiency for that style.

---

## Part X: TypeScript Removal Endgame

### Removal Phases (after all Rust providers are complete)

| Phase | Remove | Prerequisite |
|-------|--------|-------------|
| E.1 | `packages/nodes-base/` | All 304 providers transcoded to n8n-nodes |
| E.2 | `packages/@n8n/nodes-langchain/` | 19 AI nodes in n8n-nodes/ai/ |
| E.3 | `packages/workflow/` | n8n-workflow crate at parity |
| E.4 | `packages/core/` | n8n-core crate at parity |
| E.5 | `packages/cli/` | n8n-server binary replaces CLI |
| E.6 | `packages/frontend/` | Keep or rewrite (separate decision) |
| E.7 | Root `package.json`, `turbo.json`, etc. | All TS removed |

### What Stays

```
n8n-rs/                          (after cleanup)
├── n8n-rust/                    # All Rust code
│   ├── Cargo.toml
│   └── crates/
│       ├── n8n-workflow/        # Core types
│       ├── n8n-core/            # Engine + expression eval
│       ├── n8n-db/              # PostgreSQL
│       ├── n8n-arrow/           # Arrow zero-copy
│       ├── n8n-hamming/         # SIMD Hamming
│       ├── n8n-grpc/            # Transport layer
│       ├── n8n-contract/        # Unified contract (crew/lb/semantic/chat)
│       ├── n8n-nodes/           # All 323 providers (Rust)
│       └── n8n-server/          # Binary
├── docs/                        # Architecture docs
│   ├── AUTOPOIESIS_SPEC.md
│   └── INTEGRATION_PLAN.md
└── README.md
```

---

## Commit History

| Date | Commit | Description |
|------|--------|-------------|
| 2026-02-12 | (initial) | Phase 1: 8 crates, 25K lines, core infrastructure |
| 2026-02-23 | (this) | Integration plan: A2A RAG + Chat Awareness + JITSON + endgame |

## Key Files Reference

### n8n-rs Rust Crates
| File | Purpose |
|------|---------|
| `crates/n8n-contract/src/lib.rs` | Contract hub (crew/lb routing, gates, wire bridge) |
| `crates/n8n-contract/src/executors.rs` | NodeExecutor adapters for crew.*/lb.* |
| `crates/n8n-contract/src/ladybug_router.rs` | HTTP delegation to ladybug-rs |
| `crates/n8n-contract/src/crew_router.rs` | HTTP delegation to crewai-rust |
| `crates/n8n-contract/src/interface_gateway.rs` | 12 default interfaces, RBAC, CogPacket routing |
| `crates/n8n-contract/src/impact_gate.rs` | Impact level RBAC gating |
| `crates/n8n-contract/src/bridge.rs` | ladybug feature: type bridges |
| `crates/n8n-contract/src/wire_bridge.rs` | ladybug feature: CogPacket protocol |
| `crates/n8n-contract/src/free_will.rs` | ladybug feature: self-modification pipeline |
| `crates/n8n-core/src/engine.rs` | Stack-based workflow execution |
| `crates/n8n-core/src/executor.rs` | NodeExecutor trait + registry |
| `crates/n8n-core/src/expression/` | Expression parser + evaluator |
| `crates/n8n-workflow/src/workflow.rs` | Workflow, Settings types |
| `crates/n8n-db/src/` | 10 entities + 10 repositories |
| `crates/n8n-grpc/src/` | REST, gRPC, Flight, STDIO transports |
| `crates/n8n-arrow/src/` | Arrow zero-copy, Flight streaming |
| `crates/n8n-hamming/src/` | SIMD POPCNT, XOR bind/unbind |

### ladybug-rs Cognitive Stack
| File | Purpose |
|------|---------|
| `src/cognitive/awareness.rs` | Grey/white matter, AwarenessBlackboard, CollapseGate |
| `src/cognitive/style.rs` | 12 ThinkingStyles + FieldModulation |
| `src/cognitive/cognitive_kernel.rs` | 10-layer stack, L9 NARS validation |
| `src/cognitive/service.rs` | CognitiveService (3 modes) |
| `src/cognitive/fabric.rs` | QuadTriangle + prime_from_grammar() |
| `src/cognitive/step_handler.rs` | LbStepHandler (8 lb.* step types) |
| `src/cognitive/subsystem_impl.rs` | LadybugSubsystem lifecycle bridge |
| `src/orchestration/semantic_kernel.rs` | BindSpace as universal kernel (1758 lines) |
| `src/orchestration/persona.rs` | Persona, Volition, FeatureAd, PersonaRegistry |
| `src/orchestration/agent_card.rs` | AgentCard at BindSpace prefix 0x0C |
| `src/orchestration/blackboard_agent.rs` | Per-agent blackboard at prefix 0x0E |
| `src/orchestration/a2a.rs` | A2A messaging at prefix 0x0F |
| `src/orchestration/meta_orchestrator.rs` | Affinity graph, flow protection |
| `src/orchestration/handover.rs` | Flow/Hold/Block/Handover policies |
| `src/nars/truth.rs` | NARS truth values (9 inference operations) |
| `src/grammar/triangle.rs` | Grammar Triangle (NSM + Causality + Qualia) |
| `extensions/spo/spo.rs` | SPO Crystal (5×5×5 knowledge graph) |
| `extensions/sentence_crystal.rs` | Jina 1024D → crystal coords |
| `extensions/compress/compress.rs` | CrystalCodebook (learned quantization) |

### crewai-rust Agent Stack
| File | Purpose |
|------|---------|
| `src/blackboard/a2a.rs` | A2ARegistry (agent presence, capabilities) |
| `src/blackboard/view.rs` | Blackboard (phase-safe, typed+bytes slots) |
| `src/a2a/types.rs` | A2A transport types, protocol versions |
| `src/meta_agents/orchestrator.rs` | MetaOrchestrator (auto-spawn, skill scoring) |
| `src/meta_agents/card_builder.rs` | AgentCard generation from blueprints |
| `src/meta_agents/skill_engine.rs` | Feedback-driven skill adjustment |
| `src/capabilities/registry.rs` | Namespaced capability resolution |
| `src/persona/mod.rs` | 36 styles, triune topology, inner loop |
| `src/memory/` | Short/long-term/entity/contextual/external |
| `docs/guides/agent-card-spec.md` | Full YAML specification (807 lines) |

### JITSON / Cranelift (vendor imports)
| Repo | Purpose |
|------|---------|
| `rustynum/jitson` | JSON/YAML → native code via Cranelift |
| `wasmtime/cranelift` | Cranelift compiler backend (forked) |

---

*Document Version: 2.0*
*Last Updated: 2026-02-23*
*Branch: claude/vsaclip-hamming-recognition-y0b94*
