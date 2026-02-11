use serde::{Deserialize, Serialize};

/// Semantic color tokens resolved by the renderer's active theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThemeToken {
    FlameHot,
    FlameWarm,
    FlameCold,
    FlameNeutral,

    LaneBackground,
    LaneBorder,
    LaneHeaderBackground,
    LaneHeaderText,

    TextPrimary,
    TextSecondary,
    TextMuted,

    SelectionHighlight,
    HoverHighlight,

    Background,
    Surface,
    Border,

    // Toolbar
    ToolbarBackground,
    ToolbarText,
    ToolbarTabActive,
    ToolbarTabHover,

    // Minimap
    MinimapBackground,
    MinimapViewport,
    MinimapDensity,
    MinimapHandle,

    // Inline lane labels
    InlineLabelText,
    InlineLabelBackground,

    // Table / Ranked view
    TableRowEven,
    TableRowOdd,
    TableHeaderBackground,
    TableBorder,
    BarFill,
    SearchHighlight,

    // Counter tracks
    CounterFill,
    CounterLine,
    CounterText,

    // Markers / navigation timing
    MarkerLine,
    MarkerText,

    // Async spans
    AsyncSpanFill,
    AsyncSpanBorder,

    // Frame cost track
    FrameGood,
    FrameWarning,
    FrameDropped,

    // Flow arrows
    FlowArrow,
    FlowArrowHead,
}
