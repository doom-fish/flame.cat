mod app;
mod renderer;
mod theme;

pub use app::FlameApp;

/// Active visualization mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewType {
    TimeOrder,
    LeftHeavy,
    Sandwich,
    Ranked,
}

impl Default for ViewType {
    fn default() -> Self {
        Self::TimeOrder
    }
}

/// Commands that can be sent from JS to the egui app.
#[derive(Debug)]
pub enum AppCommand {
    SetTheme(theme::ThemeMode),
    SetSearch(String),
    ResetZoom,
    SetViewport(f64, f64),
    SetLaneVisibility(usize, bool),
    SetLaneHeight(usize, f32),
    ReorderLanes(usize, usize),
    SelectSpan(Option<u64>),
    SetViewType(ViewType),
    NavigateBack,
    NavigateForward,
    NavigateToParent,
    NavigateToChild,
    NavigateToNextSibling,
    NavigateToPrevSibling,
    NextSearchResult,
    PrevSearchResult,
}

/// Global command queue drained by the app each frame.
static COMMAND_QUEUE: std::sync::Mutex<Vec<AppCommand>> = std::sync::Mutex::new(Vec::new());

pub fn push_command(cmd: AppCommand) {
    if let Ok(mut q) = COMMAND_QUEUE.lock() {
        q.push(cmd);
    }
}

pub fn drain_commands() -> Vec<AppCommand> {
    if let Ok(mut q) = COMMAND_QUEUE.lock() {
        std::mem::take(&mut *q)
    } else {
        Vec::new()
    }
}

/// Lightweight state snapshot written by the app each frame, read by JS.
#[derive(Default, serde::Serialize)]
pub struct StateSnapshot {
    pub profile: Option<ProfileSnapshot>,
    pub lanes: Vec<LaneSnapshot>,
    pub viewport: ViewportSnapshot,
    pub selected: Option<SelectedSpanSnapshot>,
    pub hovered: Option<SelectedSpanSnapshot>,
    pub search: String,
    pub theme: String,
    pub view_type: ViewType,
    pub can_go_back: bool,
    pub can_go_forward: bool,
}

#[derive(serde::Serialize)]
pub struct ProfileSnapshot {
    pub name: Option<String>,
    pub format: String,
    pub duration_us: f64,
    pub start_time: f64,
    pub end_time: f64,
    pub span_count: usize,
    pub thread_count: usize,
}

#[derive(serde::Serialize)]
pub struct LaneSnapshot {
    pub name: String,
    pub kind: String,
    pub height: f32,
    pub visible: bool,
    pub span_count: usize,
}

#[derive(Default, serde::Serialize)]
pub struct ViewportSnapshot {
    pub start: f64,
    pub end: f64,
    pub scroll_y: f32,
}

#[derive(serde::Serialize)]
pub struct SelectedSpanSnapshot {
    pub name: String,
    pub frame_id: u64,
    pub lane_index: usize,
    pub start_us: f64,
    pub end_us: f64,
}

static STATE: std::sync::Mutex<StateSnapshot> = std::sync::Mutex::new(StateSnapshot {
    profile: None,
    lanes: Vec::new(),
    viewport: ViewportSnapshot {
        start: 0.0,
        end: 1.0,
        scroll_y: 0.0,
    },
    selected: None,
    hovered: None,
    search: String::new(),
    theme: String::new(),
    view_type: ViewType::TimeOrder,
    can_go_back: false,
    can_go_forward: false,
});

/// Cached serialized profile for export (set when profile loads).
static PROFILE_JSON: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

pub fn set_profile_json(json: Option<String>) {
    if let Ok(mut p) = PROFILE_JSON.lock() {
        *p = json;
    }
}

pub fn write_snapshot(snap: StateSnapshot) {
    let changed = if let Ok(mut s) = STATE.lock() {
        let changed = s.viewport.start != snap.viewport.start
            || s.viewport.end != snap.viewport.end
            || s.viewport.scroll_y != snap.viewport.scroll_y
            || s.search != snap.search
            || s.theme != snap.theme
            || s.selected.is_some() != snap.selected.is_some()
            || s.hovered.is_some() != snap.hovered.is_some()
            || s.profile.is_some() != snap.profile.is_some()
            || s.lanes.len() != snap.lanes.len()
            || std::mem::discriminant(&s.view_type) != std::mem::discriminant(&snap.view_type);
        *s = snap;
        changed
    } else {
        false
    };
    if changed {
        #[cfg(target_arch = "wasm32")]
        notify_js();
    }
}

// ── WASM entry point + JS API ──────────────────────────────────────────

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

#[cfg(target_arch = "wasm32")]
static PENDING_DATA: std::sync::OnceLock<std::sync::Arc<std::sync::Mutex<Option<Vec<u8>>>>> =
    std::sync::OnceLock::new();

#[cfg(target_arch = "wasm32")]
static EGUI_CTX: std::sync::OnceLock<egui::Context> = std::sync::OnceLock::new();

/// Store JS callback in thread-local (WASM is single-threaded).
#[cfg(target_arch = "wasm32")]
thread_local! {
    static STATE_CALLBACK: std::cell::RefCell<Option<js_sys::Function>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(target_arch = "wasm32")]
fn notify_js() {
    STATE_CALLBACK.with(|cb| {
        if let Some(f) = cb.borrow().as_ref() {
            let _ = f.call0(&JsValue::NULL);
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn request_repaint() {
    if let Some(ctx) = EGUI_CTX.get() {
        ctx.request_repaint();
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    start_on_canvas("flame_cat_canvas")
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = "startOnCanvas")]
pub fn start_on_canvas(canvas_id: &str) -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    let web_options = eframe::WebOptions::default();
    let id = canvas_id.to_string();
    wasm_bindgen_futures::spawn_local(async move {
        let document = web_sys::window()
            .expect("no window")
            .document()
            .expect("no document");
        let canvas = document
            .get_element_by_id(&id)
            .unwrap_or_else(|| panic!("no canvas element with id '{id}'"))
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("element is not a canvas");
        let start_result = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| {
                    let app = FlameApp::new(cc);
                    let _ = PENDING_DATA.set(app.pending_data_handle());
                    let _ = EGUI_CTX.set(cc.egui_ctx.clone());
                    Ok(Box::new(app))
                }),
            )
            .await;
        if let Err(e) = start_result {
            web_sys::console::error_1(&format!("Failed to start eframe: {e:?}").into());
        }
    });
    Ok(())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = "loadProfile")]
pub fn load_profile(data: &[u8]) -> Result<(), JsValue> {
    let pending = PENDING_DATA
        .get()
        .ok_or_else(|| JsValue::from_str("flame-cat not initialized yet"))?;
    if let Ok(mut lock) = pending.lock() {
        *lock = Some(data.to_vec());
    }
    request_repaint();
    Ok(())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = "setTheme")]
pub fn set_theme(mode: &str) -> Result<(), JsValue> {
    let theme = match mode {
        "light" => theme::ThemeMode::Light,
        "dark" => theme::ThemeMode::Dark,
        _ => return Err(JsValue::from_str("theme must be 'dark' or 'light'")),
    };
    push_command(AppCommand::SetTheme(theme));
    request_repaint();
    Ok(())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = "setSearch")]
pub fn set_search(query: &str) {
    push_command(AppCommand::SetSearch(query.to_string()));
    request_repaint();
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = "resetZoom")]
pub fn reset_zoom() {
    push_command(AppCommand::ResetZoom);
    request_repaint();
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = "setViewport")]
pub fn set_viewport(start: f64, end: f64) {
    push_command(AppCommand::SetViewport(start, end));
    request_repaint();
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = "setLaneVisibility")]
pub fn set_lane_visibility(index: usize, visible: bool) {
    push_command(AppCommand::SetLaneVisibility(index, visible));
    request_repaint();
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = "setLaneHeight")]
pub fn set_lane_height(index: usize, height: f32) {
    push_command(AppCommand::SetLaneHeight(index, height));
    request_repaint();
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = "reorderLanes")]
pub fn reorder_lanes(from_index: usize, to_index: usize) {
    push_command(AppCommand::ReorderLanes(from_index, to_index));
    request_repaint();
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = "setViewType")]
pub fn set_view_type(view_type: &str) -> Result<(), JsValue> {
    let vt = match view_type {
        "time_order" => ViewType::TimeOrder,
        "left_heavy" => ViewType::LeftHeavy,
        "sandwich" => ViewType::Sandwich,
        "ranked" => ViewType::Ranked,
        _ => {
            return Err(JsValue::from_str(
                "view_type must be 'time_order', 'left_heavy', 'sandwich', or 'ranked'",
            ))
        }
    };
    push_command(AppCommand::SetViewType(vt));
    request_repaint();
    Ok(())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = "navigateBack")]
pub fn navigate_back() {
    push_command(AppCommand::NavigateBack);
    request_repaint();
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = "navigateForward")]
pub fn navigate_forward() {
    push_command(AppCommand::NavigateForward);
    request_repaint();
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = "selectSpan")]
pub fn select_span(frame_id: Option<u64>) {
    push_command(AppCommand::SelectSpan(frame_id));
    request_repaint();
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = "navigateToParent")]
pub fn navigate_to_parent() {
    push_command(AppCommand::NavigateToParent);
    request_repaint();
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = "navigateToChild")]
pub fn navigate_to_child() {
    push_command(AppCommand::NavigateToChild);
    request_repaint();
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = "navigateToNextSibling")]
pub fn navigate_to_next_sibling() {
    push_command(AppCommand::NavigateToNextSibling);
    request_repaint();
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = "navigateToPrevSibling")]
pub fn navigate_to_prev_sibling() {
    push_command(AppCommand::NavigateToPrevSibling);
    request_repaint();
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = "nextSearchResult")]
pub fn next_search_result() {
    push_command(AppCommand::NextSearchResult);
    request_repaint();
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = "prevSearchResult")]
pub fn prev_search_result() {
    push_command(AppCommand::PrevSearchResult);
    request_repaint();
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = "onStateChange")]
pub fn on_state_change(callback: js_sys::Function) {
    STATE_CALLBACK.with(|cb| {
        *cb.borrow_mut() = Some(callback);
    });
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = "exportProfile")]
pub fn export_profile() -> Option<String> {
    if let Ok(p) = PROFILE_JSON.lock() {
        p.clone()
    } else {
        None
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = "getState")]
pub fn get_state() -> String {
    if let Ok(s) = STATE.lock() {
        serde_json::to_string(&*s).unwrap_or_default()
    } else {
        "{}".to_string()
    }
}
