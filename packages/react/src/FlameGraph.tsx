import { useRef, useEffect, useState, useCallback, type CSSProperties } from "react";
import type { FlameGraphController } from "./useFlameGraph";

export interface FlameGraphProps {
  /** Controller created by `useFlameGraph()`. */
  controller: FlameGraphController;
  /**
   * URL to the flame-cat WASM JS glue file.
   * Point to the `.js` output from `trunk build`.
   */
  wasmUrl: string;
  /** Width. Defaults to "100%". */
  width?: number | string;
  /** Height. Defaults to "100%". */
  height?: number | string;
  /**
   * When true, the canvas automatically resizes to fill its container
   * using ResizeObserver. Overrides width/height props.
   */
  adaptive?: boolean;
  /** CSS class for the container. */
  className?: string;
  /** Inline styles for the container. */
  style?: CSSProperties;
  /** Called if WASM initialization fails. */
  onError?: (error: Error) => void;
  /** Called when the container resizes (only with adaptive={true}). */
  onResize?: (width: number, height: number) => void;
}

let instanceCounter = 0;

export function FlameGraph({
  controller,
  wasmUrl,
  width = "100%",
  height = "100%",
  adaptive = false,
  className,
  style,
  onError,
  onResize,
}: FlameGraphProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const canvasIdRef = useRef(`flame_cat_${++instanceCounter}`);
  const [error, setError] = useState<string | null>(null);
  const [size, setSize] = useState<{ w: number; h: number } | null>(null);

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

  // Resize canvas to match observed size
  const resizeCanvas = useCallback(() => {
    if (!adaptive || !size) return;
    const canvas = document.getElementById(canvasIdRef.current) as HTMLCanvasElement | null;
    if (!canvas) return;
    canvas.style.width = `${size.w}px`;
    canvas.style.height = `${size.h}px`;
  }, [adaptive, size]);

  useEffect(resizeCanvas, [resizeCanvas]);

  useEffect(() => {
    let cancelled = false;
    const canvasId = canvasIdRef.current;

    async function init() {
      try {
        const mod = await import(/* @vite-ignore */ wasmUrl);
        if (cancelled) return;

        if (typeof mod.default === "function") {
          const wasmBinaryUrl = wasmUrl.replace(/\.js$/, "_bg.wasm");
          await mod.default(wasmBinaryUrl);
        }
        if (cancelled) return;

        if (canvasId !== "flame_cat_canvas" && mod.startOnCanvas) {
          mod.startOnCanvas(canvasId);
        }

        controller._attach({
          startOnCanvas: mod.startOnCanvas,
          loadProfile: mod.loadProfile,
          setTheme: mod.setTheme,
          setSearch: mod.setSearch,
          resetZoom: mod.resetZoom,
        });
      } catch (e) {
        if (cancelled) return;
        const err = e instanceof Error ? e : new Error(String(e));
        setError(err.message);
        onError?.(err);
      }
    }

    init();
    return () => {
      cancelled = true;
    };
  }, [wasmUrl, controller]); // eslint-disable-line react-hooks/exhaustive-deps

  const containerStyle: CSSProperties = adaptive
    ? { position: "relative", width: "100%", height: "100%", overflow: "hidden", ...style }
    : { position: "relative", width, height, overflow: "hidden", ...style };

  if (error) {
    return (
      <div
        className={className}
        style={{
          ...containerStyle,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: "#ef4444",
          fontFamily: "system-ui, sans-serif",
          fontSize: 14,
        }}
      >
        Failed to load flame graph: {error}
      </div>
    );
  }

  return (
    <div ref={containerRef} className={className} style={containerStyle}>
      <canvas id={canvasIdRef.current} style={{ width: "100%", height: "100%" }} />
      {!controller.ready && (
        <div
          style={{
            position: "absolute",
            inset: 0,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            color: "#888",
            fontFamily: "system-ui, sans-serif",
            fontSize: 14,
          }}
        >
          Loading…
        </div>
      )}
    </div>
  );
}
