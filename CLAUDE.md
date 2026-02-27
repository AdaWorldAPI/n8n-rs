# CLAUDE.md — n8n-rs

> **Last Updated**: 2026-02-27
> **Branch**: `claude/check-ladybug-rs-access-yVeJG`
> **Owner**: Jan Hübener (jahube)

---

## READ THIS FIRST — Role in the Four-Level Architecture

n8n-rs is a **Rust transcode** of the n8n workflow automation platform,
serving as the **meta-orchestration** layer in the four-level Ada architecture.
The Rust crates in `n8n-rust/` are the transcoded portion; a bulk of the
original TypeScript remains to be transcoded. The Rust crates provide
multi-transport support, Arrow zero-copy data streaming, 10kbit Hamming
vector similarity, and unified execution contracts for routing between
crewai-rust and ladybug-rs.

> **Canonical cross-repo architecture:** [ada-docs/architecture/FOUR_LEVEL_ARCHITECTURE.md](https://github.com/AdaWorldAPI/ada-docs/blob/main/architecture/FOUR_LEVEL_ARCHITECTURE.md)

### Rust Crates (`n8n-rust/`)

| Crate | Purpose |
|-------|---------|
| **n8n-core** | Execution engine |
| **n8n-workflow** | Workflow types (Workflow, Node, Connection) |
| **n8n-grpc** | Multi-transport services (REST, gRPC, Arrow Flight, STDIO) |
| **n8n-arrow** | Apache Arrow integration (zero-copy IPC, Flight) |
| **n8n-hamming** | 10kbit Hamming vector similarity |
| **n8n-db** | PostgreSQL persistence |
| **n8n-contract** | Unified execution contract (crew/ladybug routing) |
| **n8n-server** | Multi-transport server binary |

---

## Arrow Zero-Copy Chain

n8n-rs integrates with ladybug-rs and rustynum through Arrow zero-copy buffers.

**The chain:**

```
n8n-rs workflow node
    -> Arrow RecordBatch (n8n-arrow, Arrow 57)
        -> Arrow Flight (port 50052, zero-copy IPC)
            -> ladybug-rs BindSpace (ArrowZeroCopy)
                -> rustynum SIMD kernels (no copy)
```

**Everything goes through BindSpace. BindSpace needs rustynum (SIMD, Fingerprint
types, DeltaLayer, CollapseGate) and Lance/Arrow (mmap'd zero-copy buffers)
and split_at_mut/parallel_into_slices (lock-free parallel writes).**

### Rules for n8n-rs Developers

- **NEVER** copy Arrow buffers -- use `Buffer::clone()` (Arc, not memcpy)
- **NEVER** implement SIMD distance functions -- ladybug-rs and rustynum own compute
- n8n-hamming has its own 10kbit Hamming -- this is for standalone workflow use.
  For cognitive search, route through ladybug-rs which calls rustynum.
- n8n-contract provides `crew_router.rs` and `ladybug_router.rs` for delegation.
  These use HTTP proxy when not vendor-linked, in-process calls when vendor-linked.
- The `wire_bridge.rs` uses `ladybug_contract::wire::CogPacket` for binary protocol.

### Dependencies

```toml
# Arrow stack (all v57)
arrow, arrow-array, arrow-schema, arrow-buffer, arrow-ipc, arrow-flight

# Lance (cold-tier persistence)
lancedb = "0.16"
lance = "2.0"

# ladybug-rs (cognitive database)
ladybug = { path = "../../ladybug-rs", features = ["lancedb"] }
ladybug-contract = { path = "../../ladybug-rs/crates/ladybug-contract" }
```

### Cross-Repo References

- `ladybug-rs/CLAUDE.md` -- "The Rustynum Acceleration Contract"
- `rustynum/CLAUDE.md` -- Section 12 "The Lance Zero-Copy Contract"
- `crewai-rust/CLAUDE.md` -- "Storage Strategy"

---

## Essential Commands

```bash
# Build Rust crates
cd n8n-rust && cargo build

# Test Rust crates
cd n8n-rust && cargo test

# Run server
cd n8n-rust && cargo run --bin n8n-server
```
