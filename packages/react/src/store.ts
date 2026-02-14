import type { StateSnapshot, WasmExports, ProfileInfo, LaneInfo, ViewportInfo, SelectedSpanInfo } from "./types";

/** Status of the WASM viewer lifecycle. */
export type FlameCatStatus = "loading" | "ready" | "error";

const EMPTY_SNAPSHOT: StateSnapshot = {
  profile: null,
  lanes: [],
  viewport: { start: 0, end: 1, scroll_y: 0 },
  selected: null,
  hovered: null,
  search: "",
  theme: "dark",
  view_type: "time_order",
  color_mode: "by_name",
  can_go_back: false,
  can_go_forward: false,
};

/**
 * Reactive store that bridges WASM state to React via useSyncExternalStore.
 * Created once per FlameCatProvider.
 */
export class FlameCatStore {
  private wasm: WasmExports | null = null;
  private snapshot: StateSnapshot = EMPTY_SNAPSHOT;
  private listeners = new Set<() => void>();
  private pending: Array<(w: WasmExports) => void> = [];
  private _status: FlameCatStatus = "loading";
  private _error: string | null = null;

  get ready(): boolean {
    return this._status === "ready";
  }

  /** Subscribe to state changes (used by useSyncExternalStore). */
  subscribe = (cb: () => void): (() => void) => {
    this.listeners.add(cb);
    return () => this.listeners.delete(cb);
  };

  /** Get current snapshot (used by useSyncExternalStore). */
  getSnapshot = (): StateSnapshot => {
    return this.snapshot;
  };

  getReady = (): boolean => {
    return this._status === "ready";
  };

  getStatus = (): FlameCatStatus => {
    return this._status;
  };

  getError = (): string | null => {
    return this._error;
  };

  // Stable property getters — return same reference if snapshot hasn't changed.
  // This avoids unnecessary re-renders with useSyncExternalStore.
  getProfile = (): ProfileInfo | null => this.snapshot.profile;
  getLanes = (): LaneInfo[] => this.snapshot.lanes;
  getViewport = (): ViewportInfo => this.snapshot.viewport;
  getSelected = (): SelectedSpanInfo | null => this.snapshot.selected;
  getHovered = (): SelectedSpanInfo | null => this.snapshot.hovered ?? null;
  getSearch = (): string => this.snapshot.search;
  getTheme = (): string => this.snapshot.theme || "dark";
  getViewType = (): string => this.snapshot.view_type || "time_order";
  getCanGoBack = (): boolean => this.snapshot.can_go_back ?? false;
  getCanGoForward = (): boolean => this.snapshot.can_go_forward ?? false;
  getColorMode = (): string => this.snapshot.color_mode || "by_name";

  /** Called when WASM module is loaded and ready. */
  attach(wasm: WasmExports): void {
    this.wasm = wasm;
    this._status = "ready";
    this._error = null;

    // Register state change callback
    wasm.onStateChange(() => {
      this.refresh();
    });

    // Flush pending commands
    for (const fn of this.pending) fn(wasm);
    this.pending.length = 0;

    // Initial snapshot
    this.refresh();
  }

  /** Mark the store as failed with an error message. */
  fail(message: string): void {
    this._status = "error";
    this._error = message;
    this.notify();
  }

  /** Queue a command; executes immediately if WASM is ready, otherwise defers. */
  exec(fn: (w: WasmExports) => void): void {
    if (this.wasm) {
      fn(this.wasm);
    } else {
      this.pending.push(fn);
    }
  }

  /** Total number of lanes. Used for bounds checking. */
  get laneCount(): number {
    return this.snapshot.lanes.length;
  }

  private refresh(): void {
    if (!this.wasm) return;
    try {
      const json = this.wasm.getState();
      const next = JSON.parse(json) as StateSnapshot;
      // Structural sharing: reuse old references for unchanged subtrees
      // to preserve React referential equality and avoid unnecessary re-renders
      const prev = this.snapshot;
      if (prev.profile && next.profile
        && prev.profile.duration_us === next.profile.duration_us
        && prev.profile.span_count === next.profile.span_count) {
        next.profile = prev.profile;
      }
      if (prev.lanes.length === next.lanes.length
        && prev.lanes.every((l, i) =>
          l.name === next.lanes[i].name
          && l.visible === next.lanes[i].visible
          && l.height === next.lanes[i].height)) {
        next.lanes = prev.lanes;
      }
      this.snapshot = next;
    } catch (e) {
      // Keep last snapshot on parse error, but log for debugging
      if (typeof console !== "undefined") {
        console.warn("flame-cat: failed to parse WASM state", e);
      }
    }
    this.notify();
  }

  private notify(): void {
    for (const cb of this.listeners) cb();
  }
}
