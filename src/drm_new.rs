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
use drm::control::{connector, crtc};
use smithay::{
    backend::{
        allocator::{
            gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
            Fourcc,
        },
        drm::{
            compositor::DrmCompositor,
            CreateDrmNodeError, DrmDevice, DrmDeviceFd, DrmEvent, DrmNode, NodeType,
            exporter::gbm::GbmFramebufferExporter,
        },
        egl::{EGLContext, EGLDevice, EGLDisplay},
        input::InputEvent,
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::{
            gles::GlesRenderer,
            multigpu::{gbm::GbmGlesBackend, GpuManager},
        },
        session::{
            libseat::{self, LibSeatSession},
            Event as SessionEvent, Session,
        },
        udev::{all_gpus, primary_gpu, UdevBackend, UdevEvent},
    },
    delegate_compositor, delegate_data_device, delegate_output, delegate_seat, delegate_shm,
    delegate_xdg_shell,
    desktop::Space,
    input::{SeatHandler, SeatState},
    reexports::{
        calloop::{EventLoop, LoopHandle, RegistrationToken},
        input::{DeviceCapability, Libinput},
        rustix::fs::OFlags,
        wayland_server::{
            backend::{ClientData, ClientId, DisconnectReason},
            protocol::{wl_seat::WlSeat, wl_surface::WlSurface},
            Display, DisplayHandle,
        },
    },
    utils::{Clock, DeviceFd, Monotonic},
    wayland::{
        buffer::BufferHandler,
        compositor::{CompositorClientState, CompositorHandler, CompositorState},
        output::{OutputHandler, OutputManagerState},
        selection::{
            data_device::{
                ClientDndGrabHandler, DataDeviceHandler, ServerDndGrabHandler, DataDeviceState,
            },
            SelectionHandler,
        },
        shell::xdg::{XdgShellHandler, XdgShellState, PopupSurface, PositionerState, ToplevelSurface},
        shm::{ShmHandler, ShmState},
    },
};
use smithay_drm_extras::drm_scanner::{DrmScanEvent, DrmScanner};
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
    drm: DrmDevice,
    gbm: GbmDevice<DrmDeviceFd>,
    render_node: DrmNode,
    registration_token: RegistrationToken,
    drm_scanner: DrmScanner,
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
    loop_handle: LoopHandle<'static, DrmCompositorState>,
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
        loop_handle: LoopHandle<'static, DrmCompositorState>,
    ) -> Self {
        Self {
            session,
            primary_gpu,
            gpus,
            backends: HashMap::new(),
            loop_handle,
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
    let loop_handle = event_loop.handle();
    let udev_data = UdevData::new(session.clone(), primary_gpu, gpus, loop_handle.clone());
    
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
        .map_err(|_| anyhow::anyhow!("Failed to assign seat to libinput"))?;
    let libinput_backend = LibinputInputBackend::new(libinput_context.clone());
    info!("✅ Libinput initialized");
    
    // Insert input handler into event loop
    loop_handle
        .insert_source(libinput_backend, move |event, _, _state| {
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
        .map_err(|e| anyhow::anyhow!("Failed to insert libinput source: {}", e))?;
    
    // Insert session notifier for VT switching
    loop_handle
        .insert_source(notifier, move |event, _, _state| {
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
        .map_err(|e| anyhow::anyhow!("Failed to insert session notifier: {}", e))?;
    
    // Insert udev backend for device hotplug
    loop_handle
        .insert_source(udev_backend, move |event, _, _state| {
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
        .map_err(|e| anyhow::anyhow!("Failed to insert udev backend: {}", e))?;
    
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

/// Handle device changes (connector hotplug, etc.)
fn device_changed(state: &mut DrmCompositorState, node: DrmNode) {
    info!("Device changed: {}, scanning connectors...", node);
    
    // Get the backend device
    let device = if let Some(device) = state.udev_data.backends.get_mut(&node) {
        device
    } else {
        warn!("Device {} not found in backends", node);
        return;
    };

    // Scan for connector changes
    let scan_result = match device.drm_scanner.scan_connectors(&device.drm) {
        Ok(scan_result) => scan_result,
        Err(err) => {
            warn!("Failed to scan connectors: {:?}", err);
            return;
        }
    };

    // Process each connector event
    for event in scan_result {
        match event {
            DrmScanEvent::Connected {
                connector,
                crtc: Some(crtc),
            } => {
                info!(
                    "Connector {}-{} connected to CRTC {:?}",
                    connector.interface().as_str(),
                    connector.interface_id(),
                    crtc
                );
                connector_connected(state, node, connector, crtc);
            }
            DrmScanEvent::Disconnected {
                connector,
                crtc: Some(crtc),
            } => {
                info!(
                    "Connector {}-{} disconnected from CRTC {:?}",
                    connector.interface().as_str(),
                    connector.interface_id(),
                    crtc
                );
                connector_disconnected(state, node, connector, crtc);
            }
            _ => {
                debug!("Unhandled connector event: {:?}", event);
            }
        }
    }
}

/// Handle connector connection
fn connector_connected(
    state: &mut DrmCompositorState,
    node: DrmNode,
    connector: connector::Info,
    crtc: crtc::Handle,
) {
    info!(
        "Setting up connector: {}-{} on CRTC {:?}",
        connector.interface().as_str(),
        connector.interface_id(),
        crtc
    );
    
    // TODO: Implement full connector setup
    // 1. Get connector properties
    // 2. Select display mode
    // 3. Create Wayland Output
    // 4. Create DRM surface
    // 5. Set up DrmCompositor
    // 6. Store SurfaceData
    
    warn!("Connector setup not yet implemented");
}

/// Handle connector disconnection
fn connector_disconnected(
    _state: &mut DrmCompositorState,
    node: DrmNode,
    connector: connector::Info,
    crtc: crtc::Handle,
) {
    info!(
        "Disconnecting connector: {}-{} from CRTC {:?}",
        connector.interface().as_str(),
        connector.interface_id(),
        crtc
    );
    
    // TODO: Clean up surface, remove output
}

/// Device addition handler
fn device_added(
    state: &mut DrmCompositorState,
    node: DrmNode,
    path: &Path,
) -> Result<(), DeviceAddError> {
    info!("Adding DRM device: {} at {:?}", node, path);
    
    // 1. Open device file descriptor using session
    let fd = state
        .udev_data
        .session
        .open(
            path,
            OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK,
        )
        .map_err(DeviceAddError::DeviceOpen)?;

    let fd = DrmDeviceFd::new(DeviceFd::from(fd));
    info!("✅ Opened device FD");

    // 2. Create DRM device and event notifier
    let (drm, notifier) = DrmDevice::new(fd.clone(), true)
        .map_err(DeviceAddError::DrmDevice)?;
    info!("✅ Created DRM device");

    // 3. Create GBM device for buffer allocation
    let gbm = GbmDevice::new(fd)
        .map_err(DeviceAddError::GbmDevice)?;
    info!("✅ Created GBM device");

    // 4. Register DRM event handler for VBlank
    let registration_token = state
        .udev_data
        .loop_handle
        .insert_source(
            notifier,
            move |event, _metadata, data: &mut DrmCompositorState| match event {
                DrmEvent::VBlank(crtc) => {
                    debug!("VBlank event for CRTC {:?}", crtc);
                    // TODO: Call frame_finish when implemented
                    // frame_finish(data, node, crtc, metadata);
                }
                DrmEvent::Error(error) => {
                    error!("DRM error: {:?}", error);
                }
            },
        )
        .map_err(|e| anyhow::anyhow!("Failed to register DRM event source: {:?}", e))
        .map_err(DeviceAddError::EventLoop)?;
    info!("✅ Registered VBlank event handler");

    // 5. Try to initialize EGL and add to GPU manager
    let render_node = {
        let display = unsafe { EGLDisplay::new(gbm.clone()) }
            .map_err(|e| DeviceAddError::AddNode(anyhow::anyhow!("Failed to create EGL display: {}", e)))?;
        
        let egl_device = EGLDevice::device_for_display(&display)
            .map_err(|e| DeviceAddError::AddNode(anyhow::anyhow!("Failed to get EGL device: {}", e)))?;

        if egl_device.is_software() {
            warn!("Device is using software rendering!");
        }

        let render_node = egl_device
            .try_get_render_node()
            .ok()
            .flatten()
            .unwrap_or(node);

        state
            .udev_data
            .gpus
            .as_mut()
            .add_node(render_node, gbm.clone())
            .map_err(|e| DeviceAddError::AddNode(anyhow::anyhow!("Failed to add GPU node: {}", e)))?;

        render_node
    };
    info!("✅ Initialized EGL and added to GPU manager (render node: {})", render_node);

    // 6. Store backend data
    let backend_data = BackendData {
        drm,
        gbm,
        render_node,
        registration_token,
        drm_scanner: DrmScanner::new(),
        surfaces: HashMap::new(),
    };

    state.udev_data.backends.insert(node, backend_data);
    info!("✅ Device {} fully initialized!", node);

    // 7. Scan for connectors (will be done in device_changed)
    device_changed(state, node);

    Ok(())
}

#[derive(Debug, thiserror::Error)]
enum DeviceAddError {
    #[error("Failed to open device: {0}")]
    DeviceOpen(#[source] libseat::Error),
    
    #[error("Failed to create DRM device: {0}")]
    DrmDevice(#[source] smithay::backend::drm::DrmError),
    
    #[error("Failed to create GBM device: {0}")]
    GbmDevice(#[source] std::io::Error),
    
    #[error("Failed to add node to GPU manager: {0}")]
    AddNode(#[source] anyhow::Error),
    
    #[error("Failed to register event source: {0}")]
    EventLoop(#[source] anyhow::Error),
    
    #[error("Failed to get DRM node: {0}")]
    DrmNode(#[source] CreateDrmNodeError),
    
    #[error("No render node available")]
    NoRenderNode,
}

// ============================================================================
// Smithay Protocol Handler Implementations
// Based on Anvil's handlers - these are required for Wayland protocol support
// ============================================================================

// Client state for tracking per-client data
pub struct ClientState {
    pub compositor_state: CompositorClientState,
}

impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}

// Compositor handler - handles surface creation and commits
impl CompositorHandler for DrmCompositorState {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(
        &self,
        client: &'a smithay::reexports::wayland_server::Client,
    ) -> &'a CompositorClientState {
        &client.get_data::<ClientState>().unwrap().compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        trace!("Surface committed: {:?}", surface);
        // TODO: Handle surface commits - update window state
    }
}

// XDG Shell handler - handles window management
impl XdgShellHandler for DrmCompositorState {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, _surface: ToplevelSurface) {
        info!("New toplevel window created");
        // TODO: Add window to space
    }

    fn toplevel_destroyed(&mut self, _surface: ToplevelSurface) {
        info!("Toplevel window destroyed");
        // TODO: Remove window from space
    }

    fn new_popup(&mut self, _surface: PopupSurface, _positioner: PositionerState) {
        debug!("New popup created");
        // TODO: Handle popup positioning
    }

    fn popup_destroyed(&mut self, _surface: PopupSurface) {
        debug!("Popup destroyed");
    }

    fn reposition_request(&mut self, _surface: PopupSurface, _positioner: PositionerState, _token: u32) {
        debug!("Popup reposition requested");
        // TODO: Handle popup repositioning
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: WlSeat, _serial: smithay::utils::Serial) {
        debug!("Popup grab requested");
        // TODO: Handle popup grabs
    }
}

// SHM handler - handles shared memory buffers
impl ShmHandler for DrmCompositorState {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

// Buffer handler - handles buffer management
impl BufferHandler for DrmCompositorState {
    fn buffer_destroyed(&mut self, _buffer: &smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer) {
        trace!("Buffer destroyed");
    }
}

// Seat handler - handles input seat management
impl SeatHandler for DrmCompositorState {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.seat_state
    }

    fn focus_changed(&mut self, _seat: &smithay::input::Seat<Self>, _focused: Option<&Self::KeyboardFocus>) {
        debug!("Keyboard focus changed");
    }

    fn cursor_image(&mut self, _seat: &smithay::input::Seat<Self>, _image: smithay::input::pointer::CursorImageStatus) {
        trace!("Cursor image changed");
        // TODO: Update cursor rendering
    }
}

// Data device handler - handles clipboard and drag-and-drop
impl DataDeviceHandler for DrmCompositorState {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}

impl ClientDndGrabHandler for DrmCompositorState {}
impl ServerDndGrabHandler for DrmCompositorState {}

// Selection handler - handles selection (clipboard)
impl SelectionHandler for DrmCompositorState {
    type SelectionUserData = ();
}

// Output handler - handles output (display) management
impl OutputHandler for DrmCompositorState {}

// Use Smithay's delegate macros to wire up the protocol handlers
delegate_compositor!(DrmCompositorState);
delegate_xdg_shell!(DrmCompositorState);
delegate_shm!(DrmCompositorState);
delegate_seat!(DrmCompositorState);
delegate_data_device!(DrmCompositorState);
delegate_output!(DrmCompositorState);
