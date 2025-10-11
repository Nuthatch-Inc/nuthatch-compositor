# Testing the DRM Minimal Module

## Quick Start

### 1. Build the Compositor

```bash
cd ~/src/nuthatch-compositor
cargo build --release
```

### 2. Switch to TTY4

Press: **Ctrl+Alt+F4**

### 3. Login (if needed)

```bash
# Username: xander
# Password: [your password]
```

### 4. Run the Minimal Test

```bash
cd ~/src/nuthatch-compositor
sudo RUST_LOG=info ./target/release/nuthatch-compositor --drm
```

### 5. Expected Output

```
🐦 Nuthatch Compositor starting...
Phase 1: Foundation - Window management basics
🖥️  Using DRM/KMS backend (native TTY mode)
🧪 Starting minimal DRM test
Step 1: Initializing LibSeat session...
✅ Session initialized for seat: seat0
Step 2: Finding primary GPU...
   Primary GPU path: "/dev/dri/..."
✅ Using GPU: renderD128
Step 3: Initializing UdevBackend...
✅ UdevBackend initialized
Step 4: Enumerating DRM devices...
   Device 1: card1 -> ...
      Render node: renderD128
✅ Found 1 DRM device(s)
Step 5: Creating event loop...
✅ Event loop created

🎉 All checks passed! Environment is ready for full DRM implementation.

Summary:
  • Session: seat0
  • Primary GPU: renderD128
  • DRM Devices: 1

Next step: Implement full DRM backend with rendering
✅ DRM test passed. Full backend not yet implemented.
Exiting for now. Next: implement full DRM rendering.
```

### 6. Return to KDE

Press: **Ctrl+Alt+F3** (or F2, depending on your system)

## Troubleshooting

### "Could not create LibSeat session"

- Make sure you're running in a TTY (not in KDE terminal)
- Try: `sudo` before the command
- Check: `groups` includes 'video' and 'input'

### "No GPU found"

- Check GPU access: `ls -la /dev/dri/`
- Verify permissions: `sudo usermod -a -G video,input $USER`
- Reboot if you just added groups

### "No DRM devices available"

- This is unusual - your GPU should appear
- Check: `lspci | grep VGA`
- Check: `dmesg | grep drm`

### Can't Return to KDE

- Try: **Ctrl+Alt+F1**, **F2**, or **F3**
- If stuck: SSH from another machine
- Last resort: `sudo reboot` from TTY

## What This Tests

✅ **Session Management**: Can we request device access from the system?  
✅ **GPU Discovery**: Can we find and identify graphics hardware?  
✅ **DRM Enumeration**: Can we list available display devices?  
✅ **Event Loop**: Can we create the async infrastructure?

## What's Next

After this test passes, we'll implement:

1. Full device initialization (open DRM, create GBM, set up EGL)
2. Display configuration (find connectors, set modes)
3. Rendering pipeline (allocate buffers, present frames)
4. Cursor and input handling

## Safety Notes

- This test is **read-only** - it doesn't change display state
- Safe to run anytime
- Won't lock up your TTY
- Clean exit with status message

---

**Ready to test?** Switch to TTY4 and run the command above! 🚀
