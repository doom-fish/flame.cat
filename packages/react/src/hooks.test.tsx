import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import React from "react";
import { renderHook, act } from "@testing-library/react";
import { FlameCatStore } from "./store";
import { FlameCatContext } from "./FlameCatProvider";
import {
  useFlameGraph,
  useStatus,
  useProfile,
  useViewType,
  useLanes,
  useViewport,
  useSearch,
  useTheme,
  useSelectedSpan,
  useHoveredSpan,
  useNavigation,
  useExport,
  useHotkeys,
} from "./hooks";
import type { WasmExports } from "./types";

function mockWasm(): WasmExports {
  let stateCallback: (() => void) | null = null;
  const state = {
    profile: {
      name: "test.json",
      format: "Chrome",
      duration_us: 5000,
      start_time: 0,
      end_time: 5000,
      span_count: 100,
      thread_count: 2,
    },
    lanes: [
      { name: "Main", kind: "thread", height: 200, visible: true, span_count: 80 },
      { name: "Worker", kind: "thread", height: 100, visible: true, span_count: 20 },
    ],
    viewport: { start: 0, end: 1, scroll_y: 0 },
    selected: null,
    search: "",
    theme: "dark",
    view_type: "time_order",
    can_go_back: false,
    can_go_forward: true,
  };

  return {
    startOnCanvas: vi.fn(),
    loadProfile: vi.fn(),
    setTheme: vi.fn((mode: string) => {
      state.theme = mode;
      stateCallback?.();
    }),
    setSearch: vi.fn((q: string) => {
      state.search = q;
      stateCallback?.();
    }),
    resetZoom: vi.fn(() => {
      state.viewport = { start: 0, end: 1, scroll_y: 0 };
      stateCallback?.();
    }),
    setViewport: vi.fn((start: number, end: number) => {
      state.viewport = { start, end, scroll_y: 0 };
      stateCallback?.();
    }),
    setLaneVisibility: vi.fn((i: number, v: boolean) => {
      if (state.lanes[i]) state.lanes[i].visible = v;
      stateCallback?.();
    }),
    setLaneHeight: vi.fn((i: number, h: number) => {
      if (state.lanes[i]) state.lanes[i].height = h;
      stateCallback?.();
    }),
    reorderLanes: vi.fn((from: number, to: number) => {
      if (from < state.lanes.length && to < state.lanes.length) {
        const [lane] = state.lanes.splice(from, 1);
        state.lanes.splice(to, 0, lane);
      }
      stateCallback?.();
    }),
    selectSpan: vi.fn((fid: number | undefined) => {
      state.selected = fid != null
        ? { name: "render", frame_id: fid, lane_index: 0, start_us: 100, end_us: 500 }
        : null;
      stateCallback?.();
    }),
    setViewType: vi.fn((vt: string) => {
      state.view_type = vt;
      stateCallback?.();
    }),
    navigateBack: vi.fn(() => {
      state.can_go_back = false;
      state.can_go_forward = true;
      stateCallback?.();
    }),
    navigateForward: vi.fn(() => {
      state.can_go_forward = false;
      stateCallback?.();
    }),
    exportProfile: vi.fn(() => '{"meta":{}}'),
    onStateChange: vi.fn((cb: () => void) => {
      stateCallback = cb;
    }),
    getState: vi.fn(() => JSON.stringify(state)),
  };
}

function createWrapper(store: FlameCatStore) {
  return function Wrapper({ children }: { children: React.ReactNode }) {
    return (
      <FlameCatContext.Provider value={store}>
        {children}
      </FlameCatContext.Provider>
    );
  };
}

describe("hooks integration", () => {
  let store: FlameCatStore;
  let wasm: WasmExports;

  beforeEach(() => {
    store = new FlameCatStore();
    wasm = mockWasm();
    store.attach(wasm);
  });

  // ── useFlameGraph ──────────────────────────────────────────────────

  it("useFlameGraph reports ready and loads profiles", () => {
    const { result } = renderHook(() => useFlameGraph(), {
      wrapper: createWrapper(store),
    });
    expect(result.current.ready).toBe(true);
    act(() => result.current.loadProfile(new Uint8Array([1, 2])));
    expect(wasm.loadProfile).toHaveBeenCalled();
  });

  // ── useStatus ──────────────────────────────────────────────────────

  it("useStatus reflects ready state", () => {
    const { result } = renderHook(() => useStatus(), {
      wrapper: createWrapper(store),
    });
    expect(result.current.status).toBe("ready");
    expect(result.current.error).toBeNull();
  });

  it("useStatus reflects error state", () => {
    const errStore = new FlameCatStore();
    errStore.fail("network error");
    const { result } = renderHook(() => useStatus(), {
      wrapper: createWrapper(errStore),
    });
    expect(result.current.status).toBe("error");
    expect(result.current.error).toBe("network error");
  });

  // ── useProfile ─────────────────────────────────────────────────────

  it("useProfile returns profile metadata", () => {
    const { result } = renderHook(() => useProfile(), {
      wrapper: createWrapper(store),
    });
    expect(result.current).toEqual(
      expect.objectContaining({ name: "test.json", span_count: 100 }),
    );
  });

  // ── useViewType ────────────────────────────────────────────────────

  it("useViewType reads and sets view type", () => {
    const { result } = renderHook(() => useViewType(), {
      wrapper: createWrapper(store),
    });
    expect(result.current.viewType).toBe("time_order");
    act(() => result.current.setViewType("left_heavy"));
    expect(wasm.setViewType).toHaveBeenCalledWith("left_heavy");
  });

  it("useViewType validates input", () => {
    const { result } = renderHook(() => useViewType(), {
      wrapper: createWrapper(store),
    });
    act(() => result.current.setViewType("invalid" as any));
    expect(wasm.setViewType).not.toHaveBeenCalled();
  });

  // ── useLanes ───────────────────────────────────────────────────────

  it("useLanes returns lane data with controls", () => {
    const { result } = renderHook(() => useLanes(), {
      wrapper: createWrapper(store),
    });
    expect(result.current.lanes).toHaveLength(2);
    expect(result.current.lanes[0].span_count).toBe(80);
  });

  it("useLanes.toggleVisibility works", () => {
    const { result } = renderHook(() => useLanes(), {
      wrapper: createWrapper(store),
    });
    act(() => result.current.toggleVisibility(0));
    expect(wasm.setLaneVisibility).toHaveBeenCalledWith(0, false);
  });

  it("useLanes.showAll/hideAll batch visibility", () => {
    const { result } = renderHook(() => useLanes(), {
      wrapper: createWrapper(store),
    });
    act(() => result.current.hideAll());
    expect(wasm.setLaneVisibility).toHaveBeenCalledWith(0, false);
    expect(wasm.setLaneVisibility).toHaveBeenCalledWith(1, false);
    act(() => result.current.showAll());
    expect(wasm.setLaneVisibility).toHaveBeenCalledWith(0, true);
    expect(wasm.setLaneVisibility).toHaveBeenCalledWith(1, true);
  });

  it("useLanes.setHeight validates bounds", () => {
    const { result } = renderHook(() => useLanes(), {
      wrapper: createWrapper(store),
    });
    // Out of bounds index → no call
    act(() => result.current.setHeight(-1, 100));
    expect(wasm.setLaneHeight).not.toHaveBeenCalled();
    // Clamps value
    act(() => result.current.setHeight(0, 5));
    expect(wasm.setLaneHeight).toHaveBeenCalledWith(0, 16);
    act(() => result.current.setHeight(0, 999));
    expect(wasm.setLaneHeight).toHaveBeenCalledWith(0, 600);
  });

  it("useLanes.reorder validates bounds", () => {
    const { result } = renderHook(() => useLanes(), {
      wrapper: createWrapper(store),
    });
    act(() => result.current.reorder(0, 5));
    expect(wasm.reorderLanes).not.toHaveBeenCalled();
    act(() => result.current.reorder(0, 0));
    expect(wasm.reorderLanes).not.toHaveBeenCalled();
    act(() => result.current.reorder(0, 1));
    expect(wasm.reorderLanes).toHaveBeenCalledWith(0, 1);
  });

  // ── useViewport ────────────────────────────────────────────────────

  it("useViewport clamps values", () => {
    const { result } = renderHook(() => useViewport(), {
      wrapper: createWrapper(store),
    });
    act(() => result.current.setViewport(-0.5, 1.5));
    expect(wasm.setViewport).toHaveBeenCalledWith(0, 1);
  });

  it("useViewport.resetZoom resets to full range", () => {
    const { result } = renderHook(() => useViewport(), {
      wrapper: createWrapper(store),
    });
    act(() => result.current.resetZoom());
    expect(wasm.resetZoom).toHaveBeenCalled();
  });

  // ── useSearch ──────────────────────────────────────────────────────

  it("useSearch reads and sets query", () => {
    const { result } = renderHook(() => useSearch(), {
      wrapper: createWrapper(store),
    });
    expect(result.current.query).toBe("");
    act(() => result.current.setQuery("render"));
    expect(wasm.setSearch).toHaveBeenCalledWith("render");
  });

  // ── useTheme ───────────────────────────────────────────────────────

  it("useTheme.toggle switches mode", () => {
    const { result } = renderHook(() => useTheme(), {
      wrapper: createWrapper(store),
    });
    expect(result.current.mode).toBe("dark");
    act(() => result.current.toggle());
    expect(wasm.setTheme).toHaveBeenCalledWith("light");
  });

  // ── useSelectedSpan ────────────────────────────────────────────────

  it("useSelectedSpan select and clear", () => {
    const { result } = renderHook(() => useSelectedSpan(), {
      wrapper: createWrapper(store),
    });
    expect(result.current.selected).toBeNull();
    act(() => result.current.select(7));
    expect(wasm.selectSpan).toHaveBeenCalledWith(7);
    act(() => result.current.clear());
    expect(wasm.selectSpan).toHaveBeenCalledWith(undefined);
  });

  // ── useHoveredSpan ─────────────────────────────────────────────────

  it("useHoveredSpan returns null initially", () => {
    const { result } = renderHook(() => useHoveredSpan(), {
      wrapper: createWrapper(store),
    });
    expect(result.current).toBeNull();
  });

  // ── useNavigation ──────────────────────────────────────────────────

  it("useNavigation exposes back/forward state and actions", () => {
    const { result } = renderHook(() => useNavigation(), {
      wrapper: createWrapper(store),
    });
    expect(result.current.canGoBack).toBe(false);
    expect(result.current.canGoForward).toBe(true);
    act(() => result.current.forward());
    expect(wasm.navigateForward).toHaveBeenCalled();
    act(() => result.current.back());
    expect(wasm.navigateBack).toHaveBeenCalled();
  });

  // ── useExport ──────────────────────────────────────────────────────

  it("useExport returns profile JSON", () => {
    const { result } = renderHook(() => useExport(), {
      wrapper: createWrapper(store),
    });
    const json = result.current.exportJSON();
    expect(json).toBe('{"meta":{}}');
    expect(wasm.exportProfile).toHaveBeenCalled();
  });

  // ── useHotkeys ─────────────────────────────────────────────────────

  it("useHotkeys responds to keyboard events", () => {
    renderHook(() => useHotkeys(), {
      wrapper: createWrapper(store),
    });
    act(() => {
      document.dispatchEvent(new KeyboardEvent("keydown", { key: "0" }));
    });
    expect(wasm.resetZoom).toHaveBeenCalled();
  });

  it("useHotkeys toggles theme on 't'", () => {
    renderHook(() => useHotkeys(), {
      wrapper: createWrapper(store),
    });
    act(() => {
      document.dispatchEvent(new KeyboardEvent("keydown", { key: "t" }));
    });
    expect(wasm.setTheme).toHaveBeenCalledWith("light");
  });

  it("useHotkeys clears selection on Escape", () => {
    renderHook(() => useHotkeys(), {
      wrapper: createWrapper(store),
    });
    act(() => {
      document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    });
    expect(wasm.selectSpan).toHaveBeenCalledWith(undefined);
  });

  it("useHotkeys can be disabled with false", () => {
    renderHook(() => useHotkeys(false), {
      wrapper: createWrapper(store),
    });
    act(() => {
      document.dispatchEvent(new KeyboardEvent("keydown", { key: "0" }));
    });
    expect(wasm.resetZoom).not.toHaveBeenCalled();
  });

  it("useHotkeys with custom key map", () => {
    renderHook(() => useHotkeys({ resetZoom: ["r"] }), {
      wrapper: createWrapper(store),
    });
    act(() => {
      document.dispatchEvent(new KeyboardEvent("keydown", { key: "r" }));
    });
    expect(wasm.resetZoom).toHaveBeenCalled();
  });

  // ── Error: hook without provider ───────────────────────────────────

  it("hooks throw without provider", () => {
    expect(() => {
      renderHook(() => useFlameGraph());
    }).toThrow("useFlameCatStore must be used within a <FlameCatProvider>");
  });
});
