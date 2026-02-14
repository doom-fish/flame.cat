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
        egui::Color32::from_rgba_unmultiplied(self.r, self.g, self.b, self.a)
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
    // Catppuccin Mocha palette
    use ThemeToken::*;
    match token {
        FlameHot => ResolvedColor::rgb(0xf3, 0x8b, 0xa8), // Red
        FlameWarm => ResolvedColor::rgb(0xfa, 0xb3, 0x87), // Peach
        FlameCold => ResolvedColor::rgb(0x89, 0xb4, 0xfa), // Blue
        FlameNeutral => ResolvedColor::rgb(0xcb, 0xa6, 0xf7), // Mauve

        LaneBackground => ResolvedColor::rgb(0x1e, 0x1e, 0x2e), // Base
        LaneBorder => ResolvedColor::rgb(0x31, 0x32, 0x44),     // Surface0
        LaneHeaderBackground => ResolvedColor::rgb(0x18, 0x18, 0x25), // Mantle
        LaneHeaderText => ResolvedColor::rgb(0xcd, 0xd6, 0xf4), // Text

        TextPrimary => ResolvedColor::rgb(0xcd, 0xd6, 0xf4), // Text
        TextSecondary => ResolvedColor::rgb(0xba, 0xc2, 0xde), // Subtext1
        TextMuted => ResolvedColor::rgb(0xa6, 0xad, 0xc8),   // Subtext0

        SelectionHighlight => ResolvedColor::rgba(0x89, 0xb4, 0xfa, 80), // Blue
        HoverHighlight => ResolvedColor::rgba(0xcd, 0xd6, 0xf4, 25),

        Background => ResolvedColor::rgb(0x11, 0x11, 0x1b), // Crust
        Surface => ResolvedColor::rgb(0x18, 0x18, 0x25),    // Mantle
        Border => ResolvedColor::rgb(0x31, 0x32, 0x44),     // Surface0

        ToolbarBackground => ResolvedColor::rgb(0x18, 0x18, 0x25),
        ToolbarText => ResolvedColor::rgb(0xcd, 0xd6, 0xf4),
        ToolbarTabActive => ResolvedColor::rgb(0x89, 0xb4, 0xfa), // Blue
        ToolbarTabHover => ResolvedColor::rgba(0xcd, 0xd6, 0xf4, 15),

        MinimapBackground => ResolvedColor::rgb(0x11, 0x11, 0x1b), // Crust
        MinimapViewport => ResolvedColor::rgba(0x89, 0xb4, 0xfa, 60),
        MinimapDensity => ResolvedColor::rgb(0xb4, 0xbe, 0xfe), // Lavender
        MinimapHandle => ResolvedColor::rgb(0xb4, 0xbe, 0xfe),

        InlineLabelText => ResolvedColor::rgb(0xcd, 0xd6, 0xf4),
        InlineLabelBackground => ResolvedColor::rgb(0x1e, 0x1e, 0x2e), // Base

        TableRowEven => ResolvedColor::rgb(0x1e, 0x1e, 0x2e), // Base
        TableRowOdd => ResolvedColor::rgb(0x18, 0x18, 0x25),  // Mantle
        TableHeaderBackground => ResolvedColor::rgb(0x31, 0x32, 0x44), // Surface0
        TableBorder => ResolvedColor::rgb(0x45, 0x47, 0x5a),  // Surface1
        BarFill => ResolvedColor::rgb(0x89, 0xb4, 0xfa),      // Blue
        SearchHighlight => ResolvedColor::rgba(0xf9, 0xe2, 0xaf, 120), // Yellow

        CounterFill => ResolvedColor::rgba(0x74, 0xc7, 0xec, 50), // Sapphire
        CounterLine => ResolvedColor::rgb(0x74, 0xc7, 0xec),
        CounterText => ResolvedColor::rgb(0xba, 0xc2, 0xde), // Subtext1

        MarkerLine => ResolvedColor::rgb(0xf9, 0xe2, 0xaf), // Yellow
        MarkerText => ResolvedColor::rgb(0xf9, 0xe2, 0xaf),

        AsyncSpanFill => ResolvedColor::rgb(0x94, 0xe2, 0xd5), // Teal
        AsyncSpanBorder => ResolvedColor::rgb(0x74, 0xc7, 0xec), // Sapphire

        FrameGood => ResolvedColor::rgb(0xa6, 0xe3, 0xa1), // Green
        FrameWarning => ResolvedColor::rgb(0xf9, 0xe2, 0xaf), // Yellow
        FrameDropped => ResolvedColor::rgb(0xf3, 0x8b, 0xa8), // Red

        FlowArrow => ResolvedColor::rgba(0x6c, 0x70, 0x86, 80), // Overlay0
        FlowArrowHead => ResolvedColor::rgba(0x6c, 0x70, 0x86, 120),
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

// ── Typography scale ───────────────────────────────────────────────────────

pub const FONT_DISPLAY: f32 = 32.0;
pub const FONT_TITLE: f32 = 18.0;
pub const FONT_EMPHASIS: f32 = 14.0;
pub const FONT_BODY: f32 = 12.0;
pub const FONT_CAPTION: f32 = 11.0;
pub const FONT_TINY: f32 = 10.0;

// ── egui visual presets ────────────────────────────────────────────────────

/// Catppuccin Mocha dark visuals for egui widgets.
pub fn catapult_dark_visuals() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    v.panel_fill = egui::Color32::from_rgb(0x18, 0x18, 0x25);
    v.window_fill = egui::Color32::from_rgb(0x1e, 0x1e, 0x2e);
    v.extreme_bg_color = egui::Color32::from_rgb(0x11, 0x11, 0x1b);
    v.faint_bg_color = egui::Color32::from_rgb(0x1e, 0x1e, 0x2e);
    v.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(0x31, 0x32, 0x44);
    v.widgets.noninteractive.fg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(0xba, 0xc2, 0xde));
    v.widgets.noninteractive.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(0x45, 0x47, 0x5a));
    v.widgets.inactive.bg_fill = egui::Color32::from_rgb(0x45, 0x47, 0x5a);
    v.widgets.inactive.fg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(0xba, 0xc2, 0xde));
    v.widgets.hovered.bg_fill = egui::Color32::from_rgb(0x58, 0x5b, 0x70);
    v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(0xcd, 0xd6, 0xf4));
    v.widgets.active.bg_fill = egui::Color32::from_rgb(0x89, 0xb4, 0xfa);
    v.widgets.active.fg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(0x1e, 0x1e, 0x2e));
    v.selection.bg_fill = egui::Color32::from_rgba_unmultiplied(0x89, 0xb4, 0xfa, 60);
    v.selection.stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(0x89, 0xb4, 0xfa));
    v.window_corner_radius = egui::CornerRadius::same(6);
    v.menu_corner_radius = egui::CornerRadius::same(6);
    v.widgets.noninteractive.corner_radius = egui::CornerRadius::same(5);
    v.widgets.inactive.corner_radius = egui::CornerRadius::same(5);
    v.widgets.hovered.corner_radius = egui::CornerRadius::same(5);
    v.widgets.active.corner_radius = egui::CornerRadius::same(5);
    v.widgets.open.corner_radius = egui::CornerRadius::same(5);
    v.hyperlink_color = egui::Color32::from_rgb(0x89, 0xb4, 0xfa);
    v.warn_fg_color = egui::Color32::from_rgb(0xf9, 0xe2, 0xaf);
    v.error_fg_color = egui::Color32::from_rgb(0xf3, 0x8b, 0xa8);
    v
}

/// Light visuals for egui widgets.
pub fn catapult_light_visuals() -> egui::Visuals {
    let mut v = egui::Visuals::light();
    v.panel_fill = egui::Color32::from_rgb(250, 250, 252);
    v.window_fill = egui::Color32::from_rgb(255, 255, 255);
    v.extreme_bg_color = egui::Color32::from_rgb(255, 255, 255);
    v.faint_bg_color = egui::Color32::from_rgb(245, 245, 248);
    v.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(240, 240, 243);
    v.widgets.noninteractive.fg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 60, 70));
    v.widgets.noninteractive.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(210, 210, 215));
    v.widgets.inactive.bg_fill = egui::Color32::from_rgb(230, 230, 235);
    v.widgets.hovered.bg_fill = egui::Color32::from_rgb(220, 220, 228);
    v.widgets.active.bg_fill = egui::Color32::from_rgb(50, 110, 220);
    v.widgets.active.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    v.selection.bg_fill = egui::Color32::from_rgba_unmultiplied(50, 110, 220, 50);
    v.selection.stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(50, 110, 220));
    v.window_corner_radius = egui::CornerRadius::same(6);
    v.menu_corner_radius = egui::CornerRadius::same(6);
    v.widgets.noninteractive.corner_radius = egui::CornerRadius::same(5);
    v.widgets.inactive.corner_radius = egui::CornerRadius::same(5);
    v.widgets.hovered.corner_radius = egui::CornerRadius::same(5);
    v.widgets.active.corner_radius = egui::CornerRadius::same(5);
    v.widgets.open.corner_radius = egui::CornerRadius::same(5);
    v.hyperlink_color = egui::Color32::from_rgb(50, 110, 220);
    v.warn_fg_color = egui::Color32::from_rgb(230, 170, 0);
    v.error_fg_color = egui::Color32::from_rgb(211, 47, 47);
    v
}

/// Apply the project's typography scale to egui styles.
pub fn apply_catapult_typography(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::proportional(FONT_TITLE),
    );
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(FONT_BODY));
    style.text_styles.insert(
        egui::TextStyle::Button,
        egui::FontId::proportional(FONT_BODY),
    );
    style.text_styles.insert(
        egui::TextStyle::Small,
        egui::FontId::proportional(FONT_CAPTION),
    );
    style.text_styles.insert(
        egui::TextStyle::Monospace,
        egui::FontId::monospace(FONT_CAPTION),
    );
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    style.spacing.interact_size.y = 24.0;
    style.spacing.icon_spacing = 6.0;
    ctx.set_style(style);
}
