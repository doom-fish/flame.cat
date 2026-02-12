import { useSyncExternalStore, useCallback } from "react";
import { useFlameCatStore } from "./FlameCatProvider";
import type { ProfileInfo, LaneInfo, ViewportInfo, SelectedSpanInfo } from "./types";

// ── useFlameGraph ──────────────────────────────────────────────────────

export interface FlameGraphController {
  /** Load a profile from raw bytes (any supported format). */
  loadProfile(data: ArrayBuffer | Uint8Array): void;
  /** Whether the WASM viewer has initialized. */
  readonly ready: boolean;
}

/** Core controller hook — load profiles and check readiness. */
export function useFlameGraph(): FlameGraphController {
  const store = useFlameCatStore();

  const ready = useSyncExternalStore(
    store.subscribe,
    store.getReady,
    store.getReady,
  );

  const loadProfile = useCallback(
    (data: ArrayBuffer | Uint8Array) => {
      const bytes = data instanceof Uint8Array ? data : new Uint8Array(data);
      store.exec((w) => w.loadProfile(bytes));
    },
    [store],
  );

  return { loadProfile, ready };
}

// ── useProfile ─────────────────────────────────────────────────────────

/** Read-only profile metadata. Null when no profile is loaded. */
export function useProfile(): ProfileInfo | null {
  const store = useFlameCatStore();
  return useSyncExternalStore(
    store.subscribe,
    () => store.getSnapshot().profile,
    () => null,
  );
}

// ── useLanes ───────────────────────────────────────────────────────────

export interface LanesState {
  lanes: LaneInfo[];
  toggleVisibility(index: number): void;
}

/** Lane metadata with visibility control. */
export function useLanes(): LanesState {
  const store = useFlameCatStore();

  const lanes = useSyncExternalStore(
    store.subscribe,
    () => store.getSnapshot().lanes,
    () => [] as LaneInfo[],
  );

  const toggleVisibility = useCallback(
    (index: number) => {
      const current = store.getSnapshot().lanes[index];
      if (current) {
        store.exec((w) => w.setLaneVisibility(index, !current.visible));
      }
    },
    [store],
  );

  return { lanes, toggleVisibility };
}

// ── useViewport ────────────────────────────────────────────────────────

export interface ViewportState extends ViewportInfo {
  setViewport(start: number, end: number): void;
  resetZoom(): void;
}

/** Viewport/zoom state with control methods. */
export function useViewport(): ViewportState {
  const store = useFlameCatStore();

  const viewport = useSyncExternalStore(
    store.subscribe,
    () => store.getSnapshot().viewport,
    () => ({ start: 0, end: 1, scroll_y: 0 }),
  );

  const setViewport = useCallback(
    (start: number, end: number) => {
      store.exec((w) => w.setViewport(start, end));
    },
    [store],
  );

  const resetZoom = useCallback(() => {
    store.exec((w) => w.resetZoom());
  }, [store]);

  return { ...viewport, setViewport, resetZoom };
}

// ── useSearch ──────────────────────────────────────────────────────────

export interface SearchState {
  query: string;
  setQuery(query: string): void;
}

/** Search/filter state. */
export function useSearch(): SearchState {
  const store = useFlameCatStore();

  const query = useSyncExternalStore(
    store.subscribe,
    () => store.getSnapshot().search,
    () => "",
  );

  const setQuery = useCallback(
    (q: string) => {
      store.exec((w) => w.setSearch(q));
    },
    [store],
  );

  return { query, setQuery };
}

// ── useTheme ───────────────────────────────────────────────────────────

export interface ThemeState {
  mode: "dark" | "light";
  setMode(mode: "dark" | "light"): void;
}

/** Theme control. */
export function useTheme(): ThemeState {
  const store = useFlameCatStore();

  const mode = useSyncExternalStore(
    store.subscribe,
    () => store.getSnapshot().theme || "dark",
    () => "dark" as const,
  );

  const setMode = useCallback(
    (m: "dark" | "light") => {
      store.exec((w) => w.setTheme(m));
    },
    [store],
  );

  return { mode: mode as "dark" | "light", setMode };
}

// ── useSelectedSpan ────────────────────────────────────────────────────

export interface SelectionState {
  selected: SelectedSpanInfo | null;
  select(frameId: number): void;
  clear(): void;
}

/** Span selection state. */
export function useSelectedSpan(): SelectionState {
  const store = useFlameCatStore();

  const selected = useSyncExternalStore(
    store.subscribe,
    () => store.getSnapshot().selected,
    () => null,
  );

  const select = useCallback(
    (frameId: number) => {
      store.exec((w) => w.selectSpan(frameId));
    },
    [store],
  );

  const clear = useCallback(() => {
    store.exec((w) => w.selectSpan(undefined));
  }, [store]);

  return { selected, select, clear };
}
