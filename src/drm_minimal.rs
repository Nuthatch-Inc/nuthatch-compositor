// Minimal DRM test module - validates we can initialize session and enumerate GPUs
// Based on Smithay Anvil's udev.rs implementation
// This is the first step before implementing full DRM backend

use anyhow::{Context, Result};
use smithay::{
    backend::{
        session::{
            libseat::{LibSeatSession},
            Session,
        },
        udev::{all_gpus, primary_gpu, UdevBackend},
        drm::{DrmNode, NodeType},
    },
    reexports::calloop::{EventLoop},
};
use tracing::{info, error};

pub fn test_drm_minimal() -> Result<()> {
    info!("🧪 Starting minimal DRM test");

    // Step 1: Initialize session (manages VT and device access)
    info!("Step 1: Initializing LibSeat session...");
    let (session, _notifier) = LibSeatSession::new()
        .context("Failed to create LibSeat session - are you running in a TTY?")?;
    
    let seat_name = session.seat();
    info!("✅ Session initialized for seat: {}", seat_name);

    // Step 2: Find primary GPU
    info!("Step 2: Finding primary GPU...");
    let primary_gpu = primary_gpu(&seat_name)
        .context("Failed to query primary GPU")?
        .and_then(|path| {
            info!("   Primary GPU path: {:?}", path);
            DrmNode::from_path(&path).ok()?.node_with_type(NodeType::Render)?.ok()
        })
        .or_else(|| {
            info!("   No primary GPU found, falling back to first available GPU");
            all_gpus(&seat_name)
                .ok()?
                .into_iter()
                .find_map(|path| {
                    info!("   Available GPU: {:?}", path);
                    DrmNode::from_path(path).ok()
                })
        })
        .context("No GPU found!")?;
    
    info!("✅ Using GPU: {}", primary_gpu);

    // Step 3: Initialize udev backend for device discovery
    info!("Step 3: Initializing UdevBackend...");
    let udev_backend = UdevBackend::new(&seat_name)
        .context("Failed to initialize udev backend")?;
    
    info!("✅ UdevBackend initialized");

    // Step 4: List all available DRM devices
    info!("Step 4: Enumerating DRM devices...");
    let mut device_count = 0;
    for (device_id, path) in udev_backend.device_list() {
        device_count += 1;
        if let Ok(node) = DrmNode::from_dev_id(device_id) {
            info!("   Device {}: {} -> {:?}", device_count, node, path);
            
            // Try to get render node - returns Option<Result<DrmNode, Error>>
            if let Some(Ok(render_node)) = node.node_with_type(NodeType::Render) {
                info!("      Render node: {}", render_node);
            }
        } else {
            info!("   Device {}: dev_id {} -> {:?}", device_count, device_id, path);
        }
    }
    
    if device_count == 0 {
        error!("❌ No DRM devices found!");
        return Err(anyhow::anyhow!("No DRM devices available"));
    }
    
    info!("✅ Found {} DRM device(s)", device_count);

    // Step 5: Create event loop (needed for async operations)
    info!("Step 5: Creating event loop...");
    let _event_loop: EventLoop<()> = EventLoop::try_new()
        .context("Failed to create event loop")?;
    info!("✅ Event loop created");

    info!("");
    info!("🎉 All checks passed! Environment is ready for full DRM implementation.");
    info!("");
    info!("Summary:");
    info!("  • Session: {}", seat_name);
    info!("  • Primary GPU: {}", primary_gpu);
    info!("  • DRM Devices: {}", device_count);
    info!("");
    info!("Next step: Implement full DRM backend with rendering");

    Ok(())
}
