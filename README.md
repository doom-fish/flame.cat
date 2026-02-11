# 🔥 flame.cat

High-performance flame graph visualization tool. Renders with egui (WebGL2/native) and uses Rust for all data processing.

[![CI](https://github.com/user/flame.cat/actions/workflows/ci.yml/badge.svg)](https://github.com/user/flame.cat/actions/workflows/ci.yml)

## Features

- **10 profile formats**: Chrome DevTools, Firefox Gecko, React DevTools, Speedscope, V8 CPU Profile, pprof, PIX, Tracy, eBPF/perf, Collapsed Stacks
- **Multi-lane visualization**: Thread flame charts, counter tracks, marker tracks, async spans, CPU samples, frame timing, object lifecycles
- **Interactive minimap**: Density heatmap with draggable viewport
- **Keyboard navigation**: WASD pan, +/- zoom, Ctrl+scroll zoom at cursor, double-click zoom to span
- **Search**: Filter spans by name with real-time highlighting
- **Cross-platform**: Runs in any browser (WASM + WebGL2) — native desktop coming soon

## Quick Start

```sh
# Install trunk (WASM bundler)
cargo install trunk

# Run dev server
cd crates/ui && trunk serve --open

# Load the demo profile
# Navigate to http://localhost:8080/#demo
```

## Build

```sh
# Development
cargo build
cargo test -p flame-cat-core -p flame-cat-protocol

# WASM release
cd crates/ui && trunk build --release

# Lint
cargo fmt --check
cargo clippy -p flame-cat-core -p flame-cat-protocol -- -D warnings
```

## Architecture

```
crates/
├── core/       # Profile parsers, view renderers → Vec<RenderCommand>
├── protocol/   # RenderCommand, ThemeToken, geometric types (shared IR)
├── ui/         # egui app (eframe for WASM + native)
└── tui/        # Terminal UI renderer (ratatui)
```

**Data flow**: `Profile bytes → Parser → VisualProfile → View Transform → Vec<RenderCommand> → egui Painter → Screen`

The render command protocol is the central abstraction. Core never draws — it produces typed commands. The UI crate translates them to egui painter calls.

## Supported Formats

| Format | Source |
|--------|--------|
| Chrome Trace | Chrome DevTools, Edge, Electron |
| Firefox Gecko | Firefox Profiler |
| React DevTools | React Profiler exports |
| Speedscope | speedscope.app exports |
| V8 CPU Profile | Node.js `--cpu-prof` |
| pprof | Go, gRPC profiling |
| PIX | Xbox/Windows game profiling |
| Tracy | Tracy profiler captures |
| eBPF/perf | `perf script`, bpftrace output |
| Collapsed Stacks | flamegraph.pl format |

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `A` / `←` | Pan left |
| `D` / `→` | Pan right |
| `W` / `↑` | Scroll up |
| `S` / `↓` | Scroll down |
| `+` | Zoom in |
| `-` | Zoom out |
| `0` | Reset zoom |
| `Ctrl+Scroll` | Zoom at cursor |
| `Double-click` | Zoom to span |
| `Click` | Select span |
| `Esc` | Deselect |
| `?` | Keyboard help |

## License

MIT
