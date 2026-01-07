//! Built-in effects
//!
//! This module contains the standard effects that ship with immersive-server.

mod auto_mask;
mod color_correction;
mod heat;
mod image_rain;
mod invert;
mod multiplex;
mod poop_rain;
mod slide;

pub use auto_mask::{AutoMaskDefinition, AutoMaskRuntime};
pub use color_correction::{ColorCorrectionDefinition, ColorCorrectionRuntime};
pub use heat::{HeatDefinition, HeatRuntime};
pub use image_rain::{ImageRainDefinition, ImageRainRuntime};
pub use invert::{InvertDefinition, InvertRuntime};
pub use multiplex::{MultiplexDefinition, MultiplexRuntime};
#[allow(unused_imports)]
pub use poop_rain::{PoopRainDefinition, PoopRainRuntime};
pub use slide::{SlideDefinition, SlideRuntime};

use super::EffectRegistry;

/// Register all built-in effects with the registry
pub fn register_builtin_effects(registry: &mut EffectRegistry) {
    registry.register(AutoMaskDefinition);
    registry.register(ColorCorrectionDefinition);
    registry.register(HeatDefinition);
    registry.register(ImageRainDefinition);
    registry.register(InvertDefinition);
    registry.register(MultiplexDefinition);
    registry.register(SlideDefinition);
}
