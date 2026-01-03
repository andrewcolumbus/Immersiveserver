//! Shared GPU context for multi-window rendering
//!
//! Provides `GpuContext` for shared GPU resources (device, queue, adapter)
//! and `WindowGpuContext` for per-window surface/renderer resources.
//!
//! This separation enables multiple windows to share a single GPU device
//! while each maintaining its own surface for rendering.

use std::sync::Arc;
use winit::window::Window;

// ═══════════════════════════════════════════════════════════════════════════════
// GPU CONTEXT — Shared GPU resources
// ═══════════════════════════════════════════════════════════════════════════════

/// Shared GPU resources that can be used across multiple windows.
///
/// Wrapped in `Arc<GpuContext>` for sharing between the main app and panel windows.
pub struct GpuContext {
    /// The wgpu instance
    pub instance: wgpu::Instance,
    /// The selected GPU adapter
    pub adapter: wgpu::Adapter,
    /// The GPU device for creating resources
    pub device: wgpu::Device,
    /// The command queue for submitting GPU work
    pub queue: wgpu::Queue,
    /// The preferred surface format (typically sRGB)
    pub surface_format: wgpu::TextureFormat,
    /// Whether BC texture compression is supported (for HAP/DXV)
    pub bc_texture_supported: bool,
}

impl GpuContext {
    /// Create a GpuContext from pre-initialized components.
    ///
    /// This is useful when the caller has already created the wgpu resources
    /// (e.g., during App initialization) and wants to wrap them for sharing.
    pub fn from_parts(
        instance: wgpu::Instance,
        adapter: wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        bc_texture_supported: bool,
    ) -> Self {
        Self {
            instance,
            adapter,
            device,
            queue,
            surface_format,
            bc_texture_supported,
        }
    }

    /// Create a new GPU context with the specified window for initial surface compatibility.
    ///
    /// The window is used to determine compatible surface formats but the context
    /// can be used with other windows as well (on the same adapter).
    pub async fn new(window: Arc<Window>) -> Self {
        // Create wgpu instance
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        // Create temporary surface for adapter selection
        let surface = instance
            .create_surface(window)
            .expect("Failed to create surface");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to find suitable GPU adapter");

        tracing::info!("Using GPU: {}", adapter.get_info().name);
        tracing::info!("Backend: {:?}", adapter.get_info().backend);

        // Request BC texture compression for GPU-native codecs (HAP/DXV)
        let bc_texture_supported = adapter.features().contains(wgpu::Features::TEXTURE_COMPRESSION_BC);
        let mut required_features = wgpu::Features::empty();
        if bc_texture_supported {
            required_features |= wgpu::Features::TEXTURE_COMPRESSION_BC;
            tracing::info!("BC texture compression enabled (for HAP/DXV support)");
        } else {
            tracing::warn!("BC texture compression not available - HAP/DXV will use software decode");
        }

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Immersive Server Device"),
                    required_features,
                    required_limits: adapter.limits(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .expect("Failed to create device");

        let surface_caps = surface.get_capabilities(&adapter);

        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        tracing::info!("Surface format: {:?}", surface_format);

        Self {
            instance,
            adapter,
            device,
            queue,
            surface_format,
            bc_texture_supported,
        }
    }

    /// Create a new GPU context using an existing surface for compatibility.
    ///
    /// This variant takes a pre-created surface, useful when the caller already
    /// has a surface from the main window.
    pub async fn with_surface(surface: &wgpu::Surface<'_>) -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to find suitable GPU adapter");

        tracing::info!("Using GPU: {}", adapter.get_info().name);
        tracing::info!("Backend: {:?}", adapter.get_info().backend);

        let bc_texture_supported = adapter.features().contains(wgpu::Features::TEXTURE_COMPRESSION_BC);
        let mut required_features = wgpu::Features::empty();
        if bc_texture_supported {
            required_features |= wgpu::Features::TEXTURE_COMPRESSION_BC;
            tracing::info!("BC texture compression enabled (for HAP/DXV support)");
        } else {
            tracing::warn!("BC texture compression not available - HAP/DXV will use software decode");
        }

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Immersive Server Device"),
                    required_features,
                    required_limits: adapter.limits(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .expect("Failed to create device");

        let surface_caps = surface.get_capabilities(&adapter);

        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        tracing::info!("Surface format: {:?}", surface_format);

        Self {
            instance,
            adapter,
            device,
            queue,
            surface_format,
            bc_texture_supported,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// WINDOW GPU CONTEXT — Per-window rendering resources
// ═══════════════════════════════════════════════════════════════════════════════

/// Per-window GPU resources for rendering.
///
/// Each window that needs to render content has its own WindowGpuContext
/// which includes the surface for presenting and an egui renderer.
pub struct WindowGpuContext {
    /// The wgpu surface for this window
    pub surface: wgpu::Surface<'static>,
    /// Surface configuration
    pub config: wgpu::SurfaceConfiguration,
    /// egui renderer for this window
    pub egui_renderer: egui_wgpu::Renderer,
}

impl WindowGpuContext {
    /// Create a new window GPU context for the given window.
    ///
    /// Uses the shared GpuContext for device/queue access.
    pub fn new(gpu: &GpuContext, window: Arc<Window>) -> Self {
        let size = window.inner_size();

        let surface = gpu
            .instance
            .create_surface(window)
            .expect("Failed to create surface");

        let surface_caps = surface.get_capabilities(&gpu.adapter);

        // Use the shared surface format from GpuContext
        let surface_format = gpu.surface_format;

        // Prefer Immediate mode for manual FPS control
        let present_mode = if surface_caps.present_modes.contains(&wgpu::PresentMode::Immediate) {
            wgpu::PresentMode::Immediate
        } else if surface_caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
            wgpu::PresentMode::Mailbox
        } else {
            wgpu::PresentMode::Fifo
        };

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 1,
        };

        surface.configure(&gpu.device, &config);

        let egui_renderer = egui_wgpu::Renderer::new(&gpu.device, surface_format, None, 1, false);

        Self {
            surface,
            config,
            egui_renderer,
        }
    }

    /// Resize the window surface.
    pub fn resize(&mut self, gpu: &GpuContext, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&gpu.device, &self.config);
        }
    }

    /// Get the current surface size.
    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_context_fields() {
        // Basic compilation test - actual GPU tests need a window
        assert!(std::mem::size_of::<GpuContext>() > 0);
    }

    #[test]
    fn test_window_gpu_context_fields() {
        assert!(std::mem::size_of::<WindowGpuContext>() > 0);
    }
}
