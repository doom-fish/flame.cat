import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { FlameCatStore } from "./store";
import type { WasmExports } from "./types";

function mockWasm(): WasmExports {
  let stateCallback: (() => void) | null = null;
  const state = {
    profile: null,
    lanes: [
      { name: "Main", kind: "thread", height: 200, visible: true, span_count: 42 },
      { name: "Worker", kind: "thread", height: 100, visible: true, span_count: 10 },
    ],
    viewport: { start: 0, end: 1, scroll_y: 0 },
    selected: null,
    search: "",
    theme: "dark",
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
    resetZoom: vi.fn(),
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

  it("starts not ready with empty snapshot", () => {
    expect(store.ready).toBe(false);
    expect(store.getSnapshot().profile).toBeNull();
    expect(store.getSnapshot().lanes).toEqual([]);
  });

  it("becomes ready after attach", () => {
    const wasm = mockWasm();
    store.attach(wasm);
    expect(store.ready).toBe(true);
  });

  it("flushes pending commands on attach", () => {
    const wasm = mockWasm();
    store.exec((w) => w.loadProfile(new Uint8Array([1, 2, 3])));
    expect(wasm.loadProfile).not.toHaveBeenCalled();
    store.attach(wasm);
    expect(wasm.loadProfile).toHaveBeenCalledWith(new Uint8Array([1, 2, 3]));
  });

  it("reads initial snapshot on attach", () => {
    const wasm = mockWasm();
    store.attach(wasm);
    expect(store.getSnapshot().lanes).toHaveLength(2);
    expect(store.getSnapshot().lanes[0].name).toBe("Main");
    expect(store.getSnapshot().lanes[0].span_count).toBe(42);
  });

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

  it("exec runs immediately when ready", () => {
    const wasm = mockWasm();
    store.attach(wasm);
    store.exec((w) => w.setSearch("hello"));
    expect(wasm.setSearch).toHaveBeenCalledWith("hello");
  });

  it("registers onStateChange callback", () => {
    const wasm = mockWasm();
    store.attach(wasm);
    expect(wasm.onStateChange).toHaveBeenCalledTimes(1);
  });

  it("handles setViewport", () => {
    const wasm = mockWasm();
    store.attach(wasm);
    store.exec((w) => w.setViewport(0.2, 0.8));
    expect(wasm.setViewport).toHaveBeenCalledWith(0.2, 0.8);
    expect(store.getSnapshot().viewport.start).toBe(0.2);
    expect(store.getSnapshot().viewport.end).toBe(0.8);
  });

  it("handles lane visibility toggle", () => {
    const wasm = mockWasm();
    store.attach(wasm);
    store.exec((w) => w.setLaneVisibility(0, false));
    expect(store.getSnapshot().lanes[0].visible).toBe(false);
  });

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

  it("handles setLaneHeight", () => {
    const wasm = mockWasm();
    store.attach(wasm);
    store.exec((w) => w.setLaneHeight(0, 300));
    expect(wasm.setLaneHeight).toHaveBeenCalledWith(0, 300);
    expect(store.getSnapshot().lanes[0].height).toBe(300);
  });

  it("handles reorderLanes", () => {
    const wasm = mockWasm();
    store.attach(wasm);
    expect(store.getSnapshot().lanes[0].name).toBe("Main");
    expect(store.getSnapshot().lanes[1].name).toBe("Worker");

    store.exec((w) => w.reorderLanes(0, 1));
    expect(wasm.reorderLanes).toHaveBeenCalledWith(0, 1);
    expect(store.getSnapshot().lanes[0].name).toBe("Worker");
    expect(store.getSnapshot().lanes[1].name).toBe("Main");
  });

  it("exposes span_count per lane", () => {
    const wasm = mockWasm();
    store.attach(wasm);
    expect(store.getSnapshot().lanes[0].span_count).toBe(42);
    expect(store.getSnapshot().lanes[1].span_count).toBe(10);
  });
});
