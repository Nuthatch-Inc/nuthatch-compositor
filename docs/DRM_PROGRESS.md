# DRM Backend Development Progress

## Current Status - October 11, 2025

### ✅ Environment Verified
- Running in TTY4 (proper environment for DRM testing)
- GPU access confirmed (`/dev/dri/card1` available, user in `video` group)
- Anvil compositor (Smithay reference) runs successfully with DRM backend
- EGL and OpenGL ES rendering working correctly
- Input devices detected and working

### 🎯 Goal
Implement a DRM/KMS backend for nuthatch-compositor to run natively on TTY4 without nested Wayland issues.

### 📊 What We Learned from Testing Anvil

Running Anvil with `--tty-udev` in TTY4 showed:
```
✅ Successfully selected EGL platform: PLATFORM_GBM_KHR
✅ EGL Initialized (1.5)
✅ GL Version: OpenGL ES 3.2 Mesa 25.1.9
✅ Using renderD128 as primary gpu
✅ DrmDevice initializing
✅ Connector eDP-1 initialized at 1920x1200@60Hz
✅ Input devices detected (keyboard, mouse, trackpad)
✅ XWayland spawned successfully
```

### 🚧 Implementation Challenges

Attempted to implement DRM backend but hit Smithay 0.7 API issues:

1. **Session Management API Changes**
   - `AutoSession` doesn't exist in Smithay 0.7
   - Need to use `LibSeatSession` instead
   - Session opening requires `OFlags` and `DeviceFd` wrappers

2. **DRM Device Initialization**
   - `DrmDevice::new()` requires `DrmDeviceFd` wrapping
   - Need proper device file descriptor handling
   - Resource handle queries require `Device` trait import

3. **Complex Setup Required**
   - UdevBackend for device discovery
   - LibinputInputBackend for input handling
   - GBM allocator for buffer management
   - DRM compositor with proper scanout
   - Multi-GPU support (render vs display nodes)

### 📝 Anvil's DRM Architecture

Anvil uses a sophisticated setup in `src/udev.rs`:

1. **Session Management**: `LibSeatSession` for VT switching and device access
2. **Device Discovery**: `UdevBackend` to find and monitor GPUs
3. **DRM Setup**: Per-device initialization with proper mode setting
4. **Input**: `LibinputInputBackend` for keyboard/mouse/touchpad
5. **Rendering**: Multi-GPU support with `GpuManager` and `MultiRenderer`
6. **Composition**: `DrmCompositor` for scanout and frame timing

Key files to study:
- `/home/xander/src/smithay/anvil/src/udev.rs` (1,600+ lines)
- Focus on `run_udev()` function and `UdevData` struct

### 🎯 Next Steps

#### Option A: Copy Anvil's Approach (Recommended)
Study and adapt Anvil's `udev.rs` implementation:

1. **Start Simple** - Basic DRM initialization:
   - `LibSeatSession` setup
   - `UdevBackend` for GPU discovery
   - Single display initialization
   - Clear screen to solid color

2. **Add Input** - Keyboard and mouse:
   - `LibinputInputBackend` integration
   - Event loop handling
   - Basic keyboard shortcuts (Ctrl+Alt+Backspace to quit)

3. **Integrate Wayland** - Connect to compositor state:
   - Use existing `NuthatchState`
   - Map outputs to displays
   - Render client surfaces

4. **Add Composition** - DrmCompositor:
   - Scanout optimization
   - Frame timing
   - VBlank synchronization

#### Option B: Use Smithay 0.8+ (If Available)
Check if newer Smithay versions have better documentation/APIs:
```bash
cargo search smithay
# Check latest version and changelog
```

### 🔧 Immediate Action Plan

1. **Study Anvil's udev.rs** in detail
   ```bash
   cd /home/xander/src/smithay/anvil
   less src/udev.rs
   # Focus on lines 1-300 for initialization
   ```

2. **Create minimal DRM test** (src/drm_minimal.rs):
   - Just open session and list GPUs
   - No rendering yet
   - Verify we can initialize without errors

3. **Incrementally add features**:
   - Phase 1: Device initialization
   - Phase 2: Display output (solid color)
   - Phase 3: Input handling
   - Phase 4: Window rendering

### 📚 Resources

- Anvil source: `/home/xander/src/smithay/anvil/src/udev.rs`
- Smithay docs: https://smithay.github.io/smithay/
- DRM documentation: https://www.kernel.org/doc/html/latest/gpu/drm-kms.html
- Our previous attempts: `src/drm.rs` (has basic structure, needs fixing)

### 🐛 Known Issues to Fix

Current `src/drm.rs` (commented out) has:
- Wrong session API (`AutoSession` → `LibSeatSession`)
- Missing `DeviceFd` wrapper for device opening
- Incomplete DRM surface creation
- No input handling
- No rendering loop

### ⚠️ Development Notes

**Running in TTY4:**
- Ctrl+Alt+F1 or F2 to return to KDE
- Ctrl+Alt+F4 to return to TTY
- Ctrl+C to stop compositor
- SSH access recommended for debugging

**Testing:**
- Always test basic initialization first
- Add `--no-xwayland` flag for simpler testing
- Use `RUST_LOG=debug` for detailed output
- Keep TTY3 logged in as backup

### 🎉 Success Criteria

A successful DRM backend will:
1. ✅ Initialize in TTY4 without KDE running
2. ✅ Display a colored screen (proving rendering works)
3. ✅ Accept keyboard input (Ctrl+C to quit)
4. ✅ Create Wayland socket for clients
5. ✅ Render simple client windows (like `weston-terminal`)
6. ✅ Handle VT switching gracefully

### 📅 Timeline Estimate

- **Day 1-2**: Study Anvil, create minimal test
- **Day 3-4**: Basic display initialization
- **Day 5-6**: Add input handling
- **Day 7-8**: Wayland client rendering
- **Day 9-10**: Polish and multi-monitor support

## Conclusion

The environment is ready and working (proven by Anvil). We need to properly implement the DRM backend by following Anvil's architecture. The API complexity is manageable if we take it step by step.

**Current blocker**: Understanding Smithay 0.7's DRM/session APIs correctly.
**Next action**: Deep dive into Anvil's udev.rs to understand the proper initialization sequence.
