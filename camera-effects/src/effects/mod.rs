//! Effects module
//!
//! Provides GPU-based visual effects for camera input.

pub mod person_particles;
pub mod hand_interaction;
pub mod paint_warp;

/// Effect types
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectType {
    /// Turn people into particles
    PersonParticles,
    /// Hand-driven particle interaction
    HandInteraction,
    /// Paint-like warping
    PaintWarp,
}

impl Default for EffectType {
    fn default() -> Self {
        Self::PersonParticles
    }
}
