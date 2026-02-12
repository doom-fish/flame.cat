use serde::Deserialize;
use thiserror::Error;

use crate::model::{Frame, Profile, ProfileMetadata};

#[derive(Debug, Error)]
pub enum PixParseError {
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("no events found")]
    NoEvents,
}

/// PIX (Microsoft Performance Investigator for Xbox) JSON timing capture export.
///
/// PIX exports GPU/CPU timing data. When exported as JSON, it typically uses
/// a Chrome-trace-compatible format with additional PIX-specific metadata,
/// or a simpler events array with marker/region data.
#[derive(Debug, Deserialize)]
struct PixExport {
    #[serde(default)]
    events: Vec<PixEvent>,
    #[serde(default)]
    info: Option<PixInfo>,
}

#[derive(Debug, Deserialize)]
struct PixInfo {
    #[serde(default, rename = "captureTitle")]
    capture_title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PixEvent {
    name: String,
    #[serde(default)]
    category: Option<String>,
    start: f64,
    #[serde(default)]
    end: Option<f64>,
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default)]
    depth: Option<u32>,
    #[serde(default)]
    thread: Option<String>,
    #[serde(default)]
    children: Vec<PixEvent>,
}

/// Parse a PIX JSON timing export into a `Profile`.
pub fn parse_pix(data: &[u8]) -> Result<Profile, PixParseError> {
    let export: PixExport = serde_json::from_slice(data)?;

    let mut frames: Vec<Frame> = Vec::new();
    let mut next_id: u64 = 0;

    for event in &export.events {
        flatten_pix_event(event, 0, None, &mut frames, &mut next_id);
    }

    if frames.is_empty() {
        return Err(PixParseError::NoEvents);
    }

    compute_self_times(&mut frames);

    let start_time = frames.iter().map(|f| f.start).fold(f64::INFINITY, f64::min);
    let end_time = frames
        .iter()
        .map(|f| f.end)
        .fold(f64::NEG_INFINITY, f64::max);

    Ok(Profile::new(
        ProfileMetadata {
            name: export.info.and_then(|i| i.capture_title),
            start_time: if start_time.is_finite() {
                start_time
            } else {
                0.0
            },
            end_time: if end_time.is_finite() { end_time } else { 0.0 },
            format: "pix".to_string(),
            time_domain: None,
        },
        frames,
    ))
}

fn flatten_pix_event(
    event: &PixEvent,
    depth: u32,
    parent_id: Option<u64>,
    frames: &mut Vec<Frame>,
    next_id: &mut u64,
) {
    let id = *next_id;
    *next_id += 1;

    let actual_depth = event.depth.unwrap_or(depth);
    let end = event
        .end
        .or_else(|| event.duration.map(|d| event.start + d))
        .unwrap_or(event.start);

    frames.push(Frame {
        id,
        name: event.name.clone(),
        start: event.start,
        end,
        depth: actual_depth,
        category: event.category.clone().or_else(|| event.thread.clone()),
        parent: parent_id,
        self_time: 0.0,
        thread: None,
    });

    for child in &event.children {
        flatten_pix_event(child, actual_depth + 1, Some(id), frames, next_id);
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
    fn parse_pix_events() {
        let json = r#"{
            "info": {"captureTitle": "GPU Frame"},
            "events": [{
                "name": "RenderFrame",
                "category": "GPU",
                "start": 0,
                "end": 16000,
                "children": [
                    {"name": "ShadowPass", "start": 0, "end": 4000, "children": []},
                    {"name": "MainPass", "start": 4000, "end": 12000, "children": [
                        {"name": "DrawMeshes", "start": 5000, "end": 10000, "children": []}
                    ]},
                    {"name": "PostProcess", "start": 12000, "end": 15000, "children": []}
                ]
            }]
        }"#;

        let profile = parse_pix(json.as_bytes()).unwrap();
        assert_eq!(profile.metadata.format, "pix");
        assert_eq!(profile.metadata.name.as_deref(), Some("GPU Frame"));
        assert_eq!(profile.frames.len(), 5);

        let render = &profile.frames[0];
        assert_eq!(render.name, "RenderFrame");
        assert_eq!(render.depth, 0);
    }

    #[test]
    fn parse_with_duration() {
        let json = r#"{"events":[{"name":"A","start":0,"duration":100,"children":[]}]}"#;
        let profile = parse_pix(json.as_bytes()).unwrap();
        assert_eq!(profile.frames[0].end, 100.0);
    }

    #[test]
    fn empty_events_errors() {
        let json = r#"{"events":[]}"#;
        assert!(parse_pix(json.as_bytes()).is_err());
    }
}
