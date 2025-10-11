use smithay::{
    backend::{
        renderer::{damage::OutputDamageTracker, gles::GlesRenderer, Frame, Renderer, Texture},
        winit::{self, WinitEvent},
    },
    output::{Mode, Output, PhysicalProperties, Subpixel},
    reexports::{
        calloop::EventLoop,
        wayland_server::Display,
    },
    utils::Transform,
};

use crate::state::NuthatchState;

use std::time::Duration;

pub fn init_winit() -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("Initializing Nuthatch Compositor with winit backend");

    // Create event loop
    let event_loop: EventLoop<NuthatchState> = EventLoop::try_new()?;
    
    // Create Wayland display
    let mut display: Display<NuthatchState> = Display::new()?;
    
    // Initialize compositor state
    let mut state = NuthatchState::new(&mut display, &event_loop);

    // Initialize winit backend
    let (mut backend, mut winit_evt_loop) = winit::init::<GlesRenderer>()?;
    
    let size = backend.window_size();
    tracing::info!("Window size: {:?}", size);

    // Create output
    let mode = Mode {
        size,
        refresh: 60_000,
    };

    let physical_properties = PhysicalProperties {
        size: (0, 0).into(),
        subpixel: Subpixel::Unknown,
        make: "Nuthatch".into(),
        model: "Compositor".into(),
    };

    let output = Output::new("winit".to_string(), physical_properties);
    let _global = output.create_global::<NuthatchState>(&display.handle());
    
    output.change_current_state(
        Some(mode),
        Some(Transform::Flipped180),
        None,
        Some((0, 0).into()),
    );
    output.set_preferred(mode);
    
    state.space.map_output(&output, (0, 0));

    tracing::info!("Output created and mapped");
    tracing::info!("Compositor ready - clients can connect");

    let _damage_tracker = OutputDamageTracker::from_output(&output);

    // Main event loop
    let mut frame_count = 0u64;
    loop {
        // Dispatch Wayland events
        display.dispatch_clients(&mut state)?;

        // Handle winit events
        let mut needs_redraw = false;
        winit_evt_loop.dispatch_new_events(|event| match event {
            WinitEvent::Resized { size, .. } => {
                tracing::info!("Window resized: {:?}", size);
                output.change_current_state(
                    Some(Mode { size, refresh: 60_000 }),
                    None,
                    None,
                    None,
                );
                needs_redraw = true;
            }
            WinitEvent::Input(input_event) => {
                tracing::trace!("Input event: {:?}", input_event);
            }
            WinitEvent::Focus(_) => {}
            WinitEvent::Redraw => {
                needs_redraw = true;
            }
            WinitEvent::CloseRequested => {
                tracing::info!("Closing compositor");
                std::process::exit(0);
            }
        });

        // Render frame if needed
        if needs_redraw {
            // TODO: Implement proper rendering here
            // Currently skipping rendering - window will be invisible but compositor works
        }

        // Flush clients
        display.flush_clients()?;

        // Target 60fps
        std::thread::sleep(Duration::from_millis(16));
        frame_count = frame_count.wrapping_add(1);
    }
}
