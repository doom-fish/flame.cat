use serde::Deserialize;
use thiserror::Error;

use crate::model::{Frame, Profile, ProfileMetadata};

#[derive(Debug, Error)]
pub enum TracyParseError {
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("no zone data found")]
    NoZones,
}

/// Tracy profiler JSON export — zone events with source location info.
///
/// Tracy can export to JSON via `tracy-export` or the GUI's "Save as Chrome trace" option.
/// This parser handles Tracy's native JSON export format with `traceEvents` (Chrome-compat)
/// and Tracy's own zone-based format.
///
/// Tracy-native format has `zones` arrays inside `threads`.
#[derive(Debug, Deserialize)]
struct TracyExport {
    #[serde(default)]
    threads: Vec<TracyThread>,
    #[serde(default)]
    info: Option<TracyInfo>,
}

#[derive(Debug, Deserialize)]
struct TracyInfo {
    #[serde(default, rename = "appName")]
    app_name: Option<String>,
    #[serde(default, rename = "captureTime")]
    #[allow(dead_code)]
    capture_time: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct TracyThread {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    zones: Vec<TracyZone>,
}

#[derive(Debug, Deserialize)]
struct TracyZone {
    name: String,
    #[serde(rename = "srcloc")]
    #[allow(dead_code)]
    src_loc: Option<String>,
    start: f64,
    end: f64,
    #[serde(default)]
    children: Vec<TracyZone>,
}

/// Parse a Tracy profiler JSON export into a `Profile`.
pub fn parse_tracy(data: &[u8]) -> Result<Profile, TracyParseError> {
    let export: TracyExport = serde_json::from_slice(data)?;

    let mut frames: Vec<Frame> = Vec::new();
    let mut next_id: u64 = 0;

    for thread in &export.threads {
        for zone in &thread.zones {
            flatten_zone(
                zone,
                0,
                None,
                thread.name.as_deref(),
                &mut frames,
                &mut next_id,
            );
        }
    }

    if frames.is_empty() {
        return Err(TracyParseError::NoZones);
    }

    compute_self_times(&mut frames);

    let start_time = frames.iter().map(|f| f.start).fold(f64::INFINITY, f64::min);
    let end_time = frames
        .iter()
        .map(|f| f.end)
        .fold(f64::NEG_INFINITY, f64::max);

    Ok(Profile::new(ProfileMetadata {
            name: export.info.and_then(|i| i.app_name),
            start_time: if start_time.is_finite() {
                start_time
            } else {
                0.0
            },
            end_time: if end_time.is_finite() { end_time } else { 0.0 },
            format: "tracy".to_string(),
            time_domain: None,
        },
        frames,
    ))
}

fn flatten_zone(
    zone: &TracyZone,
    depth: u32,
    parent_id: Option<u64>,
    thread_name: Option<&str>,
    frames: &mut Vec<Frame>,
    next_id: &mut u64,
) {
    let id = *next_id;
    *next_id += 1;

    frames.push(Frame {
        id,
        name: zone.name.clone(),
        start: zone.start,
        end: zone.end,
        depth,
        category: thread_name.map(ToString::to_string),
        parent: parent_id,
        self_time: 0.0,
        thread: None,
    });

    for child in &zone.children {
        flatten_zone(child, depth + 1, Some(id), thread_name, frames, next_id);
    }
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
    fn parse_tracy_zones() {
        let json = r#"{
            "info": {"appName": "MyApp"},
            "threads": [{
                "name": "Main",
                "zones": [{
                    "name": "Update",
                    "start": 0,
                    "end": 1000,
                    "children": [
                        {"name": "Physics", "start": 100, "end": 400, "children": []},
                        {"name": "Render", "start": 500, "end": 900, "children": [
                            {"name": "Draw", "start": 600, "end": 800, "children": []}
                        ]}
                    ]
                }]
            }]
        }"#;

        let profile = parse_tracy(json.as_bytes()).unwrap();
        assert_eq!(profile.metadata.format, "tracy");
        assert_eq!(profile.metadata.name.as_deref(), Some("MyApp"));
        assert_eq!(profile.frames.len(), 4);

        let update = &profile.frames[0];
        assert_eq!(update.name, "Update");
        assert_eq!(update.depth, 0);

        let draw = profile.frames.iter().find(|f| f.name == "Draw").unwrap();
        assert_eq!(draw.depth, 2);
    }

    #[test]
    fn empty_zones_errors() {
        let json = r#"{"threads":[{"name":"t","zones":[]}]}"#;
        assert!(parse_tracy(json.as_bytes()).is_err());
    }
}
