# Hybrid Development Workflow

## Overview

Develop compositor features in VS Code GUI, then switch to TTY for testing on real hardware.

**Best of both worlds:**

- ✅ Full VS Code features (Copilot, debugging, extensions)
- ✅ Real hardware testing with DRM/KMS
- ✅ Fast iteration cycle
- ✅ Safe recovery if compositor crashes

> **Note:** On this system, KDE runs on TTY3 (Ctrl+Alt+F3). Use TTY2 (Ctrl+Alt+F2) for compositor testing.

## Quick Reference

### Development Cycle

```
1. Edit in VS Code → Save (Ctrl+S)
2. Switch to TTY2 → Ctrl+Alt+F2
3. Run compositor → ↑ (previous command)
4. Test features
5. Ctrl+C to stop
6. Back to VS Code → Ctrl+Alt+F3 (KDE runs on F3)
7. Make changes and repeat
```

### Key Commands

**From GUI (VS Code terminal):**

```bash
cargo check                              # Quick syntax check
cargo build --release                    # Build for testing
```

**From TTY:**

```bash
cd ~/src/nuthatch-compositor
sudo ./run-tty.sh                        # Run compositor on hardware

# Or use full command:
sudo -E RUST_LOG=info ~/.cargo/bin/cargo run --release

# View logs in real-time
tail -f /tmp/nuthatch-compositor.log
```

## Initial Setup (One Time)

### 1. User Permissions

Add yourself to hardware access groups:

```bash
sudo usermod -a -G video $USER
sudo usermod -a -G input $USER
sudo usermod -a -G render $USER

# Log out and back in (or reboot) for groups to take effect
```

Verify:

```bash
groups | grep -E "video|input|render"
```

### 2. Log to File

We already configured this in the code. Logs go to:

- `/tmp/nuthatch-compositor.log`

### 3. Prepare TTY3

Open a terminal and prepare the TTY:

```bash
# Set up convenient aliases (add to ~/.bashrc)
cat >> ~/.bashrc << 'EOF'

# Nuthatch Compositor Development
alias nc-check='cd ~/src/nuthatch-compositor && cargo check 2>&1 | tail -20'
alias nc-build='cd ~/src/nuthatch-compositor && cargo build --release 2>&1 | tail -20'
alias nc-run='cd ~/src/nuthatch-compositor && sudo RUST_LOG=info cargo run --release'
alias nc-log='tail -f /tmp/nuthatch-compositor.log'
alias nc-clean='cd ~/src/nuthatch-compositor && cargo clean'

# Quick kill if compositor hangs
alias nc-kill='sudo pkill -9 -f nuthatch-compositor'
EOF

source ~/.bashrc
```

### 4. Test TTY Access

```bash
# Switch to TTY2 (KDE runs on F3, so use F2 for testing)
# Press: Ctrl+Alt+F2

# Login with your username/password

# Test that you're in the right groups
groups

# Navigate to project
cd ~/src/nuthatch-compositor

# Test build (do this now so it's fast later)
cargo build --release

# Switch back to KDE
# Press: Ctrl+Alt+F3
```

## Development Workflow

### Phase 1: Implement in VS Code

1. **Open VS Code to compositor project:**

   ```bash
   code ~/src/nuthatch-compositor
   ```

2. **Make changes:**

   - Use VS Code Copilot for suggestions
   - Full IntelliSense and error checking
   - Git integration
   - Terminal at bottom for quick builds

3. **Quick validation:**

   ```bash
   # In VS Code terminal (Ctrl+`)
   cargo check
   ```

4. **Save and commit:**
   ```bash
   git add -A
   git commit -m "Description of changes"
   ```

### Phase 2: Test in TTY

1. **Switch to TTY3:**

   ```
   Ctrl + Alt + F3
   ```

2. **Navigate and build:**

   ```bash
   cd ~/src/nuthatch-compositor
   cargo build --release
   ```

3. **Run compositor:**

   ```bash
   sudo RUST_LOG=info cargo run --release
   ```

   **What happens:**

   - Screen goes black briefly
   - Your compositor takes over the display
   - Full screen dark blue (or whatever you render)

4. **Test features:**

   - Keyboard input
   - Mouse movement
   - Window management (if implemented)

5. **Stop compositor:**

   ```
   Ctrl + C
   ```

6. **Check logs:**

   ```bash
   tail -50 /tmp/nuthatch-compositor.log
   ```

7. **Switch back to GUI:**
   ```
   Ctrl + Alt + F1
   ```

### Phase 3: Debug and Iterate

Back in VS Code:

1. Review logs
2. Make fixes
3. Save
4. Return to Phase 2

## Safety Procedures

### If Compositor Crashes

**Scenario 1: Clean exit (Ctrl+C works)**

- You're returned to TTY shell
- Check logs, switch back to GUI

**Scenario 2: Display frozen**

```
1. Try: Ctrl+C (multiple times)
2. Try: Ctrl+Alt+F4 (switch to TTY4)
3. Login and run: sudo pkill -9 -f nuthatch-compositor
4. Switch back: Ctrl+Alt+F1
```

**Scenario 3: Total freeze**

```
1. Try: Ctrl+Alt+Del (clean reboot)
2. Hold power button for 5 seconds (hard reboot)
```

### If Display Won't Come Back

Your compositor may still be running:

```bash
# From another TTY (Ctrl+Alt+F4)
sudo pkill -9 -f nuthatch-compositor

# Restart display manager
sudo systemctl restart sddm

# Switch to GUI
# Ctrl+Alt+F1
```

### Remote SSH Backup

Set up SSH from another device:

```bash
# On another computer
ssh xander@your-fedora-ip

# Check compositor status
ps aux | grep nuthatch

# Kill if needed
sudo pkill -9 -f nuthatch-compositor

# Restart display
sudo systemctl restart sddm
```

## Optimization Tips

### 1. Keep Build Artifacts

Don't clean between builds:

```bash
# DON'T do this unless necessary:
# cargo clean

# DO keep artifacts for faster rebuilds
# Just: cargo build --release
```

### 2. Build Before Testing

Always build in VS Code before switching to TTY:

```bash
# In VS Code terminal
cargo build --release 2>&1 | tail -20
```

Then in TTY you just run the pre-built binary.

### 3. Use Build Cache

First build is slow (~2-5 minutes with dependencies).
Subsequent builds are fast (~5-30 seconds for your changes).

### 4. Parallel Development

Have two shells in VS Code:

- **Shell 1:** Main development
- **Shell 2:** `tail -f /tmp/nuthatch-compositor.log`

### 5. Quick Syntax Check

Before building:

```bash
cargo check  # Much faster than build
```

## Typical Session

### Morning Startup

```bash
# 1. VS Code in GUI
cd ~/src/nuthatch-compositor
code .

# 2. Pull latest changes
git pull

# 3. Build dependencies (once)
cargo build --release

# 4. Start developing
# ... make changes ...
```

### Development Loop

```bash
# In VS Code:
# - Edit code
# - Save (Ctrl+S)
# - Check: cargo check

# When ready to test:
# Ctrl+Alt+F3

# In TTY:
cd ~/src/nuthatch-compositor
cargo build --release && sudo RUST_LOG=info cargo run --release

# Test...
# Ctrl+C

# Back to VS Code:
# Ctrl+Alt+F1

# Repeat
```

### End of Day

```bash
# In VS Code
git add -A
git commit -m "Progress on [feature]"
git push

# Close VS Code
# Power off or leave running
```

## Monitoring and Logging

### Real-time Log Viewing

**Option 1: Second terminal in VS Code**

```bash
# Terminal 1: Development
cargo check

# Terminal 2: Logs
tail -f /tmp/nuthatch-compositor.log
```

**Option 2: After running in TTY**

```bash
# Back in GUI, check what happened
cat /tmp/nuthatch-compositor.log | tail -100
```

### Log Levels

```bash
# Info level (default)
RUST_LOG=info cargo run

# Debug level (verbose)
RUST_LOG=debug cargo run

# Trace level (very verbose)
RUST_LOG=trace cargo run

# Specific module
RUST_LOG=nuthatch_compositor::drm=debug cargo run
```

## Troubleshooting

### "Permission denied" for /dev/dri/card0

```bash
# Check groups
groups

# Add to video group if not there
sudo usermod -a -G video $USER

# Log out and back in
```

### "Device or resource busy"

Another compositor (KDE) is using the GPU:

```bash
# You must run from TTY, not within GUI
# Ctrl+Alt+F3, then run compositor
```

### Build errors after git pull

```bash
# Clean and rebuild
cargo clean
cargo build --release
```

### Compositor starts but shows black screen

Check logs:

```bash
tail -100 /tmp/nuthatch-compositor.log | grep ERROR
```

Common issues:

- EGL initialization failed
- No connected displays found
- Permission issues with DRM

## Quick Commands Reference

```bash
# Check for compile errors (fast)
cargo check

# Build release binary
cargo build --release

# Run on hardware (from TTY only)
sudo RUST_LOG=info cargo run --release

# View recent logs
tail -50 /tmp/nuthatch-compositor.log

# Follow logs in real-time
tail -f /tmp/nuthatch-compositor.log

# Force kill compositor
sudo pkill -9 -f nuthatch-compositor

# Restart display manager
sudo systemctl restart sddm

# Check GPU device
ls -la /dev/dri/

# Check processes using GPU
sudo fuser -v /dev/dri/card0

# Test groups
groups | grep video
```

## Next Steps

1. ✅ Set up permissions (usermod)
2. ✅ Add bash aliases
3. ✅ Test TTY access
4. 🔄 Implement DRM backend
5. 🔜 Test on hardware
6. 🔜 Iterate and refine

Ready to start implementing the DRM backend!
