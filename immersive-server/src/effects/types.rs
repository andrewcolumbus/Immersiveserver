//! Core effect data types
//!
//! These types define the serializable data model for effects.
//! They are separate from runtime GPU resources, following the
//! Layer/LayerRuntime pattern used elsewhere in the codebase.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Parameter value types supported by effects
#[derive(Debug, Clone, PartialEq)]
pub enum ParameterValue {
    /// Floating point value
    Float(f32),
    /// Integer value
    Int(i32),
    /// Boolean value
    Bool(bool),
    /// RGBA color (0.0-1.0 per channel)
    Color([f32; 4]),
    /// 2D vector
    Vec2([f32; 2]),
    /// 3D vector
    Vec3([f32; 3]),
    /// Enumeration (index into options list)
    Enum {
        index: usize,
        options: Vec<String>,
    },
    /// String value (for file paths, text, etc.)
    String(String),
}

/// Helper struct for ParameterValue serialization (quick-xml compatible)
#[derive(Serialize, Deserialize)]
struct ParameterValueHelper {
    #[serde(rename = "type")]
    value_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    float_val: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    int_val: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bool_val: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    color_val: Option<[f32; 4]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    vec2_val: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    vec3_val: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    enum_index: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    enum_options: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    string_val: Option<String>,
}

impl Serialize for ParameterValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let helper = match self {
            ParameterValue::Float(v) => ParameterValueHelper {
                value_type: "Float".to_string(),
                float_val: Some(*v),
                int_val: None,
                bool_val: None,
                color_val: None,
                vec2_val: None,
                vec3_val: None,
                enum_index: None,
                enum_options: None,
                string_val: None,
            },
            ParameterValue::Int(v) => ParameterValueHelper {
                value_type: "Int".to_string(),
                float_val: None,
                int_val: Some(*v),
                bool_val: None,
                color_val: None,
                vec2_val: None,
                vec3_val: None,
                enum_index: None,
                enum_options: None,
                string_val: None,
            },
            ParameterValue::Bool(v) => ParameterValueHelper {
                value_type: "Bool".to_string(),
                float_val: None,
                int_val: None,
                bool_val: Some(*v),
                color_val: None,
                vec2_val: None,
                vec3_val: None,
                enum_index: None,
                enum_options: None,
                string_val: None,
            },
            ParameterValue::Color(v) => ParameterValueHelper {
                value_type: "Color".to_string(),
                float_val: None,
                int_val: None,
                bool_val: None,
                color_val: Some(*v),
                vec2_val: None,
                vec3_val: None,
                enum_index: None,
                enum_options: None,
                string_val: None,
            },
            ParameterValue::Vec2(v) => ParameterValueHelper {
                value_type: "Vec2".to_string(),
                float_val: None,
                int_val: None,
                bool_val: None,
                color_val: None,
                vec2_val: Some(*v),
                vec3_val: None,
                enum_index: None,
                enum_options: None,
                string_val: None,
            },
            ParameterValue::Vec3(v) => ParameterValueHelper {
                value_type: "Vec3".to_string(),
                float_val: None,
                int_val: None,
                bool_val: None,
                color_val: None,
                vec2_val: None,
                vec3_val: Some(*v),
                enum_index: None,
                enum_options: None,
                string_val: None,
            },
            ParameterValue::Enum { index, options } => ParameterValueHelper {
                value_type: "Enum".to_string(),
                float_val: None,
                int_val: None,
                bool_val: None,
                color_val: None,
                vec2_val: None,
                vec3_val: None,
                enum_index: Some(*index),
                enum_options: Some(options.clone()),
                string_val: None,
            },
            ParameterValue::String(v) => ParameterValueHelper {
                value_type: "String".to_string(),
                float_val: None,
                int_val: None,
                bool_val: None,
                color_val: None,
                vec2_val: None,
                vec3_val: None,
                enum_index: None,
                enum_options: None,
                string_val: Some(v.clone()),
            },
        };
        helper.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ParameterValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let helper = ParameterValueHelper::deserialize(deserializer)?;
        match helper.value_type.as_str() {
            "Float" => Ok(ParameterValue::Float(helper.float_val.unwrap_or(0.0))),
            "Int" => Ok(ParameterValue::Int(helper.int_val.unwrap_or(0))),
            "Bool" => Ok(ParameterValue::Bool(helper.bool_val.unwrap_or(false))),
            "Color" => Ok(ParameterValue::Color(helper.color_val.unwrap_or([1.0, 1.0, 1.0, 1.0]))),
            "Vec2" => Ok(ParameterValue::Vec2(helper.vec2_val.unwrap_or([0.0, 0.0]))),
            "Vec3" => Ok(ParameterValue::Vec3(helper.vec3_val.unwrap_or([0.0, 0.0, 0.0]))),
            "Enum" => Ok(ParameterValue::Enum {
                index: helper.enum_index.unwrap_or(0),
                options: helper.enum_options.unwrap_or_default(),
            }),
            "String" => Ok(ParameterValue::String(helper.string_val.unwrap_or_default())),
            _ => Ok(ParameterValue::Float(helper.float_val.unwrap_or(0.0))),
        }
    }
}

impl ParameterValue {
    /// Get the value as f32 (returns 0.0 for non-float types)
    pub fn as_f32(&self) -> f32 {
        match self {
            ParameterValue::Float(v) => *v,
            ParameterValue::Int(v) => *v as f32,
            ParameterValue::Bool(v) => if *v { 1.0 } else { 0.0 },
            _ => 0.0,
        }
    }

    /// Get the value as i32 (returns 0 for non-int types)
    pub fn as_i32(&self) -> i32 {
        match self {
            ParameterValue::Int(v) => *v,
            ParameterValue::Float(v) => *v as i32,
            ParameterValue::Bool(v) => if *v { 1 } else { 0 },
            ParameterValue::Enum { index, .. } => *index as i32,
            _ => 0,
        }
    }

    /// Get the value as bool (returns false for non-bool types)
    pub fn as_bool(&self) -> bool {
        match self {
            ParameterValue::Bool(v) => *v,
            ParameterValue::Float(v) => *v > 0.5,
            ParameterValue::Int(v) => *v != 0,
            _ => false,
        }
    }

    /// Get the value as String (returns empty string for non-string types)
    pub fn as_string(&self) -> String {
        match self {
            ParameterValue::String(v) => v.clone(),
            _ => String::new(),
        }
    }

    /// Get the value as &str (returns empty string for non-string types)
    pub fn as_str(&self) -> &str {
        match self {
            ParameterValue::String(v) => v.as_str(),
            _ => "",
        }
    }
}

/// Metadata for a parameter (describes the parameter, doesn't hold the value)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterMeta {
    /// Internal name (used as key)
    pub name: String,
    /// Display label in UI
    pub label: String,
    /// Default value
    pub default: ParameterValue,
    /// Minimum value (for numeric types)
    pub min: Option<f32>,
    /// Maximum value (for numeric types)
    pub max: Option<f32>,
    /// Step increment (for numeric types)
    pub step: Option<f32>,
}

impl ParameterMeta {
    /// Create a new float parameter metadata
    pub fn float(name: impl Into<String>, label: impl Into<String>, default: f32, min: f32, max: f32) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            default: ParameterValue::Float(default),
            min: Some(min),
            max: Some(max),
            step: None,
        }
    }

    /// Create a new float parameter with step increment
    pub fn float_with_step(name: impl Into<String>, label: impl Into<String>, default: f32, min: f32, max: f32, step: f32) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            default: ParameterValue::Float(default),
            min: Some(min),
            max: Some(max),
            step: Some(step),
        }
    }

    /// Create a new boolean parameter metadata
    pub fn bool(name: impl Into<String>, label: impl Into<String>, default: bool) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            default: ParameterValue::Bool(default),
            min: None,
            max: None,
            step: None,
        }
    }

    /// Create a new color parameter metadata
    pub fn color(name: impl Into<String>, label: impl Into<String>, default: [f32; 4]) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            default: ParameterValue::Color(default),
            min: None,
            max: None,
            step: None,
        }
    }

    /// Create a new enum parameter metadata
    pub fn enumeration(name: impl Into<String>, label: impl Into<String>, options: Vec<String>, default_index: usize) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            default: ParameterValue::Enum { index: default_index, options },
            min: None,
            max: None,
            step: None,
        }
    }

    /// Create a new string parameter metadata (for file paths, text, etc.)
    pub fn string(name: impl Into<String>, label: impl Into<String>, default: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            default: ParameterValue::String(default.into()),
            min: None,
            max: None,
            step: None,
        }
    }
}

/// A parameter with its current value and optional automation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    /// Parameter metadata
    pub meta: ParameterMeta,
    /// Current value
    pub value: ParameterValue,
    /// Optional automation source
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub automation: Option<AutomationSource>,
}

impl Parameter {
    /// Create a new parameter with default value
    pub fn new(meta: ParameterMeta) -> Self {
        let value = meta.default.clone();
        Self {
            meta,
            value,
            automation: None,
        }
    }

    /// Create a new parameter with a specific value
    pub fn with_value(meta: ParameterMeta, value: ParameterValue) -> Self {
        Self {
            meta,
            value,
            automation: None,
        }
    }

    /// Reset to default value
    pub fn reset(&mut self) {
        self.value = self.meta.default.clone();
    }
}

/// LFO waveform shape
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LfoShape {
    #[default]
    Sine,
    Triangle,
    Square,
    Sawtooth,
    SawtoothReverse,
    Random,
}

impl LfoShape {
    /// Get all available shapes
    pub fn all() -> &'static [LfoShape] {
        &[
            LfoShape::Sine,
            LfoShape::Triangle,
            LfoShape::Square,
            LfoShape::Sawtooth,
            LfoShape::SawtoothReverse,
            LfoShape::Random,
        ]
    }

    /// Get display name
    pub fn name(&self) -> &'static str {
        match self {
            LfoShape::Sine => "Sine",
            LfoShape::Triangle => "Triangle",
            LfoShape::Square => "Square",
            LfoShape::Sawtooth => "Sawtooth",
            LfoShape::SawtoothReverse => "Saw Rev",
            LfoShape::Random => "Random",
        }
    }
}

/// LFO automation source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LfoSource {
    /// Waveform shape
    pub shape: LfoShape,
    /// Frequency in Hz (or beats if sync_to_bpm)
    pub frequency: f32,
    /// Phase offset (0.0-1.0)
    pub phase: f32,
    /// Modulation depth (0.0-1.0)
    pub amplitude: f32,
    /// Center value offset
    pub offset: f32,
    /// Whether to sync frequency to BPM clock
    pub sync_to_bpm: bool,
    /// Beats per cycle (when sync_to_bpm is true)
    pub beats: f32,
}

impl Default for LfoSource {
    fn default() -> Self {
        Self {
            shape: LfoShape::Sine,
            frequency: 1.0,
            phase: 0.0,
            amplitude: 1.0,
            offset: 0.0,
            sync_to_bpm: false,
            beats: 4.0,
        }
    }
}

/// Beat trigger type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BeatTrigger {
    #[default]
    Beat,
    Bar,
    TwoBars,
    FourBars,
}

impl BeatTrigger {
    /// Get all available triggers
    pub fn all() -> &'static [BeatTrigger] {
        &[
            BeatTrigger::Beat,
            BeatTrigger::Bar,
            BeatTrigger::TwoBars,
            BeatTrigger::FourBars,
        ]
    }

    /// Get display name
    pub fn name(&self) -> &'static str {
        match self {
            BeatTrigger::Beat => "Beat",
            BeatTrigger::Bar => "Bar",
            BeatTrigger::TwoBars => "2 Bars",
            BeatTrigger::FourBars => "4 Bars",
        }
    }
}

/// Beat-sync automation (ADSR envelope triggered by beat/bar)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeatSource {
    /// What triggers the envelope
    pub trigger_on: BeatTrigger,
    /// Attack time in milliseconds
    pub attack_ms: f32,
    /// Decay time in milliseconds
    pub decay_ms: f32,
    /// Sustain level (0.0-1.0)
    pub sustain: f32,
    /// Release time in milliseconds
    pub release_ms: f32,
}

impl Default for BeatSource {
    fn default() -> Self {
        Self {
            trigger_on: BeatTrigger::Beat,
            attack_ms: 10.0,
            decay_ms: 100.0,
            sustain: 0.5,
            release_ms: 200.0,
        }
    }
}

/// All automation source types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AutomationSource {
    /// Low-frequency oscillator
    Lfo(LfoSource),
    /// Beat-triggered envelope
    Beat(BeatSource),
    // Future: Audio { band: AudioBand, sensitivity: f32 }
    // Future: Midi { channel: u8, cc: u8 }
    // Future: Osc { address: String }
}

/// Effect instance in the stack
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectInstance {
    /// Unique instance ID within the stack
    pub id: u32,
    /// Effect type identifier (e.g., "color_correction", "invert")
    pub effect_type: String,
    /// Human-readable name (can be customized)
    pub name: String,
    /// Parameters with current values
    pub parameters: Vec<Parameter>,
    /// Whether the effect is bypassed
    #[serde(default)]
    pub bypassed: bool,
    /// Whether the effect is soloed (only this effect renders when true)
    #[serde(default)]
    pub soloed: bool,
    /// Whether the effect is expanded in UI (not saved)
    #[serde(skip, default = "default_expanded")]
    pub expanded: bool,
}

fn default_expanded() -> bool {
    true
}

impl EffectInstance {
    /// Create a new effect instance
    pub fn new(id: u32, effect_type: impl Into<String>, name: impl Into<String>, parameters: Vec<Parameter>) -> Self {
        Self {
            id,
            effect_type: effect_type.into(),
            name: name.into(),
            parameters,
            bypassed: false,
            soloed: false,
            expanded: true,
        }
    }

    /// Get a parameter by name
    pub fn get_parameter(&self, name: &str) -> Option<&Parameter> {
        self.parameters.iter().find(|p| p.meta.name == name)
    }

    /// Get a mutable parameter by name
    pub fn get_parameter_mut(&mut self, name: &str) -> Option<&mut Parameter> {
        self.parameters.iter_mut().find(|p| p.meta.name == name)
    }

    /// Get parameter value as f32 by name
    pub fn get_f32(&self, name: &str) -> Option<f32> {
        self.get_parameter(name).map(|p| p.value.as_f32())
    }

    /// Get parameter value as bool by name
    pub fn get_bool(&self, name: &str) -> Option<bool> {
        self.get_parameter(name).map(|p| p.value.as_bool())
    }

    /// Get parameter value as string by name
    pub fn get_string(&self, name: &str) -> Option<String> {
        self.get_parameter(name).map(|p| p.value.as_string())
    }

    /// Set parameter value by name
    pub fn set_parameter(&mut self, name: &str, value: ParameterValue) -> bool {
        if let Some(param) = self.get_parameter_mut(name) {
            param.value = value;
            true
        } else {
            false
        }
    }
}

/// Stack of effects for a target (Environment/Layer/Clip)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EffectStack {
    /// Effects in the stack (processed in order)
    #[serde(default)]
    pub effects: Vec<EffectInstance>,
    /// Next effect instance ID
    #[serde(skip, default)]
    next_effect_id: u32,
}

impl EffectStack {
    /// Create a new empty effect stack
    pub fn new() -> Self {
        Self {
            effects: Vec::new(),
            next_effect_id: 1,
        }
    }

    /// Add an effect to the stack
    pub fn add(&mut self, effect_type: impl Into<String>, name: impl Into<String>, parameters: Vec<Parameter>) -> u32 {
        let id = self.next_effect_id;
        self.next_effect_id += 1;
        self.effects.push(EffectInstance::new(id, effect_type, name, parameters));
        id
    }

    /// Insert an effect at a specific index
    pub fn insert(&mut self, index: usize, effect_type: impl Into<String>, name: impl Into<String>, parameters: Vec<Parameter>) -> u32 {
        let id = self.next_effect_id;
        self.next_effect_id += 1;
        let index = index.min(self.effects.len());
        self.effects.insert(index, EffectInstance::new(id, effect_type, name, parameters));
        id
    }

    /// Remove an effect by ID
    pub fn remove(&mut self, effect_id: u32) -> bool {
        if let Some(pos) = self.effects.iter().position(|e| e.id == effect_id) {
            self.effects.remove(pos);
            true
        } else {
            false
        }
    }

    /// Get an effect by ID
    pub fn get(&self, effect_id: u32) -> Option<&EffectInstance> {
        self.effects.iter().find(|e| e.id == effect_id)
    }

    /// Get a mutable effect by ID
    pub fn get_mut(&mut self, effect_id: u32) -> Option<&mut EffectInstance> {
        self.effects.iter_mut().find(|e| e.id == effect_id)
    }

    /// Move an effect to a new index
    pub fn move_to(&mut self, effect_id: u32, new_index: usize) -> bool {
        if let Some(current_pos) = self.effects.iter().position(|e| e.id == effect_id) {
            let effect = self.effects.remove(current_pos);
            let insert_pos = new_index.min(self.effects.len());
            self.effects.insert(insert_pos, effect);
            true
        } else {
            false
        }
    }

    /// Check if any effect is soloed
    pub fn has_solo(&self) -> bool {
        self.effects.iter().any(|e| e.soloed)
    }

    /// Solo an effect (unsolo all others first)
    pub fn solo(&mut self, effect_id: u32) {
        for effect in &mut self.effects {
            effect.soloed = effect.id == effect_id;
        }
    }

    /// Unsolo all effects
    pub fn unsolo(&mut self) {
        for effect in &mut self.effects {
            effect.soloed = false;
        }
    }

    /// Move effect to a new index (alias for move_to)
    pub fn move_effect(&mut self, effect_id: u32, new_index: usize) {
        self.move_to(effect_id, new_index);
    }

    /// Get active (non-bypassed, respecting solo) effects
    pub fn active_effects(&self) -> impl Iterator<Item = &EffectInstance> {
        let has_solo = self.has_solo();
        self.effects.iter().filter(move |e| {
            !e.bypassed && (!has_solo || e.soloed)
        })
    }

    /// Check if stack is empty
    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    /// Get number of effects
    pub fn len(&self) -> usize {
        self.effects.len()
    }

    /// Clear all effects
    pub fn clear(&mut self) {
        self.effects.clear();
    }
}

/// Target for effect application
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EffectTarget {
    /// Master effects (applied to entire environment)
    Environment,
    /// Layer effects (applied to a specific layer)
    Layer(u32),
    /// Clip effects (applied to a specific clip)
    Clip { layer_id: u32, slot: usize },
}

impl std::fmt::Display for EffectTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EffectTarget::Environment => write!(f, "Environment"),
            EffectTarget::Layer(id) => write!(f, "Layer {}", id),
            EffectTarget::Clip { layer_id, slot } => write!(f, "Clip {}/{}", layer_id, slot),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parameter_value_conversions() {
        assert_eq!(ParameterValue::Float(0.5).as_f32(), 0.5);
        assert_eq!(ParameterValue::Int(42).as_i32(), 42);
        assert!(ParameterValue::Bool(true).as_bool());
        assert!(!ParameterValue::Bool(false).as_bool());
    }

    #[test]
    fn test_effect_stack_operations() {
        let mut stack = EffectStack::new();

        let id1 = stack.add("test1", "Test 1", vec![]);
        let id2 = stack.add("test2", "Test 2", vec![]);

        assert_eq!(stack.len(), 2);
        assert_eq!(stack.get(id1).map(|e| &e.name), Some(&"Test 1".to_string()));

        stack.remove(id1);
        assert_eq!(stack.len(), 1);
        assert!(stack.get(id1).is_none());
        assert!(stack.get(id2).is_some());
    }

    #[test]
    fn test_effect_stack_reorder() {
        let mut stack = EffectStack::new();

        let id1 = stack.add("test1", "Test 1", vec![]);
        let id2 = stack.add("test2", "Test 2", vec![]);
        let id3 = stack.add("test3", "Test 3", vec![]);

        // Move last to first
        stack.move_to(id3, 0);
        assert_eq!(stack.effects[0].id, id3);
        assert_eq!(stack.effects[1].id, id1);
        assert_eq!(stack.effects[2].id, id2);
    }

    #[test]
    fn test_effect_bypass_solo() {
        let mut stack = EffectStack::new();

        let id1 = stack.add("test1", "Test 1", vec![]);
        let id2 = stack.add("test2", "Test 2", vec![]);
        let id3 = stack.add("test3", "Test 3", vec![]);

        // All active by default
        assert_eq!(stack.active_effects().count(), 3);

        // Bypass one
        stack.get_mut(id2).unwrap().bypassed = true;
        assert_eq!(stack.active_effects().count(), 2);

        // Solo one
        stack.get_mut(id1).unwrap().soloed = true;
        assert_eq!(stack.active_effects().count(), 1);
        assert_eq!(stack.active_effects().next().unwrap().id, id1);
    }
}
