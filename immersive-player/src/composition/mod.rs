//! Composition module for layer-based video mixing
//!
//! Provides a Resolume-style composition system with layers, clips, and decks.

#![allow(dead_code)]

mod clip;
mod layer;
mod playback;
mod settings;

pub use clip::{Clip, ClipSlot, GeneratorClip, GeneratorType, ImageClip, SolidColorClip, TriggerMode, VideoClip};
pub use layer::{BlendMode, Layer, LayerTransform};
pub use playback::{ClipPlayback, PlaybackState};
pub use settings::CompositionSettings;

use serde::{Deserialize, Serialize};

/// The central composition canvas that layers render onto
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Composition {
    /// Composition settings (resolution, FPS, bit depth)
    pub settings: CompositionSettings,
    /// Layers in the composition (bottom to top order)
    pub layers: Vec<Layer>,
    /// Number of deck columns
    pub columns: usize,
    /// Master opacity (0.0 to 1.0)
    pub master_opacity: f32,
    /// Master playback speed multiplier
    pub master_speed: f32,
}

impl Default for Composition {
    fn default() -> Self {
        Self::new(CompositionSettings::default(), 6, 4)
    }
}

impl Composition {
    /// Create a new composition with specified settings, columns, and layer count
    pub fn new(settings: CompositionSettings, columns: usize, layer_count: usize) -> Self {
        let layers = (0..layer_count)
            .map(|i| Layer::new(i as u32, format!("Layer {}", layer_count - i), columns))
            .collect();

        Self {
            settings,
            layers,
            columns,
            master_opacity: 1.0,
            master_speed: 1.0,
        }
    }

    /// Get the composition width
    pub fn width(&self) -> u32 {
        self.settings.width
    }

    /// Get the composition height
    pub fn height(&self) -> u32 {
        self.settings.height
    }

    /// Get the composition FPS
    pub fn fps(&self) -> f32 {
        self.settings.fps
    }

    /// Get number of layers
    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    /// Add a new layer at the top
    pub fn add_layer(&mut self) -> u32 {
        let id = self.layers.iter().map(|l| l.id).max().unwrap_or(0) + 1;
        let layer = Layer::new(id, format!("Layer {}", id + 1), self.columns);
        self.layers.push(layer);
        id
    }

    /// Remove a layer by ID
    pub fn remove_layer(&mut self, id: u32) -> Option<Layer> {
        if let Some(pos) = self.layers.iter().position(|l| l.id == id) {
            Some(self.layers.remove(pos))
        } else {
            None
        }
    }

    /// Get a layer by ID
    pub fn get_layer(&self, id: u32) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }

    /// Get a layer by ID mutably
    pub fn get_layer_mut(&mut self, id: u32) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.id == id)
    }

    /// Get a layer by index
    pub fn get_layer_by_index(&self, index: usize) -> Option<&Layer> {
        self.layers.get(index)
    }

    /// Get a layer by index mutably
    pub fn get_layer_by_index_mut(&mut self, index: usize) -> Option<&mut Layer> {
        self.layers.get_mut(index)
    }

    /// Trigger a clip at the specified layer and column
    pub fn trigger_clip(&mut self, layer_index: usize, column: usize) {
        if let Some(layer) = self.layers.get_mut(layer_index) {
            layer.trigger_column(column);
        }
    }

    /// Stop the clip on the specified layer
    pub fn stop_layer(&mut self, layer_index: usize) {
        if let Some(layer) = self.layers.get_mut(layer_index) {
            layer.stop();
        }
    }

    /// Update all layers (call each frame)
    pub fn update(&mut self, delta_time: f64) {
        let speed = self.master_speed as f64;
        for layer in &mut self.layers {
            layer.update(delta_time * speed);
        }
    }

    /// Check if any layer is soloed
    pub fn has_solo(&self) -> bool {
        self.layers.iter().any(|l| l.solo)
    }

    /// Get layers that should be rendered (respecting solo/bypass)
    pub fn visible_layers(&self) -> impl Iterator<Item = &Layer> {
        let has_solo = self.has_solo();
        self.layers.iter().filter(move |l| {
            if l.bypass {
                return false;
            }
            if has_solo {
                l.solo
            } else {
                true
            }
        })
    }

    /// Resize the composition
    pub fn resize(&mut self, width: u32, height: u32) {
        self.settings.width = width;
        self.settings.height = height;
    }

    /// Set the FPS
    pub fn set_fps(&mut self, fps: f32) {
        self.settings.fps = fps;
    }

    /// Add a column to all layers
    pub fn add_column(&mut self) {
        self.columns += 1;
        for layer in &mut self.layers {
            layer.add_column();
        }
    }

    /// Remove a column from all layers
    pub fn remove_column(&mut self) {
        if self.columns > 1 {
            self.columns -= 1;
            for layer in &mut self.layers {
                layer.remove_column();
            }
        }
    }

    /// Save composition to an .immersive XML file
    pub fn save_to_file(&self, path: &std::path::Path) -> anyhow::Result<()> {
        crate::project::save_immersive(self, path)
    }

    /// Load composition from an .immersive XML file
    pub fn load_from_file(path: &std::path::Path) -> anyhow::Result<Self> {
        crate::project::load_immersive(path)
    }

    /// Convert to XML string
    pub fn to_xml(&self) -> anyhow::Result<String> {
        crate::project::immersive_format::composition_to_xml(self)
    }

    /// Load from XML string
    pub fn from_xml(xml: &str) -> anyhow::Result<Self> {
        crate::project::immersive_format::xml_to_composition(xml)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_composition_creation() {
        let comp = Composition::default();
        assert_eq!(comp.columns, 6);
        assert_eq!(comp.layers.len(), 4);
        assert_eq!(comp.settings.width, 1920);
        assert_eq!(comp.settings.height, 1080);
    }

    #[test]
    fn test_trigger_clip() {
        let mut comp = Composition::default();
        comp.trigger_clip(0, 0);
        assert_eq!(comp.layers[0].active_column, Some(0));
    }
}

