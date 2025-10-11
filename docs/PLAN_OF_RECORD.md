# Plan of Record - Nuthatch Compositor Development

**Date Established**: October 11, 2025  
**Status**: Active

## Strategic Decision

**Adopt Anvil's DRM backend strategically while maintaining focus on Nuthatch's unique features.**

### Core Principle

Nuthatch's value is NOT in reimplementing DRM/KMS plumbing. Our value is in:

- Custom window behaviors (pseudo-maximize, snapping)
- Hardware-accelerated blur effects
- React-based window decorations
- Seamless React desktop shell integration

### What We Copy from Anvil

✅ **DRM initialization and session management** - This is plumbing
✅ **Buffer allocation and presentation** - This is plumbing  
✅ **Input device handling** - This is plumbing
✅ **Multi-GPU support** - This is plumbing

### What We Build Custom

🎨 **Window management logic** - Unique to Nuthatch
🎨 **Visual effects pipeline** - Blur, animations, transitions
🎨 **React window chrome** - Custom decorations
🎨 **IPC with desktop shell** - State synchronization
🎨 **Custom behaviors** - Pseudo-maximize, intelligent snapping

## Three-Phase Execution Plan

### Phase 1: Foundation (1 Week) - **CURRENT**

**Goal**: Get DRM rendering working by adapting Anvil's proven code

**Tasks**:

1. ✅ Review Anvil's architecture and confirm compatibility
2. [ ] Copy Anvil's DRM initialization into `drm.rs`
3. [ ] Adapt session management (LibSeatSession)
4. [ ] Implement UdevBackend for GPU discovery
5. [ ] Set up DrmCompositor for scanout
6. [ ] Test: Display solid color screen in TTY4
7. [ ] Test: Render cursor and handle mouse input
8. [ ] Test: Handle VT switching (Ctrl+Alt+Fn)

**Success Criteria**:

- Compositor initializes in TTY without errors
- Display shows colored output (proves rendering works)
- Mouse cursor visible and responsive
- Can exit cleanly with Ctrl+C
- Can switch back to KDE without issues

### Phase 2: Window Support (1 Week)

**Goal**: Render actual Wayland client windows

**Tasks**:

1. [ ] Port Anvil's rendering loop
2. [ ] Implement basic window surface rendering
3. [ ] Add window damage tracking
4. [ ] Test with simple client (weston-terminal, foot)
5. [ ] Implement OUR custom window positioning logic
6. [ ] Add keyboard input handling
7. [ ] Create basic window focus system

**Success Criteria**:

- Can launch and display wayland terminal
- Windows render with proper contents
- Keyboard input works in client apps
- Basic window management (open/close/focus)

### Phase 3: Nuthatch Features (Ongoing)

**Goal**: Implement unique Nuthatch functionality

**Tasks**:

1. [ ] Pseudo-maximize window behavior
2. [ ] Side-by-side snapping with visual feedback
3. [ ] Blur effect shader pipeline
4. [ ] Window animations (open/close/move)
5. [ ] React-based window decorations (chrome)
6. [ ] IPC bridge to React desktop shell
7. [ ] Multi-monitor support
8. [ ] Performance optimization

**Success Criteria**:

- Windows behave according to Nuthatch UX spec
- Blur effects render smoothly at 60fps
- React chrome integrates seamlessly
- Desktop shell can control compositor

## Technical Strategy

### Code Organization

```
src/
├── main.rs           # Entry point, backend selection
├── state.rs          # Compositor state (keep our structure)
├── winit.rs          # Nested mode for development
├── drm.rs            # ADAPTED FROM ANVIL - DRM/KMS backend
├── window.rs         # CUSTOM - Our window management logic
├── effects.rs        # CUSTOM - Blur and visual effects
├── chrome.rs         # CUSTOM - React window decorations
└── ipc.rs            # CUSTOM - Communication with desktop shell
```

### Development Workflow

1. **Code in VS Code** with Copilot and full tooling
2. **Test basics** in nested mode (winit) when possible
3. **Test DRM** in TTY4 for hardware-specific features
4. **Iterate quickly** on custom features in nested mode
5. **Validate on hardware** before committing

### Testing Strategy

- **Unit tests**: For window logic and state management
- **Nested mode**: Rapid iteration on features
- **TTY mode**: Hardware rendering validation
- **Integration tests**: With React desktop shell
- **Performance profiling**: GPU utilization, frame timing

## Risk Mitigation

### Known Risks

1. **Anvil code complexity** - Mitigation: Copy incrementally, test each piece
2. **API incompatibilities** - Mitigation: Already using Smithay 0.7, same as Anvil
3. **Merge conflicts** - Mitigation: Keep Nuthatch-specific code in separate modules
4. **Performance issues** - Mitigation: Profile early, optimize rendering pipeline

### Fallback Plans

- If DRM too complex: Focus on nested mode first, DRM later
- If blur too slow: Implement simpler transparency first
- If React chrome complex: Start with native chrome, add React later

## Success Metrics

### Week 1 (Oct 11-18)

- [ ] Compositor displays output in TTY4
- [ ] Cursor and input working
- [ ] Clean startup and shutdown

### Week 2 (Oct 18-25)

- [ ] Simple Wayland clients render
- [ ] Basic window management works
- [ ] Can use terminal in compositor

### Month 1 (Oct 11 - Nov 11)

- [ ] Custom window behaviors implemented
- [ ] Basic blur effects working
- [ ] Multiple windows managed properly

### Month 2 (Nov 11 - Dec 11)

- [ ] React chrome integrated
- [ ] Desktop shell communication
- [ ] Performance optimized

## License & Attribution

Anvil is MIT licensed. We will:

1. Include MIT license text for copied code
2. Add attribution comments in `drm.rs`
3. Maintain separate authorship for custom modules
4. Contribute improvements back to Smithay if applicable

## References

- Anvil source: `/home/xander/src/smithay/anvil/`
- Key file: `anvil/src/udev.rs` (DRM backend)
- Smithay docs: https://smithay.github.io/smithay/
- Our PRD: `nuthatch-desktop/docs/NUTHATCH_LINUX_PRD.md`

---

**Next Action**: Begin Phase 1, Task 2 - Copy Anvil's DRM initialization
**Current Focus**: Getting colored output in TTY4 this weekend
**Mindset**: Pragmatic on plumbing, creative on features
