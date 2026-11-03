//! Build-progress painters over the core build feed model.
//!
//! Ported from jump-cannon's `build_progress.rs` painters: when a source is
//! still indexing, two surfaces render the wait and share this markup. The
//! fold itself lives in [`panel_kit_core::widgets::build_progress`].

use dioxus::prelude::*;

/// The build progress bar: a determinate `<progress>` when the host reports a
/// fraction, otherwise a CSS-animated indeterminate bar (no JavaScript).
pub fn progress_bar(fraction: Option<f32>) -> Element {
    match fraction {
        Some(f) => {
            let f = f.clamp(0.0, 1.0);
            rsx! {
                progress {
                    class: "pk-build-progress-bar",
                    max: "1",
                    value: "{f}",
                }
            }
        }
        None => rsx! {
            div {
                class: "pk-build-progress-bar",
                "indeterminate": "true",
                role: "progressbar",
                aria_label: "importing",
            }
        },
    }
}

/// First-paint placeholder for a panel (or a panel sub-surface) whose initial
/// content is still being fetched. Presentational only: the shared
/// [`progress_bar`] — indeterminate unless `fraction` is known — above a
/// concrete label, so each surface shows its own hydration state instead of
/// hiding behind one global wait. Distinct from [`crate::Spinner`], which
/// stays for transient in-panel actions after first paint.
#[component]
pub fn PanelHydrating(
    /// Concrete phase text, e.g. `indexing sources…`.
    label: String,
    /// Completion in `0.0..=1.0` once measured; `None` stays indeterminate.
    #[props(default)]
    fraction: Option<f32>,
) -> Element {
    rsx! {
        div { class: "pk-panel-hydrating", role: "status", aria_live: "polite",
            {progress_bar(fraction)}
            span { class: "pk-panel-hydrating-label", "{label}" }
        }
    }
}
