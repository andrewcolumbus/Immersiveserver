//! Automation system for effect parameters
//!
//! Provides BPM clock, LFO generation, beat-triggered envelopes,
//! and FFT audio-reactive automation for effect parameters.

use std::time::Instant;

use super::types::{AutomationSource, BeatSource, BeatTrigger, FftSource, LfoShape, LfoSource, Parameter, TimelineSource, TimelineMode, TimelineDirection};
use crate::audio::AudioManager;

/// Global BPM clock for synchronizing automation
#[derive(Debug, Clone)]
pub struct BpmClock {
    /// Beats per minute
    bpm: f32,
    /// Beats per bar (time signature numerator)
    beats_per_bar: u32,
    /// Total beats elapsed since start/reset
    current_beat: f32,
    /// Phase within current beat (0.0-1.0)
    beat_phase: f32,
    /// Phase within current bar (0.0-1.0)
    bar_phase: f32,
    /// When the clock was last updated
    last_update: Instant,
    /// Whether the clock is running
    running: bool,
    /// Tap tempo samples for averaging
    tap_times: Vec<Instant>,
}

impl Default for BpmClock {
    fn default() -> Self {
        Self::new(120.0)
    }
}

impl BpmClock {
    /// Create a new BPM clock
    pub fn new(bpm: f32) -> Self {
        Self {
            bpm: bpm.clamp(20.0, 300.0),
            beats_per_bar: 4,
            current_beat: 0.0,
            beat_phase: 0.0,
            bar_phase: 0.0,
            last_update: Instant::now(),
            running: true,
            tap_times: Vec::with_capacity(8),
        }
    }

    /// Update the clock (call once per frame)
    pub fn update(&mut self) {
        if !self.running {
            self.last_update = Instant::now();
            return;
        }

        let now = Instant::now();
        let delta = now.duration_since(self.last_update).as_secs_f32();
        self.last_update = now;

        // Calculate beats elapsed
        let beats_per_second = self.bpm / 60.0;
        let beats_delta = delta * beats_per_second;

        self.current_beat += beats_delta;
        self.beat_phase = self.current_beat.fract();
        self.bar_phase = (self.current_beat / self.beats_per_bar as f32).fract();
    }

    /// Get the current BPM
    pub fn bpm(&self) -> f32 {
        self.bpm
    }

    /// Set the BPM
    pub fn set_bpm(&mut self, bpm: f32) {
        self.bpm = bpm.clamp(20.0, 300.0);
    }

    /// Get beats per bar
    pub fn beats_per_bar(&self) -> u32 {
        self.beats_per_bar
    }

    /// Set beats per bar
    pub fn set_beats_per_bar(&mut self, beats: u32) {
        self.beats_per_bar = beats.clamp(1, 16);
    }

    /// Get the total beats elapsed
    pub fn current_beat(&self) -> f32 {
        self.current_beat
    }

    /// Get the phase within the current beat (0.0-1.0)
    pub fn beat_phase(&self) -> f32 {
        self.beat_phase
    }

    /// Get the phase within the current bar (0.0-1.0)
    pub fn bar_phase(&self) -> f32 {
        self.bar_phase
    }

    /// Check if the clock is running
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Start/resume the clock
    pub fn start(&mut self) {
        self.running = true;
        self.last_update = Instant::now();
    }

    /// Stop/pause the clock
    pub fn stop(&mut self) {
        self.running = false;
    }

    /// Toggle running state
    pub fn toggle(&mut self) {
        if self.running {
            self.stop();
        } else {
            self.start();
        }
    }

    /// Reset the clock to beat 0
    pub fn reset(&mut self) {
        self.current_beat = 0.0;
        self.beat_phase = 0.0;
        self.bar_phase = 0.0;
        self.last_update = Instant::now();
    }

    /// Tap tempo - call repeatedly to set BPM from taps
    pub fn tap(&mut self) {
        let now = Instant::now();

        // Remove old taps (older than 2 seconds)
        self.tap_times.retain(|t| now.duration_since(*t).as_secs_f32() < 2.0);

        self.tap_times.push(now);

        // Need at least 2 taps to calculate BPM
        if self.tap_times.len() >= 2 {
            let mut total_interval = 0.0;
            for i in 1..self.tap_times.len() {
                total_interval += self.tap_times[i]
                    .duration_since(self.tap_times[i - 1])
                    .as_secs_f32();
            }
            let avg_interval = total_interval / (self.tap_times.len() - 1) as f32;
            let new_bpm = 60.0 / avg_interval;
            self.set_bpm(new_bpm);
        }

        // Keep only last 8 taps
        if self.tap_times.len() > 8 {
            self.tap_times.remove(0);
        }
    }

    /// Nudge the beat phase forward (for manual sync)
    pub fn nudge_forward(&mut self, amount: f32) {
        self.current_beat += amount;
        self.beat_phase = self.current_beat.fract();
        self.bar_phase = (self.current_beat / self.beats_per_bar as f32).fract();
    }

    /// Nudge the beat phase backward (for manual sync)
    pub fn nudge_backward(&mut self, amount: f32) {
        self.current_beat = (self.current_beat - amount).max(0.0);
        self.beat_phase = self.current_beat.fract();
        self.bar_phase = (self.current_beat / self.beats_per_bar as f32).fract();
    }

    /// Resync to the start of the current bar
    pub fn resync_to_bar(&mut self) {
        let bars = (self.current_beat / self.beats_per_bar as f32).floor();
        self.current_beat = bars * self.beats_per_bar as f32;
        self.beat_phase = 0.0;
        self.bar_phase = 0.0;
    }
}

/// Evaluate an LFO source
impl LfoSource {
    /// Evaluate the LFO at the current time
    ///
    /// Returns a value in the range [offset - amplitude, offset + amplitude]
    pub fn evaluate(&self, clock: &BpmClock, time: f32) -> f32 {
        let phase = if self.sync_to_bpm {
            // Phase based on beat count
            let beats_per_cycle = self.beats.max(0.001);
            ((clock.current_beat() / beats_per_cycle) + self.phase).fract()
        } else {
            // Phase based on time
            ((time * self.frequency) + self.phase).fract()
        };

        let wave = self.evaluate_waveform(phase);

        // Map from 0-1 to offset +/- amplitude
        self.offset + (wave - 0.5) * 2.0 * self.amplitude
    }

    /// Evaluate the waveform at a given phase (0-1)
    /// Returns a value in the range [0, 1]
    fn evaluate_waveform(&self, phase: f32) -> f32 {
        match self.shape {
            LfoShape::Sine => {
                // Sine wave: 0.5 + 0.5 * sin(2π * phase)
                0.5 + 0.5 * (phase * std::f32::consts::TAU).sin()
            }
            LfoShape::Triangle => {
                // Triangle wave: ramps up then down
                if phase < 0.5 {
                    phase * 2.0
                } else {
                    1.0 - (phase - 0.5) * 2.0
                }
            }
            LfoShape::Square => {
                // Square wave: 0 or 1
                if phase < 0.5 { 1.0 } else { 0.0 }
            }
            LfoShape::Sawtooth => {
                // Sawtooth: ramps up
                phase
            }
            LfoShape::SawtoothReverse => {
                // Reverse sawtooth: ramps down
                1.0 - phase
            }
            LfoShape::Random => {
                // Random: use phase as seed for deterministic randomness
                // This gives a stepped random value that changes each cycle
                let seed = (phase * 1000.0) as u32;
                let hash = Self::simple_hash(seed);
                (hash as f32) / (u32::MAX as f32)
            }
        }
    }

    /// Simple hash function for deterministic random
    fn simple_hash(mut x: u32) -> u32 {
        x = ((x >> 16) ^ x).wrapping_mul(0x45d9f3b);
        x = ((x >> 16) ^ x).wrapping_mul(0x45d9f3b);
        (x >> 16) ^ x
    }
}

/// Beat-triggered envelope state
#[derive(Debug, Clone)]
pub struct BeatEnvelopeState {
    /// Current envelope value (0-1)
    value: f32,
    /// Current phase (attack, decay, sustain, release)
    phase: EnvelopePhase,
    /// Time since phase started (in seconds)
    phase_time: f32,
    /// Last beat/bar number when triggered
    last_trigger: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnvelopePhase {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

impl Default for BeatEnvelopeState {
    fn default() -> Self {
        Self {
            value: 0.0,
            phase: EnvelopePhase::Idle,
            phase_time: 0.0,
            last_trigger: 0,
        }
    }
}

impl BeatEnvelopeState {
    /// Update the envelope state
    pub fn update(&mut self, source: &BeatSource, clock: &BpmClock, delta_time: f32) {
        // Check for trigger
        let trigger_beat = match source.trigger_on {
            BeatTrigger::Beat => clock.current_beat().floor() as u32,
            BeatTrigger::Bar => (clock.current_beat() / clock.beats_per_bar() as f32).floor() as u32,
            BeatTrigger::TwoBars => (clock.current_beat() / (clock.beats_per_bar() * 2) as f32).floor() as u32,
            BeatTrigger::FourBars => (clock.current_beat() / (clock.beats_per_bar() * 4) as f32).floor() as u32,
        };

        if trigger_beat > self.last_trigger {
            // Trigger the envelope
            self.last_trigger = trigger_beat;
            self.phase = EnvelopePhase::Attack;
            self.phase_time = 0.0;
        }

        // Update envelope based on phase
        self.phase_time += delta_time;

        let attack_time = source.attack_ms / 1000.0;
        let decay_time = source.decay_ms / 1000.0;
        let release_time = source.release_ms / 1000.0;

        match self.phase {
            EnvelopePhase::Idle => {
                self.value = 0.0;
            }
            EnvelopePhase::Attack => {
                if attack_time > 0.0 {
                    self.value = (self.phase_time / attack_time).min(1.0);
                } else {
                    self.value = 1.0;
                }
                if self.phase_time >= attack_time {
                    self.phase = EnvelopePhase::Decay;
                    self.phase_time = 0.0;
                }
            }
            EnvelopePhase::Decay => {
                if decay_time > 0.0 {
                    let decay_progress = (self.phase_time / decay_time).min(1.0);
                    self.value = 1.0 - (1.0 - source.sustain) * decay_progress;
                } else {
                    self.value = source.sustain;
                }
                if self.phase_time >= decay_time {
                    self.phase = EnvelopePhase::Sustain;
                    self.phase_time = 0.0;
                }
            }
            EnvelopePhase::Sustain => {
                self.value = source.sustain;
                // Sustain until next trigger or release
                // For now, auto-release after sustain
                self.phase = EnvelopePhase::Release;
                self.phase_time = 0.0;
            }
            EnvelopePhase::Release => {
                if release_time > 0.0 {
                    let release_progress = (self.phase_time / release_time).min(1.0);
                    self.value = source.sustain * (1.0 - release_progress);
                } else {
                    self.value = 0.0;
                }
                if self.phase_time >= release_time {
                    self.phase = EnvelopePhase::Idle;
                    self.value = 0.0;
                }
            }
        }
    }

    /// Get the current envelope value (0-1)
    pub fn value(&self) -> f32 {
        self.value
    }
}

/// FFT envelope state for attack/release smoothing
#[derive(Debug, Clone, Default)]
pub struct FftEnvelopeState {
    /// Current smoothed value (0-1)
    current_value: f32,
    /// Target value from FFT
    target_value: f32,
}

impl FftEnvelopeState {
    /// Create new FFT envelope state
    pub fn new() -> Self {
        Self::default()
    }

    /// Update the envelope with new FFT value
    pub fn update(&mut self, source: &FftSource, fft_value: f32, delta_time: f32) {
        // Apply gain
        self.target_value = (fft_value * source.gain).min(1.0);

        // Apply attack/release envelope
        let time_constant = if self.target_value > self.current_value {
            source.attack_ms / 1000.0
        } else {
            source.release_ms / 1000.0
        };

        if time_constant > 0.001 {
            // Exponential smoothing
            let factor = 1.0 - (-delta_time / time_constant).exp();
            self.current_value += (self.target_value - self.current_value) * factor;
        } else {
            self.current_value = self.target_value;
        }

        // Clamp to valid range
        self.current_value = self.current_value.clamp(0.0, 1.0);
    }

    /// Get the current envelope value (0-1)
    pub fn value(&self) -> f32 {
        self.current_value
    }

    /// Reset the envelope state
    pub fn reset(&mut self) {
        self.current_value = 0.0;
        self.target_value = 0.0;
    }
}

/// Timeline envelope state for time-based ramps
#[derive(Debug, Clone)]
pub struct TimelineEnvelopeState {
    /// Start time of the timeline (set on first evaluation)
    start_time: Option<Instant>,
    /// Current normalized value (0.0-1.0)
    current_value: f32,
    /// Whether the timeline has completed (for PlayOnceAndHold)
    completed: bool,
}

impl Default for TimelineEnvelopeState {
    fn default() -> Self {
        Self {
            start_time: None,
            current_value: 0.0,
            completed: false,
        }
    }
}

impl TimelineEnvelopeState {
    /// Create new timeline envelope state
    pub fn new() -> Self {
        Self::default()
    }

    /// Update the timeline envelope
    pub fn update(&mut self, source: &TimelineSource) {
        let now = Instant::now();

        // Initialize start time on first update
        let start = self.start_time.get_or_insert(now);

        // If already completed in PlayOnceAndHold mode, keep the final value
        if self.completed {
            return;
        }

        let elapsed_ms = now.duration_since(*start).as_secs_f32() * 1000.0;
        let duration = source.duration_ms.max(1.0);

        let progress = match source.mode {
            TimelineMode::Loop => (elapsed_ms % duration) / duration,
            TimelineMode::PlayOnceAndHold => {
                let p = (elapsed_ms / duration).min(1.0);
                if p >= 1.0 {
                    self.completed = true;
                }
                p
            }
        };

        // Apply easing
        let eased = source.easing.apply(progress);

        // Apply direction
        self.current_value = match source.direction {
            TimelineDirection::RampUp => eased,
            TimelineDirection::RampDown => 1.0 - eased,
        };
    }

    /// Get the current envelope value (0-1)
    pub fn value(&self) -> f32 {
        self.current_value
    }

    /// Reset the timeline (restart from beginning)
    pub fn reset(&mut self) {
        self.start_time = None;
        self.current_value = 0.0;
        self.completed = false;
    }

    /// Check if the timeline has completed (only relevant for PlayOnceAndHold)
    pub fn is_completed(&self) -> bool {
        self.completed
    }
}

/// Evaluate automation for a parameter
///
/// Returns the automated value, or the base value if no automation
pub fn evaluate_parameter(
    param: &Parameter,
    clock: &BpmClock,
    time: f32,
    envelope_state: Option<&mut BeatEnvelopeState>,
    delta_time: f32,
) -> f32 {
    let base_value = param.value.as_f32();

    match &param.automation {
        None => base_value,
        Some(AutomationSource::Lfo(lfo)) => {
            let lfo_value = lfo.evaluate(clock, time);
            // Modulate around base value
            let min = param.meta.min.unwrap_or(0.0);
            let max = param.meta.max.unwrap_or(1.0);
            let range = max - min;
            (base_value + lfo_value * range * 0.5).clamp(min, max)
        }
        Some(AutomationSource::Beat(beat)) => {
            if let Some(state) = envelope_state {
                state.update(beat, clock, delta_time);
                let min = param.meta.min.unwrap_or(0.0);
                let max = param.meta.max.unwrap_or(1.0);
                // Envelope modulates from min to max
                min + state.value() * (max - min)
            } else {
                base_value
            }
        }
        Some(AutomationSource::Fft(_)) => {
            // FFT automation requires audio manager - use evaluate_parameter_with_audio
            base_value
        }
        Some(AutomationSource::Timeline(_)) => {
            // Timeline automation requires timeline envelope state
            base_value
        }
    }
}

/// Evaluate automation for a parameter with audio manager support
///
/// This variant supports FFT audio automation in addition to LFO and Beat
pub fn evaluate_parameter_with_audio(
    param: &Parameter,
    clock: &BpmClock,
    time: f32,
    beat_envelope_state: Option<&mut BeatEnvelopeState>,
    fft_envelope_state: Option<&mut FftEnvelopeState>,
    audio_manager: Option<&AudioManager>,
    delta_time: f32,
) -> f32 {
    let base_value = param.value.as_f32();

    match &param.automation {
        None => base_value,
        Some(AutomationSource::Lfo(lfo)) => {
            let lfo_value = lfo.evaluate(clock, time);
            // Modulate around base value
            let min = param.meta.min.unwrap_or(0.0);
            let max = param.meta.max.unwrap_or(1.0);
            let range = max - min;
            (base_value + lfo_value * range * 0.5).clamp(min, max)
        }
        Some(AutomationSource::Beat(beat)) => {
            if let Some(state) = beat_envelope_state {
                state.update(beat, clock, delta_time);
                let min = param.meta.min.unwrap_or(0.0);
                let max = param.meta.max.unwrap_or(1.0);
                // Envelope modulates from min to max
                min + state.value() * (max - min)
            } else {
                base_value
            }
        }
        Some(AutomationSource::Fft(fft)) => {
            if let (Some(state), Some(manager)) = (fft_envelope_state, audio_manager) {
                // Get raw FFT value from audio manager
                let raw_value = manager.get_band_value(fft.band);

                // Update envelope with smoothing
                state.update(fft, raw_value, delta_time);

                // Map envelope value to parameter range
                let min = param.meta.min.unwrap_or(0.0);
                let max = param.meta.max.unwrap_or(1.0);
                min + state.value() * (max - min)
            } else {
                base_value
            }
        }
        Some(AutomationSource::Timeline(_)) => {
            // Timeline automation requires timeline envelope state
            // Use pack_parameters_with_automation_and_envelopes for full support
            base_value
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bpm_clock_new() {
        let clock = BpmClock::new(120.0);
        assert_eq!(clock.bpm(), 120.0);
        assert_eq!(clock.beats_per_bar(), 4);
        assert!(clock.is_running());
    }

    #[test]
    fn test_bpm_clock_set_bpm() {
        let mut clock = BpmClock::new(120.0);
        clock.set_bpm(140.0);
        assert_eq!(clock.bpm(), 140.0);

        // Test clamping
        clock.set_bpm(10.0);
        assert_eq!(clock.bpm(), 20.0);
        clock.set_bpm(400.0);
        assert_eq!(clock.bpm(), 300.0);
    }

    #[test]
    fn test_lfo_sine() {
        let lfo = LfoSource {
            shape: LfoShape::Sine,
            frequency: 1.0,
            phase: 0.0,
            amplitude: 1.0,
            offset: 0.0,
            sync_to_bpm: false,
            beats: 4.0,
        };

        let clock = BpmClock::new(120.0);

        // At phase 0, sine should be at 0.5, so output = 0 + (0.5 - 0.5) * 2 * 1 = 0
        let val = lfo.evaluate(&clock, 0.0);
        assert!((val - 0.0).abs() < 0.01);

        // At phase 0.25 (quarter cycle), sine should be at 1.0, so output = 0 + (1 - 0.5) * 2 * 1 = 1
        let val = lfo.evaluate(&clock, 0.25);
        assert!((val - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_lfo_triangle() {
        let lfo = LfoSource {
            shape: LfoShape::Triangle,
            frequency: 1.0,
            phase: 0.0,
            amplitude: 0.5,
            offset: 0.5,
            sync_to_bpm: false,
            beats: 4.0,
        };

        let clock = BpmClock::new(120.0);

        // At phase 0, triangle = 0, so output = 0.5 + (0 - 0.5) * 2 * 0.5 = 0
        let val = lfo.evaluate(&clock, 0.0);
        assert!((val - 0.0).abs() < 0.01);

        // At phase 0.5, triangle = 1, so output = 0.5 + (1 - 0.5) * 2 * 0.5 = 1
        let val = lfo.evaluate(&clock, 0.5);
        assert!((val - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_beat_envelope() {
        let source = BeatSource {
            trigger_on: BeatTrigger::Beat,
            attack_ms: 10.0,
            decay_ms: 50.0,
            sustain: 0.5,
            release_ms: 100.0,
        };

        let mut state = BeatEnvelopeState::default();
        let mut clock = BpmClock::new(120.0);

        // Initial state
        assert_eq!(state.value(), 0.0);

        // Advance clock past first beat
        clock.current_beat = 1.0;
        state.update(&source, &clock, 0.005);

        // Should be in attack phase
        assert!(state.value() > 0.0);
    }
}
