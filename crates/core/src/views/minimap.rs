use flame_cat_protocol::{Rect, RenderCommand, ThemeToken, Viewport, VisualProfile};

const MINIMAP_FRAME_HEIGHT: f64 = 3.0;

/// Render a minimap overview of the entire profile. The minimap shows all
/// spans compressed to fit the viewport width, with a viewport indicator
/// overlay showing the currently visible region.
pub fn render_minimap(
    profile: &VisualProfile,
    viewport: &Viewport,
    visible_start_frac: f64,
    visible_end_frac: f64,
) -> Vec<RenderCommand> {
    let duration = profile.duration();
    if duration <= 0.0 {
        return Vec::new();
    }

    let start = profile.meta.start_time;
    let x_scale = viewport.width / duration;

    let mut commands = Vec::with_capacity(profile.span_count() + 4);
    commands.push(RenderCommand::BeginGroup {
        id: "minimap".into(),
        label: Some("Minimap".into()),
    });

    // Background
    commands.push(RenderCommand::DrawRect {
        rect: Rect::new(0.0, 0.0, viewport.width, viewport.height),
        color: ThemeToken::MinimapBackground,
        border_color: None,
        label: None,
        frame_id: None,
    });

    // Draw compressed spans
    for span in profile.all_spans() {
        let x = (span.start - start) * x_scale;
        let w = span.duration() * x_scale;
        let y = f64::from(span.depth) * MINIMAP_FRAME_HEIGHT;

        if w < 0.3 || y + MINIMAP_FRAME_HEIGHT > viewport.height {
            continue;
        }

        let color = match span.depth % 4 {
            0 => ThemeToken::FlameHot,
            1 => ThemeToken::FlameWarm,
            2 => ThemeToken::FlameCold,
            _ => ThemeToken::FlameNeutral,
        };

        commands.push(RenderCommand::DrawRect {
            rect: Rect::new(x, y, w, MINIMAP_FRAME_HEIGHT),
            color,
            border_color: None,
            label: None,
            frame_id: None,
        });
    }

    // Viewport indicator overlay
    let vp_x = visible_start_frac * viewport.width;
    let vp_w = (visible_end_frac - visible_start_frac) * viewport.width;
    commands.push(RenderCommand::DrawRect {
        rect: Rect::new(vp_x, 0.0, vp_w, viewport.height),
        color: ThemeToken::MinimapViewport,
        border_color: Some(ThemeToken::Border),
        label: None,
        frame_id: None,
    });

    commands.push(RenderCommand::EndGroup);
    commands
}

#[cfg(test)]
mod tests {
    use super::*;
    use flame_cat_protocol::{ProfileMeta, SourceFormat, Span, SpanKind, ThreadGroup, ValueUnit};

    #[test]
    fn renders_minimap_with_viewport() {
        let profile = VisualProfile {
            meta: ProfileMeta {
                name: None,
                source_format: SourceFormat::Unknown,
                value_unit: ValueUnit::Microseconds,
                total_value: 100.0,
                start_time: 0.0,
                end_time: 100.0,
                time_domain: None,
            },
            threads: vec![ThreadGroup {
                id: 0,
                name: "Main".into(),
                sort_key: 0,
                spans: vec![Span {
                    id: 0,
                    name: "main".into(),
                    start: 0.0,
                    end: 100.0,
                    depth: 0,
                    parent: None,
                    self_value: 100.0,
                    kind: SpanKind::Event,
                    category: None,
                }],
            }],
            frames: vec![],
            counters: vec![],
            async_spans: vec![],
            flow_arrows: vec![],
            markers: vec![],
            instant_events: vec![],
            object_events: vec![],
            cpu_samples: None,
        };
        let vp = Viewport {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 40.0,
            dpr: 1.0,
        };
        let cmds = render_minimap(&profile, &vp, 0.0, 0.5);
        let rects: Vec<_> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCommand::DrawRect { .. }))
            .collect();
        // Background + frame + viewport indicator
        assert!(rects.len() >= 3);
    }
}
