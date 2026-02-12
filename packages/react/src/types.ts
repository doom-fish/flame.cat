/** State snapshot from the WASM viewer, deserialized from JSON. */
export interface StateSnapshot {
  profile: ProfileInfo | null;
  lanes: LaneInfo[];
  viewport: ViewportInfo;
  selected: SelectedSpanInfo | null;
  search: string;
  theme: "dark" | "light";
}

export interface ProfileInfo {
  name: string | null;
  format: string;
  duration_us: number;
  start_time: number;
  end_time: number;
  span_count: number;
  thread_count: number;
}

export interface LaneInfo {
  name: string;
  kind: string;
  height: number;
  visible: boolean;
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
  selectSpan(frameId: number | undefined): void;
  onStateChange(callback: () => void): void;
  getState(): string;
}
