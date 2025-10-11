mod state;
mod winit;
mod drm;
mod drm_minimal;
// mod drm_new; // TODO: Enable once trait implementations are complete

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
        
        // Run minimal DRM test to validate environment
        tracing::info!("Running minimal DRM test...");
        if let Err(err) = drm_minimal::test_drm_minimal() {
            tracing::error!("DRM minimal test failed: {}", err);
            tracing::error!("Fix environment issues before proceeding");
            std::process::exit(1);
        }
        
        tracing::info!("✅ DRM test passed.");
        tracing::info!("");
        tracing::info!("Next steps:");
        tracing::info!("  1. Implement trait handlers for DrmCompositorState in drm_new.rs");
        tracing::info!("  2. Complete device_added() function");
        tracing::info!("  3. Add rendering loop and frame presentation");
    } else {
        tracing::info!("🪟 Using winit backend (nested mode)");
        if let Err(err) = winit::init_winit() {
            tracing::error!("Failed to initialize winit backend: {}", err);
            std::process::exit(1);
        }
    }
}
