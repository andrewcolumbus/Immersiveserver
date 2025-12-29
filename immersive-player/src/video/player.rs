//! Video player for HAP playback
//!
//! Provides video playback functionality with transport controls.

#![allow(dead_code)]

use super::{HapDecoder, HapFrame, HapFormat};
use anyhow::Result;
use std::path::Path;
use std::time::Instant;

/// Loop mode for video playback
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum LoopMode {
    /// Play once and stop
    #[default]
    None,
    /// Loop continuously
    Loop,
    /// Ping-pong (forward then backward)
    PingPong,
}

/// Playback state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum PlaybackState {
    #[default]
    Stopped,
    Playing,
    Paused,
}

/// Video player for HAP video playback
#[derive(Debug)]
pub struct VideoPlayer {
    /// Current decoder
    decoder: Option<HapDecoder>,
    /// Playback state
    state: PlaybackState,
    /// Loop mode
    pub loop_mode: LoopMode,
    /// Current playback time in seconds
    current_time: f64,
    /// Playback speed (1.0 = normal)
    pub speed: f32,
    /// Last update time
    last_update: Option<Instant>,
    /// Current frame
    current_frame: Option<HapFrame>,
    /// FPS counter
    fps_counter: FpsCounter,
}

impl Default for VideoPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl VideoPlayer {
    /// Create a new video player
    pub fn new() -> Self {
        Self {
            decoder: None,
            state: PlaybackState::Stopped,
            loop_mode: LoopMode::None,
            current_time: 0.0,
            speed: 1.0,
            last_update: None,
            current_frame: None,
            fps_counter: FpsCounter::new(),
        }
    }

    /// Load a video file
    pub fn load(&mut self, path: &Path) -> Result<()> {
        log::info!("Loading video: {:?}", path);
        let decoder = HapDecoder::new(path)?;
        self.decoder = Some(decoder);
        self.current_time = 0.0;
        self.state = PlaybackState::Stopped;
        self.current_frame = None;
        Ok(())
    }

    /// Load a test pattern
    pub fn load_test_pattern(&mut self, width: u32, height: u32) {
        log::info!("Loading test pattern {}x{}", width, height);
        let decoder = HapDecoder::new_test_pattern(width, height, HapFormat::Hap);
        self.decoder = Some(decoder);
        self.current_time = 0.0;
        self.state = PlaybackState::Stopped;
    }

    /// Check if a video is loaded
    pub fn is_loaded(&self) -> bool {
        self.decoder.is_some()
    }

    /// Check if playing
    pub fn is_playing(&self) -> bool {
        self.state == PlaybackState::Playing
    }

    /// Get dimensions
    pub fn dimensions(&self) -> Option<(u32, u32)> {
        self.decoder.as_ref().map(|d| (d.width, d.height))
    }

    /// Get duration in seconds
    pub fn duration(&self) -> f64 {
        self.decoder.as_ref().map(|d| d.duration).unwrap_or(0.0)
    }

    /// Get current time in seconds
    pub fn current_time(&self) -> f64 {
        self.current_time
    }

    /// Get playback progress (0.0 to 1.0)
    pub fn progress(&self) -> f64 {
        let duration = self.duration();
        if duration > 0.0 {
            self.current_time / duration
        } else {
            0.0
        }
    }

    /// Get current FPS
    pub fn current_fps(&self) -> f32 {
        self.fps_counter.fps()
    }

    /// Get frame rate
    pub fn frame_rate(&self) -> f64 {
        self.decoder.as_ref().map(|d| d.frame_rate).unwrap_or(30.0)
    }

    /// Play
    pub fn play(&mut self) {
        if self.decoder.is_some() {
            self.state = PlaybackState::Playing;
            self.last_update = Some(Instant::now());
        }
    }

    /// Pause
    pub fn pause(&mut self) {
        self.state = PlaybackState::Paused;
    }

    /// Stop
    pub fn stop(&mut self) {
        self.state = PlaybackState::Stopped;
        self.current_time = 0.0;
        if let Some(decoder) = &mut self.decoder {
            let _ = decoder.seek_to_time(0.0);
        }
    }

    /// Toggle play/pause
    pub fn toggle_play(&mut self) {
        match self.state {
            PlaybackState::Playing => self.pause(),
            _ => self.play(),
        }
    }

    /// Seek to a time
    pub fn seek(&mut self, time: f64) {
        self.current_time = time.clamp(0.0, self.duration());
        if let Some(decoder) = &mut self.decoder {
            let _ = decoder.seek_to_time(self.current_time);
        }
    }

    /// Update playback state
    pub fn update(&mut self) {
        if self.state != PlaybackState::Playing {
            return;
        }

        let now = Instant::now();
        if let Some(last) = self.last_update {
            let delta = now.duration_since(last).as_secs_f64();
            self.current_time += delta * self.speed as f64;

            // Handle looping
            let duration = self.duration();
            if duration > 0.0 && self.current_time >= duration {
                match self.loop_mode {
                    LoopMode::None => {
                        self.current_time = duration;
                        self.state = PlaybackState::Stopped;
                    }
                    LoopMode::Loop => {
                        self.current_time = self.current_time % duration;
                    }
                    LoopMode::PingPong => {
                        // TODO: Implement ping-pong
                        self.current_time = self.current_time % duration;
                    }
                }
            }

            // Decode next frame
            if let Some(decoder) = &mut self.decoder {
                let _ = decoder.seek_to_time(self.current_time);
                if let Ok(Some(frame)) = decoder.decode_next() {
                    self.current_frame = Some(frame);
                    self.fps_counter.tick();
                }
            }
        }
        self.last_update = Some(now);
    }

    /// Get the current frame
    pub fn current_frame(&self) -> Option<&HapFrame> {
        self.current_frame.as_ref()
    }
}

/// FPS counter for performance monitoring
#[derive(Debug)]
struct FpsCounter {
    frame_times: Vec<Instant>,
    max_samples: usize,
}

impl FpsCounter {
    fn new() -> Self {
        Self {
            frame_times: Vec::with_capacity(60),
            max_samples: 60,
        }
    }

    fn tick(&mut self) {
        let now = Instant::now();
        self.frame_times.push(now);
        
        // Remove old samples
        let one_second_ago = now - std::time::Duration::from_secs(1);
        self.frame_times.retain(|t| *t > one_second_ago);
        
        // Limit samples
        if self.frame_times.len() > self.max_samples {
            self.frame_times.remove(0);
        }
    }

    fn fps(&self) -> f32 {
        if self.frame_times.len() < 2 {
            return 0.0;
        }
        self.frame_times.len() as f32
    }
}

/// Format time as MM:SS.FF
pub fn format_time(seconds: f64) -> String {
    let mins = (seconds / 60.0) as u32;
    let secs = (seconds % 60.0) as u32;
    let frames = ((seconds % 1.0) * 100.0) as u32;
    format!("{:02}:{:02}.{:02}", mins, secs, frames)
}
