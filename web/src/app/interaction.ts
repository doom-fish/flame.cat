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

/**
 * Binds mouse/keyboard/touch events to the LaneManager and triggers re-renders.
 */
export function bindInteraction(
  canvas: HTMLCanvasElement,
  laneManager: LaneManager,
  onRender: () => void,
): () => void {
  // --- Mouse ---

  const onWheel = (e: WheelEvent) => {
    e.preventDefault();
    if (e.ctrlKey || e.metaKey) {
      const factor = e.deltaY > 0 ? 0.9 : 1.1;
      laneManager.zoomAt(factor, e.offsetX);
    } else if (e.shiftKey) {
      laneManager.scrollBy(e.deltaY, 0);
    } else {
      const laneIdx = laneManager.laneAtY(e.offsetY);
      if (laneIdx >= 0) {
        laneManager.scrollLane(laneIdx, e.deltaY);
      }
    }
    onRender();
  };

  const onMouseDown = (e: MouseEvent) => {
    const handleIdx = laneManager.isOnDragHandle(e.offsetY);
    if (handleIdx >= 0) {
      laneManager.startDrag(handleIdx, e.clientY);
      e.preventDefault();
    }
  };

  const onMouseMove = (e: MouseEvent) => {
    if (laneManager.isDragging) {
      laneManager.updateDrag(e.clientY);
      onRender();
    }
    const handleIdx = laneManager.isOnDragHandle(e.offsetY);
    canvas.style.cursor = handleIdx >= 0 || laneManager.isDragging ? "row-resize" : "default";
  };

  const onMouseUp = () => {
    if (laneManager.isDragging) {
      laneManager.endDrag();
      onRender();
    }
  };

  const onKeyDown = (e: KeyboardEvent) => {
    const step = 40;
    switch (e.key) {
      case "ArrowLeft":
        laneManager.scrollBy(-step, 0);
        onRender();
        break;
      case "ArrowRight":
        laneManager.scrollBy(step, 0);
        onRender();
        break;
      case "+":
      case "=":
        laneManager.zoomAt(1.2, canvas.clientWidth / 2);
        onRender();
        break;
      case "-":
        laneManager.zoomAt(0.8, canvas.clientWidth / 2);
        onRender();
        break;
    }
  };

  // --- Touch ---

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
      touchState = {
        startX: pos.x,
        startY: pos.y,
        lastX: pos.x,
        lastY: pos.y,
        pinchDist: null,
        isSingleFinger: true,
      };
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
    if (!touchState) return;

    if (e.touches.length === 2 && touchState.pinchDist !== null) {
      const t0 = e.touches[0];
      const t1 = e.touches[1];
      if (!t0 || !t1) return;
      e.preventDefault();
      const newDist = touchDistance(t0, t1);
      const factor = newDist / touchState.pinchDist;
      const focalX = touchMidX(t0, t1, canvas);
      laneManager.zoomAt(factor, focalX);
      touchState.pinchDist = newDist;
      onRender();
    } else if (e.touches.length === 1 && touchState.isSingleFinger) {
      const t = e.touches[0];
      if (!t) return;
      e.preventDefault();
      const pos = touchOffset(t, canvas);
      const dx = touchState.lastX - pos.x;
      const dy = touchState.lastY - pos.y;
      laneManager.scrollBy(dx, 0);
      const laneIdx = laneManager.laneAtY(pos.y);
      if (laneIdx >= 0) {
        laneManager.scrollLane(laneIdx, dy);
      }
      touchState.lastX = pos.x;
      touchState.lastY = pos.y;
      onRender();
    }
  };

  const onTouchEnd = (_e: TouchEvent) => {
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
