# Deprecated: PR #21 (partial) — Cognitive Workflow Contracts

**Date**: 2026-02-17
**PR**: #21 (Add autopoiesis spec, workflow contracts, and integration plan)
**Note**: Only COGNITIVE_WORKFLOW_CONTRACTS.md deprecated. AUTOPOIESIS_SPEC.md is kept.

## Why deprecated

The contracts define Arrow Flight RPC surfaces and service endpoints between
n8n-rs, crewai-rust, and ladybug-rs. In the one-binary model, these become
direct function calls with `&self` / `&mut self` borrows.

## What to salvage

- The FreeWillPipeline 7-step evaluation is sound logic
- The ImpactGate RBAC concept is correct
- TopologyChange enum (PruneEdge, GrowEdge, DeactivateNode, Replicate) is clean
- Rewrite as trait methods on shared substrate, not RPC actions
