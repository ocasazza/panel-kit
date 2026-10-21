//! The table's browser side: storage, clipboard, download, focus and layout
//! measurements. Every function is a no-op off the web, so the component
//! renders the same markup under SSR and in tests.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

/// The stored payload for a table, if any.
pub fn load(key: &str) -> Option<panel_kit_core::widgets::data_table::StoredTable> {
    #[cfg(target_arch = "wasm32")]
    {
        use gloo_storage::{LocalStorage, Storage};
        LocalStorage::get(key).ok()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = key;
        None
    }
}

/// Store a table's payload.
pub fn save(key: &str, payload: &impl serde::Serialize) {
    #[cfg(target_arch = "wasm32")]
    {
        use gloo_storage::{LocalStorage, Storage};
        let _ = LocalStorage::set(key, payload);
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (key, payload);
}

/// Put text on the clipboard.
pub fn copy(text: &str) {
    #[cfg(target_arch = "wasm32")]
    if let Some(window) = web_sys::window() {
        let _ = window.navigator().clipboard().write_text(text);
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = text;
}

/// Offer text as a file download.
pub fn download(name: &str, mime: &str, text: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        let parts = js_sys::Array::of1(&wasm_bindgen::JsValue::from_str(text));
        let options = web_sys::BlobPropertyBag::new();
        options.set_type(mime);
        let Ok(blob) = web_sys::Blob::new_with_str_sequence_and_options(&parts, &options) else {
            return;
        };
        let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) else {
            return;
        };
        let anchor = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.create_element("a").ok())
            .and_then(|a| a.dyn_into::<web_sys::HtmlAnchorElement>().ok());
        if let Some(anchor) = anchor {
            anchor.set_href(&url);
            anchor.set_download(name);
            anchor.click();
        }
        let _ = web_sys::Url::revoke_object_url(&url);
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (name, mime, text);
}

#[cfg(target_arch = "wasm32")]
fn element(id: &str) -> Option<web_sys::Element> {
    web_sys::window()?.document()?.get_element_by_id(id)
}

/// Whether focus is in a text field, which owns its own keys.
pub fn typing() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.active_element())
            .map(|el| {
                let tag = el.tag_name();
                tag == "INPUT"
                    || tag == "TEXTAREA"
                    || tag == "SELECT"
                    || el.has_attribute("contenteditable")
            })
            .unwrap_or(false)
    }
    #[cfg(not(target_arch = "wasm32"))]
    false
}

/// Whether the page has a text selection, which ctrl-C must copy instead.
pub fn selecting_text() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|w| w.get_selection().ok().flatten())
            .and_then(|s| s.to_string().as_string())
            .is_some_and(|text| !text.is_empty())
    }
    #[cfg(not(target_arch = "wasm32"))]
    false
}

/// Focus an element without scrolling, then bring it into view if `scroll`
/// says the browser should.
pub fn focus(id: &str, scroll: bool) {
    #[cfg(target_arch = "wasm32")]
    if let Some(el) = element(id).and_then(|el| el.dyn_into::<web_sys::HtmlElement>().ok()) {
        let options = web_sys::FocusOptions::new();
        options.set_prevent_scroll(true);
        let _ = el.focus_with_options(&options);
        if scroll {
            let view = web_sys::ScrollIntoViewOptions::new();
            view.set_block(web_sys::ScrollLogicalPosition::Nearest);
            el.scroll_into_view_with_scroll_into_view_options(&view);
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (id, scroll);
}

/// Whether a focus-out moved focus to an element outside `root`. A row that
/// unmounts under the cursor blurs to nothing, which is not leaving.
pub fn left_for_outside(event: &dioxus::prelude::FocusEvent, root: &str) -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        let Some(web) = event.data().downcast::<web_sys::FocusEvent>().cloned() else {
            return true;
        };
        let Some(next) = web
            .related_target()
            .and_then(|t| t.dyn_into::<web_sys::Node>().ok())
        else {
            return false;
        };
        !element(root).is_some_and(|table| table.contains(Some(&next)))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (event, root);
        true
    }
}

/// Wait `ms` milliseconds; resolves at once off the web.
pub async fn sleep(ms: i32) {
    #[cfg(target_arch = "wasm32")]
    {
        let promise = js_sys::Promise::new(&mut |resolve, _| {
            if let Some(window) = web_sys::window() {
                let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
            }
        });
        let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = ms;
}

/// An element's rendered width.
pub fn width(id: &str) -> Option<f64> {
    #[cfg(target_arch = "wasm32")]
    {
        element(id).map(|el| el.get_bounding_client_rect().width())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = id;
        None
    }
}

/// What the scroller shows: its scroll offset and the height rows get below
/// the header.
pub struct Scroll {
    /// Pixels scrolled.
    pub top: f64,
    /// Visible height under the sticky header.
    pub visible: f64,
}

/// Measure the scroller `id`.
pub fn scroll(id: &str) -> Option<Scroll> {
    #[cfg(target_arch = "wasm32")]
    {
        let el = element(id)?;
        let head = el
            .query_selector("thead")
            .ok()
            .flatten()
            .map(|h| h.get_bounding_client_rect().height())
            .unwrap_or(0.0);
        Some(Scroll {
            top: f64::from(el.scroll_top()),
            visible: (f64::from(el.client_height()) - head).max(0.0),
        })
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = id;
        None
    }
}

/// Scroll the scroller `id` to `top` pixels.
pub fn scroll_to(id: &str, top: f64) {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(el) = element(id) {
            el.set_scroll_top(top.round() as i32);
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (id, top);
    }
}
