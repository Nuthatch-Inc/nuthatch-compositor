# Nuthatch Compositor

A custom Wayland compositor built with Smithay for the Nuthatch Desktop Linux port.

## Features (Planned)

- ✨ Custom window management with pseudo-maximize and side-by-side snapping
- 🎨 Hardware-accelerated blur effects and smooth animations
- 🪟 React-based window decorations with custom chrome
- 🎯 Integration with existing nuthatch-desktop React applications
- ⚡ Native Linux application support with custom theming

## Development Status

**Phase 1: Foundation** - In Progress

Current tasks:

- [x] Project setup with Smithay
- [ ] Basic window rendering
- [ ] Window focus and stacking
- [ ] Keyboard and mouse input
- [ ] Basic window management (open, close, move, resize)

## Building

```bash
# Build and run in development mode
cargo run

# Build for release
cargo build --release
```

## Running

### Nested Mode (Safe for Development)

Run the compositor in a window inside your existing desktop environment:

```bash
cargo run
```

This creates a nested Wayland session. You can test by running applications with:

```bash
WAYLAND_DISPLAY=wayland-1 kitty
```

### TTY Mode (Full Screen)

Switch to a TTY (Ctrl+Alt+F3), login, and run:

```bash
cd ~/src/nuthatch-compositor
cargo run --release
```

Press Ctrl+Alt+F2 to return to your KDE session.

## Architecture

```
nuthatch-compositor/
├── src/
│   ├── main.rs           # Entry point and compositor setup
│   ├── window.rs         # Window management logic
│   ├── input.rs          # Keyboard and mouse handling
│   ├── render.rs         # Rendering and visual effects
│   └── ipc.rs            # Communication with desktop shell
├── Cargo.toml
└── README.md
```

## Resources

- [Smithay Documentation](https://smithay.github.io/smithay/)
- [Wayland Book](https://wayland-book.com/)
- [Anvil Reference Compositor](https://github.com/Smithay/smithay/tree/master/anvil)

## License

ISC
