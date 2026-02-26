# n8n-rust Integration Status

## Architecture: One Binary via JITSON

The goal is a **single binary** where the workflow orchestrator uses **JIT compilation
(Cranelift)** instead of JSON serialization between components. Config IS the code.

```
Workflow YAML/JSON
    │  (deploy/activate time)
    ▼
JITSON (Cranelift via wasmtime fork)
    ├── Node routing tables → compiled dispatch (fn ptrs, not HashMap lookups)
    ├── Static parameters → baked as immediates (not serde_json::Value)
    ├── Thinking style thresholds → CMP immediates + branch hints
    ├── Focus masks → VPANDQ bitmasks
    │  (runtime)
    ▼
One binary executes compiled kernels
    ├── n8n-core calls fn ptrs directly (no JSON between nodes for static data)
    ├── crewai-rust linked as library crate (no HTTP to localhost:8090)
    ├── ladybug-rs linked as library crate (no HTTP to localhost)
    └── Dynamic data (API responses, user input) still flows as runtime values
```

## JITSON Integration TODO (J-series)

| Step | Task | Crate | Status |
|------|------|-------|--------|
| J.1 | Add `jitson` as vendor dependency (`path = "../../rustynum/jitson"`) | n8n-contract | **Done** |
| J.2 | Cranelift via wasmtime fork path deps | jitson | **Done** (already wired) |
| J.3 | `CompiledStyle` wrapper: ThinkingStyle → ScanParams → ScanKernel | n8n-contract | **Done** |
| J.4 | `WorkflowHotPath`: compile static node routing tables at activation | n8n-core | **Done** |
| J.5 | Kernel cache by parameter hash in engine | n8n-core | **Done** |
| J.6 | Activation pipeline: activate → compile → cache → execute | n8n-server | **Done** |

## Single Binary TODO (S-series)

| Step | Task | Crate | Status |
|------|------|-------|--------|
| S.1 | Uncomment vendor path deps in ladybug-rs/Cargo.toml | ladybug-rs | Planned |
| S.2 | Replace HTTP proxy routes with direct library calls behind `#[cfg(feature)]` | ladybug-rs server | Planned |
| S.3 | Wire crewai-rust as library dep (not HTTP to :8090) | ladybug-rs | Planned |
| S.4 | Wire n8n-rs crates as library deps (not HTTP to :8091) | ladybug-rs | Planned |
| S.5 | Single Dockerfile that builds one binary with all features | ladybug-rs | Planned |

## Key Repos & Paths

| Component | Path | Role |
|-----------|------|------|
| n8n-rs Rust crates | `/n8n-rs/n8n-rust/crates/` | Workflow engine (8 crates) |
| crewai-rust | `/crewai-rust/` | Agent orchestration, A2A, blackboard |
| ladybug-rs | `/ladybug-rs/` | Cognitive DB, BindSpace, host binary |
| jitson | `/rustynum/jitson/` | Cranelift JIT engine |
| wasmtime fork | `/wasmtime/` | Cranelift backend with AVX-512 |
| rustynum | `/rustynum/` | SIMD kernels (hamming, dot, etc.) |

## What CAN Be JIT-Compiled (deploy-time-known)

- Workflow node routing tables → compiled dispatch
- Thinking style thresholds → CMP immediates
- Focus masks (attention gating) → VPANDQ bitmasks
- CollapseGate voting weights → branch probability hints
- Flow/Hold/Block thresholds → CMP immediates
- Resonance search prefetch → PREFETCHT0 offsets

## What CANNOT Be JIT-Compiled (runtime-dynamic)

- External JSON from API responses, webhook payloads
- User-submitted workflow input data
- Dynamic expression results (`{{ $json.field }}`)
- Real-time A2A message payloads

## Branch

All work on: `claude/compare-rustynum-ndarray-5ePRn`
