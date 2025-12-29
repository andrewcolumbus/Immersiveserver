//! Environment (composition canvas)
//!
//! The Environment is a fixed-resolution render target that represents the
//! composition canvas. It is independent of the window size; the window is
//! simply a viewport that displays the environment (typically scaled to fit).
//!
//! The Environment contains a list of Layers that are composited together
//! in back-to-front order.

use super::Layer;

/// Fixed-resolution composition canvas backed by a GPU texture.
///
/// The Environment holds all layers and manages the render target
/// for the final composited output.
pub struct Environment {
    /// Width of the composition canvas in pixels
    width: u32,
    /// Height of the composition canvas in pixels
    height: u32,
    /// Texture format for the environment render target
    format: wgpu::TextureFormat,
    /// GPU texture for the environment (render target)
    texture: wgpu::Texture,
    /// View of the environment texture
    texture_view: wgpu::TextureView,
    /// Layers in the environment (rendered back-to-front)
    layers: Vec<Layer>,
    /// Next available layer ID
    next_layer_id: u32,
}

impl Environment {
    /// Create a new Environment render target.
    pub fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Self {
        let width = width.max(1);
        let height = height.max(1);

        let (texture, texture_view) = Self::create_texture(device, width, height, format);

        Self {
            width,
            height,
            format,
            texture,
            texture_view,
            layers: Vec::new(),
            next_layer_id: 1,
        }
    }

    /// Resize the environment canvas.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);

        if width == self.width && height == self.height {
            return;
        }

        let (texture, texture_view) = Self::create_texture(device, width, height, self.format);

        self.width = width;
        self.height = height;
        self.texture = texture;
        self.texture_view = texture_view;
    }

    // ========== Dimension Accessors ==========

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn texture_view(&self) -> &wgpu::TextureView {
        &self.texture_view
    }

    // ========== Layer Management ==========

    /// Add a new layer with the given name.
    /// Returns the ID of the newly created layer.
    pub fn add_layer(&mut self, name: impl Into<String>) -> u32 {
        let id = self.next_layer_id;
        self.next_layer_id += 1;

        let layer = Layer::new(id, name);
        self.layers.push(layer);
        id
    }

    /// Add an existing layer to the environment.
    /// The layer's ID will be preserved. Updates next_layer_id if necessary.
    pub fn add_existing_layer(&mut self, layer: Layer) {
        if layer.id >= self.next_layer_id {
            self.next_layer_id = layer.id + 1;
        }
        self.layers.push(layer);
    }

    /// Remove a layer by ID.
    /// Returns the removed layer if found, or None if not found.
    pub fn remove_layer(&mut self, id: u32) -> Option<Layer> {
        if let Some(index) = self.layers.iter().position(|l| l.id == id) {
            Some(self.layers.remove(index))
        } else {
            None
        }
    }

    /// Get a reference to a layer by ID.
    pub fn get_layer(&self, id: u32) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }

    /// Get a mutable reference to a layer by ID.
    pub fn get_layer_mut(&mut self, id: u32) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.id == id)
    }

    /// Get a reference to a layer by index.
    pub fn get_layer_at(&self, index: usize) -> Option<&Layer> {
        self.layers.get(index)
    }

    /// Get a mutable reference to a layer by index.
    pub fn get_layer_at_mut(&mut self, index: usize) -> Option<&mut Layer> {
        self.layers.get_mut(index)
    }

    /// Get all layers (immutable).
    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    /// Get all layers (mutable).
    pub fn layers_mut(&mut self) -> &mut [Layer] {
        &mut self.layers
    }

    /// Get the number of layers.
    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    /// Check if there are any layers.
    pub fn has_layers(&self) -> bool {
        !self.layers.is_empty()
    }

    /// Move a layer to a different position in the render order.
    /// Lower index = rendered first (behind), higher index = rendered last (in front).
    pub fn move_layer(&mut self, from_index: usize, to_index: usize) {
        if from_index >= self.layers.len() || to_index >= self.layers.len() {
            return;
        }

        let layer = self.layers.remove(from_index);
        self.layers.insert(to_index, layer);
    }

    /// Move a layer by ID to the front (top of render order).
    pub fn move_layer_to_front(&mut self, id: u32) {
        if let Some(index) = self.layers.iter().position(|l| l.id == id) {
            let last = self.layers.len() - 1;
            if index != last {
                self.move_layer(index, last);
            }
        }
    }

    /// Move a layer by ID to the back (bottom of render order).
    pub fn move_layer_to_back(&mut self, id: u32) {
        if let Some(index) = self.layers.iter().position(|l| l.id == id) {
            if index != 0 {
                self.move_layer(index, 0);
            }
        }
    }

    /// Clear all layers.
    pub fn clear_layers(&mut self) {
        self.layers.clear();
    }

    // ========== Private Helpers ==========

    fn create_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Environment Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }
}

/// A minimal layer store for testing layer management without GPU dependencies.
/// This allows unit testing the layer management logic separately.
#[cfg(test)]
pub(crate) struct LayerStore {
    layers: Vec<Layer>,
    next_layer_id: u32,
}

#[cfg(test)]
impl LayerStore {
    fn new() -> Self {
        Self {
            layers: Vec::new(),
            next_layer_id: 1,
        }
    }

    fn add_layer(&mut self, name: impl Into<String>) -> u32 {
        let id = self.next_layer_id;
        self.next_layer_id += 1;
        self.layers.push(Layer::new(id, name));
        id
    }

    fn remove_layer(&mut self, id: u32) -> Option<Layer> {
        if let Some(index) = self.layers.iter().position(|l| l.id == id) {
            Some(self.layers.remove(index))
        } else {
            None
        }
    }

    fn get_layer(&self, id: u32) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }

    fn get_layer_mut(&mut self, id: u32) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.id == id)
    }

    fn get_layer_at(&self, index: usize) -> Option<&Layer> {
        self.layers.get(index)
    }

    fn layer_count(&self) -> usize {
        self.layers.len()
    }

    fn has_layers(&self) -> bool {
        !self.layers.is_empty()
    }

    fn move_layer(&mut self, from_index: usize, to_index: usize) {
        if from_index >= self.layers.len() || to_index >= self.layers.len() {
            return;
        }
        let layer = self.layers.remove(from_index);
        self.layers.insert(to_index, layer);
    }

    fn move_layer_to_front(&mut self, id: u32) {
        if let Some(index) = self.layers.iter().position(|l| l.id == id) {
            let last = self.layers.len() - 1;
            if index != last {
                self.move_layer(index, last);
            }
        }
    }

    fn move_layer_to_back(&mut self, id: u32) {
        if let Some(index) = self.layers.iter().position(|l| l.id == id) {
            if index != 0 {
                self.move_layer(index, 0);
            }
        }
    }

    fn clear_layers(&mut self) {
        self.layers.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compositor::BlendMode;

    #[test]
    fn test_layer_store_new() {
        let store = LayerStore::new();
        assert_eq!(store.layer_count(), 0);
        assert!(!store.has_layers());
    }

    #[test]
    fn test_add_layer() {
        let mut store = LayerStore::new();

        let id1 = store.add_layer("Layer 1");
        let id2 = store.add_layer("Layer 2");

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(store.layer_count(), 2);
        assert!(store.has_layers());

        let layer1 = store.get_layer(id1).unwrap();
        assert_eq!(layer1.name, "Layer 1");

        let layer2 = store.get_layer(id2).unwrap();
        assert_eq!(layer2.name, "Layer 2");
    }

    #[test]
    fn test_remove_layer() {
        let mut store = LayerStore::new();

        let id1 = store.add_layer("Layer 1");
        let id2 = store.add_layer("Layer 2");

        let removed = store.remove_layer(id1);
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().name, "Layer 1");
        assert_eq!(store.layer_count(), 1);

        // Try to remove non-existent layer
        let not_found = store.remove_layer(999);
        assert!(not_found.is_none());

        // Remaining layer should still be accessible
        assert!(store.get_layer(id2).is_some());
    }

    #[test]
    fn test_get_layer_mut() {
        let mut store = LayerStore::new();

        let id = store.add_layer("Test Layer");

        if let Some(layer) = store.get_layer_mut(id) {
            layer.set_opacity(0.5);
            layer.set_blend_mode(BlendMode::Additive);
        }

        let layer = store.get_layer(id).unwrap();
        assert_eq!(layer.opacity, 0.5);
        assert_eq!(layer.blend_mode, BlendMode::Additive);
    }

    #[test]
    fn test_move_layer() {
        let mut store = LayerStore::new();

        let id1 = store.add_layer("Layer 1");
        let id2 = store.add_layer("Layer 2");
        let id3 = store.add_layer("Layer 3");

        // Initial order: [1, 2, 3]
        assert_eq!(store.get_layer_at(0).unwrap().id, id1);
        assert_eq!(store.get_layer_at(1).unwrap().id, id2);
        assert_eq!(store.get_layer_at(2).unwrap().id, id3);

        // Move layer 1 to front
        store.move_layer_to_front(id1);
        // New order: [2, 3, 1]
        assert_eq!(store.get_layer_at(0).unwrap().id, id2);
        assert_eq!(store.get_layer_at(1).unwrap().id, id3);
        assert_eq!(store.get_layer_at(2).unwrap().id, id1);

        // Move layer 1 to back
        store.move_layer_to_back(id1);
        // New order: [1, 2, 3]
        assert_eq!(store.get_layer_at(0).unwrap().id, id1);
        assert_eq!(store.get_layer_at(1).unwrap().id, id2);
        assert_eq!(store.get_layer_at(2).unwrap().id, id3);
    }

    #[test]
    fn test_clear_layers() {
        let mut store = LayerStore::new();

        store.add_layer("Layer 1");
        store.add_layer("Layer 2");
        assert_eq!(store.layer_count(), 2);

        store.clear_layers();
        assert_eq!(store.layer_count(), 0);
        assert!(!store.has_layers());
    }

    #[test]
    fn test_move_layer_direct() {
        let mut store = LayerStore::new();

        store.add_layer("Layer 1");
        store.add_layer("Layer 2");
        store.add_layer("Layer 3");

        // Move first to last position
        store.move_layer(0, 2);
        assert_eq!(store.get_layer_at(0).unwrap().name, "Layer 2");
        assert_eq!(store.get_layer_at(1).unwrap().name, "Layer 3");
        assert_eq!(store.get_layer_at(2).unwrap().name, "Layer 1");
    }

    #[test]
    fn test_move_layer_out_of_bounds() {
        let mut store = LayerStore::new();

        store.add_layer("Layer 1");
        store.add_layer("Layer 2");

        // Should be no-op for out of bounds
        store.move_layer(0, 10);
        store.move_layer(10, 0);

        // Order should be unchanged
        assert_eq!(store.get_layer_at(0).unwrap().name, "Layer 1");
        assert_eq!(store.get_layer_at(1).unwrap().name, "Layer 2");
    }
}
