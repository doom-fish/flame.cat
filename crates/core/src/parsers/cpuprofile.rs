use serde::Deserialize;
use thiserror::Error;

use crate::model::{Frame, Profile, ProfileMetadata};

#[derive(Debug, Error)]
pub enum CpuProfileParseError {
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("missing or empty nodes")]
    MissingNodes,
}

/// V8 CPU profile node.
#[derive(Debug, Deserialize)]
struct CpuProfileNode {
    id: u64,
    #[serde(rename = "callFrame")]
    call_frame: CallFrame,
    #[serde(default)]
    children: Vec<u64>,
    #[serde(default)]
    #[allow(dead_code)]
    hit_count: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct CallFrame {
    #[serde(rename = "functionName")]
    function_name: String,
    #[serde(default, rename = "scriptId")]
    #[allow(dead_code)]
    script_id: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default, rename = "lineNumber")]
    #[allow(dead_code)]
    line_number: Option<i64>,
    #[serde(default, rename = "columnNumber")]
    #[allow(dead_code)]
    column_number: Option<i64>,
}

/// V8 CPU profile top-level structure (.cpuprofile files).
#[derive(Debug, Deserialize)]
struct CpuProfile {
    nodes: Vec<CpuProfileNode>,
    #[serde(rename = "startTime")]
    start_time: f64,
    #[serde(rename = "endTime")]
    end_time: f64,
    #[serde(default)]
    samples: Vec<u64>,
    #[serde(default, rename = "timeDeltas")]
    time_deltas: Vec<f64>,
}

fn compute_depth(
    node_id: u64,
    parent_map: &std::collections::HashMap<u64, u64>,
    cache: &mut std::collections::HashMap<u64, u32>,
) -> u32 {
    if let Some(&d) = cache.get(&node_id) {
        return d;
    }
    let depth = match parent_map.get(&node_id) {
        Some(&pid) => compute_depth(pid, parent_map, cache) + 1,
        None => 0,
    };
    cache.insert(node_id, depth);
    depth
}

/// Parse a V8 CPU profile (.cpuprofile) into a `Profile`.
///
/// Used by: Node.js `--cpu-prof`, Chrome DevTools CPU profiler, Deno.
pub fn parse_cpuprofile(data: &[u8]) -> Result<Profile, CpuProfileParseError> {
    let cpu_profile: CpuProfile = serde_json::from_slice(data)?;

    if cpu_profile.nodes.is_empty() {
        return Err(CpuProfileParseError::MissingNodes);
    }

    // Build node lookup and parent map.
    let node_map: std::collections::HashMap<u64, &CpuProfileNode> =
        cpu_profile.nodes.iter().map(|n| (n.id, n)).collect();

    let mut parent_map: std::collections::HashMap<u64, u64> = std::collections::HashMap::new();
    for node in &cpu_profile.nodes {
        for &child_id in &node.children {
            parent_map.insert(child_id, node.id);
        }
    }

    let mut depth_cache: std::collections::HashMap<u64, u32> = std::collections::HashMap::new();

    // If we have samples + timeDeltas, reconstruct timeline from sample data.
    if !cpu_profile.samples.is_empty() && !cpu_profile.time_deltas.is_empty() {
        return parse_from_samples(&cpu_profile, &node_map, &parent_map, &mut depth_cache);
    }

    // Fallback: build frames from the node tree with synthetic timing.
    let mut frames: Vec<Frame> = Vec::new();
    let mut next_id: u64 = 0;
    let total_duration = cpu_profile.end_time - cpu_profile.start_time;

    // DFS to assign synthetic time spans.
    let root_ids: Vec<u64> = cpu_profile
        .nodes
        .iter()
        .filter(|n| !parent_map.contains_key(&n.id))
        .map(|n| n.id)
        .collect();

    struct DfsState<'a> {
        node_map: &'a std::collections::HashMap<u64, &'a CpuProfileNode>,
        parent_map: &'a std::collections::HashMap<u64, u64>,
        depth_cache: &'a mut std::collections::HashMap<u64, u32>,
        frames: &'a mut Vec<Frame>,
        next_id: &'a mut u64,
        offset: f64,
    }

    fn dfs(state: &mut DfsState, node_id: u64, parent_frame_id: Option<u64>) -> f64 {
        let node = match state.node_map.get(&node_id) {
            Some(n) => *n,
            None => return state.offset,
        };

        let depth = compute_depth(node_id, state.parent_map, state.depth_cache);
        let name = if node.call_frame.function_name.is_empty() {
            "(anonymous)".to_string()
        } else {
            node.call_frame.function_name.clone()
        };

        let id = *state.next_id;
        *state.next_id += 1;
        let frame_start = state.offset;

        // Reserve space for this frame.
        let frame_idx = state.frames.len();
        state.frames.push(Frame {
            id,
            name,
            start: frame_start,
            end: frame_start, // updated after children
            depth,
            category: node.call_frame.url.clone(),
            parent: parent_frame_id,
            self_time: 0.0,
            thread: None,
        });

        // Leaf nodes get 1.0 unit of time.
        if node.children.is_empty() {
            state.offset += 1.0;
        }

        for &child_id in &node.children {
            dfs(state, child_id, Some(id));
        }

        state.frames[frame_idx].end = state.offset;
        state.offset
    }

    let mut offset = 0.0;
    for root_id in root_ids {
        let mut state = DfsState {
            node_map: &node_map,
            parent_map: &parent_map,
            depth_cache: &mut depth_cache,
            frames: &mut frames,
            next_id: &mut next_id,
            offset,
        };
        offset = dfs(&mut state, root_id, None);
    }

    // Scale to actual duration if available.
    if total_duration > 0.0 && offset > 0.0 {
        let scale = total_duration / offset;
        for f in &mut frames {
            f.start *= scale;
            f.end *= scale;
        }
    }

    compute_self_times(&mut frames);

    Ok(Profile {
        metadata: ProfileMetadata {
            name: None,
            start_time: cpu_profile.start_time,
            end_time: cpu_profile.end_time,
            format: "cpuprofile".to_string(),
            time_domain: None,
        },
        frames,
    })
}

/// Reconstruct timeline from V8 sample data.
fn parse_from_samples(
    cpu_profile: &CpuProfile,
    node_map: &std::collections::HashMap<u64, &CpuProfileNode>,
    parent_map: &std::collections::HashMap<u64, u64>,
    depth_cache: &mut std::collections::HashMap<u64, u32>,
) -> Result<Profile, CpuProfileParseError> {
    let mut frames: Vec<Frame> = Vec::new();
    let mut next_id: u64 = 0;

    // Build timestamps from time deltas.
    let mut timestamps = Vec::with_capacity(cpu_profile.time_deltas.len());
    let mut t = cpu_profile.start_time;
    for &delta in &cpu_profile.time_deltas {
        t += delta;
        timestamps.push(t);
    }

    // For each sample, walk up the stack to build the full call stack.
    // Merge adjacent identical stacks into continuous frames.
    struct ActiveFrame {
        frame_idx: usize,
        node_id: u64,
    }

    let mut active_stacks: Vec<ActiveFrame> = Vec::new();

    let sample_count = cpu_profile.samples.len().min(timestamps.len());

    for i in 0..sample_count {
        let sample_node_id = cpu_profile.samples[i];
        let sample_time = timestamps[i];
        let next_time = if i + 1 < sample_count {
            timestamps[i + 1]
        } else {
            cpu_profile.end_time
        };

        // Build current stack (leaf to root).
        let mut stack = Vec::new();
        let mut nid = sample_node_id;
        loop {
            stack.push(nid);
            match parent_map.get(&nid) {
                Some(&pid) => nid = pid,
                None => break,
            }
        }
        stack.reverse(); // root to leaf

        // Find common prefix with active stacks.
        let mut common_len = 0;
        for (j, active) in active_stacks.iter().enumerate() {
            if j < stack.len() && stack[j] == active.node_id {
                common_len = j + 1;
            } else {
                break;
            }
        }

        // Close frames that are no longer in the stack.
        while active_stacks.len() > common_len {
            if let Some(af) = active_stacks.pop() {
                frames[af.frame_idx].end = sample_time;
            }
        }

        // Open new frames for the rest of the stack.
        for (depth_idx, &nid) in stack.iter().enumerate().skip(common_len) {
            let node = match node_map.get(&nid) {
                Some(n) => *n,
                None => continue,
            };

            let depth = compute_depth(nid, parent_map, depth_cache);
            let name = if node.call_frame.function_name.is_empty() {
                "(anonymous)".to_string()
            } else {
                node.call_frame.function_name.clone()
            };

            let parent_frame_id = if depth_idx > 0 {
                active_stacks.last().map(|af| frames[af.frame_idx].id)
            } else {
                None
            };

            let id = next_id;
            next_id += 1;
            let frame_idx = frames.len();

            frames.push(Frame {
                id,
                name,
                start: sample_time,
                end: next_time,
                depth,
                category: node.call_frame.url.clone(),
                parent: parent_frame_id,
                self_time: 0.0,
                thread: None,
            });

            active_stacks.push(ActiveFrame {
                frame_idx,
                node_id: nid,
            });
        }
    }

    // Close remaining active frames.
    for af in &active_stacks {
        frames[af.frame_idx].end = cpu_profile.end_time;
    }

    compute_self_times(&mut frames);

    Ok(Profile {
        metadata: ProfileMetadata {
            name: None,
            start_time: cpu_profile.start_time,
            end_time: cpu_profile.end_time,
            format: "cpuprofile".to_string(),
            time_domain: None,
        },
        frames,
    })
}

fn compute_self_times(frames: &mut [Frame]) {
    let child_time = {
        let mut map = std::collections::HashMap::<u64, f64>::new();
        for f in frames.iter() {
            if let Some(pid) = f.parent {
                *map.entry(pid).or_default() += f.duration();
            }
        }
        map
    };
    for f in frames.iter_mut() {
        let children_total = child_time.get(&f.id).copied().unwrap_or(0.0);
        f.self_time = (f.duration() - children_total).max(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_cpuprofile() {
        let json = r#"{
            "nodes": [
                {"id":1,"callFrame":{"functionName":"(root)"},"children":[2]},
                {"id":2,"callFrame":{"functionName":"main"},"children":[3,4]},
                {"id":3,"callFrame":{"functionName":"foo"},"children":[]},
                {"id":4,"callFrame":{"functionName":"bar"},"children":[]}
            ],
            "startTime": 0,
            "endTime": 1000,
            "samples": [],
            "timeDeltas": []
        }"#;

        let profile = parse_cpuprofile(json.as_bytes()).unwrap();
        assert_eq!(profile.metadata.format, "cpuprofile");
        assert_eq!(profile.frames.len(), 4);

        let root = profile.frames.iter().find(|f| f.name == "(root)").unwrap();
        assert_eq!(root.depth, 0);

        let main_f = profile.frames.iter().find(|f| f.name == "main").unwrap();
        assert_eq!(main_f.depth, 1);
        assert_eq!(main_f.parent, Some(root.id));
    }

    #[test]
    fn parse_with_samples() {
        let json = r#"{
            "nodes": [
                {"id":1,"callFrame":{"functionName":"(root)"},"children":[2]},
                {"id":2,"callFrame":{"functionName":"main"},"children":[3]},
                {"id":3,"callFrame":{"functionName":"work"},"children":[]}
            ],
            "startTime": 0,
            "endTime": 300,
            "samples": [3, 3, 2],
            "timeDeltas": [0, 100, 100]
        }"#;

        let profile = parse_cpuprofile(json.as_bytes()).unwrap();
        assert_eq!(profile.metadata.format, "cpuprofile");
        assert!(!profile.frames.is_empty());

        // "work" should appear as a frame
        let work = profile.frames.iter().find(|f| f.name == "work");
        assert!(work.is_some());
    }

    #[test]
    fn empty_nodes_errors() {
        let json = r#"{"nodes":[],"startTime":0,"endTime":0,"samples":[],"timeDeltas":[]}"#;
        assert!(parse_cpuprofile(json.as_bytes()).is_err());
    }
}
