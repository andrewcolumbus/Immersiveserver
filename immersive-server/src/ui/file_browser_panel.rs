//! File Browser Panel
//!
//! In-app file browser for browsing and dragging video files to the clip grid.
//! Replaces OS file drag-drop (which doesn't work reliably with egui).

use std::path::PathBuf;

use super::DraggableSource;

/// Supported video file extensions
const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mov", "avi", "mkv", "webm", "m4v", "hap"];

/// Supported image file extensions
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "bmp", "tiff", "tif", "webp"];

/// File type for entries
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Directory,
    Video,
    Image,
}

/// Filter mode for the file browser
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileFilter {
    /// Show only video files
    #[default]
    VideosOnly,
    /// Show only image files
    ImagesOnly,
    /// Show both videos and images
    All,
}

/// Actions that can be returned from the file browser panel
#[derive(Debug, Clone)]
pub enum FileBrowserAction {
    // Currently none needed - drag-drop handles file assignment
}

/// A file or directory entry
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub name: String,
    pub file_type: FileType,
}

/// State for the file browser panel
pub struct FileBrowserPanel {
    /// Whether the panel is open
    pub open: bool,
    /// Current directory path
    current_path: PathBuf,
    /// Cached directory entries
    entries: Vec<FileEntry>,
    /// Error message if directory couldn't be read
    error_message: Option<String>,
    /// Path to navigate to (set by double-click, processed next frame)
    pending_navigation: Option<PathBuf>,
    /// Current file filter
    filter: FileFilter,
}

impl Default for FileBrowserPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl FileBrowserPanel {
    /// Create a new file browser panel starting at the user's home directory
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        let mut panel = Self {
            open: true,
            current_path: home,
            entries: Vec::new(),
            error_message: None,
            pending_navigation: None,
            filter: FileFilter::default(),
        };
        panel.refresh_entries();
        panel
    }

    /// Refresh the directory listing
    fn refresh_entries(&mut self) {
        self.entries.clear();
        self.error_message = None;

        match std::fs::read_dir(&self.current_path) {
            Ok(read_dir) => {
                for entry in read_dir.flatten() {
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().to_string();

                    // Skip hidden files
                    if name.starts_with('.') {
                        continue;
                    }

                    // Determine file type
                    let file_type = if path.is_dir() {
                        FileType::Directory
                    } else if Self::is_video_file(&path) {
                        FileType::Video
                    } else if Self::is_image_file(&path) {
                        FileType::Image
                    } else {
                        continue; // Skip unsupported files
                    };

                    // Apply filter
                    let show = match file_type {
                        FileType::Directory => self.directory_has_compatible_files(&path),
                        FileType::Video => matches!(self.filter, FileFilter::VideosOnly | FileFilter::All),
                        FileType::Image => matches!(self.filter, FileFilter::ImagesOnly | FileFilter::All),
                    };

                    if show {
                        self.entries.push(FileEntry { path, name, file_type });
                    }
                }

                // Sort: directories first, then alphabetically by name
                self.entries.sort_by(|a, b| {
                    let a_is_dir = a.file_type == FileType::Directory;
                    let b_is_dir = b.file_type == FileType::Directory;
                    match (a_is_dir, b_is_dir) {
                        (true, false) => std::cmp::Ordering::Less,
                        (false, true) => std::cmp::Ordering::Greater,
                        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                    }
                });
            }
            Err(e) => {
                self.error_message = Some(format!("Error: {}", e));
            }
        }
    }

    /// Check if a directory contains compatible files based on current filter
    fn directory_has_compatible_files(&self, dir_path: &PathBuf) -> bool {
        let Ok(read_dir) = std::fs::read_dir(dir_path) else {
            return false;
        };

        for entry in read_dir.flatten() {
            let path = entry.path();

            // Check subdirectories recursively
            if path.is_dir() {
                // Skip hidden directories
                if entry.file_name().to_string_lossy().starts_with('.') {
                    continue;
                }
                if self.directory_has_compatible_files(&path) {
                    return true;
                }
            } else {
                // Check if file matches current filter
                let matches = match self.filter {
                    FileFilter::VideosOnly => Self::is_video_file(&path),
                    FileFilter::ImagesOnly => Self::is_image_file(&path),
                    FileFilter::All => Self::is_video_file(&path) || Self::is_image_file(&path),
                };
                if matches {
                    return true;
                }
            }
        }
        false
    }

    /// Check if a file is a supported video file
    fn is_video_file(path: &PathBuf) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| VIDEO_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
            .unwrap_or(false)
    }

    /// Check if a file is a supported image file
    fn is_image_file(path: &PathBuf) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| IMAGE_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
            .unwrap_or(false)
    }

    /// Navigate to a new directory
    fn navigate_to(&mut self, path: PathBuf) {
        self.current_path = path;
        self.refresh_entries();
    }

    /// Render the file browser panel contents
    pub fn render_contents(&mut self, ui: &mut egui::Ui) -> Vec<FileBrowserAction> {
        let actions = Vec::new();

        // Process any pending navigation from previous frame
        if let Some(path) = self.pending_navigation.take() {
            self.navigate_to(path);
        }

        // Navigation bar
        ui.horizontal(|ui| {
            // Up button
            if ui.button("⬆").on_hover_text("Go up").clicked() {
                if let Some(parent) = self.current_path.parent() {
                    self.navigate_to(parent.to_path_buf());
                }
            }
            // Home button
            if ui.button("🏠").on_hover_text("Home").clicked() {
                if let Some(home) = dirs::home_dir() {
                    self.navigate_to(home);
                }
            }
            // Refresh button
            if ui.button("🔄").on_hover_text("Refresh").clicked() {
                self.refresh_entries();
            }
        });

        // Filter toggle
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Show:").size(11.0).color(egui::Color32::GRAY));
            let old_filter = self.filter;
            ui.selectable_value(&mut self.filter, FileFilter::VideosOnly, "🎬 Videos");
            ui.selectable_value(&mut self.filter, FileFilter::ImagesOnly, "🖼 Images");
            ui.selectable_value(&mut self.filter, FileFilter::All, "All");
            // Refresh if filter changed
            if self.filter != old_filter {
                self.refresh_entries();
            }
        });

        // Current path display
        ui.label(
            egui::RichText::new(self.current_path.to_string_lossy())
                .size(10.0)
                .color(egui::Color32::GRAY),
        );
        ui.separator();

        // Error message
        if let Some(err) = &self.error_message {
            ui.label(egui::RichText::new(err).color(egui::Color32::RED));
        }

        // File list
        egui::ScrollArea::vertical().show(ui, |ui| {
            let entries = self.entries.clone();
            for entry in entries {
                self.render_entry(ui, &entry);
            }
        });

        actions
    }

    /// Render a single file/directory entry
    fn render_entry(&mut self, ui: &mut egui::Ui, entry: &FileEntry) {
        let icon = match entry.file_type {
            FileType::Directory => "📁",
            FileType::Video => "🎬",
            FileType::Image => "🖼",
        };
        let id = egui::Id::new(&entry.path);

        let frame = egui::Frame::new()
            .fill(egui::Color32::from_rgb(45, 45, 55))
            .corner_radius(4.0)
            .inner_margin(egui::Margin::symmetric(8, 4));

        let response = frame
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(icon);
                    ui.label(&entry.name);
                });
            })
            .response;

        let response = ui.interact(response.rect, id, egui::Sense::click_and_drag());

        // Double-click to navigate into directory
        if entry.file_type == FileType::Directory && response.double_clicked() {
            self.pending_navigation = Some(entry.path.clone());
        }

        // Drag files (videos and images)
        if entry.file_type != FileType::Directory && response.dragged() {
            let source = DraggableSource::File {
                path: entry.path.clone(),
                name: entry.name.clone(),
            };
            egui::DragAndDrop::set_payload(ui.ctx(), source);

            // Draw drag ghost
            if let Some(pos) = ui.ctx().pointer_latest_pos() {
                egui::Area::new(egui::Id::new("file_drag_ghost"))
                    .fixed_pos(pos + egui::vec2(10.0, 10.0))
                    .order(egui::Order::Tooltip)
                    .show(ui.ctx(), |ui| {
                        egui::Frame::popup(ui.style())
                            .fill(egui::Color32::from_rgba_unmultiplied(60, 60, 80, 220))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(icon);
                                    ui.label(&entry.name);
                                });
                            });
                    });
            }
        }

        response.on_hover_text(entry.path.to_string_lossy());
    }
}
