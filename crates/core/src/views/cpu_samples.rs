use flame_cat_protocol::{CpuSamples, Rect, RenderCommand, SharedStr, ThemeToken, Viewport};
use std::collections::HashMap;

const ROW_HEIGHT: f64 = 18.0;
const ROW_GAP: f64 = 1.0;

/// Build full stack for a node by walking parent pointers.
fn build_stack(
    node_id: u32,
    node_map: &HashMap<u32, (Option<u32>, SharedStr)>,
) -> Vec<(u32, SharedStr)> {
    let mut stack = Vec::new();
    let mut cur = Some(node_id);
    let mut depth = 0;
    while let Some(id) = cur {
        depth += 1;
        if depth > 128 {
            break; // guard against cycles
        }
        if let Some((parent, name)) = node_map.get(&id) {
            if !name.is_empty() && name.as_ref() != "(root)" && name.as_ref() != "(idle)" {
                stack.push((id, name.clone()));
            }
            cur = *parent;
        } else {
            break;
        }
    }
    stack.reverse();
    stack
}

/// Render CPU samples as a flame chart.
///
/// Consecutive samples with the same leaf node are merged into bars.
/// Each stack frame depth gets its own row, with the deepest frame at top.
pub fn render_cpu_samples(
    samples: &CpuSamples,
    viewport: &Viewport,
    view_start: f64,
    view_end: f64,
) -> Vec<RenderCommand> {
    let duration = view_end - view_start;
    if duration <= 0.0 || samples.samples.is_empty() {
        return Vec::new();
    }

    // Build node lookup: id → (parent, function_name)
    let node_map: HashMap<u32, (Option<u32>, SharedStr)> = samples
        .nodes
        .iter()
        .map(|n| (n.id, (n.parent, n.function_name.clone())))
        .collect();

    let x_scale = viewport.width / duration;

    // Merge consecutive same-leaf samples into runs
    struct Run {
        start: f64,
        end: f64,
        node_id: u32,
    }

    let mut runs: Vec<Run> = Vec::new();
    for (i, &node_id) in samples.samples.iter().enumerate() {
        let ts = samples.timestamps[i];
        if ts > view_end {
            break;
        }
        // Estimate sample end from next sample timestamp
        let next_ts = if i + 1 < samples.timestamps.len() {
            samples.timestamps[i + 1]
        } else {
            ts + 1000.0 // 1ms fallback
        };
        if next_ts < view_start {
            continue;
        }

        if let Some(last) = runs.last_mut()
            && last.node_id == node_id
        {
            last.end = next_ts;
            continue;
        }
        runs.push(Run {
            start: ts,
            end: next_ts,
            node_id,
        });
    }

    let mut commands = Vec::with_capacity(runs.len() * 4 + 4);

    commands.push(RenderCommand::BeginGroup {
        id: "cpu-samples".into(),
        label: Some("CPU Samples".into()),
    });

    // Background
    commands.push(RenderCommand::DrawRect {
        rect: Rect::new(0.0, 0.0, viewport.width, viewport.height),
        color: ThemeToken::LaneBackground,
        border_color: Some(ThemeToken::LaneBorder),
        label: None,
        frame_id: None,
    });

    // Color palette for depth
    let depth_colors = [
        ThemeToken::FlameHot,
        ThemeToken::FlameWarm,
        ThemeToken::FlameCold,
        ThemeToken::AsyncSpanFill,
    ];

    for run in &runs {
        let stack = build_stack(run.node_id, &node_map);
        if stack.is_empty() {
            continue;
        }

        let x = (run.start - view_start) * x_scale;
        let w = (run.end - run.start) * x_scale;
        let clamped_x = x.max(0.0);
        let clamped_w = (x + w).min(viewport.width) - clamped_x;

        if clamped_w < 0.5 {
            continue;
        }

        for (depth, (_nid, name)) in stack.iter().enumerate() {
            let y = depth as f64 * (ROW_HEIGHT + ROW_GAP);
            if y + ROW_HEIGHT > viewport.height {
                break;
            }

            let color = depth_colors[depth % depth_colors.len()];

            commands.push(RenderCommand::DrawRect {
                rect: Rect::new(clamped_x, y, clamped_w, ROW_HEIGHT),
                color,
                border_color: Some(ThemeToken::LaneBorder),
                label: Some(name.clone()),
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
    use flame_cat_protocol::CpuNode;

    fn test_samples() -> CpuSamples {
        CpuSamples {
            nodes: vec![
                CpuNode {
                    id: 1,
                    parent: None,
                    function_name: "(root)".into(),
                    script_id: 0,
                },
                CpuNode {
                    id: 2,
                    parent: Some(1),
                    function_name: "main".into(),
                    script_id: 1,
                },
                CpuNode {
                    id: 3,
                    parent: Some(2),
                    function_name: "compute".into(),
                    script_id: 1,
                },
            ],
            samples: vec![2, 3, 3, 2],
            timestamps: vec![0.0, 1000.0, 2000.0, 3000.0],
        }
    }

    #[test]
    fn renders_cpu_samples() {
        let samples = test_samples();
        let vp = Viewport {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 200.0,
            dpr: 1.0,
        };
        let cmds = render_cpu_samples(&samples, &vp, 0.0, 4000.0);
        assert!(!cmds.is_empty());

        let rects: Vec<_> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCommand::DrawRect { label: Some(_), .. }))
            .collect();
        // 3 runs: main, compute(2x merged), main → expect rect for each stack frame
        assert!(rects.len() >= 3, "got {} rects", rects.len());
    }

    #[test]
    fn merges_consecutive_same_leaf() {
        let samples = test_samples();
        let vp = Viewport {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 200.0,
            dpr: 1.0,
        };
        let cmds = render_cpu_samples(&samples, &vp, 0.0, 4000.0);
        // Samples 1,2 are both node 3 → should merge into 1 run
        // So we have: run(node=2, 0-1000), run(node=3, 1000-3000), run(node=2, 3000-4000)
        // Run 1: depth 0 (main) = 1 rect
        // Run 2: depth 0 (main) + depth 1 (compute) = 2 rects
        // Run 3: depth 0 (main) = 1 rect
        // Total: 4 labeled rects
        let labeled_rects: Vec<_> = cmds
            .iter()
            .filter(|c| matches!(c, RenderCommand::DrawRect { label: Some(_), .. }))
            .collect();
        assert_eq!(labeled_rects.len(), 4);
    }

    #[test]
    fn empty_samples_returns_empty() {
        let samples = CpuSamples {
            nodes: vec![],
            samples: vec![],
            timestamps: vec![],
        };
        let vp = Viewport {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 200.0,
            dpr: 1.0,
        };
        let cmds = render_cpu_samples(&samples, &vp, 0.0, 100.0);
        assert!(cmds.is_empty());
    }
}
