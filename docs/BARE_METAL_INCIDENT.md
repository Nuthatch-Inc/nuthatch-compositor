# Bare Metal Testing Incident - Black Screen Hang

**Date:** 2024-10-11  
**Severity:** HIGH - Required hard reset  
**Status:** Testing moved to VM

## What Happened

Tested full DRM compositor on bare metal (TTY4) with `--drm-full` flag:
- Screen went completely black
- System became unresponsive
- Could not switch VTs (Ctrl+Alt+F1-F7 did not work)
- Required **hard reset** using power button

## Likely Causes

### 1. VT Switching Lock (Most Likely)
- Compositor took exclusive control of display
- Did not properly register VT switch handlers
- System couldn't switch back to text console

### 2. GPU Driver Issue
- DRM initialization may have put GPU in bad state
- Possible kernel driver hang
- Buffer presentation might have deadlocked GPU

### 3. Event Loop Deadlock
- Main event loop may have blocked waiting for event
- VBlank events might not be firing
- Input processing could be stuck

## Why This Happened

Looking at the code:
- ✅ Session management initialized (LibSeat)
- ✅ VT switch handlers registered
- ❌ **BUT**: Session pause/resume handlers are empty (just log, don't actually pause DRM)
- ❌ No timeout or safety exit mechanism
- ❌ Very verbose logging could overwhelm system

## Safety Measures Implemented

### 1. Auto-Exit Timeout
```rust
// SAFETY: Auto-exit after 10 seconds to prevent hangs during testing
if start_time.elapsed() > Duration::from_secs(10) {
    info!("⏱️  10 second timeout reached - exiting for safety");
    break;
}
```

### 2. Testing Strategy
- **NO MORE BARE METAL TESTING** until we have:
  - Proper VT switch handling
  - DRM pause/resume on session events
  - Known-good rendering path
- Use VM for all testing going forward
- Anvil test mode first before full compositor

## VM Testing Plan

### Setup
1. Use existing VM setup (already documented in VM_SETUP.md)
2. Passthrough GPU if possible for real DRM testing
3. Or use virtual GPU for initial debugging

### Test Progression
1. **Step 1**: Run with 10-second timeout, capture logs
2. **Step 2**: Verify VT switching works (can exit back to shell)
3. **Step 3**: Confirm rendering actually happens (blue screen)
4. **Step 4**: Test session pause/resume
5. **Step 5**: Only then consider bare metal again

## Code Locations That Need Fixing

### Session Event Handlers (src/drm_new.rs ~line 340)
```rust
SessionEvent::PauseSession => {
    info!("Session paused - VT switched away");
    libinput_context.suspend();
    // TODO: Pause all DRM outputs ← THIS IS CRITICAL!
}
SessionEvent::ActivateSession => {
    info!("Session activated - VT switched back");
    if let Err(err) = libinput_context.resume() {
        error!("Failed to resume libinput: {:?}", err);
    }
    // TODO: Resume all DRM outputs ← THIS TOO!
}
```

**We need to:**
- Pause all DRM compositors when session pauses
- Resume them when session resumes
- This allows VT switching to work properly

### Missing DRM Cleanup
- No cleanup code when compositor exits
- Should restore previous VT mode
- Should release DRM resources gracefully

## Recovery Procedure (If It Happens Again)

1. **First**: Try Ctrl+Alt+F1-F7 to switch VTs
2. **Second**: Try Alt+SysRq+K to kill all on current VT
3. **Third**: SSH from another machine and `killall nuthatch-compositor`
4. **Last Resort**: Hard reset (what we had to do)

## Next Steps

✅ Add 10-second safety timeout (DONE)  
⏳ Move all testing to VM  
⏳ Implement proper session pause/resume  
⏳ Add cleanup code for graceful exit  
⏳ Test VT switching in VM before bare metal  

## Lessons Learned

1. **Never test bare metal without:**
   - Working VT switch handlers
   - Safety timeout
   - Known-good recovery path

2. **VM testing is essential for:**
   - Low-level display code
   - Anything that takes exclusive display control
   - Testing VT switching behavior

3. **Add safety measures early:**
   - Timeouts
   - Emergency exit keys
   - Graceful cleanup paths
