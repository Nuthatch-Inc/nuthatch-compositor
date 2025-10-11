# DRM Backend Issues - Black Screen Problem

## Issue Summary

The DRM backend implementation causes a **black screen with unresponsive TTY switching** when run in native TTY mode. The machine remains operational but display output is lost.

## Root Causes

### 1. No Actual Rendering Pipeline
The code initializes the DRM device and creates a renderer but **never actually presents frames to the screen**:
- No `DrmCompositor` is created
- No framebuffers are allocated or presented
- VBlank events are received but ignored (only logged)
- The rendering loop never calls `render()` or presents buffers

### 2. Display Mode Not Set
The code queries available modes and connectors but **never calls `set_crtc()`** to actually configure the display output. This leaves the display in an unknown state.

### 3. Missing Surface/Framebuffer Management
- No DRM surfaces created for outputs
- No GBM surfaces allocated
- No framebuffer objects created or attached to CRTCs
- No scanout buffers configured

### 4. Broken TTY/Session Handling
The session notifier is inserted into the event loop but with an **empty handler**:
```rust
loop_handle.insert_source(notifier, |_, _, _| {})?;
```

This means:
- TTY switch signals are ignored
- Session pause/resume not handled
- VT switching (Ctrl+Alt+Fn) becomes unresponsive

## What Works

- ✅ LibSeat session initialization
- ✅ GPU device enumeration via udev
- ✅ DRM device opening and resource query
- ✅ Connector detection and mode selection
- ✅ GBM device creation
- ✅ EGL context and renderer initialization
- ✅ Wayland output object creation

## What's Missing

- ❌ Actual display mode setting (`drm.set_crtc()`)
- ❌ DRM surface creation
- ❌ Framebuffer allocation and management
- ❌ Rendering loop with frame presentation
- ❌ VBlank handling with actual redraws
- ❌ Session pause/resume handling
- ❌ TTY switch support

## Recommended Fix Approach

1. **Use Smithay's higher-level DRM compositor abstractions**
   - `DrmCompositor` handles most of the complexity
   - Proper surface and framebuffer management
   - Automatic mode setting

2. **Implement proper session handling**
   - Handle pause/resume events
   - Save/restore TTY state
   - Enable VT switching

3. **Create a real rendering loop**
   - Present frames on VBlank
   - Handle damage tracking
   - Implement buffer swapping

4. **Reference Anvil implementation**
   - Smithay's reference compositor has working DRM backend
   - Study `anvil/src/udev.rs` for proper patterns

## Testing in VM

**CRITICAL**: Test DRM backend changes **only in a VM** to avoid:
- Host system lockups
- Loss of display access
- Forced reboots
- Data loss from hard resets

VM setup allows safe TTY testing with:
- Easy recovery via host console
- Snapshots before testing
- Serial console access
- Force reset without hardware impact

## Current State

The code is checked in as work-in-progress. **DO NOT RUN** with DRM backend enabled on bare metal until rendering pipeline is implemented.

Safe testing:
```bash
# Nested mode (safe) - default
cargo run

# DRM mode (UNSAFE - VM only!)
cargo run -- --drm
```

## Next Steps

1. Set up VM for safe DRM testing
2. Study Anvil's DRM backend implementation
3. Implement `DrmCompositor` usage
4. Add proper session handling
5. Test TTY switching in VM
6. Implement rendering loop with frame presentation
