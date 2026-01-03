//! Effect runtime - GPU resources and effect chain processing
//!
//! This module provides the runtime infrastructure for processing effect chains:
//! - `EffectTexturePool` - Ping-pong textures for multi-effect chains
//! - `EffectStackRuntime` - Manages effect instances and processes chains

use std::collections::HashMap;

use super::traits::{EffectParams, GpuEffectRuntime};
use super::{EffectInstance, EffectRegistry, EffectStack};

/// Manages intermediate textures for effect chain processing
///
/// Uses a ping-pong strategy: effects alternate reading from one texture
/// and writing to the other, avoiding per-effect allocation.
pub struct EffectTexturePool {
    /// Two textures for ping-pong rendering
    textures: [wgpu::Texture; 2],
    /// Views for the textures
    views: [wgpu::TextureView; 2],
    /// Current dimensions
    width: u32,
    height: u32,
    /// Texture format
    format: wgpu::TextureFormat,
}

impl EffectTexturePool {
    /// Create a new texture pool with the given dimensions
    pub fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Self {
        let textures = [
            Self::create_texture(device, width, height, format, "Effect Texture A"),
            Self::create_texture(device, width, height, format, "Effect Texture B"),
        ];

        let views = [
            textures[0].create_view(&wgpu::TextureViewDescriptor::default()),
            textures[1].create_view(&wgpu::TextureViewDescriptor::default()),
        ];

        Self {
            textures,
            views,
            width,
            height,
            format,
        }
    }

    /// Resize the texture pool if dimensions changed
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if width == self.width && height == self.height {
            return;
        }

        self.textures = [
            Self::create_texture(device, width, height, self.format, "Effect Texture A"),
            Self::create_texture(device, width, height, self.format, "Effect Texture B"),
        ];

        self.views = [
            self.textures[0].create_view(&wgpu::TextureViewDescriptor::default()),
            self.textures[1].create_view(&wgpu::TextureViewDescriptor::default()),
        ];

        self.width = width;
        self.height = height;
    }

    /// Get the texture views for a given effect index in the chain
    ///
    /// Even indices: read from A, write to B
    /// Odd indices: read from B, write to A
    pub fn get_views(&self, effect_index: usize) -> (&wgpu::TextureView, &wgpu::TextureView) {
        if effect_index % 2 == 0 {
            (&self.views[0], &self.views[1])
        } else {
            (&self.views[1], &self.views[0])
        }
    }

    /// Get the first texture view (for initial copy)
    pub fn first_view(&self) -> &wgpu::TextureView {
        &self.views[0]
    }

    /// Get the last output view based on effect count
    pub fn output_view(&self, effect_count: usize) -> &wgpu::TextureView {
        if effect_count % 2 == 0 {
            &self.views[0]
        } else {
            &self.views[1]
        }
    }

    /// Get texture dimensions
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn create_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        label: &str,
    ) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    }
}

/// Runtime entry for a single effect instance
struct EffectRuntimeEntry {
    /// GPU runtime (if GPU effect)
    gpu: Option<Box<dyn GpuEffectRuntime>>,
    // Future: CPU runtime
    // cpu: Option<Box<dyn CpuEffectRuntime>>,
}

/// Runtime state for an effect stack
///
/// Manages GPU resources and processes effect chains for a target
/// (Environment, Layer, or Clip).
pub struct EffectStackRuntime {
    /// Effect runtimes keyed by effect instance ID
    effect_runtimes: HashMap<u32, EffectRuntimeEntry>,
    /// Ping-pong textures for effect chain
    texture_pool: Option<EffectTexturePool>,
    /// Copy pipeline for texture blitting
    copy_pipeline: Option<wgpu::RenderPipeline>,
    /// Copy bind group layout
    copy_bind_group_layout: Option<wgpu::BindGroupLayout>,
    /// Sampler for texture sampling
    sampler: Option<wgpu::Sampler>,
}

impl Default for EffectStackRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl EffectStackRuntime {
    /// Create a new effect stack runtime
    pub fn new() -> Self {
        Self {
            effect_runtimes: HashMap::new(),
            texture_pool: None,
            copy_pipeline: None,
            copy_bind_group_layout: None,
            sampler: None,
        }
    }

    /// Initialize GPU resources
    pub fn init(
        &mut self,
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) {
        // Create texture pool
        self.texture_pool = Some(EffectTexturePool::new(device, width, height, format));

        // Create sampler
        self.sampler = Some(device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Effect Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        }));

        // Create copy pipeline for texture blitting
        let copy_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Effect Copy Shader"),
            source: wgpu::ShaderSource::Wgsl(COPY_SHADER.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Effect Copy Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Effect Copy Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Effect Copy Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &copy_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &copy_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        self.copy_bind_group_layout = Some(bind_group_layout);
        self.copy_pipeline = Some(pipeline);
    }

    /// Ensure texture pool has correct dimensions
    pub fn ensure_size(&mut self, device: &wgpu::Device, width: u32, height: u32, format: wgpu::TextureFormat) {
        if let Some(pool) = &mut self.texture_pool {
            pool.resize(device, width, height);
        } else {
            self.init(device, width, height, format);
        }
    }

    /// Sync effect runtimes with the effect stack
    ///
    /// Creates runtimes for new effects, removes runtimes for deleted effects.
    pub fn sync_with_stack(
        &mut self,
        stack: &EffectStack,
        registry: &EffectRegistry,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) {
        // Remove runtimes for effects that no longer exist
        let effect_ids: std::collections::HashSet<_> = stack.effects.iter().map(|e| e.id).collect();
        self.effect_runtimes.retain(|id, _| effect_ids.contains(id));

        // Create runtimes for new effects
        for effect in &stack.effects {
            if !self.effect_runtimes.contains_key(&effect.id) {
                if let Some(gpu_runtime) = registry.create_gpu_runtime(&effect.effect_type, device, queue, format) {
                    self.effect_runtimes.insert(
                        effect.id,
                        EffectRuntimeEntry { gpu: Some(gpu_runtime) },
                    );
                }
            }
        }
    }

    /// Process the effect stack (without automation)
    ///
    /// # Arguments
    /// * `encoder` - Command encoder
    /// * `queue` - GPU queue
    /// * `input` - Input texture view
    /// * `output` - Output texture view
    /// * `stack` - Effect stack definition
    /// * `params` - Base effect parameters (timing, beat)
    pub fn process(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
        input: &wgpu::TextureView,
        output: &wgpu::TextureView,
        stack: &EffectStack,
        base_params: &EffectParams,
    ) {
        self.process_internal(encoder, queue, device, input, output, stack, base_params, None, None)
    }

    /// Process the effect stack with automation support
    ///
    /// # Arguments
    /// * `encoder` - Command encoder
    /// * `queue` - GPU queue
    /// * `input` - Input texture view
    /// * `output` - Output texture view
    /// * `stack` - Effect stack definition
    /// * `params` - Base effect parameters (timing, beat)
    /// * `clock` - BPM clock for LFO/Beat automation
    /// * `audio_manager` - Audio manager for FFT automation
    pub fn process_with_automation(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
        input: &wgpu::TextureView,
        output: &wgpu::TextureView,
        stack: &EffectStack,
        base_params: &EffectParams,
        clock: &super::automation::BpmClock,
        audio_manager: Option<&crate::audio::AudioManager>,
    ) {
        self.process_internal(encoder, queue, device, input, output, stack, base_params, Some(clock), audio_manager)
    }

    /// Internal process implementation
    fn process_internal(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
        input: &wgpu::TextureView,
        output: &wgpu::TextureView,
        stack: &EffectStack,
        base_params: &EffectParams,
        clock: Option<&super::automation::BpmClock>,
        audio_manager: Option<&crate::audio::AudioManager>,
    ) {
        // Get active effects (not bypassed, respecting solo)
        let active_effects: Vec<&EffectInstance> = stack.active_effects().collect();

        if active_effects.is_empty() {
            // No effects: copy input to output
            self.copy_texture(encoder, device, input, output);
            return;
        }

        let pool = match &self.texture_pool {
            Some(p) => p,
            None => {
                // No texture pool: just copy
                self.copy_texture(encoder, device, input, output);
                return;
            }
        };

        // Process effect chain
        for (i, effect) in active_effects.iter().enumerate() {
            // Determine input/output for this effect
            let (effect_input, effect_output) = if active_effects.len() == 1 {
                // Single effect: input -> output directly
                (input, output)
            } else if i == 0 {
                // First effect: input -> pool
                (input, pool.get_views(i).1)
            } else if i == active_effects.len() - 1 {
                // Last effect: pool -> output
                (pool.get_views(i).0, output)
            } else {
                // Middle effect: pool -> pool (ping-pong)
                pool.get_views(i)
            };

            // Build params for this effect, with automation if available
            let mut params = *base_params;
            if let Some(clk) = clock {
                params.pack_parameters_with_automation(&effect.parameters, clk, audio_manager);
            } else {
                params.pack_parameters(&effect.parameters);
            }

            // Process through runtime
            if let Some(entry) = self.effect_runtimes.get_mut(&effect.id) {
                if let Some(gpu) = &mut entry.gpu {
                    // Update runtime with non-numeric params (like strings)
                    gpu.update_from_instance(effect, device, queue);
                    gpu.process(encoder, device, effect_input, effect_output, &params, queue);
                }
            }
        }
    }

    /// Copy a texture to another texture
    fn copy_texture(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
        input: &wgpu::TextureView,
        output: &wgpu::TextureView,
    ) {
        let Some(pipeline) = &self.copy_pipeline else { return };
        let Some(layout) = &self.copy_bind_group_layout else { return };
        let Some(sampler) = &self.sampler else { return };

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Effect Copy Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(input),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Effect Copy Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(pipeline);
        render_pass.set_bind_group(0, &bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }

    /// Check if any effect runtimes exist
    pub fn has_runtimes(&self) -> bool {
        !self.effect_runtimes.is_empty()
    }

    /// Clear all runtimes
    pub fn clear(&mut self) {
        self.effect_runtimes.clear();
    }

    /// Get the input texture view (for rendering layer content before processing)
    pub fn input_view(&self) -> Option<&wgpu::TextureView> {
        self.texture_pool.as_ref().map(|pool| pool.first_view())
    }

    /// Get the output texture view after processing a given number of effects
    pub fn output_view(&self, effect_count: usize) -> Option<&wgpu::TextureView> {
        self.texture_pool.as_ref().map(|pool| pool.output_view(effect_count))
    }

    /// Get texture pool dimensions
    pub fn dimensions(&self) -> Option<(u32, u32)> {
        self.texture_pool.as_ref().map(|pool| pool.dimensions())
    }

    /// Copy an external texture to the effect pool's input texture.
    ///
    /// This uses a simple 1:1 copy with NO transforms applied.
    /// Used to copy video textures to effect input before processing.
    pub fn copy_input_texture(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
        input: &wgpu::TextureView,
    ) {
        if let Some(pool) = &self.texture_pool {
            self.copy_texture(encoder, device, input, pool.first_view());
        }
    }
}

/// Simple copy shader for texture blitting
const COPY_SHADER: &str = r#"
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    // Fullscreen triangle
    let x = f32((vertex_index << 1u) & 2u);
    let y = f32(vertex_index & 2u);
    out.position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    out.uv = vec2<f32>(x, y);
    return out;
}

@group(0) @binding(0) var t_input: texture_2d<f32>;
@group(0) @binding(1) var s_input: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_input, s_input, in.uv);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effect_stack_runtime_new() {
        let runtime = EffectStackRuntime::new();
        assert!(!runtime.has_runtimes());
    }
}
