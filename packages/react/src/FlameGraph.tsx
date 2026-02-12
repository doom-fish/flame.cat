import {
  useRef,
  useEffect,
  useCallback,
  useState,
  useMemo,
  type CSSProperties,
} from "react";
import type { FlameSpan, FlameTheme, FlameViewport, SpanEvent, HitRegion } from "./types";
import { DARK_THEME } from "./types";
import {
  renderFlameGraph,
  hitTest,
  computeTimeRange,
  computeContentHeight,
} from "./render";

export interface FlameGraphProps {
  /** Array of spans to render. */
  spans: FlameSpan[];
  /** Width in CSS pixels. Defaults to 100% of container. */
  width?: number;
  /** Height in CSS pixels. Defaults to auto-computed from span depth. */
  height?: number;
  /** Color theme. Defaults to dark theme. */
  theme?: FlameTheme;
  /** Search query to highlight matching spans. */
  search?: string;
  /** Called when a span is clicked. */
  onSpanClick?: (event: SpanEvent) => void;
  /** Called when a span is hovered. */
  onSpanHover?: (event: SpanEvent | null) => void;
  /** Called when the viewport changes (zoom/pan). */
  onViewportChange?: (viewport: FlameViewport) => void;
  /** Controlled viewport. If provided, component is controlled. */
  viewport?: FlameViewport;
  /** Minimum zoom level as fraction of total duration. Default: 0.0001 */
  minZoom?: number;
  /** Additional CSS class name. */
  className?: string;
  /** Additional inline styles. */
  style?: CSSProperties;
}

const DEFAULT_MIN_ZOOM = 0.0001;
const ZOOM_FACTOR = 0.001;
const SCROLL_PAN_FACTOR = 0.001;

/** Auto-assign IDs to spans that don't have them. */
function ensureIds(spans: FlameSpan[]): FlameSpan[] {
  let needsIds = false;
  for (const s of spans) {
    if (s.id === undefined) {
      needsIds = true;
      break;
    }
  }
  if (!needsIds) return spans;
  return spans.map((s, i) => (s.id === undefined ? { ...s, id: i } : s));
}

export function FlameGraph({
  spans: rawSpans,
  width: propWidth,
  height: propHeight,
  theme = DARK_THEME,
  search,
  onSpanClick,
  onSpanHover,
  onViewportChange,
  viewport: controlledViewport,
  minZoom = DEFAULT_MIN_ZOOM,
  className,
  style,
}: FlameGraphProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const hitRegionsRef = useRef<HitRegion[]>([]);
  const isDragging = useRef(false);
  const lastDragX = useRef(0);

  const spans = useMemo(() => ensureIds(rawSpans), [rawSpans]);

  const [containerWidth, setContainerWidth] = useState(0);
  const [internalViewport, setInternalViewport] = useState<FlameViewport>({
    start: 0,
    end: 1,
    scrollY: 0,
  });
  const [hoveredId, setHoveredId] = useState<number | undefined>();
  const [selectedId, setSelectedId] = useState<number | undefined>();

  const viewport = controlledViewport ?? internalViewport;

  const setViewport = useCallback(
    (vp: FlameViewport) => {
      if (!controlledViewport) {
        setInternalViewport(vp);
      }
      onViewportChange?.(vp);
    },
    [controlledViewport, onViewportChange],
  );

  const contentHeight = useMemo(() => computeContentHeight(spans), [spans]);
  const canvasWidth = propWidth ?? containerWidth;
  const canvasHeight = propHeight ?? Math.max(contentHeight + 20, 100);

  // Observe container width
  useEffect(() => {
    if (propWidth !== undefined) return;
    const container = containerRef.current;
    if (!container) return;
    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) {
        setContainerWidth(entry.contentRect.width);
      }
    });
    observer.observe(container);
    setContainerWidth(container.clientWidth);
    return () => observer.disconnect();
  }, [propWidth]);

  // Render
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || canvasWidth === 0) return;

    const dpr = window.devicePixelRatio || 1;
    canvas.width = canvasWidth * dpr;
    canvas.height = canvasHeight * dpr;
    canvas.style.width = `${canvasWidth}px`;
    canvas.style.height = `${canvasHeight}px`;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const result = renderFlameGraph(ctx, {
      spans,
      theme,
      viewport,
      width: canvasWidth,
      height: canvasHeight,
      dpr,
      search,
      selectedId,
      hoveredId,
    });

    hitRegionsRef.current = result.hitRegions;
  }, [
    spans,
    theme,
    viewport,
    canvasWidth,
    canvasHeight,
    search,
    selectedId,
    hoveredId,
  ]);

  // Mouse move (hover)
  const handleMouseMove = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      const canvas = canvasRef.current;
      if (!canvas) return;
      const rect = canvas.getBoundingClientRect();
      const px = e.clientX - rect.left;
      const py = e.clientY - rect.top;

      if (isDragging.current) {
        const dx = e.clientX - lastDragX.current;
        lastDragX.current = e.clientX;
        const span = viewport.end - viewport.start;
        const panAmount = (-dx / canvasWidth) * span;
        const newStart = Math.max(0, Math.min(1 - span, viewport.start + panAmount));
        setViewport({ ...viewport, start: newStart, end: newStart + span });
        return;
      }

      const hit = hitTest(hitRegionsRef.current, px, py);
      if (hit) {
        setHoveredId(hit.id);
        canvas.style.cursor = "pointer";
        onSpanHover?.({ span: hit, start: hit.start, end: hit.end });
      } else {
        setHoveredId(undefined);
        canvas.style.cursor = "grab";
        onSpanHover?.(null);
      }
    },
    [viewport, canvasWidth, setViewport, onSpanHover],
  );

  // Mouse down (start drag)
  const handleMouseDown = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      if (e.button === 0) {
        isDragging.current = true;
        lastDragX.current = e.clientX;
        if (canvasRef.current) canvasRef.current.style.cursor = "grabbing";
      }
    },
    [],
  );

  // Mouse up (end drag or click)
  const handleMouseUp = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      if (!isDragging.current) return;
      isDragging.current = false;
      if (canvasRef.current) canvasRef.current.style.cursor = "grab";

      // Detect click (minimal movement)
      const canvas = canvasRef.current;
      if (!canvas) return;
      const rect = canvas.getBoundingClientRect();
      const px = e.clientX - rect.left;
      const py = e.clientY - rect.top;
      const hit = hitTest(hitRegionsRef.current, px, py);
      if (hit) {
        setSelectedId(hit.id);
        onSpanClick?.({ span: hit, start: hit.start, end: hit.end });
      } else {
        setSelectedId(undefined);
      }
    },
    [onSpanClick],
  );

  // Mouse leave
  const handleMouseLeave = useCallback(() => {
    isDragging.current = false;
    setHoveredId(undefined);
    onSpanHover?.(null);
    if (canvasRef.current) canvasRef.current.style.cursor = "default";
  }, [onSpanHover]);

  // Wheel (zoom + vertical scroll)
  const handleWheel = useCallback(
    (e: React.WheelEvent<HTMLCanvasElement>) => {
      e.preventDefault();
      const canvas = canvasRef.current;
      if (!canvas) return;
      const rect = canvas.getBoundingClientRect();
      const px = e.clientX - rect.left;

      if (e.ctrlKey || e.metaKey) {
        // Zoom centered on cursor
        const fraction = px / canvasWidth;
        const span = viewport.end - viewport.start;
        const zoomDelta = e.deltaY * ZOOM_FACTOR * span;
        const newSpan = Math.max(minZoom, Math.min(1, span + zoomDelta));
        const anchor = viewport.start + fraction * span;
        const newStart = Math.max(0, anchor - fraction * newSpan);
        const newEnd = Math.min(1, newStart + newSpan);
        setViewport({
          ...viewport,
          start: Math.max(0, newEnd - newSpan),
          end: newEnd,
        });
      } else if (e.shiftKey) {
        // Horizontal pan
        const span = viewport.end - viewport.start;
        const panAmount = e.deltaY * SCROLL_PAN_FACTOR * span;
        const newStart = Math.max(0, Math.min(1 - span, viewport.start + panAmount));
        setViewport({ ...viewport, start: newStart, end: newStart + span });
      } else {
        // Vertical scroll
        const newScrollY = Math.max(0, viewport.scrollY + e.deltaY);
        setViewport({ ...viewport, scrollY: newScrollY });
      }
    },
    [viewport, canvasWidth, minZoom, setViewport],
  );

  // Double-click to zoom to span
  const handleDoubleClick = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      const canvas = canvasRef.current;
      if (!canvas) return;
      const rect = canvas.getBoundingClientRect();
      const px = e.clientX - rect.left;
      const py = e.clientY - rect.top;
      const hit = hitTest(hitRegionsRef.current, px, py);
      if (hit) {
        const timeRange = computeTimeRange(spans);
        const totalDuration = timeRange.end - timeRange.start;
        if (totalDuration <= 0) return;
        const spanStart = (hit.start - timeRange.start) / totalDuration;
        const spanEnd = (hit.end - timeRange.start) / totalDuration;
        const pad = (spanEnd - spanStart) * 0.15;
        setViewport({
          ...viewport,
          start: Math.max(0, spanStart - pad),
          end: Math.min(1, spanEnd + pad),
        });
      } else {
        // Reset zoom
        setViewport({ start: 0, end: 1, scrollY: 0 });
      }
    },
    [spans, viewport, setViewport],
  );

  return (
    <div
      ref={containerRef}
      className={className}
      style={{
        position: "relative",
        width: propWidth ?? "100%",
        ...style,
      }}
    >
      <canvas
        ref={canvasRef}
        onMouseMove={handleMouseMove}
        onMouseDown={handleMouseDown}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseLeave}
        onWheel={handleWheel}
        onDoubleClick={handleDoubleClick}
        style={{
          display: "block",
          width: canvasWidth,
          height: canvasHeight,
        }}
      />
    </div>
  );
}
