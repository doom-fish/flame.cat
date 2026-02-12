# @flame-cat/react

Embeds the flame.cat egui/WASM flame graph viewer as a React component. All control is through an imperative ref handle.

## Install

```bash
npm install @flame-cat/react
```

## Usage

```tsx
import { useRef } from "react";
import { FlameGraph, type FlameGraphHandle } from "@flame-cat/react";

function App() {
  const fg = useRef<FlameGraphHandle>(null);

  async function handleFile(e: React.ChangeEvent<HTMLInputElement>) {
    const buf = await e.target.files?.[0]?.arrayBuffer();
    if (buf) fg.current?.loadProfile(buf);
  }

  return (
    <div>
      <input type="file" onChange={handleFile} />
      <button onClick={() => fg.current?.resetZoom()}>Reset Zoom</button>
      <button onClick={() => fg.current?.setTheme("light")}>Light</button>
      <button onClick={() => fg.current?.setSearch("render")}>Search</button>

      <FlameGraph
        ref={fg}
        wasmUrl="/wasm/flame-cat-ui.js"
        width="100%"
        height={600}
        onReady={() => console.log("flame graph ready")}
      />
    </div>
  );
}
```

## Props

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `wasmUrl` | `string` | **required** | URL to the WASM JS glue file from `trunk build` |
| `width` | `number \| string` | `"100%"` | Container width |
| `height` | `number \| string` | `"100%"` | Container height |
| `className` | `string` | — | CSS class for container |
| `style` | `CSSProperties` | — | Inline styles for container |
| `onReady` | `() => void` | — | Called when WASM is initialized |
| `onError` | `(error: Error) => void` | — | Called on initialization failure |

## Ref Handle (`FlameGraphHandle`)

All control is through the ref:

```ts
interface FlameGraphHandle {
  /** Load a profile (Chrome DevTools, React DevTools, Firefox, perf, etc). */
  loadProfile(data: ArrayBuffer | Uint8Array): void;
  /** Set theme: "dark" or "light". */
  setTheme(mode: "dark" | "light"): void;
  /** Set search/filter query. Empty string clears. */
  setSearch(query: string): void;
  /** Reset zoom to fit all data. */
  resetZoom(): void;
  /** True once WASM has initialized. */
  isReady(): boolean;
}
```

## Building the WASM Bundle

```bash
cd crates/ui
trunk build --release
# Output: crates/ui/dist/
# Copy dist/ contents to your app's public/wasm/ directory
```

## License

MIT
