import type { FlameSpan, FlameTheme, FlameViewport, HitRegion } from "./types";

const FRAME_HEIGHT = 20;
const FRAME_GAP = 1;
const MIN_WIDTH_PX = 0.5;
const TEXT_PADDING = 3;
const TEXT_MIN_WIDTH = 20;

export interface RenderOptions {
  spans: FlameSpan[];
  theme: FlameTheme;
  viewport: FlameViewport;
  width: number;
  height: number;
  dpr: number;
  /** Optional search query — non-matching spans are dimmed. */
  search?: string;
  /** Currently selected span ID. */
  selectedId?: number;
  /** Currently hovered span ID. */
  hoveredId?: number;
}

export interface RenderResult {
  hitRegions: HitRegion[];
}

/** Compute the total time range from spans. */
export function computeTimeRange(spans: FlameSpan[]): {
  start: number;
  end: number;
} {
  if (spans.length === 0) return { start: 0, end: 1 };
  let min = Infinity;
  let max = -Infinity;
  for (const s of spans) {
    if (s.start < min) min = s.start;
    if (s.end > max) max = s.end;
  }
  return { start: min, end: max };
}

/** Compute the maximum stack depth from spans. */
export function computeMaxDepth(spans: FlameSpan[]): number {
  let max = 0;
  for (const s of spans) {
    if (s.depth > max) max = s.depth;
  }
  return max;
}

/** Compute the natural height needed to display all spans. */
export function computeContentHeight(spans: FlameSpan[]): number {
  return (computeMaxDepth(spans) + 1) * FRAME_HEIGHT;
}

/** Render flame graph spans to a Canvas2D context. Returns hit regions. */
export function renderFlameGraph(
  ctx: CanvasRenderingContext2D,
  options: RenderOptions,
): RenderResult {
  const { spans, theme, viewport, width, height, dpr, search, selectedId, hoveredId } = options;

  ctx.save();
  ctx.scale(dpr, dpr);

  // Clear
  ctx.fillStyle = theme.background;
  ctx.fillRect(0, 0, width, height);

  const timeRange = computeTimeRange(spans);
  const totalDuration = timeRange.end - timeRange.start;
  if (totalDuration <= 0) {
    ctx.restore();
    return { hitRegions: [] };
  }

  const viewStart = timeRange.start + viewport.start * totalDuration;
  const viewEnd = timeRange.start + viewport.end * totalDuration;
  const viewDuration = viewEnd - viewStart;
  if (viewDuration <= 0) {
    ctx.restore();
    return { hitRegions: [] };
  }

  const xScale = width / viewDuration;
  const hitRegions: HitRegion[] = [];
  const searchLower = search?.toLowerCase();

  for (const span of spans) {
    const x = (span.start - viewStart) * xScale;
    const w = (span.end - span.start) * xScale;
    const y = span.depth * FRAME_HEIGHT - viewport.scrollY;
    const h = FRAME_HEIGHT - FRAME_GAP;

    // Cull off-screen
    if (x + w < 0 || x > width) continue;
    if (y + h < 0 || y > height) continue;

    // Skip sub-pixel
    if (w < MIN_WIDTH_PX) continue;

    // Color by depth
    const paletteIdx = span.depth % theme.flamePalette.length;
    let fillColor = theme.flamePalette[paletteIdx];

    // Search dimming
    if (searchLower && !span.name.toLowerCase().includes(searchLower)) {
      ctx.globalAlpha = theme.dimmedAlpha;
    } else {
      ctx.globalAlpha = 1.0;
    }

    // Fill
    ctx.fillStyle = fillColor;
    ctx.fillRect(x, y, w, h);

    // Border
    ctx.globalAlpha = 1.0;
    ctx.strokeStyle = theme.borderColor;
    ctx.lineWidth = 1;
    ctx.strokeRect(x + 0.5, y + 0.5, w - 1, h - 1);

    // Hover highlight
    if (hoveredId !== undefined && span.id === hoveredId) {
      ctx.fillStyle = theme.hoverColor;
      ctx.fillRect(x, y, w, h);
    }

    // Selected highlight
    if (selectedId !== undefined && span.id === selectedId) {
      ctx.strokeStyle = theme.selectedColor;
      ctx.lineWidth = 2;
      ctx.strokeRect(x + 1, y + 1, w - 2, h - 2);
    }

    // Label text
    if (w > TEXT_MIN_WIDTH && h > 10) {
      const fontSize = Math.min(Math.max(h - 6, 8), 11);
      ctx.font = `${fontSize}px -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif`;
      ctx.fillStyle = theme.textColor;
      ctx.textBaseline = "middle";

      const maxTextWidth = w - TEXT_PADDING * 2;
      const textY = y + h / 2;

      // Measure and truncate
      const measured = ctx.measureText(span.name);
      if (measured.width <= maxTextWidth) {
        ctx.fillText(span.name, x + TEXT_PADDING, textY);
      } else {
        // Truncate with ellipsis
        let truncated = span.name;
        while (truncated.length > 1) {
          truncated = truncated.slice(0, -1);
          if (ctx.measureText(truncated + "…").width <= maxTextWidth) {
            ctx.fillText(truncated + "…", x + TEXT_PADDING, textY);
            break;
          }
        }
      }
    }

    // Hit region
    const spanId = span.id ?? 0;
    hitRegions.push({ x, y, w, h, span: { ...span, id: spanId } });
  }

  ctx.restore();
  return { hitRegions };
}

/** Find the span at a given pixel position. */
export function hitTest(
  hitRegions: HitRegion[],
  px: number,
  py: number,
): FlameSpan | null {
  // Search in reverse so top-most (last drawn) wins
  for (let i = hitRegions.length - 1; i >= 0; i--) {
    const r = hitRegions[i];
    if (px >= r.x && px <= r.x + r.w && py >= r.y && py <= r.y + r.h) {
      return r.span;
    }
  }
  return null;
}
