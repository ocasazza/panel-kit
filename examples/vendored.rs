//! Vendored-components canary — one surface exercising every painter added
//! with the jump-cannon vendor batch (see VENDORED.md): the selectable card,
//! the form rows, the hint bar + registry, build progress and the hydration
//! placeholder, neighbour pills, the frontmatter leftover grid, and the
//! status-colour mapping over the shared badge chrome.
//!
//! Run with: `dx serve --example vendored --platform web`
//! (dioxus-cli 0.6.x; provided by `nix develop`)

use dioxus::prelude::*;
use panel_kit::badge::{Badge, BadgeAction, BadgeKind, BadgeSpec};
use panel_kit::widgets::{build_progress, detect, forms, frontmatter, hints, pills, selection};
use panel_kit::CSS;
use panel_kit_core::widgets::build_progress::{BuildFeed, ProgressEvent};
use panel_kit_core::widgets::hints::{node_hint, NodeHintMeta};
use panel_kit_core::widgets::selection::{build_pills, neighbor_set, SelectionCard};

const DEMO_CSS: &str = "
body { overflow: auto !important; }
.demo { padding: 1rem; max-width: 880px; margin: 0 auto; display: flex; flex-direction: column; gap: .8rem; }
.demo h1 { font-size: 1rem; }
.demo h2 { font-size: .78rem; margin: 0; text-transform: uppercase; letter-spacing: .08em;
  color: var(--color-text-muted); }
.demo section { border: 1px solid var(--color-divider); border-radius: 4px;
  background: var(--color-surface); padding: .6rem .8rem; display: flex; flex-direction: column;
  gap: .45rem; }
.demo .row { display: flex; align-items: center; gap: 1rem; }
.demo .k { color: var(--color-text-muted); font-size: .72rem; width: 11rem; flex: none; }
.demo .grow { flex: 1; min-width: 0; }
.demo .cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(15rem, 1fr)); gap: .6rem; }
.demo .log { color: var(--color-text-muted); font-size: .68rem; white-space: pre-wrap;
  word-break: break-word; }
.demo button { background: var(--color-background); color: var(--color-text);
  border: 1px solid var(--color-border); border-radius: 3px; padding: .15rem .5rem;
  font-size: .72rem; cursor: pointer; }
.demo .grid2 { display: grid; grid-template-columns: 1fr 1fr; gap: .8rem; }
@media (max-width: 720px) { .demo .grid2 { grid-template-columns: 1fr; } }
";

/// Frontmatter fixture: `title`/`tags` are on the grid skip list, `note`,
/// `cfg`, `mixed`, and `gone` are leftovers the chip walker declines.
const FRONTMATTER: &str = r#"{
  "title": "kept out by the grid skip list",
  "tags": ["also", "kept", "out"],
  "note": "a long-form string the chip walker declines: it exceeds the one hundred and twenty character chip budget, so the leftover grid owns this row and scrolls it instead",
  "cfg": { "depth": 300, "stride": 1 },
  "mixed": [["nested"], "plain", 3],
  "gone": null
}"#;

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut log = use_signal(Vec::<String>::new);
    let mut slider = use_signal(|| 40.0_f64);
    let mut passes = use_signal(|| 3_i64);
    let mut keepalive = use_signal(|| true);
    let mut label = use_signal(|| "engine".to_string());
    let mut selected = use_signal(|| "graph-force".to_string());
    let mut feed = use_signal(BuildFeed::default);

    // Fake neighbour graph: ids 1, 2, 3 are in/out/both neighbours of 0; the
    // pill row caps at 200, so eight ids keep the truncation note hidden.
    let ids: Vec<String> = (0..8).map(|i| format!("journal/2026/week-{i}.md")).collect();
    let items: Vec<(u32, Option<_>)> =
        neighbor_set(0, &[1, 0, 0, 2, 2, 0, 0, 3]).into_iter().map(|(i, d)| (i, Some(d))).collect();
    let (pill_models, pill_truncated) = build_pills(&items, &ids, |_| None);
    let feed_text = feed.read().lines.iter().cloned().collect::<Vec<_>>().join(" · ");
    let log_text = log.read().join("\n");
    let summary = format!(
        "iterations={} · passes={} · keepalive={} · label={}",
        *slider.read(),
        *passes.read(),
        *keepalive.read(),
        label.read()
    );
    let pill_select = EventHandler::new(move |id: String| {
        log.write().push(format!("pill selected: {id}"));
    });

    rsx! {
        style { {CSS} }
        style { {DEMO_CSS} }
        div { class: "demo",
            h1 { "vendored components canary" }

            // Hint bar: publish paints the node hint (title · summary +
            // hotkey chip); clear falls back to the previous slot.
            div { class: "row",
                hints::HintBar {}
                button {
                    onclick: move |_| {
                        hints::publish(node_hint(
                            "vendored",
                            "journal/2026/week-2.md",
                            Some(&NodeHintMeta {
                                title: String::new(),
                                path: "journal/2026/week-2.md".into(),
                                doctype: Some("weekly".into()),
                                folder: "journal".into(),
                                tags: vec!["review".into(), "ship".into(), "demo".into(), "extra".into()],
                                body: "first non-blank body line becomes the excerpt; the rest folds away".into(),
                            }),
                        ));
                        log.write().push("hint published".into());
                    },
                    "publish node hint"
                }
                button { onclick: move |_| hints::clear("vendored"), "clear hint" }
            }

            div { class: "grid2",
                section {
                    h2 { "form rows" }
                    forms::Slider {
                        label: "iterations (log)".to_string(),
                        min: 0.1,
                        max: 1000.0,
                        value: *slider.read(),
                        log: Some(true),
                        on: move |v: f64| slider.set(v),
                    }
                    forms::IntRow {
                        label: "passes".to_string(),
                        min: 1,
                        max: 8,
                        value: *passes.read(),
                        note: Some("clamped into 1..=8".into()),
                        on: move |v: i64| passes.set(v),
                    }
                    forms::CheckRow {
                        label: "keepalive".to_string(),
                        value: *keepalive.read(),
                        text: Some("hold the panel shell".into()),
                        on: move |v: bool| keepalive.set(v),
                    }
                    forms::TextRow {
                        label: "label".to_string(),
                        value: "{label}",
                        on: move |v: String| label.set(v),
                    }
                    div { class: "log", "{summary}" }
                }

                section {
                    h2 { "build progress + hydration" }
                    div { class: "row",
                        span { class: "k", "determinate" }
                        div { class: "grow", {build_progress::progress_bar(Some(0.42))} }
                    }
                    div { class: "row",
                        span { class: "k", "indeterminate" }
                        div { class: "grow", {build_progress::progress_bar(None)} }
                    }
                    build_progress::PanelHydrating { label: "indexing sources…" }
                    div { class: "row",
                        button {
                            onclick: move |_| {
                                let next = feed.peek().since + 1;
                                let event = ProgressEvent::Log {
                                    message: format!("stage {}", feed.peek().lines.len() + 1),
                                };
                                feed.write().fold(next, &[event]);
                            },
                            "fold one event"
                        }
                        div { class: "log", "feed: {feed_text}" }
                    }
                }
            }

            section {
                h2 { "selectable cards" }
                div { class: "cards",
                    for id in ["graph-force", "graph-static", "github"] {
                        {
                            let is_selected = *selected.read() == id;
                            let card = match id {
                                "graph-force" => SelectionCard {
                                    source_id: "graph-force".into(),
                                    kind: "physics".into(),
                                    class: if is_selected { "pk-select-card default viewing".into() } else { "pk-select-card".into() },
                                    name: "Force (CPU)".into(),
                                    subtitle: "jump-cannon:gpu-force".into(),
                                    chips: vec![
                                        ("kind-physics".into(), "physics".into()),
                                        ("kind-live".into(), "live".into()),
                                    ],
                                    description: "N-body force sim over the vault graph.".into(),
                                    ..Default::default()
                                },
                                "graph-static" => SelectionCard {
                                    source_id: "graph-static".into(),
                                    kind: "static".into(),
                                    class: "pk-select-card".into(),
                                    name: "Static".into(),
                                    subtitle: "jump-cannon:static".into(),
                                    chips: vec![("kind-static".into(), "static".into())],
                                    description: "Precomputed layout, no simulation.".into(),
                                    ..Default::default()
                                },
                                _ => SelectionCard {
                                    source_id: "github".into(),
                                    kind: "github".into(),
                                    class: "pk-select-card ghost".into(),
                                    name: "GitHub import".into(),
                                    subtitle: "okf:github".into(),
                                    chips: vec![("kind-okf".into(), "okf".into())],
                                    description: "Kept-but-unadvertised remote source.".into(),
                                    is_ghost: true,
                                    disabled: true,
                                    disabled_reason: "token not configured".into(),
                                    ..Default::default()
                                },
                            };
                            let id = id.to_string();
                            rsx! {
                                selection::SelectableCard {
                                    card,
                                    on_select: move |_: ()| {
                                        selected.set(id.clone());
                                        log.write().push(format!("card selected: {id}"));
                                    },
                                }
                            }
                        }
                    }
                }
                div { class: "log", "selected: {selected}" }
            }

            div { class: "grid2",
                section {
                    h2 { "neighbour pills" }
                    {pills::pill_list("node", pill_models, pill_truncated, pill_select)}
                }

                section {
                    h2 { "status colours + frontmatter grid" }
                    div { class: "row",
                        for s in ["active", "failed", "needs-review", "draft", "archived", "mystery"] {
                            {
                                let accent = detect::status_color(s).map(str::to_string);
                                rsx! {
                                    Badge {
                                        spec: BadgeSpec::new("status", s, BadgeKind::Status),
                                        accent_color: accent,
                                        on_action: move |_: BadgeAction| {},
                                    }
                                }
                            }
                        }
                    }
                    {frontmatter::frontmatter_grid(FRONTMATTER)}
                }
            }

            div { class: "log", "event log:\n{log_text}" }
        }
    }
}
