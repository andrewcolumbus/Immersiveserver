//! Native OS menu bar integration using muda
//!
//! Provides native menus on macOS (in system menu bar) and Windows (attached to window).
//! Falls back to egui menus on Linux.

use muda::{
    accelerator::{Accelerator, Code, Modifiers},
    AboutMetadata, CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu,
};
use std::sync::mpsc::{self, Receiver, Sender};

use super::menu_bar::{FileAction, MenuAction};

/// Activate the application on macOS (make it frontmost and focused)
/// This is needed when we disable winit's default menu, as it skips the normal activation.
#[cfg(target_os = "macos")]
pub fn activate_macos_app() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApp, NSApplicationActivationPolicy};

    // We're on the main thread since this is called from the event loop
    if let Some(mtm) = MainThreadMarker::new() {
        let app = NSApp(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);
    }
}

#[cfg(not(target_os = "macos"))]
pub fn activate_macos_app() {
    // No-op on other platforms
}

/// Make the window key and bring it to front on macOS
/// Call this when the window receives a mouse click to ensure it gets focus
#[cfg(target_os = "macos")]
pub fn focus_window_on_click(window: &winit::window::Window) {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApp;
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    if let Some(mtm) = MainThreadMarker::new() {
        let app = NSApp(mtm);
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);

        // Also make the specific window key
        if let Ok(handle) = window.window_handle() {
            if let RawWindowHandle::AppKit(appkit_handle) = handle.as_raw() {
                use objc2::runtime::AnyObject;
                use objc2_app_kit::NSWindow;

                unsafe {
                    let ns_view = appkit_handle.ns_view.as_ptr() as *mut AnyObject;
                    let ns_view: &AnyObject = &*ns_view;
                    // Get the window from the view
                    let ns_window: *mut NSWindow = objc2::msg_send![ns_view, window];
                    if !ns_window.is_null() {
                        let ns_window: &NSWindow = &*ns_window;
                        ns_window.makeKeyAndOrderFront(None);
                    }
                }
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn focus_window_on_click(_window: &winit::window::Window) {
    // No-op on other platforms
}

/// Menu item IDs for identifying which item was clicked
mod menu_ids {
    pub const OPEN: &str = "file_open";
    pub const SAVE: &str = "file_save";
    pub const SAVE_AS: &str = "file_save_as";
    pub const EXIT: &str = "file_exit";

    pub const PREFERENCES: &str = "app_preferences";

    pub const PANEL_CLIP_GRID: &str = "view_clip_grid";
    pub const PANEL_PROPERTIES: &str = "view_properties";
    pub const PANEL_SOURCES: &str = "view_sources";
    pub const PANEL_EFFECTS_BROWSER: &str = "view_effects_browser";
    pub const PANEL_PREVIEW_MONITOR: &str = "view_preview_monitor";
    pub const PANEL_PERFORMANCE: &str = "view_performance";
    pub const SHOW_FPS: &str = "view_show_fps";

    pub const HAP_CONVERTER: &str = "tools_hap_converter";
}

/// Native menu state and event handling
pub struct NativeMenu {
    /// The root menu (kept alive for the lifetime of the app)
    #[allow(dead_code)]
    menu: Menu,
    /// Receiver for menu events
    event_receiver: Receiver<MenuEvent>,
    /// Check menu items that need state updates
    panel_items: PanelCheckItems,
    /// Show FPS check item
    show_fps_item: CheckMenuItem,
}

/// Check menu items for panel toggles
struct PanelCheckItems {
    clip_grid: CheckMenuItem,
    properties: CheckMenuItem,
    sources: CheckMenuItem,
    effects_browser: CheckMenuItem,
    preview_monitor: CheckMenuItem,
    performance: CheckMenuItem,
}

/// Result of processing menu events
pub enum NativeMenuEvent {
    FileAction(FileAction),
    MenuAction(MenuAction),
    ShowFpsToggled(bool),
    OpenPreferences,
    Exit,
    None,
}

impl NativeMenu {
    /// Create and initialize the native menu bar
    ///
    /// On macOS, this sets up the global app menu.
    /// On Windows, this returns a menu that must be attached to a window.
    pub fn new() -> Self {
        let menu = Menu::new();

        // Create event channel
        let (sender, receiver): (Sender<MenuEvent>, Receiver<MenuEvent>) = mpsc::channel();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let _ = sender.send(event);
        }));

        // === App menu (macOS) / Help menu item (Windows) ===
        #[cfg(target_os = "macos")]
        {
            let app_menu = Submenu::new("Immersive Server", true);
            let _ = app_menu.append(&PredefinedMenuItem::about(
                Some("About Immersive Server"),
                Some(AboutMetadata {
                    name: Some("Immersive Server".to_string()),
                    version: Some(env!("CARGO_PKG_VERSION").to_string()),
                    ..Default::default()
                }),
            ));
            let _ = app_menu.append(&PredefinedMenuItem::separator());
            let preferences_item = MenuItem::with_id(
                menu_ids::PREFERENCES,
                "Preferences...",
                true,
                Some(Accelerator::new(Some(Modifiers::META), Code::Comma)),
            );
            let _ = app_menu.append(&preferences_item);
            let _ = app_menu.append(&PredefinedMenuItem::separator());
            let _ = app_menu.append(&PredefinedMenuItem::quit(Some("Quit Immersive Server")));
            let _ = menu.append(&app_menu);
        }

        // === File menu ===
        let file_menu = Submenu::new("File", true);

        let open_item = MenuItem::with_id(
            menu_ids::OPEN,
            "Open Environment...",
            true,
            Some(Accelerator::new(Some(Modifiers::META), Code::KeyO)),
        );
        let _ = file_menu.append(&open_item);

        let _ = file_menu.append(&PredefinedMenuItem::separator());

        let save_item = MenuItem::with_id(
            menu_ids::SAVE,
            "Save",
            true,
            Some(Accelerator::new(Some(Modifiers::META), Code::KeyS)),
        );
        let _ = file_menu.append(&save_item);

        let save_as_item = MenuItem::with_id(
            menu_ids::SAVE_AS,
            "Save As...",
            true,
            Some(Accelerator::new(
                Some(Modifiers::META | Modifiers::SHIFT),
                Code::KeyS,
            )),
        );
        let _ = file_menu.append(&save_as_item);

        // Exit item (Windows only - macOS uses Quit in app menu)
        #[cfg(target_os = "windows")]
        {
            let _ = file_menu.append(&PredefinedMenuItem::separator());
            let exit_item = MenuItem::with_id(
                menu_ids::EXIT,
                "Exit",
                true,
                Some(Accelerator::new(Some(Modifiers::ALT), Code::F4)),
            );
            let _ = file_menu.append(&exit_item);
        }

        let _ = menu.append(&file_menu);

        // === Edit menu (Windows only - macOS uses app menu for Preferences) ===
        #[cfg(target_os = "windows")]
        {
            let edit_menu = Submenu::new("Edit", true);
            let preferences_item = MenuItem::with_id(
                menu_ids::PREFERENCES,
                "Preferences...",
                true,
                None,
            );
            let _ = edit_menu.append(&preferences_item);
            let _ = menu.append(&edit_menu);
        }

        // === View menu ===
        let view_menu = Submenu::new("View", true);

        let clip_grid_item =
            CheckMenuItem::with_id(menu_ids::PANEL_CLIP_GRID, "Clip Grid Panel", true, true, None);
        let _ = view_menu.append(&clip_grid_item);

        let properties_item =
            CheckMenuItem::with_id(menu_ids::PANEL_PROPERTIES, "Properties Panel", true, true, None);
        let _ = view_menu.append(&properties_item);

        let sources_item =
            CheckMenuItem::with_id(menu_ids::PANEL_SOURCES, "Sources Panel", true, true, None);
        let _ = view_menu.append(&sources_item);

        let effects_browser_item = CheckMenuItem::with_id(
            menu_ids::PANEL_EFFECTS_BROWSER,
            "Effects Browser Panel",
            true,
            true,
            None,
        );
        let _ = view_menu.append(&effects_browser_item);

        let preview_monitor_item = CheckMenuItem::with_id(
            menu_ids::PANEL_PREVIEW_MONITOR,
            "Preview Monitor Panel",
            true,
            true,
            None,
        );
        let _ = view_menu.append(&preview_monitor_item);

        let performance_item = CheckMenuItem::with_id(
            menu_ids::PANEL_PERFORMANCE,
            "Performance Panel",
            true,
            false,
            None,
        );
        let _ = view_menu.append(&performance_item);

        let _ = view_menu.append(&PredefinedMenuItem::separator());

        let show_fps_item =
            CheckMenuItem::with_id(menu_ids::SHOW_FPS, "Show FPS", true, false, None);
        let _ = view_menu.append(&show_fps_item);

        let _ = menu.append(&view_menu);

        // === Tools menu ===
        let tools_menu = Submenu::new("Tools", true);

        let hap_converter_item =
            MenuItem::with_id(menu_ids::HAP_CONVERTER, "HAP Converter...", true, None);
        let _ = tools_menu.append(&hap_converter_item);

        let _ = menu.append(&tools_menu);

        // === Help menu (Windows - includes About) ===
        #[cfg(target_os = "windows")]
        {
            let help_menu = Submenu::new("Help", true);
            let _ = help_menu.append(&PredefinedMenuItem::about(
                Some("About Immersive Server"),
                Some(AboutMetadata {
                    name: Some("Immersive Server".to_string()),
                    version: Some(env!("CARGO_PKG_VERSION").to_string()),
                    ..Default::default()
                }),
            ));
            let _ = menu.append(&help_menu);
        }

        // Initialize menu on macOS (global app menu)
        #[cfg(target_os = "macos")]
        {
            menu.init_for_nsapp();
        }

        Self {
            menu,
            event_receiver: receiver,
            panel_items: PanelCheckItems {
                clip_grid: clip_grid_item,
                properties: properties_item,
                sources: sources_item,
                effects_browser: effects_browser_item,
                preview_monitor: preview_monitor_item,
                performance: performance_item,
            },
            show_fps_item,
        }
    }

    /// Attach the menu to a window (Windows only)
    #[cfg(target_os = "windows")]
    pub fn attach_to_window(&self, window: &winit::window::Window) {
        use muda::MenuKind;
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

        if let Ok(handle) = window.window_handle() {
            if let RawWindowHandle::Win32(win32_handle) = handle.as_raw() {
                let hwnd = win32_handle.hwnd.get();
                unsafe {
                    self.menu.init_for_hwnd(hwnd as *mut _).ok();
                }
            }
        }
    }

    /// No-op on non-Windows platforms
    #[cfg(not(target_os = "windows"))]
    pub fn attach_to_window(&self, _window: &winit::window::Window) {}

    /// Poll for and process any pending menu events
    pub fn poll_events(&self) -> NativeMenuEvent {
        match self.event_receiver.try_recv() {
            Ok(event) => self.handle_event(event),
            Err(_) => NativeMenuEvent::None,
        }
    }

    /// Handle a menu event and return the corresponding action
    fn handle_event(&self, event: MenuEvent) -> NativeMenuEvent {
        let id = event.id().0.as_str();

        match id {
            // File menu
            menu_ids::OPEN => NativeMenuEvent::FileAction(FileAction::Open),
            menu_ids::SAVE => NativeMenuEvent::FileAction(FileAction::Save),
            menu_ids::SAVE_AS => NativeMenuEvent::FileAction(FileAction::SaveAs),
            menu_ids::EXIT => NativeMenuEvent::Exit,

            // App menu (macOS) / Edit menu (Windows)
            menu_ids::PREFERENCES => NativeMenuEvent::OpenPreferences,

            // View menu - panel toggles
            menu_ids::PANEL_CLIP_GRID => NativeMenuEvent::MenuAction(MenuAction::TogglePanel {
                panel_id: "clip_grid".to_string(),
            }),
            menu_ids::PANEL_PROPERTIES => NativeMenuEvent::MenuAction(MenuAction::TogglePanel {
                panel_id: "properties".to_string(),
            }),
            menu_ids::PANEL_SOURCES => NativeMenuEvent::MenuAction(MenuAction::TogglePanel {
                panel_id: "sources".to_string(),
            }),
            menu_ids::PANEL_EFFECTS_BROWSER => {
                NativeMenuEvent::MenuAction(MenuAction::TogglePanel {
                    panel_id: "effects_browser".to_string(),
                })
            }
            menu_ids::PANEL_PREVIEW_MONITOR => {
                NativeMenuEvent::MenuAction(MenuAction::TogglePanel {
                    panel_id: "preview_monitor".to_string(),
                })
            }
            menu_ids::PANEL_PERFORMANCE => NativeMenuEvent::MenuAction(MenuAction::TogglePanel {
                panel_id: "performance".to_string(),
            }),
            menu_ids::SHOW_FPS => {
                // Get the new checked state from the menu item
                let checked = self.show_fps_item.is_checked();
                NativeMenuEvent::ShowFpsToggled(checked)
            }

            // Tools menu
            menu_ids::HAP_CONVERTER => NativeMenuEvent::MenuAction(MenuAction::OpenHAPConverter),

            _ => NativeMenuEvent::None,
        }
    }

    /// Update panel check states to match current UI state
    pub fn update_panel_states(&self, panel_states: &[(&str, &str, bool)]) {
        for (panel_id, _, is_open) in panel_states {
            match *panel_id {
                "clip_grid" => self.panel_items.clip_grid.set_checked(*is_open),
                "properties" => self.panel_items.properties.set_checked(*is_open),
                "sources" => self.panel_items.sources.set_checked(*is_open),
                "effects_browser" => self.panel_items.effects_browser.set_checked(*is_open),
                "preview_monitor" => self.panel_items.preview_monitor.set_checked(*is_open),
                "performance" => self.panel_items.performance.set_checked(*is_open),
                _ => {}
            }
        }
    }

    /// Update the show FPS check state
    pub fn update_show_fps(&self, show_fps: bool) {
        self.show_fps_item.set_checked(show_fps);
    }
}

/// Returns true if native menus are supported on this platform
pub fn is_native_menu_supported() -> bool {
    cfg!(any(target_os = "macos", target_os = "windows"))
}
