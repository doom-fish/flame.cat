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
    #[serde(rename = "dataForRoots")]
    data_for_roots: Vec<ReactRoot>,
}

#[derive(Debug, Deserialize)]
struct ReactRoot {
    #[serde(rename = "commitData")]
    commit_data: Vec<ReactCommit>,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    /// Initial component tree snapshot — Map<fiberID, SnapshotNode> as tuples.
    #[serde(default)]
    snapshots: Vec<(u64, SnapshotNode)>,
    /// Tree mutations per commit — used to reconstruct the tree at each commit.
    #[serde(default)]
    operations: Vec<Vec<i64>>,
}

/// A node in the React component tree snapshot.
#[derive(Debug, Clone, Deserialize)]
struct SnapshotNode {
    #[serde(default)]
    children: Vec<u64>,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
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
}

/// What caused a component to re-render.
#[derive(Debug, Clone, Deserialize)]
struct ChangeDescription {
    #[serde(default, rename = "didHooksChange")]
    did_hooks_change: bool,
    #[serde(default, rename = "isFirstMount")]
    is_first_mount: bool,
    #[serde(default)]
    props: Option<Vec<String>>,
    #[serde(default)]
    state: Option<Vec<String>>,
}

/// Reconstructed component tree at a given commit point.
#[derive(Clone)]
struct FiberTree {
    nodes: std::collections::HashMap<u64, SnapshotNode>,
}

/// React DevTools operation type constants.
/// From facebook/react: packages/react-devtools-shared/src/constants.js
const TREE_OPERATION_ADD: i64 = 1;
const TREE_OPERATION_REMOVE: i64 = 2;
const TREE_OPERATION_REORDER_CHILDREN: i64 = 3;
const TREE_OPERATION_UPDATE_TREE_BASE_DURATION: i64 = 4;
const TREE_OPERATION_UPDATE_ERRORS_OR_WARNINGS: i64 = 5;
const TREE_OPERATION_SET_SUBTREE_MODE: i64 = 7;

impl FiberTree {
    /// Build initial tree from snapshot data.
    fn from_snapshots(snapshots: &[(u64, SnapshotNode)]) -> Self {
        let nodes: std::collections::HashMap<u64, SnapshotNode> =
            snapshots.iter().cloned().collect();
        Self { nodes }
    }

    /// Apply a single commit's operations to mutate this tree in place.
    ///
    /// The operations array is a flat i64 array with a compact binary encoding:
    /// - `[0]` = rootID, `[1]` = renderer ID (both skipped)
    /// - `[2]` = string table size, followed by encoded strings
    /// - Then variable-length operation records
    ///
    /// Encoding matches facebook/react CommitTreeBuilder.js `updateTree()`.
    fn apply_operations(&mut self, ops: &[i64]) {
        if ops.len() < 3 {
            return;
        }

        let mut i: usize = 2;

        // Decode string table.
        let string_table_size = ops[i] as usize;
        i += 1;
        let string_table_end = i + string_table_size;
        let mut string_table: Vec<Option<String>> = vec![None]; // ID 0 = null string

        while i < string_table_end && i < ops.len() {
            let length = ops[i] as usize;
            i += 1;
            // Decode UTF-16 code units stored as i64 values.
            let end = (i + length).min(ops.len());
            let code_units: Vec<u16> = ops[i..end].iter().map(|&v| v as u16).collect();
            let s = String::from_utf16_lossy(&code_units);
            string_table.push(Some(s));
            i = end;
        }

        // Process operations.
        while i < ops.len() {
            let operation = ops[i];

            match operation {
                TREE_OPERATION_ADD => {
                    let id = ops.get(i + 1).copied().unwrap_or(0) as u64;
                    let element_type = ops.get(i + 2).copied().unwrap_or(0) as u32;
                    i += 3;

                    // ElementTypeRoot = 11 in React DevTools
                    if element_type == 11 {
                        // Root node: skip 4 flags
                        i += 4;
                        self.nodes.insert(
                            id,
                            SnapshotNode {
                                children: vec![],
                                display_name: None,
                            },
                        );
                    } else {
                        let parent_id = ops.get(i).copied().unwrap_or(0) as u64;
                        i += 1; // parentID
                        i += 1; // ownerID

                        let display_name_id = ops.get(i).copied().unwrap_or(0) as usize;
                        let display_name = string_table.get(display_name_id).and_then(Clone::clone);
                        i += 1;

                        i += 1; // key

                        // React 19.2+ adds a nameProp field here. Detect by
                        // checking if the next value is a valid operation code
                        // (1-13). If not, it's the nameProp field — consume it.
                        if i < ops.len() {
                            let next = ops[i];
                            let is_valid_op =
                                (1..=5).contains(&next) || next == 7 || (8..=13).contains(&next);
                            if !is_valid_op {
                                i += 1; // nameProp (React 19.2+)
                            }
                        }

                        let node = SnapshotNode {
                            children: vec![],
                            display_name,
                        };
                        self.nodes.insert(id, node);

                        // Add as child of parent.
                        if let Some(parent) = self.nodes.get_mut(&parent_id) {
                            parent.children.push(id);
                        }
                    }
                }
                TREE_OPERATION_REMOVE => {
                    let remove_count = ops.get(i + 1).copied().unwrap_or(0) as usize;
                    i += 2;

                    for _ in 0..remove_count {
                        let id = ops.get(i).copied().unwrap_or(0) as u64;
                        i += 1;

                        // Remove from parent's children list.
                        if let Some(node) = self.nodes.get(&id).cloned() {
                            // Find parent by checking all nodes (snapshots don't
                            // always store parentID reliably).
                            for other in self.nodes.values_mut() {
                                other.children.retain(|&c| c != id);
                            }
                            // Recursively remove children.
                            let mut to_remove = vec![id];
                            to_remove.extend(node.children.iter());
                            for rid in to_remove {
                                self.nodes.remove(&rid);
                            }
                        }
                    }
                }
                TREE_OPERATION_REORDER_CHILDREN => {
                    let id = ops.get(i + 1).copied().unwrap_or(0) as u64;
                    let num_children = ops.get(i + 2).copied().unwrap_or(0) as usize;
                    i += 3;

                    let children: Vec<u64> = ops[i..i + num_children.min(ops.len() - i)]
                        .iter()
                        .map(|&v| v as u64)
                        .collect();
                    i += num_children;

                    if let Some(node) = self.nodes.get_mut(&id) {
                        node.children = children;
                    }
                }
                TREE_OPERATION_UPDATE_TREE_BASE_DURATION => {
                    // [op, id, duration_us] — skip, we use fiberActualDurations
                    i += 3;
                }
                TREE_OPERATION_UPDATE_ERRORS_OR_WARNINGS => {
                    // [op, id, numErrors, numWarnings]
                    i += 4;
                }
                TREE_OPERATION_SET_SUBTREE_MODE => {
                    // [op, id, mode]
                    i += 3;
                }
                _ => {
                    // Unknown operation — skip it (best effort).
                    // The Suspense operations (8-12) and activity slice (13)
                    // are more complex but don't affect the fiber tree for
                    // our purposes. Break to avoid infinite loop.
                    break;
                }
            }
        }
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
        // Durations from React are in ms; convert to µs to match commit_start.
        let actual_durations: std::collections::HashMap<u64, f64> = commit
            .fiber_actual_durations
            .iter()
            .map(|&(id, dur)| (id, dur * 1000.0))
            .collect();
        let self_durations: std::collections::HashMap<u64, f64> = commit
            .fiber_self_durations
            .iter()
            .map(|&(id, dur)| (id, dur * 1000.0))
            .collect();
        let change_descs: std::collections::HashMap<u64, &ChangeDescription> = commit
            .change_descriptions
            .as_ref()
            .map(|descs| descs.iter().map(|(id, desc)| (*id, desc)).collect())
            .unwrap_or_default();

        // Depth-first walk, tracking time offset within the commit.
        let mut stack: Vec<(u64, u32, f64, Option<u64>)> = vec![];

        if actual_durations.get(&root_id).is_some_and(|d| *d > 0.0) {
            stack.push((root_id, 0, commit_start, None));
        } else {
            // Root fiber (e.g. type=11) often has no actualDuration.
            // Start from its children that did render.
            if let Some(root_node) = self.nodes.get(&root_id) {
                for &child_id in &root_node.children {
                    if actual_durations.get(&child_id).is_some_and(|d| *d > 0.0) {
                        stack.push((child_id, 0, commit_start, None));
                    }
                }
            }
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
            let base_tree = FiberTree::from_snapshots(&root.snapshots);

            // Find root fiber IDs from the initial snapshot (fibers that
            // appear in snapshots but not as children of any other fiber).
            let all_children: std::collections::HashSet<u64> = root
                .snapshots
                .iter()
                .flat_map(|(_, node)| node.children.iter().copied())
                .collect();
            let initial_root_ids: Vec<u64> = root
                .snapshots
                .iter()
                .map(|(id, _)| *id)
                .filter(|id| !all_children.contains(id))
                .collect();

            let has_operations = !root.operations.is_empty();

            // Evolve the tree commit by commit, replaying operations.
            let mut tree = base_tree;

            for (commit_idx, commit) in root.commit_data.iter().enumerate() {
                // Apply this commit's operations to evolve the tree.
                if has_operations
                    && commit_idx < root.operations.len()
                    && !root.operations[commit_idx].is_empty()
                {
                    tree.apply_operations(&root.operations[commit_idx]);
                }

                let commit_start = commit.timestamp * 1000.0; // ms → µs
                let commit_end = commit_start + commit.duration * 1000.0;

                global_start = global_start.min(commit_start);
                global_end = global_end.max(commit_end);

                // Recompute root IDs from current tree state — operations
                // may have added/removed root-level fibers.
                let current_root_ids = if has_operations {
                    let child_set: std::collections::HashSet<u64> = tree
                        .nodes
                        .values()
                        .flat_map(|n| n.children.iter().copied())
                        .collect();
                    tree.nodes
                        .keys()
                        .copied()
                        .filter(|id| !child_set.contains(id))
                        .collect::<Vec<_>>()
                } else {
                    initial_root_ids.clone()
                };

                for &root_id in &current_root_ids {
                    tree.walk_commit(root_id, commit, commit_start, &mut frames, &mut next_id);
                }
            }
        } else {
            // Fallback: flat representation without tree structure.
            for commit in &root.commit_data {
                let commit_start = commit.timestamp * 1000.0; // ms → µs
                let commit_end = commit_start + commit.duration * 1000.0;

                global_start = global_start.min(commit_start);
                global_end = global_end.max(commit_end);

                let self_durations: std::collections::HashMap<u64, f64> = commit
                    .fiber_self_durations
                    .iter()
                    .map(|&(id, dur)| (id, dur * 1000.0))
                    .collect();

                let mut offset = commit_start;
                for (fiber_id, actual_duration) in &commit.fiber_actual_durations {
                    let actual_us = actual_duration * 1000.0;
                    if actual_us <= 0.0 {
                        continue;
                    }

                    let self_time = self_durations.get(fiber_id).copied().unwrap_or(0.0);
                    let id = next_id;
                    next_id += 1;

                    frames.push(Frame {
                        id,
                        name: format!("fiber-{fiber_id}"),
                        start: offset,
                        end: offset + actual_us,
                        depth: 0,
                        category: Some("react".to_string()),
                        parent: None,
                        self_time,
                        thread: Some("React Components".to_string()),
                    });

                    offset += actual_us;
                }
            }
        }
    }

    Ok(Profile::new(
        ProfileMetadata {
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
            time_domain: Some(flame_cat_protocol::TimeDomain {
                clock_kind: flame_cat_protocol::ClockKind::PerformanceNow,
                origin_label: Some("React DevTools (performance.now)".into()),
                navigation_start_us: None,
            }),
        },
        frames,
    ))
}

/// Merge React DevTools change descriptions into a Chrome trace Profile.
///
/// When a user loads both a Chrome trace (with React Performance Tracks) and
/// a React DevTools legacy export for the same session, this function annotates
/// the Chrome trace's React component frames with `changeDescriptions` and
/// `updaters` from the legacy export.
///
/// Matching strategy: component name + overlapping time range. The legacy
/// export timestamps are relative to profiling start (ms), while Chrome
/// trace timestamps are absolute (µs). The caller must provide the
/// `profiling_start_us` anchor (the absolute µs timestamp at which React
/// profiling started, typically derivable from the Chrome trace's
/// `navigationStart` event + the legacy export's first commit timestamp).
///
/// Returns the number of frames that were annotated.
pub fn merge_change_descriptions(
    chrome_profile: &mut Profile,
    react_data: &[u8],
    profiling_start_us: f64,
) -> Result<usize, ReactParseError> {
    let export: ReactProfileExport = serde_json::from_slice(react_data)?;

    // Build a lookup: (component_name, commit_start_us, commit_end_us) → category
    let mut annotations: Vec<(String, f64, f64, String)> = Vec::new();

    for root in &export.data_for_roots {
        let tree = if !root.snapshots.is_empty() {
            Some(FiberTree::from_snapshots(&root.snapshots))
        } else {
            None
        };

        for commit in &root.commit_data {
            let commit_start_us = profiling_start_us + commit.timestamp * 1000.0;
            let commit_end_us = commit_start_us + commit.duration * 1000.0;

            if let Some(descs) = &commit.change_descriptions {
                for (fiber_id, desc) in descs {
                    let name = tree
                        .as_ref()
                        .map(|t| t.display_name(*fiber_id))
                        .unwrap_or_else(|| format!("fiber-{fiber_id}"));
                    let category = format_change_description(desc);
                    annotations.push((name, commit_start_us, commit_end_us, category));
                }
            }
        }
    }

    let mut annotated = 0;

    for frame in &mut chrome_profile.frames {
        // Only annotate React component frames from Chrome traces.
        let is_react = frame
            .category
            .as_ref()
            .is_some_and(|c| c.starts_with("react.component"));
        if !is_react {
            continue;
        }

        // Find a matching annotation by name and overlapping time.
        for (name, start, end, category) in &annotations {
            if frame.name == *name && frame.start < *end && frame.end > *start {
                frame.category = Some(category.clone());
                annotated += 1;
                break;
            }
        }
    }

    Ok(annotated)
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
        assert!((app.self_time - 2000.0).abs() < f64::EPSILON); // 2ms = 2000µs

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

        // First commit at t=100ms = 100000µs
        assert!((profile.frames[0].start - 100_000.0).abs() < f64::EPSILON);
        // Second commit at t=200ms = 200000µs
        assert!((profile.frames[2].start - 200_000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn change_description_formatting() {
        let mount = ChangeDescription {
            did_hooks_change: false,
            is_first_mount: true,
            props: None,
            state: None,
        };
        assert_eq!(format_change_description(&mount), "react.mount");

        let props_update = ChangeDescription {
            did_hooks_change: false,
            is_first_mount: false,
            props: Some(vec!["count".to_string(), "label".to_string()]),
            state: None,
        };
        assert_eq!(
            format_change_description(&props_update),
            "react.update(props: count, label)"
        );

        let hooks_update = ChangeDescription {
            did_hooks_change: true,
            is_first_mount: false,
            props: None,
            state: Some(vec!["value".to_string()]),
        };
        assert_eq!(
            format_change_description(&hooks_update),
            "react.update(state: value; hooks)"
        );
    }

    /// Debug test: parse a real React DevTools export and verify correctness
    /// against what React DevTools shows.
    ///
    /// The fixture (metronome app) has:
    /// - Snapshot tree: root(31) → App(32) → Profiler(33) → Homepage(36) → [Header(39), HomepageBody(48)]
    /// - 1 commit at t=2836.4ms, duration 9.5ms
    /// - Operations that add new fibers: Login(61), Header(63),
    ///   ForwardRef(MotionComponent)(69), etc.
    /// - changeDescriptions: fiber 61,63,69,77,85,87,88,95 are first mounts;
    ///   fiber 36 has props ["view"] change; fiber 32 has hooks change.
    ///
    /// React DevTools Flame Graph for Commit 1 shows:
    /// ```
    /// App (9.5ms self 0.7ms)
    ///   Profiler (8.8ms self 0ms)  -- pass-through, no self-time
    ///     Homepage (8.6ms self 0.2ms)
    ///       Login (8.4ms self 1.0ms)
    ///         Header (0.7ms self 0.2ms)
    ///           Context.Provider (0.5ms self 0ms)  -- not in fiberActualDurations?
    ///         ForwardRef(MotionComponent) (6.2ms self 2.9ms)
    ///           ... children
    ///       Context.Provider (0.3ms)
    /// ```
    #[test]
    fn parse_real_metronome_profile() {
        let data = include_bytes!("../../tests/fixtures/react-devtools-metronome.json");
        let profile = parse_react_profile(data).unwrap();

        assert_eq!(profile.metadata.name.as_deref(), Some("App"));
        assert_eq!(profile.metadata.format, "react");

        // Print frames for debugging — cargo test -- --nocapture parse_real_metronome
        let mut sorted = profile.frames.clone();
        sorted.sort_by(|a, b| {
            a.start
                .partial_cmp(&b.start)
                .unwrap()
                .then(b.duration().partial_cmp(&a.duration()).unwrap())
        });

        eprintln!("\n=== Real React Profile: Metronome App ===");
        eprintln!("Frames: {}", profile.frames.len());
        eprintln!(
            "Time range: {:.2}µs → {:.2}µs (dur {:.2}µs)",
            profile.metadata.start_time,
            profile.metadata.end_time,
            profile.duration()
        );

        for f in &sorted {
            let indent = "  ".repeat(f.depth as usize);
            eprintln!(
                "{}[d{}] {} | {:.2}µs → {:.2}µs (dur={:.2}µs self={:.2}µs) | {}",
                indent,
                f.depth,
                f.name,
                f.start,
                f.end,
                f.duration(),
                f.self_time,
                f.category.as_deref().unwrap_or("-"),
            );
        }

        // Verify key properties based on the raw data:
        //
        // Commit 1: timestamp=2836.4ms, duration=9.5ms
        // The fiber tree after operations should contain the new fibers.
        //
        // fiberActualDurations shows these fibers rendered:
        //   32(App)=9.5, 33(Profiler)=8.8, 36(Homepage)=8.6,
        //   61(Login)=8.4, 63(Header)=0.7, 69(ForwardRef(MotionComponent))=6.2,
        //   70=3.3, 77=0.8, 78=0.3, 85=1.2, 87=1.1, 88=0.7, 89=0.7, 95=0.3, 96=0
        //
        // All except 96 should produce frames (96 has 0 duration).

        // Should have 14 frames (15 fibers minus fiber 96 with 0 duration)
        assert_eq!(
            sorted.len(),
            14,
            "Expected 14 frames (all fibers with actualDuration > 0)"
        );

        // App should be the outermost frame
        let app = sorted.iter().find(|f| f.name == "App").unwrap();
        assert_eq!(app.depth, 0, "App should be at depth 0 (root)");
        assert!(
            (app.self_time - 700.0).abs() < 10.0,
            "App self_time should be ~700µs (0.7ms), got {}",
            app.self_time
        );
        assert!(
            (app.duration() - 9500.0).abs() < 10.0,
            "App duration should be ~9500µs (9.5ms)"
        );

        // Verify the commit starts at the correct time
        assert!(
            (app.start - 2_836_400.0).abs() < 10.0,
            "App should start at commit timestamp 2836.4ms = 2836400µs, got {}",
            app.start
        );

        // changeDescriptions: App(32) has hooks change
        assert_eq!(
            app.category.as_deref(),
            Some("react.update(hooks)"),
            "App should show hooks change"
        );

        // Login(61) is first mount
        let login = sorted.iter().find(|f| f.name == "Login").unwrap();
        assert_eq!(
            login.category.as_deref(),
            Some("react.mount"),
            "Login should be first mount"
        );

        // Homepage(36) has props ["view"] change
        let homepage = sorted.iter().find(|f| f.name == "Homepage").unwrap();
        assert_eq!(
            homepage.category.as_deref(),
            Some("react.update(props: view)"),
            "Homepage should show props:view change"
        );

        // Verify nesting: App → Profiler → Homepage → Login
        let profiler = sorted.iter().find(|f| f.name == "Profiler").unwrap();
        assert_eq!(
            profiler.parent,
            Some(app.id),
            "Profiler should be child of App"
        );
        assert_eq!(
            homepage.parent,
            Some(profiler.id),
            "Homepage should be child of Profiler"
        );
        assert_eq!(
            login.parent,
            Some(homepage.id),
            "Login should be child of Homepage"
        );

        // All frames should be on the React Components thread
        assert!(
            sorted
                .iter()
                .all(|f| f.thread.as_deref() == Some("React Components"))
        );

        eprintln!("\n=== Comparison with React DevTools ===");
        eprintln!("React DevTools Flame Graph shows Commit 1:");
        eprintln!("  App (9.5ms of 9.5ms) — hooks changed");
        eprintln!("    Profiler (8.8ms of 8.8ms) — no self-time");
        eprintln!("      Homepage (8.6ms of 8.6ms) — props: view");
        eprintln!("        Login (8.4ms of 8.4ms) — first mount");
        eprintln!("          Header (0.7ms) — first mount");
        eprintln!("          ForwardRef(MotionComponent) (6.2ms) — first mount");
        eprintln!("          ... etc");
        eprintln!("Our parser should produce matching tree structure ↑");
    }

    #[test]
    fn operations_replay_adds_fibers() {
        // Test that operations correctly add new fibers to the tree.
        //
        // operations encoding: [rootID, rendererID, stringTableSize, ...strings, ...ops]
        // TREE_OPERATION_ADD = 1: [op, id, type, parentID, ownerID, displayNameStrID, keyStrID, nameStrID]
        let ops: Vec<i64> = vec![
            1, // rootID
            0, // rendererID
            // String table: size=9, one string "NewChild" (8 chars)
            9, 8, 78, 101, 119, 67, 104, 105, 108, 100, // "NewChild" as UTF-16 code units
            // TREE_OPERATION_ADD non-root: [1, id=10, type=5, parentID=1, ownerID=0, nameStrID=1, keyStrID=0, namePropStrID=0]
            1, 10, 5, 1, 0, 1, 0, 0,
        ];

        let mut tree = FiberTree::from_snapshots(&[(
            1,
            SnapshotNode {
                children: vec![],
                display_name: Some("Root".to_string()),
            },
        )]);

        assert_eq!(tree.nodes.len(), 1);
        tree.apply_operations(&ops);
        assert_eq!(tree.nodes.len(), 2, "Should have added a new fiber");

        let new_node = tree.nodes.get(&10).expect("Fiber 10 should exist");
        assert_eq!(new_node.display_name.as_deref(), Some("NewChild"));

        // Root should now have fiber 10 as a child
        let root = tree.nodes.get(&1).expect("Root should exist");
        assert!(root.children.contains(&10), "Root should contain child 10");
    }

    #[test]
    fn operations_replay_removes_fibers() {
        let ops: Vec<i64> = vec![
            1, 0, // rootID, rendererID
            0, // stringTableSize = 0 (no strings)
            // TREE_OPERATION_REMOVE = 2: [op, count, id1, ...]
            2, 1, 2, // remove 1 fiber: fiber 2
        ];

        let mut tree = FiberTree::from_snapshots(&[
            (
                1,
                SnapshotNode {
                    children: vec![2],
                    display_name: Some("Root".to_string()),
                },
            ),
            (
                2,
                SnapshotNode {
                    children: vec![],
                    display_name: Some("Child".to_string()),
                },
            ),
        ]);

        assert_eq!(tree.nodes.len(), 2);
        tree.apply_operations(&ops);
        assert_eq!(tree.nodes.len(), 1, "Should have removed fiber 2");
        assert!(tree.nodes.get(&2).is_none(), "Fiber 2 should be gone");

        // Root should no longer have fiber 2 as a child
        let root = tree.nodes.get(&1).expect("Root should exist");
        assert!(
            !root.children.contains(&2),
            "Root should not contain child 2"
        );
    }

    #[test]
    fn parse_react_devtools_demo() {
        let data = include_bytes!("../../../ui/assets/react-devtools-demo.json");
        let profile = parse_react_profile(data).unwrap();

        assert_eq!(profile.metadata.format, "react");
        assert_eq!(profile.metadata.name.as_deref(), Some("App"));

        // 12 commits × multiple fibers each = 200+ frames
        assert!(
            profile.frames.len() > 100,
            "Expected 100+ frames across 12 commits, got {}",
            profile.frames.len()
        );

        // Verify we have realistic component names
        let names: std::collections::HashSet<&str> =
            profile.frames.iter().map(|f| f.name.as_str()).collect();
        assert!(names.contains("ProductGrid"), "Should have ProductGrid");
        assert!(names.contains("Header"), "Should have Header");
        assert!(names.contains("SearchBar"), "Should have SearchBar");
        assert!(names.contains("CartDrawer"), "Should have CartDrawer");

        eprintln!(
            "React DevTools demo: {} frames, {:.0}µs → {:.0}µs",
            profile.frames.len(),
            profile.metadata.start_time,
            profile.metadata.end_time
        );
    }
}
