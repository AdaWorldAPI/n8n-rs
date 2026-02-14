//! Legacy DTO types for standalone mode (without ladybug-rs integration).
//!
//! When compiled without the `ladybug` feature, these lightweight types
//! replace the ladybug-contract types used by impact_gate and interface_gateway.
//! Values are kept identical to ladybug-contract so that serialized data
//! is wire-compatible when the full stack is assembled.
//!
//! When compiled WITH the `ladybug` feature, this module is not used —
//! the real types from ladybug-contract are imported directly.

use serde::{Deserialize, Serialize};

/// Standalone NARS truth value: <frequency, confidence>.
///
/// Mirrors `ladybug_contract::nars::TruthValue` for standalone mode.
/// When the `ladybug` feature is enabled, use the real type instead.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TruthValue {
    pub frequency: f32,
    pub confidence: f32,
}

impl TruthValue {
    pub fn new(frequency: f32, confidence: f32) -> Self {
        Self { frequency, confidence }
    }
}

/// Wire protocol opcode constants — standalone copies of ladybug-contract values.
///
/// These match the canonical opcodes from `ladybug_contract::wire::wire_ops`
/// so that InterfaceDefinition metadata is compatible across standalone and
/// full-stack modes.
pub mod wire_ops {
    pub const RESONATE: u16 = 0x300;
    pub const EXECUTE: u16 = 0x301;
    pub const DELEGATE: u16 = 0x302;
    pub const INTEGRATE: u16 = 0x304;
    pub const CRYSTALLIZE: u16 = 0x306;
    pub const ROUTE: u16 = 0x307;
    pub const COLLAPSE: u16 = 0x308;
}
