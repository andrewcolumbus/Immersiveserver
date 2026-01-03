//! Hand Interaction effect
//!
//! Particles respond to hand landmark positions with various interaction modes.

use bytemuck::{Pod, Zeroable};
use crate::ml::Hand;

/// Interaction mode
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionMode {
    Attract,
    Repel,
    Swirl,
    Push,
}

/// Hand point for GPU
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct HandPoint {
    pub pos: [f32; 2],
    pub active: f32,
    pub influence: f32,
}

/// Hand data for GPU (44 points: 21 per hand * 2 + 2 palm centers)
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct HandData {
    pub points: [HandPoint; 44],
    pub hand_count: u32,
    pub _pad: [u32; 3],
}

impl Default for HandData {
    fn default() -> Self {
        Self {
            points: [HandPoint { pos: [0.0; 2], active: 0.0, influence: 0.0 }; 44],
            hand_count: 0,
            _pad: [0; 3],
        }
    }
}

/// Effect parameters
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct HandInteractionParams {
    pub time: f32,
    pub delta_time: f32,
    pub mode: u32,
    pub force_strength: f32,
    pub force_radius: f32,
    pub max_velocity: f32,
    pub palm_force_multiplier: f32,
    pub fingertip_force_multiplier: f32,
}

impl Default for HandInteractionParams {
    fn default() -> Self {
        Self {
            time: 0.0,
            delta_time: 1.0 / 60.0,
            mode: 0, // Attract
            force_strength: 0.5,
            force_radius: 0.2,
            max_velocity: 1.0,
            palm_force_multiplier: 2.0,
            fingertip_force_multiplier: 1.5,
        }
    }
}

/// Hand Interaction effect runtime
pub struct HandInteractionEffect {
    /// Effect parameters
    params: HandInteractionParams,
    /// Current hand data
    hand_data: HandData,
    /// Interaction mode
    mode: InteractionMode,
}

impl HandInteractionEffect {
    /// Create a new effect instance
    pub fn new() -> Self {
        Self {
            params: HandInteractionParams::default(),
            hand_data: HandData::default(),
            mode: InteractionMode::Attract,
        }
    }

    /// Update hand data from ML results
    pub fn update_hands(&mut self, hands: &[Hand]) {
        self.hand_data = HandData::default();
        self.hand_data.hand_count = hands.len().min(2) as u32;

        for (hand_idx, hand) in hands.iter().enumerate().take(2) {
            let base = hand_idx * 22; // 21 landmarks + 1 palm center

            // Copy landmarks
            for (i, landmark) in hand.landmarks.iter().enumerate() {
                let influence = match i {
                    4 | 8 | 12 | 16 | 20 => 1.5, // Fingertips
                    0 => 0.5,                     // Wrist
                    _ => 1.0,
                };

                self.hand_data.points[base + i] = HandPoint {
                    pos: [landmark.x, landmark.y],
                    active: 1.0,
                    influence,
                };
            }

            // Calculate palm center (average of landmarks 0, 5, 9, 13, 17)
            let palm_indices = [0, 5, 9, 13, 17];
            let palm_x: f32 = palm_indices.iter().map(|&i| hand.landmarks[i].x).sum::<f32>() / 5.0;
            let palm_y: f32 = palm_indices.iter().map(|&i| hand.landmarks[i].y).sum::<f32>() / 5.0;

            self.hand_data.points[base + 21] = HandPoint {
                pos: [palm_x, palm_y],
                active: 1.0,
                influence: self.params.palm_force_multiplier,
            };
        }
    }

    /// Set interaction mode
    pub fn set_mode(&mut self, mode: InteractionMode) {
        self.mode = mode;
        self.params.mode = match mode {
            InteractionMode::Attract => 0,
            InteractionMode::Repel => 1,
            InteractionMode::Swirl => 2,
            InteractionMode::Push => 3,
        };
    }

    /// Get effect parameters
    pub fn params(&self) -> &HandInteractionParams {
        &self.params
    }

    /// Get hand data
    pub fn hand_data(&self) -> &HandData {
        &self.hand_data
    }
}

impl Default for HandInteractionEffect {
    fn default() -> Self {
        Self::new()
    }
}
