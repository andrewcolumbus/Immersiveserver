//! Sources Panel
//!
//! Displays available video sources that can be dragged onto the clip grid:
//! - OMT (Open Media Transport) network sources
//! - NDI sources (future)
//! - Local video files (future)
//!
//! Drag sources from this panel and drop them onto clip grid cells.

use std::path::PathBuf;

/// Represents a source that can be dragged to the clip grid
#[derive(Debug, Clone)]
pub enum DraggableSource {
    /// OMT network source
    Omt {
        id: String,
        name: String,
        address: String,
    },
    /// Local video file
    File {
        path: PathBuf,
        name: String,
    },
    // Future: NDI source
    // Ndi { source_name: String },
}

impl DraggableSource {
    /// Get the display name for this source
    pub fn display_name(&self) -> &str {
        match self {
            DraggableSource::Omt { name, .. } => name,
            DraggableSource::File { name, .. } => name,
        }
    }

    /// Get the type indicator emoji
    pub fn type_indicator(&self) -> &'static str {
        match self {
            DraggableSource::Omt { .. } => "📡",
            DraggableSource::File { .. } => "📁",
        }
    }

    /// Get a tooltip description
    pub fn tooltip(&self) -> String {
        match self {
            DraggableSource::Omt { address, name, .. } => {
                format!("📡 OMT Source: {}\nAddress: {}\n\nDrag to clip grid to assign", name, address)
            }
            DraggableSource::File { path, .. } => {
                format!("📁 Video File\n{}\n\nDrag to clip grid to assign", path.display())
            }
        }
    }
}

/// Payload type for egui drag-and-drop
pub const DRAG_SOURCE_PAYLOAD: &str = "draggable_source";

/// Actions that can be returned from the sources panel
#[derive(Debug, Clone)]
pub enum SourcesAction {
    /// Refresh OMT source discovery
    RefreshOmtSources,
}

/// State for the sources panel
#[derive(Default)]
pub struct SourcesPanel {
    /// Whether the panel is open
    pub open: bool,
    /// Discovered OMT sources
    omt_sources: Vec<DraggableSource>,
}

impl SourcesPanel {
    /// Create a new sources panel
    pub fn new() -> Self {
        Self {
            open: true,
            omt_sources: Vec::new(),
        }
    }

    /// Update the list of OMT sources
    pub fn set_omt_sources(&mut self, sources: Vec<(String, String, String)>) {
        self.omt_sources = sources
            .into_iter()
            .map(|(id, name, address)| DraggableSource::Omt { id, name, address })
            .collect();
    }

    /// Render the sources panel contents
    pub fn render_contents(&mut self, ui: &mut egui::Ui) -> Vec<SourcesAction> {
        let mut actions = Vec::new();

        // Header
        ui.horizontal(|ui| {
            ui.heading("Sources");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("🔄").on_hover_text("Refresh sources").clicked() {
                    actions.push(SourcesAction::RefreshOmtSources);
                }
            });
        });
        ui.separator();

        // OMT Sources section
        let omt_sources = self.omt_sources.clone();
        ui.collapsing("📡 OMT Sources", |ui| {
            if omt_sources.is_empty() {
                ui.label(egui::RichText::new("No OMT sources found").italics().color(egui::Color32::GRAY));
                ui.add_space(4.0);
                if ui.small_button("🔄 Refresh").clicked() {
                    actions.push(SourcesAction::RefreshOmtSources);
                }
            } else {
                for source in &omt_sources {
                    self.render_draggable_source(ui, source.clone());
                }
            }
        });

        ui.add_space(8.0);

        // Instructions
        ui.separator();
        ui.vertical_centered(|ui| {
            ui.label(egui::RichText::new("Drag sources to clip grid").size(11.0).color(egui::Color32::GRAY));
        });

        actions
    }

    /// Render a single draggable source item
    fn render_draggable_source(&self, ui: &mut egui::Ui, source: DraggableSource) {
        let id = egui::Id::new(format!("source_{}", source.display_name()));
        
        // Create a frame for the source item
        let frame = egui::Frame::new()
            .fill(egui::Color32::from_rgb(45, 45, 55))
            .corner_radius(4.0)
            .inner_margin(egui::Margin::symmetric(8, 6));

        let response = frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(source.type_indicator());
                ui.label(source.display_name());
            });
        }).response;

        // Make it draggable
        let response = ui.interact(response.rect, id, egui::Sense::drag());

        // Show tooltip
        let response = response.on_hover_text(source.tooltip());

        // Set drag payload when dragging
        if response.dragged() {
            // Store the source in egui's drag-and-drop payload
            egui::DragAndDrop::set_payload(ui.ctx(), source.clone());
        }

        // Visual feedback when dragging
        if response.dragged() {
            // Draw a ghost of the item at cursor
            if let Some(pointer_pos) = ui.ctx().pointer_latest_pos() {
                egui::Area::new(egui::Id::new("drag_ghost"))
                    .fixed_pos(pointer_pos + egui::vec2(10.0, 10.0))
                    .order(egui::Order::Tooltip)
                    .show(ui.ctx(), |ui| {
                        egui::Frame::popup(ui.style())
                            .fill(egui::Color32::from_rgba_unmultiplied(60, 60, 80, 220))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(source.type_indicator());
                                    ui.label(source.display_name());
                                });
                            });
                    });
            }
        }
    }

    /// Render as a floating window
    pub fn render(&mut self, ctx: &egui::Context) -> Vec<SourcesAction> {
        if !self.open {
            return Vec::new();
        }

        let mut actions = Vec::new();
        
        egui::Window::new("Sources")
            .default_size([250.0, 300.0])
            .resizable(true)
            .collapsible(true)
            .show(ctx, |ui| {
                actions = self.render_contents(ui);
            });

        actions
    }
}

/// Check if there's a source being dragged over a position and return it
pub fn get_dropped_source(ctx: &egui::Context) -> Option<DraggableSource> {
    // Check if a drag operation just ended
    if ctx.input(|i| i.pointer.any_released()) {
        // Try to get the payload
        if let Some(payload) = egui::DragAndDrop::take_payload::<DraggableSource>(ctx) {
            return Some((*payload).clone());
        }
    }
    None
}

/// Check if there's a source currently being dragged
pub fn is_source_being_dragged(ctx: &egui::Context) -> bool {
    egui::DragAndDrop::has_payload_of_type::<DraggableSource>(ctx)
}

/// Peek at the currently dragged source without consuming it
pub fn peek_dragged_source(ctx: &egui::Context) -> Option<DraggableSource> {
    egui::DragAndDrop::payload::<DraggableSource>(ctx).map(|p| (*p).clone())
}

