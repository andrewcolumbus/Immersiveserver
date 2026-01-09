//! Effect manager - coordinates effect processing in the render loop
//!
//! The EffectManager owns the effect registry, BPM clock, and per-layer
//! effect runtimes. It provides methods to process effects during rendering.

use std::collections::HashMap;
use std::time::Instant;

use super::automation::BpmClock;
use super::builtin::register_builtin_effects;
use super::runtime::EffectStackRuntime;
use super::traits::EffectParams;
use super::types::EffectStack;
use super::EffectRegistry;

/// Manages effect processing for the entire application
pub struct EffectManager {
    /// Registry of available effects
    registry: EffectRegistry,
    /// BPM clock for automation
    bpm_clock: BpmClock,
    /// Per-layer effect runtimes
    layer_runtimes: HashMap<u32, EffectStackRuntime>,
    /// Per-clip effect runtimes, keyed by (layer_id, slot_index)
    clip_runtimes: HashMap<(u32, usize), EffectStackRuntime>,
    /// Environment effect runtime
    environment_runtime: Option<EffectStackRuntime>,
    /// Preview clip effect runtime (separate from live clip runtimes)
    preview_runtime: Option<EffectStackRuntime>,
    /// Application start time for effect timing
    start_time: Instant,
    /// Current frame time
    time: f32,
    /// Delta time since last frame
    delta_time: f32,
    /// Last frame timestamp
    last_frame: Instant,
    /// Whether effects are globally enabled
    enabled: bool,
}

impl Default for EffectManager {
    fn default() -> Self {
        Self::new()
    }
}

impl EffectManager {
    /// Create a new effect manager with built-in effects registered
    pub fn new() -> Self {
        let mut registry = EffectRegistry::new();
        register_builtin_effects(&mut registry);

        Self {
            registry,
            bpm_clock: BpmClock::new(120.0),
            layer_runtimes: HashMap::new(),
            clip_runtimes: HashMap::new(),
            environment_runtime: None,
            preview_runtime: None,
            start_time: Instant::now(),
            time: 0.0,
            delta_time: 0.0,
            last_frame: Instant::now(),
            enabled: true,
        }
    }

    /// Get a reference to the effect registry
    pub fn registry(&self) -> &EffectRegistry {
        &self.registry
    }

    /// Get a mutable reference to the effect registry
    pub fn registry_mut(&mut self) -> &mut EffectRegistry {
        &mut self.registry
    }

    /// Get a reference to the BPM clock
    pub fn bpm_clock(&self) -> &BpmClock {
        &self.bpm_clock
    }

    /// Get a mutable reference to the BPM clock
    pub fn bpm_clock_mut(&mut self) -> &mut BpmClock {
        &mut self.bpm_clock
    }

    /// Check if effects are enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enable or disable effects globally
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Get the current effect time
    pub fn time(&self) -> f32 {
        self.time
    }

    /// Get the delta time since last frame
    pub fn delta_time(&self) -> f32 {
        self.delta_time
    }

    /// Update timing (call once per frame at start of render)
    pub fn update(&mut self) {
        let now = Instant::now();
        self.time = now.duration_since(self.start_time).as_secs_f32();
        self.delta_time = now.duration_since(self.last_frame).as_secs_f32();
        self.last_frame = now;

        // Update BPM clock
        self.bpm_clock.update();
    }

    // ========== Envelope Value Accessors for UI ==========

    /// Get the current FFT envelope value for a parameter (for UI display)
    ///
    /// Returns the smoothed FFT value after attack/release processing.
    /// Returns None if no envelope state exists for this parameter.
    pub fn get_fft_envelope_value(
        &self,
        layer_id: Option<u32>,
        clip_slot: Option<(u32, usize)>,
        effect_id: u32,
        param_name: &str,
    ) -> Option<f32> {
        if let Some((lid, slot)) = clip_slot {
            self.clip_runtimes
                .get(&(lid, slot))
                .and_then(|r| r.get_fft_envelope_value(effect_id, param_name))
        } else if let Some(lid) = layer_id {
            self.layer_runtimes
                .get(&lid)
                .and_then(|r| r.get_fft_envelope_value(effect_id, param_name))
        } else {
            self.environment_runtime
                .as_ref()
                .and_then(|r| r.get_fft_envelope_value(effect_id, param_name))
        }
    }

    /// Get the current Beat envelope value for a parameter (for UI display)
    ///
    /// Returns the ADSR envelope value (0-1).
    /// Returns None if no envelope state exists for this parameter.
    pub fn get_beat_envelope_value(
        &self,
        layer_id: Option<u32>,
        clip_slot: Option<(u32, usize)>,
        effect_id: u32,
        param_name: &str,
    ) -> Option<f32> {
        if let Some((lid, slot)) = clip_slot {
            self.clip_runtimes
                .get(&(lid, slot))
                .and_then(|r| r.get_beat_envelope_value(effect_id, param_name))
        } else if let Some(lid) = layer_id {
            self.layer_runtimes
                .get(&lid)
                .and_then(|r| r.get_beat_envelope_value(effect_id, param_name))
        } else {
            self.environment_runtime
                .as_ref()
                .and_then(|r| r.get_beat_envelope_value(effect_id, param_name))
        }
    }

    /// Get the current timeline envelope value for a parameter (for UI display)
    ///
    /// Looks up the timeline envelope state from the appropriate runtime (clip, layer, or environment).
    /// Returns the time-based ramp value (0-1).
    /// Returns None if no envelope state exists for this parameter.
    pub fn get_timeline_envelope_value(
        &self,
        layer_id: Option<u32>,
        clip_slot: Option<(u32, usize)>,
        effect_id: u32,
        param_name: &str,
    ) -> Option<f32> {
        if let Some((lid, slot)) = clip_slot {
            self.clip_runtimes
                .get(&(lid, slot))
                .and_then(|r| r.get_timeline_envelope_value(effect_id, param_name))
        } else if let Some(lid) = layer_id {
            self.layer_runtimes
                .get(&lid)
                .and_then(|r| r.get_timeline_envelope_value(effect_id, param_name))
        } else {
            self.environment_runtime
                .as_ref()
                .and_then(|r| r.get_timeline_envelope_value(effect_id, param_name))
        }
    }

    /// Initialize GPU resources for a layer's effects
    pub fn init_layer_effects(
        &mut self,
        layer_id: u32,
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) {
        let runtime = self.layer_runtimes.entry(layer_id).or_insert_with(EffectStackRuntime::new);
        runtime.init(device, width, height, format);
    }

    // ========== Clip Effect Methods ==========

    /// Initialize GPU resources for a clip's effects
    pub fn init_clip_effects(
        &mut self,
        layer_id: u32,
        slot: usize,
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) {
        let runtime = self
            .clip_runtimes
            .entry((layer_id, slot))
            .or_insert_with(EffectStackRuntime::new);
        runtime.init(device, width, height, format);
    }

    /// Sync clip effect runtimes with effect stacks
    pub fn sync_clip_effects(
        &mut self,
        layer_id: u32,
        slot: usize,
        stack: &EffectStack,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) {
        if let Some(runtime) = self.clip_runtimes.get_mut(&(layer_id, slot)) {
            runtime.sync_with_stack(stack, &self.registry, device, queue, format);
        }
    }

    /// Process effects for a clip (without automation)
    ///
    /// # Arguments
    /// * `encoder` - Command encoder
    /// * `device` - GPU device
    /// * `queue` - GPU queue
    /// * `layer_id` - Layer ID containing the clip
    /// * `slot` - Clip slot index
    /// * `input` - Input texture view (video content)
    /// * `output` - Output texture view (processed result)
    /// * `stack` - Effect stack for this clip
    ///
    /// # Returns
    /// `true` if effects were processed, `false` if no effects or disabled
    pub fn process_clip_effects(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layer_id: u32,
        slot: usize,
        input: &wgpu::TextureView,
        output: &wgpu::TextureView,
        stack: &EffectStack,
    ) -> bool {
        self.process_clip_effects_with_automation(encoder, device, queue, layer_id, slot, input, output, stack, None)
    }

    /// Process effects for a clip with automation support
    ///
    /// # Arguments
    /// * `encoder` - Command encoder
    /// * `device` - GPU device
    /// * `queue` - GPU queue
    /// * `layer_id` - Layer ID containing the clip
    /// * `slot` - Clip slot index
    /// * `input` - Input texture view (video content)
    /// * `output` - Output texture view (processed result)
    /// * `stack` - Effect stack for this clip
    /// * `audio_manager` - Audio manager for FFT automation
    ///
    /// # Returns
    /// `true` if effects were processed, `false` if no effects or disabled
    pub fn process_clip_effects_with_automation(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layer_id: u32,
        slot: usize,
        input: &wgpu::TextureView,
        output: &wgpu::TextureView,
        stack: &EffectStack,
        audio_manager: Option<&crate::audio::AudioManager>,
    ) -> bool {
        if !self.enabled || stack.is_empty() {
            return false;
        }

        // Check if any effects are active
        if stack.active_effects().count() == 0 {
            return false;
        }

        let params = self.build_params();

        if let Some(runtime) = self.clip_runtimes.get_mut(&(layer_id, slot)) {
            runtime.process_with_automation(encoder, queue, device, input, output, stack, &params, &self.bpm_clock, audio_manager);
            true
        } else {
            false
        }
    }

    /// Ensure clip runtime exists and is properly sized
    pub fn ensure_clip_runtime(
        &mut self,
        layer_id: u32,
        slot: usize,
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) {
        let runtime = self
            .clip_runtimes
            .entry((layer_id, slot))
            .or_insert_with(EffectStackRuntime::new);
        runtime.ensure_size(device, width, height, format);
    }

    /// Get a reference to a clip's effect stack runtime
    pub fn get_clip_runtime(&self, layer_id: u32, slot: usize) -> Option<&EffectStackRuntime> {
        self.clip_runtimes.get(&(layer_id, slot))
    }

    /// Get a mutable reference to a clip's effect stack runtime
    pub fn get_clip_runtime_mut(
        &mut self,
        layer_id: u32,
        slot: usize,
    ) -> Option<&mut EffectStackRuntime> {
        self.clip_runtimes.get_mut(&(layer_id, slot))
    }

    /// Remove effect runtime for a specific clip
    pub fn remove_clip_runtime(&mut self, layer_id: u32, slot: usize) {
        self.clip_runtimes.remove(&(layer_id, slot));
    }

    /// Remove all clip effect runtimes for a layer
    pub fn remove_layer_clip_runtimes(&mut self, layer_id: u32) {
        self.clip_runtimes
            .retain(|(lid, _slot), _runtime| *lid != layer_id);
    }

    // ========== Preview Effect Methods ==========

    /// Ensure preview runtime exists and is properly sized
    pub fn ensure_preview_runtime(
        &mut self,
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) {
        let runtime = self.preview_runtime.get_or_insert_with(EffectStackRuntime::new);
        runtime.ensure_size(device, width, height, format);
    }

    /// Get a mutable reference to the preview runtime
    pub fn get_preview_runtime_mut(&mut self) -> Option<&mut EffectStackRuntime> {
        self.preview_runtime.as_mut()
    }

    /// Get a reference to the preview runtime
    pub fn get_preview_runtime(&self) -> Option<&EffectStackRuntime> {
        self.preview_runtime.as_ref()
    }

    /// Sync preview effect runtime with an effect stack
    pub fn sync_preview_effects(
        &mut self,
        stack: &EffectStack,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) {
        if let Some(runtime) = self.preview_runtime.as_mut() {
            runtime.sync_with_stack(stack, &self.registry, device, queue, format);
        }
    }

    /// Clear the preview runtime
    pub fn clear_preview_runtime(&mut self) {
        self.preview_runtime = None;
    }

    // ========== Environment Effect Methods ==========

    /// Initialize GPU resources for environment effects
    pub fn init_environment_effects(
        &mut self,
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) {
        let runtime = self.environment_runtime.get_or_insert_with(EffectStackRuntime::new);
        runtime.init(device, width, height, format);
    }

    /// Sync layer effect runtimes with effect stacks
    pub fn sync_layer_effects(
        &mut self,
        layer_id: u32,
        stack: &EffectStack,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) {
        if let Some(runtime) = self.layer_runtimes.get_mut(&layer_id) {
            runtime.sync_with_stack(stack, &self.registry, device, queue, format);
        }
    }

    /// Sync environment effect runtime with effect stack
    pub fn sync_environment_effects(
        &mut self,
        stack: &EffectStack,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) {
        if let Some(runtime) = &mut self.environment_runtime {
            runtime.sync_with_stack(stack, &self.registry, device, queue, format);
        }
    }

    /// Build effect params for the current frame
    pub fn build_params(&self) -> EffectParams {
        EffectParams::new(
            self.time,
            self.delta_time,
            self.bpm_clock.beat_phase(),
            self.bpm_clock.bar_phase(),
        )
    }

    /// Process effects for a layer (without automation)
    ///
    /// # Arguments
    /// * `encoder` - Command encoder
    /// * `device` - GPU device
    /// * `queue` - GPU queue
    /// * `layer_id` - Layer ID
    /// * `input` - Input texture view (layer content)
    /// * `output` - Output texture view (processed result)
    /// * `stack` - Effect stack for this layer
    ///
    /// # Returns
    /// `true` if effects were processed, `false` if no effects or disabled
    pub fn process_layer_effects(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layer_id: u32,
        input: &wgpu::TextureView,
        output: &wgpu::TextureView,
        stack: &EffectStack,
    ) -> bool {
        self.process_layer_effects_with_automation(encoder, device, queue, layer_id, input, output, stack, None)
    }

    /// Process effects for a layer with automation support
    ///
    /// # Arguments
    /// * `encoder` - Command encoder
    /// * `device` - GPU device
    /// * `queue` - GPU queue
    /// * `layer_id` - Layer ID
    /// * `input` - Input texture view (layer content)
    /// * `output` - Output texture view (processed result)
    /// * `stack` - Effect stack for this layer
    /// * `audio_manager` - Audio manager for FFT automation
    ///
    /// # Returns
    /// `true` if effects were processed, `false` if no effects or disabled
    pub fn process_layer_effects_with_automation(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layer_id: u32,
        input: &wgpu::TextureView,
        output: &wgpu::TextureView,
        stack: &EffectStack,
        audio_manager: Option<&crate::audio::AudioManager>,
    ) -> bool {
        if !self.enabled || stack.is_empty() {
            return false;
        }

        // Check if any effects are active
        if stack.active_effects().count() == 0 {
            return false;
        }

        let params = self.build_params();

        if let Some(runtime) = self.layer_runtimes.get_mut(&layer_id) {
            runtime.process_with_automation(encoder, queue, device, input, output, stack, &params, &self.bpm_clock, audio_manager);
            true
        } else {
            false
        }
    }

    /// Process environment effects (master effects, without automation)
    ///
    /// # Arguments
    /// * `encoder` - Command encoder
    /// * `device` - GPU device
    /// * `queue` - GPU queue
    /// * `texture` - Environment texture view (both input and output)
    /// * `stack` - Environment effect stack
    ///
    /// # Returns
    /// `true` if effects were processed, `false` if no effects or disabled
    pub fn process_environment_effects(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture: &wgpu::TextureView,
        stack: &EffectStack,
    ) -> bool {
        self.process_environment_effects_with_automation(encoder, device, queue, texture, stack, None)
    }

    /// Process environment effects with automation support
    ///
    /// # Arguments
    /// * `encoder` - Command encoder
    /// * `device` - GPU device
    /// * `queue` - GPU queue
    /// * `texture` - Environment texture view (both input and output)
    /// * `stack` - Environment effect stack
    /// * `audio_manager` - Audio manager for FFT automation
    ///
    /// # Returns
    /// `true` if effects were processed, `false` if no effects or disabled
    pub fn process_environment_effects_with_automation(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture: &wgpu::TextureView,
        stack: &EffectStack,
        audio_manager: Option<&crate::audio::AudioManager>,
    ) -> bool {
        if !self.enabled || stack.is_empty() {
            return false;
        }

        // Check if any effects are active
        if stack.active_effects().count() == 0 {
            return false;
        }

        let params = self.build_params();

        if let Some(runtime) = &mut self.environment_runtime {
            // Use in-place processing to avoid texture read/write conflicts
            // (same texture cannot be both RESOURCE and COLOR_TARGET in same pass)
            runtime.process_in_place_with_automation(encoder, queue, device, texture, stack, &params, &self.bpm_clock, audio_manager);
            true
        } else {
            false
        }
    }

    /// Remove effect runtime for a layer
    pub fn remove_layer_runtime(&mut self, layer_id: u32) {
        self.layer_runtimes.remove(&layer_id);
    }

    /// Clear all effect runtimes
    pub fn clear(&mut self) {
        self.layer_runtimes.clear();
        self.clip_runtimes.clear();
        self.environment_runtime = None;
    }

    /// Check if a layer has effect runtime initialized
    pub fn has_layer_runtime(&self, layer_id: u32) -> bool {
        self.layer_runtimes.contains_key(&layer_id)
    }

    /// Ensure layer runtime exists and is properly sized
    pub fn ensure_layer_runtime(
        &mut self,
        layer_id: u32,
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) {
        let runtime = self.layer_runtimes.entry(layer_id).or_insert_with(EffectStackRuntime::new);
        runtime.ensure_size(device, width, height, format);
    }

    /// Get a reference to a layer's effect stack runtime
    pub fn get_layer_runtime(&self, layer_id: u32) -> Option<&EffectStackRuntime> {
        self.layer_runtimes.get(&layer_id)
    }

    /// Get a mutable reference to a layer's effect stack runtime
    pub fn get_layer_runtime_mut(&mut self, layer_id: u32) -> Option<&mut EffectStackRuntime> {
        self.layer_runtimes.get_mut(&layer_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effect_manager_new() {
        let manager = EffectManager::new();
        assert!(manager.is_enabled());
        assert!(manager.registry().len() > 0); // Built-in effects registered
    }

    #[test]
    fn test_effect_manager_timing() {
        let mut manager = EffectManager::new();
        std::thread::sleep(std::time::Duration::from_millis(10));
        manager.update();
        assert!(manager.time() > 0.0);
        assert!(manager.delta_time() > 0.0);
    }

    #[test]
    fn test_effect_manager_bpm() {
        let mut manager = EffectManager::new();
        assert_eq!(manager.bpm_clock().bpm(), 120.0);
        manager.bpm_clock_mut().set_bpm(140.0);
        assert_eq!(manager.bpm_clock().bpm(), 140.0);
    }
}
