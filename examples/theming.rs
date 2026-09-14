//! Theming demo — retheme panel-kit by overriding the `:root` CSS variables.
//!
//! Run with: `dx serve --example theming --platform web`
//! (dioxus-cli 0.6.x; provided by `nix develop`)
//!
//! What it demonstrates:
//! - The documented theming path: inject `panel_kit::CSS` once, then layer
//!   a stylesheet after it that overrides the full `:root` palette (`--bg`,
//!   `--panel`, `--fg`, `--dim`, `--line`, `--line2`, `--inv-bg`,
//!   `--inv-fg`, `--accent`, `--red`, `--yellow`, `--green`, `--blue`,
//!   `--pink`, `--badge-info`, and `--mono`).
//! - Three presets switchable at runtime: the built-in dark default, a
//!   light "paper" theme, and a CRT phosphor theme (which also swaps
//!   `--mono` to show the font variable).
//! - The whole chrome restyles: workspace panels, printer-CMY traffic lights
//!   (blue mode, yellow minimize, pink maximize), dock, badges, and spinner.

use dioxus::prelude::*;
use panel_kit::badge::{Badge, BadgeAction, BadgeKind};
use panel_kit::{use_workspace, LayoutBuilder, PanelKind, PanelWin, Spinner, CSS};
use serde::{Deserialize, Serialize};

const DEMO_CSS: &str = "
.theme-pick { display: flex; gap: .4rem; margin-left: auto; }
.theme-pick button { background: var(--bg); color: var(--dim); border: 1px solid var(--line2);
  border-radius: 3px; padding: .15rem .5rem; font-size: .72rem; cursor: pointer; }
.theme-pick button.on { color: var(--fg); border-color: var(--accent); }
.swatches { display: flex; flex-wrap: wrap; gap: .5rem; align-items: center; }
";

/// Every color variable, overridden to a warm light palette.
///
/// WCAG sRGB contrast-script ratios, foreground on `--panel`:
/// fg 16.54:1, dim 5.37:1, accent 5.16:1, red 5.34:1, yellow
/// 4.19:1, green 5.14:1, blue 5.80:1, pink 5.14:1, badge-info
/// 5.36:1; line2 edge 3.001:1; inverted pair 15.43:1. The 1.07:1
/// bg/panel tonal step and 1.33:1 line/bg seam are deliberately structural,
/// non-text layers; the independently passing line2 edge carries the bound.
const THEME_PAPER: &str = ":root {
  --bg:        #f4f1ea;
  --panel:     #fbf9f4;
  --fg:        #1a1a1a;
  --dim:       #6e665c;
  --line:      #d8d2c6;
  --line2:     #99907f;
  --inv-bg:    #1a1a1a;
  --inv-fg:    #f4f1ea;
  --accent:    #0c7a3d;
  --red:       #c62828;
  --yellow:    #a06f00;
  --green:     #1d7a33;
  --blue:      #1f5ec2;
  --pink:      #c21f8e;
  --badge-info:#2e6e8c;
}";

/// Green-phosphor CRT — also swaps `--mono` to prove the font variable.
///
/// WCAG sRGB contrast-script ratios, foreground on `--panel`:
/// fg 10.44:1, dim 5.98:1, accent 14.87:1, red 6.49:1, yellow
/// 11.74:1, green 10.44:1, blue 6.90:1, pink 7.60:1, badge-info
/// 7.90:1; line2 edge 3.060:1; inverted pair 11.06:1. The 1.06:1
/// bg/panel tonal step and 1.37:1 line/bg seam have the same structural,
/// non-text role as the paper preset.
const THEME_CRT: &str = ":root {
  --bg:        #041204;
  --panel:     #061a06;
  --fg:        #4ee04e;
  --dim:       #44a844;
  --line:      #0c330c;
  --line2:     #327132;
  --inv-bg:    #4ee04e;
  --inv-fg:    #041204;
  --accent:    #9dff9d;
  --red:       #ff6b5f;
  --yellow:    #e0d34e;
  --green:     #4ee04e;
  --blue:      #62a0ff;
  --pink:      #ff79c6;
  --badge-info:#72b5c7;
  --mono: 'Courier New', 'Courier', monospace;
}";

#[derive(Clone, Copy, PartialEq)]
enum Theme {
    Default,
    Paper,
    Crt,
}

impl Theme {
    fn label(self) -> &'static str {
        match self {
            Theme::Default => "dark (built-in)",
            Theme::Paper => "paper",
            Theme::Crt => "crt",
        }
    }
    /// The `:root` override block layered after `panel_kit::CSS`.
    fn css(self) -> &'static str {
        match self {
            Theme::Default => "", // no overrides — the library's own palette
            Theme::Paper => THEME_PAPER,
            Theme::Crt => THEME_CRT,
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
    let ws = use_workspace("panel_kit_example_theming", default_layout);
    let mut theme = use_signal(|| Theme::Default);
    let theme_css = theme().css();

    let body = move |kind: Panel, _maximized: bool| -> Element {
        match kind {
            Panel::Swatches => rsx! {
                p { "Badges and the spinner pick the variables up too:" }
                div { class: "swatches",
                    Badge { field: "tag", value: "accent", kind: BadgeKind::Tag,
                        active: true, on_action: move |_: BadgeAction| {} }
                    Badge { field: "status", value: "green", kind: BadgeKind::Status,
                        on_action: move |_: BadgeAction| {} }
                    Badge { field: "entity", value: "yellow",
                        kind: BadgeKind::Entity { ty: None },
                        on_action: move |_: BadgeAction| {} }
                    Badge { field: "link", value: "red (unresolved)",
                        kind: BadgeKind::Wikilink { resolved: false, target: "x".to_string() },
                        on_action: move |_: BadgeAction| {} }
                    Spinner { label: "spinning in theme colours" }
                }
            },
            Panel::About => rsx! {
                p { "The stylesheet order is the whole mechanism:" }
                ol {
                    li { code { "style {{ {{panel_kit::CSS}} }}" } " — the library chrome" }
                    li { code { ":root {{ --bg: …; --accent: …; }}" }
                        " — your overrides, injected after it" }
                }
                p { "Switch presets in the top bar. The CRT preset also overrides "
                    code { "--mono" } ", the font stack variable." }
            },
        }
    };

    rsx! {
        style { {CSS} }
        style { {DEMO_CSS} }
        style { {theme_css} }
        div {
            class: ws.root_class(),
            onpointermove: move |event| ws.handle_pointer_move(&event),
            onpointerup: move |event| ws.handle_pointer_up(&event),
            header { class: "topbar",
                h1 { "panel-kit theming demo" }
                div { class: "theme-pick",
                    for t in [Theme::Default, Theme::Paper, Theme::Crt] {
                        button {
                            class: if theme() == t { "on" } else { "" },
                            onclick: move |_| theme.set(t),
                            {t.label()}
                        }
                    }
                }
            }
            {ws.render(body)}
            {ws.dock()}
        }
    }
}
