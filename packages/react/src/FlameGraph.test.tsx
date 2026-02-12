import { describe, it, expect, vi, beforeAll } from "vitest";
import { render, renderHook, act } from "@testing-library/react";
import { FlameGraph } from "./FlameGraph";
import { useFlameGraph } from "./useFlameGraph";

// jsdom doesn't provide ResizeObserver
beforeAll(() => {
  if (typeof globalThis.ResizeObserver === "undefined") {
    globalThis.ResizeObserver = class {
      observe() {}
      unobserve() {}
      disconnect() {}
    } as unknown as typeof ResizeObserver;
  }
});

describe("useFlameGraph", () => {
  it("returns a stable controller", () => {
    const { result, rerender } = renderHook(() => useFlameGraph());
    const first = result.current;
    rerender();
    expect(result.current).toBe(first);
  });

  it("starts not ready", () => {
    const { result } = renderHook(() => useFlameGraph());
    expect(result.current.ready).toBe(false);
  });

  it("queues calls before WASM attaches, then flushes", () => {
    const { result } = renderHook(() => useFlameGraph());
    const ctrl = result.current;

    const mockWasm = {
      startOnCanvas: vi.fn(),
      loadProfile: vi.fn(),
      setTheme: vi.fn(),
      setSearch: vi.fn(),
      resetZoom: vi.fn(),
    };

    // Queue before attach
    ctrl.setTheme("light");
    ctrl.setSearch("render");
    ctrl.resetZoom();
    expect(mockWasm.setTheme).not.toHaveBeenCalled();

    // Attach flushes
    act(() => ctrl._attach(mockWasm));

    expect(mockWasm.setTheme).toHaveBeenCalledWith("light");
    expect(mockWasm.setSearch).toHaveBeenCalledWith("render");
    expect(mockWasm.resetZoom).toHaveBeenCalled();
    expect(ctrl.ready).toBe(true);
  });

  it("calls WASM directly after attach", () => {
    const { result } = renderHook(() => useFlameGraph());
    const ctrl = result.current;
    const mockWasm = {
      startOnCanvas: vi.fn(),
      loadProfile: vi.fn(),
      setTheme: vi.fn(),
      setSearch: vi.fn(),
      resetZoom: vi.fn(),
    };
    act(() => ctrl._attach(mockWasm));

    ctrl.setTheme("dark");
    expect(mockWasm.setTheme).toHaveBeenCalledWith("dark");

    ctrl.loadProfile(new Uint8Array([1, 2, 3]));
    expect(mockWasm.loadProfile).toHaveBeenCalled();
  });
});

describe("FlameGraph component", () => {
  it("renders a canvas with unique ID", () => {
    const { result } = renderHook(() => useFlameGraph());
    const { container } = render(
      <FlameGraph controller={result.current} wasmUrl="/wasm/fc.js" />,
    );
    const canvas = container.querySelector("canvas");
    expect(canvas).toBeTruthy();
    expect(canvas?.id).toMatch(/^flame_cat_\d+$/);
  });

  it("shows loading when not ready", () => {
    const { result } = renderHook(() => useFlameGraph());
    const { container } = render(
      <FlameGraph controller={result.current} wasmUrl="/wasm/fc.js" />,
    );
    expect(container.textContent).toContain("Loading");
  });

  it("applies width, height, className, style", () => {
    const { result } = renderHook(() => useFlameGraph());
    const { container } = render(
      <FlameGraph
        controller={result.current}
        wasmUrl="/wasm/fc.js"
        width={800}
        height={400}
        className="fg"
        style={{ border: "1px solid red" }}
      />,
    );
    const div = container.firstElementChild as HTMLElement;
    expect(div.className).toBe("fg");
    expect(div.style.width).toBe("800px");
    expect(div.style.height).toBe("400px");
    expect(div.style.border).toBe("1px solid red");
  });

  it("renders multiple instances with unique IDs", () => {
    const { result: r1 } = renderHook(() => useFlameGraph());
    const { result: r2 } = renderHook(() => useFlameGraph());
    const { container } = render(
      <div>
        <FlameGraph controller={r1.current} wasmUrl="/a.js" />
        <FlameGraph controller={r2.current} wasmUrl="/b.js" />
      </div>,
    );
    const ids = Array.from(container.querySelectorAll("canvas")).map((c) => c.id);
    expect(ids.length).toBe(2);
    expect(ids[0]).not.toBe(ids[1]);
  });

  it("uses 100% sizing in adaptive mode", () => {
    const { result } = renderHook(() => useFlameGraph());
    const { container } = render(
      <FlameGraph controller={result.current} wasmUrl="/fc.js" adaptive />,
    );
    const div = container.firstElementChild as HTMLElement;
    expect(div.style.width).toBe("100%");
    expect(div.style.height).toBe("100%");
  });
});
