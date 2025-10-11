use smithay::{
    backend::{
        allocator::gbm::GbmDevice,
        drm::{DrmDevice, DrmDeviceFd, DrmNode},
        egl::{EGLContext, EGLDisplay},
        renderer::gles::GlesRenderer,
        session::{libseat::LibSeatSession, Session},
        udev::UdevBackend,
    },
    output::{Mode, Output, PhysicalProperties, Subpixel},
    reexports::{
        calloop::EventLoop,
        drm::control::{connector, Device, ModeTypeFlags},
        rustix::fs::OFlags,
        wayland_server::Display,
    },
    utils::Transform,
};

use smithay::backend::session::libseat::backend::DeviceFd;

use crate::state::NuthatchState;

use std::{
    path::Path,
    time::Duration,
};

pub fn init_drm() -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("🚀 Initializing DRM/KMS backend");
    
    // Create event loop
    let mut event_loop: EventLoop<NuthatchState> = EventLoop::try_new()?;
    let _loop_handle = event_loop.handle();
    
    // Create Wayland display
    let mut display: Display<NuthatchState> = Display::new()?;
    
    // Initialize compositor state
    let mut state = NuthatchState::new(&mut display, &event_loop);

    // Initialize session for VT switching and device access
    tracing::info!("Starting libseat session...");
    let (session, _notifier) = LibSeatSession::new()?;
    
    // Initialize udev backend to handle GPU device discovery
    tracing::info!("Setting up udev backend...");
    let udev_backend = UdevBackend::new(session.seat())?;
    
    // Find and use the first available GPU
    let mut gpu_found = false;
    for (dev_id, path) in udev_backend.device_list() {
        tracing::info!("Found GPU device: {:?} at {:?}", dev_id, path);
        
        // Try to open this device
        match try_initialize_gpu(&session, path, &mut display, &mut state) {
            Ok(()) => {
                tracing::info!("✅ Successfully initialized GPU: {:?}", path);
                gpu_found = true;
                break;
            }
            Err(e) => {
                tracing::warn!("Failed to initialize GPU {:?}: {}", path, e);
            }
        }
    }
    
    if !gpu_found {
        return Err("No usable GPU found!".into());
    }

    tracing::info!("🎨 Compositor ready - clients can connect");
    
    // Main event loop
    let mut frame_count = 0u64;
    loop {
        // Dispatch Wayland events
        display.dispatch_clients(&mut state)?;
        
        // Run event loop
        event_loop.dispatch(Duration::from_millis(16), &mut state)?;
        
        // Flush clients
        display.flush_clients()?;
        
        frame_count = frame_count.wrapping_add(1);
    }
}

fn try_initialize_gpu(
    session: &LibSeatSession,
    path: &Path,
    display: &mut Display<NuthatchState>,
    state: &mut NuthatchState,
) -> Result<(), Box<dyn std::error::Error>> {
    // Open DRM device
    tracing::info!("Opening DRM device: {:?}", path);
    
    // Open the device with proper flags
    let device_fd = session.open(
        path,
        OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK,
    )?;
    
    // Wrap in DeviceFd then DrmDeviceFd
    let device_fd = DeviceFd::new(device_fd, session.clone());
    let drm_fd = DrmDeviceFd::new(device_fd);
    
    // Create DRM device
    let (drm, _notifier) = DrmDevice::new(drm_fd, true)?;
    
    tracing::info!("DRM device created");
    
    // Get resource handles to find connectors
    let res_handles = drm.resource_handles()
        .map_err(|e| format!("Failed to get resource handles: {}", e))?;
    
    tracing::info!("Found {} connectors", res_handles.connectors().len());
    
    // Find first connected display
    let mut connector_info = None;
    for &connector_handle in res_handles.connectors() {
        if let Ok(info) = drm.get_connector(connector_handle, true) {
            if info.state() == connector::State::Connected {
                tracing::info!("Found connected display: {:?}", info.interface());
                connector_info = Some((connector_handle, info));
                break;
            }
        }
    }
    
    let (connector_handle, connector_info) = connector_info
        .ok_or("No connected display found")?;
    
    // Get preferred mode (usually highest resolution)
    let modes = connector_info.modes();
    let mode = modes
        .iter()
        .find(|mode| mode.mode_type().contains(ModeTypeFlags::PREFERRED))
        .or_else(|| modes.first())
        .ok_or("No display modes available")?
        .clone();
    
    tracing::info!(
        "Using mode: {}x{} @ {}Hz",
        mode.size().0,
        mode.size().1,
        mode.vrefresh()
    );
    
    // Find a CRTC for this connector
    let crtcs = res_handles.crtcs();
    let crtc = crtcs.first().ok_or("No CRTCs available")?.clone();
    
    tracing::info!("Using CRTC: {:?}", crtc);
    
    // Initialize GBM (Graphics Buffer Manager) for buffer allocation
    let gbm = GbmDevice::new(drm.device_fd().clone())?;
    
    // Initialize EGL for OpenGL rendering
    tracing::info!("Initializing EGL...");
    let egl_display = EGLDisplay::new(gbm.clone())?;
    let egl_context = EGLContext::new(&egl_display)?;
    
    tracing::info!("Creating renderer...");
    let renderer = unsafe { GlesRenderer::new(egl_context)? };
    
    tracing::info!("✅ Renderer initialized");
    
    // Create Wayland output
    let size = mode.size();
    let output_mode = Mode {
        size: (size.0 as i32, size.1 as i32).into(),
        refresh: (mode.vrefresh() * 1000) as i32,
    };
    
    let physical_properties = PhysicalProperties {
        size: (0, 0).into(),
        subpixel: Subpixel::Unknown,
        make: "Nuthatch".into(),
        model: "DRM".into(),
    };
    
    let output = Output::new("DRM-1".to_string(), physical_properties);
    let _global = output.create_global::<NuthatchState>(&display.handle());
    
    output.change_current_state(
        Some(output_mode),
        Some(Transform::Normal),
        None,
        Some((0, 0).into()),
    );
    output.set_preferred(output_mode);
    
    state.space.map_output(&output, (0, 0));
    
    tracing::info!("✅ Output created and mapped");
    
    Ok(())
}
