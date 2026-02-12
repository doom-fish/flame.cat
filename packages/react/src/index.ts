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
  useViewType,
  useLanes,
  useViewport,
  useSearch,
  useTheme,
  useColorMode,
  useSelectedSpan,
  useSpanNavigation,
  useHoveredSpan,
  useNavigation,
  useExport,
  useHotkeys,
} from "./hooks";

export type {
  FlameGraphController,
  StatusState,
  ViewTypeState,
  LanesState,
  ViewportState,
  SearchState,
  ThemeState,
  ColorMode,
  ColorModeState,
  SelectionState,
  SpanNavigationState,
  NavigationState,
  ExportState,
  HotkeyMap,
} from "./hooks";

// Types
export type {
  StateSnapshot,
  ProfileInfo,
  LaneInfo,
  LaneKind,
  ViewportInfo,
  SelectedSpanInfo,
  ViewType,
} from "./types";
