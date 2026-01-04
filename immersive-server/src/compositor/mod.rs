//! Compositor module
//!
//! The compositor is responsible for rendering sources (video, NDI, OMT, etc.)
//! into a fixed-resolution composition canvas (`Environment`).
//!
//! # Architecture
//!
//! - `Environment`: The fixed-resolution canvas that holds all layers
//! - `Layer`: A single compositing element with source, transform, opacity, blend mode
//! - `ClipCell`: A video clip that can be triggered on a layer
//! - `Viewport`: Pan/zoom navigation for viewing the environment

pub mod blend;
pub mod clip;
pub mod environment;
pub mod layer;
pub mod viewport;

pub use blend::BlendMode;
pub use clip::{ClipCell, ClipSource, ClipTransition, LoopMode, DEFAULT_CLIP_SLOTS};
pub use environment::Environment;
pub use layer::{Layer, LayerSource, Transform2D};
pub use viewport::Viewport;
