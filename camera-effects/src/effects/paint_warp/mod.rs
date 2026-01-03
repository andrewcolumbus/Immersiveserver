//! Paint Warp effect
//!
//! Image warps and smears like wet paint when touched,
//! with persistent trails that decay over time.

use bytemuck::{Pod, Zeroable};

/// Touch point for input
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct TouchPoint {
    pub pos: [f32; 2],
    pub prev_pos: [f32; 2],
    pub pressure: f32,
    pub active: f32,
    pub _pad: [f32; 2],
}

impl Default for TouchPoint {
    fn default() -> Self {
        Self {
            pos: [0.0; 2],
            prev_pos: [0.0; 2],
            pressure: 1.0,
            active: 0.0,
            _pad: [0.0; 2],
        }
    }
}

/// Touch data for GPU
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct TouchData {
    pub points: [TouchPoint; 10],
    pub point_count: u32,
    pub _pad: [u32; 3],
}

impl Default for TouchData {
    fn default() -> Self {
        Self {
            points: [TouchPoint::default(); 10],
            point_count: 0,
            _pad: [0; 3],
        }
    }
}

/// Effect parameters
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct PaintWarpParams {
    pub time: f32,
    pub delta_time: f32,
    pub viscosity: f32,
    pub displacement_strength: f32,
    pub brush_radius: f32,
    pub brush_softness: f32,
    pub smear_length: f32,
    pub flow_speed: f32,
}

impl Default for PaintWarpParams {
    fn default() -> Self {
        Self {
            time: 0.0,
            delta_time: 1.0 / 60.0,
            viscosity: 0.98,
            displacement_strength: 0.5,
            brush_radius: 0.1,
            brush_softness: 0.5,
            smear_length: 2.0,
            flow_speed: 0.0,
        }
    }
}

/// Paint Warp effect runtime
pub struct PaintWarpEffect {
    /// Effect parameters
    params: PaintWarpParams,
    /// Touch data
    touch_data: TouchData,
    /// Previous touch positions for velocity calculation
    prev_touches: Vec<(f32, f32)>,
    /// Elapsed time
    time: f32,
}

impl PaintWarpEffect {
    /// Create a new effect instance
    pub fn new() -> Self {
        Self {
            params: PaintWarpParams::default(),
            touch_data: TouchData::default(),
            prev_touches: Vec::new(),
            time: 0.0,
        }
    }

    /// Update touch positions
    pub fn update_touches(&mut self, touches: &[(f32, f32)]) {
        // Store previous positions
        for (i, touch) in touches.iter().enumerate().take(10) {
            if i < self.prev_touches.len() {
                self.touch_data.points[i].prev_pos = [self.prev_touches[i].0, self.prev_touches[i].1];
            } else {
                self.touch_data.points[i].prev_pos = [touch.0, touch.1];
            }
            self.touch_data.points[i].pos = [touch.0, touch.1];
            self.touch_data.points[i].active = 1.0;
        }

        // Deactivate unused points
        for i in touches.len()..10 {
            self.touch_data.points[i].active = 0.0;
        }

        self.touch_data.point_count = touches.len().min(10) as u32;
        self.prev_touches = touches.to_vec();
    }

    /// Update from hand landmarks (use palm and fingertips as touch points)
    pub fn update_from_hands(&mut self, hands: &[crate::ml::Hand]) {
        let mut touches = Vec::new();

        for hand in hands.iter().take(2) {
            // Palm center
            let palm_indices = [0, 5, 9, 13, 17];
            let palm_x: f32 = palm_indices.iter().map(|&i| hand.landmarks[i].x).sum::<f32>() / 5.0;
            let palm_y: f32 = palm_indices.iter().map(|&i| hand.landmarks[i].y).sum::<f32>() / 5.0;
            touches.push((palm_x, palm_y));

            // Fingertips
            for &i in &[4, 8, 12, 16, 20] {
                touches.push((hand.landmarks[i].x, hand.landmarks[i].y));
            }
        }

        self.update_touches(&touches);
    }

    /// Update the effect
    pub fn update(&mut self, delta_time: f32) {
        self.time += delta_time;
        self.params.time = self.time;
        self.params.delta_time = delta_time;
    }

    /// Get effect parameters
    pub fn params(&self) -> &PaintWarpParams {
        &self.params
    }

    /// Get touch data
    pub fn touch_data(&self) -> &TouchData {
        &self.touch_data
    }

    /// Set a parameter
    pub fn set_param(&mut self, name: &str, value: f32) {
        match name {
            "viscosity" => self.params.viscosity = value.clamp(0.9, 0.999),
            "displacement_strength" => self.params.displacement_strength = value,
            "brush_radius" => self.params.brush_radius = value,
            "brush_softness" => self.params.brush_softness = value,
            "smear_length" => self.params.smear_length = value,
            _ => {}
        }
    }

    /// Clear the displacement field
    pub fn clear(&mut self) {
        // TODO: Reset displacement texture on GPU
    }
}

impl Default for PaintWarpEffect {
    fn default() -> Self {
        Self::new()
    }
}
