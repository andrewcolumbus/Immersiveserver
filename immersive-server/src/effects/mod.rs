//! Effects system for immersive-server
//!
//! This module provides a Resolume-style effects system with:
//! - Stackable effects with bypass/solo controls
//! - Application at Environment, Layer, or Clip level
//! - Hybrid GPU + CPU effect processing
//! - BPM/LFO automation for parameter modulation
//!
//! # Architecture
//!
//! The effects system follows the same data/runtime separation pattern as
//! Layer/LayerRuntime:
//!
//! - **Data types** (`types.rs`): Serializable effect definitions (EffectStack,
//!   EffectInstance, Parameter, etc.) that can be saved to .immersive files
//! - **Traits** (`traits.rs`): EffectDefinition trait for effect factories,
//!   GpuEffectRuntime/CpuEffectRuntime traits for processing
//! - **Registry** (`registry.rs`): Central registry of available effects
//! - **Runtime** (`runtime.rs`): GPU resources and effect chain processing
//! - **Builtin** (`builtin/`): Built-in effects (color_correction, invert, etc.)
//!
//! # Usage
//!
//! ```ignore
//! // Create registry and register built-in effects
//! let mut registry = EffectRegistry::new();
//! builtin::register_builtin_effects(&mut registry);
//!
//! // Add effect to a layer
//! let params = registry.default_parameters("color_correction").unwrap();
//! layer.effects.add("color_correction", "Color Correction", params);
//!
//! // Modify effect parameter
//! if let Some(effect) = layer.effects.get_mut(effect_id) {
//!     effect.set_parameter("brightness", ParameterValue::Float(0.5));
//! }
//!
//! // Bypass effect
//! if let Some(effect) = layer.effects.get_mut(effect_id) {
//!     effect.bypassed = true;
//! }
//! ```

mod types;
mod traits;
mod registry;
mod runtime;
mod automation;
mod manager;
pub mod builtin;

pub use types::*;
pub use traits::*;
pub use registry::*;
pub use runtime::*;
pub use automation::*;
pub use manager::*;
