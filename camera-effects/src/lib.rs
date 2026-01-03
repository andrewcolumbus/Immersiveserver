//! Camera Effects - ML-powered visual effects with Syphon/Spout output
//!
//! A cross-platform application that captures camera input, applies ML-powered
//! visual effects (person segmentation, hand tracking), and outputs via
//! Syphon (macOS) or Spout (Windows).

pub mod app;
pub mod camera;
pub mod effects;
pub mod ml;
pub mod network;
pub mod ui;

pub use app::App;
