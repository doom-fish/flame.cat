import {
  createContext,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { FlameCatStore } from "./store";
import type { WasmExports } from "./types";

const FlameCatContext = createContext<FlameCatStore | null>(null);

export function useFlameCatStore(): FlameCatStore {
  const store = useContext(FlameCatContext);
  if (!store) {
    throw new Error("useFlameCatStore must be used within a <FlameCatProvider>");
  }
  return store;
}

export interface FlameCatProviderProps {
  /** URL to the flame-cat WASM JS glue file (e.g. "/wasm/flame-cat-ui.js"). */
  wasmUrl: string;
  /** Optional canvas DOM ID. Defaults to an auto-generated unique ID. */
  canvasId?: string;
  children: ReactNode;
}

let providerCounter = 0;

export function FlameCatProvider({
  wasmUrl,
  canvasId,
  children,
}: FlameCatProviderProps) {
  const storeRef = useRef<FlameCatStore | null>(null);
  if (!storeRef.current) {
    storeRef.current = new FlameCatStore();
  }
  const store = storeRef.current;
  const [error, setError] = useState<string | null>(null);
  const idRef = useRef(canvasId ?? `flame_cat_${++providerCounter}`);

  useEffect(() => {
    let cancelled = false;

    async function init() {
      try {
        const mod = await import(/* @vite-ignore */ wasmUrl);
        if (cancelled) return;

        // Initialize WASM module
        if (typeof mod.default === "function") {
          const wasmBinaryUrl = wasmUrl.replace(/\.js$/, "_bg.wasm");
          await mod.default(wasmBinaryUrl);
        }
        if (cancelled) return;

        const wasm: WasmExports = {
          startOnCanvas: mod.startOnCanvas,
          loadProfile: mod.loadProfile,
          setTheme: mod.setTheme,
          setSearch: mod.setSearch,
          resetZoom: mod.resetZoom,
          setViewport: mod.setViewport,
          setLaneVisibility: mod.setLaneVisibility,
          selectSpan: mod.selectSpan,
          onStateChange: mod.onStateChange,
          getState: mod.getState,
        };

        store.attach(wasm);
      } catch (e) {
        if (cancelled) return;
        setError(e instanceof Error ? e.message : String(e));
      }
    }

    init();
    return () => {
      cancelled = true;
    };
  }, [wasmUrl, store]);

  if (error) {
    return (
      <div
        style={{
          color: "#ef4444",
          fontFamily: "system-ui, sans-serif",
          fontSize: 14,
          padding: 16,
        }}
      >
        flame-cat failed to initialize: {error}
      </div>
    );
  }

  return (
    <FlameCatContext.Provider value={store}>
      {children}
    </FlameCatContext.Provider>
  );
}

export { FlameCatContext };
