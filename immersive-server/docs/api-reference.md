# Immersive Server REST API Reference

This document describes the REST API and WebSocket interface for controlling Immersive Server remotely.

## Overview

- **Base URL:** `http://localhost:8080/api`
- **Default Port:** 8080 (configurable in Environment settings)
- **Content Type:** `application/json`

## Status

### GET /api/status

Returns server status and version information.

**Response:**
```json
{
  "status": "ok",
  "version": "0.1.0",
  "uptime_seconds": 3600
}
```

---

## Environment

### GET /api/environment

Get full environment state including resolution, FPS, and thumbnail mode.

**Response:**
```json
{
  "width": 1920,
  "height": 1080,
  "fps": 60,
  "thumbnail_mode": false
}
```

### PUT /api/environment

Update environment settings.

**Request:**
```json
{
  "width": 3840,
  "height": 2160,
  "fps": 60
}
```

### GET /api/environment/effects

List effects applied to the master/environment chain.

**Response:**
```json
{
  "effects": [
    {
      "id": "abc123",
      "type": "color_correction",
      "enabled": true,
      "parameters": {
        "brightness": 1.0,
        "contrast": 1.0,
        "saturation": 1.0
      }
    }
  ]
}
```

### POST /api/environment/effects

Add effect to master chain.

**Request:**
```json
{
  "type": "color_correction"
}
```

### PUT /api/environment/effects/:id

Update effect parameters.

**Request:**
```json
{
  "parameters": {
    "brightness": 1.2,
    "saturation": 0.8
  }
}
```

### DELETE /api/environment/effects/:id

Remove effect from master chain.

### POST /api/environment/effects/:id/bypass

Toggle bypass state for an effect.

### POST /api/environment/effects/:id/solo

Toggle solo state for an effect.

### POST /api/environment/effects/reorder

Reorder effects in the chain.

**Request:**
```json
{
  "order": ["effect_id_1", "effect_id_2", "effect_id_3"]
}
```

---

## Layers

### GET /api/layers

List all layers.

**Response:**
```json
{
  "layers": [
    {
      "id": 0,
      "name": "Layer 1",
      "visible": true,
      "opacity": 1.0,
      "blend_mode": "Normal"
    }
  ]
}
```

### POST /api/layers

Create a new layer.

**Request:**
```json
{
  "name": "New Layer"
}
```

### GET /api/layers/:id

Get layer details.

**Response:**
```json
{
  "id": 0,
  "name": "Layer 1",
  "visible": true,
  "opacity": 1.0,
  "blend_mode": "Normal",
  "transform": {
    "position": { "x": 0, "y": 0 },
    "scale": { "x": 1.0, "y": 1.0 },
    "rotation": 0.0,
    "anchor": "Center"
  },
  "tile_x": 1,
  "tile_y": 1,
  "clip_transition": {
    "type": "Cut"
  }
}
```

### PUT /api/layers/:id

Update layer properties.

**Request:**
```json
{
  "name": "Updated Layer",
  "opacity": 0.8,
  "visible": true
}
```

### DELETE /api/layers/:id

Delete a layer.

### POST /api/layers/:id/clone

Clone a layer with all its clips and settings.

### POST /api/layers/reorder

Reorder layers (move_to_front/back).

**Request:**
```json
{
  "layer_id": 0,
  "position": 2
}
```

---

## Layer Transform

### PUT /api/layers/:id/transform

Set all transform properties.

**Request:**
```json
{
  "position": { "x": 100, "y": 50 },
  "scale": { "x": 1.5, "y": 1.5 },
  "rotation": 45.0,
  "anchor": "Center"
}
```

### PUT /api/layers/:id/position

Set layer position.

**Request:**
```json
{
  "x": 100,
  "y": 50
}
```

### PUT /api/layers/:id/scale

Set layer scale.

**Request:**
```json
{
  "scale_x": 1.5,
  "scale_y": 1.5
}
```

### PUT /api/layers/:id/rotation

Set layer rotation.

**Request:**
```json
{
  "rotation": 45.0
}
```

---

## Layer Properties

### PUT /api/layers/:id/opacity

Set layer opacity.

**Request:**
```json
{
  "opacity": 0.75
}
```

**Range:** 0.0 (transparent) to 1.0 (opaque)

### PUT /api/layers/:id/blend

Set layer blend mode.

**Request:**
```json
{
  "blend_mode": "Additive"
}
```

**Options:** `Normal`, `Additive`, `Multiply`, `Screen`

### PUT /api/layers/:id/visibility

Set layer visibility.

**Request:**
```json
{
  "visible": true
}
```

### PUT /api/layers/:id/tiling

Set layer tiling.

**Request:**
```json
{
  "tile_x": 2,
  "tile_y": 2
}
```

### PUT /api/layers/:id/transition

Set clip transition mode for the layer.

**Request:**
```json
{
  "type": "Fade",
  "duration_ms": 500
}
```

**Types:** `Cut`, `Fade`, `Crossfade`

---

## Layer Effects

### GET /api/layers/:id/effects

List effects on a layer.

### POST /api/layers/:id/effects

Add effect to layer.

**Request:**
```json
{
  "type": "invert"
}
```

### PUT /api/layers/:id/effects/:eid

Update effect parameters.

### DELETE /api/layers/:id/effects/:eid

Remove effect from layer.

### POST /api/layers/:id/effects/:eid/bypass

Toggle effect bypass.

### POST /api/layers/:id/effects/:eid/solo

Toggle effect solo.

### POST /api/layers/:id/effects/reorder

Reorder layer effects.

---

## Clips

### GET /api/layers/:id/clips

List all clips in layer's grid.

**Response:**
```json
{
  "active_slot": 0,
  "clips": [
    {
      "slot": 0,
      "source": "/path/to/video.mp4",
      "source_type": "file",
      "label": "Intro"
    },
    {
      "slot": 1,
      "source": null
    }
  ]
}
```

### GET /api/layers/:id/clips/:slot

Get clip at specific slot.

### PUT /api/layers/:id/clips/:slot

Assign clip to slot.

**Request (file):**
```json
{
  "source_type": "file",
  "path": "/path/to/video.mp4",
  "label": "My Clip"
}
```

**Request (OMT source):**
```json
{
  "source_type": "omt",
  "source_id": "omt-source-name"
}
```

**Request (NDI source):**
```json
{
  "source_type": "ndi",
  "source_id": "NDI-SOURCE-NAME"
}
```

### DELETE /api/layers/:id/clips/:slot

Clear clip from slot.

### POST /api/layers/:id/clips/:slot/trigger

Trigger clip playback.

**Request (optional):**
```json
{
  "transition": "Crossfade",
  "duration_ms": 1000
}
```

### POST /api/layers/:id/clips/stop

Stop current clip immediately.

### POST /api/layers/:id/clips/stop-fade

Stop current clip with fade out.

**Request:**
```json
{
  "duration_ms": 500
}
```

---

## Clip Effects

### GET /api/layers/:id/clips/:slot/effects

List effects on a clip.

### POST /api/layers/:id/clips/:slot/effects

Add effect to clip.

### PUT /api/layers/:id/clips/:slot/effects/:eid

Update clip effect parameters.

### DELETE /api/layers/:id/clips/:slot/effects/:eid

Remove effect from clip.

### POST /api/layers/:id/clips/:slot/effects/:eid/bypass

Toggle clip effect bypass.

---

## Clip Clipboard

### POST /api/layers/:id/clips/:slot/copy

Copy clip to clipboard.

### POST /api/layers/:id/clips/:slot/paste

Paste clip from clipboard.

---

## Grid Management

### POST /api/layers/columns

Add column to all layers.

### DELETE /api/layers/columns/:index

Delete column from all layers.

---

## Playback Control

### POST /api/playback/pause

Pause all layers.

### POST /api/playback/resume

Resume all layers.

### POST /api/playback/toggle

Toggle pause state.

### POST /api/playback/restart

Restart all videos from beginning.

### GET /api/playback/status

Get playback state.

**Response:**
```json
{
  "paused": false,
  "playing_layers": [0, 2]
}
```

---

## Per-Layer Playback

### POST /api/layers/:id/playback/pause

Pause specific layer.

### POST /api/layers/:id/playback/resume

Resume specific layer.

### POST /api/layers/:id/playback/toggle

Toggle specific layer.

### POST /api/layers/:id/playback/restart

Restart specific layer video.

---

## Effects Registry

### GET /api/effects

List all available effect types.

**Response:**
```json
{
  "effects": [
    {
      "type": "color_correction",
      "name": "Color Correction",
      "category": "Color"
    },
    {
      "type": "invert",
      "name": "Invert",
      "category": "Color"
    }
  ]
}
```

### GET /api/effects/:type

Get effect definition with parameters.

**Response:**
```json
{
  "type": "color_correction",
  "name": "Color Correction",
  "category": "Color",
  "parameters": [
    {
      "name": "brightness",
      "type": "float",
      "default": 1.0,
      "min": 0.0,
      "max": 2.0
    },
    {
      "name": "contrast",
      "type": "float",
      "default": 1.0,
      "min": 0.0,
      "max": 2.0
    }
  ]
}
```

### GET /api/effects/categories

List effect categories.

**Response:**
```json
{
  "categories": ["Color", "Distort", "Blur", "Stylize"]
}
```

---

## Source Discovery

### GET /api/sources

List all discovered sources (OMT and NDI).

**Response:**
```json
{
  "sources": [
    {
      "id": "omt-source-1",
      "type": "omt",
      "name": "OMT Source 1",
      "ip": "192.168.1.100",
      "port": 5000
    },
    {
      "id": "ndi-source-1",
      "type": "ndi",
      "name": "NDI Source (Machine)"
    }
  ]
}
```

### GET /api/sources/omt

List OMT sources only.

### GET /api/sources/ndi

List NDI sources only.

### POST /api/sources/omt/refresh

Refresh OMT source discovery.

### POST /api/sources/ndi/start

Start NDI discovery.

### POST /api/sources/ndi/stop

Stop NDI discovery.

### POST /api/sources/ndi/refresh

Refresh NDI sources.

---

## Streaming - OMT Broadcast

### GET /api/streaming/omt

Get OMT broadcast status.

**Response:**
```json
{
  "broadcasting": true,
  "name": "Immersive Server",
  "port": 5000,
  "capture_fps": 60
}
```

### POST /api/streaming/omt/start

Start OMT broadcast.

**Request:**
```json
{
  "name": "My Broadcast",
  "port": 5000
}
```

### POST /api/streaming/omt/stop

Stop OMT broadcast.

### PUT /api/streaming/omt/fps

Set OMT capture FPS.

**Request:**
```json
{
  "fps": 30
}
```

**Range:** 1-60

---

## Streaming - NDI Broadcast

### GET /api/streaming/ndi

Get NDI broadcast status.

### POST /api/streaming/ndi/start

Start NDI broadcast.

**Request:**
```json
{
  "name": "Immersive Server NDI"
}
```

### POST /api/streaming/ndi/stop

Stop NDI broadcast.

### PUT /api/streaming/ndi/fps

Set NDI capture FPS.

---

## Streaming - Texture Sharing (Syphon/Spout)

### GET /api/streaming/texture

Get texture sharing status.

### POST /api/streaming/texture/start

Start texture sharing (Syphon on macOS, Spout on Windows).

### POST /api/streaming/texture/stop

Stop texture sharing.

---

## Output Displays

### GET /api/outputs

List connected displays.

**Response:**
```json
{
  "outputs": [
    {
      "id": 0,
      "name": "Built-in Display",
      "width": 2560,
      "height": 1600,
      "primary": true
    },
    {
      "id": 1,
      "name": "External Projector",
      "width": 1920,
      "height": 1080,
      "primary": false
    }
  ]
}
```

### GET /api/outputs/:id

Get output configuration.

### PUT /api/outputs/:id

Update output configuration (mapping, blend settings).

---

## Viewport Control

### GET /api/viewport

Get current viewport state.

**Response:**
```json
{
  "zoom": 1.0,
  "pan_x": 0.0,
  "pan_y": 0.0
}
```

### POST /api/viewport/reset

Reset viewport to fit-to-window.

### PUT /api/viewport/zoom

Set zoom level.

**Request:**
```json
{
  "zoom": 2.0
}
```

**Range:** 0.1-8.0

### PUT /api/viewport/pan

Set pan offset.

**Request:**
```json
{
  "x": 100.0,
  "y": -50.0
}
```

---

## File Operations

### GET /api/files/current

Get current file path.

**Response:**
```json
{
  "path": "/path/to/project.immersive",
  "modified": false
}
```

### POST /api/files/open

Open environment file.

**Request:**
```json
{
  "path": "/path/to/project.immersive"
}
```

### POST /api/files/save

Save to current file.

### POST /api/files/save-as

Save to new file.

**Request:**
```json
{
  "path": "/path/to/new-project.immersive"
}
```

### GET /api/files/recent

List recent files.

**Response:**
```json
{
  "files": [
    "/path/to/recent1.immersive",
    "/path/to/recent2.immersive"
  ]
}
```

---

## Status & Metrics

### GET /api/status

Full system status (see above).

### GET /api/status/fps

Get current FPS and frame time.

**Response:**
```json
{
  "fps": 60.0,
  "frame_time_ms": 16.6,
  "target_fps": 60
}
```

### GET /api/status/connections

Get OMT/NDI connection counts.

**Response:**
```json
{
  "omt_sources": 2,
  "ndi_sources": 3,
  "omt_broadcasting": true,
  "ndi_broadcasting": false
}
```

### GET /api/status/performance

Get GPU/memory metrics.

**Response:**
```json
{
  "gpu_usage_percent": 45.0,
  "memory_used_mb": 512,
  "texture_count": 24,
  "layer_count": 4
}
```

---

## WebSocket API

Connect to `ws://localhost:8080/ws` for real-time updates.

### Event Types

Events are sent as JSON messages with a `type` field:

```json
{
  "type": "layer:updated",
  "data": {
    "layer_id": 0,
    "changes": { "opacity": 0.8 }
  }
}
```

| Event | Description |
|-------|-------------|
| `layer:added` | New layer created |
| `layer:removed` | Layer deleted |
| `layer:updated` | Layer properties changed |
| `clip:triggered` | Clip playback started |
| `clip:stopped` | Clip playback stopped |
| `effect:added` | Effect added to layer/clip/environment |
| `effect:removed` | Effect removed |
| `effect:updated` | Effect parameters changed |
| `playback:paused` | Global playback paused |
| `playback:resumed` | Global playback resumed |
| `source:discovered` | New OMT/NDI source found |
| `source:lost` | OMT/NDI source disconnected |
| `streaming:started` | Broadcast started (OMT/NDI) |
| `streaming:stopped` | Broadcast stopped |
| `viewport:changed` | Viewport zoom/pan changed |
| `file:opened` | Environment file opened |
| `file:saved` | Environment file saved |
| `status:fps` | Periodic FPS update (every 1s) |

### Bi-directional Commands

You can send commands over WebSocket for low-latency control:

```json
{
  "action": "trigger_clip",
  "layer_id": 0,
  "slot": 5
}
```

---

## Error Responses

All error responses follow this format:

```json
{
  "error": "Not Found",
  "message": "Layer with id 99 not found",
  "code": 404
}
```

| Status Code | Description |
|-------------|-------------|
| 200 | Success |
| 201 | Created |
| 400 | Bad Request (invalid JSON, missing fields) |
| 401 | Unauthorized (invalid/missing auth token) |
| 404 | Not Found (resource doesn't exist) |
| 500 | Internal Server Error |

---

## Authentication

When authentication is enabled, include the token in the `Authorization` header:

```
Authorization: Bearer <token>
```

Authentication can be enabled in the Environment settings. When disabled, all endpoints are accessible without a token.
