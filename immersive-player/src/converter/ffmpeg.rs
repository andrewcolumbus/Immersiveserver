//! FFmpeg wrapper for video conversion.

#![allow(dead_code)]

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use thiserror::Error;

use super::formats::{HapVariant, QualityPreset};

/// Errors that can occur during FFmpeg operations.
#[derive(Error, Debug)]
pub enum FFmpegError {
    #[error("FFmpeg binary not found. Please install FFmpeg or place it in assets/ffmpeg/")]
    NotFound,
    #[error("Failed to spawn FFmpeg process: {0}")]
    SpawnFailed(#[from] std::io::Error),
    #[error("FFmpeg conversion failed: {0}")]
    ConversionFailed(String),
    #[error("Conversion was cancelled")]
    Cancelled,
    #[error("Failed to parse video info: {0}")]
    ParseError(String),
}

/// Progress information during conversion.
#[derive(Debug, Clone, PartialEq)]
pub struct ConversionProgress {
    /// Current frame being processed
    pub frame: u64,
    /// Total frames (if known)
    pub total_frames: Option<u64>,
    /// Current time in seconds
    pub time_seconds: f64,
    /// Total duration in seconds (if known)
    pub duration_seconds: Option<f64>,
    /// Estimated percentage complete (0.0 - 100.0)
    pub percent: f64,
    /// Processing speed (e.g., "2.5x")
    pub speed: Option<String>,
    /// Estimated time remaining in seconds
    pub eta_seconds: Option<f64>,
}

impl Default for ConversionProgress {
    fn default() -> Self {
        Self {
            frame: 0,
            total_frames: None,
            time_seconds: 0.0,
            duration_seconds: None,
            percent: 0.0,
            speed: None,
            eta_seconds: None,
        }
    }
}

/// Video metadata extracted from input file.
#[derive(Debug, Clone)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    pub duration_seconds: f64,
    pub frame_rate: f64,
    pub total_frames: u64,
    pub codec: String,
    pub has_alpha: bool,
}

/// FFmpeg wrapper for spawning and managing FFmpeg processes.
pub struct FFmpegWrapper {
    /// Path to FFmpeg binary
    ffmpeg_path: PathBuf,
    /// Path to FFprobe binary (for metadata extraction)
    ffprobe_path: Option<PathBuf>,
}

impl FFmpegWrapper {
    /// Create a new FFmpeg wrapper, searching for the binary.
    pub fn new() -> Result<Self, FFmpegError> {
        let ffmpeg_path = Self::find_ffmpeg()?;
        let ffprobe_path = Self::find_ffprobe();
        
        Ok(Self {
            ffmpeg_path,
            ffprobe_path,
        })
    }

    /// Find FFmpeg binary in various locations.
    fn find_ffmpeg() -> Result<PathBuf, FFmpegError> {
        // 1. Check bundled location
        let bundled_paths = if cfg!(target_os = "macos") {
            vec![
                PathBuf::from("assets/ffmpeg/ffmpeg-macos"),
                PathBuf::from("assets/ffmpeg/ffmpeg"),
            ]
        } else if cfg!(target_os = "windows") {
            vec![
                PathBuf::from("assets/ffmpeg/ffmpeg-windows.exe"),
                PathBuf::from("assets/ffmpeg/ffmpeg.exe"),
            ]
        } else {
            vec![PathBuf::from("assets/ffmpeg/ffmpeg")]
        };

        for path in bundled_paths {
            if path.exists() {
                return Ok(path);
            }
        }

        // 2. Check system PATH using which crate
        if let Ok(path) = which::which("ffmpeg") {
            return Ok(path);
        }

        // 3. Check common install locations
        let common_paths = if cfg!(target_os = "macos") {
            vec![
                "/usr/local/bin/ffmpeg",
                "/opt/homebrew/bin/ffmpeg",
                "/opt/local/bin/ffmpeg",
            ]
        } else if cfg!(target_os = "windows") {
            vec![
                "C:\\ffmpeg\\bin\\ffmpeg.exe",
                "C:\\Program Files\\ffmpeg\\bin\\ffmpeg.exe",
            ]
        } else {
            vec![
                "/usr/bin/ffmpeg",
                "/usr/local/bin/ffmpeg",
            ]
        };

        for path_str in common_paths {
            let path = PathBuf::from(path_str);
            if path.exists() {
                return Ok(path);
            }
        }

        Err(FFmpegError::NotFound)
    }

    /// Find FFprobe binary.
    fn find_ffprobe() -> Option<PathBuf> {
        // Try which first
        if let Ok(path) = which::which("ffprobe") {
            return Some(path);
        }

        // Check common locations
        let common_paths = if cfg!(target_os = "macos") {
            vec![
                "/usr/local/bin/ffprobe",
                "/opt/homebrew/bin/ffprobe",
            ]
        } else if cfg!(target_os = "windows") {
            vec![
                "C:\\ffmpeg\\bin\\ffprobe.exe",
            ]
        } else {
            vec![
                "/usr/bin/ffprobe",
                "/usr/local/bin/ffprobe",
            ]
        };

        for path_str in common_paths {
            let path = PathBuf::from(path_str);
            if path.exists() {
                return Some(path);
            }
        }

        None
    }

    /// Get video information using FFprobe.
    pub fn get_video_info(&self, input: &Path) -> Result<VideoInfo, FFmpegError> {
        let ffprobe = self.ffprobe_path.as_ref().ok_or_else(|| {
            FFmpegError::ParseError("FFprobe not found".to_string())
        })?;

        let output = Command::new(ffprobe)
            .args([
                "-v", "quiet",
                "-print_format", "json",
                "-show_format",
                "-show_streams",
            ])
            .arg(input)
            .output()
            .map_err(FFmpegError::SpawnFailed)?;

        if !output.status.success() {
            return Err(FFmpegError::ParseError(
                String::from_utf8_lossy(&output.stderr).to_string()
            ));
        }

        let json_str = String::from_utf8_lossy(&output.stdout);
        Self::parse_video_info(&json_str)
    }

    /// Parse video info from FFprobe JSON output.
    fn parse_video_info(json_str: &str) -> Result<VideoInfo, FFmpegError> {
        // Simple JSON parsing without additional dependencies
        // Look for video stream info
        
        let mut width = 0u32;
        let mut height = 0u32;
        let mut duration_seconds = 0.0f64;
        let mut frame_rate = 30.0f64;
        let mut codec = String::from("unknown");
        let mut has_alpha = false;

        // Parse width
        if let Some(start) = json_str.find("\"width\":") {
            let rest = &json_str[start + 8..];
            if let Some(end) = rest.find([',', '}']) {
                if let Ok(w) = rest[..end].trim().parse() {
                    width = w;
                }
            }
        }

        // Parse height
        if let Some(start) = json_str.find("\"height\":") {
            let rest = &json_str[start + 9..];
            if let Some(end) = rest.find([',', '}']) {
                if let Ok(h) = rest[..end].trim().parse() {
                    height = h;
                }
            }
        }

        // Parse duration
        if let Some(start) = json_str.find("\"duration\":") {
            let rest = &json_str[start + 11..];
            if let Some(end) = rest.find([',', '}']) {
                let dur_str = rest[..end].trim().trim_matches('"');
                if let Ok(d) = dur_str.parse() {
                    duration_seconds = d;
                }
            }
        }

        // Parse frame rate (r_frame_rate format like "30000/1001")
        if let Some(start) = json_str.find("\"r_frame_rate\":") {
            let rest = &json_str[start + 15..];
            if let Some(end) = rest.find([',', '}']) {
                let rate_str = rest[..end].trim().trim_matches('"');
                if let Some((num, den)) = rate_str.split_once('/') {
                    if let (Ok(n), Ok(d)) = (num.parse::<f64>(), den.parse::<f64>()) {
                        if d > 0.0 {
                            frame_rate = n / d;
                        }
                    }
                }
            }
        }

        // Parse codec name
        if let Some(start) = json_str.find("\"codec_name\":") {
            let rest = &json_str[start + 13..];
            if let Some(end) = rest.find([',', '}']) {
                codec = rest[..end].trim().trim_matches('"').to_string();
            }
        }

        // Check for alpha (look for pix_fmt containing "a" like "rgba", "yuva")
        if let Some(start) = json_str.find("\"pix_fmt\":") {
            let rest = &json_str[start + 10..];
            if let Some(end) = rest.find([',', '}']) {
                let pix_fmt = rest[..end].trim().trim_matches('"');
                has_alpha = pix_fmt.contains("rgba") || 
                           pix_fmt.contains("yuva") || 
                           pix_fmt.contains("gbrap");
            }
        }

        let total_frames = (duration_seconds * frame_rate).round() as u64;

        Ok(VideoInfo {
            width,
            height,
            duration_seconds,
            frame_rate,
            total_frames,
            codec,
            has_alpha,
        })
    }

    /// Start a conversion process.
    pub fn start_conversion(
        &self,
        input: &Path,
        output: &Path,
        variant: HapVariant,
        _preset: QualityPreset,
    ) -> Result<ConversionProcess, FFmpegError> {
        let cancel_flag = Arc::new(AtomicBool::new(false));

        // Build FFmpeg command
        let mut cmd = Command::new(&self.ffmpeg_path);
        
        cmd.args([
            "-y",                           // Overwrite output
            "-progress", "pipe:1",          // Progress to stdout
            "-i",
        ])
        .arg(input)
        .args([
            "-c:v", "hap",                  // HAP codec
            "-format", variant.ffmpeg_format(),
            "-compressor", "snappy",        // Snappy compression
            "-an",                          // No audio (for now)
        ])
        .arg(output)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

        let child = cmd.spawn().map_err(FFmpegError::SpawnFailed)?;

        Ok(ConversionProcess {
            child,
            cancel_flag,
        })
    }

    /// Generate output path for a converted file.
    pub fn generate_output_path(input: &Path, output_dir: &Path, variant: HapVariant) -> PathBuf {
        let stem = input.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "output".to_string());
        
        let suffix = match variant {
            HapVariant::Hap => "_hap",
            HapVariant::HapAlpha => "_hap_alpha",
            HapVariant::HapQ => "_hap_q",
        };

        output_dir.join(format!("{}{}.mov", stem, suffix))
    }
}

/// Handle for a running conversion process.
pub struct ConversionProcess {
    child: Child,
    cancel_flag: Arc<AtomicBool>,
}

impl ConversionProcess {
    /// Poll for progress updates. Returns None if process has finished.
    pub fn poll_progress(&mut self) -> Option<Result<ConversionProgress, FFmpegError>> {
        // Check if cancelled
        if self.cancel_flag.load(Ordering::Relaxed) {
            let _ = self.child.kill();
            return Some(Err(FFmpegError::Cancelled));
        }

        // Try to read progress from stdout
        if let Some(stdout) = self.child.stdout.as_mut() {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            let mut progress = ConversionProgress::default();
            
            // Read available lines
            while reader.read_line(&mut line).unwrap_or(0) > 0 {
                if let Some((key, value)) = line.trim().split_once('=') {
                    match key {
                        "frame" => {
                            progress.frame = value.parse().unwrap_or(0);
                        }
                        "out_time_us" => {
                            if let Ok(us) = value.parse::<u64>() {
                                progress.time_seconds = us as f64 / 1_000_000.0;
                            }
                        }
                        "speed" => {
                            progress.speed = Some(value.to_string());
                        }
                        "progress" => {
                            if value == "end" {
                                progress.percent = 100.0;
                                return None; // Process finished
                            }
                        }
                        _ => {}
                    }
                }
                line.clear();
            }

            // Calculate percentage if we have duration info
            if let Some(duration) = progress.duration_seconds {
                if duration > 0.0 {
                    progress.percent = (progress.time_seconds / duration * 100.0).min(100.0);
                }
            }

            return Some(Ok(progress));
        }

        // Check if process has exited
        match self.child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    None // Successfully completed
                } else {
                    Some(Err(FFmpegError::ConversionFailed(
                        format!("FFmpeg exited with code: {:?}", status.code())
                    )))
                }
            }
            Ok(None) => Some(Ok(ConversionProgress::default())), // Still running
            Err(e) => Some(Err(FFmpegError::SpawnFailed(e))),
        }
    }

    /// Cancel the conversion.
    pub fn cancel(&mut self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
        let _ = self.child.kill();
    }

    /// Wait for the conversion to complete.
    pub fn wait(mut self) -> Result<(), FFmpegError> {
        let status = self.child.wait().map_err(FFmpegError::SpawnFailed)?;
        
        if status.success() {
            Ok(())
        } else {
            Err(FFmpegError::ConversionFailed(
                format!("FFmpeg exited with code: {:?}", status.code())
            ))
        }
    }

    /// Get a clone of the cancel flag for external cancellation.
    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancel_flag)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_output_path() {
        let input = Path::new("/videos/my_video.mp4");
        let output_dir = Path::new("/output");
        
        assert_eq!(
            FFmpegWrapper::generate_output_path(input, output_dir, HapVariant::Hap),
            PathBuf::from("/output/my_video_hap.mov")
        );
        
        assert_eq!(
            FFmpegWrapper::generate_output_path(input, output_dir, HapVariant::HapQ),
            PathBuf::from("/output/my_video_hap_q.mov")
        );
    }
}

