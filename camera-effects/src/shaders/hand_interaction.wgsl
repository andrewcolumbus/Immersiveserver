// Hand Interaction compute shader
// Applies forces to particles based on hand landmark positions

struct Particle {
    pos: vec2<f32>,
    vel: vec2<f32>,
    spawn_uv: vec2<f32>,
    life: f32,
    size: f32,
    rotation: f32,
    seed: f32,
    _pad: vec2<f32>,
}

struct HandPoint {
    pos: vec2<f32>,
    active: f32,
    influence: f32,
}

struct HandData {
    points: array<HandPoint, 44>,
    hand_count: u32,
    _pad: vec3<u32>,
}

struct HandInteractionParams {
    time: f32,
    delta_time: f32,
    mode: u32,           // 0=attract, 1=repel, 2=swirl, 3=push
    force_strength: f32,
    force_radius: f32,
    max_velocity: f32,
    palm_force_multiplier: f32,
    fingertip_force_multiplier: f32,
}

@group(0) @binding(0) var<storage, read_write> particles: array<Particle>;
@group(0) @binding(1) var<uniform> params: HandInteractionParams;
@group(0) @binding(2) var<uniform> hand_data: HandData;
@group(0) @binding(3) var<uniform> particle_count: u32;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let idx = global_id.x;
    if idx >= particle_count {
        return;
    }

    var p = particles[idx];

    // Skip dead particles
    if p.life <= 0.0 {
        return;
    }

    var total_force = vec2<f32>(0.0, 0.0);

    // Calculate forces from all active hand points
    for (var i = 0u; i < 44u; i++) {
        let point = hand_data.points[i];
        if point.active < 0.5 {
            continue;
        }

        // Calculate distance to hand point
        let delta = point.pos - p.pos;
        let dist = length(delta);

        if dist > params.force_radius || dist < 0.001 {
            continue;
        }

        // Calculate force falloff
        let falloff = 1.0 - (dist / params.force_radius);
        let force_magnitude = params.force_strength * falloff * point.influence;

        var force: vec2<f32>;

        switch params.mode {
            case 0u: { // Attract
                force = normalize(delta) * force_magnitude;
            }
            case 1u: { // Repel
                force = -normalize(delta) * force_magnitude;
            }
            case 2u: { // Swirl
                let perpendicular = vec2<f32>(-delta.y, delta.x);
                force = normalize(perpendicular) * force_magnitude;
            }
            case 3u: { // Push (based on hand movement - TODO: need velocity)
                force = -normalize(delta) * force_magnitude * 0.5;
            }
            default: {
                force = vec2<f32>(0.0, 0.0);
            }
        }

        total_force += force;
    }

    // Apply force to velocity
    p.vel += total_force * params.delta_time;

    // Clamp velocity
    let speed = length(p.vel);
    if speed > params.max_velocity {
        p.vel = normalize(p.vel) * params.max_velocity;
    }

    particles[idx] = p;
}
