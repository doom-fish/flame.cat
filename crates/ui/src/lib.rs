mod app;
mod renderer;
mod theme;

pub use app::FlameApp;

/// Commands that can be sent from JS to the egui app.
#[derive(Debug)]
pub enum AppCommand {
    SetTheme(theme::ThemeMode),
    SetSearch(String),
    ResetZoom,
}

/// Global command queue drained by the app each frame.
static COMMAND_QUEUE: std::sync::Mutex<Vec<AppCommand>> = std::sync::Mutex::new(Vec::new());

/// Push a command to the global queue.
pub fn push_command(cmd: AppCommand) {
    if let Ok(mut q) = COMMAND_QUEUE.lock() {
        q.push(cmd);
    }
}

/// Drain all pending commands.
pub fn drain_commands() -> Vec<AppCommand> {
    if let Ok(mut q) = COMMAND_QUEUE.lock() {
        std::mem::take(&mut *q)
    } else {
        Vec::new()
    }
}

// WASM entry point + JS API
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

/// Global handle to the pending_data channel so JS can push profile data.
#[cfg(target_arch = "wasm32")]
static PENDING_DATA: std::sync::OnceLock<std::sync::Arc<std::sync::Mutex<Option<Vec<u8>>>>> =
    std::sync::OnceLock::new();

/// Global handle to the egui context for requesting repaints from JS.
#[cfg(target_arch = "wasm32")]
static EGUI_CTX: std::sync::OnceLock<egui::Context> = std::sync::OnceLock::new();

/// Default entry point — mounts on `#flame_cat_canvas`.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    start_on_canvas("flame_cat_canvas")
}

/// Mount the flame graph viewer on a canvas element with the given DOM ID.
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

/// Load a profile from JS. Accepts a `Uint8Array` of profile data.
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

/// Set theme: "dark" or "light".
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

/// Set search query. Empty string clears search.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = "setSearch")]
pub fn set_search(query: &str) {
    push_command(AppCommand::SetSearch(query.to_string()));
    request_repaint();
}

/// Reset zoom to fit all data.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = "resetZoom")]
pub fn reset_zoom() {
    push_command(AppCommand::ResetZoom);
    request_repaint();
}

#[cfg(target_arch = "wasm32")]
fn request_repaint() {
    if let Some(ctx) = EGUI_CTX.get() {
        ctx.request_repaint();
    }
}
