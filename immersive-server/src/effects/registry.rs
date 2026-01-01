//! Effect registry for managing available effects
//!
//! The registry holds all registered effect definitions and provides
//! methods to query and create effect instances.

use std::collections::HashMap;
use std::sync::Arc;

use super::traits::{EffectDefinition, EffectProcessor};
use super::Parameter;

/// Registry of available effects
///
/// Effects are registered at startup and can be queried by type or category.
/// The registry owns the effect definitions and provides factory methods.
pub struct EffectRegistry {
    /// Effect definitions by type identifier
    effects: HashMap<String, Arc<dyn EffectDefinition>>,
    /// Effect types grouped by category
    categories: HashMap<String, Vec<String>>,
    /// Ordered list of categories for UI display
    category_order: Vec<String>,
}

impl Default for EffectRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl EffectRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            effects: HashMap::new(),
            categories: HashMap::new(),
            category_order: Vec::new(),
        }
    }

    /// Register an effect definition
    pub fn register(&mut self, definition: impl EffectDefinition + 'static) {
        let effect_type = definition.effect_type().to_string();
        let category = definition.category().to_string();

        // Add to category index
        if !self.categories.contains_key(&category) {
            self.categories.insert(category.clone(), Vec::new());
            self.category_order.push(category.clone());
        }
        self.categories
            .get_mut(&category)
            .unwrap()
            .push(effect_type.clone());

        // Store the definition
        self.effects.insert(effect_type, Arc::new(definition));
    }

    /// Get an effect definition by type
    pub fn get(&self, effect_type: &str) -> Option<Arc<dyn EffectDefinition>> {
        self.effects.get(effect_type).cloned()
    }

    /// Check if an effect type is registered
    pub fn contains(&self, effect_type: &str) -> bool {
        self.effects.contains_key(effect_type)
    }

    /// Get all registered effect types
    pub fn effect_types(&self) -> impl Iterator<Item = &str> {
        self.effects.keys().map(|s| s.as_str())
    }

    /// Get all effect definitions
    pub fn effects(&self) -> impl Iterator<Item = &Arc<dyn EffectDefinition>> {
        self.effects.values()
    }

    /// Get the number of registered effects
    pub fn len(&self) -> usize {
        self.effects.len()
    }

    /// Check if the registry is empty
    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    /// Get all category names in display order
    pub fn categories(&self) -> &[String] {
        &self.category_order
    }

    /// Get all effect types in a category
    pub fn effects_in_category(&self, category: &str) -> Option<&[String]> {
        self.categories.get(category).map(|v| v.as_slice())
    }

    /// Get default parameters for an effect type
    pub fn default_parameters(&self, effect_type: &str) -> Option<Vec<Parameter>> {
        self.get(effect_type).map(|def| def.default_parameters())
    }

    /// Get the display name for an effect type
    pub fn display_name(&self, effect_type: &str) -> Option<&'static str> {
        self.get(effect_type).map(|def| def.display_name())
    }

    /// Get the processor type for an effect
    pub fn processor(&self, effect_type: &str) -> Option<EffectProcessor> {
        self.get(effect_type).map(|def| def.processor())
    }

    /// Create a GPU runtime for an effect
    pub fn create_gpu_runtime(
        &self,
        effect_type: &str,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        output_format: wgpu::TextureFormat,
    ) -> Option<Box<dyn super::traits::GpuEffectRuntime>> {
        self.get(effect_type)
            .and_then(|def| def.create_gpu_runtime(device, queue, output_format))
    }

    /// Create a CPU runtime for an effect
    pub fn create_cpu_runtime(
        &self,
        effect_type: &str,
    ) -> Option<Box<dyn super::traits::CpuEffectRuntime>> {
        self.get(effect_type).and_then(|def| def.create_cpu_runtime())
    }

    /// Get all effects matching a filter
    pub fn search(&self, query: &str) -> Vec<Arc<dyn EffectDefinition>> {
        let query_lower = query.to_lowercase();
        self.effects
            .values()
            .filter(|def| {
                def.display_name().to_lowercase().contains(&query_lower)
                    || def.effect_type().to_lowercase().contains(&query_lower)
                    || def.category().to_lowercase().contains(&query_lower)
            })
            .cloned()
            .collect()
    }

    /// Get all GPU effects
    pub fn gpu_effects(&self) -> impl Iterator<Item = &Arc<dyn EffectDefinition>> {
        self.effects
            .values()
            .filter(|def| def.processor() == EffectProcessor::Gpu)
    }

    /// Get all CPU effects
    pub fn cpu_effects(&self) -> impl Iterator<Item = &Arc<dyn EffectDefinition>> {
        self.effects
            .values()
            .filter(|def| def.processor() == EffectProcessor::Cpu)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::traits::GpuEffectRuntime;
    use crate::effects::Parameter;

    // Mock effect for testing
    struct MockEffect {
        effect_type: &'static str,
        display_name: &'static str,
        category: &'static str,
    }

    impl EffectDefinition for MockEffect {
        fn effect_type(&self) -> &'static str {
            self.effect_type
        }

        fn display_name(&self) -> &'static str {
            self.display_name
        }

        fn category(&self) -> &'static str {
            self.category
        }

        fn processor(&self) -> EffectProcessor {
            EffectProcessor::Gpu
        }

        fn default_parameters(&self) -> Vec<Parameter> {
            vec![]
        }

        fn create_gpu_runtime(
            &self,
            _device: &wgpu::Device,
            _queue: &wgpu::Queue,
            _output_format: wgpu::TextureFormat,
        ) -> Option<Box<dyn GpuEffectRuntime>> {
            None
        }

        fn create_cpu_runtime(
            &self,
        ) -> Option<Box<dyn super::super::traits::CpuEffectRuntime>> {
            None
        }
    }

    #[test]
    fn test_registry_new() {
        let registry = EffectRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_registry_register() {
        let mut registry = EffectRegistry::new();

        registry.register(MockEffect {
            effect_type: "test_effect",
            display_name: "Test Effect",
            category: "Test",
        });

        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);
        assert!(registry.contains("test_effect"));
    }

    #[test]
    fn test_registry_categories() {
        let mut registry = EffectRegistry::new();

        registry.register(MockEffect {
            effect_type: "color_correct",
            display_name: "Color Correct",
            category: "Color",
        });

        registry.register(MockEffect {
            effect_type: "invert",
            display_name: "Invert",
            category: "Color",
        });

        registry.register(MockEffect {
            effect_type: "blur",
            display_name: "Blur",
            category: "Blur",
        });

        assert_eq!(registry.categories().len(), 2);
        assert_eq!(registry.effects_in_category("Color").unwrap().len(), 2);
        assert_eq!(registry.effects_in_category("Blur").unwrap().len(), 1);
    }

    #[test]
    fn test_registry_search() {
        let mut registry = EffectRegistry::new();

        registry.register(MockEffect {
            effect_type: "color_correction",
            display_name: "Color Correction",
            category: "Color",
        });

        registry.register(MockEffect {
            effect_type: "invert",
            display_name: "Invert",
            category: "Color",
        });

        let results = registry.search("color");
        assert_eq!(results.len(), 2); // Both are in "Color" category

        let results = registry.search("invert");
        assert_eq!(results.len(), 1);
    }
}
