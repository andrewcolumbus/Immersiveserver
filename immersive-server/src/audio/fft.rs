//! FFT analysis for audio frequency band extraction

use super::types::{AudioBand, AudioBuffer, FftData};
use rustfft::{num_complex::Complex, FftPlanner};
use std::time::Instant;

/// FFT window size (power of 2 for efficiency)
const FFT_SIZE: usize = 2048;

/// Default smoothing factor (0 = no smoothing, 1 = full smoothing)
const DEFAULT_SMOOTHING: f32 = 0.0;

/// FFT analyzer with frequency band extraction
pub struct FftAnalyzer {
    /// FFT planner (reusable)
    planner: FftPlanner<f32>,
    /// FFT input buffer
    input_buffer: Vec<Complex<f32>>,
    /// Hann window coefficients
    window: Vec<f32>,
    /// Sample rate for frequency calculations
    sample_rate: u32,
    /// Previous FFT data for smoothing
    prev_data: FftData,
    /// Magnitude bins from last FFT
    magnitudes: Vec<f32>,
    /// Smoothing factor
    smoothing: f32,
}

impl FftAnalyzer {
    /// Create a new FFT analyzer
    pub fn new(sample_rate: u32) -> Self {
        let planner = FftPlanner::new();

        // Pre-compute Hann window
        let window: Vec<f32> = (0..FFT_SIZE)
            .map(|i| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / FFT_SIZE as f32).cos()))
            .collect();

        Self {
            planner,
            input_buffer: vec![Complex::new(0.0, 0.0); FFT_SIZE],
            window,
            sample_rate,
            prev_data: FftData::new(),
            magnitudes: vec![0.0; FFT_SIZE / 2],
            smoothing: DEFAULT_SMOOTHING,
        }
    }

    /// Set sample rate (if source changes)
    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate;
    }

    /// Set smoothing factor (0.0 = instant, 1.0 = very smooth)
    pub fn set_smoothing(&mut self, smoothing: f32) {
        self.smoothing = smoothing.clamp(0.0, 0.99);
    }

    /// Get the FFT size
    pub fn fft_size(&self) -> usize {
        FFT_SIZE
    }

    /// Analyze audio buffer and return FFT band data
    pub fn analyze(&mut self, buffer: &AudioBuffer) -> FftData {
        // Downmix to mono
        let mono = buffer.to_mono();

        if mono.len() < FFT_SIZE {
            // Not enough samples, return smoothed previous
            return self.prev_data.clone();
        }

        // Take most recent FFT_SIZE samples
        let start = mono.len().saturating_sub(FFT_SIZE);
        let samples = &mono[start..start + FFT_SIZE];

        // Apply window and copy to input buffer
        for (i, &sample) in samples.iter().enumerate() {
            self.input_buffer[i] = Complex::new(sample * self.window[i], 0.0);
        }

        // Perform FFT (in-place)
        let fft = self.planner.plan_fft_forward(FFT_SIZE);
        fft.process(&mut self.input_buffer);

        // Calculate magnitudes (only first half - Nyquist)
        for (i, complex) in self.input_buffer[..FFT_SIZE / 2].iter().enumerate() {
            self.magnitudes[i] = complex.norm();
        }

        // Extract band energies
        let low = self.calculate_band_energy(AudioBand::Low);
        let mid = self.calculate_band_energy(AudioBand::Mid);
        let high = self.calculate_band_energy(AudioBand::High);

        // Calculate full spectrum RMS
        let full = (self.magnitudes.iter().map(|m| m * m).sum::<f32>()
            / self.magnitudes.len() as f32)
            .sqrt();

        // Normalize values (empirical scaling for 0-1 range)
        // Microphone input has low amplitude, so use gentle normalization
        let normalize = |v: f32| (v / 4.0).min(1.0);

        let raw_data = FftData {
            low: normalize(low),
            mid: normalize(mid),
            high: normalize(high),
            full: normalize(full),
            timestamp: Instant::now(),
        };

        // Apply smoothing
        let smoothed = FftData {
            low: lerp(self.prev_data.low, raw_data.low, 1.0 - self.smoothing),
            mid: lerp(self.prev_data.mid, raw_data.mid, 1.0 - self.smoothing),
            high: lerp(self.prev_data.high, raw_data.high, 1.0 - self.smoothing),
            full: lerp(self.prev_data.full, raw_data.full, 1.0 - self.smoothing),
            timestamp: raw_data.timestamp,
        };

        self.prev_data = smoothed.clone();
        smoothed
    }

    /// Calculate energy in a frequency band
    fn calculate_band_energy(&self, band: AudioBand) -> f32 {
        let (min_freq, max_freq) = band.frequency_range();
        let bin_width = self.sample_rate as f32 / FFT_SIZE as f32;

        let min_bin = (min_freq / bin_width) as usize;
        let max_bin = ((max_freq / bin_width) as usize).min(self.magnitudes.len() - 1);

        if min_bin >= max_bin {
            return 0.0;
        }

        // Calculate RMS energy in the band
        let sum: f32 = self.magnitudes[min_bin..=max_bin]
            .iter()
            .map(|m| m * m)
            .sum();

        (sum / (max_bin - min_bin + 1) as f32).sqrt()
    }

    /// Get the raw magnitudes from the last analysis (for visualization)
    pub fn magnitudes(&self) -> &[f32] {
        &self.magnitudes
    }

    /// Get the bin index for a given frequency
    pub fn frequency_to_bin(&self, frequency: f32) -> usize {
        let bin_width = self.sample_rate as f32 / FFT_SIZE as f32;
        (frequency / bin_width) as usize
    }

    /// Get the frequency for a given bin index
    pub fn bin_to_frequency(&self, bin: usize) -> f32 {
        let bin_width = self.sample_rate as f32 / FFT_SIZE as f32;
        bin as f32 * bin_width
    }
}

/// Linear interpolation
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fft_analyzer_creation() {
        let analyzer = FftAnalyzer::new(48000);
        assert_eq!(analyzer.fft_size(), 2048);
    }

    #[test]
    fn test_frequency_to_bin() {
        let analyzer = FftAnalyzer::new(48000);
        // At 48kHz with 2048 FFT, bin width = 48000/2048 = 23.4375 Hz
        let bin = analyzer.frequency_to_bin(1000.0);
        assert!(bin > 40 && bin < 45); // Should be around 42-43
    }

    #[test]
    fn test_analyze_silence() {
        let mut analyzer = FftAnalyzer::new(48000);

        // Create silent buffer
        let buffer = AudioBuffer {
            samples: vec![0.0; 4096],
            sample_rate: 48000,
            channels: 1,
        };

        let fft = analyzer.analyze(&buffer);
        assert!(fft.low < 0.01);
        assert!(fft.mid < 0.01);
        assert!(fft.high < 0.01);
    }

    #[test]
    fn test_analyze_sine_wave() {
        let mut analyzer = FftAnalyzer::new(48000);

        // Create 100Hz sine wave (should appear in low band)
        let samples: Vec<f32> = (0..4096)
            .map(|i| (2.0 * std::f32::consts::PI * 100.0 * i as f32 / 48000.0).sin())
            .collect();

        let buffer = AudioBuffer {
            samples,
            sample_rate: 48000,
            channels: 1,
        };

        let fft = analyzer.analyze(&buffer);
        // Low band should have significant energy
        assert!(fft.low > 0.1, "Low band should detect 100Hz sine: {}", fft.low);
        // Mid and high should be lower
        assert!(fft.mid < fft.low);
    }
}
