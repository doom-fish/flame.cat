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

    // Table / Ranked view
    TableRowEven,
    TableRowOdd,
    TableHeaderBackground,
    TableBorder,
    BarFill,
    SearchHighlight,
}
