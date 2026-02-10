# n8n-rust

A Rust implementation of the n8n workflow automation engine with multi-transport support, intelligent content negotiation, Arrow/LanceDB zero-copy data streaming, and 10kbit Hamming vector similarity search.

## Overview

This project provides a high-performance Rust backend for n8n workflow automation with:

- **Multi-Transport**: gRPC, Arrow Flight, REST, and STDIO transports
- **Intelligent Negotiation**: Automatic format/transport selection based on capabilities
- **Arrow Zero-Copy**: Apache Arrow IPC for efficient data transfer without serialization overhead
- **Arrow Flight**: High-performance streaming of execution data
- **Hamming Vectors**: 10,000-bit fingerprints for similarity search (inspired by ladybug-rs/firefly)
- **Graceful Fallback**: Seamless degradation with upgrade hints

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              n8n-server                                      │
│  ┌───────────┐  ┌───────────┐  ┌───────────┐  ┌───────────┐  ┌───────────┐  │
│  │   REST    │  │   gRPC    │  │  Flight   │  │   STDIO   │  │ Negotiator│  │
│  │  :8080    │  │  :50051   │  │  :50052   │  │  stdin/out│  │           │  │
│  └─────┬─────┘  └─────┬─────┘  └─────┬─────┘  └─────┬─────┘  └─────┬─────┘  │
│        │              │              │              │              │        │
│        └──────────────┴──────────────┴──────────────┴──────────────┘        │
│                                      │                                       │
│  ┌───────────────────────────────────┴───────────────────────────────────┐  │
│  │                           Transport Layer                              │  │
│  │   Content Negotiation • Health Tracking • Graceful Fallback           │  │
│  └───────────────────────────────────┬───────────────────────────────────┘  │
│                                      │                                       │
│  ┌─────────────┐  ┌─────────────┐  ┌─┴───────────┐  ┌─────────────┐        │
│  │  Workflow   │  │    Arrow    │  │   Hamming   │  │    Core     │        │
│  │  Service    │  │   Service   │  │   Service   │  │   Engine    │        │
│  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘        │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Multi-Transport Support

The server supports multiple transports with intelligent negotiation:

| Transport | Best For | Streaming | Zero-Copy |
|-----------|----------|-----------|-----------|
| Arrow Flight | Large datasets, analytics | ✓ | ✓ |
| gRPC | RPC calls, bidirectional | ✓ | - |
| REST | Universal compatibility | - | - |
| STDIO | CLI tools, pipes | ✓ | - |

### Fallback Chain

```
Flight → gRPC → REST → STDIO
```

If a transport fails, the system automatically suggests alternatives with upgrade hints.

## Content Negotiation

### Via Accept Header

```bash
# Request Arrow format
curl -H "Accept: application/vnd.apache.arrow.stream" \
     http://localhost:8080/api/v1/executions/123

# Response includes negotiation info
# X-Content-Format: arrow-ipc
# X-Format-Negotiated: true
# X-Upgrade-Available: arrow-flight
```

### Via Query Parameter

```bash
curl "http://localhost:8080/api/v1/executions/123?fmt=arrow-ipc"
```

### Via Format Switch Endpoint

```bash
# Switch to Arrow Flight mid-session
curl -X POST http://localhost:8080/api/v1/format/switch \
  -H "Content-Type: application/json" \
  -d '{"format": "arrow-flight", "transport": "flight"}'

# Response:
# {
#   "success": true,
#   "newFormat": "arrow-flight",
#   "newTransport": "flight",
#   "newEndpoint": "/flight",
#   "headers": {"Accept": "application/vnd.apache.arrow.flight"}
# }
```

### Negotiate Endpoint

```bash
# Ask server for best format based on data characteristics
curl -X POST http://localhost:8080/api/v1/negotiate \
  -H "Content-Type: application/json" \
  -d '{
    "formats": ["json", "arrow-ipc"],
    "transports": ["rest", "flight"],
    "dataHints": {
      "sizeBytes": 10000000,
      "streaming": true,
      "columnar": true
    }
  }'

# Response recommends optimal format/transport
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

Multi-transport services:
- Protocol Buffer definitions
- REST API with content negotiation
- STDIO transport for CLI
- WorkflowService (CRUD, execution, streaming)
- ArrowDataService (zero-copy data streaming)
- HammingService (similarity search)

### n8n-server

Multi-transport server binary.

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

### STDIO Transport

```bash
# Enable STDIO mode
N8N_STDIO_ENABLED=1 cargo run --bin n8n-server

# Send JSON-RPC style messages
echo '{"type":"request","id":"1","method":"ping","params":{}}' | n8n-server
# {"type":"response","id":"1","result":{"pong":true}}
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

## Configuration

Environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `N8N_REST_ENABLED` | `true` | Enable REST API |
| `N8N_REST_ADDR` | `0.0.0.0:8080` | REST API address |
| `N8N_GRPC_ENABLED` | `true` | Enable gRPC |
| `N8N_GRPC_ADDR` | `0.0.0.0:50051` | gRPC address |
| `N8N_FLIGHT_ENABLED` | `true` | Enable Arrow Flight |
| `N8N_FLIGHT_ADDR` | `0.0.0.0:50052` | Flight address |
| `N8N_STDIO_ENABLED` | `false` | Enable STDIO transport |

## Building

```bash
cd n8n-rust
cargo build --release
```

## Running

```bash
# Run with defaults (REST + gRPC + Flight)
cargo run --release --bin n8n-server

# With STDIO enabled
N8N_STDIO_ENABLED=1 cargo run --release --bin n8n-server

# Custom ports
N8N_REST_ADDR=0.0.0.0:3000 N8N_GRPC_ADDR=0.0.0.0:9000 cargo run --release --bin n8n-server
```

## API Endpoints

### REST

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/health` | Health check |
| GET | `/ready` | Readiness check |
| GET | `/api/v1/capabilities` | Server capabilities |
| POST | `/api/v1/negotiate` | Format negotiation |
| POST | `/api/v1/format/switch` | Switch format |

### STDIO Methods

| Method | Description |
|--------|-------------|
| `capabilities` | Get server capabilities |
| `ping` | Health check |
| `workflow.list` | List workflows |
| `workflow.execute` | Execute workflow |
| `hamming.create` | Create fingerprint |

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
