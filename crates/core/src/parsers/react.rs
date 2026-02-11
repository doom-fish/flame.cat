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

/// React DevTools profiler export format.
/// The export contains a list of commits, each with a tree of components.
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
}

#[derive(Debug, Deserialize)]
struct ReactCommit {
    #[serde(rename = "fiberActualDurations")]
    fiber_actual_durations: Vec<(u64, f64)>,
    #[serde(rename = "fiberSelfDurations")]
    fiber_self_durations: Vec<(u64, f64)>,
    timestamp: f64,
    duration: f64,
}

/// Parse a React DevTools profiler export into a `Profile`.
///
/// Maps each commit's component tree into frames. Each component becomes
/// a frame with start = commit timestamp, duration = actual render duration.
pub fn parse_react_profile(data: &[u8]) -> Result<Profile, ReactParseError> {
    let export: ReactProfileExport = serde_json::from_slice(data)?;

    let mut frames = Vec::new();
    let mut next_id: u64 = 0;

    let mut global_start = f64::INFINITY;
    let mut global_end = f64::NEG_INFINITY;

    for root in &export.data_for_roots {
        for commit in &root.commit_data {
            let commit_start = commit.timestamp;
            let commit_end = commit_start + commit.duration;

            global_start = global_start.min(commit_start);
            global_end = global_end.max(commit_end);

            // Build self-duration lookup.
            let self_durations: std::collections::HashMap<u64, f64> =
                commit.fiber_self_durations.iter().copied().collect();

            // Each fiber becomes a frame within this commit.
            let mut depth = 0u32;
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
                    depth,
                    category: Some("react".to_string()),
                    parent: None, // simplified — full tree reconstruction requires the fiber tree
                    self_time,
                    thread: None,
                });

                offset += actual_duration;
                depth = (depth + 1) % 8; // rotate depths to create visual layering
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
}
