//! 3D Previsualization module
//!
//! Renders environment texture onto 3D surfaces (circle, walls, dome) for previewing
//! how content will appear on physical installations.

pub mod camera;
pub mod mesh;
pub mod renderer;
pub mod types;

pub use camera::OrbitCamera;
pub use mesh::{PrevisMesh, PrevisVertex};
pub use renderer::PrevisRenderer;
pub use types::{PrevisSettings, SurfaceType};
