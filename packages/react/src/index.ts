// Provider
export { FlameCatProvider } from "./FlameCatProvider";
export type { FlameCatProviderProps } from "./FlameCatProvider";

// Viewer (egui rendering surface)
export { FlameCatViewer } from "./FlameCatViewer";
export type { FlameCatViewerProps } from "./FlameCatViewer";

// Store
export type { FlameCatStatus } from "./store";

// Composable hooks
export {
  useFlameGraph,
  useStatus,
  useProfile,
  useLanes,
  useViewport,
  useSearch,
  useTheme,
  useSelectedSpan,
  useHotkeys,
} from "./hooks";

export type {
  FlameGraphController,
  StatusState,
  LanesState,
  ViewportState,
  SearchState,
  ThemeState,
  SelectionState,
  HotkeyMap,
} from "./hooks";

// Types
export type {
  StateSnapshot,
  ProfileInfo,
  LaneInfo,
  ViewportInfo,
  SelectedSpanInfo,
} from "./types";
