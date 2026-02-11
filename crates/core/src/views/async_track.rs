use flame_cat_protocol::{
    AsyncSpan, Point, Rect, RenderCommand, SharedStr, TextAlign, ThemeToken, Viewport,
};
use std::collections::HashMap;

const ROW_HEIGHT: f64 = 18.0;
const ROW_GAP: f64 = 2.0;
const FONT_SIZE: f64 = 10.0;
const MIN_LABEL_WIDTH: f64 = 30.0;

/// Render async spans as horizontal bars grouped by category.
///
/// Each category gets its own row. Spans within a category are laid out
/// in parallel rows to avoid overlapping (swimlane packing).
pub fn render_async_track(
    spans: &[AsyncSpan],
    viewport: &Viewport,
    view_start: f64,
    view_end: f64,
) -> Vec<RenderCommand> {
    let duration = view_end - view_start;
    if duration <= 0.0 || spans.is_empty() {
        return Vec::new();
    }

    let x_scale = viewport.width / duration;
    let mut commands = Vec::with_capacity(spans.len() * 2 + 4);

    commands.push(RenderCommand::BeginGroup {
        id: "async-track".into(),
        label: Some("Async Spans".into()),
    });

    // Background
    commands.push(RenderCommand::DrawRect {
        rect: Rect::new(0.0, 0.0, viewport.width, viewport.height),
        color: ThemeToken::LaneBackground,
        border_color: Some(ThemeToken::LaneBorder),
        label: None,
        frame_id: None,
    });

    // Group spans by category
    let mut groups: HashMap<SharedStr, Vec<&AsyncSpan>> = HashMap::new();
    for span in spans {
        if span.end < view_start || span.start > view_end {
            continue;
        }
        let key = span.cat.clone().unwrap_or_else(|| "uncategorized".into());
        groups.entry(key).or_default().push(span);
    }

    // Sort groups by name for stable ordering
    let mut group_keys: Vec<_> = groups.keys().cloned().collect();
    group_keys.sort();

    let mut current_y = 2.0;

    for key in &group_keys {
        let group_spans = groups.get(key).unwrap();

        // Category label
        commands.push(RenderCommand::DrawText {
            position: Point::new(4.0, current_y + FONT_SIZE),
            text: key.clone(),
            color: ThemeToken::TextMuted,
            font_size: FONT_SIZE - 1.0,
            align: TextAlign::Left,
        });
        current_y += FONT_SIZE + 2.0;

        // Swimlane packing: assign each span to the first row where it fits
        let mut row_ends: Vec<f64> = Vec::new();
        let mut assignments: Vec<(usize, &AsyncSpan)> = Vec::new();

        // Sort by start time
        let mut sorted: Vec<&AsyncSpan> = group_spans.clone();
        sorted.sort_by(|a, b| a.start.total_cmp(&b.start));

        for span in sorted {
            let mut placed = false;
            for (row_idx, row_end) in row_ends.iter_mut().enumerate() {
                if span.start >= *row_end {
                    *row_end = span.end;
                    assignments.push((row_idx, span));
                    placed = true;
                    break;
                }
            }
            if !placed {
                row_ends.push(span.end);
                assignments.push((row_ends.len() - 1, span));
            }
        }

        // Render spans
        for (row, span) in &assignments {
            let x = (span.start - view_start) * x_scale;
            let w = (span.end - span.start) * x_scale;
            let y = current_y + *row as f64 * (ROW_HEIGHT + ROW_GAP);

            if y + ROW_HEIGHT > viewport.height {
                continue;
            }

            let clamped_x = x.max(0.0);
            let clamped_w = (x + w).min(viewport.width) - clamped_x;

            if clamped_w < 0.5 {
                continue;
            }

            commands.push(RenderCommand::DrawRect {
                rect: Rect::new(clamped_x, y, clamped_w, ROW_HEIGHT),
                color: ThemeToken::AsyncSpanFill,
                border_color: Some(ThemeToken::AsyncSpanBorder),
                label: Some(span.name.clone()),
                frame_id: None,
            });

            // Label if wide enough
            if clamped_w > MIN_LABEL_WIDTH {
                commands.push(RenderCommand::DrawText {
                    position: Point::new(clamped_x + 3.0, y + ROW_HEIGHT / 2.0 + 4.0),
                    text: span.name.clone(),
                    color: ThemeToken::TextPrimary,
                    font_size: FONT_SIZE,
                    align: TextAlign::Left,
                });
            }
        }

        let rows_used = row_ends.len().max(1);
        current_y += rows_used as f64 * (ROW_HEIGHT + ROW_GAP) + 4.0;
    }

    commands.push(RenderCommand::EndGroup);
    commands
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_async_spans() {
        let spans = vec![
            AsyncSpan {
                id: "1".into(),
                name: "PipelineReporter".into(),
                cat: Some("benchmark".into()),
                start: 10.0,
                end: 50.0,
                pid: 1,
                tid: 1,
            },
            AsyncSpan {
                id: "2".into(),
                name: "PipelineReporter".into(),
                cat: Some("benchmark".into()),
                start: 30.0,
                end: 80.0,
                pid: 1,
                tid: 1,
            },
        ];
        let vp = Viewport {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 100.0,
            dpr: 1.0,
        };
        let cmds = render_async_track(&spans, &vp, 0.0, 100.0);
        assert!(!cmds.is_empty());

        let rects: Vec<_> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCommand::DrawRect { .. }))
            .collect();
        // bg + 2 span rects
        assert!(rects.len() >= 3);
    }

    #[test]
    fn empty_spans_returns_empty() {
        let vp = Viewport {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 100.0,
            dpr: 1.0,
        };
        let cmds = render_async_track(&[], &vp, 0.0, 100.0);
        assert!(cmds.is_empty());
    }
}
