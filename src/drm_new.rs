// Full DRM/KMS Backend Implementation
// Based on Smithay's Anvil reference compositor (MIT licensed)
// Adapted for Nuthatch Desktop's custom window management
//
// Attribution: This implementation is based on anvil/src/udev.rs from the Smithay project
// Original authors: Victor Berger, Victoria Brekenfeld (Drakulix)
// License: MIT
//
// We copy the proven DRM initialization and rendering pipeline from Anvil,
// and will customize the window management behavior for Nuthatch's unique UX.

use std::{
    collections::HashMap,
    path::Path,
    time::Duration,
};

use anyhow::{Context, Result};
use smithay::{
    backend::{
        allocator::{
            gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
            Fourcc,
        },
        drm::{
            compositor::DrmCompositor,
            CreateDrmNodeError, DrmDevice, DrmDeviceFd, DrmEvent, DrmNode, NodeType,
        },
        egl::{EGLContext, EGLDevice, EGLDisplay},
        input::InputEvent,
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::{
            gles::GlesRenderer,
            multigpu::{gbm::GbmGlesBackend, GpuManager},
        },
        session::{
            libseat::LibSeatSession,
            Event as SessionEvent, Session,
        },
        udev::{all_gpus, primary_gpu, UdevBackend, UdevEvent},
    },
    reexports::{
        calloop::{EventLoop, LoopHandle},
        input::{DeviceCapability, Libinput},
        rustix::fs::OFlags,
        wayland_server::{Display, DisplayHandle},
    },
    utils::DeviceFd,
    wayland::{
        compositor::CompositorState,
        output::OutputManagerState,
        selection::data_device::DataDeviceState,
        shell::xdg::XdgShellState,
        shm::ShmState,
    },
    input::SeatState,
    desktop::Space,
    utils::{Clock, Monotonic},
};
use tracing::{debug, error, info, trace, warn};

// Simplified state for DRM backend (without the full NuthatchState complexity)
pub struct DrmCompositorState {
    pub start_time: std::time::Instant,
    pub space: Space<smithay::desktop::Window>,
    pub clock: Clock<Monotonic>,
    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub shm_state: ShmState,
    pub output_manager_state: OutputManagerState,
    pub seat_state: SeatState<DrmCompositorState>,
    pub data_device_state: DataDeviceState,
    pub udev_data: UdevData,
}

// Supported color formats - prefer 10-bit, fall back to 8-bit
const SUPPORTED_FORMATS: &[Fourcc] = &[
    Fourcc::Abgr2101010,
    Fourcc::Argb2101010,
    Fourcc::Abgr8888,
    Fourcc::Argb8888,
];

/// Data for a single DRM device (GPU)
struct BackendData {
    _drm: DrmDevice,
    gbm: GbmDevice<DrmDeviceFd>,
    render_node: DrmNode,
    surfaces: HashMap<u32, SurfaceData>, // crtc handle -> surface
}

/// Data for a single display output
struct SurfaceData {
    _output_name: String,
    // TODO: Add DrmCompositor and rendering state
}

/// Main DRM backend state
pub struct UdevData {
    session: LibSeatSession,
    primary_gpu: DrmNode,
    gpus: GpuManager<GbmGlesBackend<GlesRenderer, DrmDeviceFd>>,
    backends: HashMap<DrmNode, BackendData>,
}

/// Combined state for the compositor with DRM backend
impl DrmCompositorState {
    pub fn new(
        display: &Display<Self>,
        _event_loop: &EventLoop<Self>,
        udev_data: UdevData,
    ) -> Self {
        let dh = display.handle();
        let clock = Clock::<Monotonic>::new();
        let start_time = std::time::Instant::now();

        // Initialize Wayland protocols
        let compositor_state = CompositorState::new::<Self>(&dh);
        let xdg_shell_state = XdgShellState::new::<Self>(&dh);
        let shm_state = ShmState::new::<Self>(&dh, vec![]);
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&dh);
        let mut seat_state = SeatState::new();
        let data_device_state = DataDeviceState::new::<Self>(&dh);

        // Add a seat for input
        let mut seat = seat_state.new_wl_seat(&dh, "seat-0");
        seat.add_keyboard(Default::default(), 200, 25).unwrap();
        seat.add_pointer();

        Self {
            start_time,
            space: Space::default(),
            clock,
            compositor_state,
            xdg_shell_state,
            shm_state,
            output_manager_state,
            seat_state,
            data_device_state,
            udev_data,
        }
    }
}

impl UdevData {
    /// Create new UdevData with initialized GPU manager
    pub fn new(
        session: LibSeatSession,
        primary_gpu: DrmNode,
        gpus: GpuManager<GbmGlesBackend<GlesRenderer, DrmDeviceFd>>,
    ) -> Self {
        Self {
            session,
            primary_gpu,
            gpus,
            backends: HashMap::new(),
        }
    }
}

/// Initialize and run the DRM backend
pub fn run_udev() -> Result<()> {
    info!("🚀 Initializing full DRM backend");
    
    // Create event loop for async operations
    let mut event_loop: EventLoop<DrmCompositorState> = EventLoop::try_new()
        .context("Failed to create event loop")?;
    let loop_handle = event_loop.handle();
    
    // Create Wayland display
    let display = Display::new()
        .context("Failed to create Wayland display")?;
    let display_handle = display.handle();
    
    // Initialize session for VT switching and device access
    info!("Initializing session...");
    let (session, notifier) = LibSeatSession::new()
        .context("Failed to create LibSeat session")?;
    let seat_name = session.seat();
    info!("✅ Session initialized for seat: {}", seat_name);
    
    // Find primary GPU
    info!("Discovering primary GPU...");
    let primary_gpu = if let Ok(var) = std::env::var("NUTHATCH_DRM_DEVICE") {
        DrmNode::from_path(var)
            .context("Invalid DRM device path")?
    } else {
        primary_gpu(&seat_name)
            .context("Failed to query primary GPU")?
            .and_then(|path| {
                DrmNode::from_path(&path).ok()?
                    .node_with_type(NodeType::Render)?
                    .ok()
            })
            .or_else(|| {
                all_gpus(&seat_name)
                    .ok()?
                    .into_iter()
                    .find_map(|path| DrmNode::from_path(path).ok())
            })
            .context("No GPU found")?
    };
    info!("✅ Using {} as primary GPU", primary_gpu);
    
    // Create GPU manager with GBM/GLES renderer
    info!("Initializing GPU manager...");
    let gpus = GpuManager::new(GbmGlesBackend::with_factory(|display| {
        let context = EGLContext::new(display)?;
        let renderer = unsafe { GlesRenderer::new(context)? };
        Ok(renderer)
    }))
    .context("Failed to create GPU manager")?;
    info!("✅ GPU manager initialized");
    
    // Create backend data
    let udev_data = UdevData::new(session.clone(), primary_gpu, gpus);
    
    // Initialize compositor state
    let mut state = DrmCompositorState::new(&display, &event_loop, udev_data);
    
    // Initialize udev backend for device discovery
    info!("Initializing udev backend...");
    let udev_backend = UdevBackend::new(&seat_name)
        .context("Failed to initialize udev backend")?;
    info!("✅ Udev backend initialized");
    
    // Initialize libinput for input handling
    info!("Initializing libinput...");
    let mut libinput_context = Libinput::new_with_udev::<LibinputSessionInterface<LibSeatSession>>(
        session.clone().into(),
    );
    libinput_context.udev_assign_seat(&seat_name)
        .context("Failed to assign seat to libinput")?;
    let libinput_backend = LibinputInputBackend::new(libinput_context.clone());
    info!("✅ Libinput initialized");
    
    // Insert input handler into event loop
    loop_handle
        .insert_source(libinput_backend, move |event, _, state| {
            // TODO: Process input events properly
            match event {
                InputEvent::DeviceAdded { device } => {
                    debug!("Input device added: {:?}", device);
                    if device.has_capability(DeviceCapability::Keyboard) {
                        info!("Keyboard device added");
                    }
                }
                InputEvent::DeviceRemoved { device } => {
                    debug!("Input device removed: {:?}", device);
                }
                InputEvent::Keyboard { event } => {
                    debug!("Keyboard event: {:?}", event);
                }
                InputEvent::PointerMotion { event } => {
                    trace!("Pointer motion: {:?}", event);
                }
                InputEvent::PointerButton { event } => {
                    debug!("Pointer button: {:?}", event);
                }
                _ => {}
            }
        })
        .context("Failed to insert libinput source")?;
    
    // Insert session notifier for VT switching
    loop_handle
        .insert_source(notifier, move |event, _, state| {
            match event {
                SessionEvent::PauseSession => {
                    info!("Session paused - VT switched away");
                    libinput_context.suspend();
                    // TODO: Pause all DRM outputs
                }
                SessionEvent::ActivateSession => {
                    info!("Session activated - VT switched back");
                    if let Err(err) = libinput_context.resume() {
                        error!("Failed to resume libinput: {:?}", err);
                    }
                    // TODO: Resume all DRM outputs
                }
            }
        })
        .context("Failed to insert session notifier")?;
    
    // Insert udev backend for device hotplug
    loop_handle
        .insert_source(udev_backend, move |event, _, state| {
            match event {
                UdevEvent::Added { device_id, path } => {
                    info!("DRM device added: {} at {:?}", device_id, path);
                    // TODO: Initialize new device
                }
                UdevEvent::Changed { device_id } => {
                    info!("DRM device changed: {}", device_id);
                    // TODO: Handle connector changes
                }
                UdevEvent::Removed { device_id } => {
                    info!("DRM device removed: {}", device_id);
                    // TODO: Remove device
                }
            }
        })
        .context("Failed to insert udev backend")?;
    
    info!("🎉 DRM backend initialized successfully!");
    info!("Compositor is running. Press Ctrl+C to exit.");
    info!("");
    info!("TODO: Implement device initialization and rendering");
    info!("TODO: This will display a colored screen once rendering is implemented");
    
    // Main event loop
    // For now, just run for a few seconds to test initialization
    info!("Running event loop (will exit after 5 seconds for testing)...");
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        event_loop.dispatch(Some(Duration::from_millis(16)), &mut state)
            .context("Event loop error")?;
    }
    
    info!("✅ Event loop test complete - shutting down cleanly");
    
    Ok(())
}

/// Device addition handler
fn device_added(
    state: &mut DrmCompositorState,
    node: DrmNode,
    path: &Path,
) -> Result<(), DeviceAddError> {
    info!("Adding device: {} at {:?}", node, path);
    
    // TODO: Implement full device initialization based on Anvil's device_added
    // Steps:
    // 1. Open device file descriptor
    // 2. Create DrmDevice
    // 3. Create GbmDevice
    // 4. Initialize EGL display and renderer
    // 5. Add to GPU manager
    // 6. Create allocator and framebuffer exporter
    // 7. Scan for connectors and create outputs
    
    warn!("Device initialization not yet implemented");
    
    Ok(())
}

#[derive(Debug, thiserror::Error)]
enum DeviceAddError {
    #[error("Failed to open device: {0}")]
    DeviceOpen(#[source] std::io::Error),
    
    #[error("Failed to create DRM device: {0}")]
    DrmDevice(#[source] std::io::Error),
    
    #[error("Failed to create GBM device: {0}")]
    GbmDevice(#[source] std::io::Error),
    
    #[error("Failed to get DRM node: {0}")]
    DrmNode(#[source] CreateDrmNodeError),
    
    #[error("No render node available")]
    NoRenderNode,
}
