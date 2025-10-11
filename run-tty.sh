#!/bin/bash
# Simple script to run compositor from TTY
# Usage: sudo ./run-tty.sh

cd ~/src/nuthatch-compositor
export RUST_LOG=info
~/.cargo/bin/cargo run --release
