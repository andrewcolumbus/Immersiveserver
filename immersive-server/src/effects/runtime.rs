//! Effect runtime - GPU resources and effect chain processing
//!
//! This module provides the runtime infrastructure for processing effect chains:
//! - `EffectTexturePool` - Ping-pong textures for multi-effect chains
//! - `EffectStackRuntime` - Manages effect instances and processes chains

use std::collections::HashMap;

use super::automation::{BeatEnvelopeState, FftEnvelopeState, TimelineEnvelopeState};
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
    /// Uniform buffer for copy shader params (is_bgra flag)
    copy_params_buffer: Option<wgpu::Buffer>,
    /// FFT envelope states keyed by (effect_id, param_name) for smoothed audio reactivity
    fft_envelope_states: HashMap<(u32, String), FftEnvelopeState>,
    /// Beat envelope states keyed by (effect_id, param_name) for ADSR beat sync
    beat_envelope_states: HashMap<(u32, String), BeatEnvelopeState>,
    /// Timeline envelope states keyed by (effect_id, param_name) for time-based ramps
    timeline_envelope_states: HashMap<(u32, String), TimelineEnvelopeState>,
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
            copy_params_buffer: None,
            fft_envelope_states: HashMap::new(),
            beat_envelope_states: HashMap::new(),
            timeline_envelope_states: HashMap::new(),
        }
    }

    /// Get the current FFT envelope value for a parameter (for UI display)
    pub fn get_fft_envelope_value(&self, effect_id: u32, param_name: &str) -> Option<f32> {
        self.fft_envelope_states
            .get(&(effect_id, param_name.to_string()))
            .map(|e| e.value())
    }

    /// Get the current Beat envelope value for a parameter (for UI display)
    pub fn get_beat_envelope_value(&self, effect_id: u32, param_name: &str) -> Option<f32> {
        self.beat_envelope_states
            .get(&(effect_id, param_name.to_string()))
            .map(|e| e.value())
    }

    /// Get the current timeline envelope value for a parameter (for UI display)
    pub fn get_timeline_envelope_value(&self, effect_id: u32, param_name: &str) -> Option<f32> {
        self.timeline_envelope_states
            .get(&(effect_id, param_name.to_string()))
            .map(|e| e.value())
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Create uniform buffer for copy params (is_bgra flag)
        // Layout: f32 is_bgra (4) + 12 padding + vec3<f32> (12) + 4 struct padding = 32 bytes
        let copy_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Effect Copy Params Buffer"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.copy_params_buffer = Some(copy_params_buffer);

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
    /// Also cleans up orphaned envelope states.
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

        // Clean up envelope states for effects that no longer exist
        self.fft_envelope_states.retain(|(eid, _), _| effect_ids.contains(eid));
        self.beat_envelope_states.retain(|(eid, _), _| effect_ids.contains(eid));
        self.timeline_envelope_states.retain(|(eid, _), _| effect_ids.contains(eid));

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
        self.process_internal(encoder, queue, device, input, output, stack, base_params, None, None, false)
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
        self.process_internal(encoder, queue, device, input, output, stack, base_params, Some(clock), audio_manager, false)
    }

    /// Process the effect stack in-place (same texture for input and output)
    ///
    /// This is used for environment effects where we want to process effects
    /// on a texture without requiring a separate output texture. Internally
    /// uses ping-pong rendering and copies back to the original texture.
    ///
    /// # Arguments
    /// * `encoder` - Command encoder
    /// * `queue` - GPU queue
    /// * `texture` - Texture view to process in-place
    /// * `stack` - Effect stack definition
    /// * `params` - Base effect parameters (timing, beat)
    pub fn process_in_place(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
        texture: &wgpu::TextureView,
        stack: &EffectStack,
        base_params: &EffectParams,
    ) {
        self.process_internal(encoder, queue, device, texture, texture, stack, base_params, None, None, true)
    }

    /// Process the effect stack in-place with automation support
    ///
    /// This is used for environment effects where we want to process effects
    /// on a texture without requiring a separate output texture. Internally
    /// uses ping-pong rendering and copies back to the original texture.
    ///
    /// # Arguments
    /// * `encoder` - Command encoder
    /// * `queue` - GPU queue
    /// * `texture` - Texture view to process in-place
    /// * `stack` - Effect stack definition
    /// * `params` - Base effect parameters (timing, beat)
    /// * `clock` - BPM clock for LFO/Beat automation
    /// * `audio_manager` - Audio manager for FFT automation
    pub fn process_in_place_with_automation(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
        texture: &wgpu::TextureView,
        stack: &EffectStack,
        base_params: &EffectParams,
        clock: &super::automation::BpmClock,
        audio_manager: Option<&crate::audio::AudioManager>,
    ) {
        // Use the same texture for input/output but flag as in_place
        // so process_internal knows to use ping-pong and copy back
        self.process_internal(encoder, queue, device, texture, texture, stack, base_params, Some(clock), audio_manager, true)
    }

    /// Internal process implementation
    ///
    /// # Arguments
    /// * `in_place` - When true, input and output are the same texture, requiring
    ///                special handling to avoid read/write conflicts
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
        in_place: bool,
    ) {
        // Get active effects (not bypassed, respecting solo)
        let active_effects: Vec<&EffectInstance> = stack.active_effects().collect();

        if active_effects.is_empty() {
            // No effects: copy input to output (unless in_place, then nothing to do)
            if !in_place {
                self.copy_texture(encoder, device, queue, input, output, false, (1.0, 1.0));
            }
            return;
        }

        let pool = match &self.texture_pool {
            Some(p) => p,
            None => {
                // No texture pool: just copy if not in_place
                if !in_place {
                    self.copy_texture(encoder, device, queue, input, output, false, (1.0, 1.0));
                }
                return;
            }
        };

        // Process effect chain
        // For in_place processing, we always use the pool and copy back at the end
        for (i, effect) in active_effects.iter().enumerate() {
            // Determine input/output for this effect
            let (effect_input, effect_output) = if in_place {
                // In-place mode: always use pool to avoid read/write conflicts
                if active_effects.len() == 1 {
                    // Single effect: input -> pool[1], then we'll copy pool[1] -> output
                    (input, pool.get_views(0).1)
                } else if i == 0 {
                    // First effect: input -> pool[1]
                    (input, pool.get_views(i).1)
                } else {
                    // Subsequent effects: ping-pong within pool
                    pool.get_views(i)
                }
            } else if active_effects.len() == 1 {
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
                params.pack_parameters_with_automation_and_envelopes(
                    &effect.parameters,
                    clk,
                    audio_manager,
                    base_params.delta_time,
                    &mut self.fft_envelope_states,
                    &mut self.beat_envelope_states,
                    &mut self.timeline_envelope_states,
                    effect.id,
                );
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

        // For in_place mode, copy result back from pool to output texture
        if in_place && !active_effects.is_empty() {
            // Determine which pool texture has the final result
            let final_view = if active_effects.len() == 1 {
                // Single effect wrote to pool[1]
                pool.get_views(0).1
            } else {
                // Multi-effect: result is in alternating buffer based on count
                pool.output_view(active_effects.len())
            };
            self.copy_texture(encoder, device, queue, final_view, output, false, (1.0, 1.0));
        }
    }

    /// Copy a texture to another texture with optional BGRA→RGBA conversion
    /// and size_scale transformation for proper video positioning
    fn copy_texture(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        input: &wgpu::TextureView,
        output: &wgpu::TextureView,
        is_bgra: bool,
        size_scale: (f32, f32),
    ) {
        let Some(pipeline) = &self.copy_pipeline else { return };
        let Some(layout) = &self.copy_bind_group_layout else { return };
        let Some(sampler) = &self.sampler else { return };
        let Some(params_buffer) = &self.copy_params_buffer else { return };

        // Write params to uniform buffer
        // Layout: f32 is_bgra, f32 size_scale_x, f32 size_scale_y, f32 _pad = 16 bytes
        let is_bgra_value: f32 = if is_bgra { 1.0 } else { 0.0 };
        queue.write_buffer(params_buffer, 0, bytemuck::cast_slice(&[
            is_bgra_value,
            size_scale.0,
            size_scale.1,
            0.0f32,
        ]));

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
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: params_buffer.as_entire_binding(),
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
    /// This applies size_scale transformation to properly position video content
    /// within the environment-sized effect texture. Used to copy video textures
    /// to effect input before processing.
    ///
    /// # Arguments
    /// * `is_bgra` - If true, performs R↔B channel swap (for NDI BGRA sources)
    /// * `video_width` - Width of the video source
    /// * `video_height` - Height of the video source
    /// * `env_width` - Width of the environment canvas
    /// * `env_height` - Height of the environment canvas
    pub fn copy_input_texture(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        input: &wgpu::TextureView,
        is_bgra: bool,
        video_width: u32,
        video_height: u32,
        env_width: u32,
        env_height: u32,
    ) {
        if let Some(pool) = &self.texture_pool {
            // Calculate size_scale: how big is the video relative to the environment
            let size_scale = (
                video_width as f32 / env_width as f32,
                video_height as f32 / env_height as f32,
            );
            self.copy_texture(encoder, device, queue, input, pool.first_view(), is_bgra, size_scale);
        }
    }
}

/// Simple copy shader for texture blitting with optional BGRA→RGBA conversion
/// and size_scale transformation for proper video positioning within environment
const COPY_SHADER: &str = r#"
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

struct CopyParams {
    is_bgra: f32,
    size_scale_x: f32,  // video_width / env_width
    size_scale_y: f32,  // video_height / env_height
    _pad: f32,
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
@group(0) @binding(2) var<uniform> params: CopyParams;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Apply size_scale to properly position video within environment space
    // This centers the video and maps environment UVs to video UVs
    var uv = in.uv - 0.5;  // Center origin
    uv = uv / vec2<f32>(params.size_scale_x, params.size_scale_y) + 0.5;

    // Bounds check - outside video area is transparent
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    var color = textureSample(t_input, s_input, uv);
    // Swap R and B channels for BGRA textures (NDI provides BGRA)
    if (params.is_bgra > 0.5) {
        color = vec4<f32>(color.b, color.g, color.r, color.a);
    }
    return color;
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
