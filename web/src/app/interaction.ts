import type { LaneManager } from "./lane-manager";

/**
 * Binds mouse/keyboard/touch events to the LaneManager and triggers re-renders.
 */
export function bindInteraction(
  canvas: HTMLCanvasElement,
  laneManager: LaneManager,
  onRender: () => void,
): () => void {
  const onWheel = (e: WheelEvent) => {
    e.preventDefault();
    if (e.ctrlKey || e.metaKey) {
      // Zoom
      const factor = e.deltaY > 0 ? 0.9 : 1.1;
      laneManager.zoomAt(factor, e.offsetX);
    } else if (e.shiftKey) {
      // Horizontal scroll
      laneManager.scrollBy(e.deltaY, 0);
    } else {
      // Vertical scroll on the lane under the cursor
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

    // Cursor style
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

  canvas.addEventListener("wheel", onWheel, { passive: false });
  canvas.addEventListener("mousedown", onMouseDown);
  window.addEventListener("mousemove", onMouseMove);
  window.addEventListener("mouseup", onMouseUp);
  window.addEventListener("keydown", onKeyDown);

  return () => {
    canvas.removeEventListener("wheel", onWheel);
    canvas.removeEventListener("mousedown", onMouseDown);
    window.removeEventListener("mousemove", onMouseMove);
    window.removeEventListener("mouseup", onMouseUp);
    window.removeEventListener("keydown", onKeyDown);
  };
}
