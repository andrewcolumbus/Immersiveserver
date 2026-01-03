//! Audio source trait and common implementations

use super::types::{AudioBuffer, AudioSourceId};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// Trait for audio input sources
pub trait AudioSource: Send + Sync {
    /// Unique identifier for this source
    fn id(&self) -> &AudioSourceId;

    /// Human-readable display name
    fn display_name(&self) -> String;

    /// Get the sample rate of this source
    fn sample_rate(&self) -> u32;

    /// Get number of channels
    fn channels(&self) -> u32;

    /// Check if source is currently active/connected
    fn is_active(&self) -> bool;

    /// Get available audio samples (non-blocking)
    /// Returns None if no new samples available
    fn take_samples(&self) -> Option<AudioBuffer>;

    /// Start capturing audio (if not auto-started)
    fn start(&self) -> Result<(), String>;

    /// Stop capturing audio
    fn stop(&self);
}

/// Shared state for audio source threads
pub struct AudioSourceState {
    /// Whether the source should keep running
    pub running: AtomicBool,
    /// Whether source is actively receiving data
    pub active: AtomicBool,
    /// Ring buffer for audio samples
    pub buffer: Mutex<AudioRingBuffer>,
}

impl AudioSourceState {
    pub fn new(buffer_size: usize, sample_rate: u32, channels: u32) -> Self {
        Self {
            running: AtomicBool::new(false),
            active: AtomicBool::new(false),
            buffer: Mutex::new(AudioRingBuffer::new(buffer_size, sample_rate, channels)),
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    pub fn set_running(&self, running: bool) {
        self.running.store(running, Ordering::Release);
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    pub fn set_active(&self, active: bool) {
        self.active.store(active, Ordering::Release);
    }
}

/// Simple ring buffer for audio samples
pub struct AudioRingBuffer {
    data: Vec<f32>,
    write_pos: usize,
    read_pos: usize,
    capacity: usize,
    sample_rate: u32,
    channels: u32,
    /// Track how many samples are available
    available: usize,
}

impl AudioRingBuffer {
    pub fn new(capacity: usize, sample_rate: u32, channels: u32) -> Self {
        Self {
            data: vec![0.0; capacity],
            write_pos: 0,
            read_pos: 0,
            capacity,
            sample_rate,
            channels,
            available: 0,
        }
    }

    pub fn set_format(&mut self, sample_rate: u32, channels: u32) {
        self.sample_rate = sample_rate;
        self.channels = channels;
    }

    /// Write samples into the buffer
    pub fn write(&mut self, samples: &[f32]) {
        for &sample in samples {
            self.data[self.write_pos] = sample;
            self.write_pos = (self.write_pos + 1) % self.capacity;

            // Track available samples, cap at capacity
            if self.available < self.capacity {
                self.available += 1;
            } else {
                // Buffer is full, advance read position
                self.read_pos = (self.read_pos + 1) % self.capacity;
            }
        }
    }

    /// Read available samples into output vector
    pub fn read(&mut self, output: &mut Vec<f32>) -> usize {
        let count = self.available;
        output.clear();
        output.reserve(count);

        for _ in 0..count {
            output.push(self.data[self.read_pos]);
            self.read_pos = (self.read_pos + 1) % self.capacity;
        }

        self.available = 0;
        count
    }

    /// Read up to max_samples into output vector
    pub fn read_max(&mut self, output: &mut Vec<f32>, max_samples: usize) -> usize {
        let count = self.available.min(max_samples);
        output.clear();
        output.reserve(count);

        for _ in 0..count {
            output.push(self.data[self.read_pos]);
            self.read_pos = (self.read_pos + 1) % self.capacity;
        }

        self.available -= count;
        count
    }

    /// Number of samples available to read
    pub fn available(&self) -> usize {
        self.available
    }

    /// Clear the buffer
    pub fn clear(&mut self) {
        self.read_pos = 0;
        self.write_pos = 0;
        self.available = 0;
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u32 {
        self.channels
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer_write_read() {
        let mut buffer = AudioRingBuffer::new(100, 48000, 2);

        // Write some samples
        buffer.write(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!(buffer.available(), 5);

        // Read them back
        let mut output = Vec::new();
        let count = buffer.read(&mut output);
        assert_eq!(count, 5);
        assert_eq!(output, vec![1.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!(buffer.available(), 0);
    }

    #[test]
    fn test_ring_buffer_overflow() {
        let mut buffer = AudioRingBuffer::new(5, 48000, 1);

        // Write more than capacity
        buffer.write(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]);

        // Should only have last 5 samples
        assert_eq!(buffer.available(), 5);

        let mut output = Vec::new();
        buffer.read(&mut output);
        assert_eq!(output, vec![3.0, 4.0, 5.0, 6.0, 7.0]);
    }
}
