/** State snapshot from the WASM viewer, deserialized from JSON. */
export interface StateSnapshot {
  profile: ProfileInfo | null;
  lanes: LaneInfo[];
  viewport: ViewportInfo;
  selected: SelectedSpanInfo | null;
  hovered: SelectedSpanInfo | null;
  search: string;
  theme: "dark" | "light";
  view_type: ViewType;
  color_mode: "by_name" | "by_depth";
  can_go_back: boolean;
  can_go_forward: boolean;
}

/** Visualization mode. */
export type ViewType = "time_order" | "left_heavy" | "sandwich" | "ranked" | "icicle";

export interface ProfileInfo {
  name: string | null;
  format: string;
  duration_us: number;
  start_time: number;
  end_time: number;
  span_count: number;
  thread_count: number;
}

export type LaneKind = "thread" | "counter" | "async" | "markers" | "cpu_samples" | "frame_track" | "object_track";

export interface LaneInfo {
  name: string;
  kind: LaneKind;
  height: number;
  visible: boolean;
  span_count: number;
}

export interface ViewportInfo {
  start: number;
  end: number;
  scroll_y: number;
}

export interface SelectedSpanInfo {
  name: string;
  frame_id: number;
  lane_index: number;
  start_us: number;
  end_us: number;
}

/** Methods exported by the flame-cat WASM module. */
export interface WasmExports {
  /** Initialize the egui viewer on a canvas element. Call once after WASM loads. */
  startOnCanvas(canvasId: string): void;
  /** Load a profiling file (any supported format). Accepts raw file bytes. */
  loadProfile(data: Uint8Array): void;
  /** Set the color theme. Accepts `"dark"` or `"light"`. */
  setTheme(mode: string): void;
  /** Set the search query. Matching spans are highlighted; non-matches are dimmed. */
  setSearch(query: string): void;
  /** Reset the viewport to show the full time range. */
  resetZoom(): void;
  /** Set the visible viewport range (0–1 fractional). Values are clamped. */
  setViewport(start: number, end: number): void;
  /** Toggle visibility of a lane by index. */
  setLaneVisibility(index: number, visible: boolean): void;
  /** Set the pixel height of a lane (clamped to 16–600). */
  setLaneHeight(index: number, height: number): void;
  /** Swap two lanes by index. Both indices must be valid. */
  reorderLanes(fromIndex: number, toIndex: number): void;
  /** Select a span by frame ID, or pass `undefined` to clear selection. */
  selectSpan(frameId: number | undefined): void;
  /** Set the visualization mode: `"time_order"`, `"left_heavy"`, `"icicle"`, `"sandwich"`, or `"ranked"`. */
  setViewType(viewType: string): void;
  /** Navigate to the previous zoom level in history. */
  navigateBack(): void;
  /** Navigate to the next zoom level in history. */
  navigateForward(): void;
  /** Set the span coloring strategy: `"by_name"` or `"by_depth"`. */
  setColorMode(mode: string): void;
  /** Select the parent of the currently selected span. */
  navigateToParent(): void;
  /** Select the first child of the currently selected span. */
  navigateToChild(): void;
  /** Select the next sibling of the currently selected span. */
  navigateToNextSibling(): void;
  /** Select the previous sibling of the currently selected span. */
  navigateToPrevSibling(): void;
  /** Jump to the next span matching the current search query. */
  nextSearchResult(): void;
  /** Jump to the previous span matching the current search query. */
  prevSearchResult(): void;
  /** Export the loaded profile as a JSON string, or `undefined` if no profile is loaded. */
  exportProfile(): string | undefined;
  /** Render the current view as an SVG string at the given dimensions. */
  exportSVG(width: number, height: number): string | undefined;
  /** Register a callback invoked whenever the viewer state changes. */
  onStateChange(callback: () => void): void;
  /** Get the full viewer state as a JSON string (used by the store for snapshots). */
  getState(): string;
}
