use std::sync::Mutex;

use flame_cat_core::views::{left_heavy, minimap, ranked, sandwich, time_order};
use flame_cat_protocol::{Viewport, VisualProfile};
use wasm_bindgen::prelude::*;

static PROFILES: Mutex<Vec<VisualProfile>> = Mutex::new(Vec::new());

fn lock_profiles() -> Result<std::sync::MutexGuard<'static, Vec<VisualProfile>>, JsError> {
    PROFILES
        .lock()
        .map_err(|_| JsError::new("internal error: profile store lock poisoned"))
}

/// Parse a profile from bytes (auto-detects format). Returns a handle (index) for later use.
#[wasm_bindgen]
pub fn parse_profile(data: &[u8]) -> Result<usize, JsError> {
    let profile = flame_cat_core::parsers::parse_auto_visual(data)
        .map_err(|e| JsError::new(&e.to_string()))?;
    let mut profiles = lock_profiles()?;
    let idx = profiles.len();
    profiles.push(profile);
    Ok(idx)
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
    let profiles = lock_profiles()?;
    let profile = profiles
        .get(profile_index)
        .ok_or_else(|| JsError::new("invalid profile index"))?;

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
        "left-heavy" => left_heavy::render_left_heavy(profile, &viewport),
        "sandwich" => {
            let frame_id = selected_frame_id
                .ok_or_else(|| JsError::new("sandwich requires selected_frame_id"))?;
            sandwich::render_sandwich(profile, frame_id, &viewport)
        }
        "ranked" => ranked::render_ranked(profile, &viewport, ranked::RankedSort::SelfTime, false),
        _ => return Err(JsError::new(&format!("unknown view type: {view_type}"))),
    };

    serde_json::to_string(&commands).map_err(|e| JsError::new(&e.to_string()))
}

/// Get profile metadata as JSON.
#[wasm_bindgen]
pub fn get_profile_metadata(profile_index: usize) -> Result<String, JsError> {
    let profiles = lock_profiles()?;
    let profile = profiles
        .get(profile_index)
        .ok_or_else(|| JsError::new("invalid profile index"))?;
    serde_json::to_string(&profile.meta).map_err(|e| JsError::new(&e.to_string()))
}

/// Get the number of spans in a profile.
#[wasm_bindgen]
pub fn get_frame_count(profile_index: usize) -> Result<usize, JsError> {
    let profiles = lock_profiles()?;
    let profile = profiles
        .get(profile_index)
        .ok_or_else(|| JsError::new("invalid profile index"))?;
    Ok(profile.span_count())
}

/// Look up span details by frame ID. Returns JSON with name, start, end,
/// duration, self_value, depth, category, and thread name.
#[wasm_bindgen]
pub fn get_span_info(profile_index: usize, frame_id: u64) -> Result<String, JsError> {
    let profiles = lock_profiles()?;
    let profile = profiles
        .get(profile_index)
        .ok_or_else(|| JsError::new("invalid profile index"))?;

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
                return serde_json::to_string(&info)
                    .map_err(|e| JsError::new(&e.to_string()));
            }
        }
    }

    Err(JsError::new(&format!("span {frame_id} not found")))
}

/// Get the actual content time bounds (min span start, max span end) as JSON.
/// Useful for zoom-to-fit, skipping empty regions at the edges.
#[wasm_bindgen]
pub fn get_content_bounds(profile_index: usize) -> Result<String, JsError> {
    let profiles = lock_profiles()?;
    let profile = profiles
        .get(profile_index)
        .ok_or_else(|| JsError::new("invalid profile index"))?;

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
    let profiles = lock_profiles()?;
    let profile = profiles
        .get(profile_index)
        .ok_or_else(|| JsError::new("invalid profile index"))?;

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
}

/// Get the list of thread groups for a profile as JSON.
///
/// Returns an array of `{ id, name, span_count, sort_key, max_depth }` objects.
#[wasm_bindgen]
pub fn get_thread_list(profile_index: usize) -> Result<String, JsError> {
    let profiles = lock_profiles()?;
    let profile = profiles
        .get(profile_index)
        .ok_or_else(|| JsError::new("invalid profile index"))?;

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
}

/// Get ranked entries for a profile as JSON.
#[wasm_bindgen]
pub fn get_ranked_entries(
    profile_index: usize,
    sort_field: &str,
    ascending: bool,
) -> Result<String, JsError> {
    let profiles = lock_profiles()?;
    let profile = profiles
        .get(profile_index)
        .ok_or_else(|| JsError::new("invalid profile index"))?;

    let sort = match sort_field {
        "self" => ranked::RankedSort::SelfTime,
        "total" => ranked::RankedSort::TotalTime,
        "name" => ranked::RankedSort::Name,
        "count" => ranked::RankedSort::Count,
        _ => ranked::RankedSort::SelfTime,
    };

    let entries = ranked::get_ranked_entries(profile, sort, ascending);
    serde_json::to_string(&entries).map_err(|e| JsError::new(&e.to_string()))
}
