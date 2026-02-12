import { describe, it, expect, vi, beforeEach } from "vitest";
import { FlameCatStore } from "./store";
import type { WasmExports } from "./types";

function mockWasm(): WasmExports {
  let stateCallback: (() => void) | null = null;
  const state = {
    profile: null,
    lanes: [
      { name: "Main", kind: "thread", height: 200, visible: true, span_count: 42 },
      { name: "Worker", kind: "thread", height: 100, visible: true, span_count: 10 },
      { name: "Async", kind: "async", height: 60, visible: false, span_count: 5 },
    ],
    viewport: { start: 0, end: 1, scroll_y: 0 },
    selected: null,
    search: "",
    theme: "dark",
    view_type: "time_order",
    color_mode: "by_name",
    can_go_back: false,
    can_go_forward: false,
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
    setViewType: vi.fn((vt: string) => {
    setColorMode: vi.fn((mode: string) => {
      state.color_mode = mode;
      stateCallback?.();
    }),
      state.view_type = vt;
      stateCallback?.();
    }),
    navigateBack: vi.fn(() => {
      state.can_go_forward = true;
      stateCallback?.();
    }),
    navigateForward: vi.fn(() => {
      state.can_go_back = true;
      stateCallback?.();
    }),
    navigateToParent: vi.fn(),
    navigateToChild: vi.fn(),
    navigateToNextSibling: vi.fn(),
    navigateToPrevSibling: vi.fn(),
    nextSearchResult: vi.fn(),
    prevSearchResult: vi.fn(),
    exportProfile: vi.fn(() => '{"meta":{}}'),
    exportSVG: vi.fn(() => '<svg></svg>'),
    selectSpan: vi.fn((fid: number | undefined) => {
      state.selected = fid != null
        ? { name: "test", frame_id: fid, lane_index: 0, start_us: 0, end_us: 100 }
        : null;
      stateCallback?.();
    }),
    onStateChange: vi.fn((cb: () => void) => {
      stateCallback = cb;
    }),
    getState: vi.fn(() => JSON.stringify(state)),
  };
}

describe("FlameCatStore", () => {
  let store: FlameCatStore;

  beforeEach(() => {
    store = new FlameCatStore();
  });

  // ── Lifecycle ──────────────────────────────────────────────────────

  it("starts in loading status", () => {
    expect(store.ready).toBe(false);
    expect(store.getStatus()).toBe("loading");
    expect(store.getError()).toBeNull();
  });

  it("becomes ready after attach", () => {
    store.attach(mockWasm());
    expect(store.ready).toBe(true);
    expect(store.getStatus()).toBe("ready");
    expect(store.getError()).toBeNull();
  });

  it("transitions to error on fail()", () => {
    store.fail("WASM load failed");
    expect(store.ready).toBe(false);
    expect(store.getStatus()).toBe("error");
    expect(store.getError()).toBe("WASM load failed");
  });

  it("notifies subscribers on fail()", () => {
    const listener = vi.fn();
    store.subscribe(listener);
    store.fail("boom");
    expect(listener).toHaveBeenCalled();
  });

  // ── Command queuing ────────────────────────────────────────────────

  it("flushes pending commands on attach", () => {
    const wasm = mockWasm();
    store.exec((w) => w.loadProfile(new Uint8Array([1, 2, 3])));
    expect(wasm.loadProfile).not.toHaveBeenCalled();
    store.attach(wasm);
    expect(wasm.loadProfile).toHaveBeenCalledWith(new Uint8Array([1, 2, 3]));
  });

  it("exec runs immediately when ready", () => {
    const wasm = mockWasm();
    store.attach(wasm);
    store.exec((w) => w.setSearch("hello"));
    expect(wasm.setSearch).toHaveBeenCalledWith("hello");
  });

  // ── Snapshot ───────────────────────────────────────────────────────

  it("reads initial snapshot on attach", () => {
    store.attach(mockWasm());
    expect(store.getSnapshot().lanes).toHaveLength(3);
    expect(store.getSnapshot().lanes[0].name).toBe("Main");
    expect(store.getSnapshot().lanes[0].span_count).toBe(42);
  });

  it("registers onStateChange callback", () => {
    const wasm = mockWasm();
    store.attach(wasm);
    expect(wasm.onStateChange).toHaveBeenCalledTimes(1);
  });

  // ── Subscriptions ──────────────────────────────────────────────────

  it("notifies subscribers on state change", () => {
    const wasm = mockWasm();
    const listener = vi.fn();
    store.subscribe(listener);
    store.attach(wasm);
    const callsAfterAttach = listener.mock.calls.length;
    wasm.setTheme("light");
    expect(listener.mock.calls.length).toBeGreaterThan(callsAfterAttach);
    expect(store.getSnapshot().theme).toBe("light");
  });

  it("unsubscribe stops notifications", () => {
    const wasm = mockWasm();
    const listener = vi.fn();
    const unsub = store.subscribe(listener);
    store.attach(wasm);
    unsub();
    const count = listener.mock.calls.length;
    wasm.setSearch("test");
    expect(listener.mock.calls.length).toBe(count);
  });

  // ── Viewport ───────────────────────────────────────────────────────

  it("handles setViewport", () => {
    const wasm = mockWasm();
    store.attach(wasm);
    store.exec((w) => w.setViewport(0.2, 0.8));
    expect(store.getSnapshot().viewport.start).toBe(0.2);
    expect(store.getSnapshot().viewport.end).toBe(0.8);
  });

  // ── View Type ──────────────────────────────────────────────────────

  it("handles setViewType", () => {
    const wasm = mockWasm();
    store.attach(wasm);
    store.exec((w) => w.setViewType("left_heavy"));
    expect(store.getSnapshot().view_type).toBe("left_heavy");
  });

  // ── Lanes ──────────────────────────────────────────────────────────

  it("handles lane visibility toggle", () => {
    const wasm = mockWasm();
    store.attach(wasm);
    store.exec((w) => w.setLaneVisibility(0, false));
    expect(store.getSnapshot().lanes[0].visible).toBe(false);
  });

  it("handles setLaneHeight", () => {
    const wasm = mockWasm();
    store.attach(wasm);
    store.exec((w) => w.setLaneHeight(0, 300));
    expect(store.getSnapshot().lanes[0].height).toBe(300);
  });

  it("handles reorderLanes", () => {
    const wasm = mockWasm();
    store.attach(wasm);
    store.exec((w) => w.reorderLanes(0, 2));
    expect(store.getSnapshot().lanes[0].name).toBe("Worker");
    expect(store.getSnapshot().lanes[2].name).toBe("Main");
  });

  it("exposes laneCount for bounds checking", () => {
    store.attach(mockWasm());
    expect(store.laneCount).toBe(3);
  });

  // ── Selection ──────────────────────────────────────────────────────

  it("handles span selection", () => {
    const wasm = mockWasm();
    store.attach(wasm);
    store.exec((w) => w.selectSpan(42));
    expect(store.getSnapshot().selected).toEqual(
      expect.objectContaining({ frame_id: 42 }),
    );
  });

  it("handles span deselection", () => {
    const wasm = mockWasm();
    store.attach(wasm);
    store.exec((w) => w.selectSpan(42));
    store.exec((w) => w.selectSpan(undefined));
    expect(store.getSnapshot().selected).toBeNull();
  });
});
