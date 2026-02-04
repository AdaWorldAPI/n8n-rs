# n8n-rust

A Rust implementation of the n8n workflow automation engine with gRPC, Arrow/LanceDB zero-copy data streaming, and 10kbit Hamming vector similarity search.

## Overview

This project provides a high-performance Rust backend for n8n workflow automation with:

- **gRPC Services**: Full workflow management and execution via Protocol Buffers
- **Arrow Zero-Copy**: Apache Arrow IPC for efficient data transfer without serialization overhead
- **Arrow Flight**: High-performance streaming of execution data
- **Hamming Vectors**: 10,000-bit fingerprints for similarity search (inspired by ladybug-rs/firefly)
- **JSON Fallback**: Full JSON compatibility for existing integrations

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        n8n-server                                │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐  │
│  │ WorkflowService │  │ ArrowDataService│  │ HammingService  │  │
│  │     (gRPC)      │  │   (Flight)      │  │    (gRPC)       │  │
│  └────────┬────────┘  └────────┬────────┘  └────────┬────────┘  │
│           │                    │                    │           │
│  ┌────────┴────────────────────┴────────────────────┴────────┐  │
│  │                       n8n-grpc                             │  │
│  └────────┬────────────────────┬────────────────────┬────────┘  │
│           │                    │                    │           │
│  ┌────────┴────────┐  ┌────────┴────────┐  ┌────────┴────────┐  │
│  │    n8n-core     │  │   n8n-arrow     │  │  n8n-hamming    │  │
│  │ (Exec Engine)   │  │ (Zero-Copy IO)  │  │ (10kbit Vecs)   │  │
│  └────────┬────────┘  └─────────────────┘  └─────────────────┘  │
│           │                                                      │
│  ┌────────┴────────┐                                            │
│  │  n8n-workflow   │                                            │
│  │  (Core Types)   │                                            │
│  └─────────────────┘                                            │
└─────────────────────────────────────────────────────────────────┘
```

## Crates

### n8n-workflow

Core workflow types matching the TypeScript n8n data model:
- `Workflow`, `Node`, `Connection` types
- `NodeExecutionData`, `TaskData` execution types
- Graph traversal utilities (topological sort, parent/child lookups)

### n8n-core

Workflow execution engine:
- Stack-based execution model (resumable, partial execution)
- Node executor registry with built-in nodes
- Runtime context and configuration
- In-memory and extensible storage backends

### n8n-arrow

Apache Arrow integration:
- Schema definitions for all n8n data types
- Conversion between n8n types and Arrow RecordBatches
- IPC serialization for zero-copy transfer
- Arrow Flight service implementation

### n8n-hamming

10,000-bit Hamming vectors (inspired by ladybug-rs/firefly):
- SIMD-accelerated distance calculation using POPCNT
- XOR-based binding/unbinding for associative operations
- SHA256-based deterministic vector generation
- Similarity index with k-NN search

### n8n-grpc

gRPC service implementations:
- Protocol Buffer definitions
- WorkflowService (CRUD, execution, streaming)
- ArrowDataService (zero-copy data streaming)
- HammingService (similarity search)

### n8n-server

Combined gRPC/Flight server binary.

## Features

### Zero-Copy Data Transfer

```rust
use n8n_arrow::{batch_to_ipc_bytes, ipc_bytes_to_batches};

// Convert execution data to Arrow
let batch = run_data_to_batch(&run.result_data.run_data)?;

// Serialize to IPC (zero-copy on receiver side)
let bytes = batch_to_ipc_bytes(&batch)?;

// Send via gRPC/Flight...

// Deserialize without copying
let batches = ipc_bytes_to_batches(&bytes)?;
```

### Hamming Similarity

```rust
use n8n_hamming::HammingVector;

// Create vectors from seeds
let cat = HammingVector::from_seed("cat");
let dog = HammingVector::from_seed("dog");

// XOR binding for associative memory
let bound = cat.bind(&dog);
let recovered = bound.unbind(&cat);  // == dog

// Fast similarity search
let distance = cat.distance(&dog);  // Hamming distance (0-10000)
let similarity = cat.similarity(&dog);  // 0.0 to 1.0
```

### Workflow Execution

```rust
use n8n_core::{WorkflowEngine, RuntimeConfig};
use n8n_workflow::{Workflow, Node, WorkflowBuilder};

// Build a workflow
let workflow = WorkflowBuilder::new("My Workflow")
    .node(Node::new("Start", "n8n-nodes-base.manualTrigger"))
    .node(Node::new("Process", "n8n-nodes-base.set"))
    .connect("Start", "Process", 0, 0)?
    .build()?;

// Execute
let engine = WorkflowEngine::new(RuntimeConfig::default());
let run = engine.execute(&workflow, WorkflowExecuteMode::Manual, None).await?;
```

## Protocol Buffers

The gRPC API is defined in `crates/n8n-grpc/proto/n8n.proto`:

```protobuf
service WorkflowService {
    rpc CreateWorkflow(CreateWorkflowRequest) returns (WorkflowResponse);
    rpc ExecuteWorkflow(ExecuteWorkflowRequest) returns (ExecutionResponse);
    rpc ExecuteWorkflowStream(ExecuteWorkflowRequest) returns (stream ExecutionEvent);
    // ...
}

service ArrowDataService {
    rpc StreamExecutionData(StreamDataRequest) returns (stream ArrowRecordBatch);
    rpc ExecuteWithArrowStream(stream ArrowInputData) returns (stream ArrowRecordBatch);
    // ...
}

service HammingService {
    rpc CreateFingerprint(CreateFingerprintRequest) returns (FingerprintResponse);
    rpc FindSimilar(SimilaritySearchRequest) returns (SimilaritySearchResponse);
    rpc BindFingerprints(BindRequest) returns (FingerprintResponse);
    // ...
}
```

## Building

```bash
cd n8n-rust
cargo build --release
```

## Running

```bash
# Set address (optional, defaults to 0.0.0.0:50051)
export N8N_GRPC_ADDR=0.0.0.0:50051

# Run server
cargo run --release --bin n8n-server
```

## Benchmarks

Run Hamming vector benchmarks:

```bash
cargo bench -p n8n-hamming
```

## Inspiration

- [ladybug-rs](https://github.com/AdaWorldAPI/ladybug-rs) - Crystal Lake Cognitive Database
- [firefly](https://github.com/AdaWorldAPI/firefly) - 10K Hamming Resonance vectors
- [n8n](https://n8n.io) - Original TypeScript workflow automation platform

## License

MIT OR Apache-2.0
