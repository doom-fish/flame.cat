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
        FlameHot => ResolvedColor::rgb(255, 100, 50),
        FlameWarm => ResolvedColor::rgb(255, 180, 50),
        FlameCold => ResolvedColor::rgb(80, 180, 255),
        FlameNeutral => ResolvedColor::rgb(100, 120, 160),

        LaneBackground => ResolvedColor::rgb(30, 30, 35),
        LaneBorder => ResolvedColor::rgb(60, 60, 70),
        LaneHeaderBackground => ResolvedColor::rgb(40, 40, 48),
        LaneHeaderText => ResolvedColor::rgb(200, 200, 210),

        TextPrimary => ResolvedColor::rgb(230, 230, 240),
        TextSecondary => ResolvedColor::rgb(160, 160, 175),
        TextMuted => ResolvedColor::rgb(100, 100, 115),

        SelectionHighlight => ResolvedColor::rgba(66, 135, 245, 80),
        HoverHighlight => ResolvedColor::rgba(255, 255, 255, 30),

        Background => ResolvedColor::rgb(22, 22, 28),
        Surface => ResolvedColor::rgb(35, 35, 42),
        Border => ResolvedColor::rgb(55, 55, 65),

        ToolbarBackground => ResolvedColor::rgb(28, 28, 34),
        ToolbarText => ResolvedColor::rgb(200, 200, 210),
        ToolbarTabActive => ResolvedColor::rgb(66, 135, 245),
        ToolbarTabHover => ResolvedColor::rgba(255, 255, 255, 20),

        MinimapBackground => ResolvedColor::rgb(25, 25, 32),
        MinimapViewport => ResolvedColor::rgba(100, 160, 255, 25),
        MinimapDensity => ResolvedColor::rgb(100, 170, 255),
        MinimapHandle => ResolvedColor::rgb(180, 200, 255),

        InlineLabelText => ResolvedColor::rgb(200, 200, 210),
        InlineLabelBackground => ResolvedColor::rgb(30, 30, 40),

        TableRowEven => ResolvedColor::rgb(30, 30, 35),
        TableRowOdd => ResolvedColor::rgb(35, 35, 42),
        TableHeaderBackground => ResolvedColor::rgb(40, 40, 48),
        TableBorder => ResolvedColor::rgb(55, 55, 65),
        BarFill => ResolvedColor::rgb(66, 135, 245),
        SearchHighlight => ResolvedColor::rgba(255, 200, 50, 120),

        CounterFill => ResolvedColor::rgba(66, 135, 245, 60),
        CounterLine => ResolvedColor::rgb(66, 135, 245),
        CounterText => ResolvedColor::rgb(160, 160, 175),

        MarkerLine => ResolvedColor::rgb(255, 200, 50),
        MarkerText => ResolvedColor::rgb(255, 200, 50),

        AsyncSpanFill => ResolvedColor::rgb(100, 160, 220),
        AsyncSpanBorder => ResolvedColor::rgb(70, 130, 200),

        FrameGood => ResolvedColor::rgb(76, 175, 80),
        FrameWarning => ResolvedColor::rgb(255, 193, 7),
        FrameDropped => ResolvedColor::rgb(244, 67, 54),

        FlowArrow => ResolvedColor::rgba(255, 150, 50, 160),
        FlowArrowHead => ResolvedColor::rgba(255, 150, 50, 200),
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
        TextMuted => ResolvedColor::rgb(140, 140, 160),

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
        MinimapViewport => ResolvedColor::rgba(50, 110, 220, 30),
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
