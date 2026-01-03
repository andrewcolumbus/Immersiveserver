//! Orbit camera for 3D previs navigation
//!
//! Provides spherical coordinate camera that orbits around a target point.

use glam::{Mat4, Vec3};

/// Orbit camera for 3D previs navigation
pub struct OrbitCamera {
    /// Horizontal angle (yaw) in radians
    yaw: f32,
    /// Vertical angle (pitch) in radians, clamped to avoid gimbal lock
    pitch: f32,
    /// Distance from target point
    distance: f32,
    /// Point the camera orbits around
    target: Vec3,
    /// Aspect ratio (width/height) for projection
    aspect: f32,
    /// Field of view in radians
    fov: f32,
    /// Near clipping plane
    near: f32,
    /// Far clipping plane
    far: f32,
}

impl OrbitCamera {
    /// Create a new orbit camera with default settings
    pub fn new() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.3, // Slight downward angle
            distance: 10.0,
            target: Vec3::ZERO,
            aspect: 16.0 / 9.0,
            fov: std::f32::consts::FRAC_PI_4, // 45 degrees
            near: 0.1,
            far: 100.0,
        }
    }

    /// Get the view matrix
    pub fn view_matrix(&self) -> Mat4 {
        let eye = self.eye_position();
        Mat4::look_at_rh(eye, self.target, Vec3::Y)
    }

    /// Get the projection matrix
    pub fn projection_matrix(&self) -> Mat4 {
        Mat4::perspective_rh(self.fov, self.aspect, self.near, self.far)
    }

    /// Get combined view-projection matrix
    pub fn view_projection_matrix(&self) -> Mat4 {
        self.projection_matrix() * self.view_matrix()
    }

    /// Calculate camera position from spherical coordinates
    pub fn eye_position(&self) -> Vec3 {
        let x = self.distance * self.pitch.cos() * self.yaw.sin();
        let y = self.distance * self.pitch.sin();
        let z = self.distance * self.pitch.cos() * self.yaw.cos();
        self.target + Vec3::new(x, y, z)
    }

    /// Handle mouse drag for orbit
    pub fn on_mouse_drag(&mut self, delta: (f32, f32), sensitivity: f32) {
        self.yaw += delta.0 * sensitivity;
        // Clamp pitch to avoid gimbal lock (~80 degrees)
        self.pitch = (self.pitch - delta.1 * sensitivity).clamp(-1.4, 1.4);
    }

    /// Handle scroll for zoom
    pub fn on_scroll(&mut self, delta: f32) {
        // Multiplicative zoom for smooth feel
        self.distance = (self.distance * (1.0 - delta * 0.1)).clamp(1.0, 50.0);
    }

    /// Update aspect ratio on resize
    pub fn set_aspect(&mut self, aspect: f32) {
        self.aspect = aspect;
    }

    /// Get current yaw
    pub fn yaw(&self) -> f32 {
        self.yaw
    }

    /// Get current pitch
    pub fn pitch(&self) -> f32 {
        self.pitch
    }

    /// Get current distance
    pub fn distance(&self) -> f32 {
        self.distance
    }

    /// Set camera state (for loading saved settings)
    pub fn set_state(&mut self, yaw: f32, pitch: f32, distance: f32) {
        self.yaw = yaw;
        self.pitch = pitch.clamp(-1.4, 1.4);
        self.distance = distance.clamp(1.0, 50.0);
    }

    /// Set target point
    pub fn set_target(&mut self, target: Vec3) {
        self.target = target;
    }

    /// Reset camera to default position
    pub fn reset(&mut self) {
        self.yaw = 0.0;
        self.pitch = 0.3;
        self.distance = 10.0;
    }
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self::new()
    }
}
