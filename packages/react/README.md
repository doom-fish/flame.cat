# @flame-cat/react

High-performance flame graph React component with Canvas2D rendering. Zero dependencies beyond React.

## Install

```bash
npm install @flame-cat/react
```

## Quick Start

```tsx
import { FlameGraph } from "@flame-cat/react";

const spans = [
  { id: 1, name: "main", start: 0, end: 100, depth: 0 },
  { id: 2, name: "handleRequest", start: 5, end: 85, depth: 1 },
  { id: 3, name: "queryDB", start: 10, end: 60, depth: 2 },
  { id: 4, name: "serialize", start: 65, end: 80, depth: 2 },
];

function App() {
  return (
    <FlameGraph
      spans={spans}
      onSpanClick={(e) => console.log("Clicked:", e.span.name)}
    />
  );
}
```

## Props

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `spans` | `FlameSpan[]` | required | Array of spans to render |
| `width` | `number` | auto | Width in CSS pixels. Auto-fills container if omitted |
| `height` | `number` | auto | Height in CSS pixels. Auto-computed from depth if omitted |
| `theme` | `FlameTheme` | `DARK_THEME` | Color theme |
| `search` | `string` | — | Highlight matching spans, dim non-matching |
| `viewport` | `FlameViewport` | — | Controlled viewport (zoom/pan state) |
| `onSpanClick` | `(e: SpanEvent) => void` | — | Called when a span is clicked |
| `onSpanHover` | `(e: SpanEvent \| null) => void` | — | Called on hover |
| `onViewportChange` | `(vp: FlameViewport) => void` | — | Called on zoom/pan |
| `minZoom` | `number` | `0.0001` | Minimum zoom fraction |
| `className` | `string` | — | CSS class for wrapper div |
| `style` | `CSSProperties` | — | Inline styles for wrapper div |

## Interaction

- **Drag** to pan horizontally
- **Ctrl+Scroll** to zoom (centered on cursor)
- **Shift+Scroll** to pan horizontally
- **Scroll** to scroll vertically
- **Double-click** a span to zoom to it
- **Double-click** empty space to reset zoom

## FlameSpan

```ts
interface FlameSpan {
  id?: number;       // Auto-assigned if omitted
  name: string;      // Display label
  start: number;     // Start time/value
  end: number;       // End time/value
  depth: number;     // Stack depth (0 = root)
  category?: string; // Optional grouping
  selfTime?: number; // Exclusive time
}
```

## Themes

Two built-in themes: `DARK_THEME` (default) and `LIGHT_THEME`.

```tsx
import { FlameGraph, LIGHT_THEME } from "@flame-cat/react";

<FlameGraph spans={spans} theme={LIGHT_THEME} />
```

Custom themes:

```tsx
const customTheme: FlameTheme = {
  background: "#0d1117",
  flamePalette: ["#ff6b6b", "#ffd93d", "#6bcb77", "#4d96ff"],
  textColor: "#c9d1d9",
  borderColor: "#30363d",
  hoverColor: "rgba(255,255,255,0.1)",
  selectedColor: "#58a6ff",
  dimmedAlpha: 0.2,
  tooltipBackground: "#161b22",
  tooltipText: "#c9d1d9",
  tooltipBorder: "#30363d",
};
```

## Controlled Viewport

```tsx
const [viewport, setViewport] = useState({ start: 0, end: 1, scrollY: 0 });

<FlameGraph
  spans={spans}
  viewport={viewport}
  onViewportChange={setViewport}
/>

// Programmatic zoom:
<button onClick={() => setViewport({ start: 0.2, end: 0.5, scrollY: 0 })}>
  Zoom to 20%-50%
</button>
```

## Headless Rendering

Use `renderFlameGraph()` directly for custom Canvas integration:

```tsx
import { renderFlameGraph, DARK_THEME } from "@flame-cat/react";

const ctx = canvas.getContext("2d")!;
const result = renderFlameGraph(ctx, {
  spans,
  theme: DARK_THEME,
  viewport: { start: 0, end: 1, scrollY: 0 },
  width: 800,
  height: 400,
  dpr: window.devicePixelRatio,
});

// result.hitRegions can be used for custom interaction
```

## License

MIT
