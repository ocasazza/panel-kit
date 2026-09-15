//! Dioxus backend for panel-kit's composable workspace parts.
//!
//! Factored out of apple-notes-ocr-flow's reviewer UI so applications can reuse
//! the same panel visual language without handing control flow to a controller.
//! Hosts own state, event priority, persistence timing, and render order; this
//! crate translates browser input and paints native Dioxus DOM over the
//! renderer-neutral contracts in `panel-kit-core`.
//!
//! The crate also ships standalone widgets: the [`badge`] module (a clickable
//! metadata chip), [`Spinner`], the [`editor`] module (a Monaco code editor
//! with a `.pest` grammar language), the [`loading`] module (store-shaped async
//! hydration: [`LoadingGate`], [`ProgressBar`], [`GlobalLoadingBar`]), and a
//! [`LoadingWorkspace`] whose static HTML/CSS twin can paint before an app's
//! WASM bundle finishes loading.
//!
//! [`LoadingGate`]: loading::LoadingGate
//! [`ProgressBar`]: loading::ProgressBar
//! [`GlobalLoadingBar`]: loading::GlobalLoadingBar
//!
//! The app supplies a panel identity, host-owned [`Snapshot`] state, a reusable
//! [`ProjectionBuffer`], and panel body elements. Core functions such as
//! [`reduce`], [`project_into`], and [`project_panel`] stay free functions; web
//! chrome is assembled explicitly from [`widgets::panel`] and [`widgets::dock`].
//! For several named, switchable layouts, own a
//! [`panel_kit_core::views::SavedViews`] registry in the host and pair each
//! active view with its own layout store.
//!
//! [`ProjectionBuffer`]: panel_kit_core::frame::ProjectionBuffer
//! [`Snapshot`]: panel_kit_core::reducer::Snapshot
//! [`project_into`]: panel_kit_core::frame::project_into
//! [`project_panel`]: panel_kit_core::frame::project_panel
//! [`reduce`]: panel_kit_core::reducer::reduce
//!
//! This is a wasm-only crate (Dioxus web): it builds for
//! `wasm32-unknown-unknown` and expects a browser environment at runtime.
//!
//! # Quick start
//!
//! ```no_run
//! use dioxus::prelude::*;
//! use panel_kit::{widgets, Clamp, LayoutBuilder, Mode, PanelKind, PanelWin};
//! use panel_kit_core::frame::{project_panel, ChromeProjectionInput, PanelProjectionInput};
//! use panel_kit_core::{ChromeMetrics, Region, SurfaceCapabilities, SurfaceProfile};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
//! enum Panel { Graph, Inspector }
//!
//! impl PanelKind for Panel {
//!     fn title(self) -> &'static str {
//!         match self {
//!             Panel::Graph => "Graph",
//!             Panel::Inspector => "Inspector",
//!         }
//!     }
//! }
//!
//! fn default_layout() -> Vec<PanelWin<Panel>> {
//!     let mut b = LayoutBuilder::new();
//!     vec![
//!         b.at(Panel::Graph, 16.0, 16.0, 520.0, 360.0),
//!         b.at(Panel::Inspector, 560.0, 16.0, 320.0, 360.0),
//!     ]
//! }
//!
//! fn graph_surface() -> Element {
//!     let panels = default_layout();
//!     let surface = SurfaceProfile::from_logical_width(
//!         900.0,
//!         panel_kit::WEB_COMPACT_MAX,
//!         panel_kit::WEB_TABLET_MAX,
//!         SurfaceCapabilities { coarse_pointer: false, hover: true, keyboard: true },
//!     );
//!     let chrome = ChromeProjectionInput::surface_only(ChromeMetrics::WEB);
//!     let projected = project_panel(&panels[0], 0, PanelProjectionInput {
//!         viewport: Region::new(0.0, 0.0, 900.0, 600.0),
//!         preferred_mode: Mode::Floating,
//!         surface,
//!         focused: true,
//!         pointer_dragging: false,
//!         tile_dragging: false,
//!         workspace_scroll: 0.0,
//!         clamp: &Clamp::WEB,
//!         chrome: &chrome,
//!         tile_grid: None,
//!         tiled_origin: None,
//!     }).expect("graph is visible");
//!
//!     widgets::panel::panel_surface(projected, Some("graph"), rsx! { "graph body" })
//! }
//! ```
//!
//! Old controller entry points are intentionally absent in the clean cutover:
//!
//! ```compile_fail,E0412
//! # enum Panel {}
//! fn takes_old_controller(_: panel_kit::Workspace < Panel >) {}
//! ```
//!
//! # Pre-WASM loading shell
//!
//! A Dioxus component cannot render until the app WASM has downloaded and
//! instantiated. Keep the Dioxus mount element empty, place [`BOOT_HTML`] as
//! its immediately following sibling, and load or inline [`BOOT_CSS`] in
//! `<head>`. The fragment is static HTML/CSS with no script or application
//! logic. It must not be placed inside the mount element because Dioxus does
//! not clear pre-existing children.
//!
//! After Rust starts, render [`LoadingWorkspace`] while app-owned data,
//! workers, or GPU resources continue initializing. It uses the same public
//! class contract as the static fragment.
//!
//! # Theming
//!
//! Inject [`CSS`] once at the app root (`style { {panel_kit::CSS} }`), then
//! layer app-specific styles after it. The generated `:root` custom properties
//! mirror [`panel_kit_core::theme::ThemeTokens`]: palette variables (`--bg`,
//! `--panel`, `--fg`, `--dim`, `--line`, `--line2`, `--inv-bg`, `--inv-fg`,
//! `--accent`, `--red`, `--yellow`, `--green`, `--blue`, `--pink`,
//! `--badge-info`, `--focus-ring`), typography (`--mono`, `--body-size`,
//! `--body-line-height`, `--label-size`, `--label-weight`,
//! `--label-tracking`), and density (`--panel-radius`, `--badge-radius`,
//! `--space-xs`, `--space-sm`, `--space-md`). Override them in a later
//! stylesheet to retheme panels, traffic lights, dock, badges, focus rings,
//! labels, and spinner.
//!
//! # Examples
//!
//! The repository ships one browser demo per component; run them with
//! `dx serve --example workspace --platform web` (dioxus-cli 0.6.x, provided
//! by `nix develop`):
//!
//! - `workspace` — the full workspace surface: floating/tiling, traffic
//!   lights, drag/resize/reorder, dock, persistence, compact stack, tooltips.
//! - `views` — host-owned named views over one workspace: core
//!   [`panel_kit_core::views::SavedViews`], per-view layout persistence keys,
//!   switching/creating/renaming/deleting views, and the legacy single-layout
//!   migration.
//! - `badge` — every [`badge::BadgeKind`], every prop, and an event log
//!   proving each [`badge::BadgeAction`] variant fires.
//! - `spinner` — [`Spinner`] with and without a label.
//! - `loading` — the full hydration arc: staged [`LoadingWorkspace`]
//!   percentage, per-panel [`loading::LoadingGate`]s behind stores, and the
//!   [`loading::GlobalLoadingBar`] aggregate.
//! - `theming` — the `:root` variable override path with switchable presets.
//! - `editor` — [`editor::MonacoEditor`]: two-way `Signal<String>` binding,
//!   `on_change` log, and the imperative [`editor::EditorHandle`] controls.
//!   Needs the vendored Monaco bundle served (see the example header).

#![warn(missing_docs)]

#[cfg(feature = "web-runtime")]
pub mod badge;
#[cfg(feature = "web-runtime")]
pub mod editor;
#[cfg(feature = "web-runtime")]
pub mod input;
#[cfg(feature = "web-runtime")]
pub mod loading;
#[cfg(feature = "spec-plan")]
pub mod spec_plan;
#[cfg(feature = "web-runtime")]
pub mod store;
#[cfg(feature = "web-runtime")]
pub mod surface;
#[cfg(feature = "web-runtime")]
pub mod theme;
#[cfg(feature = "web-runtime")]
pub mod widgets;


#[cfg(feature = "web-runtime")]
use dioxus::events::{KeyboardEvent, PointerEvent as DioxusPointerEvent};
#[cfg(feature = "web-runtime")]
use dioxus::prelude::*;

pub use panel_kit_core::{
    apply_command, command_for, effective_mode, migrate_v1, reconcile_units, Clamp, CommandStep,
    Drag, DragKind, FocusContext, Key, KeyChord, LayoutBuilder, Mode, PanelCommand, PanelKind,
    PanelWin, PointerButton, PointerEvent, PointerEventKind, SavedLayout, SavedLayoutV2,
    StoredLayout, SurfaceCapabilities, SurfaceClass, SurfaceProfile, Units, WinState,
    CELLS_COMPACT_MAX, CELLS_TABLET_MAX, LAYOUT_SCHEMA_VERSION, TILE_ROW_PX, TILE_W_MAX,
    WEB_COMPACT_MAX, WEB_TABLET_MAX,
};

/// Critical stylesheet for the loading-workspace contract.
///
/// Trunk apps cannot call Rust before their WASM bundle has instantiated, so
/// keep the Dioxus mount element empty, place [`BOOT_HTML`] as its immediately
/// following sibling, and inline this CSS in the document head (or copy the
/// asset into the build). Once Dioxus marks the mount element, the adjacent
/// sibling selector hides only that static fragment. After WASM is live,
/// [`LoadingWorkspace`] renders the same app-agnostic contract.
/// Generated at build time from [`panel_kit_core::tokens`] (see `build.rs`
/// and `assets/panel-kit-boot.css.in`), so its palette and font stack cannot
/// drift from the injected [`CSS`] the way a hand-maintained copy did.
#[cfg(feature = "web-runtime")]
pub const BOOT_CSS: &str = include_str!(concat!(env!("OUT_DIR"), "/panel-kit-boot.css"));

/// Static, JavaScript-free loading-workspace fragment for pre-WASM first paint.
///
/// Place this fragment immediately after, never inside, the empty Dioxus mount
/// element. Consumers should replace the generic application title and status
/// text. See [`BOOT_CSS`] for wiring details.
#[cfg(feature = "web-runtime")]
pub const BOOT_HTML: &str = include_str!("../assets/panel-kit-boot.html");

/// Base stylesheet for the workspace chrome (panels, lights, dock, badges,
/// spinner, tooltip overlay, and surface tiers). Inject once at the app root
/// with `style { {panel_kit::CSS} }`, then layer app-specific styles after
/// it; override the `:root` CSS variables to retheme (see the
/// [crate-level theming notes](crate#theming)).
#[cfg(feature = "web-runtime")]
pub const CSS: &str = include_str!("../assets/panel-kit.css");
/// Panel-shaped loading state for work that continues after WASM has mounted.
///
/// For the earlier download/instantiation gap, render the static [`BOOT_HTML`]
/// contract in the app's HTML and inline [`BOOT_CSS`]. Both surfaces use the
/// same class names and visual language; the app owns only the phase text.
///
/// Pass `progress` once the app can measure a phase (download bytes, staged
/// init steps): the header bar turns determinate and shows the percentage —
/// the bar, never a spinner, and never a fabricated number. `None` keeps the
/// honest indeterminate animation the static fragment painted.
#[cfg(feature = "web-runtime")]
#[component]
pub fn LoadingWorkspace(
    /// Application name shown in the compact top bar.
    title: String,
    /// Current app-owned phase, such as `loading graph…` or `initializing GPU…`.
    status: String,
    /// Completion in `0.0..=1.0`, or `None` while indeterminate.
    #[props(default)]
    progress: Option<f64>,
) -> Element {
    let pct = progress.map(|f| (f.clamp(0.0, 1.0) * 100.0).round() as u32);
    // Fill width and the percentage text come from the same rounded integer.
    let width = pct.map(|p| p.to_string()).unwrap_or_default();
    rsx! {
        style { {BOOT_CSS} }
        section {
            class: "panel-kit-boot",
            role: "status",
            aria_live: "polite",
            header { class: "panel-kit-boot-bar",
                strong { class: "panel-kit-boot-title", "{title}" }
                span { class: "panel-kit-boot-status", "{status}" }
                div {
                    class: "panel-kit-boot-progress",
                    role: "progressbar",
                    aria_valuemin: "0",
                    aria_valuemax: "100",
                    aria_valuenow: pct.map(|p| p.to_string()),
                    aria_label: "load progress",
                    if pct.is_some() {
                        div {
                            class: "panel-kit-boot-fill",
                            style: "width: {width}%",
                        }
                    } else {
                        div { class: "panel-kit-boot-fill indeterminate" }
                    }
                }
                if let Some(pct) = pct {
                    span { class: "panel-kit-boot-pct", "{pct}%" }
                }
            }
            main { class: "panel-kit-boot-panels", aria_hidden: "true",
                for i in 0..3 {
                    div { key: "{i}", class: "panel-kit-boot-panel",
                        div { class: "panel-kit-boot-line" }
                        div { class: "panel-kit-boot-line" }
                    }
                }
            }
        }
    }
}

/// Use this in the `header_actions` slot passed to
/// [`widgets::panel::panel_chrome`] or
/// [`widgets::panel::panel_chrome_with_events`]. Pointer-down is stopped at the
/// button so clicking an action never begins a panel move or tile reorder.
#[cfg(feature = "web-runtime")]
#[component]
pub fn PanelHeaderButton(
    /// Short visible label. Header space is intentionally tight, so prefer a
    /// compact word or glyph and put the full description in `title`.
    label: String,
    /// Full hover and accessible label for the action.
    title: String,
    /// Whether to draw the selected/engaged treatment.
    #[props(default)]
    active: bool,
    /// Whether the action is currently unavailable.
    #[props(default)]
    disabled: bool,
    /// Application-owned action handler.
    on_press: EventHandler<MouseEvent>,
) -> Element {
    let class = if active {
        "panel-head-action active"
    } else {
        "panel-head-action"
    };
    rsx! {
        button {
            class: "{class}",
            r#type: "button",
            title: "{title}",
            aria_label: "{title}",
            disabled,
            onpointerdown: move |e: DioxusPointerEvent| e.stop_propagation(),
            onkeydown: move |e: KeyboardEvent| e.stop_propagation(),
            onclick: move |e| on_press.call(e),
            "{label}"
        }
    }
}

// The renderer-neutral state machine lives in panel-kit-core and is re-exported
// above. This crate stays the Dioxus adapter: browser input translation, CSS,
// localStorage transport, and native DOM painters.


/// True while an `<input>` or `<textarea>` has focus.
///
/// Re-exported at the crate root for applications that decide whether the
/// focused browser control gets first refusal before translating a key through
/// [`input::keyboard_event`].
#[cfg(feature = "web-runtime")]
pub fn is_editing() -> bool {
    input::is_editing()
}


/// Reusable spinner — a small rotating ring with an optional label.
///
/// With the default empty `label` only the ring renders; a non-empty label
/// renders next to it. Styling comes from the `.spinner` rules in [`CSS`].
///
/// # Examples
///
/// ```no_run
/// use dioxus::prelude::*;
/// use panel_kit::Spinner;
///
/// # fn busy() -> Element {
/// rsx! {
///     Spinner {}                          // ring only
///     Spinner { label: "indexing…" }      // ring + label
/// }
/// # }
/// ```
#[cfg(feature = "web-runtime")]
#[component]
pub fn Spinner(
    /// Text shown after the ring; the label span is omitted entirely when
    /// empty (the default).
    #[props(default = String::new())]
    label: String,
) -> Element {
    let model = panel_kit_core::widgets::spinner::SpinnerModel {
        label: (!label.is_empty()).then_some(label),
    };

    widgets::spinner::from_model(&model, 0)
}

/// Viewport-aware tooltip placement: prefer left of the cursor, flip right if
/// there's no room, and clamp inside the window. (CSS anchor-positioning would
/// do this natively but WebKit doesn't support it yet.)
///
/// `(cx, cy)` is the cursor position and `(tw, th)` the tooltip size, all in
/// px / client coordinates; the returned `(x, y)` is the tooltip's top-left,
/// ready for `position: fixed; left:{x}px; top:{y}px`.
#[cfg(feature = "web-runtime")]
pub fn tip_pos(cx: f64, cy: f64, tw: f64, th: f64) -> (f64, f64) {
    let win = web_sys::window();
    let vw = win
        .as_ref()
        .and_then(|w| w.inner_width().ok())
        .and_then(|v| v.as_f64())
        .unwrap_or(1280.0);
    let vh = win
        .and_then(|w| w.inner_height().ok())
        .and_then(|v| v.as_f64())
        .unwrap_or(800.0);
    let mut x = cx - tw - 14.0;
    if x < 8.0 {
        x = cx + 14.0;
    }
    if x + tw > vw - 8.0 {
        x = vw - tw - 8.0;
    }
    let mut y = cy - 12.0;
    if y + th > vh - 8.0 {
        y = vh - th - 8.0;
    }
    (x.max(8.0), y.max(8.0))
}

#[cfg(all(test, feature = "web-runtime"))]
mod theme_parity_tests;

#[cfg(all(test, feature = "web-runtime"))]
mod boot_contract_tests {
    use super::{LoadingWorkspace, BOOT_CSS, BOOT_HTML};
    use dioxus::prelude::*;

    const CLASSES: [&str; 10] = [
        "panel-kit-boot",
        "panel-kit-boot-bar",
        "panel-kit-boot-title",
        "panel-kit-boot-status",
        "panel-kit-boot-panels",
        "panel-kit-boot-panel",
        "panel-kit-boot-line",
        "panel-kit-boot-progress",
        "panel-kit-boot-fill",
        "panel-kit-boot-pct",
    ];

    #[test]
    fn static_html_and_critical_css_share_the_public_class_contract() {
        for class in CLASSES {
            assert!(BOOT_HTML.contains(class), "BOOT_HTML is missing {class}");
            assert!(
                BOOT_CSS.contains(&format!(".{class}")),
                "BOOT_CSS is missing {class}"
            );
        }
    }

    /// The injected stylesheet's theme region is generated from the same
    /// core emitter that `build.rs` uses for committed assets. The test
    /// asserts the generated region, not hand-pinned token literals.
    #[test]
    fn injected_stylesheet_declares_every_token_from_the_theme_emitter() {
        use panel_kit_core::theme::{ThemeColor, ThemeTokens};

        let generated = super::theme::css_root_block(&ThemeTokens::dark());
        assert!(super::CSS.contains(super::theme::CSS_THEME_REGION_BEGIN));
        assert!(super::CSS.contains(&generated));
        assert!(super::CSS.contains(super::theme::CSS_THEME_REGION_END));

        for slot in ThemeColor::ALL.iter().copied() {
            let needle = format!(
                "--{}: {};",
                slot.key(),
                String::from(ThemeTokens::dark().colors.resolve(slot))
            );
            assert!(
                generated.contains(&needle),
                "generated CSS root block is missing {needle}"
            );
        }
    }

    /// The generated boot sheet must carry the token values literally — it
    /// cannot reference `var(--…)`, because it paints before the injected
    /// stylesheet exists.
    #[test]
    fn generated_boot_stylesheet_carries_token_values_not_variables() {
        use panel_kit_core::tokens;

        for token in [tokens::BG, tokens::PANEL, tokens::FG, tokens::LINE2] {
            assert!(
                BOOT_CSS.contains(token.hex),
                "generated BOOT_CSS is missing {} ({})",
                token.name,
                token.hex
            );
        }
        assert!(
            BOOT_CSS.contains(tokens::MONO),
            "generated BOOT_CSS does not use the canonical monospace stack"
        );
        assert!(
            !BOOT_CSS.contains("{{"),
            "generated BOOT_CSS still contains an unsubstituted placeholder"
        );
    }

    #[test]
    fn static_boot_bar_is_a_script_free_indeterminate_progressbar() {
        // Pre-WASM there is no script to measure download progress, so the
        // static fragment renders the honest indeterminate bar; the Rust
        // LoadingWorkspace takes over with a real percentage once mounted.
        assert!(BOOT_HTML.contains("role=\"progressbar\""));
        assert!(!BOOT_HTML.contains("aria-valuenow"));
        assert!(!BOOT_HTML.contains("%</span>"));
    }

    #[test]
    fn loading_workspace_progress_markup_matches_the_static_contract() {
        let determinate = dioxus_ssr::render_element(rsx! {
            LoadingWorkspace {
                title: "APP".to_string(),
                status: "loading graph…".to_string(),
                progress: Some(0.6),
            }
        });
        assert!(determinate.contains("60%"), "{determinate}");
        assert!(
            determinate.contains("aria-valuenow=\"60\""),
            "{determinate}"
        );
        assert!(determinate.contains("width: 60%"), "{determinate}");
        // Class-attribute shape ("fill indeterminate", space-separated) — the
        // embedded BOOT_CSS text also contains the word "indeterminate".
        assert!(
            !determinate.contains("panel-kit-boot-fill indeterminate"),
            "{determinate}"
        );

        let indeterminate = dioxus_ssr::render_element(rsx! {
            LoadingWorkspace {
                title: "APP".to_string(),
                status: "loading graph…".to_string(),
                progress: None,
            }
        });
        assert!(
            indeterminate.contains("panel-kit-boot-fill indeterminate"),
            "{indeterminate}"
        );
        assert!(!indeterminate.contains("%</span>"), "{indeterminate}");
    }

    #[test]
    fn static_boot_contract_is_script_free_and_marked_for_handoff() {
        let html = BOOT_HTML.to_ascii_lowercase();
        assert!(!html.contains("<script"));
        assert!(!html.contains("onclick="));
        assert!(!html.contains("onload="));
        assert!(html.contains("data-panel-kit-static-boot"));
        assert!(html.contains("role=\"status\""));
        assert!(html.contains("immediately after an empty dioxus mount element"));
        assert!(html.contains("never place it inside the mount element"));

        const HANDOFF_SELECTOR: &str =
            "[data-dioxus-id] + .panel-kit-boot[data-panel-kit-static-boot]";
        assert!(BOOT_CSS.contains(HANDOFF_SELECTOR));
        assert!(!BOOT_CSS.contains("[data-dioxus-id] > .panel-kit-boot"));
    }
}
