# Menu System Architecture

This document describes the declarative menu system used in Immersive Server, which generates both native OS menus and egui menus from a single shared definition.

## Overview

The menu system uses a declarative approach where the menu structure is defined once in `menu_definition.rs`, then rendered to both:
- **Native menus** via the `muda` crate (macOS system menu bar, Windows window menu)
- **egui menus** for the in-app menu bar

```
MenuBarDefinition::build()
         │
    ┌────┴────┐
    ▼         ▼
NativeMenu   MenuBar
 (muda)      (egui)
```

## Native vs egui Boundaries

### Native Menus (LEFT side)

Native menus handle standard menu items that can be represented as text, checkboxes, and submenus:

| Menu | Items |
|------|-------|
| **File** | Open Environment, Save, Save As, Exit (Windows) |
| **Edit** | Preferences (Windows only; macOS uses app menu) |
| **View** | Panel toggles, Show FPS, Show BPM, Layout submenu |
| **Tools** | HAP Converter, MIDI Input submenu, MIDI Mapping |

Platform-specific menus:
- **macOS App Menu**: About, Preferences, Quit
- **Windows Help Menu**: About

### egui Only (RIGHT side)

The right-side status area remains egui-only because native menus cannot render:
- Custom widgets (DragValue for BPM editing)
- Animated graphics (beat indicator dots)
- Real-time updating displays (FPS counter)
- Fade animations (status messages)

| Element | Why egui-only |
|---------|---------------|
| FPS display | Real-time updates, monospace formatting |
| BPM beat dots | Custom circle drawing with pulse animation |
| BPM value | Editable DragValue widget |
| TAP button | Interactive button with hover tooltip |
| Resync button | Interactive button |
| Status messages | Fade-out animation over 3 seconds |

## File Structure

```
src/ui/
├── menu_definition.rs   # Shared declarative menu structures
├── native_menu.rs       # Builds muda menus from definition
├── menu_bar.rs          # Renders egui menus from definition
└── mod.rs               # Module exports
```

## Core Types

### MenuItemId

Unique identifier for each menu item. Used for:
- Mapping native menu events to actions
- Looking up check items for state synchronization
- Converting to/from string IDs for muda

```rust
pub enum MenuItemId {
    // File menu
    Open, Save, SaveAs, Exit,

    // Edit/App menu
    Preferences,

    // View menu - panels
    PanelClipGrid, PanelProperties, PanelSources,
    PanelEffectsBrowser, PanelPreviewMonitor,
    PanelPerformance, PanelPrevis, PanelMidi,

    // View menu - settings
    ShowFps, ShowBpm,

    // Layout submenu
    LayoutPreset(usize), LayoutSave, LayoutReset,

    // Tools menu
    HapConverter, MidiMapping,
}
```

### MenuItem

Declarative menu item types:

```rust
pub enum MenuItem {
    // Clickable button with optional keyboard shortcut
    Action { id: MenuItemId, label: String, shortcut: Option<Shortcut>, enabled: bool },

    // Checkbox with state
    Check { id: MenuItemId, label: String, shortcut: Option<Shortcut>, default_checked: bool },

    // Visual separator line
    Separator,

    // Nested submenu
    Submenu { label: String, items: Vec<MenuItem> },

    // Non-clickable label (section headers)
    Label { text: String },

    // Platform-provided items (About, Quit)
    Predefined(PredefinedItem),
}
```

### MenuBarDefinition

Factory that builds the complete menu structure:

```rust
pub struct MenuBarDefinition {
    pub app_menu: Option<MenuDefinition>,   // macOS only
    pub menus: Vec<MenuDefinition>,         // File, Edit, View, Tools
    pub help_menu: Option<MenuDefinition>,  // Windows only
}

impl MenuBarDefinition {
    pub fn build() -> Self { ... }
}
```

## Adding a New Menu Item

### 1. Add the MenuItemId

In `menu_definition.rs`, add a new variant to `MenuItemId`:

```rust
pub enum MenuItemId {
    // ... existing items ...
    MyNewAction,
}
```

### 2. Implement ID Conversions

Add string conversion for muda event handling:

```rust
impl MenuItemId {
    pub fn to_string_id(&self) -> String {
        match self {
            // ... existing matches ...
            Self::MyNewAction => "tools_my_new_action".into(),
        }
    }

    pub fn from_string_id(s: &str) -> Option<Self> {
        match s {
            // ... existing matches ...
            "tools_my_new_action" => Some(Self::MyNewAction),
            _ => None,
        }
    }
}
```

### 3. Map to Action

Define what happens when the item is clicked:

```rust
impl MenuItemId {
    pub fn to_action(&self) -> MenuItemAction {
        match self {
            // ... existing matches ...
            Self::MyNewAction => MenuItemAction::Menu(MenuAction::MyNewAction),
        }
    }
}
```

### 4. Add to Menu Definition

Add the item to the appropriate menu builder:

```rust
fn build_tools_menu() -> MenuDefinition {
    MenuDefinition {
        label: "Tools".into(),
        items: vec![
            // ... existing items ...
            MenuItem::Action {
                id: MenuItemId::MyNewAction,
                label: "My New Action...".into(),
                shortcut: None,
                enabled: true,
            },
        ],
    }
}
```

### 5. Handle the Action

In `menu_bar.rs`, add the `MenuAction` variant and handle it in `app.rs`.

## Keyboard Shortcuts

Shortcuts are defined using the `Shortcut` struct:

```rust
MenuItem::Action {
    id: MenuItemId::Save,
    label: "Save".into(),
    shortcut: Some(Shortcut::new(Modifiers::META, Code::KeyS)),
    enabled: true,
}
```

The `Shortcut` type handles platform differences:
- **Native menu**: Converts to `muda::Accelerator` for OS-level handling
- **egui menu**: Displays formatted text (e.g., "⌘S" on macOS, "Ctrl+S" on Windows)

## State Synchronization

Checkbox items (panels, Show FPS, Show BPM) need bidirectional state sync:

### Native Menu → App

When a native menu checkbox is clicked, `NativeMenu::poll_events()` returns a `NativeMenuEvent` that the app handles to update state.

### App → Native Menu

When state changes programmatically (e.g., closing a panel via its X button), call:

```rust
native_menu.sync_state(&panel_states, &settings);
```

This updates all check items to match the current app state.

## Dynamic Content

Some submenus contain dynamic content that can't be defined statically:

### Layout Submenu

The Layout submenu shows available presets from `LayoutPresetManager`. The static definition includes only the separator and action buttons:

```rust
MenuItem::Submenu {
    label: "Layout".into(),
    items: vec![
        MenuItem::Separator,
        MenuItem::Action { id: MenuItemId::LayoutSave, ... },
        MenuItem::Action { id: MenuItemId::LayoutReset, ... },
    ],
}
```

The egui renderer (`render_layout_submenu`) injects preset items dynamically.

### MIDI Input Submenu

The MIDI submenu shows available devices from runtime discovery. The static definition is empty:

```rust
MenuItem::Submenu {
    label: "MIDI Input".into(),
    items: vec![],
}
```

The egui renderer (`render_midi_submenu`) populates it with current device list.

**Note**: Native menus don't support dynamic MIDI device lists. This submenu is egui-only for full functionality.

## Platform-Specific Behavior

### macOS
- App menu with About, Preferences (⌘,), and Quit
- Menu appears in system menu bar (not window)
- Uses `menu.init_for_nsapp()` for global menu

### Windows
- Edit menu contains Preferences
- Help menu contains About
- Exit item in File menu (Alt+F4)
- Menu attached to window via `menu.init_for_hwnd()`

### Linux
- Native menus not supported
- Falls back to egui-only menus
- `is_native_menu_supported()` returns `false`
