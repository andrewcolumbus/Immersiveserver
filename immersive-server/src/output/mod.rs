//! Advanced Output system for multi-screen projection mapping
//!
//! This module provides support for:
//! - Multiple output screens with independent configurations
//! - Slice-based input selection (crop/position from composition or layers)
//! - Output transformations (perspective warp, mesh deformation)
//! - Edge blending for seamless projector overlap
//! - Per-output masking and color correction
//! - Display enumeration and multi-monitor output

mod color;
pub mod display;
mod edge_blend;
mod mask;
pub mod runtime;
mod screen;
pub mod slice;
mod warp;

pub use color::{OutputColorCorrection, SliceColorCorrection};
pub use display::{DisplayEvent, DisplayInfo, DisplayManager, DisplayStatus};
pub use edge_blend::{EdgeBlendConfig, EdgeBlendRegion};
pub use mask::{BezierSegment, MaskShape, Point2D, SliceMask};
pub use runtime::{OutputManager, ScreenRuntime, SliceParams, SliceRuntime};
pub use screen::{OutputDevice, Screen, ScreenId};
pub use slice::{Rect, Slice, SliceId, SliceInput, SliceOutput};
pub use warp::{WarpInterpolation, WarpMesh, WarpPoint};
