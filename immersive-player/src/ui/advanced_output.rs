//! Advanced Output window - Resolume-style output configuration
//!
//! Main window for configuring output screens, slices, and devices.
//! Features a three-panel layout: Screen Tree | Canvas | Properties
//! All screen/output configuration is consolidated here.

use crate::output::OutputManager;
use crate::ui::{BlendPanel, MainWindow, OutputEditor, ScreenTreePanel, TreeSelection, WarpPanel};
use crate::video::VideoPlayer;
use eframe::egui::{self, Color32, RichText, Vec2};

/// Property panel mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PropertyMode {
    #[default]
    General,
    Blend,
    Warp,
    Mask,
}

/// Advanced Output window state
pub struct AdvancedOutputWindow {
    /// Whether the window is open
    pub is_open: bool,
    /// Whether changes should be saved when window closes
    pub should_save: bool,
    /// Screen tree panel (left)
    pub screen_tree: ScreenTreePanel,
    /// Output editor canvas (center)
    pub output_editor: OutputEditor,
    /// Main window for properties (right)
    pub main_window: MainWindow,
    /// Blend panel for edge blending
    pub blend_panel: BlendPanel,
    /// Warp panel for geometric correction
    pub warp_panel: WarpPanel,
    /// Current selection in the tree
    pub selection: TreeSelection,
    /// Show test pattern on outputs
    pub show_test_pattern: bool,
    /// Current property panel mode
    pub property_mode: PropertyMode,
}

impl Default for AdvancedOutputWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl AdvancedOutputWindow {
    pub fn new() -> Self {
        Self {
            is_open: false,
            should_save: false,
            screen_tree: ScreenTreePanel::new(),
            output_editor: OutputEditor::new(),
            main_window: MainWindow::default(),
            blend_panel: BlendPanel::new(),
            warp_panel: WarpPanel::new(),
            selection: TreeSelection::None,
            show_test_pattern: false,
            property_mode: PropertyMode::General,
        }
    }

    /// Toggle window visibility
    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
    }

    /// Get the current composition size from the output editor
    pub fn get_composition_size(&self) -> (u32, u32) {
        self.output_editor.composition_size
    }

    /// Set the composition size in the output editor
    pub fn set_composition_size(&mut self, width: u32, height: u32) {
        self.output_editor.composition_size = (width, height);
    }

    /// Show the Advanced Output window
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        output_manager: &mut OutputManager,
        player: &VideoPlayer,
    ) {
        if !self.is_open {
            return;
        }

        let mut open = self.is_open;

        egui::Window::new("Advanced Output")
            .open(&mut open)
            .default_size([1000.0, 700.0])
            .min_width(600.0)
            .min_height(400.0)
            .resizable(true)
            .collapsible(true)
            .show(ctx, |ui| {
                self.show_content(ui, output_manager, player);
            });

        // Only use the X button state if buttons didn't already close the window
        // (buttons set self.is_open = false directly, so don't overwrite that)
        if self.is_open {
            self.is_open = open;
        }
    }

    /// Show window content
    fn show_content(
        &mut self,
        ui: &mut egui::Ui,
        output_manager: &mut OutputManager,
        player: &VideoPlayer,
    ) {
        // Top menu bar
        self.show_menu_bar(ui, output_manager);

        ui.separator();

        // Use a horizontal layout with proper constraints
        let available = ui.available_size();
        let panel_height = (available.y - 50.0).max(200.0); // Reserve space for bottom bar

        ui.horizontal(|ui| {
            // Left panel - Screen Tree
            let left_width = (available.x * 0.25).clamp(150.0, 280.0);
            ui.allocate_ui_with_layout(
                Vec2::new(left_width, panel_height),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    ui.set_min_size(Vec2::new(left_width, panel_height));
                    egui::Frame::none()
                        .fill(Color32::from_gray(35))
                        .inner_margin(4.0)
                        .show(ui, |ui| {
                            self.show_left_panel(ui, output_manager);
                        });
                },
            );

            ui.separator();

            // Center panel - Canvas (takes remaining space minus right panel)
            let right_width = (available.x * 0.28).clamp(180.0, 300.0);
            let center_width = (available.x - left_width - right_width - 20.0).max(200.0);
            ui.allocate_ui_with_layout(
                Vec2::new(center_width, panel_height),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    ui.set_min_size(Vec2::new(center_width, panel_height));
                    self.show_center_panel(ui, output_manager, player);
                },
            );

            ui.separator();

            // Right panel - Properties
            ui.allocate_ui_with_layout(
                Vec2::new(right_width, panel_height),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    ui.set_min_size(Vec2::new(right_width, panel_height));
                    egui::Frame::none()
                        .fill(Color32::from_gray(35))
                        .inner_margin(4.0)
                        .show(ui, |ui| {
                            self.show_right_panel(ui, output_manager);
                        });
                },
            );
        });

        ui.add_space(4.0);

        // Bottom bar with Save & Close
        self.show_bottom_bar(ui, output_manager);
    }

    /// Show the top menu bar
    fn show_menu_bar(&mut self, ui: &mut egui::Ui, output_manager: &mut OutputManager) {
        ui.horizontal(|ui| {
            // Preset dropdown
            egui::ComboBox::from_id_source("preset_dropdown")
                .selected_text("Default")
                .width(120.0)
                .show_ui(ui, |ui| {
                    let _ = ui.selectable_label(true, "Default");
                    let _ = ui.selectable_label(false, "ShowReady");
                    ui.separator();
                    let _ = ui.selectable_label(false, "Save As...");
                    let _ = ui.selectable_label(false, "Load...");
                });

            ui.separator();

            // Quick actions
            if ui.button("+ Add Screen").clicked() {
                let mut screen = crate::output::Screen::new(
                    format!("Screen {}", output_manager.screens.len() + 1),
                    0,
                    (1920, 1080),
                );
                screen.add_slice(crate::output::Slice::full_screen(1920, 1080));
                output_manager.add_screen(screen);
            }

            if ui.button("🔄 Detect Displays").clicked() {
                output_manager.enumerate_displays();
            }

            if ui.button("🔗 Auto-Blend").clicked() {
                let count = output_manager.auto_blend();
                log::info!("Auto-blend configured {} regions", count);
            }

            ui.separator();

            // Go Live / Stop Output button
            if output_manager.is_live {
                if ui.add(
                    egui::Button::new(RichText::new("⏹ Stop Output").size(13.0))
                        .fill(Color32::from_rgb(200, 60, 60))
                        .min_size(Vec2::new(110.0, 26.0))
                ).clicked() {
                    output_manager.stop_outputs();
                }
            } else {
                if ui.add(
                    egui::Button::new(RichText::new("▶ Go Live").size(13.0))
                        .fill(Color32::from_rgb(74, 157, 91))
                        .min_size(Vec2::new(90.0, 26.0))
                ).clicked() {
                    // Sync test pattern setting before going live
                    output_manager.show_test_pattern = self.show_test_pattern;
                    output_manager.go_live();
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Test pattern toggle
                let mut test_pattern = self.show_test_pattern;
                if ui.checkbox(&mut test_pattern, "Test Pattern").changed() {
                    self.show_test_pattern = test_pattern;
                    output_manager.show_test_pattern = test_pattern;
                }

                // Live status indicator
                if output_manager.is_live {
                    ui.label(
                        RichText::new("🔴 LIVE")
                            .size(12.0)
                            .color(Color32::from_rgb(255, 80, 80))
                    );
                    ui.label(
                        RichText::new(format!("{} output(s)", output_manager.window_manager.active_count()))
                            .size(11.0)
                            .color(Color32::from_gray(150))
                    );
                }
            });
        });
    }

    /// Show left panel (Screen Tree)
    fn show_left_panel(&mut self, ui: &mut egui::Ui, output_manager: &mut OutputManager) {
        ui.set_min_width(260.0);

        egui::ScrollArea::vertical()
            .id_source("advanced_output_left_scroll")
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                // Tree panel
                self.screen_tree.show(ui, output_manager, &mut self.selection);

                ui.add_space(8.0);

                // Help section at bottom
                ui.separator();
                ui.colored_label(Color32::from_rgb(74, 157, 91), "Help");

                let help_text = match &self.selection {
                    TreeSelection::None => "Select a screen or slice to edit.",
                    TreeSelection::Screen(_) => {
                        "Drag to reposition. Use properties panel to configure blend."
                    }
                    TreeSelection::Slice(_, _) => "Edit slice input region and warp transformation.",
                    TreeSelection::Mask(_, _) => "Configure mask settings for this slice.",
                };

                ui.label(RichText::new(help_text).size(11.0).color(Color32::from_gray(140)));
            });
    }

    /// Show center panel (Canvas)
    fn show_center_panel(
        &mut self,
        ui: &mut egui::Ui,
        output_manager: &mut OutputManager,
        player: &VideoPlayer,
    ) {
        // Convert TreeSelection to screen/slice indices for the editor
        let (mut selected_screen, mut selected_slice) = match &self.selection {
            TreeSelection::None => (None, None),
            TreeSelection::Screen(idx) => (Some(*idx), None),
            TreeSelection::Slice(screen_idx, slice_idx) => (Some(*screen_idx), Some(*slice_idx)),
            TreeSelection::Mask(screen_idx, slice_idx) => (Some(*screen_idx), Some(*slice_idx)),
        };

        self.output_editor.show(
            ui,
            output_manager,
            player,
            &mut selected_screen,
            &mut selected_slice,
            self.show_test_pattern,
        );

        // Update selection based on canvas interaction
        self.selection = match (selected_screen, selected_slice) {
            (None, _) => TreeSelection::None,
            (Some(screen), None) => TreeSelection::Screen(screen),
            (Some(screen), Some(slice)) => TreeSelection::Slice(screen, slice),
        };
    }

    /// Show right panel (Properties with Blend/Warp/Mask)
    fn show_right_panel(&mut self, ui: &mut egui::Ui, output_manager: &mut OutputManager) {
        ui.set_min_width(280.0);

        // Property mode tabs
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.property_mode, PropertyMode::General, "General");
            ui.selectable_value(&mut self.property_mode, PropertyMode::Blend, "Blend");
            ui.selectable_value(&mut self.property_mode, PropertyMode::Warp, "Warp");
            ui.selectable_value(&mut self.property_mode, PropertyMode::Mask, "Mask");
        });

        ui.separator();

        egui::ScrollArea::vertical()
            .id_source("advanced_output_right_scroll")
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                match self.property_mode {
                    PropertyMode::General => {
                        self.show_general_properties(ui, output_manager);
                    }
                    PropertyMode::Blend => {
                        self.show_blend_properties(ui, output_manager);
                    }
                    PropertyMode::Warp => {
                        self.show_warp_properties(ui, output_manager);
                    }
                    PropertyMode::Mask => {
                        self.show_mask_properties(ui, output_manager);
                    }
                }
            });
    }

    /// Show general properties based on selection
    fn show_general_properties(&mut self, ui: &mut egui::Ui, output_manager: &mut OutputManager) {
        match &self.selection {
            TreeSelection::None => {
                ui.label("Select a screen or slice to view properties.");
                ui.add_space(16.0);
                ui.label(
                    RichText::new("Tip: Use the tree on the left to select items, or click directly on the canvas.")
                        .size(11.0)
                        .color(Color32::from_gray(100)),
                );
            }
            TreeSelection::Screen(screen_idx) => {
                let screen_idx = *screen_idx;
                // Clone displays to avoid borrow conflict with mutable screens access
                let displays = output_manager.displays.clone();
                if let Some(screen) = output_manager.screens.get_mut(screen_idx) {
                    self.main_window.show_properties(ui, screen, &displays);
                }
            }
            TreeSelection::Slice(screen_idx, slice_idx) => {
                let (screen_idx, slice_idx) = (*screen_idx, *slice_idx);
                if let Some(screen) = output_manager.screens.get_mut(screen_idx) {
                    if let Some(slice) = screen.slices.get_mut(slice_idx) {
                        self.main_window.show_slice_properties(ui, slice);
                    }
                }
            }
            TreeSelection::Mask(screen_idx, slice_idx) => {
                let (screen_idx, slice_idx) = (*screen_idx, *slice_idx);
                if let Some(screen) = output_manager.screens.get_mut(screen_idx) {
                    if let Some(slice) = screen.slices.get_mut(slice_idx) {
                        ui.heading("Mask Properties");
                        ui.add_space(8.0);
                        if slice.mask.is_some() {
                            ui.label("Mask is enabled.");
                        } else {
                            ui.label("No mask configured.");
                        }
                    }
                }
            }
        }
    }

    /// Show blend properties for selected screen
    fn show_blend_properties(&mut self, ui: &mut egui::Ui, output_manager: &mut OutputManager) {
        match &self.selection {
            TreeSelection::Screen(screen_idx) => {
                let screen_idx = *screen_idx;
                if let Some(screen) = output_manager.screens.get_mut(screen_idx) {
                    ui.heading(format!("Blend: {}", screen.name));
                    ui.add_space(8.0);
                    self.blend_panel.show(ui, &mut screen.blend_config);
                }
            }
            TreeSelection::Slice(screen_idx, _) | TreeSelection::Mask(screen_idx, _) => {
                // For slices, show parent screen's blend settings
                let screen_idx = *screen_idx;
                if let Some(screen) = output_manager.screens.get_mut(screen_idx) {
                    ui.heading(format!("Blend: {}", screen.name));
                    ui.label(
                        RichText::new("(Blend applies to entire screen)")
                            .size(11.0)
                            .color(Color32::from_gray(120)),
                    );
                    ui.add_space(8.0);
                    self.blend_panel.show(ui, &mut screen.blend_config);
                }
            }
            TreeSelection::None => {
                ui.label("Select a screen to configure edge blending.");
                ui.add_space(16.0);
                ui.label(
                    RichText::new("Edge blending allows seamless projection across multiple displays.")
                        .size(11.0)
                        .color(Color32::from_gray(100)),
                );

                ui.add_space(16.0);
                ui.heading("Quick Actions");
                if ui.button("🔗 Auto-Blend All Screens").clicked() {
                    let count = output_manager.auto_blend();
                    log::info!("Auto-blend configured {} regions", count);
                }
            }
        }
    }

    /// Show warp properties for selected slice
    fn show_warp_properties(&mut self, ui: &mut egui::Ui, output_manager: &mut OutputManager) {
        match &self.selection {
            TreeSelection::Slice(screen_idx, slice_idx) | TreeSelection::Mask(screen_idx, slice_idx) => {
                let (screen_idx, slice_idx) = (*screen_idx, *slice_idx);
                if let Some(screen) = output_manager.screens.get_mut(screen_idx) {
                    if let Some(slice) = screen.slices.get_mut(slice_idx) {
                        ui.heading(format!("Warp: {}", slice.name));
                        ui.add_space(8.0);
                        self.warp_panel.show(ui, &mut slice.warp_mode);
                    }
                }
            }
            TreeSelection::Screen(screen_idx) => {
                let screen_idx = *screen_idx;
                if let Some(screen) = output_manager.screens.get(screen_idx) {
                    ui.label(format!("Screen: {}", screen.name));
                    ui.add_space(8.0);
                    ui.label("Select a slice to configure warping.");
                    ui.add_space(8.0);

                    if screen.slices.is_empty() {
                        ui.label(
                            RichText::new("This screen has no slices.")
                                .color(Color32::from_gray(100)),
                        );
                    } else {
                        ui.label(
                            RichText::new(format!("{} slices available", screen.slices.len()))
                                .size(11.0)
                                .color(Color32::from_gray(100)),
                        );
                    }
                }
            }
            TreeSelection::None => {
                ui.label("Select a slice to configure geometric warping.");
                ui.add_space(16.0);
                ui.label(
                    RichText::new("Warping allows corner correction and perspective adjustment for each slice.")
                        .size(11.0)
                        .color(Color32::from_gray(100)),
                );
            }
        }
    }

    /// Show mask properties for selected slice
    fn show_mask_properties(&mut self, ui: &mut egui::Ui, output_manager: &mut OutputManager) {
        match &self.selection {
            TreeSelection::Slice(screen_idx, slice_idx) | TreeSelection::Mask(screen_idx, slice_idx) => {
                let (screen_idx, slice_idx) = (*screen_idx, *slice_idx);
                if let Some(screen) = output_manager.screens.get_mut(screen_idx) {
                    if let Some(slice) = screen.slices.get_mut(slice_idx) {
                        ui.heading(format!("Mask: {}", slice.name));
                        ui.add_space(8.0);

                        if slice.mask.is_some() {
                            ui.label("Mask is enabled for this slice.");
                            ui.add_space(8.0);

                            if ui.button("Remove Mask").clicked() {
                                slice.mask = None;
                            }

                            ui.add_space(16.0);
                            ui.label(
                                RichText::new("Mask editing coming soon...")
                                    .color(Color32::from_gray(100)),
                            );
                        } else {
                            ui.label("No mask configured.");
                            ui.add_space(8.0);

                            if ui.button("Add Rectangle Mask").clicked() {
                                // Would add mask here
                            }
                            if ui.button("Add Ellipse Mask").clicked() {
                                // Would add mask here
                            }
                            if ui.button("Add Bezier Mask").clicked() {
                                // Would add mask here
                            }
                        }
                    }
                }
            }
            TreeSelection::Screen(_) => {
                ui.label("Select a slice to configure masking.");
                ui.add_space(8.0);
                ui.label(
                    RichText::new("Masks are applied per-slice, not per-screen.")
                        .size(11.0)
                        .color(Color32::from_gray(100)),
                );
            }
            TreeSelection::None => {
                ui.label("Select a slice to configure masking.");
                ui.add_space(16.0);
                ui.label(
                    RichText::new("Masks allow you to hide portions of a slice output.")
                        .size(11.0)
                        .color(Color32::from_gray(100)),
                );
            }
        }
    }

    /// Show bottom bar with action buttons
    fn show_bottom_bar(&mut self, ui: &mut egui::Ui, output_manager: &OutputManager) {
        ui.horizontal(|ui| {
            // Left side info
            match &self.selection {
                TreeSelection::None => {
                    ui.label(RichText::new("No selection").color(Color32::from_gray(100)));
                }
                TreeSelection::Screen(idx) => {
                    ui.label(format!("Screen {}", idx + 1));
                }
                TreeSelection::Slice(s, sl) => {
                    ui.label(format!("Screen {} / Slice {}", s + 1, sl + 1));
                }
                TreeSelection::Mask(s, sl) => {
                    ui.label(format!("Screen {} / Slice {} / Mask", s + 1, sl + 1));
                }
            }

            ui.separator();

            // Live status
            if output_manager.is_live {
                ui.label(
                    RichText::new(format!("🔴 LIVE - {} output(s) active", output_manager.window_manager.active_count()))
                        .size(11.0)
                        .color(Color32::from_rgb(255, 100, 100))
                );
            } else {
                ui.label(
                    RichText::new("⚫ Outputs stopped")
                        .size(11.0)
                        .color(Color32::from_gray(100))
                );
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Save & Close button
                if ui
                    .add(
                        egui::Button::new(RichText::new("Save & Close").size(12.0))
                            .fill(Color32::from_rgb(74, 157, 91))
                            .min_size(Vec2::new(100.0, 28.0)),
                    )
                    .clicked()
                {
                    self.should_save = true;
                    self.is_open = false;
                    log::info!("Saving output configuration...");
                }

                // Cancel button
                if ui
                    .add(
                        egui::Button::new(RichText::new("Cancel").size(12.0))
                            .fill(Color32::from_gray(60))
                            .min_size(Vec2::new(80.0, 28.0)),
                    )
                    .clicked()
                {
                    self.should_save = false;
                    self.is_open = false;
                    log::info!("Discarding output changes");
                }
            });
        });
    }
}
