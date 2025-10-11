#!/bin/bash
# Run these commands inside the VM to set up the compositor development environment

echo "=== Installing Development Tools ==="
sudo dnf update -y

echo ""
echo "=== Installing Rust and Compositor Dependencies ==="
sudo dnf install -y \
    rust cargo rustfmt clippy \
    git \
    gcc clang \
    mesa-libgbm-devel \
    libdrm-devel \
    systemd-devel \
    wayland-devel \
    libinput-devel \
    libxkbcommon-devel \
    mesa-vulkan-devel \
    vulkan-loader \
    seatd \
    cmake \
    pkg-config

echo ""
echo "=== Cloning Compositor Code ==="
cd ~
git clone https://github.com/Nuthatch-Inc/nuthatch-compositor.git || echo "Already cloned or clone failed"
cd nuthatch-compositor

echo ""
echo "=== Building Compositor ==="
cargo build

echo ""
echo "=== Setup Complete! ==="
echo ""
echo "To test the compositor:"
echo "1. Switch to a TTY: Ctrl+Alt+F3"
echo "2. Log in"
echo "3. Run: cd ~/nuthatch-compositor && cargo run"
echo ""
echo "The compositor should take over the TTY and start rendering!"
