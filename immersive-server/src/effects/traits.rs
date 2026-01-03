//! Effect traits and runtime interfaces
//!
//! This module defines the core traits for implementing effects:
//! - `EffectDefinition` - Factory trait for creating effect instances
//! - `GpuEffectRuntime` - Runtime trait for GPU shader-based effects
//! - `CpuEffectRuntime` - Runtime trait for CPU-based effects

use super::{Parameter, ParameterMeta, ParameterValue, EffectInstance};

/// The type of effect processor
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectProcessor {
    /// GPU shader-based effect (fast, runs on GPU)
    Gpu,
    /// CPU-based effect (more flexible, runs on background thread)
    Cpu,
}

/// Uniform data passed to effect shaders
///
/// This struct is laid out for efficient GPU transfer.
/// The first 4 floats are timing/beat info, followed by 28 parameter slots.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct EffectParams {
    /// Time in seconds since effect started
    pub time: f32,
    /// Delta time since last frame
    pub delta_time: f32,
    /// Phase within current beat (0.0-1.0)
    pub beat_phase: f32,
    /// Phase within current bar (0.0-1.0)
    pub bar_phase: f32,
    /// Parameter values (up to 28 floats = 7 vec4s)
    /// Parameters are packed: floats directly, bools as 0/1, colors as 4 consecutive values
    pub params: [f32; 28],
}

impl Default for EffectParams {
    fn default() -> Self {
        Self {
            time: 0.0,
            delta_time: 0.0,
            beat_phase: 0.0,
            bar_phase: 0.0,
            params: [0.0; 28],
        }
    }
}

impl EffectParams {
    /// Create new effect params with timing info
    pub fn new(time: f32, delta_time: f32, beat_phase: f32, bar_phase: f32) -> Self {
        Self {
            time,
            delta_time,
            beat_phase,
            bar_phase,
            params: [0.0; 28],
        }
    }

    /// Set a float parameter at the given index
    pub fn set_float(&mut self, index: usize, value: f32) {
        if index < self.params.len() {
            self.params[index] = value;
        }
    }

    /// Set a bool parameter at the given index (as 0.0 or 1.0)
    pub fn set_bool(&mut self, index: usize, value: bool) {
        if index < self.params.len() {
            self.params[index] = if value { 1.0 } else { 0.0 };
        }
    }

    /// Set a vec2 parameter starting at the given index
    pub fn set_vec2(&mut self, index: usize, value: [f32; 2]) {
        if index + 1 < self.params.len() {
            self.params[index] = value[0];
            self.params[index + 1] = value[1];
        }
    }

    /// Set a vec3 parameter starting at the given index
    pub fn set_vec3(&mut self, index: usize, value: [f32; 3]) {
        if index + 2 < self.params.len() {
            self.params[index] = value[0];
            self.params[index + 1] = value[1];
            self.params[index + 2] = value[2];
        }
    }

    /// Set a color (vec4) parameter starting at the given index
    pub fn set_color(&mut self, index: usize, value: [f32; 4]) {
        if index + 3 < self.params.len() {
            self.params[index] = value[0];
            self.params[index + 1] = value[1];
            self.params[index + 2] = value[2];
            self.params[index + 3] = value[3];
        }
    }

    /// Pack parameters from a list of Parameter structs
    /// Returns the number of floats used
    pub fn pack_parameters(&mut self, parameters: &[Parameter]) -> usize {
        let mut offset = 0;
        for param in parameters {
            match &param.value {
                ParameterValue::Float(v) => {
                    self.set_float(offset, *v);
                    offset += 1;
                }
                ParameterValue::Int(v) => {
                    self.set_float(offset, *v as f32);
                    offset += 1;
                }
                ParameterValue::Bool(v) => {
                    self.set_bool(offset, *v);
                    offset += 1;
                }
                ParameterValue::Vec2(v) => {
                    self.set_vec2(offset, *v);
                    offset += 2;
                }
                ParameterValue::Vec3(v) => {
                    self.set_vec3(offset, *v);
                    offset += 3;
                }
                ParameterValue::Color(v) => {
                    self.set_color(offset, *v);
                    offset += 4;
                }
                ParameterValue::Enum { index, .. } => {
                    self.set_float(offset, *index as f32);
                    offset += 1;
                }
                ParameterValue::String(_) => {
                    // String parameters are not passed to shaders
                }
            }
            if offset >= self.params.len() {
                break;
            }
        }
        offset
    }

    /// Pack parameters with automation evaluation
    ///
    /// For parameters with automation (LFO, FFT, etc.), evaluates the automation
    /// and uses the modulated value. Otherwise uses the static value.
    ///
    /// # Arguments
    /// * `parameters` - List of parameters to pack
    /// * `clock` - BPM clock for LFO/Beat automation
    /// * `audio_manager` - Audio manager for FFT automation (can be None if no FFT used)
    pub fn pack_parameters_with_automation(
        &mut self,
        parameters: &[Parameter],
        clock: &super::automation::BpmClock,
        audio_manager: Option<&crate::audio::AudioManager>,
    ) -> usize {
        let mut offset = 0;
        for param in parameters {
            // Get the evaluated value (with automation if present)
            let evaluated = self.evaluate_param_value(param, clock, audio_manager);

            match &param.value {
                ParameterValue::Float(_) => {
                    self.set_float(offset, evaluated);
                    offset += 1;
                }
                ParameterValue::Int(_) => {
                    self.set_float(offset, evaluated);
                    offset += 1;
                }
                ParameterValue::Bool(_) => {
                    // For bool, automation returns 0.0-1.0, threshold at 0.5
                    self.set_bool(offset, evaluated > 0.5);
                    offset += 1;
                }
                ParameterValue::Vec2(v) => {
                    // Vec2 automation modulates first component only for now
                    self.set_vec2(offset, [evaluated, v[1]]);
                    offset += 2;
                }
                ParameterValue::Vec3(v) => {
                    // Vec3 automation modulates first component only for now
                    self.set_vec3(offset, [evaluated, v[1], v[2]]);
                    offset += 3;
                }
                ParameterValue::Color(v) => {
                    // Color automation modulates all RGB equally (brightness)
                    if param.automation.is_some() {
                        let base = param.value.as_f32();
                        let factor = if base > 0.0 { evaluated / base } else { evaluated };
                        self.set_color(offset, [
                            (v[0] * factor).min(1.0),
                            (v[1] * factor).min(1.0),
                            (v[2] * factor).min(1.0),
                            v[3],
                        ]);
                    } else {
                        self.set_color(offset, *v);
                    }
                    offset += 4;
                }
                ParameterValue::Enum { .. } => {
                    self.set_float(offset, evaluated);
                    offset += 1;
                }
                ParameterValue::String(_) => {
                    // String parameters are not passed to shaders
                }
            }
            if offset >= self.params.len() {
                break;
            }
        }
        offset
    }

    /// Evaluate a single parameter's value with automation
    fn evaluate_param_value(
        &self,
        param: &Parameter,
        clock: &super::automation::BpmClock,
        audio_manager: Option<&crate::audio::AudioManager>,
    ) -> f32 {
        use super::types::AutomationSource;

        let base_value = param.value.as_f32();
        let min = param.meta.min.unwrap_or(0.0);
        let max = param.meta.max.unwrap_or(1.0);
        let range = max - min;

        match &param.automation {
            None => base_value,
            Some(AutomationSource::Lfo(lfo)) => {
                // Evaluate LFO and modulate around base value
                let lfo_value = lfo.evaluate(clock, self.time);
                (base_value + lfo_value * range * 0.5).clamp(min, max)
            }
            Some(AutomationSource::Beat(_beat)) => {
                // Beat automation needs envelope state tracking
                // For now, return base value (full envelope support requires state)
                base_value
            }
            Some(AutomationSource::Fft(fft)) => {
                if let Some(manager) = audio_manager {
                    // Get raw FFT value with gain applied
                    let raw = manager.get_band_value(fft.band);
                    let gained = (raw * fft.gain).min(1.0);

                    // Map FFT value to parameter range
                    min + gained * range
                } else {
                    base_value
                }
            }
        }
    }
}

/// Trait for effect definitions (factory pattern)
///
/// Each effect type implements this trait to provide metadata and
/// create runtime instances. Effects are registered with the
/// `EffectRegistry` at startup.
pub trait EffectDefinition: Send + Sync {
    /// Unique identifier for this effect type (e.g., "color_correction", "invert")
    fn effect_type(&self) -> &'static str;

    /// Human-readable display name (e.g., "Color Correction", "Invert")
    fn display_name(&self) -> &'static str;

    /// Category for UI grouping (e.g., "Color", "Distort", "Blur", "Generate")
    fn category(&self) -> &'static str;

    /// Processor type (GPU or CPU)
    fn processor(&self) -> EffectProcessor;

    /// Get the default parameters for this effect
    fn default_parameters(&self) -> Vec<Parameter>;

    /// Create a GPU runtime instance for this effect
    /// Returns None if this is a CPU-only effect
    fn create_gpu_runtime(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        output_format: wgpu::TextureFormat,
    ) -> Option<Box<dyn GpuEffectRuntime>>;

    /// Create a CPU runtime instance for this effect
    /// Returns None if this is a GPU-only effect
    fn create_cpu_runtime(&self) -> Option<Box<dyn CpuEffectRuntime>>;
}

/// Runtime trait for GPU shader-based effects
///
/// Implementations process frames using wgpu render pipelines.
/// The effect reads from an input texture and writes to an output texture.
pub trait GpuEffectRuntime: Send {
    /// Process a frame through the effect
    ///
    /// # Arguments
    /// * `encoder` - Command encoder for recording GPU commands
    /// * `device` - GPU device for resource creation
    /// * `input` - Input texture view to read from
    /// * `output` - Output texture view to write to
    /// * `params` - Effect parameters packed for GPU
    /// * `queue` - GPU queue for buffer writes
    fn process(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
        input: &wgpu::TextureView,
        output: &wgpu::TextureView,
        params: &EffectParams,
        queue: &wgpu::Queue,
    );

    /// Rebuild the pipeline (for hot-reload support)
    ///
    /// # Arguments
    /// * `device` - GPU device
    /// * `shader_source` - New WGSL shader source
    fn rebuild(&mut self, device: &wgpu::Device, shader_source: &str) -> Result<(), String>;

    /// Get the effect type identifier
    fn effect_type(&self) -> &'static str;

    /// Update runtime state from effect instance parameters.
    ///
    /// Called before process() to allow effects to extract non-numeric parameters
    /// like strings that aren't packed into EffectParams.
    ///
    /// Default implementation is a no-op.
    fn update_from_instance(
        &mut self,
        _instance: &EffectInstance,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
    ) {
        // Default: no-op
    }
}

/// Runtime trait for CPU-based effects
///
/// Implementations process frames on the CPU, typically on a background thread.
/// The effect reads from an input buffer and writes to an output buffer.
pub trait CpuEffectRuntime: Send {
    /// Process a frame through the effect
    ///
    /// # Arguments
    /// * `input` - Input pixel data (RGBA8 format)
    /// * `output` - Output pixel data buffer (RGBA8 format, same size as input)
    /// * `width` - Frame width in pixels
    /// * `height` - Frame height in pixels
    /// * `params` - Effect parameters
    fn process(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        width: u32,
        height: u32,
        params: &EffectParams,
    );

    /// Get the effect type identifier
    fn effect_type(&self) -> &'static str;
}

/// Helper to create parameter metadata for common effect parameters
pub struct ParamBuilder;

impl ParamBuilder {
    /// Create a brightness parameter (-1 to 1, default 0)
    pub fn brightness() -> ParameterMeta {
        ParameterMeta::float("brightness", "Brightness", 0.0, -1.0, 1.0)
    }

    /// Create a contrast parameter (0 to 2, default 1)
    pub fn contrast() -> ParameterMeta {
        ParameterMeta::float("contrast", "Contrast", 1.0, 0.0, 2.0)
    }

    /// Create a saturation parameter (0 to 2, default 1)
    pub fn saturation() -> ParameterMeta {
        ParameterMeta::float("saturation", "Saturation", 1.0, 0.0, 2.0)
    }

    /// Create a hue shift parameter (0 to 1, default 0)
    pub fn hue_shift() -> ParameterMeta {
        ParameterMeta::float("hue_shift", "Hue Shift", 0.0, 0.0, 1.0)
    }

    /// Create a gamma parameter (0.1 to 3, default 1)
    pub fn gamma() -> ParameterMeta {
        ParameterMeta::float("gamma", "Gamma", 1.0, 0.1, 3.0)
    }

    /// Create an amount/mix parameter (0 to 1, default 1)
    pub fn amount() -> ParameterMeta {
        ParameterMeta::float("amount", "Amount", 1.0, 0.0, 1.0)
    }

    /// Create a generic 0-1 parameter
    pub fn normalized(name: &str, label: &str, default: f32) -> ParameterMeta {
        ParameterMeta::float(name, label, default, 0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effect_params_default() {
        let params = EffectParams::default();
        assert_eq!(params.time, 0.0);
        assert_eq!(params.beat_phase, 0.0);
        assert!(params.params.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn test_effect_params_set_values() {
        let mut params = EffectParams::default();
        params.set_float(0, 0.5);
        params.set_bool(1, true);
        params.set_vec2(2, [1.0, 2.0]);
        params.set_color(4, [0.1, 0.2, 0.3, 1.0]);

        assert_eq!(params.params[0], 0.5);
        assert_eq!(params.params[1], 1.0);
        assert_eq!(params.params[2], 1.0);
        assert_eq!(params.params[3], 2.0);
        assert_eq!(params.params[4], 0.1);
        assert_eq!(params.params[5], 0.2);
        assert_eq!(params.params[6], 0.3);
        assert_eq!(params.params[7], 1.0);
    }

    #[test]
    fn test_effect_params_pack() {
        let parameters = vec![
            Parameter::new(ParameterMeta::float("a", "A", 0.5, 0.0, 1.0)),
            Parameter::new(ParameterMeta::bool("b", "B", true)),
        ];

        let mut params = EffectParams::default();
        let used = params.pack_parameters(&parameters);

        assert_eq!(used, 2);
        assert_eq!(params.params[0], 0.5);
        assert_eq!(params.params[1], 1.0);
    }
}
