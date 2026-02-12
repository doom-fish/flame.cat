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
  startOnCanvas(canvasId: string): void;
  loadProfile(data: Uint8Array): void;
  setTheme(mode: string): void;
  setSearch(query: string): void;
  resetZoom(): void;
  setViewport(start: number, end: number): void;
  setLaneVisibility(index: number, visible: boolean): void;
  setLaneHeight(index: number, height: number): void;
  reorderLanes(fromIndex: number, toIndex: number): void;
  selectSpan(frameId: number | undefined): void;
  setViewType(viewType: string): void;
  navigateBack(): void;
  navigateForward(): void;
  navigateToParent(): void;
  navigateToChild(): void;
  navigateToNextSibling(): void;
  navigateToPrevSibling(): void;
  nextSearchResult(): void;
  prevSearchResult(): void;
  exportProfile(): string | undefined;
  exportSVG(width: number, height: number): string | undefined;
  onStateChange(callback: () => void): void;
  getState(): string;
}
