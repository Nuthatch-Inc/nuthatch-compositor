// DRM/KMS Backend - BEING REWRITTEN
// This file is being replaced with Anvil-based implementation
// See drm_minimal.rs for the new approach

#![allow(dead_code, unused_imports, unused_variables)]

use smithay::{
    backend::{
        allocator::gbm::GbmDevice,
        drm::{DrmDevice, DrmDeviceFd, DrmEvent, DrmNode, NodeType},
        egl::{EGLContext, EGLDisplay},
        libinput::LibinputInputBackend,
        renderer::{gles::GlesRenderer, Bind, Frame, Renderer},
        session::{libseat::LibSeatSession, Session},
        udev::{all_gpus, primary_gpu, UdevBackend, UdevEvent},
    },
    output::{Mode, Output, PhysicalProperties, Subpixel},
    reexports::{
        calloop::EventLoop,
        drm::control::{connector, crtc, Device, ModeTypeFlags},
        input::Libinput,
        rustix::fs::OFlags,
        wayland_server::Display,
    },
    utils::{DeviceFd, Transform},
};

use crate::state::NuthatchState;

use std::{
    collections::HashMap,
    path::Path,
    time::Duration,
};

struct DrmBackendData {
    _drm: DrmDevice,
    gbm: GbmDevice<DrmDeviceFd>,
    _render_node: DrmNode,
}

/// Initialize DRM/KMS backend
/// This is being rewritten based on Anvil's approach
pub fn init_drm() -> anyhow::Result<()> {
    // TODO: Implement full DRM backend
    // For now, see drm_minimal.rs for basic tests
    unimplemented!("DRM backend being rewritten - use drm_minimal for now");
}

/*
// OLD IMPLEMENTATION - Being replaced
pub fn init_drm_old() -> anyhow::Result<()> {
    tracing::info!("🚀 Initializing DRM/KMS backend");
    
    // Create event loop
    let mut event_loop: EventLoop<NuthatchState> = EventLoop::try_new()?;
    let loop_handle = event_loop.handle();
    
    // Create Wayland display
    let mut display: Display<NuthatchState> = Display::new()?;
    
    // Initialize compositor state
    let mut state = NuthatchState::new(&mut display, &event_loop);

    // Initialize session for VT switching and device access
    tracing::info!("Starting libseat session...");
    let (session, notifier) = LibSeatSession::new()?;
    
    // Insert session notifier into event loop
    loop_handle.insert_source(notifier, |_, _, _| {})?;
    
    let seat_name = session.seat();
    tracing::info!("Session started on seat: {}", seat_name);
    
    // Initialize libinput for keyboard/mouse input
    tracing::info!("Initializing libinput...");
    let mut libinput_context: Libinput = Libinput::new_with_udev(session.clone().into());
    libinput_context.udev_assign_seat(&seat_name)
        .map_err(|_| "Failed to assign seat to libinput")?;
    
    let libinput_backend = LibinputInputBackend::new(libinput_context);
    
    // Add input event handler to event loop
    loop_handle.insert_source(libinput_backend, move |event, _, _state| {
        tracing::info!("Input event: {:?}", event);
        // For now, just log all input events
        // TODO: Process keyboard/mouse events properly
    })?;
    
    tracing::info!("✅ Input handling initialized");
    
    // Find primary GPU  
    let primary_gpu = primary_gpu(&seat_name)?
        .and_then(|path| DrmNode::from_path(path).ok())
        .or_else(|| {
            all_gpus(&seat_name)
                .ok()?
                .into_iter()
                .find_map(|path| DrmNode::from_path(path).ok())
        })
        .ok_or("No GPU found!")?;
    
    tracing::info!("Using {:?} as primary GPU", primary_gpu);
    
    // Initialize udev backend to handle GPU device discovery
    tracing::info!("Setting up udev backend...");
    let udev_backend = UdevBackend::new(&seat_name)?;
    
    // Store backend data
    let mut backends: HashMap<DrmNode, DrmBackendData> = HashMap::new();
    
    // Process initial devices
    for (device_id, path) in udev_backend.device_list() {
        tracing::info!("Found device: {:?} at {:?}", device_id, path);
        
        if let Ok(node) = DrmNode::from_dev_id(device_id) {
            match device_added(&mut session.clone(), node, &path, &mut display, &mut state, &loop_handle) {
                Ok(backend_data) => {
                    backends.insert(node, backend_data);
                    tracing::info!("✅ Successfully initialized device: {:?}", node);
                }
                Err(e) => {
                    tracing::warn!("Failed to initialize device {:?}: {}", node, e);
                }
            }
        }
    }
    
    if backends.is_empty() {
        return Err("No usable GPU devices found!".into());
    }
    
    // Set up udev event handler
    loop_handle.insert_source(udev_backend, move |event, _, _state| {
        match event {
            UdevEvent::Added { device_id, path } => {
                tracing::info!("Device added: {:?}", device_id);
                // TODO: Handle hotplug
            }
            UdevEvent::Changed { device_id } => {
                tracing::debug!("Device changed: {:?}", device_id);
            }
            UdevEvent::Removed { device_id } => {
                tracing::info!("Device removed: {:?}", device_id);
                // TODO: Handle removal
            }
        }
    })?;

    tracing::info!("🎨 Compositor ready - clients can connect");
    tracing::info!("Wayland socket: wayland-1 (should be auto-created)");
    
    // Main event loop
    loop {
        // Dispatch Wayland events
        display.dispatch_clients(&mut state)?;
        
        // Run event loop
        event_loop.dispatch(Duration::from_millis(16), &mut state)?;
        
        // Flush clients
        display.flush_clients()?;
    }
}

fn device_added(
    session: &mut LibSeatSession,
    node: DrmNode,
    path: &Path,
    display: &mut Display<NuthatchState>,
    state: &mut NuthatchState,
    loop_handle: &calloop::LoopHandle<NuthatchState>,
) -> Result<DrmBackendData, Box<dyn std::error::Error>> {
    tracing::info!("Opening DRM device: {:?}", path);
    
    // Open the device with proper flags
    let fd = session.open(
        path,
        OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK,
    )?;
    
    // Wrap in DeviceFd and DrmDeviceFd
    let device_fd = DeviceFd::from(fd);
    let drm_fd = DrmDeviceFd::new(device_fd);
    
    // Create DRM device
    let (drm, drm_notifier) = DrmDevice::new(drm_fd.clone(), true)?;
    
    // Set up DRM event handler
    loop_handle.insert_source(drm_notifier, move |event, _metadata, _state| {
        match event {
            DrmEvent::VBlank(crtc) => {
                tracing::trace!("VBlank on crtc {:?}", crtc);
            }
            DrmEvent::Error(error) => {
                tracing::error!("DRM error: {:?}", error);
            }
        }
    })?;
    
    tracing::info!("DRM device created");
    
    // Create GBM device for buffer allocation
    let gbm = GbmDevice::new(drm_fd)?;
    
    // Initialize EGL for OpenGL rendering
    tracing::info!("Initializing EGL...");
    let egl_display = unsafe { EGLDisplay::new(gbm.clone())? };
    let egl_context = EGLContext::new(&egl_display)?;
    
    tracing::info!("Creating renderer...");
    let mut renderer = unsafe { GlesRenderer::new(egl_context)? };
    
    tracing::info!("✅ Renderer initialized");
    
    // Get resource handles to find connectors
    let res_handles = drm.resource_handles()?;
    
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
    
    let (_connector_handle, connector_info) = connector_info
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
        model: format!("{:?}", connector_info.interface()),
    };
    
    let output_name = format!("{}-{}", 
        connector_info.interface().as_str(),
        connector_info.interface_id()
    );
    
    let output = Output::new(output_name.clone(), physical_properties);
    let _global = output.create_global::<NuthatchState>(&display.handle());
    
    output.change_current_state(
        Some(output_mode),
        Some(Transform::Normal),
        None,
        Some((0, 0).into()),
    );
    output.set_preferred(output_mode);
    
    state.space.map_output(&output, (0, 0));
    
    tracing::info!("✅ Output '{}' created and mapped", output_name);
    
    // TODO: Set up actual rendering pipeline with DrmCompositor
    tracing::info!("Note: Rendering pipeline not yet implemented");
    
    Ok(DrmBackendData {
        _drm: drm,
        gbm,
        _render_node: node,
    })
}
*/
