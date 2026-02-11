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
  | { kind: "lane-reorder"; visibleIndex: number; startY: number; currentY: number }
  | { kind: "time-select"; anchorFrac: number };

/** Time selection range in view-fractional coordinates [0,1]. */
export interface TimeSelection {
  start: number;
  end: number;
}

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
): { cleanup: () => void; animateViewTo: (start: number, end: number, durationMs?: number) => void; getTimeSelection: () => TimeSelection | null; clearTimeSelection: () => void } {
  let drag: DragMode = { kind: "none" };
  let timeSelection: TimeSelection | null = null;

  /** Convert a canvas X to a fractional position [0,1] across the full timeline. */
  const xToFrac = (x: number): number => Math.max(0, Math.min(1, x / canvas.clientWidth));

  /** Convert a canvas X to a view-fractional position (fraction within the visible window). */
  const xToViewFrac = (x: number): number => {
    const { viewStart, viewEnd } = laneManager.getViewWindow();
    const frac = x / canvas.clientWidth;
    return viewStart + frac * (viewEnd - viewStart);
  };

  /** Is the Y coordinate inside the minimap area? (excludes time axis) */
  const isInMinimap = (y: number): boolean => isProfileLoaded() && y < minimapHeight() - 24;

  /** Is the Y coordinate inside the time axis area? (between minimap and lanes) */
  const isInTimeAxis = (y: number): boolean => {
    if (!isProfileLoaded()) return false;
    const mmH = minimapHeight(); // This is MINIMAP_HEIGHT + TIME_AXIS_HEIGHT
    return y >= mmH - 24 && y < mmH;
  };

  /** Is the Y coordinate over the minimap's viewport indicator? */
  const isOnMinimapViewport = (x: number, y: number): boolean => {
    if (!isInMinimap(y)) return false;
    const { viewStart, viewEnd } = laneManager.getViewWindow();
    const vpLeft = viewStart * canvas.clientWidth;
    const vpRight = viewEnd * canvas.clientWidth;
    return x >= vpLeft && x <= vpRight;
  };

  // ── Smooth animation ────────────────────────────────────────────────

  let animationId: number | null = null;

  /** Smoothly animate the view window from current to target values. */
  const animateViewTo = (targetStart: number, targetEnd: number, durationMs = 200) => {
    if (animationId != null) cancelAnimationFrame(animationId);
    const fromStart = laneManager.viewStart;
    const fromEnd = laneManager.viewEnd;
    const startTime = performance.now();

    const step = (now: number) => {
      const t = Math.min(1, (now - startTime) / durationMs);
      // Ease-out cubic
      const ease = 1 - (1 - t) ** 3;
      laneManager.viewStart = fromStart + (targetStart - fromStart) * ease;
      laneManager.viewEnd = fromEnd + (targetEnd - fromEnd) * ease;
      onRender();
      if (t < 1) {
        animationId = requestAnimationFrame(step);
      } else {
        animationId = null;
      }
    };
    animationId = requestAnimationFrame(step);
  };

  // ── WASD Navigation (Perfetto-style spring-animated) ──────────────

  const wasdState = {
    panVelocity: 0,    // fractional units per second (positive = right)
    zoomVelocity: 0,   // zoom factor per second (positive = zoom in)
    keys: new Set<string>(),
    mouseX: canvas.clientWidth / 2,
    active: false,
    lastFrame: 0,
  };

  const WASD_PAN_ACCEL = 1.5;   // fractional units/s² (at current zoom)
  const WASD_ZOOM_ACCEL = 3.0;  // zoom factor/s²
  const WASD_SNAP = 0.4;        // velocity snap-to-zero threshold
  const WASD_FRICTION = 6.0;    // velocity decay rate

  let wasdAnimId: number | null = null;

  canvas.addEventListener("mousemove", (e) => {
    wasdState.mouseX = e.offsetX;
  });

  const wasdTick = (now: number) => {
    if (!wasdState.active && Math.abs(wasdState.panVelocity) < 0.0001 && Math.abs(wasdState.zoomVelocity) < 0.001) {
      wasdAnimId = null;
      return;
    }

    const dt = wasdState.lastFrame > 0 ? Math.min((now - wasdState.lastFrame) / 1000, 0.05) : 0.016;
    wasdState.lastFrame = now;

    // Apply acceleration from held keys
    let panInput = 0;
    let zoomInput = 0;
    if (wasdState.keys.has("a") || wasdState.keys.has("A")) panInput -= 1;
    if (wasdState.keys.has("d") || wasdState.keys.has("D")) panInput += 1;
    if (wasdState.keys.has("w") || wasdState.keys.has("W")) zoomInput += 1;
    if (wasdState.keys.has("s") || wasdState.keys.has("S")) zoomInput -= 1;

    const viewSpan = laneManager.viewEnd - laneManager.viewStart;

    // Acceleration scales with current zoom level
    wasdState.panVelocity += panInput * WASD_PAN_ACCEL * viewSpan * dt;
    wasdState.zoomVelocity += zoomInput * WASD_ZOOM_ACCEL * dt;

    // Friction / decay
    wasdState.panVelocity *= Math.exp(-WASD_FRICTION * dt);
    wasdState.zoomVelocity *= Math.exp(-WASD_FRICTION * dt);

    // Snap to zero
    if (Math.abs(wasdState.panVelocity) < 0.0001 * viewSpan) wasdState.panVelocity = 0;
    if (Math.abs(wasdState.zoomVelocity) < 0.001) wasdState.zoomVelocity = 0;

    // Apply pan
    if (wasdState.panVelocity !== 0) {
      const panDelta = wasdState.panVelocity * dt;
      laneManager.viewStart = Math.max(0, Math.min(1 - viewSpan, laneManager.viewStart + panDelta));
      laneManager.viewEnd = laneManager.viewStart + viewSpan;
    }

    // Apply zoom at mouse cursor
    if (wasdState.zoomVelocity !== 0) {
      const zoomFactor = Math.pow(2, wasdState.zoomVelocity * dt);
      laneManager.zoomAt(zoomFactor, wasdState.mouseX, canvas.clientWidth);
    }

    onRender();
    wasdAnimId = requestAnimationFrame(wasdTick);
  };

  const startWasd = () => {
    if (wasdAnimId == null) {
      wasdState.lastFrame = 0;
      wasdAnimId = requestAnimationFrame(wasdTick);
    }
  };

  // ── Mouse ──────────────────────────────────────────────────────────

  const onWheel = (e: WheelEvent) => {
    e.preventDefault();
    if (e.ctrlKey || e.metaKey) {
      // Pinch-to-zoom on trackpad (or Ctrl+scroll on mouse)
      const factor = e.deltaY > 0 ? 0.9 : 1.1;
      laneManager.zoomAt(factor, e.offsetX, canvas.clientWidth);
    } else {
      // Horizontal: deltaX from trackpad swipe or Shift+scroll
      const dx = e.shiftKey ? e.deltaY : e.deltaX;
      if (dx !== 0) {
        laneManager.scrollBy(dx, 0, canvas.clientWidth);
      }
      // Vertical scroll
      const dy = e.shiftKey ? 0 : e.deltaY;
      if (dy !== 0) {
        const mmH = minimapHeight();
        const viewportHeight = canvas.clientHeight - mmH;
        laneManager.scrollGlobal(dy, viewportHeight);
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

    // 1b. Time axis drag-to-select
    if (isInTimeAxis(e.offsetY)) {
      const frac = xToViewFrac(e.offsetX);
      drag = { kind: "time-select", anchorFrac: frac };
      timeSelection = { start: frac, end: frac };
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
      case "time-select": {
        const frac = xToViewFrac(e.offsetX);
        const lo = Math.min(drag.anchorFrac, frac);
        const hi = Math.max(drag.anchorFrac, frac);
        timeSelection = { start: lo, end: hi };
        onRender();
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
        const dy = e.clientY - drag.lastY;
        laneManager.scrollBy(dx, 0, canvas.clientWidth);
        // Global vertical scroll
        const mmH = minimapHeight();
        const viewportHeight = canvas.clientHeight - mmH;
        laneManager.scrollGlobal(-dy, viewportHeight);
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
        } else if (isInTimeAxis(e.offsetY)) {
          canvas.style.cursor = "text";
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
    if (drag.kind === "time-select") {
      // Clear tiny selections (just a click)
      if (timeSelection && timeSelection.end - timeSelection.start < 0.0005) {
        timeSelection = null;
      }
      onRender();
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
    // WASD navigation (skip if typing in an input)
    const tag = (document.activeElement as HTMLElement)?.tagName;
    if ("wasdWASD".includes(e.key) && !e.ctrlKey && !e.metaKey && !e.altKey && tag !== "INPUT" && tag !== "TEXTAREA") {
      wasdState.keys.add(e.key);
      wasdState.active = true;
      startWasd();
      e.preventDefault();
      return;
    }

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
      case "ArrowUp": {
        const mmH = minimapHeight();
        laneManager.scrollGlobal(-step, canvas.clientHeight - mmH);
        onRender();
        break;
      }
      case "ArrowDown": {
        const mmH = minimapHeight();
        laneManager.scrollGlobal(step, canvas.clientHeight - mmH);
        onRender();
        break;
      }
      case "+":
      case "=":
        laneManager.zoomAt(1.2, canvas.clientWidth / 2, canvas.clientWidth);
        onRender();
        break;
      case "-":
        laneManager.zoomAt(0.8, canvas.clientWidth / 2, canvas.clientWidth);
        onRender();
        break;
      case "Home":
        // Reset zoom to full view (animated)
        animateViewTo(0, 1);
        laneManager.globalScrollY = 0;
        for (const lane of laneManager.visibleLanes) lane.scrollY = 0;
        break;
      case "0":
        // Also reset zoom with 0 key (animated)
        animateViewTo(0, 1);
        break;
      case "f":
      case "F":
        if (tag !== "INPUT" && tag !== "TEXTAREA") {
          // Fit view to full profile
          animateViewTo(0, 1);
        }
        break;
      case "z":
      case "Z":
        if (tag !== "INPUT" && tag !== "TEXTAREA" && timeSelection) {
          // Zoom to time selection
          const padding = (timeSelection.end - timeSelection.start) * 0.05;
          animateViewTo(
            Math.max(0, timeSelection.start - padding),
            Math.min(1, timeSelection.end + padding),
          );
        }
        break;
      case "Escape":
        if (timeSelection) {
          timeSelection = null;
          onRender();
        }
        break;
    }
  };

  const onKeyUp = (e: KeyboardEvent) => {
    if ("wasdWASD".includes(e.key)) {
      wasdState.keys.delete(e.key);
      if (wasdState.keys.size === 0) {
        wasdState.active = false;
      }
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
      const viewportHeight = canvas.clientHeight - mmH;
      laneManager.scrollGlobal(dy, viewportHeight);
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
  window.addEventListener("keyup", onKeyUp);
  canvas.addEventListener("touchstart", onTouchStart, { passive: false });
  canvas.addEventListener("touchmove", onTouchMove, { passive: false });
  canvas.addEventListener("touchend", onTouchEnd);

  const cleanup = () => {
    canvas.removeEventListener("wheel", onWheel);
    canvas.removeEventListener("mousedown", onMouseDown);
    window.removeEventListener("mousemove", onMouseMove);
    window.removeEventListener("mouseup", onMouseUp);
    window.removeEventListener("keydown", onKeyDown);
    window.removeEventListener("keyup", onKeyUp);
    canvas.removeEventListener("touchstart", onTouchStart);
    canvas.removeEventListener("touchmove", onTouchMove);
    canvas.removeEventListener("touchend", onTouchEnd);
  };

  return {
    cleanup,
    animateViewTo,
    getTimeSelection: () => timeSelection,
    clearTimeSelection: () => { timeSelection = null; },
  };
}
