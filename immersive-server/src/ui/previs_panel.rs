//! 3D Previsualization Panel
//!
//! Displays a 3D preview of the environment texture mapped onto
//! configurable surfaces (circle, walls, dome).

use crate::previs::{PrevisRenderer, PrevisSettings, SurfaceType};
use crate::ui::{draw_texture, draw_texture_placeholder};
use egui::PointerButton;

/// Which wall is being modified
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallId {
    Front,
    Back,
    Left,
    Right,
}

/// Actions that can be returned from the previs panel
#[derive(Debug, Clone)]
pub enum PrevisAction {
    /// Change the surface type
    SetSurfaceType(SurfaceType),
    /// Toggle previs enabled state
    SetEnabled(bool),
    // Circle parameters
    SetCircleRadius(f32),
    SetCircleSegments(u32),
    // Individual wall parameters
    SetWallEnabled(WallId, bool),
    SetWallWidth(WallId, f32),
    SetWallHeight(WallId, f32),
    // Floor parameters (for walls mode)
    SetFloorEnabled(bool),
    SetFloorLayerIndex(usize),
    // Dome parameters
    SetDomeRadius(f32),
    SetDomeSegmentsH(u32),
    SetDomeSegmentsV(u32),
    // Camera state (saved periodically)
    SaveCameraState { yaw: f32, pitch: f32, distance: f32 },
    // Reset camera to default position
    ResetCamera,
}

/// State for the previs panel
pub struct PrevisPanel {
    /// egui texture ID for the rendered 3D view
    pub texture_id: Option<egui::TextureId>,
    /// Size of the 3D viewport in the panel (in logical pixels)
    viewport_size: (f32, f32),
    /// Whether the camera is being dragged
    is_dragging: bool,
}

impl Default for PrevisPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl PrevisPanel {
    /// Create a new previs panel
    pub fn new() -> Self {
        Self {
            texture_id: None,
            viewport_size: (320.0, 240.0),
            is_dragging: false,
        }
    }

    /// Render the panel contents
    ///
    /// Returns a list of actions to be handled by the app.
    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        settings: &PrevisSettings,
        renderer: &mut PrevisRenderer,
    ) -> Vec<PrevisAction> {
        let mut actions = Vec::new();

        // Header with enable toggle
        ui.horizontal(|ui| {
            let mut enabled = settings.enabled;
            if ui.checkbox(&mut enabled, "Enable 3D Preview").changed() {
                actions.push(PrevisAction::SetEnabled(enabled));
            }
        });

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        // Surface type selector
        ui.horizontal(|ui| {
            ui.label("Surface:");
            egui::ComboBox::from_id_salt("previs_surface_type")
                .selected_text(settings.surface_type.display_name())
                .show_ui(ui, |ui| {
                    for surface in SurfaceType::all() {
                        let is_selected = settings.surface_type == *surface;
                        if ui
                            .selectable_label(is_selected, surface.display_name())
                            .clicked()
                            && !is_selected
                        {
                            actions.push(PrevisAction::SetSurfaceType(*surface));
                        }
                    }
                });
        });

        ui.add_space(8.0);

        // Surface-specific parameters
        match settings.surface_type {
            SurfaceType::Circle => {
                self.render_circle_params(ui, settings, &mut actions);
            }
            SurfaceType::Walls => {
                self.render_walls_params(ui, settings, &mut actions);
            }
            SurfaceType::Dome => {
                self.render_dome_params(ui, settings, &mut actions);
            }
        }

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // Camera controls header
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Camera").strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Reset").clicked() {
                    actions.push(PrevisAction::ResetCamera);
                }
            });
        });

        ui.add_space(4.0);

        // Camera info
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Drag to orbit, scroll to zoom").small().weak());
        });

        ui.add_space(8.0);

        // 3D Viewport
        let available_size = ui.available_size();
        let viewport_size = egui::vec2(
            available_size.x.max(200.0),
            (available_size.y - 20.0).max(150.0),
        );

        let (rect, response) = ui.allocate_exact_size(viewport_size, egui::Sense::click_and_drag());

        // Handle orbit camera input
        if response.drag_started() {
            self.is_dragging = true;
        }
        if response.drag_stopped() {
            self.is_dragging = false;
            // Save camera state when drag ends
            actions.push(PrevisAction::SaveCameraState {
                yaw: renderer.camera().yaw(),
                pitch: renderer.camera().pitch(),
                distance: renderer.camera().distance(),
            });
        }

        if response.dragged_by(egui::PointerButton::Primary) {
            let delta = response.drag_delta();
            renderer
                .camera_mut()
                .on_mouse_drag((delta.x * 0.01, delta.y * 0.01), 1.0);
        }

        // Handle scroll zoom (only when hovering over viewport)
        if response.hovered() {
            let scroll = ui.input(|i| i.raw_scroll_delta.y);
            if scroll.abs() > 0.0 {
                renderer.camera_mut().on_scroll(scroll * 0.01);
                // Save camera state after scroll
                actions.push(PrevisAction::SaveCameraState {
                    yaw: renderer.camera().yaw(),
                    pitch: renderer.camera().pitch(),
                    distance: renderer.camera().distance(),
                });
            }
        }

        // Draw the 3D rendered texture or placeholder
        if let Some(tex_id) = self.texture_id {
            draw_texture(ui, tex_id, rect);
        } else {
            let msg = if settings.enabled { "3D Preview" } else { "Preview Disabled" };
            draw_texture_placeholder(ui, rect, msg);
        }

        // Draw border around viewport
        ui.painter().rect_stroke(
            rect,
            4.0,
            egui::Stroke::new(1.0, egui::Color32::from_gray(60)),
            egui::StrokeKind::Outside,
        );

        // Store viewport size for render target sizing
        self.viewport_size = (viewport_size.x, viewport_size.y);

        actions
    }

    fn render_circle_params(
        &self,
        ui: &mut egui::Ui,
        settings: &PrevisSettings,
        actions: &mut Vec<PrevisAction>,
    ) {
        ui.label(egui::RichText::new("Circle Parameters").strong());
        ui.add_space(4.0);

        let mut radius = settings.circle_radius;
        let response = ui.add(egui::Slider::new(&mut radius, 1.0..=20.0).text("Radius"));
        if response.changed() {
            actions.push(PrevisAction::SetCircleRadius(radius));
        }
        // Right-click instantly resets to 5.0
        if response.clicked_by(PointerButton::Secondary) {
            actions.push(PrevisAction::SetCircleRadius(5.0));
        }

        let mut segments = settings.circle_segments;
        let response = ui.add(egui::Slider::new(&mut segments, 8..=64).text("Segments"));
        if response.changed() {
            actions.push(PrevisAction::SetCircleSegments(segments));
        }
        // Right-click instantly resets to 32
        if response.clicked_by(PointerButton::Secondary) {
            actions.push(PrevisAction::SetCircleSegments(32));
        }
    }

    fn render_walls_params(
        &self,
        ui: &mut egui::Ui,
        settings: &PrevisSettings,
        actions: &mut Vec<PrevisAction>,
    ) {
        ui.label(egui::RichText::new("Walls Parameters").strong());
        ui.add_space(2.0);
        ui.label(egui::RichText::new("Inside view (camera at center)").small().weak());
        ui.add_space(4.0);

        // Helper to render a single wall's controls
        let mut render_wall = |ui: &mut egui::Ui,
                               wall_id: WallId,
                               name: &str,
                               enabled: bool,
                               width: f32,
                               height: f32| {
            egui::CollapsingHeader::new(name)
                .default_open(true)
                .show(ui, |ui| {
                    let mut wall_enabled = enabled;
                    if ui.checkbox(&mut wall_enabled, "Enabled").changed() {
                        actions.push(PrevisAction::SetWallEnabled(wall_id, wall_enabled));
                    }

                    ui.add_enabled_ui(wall_enabled, |ui| {
                        let mut w = width;
                        let response = ui.add(egui::Slider::new(&mut w, 1.0..=10.0).text("Width"));
                        if response.changed() {
                            actions.push(PrevisAction::SetWallWidth(wall_id, w));
                        }
                        // Right-click instantly resets to 4.0
                        if response.clicked_by(PointerButton::Secondary) {
                            actions.push(PrevisAction::SetWallWidth(wall_id, 4.0));
                        }

                        let mut h = height;
                        let response = ui.add(egui::Slider::new(&mut h, 1.0..=10.0).text("Height"));
                        if response.changed() {
                            actions.push(PrevisAction::SetWallHeight(wall_id, h));
                        }
                        // Right-click instantly resets to 3.0
                        if response.clicked_by(PointerButton::Secondary) {
                            actions.push(PrevisAction::SetWallHeight(wall_id, 3.0));
                        }
                    });
                });
        };

        render_wall(
            ui,
            WallId::Front,
            "Front Wall",
            settings.wall_front.enabled,
            settings.wall_front.width,
            settings.wall_front.height,
        );
        render_wall(
            ui,
            WallId::Back,
            "Back Wall",
            settings.wall_back.enabled,
            settings.wall_back.width,
            settings.wall_back.height,
        );
        render_wall(
            ui,
            WallId::Left,
            "Left Wall",
            settings.wall_left.enabled,
            settings.wall_left.width,
            settings.wall_left.height,
        );
        render_wall(
            ui,
            WallId::Right,
            "Right Wall",
            settings.wall_right.enabled,
            settings.wall_right.width,
            settings.wall_right.height,
        );

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // Floor settings
        ui.label(egui::RichText::new("Floor").strong());
        ui.add_space(4.0);

        let mut floor_enabled = settings.floor_enabled;
        if ui.checkbox(&mut floor_enabled, "Enable Floor").changed() {
            actions.push(PrevisAction::SetFloorEnabled(floor_enabled));
        }

        ui.add_enabled_ui(floor_enabled, |ui| {
            ui.horizontal(|ui| {
                ui.label("Layer:");
                let mut layer_idx = settings.floor_layer_index;
                if ui
                    .add(egui::DragValue::new(&mut layer_idx).range(0..=15).speed(0.1))
                    .changed()
                {
                    actions.push(PrevisAction::SetFloorLayerIndex(layer_idx));
                }
                ui.label(egui::RichText::new("(0 = first layer)").small().weak());
            });
        });
    }

    fn render_dome_params(
        &self,
        ui: &mut egui::Ui,
        settings: &PrevisSettings,
        actions: &mut Vec<PrevisAction>,
    ) {
        ui.label(egui::RichText::new("Dome Parameters").strong());
        ui.add_space(2.0);
        ui.label(egui::RichText::new("Inside view (camera at base)").small().weak());
        ui.add_space(4.0);

        let mut radius = settings.dome_radius;
        let response = ui.add(egui::Slider::new(&mut radius, 1.0..=20.0).text("Radius"));
        if response.changed() {
            actions.push(PrevisAction::SetDomeRadius(radius));
        }
        // Right-click instantly resets to 5.0
        if response.clicked_by(PointerButton::Secondary) {
            actions.push(PrevisAction::SetDomeRadius(5.0));
        }

        let mut h_seg = settings.dome_segments_horizontal;
        let response = ui.add(egui::Slider::new(&mut h_seg, 8..=64).text("H Segments"));
        if response.changed() {
            actions.push(PrevisAction::SetDomeSegmentsH(h_seg));
        }
        // Right-click instantly resets to 32
        if response.clicked_by(PointerButton::Secondary) {
            actions.push(PrevisAction::SetDomeSegmentsH(32));
        }

        let mut v_seg = settings.dome_segments_vertical;
        let response = ui.add(egui::Slider::new(&mut v_seg, 4..=32).text("V Segments"));
        if response.changed() {
            actions.push(PrevisAction::SetDomeSegmentsV(v_seg));
        }
        // Right-click instantly resets to 16
        if response.clicked_by(PointerButton::Secondary) {
            actions.push(PrevisAction::SetDomeSegmentsV(16));
        }
    }

    /// Get the desired viewport size for render target sizing
    pub fn viewport_size(&self) -> (u32, u32) {
        (self.viewport_size.0 as u32, self.viewport_size.1 as u32)
    }

    /// Check if currently dragging
    pub fn is_dragging(&self) -> bool {
        self.is_dragging
    }
}
