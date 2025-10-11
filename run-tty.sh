#!/bin/bash
# Simple script to run compositor from TTY
# Usage: ./run-tty.sh (builds as user, runs with sudo)

cd ~/src/nuthatch-compositor

# Build as regular user
echo "Building compositor..."
cargo build --release

# Run with sudo
echo "Running compositor with sudo..."
sudo RUST_LOG=info ./target/release/nuthatch-compositor
