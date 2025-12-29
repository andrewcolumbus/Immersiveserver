//! Immersive Player - HAP Video Player with Projection Blending
//!
//! Main entry point for the application.

mod api;
mod app;
mod composition;
mod converter;
mod output;
mod project;
mod render;
mod ui;
mod video;

use app::ImmersivePlayerApp;

fn main() -> eframe::Result<()> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    log::info!("Starting Immersive Player v{}", env!("CARGO_PKG_VERSION"));

    // Configure native options
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("Immersive Player"),
        vsync: true,
        multisampling: 0,
        ..Default::default()
    };

    // Run the app
    eframe::run_native(
        "Immersive Player",
        native_options,
        Box::new(|cc| Box::new(ImmersivePlayerApp::new(cc))),
    )
}
