//! Projection Mapping Calibration Tool
//!
//! Entry point for the projmap-calibrator application.

use projmap_calibrator::app::CalibrationApp;
use winit::event_loop::EventLoop;

fn main() {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    log::info!("ProjMap Calibrator starting...");

    // Initialize NDI
    if !projmap_calibrator::camera::ndi_initialize() {
        log::error!("Failed to initialize NDI library");
        return;
    }
    log::info!("NDI version: {}", projmap_calibrator::camera::ndi_version());

    // Create event loop and run application
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut app = CalibrationApp::new();

    if let Err(e) = event_loop.run_app(&mut app) {
        log::error!("Event loop error: {}", e);
    }

    // Cleanup NDI
    projmap_calibrator::camera::ndi_destroy();
    log::info!("ProjMap Calibrator exiting");
}
