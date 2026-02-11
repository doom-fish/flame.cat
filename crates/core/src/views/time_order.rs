use flame_cat_protocol::{Rect, RenderCommand, ThemeToken, Viewport, VisualProfile};

const FRAME_HEIGHT: f64 = 20.0;

/// Render a profile in time-order view: frames are laid out chronologically,
/// X-axis = wall time, Y-axis = stack depth.
pub fn render_time_order(profile: &VisualProfile, viewport: &Viewport) -> Vec<RenderCommand> {
    let duration = profile.duration();
    if duration <= 0.0 {
        return Vec::new();
    }

    let start = profile.meta.start_time;
    let x_scale = viewport.width / duration;

    let mut commands = Vec::with_capacity(profile.span_count() + 2);

    commands.push(RenderCommand::BeginGroup {
        id: "time-order".to_string(),
        label: Some("Time Order".to_string()),
    });

    for span in profile.all_spans() {
        let x = (span.start - start) * x_scale;
        let w = span.duration() * x_scale;
        let y = f64::from(span.depth) * FRAME_HEIGHT;

        // Skip frames outside the viewport
        if x + w < viewport.x || x > viewport.x + viewport.width {
            continue;
        }
        if y + FRAME_HEIGHT < viewport.y || y > viewport.y + viewport.height {
            continue;
        }

        // Skip sub-pixel frames
        if w < 0.5 {
            continue;
        }

        let color = color_for_depth(span.depth);

        commands.push(RenderCommand::DrawRect {
            rect: Rect::new(x, y, w, FRAME_HEIGHT - 1.0),
            color,
            border_color: Some(ThemeToken::Border),
            label: Some(span.name.clone()),
            frame_id: Some(span.id),
        });
    }

    commands.push(RenderCommand::EndGroup);
    commands
}

fn color_for_depth(depth: u32) -> ThemeToken {
    match depth % 4 {
        0 => ThemeToken::FlameHot,
        1 => ThemeToken::FlameWarm,
        2 => ThemeToken::FlameCold,
        _ => ThemeToken::FlameNeutral,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flame_cat_protocol::{ProfileMeta, SourceFormat, Span, SpanKind, ThreadGroup, ValueUnit};

    fn test_profile() -> VisualProfile {
        VisualProfile {
            meta: ProfileMeta {
                name: None,
                source_format: SourceFormat::Unknown,
                value_unit: ValueUnit::Microseconds,
                total_value: 100.0,
                start_time: 0.0,
                end_time: 100.0,
            },
            threads: vec![ThreadGroup {
                id: 0,
                name: "Main".into(),
                sort_key: 0,
                spans: vec![
                    Span {
                        id: 0,
                        name: "main".into(),
                        start: 0.0,
                        end: 100.0,
                        depth: 0,
                        parent: None,
                        self_value: 50.0,
                        kind: SpanKind::Event,
                        category: None,
                    },
                    Span {
                        id: 1,
                        name: "child".into(),
                        start: 10.0,
                        end: 60.0,
                        depth: 1,
                        parent: Some(0),
                        self_value: 50.0,
                        kind: SpanKind::Event,
                        category: None,
                    },
                ],
            }],
        }
    }

    #[test]
    fn produces_draw_rects() {
        let vp = Viewport {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
            dpr: 1.0,
        };
        let cmds = render_time_order(&test_profile(), &vp);
        let rects: Vec<_> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCommand::DrawRect { .. }))
            .collect();
        assert_eq!(rects.len(), 2);
    }

    #[test]
    fn empty_profile() {
        let profile = VisualProfile {
            meta: ProfileMeta {
                name: None,
                source_format: SourceFormat::Unknown,
                value_unit: ValueUnit::Microseconds,
                total_value: 0.0,
                start_time: 0.0,
                end_time: 0.0,
            },
            threads: vec![],
        };
        let vp = Viewport {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
            dpr: 1.0,
        };
        assert!(render_time_order(&profile, &vp).is_empty());
    }
}
