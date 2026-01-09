//! Logging configuration and initialization
//!
//! Provides structured logging with tracing, supporting console output,
//! file logging with rotation, and JSON format for production.

use std::path::PathBuf;
use tracing_subscriber::{
    fmt,
    prelude::*,
    filter::EnvFilter,
};
use tracing_appender::non_blocking::WorkerGuard;

/// Logging configuration
#[derive(Debug, Clone)]
pub struct LogConfig {
    /// Enable console output (default: true)
    pub console_enabled: bool,
    /// Enable file logging (default: false)
    pub file_enabled: bool,
    /// Path for log files (default: None, uses system log directory)
    pub file_path: Option<PathBuf>,
    /// Use JSON format for logs (default: false)
    pub json_format: bool,
    /// Default log level filter (default: "info")
    pub default_level: String,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            console_enabled: true,
            file_enabled: false,
            file_path: None,
            json_format: false,
            default_level: "info".to_string(),
        }
    }
}

/// Initialize the logging system with the given configuration
///
/// Returns a guard that must be kept alive for the duration of the program
/// to ensure file logging is properly flushed.
///
/// # Environment Variables
///
/// - `IMMERSIVE_LOG`: Set log level filter (e.g., "debug", "info,immersive_server=debug")
/// - `IMMERSIVE_LOG_FORMAT`: Set to "json" for JSON output
///
/// # Example
///
/// ```no_run
/// use immersive_server::telemetry::{init_logging, LogConfig};
///
/// let config = LogConfig::default();
/// let _guard = init_logging(&config).expect("Failed to initialize logging");
/// // Keep _guard alive for the program duration
/// ```
pub fn init_logging(config: &LogConfig) -> Result<Option<WorkerGuard>, Box<dyn std::error::Error + Send + Sync>> {
    // Build the environment filter
    // Check IMMERSIVE_LOG first, then fall back to RUST_LOG, then to config default
    let env_filter = EnvFilter::try_from_env("IMMERSIVE_LOG")
        .or_else(|_| EnvFilter::try_from_env("RUST_LOG"))
        .unwrap_or_else(|_| EnvFilter::new(&config.default_level));

    // Check if JSON format is requested via environment
    let use_json = std::env::var("IMMERSIVE_LOG_FORMAT")
        .map(|v| v.to_lowercase() == "json")
        .unwrap_or(config.json_format);

    let mut file_guard: Option<WorkerGuard> = None;

    // Build the subscriber with layers
    let subscriber = tracing_subscriber::registry().with(env_filter);

    if config.file_enabled {
        // Setup file logging with tracing-appender
        let log_path = config.file_path.clone().unwrap_or_else(|| PathBuf::from("viewport_debug.log"));
        let file = std::fs::File::create(&log_path)?;
        let (non_blocking, guard) = tracing_appender::non_blocking(file);
        file_guard = Some(guard);

        let file_layer = fmt::layer()
            .with_writer(non_blocking)
            .with_target(true)
            .with_thread_ids(false)
            .with_file(true)
            .with_line_number(true)
            .with_ansi(false);  // No ANSI colors in file

        if config.console_enabled {
            // Both console and file
            let console_layer = fmt::layer()
                .with_target(true)
                .with_thread_ids(false)
                .with_file(false)
                .with_line_number(false)
                .compact();

            subscriber
                .with(file_layer)
                .with(console_layer)
                .init();
        } else {
            // File only
            subscriber.with(file_layer).init();
        }

        eprintln!("Logging to file: {}", log_path.display());
    } else if config.console_enabled {
        if use_json {
            // JSON format for production/log aggregation
            let json_layer = fmt::layer()
                .json()
                .with_target(true)
                .with_thread_ids(true)
                .with_file(true)
                .with_line_number(true);

            subscriber.with(json_layer).init();
        } else {
            // Pretty console format for development
            let console_layer = fmt::layer()
                .with_target(true)
                .with_thread_ids(false)
                .with_file(false)
                .with_line_number(false)
                .compact();

            subscriber.with(console_layer).init();
        }
    } else {
        // No console output - just initialize with filter
        subscriber.init();
    }

    // Log startup info
    tracing::info!(
        target: "immersive_server",
        version = env!("CARGO_PKG_VERSION"),
        json_format = use_json,
        file_enabled = config.file_enabled,
        "Logging initialized"
    );

    Ok(file_guard)
}

/// Initialize logging from environment with sensible defaults
///
/// This is a convenience function that uses default LogConfig
/// and is suitable for most use cases.
pub fn init_logging_default() -> Result<Option<WorkerGuard>, Box<dyn std::error::Error + Send + Sync>> {
    init_logging(&LogConfig::default())
}

// Re-export WorkerGuard so callers can store it
pub use tracing_appender::non_blocking::WorkerGuard as LogGuard;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_config_default() {
        let config = LogConfig::default();
        assert!(config.console_enabled);
        assert!(!config.file_enabled);
        assert!(!config.json_format);
        assert_eq!(config.default_level, "info");
    }
}
