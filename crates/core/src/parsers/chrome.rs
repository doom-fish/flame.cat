use flame_cat_protocol::{
    AsyncSpan, ClockKind, CounterSample, CounterTrack, CounterUnit, CpuNode, CpuSamples,
    FlowArrow, InstantEvent, Marker, MarkerScope, NetworkRequest, ObjectEvent, ObjectPhase,
    SharedStr, TimeDomain,
};
use serde::Deserialize;
use thiserror::Error;

use crate::model::{Frame, Profile, ProfileMetadata};

/// Deserialize an optional id that can be either a string or a number.
fn deserialize_optional_id<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum IdValue {
        Str(String),
        Num(u64),
    }
    Option::<IdValue>::deserialize(deserializer).map(|opt| {
        opt.map(|v| match v {
            IdValue::Str(s) => s,
            IdValue::Num(n) => n.to_string(),
        })
    })
}

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
    /// Event id for async/flow/object events (can be string or number in JSON).
    #[serde(default, deserialize_with = "deserialize_optional_id")]
    id: Option<String>,
    /// Alternative id format used by some Chrome traces.
    #[serde(default)]
    id2: Option<serde_json::Value>,
    /// Scope for instant events ("g"=global, "p"=process, "t"=thread).
    #[serde(default)]
    s: Option<String>,
}

impl TraceEvent {
    /// Get effective event id, checking both `id` and `id2` fields.
    fn effective_id(&self) -> Option<String> {
        if let Some(ref id) = self.id {
            return Some(id.clone());
        }
        if let Some(ref id2) = self.id2 {
            // id2 can be {"local": "0x..."} or {"global": "0x..."}
            if let Some(local) = id2.get("local").and_then(|v| v.as_str()) {
                return Some(local.to_string());
            }
            if let Some(global) = id2.get("global").and_then(|v| v.as_str()) {
                return Some(global.to_string());
            }
        }
        None
    }
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

/// Guess the counter unit from its name.
fn guess_counter_unit(name: &str) -> CounterUnit {
    let lower = name.to_lowercase();
    if lower.contains("heap") || lower.contains("memory") || lower.contains("bytes") {
        CounterUnit::Bytes
    } else if lower.contains("percent") || lower.contains("%") {
        CounterUnit::Percent
    } else {
        CounterUnit::Count
    }
}

/// Extract counters from an UpdateCounters instant event's `data` field.
fn extract_update_counters(
    data: &serde_json::Value,
    ts: f64,
    counter_map: &mut std::collections::HashMap<String, (CounterUnit, Vec<CounterSample>)>,
) {
    let fields = [
        ("jsHeapSizeUsed", "JS Heap Size", CounterUnit::Bytes),
        ("documents", "Documents", CounterUnit::Count),
        ("nodes", "DOM Nodes", CounterUnit::Count),
        (
            "jsEventListeners",
            "JS Event Listeners",
            CounterUnit::Count,
        ),
    ];
    for (key, name, unit) in &fields {
        if let Some(v) = data.get(key).and_then(serde_json::Value::as_f64) {
            let entry = counter_map
                .entry(name.to_string())
                .or_insert((*unit, Vec::new()));
            entry.1.push(CounterSample { ts, value: v });
        }
    }
}

/// Extract CPU profile chunk data from a P event's `data` field.
fn extract_cpu_profile_chunk(
    data: &serde_json::Value,
    base_ts: f64,
    nodes: &mut Vec<CpuNode>,
    samples: &mut Vec<u32>,
    timestamps: &mut Vec<f64>,
) {
    // Extract nodes from cpuProfile.nodes
    if let Some(profile_nodes) = data
        .get("cpuProfile")
        .and_then(|cp| cp.get("nodes"))
        .and_then(|n| n.as_array())
    {
        for node in profile_nodes {
            let id = node
                .get("id")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0) as u32;
            let parent = node
                .get("parent")
                .and_then(serde_json::Value::as_u64)
                .map(|v| v as u32);
            let call_frame = node.get("callFrame");
            let function_name = call_frame
                .and_then(|cf| cf.get("functionName"))
                .and_then(|v| v.as_str())
                .unwrap_or("(anonymous)");
            let script_id = call_frame
                .and_then(|cf| cf.get("scriptId"))
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);

            // Deduplicate: only add if we haven't seen this id yet
            if !nodes.iter().any(|n| n.id == id) {
                nodes.push(CpuNode {
                    id,
                    parent,
                    function_name: SharedStr::from(function_name),
                    script_id,
                });
            }
        }
    }

    // Extract samples (node IDs)
    if let Some(sample_ids) = data
        .get("cpuProfile")
        .and_then(|cp| cp.get("samples"))
        .and_then(|s| s.as_array())
    {
        for s in sample_ids {
            if let Some(id) = s.as_u64() {
                samples.push(id as u32);
            }
        }
    }

    // Extract time deltas and convert to absolute timestamps
    if let Some(deltas) = data.get("timeDeltas").and_then(|d| d.as_array()) {
        let mut current_ts = if timestamps.is_empty() {
            base_ts
        } else {
            *timestamps.last().unwrap_or(&base_ts)
        };
        for delta in deltas {
            if let Some(d) = delta.as_f64() {
                current_ts += d;
                timestamps.push(current_ts);
            }
        }
    }
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

    // Non-span event collectors
    let mut instant_events: Vec<InstantEvent> = Vec::new();
    let mut markers: Vec<Marker> = Vec::new();
    let mut object_events: Vec<ObjectEvent> = Vec::new();
    // Network request correlation
    let mut net_sends: std::collections::HashMap<String, NetworkRequest> =
        std::collections::HashMap::new();
    let mut network_requests: Vec<NetworkRequest> = Vec::new();

    // Counter state: name → (unit, samples)
    let mut counter_map: std::collections::HashMap<String, (CounterUnit, Vec<CounterSample>)> =
        std::collections::HashMap::new();

    // Async span state: (cat, id) → pending begin event
    let mut async_begins: std::collections::HashMap<(String, String), (f64, String, u64, u64)> =
        std::collections::HashMap::new();
    let mut async_spans: Vec<AsyncSpan> = Vec::new();

    // Flow event state: id → pending start event
    let mut flow_starts: std::collections::HashMap<String, (f64, u64, String)> =
        std::collections::HashMap::new();
    let mut flow_arrows: Vec<FlowArrow> = Vec::new();

    // CPU sample state
    let mut cpu_nodes: Vec<CpuNode> = Vec::new();
    let mut cpu_samples: Vec<u32> = Vec::new();
    let mut cpu_timestamps: Vec<f64> = Vec::new();

    // Sort events by timestamp for correct stack reconstruction.
    let mut sorted_events: Vec<TraceEvent> = events
        .into_iter()
        .filter(|e| e.ph != "M") // metadata already processed
        .collect();
    sorted_events.sort_by(|a, b| a.ts.total_cmp(&b.ts));

    for event in &sorted_events {
        let key = (event.pid, event.tid);
        let thread_name = thread_names.get(&key).cloned();

        match event.ph.as_str() {
            // === Duration events (existing) ===
            "X" | "B" | "E" => {
                // Pop completed X events from the stack before processing.
                if let Some(stack) = stacks.get_mut(&key) {
                    while let Some(&top_idx) = stack.last() {
                        let top = &frames[top_idx];
                        if top.end > top.start && top.end <= event.ts {
                            stack.pop();
                        } else {
                            break;
                        }
                    }
                }

                let category = if is_react_component_event(event) {
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
                            self_time: 0.0,
                            thread: effective_thread,
                        });
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
                            end: event.ts,
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

            // === Instant events (ph:"I" or "i") ===
            "I" | "i" => {
                let scope = match event.s.as_deref() {
                    Some("g") => MarkerScope::Global,
                    Some("p") => MarkerScope::Process,
                    _ => MarkerScope::Thread,
                };

                // Extract UpdateCounters → counter tracks
                if event.name == "UpdateCounters"
                    && let Some(data) = event.args.as_ref().and_then(|a| a.get("data"))
                {
                    extract_update_counters(data, event.ts, &mut counter_map);
                }

                // Network request correlation
                if let Some(data) = event.args.as_ref().and_then(|a| a.get("data")) {
                    match event.name.as_str() {
                        "ResourceSendRequest" => {
                            if let Some(rid) = data.get("requestId").and_then(|v| v.as_str()) {
                                let url = data
                                    .get("url")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                net_sends.insert(
                                    rid.to_string(),
                                    NetworkRequest {
                                        request_id: SharedStr::from(rid),
                                        url: SharedStr::from(url),
                                        send_ts: event.ts,
                                        response_ts: None,
                                        finish_ts: None,
                                        mime_type: None,
                                        from_cache: false,
                                    },
                                );
                            }
                        }
                        "ResourceReceiveResponse" => {
                            if let Some(rid) = data.get("requestId").and_then(|v| v.as_str()) {
                                if let Some(req) = net_sends.get_mut(rid) {
                                    req.response_ts = Some(event.ts);
                                    if let Some(mime) = data.get("mimeType").and_then(|v| v.as_str()) {
                                        req.mime_type = Some(SharedStr::from(mime));
                                    }
                                    req.from_cache = data.get("fromCache").and_then(|v| v.as_bool()).unwrap_or(false);
                                }
                            }
                        }
                        "ResourceFinish" => {
                            if let Some(rid) = data.get("requestId").and_then(|v| v.as_str()) {
                                if let Some(mut req) = net_sends.remove(rid) {
                                    req.finish_ts = Some(event.ts);
                                    network_requests.push(req);
                                } else {
                                    // Finish without send — skip
                                }
                            }
                        }
                        _ => {}
                    }
                }

                instant_events.push(InstantEvent {
                    ts: event.ts,
                    name: SharedStr::from(event.name.as_str()),
                    cat: if event.cat.is_empty() {
                        None
                    } else {
                        Some(SharedStr::from(event.cat.as_str()))
                    },
                    scope,
                    pid: event.pid,
                    tid: event.tid,
                });
            }

            // === Mark events (ph:"R") — Web Vitals and navigation timing ===
            "R" => {
                let category = match event.name.as_str() {
                    "firstPaint" | "firstContentfulPaint" | "firstMeaningfulPaint"
                    | "largestContentfulPaint::Candidate" => Some("web-vital"),
                    "InteractiveTime" => Some("web-vital"),
                    "LayoutShift" => Some("web-vital"),
                    "navigationStart" | "fetchStart" | "responseEnd" | "domLoading"
                    | "domInteractive" | "domContentLoadedEventStart"
                    | "domContentLoadedEventEnd" | "domComplete" | "loadEventStart"
                    | "loadEventEnd" => Some("navigation"),
                    _ => None,
                };
                // Normalize LCP candidate name
                let name = if event.name == "largestContentfulPaint::Candidate" {
                    "LCP"
                } else {
                    &event.name
                };
                markers.push(Marker {
                    ts: event.ts,
                    name: SharedStr::from(name),
                    scope: MarkerScope::Global,
                    category: category.map(SharedStr::from),
                });
            }

            // === Counter events (ph:"C") ===
            "C" => {
                if let Some(obj) = event.args.as_ref().and_then(|a| a.as_object()) {
                    for (counter_name, value) in obj {
                        if let Some(v) = value.as_f64() {
                            let full_name = if event.name.is_empty() {
                                counter_name.clone()
                            } else {
                                format!("{} — {}", event.name, counter_name)
                            };
                            let unit = guess_counter_unit(&full_name);
                            let entry =
                                counter_map.entry(full_name).or_insert((unit, Vec::new()));
                            entry.1.push(CounterSample {
                                ts: event.ts,
                                value: v,
                            });
                        }
                    }
                }
            }

            // === Async events (ph:"b"/"e"/"n") ===
            "b" => {
                if let Some(id) = event.effective_id() {
                    async_begins.insert(
                        (event.cat.clone(), id.clone()),
                        (event.ts, event.name.clone(), event.pid, event.tid),
                    );
                }
            }
            "e" => {
                if let Some(id) = event.effective_id() {
                    let begin_key = (event.cat.clone(), id.clone());
                    if let Some((start_ts, name, pid, tid)) = async_begins.remove(&begin_key) {
                        async_spans.push(AsyncSpan {
                            id: SharedStr::from(id.as_str()),
                            name: SharedStr::from(name.as_str()),
                            cat: if event.cat.is_empty() {
                                None
                            } else {
                                Some(SharedStr::from(event.cat.as_str()))
                            },
                            start: start_ts,
                            end: event.ts,
                            pid,
                            tid,
                        });
                    }
                }
            }
            "n" => {
                // Async instant — we store as a zero-duration async span
                if let Some(id) = event.effective_id() {
                    async_spans.push(AsyncSpan {
                        id: SharedStr::from(id.as_str()),
                        name: SharedStr::from(event.name.as_str()),
                        cat: if event.cat.is_empty() {
                            None
                        } else {
                            Some(SharedStr::from(event.cat.as_str()))
                        },
                        start: event.ts,
                        end: event.ts,
                        pid: event.pid,
                        tid: event.tid,
                    });
                }
            }

            // === Flow events (ph:"s"/"f"/"t") ===
            "s" => {
                if let Some(id) = event.effective_id() {
                    flow_starts.insert(
                        id.clone(),
                        (event.ts, event.tid, event.name.clone()),
                    );
                }
            }
            "f" => {
                if let Some(id) = event.effective_id()
                    && let Some((from_ts, from_tid, name)) = flow_starts.remove(&id)
                {
                    flow_arrows.push(FlowArrow {
                        name: SharedStr::from(name.as_str()),
                        id: SharedStr::from(id.as_str()),
                        from_ts,
                        from_tid,
                        to_ts: event.ts,
                        to_tid: event.tid,
                    });
                }
            }
            "t" => {
                // Flow step: end current flow, start new one
                if let Some(id) = event.effective_id() {
                    if let Some((from_ts, from_tid, name)) = flow_starts.remove(&id) {
                        flow_arrows.push(FlowArrow {
                            name: SharedStr::from(name.as_str()),
                            id: SharedStr::from(id.as_str()),
                            from_ts,
                            from_tid,
                            to_ts: event.ts,
                            to_tid: event.tid,
                        });
                    }
                    flow_starts.insert(
                        id.clone(),
                        (event.ts, event.tid, event.name.clone()),
                    );
                }
            }

            // === CPU profiler samples (ph:"P") ===
            "P" => {
                if let Some(data) = event.args.as_ref().and_then(|a| a.get("data")) {
                    extract_cpu_profile_chunk(
                        data,
                        event.ts,
                        &mut cpu_nodes,
                        &mut cpu_samples,
                        &mut cpu_timestamps,
                    );
                }
            }

            // === Object lifecycle (ph:"N"/"O"/"D") ===
            "N" | "O" | "D" => {
                let phase = match event.ph.as_str() {
                    "N" => ObjectPhase::Create,
                    "O" => ObjectPhase::Snapshot,
                    _ => ObjectPhase::Destroy,
                };
                let obj_id = event
                    .effective_id()
                    .unwrap_or_default();
                object_events.push(ObjectEvent {
                    id: SharedStr::from(obj_id.as_str()),
                    name: SharedStr::from(event.name.as_str()),
                    phase,
                    ts: event.ts,
                });
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

    // Determine time range from all event types
    let mut min_ts = f64::INFINITY;
    let mut max_ts = f64::NEG_INFINITY;
    for f in &frames {
        min_ts = min_ts.min(f.start);
        max_ts = max_ts.max(f.end);
    }
    for e in &instant_events {
        min_ts = min_ts.min(e.ts);
        max_ts = max_ts.max(e.ts);
    }
    for m in &markers {
        min_ts = min_ts.min(m.ts);
        max_ts = max_ts.max(m.ts);
    }
    for a in &async_spans {
        min_ts = min_ts.min(a.start);
        max_ts = max_ts.max(a.end);
    }

    // Build counter tracks from collected data
    let counters: Vec<CounterTrack> = counter_map
        .into_iter()
        .map(|(name, (unit, mut samples))| {
            samples.sort_by(|a, b| a.ts.total_cmp(&b.ts));
            CounterTrack {
                name: SharedStr::from(name.as_str()),
                unit,
                samples,
            }
        })
        .collect();

    // Build CPU samples
    let cpu_sample_data = if !cpu_nodes.is_empty() {
        Some(CpuSamples {
            nodes: cpu_nodes,
            samples: cpu_samples,
            timestamps: cpu_timestamps,
        })
    } else {
        None
    };

    // Flush remaining network sends (no finish event)
    for (_, req) in net_sends {
        network_requests.push(req);
    }
    // Sort network requests by send timestamp
    network_requests.sort_by(|a, b| a.send_ts.total_cmp(&b.send_ts));

    let mut profile = Profile::new(
        ProfileMetadata {
            name: None,
            start_time: if min_ts.is_finite() { min_ts } else { 0.0 },
            end_time: if max_ts.is_finite() { max_ts } else { 0.0 },
            format: "chrome".to_string(),
            time_domain: trace_meta.time_domain,
        },
        frames,
    );
    profile.counters = counters;
    profile.async_spans = async_spans;
    profile.flow_arrows = flow_arrows;
    profile.markers = markers;
    profile.instant_events = instant_events;
    profile.object_events = object_events;
    profile.cpu_samples = cpu_sample_data;
    profile.network_requests = network_requests;

    Ok(profile)
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

    #[test]
    fn parse_instant_events() {
        let json = r#"{"traceEvents":[
            {"name":"Paint","ph":"I","ts":100,"pid":1,"tid":1,"cat":"devtools.timeline","s":"t"},
            {"name":"GC","ph":"i","ts":200,"pid":1,"tid":1,"cat":"v8","s":"p"},
            {"name":"Screenshot","ph":"I","ts":300,"pid":1,"tid":1,"cat":"screenshot","s":"g"}
        ]}"#;

        let profile = parse_chrome_trace(json.as_bytes()).unwrap();
        assert_eq!(profile.instant_events.len(), 3);
        assert_eq!(profile.instant_events[0].name.as_ref(), "Paint");
        assert_eq!(
            profile.instant_events[0].scope,
            flame_cat_protocol::MarkerScope::Thread
        );
        assert_eq!(
            profile.instant_events[1].scope,
            flame_cat_protocol::MarkerScope::Process
        );
        assert_eq!(
            profile.instant_events[2].scope,
            flame_cat_protocol::MarkerScope::Global
        );
    }

    #[test]
    fn parse_update_counters() {
        let json = r#"{"traceEvents":[
            {"name":"UpdateCounters","ph":"I","ts":100,"pid":1,"tid":1,"cat":"devtools.timeline","s":"t",
             "args":{"data":{"jsHeapSizeUsed":1048576,"documents":5,"nodes":100,"jsEventListeners":50}}},
            {"name":"UpdateCounters","ph":"I","ts":200,"pid":1,"tid":1,"cat":"devtools.timeline","s":"t",
             "args":{"data":{"jsHeapSizeUsed":2097152,"documents":6,"nodes":120,"jsEventListeners":55}}}
        ]}"#;

        let profile = parse_chrome_trace(json.as_bytes()).unwrap();
        assert_eq!(profile.counters.len(), 4);

        let heap = profile
            .counters
            .iter()
            .find(|c| c.name.as_ref() == "JS Heap Size")
            .expect("should have JS Heap counter");
        assert_eq!(heap.unit, flame_cat_protocol::CounterUnit::Bytes);
        assert_eq!(heap.samples.len(), 2);
        assert!((heap.samples[0].value - 1048576.0).abs() < f64::EPSILON);
        assert!((heap.samples[1].value - 2097152.0).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_counter_events() {
        let json = r#"{"traceEvents":[
            {"name":"GPU Memory","ph":"C","ts":100,"pid":1,"tid":1,"cat":"gpu",
             "args":{"allocated":4096,"used":2048}},
            {"name":"GPU Memory","ph":"C","ts":200,"pid":1,"tid":1,"cat":"gpu",
             "args":{"allocated":8192,"used":3072}}
        ]}"#;

        let profile = parse_chrome_trace(json.as_bytes()).unwrap();
        assert!(profile.counters.len() >= 2);

        let allocated = profile
            .counters
            .iter()
            .find(|c| c.name.as_ref().contains("allocated"))
            .expect("should have allocated counter");
        assert_eq!(allocated.samples.len(), 2);
    }

    #[test]
    fn parse_async_events() {
        let json = r#"{"traceEvents":[
            {"name":"PipelineReporter","ph":"b","ts":100,"pid":1,"tid":1,"cat":"benchmark","id":"0x1"},
            {"name":"PipelineReporter","ph":"e","ts":500,"pid":1,"tid":1,"cat":"benchmark","id":"0x1"},
            {"name":"Step","ph":"n","ts":300,"pid":1,"tid":1,"cat":"benchmark","id":"0x2"}
        ]}"#;

        let profile = parse_chrome_trace(json.as_bytes()).unwrap();
        assert_eq!(profile.async_spans.len(), 2);

        let pipeline = profile
            .async_spans
            .iter()
            .find(|s| s.name.as_ref() == "PipelineReporter")
            .expect("should have PipelineReporter");
        assert!((pipeline.start - 100.0).abs() < f64::EPSILON);
        assert!((pipeline.end - 500.0).abs() < f64::EPSILON);

        // Async instant (n) becomes a zero-duration span
        let step = profile
            .async_spans
            .iter()
            .find(|s| s.name.as_ref() == "Step")
            .expect("should have Step");
        assert!((step.start - step.end).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_flow_events() {
        let json = r#"{"traceEvents":[
            {"name":"AnimFrame","ph":"s","ts":100,"pid":1,"tid":1,"cat":"blink","id":"42"},
            {"name":"AnimFrame","ph":"f","ts":300,"pid":1,"tid":2,"cat":"blink","id":"42"}
        ]}"#;

        let profile = parse_chrome_trace(json.as_bytes()).unwrap();
        assert_eq!(profile.flow_arrows.len(), 1);

        let arrow = &profile.flow_arrows[0];
        assert_eq!(arrow.name.as_ref(), "AnimFrame");
        assert!((arrow.from_ts - 100.0).abs() < f64::EPSILON);
        assert_eq!(arrow.from_tid, 1);
        assert!((arrow.to_ts - 300.0).abs() < f64::EPSILON);
        assert_eq!(arrow.to_tid, 2);
    }

    #[test]
    fn parse_flow_with_steps() {
        let json = r#"{"traceEvents":[
            {"name":"loader","ph":"s","ts":100,"pid":1,"tid":1,"cat":"loading","id":"1"},
            {"name":"loader","ph":"t","ts":200,"pid":1,"tid":2,"cat":"loading","id":"1"},
            {"name":"loader","ph":"f","ts":300,"pid":1,"tid":3,"cat":"loading","id":"1"}
        ]}"#;

        let profile = parse_chrome_trace(json.as_bytes()).unwrap();
        assert_eq!(profile.flow_arrows.len(), 2);
        assert_eq!(profile.flow_arrows[0].from_tid, 1);
        assert_eq!(profile.flow_arrows[0].to_tid, 2);
        assert_eq!(profile.flow_arrows[1].from_tid, 2);
        assert_eq!(profile.flow_arrows[1].to_tid, 3);
    }

    #[test]
    fn parse_object_events() {
        let json = r#"{"traceEvents":[
            {"name":"Layer","ph":"N","ts":100,"pid":1,"tid":1,"cat":"cc","id":"0xabc"},
            {"name":"Layer","ph":"O","ts":200,"pid":1,"tid":1,"cat":"cc","id":"0xabc",
             "args":{"snapshot":{"bounds":[100,200]}}},
            {"name":"Layer","ph":"D","ts":500,"pid":1,"tid":1,"cat":"cc","id":"0xabc"}
        ]}"#;

        let profile = parse_chrome_trace(json.as_bytes()).unwrap();
        assert_eq!(profile.object_events.len(), 3);
        assert_eq!(
            profile.object_events[0].phase,
            flame_cat_protocol::ObjectPhase::Create
        );
        assert_eq!(
            profile.object_events[1].phase,
            flame_cat_protocol::ObjectPhase::Snapshot
        );
        assert_eq!(
            profile.object_events[2].phase,
            flame_cat_protocol::ObjectPhase::Destroy
        );
        assert_eq!(profile.object_events[0].id.as_ref(), "0xabc");
    }

    #[test]
    fn parse_mark_events() {
        let json = r#"{"traceEvents":[
            {"name":"navigationStart","ph":"R","ts":100,"pid":1,"tid":1,"cat":"blink.user_timing"},
            {"name":"domInteractive","ph":"R","ts":500,"pid":1,"tid":1,"cat":"blink.user_timing"},
            {"name":"loadEventEnd","ph":"R","ts":1000,"pid":1,"tid":1,"cat":"blink.user_timing"}
        ]}"#;

        let profile = parse_chrome_trace(json.as_bytes()).unwrap();
        assert_eq!(profile.markers.len(), 3);
        assert_eq!(profile.markers[0].name.as_ref(), "navigationStart");
        assert_eq!(
            profile.markers[0].category.as_ref().map(|s| s.as_ref()),
            Some("navigation")
        );
        assert_eq!(profile.markers[1].name.as_ref(), "domInteractive");
        assert_eq!(profile.markers[2].name.as_ref(), "loadEventEnd");
    }

    #[test]
    fn parse_web_vitals_markers() {
        let json = r#"{"traceEvents":[
            {"name":"navigationStart","ph":"R","ts":100,"pid":1,"tid":1,"cat":"blink.user_timing"},
            {"name":"firstContentfulPaint","ph":"R","ts":200,"pid":1,"tid":1,"cat":"blink.user_timing"},
            {"name":"largestContentfulPaint::Candidate","ph":"R","ts":300,"pid":1,"tid":1,"cat":"blink.user_timing"},
            {"name":"InteractiveTime","ph":"R","ts":400,"pid":1,"tid":1,"cat":"blink.user_timing,rail"},
            {"name":"LayoutShift","ph":"R","ts":350,"pid":1,"tid":1,"cat":"blink.user_timing,rail"}
        ]}"#;

        let profile = parse_chrome_trace(json.as_bytes()).unwrap();
        assert_eq!(profile.markers.len(), 5);

        // FCP should be a web-vital
        let fcp = profile
            .markers
            .iter()
            .find(|m| m.name.as_ref() == "firstContentfulPaint")
            .unwrap();
        assert_eq!(
            fcp.category.as_ref().map(|s| s.as_ref()),
            Some("web-vital")
        );

        // LCP should be normalized to "LCP"
        let lcp = profile.markers.iter().find(|m| m.name.as_ref() == "LCP").unwrap();
        assert_eq!(
            lcp.category.as_ref().map(|s| s.as_ref()),
            Some("web-vital")
        );
        assert!((lcp.ts - 300.0).abs() < f64::EPSILON);

        // TTI
        let tti = profile
            .markers
            .iter()
            .find(|m| m.name.as_ref() == "InteractiveTime")
            .unwrap();
        assert_eq!(
            tti.category.as_ref().map(|s| s.as_ref()),
            Some("web-vital")
        );

        // Navigation markers
        let nav = profile
            .markers
            .iter()
            .find(|m| m.name.as_ref() == "navigationStart")
            .unwrap();
        assert_eq!(
            nav.category.as_ref().map(|s| s.as_ref()),
            Some("navigation")
        );
    }

    #[test]
    fn parse_cpu_profile_chunks() {
        let json = r#"{"traceEvents":[
            {"name":"Profile","ph":"P","ts":0,"pid":1,"tid":1,"cat":"disabled-by-default-v8.cpu_profiler",
             "args":{"data":{
                "cpuProfile":{
                    "nodes":[
                        {"id":1,"callFrame":{"functionName":"(root)","scriptId":"0"}},
                        {"id":2,"parent":1,"callFrame":{"functionName":"main","scriptId":"42"}}
                    ],
                    "samples":[1,2,2,1]
                },
                "timeDeltas":[0,100,100,100]
             }}}
        ]}"#;

        let profile = parse_chrome_trace(json.as_bytes()).unwrap();
        let cpu = profile.cpu_samples.as_ref().expect("should have CPU samples");
        assert_eq!(cpu.nodes.len(), 2);
        assert_eq!(cpu.nodes[0].function_name.as_ref(), "(root)");
        assert_eq!(cpu.nodes[1].function_name.as_ref(), "main");
        assert_eq!(cpu.nodes[1].parent, Some(1));
        assert_eq!(cpu.samples.len(), 4);
        assert_eq!(cpu.timestamps.len(), 4);
    }

    #[test]
    fn parse_numeric_id_flow() {
        let json = r#"{"traceEvents":[
            {"name":"anim","ph":"s","ts":100,"pid":1,"tid":1,"cat":"blink","id":42},
            {"name":"anim","ph":"f","ts":300,"pid":1,"tid":2,"cat":"blink","id":42}
        ]}"#;

        let profile = parse_chrome_trace(json.as_bytes()).unwrap();
        assert_eq!(profile.flow_arrows.len(), 1);
        assert_eq!(profile.flow_arrows[0].id.as_ref(), "42");
    }

    #[test]
    fn parse_real_chrome_fixture() {
        let data = include_bytes!("../../../core/tests/fixtures/chrome-trace-sample.json");
        let profile = parse_chrome_trace(data).unwrap();

        // Verify we got all event types from the real fixture
        assert!(
            !profile.frames.is_empty(),
            "should have duration spans"
        );
        assert!(
            !profile.instant_events.is_empty(),
            "should have instant events"
        );
        assert!(
            !profile.markers.is_empty(),
            "should have markers"
        );
        assert!(
            !profile.async_spans.is_empty(),
            "should have async spans"
        );
        assert!(
            !profile.flow_arrows.is_empty(),
            "should have flow arrows"
        );
        assert!(
            !profile.object_events.is_empty(),
            "should have object events"
        );
        assert!(
            !profile.counters.is_empty(),
            "should have counter tracks"
        );
        assert!(
            profile.cpu_samples.is_some(),
            "should have CPU samples"
        );

        // Verify Web Vital markers have categories
        let web_vitals: Vec<_> = profile
            .markers
            .iter()
            .filter(|m| m.category.as_ref().is_some_and(|c| c.as_ref() == "web-vital"))
            .collect();
        assert!(
            !web_vitals.is_empty(),
            "should have web vital markers"
        );

        let nav_markers: Vec<_> = profile
            .markers
            .iter()
            .filter(|m| m.category.as_ref().is_some_and(|c| c.as_ref() == "navigation"))
            .collect();
        assert!(
            !nav_markers.is_empty(),
            "should have navigation markers"
        );
    }
}
