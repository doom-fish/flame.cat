import type { StateSnapshot, WasmExports } from "./types";

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
      this.snapshot = JSON.parse(json) as StateSnapshot;
    } catch {
      // keep last snapshot on parse error
    }
    this.notify();
  }

  private notify(): void {
    for (const cb of this.listeners) cb();
  }
}
