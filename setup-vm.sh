#!/bin/bash
set -e

echo "=== Nuthatch Compositor VM Setup ==="
echo ""
echo "This script will:"
echo "1. Install virtualization tools (QEMU/KVM, virt-manager)"
echo "2. Enable and start libvirtd service"
echo "3. Add your user to the libvirt group"
echo "4. Download Fedora 42 KDE Spin ISO"
echo "5. Create a VM optimized for compositor development"
echo ""

# Check if running as root
if [ "$EUID" -eq 0 ]; then
    echo "Please run this script as your normal user (it will prompt for sudo when needed)"
    exit 1
fi

# 1. Install virtualization packages
echo "=== Step 1: Installing virtualization packages ==="
sudo dnf install -y @virtualization virt-manager qemu libvirt-daemon-config-network

# 2. Enable and start libvirtd
echo ""
echo "=== Step 2: Starting libvirtd service ==="
sudo systemctl enable --now libvirtd

# 3. Add user to libvirt group
echo ""
echo "=== Step 3: Adding user to libvirt group ==="
sudo usermod -a -G libvirt $(whoami)

# 4. Check for KVM support
echo ""
echo "=== Step 4: Checking KVM support ==="
if [ -e /dev/kvm ]; then
    echo "✓ KVM is available"
    ls -l /dev/kvm
else
    echo "⚠ WARNING: /dev/kvm not found. Hardware virtualization may not be enabled."
    echo "  Check BIOS settings for Intel VT-x or AMD-V"
fi

# 5. Download Fedora 42 KDE ISO
echo ""
echo "=== Step 5: Preparing Fedora 42 KDE ISO ==="
ISO_DIR="/var/lib/libvirt/images"
ISO_FILE="$ISO_DIR/Fedora-KDE-Live-x86_64-42.iso"
ISO_DOWNLOAD="$HOME/Downloads/Fedora-KDE-Live-x86_64-42.iso"

if [ -f "$ISO_FILE" ]; then
    echo "✓ ISO already in system location: $ISO_FILE"
elif [ -f "$ISO_DOWNLOAD" ]; then
    echo "Found ISO in Downloads, copying to system location..."
    sudo cp "$ISO_DOWNLOAD" "$ISO_FILE"
    echo "✓ ISO copied to system location"
else
    echo "Downloading Fedora 42 KDE Spin ISO..."
    echo "Note: This is ~2.8GB and may take a while"
    
    # Using Fedora's download server
    ISO_URL="https://download.fedoraproject.org/pub/fedora/linux/releases/42/Spins/x86_64/iso/Fedora-KDE-Live-x86_64-42-1.1.iso"
    
    echo "Downloading from: $ISO_URL"
    curl -L -o "$ISO_DOWNLOAD.tmp" "$ISO_URL" && mv "$ISO_DOWNLOAD.tmp" "$ISO_DOWNLOAD"
    
    if [ -f "$ISO_DOWNLOAD" ]; then
        echo "✓ ISO downloaded successfully"
        echo "Copying to system location..."
        sudo cp "$ISO_DOWNLOAD" "$ISO_FILE"
    else
        echo "✗ Failed to download ISO"
        exit 1
    fi
fi

# 6. Create VM
echo ""
echo "=== Step 6: Creating VM for compositor development ==="
VM_NAME="nuthatch-compositor-dev"
VM_DISK="/var/lib/libvirt/images/${VM_NAME}.qcow2"

# Check if VM already exists
if sudo virsh list --all | grep -q "$VM_NAME"; then
    echo "VM '$VM_NAME' already exists"
    echo "To recreate it, run: sudo virsh undefine $VM_NAME --remove-all-storage"
    echo "Then run this script again"
else
    echo "Creating VM with:"
    echo "  - Name: $VM_NAME"
    echo "  - RAM: 4GB"
    echo "  - CPUs: 2"
    echo "  - Disk: 40GB"
    echo "  - Graphics: VirtIO-GPU with 3D acceleration"
    echo ""
    
    mkdir -p "$(dirname "$VM_DISK")"
    
    sudo virt-install \
        --connect qemu:///system \
        --name "$VM_NAME" \
        --ram 4096 \
        --vcpus 2 \
        --disk path="$VM_DISK",size=40,format=qcow2,bus=virtio \
        --cdrom "$ISO_FILE" \
        --os-variant fedora41 \
        --network network=default,model=virtio \
        --graphics spice,gl.enable=yes,listen=none \
        --video virtio \
        --channel spicevmc,target_type=virtio,name=com.redhat.spice.0 \
        --console pty,target_type=serial \
        --noautoconsole \
        --boot uefi
    
    echo ""
    echo "✓ VM created successfully!"
fi

echo ""
echo "=== Setup Complete! ==="
echo ""
echo "Next steps:"
echo "1. Log out and back in (or run: newgrp libvirt) to apply group membership"
echo "2. Start virt-manager: virt-manager"
echo "3. Open the '$VM_NAME' VM and install Fedora"
echo "4. After installation, install development tools in the VM:"
echo "   sudo dnf install -y rust cargo git"
echo "5. Clone your compositor code in the VM or use shared folder"
echo ""
echo "VM Features:"
echo "- VirtIO-GPU with 3D acceleration for testing compositor"
echo "- 4GB RAM (adjustable via virt-manager)"
echo "- 2 CPUs (adjustable via virt-manager)"
echo "- 40GB disk"
echo ""
echo "Tips:"
echo "- The VM has its own TTY - perfect for testing DRM mode"
echo "- If compositor crashes, you can force reset the VM without affecting your host"
echo "- Use shared folder or git to sync code between host and VM"
