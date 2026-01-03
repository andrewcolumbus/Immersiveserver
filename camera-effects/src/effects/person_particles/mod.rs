//! Person to Particles effect
//!
//! Dissolves a person's silhouette into particles that drift away,
//! with configurable amount, size, color, and shape.

use bytemuck::{Pod, Zeroable};
use rand::Rng;

/// Maximum number of particles
pub const MAX_PARTICLES: usize = 100000;

/// Particle shape types
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum ParticleShape {
    #[default]
    Circle = 0,
    Square = 1,
    Star = 2,
    Heart = 3,
    Diamond = 4,
}

/// Color mode for particles
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum ColorMode {
    #[default]
    Original = 0,    // Sample from camera
    Solid = 1,       // Single color
    Rainbow = 2,     // Rainbow based on position/time
    Gradient = 3,    // Gradient based on lifetime
}

/// GPU particle data (64 bytes, aligned for WGSL)
/// Note: vec4 in WGSL requires 16-byte alignment, so we add padding
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Particle {
    /// Position in normalized coordinates [-1, 1]
    pub pos: [f32; 2],
    /// Velocity
    pub vel: [f32; 2],
    /// Original UV for color sampling [0, 1]
    pub spawn_uv: [f32; 2],
    /// Remaining lifetime (0 = dead)
    pub life: f32,
    /// Particle size
    pub size: f32,
    /// Visual rotation
    pub rotation: f32,
    /// Random seed for noise
    pub seed: f32,
    /// Padding to align color to 16 bytes (vec4 alignment in WGSL)
    pub _pad: [f32; 2],
    /// Custom color override (RGBA)
    pub color: [f32; 4],
}

/// Effect parameters uniform buffer (must match shader)
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct ParticleParams {
    pub time: f32,
    pub delta_time: f32,
    pub spawn_rate: f32,
    pub particle_lifetime: f32,
    pub gravity: [f32; 2],
    pub wind: [f32; 2],
    pub dissolve_threshold: f32,
    pub turbulence_strength: f32,
    pub particle_size: f32,
    pub size_variance: f32,
    pub velocity_initial: f32,
    pub drag: f32,
    // New parameters
    pub shape: u32,           // ParticleShape as u32
    pub color_mode: u32,      // ColorMode as u32
    pub solid_color: [f32; 4], // RGBA for solid color mode
    pub gradient_start: [f32; 4], // Start color for gradient
    pub gradient_end: [f32; 4],   // End color for gradient
    pub spawn_inside: u32,    // Spawn inside silhouette (not just edges)
    pub fade_person: f32,     // How much to fade the original person (0-1)
    pub _pad: [f32; 2],
}

impl Default for ParticleParams {
    fn default() -> Self {
        Self {
            time: 0.0,
            delta_time: 1.0 / 60.0,
            spawn_rate: 2000.0,
            particle_lifetime: 3.0,
            gravity: [0.0, 0.1],
            wind: [0.02, 0.0],
            dissolve_threshold: 0.5,
            turbulence_strength: 0.4,
            particle_size: 0.015,
            size_variance: 0.008,
            velocity_initial: 0.05,
            drag: 0.02,
            shape: ParticleShape::Circle as u32,
            color_mode: ColorMode::Original as u32,
            solid_color: [1.0, 0.5, 0.2, 1.0], // Orange default
            gradient_start: [1.0, 0.2, 0.5, 1.0], // Pink
            gradient_end: [0.2, 0.5, 1.0, 1.0],   // Blue
            spawn_inside: 1, // Spawn inside silhouette
            fade_person: 0.3, // Partially fade person
            _pad: [0.0; 2],
        }
    }
}

/// Person to Particles effect runtime
pub struct PersonParticlesEffect {
    /// Particles (CPU side for now)
    particles: Vec<Particle>,
    /// Effect parameters
    params: ParticleParams,
    /// Elapsed time
    time: f32,
    /// Previous segmentation mask for edge detection
    prev_mask: Vec<f32>,
    /// Mask dimensions
    mask_width: u32,
    mask_height: u32,
    /// Spawn accumulator
    spawn_accumulator: f32,
    /// Random generator
    rng: rand::rngs::ThreadRng,
    /// Current shape
    shape: ParticleShape,
    /// Current color mode
    color_mode: ColorMode,
}

impl PersonParticlesEffect {
    /// Create a new effect instance
    pub fn new() -> Self {
        Self {
            particles: Vec::with_capacity(MAX_PARTICLES),
            params: ParticleParams::default(),
            time: 0.0,
            prev_mask: Vec::new(),
            mask_width: 0,
            mask_height: 0,
            spawn_accumulator: 0.0,
            rng: rand::rng(),
            shape: ParticleShape::Circle,
            color_mode: ColorMode::Original,
        }
    }

    /// Update the effect with segmentation result
    pub fn update(
        &mut self,
        delta_time: f32,
        segmentation: Option<&crate::ml::SegmentationResult>,
    ) {
        self.time += delta_time;
        self.params.time = self.time;
        self.params.delta_time = delta_time;

        // Update existing particles
        self.particles.retain_mut(|p| {
            if p.life <= 0.0 {
                return false;
            }

            // Age particle
            p.life -= delta_time;

            // Apply turbulence (simplex noise approximation)
            let noise_x = ((p.pos[0] * 10.0 + self.time * 2.0).sin()
                + (p.pos[1] * 7.0 + self.time * 1.5).cos())
                * self.params.turbulence_strength
                * delta_time;
            let noise_y = ((p.pos[0] * 8.0 - self.time * 1.8).cos()
                + (p.pos[1] * 9.0 + self.time * 2.2).sin())
                * self.params.turbulence_strength
                * delta_time;

            // Apply forces
            p.vel[0] += self.params.gravity[0] * delta_time + noise_x;
            p.vel[1] += self.params.gravity[1] * delta_time + noise_y;
            p.vel[0] += self.params.wind[0] * delta_time;
            p.vel[1] += self.params.wind[1] * delta_time;

            // Apply drag
            let drag_factor = 1.0 - self.params.drag * delta_time;
            p.vel[0] *= drag_factor;
            p.vel[1] *= drag_factor;

            // Update position
            p.pos[0] += p.vel[0] * delta_time;
            p.pos[1] += p.vel[1] * delta_time;

            // Update rotation based on velocity
            let speed = (p.vel[0] * p.vel[0] + p.vel[1] * p.vel[1]).sqrt();
            p.rotation += speed * delta_time * 3.0 + p.seed * delta_time;

            // Fade size as particle ages
            let life_ratio = p.life / self.params.particle_lifetime;
            p.size = self.params.particle_size * (0.3 + 0.7 * life_ratio);

            true
        });

        // Spawn new particles from segmentation
        if let Some(seg) = segmentation {
            self.spawn_from_mask(&seg.mask, seg.width, seg.height, delta_time);
        }
    }

    /// Spawn particles from segmentation mask
    fn spawn_from_mask(&mut self, mask: &[f32], width: u32, height: u32, delta_time: f32) {
        // Update spawn accumulator
        self.spawn_accumulator += self.params.spawn_rate * delta_time;
        let particles_to_spawn = self.spawn_accumulator as usize;
        self.spawn_accumulator -= particles_to_spawn as f32;

        if particles_to_spawn == 0 || self.particles.len() >= MAX_PARTICLES {
            return;
        }

        let spawn_inside = self.params.spawn_inside != 0;

        // Find spawn pixels
        let mut spawn_pixels: Vec<(u32, u32, f32)> = Vec::new();

        for y in 1..(height - 1) {
            for x in 1..(width - 1) {
                let idx = (y * width + x) as usize;
                let current = mask[idx];

                if spawn_inside {
                    // Spawn anywhere inside the person silhouette
                    if current >= self.params.dissolve_threshold {
                        spawn_pixels.push((x, y, current));
                    }
                } else {
                    // Only spawn at edges
                    if current < self.params.dissolve_threshold {
                        continue;
                    }

                    let is_edge = mask[idx - 1] < self.params.dissolve_threshold
                        || mask[idx + 1] < self.params.dissolve_threshold
                        || mask[(y - 1) as usize * width as usize + x as usize]
                            < self.params.dissolve_threshold
                        || mask[(y + 1) as usize * width as usize + x as usize]
                            < self.params.dissolve_threshold;

                    if is_edge {
                        spawn_pixels.push((x, y, current));
                    }
                }
            }
        }

        // Spawn particles at random locations
        if !spawn_pixels.is_empty() {
            let spawn_count = particles_to_spawn.min(MAX_PARTICLES - self.particles.len());
            for _ in 0..spawn_count {
                let (px, py, _mask_val) = spawn_pixels[self.rng.random_range(0..spawn_pixels.len())];

                // Convert to normalized coordinates
                let u = px as f32 / width as f32;
                let v = py as f32 / height as f32;

                // Convert to screen space [-1, 1]
                let x = u * 2.0 - 1.0;
                let y = v * 2.0 - 1.0;

                // Random initial velocity
                let angle = self.rng.random_range(0.0..std::f32::consts::TAU);
                let speed = self.params.velocity_initial
                    * (0.5 + self.rng.random_range(0.0..1.0));
                let vx = angle.cos() * speed;
                let vy = angle.sin() * speed;

                // Generate color based on mode
                let color = self.generate_color(u, v);

                let particle = Particle {
                    pos: [x, y],
                    vel: [vx, vy],
                    spawn_uv: [u, v],
                    life: self.params.particle_lifetime * (0.6 + self.rng.random_range(0.0..0.8)),
                    size: self.params.particle_size
                        + self.rng.random_range(-1.0..1.0) * self.params.size_variance,
                    rotation: self.rng.random_range(0.0..std::f32::consts::TAU),
                    seed: self.rng.random_range(-1.0..1.0),
                    _pad: [0.0; 2],
                    color,
                };

                self.particles.push(particle);
            }
        }

        // Store current mask
        self.prev_mask = mask.to_vec();
        self.mask_width = width;
        self.mask_height = height;
    }

    /// Generate color based on current color mode
    fn generate_color(&mut self, u: f32, v: f32) -> [f32; 4] {
        match self.color_mode {
            ColorMode::Original => [0.0, 0.0, 0.0, 0.0], // Shader will sample from texture
            ColorMode::Solid => self.params.solid_color,
            ColorMode::Rainbow => {
                // HSV to RGB based on position and time
                let hue = (u + v + self.time * 0.1) % 1.0;
                let (r, g, b) = hsv_to_rgb(hue, 0.8, 1.0);
                [r, g, b, 1.0]
            }
            ColorMode::Gradient => {
                // Interpolate between gradient colors based on random value
                let t = self.rng.random_range(0.0..1.0);
                [
                    self.params.gradient_start[0] * (1.0 - t) + self.params.gradient_end[0] * t,
                    self.params.gradient_start[1] * (1.0 - t) + self.params.gradient_end[1] * t,
                    self.params.gradient_start[2] * (1.0 - t) + self.params.gradient_end[2] * t,
                    1.0,
                ]
            }
        }
    }

    /// Get particles for rendering
    pub fn particles(&self) -> &[Particle] {
        &self.particles
    }

    /// Get particle count
    pub fn particle_count(&self) -> usize {
        self.particles.len()
    }

    /// Get effect parameters
    pub fn params(&self) -> &ParticleParams {
        &self.params
    }

    /// Get mutable parameters
    pub fn params_mut(&mut self) -> &mut ParticleParams {
        &mut self.params
    }

    /// Set particle shape
    pub fn set_shape(&mut self, shape: ParticleShape) {
        self.shape = shape;
        self.params.shape = shape as u32;
    }

    /// Get current shape
    pub fn shape(&self) -> ParticleShape {
        self.shape
    }

    /// Set color mode
    pub fn set_color_mode(&mut self, mode: ColorMode) {
        self.color_mode = mode;
        self.params.color_mode = mode as u32;
    }

    /// Get current color mode
    pub fn color_mode(&self) -> ColorMode {
        self.color_mode
    }

    /// Set solid color (RGBA)
    pub fn set_solid_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.params.solid_color = [r, g, b, a];
    }

    /// Set gradient colors
    pub fn set_gradient(&mut self, start: [f32; 4], end: [f32; 4]) {
        self.params.gradient_start = start;
        self.params.gradient_end = end;
    }

    /// Set spawn rate (particles per second)
    pub fn set_spawn_rate(&mut self, rate: f32) {
        self.params.spawn_rate = rate.max(0.0);
    }

    /// Set particle size
    pub fn set_particle_size(&mut self, size: f32) {
        self.params.particle_size = size.max(0.001);
    }

    /// Set whether to spawn inside silhouette or just at edges
    pub fn set_spawn_inside(&mut self, inside: bool) {
        self.params.spawn_inside = if inside { 1 } else { 0 };
    }

    /// Set how much to fade the original person
    pub fn set_fade_person(&mut self, fade: f32) {
        self.params.fade_person = fade.clamp(0.0, 1.0);
    }

    /// Set a parameter by name
    pub fn set_param(&mut self, name: &str, value: f32) {
        match name {
            "spawn_rate" => self.params.spawn_rate = value,
            "particle_lifetime" => self.params.particle_lifetime = value,
            "gravity_x" => self.params.gravity[0] = value,
            "gravity_y" => self.params.gravity[1] = value,
            "wind_x" => self.params.wind[0] = value,
            "wind_y" => self.params.wind[1] = value,
            "turbulence" => self.params.turbulence_strength = value,
            "particle_size" => self.params.particle_size = value,
            "dissolve_threshold" => self.params.dissolve_threshold = value,
            "drag" => self.params.drag = value,
            "fade_person" => self.params.fade_person = value,
            _ => {}
        }
    }

    /// Clear all particles
    pub fn clear(&mut self) {
        self.particles.clear();
    }

    /// Spawn test particles for debugging
    pub fn spawn_test_particles(&mut self, count: usize) {
        for _ in 0..count {
            if self.particles.len() >= MAX_PARTICLES {
                break;
            }

            let x = self.rng.random_range(-0.8..0.8);
            let y = self.rng.random_range(-0.8..0.8);
            let angle = self.rng.random_range(0.0..std::f32::consts::TAU);
            let speed = self.params.velocity_initial * (0.5 + self.rng.random_range(0.0..1.0));
            let u = (x + 1.0) / 2.0;
            let v = (y + 1.0) / 2.0;
            let color = self.generate_color(u, v);

            let particle = Particle {
                pos: [x, y],
                vel: [angle.cos() * speed, angle.sin() * speed],
                spawn_uv: [u, v],
                life: self.params.particle_lifetime * (0.6 + self.rng.random_range(0.0..0.8)),
                size: self.params.particle_size
                    + self.rng.random_range(-1.0..1.0) * self.params.size_variance,
                rotation: self.rng.random_range(0.0..std::f32::consts::TAU),
                seed: self.rng.random_range(-1.0..1.0),
                _pad: [0.0; 2],
                color,
            };

            self.particles.push(particle);
        }
    }
}

impl Default for PersonParticlesEffect {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert HSV to RGB (h in 0-1, s in 0-1, v in 0-1)
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let h = h * 6.0;
    let i = h.floor() as i32;
    let f = h - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));

    match i % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    }
}
