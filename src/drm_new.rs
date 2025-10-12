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
use drm::control::{connector, crtc, ModeTypeFlags};
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
            output::{DrmOutput, DrmOutputManager, DrmOutputRenderElements},
        },
        renderer::element::{
            RenderElement,
            memory::MemoryRenderBufferRenderElement,
        },
        egl::{EGLContext, EGLDevice, EGLDisplay},
        input::InputEvent,
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::{
            gles::GlesRenderer,
            ImportMem,
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
    input::{SeatHandler, SeatState, Seat},
    output::{Mode as WlMode, Output, PhysicalProperties},
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
use smithay::backend::renderer::{
    element::{
        memory::MemoryRenderBuffer,
        AsRenderElements, Kind,
    },
    Texture,
};
use smithay::utils::{Logical, Point, Scale, Physical, Transform};
use tracing::{debug, error, info, trace, warn};

/// Convert HSV hue (0-360) to RGB (0.0-1.0) with full saturation and value
fn hue_to_rgb(hue: f32) -> (f32, f32, f32) {
    let h = hue / 60.0;
    let c = 1.0;  // Full saturation and value
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    
    let (r, g, b) = match h as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    
    (r, g, b)
}

// Simple render element type for our compositor
// For now we only support memory buffer rendering (for simple shapes/colors)
smithay::backend::renderer::element::render_elements! {
    pub NuthatchRenderElements<R> where R: ImportMem;
    Memory=MemoryRenderBufferRenderElement<R>,
}

// Implement Debug for NuthatchRenderElements
impl<R: smithay::backend::renderer::Renderer> std::fmt::Debug for NuthatchRenderElements<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Memory(arg0) => f.debug_tuple("Memory").field(arg0).finish(),
            Self::_GenericCatcher(arg0) => f.debug_tuple("_GenericCatcher").field(arg0).finish(),
        }
    }
}

/// Pointer/cursor element for rendering the mouse cursor
pub struct PointerElement {
    buffer: Option<MemoryRenderBuffer>,
}

impl Default for PointerElement {
    fn default() -> Self {
        Self {
            buffer: None,
        }
    }
}

impl PointerElement {
    pub fn set_buffer(&mut self, buffer: MemoryRenderBuffer) {
        self.buffer = Some(buffer);
    }
}

impl<T, R> AsRenderElements<R> for PointerElement
where
    T: Texture + Clone + Send + 'static,
    R: smithay::backend::renderer::Renderer<TextureId = T> + ImportMem,
{
    type RenderElement = MemoryRenderBufferRenderElement<R>;
    
    fn render_elements<E>(
        &self,
        renderer: &mut R,
        location: Point<i32, Physical>,
        scale: Scale<f64>,
        _alpha: f32,
    ) -> Vec<E>
    where
        E: From<MemoryRenderBufferRenderElement<R>>,
    {
        if let Some(buffer) = &self.buffer {
            vec![MemoryRenderBufferRenderElement::from_buffer(
                renderer,
                location.to_f64(),
                buffer,
                None,
                None,
                None,
                Kind::Cursor,
            )
            .expect("Failed to create cursor render element")
            .into()]
        } else {
            vec![]
        }
    }
}

// Simplified state for DRM backend (without the full NuthatchState complexity)
pub struct DrmCompositorState {
    pub start_time: std::time::Instant,
    pub space: Space<smithay::desktop::Window>,
    pub clock: Clock<Monotonic>,
    pub display_handle: DisplayHandle,
    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub shm_state: ShmState,
    pub output_manager_state: OutputManagerState,
    pub seat_state: SeatState<DrmCompositorState>,
    pub seat: Seat<DrmCompositorState>,  // Store the seat for easy access
    pub data_device_state: DataDeviceState,
    pub udev_data: UdevData,
    pub frame_count: u64,  // Frame counter for animation
    pub running: bool,  // Track if compositor should keep running
    pub pointer_location: Point<f64, Logical>,  // Current cursor position
    pub cursor: crate::cursor::Cursor,  // Cursor theme and images
    pub pointer_element: PointerElement,  // Cursor rendering element
    pub pointer_image: Option<MemoryRenderBuffer>,  // Cached cursor image
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
    drm_output_manager: DrmOutputManager<
        GbmAllocator<DrmDeviceFd>,
        GbmFramebufferExporter<DrmDeviceFd>,
        (),  // Simplified - no presentation feedback for now
        DrmDeviceFd,
    >,
    gbm: GbmDevice<DrmDeviceFd>,
    render_node: DrmNode,
    registration_token: RegistrationToken,
    drm_scanner: DrmScanner,
    surfaces: HashMap<u32, SurfaceData>, // crtc handle -> surface
}

/// Data for a single display output
struct SurfaceData {
    output: Output,
    drm_output: Option<DrmOutput<
        GbmAllocator<DrmDeviceFd>,
        GbmFramebufferExporter<DrmDeviceFd>,
        (),  // Simplified - no presentation feedback
        DrmDeviceFd,
    >>,
    render_node: DrmNode,
    connector: connector::Handle,
    mode: drm::control::Mode,
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

        // Load cursor theme
        info!("Loading cursor theme...");
        let cursor = crate::cursor::Cursor::load();
        info!("✅ Cursor theme loaded");

        // Initialize pointer at screen center (assuming 1920x1200 for now)
        let pointer_location = Point::from((960.0, 600.0));
        info!("🖱️  Initial pointer location: ({}, {})", pointer_location.x, pointer_location.y);

        Self {
            start_time,
            space: Space::default(),
            clock,
            display_handle: dh,
            compositor_state,
            xdg_shell_state,
            shm_state,
            output_manager_state,
            seat_state,
            seat,  // Store the seat for input handling
            data_device_state,
            udev_data,
            frame_count: 0,
            running: true,
            pointer_location,
            cursor,
            pointer_element: PointerElement::default(),
            pointer_image: None,
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
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    info!("🚀 STARTING FULL DRM BACKEND");
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    // Create event loop for async operations
    info!("Step 1: Creating event loop...");
    let mut event_loop: EventLoop<DrmCompositorState> = EventLoop::try_new()
        .context("Failed to create event loop")?;
    let loop_handle = event_loop.handle();
    info!("✅ Event loop created");
    
    // Create Wayland display
    info!("Step 2: Creating Wayland display...");
    let display = Display::new()
        .context("Failed to create Wayland display")?;
    let display_handle = display.handle();
    info!("✅ Wayland display created");
    
    // Initialize session for VT switching and device access
    info!("Step 3: Initializing session...");
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
    let mut udev_backend = UdevBackend::new(&seat_name)
        .context("Failed to initialize udev backend")?;
    info!("✅ Udev backend initialized");
    
    // Enumerate existing devices BEFORE inserting backend into event loop
    // Collect into owned Vec to drop the borrow
    info!("Enumerating existing DRM devices...");
    let existing_devices: Vec<(u64, std::path::PathBuf)> = udev_backend
        .device_list()
        .map(|(id, path)| (id, path.to_path_buf()))
        .collect();
    
    // Initialize libinput for input handling
    info!("Initializing libinput...");
    let mut libinput_context = Libinput::new_with_udev::<LibinputSessionInterface<LibSeatSession>>(
        session.clone().into(),
    );
    libinput_context.udev_assign_seat(&seat_name)
        .map_err(|_| anyhow::anyhow!("Failed to assign seat to libinput"))?;
    let libinput_backend = LibinputInputBackend::new(libinput_context.clone());
    info!("✅ Libinput initialized");
    
    // Set up libinput for input events (keyboard, mouse, etc.)
    loop_handle
        .insert_source(libinput_backend, move |event, _, state| {
            use smithay::backend::input::{KeyState, KeyboardKeyEvent, Event as InputEventTrait};
            use smithay::input::keyboard::FilterResult;
            use smithay::utils::SERIAL_COUNTER;
            
            match event {
                InputEvent::Keyboard { event } => {
                    let keycode = event.key_code();
                    let key_state = event.state();
                    let time = InputEventTrait::time_msec(&event);
                    let serial = SERIAL_COUNTER.next_serial();
                    
                    // Use keyboard.input() to properly update modifier state
                    if let Some(keyboard) = state.seat.get_keyboard() {
                        keyboard.input(
                            state,
                            keycode,
                            key_state,
                            serial,
                            time,
                            |_state, modifiers, handle| {
                                // Check for exit shortcuts on key press
                                if key_state == KeyState::Pressed {
                                    let raw_code = keycode.raw();
                                    let is_q = raw_code == 24;  // Q key
                                    let is_backspace = raw_code == 22;  // Backspace
                                    
                                    if modifiers.ctrl && modifiers.alt && (is_q || is_backspace) {
                                        info!("🛑 Exit key combination detected (Ctrl+Alt+{}) - shutting down gracefully",
                                              if is_q { "Q" } else { "Backspace" });
                                        // Signal exit - but we can't set state.running here due to borrow
                                        // So we'll return Intercept to signal we handled it
                                        return FilterResult::Intercept(true);
                                    }
                                    
                                    // Log key presses with modifier state for debugging
                                    let keysym = handle.modified_sym();
                                    info!(
                                        "⌨️  Key pressed: code={} keysym={} (ctrl={} alt={} shift={})",
                                        raw_code,
                                        xkbcommon::xkb::keysym_get_name(keysym),
                                        modifiers.ctrl, modifiers.alt, modifiers.shift
                                    );
                                }
                                
                                FilterResult::Forward  // Forward other keys normally
                            }
                        );
                        
                        // Check if we should exit (ugly workaround for borrow checker)
                        // The FilterResult::Intercept(true) signals we should exit
                        if key_state == KeyState::Pressed {
                            if let Some(kbd) = state.seat.get_keyboard() {
                                let mods = kbd.modifier_state();
                                let raw_code = keycode.raw();
                                let is_q = raw_code == 24;
                                let is_backspace = raw_code == 22;
                                if mods.ctrl && mods.alt && (is_q || is_backspace) {
                                    state.running = false;
                                }
                            }
                        }
                    }
                }
                InputEvent::DeviceAdded { device } => {
                    info!("🔌 Input device added: {:?}", device.name());
                }
                InputEvent::DeviceRemoved { device } => {
                    info!("🔌 Input device removed: {:?}", device.name());
                }
                InputEvent::PointerMotion { event } => {
                    use smithay::backend::input::{PointerMotionEvent};
                    let delta = event.delta();
                    state.pointer_location += delta;
                    // Clamp to screen bounds (assuming 1920x1200)
                    state.pointer_location.x = state.pointer_location.x.max(0.0).min(1920.0);
                    state.pointer_location.y = state.pointer_location.y.max(0.0).min(1200.0);
                    info!("🖱️  Pointer moved: delta=({:.2}, {:.2}) -> pos=({:.1}, {:.1})", 
                           delta.x, delta.y, state.pointer_location.x, state.pointer_location.y);
                }
                InputEvent::PointerButton { event } => {
                    use smithay::backend::input::PointerButtonEvent;
                    let button = event.button_code();
                    debug!("🖱️  Mouse button: code={}", button);
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
        .insert_source(udev_backend, move |event, _, state| {
            match event {
                UdevEvent::Added { device_id, path } => {
                    info!("DRM device added: {} at {:?}", device_id, path);
                    if let Ok(node) = DrmNode::from_dev_id(device_id) {
                        if let Err(e) = device_added(state, node, &path) {
                            error!("Failed to add device {}: {}", device_id, e);
                        }
                    } else {
                        error!("Invalid device id: {}", device_id);
                    }
                }
                UdevEvent::Changed { device_id } => {
                    info!("DRM device changed: {}", device_id);
                    if let Ok(node) = DrmNode::from_dev_id(device_id) {
                        device_changed(state, node);
                    } else {
                        error!("Invalid device id: {}", device_id);
                    }
                }
                UdevEvent::Removed { device_id } => {
                    info!("DRM device removed: {}", device_id);
                    // TODO: Remove device - implement device_removed()
                }
            }
        })
        .map_err(|e| anyhow::anyhow!("Failed to insert udev backend: {}", e))?;
    
    // Initialize existing devices
    info!("Initializing {} existing DRM devices...", existing_devices.len());
    for (device_id, path) in existing_devices {
        info!("🔍 Processing device: {} at {:?}", device_id, path);
        if let Ok(node) = DrmNode::from_dev_id(device_id) {
            info!("✅ Converted device_id {} to DrmNode: {}", device_id, node);
            if let Err(e) = device_added(&mut state, node, &path) {
                error!("❌ Failed to initialize device {}: {}", device_id, e);
            } else {
                info!("✅ Successfully initialized device {}", device_id);
            }
        } else {
            error!("❌ Invalid device id: {}", device_id);
        }
    }
    
    info!("🎉 DRM backend initialized successfully!");
    info!("📊 Event loop status: Starting...");
    info!("Compositor is running. Press Ctrl+Alt+Q or Ctrl+Alt+Backspace to exit.");
    info!("⚠️  SAFETY: Will auto-exit after 10 seconds for testing");
    
    // Main event loop - run until user quits or timeout
    info!("🔄 Entering main event loop...");
    let start_time = std::time::Instant::now();
    let mut iteration = 0u64;
    loop {
        // Check if user requested exit via keyboard shortcut
        if !state.running {
            info!("👋 User requested exit - shutting down gracefully");
            break;
        }
        
        // SAFETY: Auto-exit after 10 seconds to prevent hangs during testing
        if start_time.elapsed() > Duration::from_secs(10) {
            info!("⏱️  10 second timeout reached - exiting for safety");
            break;
        }
        
        iteration += 1;
        if iteration % 60 == 0 {  // Log every ~1 second at 60fps
            info!("Event loop iteration: {} ({}s elapsed)", iteration, start_time.elapsed().as_secs());
        }
        
        match event_loop.dispatch(Some(Duration::from_millis(16)), &mut state) {
            Ok(_) => {},
            Err(e) => {
                error!("❌ Event loop error: {:?}", e);
                return Err(e).context("Event loop error");
            }
        }
    }
    
    info!("🛑 Exiting compositor safely...");
    Ok(())
}

/// Handle device changes (connector hotplug, etc.)
fn device_changed(state: &mut DrmCompositorState, node: DrmNode) {
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    info!("🔌 DEVICE_CHANGED called for {}", node);
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    // Get the backend device
    let device = if let Some(device) = state.udev_data.backends.get_mut(&node) {
        info!("✅ Found device in backends");
        device
    } else {
        warn!("❌ Device {} not found in backends", node);
        return;
    };

    // Scan for connector changes
    info!("Scanning connectors...");
    let scan_result = match device.drm_scanner.scan_connectors(device.drm_output_manager.device()) {
        Ok(scan_result) => {
            info!("✅ Connector scan successful");
            scan_result
        }
        Err(err) => {
            warn!("❌ Failed to scan connectors: {:?}", err);
            return;
        }
    };

    info!("Processing connector events...");
    info!("   Connected: {}, Disconnected: {}", scan_result.connected.len(), scan_result.disconnected.len());
    
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
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    info!("🔗 CONNECTOR_CONNECTED called");
    info!(
        "   Connector: {}-{} on CRTC {:?}",
        connector.interface().as_str(),
        connector.interface_id(),
        crtc
    );
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    // Get the backend device
    let device = if let Some(device) = state.udev_data.backends.get_mut(&node) {
        device
    } else {
        warn!("Device {} not found in backends", node);
        return;
    };

    // Create output name
    let output_name = format!("{}-{}", connector.interface().as_str(), connector.interface_id());
    info!("Output name: {}", output_name);
    
    // Select display mode (prefer the first preferred mode, or use first available)
    let mode_id = connector
        .modes()
        .iter()
        .position(|mode| mode.mode_type().contains(ModeTypeFlags::PREFERRED))
        .unwrap_or(0);
    
    let drm_mode = connector.modes()[mode_id];
    let wl_mode = WlMode::from(drm_mode);
    
    info!(
        "✅ Selected mode for {}: {}x{}@{:.2}Hz",
        output_name,
        wl_mode.size.w,
        wl_mode.size.h,
        wl_mode.refresh as f64 / 1000.0
    );
    
    // Get physical size
    let (phys_w, phys_h) = connector.size().unwrap_or((0, 0));
    info!("Physical size: {}x{} mm", phys_w, phys_h);
    
    // Create Wayland Output
    info!("Creating Wayland Output...");
    let output = Output::new(
        output_name.clone(),
        PhysicalProperties {
            size: (phys_w as i32, phys_h as i32).into(),
            subpixel: connector.subpixel().into(),
            make: "Unknown".into(),
            model: "Unknown".into(),
        },
    );
    info!("✅ Created Wayland Output");
    
    // Create global for clients
    info!("Creating global for clients...");
    let _global = output.create_global::<DrmCompositorState>(&state.display_handle);
    info!("✅ Created global");
    
    // Calculate position (place outputs side by side)
    let x = state
        .space
        .outputs()
        .fold(0, |acc, o| acc + state.space.output_geometry(o).unwrap().size.w);
    let position = (x, 0).into();
    info!("Output position: {:?}", position);
    
    // Configure output
    info!("Configuring output state...");
    output.set_preferred(wl_mode);
    output.change_current_state(Some(wl_mode), None, None, Some(position));
    state.space.map_output(&output, position);
    info!("✅ Output configured and mapped to space");
    
    info!(
        "✅ Output {} created at position {:?} with mode {}x{}",
        output_name, position, wl_mode.size.w, wl_mode.size.h
    );
    
    info!("Preparing surface data...");
    info!("   DRM output will be initialized during first render");
    
    // Store surface data (DRM output will be created during rendering)
    let surface = SurfaceData {
        output: output.clone(),
        drm_output: None,  // Will be initialized on first frame
        render_node: device.render_node,
        connector: connector.handle(),
        mode: drm_mode,
    };
    
    info!("Storing surface data for CRTC {:?}...", crtc);
    device.surfaces.insert(crtc.into(), surface);
    info!("✅ Surface stored (total surfaces: {})", device.surfaces.len());
    
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    info!("✅ CONNECTOR_CONNECTED COMPLETE: {}", output_name);
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    // Trigger initial render to start VBlank cycle
    info!("🎬 Triggering initial render to start VBlank events...");
    render_surface(state, node, crtc);
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

/// Render a frame for a specific surface
fn render_surface(
    state: &mut DrmCompositorState,
    node: DrmNode,
    crtc: crtc::Handle,
) {
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    info!("🎬 RENDER_SURFACE called");
    info!("   Node: {}, CRTC: {:?}", node, crtc);
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    // Check if DRM output needs initialization (don't hold device borrow)
    let needs_init = state.udev_data.backends.get(&node)
        .and_then(|d| d.surfaces.get(&(crtc.into())))
        .map(|s| s.drm_output.is_none())
        .unwrap_or(false);
    
    if needs_init {
        // Initialize DRM output on first render
        info!("🎨 Initializing DRM output for first render!");
        
        // CRITICAL: Get render_node WITHOUT holding mutable borrows
        let render_node = state.udev_data.backends.get(&node)
            .and_then(|d| d.surfaces.get(&(crtc.into())))
            .map(|s| s.render_node.clone())
            .expect("Surface must exist");
        
        // Get renderer (accessing GPU manager - must not have device borrow active)
        let mut renderer = state.udev_data.gpus.single_renderer(&render_node).unwrap();
        
        // NOW get mutable device reference for initialization
        let device = state.udev_data.backends.get_mut(&node).expect("Device must exist");
        let surface = device.surfaces.get_mut(&(crtc.into())).unwrap();
        
        // Create empty render elements for initialization
        use smithay::backend::renderer::multigpu::MultiRenderer;
        type NuthatchMultiRenderer<'a, 'b> = MultiRenderer<
            'a,
            'b,
            GbmGlesBackend<GlesRenderer, DrmDeviceFd>,
            GbmGlesBackend<GlesRenderer, DrmDeviceFd>,
        >;
        
        let render_elements: DrmOutputRenderElements<NuthatchMultiRenderer, NuthatchRenderElements<NuthatchMultiRenderer>> = 
            DrmOutputRenderElements::new();
        
        match device.drm_output_manager.initialize_output(
            crtc,
            surface.mode,
            &[surface.connector],
            &surface.output,
            None,  // No plane restrictions for now
            &mut renderer,
            &render_elements,
        ) {
            Ok(drm_output) => {
                info!("✅ DRM output initialized successfully!");
                surface.drm_output = Some(drm_output);
            }
            Err(e) => {
                error!("Failed to initialize DRM output: {}", e);
                return;
            }
        }
        // device and surface borrows dropped here
    }
    
    // CRITICAL BORROW ORDERING:
    // 1. Get render_node WITHOUT holding device mutable borrow
    let render_node = state.udev_data.backends.get(&node)
        .and_then(|d| d.surfaces.get(&(crtc.into())))
        .map(|s| s.render_node.clone())
        .expect("Surface must exist");
    
    info!("🎨 Getting renderer...");
    // 2. Get renderer (needs access to state.udev_data.gpus)
    let mut renderer = state.udev_data.gpus.single_renderer(&render_node).unwrap();
    
    info!("✅ Got renderer, now getting output...");
    // 3. Get mutable device/surface references to extract drm_output
    let mut drm_output = {
        let device = state.udev_data.backends.get_mut(&node).expect("Device must exist");
        let surface = device.surfaces.get_mut(&(crtc.into())).unwrap();
        // Take ownership of drm_output to release device borrow
        surface.drm_output.take().expect("DRM output must exist")
    }; // device borrow dropped here
    
    info!("🎨 Getting renderer...");
    let mut renderer = state.udev_data.gpus.single_renderer(&state.udev_data.primary_gpu)
        .expect("Failed to get renderer");
    info!("✅ Got renderer, now getting output...");

    info!("🎨 Rendering frame...");
    // Animate color based on frame count to create changing content
    // This ensures damage tracking detects changes and allows continuous VBlanks
    state.frame_count += 1;
    let hue = (state.frame_count as f32 * 2.0) % 360.0;  // Cycle through hues
    let (r, g, b) = hue_to_rgb(hue);
    let clear_color = [r, g, b, 1.0];
    info!("   Frame #{}: hue={:.1}° color=({:.2},{:.2},{:.2})", 
          state.frame_count, hue, r, g, b);
    
    // Load cursor image if not cached
    if state.pointer_image.is_none() {
        use smithay::backend::allocator::Fourcc;
        // Use scale 2 for a larger, more visible cursor (48x48 instead of 24x24)
        let cursor_image = state.cursor.get_image(2, Duration::ZERO);
        let buffer = MemoryRenderBuffer::from_slice(
            &cursor_image.pixels_rgba,
            Fourcc::Argb8888,
            (cursor_image.width as i32, cursor_image.height as i32),
            1,
            Transform::Normal,
            None,
        );
        state.pointer_element.set_buffer(buffer.clone());
        state.pointer_image = Some(buffer);
        info!("✅ Loaded cursor image ({}x{}) at scale 2", cursor_image.width, cursor_image.height);
    }
    
    // Render cursor at current pointer location
    let scale = Scale::from(1.0);
    let cursor_pos = state.pointer_location.to_physical(scale).to_i32_round();
    let cursor_elements: Vec<MemoryRenderBufferRenderElement<_>> = state.pointer_element
        .render_elements(&mut renderer, cursor_pos, scale, 1.0);
    
    let mut elements: Vec<NuthatchRenderElements<_>> = cursor_elements
        .into_iter()
        .map(NuthatchRenderElements::from)
        .collect();
    
    info!("🖱️  Rendering cursor at ({}, {}) - {} elements", 
          cursor_pos.x, cursor_pos.y, elements.len());

    
    use smithay::backend::drm::compositor::FrameFlags;
    
    match drm_output.render_frame(&mut renderer, &elements, clear_color, FrameFlags::DEFAULT) {
        Ok(render_result) => {
            info!("✅ Frame rendered (is_empty: {})", render_result.is_empty);
            
            // Queue frame regardless of damage tracking for now
            // This ensures we get continuous VBlanks during testing
            match drm_output.queue_frame(()) {
                Ok(_) => {
                    info!("✅ Frame queued - waiting for next VBlank");
                }
                Err(e) => {
                    error!("❌ Failed to queue frame for {:?}: {}", crtc, e);
                }
            }
        }
        Err(e) => {
            error!("❌ Frame rendering error for {:?}: {}", crtc, e);
        }
    }
    
    // Put drm_output back
    let device = state.udev_data.backends.get_mut(&node).expect("Device must exist");
    let surface = device.surfaces.get_mut(&(crtc.into())).unwrap();
    surface.drm_output = Some(drm_output);
}

/// Device addition handler
fn device_added(
    state: &mut DrmCompositorState,
    node: DrmNode,
    path: &Path,
) -> Result<(), DeviceAddError> {
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    info!("📍 DEVICE_ADDED called");
    info!("   Node: {}", node);
    info!("   Path: {:?}", path);
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    // 1. Open device file descriptor using session
    info!("Step 1: Opening device FD...");
    let fd = state
        .udev_data
        .session
        .open(
            path,
            OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK,
        )
        .map_err(DeviceAddError::DeviceOpen)?;

    let fd = DrmDeviceFd::new(DeviceFd::from(fd));
    info!("✅ Step 1 complete: Opened device FD");

    // 2. Create DRM device and event notifier
    info!("Step 2: Creating DRM device...");
    let (drm, notifier) = DrmDevice::new(fd.clone(), true)
        .map_err(DeviceAddError::DrmDevice)?;
    info!("✅ Step 2 complete: Created DRM device");

    // 3. Create GBM device for buffer allocation
    info!("Step 3: Creating GBM device...");
    let gbm = GbmDevice::new(fd)
        .map_err(DeviceAddError::GbmDevice)?;
    info!("✅ Step 3 complete: Created GBM device");

    // 4. Register DRM event handler for VBlank
    info!("Step 4: Registering VBlank event handler...");
    let registration_token = state
        .udev_data
        .loop_handle
        .insert_source(
            notifier,
            move |event, _metadata, data: &mut DrmCompositorState| match event {
                DrmEvent::VBlank(crtc) => {
                    info!("🎬 VBlank event for CRTC {:?}", crtc);
                    
                    // CRITICAL: Mark previous frame as submitted to release buffer back to swapchain
                    let device = data.udev_data.backends.get_mut(&node).expect("Device must exist");
                    let surface = device.surfaces.get_mut(&(crtc.into())).expect("Surface must exist");
                    
                    if let Some(ref mut drm_output) = surface.drm_output {
                        match drm_output.frame_submitted() {
                            Ok(_) => info!("   Frame submitted, buffer released to swapchain"),
                            Err(e) => error!("   Failed to mark frame as submitted: {:?}", e),
                        }
                    }
                    
                    // Now render the next frame
                    info!("   Triggering render for next frame...");
                    render_surface(data, node, crtc);
                    info!("   Render_surface completed");
                }
                DrmEvent::Error(error) => {
                    error!("DRM error: {:?}", error);
                }
            },
        )
        .map_err(|e| anyhow::anyhow!("Failed to register DRM event source: {:?}", e))
        .map_err(DeviceAddError::EventLoop)?;
    info!("✅ Step 4 complete: Registered VBlank event handler");

    // 5. Try to initialize EGL and add to GPU manager
    info!("Step 5: Initializing EGL and GPU manager...");
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

    // 6. Create allocator and framebuffer exporter
    let allocator = GbmAllocator::new(
        gbm.clone(),
        GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT,
    );
    let framebuffer_exporter = GbmFramebufferExporter::new(gbm.clone(), render_node.into());
    info!("✅ Created allocator and framebuffer exporter");

    // 7. Create DRM output manager
    // Get supported formats from the GPU renderer
    let mut renderer = state.udev_data.gpus.single_renderer(&render_node)
        .map_err(|e| DeviceAddError::AddNode(anyhow::anyhow!("Failed to get renderer: {}", e)))?;
    let render_formats = renderer.as_mut().egl_context().dmabuf_render_formats().clone();
    
    info!("Got render formats from GPU renderer");
    
    let drm_output_manager = DrmOutputManager::new(
        drm,
        allocator,
        framebuffer_exporter,
        Some(gbm.clone()),
        SUPPORTED_FORMATS.iter().copied(),
        render_formats.into_iter().collect::<Vec<_>>(),
    );
    info!("✅ Created DRM output manager");

    // 8. Store backend data
    let backend_data = BackendData {
        drm_output_manager,
        gbm,
        render_node,
        registration_token,
        drm_scanner: DrmScanner::new(),
        surfaces: HashMap::new(),
    };

    state.udev_data.backends.insert(node, backend_data);
    info!("✅ Step 8 complete: Device {} stored in backends", node);

    // 9. Scan for connectors (will be done in device_changed)
    info!("Step 9: Scanning for connectors via device_changed()...");
    device_changed(state, node);

    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    info!("✅ DEVICE_ADDED COMPLETE for {}", node);
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
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
