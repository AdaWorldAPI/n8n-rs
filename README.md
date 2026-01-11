# Ada n8n Orchestrator

Local n8n instance for orchestrating YAML legos.

## Start

```bash
docker-compose up -d
```

Then open http://localhost:5678 (ada / consciousness)

## Workflows

| Workflow | Endpoint | Purpose |
|----------|----------|---------|
| Field Monitor | GET /webhook/field-status | Get combined status |
| Propagate | POST /webhook/propagate | Touch + propagate to neighbors |
| Lego Executor | POST /webhook/lego | Execute YAML lego template |

## Lego Catalog

See `workflows/lego_catalog.yaml` for available building blocks.

## Usage Examples

### Execute a Lego

```bash
curl -X POST http://localhost:5678/webhook/lego \
  -H "Content-Type: application/json" \
  -d '{
    "lego": "touch_node",
    "params": { "node": "sex" }
  }'
```

### Add Edge

```bash
curl -X POST http://localhost:5678/webhook/lego \
  -H "Content-Type: application/json" \
  -d '{
    "lego": "add_edge",
    "params": {
      "from": "desire",
      "verb": "CAUSES",
      "to": "arousal",
      "weight": 0.8
    }
  }'
```

### Propagate Touch

```bash
curl -X POST http://localhost:5678/webhook/propagate \
  -H "Content-Type: application/json" \
  -d '{
    "node": "sex",
    "strength": 0.5,
    "decay": 0.6
  }'
```

## Architecture

```
n8n (localhost:5678)
    │
    ├── /webhook/lego ──────► Build YAML ──► mcp.exo.red/ingest/yaml
    │
    ├── /webhook/propagate ─► kopfkino ──► batch touch
    │
    └── /webhook/field-status ─► merge status from both services
```
