import {
  useRef,
  useEffect,
  useState,
  useImperativeHandle,
  forwardRef,
  type CSSProperties,
} from "react";
import type { FlameCatWasm, FlameGraphHandle } from "./types";

export interface FlameGraphProps {
  /**
   * URL to the flame-cat WASM JS glue file.
   * Point this to the `.js` file from `trunk build` output.
   */
  wasmUrl: string;
  /** Width. Defaults to "100%". */
  width?: number | string;
  /** Height. Defaults to "100%". */
  height?: number | string;
  /** CSS class for the container div. */
  className?: string;
  /** Inline styles for the container div. */
  style?: CSSProperties;
  /** Called when the WASM module is ready. */
  onReady?: () => void;
  /** Called if WASM initialization fails. */
  onError?: (error: Error) => void;
}

let instanceCounter = 0;

export const FlameGraph = forwardRef<FlameGraphHandle, FlameGraphProps>(
  function FlameGraph(
    { wasmUrl, width = "100%", height = "100%", className, style, onReady, onError },
    ref,
  ) {
    const canvasIdRef = useRef(`flame_cat_${++instanceCounter}`);
    const wasmRef = useRef<FlameCatWasm | null>(null);
    const [ready, setReady] = useState(false);
    const [error, setError] = useState<string | null>(null);

    useImperativeHandle(
      ref,
      () => ({
        loadProfile(data: ArrayBuffer | Uint8Array) {
          const bytes = data instanceof Uint8Array ? data : new Uint8Array(data);
          wasmRef.current?.loadProfile(bytes);
        },
        setTheme(mode: "dark" | "light") {
          wasmRef.current?.setTheme(mode);
        },
        setSearch(query: string) {
          wasmRef.current?.setSearch(query);
        },
        resetZoom() {
          wasmRef.current?.resetZoom();
        },
        isReady() {
          return wasmRef.current !== null;
        },
      }),
      [],
    );

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

          // The #[wasm_bindgen(start)] auto-mounts on "flame_cat_canvas".
          // If our canvas has a different ID, mount explicitly.
          if (canvasId !== "flame_cat_canvas" && mod.startOnCanvas) {
            mod.startOnCanvas(canvasId);
          }

          wasmRef.current = {
            startOnCanvas: mod.startOnCanvas,
            loadProfile: mod.loadProfile,
            setTheme: mod.setTheme,
            setSearch: mod.setSearch,
            resetZoom: mod.resetZoom,
          };
          setReady(true);
          onReady?.();
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
    }, [wasmUrl]); // eslint-disable-line react-hooks/exhaustive-deps

    if (error) {
      return (
        <div
          className={className}
          style={{
            width,
            height,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            color: "#ef4444",
            fontFamily: "system-ui, sans-serif",
            fontSize: 14,
            ...style,
          }}
        >
          Failed to load flame graph: {error}
        </div>
      );
    }

    return (
      <div
        className={className}
        style={{ position: "relative", width, height, overflow: "hidden", ...style }}
      >
        <canvas id={canvasIdRef.current} style={{ width: "100%", height: "100%" }} />
        {!ready && (
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
  },
);
