//! Auto Mask Effect (Resolume-style Luma Key)
//!
//! Keys out dark (or bright) areas of video based on luminance, making them transparent.

use crate::effects::traits::{
    CpuEffectRuntime, EffectDefinition, EffectParams, EffectProcessor, GpuEffectRuntime, ParamBuilder,
};
use crate::effects::types::{Parameter, ParameterMeta};

/// Auto Mask effect definition
pub struct AutoMaskDefinition;

impl EffectDefinition for AutoMaskDefinition {
    fn effect_type(&self) -> &'static str {
        "auto_mask"
    }

    fn display_name(&self) -> &'static str {
        "Auto Mask"
    }

    fn category(&self) -> &'static str {
        "Keying"
    }

    fn processor(&self) -> EffectProcessor {
        EffectProcessor::Gpu
    }

    fn default_parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new(ParameterMeta::float("threshold", "Threshold", 0.1, 0.0, 1.0)),
            Parameter::new(ParameterMeta::float("softness", "Softness", 0.1, 0.0, 0.5)),
            Parameter::new(ParameterMeta::bool("invert", "Invert", false)),
            Parameter::new(ParamBuilder::amount()),
        ]
    }

    fn create_gpu_runtime(
        &self,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        output_format: wgpu::TextureFormat,
    ) -> Option<Box<dyn GpuEffectRuntime>> {
        Some(Box::new(AutoMaskRuntime::new(device, output_format)))
    }

    fn create_cpu_runtime(&self) -> Option<Box<dyn CpuEffectRuntime>> {
        None
    }
}

/// GPU runtime for Auto Mask effect
pub struct AutoMaskRuntime {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    params_buffer: wgpu::Buffer,
    sampler: wgpu::Sampler,
}

impl AutoMaskRuntime {
    /// Create a new auto mask runtime
    pub fn new(device: &wgpu::Device, output_format: wgpu::TextureFormat) -> Self {
        let shader_source = include_str!("../../shaders/effects/auto_mask.wgsl");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Auto Mask Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Auto Mask Bind Group Layout"),
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
            label: Some("Auto Mask Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Auto Mask Pipeline"),
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
        // Layout: time, delta_time, beat_phase, bar_phase, threshold, softness, invert, amount
        // Total: 8 floats = 32 bytes
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Auto Mask Params Buffer"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Auto Mask Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
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

impl GpuEffectRuntime for AutoMaskRuntime {
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
        // [time, delta_time, beat_phase, bar_phase, threshold, softness, invert, amount]
        let invert_f32 = if params.params.len() > 2 && params.params[2] > 0.5 { 1.0 } else { 0.0 };
        let uniform_data: [f32; 8] = [
            params.time,
            params.delta_time,
            params.beat_phase,
            params.bar_phase,
            params.params.first().copied().unwrap_or(0.1),  // threshold
            params.params.get(1).copied().unwrap_or(0.1),   // softness
            invert_f32,                                      // invert (as float)
            params.params.get(3).copied().unwrap_or(1.0),   // amount
        ];

        queue.write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&uniform_data));

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Auto Mask Bind Group"),
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
            label: Some("Auto Mask Pass"),
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
        "auto_mask"
    }
}
