# Copilot Instructions — flame.cat

## Project Overview

flame.cat is a high-performance flame graph visualization tool. It renders with WebGPU for pixel-perfect graphics and uses Rust for backend data processing. The tool is inspired by [Speedscope](https://github.com/jlfwong/speedscope) and [React DevTools Profiler](https://react.dev/reference/react/Profiler), combining their visualization approaches into a unified multi-lane interface.

## Architecture

Monorepo with a **shared render command protocol**. The Rust core never draws anything — it produces a flat list of typed render commands. Multiple renderers consume these commands independently.

```
flame-cat/
├── crates/
│   ├── core/           # Profile model, parsers, view transforms
│   ├── protocol/       # Render command types (shared by all renderers)
│   ├── tui/            # Rust TUI renderer (ratatui + crossterm)
│   └── wasm/           # WASM bridge for web frontends
├── web/                # Web frontend (TypeScript)
│   └── src/renderers/  # webgpu/ canvas/ svg/ webgl/
└── Cargo.toml          # Workspace root
```

### Data flow

```
Profile File → Parser → FlameGraph Model → View Transform → Render Commands → Renderer
                  (Rust core)                  (Rust core)         ↕           (per-target)
                                                            protocol crate
```

### Render command protocol (`crates/protocol/`)

The central abstraction. `RenderCommand` is an enum: `DrawRect`, `DrawText`, `DrawLine`, `SetClip`, `PushTransform`, `PopTransform`. Commands carry semantic `ThemeToken` values for colors — never raw RGBA. Commands should be stateless (prefer `DrawRect { x, y, w, h, color_token, label }` over `SetColor` + `DrawRect`). Must be `serde`-serializable and WASM-compatible.

### Rust core (`crates/core/`)

- Parsing profile formats (Chrome DevTools, Firefox, Node.js, perf, Instruments, React DevTools exports)
- Building and transforming flame graph data structures (call trees, aggregation, folding)
- View transforms: pure functions from `Profile` → `Vec<RenderCommand>`

### WASM bridge (`crates/wasm/`)

Compiled with `wasm-pack`. Exposes `parse_profile(bytes)` and `render_view(handle, view_type, viewport)` to TypeScript. All web renderers consume render commands through this bridge.

### Web renderers (`web/src/renderers/`)

Each renderer implements the same interface — consuming `RenderCommand[]` and drawing to its target:

- **WebGPU** — instanced quad rendering, SDF text, scissor clipping. Primary renderer.
- **Canvas2D** — `CanvasRenderingContext2D` fallback for browsers without WebGPU
- **WebGL** — middle ground between Canvas2D and WebGPU compatibility/performance
- **SVG** — for static exports and embedding

### TUI renderer (`crates/tui/`)

Native Rust binary using `ratatui` + `crossterm`. Consumes the same `RenderCommand` protocol, mapping rectangles to colored terminal cells. Supports mouse interaction. Run as `flame-cat profile.json`.

## Visualization Modes

All views render in synchronized **lanes** — horizontal tracks that can be vertically stacked and precisely time-aligned. This is a core differentiator.

### Speedscope-inspired views

- **Time Order** — call stacks in chronological order, X-axis = wall time
- **Left Heavy** — identical stacks merged and sorted heaviest-left, X-axis = aggregated time
- **Sandwich** — select a frame to see callers above and callees below

### React DevTools-inspired views

- **Component Tree Flame** — hierarchical view where bar width = render duration, color = render cost
- **Ranked View** — flat list of components sorted by render time
- **Commit Timeline** — horizontal timeline of React commits, clickable to inspect individual renders

### Lane system

Lanes are the fundamental layout primitive. Any view above can be placed in a lane. Multiple lanes stack vertically with:

- Shared time axis (for time-aligned views)
- Independent vertical scroll per lane
- Resizable lane heights via drag handles
- Lane headers showing the view type and data source

## Rendering Conventions

### Pixel-perfect rendering

- All rectangle edges must align to exact pixel boundaries (snap coordinates to device pixels)
- Text must be crisp at all zoom levels — use SDF (Signed Distance Field) or multi-channel SDF font rendering
- Line separators and borders must be exactly 1 device pixel wide
- Test rendering at multiple DPR values (1x, 2x, 3x)

### Theming

Support light and dark themes via a theme token system:

- Define all colors as semantic tokens (e.g., `flame.hot`, `flame.cold`, `lane.background`, `lane.border`)
- Theme tokens are passed as a uniform buffer to shaders
- Never hardcode colors in shaders or TypeScript rendering code
- Respect `prefers-color-scheme` by default, allow manual override

## Build & Run

```sh
# Rust backend
cargo build                    # debug build
cargo build --release          # release build
cargo test                     # run all Rust tests
cargo test test_name           # run a single test
cargo clippy                   # lint Rust code
cargo fmt --check              # check Rust formatting

# WASM build
wasm-pack build --target web

# Frontend
npm install                    # install dependencies
npm run dev                    # dev server with hot reload
npm run build                  # production build
npm run lint                   # lint TypeScript (ESLint)
npm run fmt                    # format check (Prettier)
npm test                       # run all frontend tests
npm test -- --grep "pattern"   # run a single test by name

# Full check before committing
cargo fmt --check && cargo clippy -- -D warnings && cargo test && npm run lint && npm test
```

## Code Conventions

### Rust

- Use `clippy` with `-D warnings` (all warnings are errors in CI)
- Format with `rustfmt` — no style debates
- Error handling: use `thiserror` for library errors, `anyhow` for application code
- Profile parsers go in dedicated modules under `src/parsers/`, one per format
- Data structures for the flame graph model live in `src/model/`

### TypeScript

- Strict TypeScript — `strict: true` in tsconfig, no `any` unless unavoidable and annotated with `// eslint-disable-next-line`
- ESLint + Prettier — format on save
- WebGPU pipeline code lives in `src/gpu/`, shader files are `.wgsl`
- WGSL shaders are kept as separate files, not inline strings
- Theme tokens are defined in `src/themes/` and passed to shaders via uniform buffers

### Commits

- Commit often — small, atomic commits with descriptive messages
- Use conventional commit format: `feat:`, `fix:`, `refactor:`, `perf:`, `test:`, `docs:`, `chore:`

### Testing

- Rust: unit tests inline (`#[cfg(test)]` modules), integration tests in `tests/`
- Frontend: test rendering output against snapshot images at key zoom/theme combinations
- Profile parsers must have test fixtures for each supported format
