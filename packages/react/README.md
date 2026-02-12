# @flame-cat/react

Flame graph React component powered by flame.cat egui/WASM. Controlled via a hook-based controller.

## Install

```bash
npm install @flame-cat/react
```

## Usage

```tsx
import { FlameGraph, useFlameGraph } from "@flame-cat/react";

function App() {
  const flame = useFlameGraph();

  async function handleFile(e: React.ChangeEvent<HTMLInputElement>) {
    const buf = await e.target.files?.[0]?.arrayBuffer();
    if (buf) flame.loadProfile(buf);
  }

  return (
    <div>
      <input type="file" onChange={handleFile} />
      <button onClick={() => flame.resetZoom()}>Reset</button>
      <button onClick={() => flame.setTheme("light")}>Light</button>
      <button onClick={() => flame.setSearch("render")}>Search</button>

      <FlameGraph
        controller={flame}
        wasmUrl="/wasm/flame-cat-ui.js"
        height={600}
      />
    </div>
  );
}
```

## API

### `useFlameGraph()` → `FlameGraphController`

Creates a controller. All methods can be called before WASM loads — they queue and flush on init.

```ts
const flame = useFlameGraph();

flame.loadProfile(arrayBuffer);      // Load any supported profile format
flame.setTheme("dark" | "light");    // Switch theme
flame.setSearch("query");            // Filter/highlight spans
flame.resetZoom();                   // Fit all data
flame.ready;                         // boolean — true once WASM is initialized
```

### `<FlameGraph>`

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `controller` | `FlameGraphController` | **required** | Controller from `useFlameGraph()` |
| `wasmUrl` | `string` | **required** | URL to WASM JS glue from `trunk build` |
| `width` | `number \| string` | `"100%"` | Container width |
| `height` | `number \| string` | `"100%"` | Container height |
| `className` | `string` | — | CSS class |
| `style` | `CSSProperties` | — | Inline styles |
| `onError` | `(error: Error) => void` | — | WASM init failure callback |

## Building the WASM Bundle

```bash
cd crates/ui && trunk build --release
# Copy dist/ to your app's public/wasm/
```

## License

MIT
