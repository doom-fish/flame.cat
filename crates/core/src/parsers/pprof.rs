use serde::Deserialize;
use thiserror::Error;

use crate::model::{Frame, Profile, ProfileMetadata};

#[derive(Debug, Error)]
pub enum PprofParseError {
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("no samples found")]
    NoSamples,
}

/// pprof JSON format (as produced by `go tool pprof -json` or pprof-rs JSON export).
///
/// This handles the JSON representation of pprof data. For binary protobuf pprof,
/// convert first with `go tool pprof -proto` then export to JSON.
#[derive(Debug, Deserialize)]
struct PprofJson {
    #[serde(default, rename = "sampleType")]
    #[allow(dead_code)]
    sample_type: Vec<PprofValueType>,
    #[serde(default)]
    samples: Vec<PprofSample>,
    #[serde(default)]
    locations: Vec<PprofLocation>,
    #[serde(default)]
    functions: Vec<PprofFunction>,
    #[serde(default, rename = "stringTable")]
    string_table: Vec<String>,
    #[serde(default, rename = "durationNanos")]
    duration_nanos: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct PprofValueType {
    #[serde(default, rename = "type")]
    #[allow(dead_code)]
    value_type: Option<u64>,
    #[serde(default)]
    #[allow(dead_code)]
    unit: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct PprofSample {
    #[serde(default, rename = "locationId")]
    location_id: Vec<u64>,
    #[serde(default)]
    value: Vec<i64>,
}

#[derive(Debug, Deserialize)]
struct PprofLocation {
    id: u64,
    #[serde(default)]
    line: Vec<PprofLine>,
}

#[derive(Debug, Deserialize)]
struct PprofLine {
    #[serde(default, rename = "functionId")]
    function_id: u64,
}

#[derive(Debug, Deserialize)]
struct PprofFunction {
    id: u64,
    #[serde(default)]
    name: u64,
    #[serde(default, rename = "filename")]
    file_name: Option<u64>,
}

/// Parse a pprof JSON export into a `Profile`.
pub fn parse_pprof(data: &[u8]) -> Result<Profile, PprofParseError> {
    let pprof: PprofJson = serde_json::from_slice(data)?;

    if pprof.samples.is_empty() {
        return Err(PprofParseError::NoSamples);
    }

    // Build lookups.
    let func_map: std::collections::HashMap<u64, &PprofFunction> =
        pprof.functions.iter().map(|f| (f.id, f)).collect();
    let loc_map: std::collections::HashMap<u64, &PprofLocation> =
        pprof.locations.iter().map(|l| (l.id, l)).collect();

    let resolve_name = |loc_id: u64| -> String {
        if let Some(loc) = loc_map.get(&loc_id)
            && let Some(line) = loc.line.first()
            && let Some(func) = func_map.get(&line.function_id)
            && let Some(name) = pprof.string_table.get(func.name as usize)
            && !name.is_empty()
        {
            return name.clone();
        }
        format!("loc-{loc_id}")
    };

    let resolve_file = |loc_id: u64| -> Option<String> {
        let loc = loc_map.get(&loc_id)?;
        let line = loc.line.first()?;
        let func = func_map.get(&line.function_id)?;
        let file_idx = func.file_name? as usize;
        pprof.string_table.get(file_idx).cloned()
    };

    let mut frames: Vec<Frame> = Vec::new();
    let mut next_id: u64 = 0;
    let mut offset: f64 = 0.0;

    for sample in &pprof.samples {
        let weight = sample.value.first().copied().unwrap_or(1) as f64;
        let sample_end = offset + weight;

        // pprof stacks are leaf-first; reverse to get root-first.
        let stack: Vec<u64> = sample.location_id.iter().copied().rev().collect();

        let mut parent_id: Option<u64> = None;
        for (depth, &loc_id) in stack.iter().enumerate() {
            let name = resolve_name(loc_id);
            let category = resolve_file(loc_id);
            let is_leaf = depth == stack.len() - 1;

            let id = next_id;
            next_id += 1;

            frames.push(Frame {
                id,
                name,
                start: offset,
                end: sample_end,
                depth: depth as u32,
                category,
                parent: parent_id,
                self_time: if is_leaf { weight } else { 0.0 },
                thread: None,
            });

            parent_id = Some(id);
        }

        offset = sample_end;
    }

    // Recompute self_time properly.
    let child_time = {
        let mut map = std::collections::HashMap::<u64, f64>::new();
        for f in &frames {
            if let Some(pid) = f.parent {
                *map.entry(pid).or_default() += f.duration();
            }
        }
        map
    };
    for f in &mut frames {
        let children_total = child_time.get(&f.id).copied().unwrap_or(0.0);
        f.self_time = (f.duration() - children_total).max(0.0);
    }

    let duration_us = pprof
        .duration_nanos
        .map(|ns| ns as f64 / 1000.0)
        .unwrap_or(offset);

    Ok(Profile::new(ProfileMetadata {
            name: None,
            start_time: 0.0,
            end_time: if duration_us > 0.0 {
                duration_us
            } else {
                offset
            },
            format: "pprof".to_string(),
            time_domain: None,
        },
        frames,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_pprof() {
        let json = r#"{
            "sampleType": [{"type": 1, "unit": 2}],
            "samples": [
                {"locationId": [3, 2, 1], "value": [10]},
                {"locationId": [3, 2], "value": [20]}
            ],
            "locations": [
                {"id": 1, "line": [{"functionId": 1}]},
                {"id": 2, "line": [{"functionId": 2}]},
                {"id": 3, "line": [{"functionId": 3}]}
            ],
            "functions": [
                {"id": 1, "name": 0},
                {"id": 2, "name": 1},
                {"id": 3, "name": 2}
            ],
            "stringTable": ["main", "work", "compute"]
        }"#;

        let profile = parse_pprof(json.as_bytes()).unwrap();
        assert_eq!(profile.metadata.format, "pprof");
        // First sample: main -> work -> compute (3 frames)
        // Second sample: work -> compute (2 frames)
        assert_eq!(profile.frames.len(), 5);

        let main_f = profile.frames.iter().find(|f| f.name == "main").unwrap();
        assert_eq!(main_f.depth, 0);
    }

    #[test]
    fn empty_samples_errors() {
        let json = r#"{"samples":[],"locations":[],"functions":[],"stringTable":[]}"#;
        assert!(parse_pprof(json.as_bytes()).is_err());
    }
}
