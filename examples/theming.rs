//! Theming demo — retheme panel-kit by overriding the `:root` CSS variables.
//!
//! Run with: `dx serve --example theming --platform web`
//! (dioxus-cli 0.6.x; provided by `nix develop`)
//!
//! What it demonstrates:
//! - The documented theming path: inject `panel_kit::CSS` once, then layer
//!   a stylesheet after it that overrides the `:root` role variables
//!   (`--color-background`, `--color-surface`, `--color-text`,
//!   `--color-text-muted`, `--color-divider`, `--color-border`,
//!   `--color-inverse-background`, `--color-inverse-text`,
//!   `--color-focus-ring`, `--color-primary`, `--color-secondary`,
//!   `--color-tertiary`, `--color-highlight`, `--color-success`,
//!   `--color-failure`, `--color-error`, `--color-warning`, `--color-info`,
//!   and `--mono`). Every role takes any value; no two need to match.
//! - Four presets switchable at runtime: the built-in dark default, a
//!   light "paper" theme, a CRT phosphor theme (which also swaps `--mono`
//!   to show the font variable), and a role remap that assigns a different
//!   value to each brand and status role on top of the dark neutrals.
//! - The whole chrome restyles: workspace panels, traffic lights
//!   (primary mode, tertiary minimize, secondary maximize), dock, badges,
//!   spinner, and the role chips in the Swatches panel.

use dioxus::prelude::*;
use panel_kit::badge::{Badge, BadgeAction, BadgeKind, BadgeSpec};
use panel_kit::{LayoutBuilder, PanelKind, PanelWin, Spinner, CSS};
use panel_kit_core::frame::Placement;
use panel_kit_core::persist::SavePolicy;
use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[path = "support/composable_workspace.rs"]
mod composable_workspace;

const DEMO_CSS: &str = "
.theme-pick { display: flex; gap: .4rem; margin-left: auto; }
.theme-pick button { background: var(--color-background); color: var(--color-text-muted);
  border: 1px solid var(--color-border);
  border-radius: 3px; padding: .15rem .5rem; font-size: .72rem; cursor: pointer; }
.theme-pick button.on { color: var(--color-text); border-color: var(--color-highlight); }
.swatches { display: flex; flex-wrap: wrap; gap: .5rem; align-items: center; }
.roles { display: flex; flex-wrap: wrap; gap: .35rem; margin-top: .6rem; }
.role-chip { display: inline-flex; align-items: center; gap: .35rem; font-size: .68rem;
  color: var(--color-text-muted); }
.role-chip i { width: .9rem; height: .9rem; border-radius: 2px;
  border: 1px solid var(--color-border); background: var(--chip, var(--color-text)); }
";
/// localStorage key retained for saved-layout compatibility.
///
/// Stable `Panel` serde IDs are unchanged: `Swatches`, `About`.
const STORAGE_KEY: &str = "panel_kit_example_theming";

/// Every color role, overridden to a warm light palette.
///
/// WCAG sRGB contrast-script ratios, foreground on `--color-surface`:
/// text 16.54:1, text-muted 5.37:1, highlight 5.16:1, error 5.34:1, warning and
/// tertiary 4.19:1, success 5.14:1, primary 5.80:1, secondary 5.14:1, info
/// 5.36:1; border edge 3.001:1; inverted pair 15.43:1. The 1.07:1
/// background/surface tonal step and 1.33:1 divider/background seam are
/// deliberately structural, non-text layers; the independently passing border
/// edge carries the bound.
const THEME_PAPER: &str = ":root {
  --color-background:         #f4f1ea;
  --color-surface:            #fbf9f4;
  --color-text:               #1a1a1a;
  --color-text-muted:         #6e665c;
  --color-divider:            #d8d2c6;
  --color-border:             #99907f;
  --color-inverse-background: #1a1a1a;
  --color-inverse-text:       #f4f1ea;
  --color-focus-ring:         #1a1a1a;
  --color-primary:            #1f5ec2;
  --color-secondary:          #c21f8e;
  --color-tertiary:           #a06f00;
  --color-highlight:          #0c7a3d;
  --color-success:            #1d7a33;
  --color-failure:            #c62828;
  --color-error:              #c62828;
  --color-warning:            #a06f00;
  --color-info:               #2e6e8c;
}";

/// Green-phosphor CRT — also swaps `--mono` to prove the font variable.
///
/// WCAG sRGB contrast-script ratios, foreground on `--color-surface`:
/// text 10.44:1, text-muted 5.98:1, highlight 14.87:1, error 6.49:1, warning and
/// tertiary 11.74:1, success 10.44:1, primary 6.90:1, secondary 7.60:1, info
/// 7.90:1; border edge 3.060:1; inverted pair 11.06:1. The 1.06:1
/// background/surface tonal step and 1.37:1 divider/background seam have the
/// same structural, non-text role as the paper preset.
const THEME_CRT: &str = ":root {
  --color-background:         #041204;
  --color-surface:            #061a06;
  --color-text:               #4ee04e;
  --color-text-muted:         #44a844;
  --color-divider:            #0c330c;
  --color-border:             #327132;
  --color-inverse-background: #4ee04e;
  --color-inverse-text:       #041204;
  --color-focus-ring:         #4ee04e;
  --color-primary:            #62a0ff;
  --color-secondary:          #ff79c6;
  --color-tertiary:           #e0d34e;
  --color-highlight:          #9dff9d;
  --color-success:            #4ee04e;
  --color-failure:            #ff6b5f;
  --color-error:              #ff6b5f;
  --color-warning:            #e0d34e;
  --color-info:               #72b5c7;
  --mono: 'Courier New', 'Courier', monospace;
}";

/// Role remap on the dark neutrals: every brand and status role gets its own
/// value, including `failure` != `error` and `tertiary` != `warning`.
const THEME_REMAP: &str = ":root {
  --color-primary:   #ff8a3d;
  --color-secondary: #2ee6c8;
  --color-tertiary:  #b388ff;
  --color-highlight: #ffe14d;
  --color-success:   #7bd88f;
  --color-failure:   #ff4d8d;
  --color-error:     #ff7a59;
  --color-warning:   #ffd166;
  --color-info:      #9db4ff;
}";

/// The roles shown as chips in the Swatches panel (role key, CSS variable).
const ROLE_CHIPS: [(&str, &str); 9] = [
    ("primary", "--color-primary"),
    ("secondary", "--color-secondary"),
    ("tertiary", "--color-tertiary"),
    ("highlight", "--color-highlight"),
    ("success", "--color-success"),
    ("failure", "--color-failure"),
    ("error", "--color-error"),
    ("warning", "--color-warning"),
    ("info", "--color-info"),
];

#[derive(Clone, Copy, PartialEq)]
enum Theme {
    Default,
    Paper,
    Crt,
    Remap,
}

impl Theme {
    fn label(self) -> &'static str {
        match self {
            Theme::Default => "dark (built-in)",
            Theme::Paper => "paper",
            Theme::Crt => "crt",
            Theme::Remap => "role remap",
        }
    }
    /// The `:root` override block layered after `panel_kit::CSS`.
    fn css(self) -> &'static str {
        match self {
            Theme::Default => "", // no overrides — the library's own palette
            Theme::Paper => THEME_PAPER,
            Theme::Crt => THEME_CRT,
            Theme::Remap => THEME_REMAP,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum Panel {
    Swatches,
    About,
}

impl PanelKind for Panel {
    fn title(self) -> &'static str {
        match self {
            Panel::Swatches => "Swatches",
            Panel::About => "About",
        }
    }
}

fn default_layout() -> Vec<PanelWin<Panel>> {
    let mut b = LayoutBuilder::new();
    vec![
        b.at(Panel::Swatches, 16.0, 16.0, 520.0, 300.0),
        b.at(Panel::About, 556.0, 16.0, 380.0, 300.0),
    ]
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let workspace =
        composable_workspace::use_demo_workspace(STORAGE_KEY, default_layout, SavePolicy::OnSettle);
    composable_workspace::mount_viewport_observer(&workspace);
    let emit = composable_workspace::workspace_event_handler(&workspace);
    let mut theme = use_signal(|| Theme::Default);
    let theme_css = theme().css();

    let body = move |kind: Panel, _maximized: bool| -> Element {
        match kind {
            Panel::Swatches => rsx! {
                p { "Badges and the spinner pick the variables up too:" }
                div { class: "swatches",
                    Badge {
                        spec: BadgeSpec { active: true, ..BadgeSpec::new("tag", "highlight", BadgeKind::Tag) },
                        on_action: move |_: BadgeAction| {},
                    }
                    Badge {
                        spec: BadgeSpec::new("status", "success", BadgeKind::Status),
                        on_action: move |_: BadgeAction| {},
                    }
                    Badge {
                        spec: BadgeSpec::new("entity", "tertiary", BadgeKind::Entity { ty: None }),
                        on_action: move |_: BadgeAction| {},
                    }
                    Badge {
                        spec: BadgeSpec::new("link", "error (unresolved)", BadgeKind::Wikilink { resolved: false, target: "x".to_string() }),
                        on_action: move |_: BadgeAction| {},
                    }
                    Spinner { label: "spinning in theme colours" }
                }
                div { class: "roles",
                    for (key, var) in ROLE_CHIPS {
                        span { class: "role-chip",
                            i { style: "--chip: var({var});" }
                            "{key}"
                        }
                    }
                }
            },
            Panel::About => rsx! {
                p { "The stylesheet order is the whole mechanism:" }
                ol {
                    li { code { "style {{ {{panel_kit::CSS}} }}" } " — the library chrome" }
                    li { code { ":root {{ --color-primary: …; --color-highlight: …; }}" }
                        " — your overrides, injected after it" }
                }
                p { "Switch presets in the top bar. The CRT preset also overrides "
                    code { "--mono" } ", the font stack variable. The role remap preset "
                    "gives every brand and status role its own value, so " code { "failure" }
                    " and " code { "error" } ", and " code { "tertiary" } " and "
                    code { "warning" } ", no longer match." }
            },
        }
    };

    let snapshot = workspace.snapshot.read();
    let mut scratch = workspace.scratch.borrow_mut();
    let frame = composable_workspace::project_workspace(&snapshot, &mut scratch);
    let root_class = panel_kit::widgets::root::root_class(&frame);
    let workspace_class = composable_workspace::workspace_area_class(&frame);
    let workspace_style = frame
        .tile_grid
        .map(panel_kit::widgets::root::tile_grid_style)
        .unwrap_or_default();
    let pointer_move_workspace = workspace.clone();
    let pointer_up_workspace = workspace.clone();
    let pointer_cancel_workspace = workspace.clone();
    let key_workspace = workspace.clone();
    let wheel_workspace = workspace.clone();

    rsx! {
        style { {CSS} }
        style { {DEMO_CSS} }
        style { {theme_css} }
        div {
            class: "{root_class}",
            tabindex: "0",
            onpointermove: move |event| composable_workspace::handle_pointer_move(&pointer_move_workspace, &event),
            onpointerup: move |event| composable_workspace::handle_pointer_up(&pointer_up_workspace, &event),
            onpointercancel: move |event| composable_workspace::handle_pointer_up(&pointer_cancel_workspace, &event),
            onkeydown: move |event: KeyboardEvent| composable_workspace::handle_key(&key_workspace, &event),
            header { class: "topbar",
                h1 { "panel-kit theming demo" }
                div { class: "theme-pick",
                    for t in [Theme::Default, Theme::Paper, Theme::Crt, Theme::Remap] {
                        button {
                            class: if theme() == t { "on" } else { "" },
                            onclick: move |_| theme.set(t),
                            {t.label()}
                        }
                    }
                }
            }
            div {
                class: "{workspace_class}",
                style: "{workspace_style}",
                onwheel: move |event| composable_workspace::handle_wheel(&wheel_workspace, &event),
                for panel in frame.panels.iter().copied() {
                    if let Some(meta) = workspace.catalog.get(panel.key) {
                        {
                            let panel_class = format!("panel-{}", meta.slug);
                            let maximized = matches!(panel.placement, Placement::Maximized);
                            panel_kit::widgets::panel::panel_shell(panel, Some(&panel_class), rsx! {
                                {panel_kit::widgets::panel::panel_chrome_with_events(
                                    panel,
                                    meta,
                                    emit,
                                    Some(panel_kit::widgets::panel::traffic_lights(panel, emit)),
                                    None,
                                )}
                                {panel_kit::widgets::panel::panel_body(body(panel.key, maximized))}
                                {panel_kit::widgets::panel::resize_grip(panel, emit)}
                            })
                        }
                    }
                }
            }
            {panel_kit::widgets::dock::dock(frame.dock, &workspace.catalog, emit, None)}
        }
    }
}
