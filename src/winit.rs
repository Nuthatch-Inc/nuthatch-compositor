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

    let mut damage_tracker = OutputDamageTracker::from_output(&output);

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

        // Always render to make window visible
        needs_redraw = true;

        // Render frame if needed
        if needs_redraw {
            // Get size before binding to avoid borrow checker issues
            let size = backend.window_size();
            
            // Log rendering attempt on first few frames
            if frame_count < 5 {
                tracing::info!("Rendering frame {} at size {:?}", frame_count, size);
            }
            
            // Bind the backend to get renderer and target
            let mut render_success = false;
            {
                match backend.bind() {
                    Ok((renderer, mut target)) => {
                        // Render a frame with a dark blue background
                        match renderer.render(
                            &mut target,
                            size.to_logical(1).to_physical(1),
                            Transform::Normal,
                        ) {
                            Ok(mut frame) => {
                                // Clear the screen with a nice dark blue color
                                if let Err(e) = frame.clear([0.1, 0.1, 0.3, 1.0].into(), &[]) {
                                    tracing::warn!("Failed to clear frame: {}", e);
                                } else if frame_count < 5 {
                                    tracing::info!("Frame {} cleared successfully", frame_count);
                                }
                                
                                // Finish the frame - this commits the rendering
                                match frame.finish() {
                                    Ok(_sync_point) => {
                                        if frame_count < 5 {
                                            tracing::info!("Frame {} finished successfully", frame_count);
                                        }
                                        render_success = true;
                                    }
                                    Err(e) => {
                                        tracing::warn!("Failed to finish frame: {}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!("Failed to start rendering: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to bind backend: {}", e);
                    }
                }
            } // Drop all borrows here
            
            // Now submit to actually present to screen with full window damage
            if render_success {
                // Create damage rect for the entire window
                use smithay::utils::{Rectangle, Physical};
                let damage = Rectangle::from_loc_and_size((0, 0), size);
                
                if let Err(e) = backend.submit(Some(&[damage])) {
                    tracing::warn!("Failed to submit frame: {}", e);
                } else if frame_count < 5 {
                    tracing::info!("Frame {} submitted successfully with damage", frame_count);
                }
            }
        }

        // Flush clients
        display.flush_clients()?;

        // Target 60fps
        std::thread::sleep(Duration::from_millis(16));
        frame_count = frame_count.wrapping_add(1);
    }
}
