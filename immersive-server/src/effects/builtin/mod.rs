//! Built-in effects
//!
//! This module contains the standard effects that ship with immersive-server.

mod color_correction;
mod invert;

pub use color_correction::{ColorCorrectionDefinition, ColorCorrectionRuntime};
pub use invert::{InvertDefinition, InvertRuntime};

use super::EffectRegistry;

/// Register all built-in effects with the registry
pub fn register_builtin_effects(registry: &mut EffectRegistry) {
    registry.register(ColorCorrectionDefinition);
    registry.register(InvertDefinition);
}
