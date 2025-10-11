use slog::{Drain, Logger, info, o};
use smithay::reexports::wayland_server::Display;

fn main() {
    // Setup logging
    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    let log = Logger::root(drain, o!());

    info!(log, "🐦 Nuthatch Compositor starting...");
    info!(log, "Phase 1: Foundation - Basic compositor setup");

    // Create Wayland display
    let mut display = Display::new().expect("Failed to create Wayland display");
    let _dh = display.handle();
    
    info!(log, "✓ Wayland display created");
    info!(log, "Socket: {:?}", std::env::var("WAYLAND_DISPLAY"));
    
    // TODO: Initialize compositor state
    // TODO: Setup input handling
    // TODO: Setup rendering
    // TODO: Event loop
    
    info!(log, "Compositor initialization complete");
    info!(log, "Press Ctrl+C to exit");
    
    // Basic event loop (will be replaced with proper implementation)
    loop {
        display.dispatch_clients(&mut ()).expect("Failed to dispatch clients");
        display.flush_clients().expect("Failed to flush clients");
        std::thread::sleep(std::time::Duration::from_millis(16)); // ~60fps
    }
}
