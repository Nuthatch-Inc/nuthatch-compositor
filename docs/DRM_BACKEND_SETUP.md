# Option A: DRM/KMS Backend - Running on Real Hardware

## Overview

Instead of running nested inside KDE Plasma, run the compositor **directly on the GPU** using Direct Rendering Manager (DRM) and Kernel Mode Setting (KMS). This is how production Wayland compositors actually work.

## What is DRM/KMS?

**DRM (Direct Rendering Manager)**
- Linux kernel subsystem for interfacing with GPUs
- Provides direct access to graphics hardware
- No intermediate compositor/window manager
- Full control of display output

**KMS (Kernel Mode Setting)**
- Part of DRM that handles display configuration
- Sets video modes, resolutions, refresh rates
- Manages multiple displays
- Hardware planes and cursor composition

**Together:** Your compositor talks directly to the kernel, which talks directly to the GPU.

## How It Works

### Current Nested Setup (winit/Wayland):
```
Your Compositor (Smithay)
    ↓ winit backend
Wayland client window
    ↓ Wayland protocol
KDE Plasma Compositor
    ↓ DRM/KMS
Linux Kernel
    ↓
GPU Hardware
```

### DRM/KMS Direct Setup:
```
Your Compositor (Smithay)
    ↓ DRM backend
Linux Kernel
    ↓
GPU Hardware
```

**Much simpler!** No nested window, no intermediate compositor.

## Requirements

### 1. Hardware Access
You need to run with permissions to access `/dev/dri/card0`:

```bash
# Check your GPU devices
ls -la /dev/dri/
# Should see something like:
# card0, card1 (GPUs)
# renderD128, renderD129 (render nodes)
```

### 2. Run from TTY (Not in Plasma Session)
You **cannot** run a DRM compositor while another compositor (KDE) is already using the GPU.

**Options:**
- **A) Switch to a TTY:** `Ctrl+Alt+F3` (or F4, F5, etc.)
- **B) Stop KDE:** `sudo systemctl stop sddm` (login manager)
- **C) Reboot to text mode:** Boot parameter `systemd.unit=multi-user.target`

### 3. User Permissions
Add your user to the necessary groups:

```bash
# Video group for GPU access
sudo usermod -a -G video $USER

# Input group for keyboard/mouse
sudo usermod -a -G input $USER

# Seat group (some distros)
sudo usermod -a -G seat $USER

# Log out and back in for groups to take effect
```

## Implementation Steps

### Step 1: Add DRM Backend Dependencies

Update `Cargo.toml`:

```toml
[dependencies]
smithay = { version = "0.7.0", features = ["backend_drm", "backend_libinput"] }
smithay-drm-extras = "0.7.0"
gbm = "0.17"  # GPU Buffer Manager
drm = "0.18"  # DRM bindings
libc = "0.2"  # For ioctl calls
```

Key additions:
- `backend_drm`: DRM/KMS support
- `backend_libinput`: Direct input device access (keyboard, mouse)
- `gbm`: Allocate GPU buffers for rendering
- `drm`: Low-level DRM control

### Step 2: Create DRM Backend Module

Create `src/drm.rs`:

```rust
use smithay::{
    backend::{
        drm::{DrmDevice, DrmSurface, DrmNode},
        renderer::{gles::GlesRenderer, Bind, Frame, Renderer},
        egl::{EGLContext, EGLDisplay},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        session::{auto::AutoSession, Session},
    },
    reexports::calloop::EventLoop,
    wayland::server::Display,
};

use std::path::PathBuf;

pub fn init_drm() -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("Initializing DRM backend");
    
    // 1. Start session (handles VT switching, device permissions)
    let (session, notifier) = AutoSession::new()?;
    
    // 2. Find primary GPU
    let gpu_path = PathBuf::from("/dev/dri/card0");
    let drm_node = DrmNode::from_path(&gpu_path)?;
    
    // 3. Open DRM device
    let drm = session.open(&gpu_path)?;
    let drm_device = DrmDevice::new(drm, true)?;
    
    // 4. Initialize EGL for rendering
    let egl_display = EGLDisplay::new(drm_device.clone())?;
    let egl_context = EGLContext::new(&egl_display)?;
    
    // 5. Create renderer
    let renderer = unsafe { GlesRenderer::new(egl_context)? };
    
    // 6. Get available displays (monitors)
    let connectors = drm_device.connectors()?;
    
    // 7. Set up display mode for first connected monitor
    for connector in connectors {
        if connector.state() == ConnectorState::Connected {
            let modes = connector.modes();
            let preferred_mode = modes[0]; // Usually highest resolution
            
            // Create surface for this display
            let surface = drm_device.create_surface(
                connector.handle(),
                preferred_mode,
            )?;
            
            tracing::info!(
                "Display: {}x{} @ {}Hz",
                mode.size().0,
                mode.size().1,
                mode.vrefresh()
            );
        }
    }
    
    // 8. Set up input (keyboard, mouse, touchpad)
    let input_backend = LibinputInputBackend::new(session)?;
    
    // 9. Main render loop
    loop {
        // Render frame
        // Handle input
        // Swap buffers
    }
    
    Ok(())
}
```

### Step 3: Input Handling

With DRM, you also need to handle input devices directly:

```rust
use smithay::backend::libinput::{LibinputInputBackend, LibinputEvent};

// LibinputEvent types:
// - KeyboardKey
// - PointerMotion
// - PointerButton  
// - PointerAxis (scroll)
// - TouchDown/TouchUp/TouchMotion

input_backend.process_events(|event| {
    match event {
        LibinputEvent::Keyboard { event, .. } => {
            // Handle key press/release
        }
        LibinputEvent::PointerMotion { event, .. } => {
            // Handle mouse movement
        }
        // etc.
    }
});
```

### Step 4: Rendering Loop

```rust
loop {
    // 1. Render to GPU buffer
    renderer.bind(surface)?;
    
    let mut frame = renderer.render(size, Transform::Normal)?;
    frame.clear([0.1, 0.1, 0.3, 1.0].into(), &[])?;
    
    // TODO: Render window surfaces here
    
    frame.finish()?;
    
    // 2. Present to screen (page flip)
    surface.page_flip()?;
    
    // 3. Handle input events
    input_backend.dispatch_new_events(handle_input)?;
    
    // 4. Process Wayland clients
    display.dispatch_clients(&mut state)?;
    display.flush_clients()?;
}
```

## Testing Procedure

### Safest Method: Run from TTY

1. **Save your work** in GUI applications

2. **Switch to TTY3:**
   ```
   Ctrl + Alt + F3
   ```

3. **Login** with your username/password

4. **Navigate and run:**
   ```bash
   cd ~/src/nuthatch-compositor
   RUST_LOG=info cargo run --release
   ```

5. **Switch back to KDE:**
   ```
   Ctrl + Alt + F1  (or F2)
   ```

### What You'll See

If successful:
- **Display goes blank** briefly
- **Your dark blue window** fills the entire screen
- **No window decorations** (you're the compositor now!)
- **Keyboard/mouse** may or may not work (depends on input implementation)

If it fails:
- Error messages in TTY
- Display stays on TTY
- Can still switch back to KDE

### Recovery

**If compositor crashes:**
- You're returned to TTY automatically
- Just switch back: `Ctrl+Alt+F1`

**If display is blank:**
- `Ctrl+C` to kill compositor
- Switch TTY: `Ctrl+Alt+F3`
- Or reboot: `Ctrl+Alt+Del`

## Advantages

✅ **Real compositor behavior** - Exactly how it will work in production
✅ **Full GPU control** - No nesting issues
✅ **Better performance** - Direct hardware access
✅ **Accurate testing** - Test the actual deployment scenario
✅ **Multiple displays** - Can manage all monitors
✅ **Hardware planes** - Access to GPU composition features

## Disadvantages

⚠️ **Takes over display** - Can't use GUI while testing
⚠️ **More complex** - More moving parts (session, libinput, DRM)
⚠️ **Harder to debug** - Can't use GUI debuggers
⚠️ **Permissions** - Need hardware access
⚠️ **Riskier** - Could lock up display (rare but possible)

## Safety Tips

### 1. SSH Backup Access
```bash
# On another machine
ssh your-computer
cd ~/src/nuthatch-compositor
pkill -f nuthatch  # Kill compositor remotely if needed
```

### 2. Auto-restart KDE
Create a recovery script:
```bash
#!/bin/bash
# ~/recover-kde.sh
sudo systemctl start sddm
```

### 3. Keep TTY Access
- Don't run in `sudo` mode unnecessarily
- Keep another TTY logged in: `Ctrl+Alt+F4`
- Know the reboot combo: `Ctrl+Alt+Del`

### 4. Start Simple
First test: Just initialize DRM and exit
```rust
pub fn init_drm() -> Result<(), Box<dyn std::error::Error>> {
    let session = AutoSession::new()?;
    let gpu = std::path::PathBuf::from("/dev/dri/card0");
    let drm = session.open(&gpu)?;
    
    tracing::info!("DRM opened successfully!");
    
    std::thread::sleep(std::time::Duration::from_secs(3));
    
    Ok(())
}
```

## Development Workflow

**Recommended hybrid approach:**

1. **Develop with winit** (even though rendering doesn't show)
   - Fast iteration
   - Can use debuggers
   - Test Wayland protocol handling
   - Verify no crashes

2. **Test with DRM** periodically
   - Verify rendering works
   - Test performance
   - Check multi-display
   - Validate input handling

3. **Final polish with DRM**
   - Polish the actual user experience
   - Tune performance
   - Handle edge cases

## Alternative: Virtual TTY (More Complex)

Run in a virtual framebuffer for testing:

```bash
# Install
sudo dnf install Xvfb xorg-x11-server-utils

# Create virtual display
Xvfb :99 -screen 0 1920x1080x24 &
export DISPLAY=:99

# Run compositor (if it supports X11)
cargo run
```

But this still doesn't give you real DRM testing.

## Next Steps

If you want to try Option A:

1. **Backup current work** ✓ (already committed)
2. **Add DRM dependencies** to Cargo.toml
3. **Create basic DRM module** (minimal test)
4. **Test from TTY** with simple "show blue screen" 
5. **Incrementally add features**:
   - Input handling
   - Window rendering
   - Multiple displays

Would you like me to start implementing the DRM backend? I can begin with a minimal test that just:
- Initializes DRM
- Shows a blue screen
- Accepts Ctrl+C to exit

This would prove the hardware access works before we build out the full implementation.
