mod state;
mod winit;
// mod drm;  // TODO: DRM backend needs more work with Smithay 0.7 API

use tracing_subscriber::fmt;

fn main() {
    // Setup logging
    fmt()
        .compact()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
        )
        .init();

    tracing::info!("🐦 Nuthatch Compositor starting...");
    tracing::info!("Phase 1: Foundation - Window management basics");

    // For now, only use winit backend
    // TODO: Implement proper DRM backend once API is sorted
    tracing::info!("🪟 Using winit backend");
    if let Err(err) = winit::init_winit() {
        tracing::error!("Failed to initialize winit backend: {}", err);
        std::process::exit(1);
    }
}
