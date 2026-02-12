import { useRef, useCallback, useSyncExternalStore } from "react";

/** Methods exposed by the flame-cat WASM module. */
interface WasmExports {
  startOnCanvas(canvasId: string): void;
  loadProfile(data: Uint8Array): void;
  setTheme(mode: string): void;
  setSearch(query: string): void;
  resetZoom(): void;
}

/** Controller for a FlameGraph instance. Create with `useFlameGraph()`. */
export interface FlameGraphController {
  /** Load a profile from raw bytes (any supported format). */
  loadProfile(data: ArrayBuffer | Uint8Array): void;
  /** Set theme to "dark" or "light". */
  setTheme(mode: "dark" | "light"): void;
  /** Set search/filter query. Empty string clears. */
  setSearch(query: string): void;
  /** Reset zoom to fit all data. */
  resetZoom(): void;
  /** Whether the WASM module has initialized. */
  readonly ready: boolean;

  /** @internal — used by FlameGraph component. */
  _attach(wasm: WasmExports): void;
  /** @internal */
  _subscribe(cb: () => void): () => void;
  /** @internal */
  _getReady(): boolean;
}

/** Create a controller to pass to `<FlameGraph>`. */
export function useFlameGraph(): FlameGraphController {
  const ref = useRef<FlameGraphController | null>(null);

  if (!ref.current) {
    let wasm: WasmExports | null = null;
    let isReady = false;
    const listeners = new Set<() => void>();
    const pending: Array<(w: WasmExports) => void> = [];

    function notify() {
      listeners.forEach((l) => l());
    }

    function enqueue(fn: (w: WasmExports) => void) {
      if (wasm) {
        fn(wasm);
      } else {
        pending.push(fn);
      }
    }

    ref.current = {
      loadProfile(data: ArrayBuffer | Uint8Array) {
        const bytes = data instanceof Uint8Array ? data : new Uint8Array(data);
        enqueue((w) => w.loadProfile(bytes));
      },
      setTheme(mode: "dark" | "light") {
        enqueue((w) => w.setTheme(mode));
      },
      setSearch(query: string) {
        enqueue((w) => w.setSearch(query));
      },
      resetZoom() {
        enqueue((w) => w.resetZoom());
      },
      get ready() {
        return isReady;
      },

      _attach(w: WasmExports) {
        wasm = w;
        isReady = true;
        for (const fn of pending) fn(w);
        pending.length = 0;
        notify();
      },
      _subscribe(cb: () => void) {
        listeners.add(cb);
        return () => listeners.delete(cb);
      },
      _getReady() {
        return isReady;
      },
    };
  }

  // Re-render when ready state changes
  useSyncExternalStore(
    ref.current._subscribe,
    ref.current._getReady,
    ref.current._getReady,
  );

  return ref.current;
}
