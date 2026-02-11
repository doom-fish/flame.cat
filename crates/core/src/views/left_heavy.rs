use std::collections::HashMap;

use flame_cat_protocol::{Rect, RenderCommand, ThemeToken, Viewport};

use crate::model::Profile;

const FRAME_HEIGHT: f64 = 20.0;

/// Merged node for left-heavy aggregation.
struct MergedNode {
    name: String,
    total_time: f64,
    children: Vec<MergedNode>,
}

/// Render a profile in left-heavy view: identical call stacks are merged
/// and sorted heaviest-first (left).
pub fn render_left_heavy(profile: &Profile, viewport: &Viewport) -> Vec<RenderCommand> {
    if profile.frames.is_empty() {
        return Vec::new();
    }

    // Group top-level frames, then recursively merge children.
    let roots = merge_children(profile, None);
    let total_time: f64 = roots.iter().map(|n| n.total_time).sum();
    if total_time <= 0.0 {
        return Vec::new();
    }

    let x_scale = viewport.width / total_time;

    let mut commands = Vec::new();
    commands.push(RenderCommand::BeginGroup {
        id: "left-heavy".to_string(),
        label: Some("Left Heavy".to_string()),
    });

    layout_nodes(&roots, 0, 0.0, x_scale, viewport, &mut commands);

    commands.push(RenderCommand::EndGroup);
    commands
}

fn merge_children(profile: &Profile, parent: Option<u64>) -> Vec<MergedNode> {
    let children: Vec<_> = profile
        .frames
        .iter()
        .filter(|f| f.parent == parent)
        .collect();

    // Group by name, sum times, recursively merge.
    let mut groups: HashMap<&str, (f64, Vec<u64>)> = HashMap::new();
    for child in &children {
        let entry = groups.entry(&child.name).or_insert((0.0, Vec::new()));
        entry.0 += child.duration();
        entry.1.push(child.id);
    }

    let mut nodes: Vec<MergedNode> = groups
        .into_iter()
        .map(|(name, (total_time, ids))| {
            // Merge grandchildren from all instances of this name.
            let mut merged_children = Vec::new();
            for id in &ids {
                let mut sub = merge_children(profile, Some(*id));
                merged_children.append(&mut sub);
            }
            // Re-merge the grandchildren by name too.
            let merged_children = re_merge(merged_children);

            MergedNode {
                name: name.to_string(),
                total_time,
                children: merged_children,
            }
        })
        .collect();

    // Sort heaviest first.
    nodes.sort_by(|a, b| b.total_time.partial_cmp(&a.total_time).unwrap());
    nodes
}

fn re_merge(nodes: Vec<MergedNode>) -> Vec<MergedNode> {
    let mut groups: HashMap<String, MergedNode> = HashMap::new();
    for node in nodes {
        let entry = groups.entry(node.name.clone()).or_insert(MergedNode {
            name: node.name.clone(),
            total_time: 0.0,
            children: Vec::new(),
        });
        entry.total_time += node.total_time;
        entry.children.extend(node.children);
    }
    let mut result: Vec<MergedNode> = groups.into_values().collect();
    result.sort_by(|a, b| b.total_time.partial_cmp(&a.total_time).unwrap());
    result
}

fn layout_nodes(
    nodes: &[MergedNode],
    depth: u32,
    mut x_offset: f64,
    x_scale: f64,
    viewport: &Viewport,
    commands: &mut Vec<RenderCommand>,
) {
    let y = f64::from(depth) * FRAME_HEIGHT;

    for node in nodes {
        let w = node.total_time * x_scale;

        if w >= 0.5 && y + FRAME_HEIGHT >= viewport.y && y <= viewport.y + viewport.height {
            let color = match depth % 4 {
                0 => ThemeToken::FlameHot,
                1 => ThemeToken::FlameWarm,
                2 => ThemeToken::FlameCold,
                _ => ThemeToken::FlameNeutral,
            };

            commands.push(RenderCommand::DrawRect {
                rect: Rect::new(x_offset, y, w, FRAME_HEIGHT - 1.0),
                color,
                border_color: Some(ThemeToken::Border),
                label: Some(node.name.clone()),
                frame_id: None,
            });
        }

        layout_nodes(
            &node.children,
            depth + 1,
            x_offset,
            x_scale,
            viewport,
            commands,
        );
        x_offset += w;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Frame, ProfileMetadata};

    #[test]
    fn merges_identical_stacks() {
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
                    name: "main".into(),
                    start: 0.0,
                    end: 50.0,
                    depth: 0,
                    category: None,
                    parent: None,
                    self_time: 50.0,
                },
                Frame {
                    id: 1,
                    name: "main".into(),
                    start: 50.0,
                    end: 100.0,
                    depth: 0,
                    category: None,
                    parent: None,
                    self_time: 50.0,
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
        let cmds = render_left_heavy(&profile, &vp);
        let rects: Vec<_> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCommand::DrawRect { .. }))
            .collect();
        // Two "main" frames should be merged into one rect.
        assert_eq!(rects.len(), 1);
    }
}
