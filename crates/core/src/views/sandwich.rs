use flame_cat_protocol::{Rect, RenderCommand, ThemeToken, Viewport, VisualProfile};

const FRAME_HEIGHT: f64 = 20.0;
const SEPARATOR_HEIGHT: f64 = 4.0;

/// Render a sandwich view: for a selected frame, show callers above and
/// callees below, each as a mini left-heavy view.
pub fn render_sandwich(
    profile: &VisualProfile,
    selected_frame_id: u64,
    viewport: &Viewport,
) -> Vec<RenderCommand> {
    let mut commands = Vec::new();
    commands.push(RenderCommand::BeginGroup {
        id: "sandwich".to_string(),
        label: Some("Sandwich".to_string()),
    });

    // Find all spans matching the selected name.
    let selected_name = match profile.span(selected_frame_id) {
        Some(s) => s.name.clone(),
        None => {
            commands.push(RenderCommand::EndGroup);
            return commands;
        }
    };

    let matching: Vec<_> = profile
        .all_spans()
        .filter(|s| s.name == selected_name)
        .collect();

    if matching.is_empty() {
        commands.push(RenderCommand::EndGroup);
        return commands;
    }

    let total_time: f64 = matching.iter().map(|s| s.duration()).sum();
    let x_scale = viewport.width / total_time.max(1.0);

    // === Callers section (walk upward) ===
    let caller_y_base = 0.0;

    let mut caller_time: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    for m in &matching {
        let mut current = m.parent;
        while let Some(pid) = current {
            if let Some(parent_span) = profile.span(pid) {
                *caller_time.entry(parent_span.name.clone()).or_default() += m.duration();
                current = parent_span.parent;
            } else {
                break;
            }
        }
    }

    let mut callers: Vec<_> = caller_time.into_iter().collect();
    callers.sort_by(|a, b| b.1.total_cmp(&a.1));

    for (i, (name, time)) in callers.iter().enumerate() {
        let w = time * x_scale;
        if w < 0.5 {
            continue;
        }
        commands.push(RenderCommand::DrawRect {
            rect: Rect::new(
                0.0,
                caller_y_base + (i as f64) * FRAME_HEIGHT,
                w,
                FRAME_HEIGHT - 1.0,
            ),
            color: ThemeToken::FlameCold,
            border_color: Some(ThemeToken::Border),
            label: Some(name.clone()),
            frame_id: None,
        });
    }

    // === Selected frame (center) ===
    let center_y = caller_y_base + (callers.len() as f64) * FRAME_HEIGHT + SEPARATOR_HEIGHT;

    commands.push(RenderCommand::DrawRect {
        rect: Rect::new(0.0, center_y, viewport.width, FRAME_HEIGHT - 1.0),
        color: ThemeToken::SelectionHighlight,
        border_color: Some(ThemeToken::Border),
        label: Some(selected_name.clone()),
        frame_id: Some(selected_frame_id),
    });

    // === Callees section (walk downward) ===
    let callee_y_base = center_y + FRAME_HEIGHT + SEPARATOR_HEIGHT;

    let mut callee_time: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    for m in &matching {
        for child in profile.children(Some(m.id)) {
            *callee_time.entry(child.name.clone()).or_default() += child.duration();
        }
    }

    let mut callees: Vec<_> = callee_time.into_iter().collect();
    callees.sort_by(|a, b| b.1.total_cmp(&a.1));

    for (i, (name, time)) in callees.iter().enumerate() {
        let w = time * x_scale;
        if w < 0.5 {
            continue;
        }
        commands.push(RenderCommand::DrawRect {
            rect: Rect::new(
                0.0,
                callee_y_base + (i as f64) * FRAME_HEIGHT,
                w,
                FRAME_HEIGHT - 1.0,
            ),
            color: ThemeToken::FlameWarm,
            border_color: Some(ThemeToken::Border),
            label: Some(name.clone()),
            frame_id: None,
        });
    }

    commands.push(RenderCommand::EndGroup);
    commands
}

#[cfg(test)]
mod tests {
    use super::*;
    use flame_cat_protocol::{ProfileMeta, SourceFormat, Span, SpanKind, ThreadGroup, ValueUnit};

    #[test]
    fn shows_callers_and_callees() {
        let profile = VisualProfile {
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
                        name: "root".into(),
                        start: 0.0,
                        end: 100.0,
                        depth: 0,
                        parent: None,
                        self_value: 0.0,
                        kind: SpanKind::Event,
                        category: None,
                    },
                    Span {
                        id: 1,
                        name: "middle".into(),
                        start: 0.0,
                        end: 100.0,
                        depth: 1,
                        parent: Some(0),
                        self_value: 0.0,
                        kind: SpanKind::Event,
                        category: None,
                    },
                    Span {
                        id: 2,
                        name: "leaf".into(),
                        start: 0.0,
                        end: 60.0,
                        depth: 2,
                        parent: Some(1),
                        self_value: 60.0,
                        kind: SpanKind::Event,
                        category: None,
                    },
                ],
            }],
        };
        let vp = Viewport {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
            dpr: 1.0,
        };

        // Select "middle" — should show "root" as caller, "leaf" as callee.
        let cmds = render_sandwich(&profile, 1, &vp);
        let rects: Vec<_> = cmds
            .iter()
            .filter_map(|c| {
                if let RenderCommand::DrawRect { label, .. } = c {
                    label.clone()
                } else {
                    None
                }
            })
            .collect();

        assert!(
            rects.contains(&"root".to_string()),
            "should have caller 'root'"
        );
        assert!(
            rects.contains(&"middle".to_string()),
            "should have selected 'middle'"
        );
        assert!(
            rects.contains(&"leaf".to_string()),
            "should have callee 'leaf'"
        );
    }

    #[test]
    fn nonexistent_frame_returns_group_only() {
        let profile = VisualProfile {
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
                spans: vec![Span {
                    id: 0,
                    name: "only".into(),
                    start: 0.0,
                    end: 100.0,
                    depth: 0,
                    parent: None,
                    self_value: 100.0,
                    kind: SpanKind::Event,
                    category: None,
                }],
            }],
        };
        let vp = Viewport {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
            dpr: 1.0,
        };
        // Non-existent frame id — should return only BeginGroup + EndGroup
        let cmds = render_sandwich(&profile, 999, &vp);
        assert_eq!(cmds.len(), 2);
        assert!(matches!(cmds[0], RenderCommand::BeginGroup { .. }));
        assert!(matches!(cmds[1], RenderCommand::EndGroup));
    }
}
