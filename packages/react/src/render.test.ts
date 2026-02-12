import { describe, it, expect } from "vitest";
import {
  computeTimeRange,
  computeMaxDepth,
  computeContentHeight,
  hitTest,
} from "./render";
import type { FlameSpan, HitRegion } from "./types";

const sampleSpans: FlameSpan[] = [
  { id: 1, name: "main", start: 0, end: 100, depth: 0 },
  { id: 2, name: "foo", start: 10, end: 60, depth: 1 },
  { id: 3, name: "bar", start: 60, end: 90, depth: 1 },
  { id: 4, name: "baz", start: 15, end: 45, depth: 2 },
];

describe("computeTimeRange", () => {
  it("returns min/max of span start/end", () => {
    const range = computeTimeRange(sampleSpans);
    expect(range.start).toBe(0);
    expect(range.end).toBe(100);
  });

  it("handles empty spans", () => {
    const range = computeTimeRange([]);
    expect(range.start).toBe(0);
    expect(range.end).toBe(1);
  });

  it("handles single span", () => {
    const range = computeTimeRange([
      { id: 1, name: "only", start: 50, end: 75, depth: 0 },
    ]);
    expect(range.start).toBe(50);
    expect(range.end).toBe(75);
  });
});

describe("computeMaxDepth", () => {
  it("returns maximum depth", () => {
    expect(computeMaxDepth(sampleSpans)).toBe(2);
  });

  it("returns 0 for empty", () => {
    expect(computeMaxDepth([])).toBe(0);
  });
});

describe("computeContentHeight", () => {
  it("computes height from depth", () => {
    // 3 depth levels (0,1,2) × 20px = 60px
    expect(computeContentHeight(sampleSpans)).toBe(60);
  });
});

describe("hitTest", () => {
  const regions: HitRegion[] = [
    {
      x: 0,
      y: 0,
      w: 100,
      h: 19,
      span: { id: 1, name: "main", start: 0, end: 100, depth: 0 },
    },
    {
      x: 10,
      y: 20,
      w: 50,
      h: 19,
      span: { id: 2, name: "foo", start: 10, end: 60, depth: 1 },
    },
    {
      x: 60,
      y: 20,
      w: 30,
      h: 19,
      span: { id: 3, name: "bar", start: 60, end: 90, depth: 1 },
    },
  ];

  it("finds span at point", () => {
    const hit = hitTest(regions, 50, 10);
    expect(hit?.name).toBe("main");
  });

  it("finds deeper span at overlapping point", () => {
    const hit = hitTest(regions, 30, 25);
    expect(hit?.name).toBe("foo");
  });

  it("returns null for empty area", () => {
    expect(hitTest(regions, 200, 200)).toBeNull();
  });

  it("returns last drawn span when overlapping", () => {
    // If two regions overlap, last one (drawn on top) wins
    const overlapping: HitRegion[] = [
      {
        x: 0,
        y: 0,
        w: 100,
        h: 20,
        span: { id: 1, name: "a", start: 0, end: 100, depth: 0 },
      },
      {
        x: 0,
        y: 0,
        w: 100,
        h: 20,
        span: { id: 2, name: "b", start: 0, end: 100, depth: 0 },
      },
    ];
    expect(hitTest(overlapping, 50, 10)?.name).toBe("b");
  });
});
