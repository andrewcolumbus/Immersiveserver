//! Image Rain Effect
//!
//! Rains emojis or custom images on video, using edge detection to make them land on shapes.
//! Uses instanced quad rendering for performance.

use crate::effects::traits::{
    CpuEffectRuntime, EffectDefinition, EffectParams, EffectProcessor, GpuEffectRuntime,
};
use crate::effects::EffectInstance;
use crate::effects::types::{Parameter, ParameterMeta};
use bytemuck::{Pod, Zeroable};
use rand::Rng;
use std::path::Path;
use std::process::Command;

const MAX_PARTICLES: usize = 200;

// Available emoji presets
const EMOJI_PRESETS: &[(&str, &str)] = &[
    ("💩", "Poop"),
    ("🔥", "Fire"),
    ("❤️", "Heart"),
    ("⭐", "Star"),
    ("🎉", "Party"),
    ("💀", "Skull"),
    ("🌟", "Sparkle"),
    ("🎈", "Balloon"),
    ("😂", "Laughing"),
    ("👻", "Ghost"),
];

/// Image Rain effect definition
pub struct ImageRainDefinition;

impl EffectDefinition for ImageRainDefinition {
    fn effect_type(&self) -> &'static str {
        "image_rain"
    }

    fn display_name(&self) -> &'static str {
        "Image Rain"
    }

    fn category(&self) -> &'static str {
        "Generate"
    }

    fn processor(&self) -> EffectProcessor {
        EffectProcessor::Gpu
    }

    fn default_parameters(&self) -> Vec<Parameter> {
        let emoji_options: Vec<String> = EMOJI_PRESETS
            .iter()
            .map(|(emoji, name)| format!("{} {}", emoji, name))
            .collect();

        vec![
            Parameter::new(ParameterMeta::enumeration(
                "emoji",
                "Emoji",
                emoji_options,
                0,
            )),
            Parameter::new(ParameterMeta::string(
                "custom_image",
                "Custom Image",
                "",
            )),
            Parameter::new(ParameterMeta::float("density", "Density", 0.5, 0.0, 1.0)),
            Parameter::new(ParameterMeta::enumeration(
                "landing_mode",
                "Landing Mode",
                vec!["Pile Up".to_string(), "Bounce".to_string(), "Splat".to_string()],
                0,
            )),
            Parameter::new(ParameterMeta::float("gravity", "Gravity", 1.0, 0.1, 3.0)),
            Parameter::new(ParameterMeta::float("edge_threshold", "Edge Sensitivity", 0.3, 0.0, 1.0)),
            Parameter::new(ParameterMeta::float("particle_size", "Size", 1.0, 0.5, 2.0)),
            Parameter::new(ParameterMeta::float("bounce_dampening", "Bounce Energy", 0.6, 0.0, 1.0)),
            Parameter::new(ParameterMeta::float("fade_duration", "Fade Time", 2.0, 0.5, 5.0)),
        ]
    }

    fn create_gpu_runtime(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        output_format: wgpu::TextureFormat,
    ) -> Option<Box<dyn GpuEffectRuntime>> {
        Some(Box::new(ImageRainRuntime::new(device, queue, output_format)))
    }

    fn create_cpu_runtime(&self) -> Option<Box<dyn CpuEffectRuntime>> {
        None
    }
}

/// Quad vertex for instanced rendering
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct QuadVertex {
    local_pos: [f32; 2],
    local_uv: [f32; 2],
}

/// Per-instance particle data (GPU)
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct ParticleInstance {
    pos: [f32; 2],
    size_rot: [f32; 2],
    alpha: f32,
    _pad: [f32; 3],
}

/// CPU-side particle state (includes velocity, etc.)
#[derive(Clone)]
struct Particle {
    x: f32,
    y: f32,
    vel_x: f32,
    vel_y: f32,
    size: f32,
    rotation: f32,
    rot_speed: f32,
    alpha: f32,
    landed: bool,
    fade_start_time: f32,
    landing_mode: u32,
}

impl Particle {
    fn new(rng: &mut impl Rng, size_scale: f32) -> Self {
        Self {
            x: rng.random::<f32>(),
            y: -0.1,
            vel_x: (rng.random::<f32>() - 0.5) * 0.02,
            vel_y: 0.0,
            size: (0.03 + rng.random::<f32>() * 0.04) * size_scale,
            rotation: rng.random::<f32>() * std::f32::consts::TAU,
            rot_speed: (rng.random::<f32>() - 0.5) * 2.0,
            alpha: 1.0,
            landed: false,
            fade_start_time: 0.0,
            landing_mode: 0,
        }
    }
}

/// GPU runtime for Image Rain effect
pub struct ImageRainRuntime {
    // Edge detection pass
    edge_pipeline: wgpu::RenderPipeline,
    edge_bind_group_layout: wgpu::BindGroupLayout,
    edge_texture: Option<wgpu::Texture>,
    edge_texture_view: Option<wgpu::TextureView>,
    edge_width: u32,
    edge_height: u32,

    // Copy pass (input -> output before particles)
    copy_pipeline: wgpu::RenderPipeline,
    copy_bind_group_layout: wgpu::BindGroupLayout,

    // Particle render pass (instanced)
    particle_pipeline: wgpu::RenderPipeline,
    particle_bind_group_layout: wgpu::BindGroupLayout,
    quad_vertex_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,

    // Image texture (emoji or custom)
    image_texture: wgpu::Texture,
    image_texture_view: wgpu::TextureView,
    current_emoji_index: i32,
    current_custom_path: String,

    // Shared sampler
    sampler: wgpu::Sampler,

    // CPU particle simulation
    particles: Vec<Particle>,
    spawn_accumulator: f32,
    last_time: f32,

    // Edge data readback (for collision) - lower res for performance
    collision_texture: Option<wgpu::Texture>,
    collision_texture_view: Option<wgpu::TextureView>,
    collision_staging_buffer: Option<wgpu::Buffer>,
    edge_data: Vec<u8>,
    collision_width: u32,
    collision_height: u32,
    readback_pending: bool,
}

impl ImageRainRuntime {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, output_format: wgpu::TextureFormat) -> Self {
        // Create edge detection pipeline
        let edge_shader_source = include_str!("../../shaders/effects/poop_rain_edges.wgsl");
        let edge_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Image Rain Edge Shader"),
            source: wgpu::ShaderSource::Wgsl(edge_shader_source.into()),
        });

        let edge_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Image Rain Edge Bind Group Layout"),
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

        let edge_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Image Rain Edge Pipeline Layout"),
            bind_group_layouts: &[&edge_bind_group_layout],
            push_constant_ranges: &[],
        });

        let edge_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Image Rain Edge Pipeline"),
            layout: Some(&edge_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &edge_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &edge_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
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

        // Create copy pipeline
        let copy_shader_source = include_str!("../../shaders/effects/poop_rain_copy.wgsl");
        let copy_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Image Rain Copy Shader"),
            source: wgpu::ShaderSource::Wgsl(copy_shader_source.into()),
        });

        let copy_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Image Rain Copy Bind Group Layout"),
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

        let copy_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Image Rain Copy Pipeline Layout"),
            bind_group_layouts: &[&copy_bind_group_layout],
            push_constant_ranges: &[],
        });

        let copy_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Image Rain Copy Pipeline"),
            layout: Some(&copy_pipeline_layout),
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

        // Create particle pipeline (instanced rendering)
        let particle_shader_source = include_str!("../../shaders/effects/poop_rain.wgsl");
        let particle_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Image Rain Particle Shader"),
            source: wgpu::ShaderSource::Wgsl(particle_shader_source.into()),
        });

        let particle_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Image Rain Particle Bind Group Layout"),
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

        let particle_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Image Rain Particle Pipeline Layout"),
            bind_group_layouts: &[&particle_bind_group_layout],
            push_constant_ranges: &[],
        });

        // Vertex buffer layout for quad vertices
        let vertex_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<QuadVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: 8,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        };

        // Instance buffer layout for particle data
        let instance_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ParticleInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: 8,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32,
                },
            ],
        };

        let particle_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Image Rain Particle Pipeline"),
            layout: Some(&particle_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &particle_shader,
                entry_point: Some("vs_main"),
                buffers: &[vertex_buffer_layout, instance_buffer_layout],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &particle_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: output_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
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

        // Create quad vertex buffer (2 triangles = 6 vertices)
        let quad_vertices: [QuadVertex; 6] = [
            // Triangle 1
            QuadVertex { local_pos: [-0.5, -0.5], local_uv: [0.0, 1.0] },
            QuadVertex { local_pos: [0.5, -0.5], local_uv: [1.0, 1.0] },
            QuadVertex { local_pos: [0.5, 0.5], local_uv: [1.0, 0.0] },
            // Triangle 2
            QuadVertex { local_pos: [-0.5, -0.5], local_uv: [0.0, 1.0] },
            QuadVertex { local_pos: [0.5, 0.5], local_uv: [1.0, 0.0] },
            QuadVertex { local_pos: [-0.5, 0.5], local_uv: [0.0, 0.0] },
        ];

        let quad_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Image Rain Quad Vertex Buffer"),
            size: std::mem::size_of_val(&quad_vertices) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&quad_vertex_buffer, 0, bytemuck::cast_slice(&quad_vertices));

        // Create instance buffer
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Image Rain Instance Buffer"),
            size: (std::mem::size_of::<ParticleInstance>() * MAX_PARTICLES) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Image Rain Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Create default emoji texture (poop)
        let (image_texture, image_texture_view) = Self::create_emoji_texture(device, queue, "💩");

        Self {
            edge_pipeline,
            edge_bind_group_layout,
            edge_texture: None,
            edge_texture_view: None,
            edge_width: 0,
            edge_height: 0,
            copy_pipeline,
            copy_bind_group_layout,
            particle_pipeline,
            particle_bind_group_layout,
            quad_vertex_buffer,
            instance_buffer,
            image_texture,
            image_texture_view,
            current_emoji_index: 0,
            current_custom_path: String::new(),
            sampler,
            particles: Vec::with_capacity(MAX_PARTICLES),
            spawn_accumulator: 0.0,
            last_time: 0.0,
            collision_texture: None,
            collision_texture_view: None,
            collision_staging_buffer: None,
            edge_data: Vec::new(),
            collision_width: 0,
            collision_height: 0,
            readback_pending: false,
        }
    }

    fn create_emoji_texture(device: &wgpu::Device, queue: &wgpu::Queue, emoji: &str) -> (wgpu::Texture, wgpu::TextureView) {
        // Try to generate emoji using Swift on macOS
        #[cfg(target_os = "macos")]
        if let Some((texture, view)) = Self::generate_emoji_texture_macos(device, queue, emoji) {
            return (texture, view);
        }

        // Fallback: use embedded poop emoji
        Self::create_fallback_texture(device, queue)
    }

    #[cfg(target_os = "macos")]
    fn generate_emoji_texture_macos(device: &wgpu::Device, queue: &wgpu::Queue, emoji: &str) -> Option<(wgpu::Texture, wgpu::TextureView)> {
        // Create temp directory for emoji generation
        let temp_dir = std::env::temp_dir();
        let swift_path = temp_dir.join("emoji_gen.swift");
        let png_path = temp_dir.join(format!("emoji_{}.png", emoji.chars().next().map(|c| c as u32).unwrap_or(0)));

        // Write Swift script
        let swift_code = r#"
import Cocoa

let emoji = CommandLine.arguments[1]
let outputPath = CommandLine.arguments[2]
let size = CGSize(width: 256, height: 256)
let image = NSImage(size: size)

image.lockFocus()
NSColor.clear.set()
NSRect(origin: .zero, size: size).fill()

let font = NSFont.systemFont(ofSize: 200)
let attrs: [NSAttributedString.Key: Any] = [.font: font]
let attrString = NSAttributedString(string: emoji, attributes: attrs)
let stringSize = attrString.size()

let point = NSPoint(
    x: (size.width - stringSize.width) / 2,
    y: (size.height - stringSize.height) / 2 - 10
)
attrString.draw(at: point)
image.unlockFocus()

guard let tiffData = image.tiffRepresentation,
      let bitmap = NSBitmapImageRep(data: tiffData),
      let pngData = bitmap.representation(using: .png, properties: [:]) else {
    exit(1)
}

try! pngData.write(to: URL(fileURLWithPath: outputPath))
"#;

        std::fs::write(&swift_path, swift_code).ok()?;

        // Run Swift to generate emoji
        let output = Command::new("swift")
            .arg(&swift_path)
            .arg(emoji)
            .arg(&png_path)
            .output()
            .ok()?;

        if !output.status.success() {
            log::warn!("Failed to generate emoji texture: {:?}", String::from_utf8_lossy(&output.stderr));
            return None;
        }

        // Load generated PNG
        let png_data = std::fs::read(&png_path).ok()?;
        let img = image::load_from_memory(&png_data).ok()?.to_rgba8();
        let (width, height) = img.dimensions();
        let pixels = img.into_raw();

        // Clean up temp files
        let _ = std::fs::remove_file(&swift_path);
        let _ = std::fs::remove_file(&png_path);

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Emoji Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Some((texture, view))
    }

    fn create_fallback_texture(device: &wgpu::Device, queue: &wgpu::Queue) -> (wgpu::Texture, wgpu::TextureView) {
        // Load embedded poop emoji as fallback
        let png_data = include_bytes!("poop_emoji.png");
        let img = image::load_from_memory(png_data)
            .expect("Failed to load embedded emoji")
            .to_rgba8();

        let (width, height) = img.dimensions();
        let pixels = img.into_raw();

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Fallback Emoji Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    fn update_emoji_texture(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, emoji_index: i32) {
        // Only update if emoji changed and no custom image is set
        if self.current_custom_path.is_empty()
            && emoji_index != self.current_emoji_index
            && (emoji_index as usize) < EMOJI_PRESETS.len()
        {
            let emoji = EMOJI_PRESETS[emoji_index as usize].0;
            let (texture, view) = Self::create_emoji_texture(device, queue, emoji);
            self.image_texture = texture;
            self.image_texture_view = view;
            self.current_emoji_index = emoji_index;
        }
    }

    fn update_custom_image(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, custom_path: &str) {
        // Custom image takes priority if set
        if !custom_path.is_empty() {
            // Only reload if path changed
            if custom_path != self.current_custom_path {
                if let Some((texture, view)) = Self::load_custom_image(device, queue, custom_path) {
                    self.image_texture = texture;
                    self.image_texture_view = view;
                    self.current_custom_path = custom_path.to_string();
                    self.current_emoji_index = -1; // Mark as custom
                    log::info!("Loaded custom image: {}", custom_path);
                } else {
                    log::warn!("Failed to load custom image: {}", custom_path);
                }
            }
        } else if !self.current_custom_path.is_empty() {
            // Clear custom path - will fall back to emoji on next frame
            self.current_custom_path.clear();
        }
    }

    fn load_custom_image(device: &wgpu::Device, queue: &wgpu::Queue, path: &str) -> Option<(wgpu::Texture, wgpu::TextureView)> {
        let path = Path::new(path);
        if !path.exists() {
            return None;
        }

        // Load image
        let img_data = std::fs::read(path).ok()?;
        let img = image::load_from_memory(&img_data).ok()?.to_rgba8();
        let (width, height) = img.dimensions();

        // Resize if too large (max 512x512 for particles)
        let (width, height, pixels) = if width > 512 || height > 512 {
            let scale = 512.0 / width.max(height) as f32;
            let new_width = (width as f32 * scale) as u32;
            let new_height = (height as f32 * scale) as u32;
            let resized = image::imageops::resize(&img, new_width, new_height, image::imageops::FilterType::Lanczos3);
            (new_width, new_height, resized.into_raw())
        } else {
            (width, height, img.into_raw())
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Custom Image Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Some((texture, view))
    }

    fn ensure_edge_texture(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.edge_width != width || self.edge_height != height {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Image Rain Edge Texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });

            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

            self.edge_texture = Some(texture);
            self.edge_texture_view = Some(view);
            self.edge_width = width;
            self.edge_height = height;

            // Create lower-res collision texture for CPU readback (256x144 for 16:9)
            let coll_width = 256u32;
            let coll_height = 144u32;

            let collision_texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Image Rain Collision Texture"),
                size: wgpu::Extent3d {
                    width: coll_width,
                    height: coll_height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });

            let collision_view = collision_texture.create_view(&wgpu::TextureViewDescriptor::default());

            self.collision_texture = Some(collision_texture);
            self.collision_texture_view = Some(collision_view);

            // Staging buffer for CPU readback - rows must be aligned to 256 bytes
            let bytes_per_row = coll_width * 4;
            let padded_bytes_per_row = ((bytes_per_row + 255) / 256) * 256;
            let buffer_size = (padded_bytes_per_row * coll_height) as wgpu::BufferAddress;

            self.collision_staging_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Image Rain Collision Staging Buffer"),
                size: buffer_size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }));

            self.edge_data = vec![0u8; (coll_width * coll_height * 4) as usize];
            self.collision_width = coll_width;
            self.collision_height = coll_height;
        }
    }

    fn simulate_particles(&mut self, params: &EffectParams) {
        let time = params.time;
        let dt = if self.last_time == 0.0 { 1.0 / 60.0 } else { (time - self.last_time).min(0.1) };
        self.last_time = time;

        // Parameters: [0]=emoji, [1]=density, [2]=landing_mode, etc.
        let density = params.params[1];
        let landing_mode = params.params[2] as u32;
        let gravity = params.params[3] * 0.3;
        let edge_threshold = params.params[4];
        let size_scale = params.params[5];
        let bounce_dampening = params.params[6];
        let fade_duration = params.params[7];

        // Spawn new particles
        let spawn_rate = density * 30.0;
        self.spawn_accumulator += spawn_rate * dt;
        let mut rng = rand::rng();

        while self.spawn_accumulator >= 1.0 && self.particles.len() < MAX_PARTICLES {
            self.spawn_accumulator -= 1.0;
            let mut p = Particle::new(&mut rng, size_scale);
            p.landing_mode = landing_mode;
            self.particles.push(p);
        }

        // Copy edge data refs to avoid borrow issues
        let edge_data = &self.edge_data;
        let edge_width = self.collision_width;
        let edge_height = self.collision_height;

        // Helper closure to sample edge (avoids self borrow)
        let sample_edge = |x: f32, y: f32| -> bool {
            if edge_data.is_empty() {
                return false;
            }
            let px = (x * edge_width as f32).clamp(0.0, (edge_width - 1) as f32) as u32;
            let py = (y * edge_height as f32).clamp(0.0, (edge_height - 1) as f32) as u32;
            let idx = ((py * edge_width + px) * 4) as usize;
            if idx < edge_data.len() {
                let edge_value = edge_data[idx] as f32 / 255.0;
                edge_value > edge_threshold
            } else {
                false
            }
        };

        // Update particles
        for particle in &mut self.particles {
            if particle.landed {
                // Fading out
                let fade_progress = (time - particle.fade_start_time) / fade_duration;
                particle.alpha = (1.0 - fade_progress).max(0.0);
            } else {
                // Apply gravity
                particle.vel_y += gravity * dt;

                // Update position
                particle.x += particle.vel_x * dt;
                particle.y += particle.vel_y * dt;
                particle.rotation += particle.rot_speed * dt;

                // Check for edge collision
                let hit_edge = sample_edge(particle.x, particle.y + particle.size * 0.5);
                let hit_bottom = particle.y > 1.0;

                if hit_edge || hit_bottom {
                    match landing_mode {
                        0 => {
                            // Pile Up - stop and fade
                            particle.landed = true;
                            particle.fade_start_time = time;
                            particle.vel_x = 0.0;
                            particle.vel_y = 0.0;
                        }
                        1 => {
                            // Bounce
                            particle.vel_y = -particle.vel_y * bounce_dampening;
                            particle.vel_x *= 0.9;
                            particle.y -= particle.vel_y.abs() * dt * 2.0;

                            if particle.vel_y.abs() < 0.01 {
                                particle.landed = true;
                                particle.fade_start_time = time;
                            }
                        }
                        _ => {
                            // Splat - stop immediately and fade faster
                            particle.landed = true;
                            particle.fade_start_time = time;
                            particle.vel_x = 0.0;
                            particle.vel_y = 0.0;
                        }
                    }
                }
            }
        }

        // Remove dead particles
        self.particles.retain(|p| p.alpha > 0.01);
    }
}

impl GpuEffectRuntime for ImageRainRuntime {
    fn process(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
        input: &wgpu::TextureView,
        output: &wgpu::TextureView,
        params: &EffectParams,
        queue: &wgpu::Queue,
    ) {
        let width = 1920u32;
        let height = 1080u32;

        // Check if we need to update the emoji texture (if no custom image set)
        let emoji_index = params.params[0] as i32;
        self.update_emoji_texture(device, queue, emoji_index);

        // Ensure edge texture exists
        self.ensure_edge_texture(device, width, height);

        let edge_view = self.edge_texture_view.as_ref().unwrap();
        let collision_view = self.collision_texture_view.as_ref().unwrap();

        // Pass 1a: Edge detection (full res for display)
        {
            let edge_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Image Rain Edge Bind Group"),
                layout: &self.edge_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(input),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });

            let mut edge_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Image Rain Edge Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: edge_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            edge_pass.set_pipeline(&self.edge_pipeline);
            edge_pass.set_bind_group(0, &edge_bind_group, &[]);
            edge_pass.draw(0..3, 0..1);
        }

        // Pass 1b: Edge detection (low res for collision)
        {
            let edge_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Image Rain Collision Edge Bind Group"),
                layout: &self.edge_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(input),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });

            let mut coll_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Image Rain Collision Edge Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: collision_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            coll_pass.set_pipeline(&self.edge_pipeline);
            coll_pass.set_bind_group(0, &edge_bind_group, &[]);
            coll_pass.draw(0..3, 0..1);
        }

        // Read previous frame's collision data (synchronous, 1 frame latency)
        if self.readback_pending {
            if let Some(staging_buf) = &self.collision_staging_buffer {
                // Wait for GPU
                device.poll(wgpu::Maintain::Wait);

                let buffer_slice = staging_buf.slice(..);

                // Synchronous map
                let (tx, rx) = std::sync::mpsc::channel();
                buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
                    let _ = tx.send(result);
                });

                device.poll(wgpu::Maintain::Wait);

                if rx.recv().ok().and_then(|r| r.ok()).is_some() {
                    let data = buffer_slice.get_mapped_range();

                    let bytes_per_row = self.collision_width * 4;
                    let padded_bytes_per_row = ((bytes_per_row + 255) / 256) * 256;

                    for y in 0..self.collision_height {
                        let src_start = (y * padded_bytes_per_row) as usize;
                        let src_end = src_start + bytes_per_row as usize;
                        let dst_start = (y * bytes_per_row) as usize;
                        let dst_end = dst_start + bytes_per_row as usize;

                        if src_end <= data.len() && dst_end <= self.edge_data.len() {
                            self.edge_data[dst_start..dst_end].copy_from_slice(&data[src_start..src_end]);
                        }
                    }

                    drop(data);
                    staging_buf.unmap();
                }
                self.readback_pending = false;
            }
        }

        // Copy collision texture to staging buffer for next frame's readback
        if let (Some(collision_tex), Some(staging_buf)) = (&self.collision_texture, &self.collision_staging_buffer) {
            let bytes_per_row = self.collision_width * 4;
            let padded_bytes_per_row = ((bytes_per_row + 255) / 256) * 256;

            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: collision_tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: staging_buf,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(padded_bytes_per_row),
                        rows_per_image: Some(self.collision_height),
                    },
                },
                wgpu::Extent3d {
                    width: self.collision_width,
                    height: self.collision_height,
                    depth_or_array_layers: 1,
                },
            );
            self.readback_pending = true;
        }

        // Pass 2: Copy input to output
        {
            let copy_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Image Rain Copy Bind Group"),
                layout: &self.copy_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(input),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });

            let mut copy_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Image Rain Copy Pass"),
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

            copy_pass.set_pipeline(&self.copy_pipeline);
            copy_pass.set_bind_group(0, &copy_bind_group, &[]);
            copy_pass.draw(0..3, 0..1);
        }

        // Simulate particles (CPU)
        self.simulate_particles(params);

        // Build instance data
        let mut instances: Vec<ParticleInstance> = Vec::with_capacity(self.particles.len());
        for p in &self.particles {
            instances.push(ParticleInstance {
                pos: [p.x, p.y],
                size_rot: [p.size, p.rotation],
                alpha: p.alpha,
                _pad: [0.0; 3],
            });
        }

        // Upload instance data
        if !instances.is_empty() {
            queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&instances));
        }

        // Pass 3: Render particles (instanced)
        if !instances.is_empty() {
            let particle_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Image Rain Particle Bind Group"),
                layout: &self.particle_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&self.image_texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });

            let mut particle_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Image Rain Particle Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: output,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            particle_pass.set_pipeline(&self.particle_pipeline);
            particle_pass.set_bind_group(0, &particle_bind_group, &[]);
            particle_pass.set_vertex_buffer(0, self.quad_vertex_buffer.slice(..));
            particle_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
            particle_pass.draw(0..6, 0..instances.len() as u32);
        }
    }

    fn rebuild(&mut self, _device: &wgpu::Device, _shader_source: &str) -> Result<(), String> {
        Ok(())
    }

    fn effect_type(&self) -> &'static str {
        "image_rain"
    }

    fn update_from_instance(
        &mut self,
        instance: &EffectInstance,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) {
        // Extract custom_image string parameter
        if let Some(custom_path) = instance.get_string("custom_image") {
            self.update_custom_image(device, queue, &custom_path);
        }
    }
}
