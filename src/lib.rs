//! Generic Dioxus panel-workspace library.
//!
//! Factored out of apple-notes-ocr-flow's reviewer UI so any app can get the
//! same shell: every view is a panel you can move/resize/minimize/maximize,
//! with floating (free placement) and tiling (auto grid) workspace modes,
//! macOS-style traffic lights, a minimized-panel dock strip, and layout
//! persistence to localStorage. The crate also ships two standalone widgets:
//! the [`badge`] module (a clickable metadata chip) and [`Spinner`].
//!
//! The app supplies two things: a [`PanelKind`] impl (an enum of its panels)
//! and a body-render callback. Everything else — geometry, z-order, drag
//! state, viewport clamping, the mobile breakpoint, persistence — lives in
//! the [`Workspace`] handle created by [`use_workspace`].
//!
//! This is a wasm-only crate (Dioxus web): it builds for
//! `wasm32-unknown-unknown` and expects a browser environment at runtime.
//!
//! # Quick start
//!
//! ```no_run
//! use dioxus::prelude::*;
//! use panel_kit::{use_workspace, LayoutBuilder, PanelKind, PanelWin};
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
//!         div { class: ws.root_class(),
//!             onmousemove: move |e| ws.handle_mouse_move(&e),
//!             onmouseup: move |_| ws.handle_mouse_up(),
//!             header { class: "topbar" /* app-specific */ }
//!             {ws.render(|kind, _maximized| rsx! { "body for {kind.title()}" })}
//!             {ws.dock()}
//!         }
//!     }
//! }
//! ```
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
//!   lights, drag/resize/reorder, dock, persistence, mobile stack, tooltips.
//! - `badge` — every [`badge::BadgeKind`], every prop, and an event log
//!   proving each [`badge::BadgeAction`] variant fires.
//! - `spinner` — [`Spinner`] with and without a label.
//! - `theming` — the `:root` variable override path with switchable presets.

#![warn(missing_docs)]

pub mod badge;

use dioxus::events::MouseEvent;
use dioxus::prelude::*;
use gloo_storage::{LocalStorage, Storage};

pub use panel_kit_core::{
    Drag, DragKind, LayoutBuilder, Mode, PanelKind, PanelWin, WinState, TILE_ROW_PX,
};
use panel_kit_core::{
    apply_drag, begin_drag as core_begin_drag, begin_tile_resize as core_begin_tile_resize,
    effective_rect as core_effective_rect, front_z, kind_slug, merge_defaults,
    reorder_tile as core_reorder_tile, Clamp, SavedLayout, TileMetrics, TILE_W_MAX,
};

/// Base stylesheet for the workspace chrome (panels, lights, dock, badges,
/// spinner, tooltip overlay, mobile breakpoint). Inject once at the app root
/// with `style { {panel_kit::CSS} }`, then layer app-specific styles after
/// it; override the `:root` CSS variables to retheme (see the
/// [crate-level theming notes](crate#theming)).
pub const CSS: &str = include_str!("../assets/panel-kit.css");

// The core types (PanelKind, PanelWin, WinState, Mode, Drag, LayoutBuilder)
// and all geometry/drag math live in panel-kit-core and are re-exported
// above — this crate is the Dioxus shell: signals, DOM events, CSS,
// localStorage persistence, and rendering.

fn viewport_size() -> (f64, f64) {
    let win = web_sys::window();
    let vw = win.as_ref().and_then(|w| w.inner_width().ok()).and_then(|v| v.as_f64()).unwrap_or(1280.0);
    let vh = win.and_then(|w| w.inner_height().ok()).and_then(|v| v.as_f64()).unwrap_or(800.0);
    (vw, vh)
}

/// Render-time viewport clamp — core math with the web px metrics.
fn effective_rect<K>(p: &PanelWin<K>, vw: f64, vh: f64) -> (f64, f64, f64, f64) {
    core_effective_rect(p, vw, vh, &Clamp::WEB)
}

/// Narrow viewport (< 760 px wide) → mobile shell (static stacked tiling
/// instead of the floating panel workspace).
///
/// [`use_workspace`] re-evaluates this on every window resize and exposes it
/// as [`Workspace::is_mobile`]; call it directly only outside a workspace.
pub fn viewport_is_mobile() -> bool {
    web_sys::window()
        .and_then(|w| w.inner_width().ok())
        .and_then(|v| v.as_f64())
        .map(|w| w < 760.0)
        .unwrap_or(false)
}

/// True while an `<input>`/`<textarea>` has focus — apps use this to
/// suppress single-key shortcuts while the user is typing. Check it at the
/// top of a global `onkeydown` handler before matching shortcut keys.
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

fn save_layout<K: PanelKind>(key: &str, panels: &[PanelWin<K>], mode: Mode) {
    let _ = LocalStorage::set(key, SavedLayout { panels: panels.to_vec(), tiling: mode == Mode::Tiling });
}

/// Load the saved layout, reconciling against the current panel set: panels
/// added since the layout was saved are appended with their default placement,
/// so new features still show up for existing users.
fn load_layout<K: PanelKind>(key: &str, defaults: &[PanelWin<K>]) -> Option<(Vec<PanelWin<K>>, Mode)> {
    let saved: SavedLayout<K> = LocalStorage::get(key).ok()?;
    let mut panels = saved.panels;
    merge_defaults(&mut panels, defaults);
    Some((panels, if saved.tiling { Mode::Tiling } else { Mode::Floating }))
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
    /// [`effective_mode`](Workspace::effective_mode) when rendering — a
    /// mobile viewport overrides this.
    pub mode: Signal<Mode>,
    /// The in-flight floating-mode move/resize [`Drag`], if any.
    pub drag: Signal<Option<Drag>>,
    /// Tiling-mode reorder drag: the kind being dragged. Hovering another
    /// panel while set live-shuffles the dragged panel into that slot.
    pub tile_drag: Signal<Option<K>>,
    /// Whether the viewport is below the mobile breakpoint (see
    /// [`viewport_is_mobile`]); re-evaluated on every window resize.
    pub is_mobile: Signal<bool>,
    /// Live window size — [`render`](Workspace::render) subscribes so
    /// floating panels re-project through the viewport clamp on every
    /// resize (both directions).
    pub viewport: Signal<(f64, f64)>,
}

impl<K: PanelKind> Clone for Workspace<K> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<K: PanelKind> Copy for Workspace<K> {}

/// Set up workspace state: restores the persisted layout (merging in any new
/// panel kinds), re-clamps + re-evaluates the mobile breakpoint on window
/// resize, and persists the layout whenever it settles (not mid-drag).
///
/// This is a Dioxus hook — call it unconditionally from one component,
/// typically the app root, and pass the returned [`Workspace`] (it's `Copy`)
/// to whatever needs it.
///
/// `storage_key` is the localStorage key for layout persistence; pick one
/// per app (e.g. `"myapp_layout"`). `defaults` produces the initial layout
/// (see [`LayoutBuilder`]) and is also consulted when a saved layout is
/// missing panels that were added to the app after it was saved.
pub fn use_workspace<K: PanelKind>(
    storage_key: &'static str,
    defaults: fn() -> Vec<PanelWin<K>>,
) -> Workspace<K> {
    let saved = load_layout(storage_key, &defaults());
    let panels =
        use_signal(|| saved.as_ref().map(|(p, _)| p.clone()).unwrap_or_else(defaults));
    let mode = use_signal(|| saved.as_ref().map(|(_, m)| *m).unwrap_or(Mode::Floating));
    let drag = use_signal(|| Option::<Drag>::None);
    let tile_drag = use_signal(|| Option::<K>::None);
    let is_mobile = use_signal(viewport_is_mobile);
    let viewport = use_signal(viewport_size);

    use_hook(|| {
        use wasm_bindgen::closure::Closure;
        use wasm_bindgen::JsCast;
        let mut viewport = viewport;
        let mut is_mobile = is_mobile;
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
                if fx.is_finite()
                    && fy.is_finite()
                    && (fx - 1.0).abs() + (fy - 1.0).abs() > 1e-3
                {
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
            is_mobile.set(viewport_is_mobile());
        };

        // A ResizeObserver on <html> is the reliable signal inside webviews
        // (Tauri/WKWebView), where the window "resize" event is flaky on
        // native-window resize. The plain window listener stays as a
        // belt-and-suspenders for ordinary browser tabs.
        let obs_cb = Closure::wrap(Box::new({
            let mut recompute = recompute.clone();
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

    use_effect(move || {
        let ps = panels.read().clone();
        let md = *mode.read();
        // Persist once a drag settles — not on every mousemove/hover-shuffle.
        if drag.read().is_none() && tile_drag.read().is_none() {
            save_layout(storage_key, &ps, md);
        }
    });

    Workspace { panels, mode, drag, tile_drag, is_mobile, viewport }
}

impl<K: PanelKind> Workspace<K> {
    /// Effective [`Mode`]: a narrow viewport forces the static stacked
    /// (tiling) layout — the floating workspace metaphor doesn't fit a
    /// phone. Use this instead of reading [`mode`](Workspace::mode) when
    /// deciding how to render.
    pub fn effective_mode(&self) -> Mode {
        if *self.is_mobile.read() {
            Mode::Tiling
        } else {
            *self.mode.read()
        }
    }

    /// Class for the app root div: `"ws-root"`, `"ws-root mobile"` below
    /// the mobile breakpoint, or `"ws-root dragging"` while a move/resize/
    /// reorder drag is in flight (suppresses text selection under the
    /// sweeping pointer). The [`CSS`] stylesheet keys the whole shell off
    /// these classes.
    pub fn root_class(&self) -> &'static str {
        if *self.is_mobile.read() {
            "ws-root mobile"
        } else if self.drag.read().is_some() || self.tile_drag.read().is_some() {
            "ws-root dragging"
        } else {
            "ws-root"
        }
    }

    /// Start a floating-mode move/resize drag from a mousedown, capturing
    /// panel geometry into [`Drag`]. [`render`](Workspace::render) wires
    /// this up for the built-in header and resize handle; call it yourself
    /// only when adding extra drag affordances.
    pub fn begin_drag(&self, idx: usize, kind: DragKind, e: &MouseEvent) {
        // Stop the browser starting a text selection from this mousedown —
        // the .ws-root.dragging no-select class only applies from the next
        // render, after the Drag signal lands.
        e.prevent_default();
        let c = e.client_coordinates();
        // Normalize-on-grab: what the user grabbed is the *clamped* on-screen
        // rect (effective_rect), which can differ from the stored geometry
        // after a window shrink. Writing it back on grab keeps the drag math
        // anchored to what's visible — no jump on the first mousemove.
        let (vw, vh) = *self.viewport.read();
        let mut panels = self.panels;
        let d = core_begin_drag(&mut *panels.write(), idx, kind, c.x, c.y, vw, vh, &Clamp::WEB);
        if d.is_some() {
            let mut drag = self.drag;
            drag.set(d);
        }
    }

    /// Start a tiling-mode resize drag from the corner grip. Unlike
    /// [`begin_drag`](Workspace::begin_drag)'s free-pixel resize, pointer
    /// deltas snap the panel's tile spans ([`PanelWin::tile_w`] quarter-row
    /// units / [`PanelWin::tile_h`] rows) so tiles always land on fit sizes.
    /// `start_w`/`start_h` on the captured [`Drag`] hold the *spans*, not
    /// pixels; [`handle_mouse_move`](Workspace::handle_mouse_move) branches
    /// on the effective mode.
    pub fn begin_tile_resize(&self, idx: usize, e: &MouseEvent) {
        let c = e.client_coordinates();
        let d = core_begin_tile_resize(&*self.panels.read(), idx, c.x, c.y);
        if d.is_some() {
            let mut drag = self.drag;
            drag.set(d);
        }
    }

    /// Attach to the app root's `onmousemove` — applies the in-flight
    /// [`Drag`], if any (move follows the pointer; resize grows/shrinks,
    /// clamped to a minimum panel size).
    pub fn handle_mouse_move(&self, e: &MouseEvent) {
        if let Some(d) = *self.drag.read() {
            let c = e.client_coordinates();
            let tiling = self.effective_mode() == Mode::Tiling;
            let (vw, _) = *self.viewport.read();
            let mut panels = self.panels;
            apply_drag(
                &mut *panels.write(),
                &d,
                c.x,
                c.y,
                tiling,
                vw,
                &Clamp::WEB,
                &TileMetrics::WEB,
            );
        }
    }

    /// Attach to the app root's `onmouseup` — ends the in-flight drag (both
    /// the floating move/resize [`Drag`] and a tiling reorder drag), which
    /// also lets the settled layout persist.
    pub fn handle_mouse_up(&self) {
        let mut drag = self.drag;
        drag.set(None);
        let mut tile_drag = self.tile_drag;
        tile_drag.set(None);
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

        let dragging_tile = *self.tile_drag.read();
        rsx! {
            div { class: "{ws_class}",
                for i in visible.iter().copied() {
                    {
                        let p = ps[i];
                        let floating = maximized.is_none() && mode_now == Mode::Floating;
                        let tiling = maximized.is_none() && mode_now == Mode::Tiling;
                        let kind = p.kind;
                        let style = if maximized.is_some() {
                            "position:absolute; inset:0;".to_string()
                        } else if floating {
                            // Project through the viewport clamp at render
                            // time — stored geometry stays intact, so panels
                            // spring back when the window grows again.
                            let (vw, vh) = *ws.viewport.read();
                            let (x, y, w, h) = effective_rect(&p, vw, vh);
                            format!("position:absolute; left:{x}px; top:{y}px; width:{w}px; height:{h}px; z-index:{};",
                                p.z)
                        } else if tiling && !*ws.is_mobile.read() {
                            // Snapped spans → flex-basis % of the row + row
                            // height. Mobile keeps the pure-CSS single-column
                            // stack (no inline geometry).
                            format!(
                                "flex:1 1 calc({}% - 8px); height:{}px;",
                                p.tile_w as f64 * (100.0 / TILE_W_MAX as f64),
                                p.tile_h as f64 * TILE_ROW_PX
                            )
                        } else {
                            String::new()
                        };
                        let slug = kind_slug(p.kind.title());
                        let drag_cls = if dragging_tile == Some(kind) { " tile-dragging" } else { "" };
                        rsx! {
                            section {
                                // Keyed by kind (stable identity), not index:
                                // tiling reorders mutate the Vec mid-drag and
                                // index keys would remount every panel.
                                key: "{slug}",
                                class: "panel panel-{slug}{drag_cls}",
                                style: "{style}",
                                onmouseenter: move |_| {
                                    // Snap the dragged panel into this slot.
                                    if tiling {
                                        if let Some(d) = *ws.tile_drag.read() {
                                            if d != kind {
                                                ws.reorder_tile(d, kind);
                                            }
                                        }
                                    }
                                },
                                onmousedown: move |_| {
                                    // z-order only matters when panels can overlap (floating).
                                    // In tiling, mutating panels here re-renders and can swallow
                                    // clicks on panel content.
                                    if floating {
                                        let mut panels = ws.panels;
                                        let z = front_z(&panels.read());
                                        if let Some(pp) = panels.write().get_mut(i) { pp.z = z; };
                                    }
                                },
                                {ws.header(i, p.kind, floating, tiling)}
                                div { class: "panel-body",
                                    {body(p.kind, maximized == Some(i))}
                                }
                                if floating || (tiling && !*ws.is_mobile.read()) {
                                    div {
                                        class: "resize",
                                        onmousedown: move |e: MouseEvent| {
                                            if floating {
                                                ws.begin_drag(i, DragKind::Resize, &e);
                                            } else {
                                                ws.begin_tile_resize(i, &e);
                                            }
                                        },
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Panel chrome: traffic lights in printer-CMY (blue = floating⇄tiling,
    /// yellow = minimize, pink = maximize⇄restore; each reveals its action
    /// glyph on hover, macOS-style — the pink glyph flips between expand and
    /// restore with the panel's state) + the drag-to-move title strip. In
    /// floating mode the drag moves the window freely; in tiling mode it
    /// starts a reorder drag (hover another panel to snap into its slot).
    /// Mobile gets neither (static stack).
    fn header(&self, idx: usize, kind: K, draggable: bool, tiling: bool) -> Element {
        let ws = *self;
        let title = kind.title();
        let is_max = self.panels.read().get(idx).map(|p| p.state) == Some(WinState::Maximized);
        // Glyphs show the state a click switches TO: mode light floating → ⊞
        // (tile up), tiling → ❐ (cut the panels loose); max light normal → ⤢
        // (expand), maximized → ⤡ (restore).
        let mode_glyph = if *self.mode.read() == Mode::Tiling { "\u{2750}" } else { "\u{229E}" };
        let max_glyph = if is_max { "\u{2921}" } else { "\u{2922}" };
        rsx! {
            header {
                class: "panel-head",
                onmousedown: move |e: MouseEvent| {
                    if draggable {
                        ws.begin_drag(idx, DragKind::Move, &e);
                    } else if tiling && !*ws.is_mobile.read() {
                        // Selection has to be suppressed at the source too:
                        // .ws-root.dragging only kicks in after this event.
                        e.prevent_default();
                        let mut tile_drag = ws.tile_drag;
                        tile_drag.set(Some(kind));
                    }
                },
                div { class: "lights",
                    button { class: "light mode", title: "tiling / floating",
                        onmousedown: move |e: MouseEvent| e.stop_propagation(),
                        onclick: move |_| {
                            let mut mode = ws.mode;
                            let next = if *mode.read() == Mode::Tiling { Mode::Floating } else { Mode::Tiling };
                            mode.set(next);
                        },
                        span { class: "light-glyph", "{mode_glyph}" } }
                    button { class: "light yellow", title: "minimize",
                        onmousedown: move |e: MouseEvent| e.stop_propagation(),
                        onclick: move |_| {
                            let mut panels = ws.panels;
                            if let Some(p) = panels.write().get_mut(idx) { p.state = WinState::Minimized; };
                        },
                        span { class: "light-glyph", "\u{2212}" } }
                    button { class: "light max", title: "maximize / restore",
                        onmousedown: move |e: MouseEvent| e.stop_propagation(),
                        onclick: move |_| {
                            let mut panels = ws.panels;
                            if let Some(p) = panels.write().get_mut(idx) {
                                p.state = if p.state == WinState::Maximized { WinState::Floating } else { WinState::Maximized };
                            };
                        },
                        span { class: "light-glyph", "{max_glyph}" } }
                }
                span { class: "panel-title", "{title}" }
                if is_max { span { class: "max-hint", "maximized" } }
            }
        }
    }

    /// Restore and raise the panel of `kind`: un-minimizes it (the
    /// programmatic twin of a dock-chip click) and brings it to the front.
    /// No-op when the layout holds no panel of that kind. Hook for command
    /// palettes / keyboard shortcuts — the built-in chrome never calls it.
    ///
    /// ```no_run
    /// # use panel_kit::{PanelKind, Workspace};
    /// # fn jump<K: PanelKind>(ws: Workspace<K>, kind: K) {
    /// ws.restore(kind);
    /// # }
    /// ```
    pub fn restore(&self, kind: K) {
        let mut panels = self.panels;
        panel_kit_core::restore(&mut *panels.write(), kind);
    }

    /// The footer dock: minimized panels collapse to chips; click restores
    /// (and raises) the panel. Render it once after
    /// [`render`](Workspace::render) in the app root.
    pub fn dock(&self) -> Element {
        let ws = *self;
        let minimized: Vec<(usize, K)> = self
            .panels
            .read()
            .iter()
            .enumerate()
            .filter(|(_, p)| p.state == WinState::Minimized)
            .map(|(i, p)| (i, p.kind))
            .collect();
        rsx! {
            footer { class: "dock",
                span { class: "dock-label", "dock:" }
                if minimized.is_empty() {
                    span { class: "dock-empty", "— nothing minimized —" }
                }
                for (i, kind) in minimized.iter().copied() {
                    button {
                        key: "{i}",
                        class: "dock-chip",
                        onclick: move |_| {
                            let mut panels = ws.panels;
                            let z = front_z(&panels.read());
                            if let Some(p) = panels.write().get_mut(i) { p.state = WinState::Floating; p.z = z; };
                        },
                        "{kind.title()}"
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
    let vw = win.as_ref().and_then(|w| w.inner_width().ok()).and_then(|v| v.as_f64()).unwrap_or(1280.0);
    let vh = win.and_then(|w| w.inner_height().ok()).and_then(|v| v.as_f64()).unwrap_or(800.0);
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
