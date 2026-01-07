//! Multiplex Effect
//!
//! Tiles/repeats the input texture in a grid pattern.

use crate::effects::traits::{
    CpuEffectRuntime, EffectDefinition, EffectParams, EffectProcessor, GpuEffectRuntime,
};
use crate::effects::types::{Parameter, ParameterMeta};

/// Multiplex effect definition
pub struct MultiplexDefinition;

impl EffectDefinition for MultiplexDefinition {
    fn effect_type(&self) -> &'static str {
        "multiplex"
    }

    fn display_name(&self) -> &'static str {
        "Multiplex"
    }

    fn category(&self) -> &'static str {
        "Distort"
    }

    fn processor(&self) -> EffectProcessor {
        EffectProcessor::Gpu
    }

    fn default_parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new(ParameterMeta::float_with_step("tile_x", "Tiles X", 2.0, 1.0, 16.0, 1.0)),
            Parameter::new(ParameterMeta::float_with_step("tile_y", "Tiles Y", 2.0, 1.0, 16.0, 1.0)),
            Parameter::new(ParameterMeta::float("spacing_x", "Spacing X", 0.0, 0.0, 1.0)),
            Parameter::new(ParameterMeta::float("spacing_y", "Spacing Y", 0.0, 0.0, 1.0)),
        ]
    }

    fn create_gpu_runtime(
        &self,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        output_format: wgpu::TextureFormat,
    ) -> Option<Box<dyn GpuEffectRuntime>> {
        Some(Box::new(MultiplexRuntime::new(device, output_format)))
    }

    fn create_cpu_runtime(&self) -> Option<Box<dyn CpuEffectRuntime>> {
        None
    }
}

/// GPU runtime for Multiplex effect
pub struct MultiplexRuntime {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    params_buffer: wgpu::Buffer,
    sampler: wgpu::Sampler,
}

impl MultiplexRuntime {
    /// Create a new multiplex runtime
    pub fn new(device: &wgpu::Device, output_format: wgpu::TextureFormat) -> Self {
        let shader_source = include_str!("../../shaders/effects/multiplex.wgsl");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Multiplex Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Multiplex Bind Group Layout"),
            entries: &[
                // Input texture
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
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // Parameters uniform
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Multiplex Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Multiplex Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: output_format,
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

        // Create uniform buffer for parameters
        // Layout: time, delta_time, beat_phase, bar_phase, tile_x, tile_y, spacing_x, spacing_y,
        //         size_scale_x, size_scale_y, _pad, _pad
        // Total: 12 floats = 48 bytes
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Multiplex Params Buffer"),
            size: 48,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Use Repeat address mode for tiling
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Multiplex Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Self {
            pipeline,
            bind_group_layout,
            params_buffer,
            sampler,
        }
    }
}

impl GpuEffectRuntime for MultiplexRuntime {
    fn process(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
        input: &wgpu::TextureView,
        output: &wgpu::TextureView,
        params: &EffectParams,
        queue: &wgpu::Queue,
    ) {
        // Pack parameters into uniform buffer format:
        // [time, delta_time, beat_phase, bar_phase, tile_x, tile_y, spacing_x, spacing_y,
        //  size_scale_x, size_scale_y, pad, pad]
        let uniform_data: [f32; 12] = [
            params.time,
            params.delta_time,
            params.beat_phase,
            params.bar_phase,
            params.params[0], // tile_x
            params.params[1], // tile_y
            params.params[2], // spacing_x
            params.params[3], // spacing_y
            params.params[26], // size_scale_x (from reserved slots)
            params.params[27], // size_scale_y
            0.0, 0.0,          // padding
        ];

        queue.write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&uniform_data));

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Multiplex Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(input),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.params_buffer.as_entire_binding(),
                },
            ],
        });

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Multiplex Pass"),
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

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }

    fn rebuild(&mut self, _device: &wgpu::Device, _shader_source: &str) -> Result<(), String> {
        // TODO: Implement hot-reload
        Ok(())
    }

    fn effect_type(&self) -> &'static str {
        "multiplex"
    }
}
