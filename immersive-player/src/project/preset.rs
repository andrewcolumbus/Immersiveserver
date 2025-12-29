//! Project preset for saving and loading configurations
//!
//! Handles serialization of project state including composition, layers, clips, and screens.

#![allow(dead_code)]

use crate::composition::{
    BlendMode, Clip, ClipSlot, Composition, CompositionSettings, GeneratorClip, GeneratorType,
    ImageClip, Layer, LayerTransform, SolidColorClip, TriggerMode, VideoClip,
};
use crate::output::{BlendConfig, EdgeBlend, Screen, Slice};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Project preset containing all configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectPreset {
    /// Project name
    pub name: String,
    /// Project description
    pub description: String,
    /// Version string
    pub version: String,
    /// Composition configuration
    pub composition: CompositionPreset,
    /// Screen configurations
    pub screens: Vec<ScreenPreset>,
}

impl Default for ProjectPreset {
    fn default() -> Self {
        Self {
            name: "Untitled Project".to_string(),
            description: String::new(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            composition: CompositionPreset::default(),
            screens: Vec::new(),
        }
    }
}

impl ProjectPreset {
    /// Create a new project preset
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Create from a composition and output manager
    pub fn from_composition(
        name: impl Into<String>,
        composition: &Composition,
        screens: &[Screen],
    ) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            composition: CompositionPreset::from_composition(composition),
            screens: screens.iter().map(ScreenPreset::from_screen).collect(),
        }
    }

    /// Apply preset to composition
    pub fn apply_to_composition(&self, composition: &mut Composition) {
        self.composition.apply_to(composition);
    }

    /// Save to a file
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        log::info!("Saved project to {:?}", path);
        Ok(())
    }

    /// Load from a file
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        let preset: Self = serde_json::from_str(&json)?;
        log::info!("Loaded project from {:?}", path);
        Ok(preset)
    }

    /// Add a screen preset
    pub fn add_screen(&mut self, screen: &Screen) {
        self.screens.push(ScreenPreset::from_screen(screen));
    }
}

/// Composition preset (layers, settings, clips)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositionPreset {
    /// Composition settings
    pub settings: CompositionSettingsPreset,
    /// Layer configurations
    pub layers: Vec<LayerPreset>,
    /// Number of columns
    pub columns: usize,
    /// Master opacity
    pub master_opacity: f32,
    /// Master speed
    pub master_speed: f32,
}

impl Default for CompositionPreset {
    fn default() -> Self {
        Self {
            settings: CompositionSettingsPreset::default(),
            layers: vec![
                LayerPreset::new("Layer 4"),
                LayerPreset::new("Layer 3"),
                LayerPreset::new("Layer 2"),
                LayerPreset::new("Layer 1"),
            ],
            columns: 6,
            master_opacity: 1.0,
            master_speed: 1.0,
        }
    }
}

impl CompositionPreset {
    /// Create from a Composition
    pub fn from_composition(composition: &Composition) -> Self {
        Self {
            settings: CompositionSettingsPreset::from_settings(&composition.settings),
            layers: composition
                .layers
                .iter()
                .map(LayerPreset::from_layer)
                .collect(),
            columns: composition.columns,
            master_opacity: composition.master_opacity,
            master_speed: composition.master_speed,
        }
    }

    /// Apply to a Composition
    pub fn apply_to(&self, composition: &mut Composition) {
        composition.settings = self.settings.to_settings();
        composition.columns = self.columns;
        composition.master_opacity = self.master_opacity;
        composition.master_speed = self.master_speed;

        // Recreate layers
        composition.layers.clear();
        for (i, layer_preset) in self.layers.iter().enumerate() {
            composition
                .layers
                .push(layer_preset.to_layer(i as u32, self.columns));
        }
    }

    /// Create a Composition from this preset
    pub fn to_composition(&self) -> Composition {
        let mut composition = Composition::new(self.settings.to_settings(), self.columns, 0);
        composition.master_opacity = self.master_opacity;
        composition.master_speed = self.master_speed;

        for (i, layer_preset) in self.layers.iter().enumerate() {
            composition
                .layers
                .push(layer_preset.to_layer(i as u32, self.columns));
        }

        composition
    }
}

/// Composition settings preset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositionSettingsPreset {
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    pub background_color: [f32; 4],
}

impl Default for CompositionSettingsPreset {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: 60.0,
            background_color: [0.0, 0.0, 0.0, 1.0],
        }
    }
}

impl CompositionSettingsPreset {
    pub fn from_settings(settings: &CompositionSettings) -> Self {
        Self {
            width: settings.width,
            height: settings.height,
            fps: settings.fps,
            background_color: settings.background_color,
        }
    }

    pub fn to_settings(&self) -> CompositionSettings {
        CompositionSettings {
            width: self.width,
            height: self.height,
            fps: self.fps,
            background_color: self.background_color,
            ..Default::default()
        }
    }
}

/// Layer preset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerPreset {
    pub name: String,
    pub clips: Vec<Option<ClipSlotPreset>>,
    pub opacity: f32,
    pub blend_mode: String,
    pub bypass: bool,
    pub solo: bool,
    pub transform: LayerTransformPreset,
}

impl LayerPreset {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            clips: Vec::new(),
            opacity: 1.0,
            blend_mode: "Normal".to_string(),
            bypass: false,
            solo: false,
            transform: LayerTransformPreset::default(),
        }
    }

    pub fn from_layer(layer: &Layer) -> Self {
        Self {
            name: layer.name.clone(),
            clips: layer
                .clips
                .iter()
                .map(|opt| opt.as_ref().map(ClipSlotPreset::from_slot))
                .collect(),
            opacity: layer.opacity,
            blend_mode: layer.blend_mode.name().to_string(),
            bypass: layer.bypass,
            solo: layer.solo,
            transform: LayerTransformPreset::from_transform(&layer.transform),
        }
    }

    pub fn to_layer(&self, id: u32, columns: usize) -> Layer {
        let mut layer = Layer::new(id, self.name.clone(), columns);
        layer.opacity = self.opacity;
        layer.blend_mode = match self.blend_mode.as_str() {
            "Add" => BlendMode::Add,
            "Multiply" => BlendMode::Multiply,
            "Screen" => BlendMode::Screen,
            "Overlay" => BlendMode::Overlay,
            _ => BlendMode::Normal,
        };
        layer.bypass = self.bypass;
        layer.solo = self.solo;
        layer.transform = self.transform.to_transform();

        // Restore clips
        for (i, clip_opt) in self.clips.iter().enumerate() {
            if i < layer.clips.len() {
                layer.clips[i] = clip_opt.as_ref().map(|c| c.to_slot());
            }
        }

        layer
    }
}

/// Layer transform preset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerTransformPreset {
    pub position: (f32, f32),
    pub scale: (f32, f32),
    pub rotation: f32,
    pub anchor: (f32, f32),
}

impl Default for LayerTransformPreset {
    fn default() -> Self {
        Self {
            position: (0.0, 0.0),
            scale: (1.0, 1.0),
            rotation: 0.0,
            anchor: (0.5, 0.5),
        }
    }
}

impl LayerTransformPreset {
    pub fn from_transform(transform: &LayerTransform) -> Self {
        Self {
            position: transform.position,
            scale: transform.scale,
            rotation: transform.rotation,
            anchor: transform.anchor,
        }
    }

    pub fn to_transform(&self) -> LayerTransform {
        LayerTransform {
            position: self.position,
            scale: self.scale,
            rotation: self.rotation,
            anchor: self.anchor,
        }
    }
}

/// Clip slot preset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipSlotPreset {
    pub clip: ClipPreset,
    pub trigger_mode: String,
    pub speed: f32,
    pub opacity: f32,
}

impl ClipSlotPreset {
    pub fn from_slot(slot: &ClipSlot) -> Self {
        Self {
            clip: ClipPreset::from_clip(&slot.clip),
            trigger_mode: match slot.trigger_mode {
                TriggerMode::Toggle => "Toggle",
                TriggerMode::Flash => "Flash",
                TriggerMode::OneShot => "OneShot",
            }
            .to_string(),
            speed: slot.speed,
            opacity: slot.opacity,
        }
    }

    pub fn to_slot(&self) -> ClipSlot {
        let clip = self.clip.to_clip();
        let mut slot = ClipSlot::new(clip);
        slot.trigger_mode = match self.trigger_mode.as_str() {
            "Flash" => TriggerMode::Flash,
            "OneShot" => TriggerMode::OneShot,
            _ => TriggerMode::Toggle,
        };
        slot.speed = self.speed;
        slot.opacity = self.opacity;
        slot
    }
}

/// Clip preset (content reference)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClipPreset {
    Video {
        name: String,
        path: PathBuf,
        loop_mode: String,
    },
    Image {
        name: String,
        path: PathBuf,
    },
    SolidColor {
        name: String,
        color: [f32; 4],
    },
    Generator {
        generator_type: String,
        speed: f32,
    },
}

impl ClipPreset {
    pub fn from_clip(clip: &Clip) -> Self {
        match clip {
            Clip::Video(v) => ClipPreset::Video {
                name: v.name.clone(),
                path: v.path.clone(),
                loop_mode: format!("{:?}", v.loop_mode),
            },
            Clip::Image(i) => ClipPreset::Image {
                name: i.name.clone(),
                path: i.path.clone(),
            },
            Clip::SolidColor(s) => ClipPreset::SolidColor {
                name: s.name.clone(),
                color: s.color,
            },
            Clip::Generator(g) => ClipPreset::Generator {
                generator_type: g.generator_type.name().to_string(),
                speed: g.speed,
            },
        }
    }

    pub fn to_clip(&self) -> Clip {
        match self {
            ClipPreset::Video {
                name,
                path,
                loop_mode: _,
            } => {
                let mut video = VideoClip::new(path.clone());
                video.name = name.clone();
                Clip::Video(video)
            }
            ClipPreset::Image { name, path } => {
                let mut image = ImageClip::new(path.clone());
                image.name = name.clone();
                Clip::Image(image)
            }
            ClipPreset::SolidColor { name, color } => {
                Clip::SolidColor(SolidColorClip::with_name(name, *color))
            }
            ClipPreset::Generator {
                generator_type,
                speed,
            } => {
                let gen_type = match generator_type.as_str() {
                    "Noise" => GeneratorType::simple_noise(),
                    "Gradient" => {
                        GeneratorType::horizontal_gradient([0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0, 1.0])
                    }
                    "Plasma" => GeneratorType::Plasma {
                        speed: 1.0,
                        scale: 1.0,
                    },
                    "Color Bars" => GeneratorType::ColorBars,
                    _ => GeneratorType::simple_noise(),
                };
                let mut gen = GeneratorClip::new(gen_type);
                gen.speed = *speed;
                Clip::Generator(gen)
            }
        }
    }
}

/// Screen configuration preset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenPreset {
    /// Screen name
    pub name: String,
    /// Display ID
    pub display_id: u32,
    /// Resolution (width, height)
    pub resolution: (u32, u32),
    /// Position (x, y)
    pub position: (f32, f32),
    /// Blend configuration
    pub blend_config: BlendConfigPreset,
    /// Slices
    pub slices: Vec<SlicePreset>,
    /// Whether enabled
    pub enabled: bool,
    /// Opacity
    pub opacity: f32,
}

impl ScreenPreset {
    /// Create from a Screen
    pub fn from_screen(screen: &Screen) -> Self {
        Self {
            name: screen.name.clone(),
            display_id: screen.display_id,
            resolution: screen.resolution,
            position: screen.position,
            blend_config: BlendConfigPreset::from_config(&screen.blend_config),
            slices: screen.slices.iter().map(SlicePreset::from_slice).collect(),
            enabled: screen.enabled,
            opacity: screen.opacity,
        }
    }

    /// Convert to a Screen
    pub fn to_screen(&self) -> Screen {
        let mut screen = Screen::new_at_position(
            self.name.clone(),
            self.display_id,
            self.resolution,
            self.position,
        );
        screen.blend_config = self.blend_config.to_config();
        screen.enabled = self.enabled;
        screen.opacity = self.opacity;
        for slice_preset in &self.slices {
            screen.add_slice(slice_preset.to_slice());
        }
        screen
    }
}

/// Blend configuration preset
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BlendConfigPreset {
    pub left: Option<EdgeBlendPreset>,
    pub right: Option<EdgeBlendPreset>,
    pub top: Option<EdgeBlendPreset>,
    pub bottom: Option<EdgeBlendPreset>,
}

impl BlendConfigPreset {
    pub fn from_config(config: &BlendConfig) -> Self {
        Self {
            left: config.left.as_ref().map(EdgeBlendPreset::from_blend),
            right: config.right.as_ref().map(EdgeBlendPreset::from_blend),
            top: config.top.as_ref().map(EdgeBlendPreset::from_blend),
            bottom: config.bottom.as_ref().map(EdgeBlendPreset::from_blend),
        }
    }

    pub fn to_config(&self) -> BlendConfig {
        BlendConfig {
            left: self.left.as_ref().map(|b| b.to_blend()),
            right: self.right.as_ref().map(|b| b.to_blend()),
            top: self.top.as_ref().map(|b| b.to_blend()),
            bottom: self.bottom.as_ref().map(|b| b.to_blend()),
        }
    }
}

/// Edge blend preset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeBlendPreset {
    pub width: u32,
    pub power: f32,
    pub gamma: f32,
    pub black_level: f32,
}

impl EdgeBlendPreset {
    pub fn from_blend(blend: &EdgeBlend) -> Self {
        Self {
            width: blend.width,
            power: blend.power,
            gamma: blend.gamma,
            black_level: blend.black_level,
        }
    }

    pub fn to_blend(&self) -> EdgeBlend {
        EdgeBlend {
            width: self.width,
            power: self.power,
            gamma: self.gamma,
            black_level: self.black_level,
        }
    }
}

/// Slice preset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlicePreset {
    pub name: String,
    pub input_x: f32,
    pub input_y: f32,
    pub input_width: f32,
    pub input_height: f32,
}

impl SlicePreset {
    pub fn from_slice(slice: &Slice) -> Self {
        Self {
            name: slice.name.clone(),
            input_x: slice.input_rect.x,
            input_y: slice.input_rect.y,
            input_width: slice.input_rect.width,
            input_height: slice.input_rect.height,
        }
    }

    pub fn to_slice(&self) -> Slice {
        Slice::normalized(self.name.clone())
    }
}

/// Global settings (legacy, kept for backwards compatibility)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSettings {
    /// Composition width
    pub composition_width: u32,
    /// Composition height
    pub composition_height: u32,
    /// Frame rate
    pub fps: f32,
    /// Background color
    pub background_color: [f32; 4],
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            composition_width: 1920,
            composition_height: 1080,
            fps: 60.0,
            background_color: [0.0, 0.0, 0.0, 1.0],
        }
    }
}
