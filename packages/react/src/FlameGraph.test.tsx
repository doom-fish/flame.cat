import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import { FlameGraph } from "./FlameGraph";
import { createRef } from "react";
import type { FlameGraphHandle } from "./types";

describe("FlameGraph", () => {
  it("renders a canvas with unique ID", () => {
    const { container } = render(<FlameGraph wasmUrl="/wasm/fc.js" />);
    const canvas = container.querySelector("canvas");
    expect(canvas).toBeTruthy();
    expect(canvas?.id).toMatch(/^flame_cat_\d+$/);
  });

  it("shows loading state", () => {
    const { container } = render(<FlameGraph wasmUrl="/wasm/fc.js" />);
    expect(container.textContent).toContain("Loading");
  });

  it("applies width, height, className, style", () => {
    const { container } = render(
      <FlameGraph
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

  it("exposes imperative handle via ref", () => {
    const ref = createRef<FlameGraphHandle>();
    render(<FlameGraph ref={ref} wasmUrl="/wasm/fc.js" />);
    expect(ref.current).toBeTruthy();
    expect(typeof ref.current!.loadProfile).toBe("function");
    expect(typeof ref.current!.setTheme).toBe("function");
    expect(typeof ref.current!.setSearch).toBe("function");
    expect(typeof ref.current!.resetZoom).toBe("function");
    expect(ref.current!.isReady()).toBe(false);
  });

  it("renders multiple instances with unique IDs", () => {
    const { container } = render(
      <div>
        <FlameGraph wasmUrl="/a.js" />
        <FlameGraph wasmUrl="/b.js" />
      </div>,
    );
    const ids = Array.from(container.querySelectorAll("canvas")).map((c) => c.id);
    expect(ids.length).toBe(2);
    expect(ids[0]).not.toBe(ids[1]);
  });
});
