/** Methods exposed by the flame-cat WASM module. */
export interface FlameCatWasm {
  startOnCanvas(canvasId: string): void;
  loadProfile(data: Uint8Array): void;
  setTheme(mode: "dark" | "light"): void;
  setSearch(query: string): void;
  resetZoom(): void;
}

/** Imperative handle returned by the FlameGraph ref. */
export interface FlameGraphHandle {
  /** Load a profile from raw bytes (any supported format). */
  loadProfile(data: ArrayBuffer | Uint8Array): void;
  /** Set theme to "dark" or "light". */
  setTheme(mode: "dark" | "light"): void;
  /** Set search/filter query. Empty string clears. */
  setSearch(query: string): void;
  /** Reset zoom to fit all data. */
  resetZoom(): void;
  /** Returns true once the WASM module has initialized. */
  isReady(): boolean;
}
