use std::sync::Mutex;

use flame_cat_core::model::Session;
use flame_cat_core::views::{async_track, counter, cpu_samples, frame_track, left_heavy, markers, minimap, object_track, ranked, sandwich, time_axis, time_order};
use flame_cat_protocol::{Viewport, VisualProfile};
use wasm_bindgen::prelude::*;

static SESSION: Mutex<Option<Session>> = Mutex::new(None);

fn lock_session() -> Result<std::sync::MutexGuard<'static, Option<Session>>, JsError> {
    SESSION
        .lock()
        .map_err(|_| JsError::new("internal error: session lock poisoned"))
}

fn with_session<T>(f: impl FnOnce(&Session) -> Result<T, JsError>) -> Result<T, JsError> {
    let guard = lock_session()?;
    let session = guard.as_ref().ok_or_else(|| JsError::new("no session"))?;
    f(session)
}

fn with_profile<T>(
    profile_index: usize,
    f: impl FnOnce(&VisualProfile) -> Result<T, JsError>,
) -> Result<T, JsError> {
    with_session(|session| {
        let entry = session
            .profiles()
            .get(profile_index)
            .ok_or_else(|| JsError::new("invalid profile index"))?;
        f(&entry.profile)
    })
}

/// Parse a profile from bytes (auto-detects format). Returns a handle (index) for later use.
///
/// The first profile creates a new session. Subsequent calls add profiles
/// to the existing session with automatic clock alignment when possible.
#[wasm_bindgen]
pub fn parse_profile(data: &[u8]) -> Result<usize, JsError> {
    let profile = flame_cat_core::parsers::parse_auto_visual(data)
        .map_err(|e| JsError::new(&e.to_string()))?;
    let guard = lock_session()?;
    let next_idx = guard.as_ref().map_or(0, Session::len);
    drop(guard);
    let label = profile
        .meta
        .name
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("profile-{next_idx}"));
    add_profile_to_session(profile, &label)
}

/// Add a profile to the session with a custom label. Returns profile index.
#[wasm_bindgen]
pub fn add_profile_with_label(data: &[u8], label: &str) -> Result<usize, JsError> {
    let profile = flame_cat_core::parsers::parse_auto_visual(data)
        .map_err(|e| JsError::new(&e.to_string()))?;
    add_profile_to_session(profile, label)
}

fn add_profile_to_session(profile: VisualProfile, label: &str) -> Result<usize, JsError> {
    let mut guard = lock_session()?;
    let session = guard.get_or_insert_with(Session::new);
    let idx = session.len();
    session.add_profile(profile, label);
    Ok(idx)
}

/// Clear the current session and all loaded profiles.
#[wasm_bindgen]
pub fn clear_session() -> Result<(), JsError> {
    let mut guard = lock_session()?;
    *guard = None;
    Ok(())
}

/// Get session metadata as JSON: profile count, unified time bounds, per-profile info.
#[wasm_bindgen]
pub fn get_session_info() -> Result<String, JsError> {
    with_session(|session| {
        #[derive(serde::Serialize)]
        struct SessionInfo {
            profile_count: usize,
            start_time: f64,
            end_time: f64,
            duration: f64,
            profiles: Vec<ProfileInfo>,
        }

        #[derive(serde::Serialize)]
        struct ProfileInfo {
            index: usize,
            label: String,
            source_format: String,
            offset_us: f64,
            start_time: f64,
            end_time: f64,
            span_count: usize,
            has_time_domain: bool,
            clock_kind: Option<String>,
        }

        let profiles = session
            .profiles()
            .iter()
            .enumerate()
            .map(|(i, e)| ProfileInfo {
                index: i,
                label: e.label.clone(),
                source_format: e.profile.meta.source_format.to_string(),
                offset_us: e.offset_us,
                start_time: e.session_start(),
                end_time: e.session_end(),
                span_count: e.profile.span_count(),
                has_time_domain: e.profile.meta.time_domain.is_some(),
                clock_kind: e
                    .profile
                    .meta
                    .time_domain
                    .as_ref()
                    .map(|td| format!("{:?}", td.clock_kind)),
            })
            .collect();

        let info = SessionInfo {
            profile_count: session.len(),
            start_time: session.start_time(),
            end_time: session.end_time(),
            duration: session.duration(),
            profiles,
        };

        serde_json::to_string(&info).map_err(|e| JsError::new(&e.to_string()))
    })
}

/// Manually set the time offset (µs) for a profile in the session.
///
/// Use this for manual alignment when automatic clock domain alignment
/// is not possible (e.g. React DevTools export + Chrome trace from
/// different sessions).
#[wasm_bindgen]
pub fn set_profile_offset(profile_index: usize, offset_us: f64) -> Result<(), JsError> {
    let mut guard = lock_session()?;
    let session = guard.as_mut().ok_or_else(|| JsError::new("no session"))?;
    let entry = session
        .profiles_mut()
        .get_mut(profile_index)
        .ok_or_else(|| JsError::new("invalid profile index"))?;
    entry.offset_us = offset_us;
    Ok(())
}

/// Render a view for a profile, returning render commands as JSON.
///
/// For time-order views, `view_start` / `view_end` define the visible time
/// window (absolute µs).  Pass `NaN` or negative values to auto-fit the full
/// profile range.
///
/// `thread_id` optionally restricts rendering to a single thread group.
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn render_view(
    profile_index: usize,
    view_type: &str,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    dpr: f64,
    selected_frame_id: Option<u64>,
    view_start: Option<f64>,
    view_end: Option<f64>,
    thread_id: Option<u32>,
) -> Result<String, JsError> {
    with_profile(profile_index, |profile| {
        let viewport = Viewport {
            x,
            y,
            width,
            height,
            dpr,
        };

        let vs = view_start.unwrap_or(profile.meta.start_time);
        let ve = view_end.unwrap_or(profile.meta.end_time);

        let commands = match view_type {
            "time-order" => time_order::render_time_order(profile, &viewport, vs, ve, thread_id),
            "left-heavy" => left_heavy::render_left_heavy(profile, &viewport, thread_id),
            "sandwich" => {
                let frame_id = selected_frame_id
                    .ok_or_else(|| JsError::new("sandwich requires selected_frame_id"))?;
                sandwich::render_sandwich(profile, frame_id, &viewport)
            }
            "ranked" => {
                ranked::render_ranked(profile, &viewport, ranked::RankedSort::SelfTime, false)
            }
            _ => return Err(JsError::new(&format!("unknown view type: {view_type}"))),
        };

        serde_json::to_string(&commands).map_err(|e| JsError::new(&e.to_string()))
    })
}

/// Get profile metadata as JSON.
#[wasm_bindgen]
pub fn get_profile_metadata(profile_index: usize) -> Result<String, JsError> {
    with_profile(profile_index, |profile| {
        serde_json::to_string(&profile.meta).map_err(|e| JsError::new(&e.to_string()))
    })
}

/// Get the number of spans in a profile.
#[wasm_bindgen]
pub fn get_frame_count(profile_index: usize) -> Result<usize, JsError> {
    with_profile(profile_index, |profile| Ok(profile.span_count()))
}

/// Look up span details by frame ID. Returns JSON with name, start, end,
/// duration, self_value, depth, category, and thread name.
#[wasm_bindgen]
pub fn get_span_info(profile_index: usize, frame_id: u64) -> Result<String, JsError> {
    with_profile(profile_index, |profile| {
        for thread in &profile.threads {
            for span in &thread.spans {
                if span.id == frame_id {
                    #[derive(serde::Serialize)]
                    struct SpanInfo {
                        name: String,
                        start: f64,
                        end: f64,
                        duration: f64,
                        self_time: f64,
                        depth: u32,
                        category: Option<String>,
                        thread: String,
                    }
                    let info = SpanInfo {
                        name: span.name.to_string(),
                        start: span.start,
                        end: span.end,
                        duration: span.duration(),
                        self_time: span.self_value,
                        depth: span.depth,
                        category: span.category.as_ref().map(|c| c.name.to_string()),
                        thread: thread.name.to_string(),
                    };
                    return serde_json::to_string(&info).map_err(|e| JsError::new(&e.to_string()));
                }
            }
        }

        Err(JsError::new(&format!("span {frame_id} not found")))
    })
}

/// Get the actual content time bounds (min span start, max span end) as JSON.
/// Useful for zoom-to-fit, skipping empty regions at the edges.
#[wasm_bindgen]
pub fn get_content_bounds(profile_index: usize) -> Result<String, JsError> {
    with_profile(profile_index, |profile| {
        let mut min_start = f64::MAX;
        let mut max_end = f64::MIN;
        for thread in &profile.threads {
            for span in &thread.spans {
                if span.start < min_start {
                    min_start = span.start;
                }
                if span.end > max_end {
                    max_end = span.end;
                }
            }
        }

        if min_start > max_end {
            min_start = profile.meta.start_time;
            max_end = profile.meta.end_time;
        }

        #[derive(serde::Serialize)]
        struct Bounds {
            start: f64,
            end: f64,
        }
        serde_json::to_string(&Bounds {
            start: min_start,
            end: max_end,
        })
        .map_err(|e| JsError::new(&e.to_string()))
    })
}

/// Render the minimap for a profile, returning render commands as JSON.
#[wasm_bindgen]
pub fn render_minimap(
    profile_index: usize,
    width: f64,
    height: f64,
    dpr: f64,
    visible_start_frac: f64,
    visible_end_frac: f64,
) -> Result<String, JsError> {
    with_profile(profile_index, |profile| {
        let viewport = Viewport {
            x: 0.0,
            y: 0.0,
            width,
            height,
            dpr,
        };

        let commands =
            minimap::render_minimap(profile, &viewport, visible_start_frac, visible_end_frac);
        serde_json::to_string(&commands).map_err(|e| JsError::new(&e.to_string()))
    })
}

/// Get the list of thread groups for a profile as JSON.
///
/// Returns an array of `{ id, name, span_count, sort_key, max_depth }` objects.
#[wasm_bindgen]
pub fn get_thread_list(profile_index: usize) -> Result<String, JsError> {
    with_profile(profile_index, |profile| {
        #[derive(serde::Serialize)]
        struct ThreadInfo {
            id: u32,
            name: String,
            span_count: usize,
            sort_key: i64,
            max_depth: u32,
        }

        let threads: Vec<ThreadInfo> = profile
            .threads
            .iter()
            .map(|t| ThreadInfo {
                id: t.id,
                name: t.name.to_string(),
                span_count: t.spans.len(),
                sort_key: t.sort_key,
                max_depth: t.spans.iter().map(|s| s.depth).max().unwrap_or(0),
            })
            .collect();

        serde_json::to_string(&threads).map_err(|e| JsError::new(&e.to_string()))
    })
}

/// Get ranked entries for a profile as JSON.
#[wasm_bindgen]
pub fn get_ranked_entries(
    profile_index: usize,
    sort_field: &str,
    ascending: bool,
) -> Result<String, JsError> {
    with_profile(profile_index, |profile| {
        let sort = match sort_field {
            "self" => ranked::RankedSort::SelfTime,
            "total" => ranked::RankedSort::TotalTime,
            "name" => ranked::RankedSort::Name,
            "count" => ranked::RankedSort::Count,
            _ => ranked::RankedSort::SelfTime,
        };

        let entries = ranked::get_ranked_entries(profile, sort, ascending);
        serde_json::to_string(&entries).map_err(|e| JsError::new(&e.to_string()))
    })
}

/// Render counter tracks for a profile, returning render commands as JSON.
///
/// Each counter gets its own area chart. Returns commands for all counters.
#[wasm_bindgen]
pub fn render_counters(
    profile_index: usize,
    width: f64,
    height: f64,
    dpr: f64,
    view_start: Option<f64>,
    view_end: Option<f64>,
) -> Result<String, JsError> {
    with_profile(profile_index, |profile| {
        let vs = view_start.unwrap_or(profile.meta.start_time);
        let ve = view_end.unwrap_or(profile.meta.end_time);
        let viewport = Viewport {
            x: 0.0,
            y: 0.0,
            width,
            height,
            dpr,
        };

        let mut all_commands = Vec::new();
        for c in &profile.counters {
            all_commands.extend(counter::render_counter_track(c, &viewport, vs, ve));
        }

        serde_json::to_string(&all_commands).map_err(|e| JsError::new(&e.to_string()))
    })
}

/// Render a single counter track by name.
#[wasm_bindgen]
pub fn render_counter(
    profile_index: usize,
    counter_name: &str,
    width: f64,
    height: f64,
    dpr: f64,
    view_start: Option<f64>,
    view_end: Option<f64>,
) -> Result<String, JsError> {
    with_profile(profile_index, |profile| {
        let vs = view_start.unwrap_or(profile.meta.start_time);
        let ve = view_end.unwrap_or(profile.meta.end_time);
        let viewport = Viewport {
            x: 0.0,
            y: 0.0,
            width,
            height,
            dpr,
        };

        let commands = if let Some(c) = profile.counters.iter().find(|c| c.name.as_ref() == counter_name) {
            counter::render_counter_track(c, &viewport, vs, ve)
        } else {
            Vec::new()
        };

        serde_json::to_string(&commands).map_err(|e| JsError::new(&e.to_string()))
    })
}

/// Render markers (navigation timing / user timing) as vertical lines.
#[wasm_bindgen]
pub fn render_markers(
    profile_index: usize,
    width: f64,
    height: f64,
    dpr: f64,
    view_start: Option<f64>,
    view_end: Option<f64>,
) -> Result<String, JsError> {
    with_profile(profile_index, |profile| {
        let vs = view_start.unwrap_or(profile.meta.start_time);
        let ve = view_end.unwrap_or(profile.meta.end_time);
        let viewport = Viewport {
            x: 0.0,
            y: 0.0,
            width,
            height,
            dpr,
        };

        let commands = markers::render_markers(&profile.markers, &viewport, vs, ve);
        serde_json::to_string(&commands).map_err(|e| JsError::new(&e.to_string()))
    })
}

/// Render the frame cost track (per-frame bars colored by cost).
#[wasm_bindgen]
pub fn render_frame_track(
    profile_index: usize,
    width: f64,
    height: f64,
    dpr: f64,
    view_start: Option<f64>,
    view_end: Option<f64>,
) -> Result<String, JsError> {
    with_profile(profile_index, |profile| {
        let vs = view_start.unwrap_or(profile.meta.start_time);
        let ve = view_end.unwrap_or(profile.meta.end_time);
        let viewport = Viewport {
            x: 0.0,
            y: 0.0,
            width,
            height,
            dpr,
        };

        let commands = frame_track::render_frame_track(&profile.frames, &viewport, vs, ve);
        serde_json::to_string(&commands).map_err(|e| JsError::new(&e.to_string()))
    })
}

/// Render a time axis ruler with ticks and labels.
#[wasm_bindgen]
pub fn render_time_axis(
    width: f64,
    dpr: f64,
    view_start: f64,
    view_end: f64,
    grid_height: f64,
) -> Result<String, JsError> {
    let viewport = Viewport {
        x: 0.0,
        y: 0.0,
        width,
        height: 24.0,
        dpr,
    };
    let commands = time_axis::render_time_axis(&viewport, view_start, view_end, grid_height);
    serde_json::to_string(&commands).map_err(|e| JsError::new(&e.to_string()))
}

/// Render async spans track.
#[wasm_bindgen]
pub fn render_async_track(
    profile_index: usize,
    width: f64,
    height: f64,
    dpr: f64,
    view_start: Option<f64>,
    view_end: Option<f64>,
) -> Result<String, JsError> {
    with_profile(profile_index, |profile| {
        let vs = view_start.unwrap_or(profile.meta.start_time);
        let ve = view_end.unwrap_or(profile.meta.end_time);
        let viewport = Viewport {
            x: 0.0,
            y: 0.0,
            width,
            height,
            dpr,
        };

        let commands = async_track::render_async_track(&profile.async_spans, &viewport, vs, ve);
        serde_json::to_string(&commands).map_err(|e| JsError::new(&e.to_string()))
    })
}

/// Render CPU sampling flame chart for a profile.
#[wasm_bindgen]
pub fn render_cpu_samples(
    profile_index: usize,
    width: f64,
    height: f64,
    dpr: f64,
    view_start: Option<f64>,
    view_end: Option<f64>,
) -> Result<String, JsError> {
    with_profile(profile_index, |profile| {
        let vs = view_start.unwrap_or(profile.meta.start_time);
        let ve = view_end.unwrap_or(profile.meta.end_time);
        let viewport = Viewport {
            x: 0.0,
            y: 0.0,
            width,
            height,
            dpr,
        };

        let commands = if let Some(ref samples) = profile.cpu_samples {
            cpu_samples::render_cpu_samples(samples, &viewport, vs, ve)
        } else {
            Vec::new()
        };
        serde_json::to_string(&commands).map_err(|e| JsError::new(&e.to_string()))
    })
}

/// Render object lifecycle track for a profile.
#[wasm_bindgen]
pub fn render_object_track(
    profile_index: usize,
    width: f64,
    height: f64,
    dpr: f64,
    view_start: Option<f64>,
    view_end: Option<f64>,
) -> Result<String, JsError> {
    with_profile(profile_index, |profile| {
        let vs = view_start.unwrap_or(profile.meta.start_time);
        let ve = view_end.unwrap_or(profile.meta.end_time);
        let viewport = Viewport {
            x: 0.0,
            y: 0.0,
            width,
            height,
            dpr,
        };

        let commands =
            object_track::render_object_track(&profile.object_events, &viewport, vs, ve);
        serde_json::to_string(&commands).map_err(|e| JsError::new(&e.to_string()))
    })
}

/// Get counter track names for a profile as JSON array.
#[wasm_bindgen]
pub fn get_counter_names(profile_index: usize) -> Result<String, JsError> {
    with_profile(profile_index, |profile| {
        let names: Vec<&str> = profile.counters.iter().map(|c| c.name.as_ref()).collect();
        serde_json::to_string(&names).map_err(|e| JsError::new(&e.to_string()))
    })
}

/// Get marker names for a profile as JSON array.
#[wasm_bindgen]
pub fn get_marker_names(profile_index: usize) -> Result<String, JsError> {
    with_profile(profile_index, |profile| {
        let names: Vec<&str> = profile.markers.iter().map(|m| m.name.as_ref()).collect();
        serde_json::to_string(&names).map_err(|e| JsError::new(&e.to_string()))
    })
}

/// Get a summary of extra data tracks available in a profile.
#[wasm_bindgen]
pub fn get_extra_tracks(profile_index: usize) -> Result<String, JsError> {
    with_profile(profile_index, |profile| {
        #[derive(serde::Serialize)]
        struct ExtraTracks {
            counter_count: usize,
            marker_count: usize,
            async_span_count: usize,
            flow_arrow_count: usize,
            instant_event_count: usize,
            object_event_count: usize,
            has_cpu_samples: bool,
            screenshot_count: usize,
            has_frames: bool,
            counter_names: Vec<String>,
            marker_names: Vec<String>,
        }

        let info = ExtraTracks {
            counter_count: profile.counters.len(),
            marker_count: profile.markers.len(),
            async_span_count: profile.async_spans.len(),
            flow_arrow_count: profile.flow_arrows.len(),
            instant_event_count: profile.instant_events.len(),
            object_event_count: profile.object_events.len(),
            has_cpu_samples: profile.cpu_samples.is_some(),
            screenshot_count: profile.screenshots.len(),
            has_frames: !profile.frames.is_empty(),
            counter_names: profile
                .counters
                .iter()
                .map(|c| c.name.to_string())
                .collect(),
            marker_names: profile
                .markers
                .iter()
                .map(|m| m.name.to_string())
                .collect(),
        };

        serde_json::to_string(&info).map_err(|e| JsError::new(&e.to_string()))
    })
}

/// Search spans by name (case-insensitive substring match).
/// Returns JSON: { match_count, total_count }
#[wasm_bindgen]
pub fn search_spans(profile_index: usize, query: &str) -> Result<String, JsError> {
    with_profile(profile_index, |profile| {
        let lower_query = query.to_lowercase();
        let mut match_count = 0usize;
        let mut total_count = 0usize;

        for thread in &profile.threads {
            for span in &thread.spans {
                total_count += 1;
                if span.name.to_lowercase().contains(&lower_query) {
                    match_count += 1;
                }
            }
        }

        #[derive(serde::Serialize)]
        struct SearchResult {
            match_count: usize,
            total_count: usize,
        }

        let result = SearchResult {
            match_count,
            total_count,
        };
        serde_json::to_string(&result).map_err(|e| JsError::new(&e.to_string()))
    })
}

/// Get flow arrows for the visible time range.
#[wasm_bindgen]
pub fn get_flow_arrows(
    profile_index: usize,
    view_start: f64,
    view_end: f64,
) -> Result<String, JsError> {
    with_profile(profile_index, |profile| {
        #[derive(serde::Serialize)]
        struct FlowArrowOut {
            name: String,
            from_ts: f64,
            from_tid: u64,
            to_ts: f64,
            to_tid: u64,
        }

        let arrows: Vec<FlowArrowOut> = profile
            .flow_arrows
            .iter()
            .filter(|a| {
                // Include if either endpoint is in the visible range
                (a.from_ts >= view_start && a.from_ts <= view_end)
                    || (a.to_ts >= view_start && a.to_ts <= view_end)
                    || (a.from_ts <= view_start && a.to_ts >= view_end)
            })
            .map(|a| FlowArrowOut {
                name: a.name.to_string(),
                from_ts: a.from_ts,
                from_tid: a.from_tid,
                to_ts: a.to_ts,
                to_tid: a.to_tid,
            })
            .collect();

        serde_json::to_string(&arrows).map_err(|e| JsError::new(&e.to_string()))
    })
}

/// Get network requests for the visible time range.
#[wasm_bindgen]
pub fn get_network_requests(
    profile_index: usize,
    view_start: f64,
    view_end: f64,
) -> Result<String, JsError> {
    with_profile(profile_index, |profile| {
        #[derive(serde::Serialize)]
        struct NetReqOut {
            url: String,
            send_ts: f64,
            response_ts: Option<f64>,
            finish_ts: Option<f64>,
            mime_type: Option<String>,
            from_cache: bool,
        }

        let reqs: Vec<NetReqOut> = profile
            .network_requests
            .iter()
            .filter(|r| {
                let end = r.finish_ts.or(r.response_ts).unwrap_or(r.send_ts);
                r.send_ts <= view_end && end >= view_start
            })
            .map(|r| NetReqOut {
                url: r.url.to_string(),
                send_ts: r.send_ts,
                response_ts: r.response_ts,
                finish_ts: r.finish_ts,
                mime_type: r.mime_type.as_ref().map(ToString::to_string),
                from_cache: r.from_cache,
            })
            .collect();

        serde_json::to_string(&reqs).map_err(|e| JsError::new(&e.to_string()))
    })
}

/// Get screenshots for a profile as JSON array of {ts, data}.
#[wasm_bindgen]
pub fn get_screenshots(profile_index: usize) -> Result<String, JsError> {
    with_profile(profile_index, |profile| {
        #[derive(serde::Serialize)]
        struct ScreenshotOut {
            ts: f64,
            data: String,
        }
        let shots: Vec<ScreenshotOut> = profile
            .screenshots
            .iter()
            .map(|s| ScreenshotOut {
                ts: s.ts,
                data: s.data.clone(),
            })
            .collect();
        serde_json::to_string(&shots).map_err(|e| JsError::new(&e.to_string()))
    })
}
