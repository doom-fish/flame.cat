use flame_cat_protocol::{
    ObjectEvent, ObjectPhase, Rect, RenderCommand, SharedStr, ThemeToken, Viewport,
};
use std::collections::HashMap;

const ROW_HEIGHT: f64 = 14.0;
const ROW_GAP: f64 = 2.0;
const SNAPSHOT_MARKER_R: f64 = 3.0;

/// Render object lifecycle events as horizontal bars from create→destroy.
///
/// Objects are grouped by name, then packed in swimlanes.
/// Snapshot events are rendered as small markers on the bar.
pub fn render_object_track(
    events: &[ObjectEvent],
    viewport: &Viewport,
    view_start: f64,
    view_end: f64,
) -> Vec<RenderCommand> {
    let duration = view_end - view_start;
    if duration <= 0.0 || events.is_empty() {
        return Vec::new();
    }

    // Group events by object id
    struct ObjLife {
        name: SharedStr,
        create_ts: Option<f64>,
        destroy_ts: Option<f64>,
        snapshots: Vec<f64>,
    }

    let mut objects: HashMap<&str, ObjLife> = HashMap::new();
    for ev in events {
        let entry = objects.entry(ev.id.as_ref()).or_insert_with(|| ObjLife {
            name: ev.name.clone(),
            create_ts: None,
            destroy_ts: None,
            snapshots: Vec::new(),
        });
        match ev.phase {
            ObjectPhase::Create => entry.create_ts = Some(ev.ts),
            ObjectPhase::Snapshot => entry.snapshots.push(ev.ts),
            ObjectPhase::Destroy => entry.destroy_ts = Some(ev.ts),
        }
    }

    // Convert to sorted list
    let mut lives: Vec<(&str, ObjLife)> = objects.into_iter().collect();
    lives.sort_by(|a, b| {
        let a_start = a.1.create_ts.unwrap_or(view_start);
        let b_start = b.1.create_ts.unwrap_or(view_start);
        a_start.total_cmp(&b_start)
    });

    let x_scale = viewport.width / duration;
    let mut commands = Vec::with_capacity(lives.len() * 3 + 4);

    commands.push(RenderCommand::BeginGroup {
        id: "object-track".into(),
        label: Some("Object Lifecycle".into()),
    });

    // Background
    commands.push(RenderCommand::DrawRect {
        rect: Rect::new(0.0, 0.0, viewport.width, viewport.height),
        color: ThemeToken::LaneBackground,
        border_color: Some(ThemeToken::LaneBorder),
        label: None,
        frame_id: None,
    });

    // Swimlane packing
    let mut row_ends: Vec<f64> = Vec::new();

    for (_id, life) in &lives {
        let start = life.create_ts.unwrap_or(view_start);
        let end = life.destroy_ts.unwrap_or(view_end);

        if end < view_start || start > view_end {
            continue;
        }

        // Find row
        let mut row = 0;
        for (ri, re) in row_ends.iter_mut().enumerate() {
            if start >= *re {
                *re = end;
                row = ri;
                break;
            }
            row = ri + 1;
        }
        if row >= row_ends.len() {
            row_ends.push(end);
        }

        let y = row as f64 * (ROW_HEIGHT + ROW_GAP);
        if y + ROW_HEIGHT > viewport.height {
            continue;
        }

        let x = ((start - view_start) * x_scale).max(0.0);
        let x_end = ((end - view_start) * x_scale).min(viewport.width);
        let w = (x_end - x).max(2.0);

        // Bar
        commands.push(RenderCommand::DrawRect {
            rect: Rect::new(x, y, w, ROW_HEIGHT),
            color: ThemeToken::AsyncSpanFill,
            border_color: Some(ThemeToken::AsyncSpanBorder),
            label: Some(life.name.clone()),
            frame_id: None,
        });

        // Snapshot markers
        for &snap_ts in &life.snapshots {
            if snap_ts < view_start || snap_ts > view_end {
                continue;
            }
            let sx = (snap_ts - view_start) * x_scale;
            commands.push(RenderCommand::DrawRect {
                rect: Rect::new(
                    sx - SNAPSHOT_MARKER_R,
                    y + ROW_HEIGHT / 2.0 - SNAPSHOT_MARKER_R,
                    SNAPSHOT_MARKER_R * 2.0,
                    SNAPSHOT_MARKER_R * 2.0,
                ),
                color: ThemeToken::MarkerLine,
                border_color: None,
                label: None,
                frame_id: None,
            });
        }
    }

    commands.push(RenderCommand::EndGroup);
    commands
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_object_lifecycle() {
        let events = vec![
            ObjectEvent {
                id: "obj1".into(),
                name: "Widget".into(),
                phase: ObjectPhase::Create,
                ts: 10.0,
            },
            ObjectEvent {
                id: "obj1".into(),
                name: "Widget".into(),
                phase: ObjectPhase::Snapshot,
                ts: 30.0,
            },
            ObjectEvent {
                id: "obj1".into(),
                name: "Widget".into(),
                phase: ObjectPhase::Destroy,
                ts: 50.0,
            },
        ];
        let vp = Viewport {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 100.0,
            dpr: 1.0,
        };
        let cmds = render_object_track(&events, &vp, 0.0, 100.0);
        assert!(!cmds.is_empty());

        let rects: Vec<_> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCommand::DrawRect { .. }))
            .collect();
        // bg + bar + snapshot marker = 3
        assert!(rects.len() >= 3);
    }

    #[test]
    fn empty_events_returns_empty() {
        let vp = Viewport {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 100.0,
            dpr: 1.0,
        };
        let cmds = render_object_track(&[], &vp, 0.0, 100.0);
        assert!(cmds.is_empty());
    }
}
