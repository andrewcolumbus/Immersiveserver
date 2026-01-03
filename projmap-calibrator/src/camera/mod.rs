//! Camera input module for projection mapping calibration.
//!
//! Provides NDI camera input for capturing projected patterns.

mod ndi_ffi;
mod ndi_input;

pub use ndi_ffi::{destroy as ndi_destroy, initialize as ndi_initialize, version as ndi_version};
pub use ndi_input::{NdiError, NdiFinder, NdiFrame, NdiReceiver};
