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

    let commands = match view_type {
        "time-order" => time_order::render_time_order(profile, &viewport),
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
