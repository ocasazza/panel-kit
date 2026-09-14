//! Workspace demo — exercises the full panel-kit workspace surface.
//!
//! Run with: `dx serve --example workspace --platform web`
//! (dioxus-cli 0.6.x; provided by `nix develop`)
//!
//! What it demonstrates:
//! - `use_workspace(storage_key, defaults)` with the storage key
//!   `panel_kit_example_workspace` — reload the page and the V2 layout is
//!   restored from localStorage; the Reset button clears it.
//! - A demo `PanelKind` enum (`Panel`) with five panels; the Help panel
//!   starts `WinState::Minimized`, i.e. as a dock chip.
//! - `LayoutBuilder` for the default floating layout and `with_tile` for
//!   explicit tiling spans.
//! - `ws.render_with_header(body, header_actions)` with per-panel body and
//!   header-action closures receiving `(kind, maximized)`, plus `ws.dock()`,
//!   `ws.root_class()`, and the root pointer handlers.
//! - Floating mode: drag a panel header to move it, drag the bottom-right
//!   corner to resize, pointer-down raises the panel (z-order), and the
//!   printer-CMY lights toggle mode (blue), minimize (yellow), and
//!   maximize/restore (pink). The scheme deliberately rejects the macOS
//!   red/yellow/green mapping because none of these controls destroys anything.
//! - Tiling mode: drag a header onto another panel to hover-snap reorder and
//!   drag a corner to change `tile_w`/`tile_h`; Status starts full-width.
//! - Viewport clamping: shrink the browser window — floating panels are
//!   clamped on screen while their stored geometry (shown in Status) remains
//!   unchanged, so they spring back when the window grows.
//! - Compact/tablet/regular `SurfaceProfile` tiers, live in Status.
//! - `Workspace::handle_key`: arrows move, Shift+arrows resize, Alt+arrows
//!   fine-move, m/f/t/Escape control state, Tab cycles focus, Enter raises.
//! - Wheel chaining: Preview deliberately overflows, and Below Fold sits
//!   beneath the viewport; scroll the panel body to its boundary, then keep
//!   scrolling to move the floating workspace.
//! - `Workspace::restore`, live focus/scroll/span state, and `tip_pos`.

use dioxus::events::PointerEvent as DioxusPointerEvent;
use dioxus::prelude::*;
use gloo_storage::Storage;
use panel_kit::{
    tip_pos, use_workspace, DragKind, LayoutBuilder, Mode, PanelHeaderButton, PanelKind, PanelWin,
    SurfaceClass, WinState, CSS,
};
use serde::{Deserialize, Serialize};

/// localStorage key the layout persists under (documented behavior:
/// reloading the page restores panel geometry, window states, and mode).
const STORAGE_KEY: &str = "panel_kit_example_workspace";

/// Demo styles layered after `panel_kit::CSS`. `.panel-status` shows the
/// per-panel slug class hook: one panel made full-width in tiling mode.
const DEMO_CSS: &str = "
.ws.tiling .panel-status { flex: 1 1 100%; }
.tip-target { border-bottom: 1px dashed var(--dim); cursor: help; }
.topbar button { background: var(--bg); color: var(--fg); border: 1px solid var(--line2);
  border-radius: 3px; padding: .15rem .5rem; font-size: .72rem; cursor: pointer; }
.topbar button:hover { border-color: var(--fg); }
.status-list { margin: .25rem 0; padding-left: 1.1rem; }
.status-list li { color: var(--dim); }
.overflow-probe { min-height: 560px; border-left: 1px solid var(--line);
  padding-left: .6rem; }
textarea.notes { width: 100%; height: 70%; background: var(--bg); color: var(--fg);
  border: 1px solid var(--line2); border-radius: 3px; font-family: var(--mono);
  font-size: .78rem; padding: .4rem; resize: none; }
";

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum Panel {
    Notes,
    Preview,
    Status,
    Help,
    BelowFold,
}

impl PanelKind for Panel {
    fn title(self) -> &'static str {
        match self {
            Panel::Notes => "Notes",
            Panel::Preview => "Preview",
            Panel::Status => "Status",
            Panel::Help => "Help",
            Panel::BelowFold => "Below Fold",
        }
    }
}

/// Default layout via `LayoutBuilder` (hands out incrementing z values).
/// Help starts minimized; Below Fold intentionally extends past the viewport.
fn default_layout() -> Vec<PanelWin<Panel>> {
    let mut b = LayoutBuilder::new();
    let mut help = b
        .at(Panel::Help, 660.0, 340.0, 380.0, 240.0)
        .with_tile(2, 2);
    help.state = WinState::Minimized;
    vec![
        b.at(Panel::Notes, 16.0, 16.0, 420.0, 300.0).with_tile(2, 2),
        b.at(Panel::Preview, 452.0, 16.0, 420.0, 300.0)
            .with_tile(2, 3),
        b.at(Panel::Status, 16.0, 332.0, 560.0, 280.0)
            .with_tile(4, 2),
        help,
        b.at(Panel::BelowFold, 180.0, 880.0, 460.0, 220.0)
            .with_tile(2, 2),
    ]
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let ws = use_workspace(STORAGE_KEY, default_layout);
    let mut header_clicks = use_signal(|| 0_u32);
    // Viewport-placed tooltip overlay, positioned through `tip_pos`.
    let mut tip = use_signal(|| Option::<(f64, f64)>::None);

    let mode_label = match ws.effective_mode() {
        Mode::Floating => "floating",
        Mode::Tiling => "tiling",
    };
    let profile = ws.surface_profile();
    let surface_label = match profile.class {
        SurfaceClass::Compact => "compact",
        SurfaceClass::Tablet => "tablet",
        SurfaceClass::Regular => "regular",
    };
    let coarse_pointer = profile.caps.coarse_pointer;
    let focused_label = ws.focused().map(|kind| kind.title()).unwrap_or("workspace");

    let body = move |kind: Panel, maximized: bool| -> Element {
        match kind {
            Panel::Notes => rsx! {
                p {
                    "Type below, then press " b { "t" } ". "
                    code { "Workspace::handle_key" }
                    " delegates text-input focus, so bare window-manager keys "
                    "do not steal keystrokes from this textarea."
                }
                textarea { class: "notes", placeholder: "type here…" }
            },
            Panel::Preview => rsx! {
                div { class: "overflow-probe",
                    p { "The body closure receives " code { "(kind, maximized)" } "." }
                    p { "This panel is currently "
                        b { if maximized { "maximized" } else { "not maximized" } }
                        " — use the pink light to maximize or restore it." }
                    p { b { "Wheel chaining probe:" } " this inner body is deliberately "
                        "taller than its panel. Scroll to its boundary, then keep "
                        "scrolling; the floating workspace takes over." }
                    for row in 1..=18 {
                        p { "overflow row {row}: panel-body scroll remains available" }
                    }
                }
            },
            Panel::Status => {
                let header_count = *header_clicks.read();
                let (vw, vh) = *ws.viewport.read();
                let ws_scroll = *ws.ws_scroll.read();
                let drag_txt = match *ws.drag.read() {
                    Some(d) => format!(
                        "{} panel #{}",
                        match d.kind {
                            DragKind::Move => "moving",
                            DragKind::Resize => "resizing",
                        },
                        d.idx
                    ),
                    None => "none".to_string(),
                };
                let tile_drag_txt = match *ws.tile_drag.read() {
                    Some(k) => format!("reordering “{}”", k.title()),
                    None => "none".to_string(),
                };
                let rows: Vec<String> = ws
                    .panels
                    .read()
                    .iter()
                    .map(|p| {
                        let state = match p.state {
                            WinState::Floating => "floating",
                            WinState::Minimized => "minimized",
                            WinState::Maximized => "maximized",
                        };
                        format!(
                            "{}: x={:.0} y={:.0} w={:.0} h={:.0} z={} tile={}×{} ({})",
                            p.kind.title(),
                            p.x,
                            p.y,
                            p.w,
                            p.h,
                            p.z,
                            p.tile_w,
                            p.tile_h,
                            state
                        )
                    })
                    .collect();
                rsx! {
                    p {
                        "mode: " b { "{mode_label}" }
                        " · surface: " b { "{surface_label}" }
                        " · viewport: " b { "{vw:.0}×{vh:.0}" }
                    }
                    p {
                        "focused: " b { "{focused_label}" }
                        " · ws_scroll: " b { "{ws_scroll:.0}px" }
                        " · coarse pointer: " b { "{coarse_pointer}" }
                    }
                    p {
                        "drag: " b { "{drag_txt}" }
                        " · tile drag: " b { "{tile_drag_txt}" }
                    }
                    p { "header action clicks: " b { "{header_count}" } }
                    p { "Stored geometry and tile_w/tile_h (drag a tiling corner "
                        "to change the spans):" }
                    ul { class: "status-list",
                        for row in rows {
                            li { "{row}" }
                        }
                    }
                    p {
                        span {
                            class: "tip-target",
                            onpointermove: move |event: DioxusPointerEvent| {
                                let coordinates = event.client_coordinates();
                                // 228×96 ≈ the .tip-overlay box; tip_pos keeps
                                // it inside the viewport near any edge.
                                tip.set(Some(tip_pos(
                                    coordinates.x,
                                    coordinates.y,
                                    228.0,
                                    96.0,
                                )));
                            },
                            onpointerleave: move |_| tip.set(None),
                            "ⓘ hover me for a tip_pos tooltip"
                        }
                        " — try it near the left or bottom edge."
                    }
                    p { "Layout persists as SavedLayoutV2 in localStorage under "
                        code { "{STORAGE_KEY}" } "; legacy V1 is migrated on load." }
                }
            }
            Panel::Help => rsx! {
                p { b { "This panel started minimized" } " via "
                    code { "WinState::Minimized" } ". The topbar's restore button "
                    "calls the public " code { "Workspace::restore" } " method." }
                ul { class: "status-list",
                    li { "blue light: toggle floating⇄tiling" }
                    li { "yellow light: minimize to the dock" }
                    li { "pink light: maximize / restore" }
                    li { "printer-CMY is deliberate: no operation destroys anything, "
                         "so the controls reject macOS red/yellow/green semantics" }
                    li { "floating: drag header/corner; arrows move; Shift+arrows resize" }
                    li { "tiling: drag a header to reorder and a corner to resize spans" }
                    li { "Tab / Shift+Tab cycles focused panels; Enter raises" }
                    li { "resize through compact, tablet, and regular surface tiers" }
                }
            },
            Panel::BelowFold => rsx! {
                p { b { "Workspace scroll layer reached." } }
                p { "This floating panel starts at y=880px, below the first fold. "
                    "Its presence extends the workspace scroll range without "
                    "rewriting stored geometry." }
                p { "Wheel Preview to its inner boundary, then continue to reach me." }
            },
        }
    };

    let header_actions = move |kind: Panel, _maximized: bool| -> Element {
        match kind {
            Panel::Status => rsx! {
                PanelHeaderButton {
                    label: "ping",
                    title: "Exercise a panel-owned header action",
                    active: *header_clicks.read() > 0,
                    on_press: move |_| *header_clicks.write() += 1,
                }
            },
            _ => rsx! {},
        }
    };

    rsx! {
        style { {CSS} }
        style { {DEMO_CSS} }
        div {
            class: ws.root_class(),
            tabindex: "0",
            onpointermove: move |event: DioxusPointerEvent| ws.handle_pointer_move(&event),
            onpointerup: move |event: DioxusPointerEvent| ws.handle_pointer_up(&event),
            onkeydown: move |event| ws.handle_key(&event),
            header { class: "topbar",
                h1 { "panel-kit workspace demo" }
                span { class: "hint",
                    "mode: {mode_label} · surface: {surface_label} · focused: {focused_label}"
                }
                button {
                    onclick: move |_| ws.restore(Panel::Help),
                    "restore Help by kind"
                }
                button {
                    onclick: move |_| {
                        gloo_storage::LocalStorage::delete(STORAGE_KEY);
                        let mut panels = ws.panels;
                        panels.set(default_layout());
                        let mut mode = ws.mode;
                        mode.set(Mode::Floating);
                        let mut focused = ws.focused;
                        focused.set(None);
                        let mut ws_scroll = ws.ws_scroll;
                        ws_scroll.set(0.0);
                    },
                    "reset layout"
                }
            }
            {ws.render_with_header(body, header_actions)}
            {ws.dock()}
            if let Some((x, y)) = tip() {
                div { class: "tip-overlay", style: "left:{x}px; top:{y}px;",
                    b { "tip_pos in action" }
                    p { "Placed left of the pointer, flipped right when there is "
                        "no room, and clamped inside the viewport." }
                }
            }
        }
    }
}
