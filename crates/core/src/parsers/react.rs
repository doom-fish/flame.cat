use serde::Deserialize;
use thiserror::Error;

use crate::model::{Frame, Profile, ProfileMetadata};

#[derive(Debug, Error)]
pub enum ReactParseError {
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("missing or invalid profiler data")]
    InvalidData,
}

/// React DevTools profiler export format (version 5).
/// Supports both the legacy commit-based profiler and timeline data.
#[derive(Debug, Deserialize)]
struct ReactProfileExport {
    #[serde(default)]
    #[allow(dead_code)]
    version: Option<u32>,
    #[serde(rename = "dataForRoots")]
    data_for_roots: Vec<ReactRoot>,
}

#[derive(Debug, Deserialize)]
struct ReactRoot {
    #[serde(rename = "commitData")]
    commit_data: Vec<ReactCommit>,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    #[serde(rename = "rootID")]
    #[allow(dead_code)]
    root_id: Option<u64>,
    /// Initial component tree snapshot — Map<fiberID, SnapshotNode> as tuples.
    #[serde(default)]
    snapshots: Vec<(u64, SnapshotNode)>,
    /// Tree mutations per commit — used to reconstruct the tree at each commit.
    #[serde(default)]
    #[allow(dead_code)]
    operations: Vec<Vec<i64>>,
    /// Baseline render durations — Map<fiberID, duration> as tuples.
    #[serde(default, rename = "initialTreeBaseDurations")]
    #[allow(dead_code)]
    initial_tree_base_durations: Vec<(u64, f64)>,
}

/// A node in the React component tree snapshot.
#[derive(Debug, Clone, Deserialize)]
struct SnapshotNode {
    #[allow(dead_code)]
    id: u64,
    #[serde(default)]
    children: Vec<u64>,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    #[serde(rename = "hocDisplayNames")]
    #[serde(default)]
    #[allow(dead_code)]
    hoc_display_names: Option<Vec<String>>,
    #[serde(default)]
    #[allow(dead_code)]
    key: Option<serde_json::Value>,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    element_type: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ReactCommit {
    #[serde(rename = "fiberActualDurations")]
    fiber_actual_durations: Vec<(u64, f64)>,
    #[serde(rename = "fiberSelfDurations")]
    fiber_self_durations: Vec<(u64, f64)>,
    timestamp: f64,
    duration: f64,
    #[serde(default, rename = "changeDescriptions")]
    change_descriptions: Option<Vec<(u64, ChangeDescription)>>,
    #[serde(default, rename = "priorityLevel")]
    #[allow(dead_code)]
    priority_level: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    updaters: Option<Vec<serde_json::Value>>,
}

/// What caused a component to re-render.
#[derive(Debug, Clone, Deserialize)]
struct ChangeDescription {
    #[serde(default)]
    #[allow(dead_code)]
    context: Option<serde_json::Value>,
    #[serde(default, rename = "didHooksChange")]
    did_hooks_change: bool,
    #[serde(default, rename = "isFirstMount")]
    is_first_mount: bool,
    #[serde(default)]
    props: Option<Vec<String>>,
    #[serde(default)]
    state: Option<Vec<String>>,
    #[serde(default)]
    #[allow(dead_code)]
    hooks: Option<Vec<u32>>,
}

/// Reconstructed component tree at a given commit point.
struct FiberTree {
    nodes: std::collections::HashMap<u64, SnapshotNode>,
}

impl FiberTree {
    /// Build initial tree from snapshot data.
    fn from_snapshots(snapshots: &[(u64, SnapshotNode)]) -> Self {
        let nodes: std::collections::HashMap<u64, SnapshotNode> =
            snapshots.iter().cloned().collect();
        Self { nodes }
    }

    /// Get display name for a fiber, falling back to "Anonymous".
    fn display_name(&self, fiber_id: u64) -> String {
        self.nodes
            .get(&fiber_id)
            .and_then(|n| n.display_name.as_ref())
            .cloned()
            .unwrap_or_else(|| format!("fiber-{fiber_id}"))
    }

    /// Walk the tree depth-first from a given root, emitting frames for
    /// fibers that rendered in this commit (have actualDuration > 0).
    fn walk_commit(
        &self,
        root_id: u64,
        commit: &ReactCommit,
        commit_start: f64,
        frames: &mut Vec<Frame>,
        next_id: &mut u64,
    ) {
        let actual_durations: std::collections::HashMap<u64, f64> =
            commit.fiber_actual_durations.iter().copied().collect();
        let self_durations: std::collections::HashMap<u64, f64> =
            commit.fiber_self_durations.iter().copied().collect();
        let change_descs: std::collections::HashMap<u64, &ChangeDescription> = commit
            .change_descriptions
            .as_ref()
            .map(|descs| descs.iter().map(|(id, desc)| (*id, desc)).collect())
            .unwrap_or_default();

        // Depth-first walk, tracking time offset within the commit.
        let mut stack: Vec<(u64, u32, f64, Option<u64>)> = vec![];

        if actual_durations.get(&root_id).is_some_and(|d| *d > 0.0) {
            stack.push((root_id, 0, commit_start, None));
        }

        while let Some((fiber_id, depth, offset, parent_frame_id)) = stack.pop() {
            let actual = actual_durations.get(&fiber_id).copied().unwrap_or(0.0);
            if actual <= 0.0 {
                continue;
            }

            let self_time = self_durations.get(&fiber_id).copied().unwrap_or(0.0);
            let name = self.display_name(fiber_id);

            // Build category string encoding change description.
            let category = if let Some(desc) = change_descs.get(&fiber_id) {
                Some(format_change_description(desc))
            } else {
                Some("react".to_string())
            };

            let id = *next_id;
            *next_id += 1;

            frames.push(Frame {
                id,
                name,
                start: offset,
                end: offset + actual,
                depth,
                category,
                parent: parent_frame_id,
                self_time,
                thread: Some("React Components".to_string()),
            });

            // Queue children in reverse order so first child is processed first.
            if let Some(node) = self.nodes.get(&fiber_id) {
                let mut child_offset = offset;
                // Add self-time gap before first child.
                // Approximate: distribute parent self-time as a gap before children.
                let children_total: f64 = node
                    .children
                    .iter()
                    .filter_map(|cid| actual_durations.get(cid))
                    .sum();
                let parent_self = (actual - children_total).max(0.0);
                child_offset += parent_self;

                // Push children in reverse so they pop in order.
                let mut child_items = Vec::new();
                for &child_id in &node.children {
                    if actual_durations.get(&child_id).is_some_and(|d| *d > 0.0) {
                        let child_dur = actual_durations.get(&child_id).copied().unwrap_or(0.0);
                        child_items.push((child_id, depth + 1, child_offset, Some(id)));
                        child_offset += child_dur;
                    }
                }
                // Push in reverse for correct DFS order via stack.
                for item in child_items.into_iter().rev() {
                    stack.push(item);
                }
            }
        }
    }
}

/// Format a ChangeDescription into a human-readable category string.
fn format_change_description(desc: &ChangeDescription) -> String {
    if desc.is_first_mount {
        return "react.mount".to_string();
    }
    let mut reasons = Vec::new();
    if let Some(props) = &desc.props
        && !props.is_empty()
    {
        reasons.push(format!("props: {}", props.join(", ")));
    }
    if let Some(state) = &desc.state
        && !state.is_empty()
    {
        reasons.push(format!("state: {}", state.join(", ")));
    }
    if desc.did_hooks_change {
        reasons.push("hooks".to_string());
    }
    if reasons.is_empty() {
        "react".to_string()
    } else {
        format!("react.update({})", reasons.join("; "))
    }
}

/// Parse a React DevTools profiler export into a `Profile`.
///
/// When snapshot data is available, reconstructs the fiber tree to emit
/// frames with correct parent-child relationships and tree depth.
/// Falls back to a flat representation when snapshots are absent.
pub fn parse_react_profile(data: &[u8]) -> Result<Profile, ReactParseError> {
    let export: ReactProfileExport = serde_json::from_slice(data)?;

    let mut frames = Vec::new();
    let mut next_id: u64 = 0;

    let mut global_start = f64::INFINITY;
    let mut global_end = f64::NEG_INFINITY;

    for root in &export.data_for_roots {
        let has_snapshots = !root.snapshots.is_empty();

        if has_snapshots {
            // Full tree reconstruction path.
            let tree = FiberTree::from_snapshots(&root.snapshots);

            // Find root fiber IDs (fibers that appear in snapshots but not as
            // children of any other fiber).
            let all_children: std::collections::HashSet<u64> = root
                .snapshots
                .iter()
                .flat_map(|(_, node)| node.children.iter().copied())
                .collect();
            let root_ids: Vec<u64> = root
                .snapshots
                .iter()
                .map(|(id, _)| *id)
                .filter(|id| !all_children.contains(id))
                .collect();

            for commit in &root.commit_data {
                let commit_start = commit.timestamp;
                let commit_end = commit_start + commit.duration;

                global_start = global_start.min(commit_start);
                global_end = global_end.max(commit_end);

                for &root_id in &root_ids {
                    tree.walk_commit(root_id, commit, commit_start, &mut frames, &mut next_id);
                }
            }
        } else {
            // Fallback: flat representation without tree structure.
            for commit in &root.commit_data {
                let commit_start = commit.timestamp;
                let commit_end = commit_start + commit.duration;

                global_start = global_start.min(commit_start);
                global_end = global_end.max(commit_end);

                let self_durations: std::collections::HashMap<u64, f64> =
                    commit.fiber_self_durations.iter().copied().collect();

                let mut offset = commit_start;
                for (fiber_id, actual_duration) in &commit.fiber_actual_durations {
                    if *actual_duration <= 0.0 {
                        continue;
                    }

                    let self_time = self_durations.get(fiber_id).copied().unwrap_or(0.0);
                    let id = next_id;
                    next_id += 1;

                    frames.push(Frame {
                        id,
                        name: format!("fiber-{fiber_id}"),
                        start: offset,
                        end: offset + actual_duration,
                        depth: 0,
                        category: Some("react".to_string()),
                        parent: None,
                        self_time,
                        thread: Some("React Components".to_string()),
                    });

                    offset += actual_duration;
                }
            }
        }
    }

    Ok(Profile {
        metadata: ProfileMetadata {
            name: export
                .data_for_roots
                .first()
                .and_then(|r| r.display_name.clone()),
            start_time: if global_start.is_finite() {
                global_start
            } else {
                0.0
            },
            end_time: if global_end.is_finite() {
                global_end
            } else {
                0.0
            },
            format: "react".to_string(),
        },
        frames,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_react_profile() {
        let json = r#"{
            "version": 5,
            "dataForRoots": [{
                "displayName": "App",
                "commitData": [{
                    "fiberActualDurations": [[1, 10.0], [2, 5.0]],
                    "fiberSelfDurations": [[1, 3.0], [2, 5.0]],
                    "timestamp": 100.0,
                    "duration": 15.0
                }]
            }]
        }"#;

        let profile = parse_react_profile(json.as_bytes()).unwrap();
        assert_eq!(profile.metadata.format, "react");
        assert_eq!(profile.metadata.name.as_deref(), Some("App"));
        assert_eq!(profile.frames.len(), 2);
    }

    #[test]
    fn empty_react_profile() {
        let json = r#"{"version": 5, "dataForRoots": []}"#;
        let profile = parse_react_profile(json.as_bytes()).unwrap();
        assert!(profile.frames.is_empty());
    }

    #[test]
    fn parse_with_snapshots_and_tree() {
        // Profile with full fiber tree: App → Header, Body → List
        let json = r#"{
            "version": 5,
            "dataForRoots": [{
                "displayName": "App",
                "rootID": 1,
                "snapshots": [
                    [1, {"id": 1, "children": [2, 3], "displayName": "App"}],
                    [2, {"id": 2, "children": [], "displayName": "Header"}],
                    [3, {"id": 3, "children": [4], "displayName": "Body"}],
                    [4, {"id": 4, "children": [], "displayName": "List"}]
                ],
                "operations": [],
                "initialTreeBaseDurations": [[1, 15.0], [2, 3.0], [3, 10.0], [4, 5.0]],
                "commitData": [{
                    "fiberActualDurations": [[1, 15.0], [2, 3.0], [3, 10.0], [4, 5.0]],
                    "fiberSelfDurations": [[1, 2.0], [2, 3.0], [3, 5.0], [4, 5.0]],
                    "timestamp": 100.0,
                    "duration": 15.0,
                    "changeDescriptions": [
                        [2, {"isFirstMount": true, "didHooksChange": false}],
                        [3, {"isFirstMount": false, "didHooksChange": false, "props": ["count"]}]
                    ]
                }]
            }]
        }"#;

        let profile = parse_react_profile(json.as_bytes()).unwrap();
        assert_eq!(profile.frames.len(), 4);

        // App should be the root at depth 0
        let app = &profile.frames[0];
        assert_eq!(app.name, "App");
        assert_eq!(app.depth, 0);
        assert!(app.parent.is_none());
        assert!((app.self_time - 2.0).abs() < f64::EPSILON);

        // Header at depth 1, child of App
        let header = &profile.frames[1];
        assert_eq!(header.name, "Header");
        assert_eq!(header.depth, 1);
        assert_eq!(header.parent, Some(app.id));
        assert_eq!(header.category.as_deref(), Some("react.mount"));

        // Body at depth 1, child of App
        let body = &profile.frames[2];
        assert_eq!(body.name, "Body");
        assert_eq!(body.depth, 1);
        assert_eq!(body.parent, Some(app.id));
        assert_eq!(body.category.as_deref(), Some("react.update(props: count)"));

        // List at depth 2, child of Body
        let list = &profile.frames[3];
        assert_eq!(list.name, "List");
        assert_eq!(list.depth, 2);
        assert_eq!(list.parent, Some(body.id));

        // All frames should be on the React Components thread
        assert!(
            profile
                .frames
                .iter()
                .all(|f| f.thread.as_deref() == Some("React Components"))
        );
    }

    #[test]
    fn parse_multiple_commits() {
        let json = r#"{
            "version": 5,
            "dataForRoots": [{
                "displayName": "App",
                "snapshots": [
                    [1, {"id": 1, "children": [2], "displayName": "App"}],
                    [2, {"id": 2, "children": [], "displayName": "Counter"}]
                ],
                "operations": [],
                "commitData": [
                    {
                        "fiberActualDurations": [[1, 10.0], [2, 5.0]],
                        "fiberSelfDurations": [[1, 5.0], [2, 5.0]],
                        "timestamp": 100.0,
                        "duration": 10.0
                    },
                    {
                        "fiberActualDurations": [[1, 8.0], [2, 3.0]],
                        "fiberSelfDurations": [[1, 5.0], [2, 3.0]],
                        "timestamp": 200.0,
                        "duration": 8.0
                    }
                ]
            }]
        }"#;

        let profile = parse_react_profile(json.as_bytes()).unwrap();
        // 2 commits × 2 components = 4 frames
        assert_eq!(profile.frames.len(), 4);

        // First commit at t=100
        assert!((profile.frames[0].start - 100.0).abs() < f64::EPSILON);
        // Second commit at t=200
        assert!((profile.frames[2].start - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn change_description_formatting() {
        let mount = ChangeDescription {
            context: None,
            did_hooks_change: false,
            is_first_mount: true,
            props: None,
            state: None,
            hooks: None,
        };
        assert_eq!(format_change_description(&mount), "react.mount");

        let props_update = ChangeDescription {
            context: None,
            did_hooks_change: false,
            is_first_mount: false,
            props: Some(vec!["count".to_string(), "label".to_string()]),
            state: None,
            hooks: None,
        };
        assert_eq!(
            format_change_description(&props_update),
            "react.update(props: count, label)"
        );

        let hooks_update = ChangeDescription {
            context: None,
            did_hooks_change: true,
            is_first_mount: false,
            props: None,
            state: Some(vec!["value".to_string()]),
            hooks: None,
        };
        assert_eq!(
            format_change_description(&hooks_update),
            "react.update(state: value; hooks)"
        );
    }
}
