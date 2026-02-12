import { describe, it, expect, vi } from "vitest";
import { render } from "@testing-library/react";
import { FlameGraph } from "./FlameGraph";

describe("FlameGraph component", () => {
  it("renders a canvas element with unique ID", () => {
    const { container } = render(
      <FlameGraph wasmUrl="/wasm/flame-cat-ui.js" />,
    );
    const canvas = container.querySelector("canvas");
    expect(canvas).toBeTruthy();
    expect(canvas?.id).toMatch(/^flame_cat_canvas_\d+$/);
  });

  it("shows loading indicator initially", () => {
    const { container } = render(
      <FlameGraph wasmUrl="/wasm/flame-cat-ui.js" />,
    );
    expect(container.textContent).toContain("Loading flame graph");
  });

  it("applies className and style", () => {
    const { container } = render(
      <FlameGraph
        wasmUrl="/wasm/flame-cat-ui.js"
        className="my-flame"
        style={{ border: "1px solid red" }}
      />,
    );
    const div = container.firstElementChild as HTMLElement;
    expect(div.className).toBe("my-flame");
    expect(div.style.border).toBe("1px solid red");
  });

  it("applies width and height", () => {
    const { container } = render(
      <FlameGraph wasmUrl="/wasm/flame-cat-ui.js" width={800} height={400} />,
    );
    const div = container.firstElementChild as HTMLElement;
    expect(div.style.width).toBe("800px");
    expect(div.style.height).toBe("400px");
  });

  it("renders multiple instances with unique canvas IDs", () => {
    const { container } = render(
      <div>
        <FlameGraph wasmUrl="/wasm/a.js" />
        <FlameGraph wasmUrl="/wasm/b.js" />
      </div>,
    );
    const canvases = container.querySelectorAll("canvas");
    expect(canvases.length).toBe(2);
    expect(canvases[0].id).not.toBe(canvases[1].id);
  });
});
