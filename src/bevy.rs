//! Bevy canvas component backed by a dynamically imported wasm-pack bundle.
//!
//! The JavaScript module must export wasm-pack's async `default()` initializer
//! and a `WebHandle` class with `new()` and `start(canvas)` methods. A running
//! handle is retained by canvas ID and can be retrieved with
//! [`get_bevy_handle`] for application-specific control calls.

use std::cell::RefCell;
use std::collections::HashMap;

use dioxus::prelude::*;
use wasm_bindgen::prelude::*;

thread_local! {
    /// Running Bevy handles keyed by the canvas DOM id.
    static BEVY_HANDLES: RefCell<HashMap<String, JsValue>> = RefCell::new(HashMap::new());
}

/// Return the running Bevy handle associated with `canvas_id`, if it is ready.
pub fn get_bevy_handle(canvas_id: &str) -> Option<JsValue> {
    BEVY_HANDLES.with(|handles| handles.borrow().get(canvas_id).cloned())
}

fn store_bevy_handle(canvas_id: &str, handle: JsValue) {
    BEVY_HANDLES.with(|handles| {
        handles.borrow_mut().insert(canvas_id.to_string(), handle);
    });
}

/// Styles for [`BevyCanvas`].
///
/// These rules are also shipped in the panel-kit stylesheet. The constant is
/// available to consumers that install this feature without the full sheet.
pub const BEVY_CSS: &str = r#"
.bevy-canvas-container { width: 100%; height: 100%; position: relative; overflow: hidden; }
.bevy-canvas { width: 100%; height: 100%; display: block; touch-action: none; position: relative; z-index: 1; }
.bevy-canvas-loading {
    position: absolute; inset: 0; z-index: 0;
    display: flex; align-items: center; justify-content: center;
    color: var(--dim, #7a7a7a); font-size: 12px;
    pointer-events: none;
}
.bevy-canvas-error {
    position: absolute; inset: 0; z-index: 2;
    display: flex; align-items: center; justify-content: center;
    color: var(--red, #ff5f56); font-size: 12px; padding: 8px; text-align: center;
}
"#;

#[derive(Clone, PartialEq)]
enum LoadState {
    Loading,
    Running,
    Error(String),
}

/// Load a wasm-pack Bevy bundle and start its `WebHandle` on a canvas.
#[component]
pub fn BevyCanvas(
    /// URL of the JavaScript module, for example `/wasm/demo/pkg/demo.js`.
    module_path: String,
    /// Unique canvas element ID.
    #[props(default = "bevy-canvas".to_string())]
    canvas_id: String,
    /// Text displayed until `WebHandle.start` is called.
    #[props(default = "loading...".to_string())]
    loading_text: String,
) -> Element {
    let mut state = use_signal(|| LoadState::Loading);
    let effect_canvas_id = canvas_id.clone();
    let effect_module_path = module_path.clone();

    use_effect(move || {
        let canvas_id = effect_canvas_id.clone();
        let module_path = effect_module_path.clone();

        wasm_bindgen_futures::spawn_local(async move {
            match load_and_start_bevy(&module_path, &canvas_id).await {
                Ok(()) => state.set(LoadState::Running),
                Err(error) => {
                    let message = format!("{error:?}");
                    web_sys::console::error_1(&message.clone().into());
                    state.set(LoadState::Error(message));
                }
            }
        });
    });

    let current_state = state.read().clone();

    rsx! {
        div { class: "bevy-canvas-container",
            canvas {
                id: "{canvas_id}",
                class: "bevy-canvas",
            }
            match current_state {
                LoadState::Loading => rsx! {
                    div {
                        class: "bevy-canvas-loading",
                        role: "status",
                        aria_live: "polite",
                        "{loading_text}"
                    }
                },
                LoadState::Error(message) => rsx! {
                    div {
                        class: "bevy-canvas-error",
                        role: "alert",
                        "{message}"
                    }
                },
                LoadState::Running => rsx! {},
            }
        }
    }
}

/// Dynamically import, initialize, and start one Bevy web bundle.
async fn load_and_start_bevy(module_path: &str, canvas_id: &str) -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;

    // Let Dioxus commit the canvas node before querying for it.
    let tick = js_sys::Promise::new(&mut |resolve, _reject| {
        window
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 10)
            .expect("window.setTimeout should accept the callback");
    });
    wasm_bindgen_futures::JsFuture::from(tick).await?;

    // Keep this as Element rather than HtmlCanvasElement: WebHandle.start sees
    // the same JS object, while panel-kit does not need an extra web-sys feature.
    let canvas = document
        .get_element_by_id(canvas_id)
        .ok_or_else(|| JsValue::from_str(&format!("canvas '{canvas_id}' not found")))?;

    let import_value = js_sys::eval(&format!(r#"import("{module_path}")"#))?;
    let module = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(import_value)).await?;

    let initializer = js_sys::Reflect::get(&module, &JsValue::from_str("default"))?;
    let init_result = js_sys::Function::from(initializer).call0(&JsValue::NULL)?;
    wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(init_result)).await?;

    let handle_class = js_sys::Reflect::get(&module, &JsValue::from_str("WebHandle"))?;
    let handle =
        js_sys::Reflect::construct(&js_sys::Function::from(handle_class), &js_sys::Array::new())?;
    store_bevy_handle(canvas_id, handle.clone());

    // Bevy owns the event loop after start. Do not await the returned promise.
    let start = js_sys::Reflect::get(&handle, &JsValue::from_str("start"))?;
    js_sys::Function::from(start).call1(&handle, canvas.as_ref())?;

    Ok(())
}
