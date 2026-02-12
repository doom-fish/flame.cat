# @flame-cat/react

Flame graph React component powered by the flame.cat egui/WASM renderer. Embeds the full flame.cat viewer — with all interaction, themes, lanes, minimap, search, and profile format support — as a React component.

## Install

```bash
npm install @flame-cat/react
```

## Prerequisites

Build the WASM bundle from the flame.cat repo:

```bash
cd crates/ui
trunk build --release
# Output in crates/ui/dist/
```

Copy the built files to your app's public directory (or serve them from a CDN).

## Quick Start

```tsx
import { FlameGraph } from "@flame-cat/react";

function App() {
  const [profileData, setProfileData] = useState<ArrayBuffer | null>(null);

  return (
    <div>
      <input
        type="file"
        onChange={async (e) => {
          const file = e.target.files?.[0];
          if (file) setProfileData(await file.arrayBuffer());
        }}
      />
      <FlameGraph
        wasmUrl="/wasm/flame-cat-ui.js"
        data={profileData}
        width="100%"
        height={600}
      />
    </div>
  );
}
```

## Props

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `wasmUrl` | `string` | **required** | URL to the flame-cat WASM JS glue file |
| `data` | `ArrayBuffer \| Uint8Array \| null` | — | Profile data to load. Supports Chrome DevTools, React DevTools, Firefox, perf, etc. |
| `width` | `number \| string` | `"100%"` | Container width |
| `height` | `number \| string` | `600` | Container height |
| `className` | `string` | — | CSS class for wrapper div |
| `style` | `CSSProperties` | — | Inline styles for wrapper div |
| `onReady` | `() => void` | — | Called when WASM finishes loading |
| `onError` | `(error: Error) => void` | — | Called if WASM loading fails |

## Ref API

```tsx
const flameRef = useRef<FlameGraphRef>(null);

<FlameGraph ref={flameRef} wasmUrl="/wasm/flame-cat-ui.js" />

// Programmatic profile loading
flameRef.current?.loadProfile(arrayBuffer);

// Access the WASM module directly
flameRef.current?.getWasmModule()?.loadProfile(uint8Array);
```

## Supported Profile Formats

The embedded WASM viewer parses all formats supported by flame.cat:

- Chrome DevTools Performance traces
- React DevTools Profiler exports
- Firefox Profiler
- Node.js CPU profiles (`.cpuprofile`)
- Linux `perf` script output
- Apple Instruments traces
- V8 CPU profile samples

## Features

All features from the flame.cat viewer are available:

- **Zoom/Pan**: Scroll wheel, drag, keyboard (WASD, +/-, 0 to reset)
- **Search**: Type to filter spans, matching highlighted
- **Minimap**: Density heatmap with draggable viewport
- **Lanes**: Thread lanes, CPU samples, counters, markers, async spans
- **Context menu**: Right-click for Copy Name, Zoom to Span, Find Similar
- **Themes**: Dark/light with automatic `prefers-color-scheme` detection
- **Flow arrows**: Visualize cross-thread relationships

## License

MIT
