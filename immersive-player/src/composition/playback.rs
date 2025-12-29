//! Playback state machine for clips
//!
//! Handles play, pause, stop, and seeking for clip playback.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Playback state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PlaybackState {
    /// Not playing
    #[default]
    Stopped,
    /// Currently playing
    Playing,
    /// Paused (retains position)
    Paused,
}

/// Playback controller for a clip
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipPlayback {
    /// Current playback state
    pub state: PlaybackState,
    /// Current time in seconds
    pub current_time: f64,
    /// Total duration in seconds (infinity for generators/images)
    pub duration: f64,
    /// Whether to loop
    pub looping: bool,
    /// Playback direction (1.0 = forward, -1.0 = reverse)
    pub direction: f64,
    /// In point (loop start) in seconds
    pub in_point: f64,
    /// Out point (loop end) in seconds, None = use duration
    pub out_point: Option<f64>,
}

impl Default for ClipPlayback {
    fn default() -> Self {
        Self::new(f64::INFINITY)
    }
}

impl ClipPlayback {
    /// Create a new playback controller with the specified duration
    pub fn new(duration: f64) -> Self {
        Self {
            state: PlaybackState::Stopped,
            current_time: 0.0,
            duration,
            looping: true,
            direction: 1.0,
            in_point: 0.0,
            out_point: None,
        }
    }

    /// Start playing
    pub fn play(&mut self) {
        self.state = PlaybackState::Playing;
    }

    /// Pause playback
    pub fn pause(&mut self) {
        if self.state == PlaybackState::Playing {
            self.state = PlaybackState::Paused;
        }
    }

    /// Stop playback and reset to start
    pub fn stop(&mut self) {
        self.state = PlaybackState::Stopped;
        self.current_time = self.in_point;
    }

    /// Toggle between play and pause
    pub fn toggle(&mut self) {
        match self.state {
            PlaybackState::Playing => self.pause(),
            PlaybackState::Paused | PlaybackState::Stopped => self.play(),
        }
    }

    /// Check if playing
    pub fn is_playing(&self) -> bool {
        self.state == PlaybackState::Playing
    }

    /// Check if paused
    pub fn is_paused(&self) -> bool {
        self.state == PlaybackState::Paused
    }

    /// Check if stopped
    pub fn is_stopped(&self) -> bool {
        self.state == PlaybackState::Stopped
    }

    /// Get the effective end point
    pub fn end_point(&self) -> f64 {
        self.out_point.unwrap_or(self.duration)
    }

    /// Get the playable duration (out_point - in_point)
    pub fn playable_duration(&self) -> f64 {
        self.end_point() - self.in_point
    }

    /// Get progress (0.0 to 1.0)
    pub fn progress(&self) -> f64 {
        let playable = self.playable_duration();
        if playable <= 0.0 || playable.is_infinite() {
            0.0
        } else {
            ((self.current_time - self.in_point) / playable).clamp(0.0, 1.0)
        }
    }

    /// Seek to a specific time
    pub fn seek(&mut self, time: f64) {
        self.current_time = time.clamp(self.in_point, self.end_point());
    }

    /// Seek to a progress value (0.0 to 1.0)
    pub fn seek_progress(&mut self, progress: f64) {
        let target = self.in_point + progress.clamp(0.0, 1.0) * self.playable_duration();
        self.seek(target);
    }

    /// Update playback (call each frame)
    pub fn update(&mut self, delta_time: f64) {
        if self.state != PlaybackState::Playing {
            return;
        }

        // Advance time
        self.current_time += delta_time * self.direction;

        let end = self.end_point();

        // Handle forward playback
        if self.direction > 0.0 {
            if self.current_time >= end {
                if self.looping {
                    // Wrap around
                    let excess = self.current_time - end;
                    self.current_time = self.in_point + (excess % self.playable_duration());
                } else {
                    // Stop at end
                    self.current_time = end;
                    self.state = PlaybackState::Stopped;
                }
            }
        } else {
            // Handle reverse playback
            if self.current_time <= self.in_point {
                if self.looping {
                    // Wrap around
                    let deficit = self.in_point - self.current_time;
                    self.current_time = end - (deficit % self.playable_duration());
                } else {
                    // Stop at start
                    self.current_time = self.in_point;
                    self.state = PlaybackState::Stopped;
                }
            }
        }
    }

    /// Set in/out points
    pub fn set_loop_region(&mut self, in_point: f64, out_point: f64) {
        self.in_point = in_point.max(0.0);
        self.out_point = Some(out_point.min(self.duration));

        // Ensure current time is within bounds
        if self.current_time < self.in_point {
            self.current_time = self.in_point;
        } else if self.current_time > self.end_point() {
            self.current_time = self.end_point();
        }
    }

    /// Clear in/out points (use full duration)
    pub fn clear_loop_region(&mut self) {
        self.in_point = 0.0;
        self.out_point = None;
    }

    /// Reverse playback direction
    pub fn reverse(&mut self) {
        self.direction = -self.direction;
    }

    /// Set playback to forward
    pub fn set_forward(&mut self) {
        self.direction = 1.0;
    }

    /// Set playback to reverse
    pub fn set_reverse(&mut self) {
        self.direction = -1.0;
    }

    /// Jump to next frame (for frame stepping)
    pub fn next_frame(&mut self, frame_rate: f64) {
        let frame_duration = 1.0 / frame_rate;
        self.current_time = (self.current_time + frame_duration).min(self.end_point());
    }

    /// Jump to previous frame (for frame stepping)
    pub fn prev_frame(&mut self, frame_rate: f64) {
        let frame_duration = 1.0 / frame_rate;
        self.current_time = (self.current_time - frame_duration).max(self.in_point);
    }

    /// Get the current frame number at a given frame rate
    pub fn current_frame(&self, frame_rate: f64) -> u64 {
        (self.current_time * frame_rate).floor() as u64
    }

    /// Get total frames at a given frame rate
    pub fn total_frames(&self, frame_rate: f64) -> u64 {
        if self.duration.is_infinite() {
            u64::MAX
        } else {
            (self.duration * frame_rate).ceil() as u64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_playback_basic() {
        let mut playback = ClipPlayback::new(10.0);
        assert_eq!(playback.state, PlaybackState::Stopped);

        playback.play();
        assert_eq!(playback.state, PlaybackState::Playing);

        playback.pause();
        assert_eq!(playback.state, PlaybackState::Paused);

        playback.stop();
        assert_eq!(playback.state, PlaybackState::Stopped);
        assert_eq!(playback.current_time, 0.0);
    }

    #[test]
    fn test_playback_update() {
        let mut playback = ClipPlayback::new(10.0);
        playback.looping = false;
        playback.play();

        // Update for 5 seconds
        playback.update(5.0);
        assert_eq!(playback.current_time, 5.0);
        assert_eq!(playback.state, PlaybackState::Playing);

        // Update past end
        playback.update(6.0);
        assert_eq!(playback.current_time, 10.0);
        assert_eq!(playback.state, PlaybackState::Stopped);
    }

    #[test]
    fn test_playback_looping() {
        let mut playback = ClipPlayback::new(10.0);
        playback.looping = true;
        playback.play();

        // Update past end
        playback.update(12.0);
        assert_eq!(playback.current_time, 2.0);
        assert_eq!(playback.state, PlaybackState::Playing);
    }

    #[test]
    fn test_progress() {
        let mut playback = ClipPlayback::new(10.0);
        playback.seek(5.0);
        assert!((playback.progress() - 0.5).abs() < 0.001);
    }
}


