# egui-widgets

Custom egui widgets for Immersive Server, located in `external_libraries/egui-widgets/`.

## Overview

This crate provides specialized widgets built on top of egui for media applications. These widgets are designed for video/audio playback interfaces and can be reused across different panels.

## Widgets

### Video Scrubber

A timeline scrubber widget with click-to-seek and drag support, designed for video/audio playback.

**Location:** `egui-widgets/src/scrubber.rs`

#### Features

- Click anywhere on the track to seek to that position
- Drag the handle for continuous scrubbing
- Visual feedback on hover and drag states
- Automatic time display (current position / duration)
- Proper scrub start/end events for pause/resume handling

#### Usage

```rust
use egui_widgets::{video_scrubber, ScrubberAction, ScrubberState};

// Store state in your panel struct
struct MyPanel {
    scrubber_state: ScrubberState,
}

// In your render function
let (actions, display_position) = video_scrubber(
    ui,
    &mut self.scrubber_state,
    current_position_secs,  // f64
    total_duration_secs,    // f64
);

// Handle actions
for action in actions {
    match action {
        ScrubberAction::StartScrub => {
            // Pause video, store play state
        }
        ScrubberAction::Seek { time_secs } => {
            // Seek to position (called during drag)
        }
        ScrubberAction::EndScrub { time_secs } => {
            // Seek to final position, restore play state
        }
    }
}
```

#### Components

| Type | Description |
|------|-------------|
| `ScrubberState` | Persistent state for tracking scrub mode |
| `ScrubberAction` | Events emitted by the scrubber |
| `video_scrubber()` | Main render function |
| `format_time()` | Utility to format seconds as `MM:SS.ff` |

#### Visual Design

```
[===========o-----------------]  <- Track with handle
 00:15.30                02:45.00  <- Time display
```

- **Track:** 4px height, dark gray background
- **Filled portion:** Light gray, shows progress
- **Handle:** 16px diameter circle, white when hovered/dragged
- **Time display:** Monospace font, current time left, duration right

## Adding New Widgets

To add a new widget:

1. Create a new file in `egui-widgets/src/`
2. Implement the widget following egui patterns
3. Export from `lib.rs`
4. Re-export in `immersive-server/src/ui/mod.rs` if needed

## Dependencies

- `egui = "0.31"` - The egui immediate mode GUI library

## Integration

The crate is included in immersive-server via:

```toml
# Cargo.toml
egui-widgets = { path = "../external_libraries/egui-widgets" }
```

Widgets are re-exported from the UI module:

```rust
// src/ui/mod.rs
pub use egui_widgets::{format_time, video_scrubber, ScrubberAction, ScrubberState};
```

## Current Usage

The video scrubber is used in:

- **Preview Monitor Panel** - Timeline for clip preview playback
- **Properties Panel** - Timeline in the Clip tab for layer playback control
