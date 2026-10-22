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

/// What a scroller shows: pixels scrolled and visible height.
pub struct Scroll {
    /// Pixels scrolled from the top.
    pub top: f64,
    /// Visible height of the scroller.
    pub height: f64,
}

#[cfg(target_arch = "wasm32")]
fn element(id: &str) -> Option<web_sys::Element> {
    web_sys::window()?.document()?.get_element_by_id(id)
}

/// Measure the scroller `id`; `None` when it is not mounted and off the web.
pub fn scroll(id: &str) -> Option<Scroll> {
    #[cfg(target_arch = "wasm32")]
    {
        let el = element(id)?;
        Some(Scroll {
            top: f64::from(el.scroll_top()),
            height: f64::from(el.client_height()),
        })
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = id;
        None
    }
}
