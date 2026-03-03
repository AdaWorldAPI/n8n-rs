//! Compiled thinking styles — JIT bridge from cognitive config to native code.
//!
//! This module converts crewai-rust `ThinkingStyle` parameters into jitson
//! `ScanParams` and compiles them via Cranelift into native function pointers.
//!
//! # Architecture
//!
//! ```text
//! ThinkingStyle (23D sparse vector)
//!       │
//!       ▼
//! CompiledStyle::from_params()
//!       │  Extracts: depth → threshold, fan_out → top_k, focus → mask
//!       ▼
//! ScanParams { threshold, top_k, prefetch_ahead, focus_mask, record_size }
//!       │
//!       ▼
//! JitEngine::compile_scan() or compile_hybrid_scan()
//!       │  Cranelift IR → native x86-64
//!       ▼
//! ScanKernel (fn ptr) — cached by parameter hash
//! ```
//!
//! The compiled kernel bakes all config values as immediates:
//! - `threshold` → CMP immediate (no memory fetch)
//! - `top_k` → loop bound constant
//! - `focus_mask` → VPANDQ bitmask
//! - `prefetch_ahead` → PREFETCHT0 offset

use jitson::{JitEngine, JitEngineBuilder, ScanKernel, ScanParams};
use jitson::ir::{CollapseParams, PhilosopherIR, RecipeIR, CollapseBias, VotingStrategy};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::{debug, info};

/// A compiled thinking style — holds the JIT-compiled scan kernel
/// and the source parameters for introspection.
pub struct CompiledStyle {
    /// The compiled scan kernel (native fn ptr).
    pub kernel: ScanKernel,
    /// Source scan parameters (for cache key / debugging).
    pub params: ScanParams,
    /// Style name (metadata).
    pub name: String,
    /// τ (tau) address from the thinking style.
    pub tau: u8,
}

impl CompiledStyle {
    /// Compile a thinking style from its 23D sparse vector representation.
    ///
    /// # Parameters
    ///
    /// - `name`: style name (e.g., "analytical", "creative")
    /// - `tau`: τ macro address (0x00–0xFF)
    /// - `sparse_vec`: 23D sparse vector (dimension name → value 0.0–1.0)
    /// - `record_size`: size of each record in the scan field (bytes)
    /// - `engine`: shared JIT engine (will compile and cache)
    pub fn from_sparse_vec(
        name: &str,
        tau: u8,
        sparse_vec: &HashMap<String, f32>,
        record_size: u32,
        engine: &mut JitEngine,
    ) -> Result<Self, jitson::ir::JitError> {
        let params = sparse_vec_to_scan_params(sparse_vec, record_size);
        debug!(
            style = name,
            tau = format!("0x{:02X}", tau),
            threshold = params.threshold,
            top_k = params.top_k,
            "Compiling thinking style to native kernel"
        );

        let kernel = engine.compile_scan(params.clone())?;

        info!(
            style = name,
            cached = engine.cached_count(),
            "Compiled thinking style to native scan kernel"
        );

        Ok(Self {
            kernel,
            params,
            name: name.to_string(),
            tau,
        })
    }

    /// Compile with a registered external distance function (hybrid mode).
    ///
    /// The distance function (e.g., `hamming_distance` from rustynum) is called
    /// by the JIT-compiled loop. The loop control is JIT'd, the kernel is native SIMD.
    pub fn from_sparse_vec_hybrid(
        name: &str,
        tau: u8,
        sparse_vec: &HashMap<String, f32>,
        record_size: u32,
        engine: &mut JitEngine,
        distance_fn_name: &str,
    ) -> Result<Self, jitson::ir::JitError> {
        let params = sparse_vec_to_scan_params(sparse_vec, record_size);
        let kernel = engine.compile_hybrid_scan(params.clone(), distance_fn_name)?;

        info!(
            style = name,
            distance_fn = distance_fn_name,
            "Compiled hybrid thinking style (JIT loop + SIMD kernel)"
        );

        Ok(Self {
            kernel,
            params,
            name: name.to_string(),
            tau,
        })
    }
}

/// Convert a thinking style recipe into jitson IR for full compilation.
///
/// This creates a `RecipeIR` that can be compiled to both a scan kernel
/// and a collapse gate. Used when the full philosopher voting pipeline
/// is needed (not just the scan).
pub fn style_to_recipe_ir(
    name: &str,
    sparse_vec: &HashMap<String, f32>,
    record_size: u32,
) -> RecipeIR {
    let scan = sparse_vec_to_scan_params(sparse_vec, record_size);

    // Extract philosopher-like thresholds from the style dimensions.
    // Each cognitive domain maps to a philosopher with voting weights.
    let philosophers = vec![
        PhilosopherIR {
            name: "analytical".to_string(),
            weight: sparse_vec.get("analytical").copied().unwrap_or(0.5),
            crystallized_min: 0.3,
            tensioned_max: 0.8,
            noise_floor: 0.05,
            collapse_bias: CollapseBias::Flow,
        },
        PhilosopherIR {
            name: "creative".to_string(),
            weight: sparse_vec.get("creative").copied().unwrap_or(0.5),
            crystallized_min: 0.2,
            tensioned_max: 0.9,
            noise_floor: 0.1,
            collapse_bias: CollapseBias::Hold,
        },
        PhilosopherIR {
            name: "empathic".to_string(),
            weight: sparse_vec.get("empathic").copied().unwrap_or(0.5),
            crystallized_min: 0.25,
            tensioned_max: 0.85,
            noise_floor: 0.08,
            collapse_bias: CollapseBias::Flow,
        },
    ];

    // Collapse gate: weighted majority with a 0.6 flow threshold.
    let collapse = CollapseParams {
        voting: VotingStrategy::WeightedMajority,
        flow_threshold: sparse_vec.get("confidence").copied().unwrap_or(0.6),
        veto_threshold: 0.9,
    };

    let plasticity = sparse_vec.get("plasticity").copied().unwrap_or(0.0);

    RecipeIR {
        name: name.to_string(),
        scan,
        philosophers,
        collapse,
        plasticity,
    }
}

/// Registry of compiled styles, keyed by τ address.
/// Thread-safe: kernels are immutable after compilation.
pub struct CompiledStyleRegistry {
    styles: HashMap<u8, CompiledStyle>,
    engine: JitEngine,
}

impl CompiledStyleRegistry {
    /// Create a new registry with a fresh JIT engine.
    pub fn new() -> Result<Self, jitson::ir::JitError> {
        Ok(Self {
            styles: HashMap::new(),
            engine: JitEngine::new()?,
        })
    }

    /// Create with an engine that has registered external distance functions.
    pub fn with_engine(engine: JitEngine) -> Self {
        Self {
            styles: HashMap::new(),
            engine,
        }
    }

    /// Compile and register a thinking style.
    pub fn register(
        &mut self,
        name: &str,
        tau: u8,
        sparse_vec: &HashMap<String, f32>,
        record_size: u32,
    ) -> Result<(), jitson::ir::JitError> {
        let compiled = CompiledStyle::from_sparse_vec(
            name, tau, sparse_vec, record_size, &mut self.engine,
        )?;
        self.styles.insert(tau, compiled);
        Ok(())
    }

    /// Get a compiled style by τ address.
    pub fn get(&self, tau: u8) -> Option<&CompiledStyle> {
        self.styles.get(&tau)
    }

    /// Number of compiled styles.
    pub fn len(&self) -> usize {
        self.styles.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.styles.is_empty()
    }

    /// Number of cached kernels in the JIT engine.
    pub fn cached_kernels(&self) -> usize {
        self.engine.cached_count()
    }
}

// ============================================================================
// Internal helpers
// ============================================================================

/// Map a 23D sparse thinking style vector to jitson ScanParams.
///
/// Mapping:
/// - `depth` (0.0–1.0) → threshold (100–2000): deeper = tighter match
/// - `breadth` (0.0–1.0) → top_k (8–128): broader = more candidates
/// - `focus` (0.0–1.0) → focus_mask density: focused = fewer active dims
/// - `speed` (0.0–1.0) → prefetch_ahead (1–8): faster = more prefetch
fn sparse_vec_to_scan_params(
    sparse_vec: &HashMap<String, f32>,
    record_size: u32,
) -> ScanParams {
    let depth = sparse_vec.get("depth").copied().unwrap_or(0.5);
    let breadth = sparse_vec.get("breadth").copied().unwrap_or(0.5);
    let speed = sparse_vec.get("speed").copied().unwrap_or(0.5);

    // Depth → threshold: deeper styles need tighter resonance matches.
    // 0.0 depth = threshold 2000 (loose), 1.0 depth = threshold 100 (tight).
    let threshold = (2000.0 - depth * 1900.0) as u32;

    // Breadth → top_k: broader styles explore more candidates.
    // 0.0 = 8, 1.0 = 128.
    let top_k = (8.0 + breadth * 120.0) as u32;

    // Speed → prefetch_ahead: faster styles prefetch more aggressively.
    let prefetch_ahead = (1.0 + speed * 7.0) as u32;

    // Focus mask: if "focus" dimension is high, mask to focused dimensions only.
    // For now, None = all dimensions awake. Future: build from active dimension indices.
    let focus_mask = None;

    ScanParams {
        threshold,
        top_k,
        prefetch_ahead,
        focus_mask,
        record_size,
    }
}
