#!/bin/bash
# Test script to run Anvil compositor (reference implementation)
# This verifies that DRM backend works in the current environment
# Use this to test before implementing our own DRM backend

cd ~/src/smithay

echo "Starting Anvil compositor with DRM backend..."
echo "Press Ctrl+C to exit"
echo "Switch TTY: Ctrl+Alt+F1/F2 to return to KDE"
echo ""

RUST_LOG=info ./target/release/anvil --tty-udev
