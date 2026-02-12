import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import { FlameGraph } from "./FlameGraph";
import type { FlameSpan } from "./types";

const spans: FlameSpan[] = [
  { id: 1, name: "main", start: 0, end: 100, depth: 0 },
  { id: 2, name: "foo", start: 10, end: 60, depth: 1 },
  { id: 3, name: "bar", start: 60, end: 90, depth: 1 },
];

describe("FlameGraph component", () => {
  it("renders a canvas element", () => {
    const { container } = render(
      <FlameGraph spans={spans} width={800} height={200} />,
    );
    const canvas = container.querySelector("canvas");
    expect(canvas).toBeTruthy();
  });

  it("renders with empty spans", () => {
    const { container } = render(
      <FlameGraph spans={[]} width={400} height={100} />,
    );
    const canvas = container.querySelector("canvas");
    expect(canvas).toBeTruthy();
  });

  it("auto-assigns IDs when missing", () => {
    const noIdSpans: FlameSpan[] = [
      { name: "a", start: 0, end: 50, depth: 0 },
      { name: "b", start: 50, end: 100, depth: 0 },
    ];
    const { container } = render(
      <FlameGraph spans={noIdSpans} width={400} height={100} />,
    );
    expect(container.querySelector("canvas")).toBeTruthy();
  });

  it("applies className and style", () => {
    const { container } = render(
      <FlameGraph
        spans={spans}
        width={400}
        height={100}
        className="my-flame"
        style={{ border: "1px solid red" }}
      />,
    );
    const div = container.firstElementChild as HTMLElement;
    expect(div.className).toBe("my-flame");
    expect(div.style.border).toBe("1px solid red");
  });
});
