use flame_cat_protocol::ThemeToken;

/// Resolved RGBA color for egui rendering.
#[derive(Debug, Clone, Copy)]
pub struct ResolvedColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl ResolvedColor {
    const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn to_color32(self) -> egui::Color32 {
        egui::Color32::from_rgba_premultiplied(self.r, self.g, self.b, self.a)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

pub fn resolve(token: ThemeToken, mode: ThemeMode) -> egui::Color32 {
    match mode {
        ThemeMode::Dark => resolve_dark(token),
        ThemeMode::Light => resolve_light(token),
    }
    .to_color32()
}

fn resolve_dark(token: ThemeToken) -> ResolvedColor {
    use ThemeToken::*;
    match token {
        // Perfetto-inspired flame palette
        FlameHot => ResolvedColor::rgb(244, 67, 54), // Material Red
        FlameWarm => ResolvedColor::rgb(255, 167, 38), // Material Orange
        FlameCold => ResolvedColor::rgb(66, 165, 245), // Material Blue
        FlameNeutral => ResolvedColor::rgb(149, 117, 205), // Material Purple

        LaneBackground => ResolvedColor::rgb(24, 24, 24), // Perfetto bg
        LaneBorder => ResolvedColor::rgb(48, 48, 48),
        LaneHeaderBackground => ResolvedColor::rgb(33, 33, 33),
        LaneHeaderText => ResolvedColor::rgb(224, 224, 224),

        TextPrimary => ResolvedColor::rgb(236, 236, 236),
        TextSecondary => ResolvedColor::rgb(158, 158, 158),
        TextMuted => ResolvedColor::rgb(136, 136, 136),

        SelectionHighlight => ResolvedColor::rgba(68, 138, 255, 90), // Perfetto accent
        HoverHighlight => ResolvedColor::rgba(255, 255, 255, 25),

        Background => ResolvedColor::rgb(18, 18, 18), // Perfetto dark bg
        Surface => ResolvedColor::rgb(33, 33, 33),
        Border => ResolvedColor::rgb(48, 48, 48),

        ToolbarBackground => ResolvedColor::rgb(24, 24, 24),
        ToolbarText => ResolvedColor::rgb(224, 224, 224),
        ToolbarTabActive => ResolvedColor::rgb(68, 138, 255),
        ToolbarTabHover => ResolvedColor::rgba(255, 255, 255, 15),

        // Minimap background — distinct from lane bg
        MinimapBackground => ResolvedColor::rgb(10, 10, 12),
        MinimapViewport => ResolvedColor::rgba(68, 138, 255, 100),
        MinimapDensity => ResolvedColor::rgb(80, 160, 255),
        MinimapHandle => ResolvedColor::rgb(200, 220, 255),
        MinimapHandle => ResolvedColor::rgb(144, 202, 249),

        InlineLabelText => ResolvedColor::rgb(224, 224, 224),
        InlineLabelBackground => ResolvedColor::rgb(33, 33, 33),

        TableRowEven => ResolvedColor::rgb(24, 24, 24),
        TableRowOdd => ResolvedColor::rgb(30, 30, 30),
        TableHeaderBackground => ResolvedColor::rgb(38, 38, 41),
        TableBorder => ResolvedColor::rgb(48, 48, 48),
        BarFill => ResolvedColor::rgb(68, 138, 255),
        SearchHighlight => ResolvedColor::rgba(255, 235, 59, 120), // Material Yellow

        CounterFill => ResolvedColor::rgba(68, 138, 255, 50),
        CounterLine => ResolvedColor::rgb(68, 138, 255),
        CounterText => ResolvedColor::rgb(158, 158, 158),

        MarkerLine => ResolvedColor::rgb(255, 214, 0),
        MarkerText => ResolvedColor::rgb(255, 214, 0),

        AsyncSpanFill => ResolvedColor::rgb(77, 182, 172), // Material Teal
        AsyncSpanBorder => ResolvedColor::rgb(0, 150, 136),

        FrameGood => ResolvedColor::rgb(76, 175, 80),
        FrameWarning => ResolvedColor::rgb(255, 193, 7),
        FrameDropped => ResolvedColor::rgb(244, 67, 54),

        FlowArrow => ResolvedColor::rgba(158, 158, 158, 80),
        FlowArrowHead => ResolvedColor::rgba(158, 158, 158, 120),
    }
}

fn resolve_light(token: ThemeToken) -> ResolvedColor {
    use ThemeToken::*;
    match token {
        FlameHot => ResolvedColor::rgb(220, 60, 20),
        FlameWarm => ResolvedColor::rgb(230, 150, 20),
        FlameCold => ResolvedColor::rgb(40, 120, 200),
        FlameNeutral => ResolvedColor::rgb(120, 140, 170),

        LaneBackground => ResolvedColor::rgb(250, 250, 252),
        LaneBorder => ResolvedColor::rgb(210, 210, 220),
        LaneHeaderBackground => ResolvedColor::rgb(240, 240, 245),
        LaneHeaderText => ResolvedColor::rgb(40, 40, 50),

        TextPrimary => ResolvedColor::rgb(20, 20, 30),
        TextSecondary => ResolvedColor::rgb(80, 80, 100),
        TextMuted => ResolvedColor::rgb(100, 100, 110),

        SelectionHighlight => ResolvedColor::rgba(66, 135, 245, 60),
        HoverHighlight => ResolvedColor::rgba(0, 0, 0, 15),

        Background => ResolvedColor::rgb(255, 255, 255),
        Surface => ResolvedColor::rgb(245, 245, 248),
        Border => ResolvedColor::rgb(210, 210, 220),

        ToolbarBackground => ResolvedColor::rgb(248, 248, 250),
        ToolbarText => ResolvedColor::rgb(40, 40, 50),
        ToolbarTabActive => ResolvedColor::rgb(50, 110, 220),
        ToolbarTabHover => ResolvedColor::rgba(0, 0, 0, 10),

        MinimapBackground => ResolvedColor::rgb(240, 240, 245),
        MinimapViewport => ResolvedColor::rgba(50, 110, 220, 50),
        MinimapDensity => ResolvedColor::rgb(50, 110, 220),
        MinimapHandle => ResolvedColor::rgb(40, 80, 180),

        InlineLabelText => ResolvedColor::rgb(40, 40, 50),
        InlineLabelBackground => ResolvedColor::rgb(240, 240, 245),

        TableRowEven => ResolvedColor::rgb(255, 255, 255),
        TableRowOdd => ResolvedColor::rgb(245, 245, 248),
        TableHeaderBackground => ResolvedColor::rgb(235, 235, 240),
        TableBorder => ResolvedColor::rgb(210, 210, 220),
        BarFill => ResolvedColor::rgb(50, 110, 220),
        SearchHighlight => ResolvedColor::rgba(255, 200, 50, 100),

        CounterFill => ResolvedColor::rgba(50, 110, 220, 40),
        CounterLine => ResolvedColor::rgb(50, 110, 220),
        CounterText => ResolvedColor::rgb(80, 80, 100),

        MarkerLine => ResolvedColor::rgb(200, 150, 20),
        MarkerText => ResolvedColor::rgb(150, 100, 10),

        AsyncSpanFill => ResolvedColor::rgb(80, 140, 200),
        AsyncSpanBorder => ResolvedColor::rgb(50, 110, 180),

        FrameGood => ResolvedColor::rgb(56, 142, 60),
        FrameWarning => ResolvedColor::rgb(230, 170, 0),
        FrameDropped => ResolvedColor::rgb(211, 47, 47),

        FlowArrow => ResolvedColor::rgba(50, 120, 220, 140),
        FlowArrowHead => ResolvedColor::rgba(50, 120, 220, 180),
    }
}
