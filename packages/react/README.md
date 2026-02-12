# @flame-cat/react

Composable React hooks for the [flame.cat](https://flame.cat) flame graph viewer (egui/WASM).

## Install

```sh
npm install @flame-cat/react
```

## Quick Start

```tsx
import {
  FlameCatProvider,
  FlameCanvas,
  useFlameGraph,
  useSearch,
  useTheme,
  useLanes,
  useSelectedSpan,
} from "@flame-cat/react";

function App() {
  return (
    <FlameCatProvider wasmUrl="/wasm/flame-cat-ui.js">
      <Toolbar />
      <div style={{ display: "flex", height: "100vh" }}>
        <LaneSidebar />
        <FlameCanvas adaptive />
      </div>
      <DetailPanel />
    </FlameCatProvider>
  );
}

function Toolbar() {
  const { loadProfile } = useFlameGraph();
  const { query, setQuery } = useSearch();
  const { mode, setMode } = useTheme();

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
        placeholder="Search…"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
      />
      <button onClick={() => setMode(mode === "dark" ? "light" : "dark")}>
        {mode === "dark" ? "☀️" : "🌙"}
      </button>
    </div>
  );
}

function LaneSidebar() {
  const { lanes, toggleVisibility } = useLanes();
  return (
    <ul>
      {lanes.map((lane, i) => (
        <li key={i}>
          <label>
            <input
              type="checkbox"
              checked={lane.visible}
              onChange={() => toggleVisibility(i)}
            />
            {lane.name}
          </label>
        </li>
      ))}
    </ul>
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

### `<FlameCatProvider wasmUrl={string}>`

Context provider that initializes the WASM viewer. Wrap your app in this.

### `<FlameCanvas adaptive? className? style? onResize?>`

The egui canvas rendering surface. Gets its store from context.

### Hooks

| Hook | Returns | Description |
|------|---------|-------------|
| `useFlameGraph()` | `{ loadProfile, ready }` | Load profiles, check readiness |
| `useProfile()` | `ProfileInfo \| null` | Profile metadata (name, format, duration, span count) |
| `useLanes()` | `{ lanes, toggleVisibility }` | Lane list with visibility control |
| `useViewport()` | `{ start, end, scroll_y, setViewport, resetZoom }` | Zoom/pan state |
| `useSearch()` | `{ query, setQuery }` | Search filter |
| `useTheme()` | `{ mode, setMode }` | Dark/light theme |
| `useSelectedSpan()` | `{ selected, select, clear }` | Span selection |

All hooks must be used within a `<FlameCatProvider>`.
