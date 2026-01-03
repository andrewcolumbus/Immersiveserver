//! Projection Mapping Calibration Tool
//!
//! A standalone application for automated projection mapping calibration using:
//! - NDI camera input
//! - Structured light (Gray code) patterns
//! - Multi-projector edge blending
//! - OpenCV for homography computation

pub mod app;
pub mod calibration;
pub mod camera;
pub mod config;
pub mod blending;
pub mod projector;
pub mod render;
pub mod ui;
pub mod export;
