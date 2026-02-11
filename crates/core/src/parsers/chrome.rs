use flame_cat_protocol::{ClockKind, TimeDomain};
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
        #[serde(default)]
        metadata: Option<serde_json::Value>,
    },
    Array(Vec<TraceEvent>),
}

/// Metadata extracted from top-level Chrome trace fields.
struct TraceMetadata {
    time_domain: Option<TimeDomain>,
    /// `navigationStart` timestamp in µs (from `blink.user_timing`).
    /// This is `performance.timeOrigin` on the monotonic clock — the anchor
    /// point for converting `performance.now()` values to monotonic time.
    navigation_start_us: Option<f64>,
}

/// Extract top-level metadata from Chrome trace object format.
fn extract_trace_metadata(metadata: &Option<serde_json::Value>) -> TraceMetadata {
    let clock_domain = metadata
        .as_ref()
        .and_then(|m| m.get("clock-domain"))
        .and_then(|v| v.as_str());

    let clock_kind = match clock_domain {
        Some("LINUX_CLOCK_MONOTONIC") => Some(ClockKind::LinuxMonotonic),
        Some(s) if s.contains("MONOTONIC") => Some(ClockKind::LinuxMonotonic),
        _ => None,
    };

    let time_domain = clock_kind.map(|kind| TimeDomain {
        clock_kind: kind,
        origin_label: clock_domain.map(String::from),
        navigation_start_us: None, // filled in later from events
    });

    TraceMetadata {
        time_domain,
        navigation_start_us: None,
    }
}

/// Check if a trace event is a React Performance Track component measure.
///
/// React 19.2+ emits `performance.measure()` calls that Chrome records as
/// user timing events. The key identifier is `args.detail.devtools.track`
/// containing `"Components ⚛"`.
fn is_react_component_event(event: &TraceEvent) -> bool {
    event.args.as_ref().is_some_and(|args| {
        args.get("detail")
            .and_then(|d| d.get("devtools"))
            .and_then(|dt| dt.get("track"))
            .and_then(|t| t.as_str())
            .is_some_and(|s| s.contains("Components"))
    })
}

/// Check if a trace event is a React Scheduler lane measure.
fn is_react_scheduler_event(event: &TraceEvent) -> bool {
    event.args.as_ref().is_some_and(|args| {
        args.get("detail")
            .and_then(|d| d.get("devtools"))
            .and_then(|dt| dt.get("track"))
            .and_then(|t| t.as_str())
            .is_some_and(|s| s == "Blocking" || s == "Transition" || s == "Suspense" || s == "Idle")
    })
}

/// Extract the React component self-time color severity from a trace event.
/// React uses: primary-light (<0.5ms), primary (<10ms), primary-dark (<100ms), error (>=100ms).
fn extract_react_color(event: &TraceEvent) -> Option<&str> {
    event.args.as_ref().and_then(|args| {
        args.get("detail")
            .and_then(|d| d.get("devtools"))
            .and_then(|dt| dt.get("color"))
            .and_then(|c| c.as_str())
    })
}

/// Extract changed props from a React DEV-mode trace event.
/// In DEV builds, React emits a `properties` array with changed prop details.
#[cfg(test)]
fn extract_react_properties(event: &TraceEvent) -> Option<Vec<(String, String)>> {
    event.args.as_ref().and_then(|args| {
        args.get("detail")
            .and_then(|d| d.get("devtools"))
            .and_then(|dt| dt.get("properties"))
            .and_then(|p| p.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|entry| {
                        let pair = entry.as_array()?;
                        if pair.len() >= 2 {
                            Some((
                                pair[0].as_str().unwrap_or("").to_string(),
                                pair[1].as_str().unwrap_or("").to_string(),
                            ))
                        } else {
                            None
                        }
                    })
                    .collect()
            })
    })
}

/// Parse a Chrome DevTools trace JSON into a `Profile`.
pub fn parse_chrome_trace(data: &[u8]) -> Result<Profile, ChromeParseError> {
    let trace_file: TraceFile = serde_json::from_slice(data)?;
    let (events, trace_meta) = match trace_file {
        TraceFile::Object {
            trace_events,
            metadata,
        } => (trace_events, extract_trace_metadata(&metadata)),
        TraceFile::Array(events) => (
            events,
            TraceMetadata {
                time_domain: None,
                navigation_start_us: None,
            },
        ),
    };

    // Collect thread name metadata and navigationStart
    let mut thread_names: std::collections::HashMap<(u64, u64), String> =
        std::collections::HashMap::new();
    let mut navigation_start_us = trace_meta.navigation_start_us;
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
        // Extract navigationStart (= performance.timeOrigin on monotonic clock)
        if navigation_start_us.is_none()
            && event.name == "navigationStart"
            && event.cat == "blink.user_timing"
        {
            navigation_start_us = Some(event.ts);
        }
    }

    // Store navigationStart on the time domain if found
    let mut trace_meta = trace_meta;
    if let Some(nav_start) = navigation_start_us {
        if let Some(ref mut td) = trace_meta.time_domain {
            td.navigation_start_us = Some(nav_start);
        }
        trace_meta.navigation_start_us = Some(nav_start);
    }

    let mut frames: Vec<Frame> = Vec::with_capacity(events.len());
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

        // Determine category: React component/scheduler events get a "react" category.
        let category = if is_react_component_event(event) {
            // Append self-time severity as subcategory for coloring.
            let color = extract_react_color(event).unwrap_or("primary");
            Some(format!("react.component.{color}"))
        } else if is_react_scheduler_event(event) {
            let track = event
                .args
                .as_ref()
                .and_then(|a| a.get("detail"))
                .and_then(|d| d.get("devtools"))
                .and_then(|dt| dt.get("track"))
                .and_then(|t| t.as_str())
                .unwrap_or("unknown");
            Some(format!("react.scheduler.{}", track.to_lowercase()))
        } else if event.cat.is_empty() {
            None
        } else {
            Some(event.cat.clone())
        };

        // For React component events, override the thread name to group
        // them in a dedicated "React Components" lane.
        let effective_thread = if is_react_component_event(event) {
            Some("React Components".to_string())
        } else if is_react_scheduler_event(event) {
            let track = event
                .args
                .as_ref()
                .and_then(|a| a.get("detail"))
                .and_then(|d| d.get("devtools"))
                .and_then(|dt| dt.get("track"))
                .and_then(|t| t.as_str())
                .unwrap_or("Scheduler");
            Some(format!("React Scheduler: {track}"))
        } else {
            thread_name
        };

        // Strip the zero-width space prefix React uses for measure names in DEV.
        let name = event.name.trim_start_matches('\u{200b}').to_string();

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
                    name,
                    start: event.ts,
                    end: event.ts + dur,
                    depth,
                    category,
                    parent: parent_id,
                    self_time: 0.0, // computed below
                    thread: effective_thread,
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
                    name,
                    start: event.ts,
                    end: event.ts, // will be updated on E
                    depth,
                    category,
                    parent: parent_id,
                    self_time: 0.0,
                    thread: effective_thread,
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
            time_domain: trace_meta.time_domain,
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

    #[test]
    fn parse_react_component_events() {
        // Simulate React 19.2 Performance Track events in a Chrome trace.
        // React emits performance.measure() with detail.devtools metadata.
        let json = r#"{"traceEvents":[
            {"name":"thread_name","ph":"M","pid":1,"tid":1,"ts":0,"cat":"__metadata",
             "args":{"name":"CrRendererMain"}},
            {"name":"\u200bApp","ph":"X","ts":1000,"dur":500,"pid":1,"tid":1,"cat":"blink.user_timing",
             "args":{"detail":{"devtools":{"track":"Components ⚛","color":"primary-light"}}}},
            {"name":"\u200bHeader","ph":"X","ts":1000,"dur":150,"pid":1,"tid":1,"cat":"blink.user_timing",
             "args":{"detail":{"devtools":{"track":"Components ⚛","color":"primary-light"}}}},
            {"name":"\u200bBody","ph":"X","ts":1200,"dur":300,"pid":1,"tid":1,"cat":"blink.user_timing",
             "args":{"detail":{"devtools":{"track":"Components ⚛","color":"primary",
              "properties":[["Changed Props",""],["count","5 → 6"]]}}}},
            {"name":"\u200bList","ph":"X","ts":1200,"dur":200,"pid":1,"tid":1,"cat":"blink.user_timing",
             "args":{"detail":{"devtools":{"track":"Components ⚛","color":"primary-dark"}}}}
        ]}"#;

        let profile = parse_chrome_trace(json.as_bytes()).unwrap();

        // Should have 4 React component frames (thread_name is metadata, not a frame)
        let react_frames: Vec<_> = profile
            .frames
            .iter()
            .filter(|f| {
                f.category
                    .as_ref()
                    .is_some_and(|c| c.starts_with("react.component"))
            })
            .collect();
        assert_eq!(react_frames.len(), 4);

        // Zero-width space prefix should be stripped from names
        assert_eq!(react_frames[0].name, "App");
        assert_eq!(react_frames[1].name, "Header");
        assert_eq!(react_frames[2].name, "Body");
        assert_eq!(react_frames[3].name, "List");

        // All React component frames go to "React Components" thread
        assert!(
            react_frames
                .iter()
                .all(|f| f.thread.as_deref() == Some("React Components"))
        );

        // Categories encode self-time severity
        assert_eq!(
            react_frames[0].category.as_deref(),
            Some("react.component.primary-light")
        );
        assert_eq!(
            react_frames[2].category.as_deref(),
            Some("react.component.primary")
        );
        assert_eq!(
            react_frames[3].category.as_deref(),
            Some("react.component.primary-dark")
        );

        // Nesting: App is the root (depth 0), Header and Body are children (depth 1),
        // List is nested inside Body (depth 2 on the React Components thread)
        assert_eq!(react_frames[0].depth, 0); // App
        assert_eq!(react_frames[1].depth, 1); // Header (inside App)
    }

    #[test]
    fn parse_react_scheduler_events() {
        let json = r#"{"traceEvents":[
            {"name":"Blocking","ph":"X","ts":0,"dur":1000,"pid":1,"tid":1,"cat":"blink.user_timing",
             "args":{"detail":{"devtools":{"track":"Blocking","color":"primary"}}}},
            {"name":"Transition","ph":"X","ts":2000,"dur":500,"pid":1,"tid":1,"cat":"blink.user_timing",
             "args":{"detail":{"devtools":{"track":"Transition","color":"primary-light"}}}}
        ]}"#;

        let profile = parse_chrome_trace(json.as_bytes()).unwrap();
        assert_eq!(profile.frames.len(), 2);

        let blocking = &profile.frames[0];
        assert_eq!(blocking.name, "Blocking");
        assert_eq!(
            blocking.category.as_deref(),
            Some("react.scheduler.blocking")
        );
        assert_eq!(
            blocking.thread.as_deref(),
            Some("React Scheduler: Blocking")
        );

        let transition = &profile.frames[1];
        assert_eq!(
            transition.category.as_deref(),
            Some("react.scheduler.transition")
        );
    }

    #[test]
    fn extract_react_dev_properties() {
        let json = r#"{"traceEvents":[
            {"name":"\u200bCounter","ph":"X","ts":0,"dur":100,"pid":1,"tid":1,"cat":"blink.user_timing",
             "args":{"detail":{"devtools":{"track":"Components ⚛","color":"primary",
              "properties":[["Changed Props",""],["count","5 → 6"],["label","\"hello\" → \"world\""]]}}}}
        ]}"#;

        let profile = parse_chrome_trace(json.as_bytes()).unwrap();
        assert_eq!(profile.frames.len(), 1);

        // Verify we can extract properties from the raw event
        let raw: serde_json::Value = serde_json::from_slice(json.as_bytes()).unwrap();
        let events = raw["traceEvents"].as_array().unwrap();
        let event: TraceEvent = serde_json::from_value(events[0].clone()).unwrap();
        let props = extract_react_properties(&event).unwrap();
        assert_eq!(props.len(), 3);
        assert_eq!(props[0], ("Changed Props".to_string(), "".to_string()));
        assert_eq!(props[1], ("count".to_string(), "5 → 6".to_string()));
    }

    #[test]
    fn parse_trace_with_metadata() {
        let json = r#"{"traceEvents":[
            {"name":"a","ph":"X","ts":0,"dur":10,"pid":1,"tid":1,"cat":""}
        ],"metadata":{"clock-domain":"LINUX_CLOCK_MONOTONIC"}}"#;

        let profile = parse_chrome_trace(json.as_bytes()).unwrap();
        assert_eq!(profile.frames.len(), 1);
    }
}
