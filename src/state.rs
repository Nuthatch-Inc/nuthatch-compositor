use smithay::{
    delegate_compositor, delegate_data_device, delegate_output, delegate_seat, delegate_shm,
    delegate_xdg_shell,
    desktop::{Space, Window},
    input::{SeatHandler, SeatState},
    reexports::{
        calloop::EventLoop,
        wayland_server::{
            backend::{ClientData, ClientId, DisconnectReason},
            protocol::{wl_seat::WlSeat, wl_surface::WlSurface},
            Display,
        },
    },
    utils::{Clock, Monotonic},
    wayland::{
        buffer::BufferHandler,
        compositor::{CompositorClientState, CompositorHandler, CompositorState},
        output::{OutputHandler, OutputManagerState},
        selection::{
            data_device::{
                ClientDndGrabHandler, DataDeviceHandler, ServerDndGrabHandler,
            },
            SelectionHandler,
        },
        shell::xdg::{XdgShellHandler, XdgShellState, PopupSurface, PositionerState, ToplevelSurface},
        shm::{ShmHandler, ShmState},
    },
};

pub struct NuthatchState {
    pub start_time: std::time::Instant,
    pub space: Space<Window>,
    pub clock: Clock<Monotonic>,
    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub shm_state: ShmState,
    pub output_manager_state: OutputManagerState,
    pub seat_state: SeatState<Self>,
    pub data_device_state: smithay::wayland::selection::data_device::DataDeviceState,
}

impl NuthatchState {
    pub fn new(
        display: &mut Display<Self>,
        event_loop: &EventLoop<'static, Self>,
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
        let data_device_state = smithay::wayland::selection::data_device::DataDeviceState::new::<Self>(&dh);

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
        }
    }
}

// Compositor handler
impl CompositorHandler for NuthatchState {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a smithay::reexports::wayland_server::Client) -> &'a CompositorClientState {
        &client.get_data::<ClientState>().unwrap().compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        tracing::trace!("Surface committed: {:?}", surface);
        // Handle surface commits - update window state if needed
        // Space doesn't have a commit method in newer Smithay
    }
}

// XDG Shell handler
impl XdgShellHandler for NuthatchState {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        tracing::info!("🪟 New window created");
        let window = Window::new_wayland_window(surface.into());
        self.space.map_element(window, (0, 0), false);
    }

    fn new_popup(&mut self, _surface: PopupSurface, _positioner: PositionerState) {
        tracing::trace!("New popup created");
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: WlSeat, _serial: smithay::utils::Serial) {
        // Handle popup grabs
    }

    fn reposition_request(
        &mut self,
        _surface: PopupSurface,
        _positioner: PositionerState,
        _token: u32,
    ) {
        // Handle reposition requests
    }
}

// SHM handler
impl ShmHandler for NuthatchState {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

// Seat handler
impl SeatHandler for NuthatchState {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.seat_state
    }

    fn focus_changed(&mut self, seat: &smithay::input::Seat<Self>, focused: Option<&WlSurface>) {
        tracing::trace!("Focus changed: {:?}", focused);
    }

    fn cursor_image(&mut self, _seat: &smithay::input::Seat<Self>, _image: smithay::input::pointer::CursorImageStatus) {
        // Handle cursor changes
    }
}

// Data device handler
impl DataDeviceHandler for NuthatchState {
    fn data_device_state(&self) -> &smithay::wayland::selection::data_device::DataDeviceState {
        &self.data_device_state
    }
}

// Selection handlers
impl SelectionHandler for NuthatchState {
    type SelectionUserData = ();
}

impl ClientDndGrabHandler for NuthatchState {}
impl ServerDndGrabHandler for NuthatchState {}

// Buffer handler
impl BufferHandler for NuthatchState {
    fn buffer_destroyed(&mut self, _buffer: &smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer) {}
}

// Output handler
impl OutputHandler for NuthatchState {}

// Client state
pub struct ClientState {
    pub compositor_state: CompositorClientState,
}

impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}

// Delegate macros
delegate_compositor!(NuthatchState);
delegate_xdg_shell!(NuthatchState);
delegate_shm!(NuthatchState);
delegate_output!(NuthatchState);
delegate_seat!(NuthatchState);
delegate_data_device!(NuthatchState);
