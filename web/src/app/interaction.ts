import type { LaneManager } from "./lane-manager";

/** Returns canvas-local coordinates from a Touch event. */
function touchOffset(touch: Touch, canvas: HTMLCanvasElement): { x: number; y: number } {
  const rect = canvas.getBoundingClientRect();
  return { x: touch.clientX - rect.left, y: touch.clientY - rect.top };
}

/** Distance between two touches. */
function touchDistance(a: Touch, b: Touch): number {
  const dx = a.clientX - b.clientX;
  const dy = a.clientY - b.clientY;
  return Math.sqrt(dx * dx + dy * dy);
}

/** Midpoint x between two touches relative to canvas. */
function touchMidX(a: Touch, b: Touch, canvas: HTMLCanvasElement): number {
  const rect = canvas.getBoundingClientRect();
  return (a.clientX + b.clientX) / 2 - rect.left;
}

type DragMode =
  | { kind: "none" }
  | { kind: "lane-resize"; laneIndex: number; startY: number; startHeight: number }
  | { kind: "minimap-slide"; startFrac: number }
  | { kind: "minimap-select"; anchorFrac: number }
  | { kind: "canvas-pan"; lastX: number; lastY: number; laneIndex: number }
  | { kind: "lane-reorder"; visibleIndex: number; startY: number; currentY: number };

/**
 * Binds mouse/keyboard/touch events to the LaneManager and triggers re-renders.
 *
 * @param minimapHeight Height of the minimap in CSS pixels (0 if no profile loaded).
 * @param isProfileLoaded Returns true when a profile is loaded.
 * @param onLaneReorder Called when a lane header drag completes reordering.
 */
export function bindInteraction(
  canvas: HTMLCanvasElement,
  laneManager: LaneManager,
  onRender: () => void,
  minimapHeight: () => number,
  isProfileLoaded: () => boolean,
  onLaneReorder?: (fromVisible: number, toVisible: number) => void,
): () => void {
  let drag: DragMode = { kind: "none" };

  /** Convert a canvas X to a fractional position [0,1] across the full timeline. */
  const xToFrac = (x: number): number => Math.max(0, Math.min(1, x / canvas.clientWidth));

  /** Is the Y coordinate inside the minimap area? */
  const isInMinimap = (y: number): boolean => isProfileLoaded() && y < minimapHeight();

  /** Is the Y coordinate over the minimap's viewport indicator? */
  const isOnMinimapViewport = (x: number, y: number): boolean => {
    if (!isInMinimap(y)) return false;
    const { viewStart, viewEnd } = laneManager.getViewWindow();
    const vpLeft = viewStart * canvas.clientWidth;
    const vpRight = viewEnd * canvas.clientWidth;
    return x >= vpLeft && x <= vpRight;
  };

  // ── Mouse ──────────────────────────────────────────────────────────

  const onWheel = (e: WheelEvent) => {
    e.preventDefault();
    if (e.ctrlKey || e.metaKey) {
      // Zoom at cursor
      const factor = e.deltaY > 0 ? 0.9 : 1.1;
      laneManager.zoomAt(factor, e.offsetX, canvas.clientWidth);
    } else if (e.shiftKey) {
      // Horizontal scroll
      laneManager.scrollBy(e.deltaY, 0, canvas.clientWidth);
    } else {
      // Vertical scroll within a lane
      const mmH = minimapHeight();
      const laneIdx = laneManager.laneAtY(e.offsetY - mmH);
      if (laneIdx >= 0) {
        laneManager.scrollLane(laneIdx, e.deltaY);
      }
    }
    onRender();
  };

  const onMouseDown = (e: MouseEvent) => {
    const mmH = minimapHeight();
    const localY = e.offsetY - mmH;

    // 1. Minimap interactions
    if (isInMinimap(e.offsetY)) {
      if (isOnMinimapViewport(e.offsetX, e.offsetY)) {
        // Drag the viewport indicator
        drag = { kind: "minimap-slide", startFrac: xToFrac(e.offsetX) };
      } else {
        // Start a range selection (or click-to-center)
        drag = { kind: "minimap-select", anchorFrac: xToFrac(e.offsetX) };
      }
      e.preventDefault();
      return;
    }

    // 2. Lane resize handles
    const handleIdx = laneManager.isOnDragHandle(localY);
    if (handleIdx >= 0) {
      const lane = laneManager.visibleLanes[handleIdx];
      if (lane) {
        drag = {
          kind: "lane-resize",
          laneIndex: handleIdx,
          startY: e.clientY,
          startHeight: lane.height,
        };
      }
      e.preventDefault();
      return;
    }

    // 3. Lane header drag-to-reorder (click on header area, x < 24 = drag handle)
    const laneIdx = laneManager.laneAtY(localY);
    if (laneIdx >= 0) {
      const headerTop = laneManager.laneY(laneIdx);
      const isOnHeader = localY >= headerTop && localY < headerTop + laneManager.headerHeight;
      if (isOnHeader && e.offsetX < 24) {
        drag = { kind: "lane-reorder", visibleIndex: laneIdx, startY: e.clientY, currentY: e.clientY };
        canvas.style.cursor = "grabbing";
        e.preventDefault();
        return;
      }
    }

    // 4. Canvas drag-to-pan
    drag = {
      kind: "canvas-pan",
      lastX: e.clientX,
      lastY: e.clientY,
      laneIndex: laneIdx,
    };
    canvas.style.cursor = "grabbing";
    e.preventDefault();
  };

  const onMouseMove = (e: MouseEvent) => {
    switch (drag.kind) {
      case "minimap-slide": {
        const frac = xToFrac(e.offsetX);
        const delta = frac - drag.startFrac;
        const viewSpan = laneManager.viewEnd - laneManager.viewStart;
        laneManager.viewStart = Math.max(0, Math.min(1 - viewSpan, laneManager.viewStart + delta));
        laneManager.viewEnd = laneManager.viewStart + viewSpan;
        drag.startFrac = frac;
        onRender();
        return;
      }
      case "minimap-select": {
        // Live preview of the selection range
        const frac = xToFrac(e.offsetX);
        const lo = Math.min(drag.anchorFrac, frac);
        const hi = Math.max(drag.anchorFrac, frac);
        if (hi - lo > 0.002) {
          laneManager.viewStart = lo;
          laneManager.viewEnd = hi;
          onRender();
        }
        return;
      }
      case "lane-resize": {
        const lane = laneManager.visibleLanes[drag.laneIndex];
        if (lane) {
          const delta = e.clientY - drag.startY;
          lane.height = Math.max(60, drag.startHeight + delta);
          onRender();
        }
        return;
      }
      case "lane-reorder": {
        drag.currentY = e.clientY;
        // Visual feedback handled by cursor; actual reorder happens on mouseup
        return;
      }
      case "canvas-pan": {
        const dx = drag.lastX - e.clientX;
        const dy = drag.lastY - e.clientY;
        laneManager.scrollBy(dx, 0, canvas.clientWidth);
        if (drag.laneIndex >= 0) {
          laneManager.scrollLane(drag.laneIndex, dy);
        }
        drag.lastX = e.clientX;
        drag.lastY = e.clientY;
        onRender();
        return;
      }
      case "none": {
        // Cursor styling
        const mmH = minimapHeight();
        if (isInMinimap(e.offsetY)) {
          canvas.style.cursor = isOnMinimapViewport(e.offsetX, e.offsetY)
            ? "ew-resize"
            : "crosshair";
        } else {
          const localY = e.offsetY - mmH;
          const handleIdx = laneManager.isOnDragHandle(localY);
          if (handleIdx >= 0) {
            canvas.style.cursor = "row-resize";
          } else {
            // Check if on lane header drag handle area
            const lIdx = laneManager.laneAtY(localY);
            if (lIdx >= 0) {
              const headerTop = laneManager.laneY(lIdx);
              const isOnHeader = localY >= headerTop && localY < headerTop + laneManager.headerHeight;
              canvas.style.cursor = isOnHeader && e.offsetX < 24 ? "grab" : "default";
            } else {
              canvas.style.cursor = "default";
            }
          }
        }
        return;
      }
    }
  };

  const onMouseUp = (e: MouseEvent) => {
    if (drag.kind === "minimap-select") {
      // If barely moved, treat as click-to-center instead of range select
      const frac = xToFrac(e.offsetX);
      const span = Math.abs(frac - drag.anchorFrac);
      if (span < 0.002) {
        // Click-to-center: move viewport so this fraction is centered
        const viewSpan = laneManager.viewEnd - laneManager.viewStart;
        const center = frac;
        laneManager.viewStart = Math.max(0, Math.min(1 - viewSpan, center - viewSpan / 2));
        laneManager.viewEnd = laneManager.viewStart + viewSpan;
        onRender();
      }
    }
    if (drag.kind === "lane-reorder") {
      // Determine target visible lane based on Y displacement
      const mmH = minimapHeight();
      const targetLocalY = e.clientY - canvas.getBoundingClientRect().top - mmH;
      const targetIdx = laneManager.laneAtY(targetLocalY);
      if (targetIdx >= 0 && targetIdx !== drag.visibleIndex) {
        const fromFull = laneManager.visibleToFullIndex(drag.visibleIndex);
        const toFull = laneManager.visibleToFullIndex(targetIdx);
        laneManager.moveLane(fromFull, toFull);
        onLaneReorder?.(drag.visibleIndex, targetIdx);
      }
    }
    if (drag.kind !== "none") {
      drag = { kind: "none" };
      canvas.style.cursor = "default";
      onRender();
    }
  };

  const onKeyDown = (e: KeyboardEvent) => {
    const step = 40;
    switch (e.key) {
      case "ArrowLeft":
        laneManager.scrollBy(-step, 0, canvas.clientWidth);
        onRender();
        break;
      case "ArrowRight":
        laneManager.scrollBy(step, 0, canvas.clientWidth);
        onRender();
        break;
      case "ArrowUp":
        if (laneManager.lanes[0]) laneManager.scrollLane(0, -step);
        onRender();
        break;
      case "ArrowDown":
        if (laneManager.lanes[0]) laneManager.scrollLane(0, step);
        onRender();
        break;
      case "+":
      case "=":
        laneManager.zoomAt(1.2, canvas.clientWidth / 2, canvas.clientWidth);
        onRender();
        break;
      case "-":
        laneManager.zoomAt(0.8, canvas.clientWidth / 2, canvas.clientWidth);
        onRender();
        break;
    }
  };

  // ── Touch ──────────────────────────────────────────────────────────

  let touchState: {
    startX: number;
    startY: number;
    lastX: number;
    lastY: number;
    pinchDist: number | null;
    isSingleFinger: boolean;
  } | null = null;

  const onTouchStart = (e: TouchEvent) => {
    if (e.touches.length === 1) {
      const t = e.touches[0];
      if (!t) return;
      const pos = touchOffset(t, canvas);

      // Minimap touch
      if (isInMinimap(pos.y)) {
        if (isOnMinimapViewport(pos.x, pos.y)) {
          drag = { kind: "minimap-slide", startFrac: xToFrac(pos.x) };
        } else {
          drag = { kind: "minimap-select", anchorFrac: xToFrac(pos.x) };
        }
        e.preventDefault();
        return;
      }

      touchState = {
        startX: pos.x,
        startY: pos.y,
        lastX: pos.x,
        lastY: pos.y,
        pinchDist: null,
        isSingleFinger: true,
      };
      e.preventDefault();
    } else if (e.touches.length === 2) {
      const t0 = e.touches[0];
      const t1 = e.touches[1];
      if (!t0 || !t1) return;
      const dist = touchDistance(t0, t1);
      if (touchState) {
        touchState.pinchDist = dist;
        touchState.isSingleFinger = false;
      } else {
        const mid = touchMidX(t0, t1, canvas);
        touchState = {
          startX: mid,
          startY: 0,
          lastX: mid,
          lastY: 0,
          pinchDist: dist,
          isSingleFinger: false,
        };
      }
      e.preventDefault();
    }
  };

  const onTouchMove = (e: TouchEvent) => {
    // Minimap drag (touch)
    if (drag.kind === "minimap-slide" || drag.kind === "minimap-select") {
      const t = e.touches[0];
      if (!t) return;
      e.preventDefault();
      const pos = touchOffset(t, canvas);
      if (drag.kind === "minimap-slide") {
        const frac = xToFrac(pos.x);
        const delta = frac - drag.startFrac;
        const viewSpan = laneManager.viewEnd - laneManager.viewStart;
        laneManager.viewStart = Math.max(0, Math.min(1 - viewSpan, laneManager.viewStart + delta));
        laneManager.viewEnd = laneManager.viewStart + viewSpan;
        drag.startFrac = frac;
      } else {
        const frac = xToFrac(pos.x);
        const lo = Math.min(drag.anchorFrac, frac);
        const hi = Math.max(drag.anchorFrac, frac);
        if (hi - lo > 0.002) {
          laneManager.viewStart = lo;
          laneManager.viewEnd = hi;
        }
      }
      onRender();
      return;
    }

    if (!touchState) return;

    if (e.touches.length === 2 && touchState.pinchDist !== null) {
      const t0 = e.touches[0];
      const t1 = e.touches[1];
      if (!t0 || !t1) return;
      e.preventDefault();
      const newDist = touchDistance(t0, t1);
      const factor = newDist / touchState.pinchDist;
      const focalX = touchMidX(t0, t1, canvas);
      laneManager.zoomAt(factor, focalX, canvas.clientWidth);
      touchState.pinchDist = newDist;
      onRender();
    } else if (e.touches.length === 1 && touchState.isSingleFinger) {
      const t = e.touches[0];
      if (!t) return;
      e.preventDefault();
      const pos = touchOffset(t, canvas);
      const dx = touchState.lastX - pos.x;
      const dy = touchState.lastY - pos.y;
      laneManager.scrollBy(dx, 0, canvas.clientWidth);
      const mmH = minimapHeight();
      const laneIdx = laneManager.laneAtY(pos.y - mmH);
      if (laneIdx >= 0) {
        laneManager.scrollLane(laneIdx, dy);
      }
      touchState.lastX = pos.x;
      touchState.lastY = pos.y;
      onRender();
    }
  };

  const onTouchEnd = (_e: TouchEvent) => {
    if (drag.kind === "minimap-select" || drag.kind === "minimap-slide") {
      drag = { kind: "none" };
    }
    touchState = null;
  };

  canvas.addEventListener("wheel", onWheel, { passive: false });
  canvas.addEventListener("mousedown", onMouseDown);
  window.addEventListener("mousemove", onMouseMove);
  window.addEventListener("mouseup", onMouseUp);
  window.addEventListener("keydown", onKeyDown);
  canvas.addEventListener("touchstart", onTouchStart, { passive: false });
  canvas.addEventListener("touchmove", onTouchMove, { passive: false });
  canvas.addEventListener("touchend", onTouchEnd);

  return () => {
    canvas.removeEventListener("wheel", onWheel);
    canvas.removeEventListener("mousedown", onMouseDown);
    window.removeEventListener("mousemove", onMouseMove);
    window.removeEventListener("mouseup", onMouseUp);
    window.removeEventListener("keydown", onKeyDown);
    canvas.removeEventListener("touchstart", onTouchStart);
    canvas.removeEventListener("touchmove", onTouchMove);
    canvas.removeEventListener("touchend", onTouchEnd);
  };
}
