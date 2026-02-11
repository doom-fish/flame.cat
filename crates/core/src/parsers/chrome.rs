use serde::Deserialize;
use thiserror::Error;

use crate::model::{Frame, Profile, ProfileMetadata};

#[derive(Debug, Error)]
pub enum ChromeParseError {
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("missing traceEvents array")]
    MissingTraceEvents,
}

/// Raw Chrome trace event as found in DevTools JSON exports.
#[derive(Debug, Clone, Deserialize)]
struct TraceEvent {
    #[serde(default)]
    name: String,
    #[serde(default)]
    cat: String,
    ph: String,
    ts: f64,
    #[serde(default)]
    dur: Option<f64>,
    #[serde(default)]
    pid: u64,
    #[serde(default)]
    tid: u64,
    #[serde(default)]
    args: Option<serde_json::Value>,
}

/// Top-level Chrome trace JSON — supports both array format and object format.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TraceFile {
    Object {
        #[serde(rename = "traceEvents")]
        trace_events: Vec<TraceEvent>,
    },
    Array(Vec<TraceEvent>),
}

/// Parse a Chrome DevTools trace JSON into a `Profile`.
pub fn parse_chrome_trace(data: &[u8]) -> Result<Profile, ChromeParseError> {
    let trace_file: TraceFile = serde_json::from_slice(data)?;
    let events = match trace_file {
        TraceFile::Object { trace_events } => trace_events,
        TraceFile::Array(events) => events,
    };

    // Collect thread name metadata
    let mut thread_names: std::collections::HashMap<(u64, u64), String> =
        std::collections::HashMap::new();
    for event in &events {
        if event.ph == "M"
            && event.name == "thread_name"
            && let Some(name) = event
                .args
                .as_ref()
                .and_then(|a| a.get("name"))
                .and_then(|n| n.as_str())
        {
            thread_names.insert((event.pid, event.tid), name.to_string());
        }
    }

    let mut frames: Vec<Frame> = Vec::new();
    let mut next_id: u64 = 0;

    // Stack of (frame_index, event) for matching B/E pairs per thread.
    let mut stacks: std::collections::HashMap<(u64, u64), Vec<usize>> =
        std::collections::HashMap::new();

    // Collect complete (X) events and matched B/E pairs.
    let mut sorted_events: Vec<TraceEvent> = events
        .into_iter()
        .filter(|e| matches!(e.ph.as_str(), "X" | "B" | "E"))
        .collect();
    sorted_events.sort_by(|a, b| a.ts.partial_cmp(&b.ts).unwrap_or(std::cmp::Ordering::Equal));

    for event in &sorted_events {
        let key = (event.pid, event.tid);
        let thread_name = thread_names.get(&key).cloned();

        // Pop completed X events from the stack before processing the next event.
        if let Some(stack) = stacks.get_mut(&key) {
            while let Some(&top_idx) = stack.last() {
                let top = &frames[top_idx];
                // Only auto-pop X events (B events are popped by their E counterpart)
                if top.end > top.start && top.end <= event.ts {
                    stack.pop();
                } else {
                    break;
                }
            }
        }

        match event.ph.as_str() {
            "X" => {
                let dur = event.dur.unwrap_or(0.0);
                let depth = stacks.entry(key).or_default().len() as u32;
                let parent_id = stacks
                    .get(&key)
                    .and_then(|s| s.last())
                    .map(|&idx| frames[idx].id);

                let id = next_id;
                next_id += 1;
                let frame_idx = frames.len();
                frames.push(Frame {
                    id,
                    name: event.name.clone(),
                    start: event.ts,
                    end: event.ts + dur,
                    depth,
                    category: if event.cat.is_empty() {
                        None
                    } else {
                        Some(event.cat.clone())
                    },
                    parent: parent_id,
                    self_time: 0.0, // computed below
                    thread: thread_name,
                });
                // Keep X events on stack so nested children get correct depth
                stacks.entry(key).or_default().push(frame_idx);
            }
            "B" => {
                let depth = stacks.entry(key).or_default().len() as u32;
                let parent_id = stacks
                    .get(&key)
                    .and_then(|s| s.last())
                    .map(|&idx| frames[idx].id);

                let id = next_id;
                next_id += 1;
                let frame_idx = frames.len();
                frames.push(Frame {
                    id,
                    name: event.name.clone(),
                    start: event.ts,
                    end: event.ts, // will be updated on E
                    depth,
                    category: if event.cat.is_empty() {
                        None
                    } else {
                        Some(event.cat.clone())
                    },
                    parent: parent_id,
                    self_time: 0.0,
                    thread: thread_name,
                });
                stacks.entry(key).or_default().push(frame_idx);
            }
            "E" => {
                if let Some(frame_idx) = stacks.entry(key).or_default().pop() {
                    frames[frame_idx].end = event.ts;
                }
            }
            _ => {}
        }
    }

    // Compute self_time = duration - sum(children durations)
    let child_time: std::collections::HashMap<u64, f64> = {
        let mut map: std::collections::HashMap<u64, f64> = std::collections::HashMap::new();
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

    let start_time = frames.iter().map(|f| f.start).fold(f64::INFINITY, f64::min);
    let end_time = frames
        .iter()
        .map(|f| f.end)
        .fold(f64::NEG_INFINITY, f64::max);

    Ok(Profile {
        metadata: ProfileMetadata {
            name: None,
            start_time: if start_time.is_finite() {
                start_time
            } else {
                0.0
            },
            end_time: if end_time.is_finite() { end_time } else { 0.0 },
            format: "chrome".to_string(),
        },
        frames,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_complete_events() {
        let json = r#"{"traceEvents":[
            {"name":"main","ph":"X","ts":0,"dur":100,"pid":1,"tid":1,"cat":""},
            {"name":"child","ph":"X","ts":10,"dur":40,"pid":1,"tid":1,"cat":"func"}
        ]}"#;

        let profile = parse_chrome_trace(json.as_bytes()).unwrap();
        assert_eq!(profile.frames.len(), 2);
        assert_eq!(profile.metadata.format, "chrome");

        let main_frame = &profile.frames[0];
        assert_eq!(main_frame.name, "main");
        assert_eq!(main_frame.depth, 0);
        assert_eq!(main_frame.duration(), 100.0);

        let child = &profile.frames[1];
        assert_eq!(child.name, "child");
        assert_eq!(child.depth, 1);
        assert_eq!(child.parent, Some(main_frame.id));
        assert_eq!(child.category.as_deref(), Some("func"));
    }

    #[test]
    fn parse_begin_end_events() {
        let json = r#"[
            {"name":"outer","ph":"B","ts":0,"pid":1,"tid":1,"cat":""},
            {"name":"inner","ph":"B","ts":10,"pid":1,"tid":1,"cat":""},
            {"name":"inner","ph":"E","ts":50,"pid":1,"tid":1,"cat":""},
            {"name":"outer","ph":"E","ts":100,"pid":1,"tid":1,"cat":""}
        ]"#;

        let profile = parse_chrome_trace(json.as_bytes()).unwrap();
        assert_eq!(profile.frames.len(), 2);

        let outer = &profile.frames[0];
        assert_eq!(outer.name, "outer");
        assert_eq!(outer.depth, 0);
        assert_eq!(outer.duration(), 100.0);
        assert_eq!(outer.self_time, 60.0);

        let inner = &profile.frames[1];
        assert_eq!(inner.name, "inner");
        assert_eq!(inner.depth, 1);
        assert_eq!(inner.parent, Some(outer.id));
    }

    #[test]
    fn parse_array_format() {
        let json = r#"[{"name":"a","ph":"X","ts":0,"dur":10,"pid":1,"tid":1,"cat":""}]"#;
        let profile = parse_chrome_trace(json.as_bytes()).unwrap();
        assert_eq!(profile.frames.len(), 1);
    }

    #[test]
    fn empty_trace() {
        let json = r#"{"traceEvents":[]}"#;
        let profile = parse_chrome_trace(json.as_bytes()).unwrap();
        assert!(profile.frames.is_empty());
    }
}
