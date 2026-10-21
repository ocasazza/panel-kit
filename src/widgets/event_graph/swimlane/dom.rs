//! The browser side of the graph: a timer for the width debounce. Off the web
//! it resolves at once, so the component renders the same markup under SSR.

/// Resolves after `ms` milliseconds.
#[cfg(target_arch = "wasm32")]
pub async fn sleep(ms: i32) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let tick = js_sys::Promise::new(&mut |resolve, _reject| {
        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
    });
    let _ = wasm_bindgen_futures::JsFuture::from(tick).await;
}

/// Resolves at once: nothing resizes off the web.
#[cfg(not(target_arch = "wasm32"))]
pub async fn sleep(_ms: i32) {}
