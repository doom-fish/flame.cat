import {
  createContext,
  useContext,
  useEffect,
  useRef,
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
  /** Called when WASM initialization fails. */
  onError?: (error: Error) => void;
  children: ReactNode;
}

export function FlameCatProvider({
  wasmUrl,
  onError,
  children,
}: FlameCatProviderProps) {
  const storeRef = useRef<FlameCatStore | null>(null);
  if (!storeRef.current) {
    storeRef.current = new FlameCatStore();
  }
  const store = storeRef.current;

  // Stable ref for onError to avoid re-triggering the init effect
  const onErrorRef = useRef(onError);
  onErrorRef.current = onError;

  useEffect(() => {
    let cancelled = false;

    async function init() {
      try {
        const mod = await import(/* @vite-ignore */ wasmUrl);
        if (cancelled) return;

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
          setLaneHeight: mod.setLaneHeight,
          reorderLanes: mod.reorderLanes,
          selectSpan: mod.selectSpan,
          setViewType: mod.setViewType,
          navigateBack: mod.navigateBack,
          navigateForward: mod.navigateForward,
          setColorMode: mod.setColorMode,
          navigateToParent: mod.navigateToParent,
          navigateToChild: mod.navigateToChild,
          navigateToNextSibling: mod.navigateToNextSibling,
          navigateToPrevSibling: mod.navigateToPrevSibling,
          nextSearchResult: mod.nextSearchResult,
          prevSearchResult: mod.prevSearchResult,
          exportProfile: mod.exportProfile,
          exportSVG: mod.exportSVG,
          onStateChange: mod.onStateChange,
          getState: mod.getState,
        };

        store.attach(wasm);
      } catch (e) {
        if (cancelled) return;
        const err = e instanceof Error ? e : new Error(String(e));
        store.fail(err.message);
        onErrorRef.current?.(err);
      }
    }

    init();
    return () => {
      cancelled = true;
    };
  }, [wasmUrl, store]);

  return (
    <FlameCatContext.Provider value={store}>
      {children}
    </FlameCatContext.Provider>
  );
}

export { FlameCatContext };
