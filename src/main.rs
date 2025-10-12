mod state;
mod winit;
mod drm;
mod drm_minimal;
mod drm_new;
mod cursor;

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
        
        // Check if user wants full DRM backend or just minimal test
        let use_full_drm = std::env::args().any(|arg| arg == "--drm-full");
        
        if use_full_drm {
            tracing::info!("Starting FULL DRM backend...");
            tracing::info!("⚠️  Note: Full rendering not yet implemented, this tests initialization only");
            if let Err(err) = drm_new::run_udev() {
                tracing::error!("Full DRM backend failed: {}", err);
                std::process::exit(1);
            }
        } else {
            // Run minimal DRM test to validate environment
            tracing::info!("Running minimal DRM test (use --drm-full for full backend)...");
            if let Err(err) = drm_minimal::test_drm_minimal() {
                tracing::error!("DRM minimal test failed: {}", err);
                tracing::error!("Fix environment issues before proceeding");
                std::process::exit(1);
            }
            
            tracing::info!("✅ DRM test passed.");
            tracing::info!("");
            tracing::info!("Next: Test full backend with --drm --drm-full");
        }
    } else {
        tracing::info!("🪟 Using winit backend (nested mode)");
        if let Err(err) = winit::init_winit() {
            tracing::error!("Failed to initialize winit backend: {}", err);
            std::process::exit(1);
        }
    }
}
