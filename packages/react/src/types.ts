/** A single span in the flame graph. */
export interface FlameSpan {
  /** Unique identifier. Auto-assigned if omitted. */
  id?: number;
  /** Display name shown on the bar. */
  name: string;
  /** Start value (typically microseconds, but unit-agnostic). */
  start: number;
  /** End value. Must be >= start. */
  end: number;
  /** Stack depth (0 = root). */
  depth: number;
  /** Optional category for color grouping. */
  category?: string;
  /** Self-time for tooltip display. */
  selfTime?: number;
}

/** Color theme for the flame graph. */
export interface FlameTheme {
  /** Background color of the canvas. */
  background: string;
  /** Colors cycled by stack depth. */
  flamePalette: string[];
  /** Text color inside bars. */
  textColor: string;
  /** Border color around bars. */
  borderColor: string;
  /** Color for hovered bar highlight. */
  hoverColor: string;
  /** Color for selected bar highlight. */
  selectedColor: string;
  /** Color for search-dimmed bars. */
  dimmedAlpha: number;
  /** Tooltip background. */
  tooltipBackground: string;
  /** Tooltip text color. */
  tooltipText: string;
  /** Tooltip border. */
  tooltipBorder: string;
}

export const DARK_THEME: FlameTheme = {
  background: "#1a1a2e",
  flamePalette: [
    "#e85d04",
    "#dc2f02",
    "#d00000",
    "#9d0208",
    "#f48c06",
    "#faa307",
  ],
  textColor: "#e0e0e0",
  borderColor: "#333344",
  hoverColor: "rgba(255, 255, 255, 0.15)",
  selectedColor: "#3b82f6",
  dimmedAlpha: 0.25,
  tooltipBackground: "#2a2a3e",
  tooltipText: "#e0e0e0",
  tooltipBorder: "#444466",
};

export const LIGHT_THEME: FlameTheme = {
  background: "#ffffff",
  flamePalette: [
    "#fb923c",
    "#f97316",
    "#ea580c",
    "#c2410c",
    "#fdba74",
    "#fed7aa",
  ],
  textColor: "#1a1a1a",
  borderColor: "#d1d5db",
  hoverColor: "rgba(0, 0, 0, 0.08)",
  selectedColor: "#2563eb",
  dimmedAlpha: 0.2,
  tooltipBackground: "#ffffff",
  tooltipText: "#1a1a1a",
  tooltipBorder: "#d1d5db",
};

/** Hit region for click/hover detection. */
export interface HitRegion {
  x: number;
  y: number;
  w: number;
  h: number;
  span: FlameSpan;
}

/** Viewport state for the flame graph. */
export interface FlameViewport {
  /** Start of visible window as fraction [0, 1] of total duration. */
  start: number;
  /** End of visible window as fraction [0, 1] of total duration. */
  end: number;
  /** Vertical scroll offset in pixels. */
  scrollY: number;
}

/** Information about a span event passed to callbacks. */
export interface SpanEvent {
  span: FlameSpan;
  /** Absolute time start of the span. */
  start: number;
  /** Absolute time end of the span. */
  end: number;
}
