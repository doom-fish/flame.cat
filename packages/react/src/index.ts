export { FlameGraph } from "./FlameGraph";
export type { FlameGraphProps } from "./FlameGraph";

export type {
  FlameSpan,
  FlameTheme,
  FlameViewport,
  HitRegion,
  SpanEvent,
} from "./types";
export { DARK_THEME, LIGHT_THEME } from "./types";

export {
  renderFlameGraph,
  hitTest,
  computeTimeRange,
  computeMaxDepth,
  computeContentHeight,
} from "./render";
export type { RenderOptions, RenderResult } from "./render";
