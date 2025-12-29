//! Layer system for composition
//!
//! Layers contain clips arranged in columns (decks). Each layer has one active clip at a time.

#![allow(dead_code)]

use super::{ClipSlot, PlaybackState};
use serde::{Deserialize, Serialize};

/// Blend modes for layer compositing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BlendMode {
    /// Normal alpha blending
    #[default]
    Normal,
    /// Additive blending (brightens)
    Add,
    /// Multiply blending (darkens)
    Multiply,
    /// Screen blending (lightens)
    Screen,
    /// Overlay blending (contrast)
    Overlay,
}

impl BlendMode {
    /// Get all blend modes
    pub fn all() -> &'static [BlendMode] {
        &[
            BlendMode::Normal,
            BlendMode::Add,
            BlendMode::Multiply,
            BlendMode::Screen,
            BlendMode::Overlay,
        ]
    }

    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            BlendMode::Normal => "Normal",
            BlendMode::Add => "Add",
            BlendMode::Multiply => "Multiply",
            BlendMode::Screen => "Screen",
            BlendMode::Overlay => "Overlay",
        }
    }

    /// Get the blend mode index for shader uniform
    pub fn shader_index(&self) -> u32 {
        match self {
            BlendMode::Normal => 0,
            BlendMode::Add => 1,
            BlendMode::Multiply => 2,
            BlendMode::Screen => 3,
            BlendMode::Overlay => 4,
        }
    }
}

/// Transform properties for a layer
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LayerTransform {
    /// Position offset (normalized, 0.0 = center)
    pub position: (f32, f32),
    /// Scale (1.0 = original size)
    pub scale: (f32, f32),
    /// Rotation in degrees
    pub rotation: f32,
    /// Anchor point (normalized, 0.5 = center)
    pub anchor: (f32, f32),
}

impl Default for LayerTransform {
    fn default() -> Self {
        Self {
            position: (0.0, 0.0),
            scale: (1.0, 1.0),
            rotation: 0.0,
            anchor: (0.5, 0.5),
        }
    }
}

impl LayerTransform {
    /// Create a transform with centered content
    pub fn centered() -> Self {
        Self::default()
    }

    /// Create a transform matrix (3x3 for 2D)
    pub fn to_matrix(&self) -> [[f32; 3]; 3] {
        let cos_r = self.rotation.to_radians().cos();
        let sin_r = self.rotation.to_radians().sin();

        // Scale
        let sx = self.scale.0;
        let sy = self.scale.1;

        // Translation
        let tx = self.position.0;
        let ty = self.position.1;

        // Anchor offset
        let ax = self.anchor.0 - 0.5;
        let ay = self.anchor.1 - 0.5;

        // Combined transform: translate to anchor, scale, rotate, translate back, then position
        [
            [cos_r * sx, -sin_r * sy, 0.0],
            [sin_r * sx, cos_r * sy, 0.0],
            [
                tx - ax * cos_r * sx + ay * sin_r * sy + ax,
                ty - ax * sin_r * sx - ay * cos_r * sy + ay,
                1.0,
            ],
        ]
    }

    /// Reset to default
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Check if transform is identity (no transformation)
    pub fn is_identity(&self) -> bool {
        self.position == (0.0, 0.0)
            && self.scale == (1.0, 1.0)
            && self.rotation == 0.0
            && self.anchor == (0.5, 0.5)
    }
}

/// A layer in the composition containing clips in columns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layer {
    /// Unique identifier
    pub id: u32,
    /// Display name
    pub name: String,
    /// Clips in each column (None = empty slot)
    pub clips: Vec<Option<ClipSlot>>,
    /// Currently active column (playing clip)
    pub active_column: Option<usize>,
    /// Layer opacity (0.0 to 1.0)
    pub opacity: f32,
    /// Blend mode for compositing
    pub blend_mode: BlendMode,
    /// Whether layer is bypassed (hidden)
    pub bypass: bool,
    /// Whether layer is soloed
    pub solo: bool,
    /// Layer transform (position, scale, rotation)
    pub transform: LayerTransform,
    /// Layer volume (for audio, 0.0 to 1.0)
    pub volume: f32,
}

impl Layer {
    /// Create a new layer with the specified number of columns
    pub fn new(id: u32, name: String, columns: usize) -> Self {
        Self {
            id,
            name,
            clips: vec![None; columns],
            active_column: None,
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
            bypass: false,
            solo: false,
            transform: LayerTransform::default(),
            volume: 1.0,
        }
    }

    /// Get the number of columns
    pub fn column_count(&self) -> usize {
        self.clips.len()
    }

    /// Add a column
    pub fn add_column(&mut self) {
        self.clips.push(None);
    }

    /// Remove the last column
    pub fn remove_column(&mut self) {
        if self.clips.len() > 1 {
            self.clips.pop();
            // If active column was removed, stop playback
            if let Some(active) = self.active_column {
                if active >= self.clips.len() {
                    self.active_column = None;
                }
            }
        }
    }

    /// Set a clip in a column
    pub fn set_clip(&mut self, column: usize, clip: ClipSlot) {
        if column < self.clips.len() {
            self.clips[column] = Some(clip);
        }
    }

    /// Remove a clip from a column
    pub fn remove_clip(&mut self, column: usize) -> Option<ClipSlot> {
        if column < self.clips.len() {
            // If this was the active column, stop playback
            if self.active_column == Some(column) {
                self.active_column = None;
            }
            self.clips[column].take()
        } else {
            None
        }
    }

    /// Get a clip in a column
    pub fn get_clip(&self, column: usize) -> Option<&ClipSlot> {
        self.clips.get(column).and_then(|c| c.as_ref())
    }

    /// Get a clip in a column mutably
    pub fn get_clip_mut(&mut self, column: usize) -> Option<&mut ClipSlot> {
        self.clips.get_mut(column).and_then(|c| c.as_mut())
    }

    /// Trigger a column (start playing that clip)
    pub fn trigger_column(&mut self, column: usize) {
        if column >= self.clips.len() {
            return;
        }

        // Check if we have a clip in this column
        if self.clips[column].is_none() {
            return;
        }

        // If clicking the same column, toggle playback
        if self.active_column == Some(column) {
            if let Some(clip) = &mut self.clips[column] {
                clip.playback.toggle();
                // Toggle video playback too
                if clip.playback.is_playing() {
                    clip.start_video();
                } else {
                    clip.stop_video();
                }
            }
        } else {
            // Stop current clip if any
            if let Some(prev_col) = self.active_column {
                if let Some(prev_clip) = &mut self.clips[prev_col] {
                    prev_clip.playback.stop();
                    prev_clip.stop_video();
                }
            }
            // Start new clip
            if let Some(clip) = &mut self.clips[column] {
                clip.playback.play();
                clip.start_video();
            }
            self.active_column = Some(column);
        }
    }

    /// Stop playback on this layer
    pub fn stop(&mut self) {
        if let Some(col) = self.active_column {
            if let Some(clip) = &mut self.clips[col] {
                clip.playback.stop();
                clip.stop_video();
            }
        }
        self.active_column = None;
    }

    /// Update playback state
    pub fn update(&mut self, delta_time: f64) {
        if let Some(col) = self.active_column {
            if let Some(clip) = &mut self.clips[col] {
                clip.update(delta_time);

                // Check if clip finished (for one-shot mode)
                if clip.playback.state == PlaybackState::Stopped {
                    self.active_column = None;
                }
            }
        }
    }

    /// Get the currently active clip
    pub fn active_clip(&self) -> Option<&ClipSlot> {
        self.active_column
            .and_then(|col| self.clips.get(col))
            .and_then(|c| c.as_ref())
    }

    /// Get the currently active clip mutably
    pub fn active_clip_mut(&mut self) -> Option<&mut ClipSlot> {
        if let Some(col) = self.active_column {
            self.clips.get_mut(col).and_then(|c| c.as_mut())
        } else {
            None
        }
    }

    /// Check if this layer has any content
    pub fn has_content(&self) -> bool {
        self.clips.iter().any(|c| c.is_some())
    }

    /// Check if this layer is currently playing
    pub fn is_playing(&self) -> bool {
        self.active_column.is_some()
            && self
                .active_clip()
                .map(|c| c.playback.state == PlaybackState::Playing)
                .unwrap_or(false)
    }

    /// Get effective opacity (considering bypass)
    pub fn effective_opacity(&self) -> f32 {
        if self.bypass {
            0.0
        } else {
            self.opacity
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layer_creation() {
        let layer = Layer::new(0, "Test Layer".to_string(), 6);
        assert_eq!(layer.clips.len(), 6);
        assert_eq!(layer.opacity, 1.0);
        assert_eq!(layer.blend_mode, BlendMode::Normal);
    }

    #[test]
    fn test_layer_transform() {
        let transform = LayerTransform::default();
        assert!(transform.is_identity());
    }
}

