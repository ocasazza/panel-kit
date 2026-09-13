//! Store-shaped async loading state for the Dioxus shell, plus the progress
//! components that render it.
//!
//! The API follows store-style state management (pinia): one store per
//! logical data source, created with [`loading_store`] and retrievable
//! from any component by the same id; a small action vocabulary mutates it
//! ([`LoadingStore::begin`], [`LoadingStore::update`],
//! [`LoadingStore::succeed`], [`LoadingStore::fail`]); components subscribe
//! to the snapshot. A process-wide registry of every store feeds
//! [`GlobalLoadingBar`], so one workspace-level surface reflects all
//! in-flight loads without prop drilling.
//!
//! Scope boundary: stores track **in-flight status only** — ephemeral,
//! never persisted, never undoable. Rewind/undo/history of *application*
//! state belongs to the app's snapshot-timeline framework (jump-cannon's
//! `appstate` ring), which stays the single owner of that logic. The two
//! compose by contract: a rewind/restore is modelled as an ordinary store
//! transition (`begin("restoring…")` → `succeed`), so history replays show
//! up on the same bars as first loads and no parallel status vocabulary
//! exists.
//!
//! Display contract (loading bars over spinners):
//!
//! - A pending load renders a [`ProgressBar`], not a spinner. [`Spinner`]
//!   remains for tiny inline waits (a search box), never for panel- or
//!   page-level hydration.
//! - A determinate bar (`fraction: Some`) MUST show its percentage text;
//!   `fraction: None` is the honest indeterminate state — the bar animates
//!   and no number is fabricated.
//!
//! ```no_run
//! use dioxus::prelude::*;
//! use panel_kit::loading::{loading_store, LoadingGate, GlobalLoadingBar};
//!
//! #[component]
//! fn Branches() -> Element {
//!     let store = loading_store("branches", "loading branches…");
//!     use_future(move || async move {
//!         store.begin();
//!         // … fetch, calling store.update(Some(f), None) as chunks arrive …
//!         store.succeed();
//!     });
//!     rsx! {
//!         LoadingGate { store,
//!             div { "the loaded content" }
//!         }
//!     }
//! }
//!
//! #[component]
//! fn TopBar() -> Element {
//!     rsx! { GlobalLoadingBar {} } // aggregates every pending store
//! }
//! ```
//!
//! [`Spinner`]: crate::Spinner

use std::collections::BTreeMap;

use dioxus::prelude::*;
pub use panel_kit_core::loading::{aggregate_pending, AggregateLoad, LoadSnapshot, LoadStatus};

/// Process-wide store registry: id → the store's snapshot signal. Same id,
/// same store — `loading_store` from any component returns a handle over
/// the one shared signal, so a panel body and the workspace-level
/// [`GlobalLoadingBar`] always observe identical state.
static REGISTRY: GlobalSignal<BTreeMap<&'static str, Signal<LoadSnapshot>>> =
    Signal::global(BTreeMap::new);

/// Handle over one data source's loading state. `Copy`; created by
/// [`loading_store`]. All mutation goes through the actions so the
/// snapshot invariants (error cleared on begin, detail kept fresh) hold.
#[derive(Clone, Copy)]
pub struct LoadingStore {
    id: &'static str,
    snapshot: Signal<LoadSnapshot>,
}

impl LoadingStore {
    /// Current snapshot (reactive read).
    pub fn snapshot(&self) -> LoadSnapshot {
        self.snapshot.read().clone()
    }

    /// True while work is in flight.
    pub fn is_pending(&self) -> bool {
        self.snapshot.read().is_pending()
    }

    /// True when data is available and gated content should render.
    pub fn is_ready(&self) -> bool {
        self.snapshot.read().is_ready()
    }

    /// Begin work: `Pending`, indeterminate, clearing any prior error.
    pub fn begin(self) {
        self.update(None, None);
    }

    /// Begin work with phase detail (`stage name`, `"restoring snapshot…"`).
    pub fn begin_with(self, detail: impl Into<String>) {
        self.update(None, Some(detail.into()));
    }

    /// Patch progress: `fraction` is completion in `0.0..=1.0` (`None` keeps
    /// the bar indeterminate), `detail` replaces the phase text when `Some`.
    pub fn update(mut self, fraction: Option<f64>, detail: Option<String>) {
        let mut snap = self.snapshot.write();
        snap.status = LoadStatus::Pending {
            fraction: fraction.map(|f| f.clamp(0.0, 1.0)),
        };
        if detail.is_some() {
            snap.detail = detail;
        }
        snap.error = None;
    }

    /// Finish successfully: gated content renders from the next frame.
    pub fn succeed(mut self) {
        let mut snap = self.snapshot.write();
        snap.status = LoadStatus::Ready;
        snap.detail = None;
        snap.error = None;
    }

    /// Finish with failure: the gate renders `message` instead of content.
    pub fn fail(mut self, message: impl Into<String>) {
        let mut snap = self.snapshot.write();
        snap.status = LoadStatus::Failed;
        snap.error = Some(message.into());
    }
}

impl PartialEq for LoadingStore {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

/// Get (creating on first use) the store registered under `id`.
///
/// `label` is the human text shown by bars and gates while pending, e.g.
/// `"loading branches…"`; it applies on creation — later calls with the same
/// id return the existing store unchanged, so every call site should pass
/// the same label.
///
/// Not a hook despite the call pattern: safe from components, event
/// handlers, and spawned tasks alike (store signals are app-lifetime, never
/// scope-disposed). Creation needs any live Dioxus runtime; call it from a
/// component on first use so headless contexts never race one in.
pub fn loading_store(id: &'static str, label: impl Into<String>) -> LoadingStore {
    if let Some(existing) = REGISTRY.peek().get(id) {
        return LoadingStore {
            id,
            snapshot: *existing,
        };
    }
    let snapshot = *REGISTRY
        .write()
        .entry(id)
        .or_insert_with(|| Signal::new(LoadSnapshot::idle(label.into())));
    LoadingStore { id, snapshot }
}


/// Determinate-or-indeterminate loading bar with label and phase detail.
///
/// `fraction: Some(_)` renders a filled bar plus the percentage text (the
/// percentage is mandatory — never strip it); `None` renders the animated
/// indeterminate fill and no number. Styling: `.pk-progress*` in
/// [`crate::CSS`], themed by the `:root` variables.
#[component]
pub fn ProgressBar(
    /// Completion in `0.0..=1.0`, or `None` for indeterminate.
    fraction: Option<f64>,
    /// What is loading, e.g. `"loading branches…"`.
    label: String,
    /// Phase detail shown dimmed after the label.
    #[props(default)]
    detail: Option<String>,
    /// Compact single-row treatment for bars and headers.
    #[props(default)]
    compact: bool,
) -> Element {
    let class = if compact {
        "pk-progress compact"
    } else {
        "pk-progress"
    };
    let pct = fraction.map(|f| (f.clamp(0.0, 1.0) * 100.0).round() as u32);
    // Fill width and the mandatory percentage text come from the same rounded
    // integer so the two can never disagree.
    let width = pct.map(|p| p.to_string()).unwrap_or_default();
    rsx! {
        div {
            class: "{class}",
            role: "progressbar",
            aria_valuemin: "0",
            aria_valuemax: "100",
            aria_valuenow: pct.map(|p| p.to_string()),
            aria_label: "{label}",
            div { class: "pk-progress-head",
                span { class: "pk-progress-label", "{label}" }
                if let Some(detail) = detail {
                    span { class: "pk-progress-detail", "{detail}" }
                }
                if let Some(pct) = pct {
                    span { class: "pk-progress-pct", "{pct}%" }
                }
            }
            div { class: "pk-progress-track",
                if fraction.is_some() {
                    div {
                        class: "pk-progress-fill",
                        style: "width: {width}%",
                    }
                } else {
                    div { class: "pk-progress-fill indeterminate" }
                }
            }
        }
    }
}


/// Panel-level hydration gate: renders a loading surface until `store`
/// reaches [`LoadStatus::Ready`], then `children`.
///
/// Pending renders [`ProgressBar`] (bar, never a bare spinner); `Failed`
/// renders the error with `role="alert"`; `Idle` renders the label without
/// animation (nothing requested yet — the common pre-first-fetch frame).
/// This component is the codified replacement for hand-rolled
/// "gate the empty state behind the first completed fetch" blocks.
#[component]
pub fn LoadingGate(
    /// The data source's store.
    store: LoadingStore,
    /// Content shown once the store is ready.
    children: Element,
) -> Element {
    let snap = store.snapshot();
    match snap.status {
        LoadStatus::Ready => children,
        LoadStatus::Failed => rsx! {
            div { class: "pk-gate pk-gate-error", role: "alert",
                span { class: "pk-gate-label", "{snap.label}" }
                span { class: "pk-gate-detail",
                    {snap.error.unwrap_or_else(|| "load failed".to_string())}
                }
            }
        },
        LoadStatus::Idle => rsx! {
            div { class: "pk-gate", role: "status",
                span { class: "pk-gate-label", "{snap.label}" }
            }
        },
        LoadStatus::Pending { fraction } => rsx! {
            div { class: "pk-gate", role: "status",
                ProgressBar { fraction, label: snap.label, detail: snap.detail }
            }
        },
    }
}

/// Workspace-level loading surface: one compact bar aggregating every
/// pending store in the registry.
///
/// Hidden while nothing is pending. Otherwise shows the pending count, the
/// first pending store's label, and — when at least one pending store
/// reports a fraction — the mean completion as a mandatory percentage (see
/// [`aggregate_pending`]). Mount once, typically in the app's top bar.
#[component]
pub fn GlobalLoadingBar() -> Element {
    let snapshots: Vec<LoadSnapshot> = REGISTRY
        .read()
        .values()
        .map(|signal| signal.read().clone())
        .collect();

    let Some(agg) = aggregate_pending(snapshots.iter()) else {
        return rsx! {};
    };
    let label = match (agg.pending, agg.label) {
        (1, Some(label)) => label,
        (n, Some(label)) => format!("{label} +{} more", n - 1),
        (_, None) => "loading…".to_string(),
    };
    rsx! {
        div { class: "pk-global-load",
            ProgressBar {
                fraction: agg.fraction,
                label,
                compact: true,
            }
        }
    }
}
#[cfg(test)]
mod tests {
    //! Behavior tests for the store and markup contract tests for the
    //! components. Store tests drive a real `VirtualDom` (signals need a
    //! runtime); component tests render through `dioxus_ssr` so assertions
    //! run against the shipped markup, not against source text.
    use super::*;
    use crate::CSS;

    // --- store behavior ------------------------------------------------------
    // `VirtualDom::new` takes a fn pointer (no captures), so the probe and
    // its sink live in thread-locals. Serialized by LOCK: the registry and
    // global signals are process-wide, so parallel doms would interleave.
    thread_local! {
        static OUT: std::cell::RefCell<Vec<LoadSnapshot>> = const { std::cell::RefCell::new(Vec::new()) };
        static PROBE: std::cell::RefCell<Option<Box<dyn Fn(&mut Vec<LoadSnapshot>)>>> =
            const { std::cell::RefCell::new(None) };
    }

    fn root() -> Element {
        PROBE.with(|p| {
            if let Some(probe) = &*p.borrow() {
                OUT.with(|o| probe(&mut o.borrow_mut()));
            }
        });
        rsx! {}
    }

    fn drive(probe: impl Fn(&mut Vec<LoadSnapshot>) + 'static) -> Vec<LoadSnapshot> {
        use std::sync::LazyLock;
        static LOCK: LazyLock<parking_lot::Mutex<()>> =
            LazyLock::new(|| parking_lot::Mutex::new(()));
        let _guard = LOCK.lock();
        OUT.with(|o| o.borrow_mut().clear());
        PROBE.with(|p| *p.borrow_mut() = Some(Box::new(probe)));
        let mut dom = VirtualDom::new(root);
        dom.rebuild(&mut dioxus_core::NoOpMutations);
        drop(dom);
        PROBE.with(|p| p.borrow_mut().take());
        OUT.with(|o| o.borrow().clone())
    }


    #[test]
    fn store_actions_drive_the_snapshot_lifecycle() {
        let out = drive(|out| {
            let store = loading_store("life", "loading life…");
            store.begin_with("stage one");
            out.push(store.snapshot());
            store.update(Some(0.5), None);
            out.push(store.snapshot());
            store.succeed();
            out.push(store.snapshot());
        });
        assert!(matches!(
            out[0].status,
            LoadStatus::Pending { fraction: None }
        ));
        assert_eq!(out[0].detail.as_deref(), Some("stage one"));
        assert_eq!(out[1].fraction(), Some(0.5));
        assert!(out[2].is_ready());
        assert_eq!(out[2].detail, None);
    }

    #[test]
    fn update_clamps_out_of_range_fractions() {
        let out = drive(|out| {
            let store = loading_store("clamp", "loading clamp…");
            store.update(Some(1.7), None);
            out.push(store.snapshot());
        });
        assert_eq!(out[0].fraction(), Some(1.0));
    }

    #[test]
    fn begin_clears_a_previous_failure() {
        let out = drive(|out| {
            let store = loading_store("retry", "loading retry…");
            store.fail("boom");
            out.push(store.snapshot());
            store.begin();
            out.push(store.snapshot());
        });
        assert!(matches!(out[0].status, LoadStatus::Failed));
        assert_eq!(out[0].error.as_deref(), Some("boom"));
        assert!(out[1].is_pending());
        assert_eq!(out[1].error, None);
    }

    #[test]
    fn same_id_shares_one_store_across_components() {
        let out = drive(|out| {
            let writer = loading_store("shared", "loading shared…");
            let reader = loading_store("shared", "loading shared…");
            writer.update(Some(0.25), None);
            out.push(reader.snapshot());
        });
        assert_eq!(out[0].fraction(), Some(0.25));
    }

    #[test]
    fn registry_aggregates_pending_stores_for_the_global_bar() {
        let out = drive(|out| {
            let a = loading_store("agg-a", "loading a…");
            let b = loading_store("agg-b", "loading b…");
            let c = loading_store("agg-c", "loading c…");
            a.update(Some(0.2), None);
            b.update(Some(0.8), None);
            c.succeed();
            // Filter to this test's ids: the registry is process-wide and
            // earlier tests' stores stay registered by design (stores are
            // app-lifetime, pinia-style).
            let snapshots: Vec<LoadSnapshot> = REGISTRY
                .read()
                .iter()
                .filter(|(id, _)| id.starts_with("agg-"))
                .map(|(_, signal)| signal.read().clone())
                .collect();
            let agg = aggregate_pending(snapshots.iter()).expect("two pending");
            out.push(LoadSnapshot {
                status: LoadStatus::Pending {
                    fraction: agg.fraction,
                },
                label: agg.pending.to_string(),
                detail: agg.label,
                error: None,
            });
        });
        // agg-a/agg-b pending at 0.2/0.8 → mean 0.5; agg-c ready, excluded.
        assert_eq!(out[0].label, "2");
        assert_eq!(out[0].fraction(), Some(0.5));
        assert_eq!(out[0].detail.as_deref(), Some("loading a…"));
    }

    // --- component markup contract (via dioxus_ssr) ---------------------------

    #[test]
    fn determinate_bar_shows_percentage_and_aria_value() {
        let html = dioxus_ssr::render_element(rsx! {
            ProgressBar { fraction: Some(0.42), label: "loading graph…" }
        });
        assert!(html.contains("42%"), "missing percentage text: {html}");
        assert!(html.contains("role=\"progressbar\""), "{html}");
        assert!(html.contains("aria-valuenow=\"42\""), "{html}");
        assert!(html.contains("width: 42%"), "missing fill width: {html}");
        assert!(!html.contains("indeterminate"), "{html}");
    }

    #[test]
    fn indeterminate_bar_animates_without_a_fabricated_number() {
        let html = dioxus_ssr::render_element(rsx! {
            ProgressBar { fraction: None, label: "loading graph…" }
        });
        assert!(html.contains("indeterminate"), "{html}");
        assert!(!html.contains("pk-progress-pct"), "{html}");
        assert!(!html.contains("aria-valuenow"), "{html}");
    }

    /// Fixed-state store built inside a probe's render scope (signals need a
    /// runtime; constructing one before `render_element` would panic). Never
    /// registered, so fixed states cannot leak into the global bar.
    fn fixed_store(status: LoadStatus, error: Option<&str>) -> LoadingStore {
        let mut snap = LoadSnapshot::idle("loading test…");
        snap.status = status;
        snap.error = error.map(str::to_string);
        LoadingStore {
            id: "test-fixed",
            snapshot: Signal::new(snap),
        }
    }

    #[component]
    fn GateProbe(case: String) -> Element {
        let store = match case.as_str() {
            "ready" => fixed_store(LoadStatus::Ready, None),
            "failed" => fixed_store(LoadStatus::Failed, Some("kaboom")),
            _ => fixed_store(LoadStatus::Pending { fraction: Some(0.5) }, None),
        };
        rsx! {
            LoadingGate { store, div { "loaded content" } }
        }
    }

    #[test]
    fn gate_renders_children_only_when_ready() {
        let ready = dioxus_ssr::render_element(rsx! {
            GateProbe { case: "ready" }
        });
        assert!(ready.contains("loaded content"), "{ready}");

        let pending = dioxus_ssr::render_element(rsx! {
            GateProbe { case: "pending" }
        });
        assert!(pending.contains("pk-progress"), "{pending}");
        assert!(pending.contains("50%"), "{pending}");
        assert!(!pending.contains("loaded content"), "{pending}");

        let failed = dioxus_ssr::render_element(rsx! {
            GateProbe { case: "failed" }
        });
        assert!(failed.contains("role=\"alert\""), "{failed}");
        assert!(failed.contains("kaboom"), "{failed}");
    }

    // --- stylesheet contract ---------------------------------------------------

    /// Every class the loading components emit must exist in the shipped
    /// stylesheet — the same drift guard the boot contract tests enforce.
    #[test]
    fn loading_classes_exist_in_base_css() {
        const CLASSES: [&str; 12] = [
            "pk-progress",
            "pk-progress-head",
            "pk-progress-label",
            "pk-progress-detail",
            "pk-progress-pct",
            "pk-progress-track",
            "pk-progress-fill",
            "pk-gate",
            "pk-gate-error",
            "pk-gate-label",
            "pk-gate-detail",
            "pk-global-load",
        ];
        for class in CLASSES {
            assert!(CSS.contains(&format!(".{class}")), "CSS missing .{class}");
        }
    }
}
