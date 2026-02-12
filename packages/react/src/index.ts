// Provider
export { FlameCatProvider } from "./FlameCatProvider";
export type { FlameCatProviderProps } from "./FlameCatProvider";

// Canvas renderer
export { FlameCanvas } from "./FlameCanvas";
export type { FlameCanvasProps } from "./FlameCanvas";

// Composable hooks
export {
  useFlameGraph,
  useProfile,
  useLanes,
  useViewport,
  useSearch,
  useTheme,
  useSelectedSpan,
} from "./hooks";

export type {
  FlameGraphController,
  LanesState,
  ViewportState,
  SearchState,
  ThemeState,
  SelectionState,
} from "./hooks";

// Types
export type {
  StateSnapshot,
  ProfileInfo,
  LaneInfo,
  ViewportInfo,
  SelectedSpanInfo,
} from "./types";
