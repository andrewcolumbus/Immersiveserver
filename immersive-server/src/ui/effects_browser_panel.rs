//! Effects Browser Panel
//!
//! Displays available effects that can be dragged onto layers or clips:
//! - Color Correction
//! - Invert
//! - (More effects to come)
//!
//! Effects are organized by category and can be searched.

use crate::effects::EffectRegistry;

/// Represents an effect that can be dragged to a layer or clip
#[derive(Debug, Clone)]
pub struct DraggableEffect {
    /// Effect type ID (e.g., "color_correction")
    pub effect_type: String,
    /// Display name (e.g., "Color Correction")
    pub name: String,
    /// Category (e.g., "Color")
    pub category: String,
}

impl DraggableEffect {
    /// Get a tooltip description
    pub fn tooltip(&self) -> String {
        format!(
            "{}\nCategory: {}\n\nDrag to a layer or clip to add effect",
            self.name, self.category
        )
    }
}

/// Payload type for egui drag-and-drop
pub const DRAG_EFFECT_PAYLOAD: &str = "draggable_effect";

/// Actions that can be returned from the effects browser panel
#[derive(Debug, Clone)]
pub enum EffectsBrowserAction {
    /// Add effect to a layer
    AddToLayer {
        layer_id: u32,
        effect_type: String,
    },
    /// Add effect to a clip
    AddToClip {
        layer_id: u32,
        slot: usize,
        effect_type: String,
    },
    /// Add effect to the environment (master effects)
    AddToEnvironment {
        effect_type: String,
    },
}

/// State for the effects browser panel
pub struct EffectsBrowserPanel {
    /// Whether the panel is open
    pub open: bool,
    /// Search filter text
    search_text: String,
    /// Cached list of effects by category
    effects_by_category: Vec<(String, Vec<DraggableEffect>)>,
    /// Whether the cache needs to be refreshed
    cache_dirty: bool,
}

impl Default for EffectsBrowserPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl EffectsBrowserPanel {
    /// Create a new effects browser panel
    pub fn new() -> Self {
        Self {
            open: true,
            search_text: String::new(),
            effects_by_category: Vec::new(),
            cache_dirty: true,
        }
    }

    /// Refresh the effects cache from the registry
    pub fn refresh_cache(&mut self, registry: &EffectRegistry) {
        self.effects_by_category.clear();

        // Get all categories
        let categories = registry.categories();

        for category in categories {
            if let Some(effect_types) = registry.effects_in_category(category) {
                let effects: Vec<DraggableEffect> = effect_types
                    .iter()
                    .filter_map(|effect_type| {
                        registry.get(effect_type).map(|def| DraggableEffect {
                            effect_type: effect_type.clone(),
                            name: def.display_name().to_string(),
                            category: category.clone(),
                        })
                    })
                    .collect();

                if !effects.is_empty() {
                    self.effects_by_category.push((category.clone(), effects));
                }
            }
        }

        self.cache_dirty = false;
    }

    /// Render the effects browser panel contents
    pub fn render_contents(
        &mut self,
        ui: &mut egui::Ui,
        registry: &EffectRegistry,
    ) -> Vec<EffectsBrowserAction> {
        let actions = Vec::new();

        // Refresh cache if needed
        if self.cache_dirty {
            self.refresh_cache(registry);
        }

        // Search box
        ui.horizontal(|ui| {
            ui.label("🔍");
            ui.add(
                egui::TextEdit::singleline(&mut self.search_text)
                    .hint_text("Search effects...")
                    .desired_width(ui.available_width() - 30.0),
            );
            if !self.search_text.is_empty() && ui.small_button("✕").clicked() {
                self.search_text.clear();
            }
        });
        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        // Filter effects by search
        let search_lower = self.search_text.to_lowercase();

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                for (category, effects) in &self.effects_by_category {
                    // Filter effects in this category
                    let filtered_effects: Vec<&DraggableEffect> = if search_lower.is_empty() {
                        effects.iter().collect()
                    } else {
                        effects
                            .iter()
                            .filter(|e| {
                                e.name.to_lowercase().contains(&search_lower)
                                    || e.effect_type.to_lowercase().contains(&search_lower)
                                    || e.category.to_lowercase().contains(&search_lower)
                            })
                            .collect()
                    };

                    if filtered_effects.is_empty() {
                        continue;
                    }

                    // Category header
                    ui.collapsing(
                        egui::RichText::new(format!("📁 {}", category)).strong(),
                        |ui| {
                            for effect in filtered_effects {
                                Self::render_effect_item(ui, effect);
                            }
                        },
                    )
                    .header_response
                    .on_hover_text(format!("{} effects in this category", effects.len()));
                }

                // Show message if no results
                if self
                    .effects_by_category
                    .iter()
                    .all(|(_, effects)| {
                        effects.iter().all(|e| {
                            !e.name.to_lowercase().contains(&search_lower)
                                && !e.effect_type.to_lowercase().contains(&search_lower)
                        })
                    })
                    && !search_lower.is_empty()
                {
                    ui.add_space(20.0);
                    ui.label(
                        egui::RichText::new("No effects match your search")
                            .italics()
                            .color(egui::Color32::GRAY),
                    );
                }
            });

        actions
    }

    /// Render a single effect item (draggable)
    fn render_effect_item(ui: &mut egui::Ui, effect: &DraggableEffect) {
        let effect_id = egui::Id::new(&effect.effect_type).with("effect_browser_item");

        // Effect icon based on category
        let icon = match effect.category.as_str() {
            "Color" => "🎨",
            "Blur" => "💨",
            "Distort" => "🌀",
            "Stylize" => "✨",
            "Generate" => "🔲",
            _ => "📦",
        };

        // Use egui's drag-and-drop source
        let response = ui
            .dnd_drag_source(effect_id, effect.clone(), |ui| {
                ui.horizontal(|ui| {
                    ui.label(icon);
                    ui.label(&effect.name);
                });
            })
            .response
            .on_hover_text(effect.tooltip());

        // Context menu
        response.context_menu(|ui| {
            if ui.button("Add to selected layer").clicked() {
                log::info!("Add effect {} to selected layer", effect.name);
                ui.close_menu();
            }
            if ui.button("Add to environment").clicked() {
                log::info!("Add effect {} to environment", effect.name);
                ui.close_menu();
            }
        });
    }

    /// Mark the cache as needing refresh
    pub fn invalidate_cache(&mut self) {
        self.cache_dirty = true;
    }
}
