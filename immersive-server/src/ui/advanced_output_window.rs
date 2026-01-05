//! Advanced Output Window
//!
//! A modal window for configuring multi-screen outputs with slice-based input selection.
//! Accessible via View → Advanced Output.

use crate::output::{OutputDevice, OutputManager, Screen, ScreenId, Slice, SliceId, SliceInput};

/// Actions returned from the Advanced Output window
#[derive(Debug, Clone)]
pub enum AdvancedOutputAction {
    /// Add a new screen
    AddScreen,
    /// Remove a screen
    RemoveScreen { screen_id: ScreenId },
    /// Add a slice to a screen
    AddSlice { screen_id: ScreenId },
    /// Remove a slice from a screen
    RemoveSlice {
        screen_id: ScreenId,
        slice_id: SliceId,
    },
    /// Update slice properties
    UpdateSlice {
        screen_id: ScreenId,
        slice_id: SliceId,
        slice: Slice,
    },
    /// Update screen properties
    UpdateScreen { screen_id: ScreenId, screen: Screen },
}

/// Advanced Output window for configuring multi-screen outputs
pub struct AdvancedOutputWindow {
    /// Whether the window is open
    pub open: bool,
    /// Currently selected screen
    selected_screen: Option<ScreenId>,
    /// Currently selected slice within selected screen
    selected_slice: Option<SliceId>,
    /// Temporary screen name for editing
    temp_screen_name: String,
    /// Temporary slice name for editing
    temp_slice_name: String,
    /// Temporary resolution strings
    temp_width: String,
    temp_height: String,
}

impl Default for AdvancedOutputWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl AdvancedOutputWindow {
    /// Create a new Advanced Output window (closed by default)
    pub fn new() -> Self {
        Self {
            open: false,
            selected_screen: None,
            selected_slice: None,
            temp_screen_name: String::new(),
            temp_slice_name: String::new(),
            temp_width: String::new(),
            temp_height: String::new(),
        }
    }

    /// Toggle the window open/closed
    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    /// Render the Advanced Output window
    ///
    /// Returns a list of actions to be processed by the app.
    pub fn render(
        &mut self,
        ctx: &egui::Context,
        output_manager: Option<&OutputManager>,
        layer_count: usize,
    ) -> Vec<AdvancedOutputAction> {
        let mut actions = Vec::new();

        if !self.open {
            return actions;
        }

        let mut open = self.open;
        egui::Window::new("Advanced Output")
            .id(egui::Id::new("advanced_output_window"))
            .open(&mut open)
            .default_size([700.0, 500.0])
            .min_width(500.0)
            .min_height(350.0)
            .resizable(true)
            .collapsible(true)
            .show(ctx, |ui| {
                self.render_contents(ui, output_manager, layer_count, &mut actions);
            });
        self.open = open;

        actions
    }

    /// Render the window contents
    fn render_contents(
        &mut self,
        ui: &mut egui::Ui,
        output_manager: Option<&OutputManager>,
        layer_count: usize,
        actions: &mut Vec<AdvancedOutputAction>,
    ) {
        // Get screens from output manager
        let screens: Vec<&Screen> = output_manager
            .map(|m| m.screens().collect())
            .unwrap_or_default();

        // Validate selected screen still exists
        if let Some(screen_id) = self.selected_screen {
            if !screens.iter().any(|s| s.id == screen_id) {
                self.selected_screen = None;
                self.selected_slice = None;
            }
        }

        // Three-column layout: Screens | Preview | Properties
        ui.horizontal(|ui| {
            // LEFT COLUMN: Screens and Slices list
            ui.vertical(|ui| {
                ui.set_min_width(150.0);
                ui.set_max_width(180.0);

                // Screens section
                ui.heading("Screens");
                ui.add_space(4.0);

                egui::ScrollArea::vertical()
                    .id_salt("screens_list")
                    .max_height(150.0)
                    .show(ui, |ui| {
                        for screen in &screens {
                            let is_selected = self.selected_screen == Some(screen.id);
                            let response = ui.selectable_label(
                                is_selected,
                                format!(
                                    "{} ({}x{})",
                                    screen.name, screen.width, screen.height
                                ),
                            );
                            if response.clicked() {
                                self.selected_screen = Some(screen.id);
                                self.selected_slice = None;
                                self.temp_screen_name = screen.name.clone();
                                self.temp_width = screen.width.to_string();
                                self.temp_height = screen.height.to_string();
                            }
                        }
                    });

                ui.horizontal(|ui| {
                    if ui.small_button("+").clicked() {
                        actions.push(AdvancedOutputAction::AddScreen);
                    }
                    if ui
                        .add_enabled(
                            self.selected_screen.is_some(),
                            egui::Button::new("-").small(),
                        )
                        .clicked()
                    {
                        if let Some(screen_id) = self.selected_screen {
                            actions.push(AdvancedOutputAction::RemoveScreen { screen_id });
                            self.selected_screen = None;
                            self.selected_slice = None;
                        }
                    }
                });

                ui.add_space(16.0);
                ui.separator();
                ui.add_space(8.0);

                // Slices section (for selected screen)
                ui.heading("Slices");
                ui.add_space(4.0);

                if let Some(screen_id) = self.selected_screen {
                    if let Some(screen) = screens.iter().find(|s| s.id == screen_id) {
                        egui::ScrollArea::vertical()
                            .id_salt("slices_list")
                            .max_height(150.0)
                            .show(ui, |ui| {
                                for slice in &screen.slices {
                                    let is_selected = self.selected_slice == Some(slice.id);
                                    let input_label = match &slice.input {
                                        SliceInput::Composition => "Comp".to_string(),
                                        SliceInput::Layer { layer_id } => {
                                            format!("L{}", layer_id)
                                        }
                                    };
                                    let response = ui.selectable_label(
                                        is_selected,
                                        format!("{} [{}]", slice.name, input_label),
                                    );
                                    if response.clicked() {
                                        self.selected_slice = Some(slice.id);
                                        self.temp_slice_name = slice.name.clone();
                                    }
                                }
                            });

                        ui.horizontal(|ui| {
                            if ui.small_button("+").clicked() {
                                actions.push(AdvancedOutputAction::AddSlice { screen_id });
                            }
                            if ui
                                .add_enabled(
                                    self.selected_slice.is_some(),
                                    egui::Button::new("-").small(),
                                )
                                .clicked()
                            {
                                if let Some(slice_id) = self.selected_slice {
                                    actions.push(AdvancedOutputAction::RemoveSlice {
                                        screen_id,
                                        slice_id,
                                    });
                                    self.selected_slice = None;
                                }
                            }
                        });
                    }
                } else {
                    ui.label(
                        egui::RichText::new("Select a screen")
                            .weak()
                            .italics(),
                    );
                }
            });

            ui.separator();

            // MIDDLE COLUMN: Preview
            ui.vertical(|ui| {
                ui.set_min_width(200.0);
                ui.heading("Preview");
                ui.add_space(4.0);

                // Preview placeholder
                let preview_size = egui::vec2(200.0, 150.0);
                let (rect, _response) =
                    ui.allocate_exact_size(preview_size, egui::Sense::hover());

                // Draw preview background
                ui.painter().rect_filled(
                    rect,
                    4.0,
                    egui::Color32::from_rgb(30, 30, 30),
                );

                if let Some(screen_id) = self.selected_screen {
                    if let Some(screen) = screens.iter().find(|s| s.id == screen_id) {
                        // Draw screen info
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            format!("{}x{}", screen.width, screen.height),
                            egui::FontId::proportional(14.0),
                            egui::Color32::GRAY,
                        );

                        // Draw slice rectangles
                        for slice in &screen.slices {
                            if slice.enabled {
                                let slice_rect = egui::Rect::from_min_size(
                                    rect.min
                                        + egui::vec2(
                                            slice.output.rect.x * preview_size.x,
                                            slice.output.rect.y * preview_size.y,
                                        ),
                                    egui::vec2(
                                        slice.output.rect.width * preview_size.x,
                                        slice.output.rect.height * preview_size.y,
                                    ),
                                );

                                let is_selected = self.selected_slice == Some(slice.id);
                                let stroke_color = if is_selected {
                                    egui::Color32::from_rgb(100, 149, 237) // Cornflower blue
                                } else {
                                    egui::Color32::from_rgb(80, 80, 80)
                                };

                                ui.painter().rect_stroke(
                                    slice_rect,
                                    0.0,
                                    egui::Stroke::new(
                                        if is_selected { 2.0 } else { 1.0 },
                                        stroke_color,
                                    ),
                                    egui::StrokeKind::Inside,
                                );
                            }
                        }
                    }
                } else {
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "No screen selected",
                        egui::FontId::proportional(12.0),
                        egui::Color32::DARK_GRAY,
                    );
                }

                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Live preview coming in future update")
                        .small()
                        .weak(),
                );
            });

            ui.separator();

            // RIGHT COLUMN: Properties
            ui.vertical(|ui| {
                ui.set_min_width(200.0);
                ui.heading("Properties");
                ui.add_space(4.0);

                egui::ScrollArea::vertical()
                    .id_salt("properties_scroll")
                    .show(ui, |ui| {
                        if let Some(screen_id) = self.selected_screen {
                            if let Some(screen) = screens.iter().find(|s| s.id == screen_id) {
                                // Show slice properties if a slice is selected
                                if let Some(slice_id) = self.selected_slice {
                                    if let Some(slice) =
                                        screen.slices.iter().find(|s| s.id == slice_id)
                                    {
                                        self.render_slice_properties(
                                            ui,
                                            screen_id,
                                            slice,
                                            layer_count,
                                            actions,
                                        );
                                    }
                                } else {
                                    // Show screen properties
                                    self.render_screen_properties(ui, screen, actions);
                                }
                            }
                        } else {
                            ui.label(
                                egui::RichText::new("Select a screen or slice to edit")
                                    .weak()
                                    .italics(),
                            );
                        }
                    });
            });
        });
    }

    /// Render screen properties panel
    fn render_screen_properties(
        &mut self,
        ui: &mut egui::Ui,
        screen: &Screen,
        actions: &mut Vec<AdvancedOutputAction>,
    ) {
        let mut changed = false;
        let mut screen_copy = screen.clone();

        ui.label(egui::RichText::new("Screen Properties").strong());
        ui.add_space(8.0);

        // Name
        ui.horizontal(|ui| {
            ui.label("Name:");
            if ui
                .text_edit_singleline(&mut self.temp_screen_name)
                .changed()
            {
                screen_copy.name = self.temp_screen_name.clone();
                changed = true;
            }
        });

        ui.add_space(4.0);

        // Resolution
        ui.label("Resolution:");
        ui.horizontal(|ui| {
            ui.label("W:");
            let width_response = ui.add(
                egui::TextEdit::singleline(&mut self.temp_width)
                    .desired_width(50.0),
            );
            ui.label("H:");
            let height_response = ui.add(
                egui::TextEdit::singleline(&mut self.temp_height)
                    .desired_width(50.0),
            );

            if width_response.lost_focus() || height_response.lost_focus() {
                if let (Ok(w), Ok(h)) =
                    (self.temp_width.parse::<u32>(), self.temp_height.parse::<u32>())
                {
                    if w > 0 && h > 0 && (w != screen.width || h != screen.height) {
                        screen_copy.width = w;
                        screen_copy.height = h;
                        changed = true;
                    }
                }
            }
        });

        ui.add_space(8.0);

        // Output device
        ui.horizontal(|ui| {
            ui.label("Output:");
            let device_text = match screen.device {
                OutputDevice::Virtual => "Virtual (Preview)",
                OutputDevice::Display { .. } => "Display",
                OutputDevice::Ndi { .. } => "NDI",
                OutputDevice::Omt { .. } => "OMT",
                #[cfg(target_os = "macos")]
                OutputDevice::Syphon { .. } => "Syphon",
                #[cfg(target_os = "windows")]
                OutputDevice::Spout { .. } => "Spout",
            };
            ui.label(egui::RichText::new(device_text).weak());
        });

        ui.add_space(4.0);

        // Enabled toggle
        let mut enabled = screen_copy.enabled;
        if ui.checkbox(&mut enabled, "Enabled").changed() {
            screen_copy.enabled = enabled;
            changed = true;
        }

        if changed {
            actions.push(AdvancedOutputAction::UpdateScreen {
                screen_id: screen.id,
                screen: screen_copy,
            });
        }
    }

    /// Render slice properties panel
    fn render_slice_properties(
        &mut self,
        ui: &mut egui::Ui,
        screen_id: ScreenId,
        slice: &Slice,
        layer_count: usize,
        actions: &mut Vec<AdvancedOutputAction>,
    ) {
        let mut changed = false;
        let mut slice_copy = slice.clone();

        ui.label(egui::RichText::new("Slice Properties").strong());
        ui.add_space(8.0);

        // Name
        ui.horizontal(|ui| {
            ui.label("Name:");
            if ui
                .text_edit_singleline(&mut self.temp_slice_name)
                .changed()
            {
                slice_copy.name = self.temp_slice_name.clone();
                changed = true;
            }
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // Input source
        ui.label("Input Source");
        ui.add_space(4.0);

        let current_input = match &slice.input {
            SliceInput::Composition => 0,
            SliceInput::Layer { layer_id } => *layer_id as usize + 1,
        };

        egui::ComboBox::from_id_salt("slice_input")
            .selected_text(match &slice.input {
                SliceInput::Composition => "Composition".to_string(),
                SliceInput::Layer { layer_id } => format!("Layer {}", layer_id),
            })
            .show_ui(ui, |ui| {
                if ui
                    .selectable_value(&mut slice_copy.input, SliceInput::Composition, "Composition")
                    .clicked()
                {
                    changed = true;
                }
                for i in 0..layer_count {
                    let layer_input = SliceInput::Layer { layer_id: i as u32 };
                    if ui
                        .selectable_label(current_input == i + 1, format!("Layer {}", i))
                        .clicked()
                    {
                        slice_copy.input = layer_input;
                        changed = true;
                    }
                }
            });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // Input rect (crop from source)
        ui.label("Input Rect (crop)");
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("X:");
            let mut x = slice_copy.input_rect.x;
            if ui
                .add(egui::DragValue::new(&mut x).range(0.0..=1.0).speed(0.01))
                .changed()
            {
                slice_copy.input_rect.x = x;
                changed = true;
            }
            ui.label("Y:");
            let mut y = slice_copy.input_rect.y;
            if ui
                .add(egui::DragValue::new(&mut y).range(0.0..=1.0).speed(0.01))
                .changed()
            {
                slice_copy.input_rect.y = y;
                changed = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("W:");
            let mut w = slice_copy.input_rect.width;
            if ui
                .add(egui::DragValue::new(&mut w).range(0.0..=1.0).speed(0.01))
                .changed()
            {
                slice_copy.input_rect.width = w;
                changed = true;
            }
            ui.label("H:");
            let mut h = slice_copy.input_rect.height;
            if ui
                .add(egui::DragValue::new(&mut h).range(0.0..=1.0).speed(0.01))
                .changed()
            {
                slice_copy.input_rect.height = h;
                changed = true;
            }
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // Output rect (position on screen)
        ui.label("Output Rect (position)");
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("X:");
            let mut x = slice_copy.output.rect.x;
            if ui
                .add(egui::DragValue::new(&mut x).range(0.0..=1.0).speed(0.01))
                .changed()
            {
                slice_copy.output.rect.x = x;
                changed = true;
            }
            ui.label("Y:");
            let mut y = slice_copy.output.rect.y;
            if ui
                .add(egui::DragValue::new(&mut y).range(0.0..=1.0).speed(0.01))
                .changed()
            {
                slice_copy.output.rect.y = y;
                changed = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("W:");
            let mut w = slice_copy.output.rect.width;
            if ui
                .add(egui::DragValue::new(&mut w).range(0.0..=1.0).speed(0.01))
                .changed()
            {
                slice_copy.output.rect.width = w;
                changed = true;
            }
            ui.label("H:");
            let mut h = slice_copy.output.rect.height;
            if ui
                .add(egui::DragValue::new(&mut h).range(0.0..=1.0).speed(0.01))
                .changed()
            {
                slice_copy.output.rect.height = h;
                changed = true;
            }
        });

        ui.add_space(8.0);

        // Rotation
        ui.horizontal(|ui| {
            ui.label("Rotation:");
            let mut rotation_deg = slice_copy.output.rotation.to_degrees();
            if ui
                .add(
                    egui::DragValue::new(&mut rotation_deg)
                        .range(-180.0..=180.0)
                        .speed(1.0)
                        .suffix("°"),
                )
                .changed()
            {
                slice_copy.output.rotation = rotation_deg.to_radians();
                changed = true;
            }
        });

        ui.add_space(4.0);

        // Flip toggles
        ui.horizontal(|ui| {
            let mut flip_h = slice_copy.output.flip_h;
            if ui.checkbox(&mut flip_h, "Flip H").changed() {
                slice_copy.output.flip_h = flip_h;
                changed = true;
            }
            let mut flip_v = slice_copy.output.flip_v;
            if ui.checkbox(&mut flip_v, "Flip V").changed() {
                slice_copy.output.flip_v = flip_v;
                changed = true;
            }
        });

        ui.add_space(8.0);

        // Enabled toggle
        let mut enabled = slice_copy.enabled;
        if ui.checkbox(&mut enabled, "Enabled").changed() {
            slice_copy.enabled = enabled;
            changed = true;
        }

        if changed {
            actions.push(AdvancedOutputAction::UpdateSlice {
                screen_id,
                slice_id: slice.id,
                slice: slice_copy,
            });
        }
    }
}
