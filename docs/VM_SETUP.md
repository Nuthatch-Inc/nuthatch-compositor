# VM Development Setup for Nuthatch Compositor

## Quick Start

```bash
# Run the setup script
./setup-vm.sh

# After setup, log out and back in, then:
virt-manager
```

## VM Configuration

The setup creates a VM with:
- **Name**: nuthatch-compositor-dev
- **OS**: Fedora 42 KDE Plasma (same as host)
- **RAM**: 4GB (increase to 8GB if needed)
- **CPUs**: 2 cores
- **Disk**: 40GB
- **Graphics**: VirtIO-GPU with 3D acceleration
- **Display**: SPICE with OpenGL enabled

## Why These Settings?

- **VirtIO-GPU**: Provides DRM/KMS device needed for testing compositor in TTY mode
- **3D Acceleration**: Better performance for compositor testing
- **Same Fedora Version**: Ensures compatibility with dependencies

## After VM Installation

### 1. Install Development Tools

```bash
# In the VM
sudo dnf install -y rust cargo git vim
```

### 2. Get Your Code Into the VM

**Option A: Clone from Git** (recommended)
```bash
# In the VM
mkdir -p ~/src
cd ~/src
git clone <your-repo-url> nuthatch-compositor
cd nuthatch-compositor
```

**Option B: Shared Folder**
```bash
# On host, create shared folder
mkdir -p ~/vm-shared

# In virt-manager:
# 1. Right-click VM → Details
# 2. Add Hardware → Filesystem
# 3. Source path: /home/xander/vm-shared
# 4. Target path: shared
# 5. In VM, mount: sudo mount -t 9p -o trans=virtio shared /mnt/shared
```

### 3. Build and Test

```bash
# In the VM
cd ~/src/nuthatch-compositor
cargo build

# Test in nested mode first
cargo run

# Test in TTY mode (the real test!)
# Press Ctrl+Alt+F3 to switch to TTY3
# Log in
cd ~/src/nuthatch-compositor
cargo run --release -- --drm

# To get back to GUI: Ctrl+Alt+F2
```

## VM Management

```bash
# Start VM
virsh start nuthatch-compositor-dev

# Stop VM
virsh shutdown nuthatch-compositor-dev

# Force stop (if compositor hangs)
virsh destroy nuthatch-compositor-dev

# Delete VM and disk
virsh undefine nuthatch-compositor-dev --remove-all-storage

# List all VMs
virsh list --all

# Increase RAM (when VM is off)
virsh setmaxmem nuthatch-compositor-dev 8G --config
virsh setmem nuthatch-compositor-dev 8G --config

# Increase CPUs (when VM is off)
virsh setvcpus nuthatch-compositor-dev 4 --config --maximum
virsh setvcpus nuthatch-compositor-dev 4 --config
```

## Troubleshooting

### "Permission denied" errors with virsh
```bash
# Check group membership
groups

# If libvirt not listed:
newgrp libvirt
# or log out and back in
```

### VM won't start
```bash
# Check libvirtd is running
sudo systemctl status libvirtd

# Check logs
sudo journalctl -u libvirtd -n 50
```

### No KVM acceleration
```bash
# Check if KVM module is loaded
lsmod | grep kvm

# Check CPU virtualization support
grep -E 'vmx|svm' /proc/cpuinfo

# If nothing, enable VT-x/AMD-V in BIOS
```

### Slow performance
- Increase RAM: virt-manager → VM → Details → Memory
- Increase CPUs: virt-manager → VM → Details → CPUs
- Ensure KVM is enabled (see above)
- Check VirtIO drivers are in use (not IDE)

## Development Workflow

1. Write code on host (use your familiar editor/IDE)
2. Commit to git
3. Pull in VM: `git pull`
4. Build and test in VM
5. If compositor crashes → just reset VM, host is unaffected!

## Benefits Over Host Development

- **Safe TTY testing**: Compositor bugs won't lock up your host system
- **Easy recovery**: Force reset VM if compositor hangs
- **Isolated environment**: No risk of corrupting host display server
- **Multiple VMs**: Create snapshots before risky changes
- **Real DRM/KMS**: Test actual DRM backend without risking host

## Advanced: VM Snapshots

```bash
# Create snapshot before risky test
virsh snapshot-create-as nuthatch-compositor-dev \
    --name "before-drm-test" \
    --description "Clean state before DRM testing"

# List snapshots
virsh snapshot-list nuthatch-compositor-dev

# Revert to snapshot
virsh snapshot-revert nuthatch-compositor-dev before-drm-test

# Delete snapshot
virsh snapshot-delete nuthatch-compositor-dev before-drm-test
```
