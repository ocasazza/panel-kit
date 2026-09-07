//! Workspace demo — exercises the full panel-kit workspace surface.
//!
//! Run with: `dx serve --example workspace --platform web`
//! (dioxus-cli 0.6.x; provided by `nix develop`)
//!
//! What it demonstrates:
//! - `use_workspace(storage_key, defaults)` with the storage key
//!   `panel_kit_example_workspace` — reload the page and the layout is
//!   restored from localStorage; the Reset button clears it.
//! - A demo `PanelKind` enum (`Panel`) with four panels; the Help panel
//!   starts `WinState::Minimized`, i.e. as a dock chip.
//! - `LayoutBuilder` for the default floating layout.
//! - `ws.render_with_header(body, header_actions)` with per-panel body and
//!   header-action closures receiving `(kind, maximized)`, plus `ws.dock()`,
//!   `ws.root_class()`, and the root
//!   `onmousemove`/`onmouseup` handlers (`handle_mouse_move`/`handle_mouse_up`).
//! - Floating mode: drag a panel header to move it, drag the bottom-right
//!   corner to resize, mousedown raises the panel (z-order), and the
//!   traffic lights minimize (yellow), maximize/restore (green), and
//!   toggle floating⇄tiling (red).
//! - Tiling mode: drag a header onto another panel to hover-snap reorder;
//!   the Status panel is full-width via its `panel-status` CSS class.
//! - Viewport clamping: shrink the browser window — floating panels are
//!   clamped on screen while their *stored* geometry (shown in the Status
//!   panel) is untouched, so they spring back when the window grows.
//! - `viewport_is_mobile` / the mobile stacked shell (window < 760px wide).
//! - The `is_editing` gate: pressing `t` toggles tiling⇄floating, but not
//!   while the Notes textarea has focus.
//! - `tip_pos`: a viewport-aware tooltip overlay — hover the ⓘ target in
//!   the Status panel near any screen edge and the tip stays on screen.

use dioxus::events::{Key, KeyboardEvent, MouseEvent};
use dioxus::prelude::*;
use gloo_storage::Storage;
use panel_kit::{
    is_editing, tip_pos, use_workspace, viewport_is_mobile, DragKind, LayoutBuilder, Mode,
    PanelHeaderButton, PanelKind, PanelWin, WinState, CSS,
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
}

impl PanelKind for Panel {
    fn title(self) -> &'static str {
        match self {
            Panel::Notes => "Notes",
            Panel::Preview => "Preview",
            Panel::Status => "Status",
            Panel::Help => "Help",
        }
    }
}

/// Default layout via `LayoutBuilder` (hands out incrementing z values).
/// Help starts minimized, so on first load it appears only as a dock chip.
fn default_layout() -> Vec<PanelWin<Panel>> {
    let mut b = LayoutBuilder::new();
    let mut help = b.at(Panel::Help, 660.0, 340.0, 380.0, 240.0);
    help.state = WinState::Minimized;
    vec![
        b.at(Panel::Notes, 16.0, 16.0, 420.0, 300.0),
        b.at(Panel::Preview, 452.0, 16.0, 420.0, 300.0),
        b.at(Panel::Status, 16.0, 332.0, 560.0, 280.0),
        help,
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

    let body = move |kind: Panel, maximized: bool| -> Element {
        match kind {
            Panel::Notes => rsx! {
                p { "Type below, then press " b { "t" } " — the mode shortcut is "
                    "suppressed by the " code { "is_editing()" } " gate while this "
                    "textarea has focus. Click elsewhere and " b { "t" } " toggles "
                    "tiling⇄floating." }
                textarea { class: "notes", placeholder: "type here…" }
            },
            Panel::Preview => rsx! {
                p { "The body closure receives " code { "(kind, maximized)" } "." }
                p { "This panel is currently "
                    b { if maximized { "maximized" } else { "not maximized" } }
                    " — press the green light to flip it." }
            },
            Panel::Status => {
                let header_count = *header_clicks.read();
                let (vw, vh) = *ws.viewport.read();
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
                        let st = match p.state {
                            WinState::Floating => "floating",
                            WinState::Minimized => "minimized",
                            WinState::Maximized => "maximized",
                        };
                        format!(
                            "{}: x={:.0} y={:.0} w={:.0} h={:.0} z={} ({})",
                            p.kind.title(),
                            p.x,
                            p.y,
                            p.w,
                            p.h,
                            p.z,
                            st
                        )
                    })
                    .collect();
                rsx! {
                    p {
                        "mode: " b { "{mode_label}" }
                        " · viewport: " b { "{vw:.0}×{vh:.0}" }
                        " · is_mobile: " b { "{ws.is_mobile.read()}" }
                        " · viewport_is_mobile(): " b { "{viewport_is_mobile()}" }
                    }
                    p { "drag: " b { "{drag_txt}" } " · tile drag: " b { "{tile_drag_txt}" } }
                    p { "header action clicks: " b { "{header_count}" } }
                    p { "stored geometry (clamping never rewrites it — shrink the "
                        "window and these numbers hold; grow it back and panels "
                        "spring back):" }
                    ul { class: "status-list",
                        for r in rows {
                            li { "{r}" }
                        }
                    }
                    p {
                        span {
                            class: "tip-target",
                            onmousemove: move |e: MouseEvent| {
                                let c = e.client_coordinates();
                                // 228×96 ≈ the .tip-overlay box; tip_pos keeps
                                // it inside the viewport near any edge.
                                tip.set(Some(tip_pos(c.x, c.y, 228.0, 96.0)));
                            },
                            onmouseleave: move |_| tip.set(None),
                            "ⓘ hover me for a tip_pos tooltip"
                        }
                        " — try it with the window scrolled so the cursor is "
                        "near the left or bottom edge."
                    }
                    p { "layout persists to localStorage under "
                        code { "{STORAGE_KEY}" } " — reload to verify." }
                }
            }
            Panel::Help => rsx! {
                p { b { "This panel started minimized" } " (a dock chip) via "
                    code { "WinState::Minimized" } " in the default layout." }
                ul { class: "status-list",
                    li { "red light: toggle floating⇄tiling" }
                    li { "yellow light: minimize to the dock" }
                    li { "green light: maximize / restore" }
                    li { "floating: drag the header to move, the corner to resize, "
                         "mousedown to raise (z-order)" }
                    li { "tiling: drag a header over another panel to reorder" }
                    li { "narrow the window under 760px for the mobile stack" }
                }
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
            onmousemove: move |e: MouseEvent| ws.handle_mouse_move(&e),
            onmouseup: move |_| ws.handle_mouse_up(),
            onkeydown: move |e: KeyboardEvent| {
                // `is_editing()` suppresses single-key shortcuts while an
                // input/textarea has focus.
                if is_editing() {
                    return;
                }
                if let Key::Character(c) = e.key() {
                    if c == "t" {
                        let mut mode = ws.mode;
                        let next = if *mode.read() == Mode::Tiling {
                            Mode::Floating
                        } else {
                            Mode::Tiling
                        };
                        mode.set(next);
                    }
                }
            },
            header { class: "topbar",
                h1 { "panel-kit workspace demo" }
                span { class: "hint", "mode: {mode_label} · press t to toggle · drag, resize, traffic lights" }
                button {
                    onclick: move |_| {
                        gloo_storage::LocalStorage::delete(STORAGE_KEY);
                        let mut panels = ws.panels;
                        panels.set(default_layout());
                        let mut mode = ws.mode;
                        mode.set(Mode::Floating);
                    },
                    "reset layout"
                }
            }
            {ws.render_with_header(body, header_actions)}
            {ws.dock()}
            if let Some((x, y)) = tip() {
                div { class: "tip-overlay", style: "left:{x}px; top:{y}px;",
                    b { "tip_pos in action" }
                    p { "Placed left of the cursor, flipped right when there's "
                        "no room, and clamped inside the viewport." }
                }
            }
        }
    }
}
