import type { StateSnapshot, WasmExports } from "./types";

const EMPTY_SNAPSHOT: StateSnapshot = {
  profile: null,
  lanes: [],
  viewport: { start: 0, end: 1, scroll_y: 0 },
  selected: null,
  search: "",
  theme: "dark",
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
  private _ready = false;

  get ready(): boolean {
    return this._ready;
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
    return this._ready;
  };

  /** Called when WASM module is loaded and ready. */
  attach(wasm: WasmExports): void {
    this.wasm = wasm;
    this._ready = true;

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

  /** Queue a command; executes immediately if WASM is ready, otherwise defers. */
  exec(fn: (w: WasmExports) => void): void {
    if (this.wasm) {
      fn(this.wasm);
    } else {
      this.pending.push(fn);
    }
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
