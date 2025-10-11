mod state;
mod winit;
mod drm;

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

    // Check if we should use DRM backend
    // Use DRM if: --drm flag, NUTHATCH_DRM env var, or no display available (TTY)
    let use_drm = std::env::args().any(|arg| arg == "--drm")
        || std::env::var("NUTHATCH_DRM").is_ok()
        || (std::env::var("DISPLAY").is_err() && std::env::var("WAYLAND_DISPLAY").is_err());
    
    if use_drm {
        tracing::info!("🖥️  Using DRM/KMS backend (native TTY mode)");
        if let Err(err) = drm::init_drm() {
            tracing::error!("Failed to initialize DRM backend: {}", err);
            std::process::exit(1);
        }
    } else {
        tracing::info!("🪟 Using winit backend (nested mode)");
        if let Err(err) = winit::init_winit() {
            tracing::error!("Failed to initialize winit backend: {}", err);
            std::process::exit(1);
        }
    }
}
