use serde::Deserialize;
use thiserror::Error;

use crate::model::{Frame, Profile, ProfileMetadata};

#[derive(Debug, Error)]
pub enum SpeedscopeParseError {
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported schema or missing profiles")]
    Unsupported,
}

/// Speedscope file format — supports evented and sampled profile types.
/// Schema: https://www.speedscope.app/file-format-spec.json
#[derive(Debug, Deserialize)]
struct SpeedscopeFile {
    #[serde(rename = "$schema")]
    #[allow(dead_code)]
    schema: Option<String>,
    #[serde(default)]
    shared: Option<SharedData>,
    profiles: Vec<SpeedscopeProfile>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SharedData {
    frames: Vec<SpeedscopeFrame>,
}

#[derive(Debug, Deserialize)]
struct SpeedscopeFrame {
    name: String,
    #[serde(default)]
    file: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    line: Option<u64>,
    #[serde(default)]
    #[allow(dead_code)]
    col: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum SpeedscopeProfile {
    #[serde(rename = "evented")]
    Evented {
        #[allow(dead_code)]
        name: Option<String>,
        #[allow(dead_code)]
        unit: String,
        #[serde(rename = "startValue")]
        start_value: f64,
        #[serde(rename = "endValue")]
        end_value: f64,
        events: Vec<SpeedscopeEvent>,
    },
    #[serde(rename = "sampled")]
    Sampled {
        #[allow(dead_code)]
        name: Option<String>,
        #[allow(dead_code)]
        unit: String,
        #[serde(rename = "startValue")]
        start_value: f64,
        #[serde(rename = "endValue")]
        end_value: f64,
        samples: Vec<Vec<usize>>,
        weights: Vec<f64>,
    },
}

#[derive(Debug, Deserialize)]
struct SpeedscopeEvent {
    #[serde(rename = "type")]
    event_type: String, // "O" (open) or "C" (close)
    frame: usize,
    at: f64,
}

/// Parse a speedscope JSON file into a `Profile`.
pub fn parse_speedscope(data: &[u8]) -> Result<Profile, SpeedscopeParseError> {
    let file: SpeedscopeFile = serde_json::from_slice(data)?;

    let shared_frames = file.shared.as_ref().map(|s| &s.frames[..]).unwrap_or(&[]);

    if file.profiles.is_empty() {
        return Err(SpeedscopeParseError::Unsupported);
    }

    let mut frames: Vec<Frame> = Vec::new();
    let mut next_id: u64 = 0;
    let mut global_start = f64::INFINITY;
    let mut global_end = f64::NEG_INFINITY;

    for profile in &file.profiles {
        match profile {
            SpeedscopeProfile::Evented {
                start_value,
                end_value,
                events,
                ..
            } => {
                global_start = global_start.min(*start_value);
                global_end = global_end.max(*end_value);

                // Process open/close events using a stack.
                let mut stack: Vec<usize> = Vec::new(); // indices into frames vec

                for event in events {
                    match event.event_type.as_str() {
                        "O" => {
                            let name = shared_frames
                                .get(event.frame)
                                .map(|f| f.name.clone())
                                .unwrap_or_else(|| format!("frame-{}", event.frame));
                            let category =
                                shared_frames.get(event.frame).and_then(|f| f.file.clone());

                            let parent_id = stack.last().map(|&idx| frames[idx].id);
                            let depth = stack.len() as u32;

                            let id = next_id;
                            next_id += 1;
                            let frame_idx = frames.len();

                            frames.push(Frame {
                                id,
                                name,
                                start: event.at,
                                end: event.at, // updated on close
                                depth,
                                category,
                                parent: parent_id,
                                self_time: 0.0,
                                thread: None,
                            });

                            stack.push(frame_idx);
                        }
                        "C" => {
                            if let Some(frame_idx) = stack.pop() {
                                frames[frame_idx].end = event.at;
                            }
                        }
                        _ => {}
                    }
                }
            }
            SpeedscopeProfile::Sampled {
                start_value,
                end_value,
                samples,
                weights,
                ..
            } => {
                global_start = global_start.min(*start_value);
                global_end = global_end.max(*end_value);

                let mut offset = *start_value;

                for (i, sample) in samples.iter().enumerate() {
                    let weight = weights.get(i).copied().unwrap_or(1.0);
                    let sample_end = offset + weight;

                    let mut parent_id: Option<u64> = None;
                    for (depth, &frame_idx) in sample.iter().enumerate() {
                        let name = shared_frames
                            .get(frame_idx)
                            .map(|f| f.name.clone())
                            .unwrap_or_else(|| format!("frame-{frame_idx}"));
                        let category = shared_frames.get(frame_idx).and_then(|f| f.file.clone());

                        let id = next_id;
                        next_id += 1;
                        let is_leaf = depth == sample.len() - 1;

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
            }
        }
    }

    // Compute self times for evented profiles.
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

    Ok(Profile::new(ProfileMetadata {
            name: file.name,
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
            format: "speedscope".to_string(),
            time_domain: None,
        },
        frames,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_evented_profile() {
        let json = r#"{
            "$schema": "https://www.speedscope.app/file-format-spec.json",
            "shared": {
                "frames": [
                    {"name": "main"},
                    {"name": "foo", "file": "foo.js"},
                    {"name": "bar"}
                ]
            },
            "profiles": [{
                "type": "evented",
                "name": "thread 0",
                "unit": "microseconds",
                "startValue": 0,
                "endValue": 100,
                "events": [
                    {"type": "O", "frame": 0, "at": 0},
                    {"type": "O", "frame": 1, "at": 10},
                    {"type": "C", "frame": 1, "at": 50},
                    {"type": "O", "frame": 2, "at": 60},
                    {"type": "C", "frame": 2, "at": 80},
                    {"type": "C", "frame": 0, "at": 100}
                ]
            }],
            "name": "test profile"
        }"#;

        let profile = parse_speedscope(json.as_bytes()).unwrap();
        assert_eq!(profile.metadata.format, "speedscope");
        assert_eq!(profile.metadata.name.as_deref(), Some("test profile"));
        assert_eq!(profile.frames.len(), 3);

        let main_f = &profile.frames[0];
        assert_eq!(main_f.name, "main");
        assert_eq!(main_f.depth, 0);
        assert_eq!(main_f.duration(), 100.0);

        let foo = &profile.frames[1];
        assert_eq!(foo.name, "foo");
        assert_eq!(foo.depth, 1);
        assert_eq!(foo.category.as_deref(), Some("foo.js"));
    }

    #[test]
    fn parse_sampled_profile() {
        let json = r#"{
            "shared": {
                "frames": [
                    {"name": "main"},
                    {"name": "work"}
                ]
            },
            "profiles": [{
                "type": "sampled",
                "name": "samples",
                "unit": "milliseconds",
                "startValue": 0,
                "endValue": 30,
                "samples": [[0, 1], [0, 1], [0]],
                "weights": [10, 10, 10]
            }]
        }"#;

        let profile = parse_speedscope(json.as_bytes()).unwrap();
        assert_eq!(profile.metadata.format, "speedscope");
        assert_eq!(profile.frames.len(), 5); // 2+2+1
    }

    #[test]
    fn empty_profiles_errors() {
        let json = r#"{"shared":{"frames":[]},"profiles":[]}"#;
        assert!(parse_speedscope(json.as_bytes()).is_err());
    }
}
