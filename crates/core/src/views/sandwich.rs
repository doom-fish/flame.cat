use flame_cat_protocol::{Rect, RenderCommand, ThemeToken, Viewport};

use crate::model::Profile;

const FRAME_HEIGHT: f64 = 20.0;
const SEPARATOR_HEIGHT: f64 = 4.0;

/// Render a sandwich view: for a selected frame, show callers above and
/// callees below, each as a mini left-heavy view.
pub fn render_sandwich(
    profile: &Profile,
    selected_frame_id: u64,
    viewport: &Viewport,
) -> Vec<RenderCommand> {
    let mut commands = Vec::new();
    commands.push(RenderCommand::BeginGroup {
        id: "sandwich".to_string(),
        label: Some("Sandwich".to_string()),
    });

    // Find all frames matching the selected name.
    let selected_name = match profile.frame(selected_frame_id) {
        Some(f) => f.name.clone(),
        None => {
            commands.push(RenderCommand::EndGroup);
            return commands;
        }
    };

    let matching: Vec<_> = profile
        .frames
        .iter()
        .filter(|f| f.name == selected_name)
        .collect();

    if matching.is_empty() {
        commands.push(RenderCommand::EndGroup);
        return commands;
    }

    let total_time: f64 = matching.iter().map(|f| f.duration()).sum();
    let x_scale = viewport.width / total_time.max(1.0);

    // === Callers section (walk upward) ===
    let caller_y_base = 0.0;

    // Collect caller chains.
    let mut caller_time: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    for m in &matching {
        let mut current = m.parent;
        while let Some(pid) = current {
            if let Some(parent_frame) = profile.frame(pid) {
                *caller_time.entry(parent_frame.name.clone()).or_default() += m.duration();
                current = parent_frame.parent;
            } else {
                break;
            }
        }
    }

    let mut callers: Vec<_> = caller_time.into_iter().collect();
    callers.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

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
    callees.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

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
    use crate::model::{Frame, ProfileMetadata};

    #[test]
    fn shows_callers_and_callees() {
        let profile = Profile {
            metadata: ProfileMetadata {
                name: None,
                start_time: 0.0,
                end_time: 100.0,
                format: "test".to_string(),
            },
            frames: vec![
                Frame {
                    id: 0,
                    name: "root".into(),
                    start: 0.0,
                    end: 100.0,
                    depth: 0,
                    category: None,
                    parent: None,
                    self_time: 0.0,
                },
                Frame {
                    id: 1,
                    name: "middle".into(),
                    start: 0.0,
                    end: 100.0,
                    depth: 1,
                    category: None,
                    parent: Some(0),
                    self_time: 0.0,
                },
                Frame {
                    id: 2,
                    name: "leaf".into(),
                    start: 0.0,
                    end: 60.0,
                    depth: 2,
                    category: None,
                    parent: Some(1),
                    self_time: 60.0,
                },
            ],
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
}
