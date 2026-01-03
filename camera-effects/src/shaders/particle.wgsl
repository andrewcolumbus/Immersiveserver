// Particle rendering shader with configurable shapes and colors
// Renders particles as instanced quads with various shapes

struct Particle {
    pos: vec2<f32>,
    vel: vec2<f32>,
    spawn_uv: vec2<f32>,
    life: f32,
    size: f32,
    rotation: f32,
    seed: f32,
    _pad: vec2<f32>,
    color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) spawn_uv: vec2<f32>,
    @location(2) life: f32,
    @location(3) alpha: f32,
    @location(4) particle_color: vec4<f32>,
}

@group(0) @binding(0) var<storage, read> particles: array<Particle>;
@group(0) @binding(1) var camera_texture: texture_2d<f32>;
@group(0) @binding(2) var camera_sampler: sampler;
@group(0) @binding(3) var<uniform> params: ParticleParams;

struct ParticleParams {
    time: f32,
    delta_time: f32,
    spawn_rate: f32,
    particle_lifetime: f32,
    gravity: vec2<f32>,
    wind: vec2<f32>,
    dissolve_threshold: f32,
    turbulence_strength: f32,
    particle_size: f32,
    size_variance: f32,
    velocity_initial: f32,
    drag: f32,
    shape: u32,
    color_mode: u32,
    solid_color: vec4<f32>,
    gradient_start: vec4<f32>,
    gradient_end: vec4<f32>,
    spawn_inside: u32,
    fade_person: f32,
    _pad: vec2<f32>,
}

// Shape constants
const SHAPE_CIRCLE: u32 = 0u;
const SHAPE_SQUARE: u32 = 1u;
const SHAPE_STAR: u32 = 2u;
const SHAPE_HEART: u32 = 3u;
const SHAPE_DIAMOND: u32 = 4u;

// Color mode constants
const COLOR_ORIGINAL: u32 = 0u;
const COLOR_SOLID: u32 = 1u;
const COLOR_RAINBOW: u32 = 2u;
const COLOR_GRADIENT: u32 = 3u;

// Quad vertices (two triangles)
const QUAD_VERTICES: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    vec2<f32>(-0.5, -0.5),
    vec2<f32>(0.5, -0.5),
    vec2<f32>(0.5, 0.5),
    vec2<f32>(-0.5, -0.5),
    vec2<f32>(0.5, 0.5),
    vec2<f32>(-0.5, 0.5),
);

const QUAD_UVS: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    vec2<f32>(0.0, 1.0),
    vec2<f32>(1.0, 1.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(0.0, 1.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(0.0, 0.0),
);

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> VertexOutput {
    var output: VertexOutput;

    let particle = particles[instance_index];

    // Skip dead particles by moving them off-screen
    if particle.life <= 0.0 {
        output.clip_position = vec4<f32>(2.0, 2.0, 0.0, 1.0);
        output.uv = vec2<f32>(0.0, 0.0);
        output.spawn_uv = vec2<f32>(0.0, 0.0);
        output.life = 0.0;
        output.alpha = 0.0;
        output.particle_color = vec4<f32>(0.0);
        return output;
    }

    // Get quad vertex
    let local_pos = QUAD_VERTICES[vertex_index % 6u];
    let local_uv = QUAD_UVS[vertex_index % 6u];

    // Apply rotation
    let c = cos(particle.rotation);
    let s = sin(particle.rotation);
    let rotated = vec2<f32>(
        local_pos.x * c - local_pos.y * s,
        local_pos.x * s + local_pos.y * c
    );

    // Scale and position
    let world_pos = particle.pos + rotated * particle.size;

    // Calculate alpha based on life
    let life_ratio = particle.life / params.particle_lifetime;
    let alpha = smoothstep(0.0, 0.15, life_ratio) * smoothstep(1.0, 0.7, life_ratio);

    output.clip_position = vec4<f32>(world_pos, 0.0, 1.0);
    output.uv = local_uv;
    output.spawn_uv = particle.spawn_uv;
    output.life = particle.life;
    output.alpha = alpha;
    output.particle_color = particle.color;

    return output;
}

// Shape distance functions (SDF)
fn sdf_circle(uv: vec2<f32>) -> f32 {
    return length(uv - vec2<f32>(0.5)) * 2.0;
}

fn sdf_square(uv: vec2<f32>) -> f32 {
    let d = abs(uv - vec2<f32>(0.5)) * 2.0;
    return max(d.x, d.y);
}

fn sdf_diamond(uv: vec2<f32>) -> f32 {
    let d = abs(uv - vec2<f32>(0.5)) * 2.0;
    return (d.x + d.y);
}

fn sdf_star(uv: vec2<f32>) -> f32 {
    let p = (uv - vec2<f32>(0.5)) * 2.0;
    let angle = atan2(p.y, p.x);
    let r = length(p);
    let spikes = 5.0;
    let inner = 0.4;
    let star_r = mix(inner, 1.0, 0.5 + 0.5 * cos(angle * spikes));
    return r / star_r;
}

fn sdf_heart(uv: vec2<f32>) -> f32 {
    var p = (uv - vec2<f32>(0.5)) * 2.2;
    p.y -= 0.3;
    p.y = -p.y;

    let a = atan2(p.x, p.y) / 3.141593;
    let r = length(p);
    let h = abs(a);
    let d = (13.0 * h - 22.0 * h * h + 10.0 * h * h * h) / (6.0 - 5.0 * h);

    return r / d;
}

fn get_shape_alpha(uv: vec2<f32>, shape: u32) -> f32 {
    var dist: f32;

    switch shape {
        case SHAPE_CIRCLE: {
            dist = sdf_circle(uv);
        }
        case SHAPE_SQUARE: {
            dist = sdf_square(uv);
        }
        case SHAPE_STAR: {
            dist = sdf_star(uv);
        }
        case SHAPE_HEART: {
            dist = sdf_heart(uv);
        }
        case SHAPE_DIAMOND: {
            dist = sdf_diamond(uv);
        }
        default: {
            dist = sdf_circle(uv);
        }
    }

    // Soft edge
    return 1.0 - smoothstep(0.8, 1.0, dist);
}

// HSV to RGB conversion
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> vec3<f32> {
    let c = v * s;
    let x = c * (1.0 - abs((h * 6.0) % 2.0 - 1.0));
    let m = v - c;

    var rgb: vec3<f32>;
    let h6 = h * 6.0;

    if h6 < 1.0 {
        rgb = vec3<f32>(c, x, 0.0);
    } else if h6 < 2.0 {
        rgb = vec3<f32>(x, c, 0.0);
    } else if h6 < 3.0 {
        rgb = vec3<f32>(0.0, c, x);
    } else if h6 < 4.0 {
        rgb = vec3<f32>(0.0, x, c);
    } else if h6 < 5.0 {
        rgb = vec3<f32>(x, 0.0, c);
    } else {
        rgb = vec3<f32>(c, 0.0, x);
    }

    return rgb + vec3<f32>(m);
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Skip fully transparent particles
    if input.alpha <= 0.001 {
        discard;
    }

    // Get shape alpha
    let shape_alpha = get_shape_alpha(input.uv, params.shape);
    if shape_alpha <= 0.001 {
        discard;
    }

    // Determine color based on color mode
    var color: vec4<f32>;

    switch params.color_mode {
        case COLOR_ORIGINAL: {
            // Sample from camera texture
            color = textureSample(camera_texture, camera_sampler, input.spawn_uv);
        }
        case COLOR_SOLID: {
            // Use particle's stored color (set at spawn time)
            if input.particle_color.a > 0.0 {
                color = input.particle_color;
            } else {
                color = params.solid_color;
            }
        }
        case COLOR_RAINBOW: {
            // Use particle's stored rainbow color
            if input.particle_color.a > 0.0 {
                color = input.particle_color;
            } else {
                // Generate rainbow based on position and time
                let hue = fract(input.spawn_uv.x + input.spawn_uv.y + params.time * 0.1);
                color = vec4<f32>(hsv_to_rgb(hue, 0.8, 1.0), 1.0);
            }
        }
        case COLOR_GRADIENT: {
            // Interpolate based on life
            let life_ratio = input.life / params.particle_lifetime;
            if input.particle_color.a > 0.0 {
                color = input.particle_color;
            } else {
                color = mix(params.gradient_end, params.gradient_start, life_ratio);
            }
        }
        default: {
            color = textureSample(camera_texture, camera_sampler, input.spawn_uv);
        }
    }

    // Final alpha with shape and life fade
    let final_alpha = color.a * shape_alpha * input.alpha;

    // Brighten slightly
    let final_color = color.rgb * 1.15;

    return vec4<f32>(final_color, final_alpha);
}
