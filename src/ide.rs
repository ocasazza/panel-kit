//! Dependency-light code and manifest editor.
//!
//! [`IdePanel`] complements the Monaco-based editor with a controlled
//! `<textarea>`, a language tag, and a line-number gutter. The gutter mirrors
//! the textarea's vertical scroll position without a JavaScript editor runtime.

use std::sync::atomic::{AtomicU64, Ordering};

use dioxus::prelude::*;

/// Process-wide counter used to give each textarea a stable, unique DOM id.
static IDE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// A controlled monospace editor with a scroll-synchronized line-number gutter.
#[component]
pub fn IdePanel(
    /// Current document text. Hosts own the value and update it from
    /// [`on_change`](IdePanelProps::on_change).
    value: String,
    /// Language tag shown in the header and exposed as `data-language`.
    /// Defaults to `text`.
    #[props(default)]
    language: Option<String>,
    /// Called with the complete textarea value after each editable input.
    on_change: EventHandler<String>,
    /// Mark the textarea read-only and suppress change events.
    #[props(default)]
    read_only: bool,
    /// Optional document title shown in the header.
    #[props(default)]
    title: Option<String>,
) -> Element {
    let mut scroll_top = use_signal(|| 0.0_f64);
    let textarea_id = use_hook(|| {
        format!(
            "ide-textarea-{}",
            IDE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        )
    });
    let language = language.unwrap_or_else(|| "text".to_string());
    // A trailing newline creates the empty final row rendered by the textarea.
    let line_count = value.bytes().filter(|byte| *byte == b'\n').count() + 1;

    let class = if read_only {
        "ide-panel read-only"
    } else {
        "ide-panel"
    };
    let gutter_style = format!("transform:translateY(-{}px);", scroll_top());

    rsx! {
        div {
            class,
            "data-language": "{language}",
            header { class: "ide-head",
                if let Some(title) = title.as_ref() {
                    span { class: "ide-title", title: "{title}", "{title}" }
                }
                span { class: "ide-spacer" }
                span { class: "ide-lang", "{language}" }
                if read_only {
                    span { class: "ide-readonly", "read-only" }
                }
            }
            div { class: "ide-body",
                div { class: "ide-gutter",
                    div { class: "ide-gutter-inner", style: "{gutter_style}",
                        for line in 1..=line_count {
                            div { class: "ide-line-no", "{line}" }
                        }
                    }
                }
                textarea {
                    id: "{textarea_id}",
                    class: "ide-textarea",
                    spellcheck: "false",
                    autocomplete: "off",
                    autocapitalize: "off",
                    "autocorrect": "off",
                    wrap: "off",
                    readonly: read_only,
                    value: "{value}",
                    oninput: move |event| {
                        if !read_only {
                            on_change.call(event.value());
                        }
                    },
                    onscroll: {
                        let textarea_id = textarea_id.clone();
                        move |_| {
                            if let Some(top) = element_scroll_top(&textarea_id) {
                                scroll_top.set(top);
                            }
                        }
                    },
                }
            }
        }
    }
}

/// Read the textarea's DOM scroll offset for gutter synchronization.
fn element_scroll_top(id: &str) -> Option<f64> {
    let element = web_sys::window()?.document()?.get_element_by_id(id)?;
    Some(f64::from(element.scroll_top()))
}
