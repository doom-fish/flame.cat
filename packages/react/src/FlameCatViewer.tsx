import { useRef, useEffect, type CSSProperties } from "react";
import { useFlameCatStore } from "./FlameCatProvider";

export interface FlameCatViewerProps {
  /** CSS class for the container div. */
  className?: string;
  /** Inline styles for the container div. */
  style?: CSSProperties;
  /** ARIA label for the canvas (accessibility). */
  ariaLabel?: string;
}

let viewerCounter = 0;

/**
 * The egui rendering surface. Place this where you want the flame graph viewer.
 * eframe handles canvas resizing internally — just give the container the size you want.
 */
export function FlameCatViewer({
  className,
  style,
  ariaLabel = "Flame graph viewer",
}: FlameCatViewerProps) {
  const store = useFlameCatStore();
  const canvasIdRef = useRef(`flame_cat_viewer_${++viewerCounter}`);
  const mountedRef = useRef(false);

  useEffect(() => {
    if (mountedRef.current) return;
    store.exec((wasm) => {
      wasm.startOnCanvas(canvasIdRef.current);
      mountedRef.current = true;
    });
  }, [store]);

  return (
    <div
      className={className}
      style={{ position: "relative", width: "100%", height: "100%", ...style }}
    >
      <canvas
        id={canvasIdRef.current}
        role="img"
        aria-label={ariaLabel}
        tabIndex={0}
        style={{ display: "block", width: "100%", height: "100%" }}
      />
    </div>
  );
}
