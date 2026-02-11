mod app;
mod renderer;
mod theme;

pub use app::FlameApp;

// WASM entry point
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    // Redirect tracing/panics to console
    console_error_panic_hook::set_once();

    let web_options = eframe::WebOptions::default();
    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("no window")
            .document()
            .expect("no document");
        let canvas = document
            .get_element_by_id("flame_cat_canvas")
            .expect("no canvas element with id 'flame_cat_canvas'")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("element is not a canvas");

        let start_result = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(FlameApp::new(cc)))),
            )
            .await;
        if let Err(e) = start_result {
            web_sys::console::error_1(&format!("Failed to start eframe: {e:?}").into());
        }
    });
    Ok(())
}
