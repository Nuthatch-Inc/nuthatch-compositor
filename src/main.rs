mod state;
mod winit;

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

    // Run winit backend (nested compositor for development)
    if let Err(err) = winit::init_winit() {
        tracing::error!("Failed to initialize compositor: {}", err);
        std::process::exit(1);
    }
}
