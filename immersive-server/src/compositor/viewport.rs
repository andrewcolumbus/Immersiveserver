//! Viewport for navigating the environment preview
//!
//! Provides pan, zoom, and constraint logic for viewing the environment
//! within the window.

/// Minimum zoom level (10%)
pub const MIN_ZOOM: f32 = 0.1;
/// Maximum zoom level (800%)
pub const MAX_ZOOM: f32 = 8.0;
/// Zoom factor per scroll step
const ZOOM_STEP: f32 = 1.1;
/// Spring stiffness for rubber-band effect
const SPRING_STIFFNESS: f32 = 12.0;
/// Damping factor for rubber-band animation
const SPRING_DAMPING: f32 = 0.85;
/// Threshold for stopping rubber-band animation
const VELOCITY_THRESHOLD: f32 = 0.5;
/// Maximum overshoot allowed (as fraction of visible area)
const MAX_OVERSHOOT: f32 = 0.5;
/// Resistance factor when dragging past bounds
const OVERSHOOT_RESISTANCE: f32 = 0.5;

/// Viewport state for navigating the environment preview
#[derive(Debug, Clone)]
pub struct Viewport {
    /// Offset in normalized coordinates (0,0 = centered on environment)
    /// Positive X = environment moves right (viewing left side)
    /// Positive Y = environment moves down (viewing top)
    offset: (f32, f32),
    
    /// Zoom level (1.0 = fit-to-window, values > 1 = zoomed in)
    zoom: f32,
    
    /// Rubber-band velocity for snap-back animation
    velocity: (f32, f32),
    
    /// Whether currently dragging (disables snap-back)
    is_dragging: bool,
    
    /// Last mouse position during drag (in window pixels)
    last_drag_pos: Option<(f32, f32)>,
    
    /// Time of last right-click (for double-click detection)
    last_right_click: Option<std::time::Instant>,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            offset: (0.0, 0.0),
            zoom: 1.0,
            velocity: (0.0, 0.0),
            is_dragging: false,
            last_drag_pos: None,
            last_right_click: None,
        }
    }
}

impl Viewport {
    /// Create a new viewport at default (fit-to-window) state
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Get current offset in normalized coordinates
    pub fn offset(&self) -> (f32, f32) {
        self.offset
    }
    
    /// Get current zoom level
    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    /// Set zoom level directly (for API control)
    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom.clamp(0.1, 8.0);
        self.velocity = (0.0, 0.0); // Stop any ongoing animation
    }

    /// Set offset directly (for API control)
    pub fn set_offset(&mut self, x: f32, y: f32) {
        self.offset = (x, y);
        self.velocity = (0.0, 0.0); // Stop any ongoing animation
    }

    /// Reset viewport to fit-to-window state
    pub fn reset(&mut self) {
        self.offset = (0.0, 0.0);
        self.zoom = 1.0;
        self.velocity = (0.0, 0.0);
    }

    /// Check if currently dragging
    pub fn is_dragging(&self) -> bool {
        self.is_dragging
    }

    /// Handle right mouse button press
    /// Returns true if this was a double-click (should reset)
    pub fn on_right_mouse_down(&mut self, pos: (f32, f32)) -> bool {
        let now = std::time::Instant::now();
        let is_double_click = self
            .last_right_click
            .map(|t| now.duration_since(t).as_millis() < 300)
            .unwrap_or(false);

        tracing::info!("[VIEWPORT DEBUG] viewport.rs: on_right_mouse_down pos={:?}, is_double_click={}", pos, is_double_click);

        if is_double_click {
            tracing::info!("[VIEWPORT DEBUG] viewport.rs: Double-click detected, resetting");
            self.reset();
            self.last_right_click = None;
            self.is_dragging = false;
            self.last_drag_pos = None;
            return true;
        }

        self.last_right_click = Some(now);
        self.is_dragging = true;
        self.last_drag_pos = Some(pos);
        self.velocity = (0.0, 0.0);
        tracing::info!("[VIEWPORT DEBUG] viewport.rs: Started dragging, is_dragging={}", self.is_dragging);
        false
    }

    /// Handle right mouse button release
    pub fn on_right_mouse_up(&mut self) {
        tracing::info!("[VIEWPORT DEBUG] viewport.rs: on_right_mouse_up, was_dragging={}", self.is_dragging);
        self.is_dragging = false;
        self.last_drag_pos = None;
    }

    /// Handle mouse movement during drag
    /// `pos` is current mouse position in window pixels
    /// `window_size` is (width, height) of window
    /// `env_size` is (width, height) of environment
    pub fn on_mouse_move(
        &mut self,
        pos: (f32, f32),
        window_size: (f32, f32),
        env_size: (f32, f32),
    ) {
        if !self.is_dragging {
            return;
        }

        let Some(last_pos) = self.last_drag_pos else {
            tracing::info!("[VIEWPORT DEBUG] viewport.rs: on_mouse_move - no last_pos, setting to {:?}", pos);
            self.last_drag_pos = Some(pos);
            return;
        };

        // Calculate delta in window pixels
        let delta_x = pos.0 - last_pos.0;
        let delta_y = pos.1 - last_pos.1;

        // Convert to normalized offset based on visible environment area
        // At zoom=1, the environment fits the window. At zoom=2, we see half.
        let base_scale = Self::compute_base_scale(window_size, env_size);
        let effective_scale = base_scale * self.zoom;

        // Delta in normalized environment coordinates
        let norm_delta_x = delta_x / (window_size.0 * effective_scale);
        let norm_delta_y = delta_y / (window_size.1 * effective_scale);

        // Calculate bounds and apply resistance if past them
        let (min_offset, max_offset) = self.compute_offset_bounds(window_size, env_size);

        let new_offset_x = self.offset.0 + norm_delta_x;
        let new_offset_y = self.offset.1 + norm_delta_y;

        // Apply resistance when past bounds
        self.offset.0 = Self::apply_drag_resistance(new_offset_x, min_offset.0, max_offset.0);
        self.offset.1 = Self::apply_drag_resistance(new_offset_y, min_offset.1, max_offset.1);

        tracing::info!("[VIEWPORT DEBUG] viewport.rs: on_mouse_move delta=({:.1}, {:.1}), new_offset=({:.4}, {:.4})", delta_x, delta_y, self.offset.0, self.offset.1);

        self.last_drag_pos = Some(pos);
    }
    
    /// Handle scroll wheel for zooming
    /// `delta` is scroll amount (positive = zoom in)
    /// `cursor_pos` is cursor position in window pixels
    /// `window_size` is (width, height) of window
    /// `env_size` is (width, height) of environment
    pub fn on_scroll(
        &mut self,
        delta: f32,
        cursor_pos: (f32, f32),
        window_size: (f32, f32),
        _env_size: (f32, f32),
    ) {
        let old_zoom = self.zoom;
        
        // Calculate new zoom
        let zoom_factor = if delta > 0.0 {
            ZOOM_STEP.powf(delta.abs().min(3.0))
        } else {
            1.0 / ZOOM_STEP.powf(delta.abs().min(3.0))
        };
        
        self.zoom = (self.zoom * zoom_factor).clamp(MIN_ZOOM, MAX_ZOOM);
        
        if (self.zoom - old_zoom).abs() < 0.0001 {
            return;
        }
        
        // Cursor-centered zoom: adjust offset so cursor stays over same env point
        // The shader does: adjusted_uv = (in.uv - 0.5) / scale + 0.5 + offset
        // Positive offset shifts sampling RIGHT, which visually shifts content LEFT
        // So we need to NEGATE to get intuitive behavior
        
        // Cursor position in normalized window coords (-0.5 to 0.5 from center)
        let cursor_norm_x = (cursor_pos.0 / window_size.0) - 0.5;
        let cursor_norm_y = (cursor_pos.1 / window_size.1) - 0.5;
        
        // Calculate offset delta to keep cursor over same point
        // When zooming, the point under cursor moves by: cursor_norm * (1/new_scale - 1/old_scale)
        // We need to compensate by adjusting offset in the OPPOSITE direction
        let offset_delta_x = cursor_norm_x * (1.0 / self.zoom - 1.0 / old_zoom);
        let offset_delta_y = cursor_norm_y * (1.0 / self.zoom - 1.0 / old_zoom);
        
        self.offset.0 -= offset_delta_x;
        self.offset.1 -= offset_delta_y;
    }
    
    /// Handle keyboard zoom (+/- keys)
    /// `zoom_in` is true for zoom in, false for zoom out
    /// `window_size` is (width, height) of window
    /// `env_size` is (width, height) of environment
    pub fn on_keyboard_zoom(
        &mut self,
        zoom_in: bool,
        window_size: (f32, f32),
        env_size: (f32, f32),
    ) {
        // Keyboard zoom is always centered
        let center = (window_size.0 / 2.0, window_size.1 / 2.0);
        let delta = if zoom_in { 1.0 } else { -1.0 };
        self.on_scroll(delta, center, window_size, env_size);
    }
    
    /// Update viewport state each frame (for rubber-band animation)
    /// `dt` is delta time in seconds
    /// `window_size` is (width, height) of window
    /// `env_size` is (width, height) of environment
    pub fn update(&mut self, dt: f32, window_size: (f32, f32), env_size: (f32, f32)) {
        if self.is_dragging {
            return;
        }
        
        let (min_offset, max_offset) = self.compute_offset_bounds(window_size, env_size);
        
        // Calculate how far we're past bounds
        let overshoot_x = Self::compute_overshoot(self.offset.0, min_offset.0, max_offset.0);
        let overshoot_y = Self::compute_overshoot(self.offset.1, min_offset.1, max_offset.1);
        
        // If we're within bounds and velocity is low, we're done
        if overshoot_x.abs() < 0.0001
            && overshoot_y.abs() < 0.0001
            && self.velocity.0.abs() < VELOCITY_THRESHOLD * 0.0001
            && self.velocity.1.abs() < VELOCITY_THRESHOLD * 0.0001
        {
            self.velocity = (0.0, 0.0);
            return;
        }
        
        // Spring physics: acceleration toward bounds
        let accel_x = -overshoot_x * SPRING_STIFFNESS;
        let accel_y = -overshoot_y * SPRING_STIFFNESS;
        
        // Apply acceleration and damping
        self.velocity.0 = (self.velocity.0 + accel_x * dt) * SPRING_DAMPING;
        self.velocity.1 = (self.velocity.1 + accel_y * dt) * SPRING_DAMPING;
        
        // Apply velocity
        self.offset.0 += self.velocity.0 * dt;
        self.offset.1 += self.velocity.1 * dt;
    }
    
    /// Compute the base scale factor for fit-to-window
    fn compute_base_scale(window_size: (f32, f32), env_size: (f32, f32)) -> f32 {
        let window_aspect = window_size.0 / window_size.1;
        let env_aspect = env_size.0 / env_size.1;
        
        if env_aspect > window_aspect {
            // Environment is wider - fit to width
            window_size.0 / env_size.0
        } else {
            // Environment is taller - fit to height
            window_size.1 / env_size.1
        }
    }
    
    /// Compute allowed offset bounds based on current zoom
    /// Rubber-band activates when environment edges go past window edges
    fn compute_offset_bounds(
        &self,
        window_size: (f32, f32),
        env_size: (f32, f32),
    ) -> ((f32, f32), (f32, f32)) {
        // Calculate how much the environment covers the window at current zoom
        // When zoom=1.0, environment exactly fits (or is letterboxed/pillarboxed)
        // When zoom>1.0, environment is larger than window (can pan)
        // When zoom<1.0, environment is smaller than window
        
        let window_aspect = window_size.0 / window_size.1;
        let env_aspect = env_size.0 / env_size.1;
        
        // Calculate what fraction of the window the environment covers at current zoom
        let (coverage_x, coverage_y) = if env_aspect > window_aspect {
            // Environment is wider - at zoom=1.0, width fills window
            (self.zoom, self.zoom * (window_aspect / env_aspect))
        } else {
            // Environment is taller - at zoom=1.0, height fills window
            (self.zoom * (env_aspect / window_aspect), self.zoom)
        };
        
        // Bounds: allow panning until an edge would go past the OPPOSITE window edge
        // When zoomed in (coverage > 1.0): can pan until content edge hits window edge
        // When zoomed out (coverage < 1.0): can pan until env edge hits opposite window edge
        // When exactly fit (coverage = 1.0): no panning needed, bounds are 0
        
        // The max offset is how far from center the environment can move before
        // its far edge exits the window on the opposite side
        // At coverage=0.5: env takes half window, can move 0.25 each way (edge to edge = 0.5)
        // At coverage=2.0: env is 2x window, can move 0.5 each way (show different halves)
        
        let max_offset_x = if coverage_x > 1.0 {
            // Zoomed in: pan until content edge hits window edge
            (coverage_x - 1.0) / 2.0 / self.zoom
        } else if coverage_x < 1.0 {
            // Zoomed out: can move env within window, rubber-band at edges
            // Allow moving until env edge reaches opposite window edge
            (1.0 - coverage_x) / 2.0 / self.zoom
        } else {
            0.0
        };
        
        let max_offset_y = if coverage_y > 1.0 {
            (coverage_y - 1.0) / 2.0 / self.zoom
        } else if coverage_y < 1.0 {
            (1.0 - coverage_y) / 2.0 / self.zoom
        } else {
            0.0
        };
        
        ((-max_offset_x, -max_offset_y), (max_offset_x, max_offset_y))
    }
    
    /// Compute how far past bounds we are
    fn compute_overshoot(value: f32, min: f32, max: f32) -> f32 {
        if value < min {
            value - min
        } else if value > max {
            value - max
        } else {
            0.0
        }
    }
    
    /// Apply resistance when dragging past bounds
    fn apply_drag_resistance(value: f32, min: f32, max: f32) -> f32 {
        if value < min {
            let overshoot = min - value;
            let resisted = overshoot * OVERSHOOT_RESISTANCE;
            let clamped = resisted.min(MAX_OVERSHOOT);
            min - clamped
        } else if value > max {
            let overshoot = value - max;
            let resisted = overshoot * OVERSHOOT_RESISTANCE;
            let clamped = resisted.min(MAX_OVERSHOOT);
            max + clamped
        } else {
            value
        }
    }
    
    /// Get scale and offset for the copy shader
    /// Returns (scale_x, scale_y, offset_x, offset_y)
    pub fn get_shader_params(
        &self,
        window_size: (f32, f32),
        env_size: (f32, f32),
    ) -> (f32, f32, f32, f32) {
        let window_aspect = window_size.0 / window_size.1;
        let env_aspect = env_size.0 / env_size.1;
        
        // Base fit-to-window scale
        let (base_scale_x, base_scale_y) = if env_aspect > window_aspect {
            // Environment is wider - pillarbox
            (1.0, window_aspect / env_aspect)
        } else {
            // Environment is taller - letterbox
            (env_aspect / window_aspect, 1.0)
        };
        
        // Apply zoom
        let scale_x = base_scale_x * self.zoom;
        let scale_y = base_scale_y * self.zoom;
        
        // Apply offset (scaled to match zoom)
        let offset_x = self.offset.0 * self.zoom;
        let offset_y = self.offset.1 * self.zoom;
        
        (scale_x, scale_y, offset_x, offset_y)
    }
    
    /// Check if viewport needs animation update
    pub fn needs_update(&self) -> bool {
        !self.is_dragging
            && (self.velocity.0.abs() > 0.0001 || self.velocity.1.abs() > 0.0001)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_viewport() {
        let vp = Viewport::new();
        assert_eq!(vp.zoom(), 1.0);
        assert_eq!(vp.offset(), (0.0, 0.0));
    }

    #[test]
    fn test_reset() {
        let mut vp = Viewport::new();
        vp.offset = (0.5, 0.5);
        vp.zoom = 4.0;
        vp.reset();
        assert_eq!(vp.zoom(), 1.0);
        assert_eq!(vp.offset(), (0.0, 0.0));
    }

    #[test]
    fn test_zoom_clamping() {
        let mut vp = Viewport::new();
        let window = (1920.0, 1080.0);
        let env = (1920.0, 1080.0);
        
        // Zoom way in
        for _ in 0..50 {
            vp.on_scroll(1.0, (960.0, 540.0), window, env);
        }
        assert!(vp.zoom() <= MAX_ZOOM);
        
        // Zoom way out
        for _ in 0..50 {
            vp.on_scroll(-1.0, (960.0, 540.0), window, env);
        }
        assert!(vp.zoom() >= MIN_ZOOM);
    }
}

