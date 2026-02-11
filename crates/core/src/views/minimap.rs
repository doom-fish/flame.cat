use flame_cat_protocol::{Point, Rect, RenderCommand, ThemeToken, Viewport, VisualProfile};

const ROW_HEIGHT: f64 = 4.0;
const CELL_WIDTH: f64 = 4.0;
const HANDLE_WIDTH: f64 = 6.0;

/// Render a density heatmap minimap of the entire profile.
///
/// Instead of drawing individual spans, this buckets spans into cells
/// and uses alpha intensity to show load. Much faster for large profiles.
/// Includes a viewport indicator with draggable edge handles.
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
    let cols = (viewport.width / CELL_WIDTH).ceil() as usize;
    let max_rows = (viewport.height / ROW_HEIGHT).floor() as usize;

    let mut commands = Vec::with_capacity(cols * max_rows / 2 + 10);
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

    // Build density grid: rows = depth, cols = time buckets
    // Each cell counts how many spans overlap it
    let mut grid = vec![0u16; cols * max_rows];
    let col_duration = duration / cols as f64;

    for span in profile.all_spans() {
        let row = span.depth as usize;
        if row >= max_rows {
            continue;
        }
        let col_start = ((span.start - start) / col_duration) as usize;
        let col_end = ((span.end - start) / col_duration).ceil() as usize;
        let col_start = col_start.min(cols);
        let col_end = col_end.min(cols);
        for c in col_start..col_end {
            grid[row * cols + c] = grid[row * cols + c].saturating_add(1);
        }
    }

    // Find max density for normalization
    let max_density = grid.iter().copied().max().unwrap_or(1).max(1);

    // Render density cells
    let colors = [
        ThemeToken::FlameHot,
        ThemeToken::FlameWarm,
        ThemeToken::FlameCold,
        ThemeToken::FlameNeutral,
    ];

    for row in 0..max_rows {
        let y = row as f64 * ROW_HEIGHT;
        if y >= viewport.height {
            break;
        }
        let color = colors[row % 4];

        // Merge adjacent cells with same density range for fewer draw calls
        let mut c = 0;
        while c < cols {
            let density = grid[row * cols + c];
            if density == 0 {
                c += 1;
                continue;
            }

            // Find run of non-zero cells
            let run_start = c;
            while c < cols && grid[row * cols + c] > 0 {
                c += 1;
            }

            // Use average density for the run
            let run_len = c - run_start;
            let avg_density: f64 = grid[row * cols + run_start..row * cols + c]
                .iter()
                .map(|&d| d as f64)
                .sum::<f64>()
                / run_len as f64;

            let alpha = 0.2 + 0.8 * (avg_density / max_density as f64);
            let x = run_start as f64 * CELL_WIDTH;
            let w = run_len as f64 * CELL_WIDTH;

            // DrawRect with the color — we rely on the theme token + will add
            // alpha via an overlay approach. For now, use the color directly
            // with higher density = solid color.
            commands.push(RenderCommand::DrawRect {
                rect: Rect::new(x, y, w, ROW_HEIGHT),
                color: if alpha > 0.7 {
                    color
                } else if alpha > 0.4 {
                    ThemeToken::FlameNeutral
                } else {
                    ThemeToken::MinimapBackground
                },
                border_color: None,
                label: None,
                frame_id: None,
            });

            // If high density, overlay with the hot color
            if alpha > 0.4 {
                commands.push(RenderCommand::DrawRect {
                    rect: Rect::new(x, y, w, ROW_HEIGHT),
                    color,
                    border_color: None,
                    label: None,
                    frame_id: None,
                });
            }
        }
    }

    // Semi-opaque overlay on non-visible regions
    let vp_x = visible_start_frac * viewport.width;
    let vp_w = (visible_end_frac - visible_start_frac) * viewport.width;

    // Left dim overlay
    if vp_x > 0.0 {
        commands.push(RenderCommand::DrawRect {
            rect: Rect::new(0.0, 0.0, vp_x, viewport.height),
            color: ThemeToken::MinimapBackground,
            border_color: None,
            label: None,
            frame_id: None,
        });
    }

    // Right dim overlay
    let right_x = vp_x + vp_w;
    if right_x < viewport.width {
        commands.push(RenderCommand::DrawRect {
            rect: Rect::new(right_x, 0.0, viewport.width - right_x, viewport.height),
            color: ThemeToken::MinimapBackground,
            border_color: None,
            label: None,
            frame_id: None,
        });
    }

    // Viewport border
    commands.push(RenderCommand::DrawRect {
        rect: Rect::new(vp_x, 0.0, vp_w, viewport.height),
        color: ThemeToken::MinimapViewport,
        border_color: Some(ThemeToken::Border),
        label: None,
        frame_id: None,
    });

    // Left handle
    commands.push(RenderCommand::DrawLine {
        from: Point::new(vp_x, 0.0),
        to: Point::new(vp_x, viewport.height),
        color: ThemeToken::Border,
        width: HANDLE_WIDTH,
    });

    // Right handle
    commands.push(RenderCommand::DrawLine {
        from: Point::new(vp_x + vp_w, 0.0),
        to: Point::new(vp_x + vp_w, viewport.height),
        color: ThemeToken::Border,
        width: HANDLE_WIDTH,
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
            network_requests: vec![],
            screenshots: vec![],
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
