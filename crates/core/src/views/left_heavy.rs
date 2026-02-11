use std::collections::HashMap;

use flame_cat_protocol::{
    Rect, RenderCommand, SharedStr, Span, ThemeToken, Viewport, VisualProfile,
};

const FRAME_HEIGHT: f64 = 20.0;

/// Merged node for left-heavy aggregation.
struct MergedNode {
    name: SharedStr,
    total_time: f64,
    children: Vec<MergedNode>,
}

/// Render a profile in left-heavy view: identical call stacks are merged
/// and sorted heaviest-first (left).
pub fn render_left_heavy(
    profile: &VisualProfile,
    viewport: &Viewport,
    thread_id: Option<u32>,
) -> Vec<RenderCommand> {
    let spans: Vec<&Span> = if let Some(tid) = thread_id {
        profile
            .threads
            .iter()
            .filter(|t| t.id == tid)
            .flat_map(|t| t.spans.iter())
            .collect()
    } else {
        profile.all_spans().collect()
    };
    if spans.is_empty() {
        return Vec::new();
    }

    // Build parent → children index for O(1) lookup
    let mut children_index: HashMap<Option<u64>, Vec<usize>> = HashMap::with_capacity(spans.len());
    for (i, span) in spans.iter().enumerate() {
        children_index.entry(span.parent).or_default().push(i);
    }

    let roots = merge_children(&spans, &children_index, None);
    let total_time: f64 = roots.iter().map(|n| n.total_time).sum();
    if total_time <= 0.0 {
        return Vec::new();
    }

    let x_scale = viewport.width / total_time;

    let mut commands = Vec::with_capacity(profile.span_count());
    commands.push(RenderCommand::BeginGroup {
        id: "left-heavy".into(),
        label: Some("Left Heavy".into()),
    });

    layout_nodes(&roots, 0, 0.0, x_scale, viewport, &mut commands);

    commands.push(RenderCommand::EndGroup);
    commands
}

fn merge_children(
    spans: &[&Span],
    children_index: &HashMap<Option<u64>, Vec<usize>>,
    parent: Option<u64>,
) -> Vec<MergedNode> {
    let Some(child_indices) = children_index.get(&parent) else {
        return Vec::new();
    };

    let mut groups: HashMap<&str, (SharedStr, f64, Vec<u64>)> = HashMap::new();
    for &idx in child_indices {
        let child = spans[idx];
        let entry = groups
            .entry(&child.name)
            .or_insert_with(|| (child.name.clone(), 0.0, Vec::new()));
        entry.1 += child.duration();
        entry.2.push(child.id);
    }

    let mut nodes: Vec<MergedNode> = groups
        .into_iter()
        .map(|(_, (name, total_time, ids))| {
            let mut merged_children = Vec::new();
            for id in &ids {
                let mut sub = merge_children(spans, children_index, Some(*id));
                merged_children.append(&mut sub);
            }
            let merged_children = re_merge(merged_children);

            MergedNode {
                name,
                total_time,
                children: merged_children,
            }
        })
        .collect();

    nodes.sort_by(|a, b| b.total_time.total_cmp(&a.total_time));
    nodes
}

fn re_merge(nodes: Vec<MergedNode>) -> Vec<MergedNode> {
    let mut groups: HashMap<SharedStr, MergedNode> = HashMap::with_capacity(nodes.len());
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
    result.sort_by(|a, b| b.total_time.total_cmp(&a.total_time));
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
    use flame_cat_protocol::{ProfileMeta, SourceFormat, SpanKind, ThreadGroup, ValueUnit};

    #[test]
    fn merges_identical_stacks() {
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
                spans: vec![
                    Span {
                        id: 0,
                        name: "main".into(),
                        start: 0.0,
                        end: 50.0,
                        depth: 0,
                        parent: None,
                        self_value: 50.0,
                        kind: SpanKind::Event,
                        category: None,
                    },
                    Span {
                        id: 1,
                        name: "main".into(),
                        start: 50.0,
                        end: 100.0,
                        depth: 0,
                        parent: None,
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
        };
        let vp = Viewport {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
            dpr: 1.0,
        };
        let cmds = render_left_heavy(&profile, &vp, None);
        let rects: Vec<_> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCommand::DrawRect { .. }))
            .collect();
        // Two "main" frames should be merged into one rect.
        assert_eq!(rects.len(), 1);
    }

    #[test]
    fn empty_profile_returns_empty() {
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
        assert!(render_left_heavy(&profile, &vp, None).is_empty());
    }
}
