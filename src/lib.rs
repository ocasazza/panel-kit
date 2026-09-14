//! Generic Dioxus panel-workspace library.
//!
//! Factored out of apple-notes-ocr-flow's reviewer UI so any app can get the
//! same shell: every view is a panel you can move/resize/minimize/maximize,
//! with floating (free placement) and tiling (auto grid) workspace modes,
//! macOS-style traffic lights, a minimized-panel dock strip, and layout
//! persistence to localStorage. The crate also ships standalone widgets:
//! the [`badge`] module (a clickable metadata chip), [`Spinner`], the
//! [`editor`] module (a Monaco code editor with a `.pest` grammar language),
//! the [`loading`] module (store-shaped async hydration: [`LoadingGate`],
//! [`ProgressBar`], [`GlobalLoadingBar`]), and a [`LoadingWorkspace`] whose
//! static HTML/CSS twin can paint before an app's WASM bundle finishes
//! loading.
//!
//! [`LoadingGate`]: loading::LoadingGate
//! [`ProgressBar`]: loading::ProgressBar
//! [`GlobalLoadingBar`]: loading::GlobalLoadingBar
//!
//! The app supplies two things: a [`PanelKind`] impl (an enum of its panels)
//! and a body-render callback. Everything else — geometry, z-order, drag
//! state, viewport classification, persistence — lives in the [`Workspace`]
//! handle created by [`use_workspace`]. For several named, switchable layouts
//! inside one workspace, [`use_views`] layers a view registry and per-view
//! persistence keys over the same machinery.
//! Panel chrome follows the ratatui renderer's compact treatment: controls
//! and title are inset into the top border row instead of occupying a
//! separate full-width header band, preserving vertical space for content.
//!
//! This is a wasm-only crate (Dioxus web): it builds for
//! `wasm32-unknown-unknown` and expects a browser environment at runtime.
//!
//! # Quick start
//!
//! ```no_run
//! use dioxus::prelude::*;
//! use panel_kit::{use_workspace, LayoutBuilder, PanelHeaderButton, PanelKind, PanelWin};
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
//! #[component]
//! fn App() -> Element {
//!     let ws = use_workspace("myapp_layout", default_layout);
//!     rsx! {
//!         style { {panel_kit::CSS} }
//!         div { class: ws.root_class(), tabindex: "0",
//!             onpointermove: move |e| ws.handle_pointer_move(&e),
//!             onpointerup: move |e| ws.handle_pointer_up(&e),
//!             onpointercancel: move |e| ws.handle_pointer_up(&e),
//!             onkeydown: move |e| ws.handle_key(&e),
//!             header { class: "topbar" /* app-specific */ }
//!             {ws.render_with_header(
//!                 |kind, _maximized| rsx! { "body for {kind.title()}" },
//!                 |_kind, _maximized| rsx! {
//!                     PanelHeaderButton {
//!                         label: "refresh",
//!                         title: "Refresh this panel",
//!                         on_press: move |_| { /* app-owned action */ },
//!                     }
//!                 },
//!             )}
//!             {ws.dock()}
//!         }
//!     }
//! }
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
//! layer app-specific styles after it. All chrome colours and the monospace
//! font come from `:root` CSS variables (`--bg`, `--panel`, `--fg`, `--dim`,
//! `--line`, `--line2`, `--inv-bg`, `--inv-fg`, `--accent`, `--red`,
//! `--yellow`, `--green`, `--blue`, `--pink`, `--mono`) — override them in a later stylesheet to
//! retheme everything: panels, traffic lights, dock, badges, and spinner.
//!
//! # Examples
//!
//! The repository ships one browser demo per component; run them with
//! `dx serve --example workspace --platform web` (dioxus-cli 0.6.x, provided
//! by `nix develop`):
//!
//! - `workspace` — the full workspace surface: floating/tiling, traffic
//!   lights, drag/resize/reorder, dock, persistence, compact stack, tooltips.
//! - `views` — named views over one workspace: [`use_views`], per-view
//!   layout persistence keys, switching/creating/renaming/deleting views,
//!   and the legacy single-layout migration.
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

pub mod badge;
pub mod editor;
pub mod loading;
pub mod views;

pub use views::{use_views, SavedViews, ViewError, Views};

use dioxus::events::{Key as DioxusKey, KeyboardEvent, PointerEvent as DioxusPointerEvent};
use dioxus::prelude::*;
use gloo_storage::{LocalStorage, Storage};
use wasm_bindgen::JsCast;

pub use panel_kit_core::{
    apply_command, command_for, effective_mode, migrate_v1, reconcile_units, Clamp, CommandStep,
    Drag, DragKind, FocusContext, Key, KeyChord, LayoutBuilder, Mode, PanelCommand, PanelKind,
    PanelWin, PointerButton, PointerEvent, PointerEventKind, SavedLayout, SavedLayoutV2,
    StoredLayout, SurfaceCapabilities, SurfaceClass, SurfaceProfile, Units, WinState,
    CELLS_COMPACT_MAX, CELLS_TABLET_MAX, LAYOUT_SCHEMA_VERSION, TILE_ROW_PX, TILE_W_MAX,
    WEB_COMPACT_MAX, WEB_TABLET_MAX,
};
use panel_kit_core::{
    apply_drag, begin_drag as core_begin_drag, begin_tile_resize as core_begin_tile_resize,
    clamp_scroll, effective_rect as core_effective_rect, floating_content_height, kind_slug,
    max_scroll, merge_defaults, reorder_tile as core_reorder_tile, TileMetrics,
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
pub const BOOT_CSS: &str = include_str!(concat!(env!("OUT_DIR"), "/panel-kit-boot.css"));

/// Static, JavaScript-free loading-workspace fragment for pre-WASM first paint.
///
/// Place this fragment immediately after, never inside, the empty Dioxus mount
/// element. Consumers should replace the generic application title and status
/// text. See [`BOOT_CSS`] for wiring details.
pub const BOOT_HTML: &str = include_str!("../assets/panel-kit-boot.html");

/// Base stylesheet for the workspace chrome (panels, lights, dock, badges,
/// spinner, tooltip overlay, and surface tiers). Inject once at the app root
/// with `style { {panel_kit::CSS} }`, then layer app-specific styles after
/// it; override the `:root` CSS variables to retheme (see the
/// [crate-level theming notes](crate#theming)).
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

/// Compact application action rendered inside a panel's header bar.
///
/// Use this from the header callback passed to
/// [`Workspace::render_with_header`]. Pointer-down is stopped at the button
/// so clicking an action never begins a panel move or tile reorder.
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

// The core types (PanelKind, PanelWin, WinState, Mode, Drag, LayoutBuilder)
// and all geometry/drag math live in panel-kit-core and are re-exported
// above — this crate is the Dioxus shell: signals, DOM events, CSS,
// localStorage persistence, and rendering.

fn viewport_size() -> (f64, f64) {
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
    (vw, vh)
}

fn browser_capabilities() -> SurfaceCapabilities {
    let media_matches = |query: &str| {
        web_sys::window()
            .and_then(|window| window.match_media(query).ok().flatten())
            .map(|media| media.matches())
            .unwrap_or(false)
    };
    SurfaceCapabilities {
        coarse_pointer: media_matches("(pointer: coarse)"),
        hover: media_matches("(hover: hover)"),
        keyboard: true,
    }
}

fn surface_profile(width: f64) -> SurfaceProfile {
    SurfaceProfile::from_logical_width(
        width,
        WEB_COMPACT_MAX,
        WEB_TABLET_MAX,
        browser_capabilities(),
    )
}

fn core_pointer_event(event: &DioxusPointerEvent, kind: PointerEventKind) -> PointerEvent {
    let coordinates = event.client_coordinates();
    PointerEvent {
        kind,
        x: coordinates.x,
        y: coordinates.y,
    }
}

fn dock_chip_id<K: PanelKind>(kind: K) -> String {
    format!("panel-kit-dock-{}", kind_slug(kind.title()))
}

/// Floating placement for a scrollable workspace: x / width / height stay
/// clamped to the viewport (so panels never grow wider than the window), but
/// the *vertical* position keeps the panel's stored `y` (only floored at 0)
/// instead of being pulled up to the bottom edge. Panels placed low can then
/// extend below the fold and be reached with workspace-level vertical scroll
/// ([`Workspace::ws_scroll`]); per-panel content scroll and drag math are
/// untouched.
fn floating_rect<K>(p: &PanelWin<K>, vw: f64, vh: f64) -> (f64, f64, f64, f64) {
    let (x, _, w, h) = core_effective_rect(p, vw, vh, &Clamp::WEB);
    (x, p.y.max(0.0), w, h)
}

fn capture_pointer(e: &DioxusPointerEvent) {
    let Some(web_event) = e.data.downcast::<web_sys::PointerEvent>() else {
        return;
    };
    let Some(target) = web_event
        .target()
        .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
    else {
        return;
    };
    let _ = target.set_pointer_capture(web_event.pointer_id());
}

fn release_pointer(e: &DioxusPointerEvent) {
    let Some(web_event) = e.data.downcast::<web_sys::PointerEvent>() else {
        return;
    };
    let Some(target) = web_event
        .target()
        .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
    else {
        return;
    };
    let _ = target.release_pointer_capture(web_event.pointer_id());
}

fn clear_selection() {
    if let Some(selection) = web_sys::window().and_then(|w| w.get_selection().ok().flatten()) {
        let _ = selection.remove_all_ranges();
    }
}

/// True while an `<input>` or `<textarea>` has focus.
///
/// [`Workspace::handle_key`] uses this DOM adapter to select
/// [`FocusContext::TextInput`]; keybinding policy remains in
/// [`command_for`].
pub fn is_editing() -> bool {
    web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.active_element())
        .map(|el| {
            let tag = el.tag_name();
            tag.eq_ignore_ascii_case("input") || tag.eq_ignore_ascii_case("textarea")
        })
        .unwrap_or(false)
}

/// Whether a panel body under the pointer can still scroll in the wheel's
/// direction (`dy` is the vertical wheel delta) — i.e. whether per-panel
/// content scroll should absorb this wheel instead of the workspace.
///
/// Reads the hovered `.panel-body` from the DOM (`:hover`) and compares its
/// scroll position against its limits. Returns `false` when nothing scrollable
/// is under the pointer, so the workspace scroll takes over (manual scroll
/// chaining).
fn panel_body_absorbs_wheel(dy: f64) -> bool {
    use wasm_bindgen::JsCast;
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
        return false;
    };
    // The last match is the innermost hovered .panel-body.
    let Ok(hovered) = doc.query_selector_all(".panel-body:hover") else {
        return false;
    };
    let len = hovered.length();
    if len == 0 {
        return false;
    }
    let Some(node) = hovered.item(len - 1) else {
        return false;
    };
    let Ok(el) = node.dyn_into::<web_sys::Element>() else {
        return false;
    };
    let scroll_top = el.scroll_top();
    let scroll_h = el.scroll_height();
    let client_h = el.client_height();
    let max = scroll_h - client_h;
    if max <= 0 {
        return false; // body isn't scrollable at all
    }
    if dy > 0.0 {
        scroll_top < max // room to scroll down
    } else if dy < 0.0 {
        scroll_top > 0 // room to scroll up
    } else {
        false
    }
}

pub(crate) fn save_layout<K: PanelKind>(
    key: &str,
    panels: &[PanelWin<K>],
    mode: Mode,
    viewport: (f64, f64),
) {
    let _ = LocalStorage::set(
        key,
        SavedLayoutV2 {
            version: LAYOUT_SCHEMA_VERSION,
            units: Units::CssPx,
            viewport,
            mode,
            panels: panels.to_vec(),
        },
    );
}

/// Load either persisted schema, reconcile geometry into current CSS pixels,
/// and append panel kinds added since the record was written.
///
/// `pub(crate)` because [`use_views`] loads each view's layout through the
/// same reader, so a per-view record gets the same V1 migration and
/// unit reconciliation as the single-layout case.
pub(crate) fn load_layout<K: PanelKind>(
    key: &str,
    defaults: &[PanelWin<K>],
    viewport: (f64, f64),
) -> Option<(Vec<PanelWin<K>>, Mode)> {
    let stored: StoredLayout<K> = LocalStorage::get(key).ok()?;
    let layout = match stored {
        StoredLayout::V1(old) => migrate_v1(old, Units::CssPx, viewport),
        StoredLayout::V2(current) => reconcile_units(current, Units::CssPx, viewport),
    };
    let mode = layout.mode;
    let mut panels = layout.panels;
    merge_defaults(&mut panels, defaults);
    Some((panels, mode))
}

/// The workspace handle: a bundle of `Copy` signals, safe to pass around and
/// capture in event handlers. Create one per app root with [`use_workspace`].
///
/// The fields are public so apps can drive the workspace directly (e.g. a
/// keyboard shortcut that flips [`mode`](Workspace::mode), or a command
/// palette that minimizes a panel by mutating
/// [`panels`](Workspace::panels)) — every mutation re-renders and persists
/// automatically.
pub struct Workspace<K: PanelKind> {
    /// All panels with their geometry and window state. `Vec` order is the
    /// tiling order and persists with the layout.
    pub panels: Signal<Vec<PanelWin<K>>>,
    /// The user-chosen layout [`Mode`]. Prefer
    /// [`effective_mode`](Workspace::effective_mode) when rendering because
    /// compact surfaces force tiling.
    pub mode: Signal<Mode>,
    /// The in-flight floating-mode move/resize [`Drag`], if any.
    pub drag: Signal<Option<Drag>>,
    /// Tiling-mode reorder drag: the kind being dragged. Hovering another
    /// panel while set live-shuffles the dragged panel into that slot.
    pub tile_drag: Signal<Option<K>>,
    /// Current width tier and browser input capabilities, re-derived on
    /// every window resize.
    pub profile: Signal<SurfaceProfile>,
    /// Panel that owns window-management keyboard commands, if any.
    pub focused: Signal<Option<K>>,
    /// Live window size — [`render`](Workspace::render) subscribes so
    /// floating panels re-project through the viewport clamp on every
    /// resize (both directions).
    pub viewport: Signal<(f64, f64)>,
    /// Workspace-level vertical scroll offset in CSS px, used in floating
    /// mode when the panels' total height overhangs the workspace area.
    /// Driven by the wheel via [`handle_wheel`](Workspace::handle_wheel) and
    /// clamped to the content bounds at render time. (Tiling mode scrolls
    /// natively through the CSS `overflow` on `.ws.tiling`, so this only
    /// applies to floating mode.)
    pub ws_scroll: Signal<f64>,
    pending_dock_focus: Signal<Option<String>>,
}

impl<K: PanelKind> Clone for Workspace<K> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<K: PanelKind> Copy for Workspace<K> {}

/// Set up workspace state: restores either persisted layout schema (merging
/// in new panel kinds), re-clamps + reclassifies the surface on window
/// resize, and persists a V2 layout whenever it settles (not mid-drag).
///
/// This is a Dioxus hook — call it unconditionally from one component,
/// typically the app root, and pass the returned [`Workspace`] (it's `Copy`)
/// to whatever needs it.
///
/// `storage_key` is the localStorage key for layout persistence; pick one
/// per app (e.g. `"myapp_layout"`). `defaults` produces the initial layout
/// (see [`LayoutBuilder`]) and is also consulted when a saved layout is
/// missing panels that were added to the app after it was saved.
///
/// For several named layouts inside one workspace (a view switcher), use
/// [`use_views`] instead — it layers per-view storage keys
/// over this same machinery.
pub fn use_workspace<K: PanelKind>(
    storage_key: &'static str,
    defaults: fn() -> Vec<PanelWin<K>>,
) -> Workspace<K> {
    let initial_viewport = viewport_size();
    let ws = use_workspace_state(
        load_layout(storage_key, &defaults(), initial_viewport),
        defaults,
    );
    use_effect(move || {
        let ps = ws.panels.read().clone();
        let md = *ws.mode.read();
        let vp = *ws.viewport.read();
        // Persist once a drag settles — not on every pointermove/hover-shuffle.
        if ws.drag.read().is_none() && ws.tile_drag.read().is_none() {
            save_layout(storage_key, &ps, md, vp);
        }
    });
    ws
}

/// The persistence-free heart of [`use_workspace`] (and
/// [`use_views`]): the signal bundle plus the window
/// resize / global pointer listeners. `saved` is the already-loaded initial
/// layout, if any; the caller wires its own persistence effect.
pub(crate) fn use_workspace_state<K: PanelKind>(
    saved: Option<(Vec<PanelWin<K>>, Mode)>,
    defaults: fn() -> Vec<PanelWin<K>>,
) -> Workspace<K> {
    let initial_viewport = viewport_size();
    let panels = use_signal(|| {
        saved
            .as_ref()
            .map(|(p, _)| p.clone())
            .unwrap_or_else(defaults)
    });
    let mode = use_signal(|| saved.as_ref().map(|(_, m)| *m).unwrap_or(Mode::Floating));
    let drag = use_signal(|| Option::<Drag>::None);
    let tile_drag = use_signal(|| Option::<K>::None);
    let profile = use_signal(|| surface_profile(initial_viewport.0));
    let focused = use_signal(|| Option::<K>::None);
    let viewport = use_signal(|| initial_viewport);
    let ws_scroll = use_signal(|| 0.0_f64);
    let pending_dock_focus = use_signal(|| Option::<String>::None);

    use_hook(|| {
        use wasm_bindgen::closure::Closure;
        let mut viewport = viewport;
        let mut profile = profile;
        let mut panels = panels;

        // One re-projection step, shared by both the ResizeObserver and the
        // window "resize" listener. Stored floating geometry is scaled by the
        // viewport delta so panels grow/shrink *with* the window — the scale
        // is a pure ratio, so growing the window back restores prior sizes
        // (render-time effective_rect still guards against off-screen). The
        // last-applied size lives in the `viewport` signal, so whichever of
        // the two sources fires second sees ow == nw and no-ops — no double
        // scaling.
        let mut recompute = move || {
            let (nw, nh) = viewport_size();
            let (ow, oh) = *viewport.peek();
            if ow > 1.0 && oh > 1.0 {
                let (fx, fy) = (nw / ow, nh / oh);
                if fx.is_finite() && fy.is_finite() && (fx - 1.0).abs() + (fy - 1.0).abs() > 1e-3 {
                    let mut ps = panels.write();
                    for p in ps.iter_mut() {
                        p.x *= fx;
                        p.y *= fy;
                        p.w *= fx;
                        p.h *= fy;
                    }
                }
            }
            viewport.set((nw, nh));
            profile.set(surface_profile(nw));
        };

        // A ResizeObserver on <html> is the reliable signal inside webviews
        // (Tauri/WKWebView), where the window "resize" event is flaky on
        // native-window resize. The plain window listener stays as a
        // belt-and-suspenders for ordinary browser tabs.
        let obs_cb = Closure::wrap(Box::new({
            let mut recompute = recompute;
            move || recompute()
        }) as Box<dyn FnMut()>);
        if let Some(el) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.document_element())
        {
            if let Ok(observer) = web_sys::ResizeObserver::new(obs_cb.as_ref().unchecked_ref()) {
                observer.observe(&el);
                // Keep the observer alive for the lifetime of the app.
                std::mem::forget(observer);
            }
        }
        obs_cb.forget();

        let win_cb = Closure::wrap(
            Box::new(move |_e: web_sys::Event| recompute()) as Box<dyn FnMut(web_sys::Event)>
        );
        if let Some(w) = web_sys::window() {
            let _ = w.add_event_listener_with_callback("resize", win_cb.as_ref().unchecked_ref());
        }
        win_cb.forget();
    });

    use_hook(|| {
        use wasm_bindgen::closure::Closure;

        let mut panels_for_move = panels;
        let drag_for_move = drag;
        let mode_for_move = mode;
        let profile_for_move = profile;
        let viewport_for_move = viewport;
        let move_cb = Closure::wrap(Box::new(move |e: web_sys::PointerEvent| {
            if let Some(d) = *drag_for_move.peek() {
                e.prevent_default();
                // Compact surfaces force tiling, so the drag math has to ask
                // the surface profile rather than a raw mode signal.
                let tiling =
                    effective_mode(*mode_for_move.peek(), &profile_for_move.peek()) == Mode::Tiling;
                let (vw, _) = *viewport_for_move.peek();
                apply_drag(
                    &mut panels_for_move.write(),
                    &d,
                    e.client_x() as f64,
                    e.client_y() as f64,
                    tiling,
                    vw,
                    &Clamp::WEB,
                    &TileMetrics::WEB,
                );
            }
        }) as Box<dyn FnMut(web_sys::PointerEvent)>);

        let mut drag_for_up = drag;
        let mut tile_drag_for_up = tile_drag;
        let up_cb = Closure::wrap(Box::new(move |e: web_sys::PointerEvent| {
            if drag_for_up.peek().is_some() || tile_drag_for_up.peek().is_some() {
                e.prevent_default();
                clear_selection();
            }
            drag_for_up.set(None);
            tile_drag_for_up.set(None);
        }) as Box<dyn FnMut(web_sys::PointerEvent)>);

        let mut drag_for_cancel = drag;
        let mut tile_drag_for_cancel = tile_drag;
        let cancel_cb = Closure::wrap(Box::new(move |_e: web_sys::PointerEvent| {
            clear_selection();
            drag_for_cancel.set(None);
            tile_drag_for_cancel.set(None);
        }) as Box<dyn FnMut(web_sys::PointerEvent)>);

        if let Some(w) = web_sys::window() {
            let _ =
                w.add_event_listener_with_callback("pointermove", move_cb.as_ref().unchecked_ref());
            let _ = w.add_event_listener_with_callback("pointerup", up_cb.as_ref().unchecked_ref());
            let _ = w.add_event_listener_with_callback(
                "pointercancel",
                cancel_cb.as_ref().unchecked_ref(),
            );
        }

        move_cb.forget();
        up_cb.forget();
        cancel_cb.forget();
    });

    use_effect(move || {
        use wasm_bindgen::JsCast;
        let target = pending_dock_focus.read().clone();
        let Some(target) = target else {
            return;
        };
        let Some(element) = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id(&target))
        else {
            return;
        };
        let Ok(element) = element.dyn_into::<web_sys::HtmlElement>() else {
            return;
        };
        let _ = element.focus();
        let mut pending = pending_dock_focus;
        pending.set(None);
    });

    Workspace {
        panels,
        mode,
        drag,
        tile_drag,
        profile,
        focused,
        viewport,
        ws_scroll,
        pending_dock_focus,
    }
}

impl<K: PanelKind> Workspace<K> {
    /// Effective [`Mode`] after applying the current surface's constraints.
    /// Compact surfaces force tiling; tablet and regular surfaces preserve
    /// the user's preferred [`mode`](Workspace::mode).
    pub fn effective_mode(&self) -> Mode {
        effective_mode(*self.mode.read(), &self.profile.read())
    }

    /// Current surface profile, including its width tier and input
    /// capabilities.
    pub fn surface_profile(&self) -> SurfaceProfile {
        *self.profile.read()
    }

    /// Panel that currently owns window-management keyboard commands.
    pub fn focused(&self) -> Option<K> {
        *self.focused.read()
    }

    /// Class for the app root: `ws-root`, plus exactly one surface tier
    /// (`compact`, `tablet`, or `regular`), plus `coarse` when the detected
    /// [`SurfaceCapabilities::coarse_pointer`] says the primary pointer is
    /// imprecise, plus `dragging` while a pointer operation is active.
    ///
    /// Touch sizing follows the *pointer*, not the width. A 1400px kiosk
    /// touchscreen is a `regular` surface that still needs 44px targets, and
    /// a 900px desktop window driven by a mouse does not. Deriving hit size
    /// from the tier would get both backwards; `--hit-min` keys off `coarse`.
    pub fn root_class(&self) -> &'static str {
        let profile = *self.profile.read();
        let dragging = self.drag.read().is_some() || self.tile_drag.read().is_some();
        match (profile.class, profile.caps.coarse_pointer, dragging) {
            (SurfaceClass::Compact, false, false) => "ws-root compact",
            (SurfaceClass::Compact, false, true) => "ws-root compact dragging",
            (SurfaceClass::Compact, true, false) => "ws-root compact coarse",
            (SurfaceClass::Compact, true, true) => "ws-root compact coarse dragging",
            (SurfaceClass::Tablet, false, false) => "ws-root tablet",
            (SurfaceClass::Tablet, false, true) => "ws-root tablet dragging",
            (SurfaceClass::Tablet, true, false) => "ws-root tablet coarse",
            (SurfaceClass::Tablet, true, true) => "ws-root tablet coarse dragging",
            (SurfaceClass::Regular, false, false) => "ws-root regular",
            (SurfaceClass::Regular, false, true) => "ws-root regular dragging",
            (SurfaceClass::Regular, true, false) => "ws-root regular coarse",
            (SurfaceClass::Regular, true, true) => "ws-root regular coarse dragging",
        }
    }

    fn execute_command(&self, target: Option<K>, command: PanelCommand) {
        let (vw, vh) = *self.viewport.read();
        let mut focused = target.or(*self.focused.read());
        let mut panels = self.panels;
        let mut mode = self.mode;
        apply_command(
            &mut panels.write(),
            &mut mode.write(),
            &mut focused,
            command,
            Clamp::WEB,
            CommandStep::WEB,
            vw,
            vh,
        );
        let mut focused_signal = self.focused;
        focused_signal.set(focused);
    }

    fn minimize_to_dock(&self, kind: K) {
        self.execute_command(Some(kind), PanelCommand::Minimize);
        let mut focused = self.focused;
        focused.set(None);
        let mut pending = self.pending_dock_focus;
        pending.set(Some(dock_chip_id(kind)));
    }

    /// Translate a Dioxus keyboard event to the renderer-neutral core key
    /// contract and apply any resulting window-management command.
    ///
    /// The focused panel owns panel commands, the workspace owns commands
    /// when no panel is focused, and an active text input delegates policy to
    /// [`FocusContext::TextInput`].
    pub fn handle_key(&self, event: &KeyboardEvent) {
        let key = match event.key() {
            DioxusKey::ArrowLeft => Key::Left,
            DioxusKey::ArrowRight => Key::Right,
            DioxusKey::ArrowUp => Key::Up,
            DioxusKey::ArrowDown => Key::Down,
            DioxusKey::Enter => Key::Enter,
            DioxusKey::Escape => Key::Escape,
            DioxusKey::Tab => Key::Tab,
            DioxusKey::Character(value) => {
                let mut characters = value.chars();
                let Some(character) = characters.next() else {
                    return;
                };
                if characters.next().is_some() {
                    return;
                }
                Key::Char(character)
            }
            _ => return,
        };
        let modifiers = event.modifiers();
        let chord = KeyChord {
            key,
            shift: modifiers.shift(),
            alt: modifiers.alt(),
            ctrl: modifiers.ctrl(),
            meta: modifiers.meta(),
        };
        let current = *self.focused.read();
        let context = if is_editing() {
            FocusContext::TextInput
        } else if let Some(kind) = current {
            FocusContext::Panel(kind)
        } else {
            FocusContext::Workspace
        };
        let Some(command) = command_for(chord, &context) else {
            return;
        };
        event.prevent_default();
        if command == PanelCommand::Minimize {
            if let Some(kind) = current {
                self.minimize_to_dock(kind);
            }
        } else {
            self.execute_command(current, command);
        }
    }

    /// Start a floating move or resize from a pointer-down, translating the
    /// Dioxus event into the core pointer contract before capturing
    /// [`Drag`] geometry.
    ///
    /// Pointer capture keeps the drag alive when the cursor crosses an
    /// iframe, a canvas, or the embedded editor — anything that would
    /// otherwise swallow the move events.
    pub fn begin_pointer_drag(&self, idx: usize, kind: DragKind, event: &DioxusPointerEvent) {
        if kind == DragKind::Move {
            event.prevent_default();
        }
        capture_pointer(event);
        let pointer = core_pointer_event(event, PointerEventKind::Down(PointerButton::Primary));
        let (vw, vh) = *self.viewport.read();
        let mut panels = self.panels;
        let mut drag = core_begin_drag(
            &mut panels.write(),
            idx,
            kind,
            pointer.x,
            pointer.y,
            vw,
            vh,
            &Clamp::WEB,
        );
        // The floating surface is translated by workspace scroll; preserve
        // the visible vertical anchor when normalizing stored geometry.
        if let Some(drag) = drag.as_mut() {
            if let Some(panel) = panels.write().get_mut(idx) {
                let (_, y, _, _) = floating_rect(panel, vw, vh);
                panel.y = y;
                drag.start_y = y;
            }
        }
        if drag.is_some() {
            if let Some(panel) = panels.read().get(idx) {
                let mut focused = self.focused;
                focused.set(Some(panel.kind));
            }
            let mut drag_signal = self.drag;
            drag_signal.set(drag);
        }
    }

    /// Start a span-snapped tiling resize from a pointer-down on the corner
    /// control.
    pub fn begin_pointer_tile_resize(&self, idx: usize, event: &DioxusPointerEvent) {
        event.prevent_default();
        capture_pointer(event);
        let pointer = core_pointer_event(event, PointerEventKind::Down(PointerButton::Primary));
        let drag = core_begin_tile_resize(&self.panels.read(), idx, pointer.x, pointer.y);
        if drag.is_some() {
            if let Some(panel) = self.panels.read().get(idx) {
                let mut focused = self.focused;
                focused.set(Some(panel.kind));
            }
            let mut drag_signal = self.drag;
            drag_signal.set(drag);
        }
    }

    /// Apply an in-flight drag from an app root's `onpointermove`.
    pub fn handle_pointer_move(&self, event: &DioxusPointerEvent) {
        let kind = if self.drag.read().is_some() {
            PointerEventKind::Drag(PointerButton::Primary)
        } else {
            PointerEventKind::Moved
        };
        let pointer = core_pointer_event(event, kind);
        if let Some(drag) = *self.drag.read() {
            let tiling = self.effective_mode() == Mode::Tiling;
            let (vw, _) = *self.viewport.read();
            let mut panels = self.panels;
            apply_drag(
                &mut panels.write(),
                &drag,
                pointer.x,
                pointer.y,
                tiling,
                vw,
                &Clamp::WEB,
                &TileMetrics::WEB,
            );
        }
    }

    /// End floating and tiling drags from an app root's `onpointerup` or
    /// `onpointercancel`, allowing the settled layout to persist.
    ///
    /// Releases pointer capture and clears any text selection the drag swept
    /// across on the way.
    pub fn handle_pointer_up(&self, event: &DioxusPointerEvent) {
        release_pointer(event);
        let pointer = core_pointer_event(event, PointerEventKind::Up(PointerButton::Primary));
        if matches!(pointer.kind, PointerEventKind::Up(_)) {
            let mut drag = self.drag;
            drag.set(None);
            let mut tile_drag = self.tile_drag;
            tile_drag.set(None);
            clear_selection();
        }
    }

    /// Workspace-area height in CSS px: the viewport height minus the top bar
    /// and dock chrome ([`Clamp::WEB`]'s `outer_h`). This is the viewport the
    /// floating panels scroll within.
    fn workspace_height(&self) -> f64 {
        let (_, vh) = *self.viewport.read();
        (vh - Clamp::WEB.outer_h).max(Clamp::WEB.floor_h)
    }

    /// Total floating-content height in CSS px: the lowest edge of any visible
    /// (non-minimized) floating panel. [`render`](Workspace::render) draws
    /// floating panels at their stored `y` (floored at 0, see
    /// [`floating_rect`]), so the drawn lowest edge equals the stored
    /// `max(y + h)` that [`floating_content_height`] computes. Bounds the
    /// workspace scroll.
    fn floating_content_h(&self) -> f64 {
        let ps = self.panels.read();
        let visible: Vec<usize> = ps
            .iter()
            .enumerate()
            .filter(|(_, p)| p.state == WinState::Floating)
            .map(|(i, _)| i)
            .collect();
        floating_content_height(&ps, &visible)
    }

    /// Attach to the workspace's `onwheel` — scrolls the whole floating
    /// workspace vertically, clamped to the content bounds (no rubber-band
    /// past the top or the bottom of the lowest panel). A no-op in tiling
    /// mode (the browser scrolls `.ws.tiling` natively) and when the content
    /// fits the workspace area.
    ///
    /// Per-panel body scroll wins first: if the wheel lands inside a
    /// `.panel-body` that can still scroll in the wheel's direction, the
    /// workspace scroll stands down (manual scroll chaining — the body absorbs
    /// the wheel until it hits its boundary, then the workspace takes over).
    pub fn handle_wheel(&self, e: &dioxus::events::WheelEvent) {
        if self.effective_mode() != Mode::Floating {
            return;
        }
        let dy = e.data().delta().strip_units().y;
        if panel_body_absorbs_wheel(dy) {
            return;
        }
        let content_h = self.floating_content_h();
        let view_h = self.workspace_height();
        if max_scroll(content_h, view_h) <= 0.0 {
            return;
        }
        let mut ws_scroll = self.ws_scroll;
        let next = *ws_scroll.read() + dy;
        ws_scroll.set(clamp_scroll(next, content_h, view_h));
    }

    /// Tiling-mode reorder: move `dragged` into `target`'s slot. Moving down
    /// the flow inserts after the target, moving up inserts before — the
    /// classic sortable-list shuffle, so the dragged panel snaps into
    /// whichever slot the pointer is over. Vec order is the tiling order and
    /// persists with the layout.
    fn reorder_tile(&self, dragged: K, target: K) {
        let mut panels = self.panels;
        core_reorder_tile(&mut *panels.write(), dragged, target);
    }

    /// Render the workspace area. `body` renders one panel's content given
    /// its kind and whether that panel is currently maximized.
    ///
    /// This draws every visible panel with its chrome (header, traffic
    /// lights, resize handle) in the current
    /// [`effective_mode`](Workspace::effective_mode); minimized panels are
    /// skipped (they live in the [`dock`](Workspace::dock)) and a maximized
    /// panel hides all others. Each panel gets a `panel panel-<slug>` class
    /// (slugified from [`PanelKind::title`]) so apps can style individual
    /// panels — e.g. making one full-width in tiling mode.
    pub fn render(&self, body: impl Fn(K, bool) -> Element) -> Element {
        self.render_with_header(body, |_, _| rsx! {})
    }

    /// Render the workspace with an application-owned action slot in every
    /// panel header.
    ///
    /// `header_actions` receives the same panel kind and maximized flag as
    /// `body`. Return an empty `rsx! {}` for panels without actions. Use
    /// [`PanelHeaderButton`] for the built-in compact styling and drag-safe
    /// pointer behavior.
    pub fn render_with_header(
        &self,
        body: impl Fn(K, bool) -> Element,
        header_actions: impl Fn(K, bool) -> Element,
    ) -> Element {
        let ws = *self;
        let mode_now = self.effective_mode();
        let ps = self.panels.read().clone();
        let maximized = ps.iter().position(|p| p.state == WinState::Maximized);
        let visible: Vec<usize> = match maximized {
            Some(mi) => vec![mi],
            None => ps
                .iter()
                .enumerate()
                .filter(|(_, p)| p.state != WinState::Minimized)
                .map(|(i, _)| i)
                .collect(),
        };
        let ws_class = if maximized.is_some() {
            "ws maxed"
        } else if mode_now == Mode::Tiling {
            "ws tiling"
        } else {
            "ws floating"
        };

        // Floating workspace-level scroll: clamp the stored offset to the
        // freshly measured content, then offset each panel's top by it. When
        // content fits, this is 0. Tiling/maximized never scroll this way.
        let floating_now = maximized.is_none() && mode_now == Mode::Floating;
        let scroll = if floating_now {
            let content_h = self.floating_content_h();
            let view_h = self.workspace_height();
            let clamped = clamp_scroll(*self.ws_scroll.read(), content_h, view_h);
            if (clamped - *self.ws_scroll.peek()).abs() > f64::EPSILON {
                let mut s = self.ws_scroll;
                s.set(clamped);
            }
            clamped
        } else {
            0.0
        };

        let dragging_tile = *self.tile_drag.read();
        let dragging_panel = self.drag.read().as_ref().map(|drag| drag.idx);
        let focused = *self.focused.read();
        let window_management = self.profile.read().window_management();
        // The column count is core surface policy, published to CSS as a
        // custom property so the stylesheet never re-tabulates the tiers.
        let tile_cols = self.profile.read().tile_columns();
        rsx! {
            div { class: "{ws_class}", style: "--tile-cols:{tile_cols};",
                onwheel: move |e| ws.handle_wheel(&e),
                for i in visible.iter().copied() {
                    {
                        let p = ps[i];
                        let floating = maximized.is_none() && mode_now == Mode::Floating;
                        let tiling = maximized.is_none() && mode_now == Mode::Tiling;
                        let kind = p.kind;
                        let style = if maximized.is_some() {
                            "position:absolute; inset:0;".to_string()
                        } else if floating {
                            // Project x / width / height through the viewport
                            // clamp, but keep the stored `y` (floored at 0) and
                            // shift it by the workspace scroll so panels placed
                            // below the fold can be scrolled into view. Stored
                            // geometry stays intact, so panels spring back when
                            // the window grows again.
                            let (vw, vh) = *ws.viewport.read();
                            let (x, y, w, h) = floating_rect(&p, vw, vh);
                            let top = y - scroll;
                            format!("position:absolute; left:{x}px; top:{top}px; width:{w}px; height:{h}px; z-index:{};",
                                p.z)
                        } else if tiling && window_management {
                            // `tile_w` / `tile_h` are spans, so they map
                            // straight onto the tiling grid. The row height
                            // itself comes from the grid
                            // (`grid-auto-rows: minmax(--tile-row-min, 1fr)`),
                            // not from the panel: a tile must be free to be
                            // shorter than its content so `.panel-body`
                            // scrolls instead of growing the row. A span wider
                            // than the tier's column count is clamped by the
                            // grid, which is what collapses a 4-wide tile to
                            // full width on tablet.
                            format!(
                                "grid-column:span {}; grid-row:span {};",
                                p.tile_w.min(tile_cols),
                                p.tile_h
                            )
                        } else {
                            String::new()
                        };
                        let slug = kind_slug(p.kind.title());
                        let tile_drag_class =
                            if dragging_tile == Some(kind) { " tile-dragging" } else { "" };
                        let pointer_drag_class =
                            if floating && dragging_panel == Some(i) { " dragging" } else { "" };
                        let focus_class =
                            if focused == Some(kind) { " focused" } else { "" };
                        rsx! {
                            section {
                                // Keyed by kind (stable identity), not index:
                                // tiling reorders mutate the Vec mid-drag and
                                // index keys would remount every panel.
                                key: "{slug}",
                                class: "panel panel-{slug}{tile_drag_class}{pointer_drag_class}{focus_class}",
                                style: "{style}",
                                onpointerenter: move |_| {
                                    // Snap the dragged panel into this slot.
                                    if tiling {
                                        if let Some(dragged) = *ws.tile_drag.read() {
                                            if dragged != kind {
                                                ws.reorder_tile(dragged, kind);
                                            }
                                        }
                                    }
                                },
                                onpointerdown: move |_| {
                                    let mut focused = ws.focused;
                                    focused.set(Some(kind));
                                    // z-order only matters when panels overlap.
                                    if floating {
                                        ws.execute_command(Some(kind), PanelCommand::Raise);
                                    }
                                },
                                {ws.header(
                                    i,
                                    p.kind,
                                    floating,
                                    tiling,
                                    header_actions(p.kind, maximized == Some(i)),
                                )}
                                div { class: "panel-body",
                                    {body(p.kind, maximized == Some(i))}
                                }
                                if window_management && (floating || tiling) {
                                    button {
                                        r#type: "button",
                                        class: "resize",
                                        aria_label: "Resize panel",
                                        tabindex: "0",
                                        onfocus: move |_| {
                                            let mut focused = ws.focused;
                                            focused.set(Some(kind));
                                        },
                                        onkeydown: move |event: KeyboardEvent| {
                                            event.stop_propagation();
                                            ws.handle_key(&event);
                                        },
                                        onpointerdown: move |event: DioxusPointerEvent| {
                                            if floating {
                                                ws.begin_pointer_drag(i, DragKind::Resize, &event);
                                            } else {
                                                ws.begin_pointer_tile_resize(i, &event);
                                            }
                                        },
                                        onpointermove: move |e: DioxusPointerEvent| ws.handle_pointer_move(&e),
                                        onpointerup: move |e: DioxusPointerEvent| ws.handle_pointer_up(&e),
                                        onpointercancel: move |e: DioxusPointerEvent| ws.handle_pointer_up(&e),
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Panel chrome: ratatui-style inline/inset title row on the panel's top
    /// border, with traffic lights in printer-CMY (blue = floating⇄tiling,
    /// amber = minimize, magenta = maximize⇄restore). The toolbar starts a
    /// floating move or tiling reorder where the surface profile permits
    /// window management, and `actions` renders app-supplied controls inside
    /// the same inset row.
    fn header(
        &self,
        idx: usize,
        kind: K,
        draggable: bool,
        tiling: bool,
        actions: Element,
    ) -> Element {
        let ws = *self;
        let title = kind.title();
        let is_max = self.panels.read().get(idx).map(|p| p.state) == Some(WinState::Maximized);
        let profile = *self.profile.read();
        let window_management = profile.window_management();
        // A tier may withhold the means to enter a window state, never the
        // means to leave one: a panel maximized on a wider surface arrives
        // maximized here through persistence, so compact still renders the
        // magenta restore light (and only that one).
        let restore_only = profile.must_offer_restore(
            self.panels
                .read()
                .get(idx)
                .map(|p| p.state)
                .unwrap_or(WinState::Floating),
        );
        rsx! {
            header {
                class: "panel-head",
                title: "{title}",
                tabindex: "0",
                role: "toolbar",
                onfocus: move |_| {
                    let mut focused = ws.focused;
                    focused.set(Some(kind));
                },
                onkeydown: move |event: KeyboardEvent| {
                    event.stop_propagation();
                    ws.handle_key(&event);
                },
                onpointerdown: move |event: DioxusPointerEvent| {
                    if draggable {
                        ws.begin_pointer_drag(idx, DragKind::Move, &event);
                    } else if tiling && window_management {
                        event.prevent_default();
                        let mut focused = ws.focused;
                        focused.set(Some(kind));
                        let mut tile_drag = ws.tile_drag;
                        tile_drag.set(Some(kind));
                    }
                },
                if window_management || restore_only {
                    div { class: "lights",
                        if window_management {
                        button {
                            r#type: "button",
                            class: "light mode",
                            title: "tiling / floating",
                            aria_label: "tiling / floating",
                            onpointerdown: move |event: DioxusPointerEvent| event.stop_propagation(),
                            onkeydown: move |event: KeyboardEvent| event.stop_propagation(),
                            onclick: move |_| {
                                ws.execute_command(Some(kind), PanelCommand::ToggleMode);
                            },
                        }
                        button {
                            r#type: "button",
                            class: "light yellow",
                            title: "minimize",
                            aria_label: "minimize",
                            onpointerdown: move |event: DioxusPointerEvent| event.stop_propagation(),
                            onkeydown: move |event: KeyboardEvent| event.stop_propagation(),
                            onclick: move |_| ws.minimize_to_dock(kind),
                        }
                        }
                        button {
                            r#type: "button",
                            class: "light max",
                            title: "maximize / restore",
                            aria_label: "maximize / restore",
                            onpointerdown: move |event: DioxusPointerEvent| event.stop_propagation(),
                            onkeydown: move |event: KeyboardEvent| event.stop_propagation(),
                            onclick: move |_| {
                                ws.execute_command(Some(kind), PanelCommand::Maximize);
                            },
                        }
                    }
                }
                span { class: "panel-title", title: "{title}", "{title}" }
                if is_max { span { class: "max-hint", "maximized" } }
                div {
                    class: "panel-head-actions",
                    onpointerdown: move |e: DioxusPointerEvent| e.stop_propagation(),
                    onkeydown: move |e: KeyboardEvent| e.stop_propagation(),
                    {actions}
                }
            }
        }
    }

    /// Restore and raise the panel of `kind`: un-minimizes it (the
    /// programmatic twin of a dock-chip click) and brings it to the front.
    /// No-op when the layout holds no panel of that kind. Hook for command
    /// palettes / keyboard shortcuts.
    ///
    /// ```no_run
    /// # use panel_kit::{PanelKind, Workspace};
    /// # fn jump<K: PanelKind>(ws: Workspace<K>, kind: K) {
    /// ws.restore(kind);
    /// # }
    /// ```
    pub fn restore(&self, kind: K) {
        let mut panels = self.panels;
        panel_kit_core::restore(&mut panels.write(), kind);
    }

    /// The footer dock: minimized panels collapse to chips; click restores
    /// (and raises) the panel. Render it once after
    /// [`render`](Workspace::render) in the app root.
    pub fn dock(&self) -> Element {
        let ws = *self;
        let minimized: Vec<K> = self
            .panels
            .read()
            .iter()
            .filter(|panel| panel.state == WinState::Minimized)
            .map(|panel| panel.kind)
            .collect();
        rsx! {
            footer { class: "dock",
                span { class: "dock-label", "dock:" }
                if minimized.is_empty() {
                    span { class: "dock-empty", "— nothing minimized —" }
                }
                for kind in minimized.iter().copied() {
                    {
                        let chip_id = dock_chip_id(kind);
                        let label = format!("Restore {}", kind.title());
                        rsx! {
                            button {
                                key: "{chip_id}",
                                id: "{chip_id}",
                                r#type: "button",
                                class: "dock-chip",
                                aria_label: "{label}",
                                onfocus: move |_| {
                                    let mut focused = ws.focused;
                                    focused.set(None);
                                },
                                onclick: move |_| {
                                    ws.restore(kind);
                                    let mut focused = ws.focused;
                                    focused.set(Some(kind));
                                },
                                "{kind.title()}"
                            }
                        }
                    }
                }
            }
        }
    }
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
#[component]
pub fn Spinner(
    /// Text shown after the ring; the label span is omitted entirely when
    /// empty (the default).
    #[props(default = String::new())]
    label: String,
) -> Element {
    rsx! {
        span { class: "spinner",
            span { class: "spin-ring" }
            if !label.is_empty() {
                span { class: "spin-label", "{label}" }
            }
        }
    }
}

/// Viewport-aware tooltip placement: prefer left of the cursor, flip right if
/// there's no room, and clamp inside the window. (CSS anchor-positioning would
/// do this natively but WebKit doesn't support it yet.)
///
/// `(cx, cy)` is the cursor position and `(tw, th)` the tooltip size, all in
/// px / client coordinates; the returned `(x, y)` is the tooltip's top-left,
/// ready for `position: fixed; left:{x}px; top:{y}px`.
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

#[cfg(test)]
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

    /// The whole point of `panel_kit_core::tokens`: three copies of the
    /// palette existed (this stylesheet, the terminal `Theme`, the boot
    /// shell) and the boot copy had already drifted seven values. The boot
    /// sheet is now generated, so it cannot drift. This stylesheet is still
    /// hand-written, so the drift has to be caught instead.
    #[test]
    fn injected_stylesheet_declares_every_token_at_its_canonical_value() {
        use panel_kit_core::tokens;

        // Parse rather than substring-match: the stylesheet aligns values in
        // a column (`--bg:    #0a0a0a;`) and some declarations carry trailing
        // comments, so a literal `--bg: #0a0a0a` needle would fail on
        // formatting instead of on drift — the worst kind of guard, because
        // the fix would be to loosen the test.
        for token in tokens::DARK {
            let needle = format!("--{}:", token.name);
            let declared = super::CSS
                .lines()
                .filter_map(|line| line.trim().strip_prefix(needle.as_str()))
                .map(|rest| {
                    rest.split(';')
                        .next()
                        .unwrap_or_default()
                        .split("/*")
                        .next()
                        .unwrap_or_default()
                        .trim()
                        .to_ascii_lowercase()
                })
                .next();

            match declared {
                Some(value) => assert_eq!(
                    value, token.hex,
                    "panel-kit.css declares --{} as {value}, but tokens.rs says \
                     {} — tokens.rs is the source of truth, so update the \
                     stylesheet, not the token",
                    token.name, token.hex
                ),
                None => panic!(
                    "panel-kit.css never declares --{}; every token must reach \
                     the injected stylesheet",
                    token.name
                ),
            }
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
