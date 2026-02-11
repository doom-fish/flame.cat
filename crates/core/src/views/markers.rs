use flame_cat_protocol::{
    Marker, Point, RenderCommand, SharedStr, TextAlign, ThemeToken, Viewport,
};

const FONT_SIZE: f64 = 10.0;
const LABEL_OFFSET_Y: f64 = 12.0;

/// Render navigation/user timing markers as vertical lines across the viewport.
///
/// Markers are rendered as thin vertical lines spanning the full viewport height,
/// with rotated name labels at the top.
pub fn render_markers(
    markers: &[Marker],
    viewport: &Viewport,
    view_start: f64,
    view_end: f64,
) -> Vec<RenderCommand> {
    let duration = view_end - view_start;
    if duration <= 0.0 || markers.is_empty() {
        return Vec::new();
    }

    let x_scale = viewport.width / duration;
    let mut commands = Vec::with_capacity(markers.len() * 3 + 2);

    commands.push(RenderCommand::BeginGroup {
        id: "markers".into(),
        label: Some("Markers".into()),
    });

    // Track label positions to avoid overlap
    let mut last_label_x = f64::NEG_INFINITY;

    for marker in markers {
        if marker.ts < view_start || marker.ts > view_end {
            continue;
        }

        let x = (marker.ts - view_start) * x_scale;

        // Vertical line
        commands.push(RenderCommand::DrawLine {
            from: Point::new(x, 0.0),
            to: Point::new(x, viewport.height),
            color: ThemeToken::MarkerLine,
            width: 1.0,
        });

        // Label (skip if too close to previous)
        if x - last_label_x > 60.0 {
            commands.push(RenderCommand::DrawText {
                position: Point::new(x + 2.0, LABEL_OFFSET_Y),
                text: marker.name.clone(),
                color: ThemeToken::MarkerText,
                font_size: FONT_SIZE,
                align: TextAlign::Left,
            });
            last_label_x = x;
        }
    }

    commands.push(RenderCommand::EndGroup);
    commands
}

/// Render markers into the minimap overlay.
pub fn render_markers_minimap(
    markers: &[Marker],
    viewport: &Viewport,
    profile_start: f64,
    profile_duration: f64,
) -> Vec<RenderCommand> {
    if profile_duration <= 0.0 || markers.is_empty() {
        return Vec::new();
    }

    let x_scale = viewport.width / profile_duration;
    let mut commands = Vec::with_capacity(markers.len());

    for marker in markers {
        let x = (marker.ts - profile_start) * x_scale;
        if x < 0.0 || x > viewport.width {
            continue;
        }

        commands.push(RenderCommand::DrawLine {
            from: Point::new(x, 0.0),
            to: Point::new(x, viewport.height),
            color: ThemeToken::MarkerLine,
            width: 1.0,
        });
    }

    commands
}

#[cfg(test)]
mod tests {
    use super::*;
    use flame_cat_protocol::MarkerScope;

    fn sample_markers() -> Vec<Marker> {
        vec![
            Marker {
                ts: 100.0,
                name: SharedStr::from("navigationStart"),
                scope: MarkerScope::Global,
                category: None,
            },
            Marker {
                ts: 500.0,
                name: SharedStr::from("domInteractive"),
                scope: MarkerScope::Global,
                category: None,
            },
            Marker {
                ts: 1000.0,
                name: SharedStr::from("loadEventEnd"),
                scope: MarkerScope::Global,
                category: None,
            },
        ]
    }

    #[test]
    fn renders_visible_markers() {
        let markers = sample_markers();
        let vp = Viewport {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
            dpr: 1.0,
        };
        let cmds = render_markers(&markers, &vp, 0.0, 1100.0);
        let lines: Vec<_> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCommand::DrawLine { .. }))
            .collect();
        assert_eq!(lines.len(), 3);

        let texts: Vec<_> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCommand::DrawText { .. }))
            .collect();
        assert_eq!(texts.len(), 3);
    }

    #[test]
    fn filters_out_of_range_markers() {
        let markers = sample_markers();
        let vp = Viewport {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
            dpr: 1.0,
        };
        // Only 100 and 500 are in range
        let cmds = render_markers(&markers, &vp, 0.0, 600.0);
        let lines: Vec<_> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCommand::DrawLine { .. }))
            .collect();
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn empty_markers_returns_empty() {
        let vp = Viewport {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
            dpr: 1.0,
        };
        let cmds = render_markers(&[], &vp, 0.0, 100.0);
        assert!(cmds.is_empty());
    }
}
