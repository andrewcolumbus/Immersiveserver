//! Compositor module
//!
//! The compositor is responsible for rendering sources (video, NDI, OMT, etc.)
//! into a fixed-resolution composition canvas (`Environment`).

pub mod environment;
pub mod viewport;

pub use environment::Environment;
pub use viewport::Viewport;
