//! GPU render pipeline for projector output.

use wgpu::{Device, Queue, Surface, SurfaceConfiguration};

/// Main render pipeline for projector output.
pub struct RenderPipeline {
    device: Device,
    queue: Queue,
    surface: Surface<'static>,
    config: SurfaceConfiguration,
}

impl RenderPipeline {
    pub fn new(
        device: Device,
        queue: Queue,
        surface: Surface<'static>,
        config: SurfaceConfiguration,
    ) -> Self {
        Self {
            device,
            queue,
            surface,
            config,
        }
    }

    pub fn device(&self) -> &Device {
        &self.device
    }

    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    pub fn surface(&self) -> &Surface<'_> {
        &self.surface
    }

    pub fn config(&self) -> &SurfaceConfiguration {
        &self.config
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }
}
