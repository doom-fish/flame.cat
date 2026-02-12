import {
  useRef,
  useEffect,
  useCallback,
  useState,
  useImperativeHandle,
  forwardRef,
  type CSSProperties,
} from "react";

/**
 * The WASM module interface exported by the flame-cat-ui crate.
 * Users provide this via the `wasmModule` prop or a global init.
 */
export interface FlameCatWasm {
  /** Mount on a canvas element by DOM ID. */
  startOnCanvas(canvasId: string): void;
  /** Load profile data (Uint8Array). */
  loadProfile(data: Uint8Array): void;
}

export interface FlameGraphProps {
  /** Width in CSS pixels. Defaults to "100%". */
  width?: number | string;
  /** Height in CSS pixels. Defaults to 600. */
  height?: number | string;
  /**
   * Profile data to load. When this changes, loadProfile() is called.
   * Accepts ArrayBuffer or Uint8Array of any supported profile format
   * (Chrome DevTools, React DevTools, Firefox, perf, etc).
   */
  data?: ArrayBuffer | Uint8Array | null;
  /**
   * URL to load the WASM bundle from. Required.
   * This should point to the output of `trunk build` — the directory
   * containing the `.js` and `_bg.wasm` files.
   *
   * Example: "/wasm/flame-cat-ui.js"
   */
  wasmUrl: string;
  /** Additional CSS class name for the container. */
  className?: string;
  /** Additional inline styles for the container. */
  style?: CSSProperties;
  /** Called when the WASM module finishes loading. */
  onReady?: () => void;
  /** Called if WASM loading fails. */
  onError?: (error: Error) => void;
}

export interface FlameGraphRef {
  /** Load a profile from raw bytes. */
  loadProfile(data: ArrayBuffer | Uint8Array): void;
  /** Access the underlying WASM module. */
  getWasmModule(): FlameCatWasm | null;
}

let instanceCounter = 0;

export const FlameGraph = forwardRef<FlameGraphRef, FlameGraphProps>(
  function FlameGraph(
    { width = "100%", height = 600, data, wasmUrl, className, style, onReady, onError },
    ref,
  ) {
    const containerRef = useRef<HTMLDivElement>(null);
    const canvasIdRef = useRef(`flame_cat_canvas_${++instanceCounter}`);
    const wasmRef = useRef<FlameCatWasm | null>(null);
    const [ready, setReady] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const pendingDataRef = useRef<ArrayBuffer | Uint8Array | null>(null);

    // Expose imperative API
    useImperativeHandle(ref, () => ({
      loadProfile(profileData: ArrayBuffer | Uint8Array) {
        const bytes =
          profileData instanceof Uint8Array
            ? profileData
            : new Uint8Array(profileData);
        if (wasmRef.current) {
          wasmRef.current.loadProfile(bytes);
        } else {
          pendingDataRef.current = profileData;
        }
      },
      getWasmModule() {
        return wasmRef.current;
      },
    }));

    // Load WASM module
    useEffect(() => {
      let cancelled = false;
      const canvasId = canvasIdRef.current;

      async function init() {
        try {
          // Dynamic import of the WASM JS glue
          const mod = await import(/* @vite-ignore */ wasmUrl);

          if (cancelled) return;

          // Trunk generates an `init()` or default export that initializes WASM.
          // wasm-bindgen generates a default export function that takes the wasm URL.
          if (typeof mod.default === "function") {
            // wasm-bindgen style: default export is the init function
            // Derive the _bg.wasm URL from the JS URL
            const wasmBinaryUrl = wasmUrl.replace(/\.js$/, "_bg.wasm");
            await mod.default(wasmBinaryUrl);
          }

          if (cancelled) return;

          // The #[wasm_bindgen(start)] runs automatically on init,
          // but we may need to call startOnCanvas for a custom ID.
          if (mod.startOnCanvas && canvasId !== "flame_cat_canvas") {
            mod.startOnCanvas(canvasId);
          }

          const wasmModule: FlameCatWasm = {
            startOnCanvas: mod.startOnCanvas,
            loadProfile: mod.loadProfile,
          };
          wasmRef.current = wasmModule;
          setReady(true);
          onReady?.();

          // Load any pending data
          if (pendingDataRef.current) {
            const bytes =
              pendingDataRef.current instanceof Uint8Array
                ? pendingDataRef.current
                : new Uint8Array(pendingDataRef.current);
            wasmModule.loadProfile(bytes);
            pendingDataRef.current = null;
          }
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

    // Load data when prop changes
    useEffect(() => {
      if (!data) return;
      const bytes =
        data instanceof Uint8Array ? data : new Uint8Array(data);
      if (wasmRef.current) {
        wasmRef.current.loadProfile(bytes);
      } else {
        pendingDataRef.current = data;
      }
    }, [data]);

    const containerStyle: CSSProperties = {
      position: "relative",
      width,
      height,
      overflow: "hidden",
      ...style,
    };

    if (error) {
      return (
        <div className={className} style={containerStyle}>
          <div
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              height: "100%",
              color: "#ef4444",
              fontFamily: "system-ui, sans-serif",
              fontSize: 14,
            }}
          >
            Failed to load flame graph: {error}
          </div>
        </div>
      );
    }

    return (
      <div ref={containerRef} className={className} style={containerStyle}>
        <canvas
          id={canvasIdRef.current}
          style={{ width: "100%", height: "100%" }}
        />
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
            Loading flame graph…
          </div>
        )}
      </div>
    );
  },
);
