use smithay::{
    backend::{
        renderer::{damage::OutputDamageTracker, gles::GlesRenderer},
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
    tracing::info!("🐦 Initializing Nuthatch Compositor with winit backend");

    // Create event loop
    let mut event_loop: EventLoop<NuthatchState> = EventLoop::try_new()?;
    
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
        refresh: 60_000, // 60 Hz
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

    tracing::info!("✓ Output created and mapped");
    tracing::info!("✓ Compositor ready! Clients can connect to: {:?}", 
        std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "wayland-0".to_string()));

    // Damage tracker for efficient rendering
    let mut damage_tracker = OutputDamageTracker::from_output(&output);

    // Main event loop
    loop {
        // Dispatch Wayland events
        let mut calloop_data = state;
        display.dispatch_clients(&mut calloop_data)?;
        state = calloop_data;

        // Handle winit events
        winit_evt_loop.dispatch_new_events(|event| match event {
            WinitEvent::Resized { size, .. } => {
                tracing::info!("Window resized: {:?}", size);
                // Handle resize
            }
            WinitEvent::Input(input_event) => {
                tracing::trace!("Input event: {:?}", input_event);
                // Handle input
            }
            WinitEvent::Focus(_) => {}
            WinitEvent::Redraw => {
                // Time to render
                tracing::trace!("Redraw requested");
            }
            WinitEvent::CloseRequested => {
                tracing::info!("Close requested, shutting down");
                std::process::exit(0);
            }
        });

        // Render
        backend.bind()?;
        
        // Just submit the frame for now
        // We'll add proper window rendering later
        
        backend.submit(None)?;

        // Flush clients
        display.flush_clients()?;

        // Small sleep to prevent busy-waiting
        std::thread::sleep(Duration::from_millis(16)); // ~60fps
    }
}
