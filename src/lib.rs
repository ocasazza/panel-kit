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
//! `--yellow`, `--green`, `--mono`) — override them in a later stylesheet to
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
use serde::{Deserialize, Serialize};

/// Base stylesheet for the workspace chrome (panels, lights, dock, badges,
/// spinner, tooltip overlay, mobile breakpoint). Inject once at the app root
/// with `style { {panel_kit::CSS} }`, then layer app-specific styles after
/// it; override the `:root` CSS variables to retheme (see the
/// [crate-level theming notes](crate#theming)).
pub const CSS: &str = include_str!("../assets/panel-kit.css");

/// The app's panel identifier — typically a fieldless enum.
///
/// This is the only trait an app must implement to use the workspace: one
/// variant per panel, plus a [`title`](PanelKind::title). The serde bounds
/// exist because the layout (including each panel's kind) persists to
/// localStorage; the `Copy + Eq + Hash` bounds let the workspace use kinds
/// as cheap, stable panel identities across reorders and reloads.
///
/// # Examples
///
/// ```no_run
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
/// enum Panel { Graph, Inspector }
///
/// impl panel_kit::PanelKind for Panel {
///     fn title(self) -> &'static str {
///         match self {
///             Panel::Graph => "Graph",
///             Panel::Inspector => "Inspector",
///         }
///     }
/// }
/// ```
pub trait PanelKind:
    Copy + PartialEq + Eq + std::hash::Hash + Serialize + serde::de::DeserializeOwned + 'static
{
    /// Human-readable panel title, shown in the panel header and its dock
    /// chip. Also slugified into the panel's `panel-<slug>` CSS class
    /// (`"Filter Strip"` → `panel-filter-strip`), so it should be stable.
    fn title(self) -> &'static str;
}

/// CSS-safe slug of a panel title ("Filter Strip" -> "filter-strip"). Every
/// panel section gets `panel panel-<slug>` so apps can style individual
/// panels — e.g. making one panel full-width in tiling mode.
fn kind_slug(title: &str) -> String {
    title
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect()
}

/// Per-panel window state, cycled by the traffic lights.
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum WinState {
    /// Normal: shown at its stored geometry (floating mode) or in its grid
    /// slot (tiling mode).
    Floating,
    /// Collapsed into a chip on the [dock](Workspace::dock); clicking the
    /// chip restores the panel.
    Minimized,
    /// Fills the whole workspace area, hiding every other panel. The green
    /// light restores it.
    Maximized,
}

/// Tiling-mode row height unit in px — [`PanelWin::tile_h`] counts these.
pub const TILE_ROW_PX: f64 = 150.0;
/// Width span ceiling (quarter-row units).
const TILE_W_MAX: u8 = 4;
/// Height span ceiling ([`TILE_ROW_PX`] rows).
const TILE_H_MAX: u8 = 6;

fn default_tile_w() -> u8 {
    1
}
fn default_tile_h() -> u8 {
    2
}

/// One panel's geometry + window state. `z` is the floating-mode stacking order.
///
/// Stored geometry is the user's *intent*: when the browser window shrinks,
/// panels are clamped on screen at render time but the stored rect is left
/// untouched, so they spring back when the window grows. Build defaults with
/// [`LayoutBuilder`].
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PanelWin<K> {
    /// Which panel this is (the app's [`PanelKind`]).
    pub kind: K,
    /// Left edge in px, relative to the workspace area.
    pub x: f64,
    /// Top edge in px, relative to the workspace area.
    pub y: f64,
    /// Width in px.
    pub w: f64,
    /// Height in px.
    pub h: f64,
    /// Window state (normal / minimized to dock / maximized).
    pub state: WinState,
    /// Floating-mode stacking order; higher is in front. Mousedown on a
    /// panel raises it to the front.
    pub z: i32,
    /// Tiling-mode width span in quarter-row units (1–4): 1 ≈ a quarter of
    /// the row, 4 = the full row. The tiling resize grip snaps this;
    /// floating geometry (`w`/`h`) is untouched. Layouts saved before this
    /// field existed deserialize to 1.
    #[serde(default = "default_tile_w")]
    pub tile_w: u8,
    /// Tiling-mode height in [`TILE_ROW_PX`] rows (1–6). Defaults to 2
    /// (300 px — the pre-resize fixed tile height).
    #[serde(default = "default_tile_h")]
    pub tile_h: u8,
}

impl<K> PanelWin<K> {
    /// Builder-style override of the tiling spans, clamped to their valid
    /// ranges (width 1–4 quarter-row units, height 1–6 rows). Chain it off
    /// [`LayoutBuilder::at`] for panels that should start bigger in tiling
    /// mode — e.g. a full-width main view: `b.at(...).with_tile(4, 3)`.
    pub fn with_tile(mut self, w: u8, h: u8) -> Self {
        self.tile_w = w.clamp(1, TILE_W_MAX);
        self.tile_h = h.clamp(1, TILE_H_MAX);
        self
    }
}

/// Workspace layout mode, toggled by the red traffic light.
///
/// A narrow viewport forces tiling regardless of this setting — see
/// [`Workspace::effective_mode`].
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Mode {
    /// Free placement: panels are absolutely positioned, draggable,
    /// resizable, and overlap by z-order.
    Floating,
    /// Auto grid: panels flow into a CSS grid in `Vec` order; dragging a
    /// panel header over another panel reorders them.
    Tiling,
}

/// What an in-flight floating-mode drag is doing.
#[derive(Clone, Copy, PartialEq)]
pub enum DragKind {
    /// Dragging the panel header: the panel follows the pointer.
    Move,
    /// Dragging the bottom-right resize handle: the panel grows/shrinks.
    Resize,
}

/// An in-flight floating-mode drag: which panel, what kind, and the mouse +
/// panel geometry captured at mousedown (deltas are applied against these).
///
/// Created by [`Workspace::begin_drag`], applied by
/// [`Workspace::handle_mouse_move`], cleared by
/// [`Workspace::handle_mouse_up`].
#[derive(Clone, Copy, PartialEq)]
pub struct Drag {
    /// Index of the dragged panel in [`Workspace::panels`].
    pub idx: usize,
    /// Move or resize.
    pub kind: DragKind,
    /// Mouse x at mousedown (client coordinates).
    pub start_mx: f64,
    /// Mouse y at mousedown (client coordinates).
    pub start_my: f64,
    /// Panel x at mousedown.
    pub start_x: f64,
    /// Panel y at mousedown.
    pub start_y: f64,
    /// Panel width at mousedown.
    pub start_w: f64,
    /// Panel height at mousedown.
    pub start_h: f64,
}

/// Convenience builder for default layouts: hands out incrementing z values.
///
/// # Examples
///
/// ```no_run
/// # use serde::{Deserialize, Serialize};
/// # #[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
/// # enum Panel { Graph, Inspector }
/// use panel_kit::{LayoutBuilder, PanelWin};
///
/// fn default_layout() -> Vec<PanelWin<Panel>> {
///     let mut b = LayoutBuilder::new();
///     vec![
///         b.at(Panel::Graph, 16.0, 16.0, 520.0, 360.0),
///         b.at(Panel::Inspector, 560.0, 16.0, 320.0, 360.0),
///     ]
/// }
/// ```
pub struct LayoutBuilder {
    z: i32,
}

impl LayoutBuilder {
    /// Start a builder; the first panel gets `z = 1`.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self { z: 0 }
    }

    /// A [`PanelWin`] at the given rect, in [`WinState::Floating`], with the
    /// next z value — so later panels stack in front of earlier ones.
    pub fn at<K>(&mut self, kind: K, x: f64, y: f64, w: f64, h: f64) -> PanelWin<K> {
        self.z += 1;
        PanelWin {
            kind,
            x,
            y,
            w,
            h,
            state: WinState::Floating,
            z: self.z,
            tile_w: default_tile_w(),
            tile_h: default_tile_h(),
        }
    }
}

fn front_z<K>(ps: &[PanelWin<K>]) -> i32 {
    ps.iter().map(|p| p.z).max().unwrap_or(0) + 1
}

fn viewport_size() -> (f64, f64) {
    let win = web_sys::window();
    let vw = win.as_ref().and_then(|w| w.inner_width().ok()).and_then(|v| v.as_f64()).unwrap_or(1280.0);
    let vh = win.and_then(|w| w.inner_height().ok()).and_then(|v| v.as_f64()).unwrap_or(800.0);
    (vw, vh)
}

/// The on-screen geometry for a floating panel: shrunk if larger than the
/// viewport, pulled back so the whole panel stays visible.
///
/// This is a render-time projection — the stored `PanelWin` keeps the
/// user's intended geometry untouched. Mutating state here instead (the
/// original behavior) made clamping one-way: shrink the window once and
/// every panel stayed crushed after the window grew back.
fn effective_rect<K>(p: &PanelWin<K>, vw: f64, vh: f64) -> (f64, f64, f64, f64) {
    let ws_w = (vw - 4.0).max(220.0);
    let ws_h = (vh - 66.0).max(180.0); // minus topbar (~36) + dock (~30)
    let w = p.w.min(ws_w - 12.0).max(180.0);
    let h = p.h.min(ws_h - 12.0).max(110.0);
    let x = p.x.min(ws_w - w - 6.0).max(0.0);
    let y = p.y.min(ws_h - h - 6.0).max(0.0);
    (x, y, w, h)
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

#[derive(Serialize, Deserialize)]
struct SavedLayout<K> {
    panels: Vec<PanelWin<K>>,
    tiling: bool,
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
    for d in defaults {
        if !panels.iter().any(|p| p.kind == d.kind) {
            panels.push(*d);
        }
    }
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
        let cb = Closure::wrap(Box::new(move |_e: web_sys::Event| {
            // Stored panel geometry is never touched on resize — render()
            // re-projects it through effective_rect from this signal.
            viewport.set(viewport_size());
            is_mobile.set(viewport_is_mobile());
        }) as Box<dyn FnMut(web_sys::Event)>);
        if let Some(w) = web_sys::window() {
            let _ = w.add_event_listener_with_callback("resize", cb.as_ref().unchecked_ref());
        }
        cb.forget();
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

    /// Class for the app root div: `"ws-root"`, or `"ws-root mobile"` below
    /// the mobile breakpoint. The [`CSS`] stylesheet keys the whole shell
    /// off these classes.
    pub fn root_class(&self) -> &'static str {
        if *self.is_mobile.read() {
            "ws-root mobile"
        } else {
            "ws-root"
        }
    }

    /// Start a floating-mode move/resize drag from a mousedown, capturing
    /// panel geometry into [`Drag`]. [`render`](Workspace::render) wires
    /// this up for the built-in header and resize handle; call it yourself
    /// only when adding extra drag affordances.
    pub fn begin_drag(&self, idx: usize, kind: DragKind, e: &MouseEvent) {
        let c = e.client_coordinates();
        // Normalize-on-grab: what the user grabbed is the *clamped* on-screen
        // rect (effective_rect), which can differ from the stored geometry
        // after a window shrink. Writing it back on grab keeps the drag math
        // anchored to what's visible — no jump on the first mousemove.
        let (vw, vh) = *self.viewport.read();
        let mut panels = self.panels;
        let p = {
            let mut ps = panels.write();
            let Some(p) = ps.get_mut(idx) else { return };
            let (x, y, w, h) = effective_rect(p, vw, vh);
            (p.x, p.y, p.w, p.h) = (x, y, w, h);
            *p
        };
        let mut drag = self.drag;
        drag.set(Some(Drag {
            idx,
            kind,
            start_mx: c.x,
            start_my: c.y,
            start_x: p.x,
            start_y: p.y,
            start_w: p.w,
            start_h: p.h,
        }));
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
        let (tw, th) = {
            let ps = self.panels.read();
            let Some(p) = ps.get(idx) else { return };
            (p.tile_w as f64, p.tile_h as f64)
        };
        let mut drag = self.drag;
        drag.set(Some(Drag {
            idx,
            kind: DragKind::Resize,
            start_mx: c.x,
            start_my: c.y,
            start_x: 0.0,
            start_y: 0.0,
            start_w: tw,
            start_h: th,
        }));
    }

    /// Attach to the app root's `onmousemove` — applies the in-flight
    /// [`Drag`], if any (move follows the pointer; resize grows/shrinks,
    /// clamped to a minimum panel size).
    pub fn handle_mouse_move(&self, e: &MouseEvent) {
        if let Some(d) = *self.drag.read() {
            let c = e.client_coordinates();
            let mut panels = self.panels;
            let mut ps = panels.write();
            if let Some(p) = ps.get_mut(d.idx) {
                match d.kind {
                    DragKind::Move => {
                        p.x = (d.start_x + (c.x - d.start_mx)).max(0.0);
                        p.y = (d.start_y + (c.y - d.start_my)).max(0.0);
                    }
                    // Tiling resize snaps to fit sizes: pointer deltas count
                    // in quarter-row width steps and TILE_ROW_PX height rows
                    // against the spans captured at mousedown (start_w/h
                    // hold spans, not px — see begin_tile_resize).
                    DragKind::Resize if self.effective_mode() == Mode::Tiling => {
                        let (vw, _) = *self.viewport.read();
                        let col = ((vw - 16.0) / TILE_W_MAX as f64).max(80.0);
                        let dw = ((c.x - d.start_mx) / col).round();
                        let dh = ((c.y - d.start_my) / TILE_ROW_PX).round();
                        p.tile_w = (d.start_w + dw).clamp(1.0, TILE_W_MAX as f64) as u8;
                        p.tile_h = (d.start_h + dh).clamp(1.0, TILE_H_MAX as f64) as u8;
                    }
                    DragKind::Resize => {
                        p.w = (d.start_w + (c.x - d.start_mx)).max(180.0);
                        p.h = (d.start_h + (c.y - d.start_my)).max(110.0);
                    }
                }
            }
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
        let mut ps = panels.write();
        let Some(from) = ps.iter().position(|p| p.kind == dragged) else { return };
        let Some(to) = ps.iter().position(|p| p.kind == target) else { return };
        if from == to {
            return;
        }
        let p = ps.remove(from);
        let after_removal = if from < to { to - 1 } else { to };
        let insert_at = (if from < to { after_removal + 1 } else { after_removal }).min(ps.len());
        ps.insert(insert_at, p);
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

    /// Panel chrome: traffic lights (red = floating⇄tiling, yellow = minimize,
    /// green = maximize⇄restore) + the drag-to-move title strip. In floating
    /// mode the drag moves the window freely; in tiling mode it starts a
    /// reorder drag (hover another panel to snap into its slot). Mobile gets
    /// neither (static stack).
    fn header(&self, idx: usize, kind: K, draggable: bool, tiling: bool) -> Element {
        let ws = *self;
        let title = kind.title();
        let is_max = self.panels.read().get(idx).map(|p| p.state) == Some(WinState::Maximized);
        rsx! {
            header {
                class: "panel-head",
                onmousedown: move |e: MouseEvent| {
                    if draggable {
                        ws.begin_drag(idx, DragKind::Move, &e);
                    } else if tiling && !*ws.is_mobile.read() {
                        let mut tile_drag = ws.tile_drag;
                        tile_drag.set(Some(kind));
                    }
                },
                div { class: "lights",
                    button { class: "light red", title: "tiling / floating",
                        onmousedown: move |e: MouseEvent| e.stop_propagation(),
                        onclick: move |_| {
                            let mut mode = ws.mode;
                            let next = if *mode.read() == Mode::Tiling { Mode::Floating } else { Mode::Tiling };
                            mode.set(next);
                        } }
                    button { class: "light yellow", title: "minimize",
                        onmousedown: move |e: MouseEvent| e.stop_propagation(),
                        onclick: move |_| {
                            let mut panels = ws.panels;
                            if let Some(p) = panels.write().get_mut(idx) { p.state = WinState::Minimized; };
                        } }
                    button { class: "light green", title: "maximize / restore",
                        onmousedown: move |e: MouseEvent| e.stop_propagation(),
                        onclick: move |_| {
                            let mut panels = ws.panels;
                            if let Some(p) = panels.write().get_mut(idx) {
                                p.state = if p.state == WinState::Maximized { WinState::Floating } else { WinState::Maximized };
                            };
                        } }
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
        let z = front_z(&panels.read());
        let idx = panels.read().iter().position(|p| p.kind == kind);
        if let Some(i) = idx {
            if let Some(p) = panels.write().get_mut(i) {
                if p.state == WinState::Minimized {
                    p.state = WinState::Floating;
                }
                p.z = z;
            }
        }
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
