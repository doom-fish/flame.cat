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
  useColorMode,
  useViewType,
  useLanes,
  useSelectedSpan,
  useHoveredSpan,
  useNavigation,
  useExport,
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
  const { toggle: toggleTheme } = useTheme();
  const { toggle: toggleColor } = useColorMode();
  const { viewType, setViewType } = useViewType();
  const { canGoBack, back, canGoForward, forward } = useNavigation();
  const { exportJSON, exportSVG } = useExport();
  const searchRef = useRef<HTMLInputElement>(null);

  useHotkeys({}, searchRef);

  if (status === "error") return <div>Error: {error}</div>;

  return (
    <div>
      <input type="file" onChange={async (e) => {
        const file = e.target.files?.[0];
        if (file) loadProfile(await file.arrayBuffer());
      }} />
      <button onClick={() => setViewType("left_heavy")}>Left Heavy</button>
      <button onClick={() => setViewType("icicle")}>Icicle</button>
      <button disabled={!canGoBack} onClick={back}>←</button>
      <button disabled={!canGoForward} onClick={forward}>→</button>
      <input ref={searchRef} placeholder="Search…" value={query}
        onChange={(e) => setQuery(e.target.value)} />
      <button onClick={toggleTheme}>Theme</button>
      <button onClick={toggleColor}>Color</button>
      <button onClick={() => exportJSON()}>💾 JSON</button>
      <button onClick={() => exportSVG()}>🖼 SVG</button>
    </div>
  );
}

function LaneSidebar() {
  const { lanes, toggleVisibility, showAll, hideAll, reorder } = useLanes();
  return (
    <div>
      <button onClick={showAll}>Show all</button>
      <button onClick={hideAll}>Hide all</button>
      <ul>
        {lanes.map((lane, i) => (
          <li key={i}>
            <input type="checkbox" checked={lane.visible}
              onChange={() => toggleVisibility(i)} />
            {lane.name} ({lane.span_count} spans)
          </li>
        ))}
      </ul>
    </div>
  );
}

function DetailPanel() {
  const { selected, clear } = useSelectedSpan();
  const hovered = useHoveredSpan();

  return (
    <div>
      {hovered && <div>Hovering: {hovered.name}</div>}
      {selected && (
        <div>
          <strong>{selected.name}</strong>
          <span> ({selected.end_us - selected.start_us}µs)</span>
          <button onClick={clear}>×</button>
        </div>
      )}
    </div>
  );
}
```

## API

### `<FlameCatProvider wasmUrl onError?>`

Context provider. Initializes WASM and creates the reactive store.

### `<FlameCatViewer className? style? ariaLabel?>`

The egui rendering surface. Size it with CSS — eframe handles the rest.

### Hooks

| Hook | Returns | Description |
|------|---------|-------------|
| `useFlameGraph()` | `{ loadProfile, ready }` | Load profiles, check readiness |
| `useStatus()` | `{ status, error }` | Lifecycle: `"loading"` → `"ready"` / `"error"` |
| `useProfile()` | `ProfileInfo \| null` | Profile metadata |
| `useViewType()` | `{ viewType, setViewType }` | Switch views: `time_order`, `left_heavy`, `icicle`, `sandwich`, `ranked` |
| `useColorMode()` | `{ colorMode, setColorMode, toggle }` | Coloring: `by_name` (package hash) or `by_depth` |
| `useLanes()` | `{ lanes, toggleVisibility, setVisibility, setHeight, reorder, showAll, hideAll }` | Full lane control |
| `useViewport()` | `{ start, end, scroll_y, setViewport, resetZoom }` | Zoom/pan (clamped 0–1) |
| `useSearch()` | `{ query, setQuery }` | Search filter |
| `useTheme()` | `{ mode, setMode, toggle }` | Dark/light theme |
| `useSelectedSpan()` | `{ selected, select, clear }` | Click selection |
| `useHoveredSpan()` | `SelectedSpanInfo \| null` | Real-time hover info |
| `useSpanNavigation()` | `{ goToParent, goToChild, goToNextSibling, goToPrevSibling, nextSearchResult, prevSearchResult }` | Keyboard-style span navigation |
| `useNavigation()` | `{ canGoBack, canGoForward, back, forward }` | Zoom history breadcrumbs |
| `useExport()` | `{ exportJSON, exportSVG }` | Export profile as JSON or SVG |
| `useHotkeys(map?, searchRef?)` | `void` | Keyboard shortcuts |

### Input Validation

- `setViewport` clamps to `[0, 1]`
- `setHeight` clamps to `[16, 600]`
- `reorder`, `setVisibility`, `setHeight` no-op on out-of-bounds indices
- `useHotkeys(false)` disables all shortcuts
- `setViewType` validates against known types

All hooks must be used within a `<FlameCatProvider>`.
