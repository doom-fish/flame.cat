# @flame-cat/react

Composable React hooks for the [flame.cat](https://flame.cat) egui/WASM flame graph viewer.

## Install

```sh
npm install @flame-cat/react
```

## Quick Start

```tsx
import {
  FlameCatProvider,
  FlameCatViewer,
  useFlameGraph,
  useStatus,
  useSearch,
  useTheme,
  useLanes,
  useSelectedSpan,
  useHotkeys,
} from "@flame-cat/react";

function App() {
  return (
    <FlameCatProvider
      wasmUrl="/wasm/flame-cat-ui.js"
      onError={(err) => console.error("WASM failed:", err)}
    >
      <Toolbar />
      <div style={{ display: "flex", height: "100vh" }}>
        <LaneSidebar />
        <FlameCatViewer />
      </div>
      <DetailPanel />
    </FlameCatProvider>
  );
}

function Toolbar() {
  const { loadProfile } = useFlameGraph();
  const { status, error } = useStatus();
  const { query, setQuery } = useSearch();
  const { toggle } = useTheme();
  const searchRef = useRef<HTMLInputElement>(null);

  useHotkeys({}, searchRef); // keyboard shortcuts

  if (status === "error") return <div>Error: {error}</div>;

  return (
    <div>
      <input
        type="file"
        onChange={async (e) => {
          const file = e.target.files?.[0];
          if (file) loadProfile(await file.arrayBuffer());
        }}
      />
      <input
        ref={searchRef}
        placeholder="Search… (press /)"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
      />
      <button onClick={toggle}>Toggle theme</button>
    </div>
  );
}

function LaneSidebar() {
  const { lanes, toggleVisibility, showAll, hideAll, setHeight, reorder } = useLanes();
  return (
    <div>
      <button onClick={showAll}>Show all</button>
      <button onClick={hideAll}>Hide all</button>
      <ul>
        {lanes.map((lane, i) => (
          <li key={i}>
            <input
              type="checkbox"
              checked={lane.visible}
              onChange={() => toggleVisibility(i)}
            />
            {lane.name} ({lane.span_count} spans)
          </li>
        ))}
      </ul>
    </div>
  );
}

function DetailPanel() {
  const { selected, clear } = useSelectedSpan();
  if (!selected) return null;
  return (
    <div>
      <strong>{selected.name}</strong>
      <span> ({selected.end_us - selected.start_us}µs)</span>
      <button onClick={clear}>×</button>
    </div>
  );
}
```

## API

### `<FlameCatProvider wasmUrl onError?>`

Context provider. Initializes WASM and creates the reactive store.

| Prop | Type | Description |
|------|------|-------------|
| `wasmUrl` | `string` | URL to WASM JS glue file |
| `onError` | `(error: Error) => void` | Called on initialization failure |

### `<FlameCatViewer className? style? ariaLabel?>`

The egui rendering surface. Size it with CSS on the container — eframe handles the rest.

### Hooks

| Hook | Returns | Description |
|------|---------|-------------|
| `useFlameGraph()` | `{ loadProfile, ready }` | Load profiles, check readiness |
| `useStatus()` | `{ status, error }` | Lifecycle: `"loading"` → `"ready"` or `"error"` |
| `useProfile()` | `ProfileInfo \| null` | Profile metadata |
| `useLanes()` | `{ lanes, toggleVisibility, setVisibility, setHeight, reorder, showAll, hideAll }` | Full lane control |
| `useViewport()` | `{ start, end, scroll_y, setViewport, resetZoom }` | Zoom/pan (values clamped 0–1) |
| `useSearch()` | `{ query, setQuery }` | Search filter |
| `useTheme()` | `{ mode, setMode, toggle }` | Dark/light theme |
| `useSelectedSpan()` | `{ selected, select, clear }` | Span selection |
| `useHotkeys(map?, searchRef?)` | `void` | Keyboard shortcuts (`0`=reset zoom, `t`=theme, `/`=search, `Esc`=clear) |

### Input Validation

- `setViewport` clamps to `[0, 1]`
- `setHeight` clamps to `[16, 600]`
- `reorder`, `setVisibility`, `setHeight` silently no-op on out-of-bounds indices
- `useHotkeys(false)` disables all shortcuts

All hooks must be used within a `<FlameCatProvider>`.
