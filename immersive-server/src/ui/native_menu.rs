//! Native OS menu bar integration using muda
//!
//! Provides native menus on macOS (in system menu bar) and Windows (attached to window).
//! Falls back to egui menus on Linux.
//!
//! This module builds menus from the shared MenuBarDefinition in menu_definition.rs,
//! ensuring consistency between native and egui menus.

use muda::{AboutMetadata, CheckMenuItem, Menu, MenuEvent, MenuItem as MudaMenuItem, PredefinedMenuItem, Submenu};
use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};

use super::menu_bar::{FileAction, MenuAction};
use super::menu_definition::{
    MenuBarDefinition, MenuDefinition, MenuItem, MenuItemAction, MenuItemId, PredefinedItem, SettingId,
};
use crate::settings::EnvironmentSettings;

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

/// Native menu state and event handling
pub struct NativeMenu {
    /// The root menu (kept alive for the lifetime of the app)
    #[allow(dead_code)]
    menu: Menu,
    /// Receiver for menu events
    event_receiver: Receiver<MenuEvent>,
    /// Map from MenuItemId to CheckMenuItem for state updates
    check_items: HashMap<MenuItemId, CheckMenuItem>,
}

impl Default for NativeMenu {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of processing menu events
pub enum NativeMenuEvent {
    FileAction(FileAction),
    MenuAction(MenuAction),
    ShowFpsToggled(bool),
    ShowBpmToggled(bool),
    OpenPreferences,
    Exit,
    None,
}

impl NativeMenu {
    /// Create and initialize the native menu bar from the shared definition
    ///
    /// On macOS, this sets up the global app menu.
    /// On Windows, this returns a menu that must be attached to a window.
    pub fn new() -> Self {
        let definition = MenuBarDefinition::build();
        let menu = Menu::new();
        let mut check_items = HashMap::new();

        // Create event channel
        let (sender, receiver): (Sender<MenuEvent>, Receiver<MenuEvent>) = mpsc::channel();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let _ = sender.send(event);
        }));

        // Build app menu (macOS only)
        if let Some(app_def) = &definition.app_menu {
            let app_menu = Self::build_submenu(app_def, &mut check_items);
            let _ = menu.append(&app_menu);
        }

        // Build standard menus
        for menu_def in &definition.menus {
            // Skip empty menus (e.g., Edit menu on macOS)
            if !menu_def.items.is_empty() {
                let submenu = Self::build_submenu(menu_def, &mut check_items);
                let _ = menu.append(&submenu);
            }
        }

        // Build help menu (Windows only)
        if let Some(help_def) = &definition.help_menu {
            let help_menu = Self::build_submenu(help_def, &mut check_items);
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
            check_items,
        }
    }

    /// Build a submenu from a MenuDefinition
    fn build_submenu(def: &MenuDefinition, check_items: &mut HashMap<MenuItemId, CheckMenuItem>) -> Submenu {
        let submenu = Submenu::new(&def.label, true);

        for item in &def.items {
            Self::append_item(&submenu, item, check_items);
        }

        submenu
    }

    /// Append a menu item to a submenu
    fn append_item(parent: &Submenu, item: &MenuItem, check_items: &mut HashMap<MenuItemId, CheckMenuItem>) {
        match item {
            MenuItem::Action {
                id,
                label,
                shortcut,
                enabled,
            } => {
                let accelerator = shortcut.as_ref().map(|s| s.to_accelerator());
                let menu_item = MudaMenuItem::with_id(id.to_string_id(), label, *enabled, accelerator);
                let _ = parent.append(&menu_item);
            }

            MenuItem::Check {
                id,
                label,
                shortcut,
                default_checked,
            } => {
                let accelerator = shortcut.as_ref().map(|s| s.to_accelerator());
                let check_item = CheckMenuItem::with_id(id.to_string_id(), label, true, *default_checked, accelerator);
                check_items.insert(id.clone(), check_item.clone());
                let _ = parent.append(&check_item);
            }

            MenuItem::Separator => {
                let _ = parent.append(&PredefinedMenuItem::separator());
            }

            MenuItem::Submenu { label, items } => {
                let sub = Submenu::new(label, true);
                for sub_item in items {
                    Self::append_item(&sub, sub_item, check_items);
                }
                let _ = parent.append(&sub);
            }

            MenuItem::Label { text } => {
                // Native menus don't support non-clickable labels well
                // Use a disabled menu item instead
                let item = MudaMenuItem::new(text, false, None);
                let _ = parent.append(&item);
            }

            MenuItem::Predefined(predef) => match predef {
                PredefinedItem::About => {
                    let _ = parent.append(&PredefinedMenuItem::about(
                        Some("About Immersive Server"),
                        Some(AboutMetadata {
                            name: Some("Immersive Server".into()),
                            version: Some(env!("CARGO_PKG_VERSION").into()),
                            ..Default::default()
                        }),
                    ));
                }
                PredefinedItem::Quit => {
                    let _ = parent.append(&PredefinedMenuItem::quit(Some("Quit Immersive Server")));
                }
            },
        }
    }

    /// Attach the menu to a window (Windows only)
    #[cfg(target_os = "windows")]
    pub fn attach_to_window(&self, window: &winit::window::Window) {
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
        let id_str = event.id().0.as_str();

        // Try to parse the ID using our shared definition
        if let Some(id) = MenuItemId::from_string_id(id_str) {
            match id.to_action() {
                MenuItemAction::File(file_action) => NativeMenuEvent::FileAction(file_action),
                MenuItemAction::Menu(menu_action) => {
                    // Special case for preferences
                    if matches!(menu_action, MenuAction::OpenPreferences) {
                        NativeMenuEvent::OpenPreferences
                    } else {
                        NativeMenuEvent::MenuAction(menu_action)
                    }
                }
                MenuItemAction::ToggleSetting(setting) => {
                    // Get the new checked state from the menu item
                    match setting {
                        SettingId::ShowFps => {
                            if let Some(check_item) = self.check_items.get(&MenuItemId::ShowFps) {
                                NativeMenuEvent::ShowFpsToggled(check_item.is_checked())
                            } else {
                                NativeMenuEvent::None
                            }
                        }
                        SettingId::ShowBpm => {
                            if let Some(check_item) = self.check_items.get(&MenuItemId::ShowBpm) {
                                NativeMenuEvent::ShowBpmToggled(check_item.is_checked())
                            } else {
                                NativeMenuEvent::None
                            }
                        }
                    }
                }
                MenuItemAction::Exit => NativeMenuEvent::Exit,
                MenuItemAction::None => NativeMenuEvent::None,
            }
        } else {
            NativeMenuEvent::None
        }
    }

    /// Update panel check states to match current UI state
    pub fn update_panel_states(&self, panel_states: &[(&str, &str, bool)]) {
        for (panel_id, _, is_open) in panel_states {
            // Find the corresponding MenuItemId for this panel
            let menu_item_id = match *panel_id {
                "clip_grid" => Some(MenuItemId::PanelClipGrid),
                "properties" => Some(MenuItemId::PanelProperties),
                "sources" => Some(MenuItemId::PanelSources),
                "effects_browser" => Some(MenuItemId::PanelEffectsBrowser),
                "preview_monitor" => Some(MenuItemId::PanelPreviewMonitor),
                "performance" => Some(MenuItemId::PanelPerformance),
                "previs" => Some(MenuItemId::PanelPrevis),
                _ => None,
            };

            if let Some(id) = menu_item_id {
                if let Some(check_item) = self.check_items.get(&id) {
                    check_item.set_checked(*is_open);
                }
            }
        }
    }

    /// Update the show FPS check state
    pub fn update_show_fps(&self, show_fps: bool) {
        if let Some(check_item) = self.check_items.get(&MenuItemId::ShowFps) {
            check_item.set_checked(show_fps);
        }
    }

    /// Update the show BPM check state
    pub fn update_show_bpm(&self, show_bpm: bool) {
        if let Some(check_item) = self.check_items.get(&MenuItemId::ShowBpm) {
            check_item.set_checked(show_bpm);
        }
    }

    /// Sync all check states from settings and panel states
    pub fn sync_state(&self, panel_states: &[(&str, &str, bool)], settings: &EnvironmentSettings) {
        self.update_panel_states(panel_states);
        self.update_show_fps(settings.show_fps);
        self.update_show_bpm(settings.show_bpm);
    }
}

/// Returns true if native menus are supported on this platform
pub fn is_native_menu_supported() -> bool {
    cfg!(any(target_os = "macos", target_os = "windows"))
}
