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

/// Hands focus from a row that has scrolled out of the scroller `id` to the
/// scroller itself. A row the window drops would otherwise take focus with it
/// to the document, and the page keys would stop scrolling.
pub fn release_hidden_focus(id: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        let Some(root) = element(id) else {
            return;
        };
        let Some(active) = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.active_element())
        else {
            return;
        };
        if active == root || !root.contains(Some(&active)) {
            return;
        }
        let (view, at) = (root.get_bounding_client_rect(), active.get_bounding_client_rect());
        if at.bottom() > view.top() && at.top() < view.bottom() {
            return;
        }
        if let Ok(root) = root.dyn_into::<web_sys::HtmlElement>() {
            let options = web_sys::FocusOptions::new();
            options.set_prevent_scroll(true);
            let _ = root.focus_with_options(&options);
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = id;
}
