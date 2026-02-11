/** Mirrors crates/protocol/src/commands.rs */

export type ThemeToken =
  | "FlameHot"
  | "FlameWarm"
  | "FlameCold"
  | "FlameNeutral"
  | "LaneBackground"
  | "LaneBorder"
  | "LaneHeaderBackground"
  | "LaneHeaderText"
  | "TextPrimary"
  | "TextSecondary"
  | "TextMuted"
  | "SelectionHighlight"
  | "HoverHighlight"
  | "Background"
  | "Surface"
  | "Border"
  | "ToolbarBackground"
  | "ToolbarText"
  | "ToolbarTabActive"
  | "ToolbarTabHover"
  | "MinimapBackground"
  | "MinimapViewport"
  | "TableRowEven"
  | "TableRowOdd"
  | "TableHeaderBackground"
  | "TableBorder"
  | "BarFill"
  | "SearchHighlight"
  | "CounterFill"
  | "CounterLine"
  | "CounterText"
  | "MarkerLine"
  | "MarkerText"
  | "AsyncSpanFill"
  | "AsyncSpanBorder"
  | "FrameGood"
  | "FrameWarning"
  | "FrameDropped"
  | "FlowArrow";

export interface Point {
  x: number;
  y: number;
}

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export type RenderCommand =
  | {
      DrawRect: {
        rect: Rect;
        color: ThemeToken;
        border_color: ThemeToken | null;
        label: string | null;
        frame_id: number | null;
      };
    }
  | {
      DrawText: {
        position: Point;
        text: string;
        color: ThemeToken;
        font_size: number;
        align: "Left" | "Center" | "Right";
      };
    }
  | { DrawLine: { from: Point; to: Point; color: ThemeToken; width: number } }
  | { SetClip: { rect: Rect } }
  | "ClearClip"
  | { PushTransform: { translate: Point; scale: Point } }
  | "PopTransform"
  | { BeginGroup: { id: string; label: string | null } }
  | "EndGroup";
