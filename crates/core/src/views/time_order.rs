use flame_cat_protocol::{
    Point, Rect, RenderCommand, SharedStr, TextAlign, ThemeToken, Viewport, VisualProfile,
};

const FRAME_HEIGHT: f64 = 20.0;
const THREAD_HEADER_HEIGHT: f64 = 22.0;
const THREAD_GAP: f64 = 4.0;

/// Render a profile in time-order view: frames are laid out chronologically,
/// X-axis = wall time, Y-axis = stack depth.
///
/// `view_start` / `view_end` define the visible time window (absolute µs).
/// The canvas pixel width comes from `viewport.width`.
///
/// When `thread_id` is `Some(id)`, only the matching thread group is rendered.
pub fn render_time_order(
    profile: &VisualProfile,
    viewport: &Viewport,
    view_start: f64,
    view_end: f64,
    thread_id: Option<u32>,
) -> Vec<RenderCommand> {
    let visible_duration = view_end - view_start;
    if visible_duration <= 0.0 {
        return Vec::new();
    }

    let x_scale = viewport.width / visible_duration;

    let mut commands = Vec::with_capacity(profile.span_count() + 2);

    commands.push(RenderCommand::BeginGroup {
        id: "time-order".into(),
        label: Some("Time Order".into()),
    });

    let mut y_offset: f64 = 0.0;

    for thread in &profile.threads {
        // Skip threads not matching the filter
        if thread_id.is_some_and(|tid| thread.id != tid) {
            continue;
        }

        // Thread header (skip when rendering a single thread — the caller provides the header)
        if thread_id.is_none() {
            let header_y = y_offset - viewport.y;
            if header_y + THREAD_HEADER_HEIGHT >= 0.0 && header_y <= viewport.height {
                commands.push(RenderCommand::DrawRect {
                    rect: Rect::new(0.0, header_y, viewport.width, THREAD_HEADER_HEIGHT - 1.0),
                    color: ThemeToken::LaneHeaderBackground,
                    border_color: Some(ThemeToken::LaneBorder),
                    label: None,
                    frame_id: None,
                });
                commands.push(RenderCommand::DrawText {
                    position: Point {
                        x: 6.0,
                        y: header_y + THREAD_HEADER_HEIGHT / 2.0,
                    },
                    text: SharedStr::from(format!(
                        "{} ({} spans)",
                        thread.name,
                        thread.spans.len()
                    )),
                    color: ThemeToken::LaneHeaderText,
                    font_size: 11.0,
                    align: TextAlign::Left,
                });
            }
            y_offset += THREAD_HEADER_HEIGHT;
        }

        // Use cached max_depth (computed at parse time)
        let max_depth = thread.max_depth;

        for span in &thread.spans {
            let x = (span.start - view_start) * x_scale;
            let w = span.duration() * x_scale;
            let y = y_offset + f64::from(span.depth) * FRAME_HEIGHT - viewport.y;

            // Skip frames outside the visible area
            if x + w < 0.0 || x > viewport.width {
                continue;
            }
            if y + FRAME_HEIGHT < 0.0 || y > viewport.height {
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

        y_offset += f64::from(max_depth + 1) * FRAME_HEIGHT + THREAD_GAP;
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
                time_domain: None,
            },
            threads: vec![ThreadGroup {
                id: 0,
                name: "Main".into(),
                sort_key: 0,
                max_depth: 0,
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
            frames: vec![],
            counters: vec![],
            async_spans: vec![],
            flow_arrows: vec![],
            markers: vec![],
            instant_events: vec![],
            object_events: vec![],
            cpu_samples: None,
            network_requests: vec![],
            screenshots: vec![],
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
        let profile = test_profile();
        let cmds = render_time_order(
            &profile,
            &vp,
            profile.meta.start_time,
            profile.meta.end_time,
            None,
        );
        let rects: Vec<_> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCommand::DrawRect { frame_id, .. } if frame_id.is_some()))
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
                time_domain: None,
            },
            threads: vec![],
            frames: vec![],
            counters: vec![],
            async_spans: vec![],
            flow_arrows: vec![],
            markers: vec![],
            instant_events: vec![],
            object_events: vec![],
            cpu_samples: None,
            network_requests: vec![],
            screenshots: vec![],
        };
        let vp = Viewport {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
            dpr: 1.0,
        };
        assert!(render_time_order(&profile, &vp, 0.0, 0.0, None).is_empty());
    }
}
