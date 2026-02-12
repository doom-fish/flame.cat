import {
  useRef,
  useEffect,
  useState,
  useCallback,
  type CSSProperties,
} from "react";
import { useFlameCatStore } from "./FlameCatProvider";

export interface FlameCanvasProps {
  /**
   * Auto-resize canvas to fill container using ResizeObserver.
   * Defaults to false.
   */
  adaptive?: boolean;
  /** CSS class for the container div. */
  className?: string;
  /** Inline styles for the container div. */
  style?: CSSProperties;
  /** Called when container resizes (only with adaptive). */
  onResize?: (width: number, height: number) => void;
}

let canvasCounter = 0;

export function FlameCanvas({
  adaptive = false,
  className,
  style,
  onResize,
}: FlameCanvasProps) {
  const store = useFlameCatStore();
  const containerRef = useRef<HTMLDivElement>(null);
  const canvasIdRef = useRef(`flame_cat_canvas_${++canvasCounter}`);
  const [size, setSize] = useState<{ w: number; h: number } | null>(null);
  const mountedRef = useRef(false);

  // Mount the WASM viewer on the canvas once store is ready
  useEffect(() => {
    if (mountedRef.current) return;
    store.exec((wasm) => {
      const id = canvasIdRef.current;
      if (id !== "flame_cat_canvas") {
        wasm.startOnCanvas(id);
      }
      mountedRef.current = true;
    });
  }, [store]);

  // ResizeObserver for adaptive sizing
  useEffect(() => {
    if (!adaptive) return;
    const el = containerRef.current;
    if (!el) return;
    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const { width: w, height: h } = entry.contentRect;
        setSize({ w, h });
        onResize?.(w, h);
      }
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, [adaptive, onResize]);

  // Apply observed size to canvas
  const resizeCanvas = useCallback(() => {
    if (!adaptive || !size) return;
    const canvas = document.getElementById(
      canvasIdRef.current,
    ) as HTMLCanvasElement | null;
    if (!canvas) return;
    canvas.style.width = `${size.w}px`;
    canvas.style.height = `${size.h}px`;
  }, [adaptive, size]);

  useEffect(resizeCanvas, [resizeCanvas]);

  const containerStyle: CSSProperties = adaptive
    ? { position: "relative", width: "100%", height: "100%", ...style }
    : { position: "relative", ...style };

  return (
    <div ref={containerRef} className={className} style={containerStyle}>
      <canvas
        id={canvasIdRef.current}
        style={{ width: "100%", height: "100%" }}
      />
    </div>
  );
}
