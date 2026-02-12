import { useSyncExternalStore, useCallback, useEffect } from "react";
import { useFlameCatStore } from "./FlameCatProvider";
import type { FlameCatStatus } from "./store";
import type { ProfileInfo, LaneInfo, ViewportInfo, SelectedSpanInfo, ViewType } from "./types";

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

// ── useStatus ──────────────────────────────────────────────────────────

export interface StatusState {
  /** Current lifecycle status: "loading" | "ready" | "error". */
  status: FlameCatStatus;
  /** Error message if status is "error", null otherwise. */
  error: string | null;
}

/** Lifecycle status of the WASM viewer. */
export function useStatus(): StatusState {
  const store = useFlameCatStore();

  const status = useSyncExternalStore(
    store.subscribe,
    store.getStatus,
    store.getStatus,
  );

  const error = useSyncExternalStore(
    store.subscribe,
    store.getError,
    store.getError,
  );

  return { status, error };
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

// ── useViewType ────────────────────────────────────────────────────────

export interface ViewTypeState {
  /** Current view type. */
  viewType: ViewType;
  /** Switch visualization mode. */
  setViewType(viewType: ViewType): void;
}

const VALID_VIEW_TYPES: ViewType[] = ["time_order", "left_heavy", "sandwich", "ranked", "icicle"];

/** View switching: time-order, left-heavy, sandwich, ranked. */
export function useViewType(): ViewTypeState {
  const store = useFlameCatStore();

  const viewType = useSyncExternalStore(
    store.subscribe,
    () => store.getSnapshot().view_type || "time_order",
    () => "time_order" as ViewType,
  );

  const setViewType = useCallback(
    (vt: ViewType) => {
      if (!VALID_VIEW_TYPES.includes(vt)) return;
      store.exec((w) => w.setViewType(vt));
    },
    [store],
  );

  return { viewType: viewType as ViewType, setViewType };
}

// ── useLanes ───────────────────────────────────────────────────────────

export interface LanesState {
  lanes: LaneInfo[];
  /** Toggle a lane's visibility. */
  toggleVisibility(index: number): void;
  /** Set a lane's visibility explicitly. */
  setVisibility(index: number, visible: boolean): void;
  /** Set a lane's height in pixels (clamped 16–600 by Rust). */
  setHeight(index: number, height: number): void;
  /** Move a lane from one position to another. */
  reorder(fromIndex: number, toIndex: number): void;
  /** Show all lanes. */
  showAll(): void;
  /** Hide all lanes. */
  hideAll(): void;
}

/** Lane metadata with visibility, height, and ordering control. */
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

  const setVisibility = useCallback(
    (index: number, visible: boolean) => {
      if (index < 0 || index >= store.laneCount) return;
      store.exec((w) => w.setLaneVisibility(index, visible));
    },
    [store],
  );

  const setHeight = useCallback(
    (index: number, height: number) => {
      if (index < 0 || index >= store.laneCount) return;
      store.exec((w) => w.setLaneHeight(index, Math.max(16, Math.min(600, height))));
    },
    [store],
  );

  const reorder = useCallback(
    (fromIndex: number, toIndex: number) => {
      const count = store.laneCount;
      if (fromIndex < 0 || fromIndex >= count || toIndex < 0 || toIndex >= count) return;
      if (fromIndex === toIndex) return;
      store.exec((w) => w.reorderLanes(fromIndex, toIndex));
    },
    [store],
  );

  const showAll = useCallback(() => {
    store.exec((w) => {
      for (let i = 0; i < store.laneCount; i++) {
        w.setLaneVisibility(i, true);
      }
    });
  }, [store]);

  const hideAll = useCallback(() => {
    store.exec((w) => {
      for (let i = 0; i < store.laneCount; i++) {
        w.setLaneVisibility(i, false);
      }
    });
  }, [store]);

  return { lanes, toggleVisibility, setVisibility, setHeight, reorder, showAll, hideAll };
}

// ── useViewport ────────────────────────────────────────────────────────

export interface ViewportState extends ViewportInfo {
  /** Set viewport range (0–1 fractional). Values are clamped. */
  setViewport(start: number, end: number): void;
  /** Reset zoom to show all data. */
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
      const s = Math.max(0, Math.min(1, start));
      const e = Math.max(s, Math.min(1, end));
      store.exec((w) => w.setViewport(s, e));
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
  /** Toggle between dark and light. */
  toggle(): void;
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

  const toggle = useCallback(() => {
    const current = store.getSnapshot().theme || "dark";
    store.exec((w) => w.setTheme(current === "dark" ? "light" : "dark"));
  }, [store]);

  return { mode: mode as "dark" | "light", setMode, toggle };
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

// ── useSpanNavigation ──────────────────────────────────────────────────

export interface SpanNavigationState {
  /** Navigate to the parent of the currently selected span. */
  goToParent(): void;
  /** Navigate to the first child of the currently selected span. */
  goToChild(): void;
  /** Navigate to the next sibling of the currently selected span. */
  goToNextSibling(): void;
  /** Navigate to the previous sibling of the currently selected span. */
  goToPrevSibling(): void;
  /** Jump to the next search result. */
  nextSearchResult(): void;
  /** Jump to the previous search result. */
  prevSearchResult(): void;
}

/** Span hierarchy and search result navigation. */
export function useSpanNavigation(): SpanNavigationState {
  const store = useFlameCatStore();

  const goToParent = useCallback(() => {
    store.exec((w) => w.navigateToParent());
  }, [store]);

  const goToChild = useCallback(() => {
    store.exec((w) => w.navigateToChild());
  }, [store]);

  const goToNextSibling = useCallback(() => {
    store.exec((w) => w.navigateToNextSibling());
  }, [store]);

  const goToPrevSibling = useCallback(() => {
    store.exec((w) => w.navigateToPrevSibling());
  }, [store]);

  const nextSearchResult = useCallback(() => {
    store.exec((w) => w.nextSearchResult());
  }, [store]);

  const prevSearchResult = useCallback(() => {
    store.exec((w) => w.prevSearchResult());
  }, [store]);

  return { goToParent, goToChild, goToNextSibling, goToPrevSibling, nextSearchResult, prevSearchResult };
}

// ── useHoveredSpan ─────────────────────────────────────────────────────

/** Currently hovered span (updates in real-time as user moves mouse). */
export function useHoveredSpan(): SelectedSpanInfo | null {
  const store = useFlameCatStore();
  return useSyncExternalStore(
    store.subscribe,
    () => store.getSnapshot().hovered ?? null,
    () => null,
  );
}

// ── useNavigation ──────────────────────────────────────────────────────

export interface NavigationState {
  /** Whether there is a previous zoom level to go back to. */
  canGoBack: boolean;
  /** Whether there is a next zoom level to go forward to. */
  canGoForward: boolean;
  /** Navigate to the previous zoom level. */
  back(): void;
  /** Navigate to the next zoom level. */
  forward(): void;
}

/** Breadcrumb zoom history navigation. */
export function useNavigation(): NavigationState {
  const store = useFlameCatStore();

  const canGoBack = useSyncExternalStore(
    store.subscribe,
    () => store.getSnapshot().can_go_back ?? false,
    () => false,
  );

  const canGoForward = useSyncExternalStore(
    store.subscribe,
    () => store.getSnapshot().can_go_forward ?? false,
    () => false,
  );

  const back = useCallback(() => {
    store.exec((w) => w.navigateBack());
  }, [store]);

  const forward = useCallback(() => {
    store.exec((w) => w.navigateForward());
  }, [store]);

  return { canGoBack, canGoForward, back, forward };
}

// ── useExport ──────────────────────────────────────────────────────────

export interface ExportState {
  /** Export the loaded profile as a JSON string (VisualProfile format). */
  exportJSON(): string | null;
  /** Export the current view as an SVG string. */
  exportSVG(width?: number, height?: number): string | null;
}

/** Profile export (JSON and SVG). */
export function useExport(): ExportState {
  const store = useFlameCatStore();

  const exportJSON = useCallback((): string | null => {
    let result: string | null = null;
    store.exec((w) => {
      result = w.exportProfile() ?? null;
    });
    return result;
  }, [store]);

  const exportSVG = useCallback(
    (width = 1200, height = 600): string | null => {
      let result: string | null = null;
      store.exec((w) => {
        result = w.exportSVG(width, height) ?? null;
      });
      return result;
    },
    [store],
  );

  return { exportJSON, exportSVG };
}

// ── useHotkeys ─────────────────────────────────────────────────────────

export interface HotkeyMap {
  /** Reset zoom (default: "0" or "Home"). */
  resetZoom?: string[];
  /** Toggle theme (default: "t"). */
  toggleTheme?: string[];
  /** Focus search (default: "/" or "f"). */
  focusSearch?: string[];
  /** Clear selection (default: "Escape"). */
  clearSelection?: string[];
}

const DEFAULT_HOTKEYS: Required<HotkeyMap> = {
  resetZoom: ["0", "Home"],
  toggleTheme: ["t"],
  focusSearch: ["/", "f"],
  clearSelection: ["Escape"],
};

/**
 * Keyboard shortcuts for common viewer actions.
 * Listens on `document` and dispatches to the appropriate hook action.
 *
 * @param hotkeys - Override default key bindings. Pass `false` to disable.
 * @param searchInputRef - Ref to search input to focus on search hotkey.
 */
export function useHotkeys(
  hotkeys: HotkeyMap | false = {},
  searchInputRef?: React.RefObject<HTMLInputElement | null>,
): void {
  const store = useFlameCatStore();

  useEffect(() => {
    if (hotkeys === false) return;

    const map = { ...DEFAULT_HOTKEYS, ...hotkeys };

    function handler(e: KeyboardEvent) {
      // Don't intercept when typing in an input/textarea
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") {
        // Only handle Escape inside inputs
        if (e.key !== "Escape") return;
      }

      if (map.resetZoom.includes(e.key)) {
        e.preventDefault();
        store.exec((w) => w.resetZoom());
      } else if (map.toggleTheme.includes(e.key)) {
        e.preventDefault();
        const current = store.getSnapshot().theme || "dark";
        store.exec((w) => w.setTheme(current === "dark" ? "light" : "dark"));
      } else if (map.focusSearch.includes(e.key)) {
        e.preventDefault();
        searchInputRef?.current?.focus();
      } else if (map.clearSelection.includes(e.key)) {
        e.preventDefault();
        store.exec((w) => w.selectSpan(undefined));
      }
    }

    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [store, hotkeys, searchInputRef]);
}
