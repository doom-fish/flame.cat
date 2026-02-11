import type { ThemeToken } from "../protocol";

export interface Color {
  r: number;
  g: number;
  b: number;
  a: number;
}

export interface Theme {
  name: string;
  tokens: Record<ThemeToken, Color>;
}

export const darkTheme: Theme = {
  name: "dark",
  tokens: {
    FlameHot: { r: 0.91, g: 0.3, b: 0.24, a: 1.0 },
    FlameWarm: { r: 0.95, g: 0.61, b: 0.07, a: 1.0 },
    FlameCold: { r: 0.2, g: 0.6, b: 0.86, a: 1.0 },
    FlameNeutral: { r: 0.56, g: 0.56, b: 0.58, a: 1.0 },
    LaneBackground: { r: 0.11, g: 0.11, b: 0.12, a: 1.0 },
    LaneBorder: { r: 0.25, g: 0.25, b: 0.27, a: 1.0 },
    LaneHeaderBackground: { r: 0.15, g: 0.15, b: 0.17, a: 1.0 },
    LaneHeaderText: { r: 0.85, g: 0.85, b: 0.87, a: 1.0 },
    TextPrimary: { r: 0.93, g: 0.93, b: 0.94, a: 1.0 },
    TextSecondary: { r: 0.7, g: 0.7, b: 0.72, a: 1.0 },
    TextMuted: { r: 0.45, g: 0.45, b: 0.47, a: 1.0 },
    SelectionHighlight: { r: 0.3, g: 0.69, b: 0.31, a: 1.0 },
    HoverHighlight: { r: 1.0, g: 1.0, b: 1.0, a: 0.15 },
    Background: { r: 0.07, g: 0.07, b: 0.08, a: 1.0 },
    Surface: { r: 0.13, g: 0.13, b: 0.15, a: 1.0 },
    Border: { r: 0.2, g: 0.2, b: 0.22, a: 1.0 },
    ToolbarBackground: { r: 0.12, g: 0.12, b: 0.14, a: 1.0 },
    ToolbarText: { r: 0.85, g: 0.85, b: 0.87, a: 1.0 },
    ToolbarTabActive: { r: 0.3, g: 0.69, b: 0.31, a: 1.0 },
    ToolbarTabHover: { r: 0.2, g: 0.2, b: 0.22, a: 1.0 },
    MinimapBackground: { r: 0.09, g: 0.09, b: 0.1, a: 1.0 },
    MinimapViewport: { r: 1.0, g: 1.0, b: 1.0, a: 0.2 },
    TableRowEven: { r: 0.09, g: 0.09, b: 0.1, a: 1.0 },
    TableRowOdd: { r: 0.11, g: 0.11, b: 0.12, a: 1.0 },
    TableHeaderBackground: { r: 0.15, g: 0.15, b: 0.17, a: 1.0 },
    TableBorder: { r: 0.2, g: 0.2, b: 0.22, a: 1.0 },
    BarFill: { r: 0.3, g: 0.69, b: 0.31, a: 0.6 },
    SearchHighlight: { r: 1.0, g: 0.85, b: 0.0, a: 0.65 },
    CounterFill: { r: 0.24, g: 0.47, b: 0.78, a: 0.35 },
    CounterLine: { r: 0.31, g: 0.63, b: 0.94, a: 0.9 },
    CounterText: { r: 0.45, g: 0.72, b: 0.95, a: 1.0 },
    MarkerLine: { r: 0.78, g: 0.39, b: 0.39, a: 0.6 },
    MarkerText: { r: 0.9, g: 0.55, b: 0.55, a: 1.0 },
    AsyncSpanFill: { r: 0.39, g: 0.59, b: 0.78, a: 0.7 },
    AsyncSpanBorder: { r: 0.31, g: 0.47, b: 0.63, a: 1.0 },
    FrameGood: { r: 0.3, g: 0.69, b: 0.31, a: 0.8 },
    FrameWarning: { r: 0.95, g: 0.77, b: 0.06, a: 0.8 },
    FrameDropped: { r: 0.91, g: 0.3, b: 0.24, a: 0.9 },
    FlowArrow: { r: 0.47, g: 0.75, b: 0.96, a: 0.7 },
    NetworkBar: { r: 0.35, g: 0.65, b: 0.55, a: 0.8 },
    NetworkTTFB: { r: 0.6, g: 0.8, b: 0.7, a: 0.5 },
  },
};

export const lightTheme: Theme = {
  name: "light",
  tokens: {
    FlameHot: { r: 0.89, g: 0.26, b: 0.2, a: 1.0 },
    FlameWarm: { r: 0.93, g: 0.56, b: 0.04, a: 1.0 },
    FlameCold: { r: 0.16, g: 0.5, b: 0.73, a: 1.0 },
    FlameNeutral: { r: 0.62, g: 0.62, b: 0.64, a: 1.0 },
    LaneBackground: { r: 1.0, g: 1.0, b: 1.0, a: 1.0 },
    LaneBorder: { r: 0.82, g: 0.82, b: 0.84, a: 1.0 },
    LaneHeaderBackground: { r: 0.96, g: 0.96, b: 0.97, a: 1.0 },
    LaneHeaderText: { r: 0.13, g: 0.13, b: 0.15, a: 1.0 },
    TextPrimary: { r: 0.1, g: 0.1, b: 0.12, a: 1.0 },
    TextSecondary: { r: 0.35, g: 0.35, b: 0.37, a: 1.0 },
    TextMuted: { r: 0.6, g: 0.6, b: 0.62, a: 1.0 },
    SelectionHighlight: { r: 0.26, g: 0.63, b: 0.28, a: 1.0 },
    HoverHighlight: { r: 0.0, g: 0.0, b: 0.0, a: 0.08 },
    Background: { r: 0.98, g: 0.98, b: 0.99, a: 1.0 },
    Surface: { r: 1.0, g: 1.0, b: 1.0, a: 1.0 },
    Border: { r: 0.85, g: 0.85, b: 0.87, a: 1.0 },
    ToolbarBackground: { r: 0.94, g: 0.94, b: 0.95, a: 1.0 },
    ToolbarText: { r: 0.13, g: 0.13, b: 0.15, a: 1.0 },
    ToolbarTabActive: { r: 0.26, g: 0.63, b: 0.28, a: 1.0 },
    ToolbarTabHover: { r: 0.88, g: 0.88, b: 0.9, a: 1.0 },
    MinimapBackground: { r: 0.96, g: 0.96, b: 0.97, a: 1.0 },
    MinimapViewport: { r: 0.0, g: 0.0, b: 0.0, a: 0.12 },
    TableRowEven: { r: 1.0, g: 1.0, b: 1.0, a: 1.0 },
    TableRowOdd: { r: 0.97, g: 0.97, b: 0.98, a: 1.0 },
    TableHeaderBackground: { r: 0.94, g: 0.94, b: 0.95, a: 1.0 },
    TableBorder: { r: 0.85, g: 0.85, b: 0.87, a: 1.0 },
    BarFill: { r: 0.26, g: 0.63, b: 0.28, a: 0.5 },
    SearchHighlight: { r: 1.0, g: 0.85, b: 0.0, a: 0.55 },
    CounterFill: { r: 0.2, g: 0.4, b: 0.7, a: 0.25 },
    CounterLine: { r: 0.25, g: 0.53, b: 0.84, a: 0.8 },
    CounterText: { r: 0.16, g: 0.5, b: 0.73, a: 1.0 },
    MarkerLine: { r: 0.7, g: 0.3, b: 0.3, a: 0.5 },
    MarkerText: { r: 0.6, g: 0.2, b: 0.2, a: 1.0 },
    AsyncSpanFill: { r: 0.3, g: 0.5, b: 0.7, a: 0.5 },
    AsyncSpanBorder: { r: 0.25, g: 0.4, b: 0.55, a: 1.0 },
    FrameGood: { r: 0.26, g: 0.63, b: 0.28, a: 0.7 },
    FrameWarning: { r: 0.9, g: 0.7, b: 0.04, a: 0.7 },
    FrameDropped: { r: 0.89, g: 0.26, b: 0.2, a: 0.8 },
    FlowArrow: { r: 0.16, g: 0.47, b: 0.78, a: 0.6 },
    NetworkBar: { r: 0.2, g: 0.55, b: 0.45, a: 0.7 },
    NetworkTTFB: { r: 0.4, g: 0.7, b: 0.6, a: 0.4 },
  },
};

export function resolveColor(theme: Theme, token: ThemeToken): Color {
  return theme.tokens[token] as Color;
}

/** Convert a Color to a Float32Array [r, g, b, a] for GPU upload. */
export function colorToF32(color: Color): Float32Array {
  return new Float32Array([color.r, color.g, color.b, color.a]);
}
