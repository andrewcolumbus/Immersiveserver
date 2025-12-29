//! Project module for save/load functionality
//!
//! Handles saving and loading project presets.

pub mod immersive_format;
mod preset;

pub use immersive_format::{load_immersive, save_immersive};
pub use preset::ProjectPreset;


