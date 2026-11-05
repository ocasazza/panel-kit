//! Backend-neutral content for the workspace canary.
//!
//! `nix/specs/workspace-canary.nix` owns panel identity, geometry, and chrome;
//! this module owns the data behind its `canary.*` bindings. Every backend
//! canary (web, TUI, browser TUI, Bevy) includes it with `#[path]` and paints
//! the same models, and `tools/spec-parity` compares [`provider_manifest`]
//! against the Nix spec. Depends only on `panel-kit-core`.

use panel_kit_core::badge::{tag_hue, BadgeKind, BadgeSpec};
use panel_kit_core::spec::{BackendKind, BindingManifest, ContentKind, PanelProviderDeclaration};
use panel_kit_core::theme::{ERROR, SUCCESS};
use panel_kit_core::widgets::badge::hue_color;
use panel_kit_core::widgets::charts::{five_num, BoxItemView, FlameSpanModel, GaugeModel, SeriesView};
use panel_kit_core::widgets::data_table::ColumnKind;
use panel_kit_core::widgets::table::{
    ColumnWidth, TableCell, TableColumn, TableModel, TableRow, TextAlign,
};

/// Executable provider declarations for the canary on `backend`.
pub fn provider_manifest(backend: BackendKind) -> BindingManifest {
    BindingManifest {
        backend,
        panels: CANARY_PROVIDERS
            .iter()
            .copied()
            .map(provider_declaration)
            .collect(),
    }
}

fn provider_declaration(
    (panel_id, content_kind, binding): (&str, ContentKind, &str),
) -> PanelProviderDeclaration {
    PanelProviderDeclaration {
        panel_id: panel_id.to_owned(),
        content_kind,
        binding: Some(binding.to_owned()),
    }
}

/// `(panel id, content kind, binding id)` for every canary panel.
pub const CANARY_PROVIDERS: [(&str, ContentKind, &str); 9] = [
    ("Workspace", ContentKind::Custom, "canary.workspace"),
    ("Activity", ContentKind::TimeSeries, "canary.activity"),
    ("Flame", ContentKind::Flamegraph, "canary.flame"),
    ("Notes", ContentKind::Text, "canary.notes"),
    ("Badges", ContentKind::Badges, "canary.badges"),
    ("Nodes", ContentKind::Table, "canary.nodes"),
    ("Capacity", ContentKind::Gauges, "canary.capacity"),
    ("Distribution", ContentKind::Boxplot, "canary.distribution"),
    ("Theme", ContentKind::Custom, "canary.theme"),
];

/// Rows for the Nodes table: status, load meter, and detail cells.
pub fn node_rows() -> TableModel {
    TableModel {
        columns: vec![
            TableColumn {
                key: "node".into(),
                title: "node".into(),
                width: ColumnWidth::Fixed { value: 12 },
                align: TextAlign::Left,
                kind: ColumnKind::Text,
            },
            TableColumn {
                key: "load".into(),
                title: "load".into(),
                width: ColumnWidth::Fixed { value: 10 },
                align: TextAlign::Left,
                kind: ColumnKind::Text,
            },
            TableColumn {
                key: "detail".into(),
                title: String::new(),
                width: ColumnWidth::Fixed { value: 10 },
                align: TextAlign::Left,
                kind: ColumnKind::Text,
            },
        ],
        rows: vec![
            node_row("pdx-01 *", true, 0.42, "leader"),
            node_row("pdx-02", true, 0.18, "ready"),
            node_row("pdx-03", true, 0.71, "busy"),
            node_row("pdx-04", true, 0.35, "ready"),
            node_row("pdx-05", true, 0.58, "warm"),
            node_row("pdx-06", true, 0.24, "ready"),
            node_row("pdx-07", true, 0.83, "hot"),
            node_row("pdx-08", true, 0.12, "draining"),
            node_row("gfr-01", false, 0.0, "stale"),
            node_row("gfr-02", true, 0.47, "ready"),
            node_row("gfr-03", true, 0.66, "warm"),
            node_row("gfr-04", true, 0.29, "ready"),
            node_row("iad-01", true, 0.53, "ready"),
            node_row("iad-02", false, 0.91, "evicting"),
            node_row("iad-03", true, 0.38, "ready"),
            node_row("syd-01", true, 0.61, "ready"),
            node_row("syd-02", true, 0.27, "catching up"),
        ],
    }
}

fn node_row(name: &str, healthy: bool, load: f64, detail: &str) -> TableRow {
    let color = if healthy { SUCCESS.rgb } else { ERROR.rgb };
    TableRow {
        id: name.to_string(),
        tags: Vec::new(),
        cells: vec![
            TableCell::Status {
                label: name.into(),
                color,
            },
            TableCell::Meter {
                ratio: load,
                text: String::new(),
                color: Some(color),
            },
            TableCell::Text(detail.into()),
        ],
    }
}

/// One chip of every badge kind, plus an active status chip.
pub fn demo_badges() -> Vec<BadgeSpec> {
    let mut tag = BadgeSpec::new("tag", "browser-tui", BadgeKind::Tag);
    tag.override_color = Some(hue_color(tag_hue("browser-tui")));
    let mut active = BadgeSpec::new("status", "canary", BadgeKind::Status);
    active.active = true;
    vec![
        tag,
        BadgeSpec::new("doctype", "example", BadgeKind::Doctype),
        BadgeSpec::new("folder", "crates/panel-kit-tui", BadgeKind::Folder),
        BadgeSpec::new("author", "olive", BadgeKind::Author),
        BadgeSpec::new(
            "entity",
            "panel-kit-core",
            BadgeKind::Entity {
                ty: Some("crate".into()),
            },
        ),
        BadgeSpec::new(
            "link",
            "WorkspaceSpec",
            BadgeKind::Wikilink {
                resolved: true,
                target: "WorkspaceSpec".into(),
            },
        ),
        BadgeSpec::new(
            "link",
            "missing-doc",
            BadgeKind::Wikilink {
                resolved: false,
                target: "missing-doc".into(),
            },
        ),
        BadgeSpec::new(
            "url",
            "ratzilla",
            BadgeKind::Url {
                href: "https://github.com/ratatui/ratzilla".into(),
                host: "github.com".into(),
            },
        ),
        BadgeSpec::new("date", "2026-06-13", BadgeKind::Date),
        active,
        BadgeSpec::new("mode", "wasm", BadgeKind::Generic),
    ]
}

/// Number of points kept in each rolling time-series window.
pub const WINDOW: usize = 48;

/// Deterministic value in `0.0..1.0` from a counter (hashed LCG), so every
/// backend animates identically without a `rand` dependency.
fn noise(seed: u64) -> f64 {
    let mut x = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51afd7ed558ccd);
    x ^= x >> 33;
    (x % 10_000) as f64 / 10_000.0
}

/// Rolling per-frame metrics behind the Activity, Flame, and Distribution
/// panels. Hosts call [`Metrics::tick`] once per frame.
pub struct Metrics {
    /// Rolling `eval/ms` samples as `(x_seconds, ms)`.
    pub eval: Vec<(f64, f64)>,
    /// Rolling `frame/ms` samples as `(x_seconds, ms)`.
    pub frame: Vec<(f64, f64)>,
    /// Per-stage duration samples for the boxplot, newest last.
    pub stage_samples: [Vec<f64>; 5],
    /// Frame counter; also the x-axis clock (0.1 s per tick).
    n: u64,
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Boxplot and flame stage names.
pub const STAGES: [&str; 5] = ["segment", "markdown", "synthesis", "charts", "assemble"];

impl Metrics {
    /// Metrics pre-filled with one window of history.
    pub fn new() -> Self {
        let mut m = Metrics {
            eval: Vec::with_capacity(WINDOW),
            frame: Vec::with_capacity(WINDOW),
            stage_samples: Default::default(),
            n: 0,
        };
        for _ in 0..WINDOW {
            m.tick();
        }
        m
    }

    /// Push one sample onto every rolling buffer, dropping the oldest.
    pub fn tick(&mut self) {
        let n = self.n;
        let x = n as f64 * 0.1;
        // eval/ms: slow sine drift plus jitter, 6..36 ms.
        let eval = 20.0 + 12.0 * (x * 0.6).sin() + 6.0 * (noise(n) - 0.5);
        // frame/ms: tight around the 16.6 ms budget with rare spikes.
        let spike = if noise(n ^ 0xa5) > 0.92 { 9.0 } else { 0.0 };
        let frame = 16.6 + 1.4 * (x * 1.3).sin() + spike + 1.0 * (noise(n ^ 0x5a) - 0.5);
        push_window(&mut self.eval, (x, eval.max(1.0)));
        push_window(&mut self.frame, (x, frame.max(1.0)));
        let means = [2.4, 5.1, 3.3, 1.8, 0.9];
        let spreads = [0.8, 2.0, 1.1, 0.6, 0.3];
        for (s, samples) in self.stage_samples.iter_mut().enumerate() {
            let v = means[s] + spreads[s] * (noise(n ^ (s as u64 * 0x9e3) ^ 0x1234) - 0.5) * 2.0;
            if samples.len() >= WINDOW {
                samples.remove(0);
            }
            samples.push(v.max(0.05));
        }
        self.n = n.wrapping_add(1);
    }

    /// The two series for the Activity chart.
    pub fn series(&self) -> [SeriesView<'_>; 2] {
        [
            SeriesView {
                name: "eval/ms",
                points: &self.eval,
            },
            SeriesView {
                name: "frame/ms",
                points: &self.frame,
            },
        ]
    }

    /// Box-and-whisker items, one per pipeline stage.
    pub fn boxes(&self) -> Vec<BoxItemView<'_>> {
        self.stage_samples
            .iter()
            .enumerate()
            .filter_map(|(i, samples)| {
                five_num(samples).map(|summary| BoxItemView {
                    label: STAGES[i],
                    summary,
                    color: None,
                })
            })
            .collect()
    }

    /// Flame tree: a `session` root over three jobs, each split into the five
    /// stages, widths driven by the latest per-stage sample.
    pub fn flame(&self) -> Vec<FlameSpanModel> {
        let latest = |i: usize| self.stage_samples[i].last().copied().unwrap_or(1.0);
        let mut spans = Vec::new();
        let jobs = 3usize;
        let mut session_total = 0.0;
        let mut job_spans: Vec<(f64, Vec<f64>)> = Vec::new();
        for j in 0..jobs {
            let weights: Vec<f64> = (0..5)
                .map(|s| latest(s) * (0.6 + 1.2 * noise(self.n ^ (j as u64 * 0x77) ^ s as u64)))
                .collect();
            let total: f64 = weights.iter().sum();
            session_total += total;
            job_spans.push((total, weights));
        }
        spans.push(FlameSpanModel {
            label: "session".into(),
            depth: 0,
            value: session_total,
            color: None,
        });
        for (j, (total, weights)) in job_spans.into_iter().enumerate() {
            spans.push(FlameSpanModel {
                label: format!("job-{j}"),
                depth: 1,
                value: total,
                color: None,
            });
            for (s, w) in weights.into_iter().enumerate() {
                spans.push(FlameSpanModel {
                    label: STAGES[s].into(),
                    depth: 2,
                    value: w,
                    color: None,
                });
            }
        }
        spans
    }
}

fn push_window(buf: &mut Vec<(f64, f64)>, p: (f64, f64)) {
    if buf.len() >= WINDOW {
        buf.remove(0);
    }
    buf.push(p);
}

/// The Capacity panel's four gauges.
pub fn capacity_items() -> [GaugeModel; 4] {
    [
        GaugeModel {
            label: "vfs".into(),
            ratio: 0.21,
            text: "31 / 148 files".into(),
        },
        GaugeModel {
            label: "wasm".into(),
            ratio: 0.63,
            text: "6.3 MB / 10 MB".into(),
        },
        GaugeModel {
            label: "events".into(),
            ratio: 0.78,
            text: "78% queue".into(),
        },
        GaugeModel {
            label: "layout".into(),
            ratio: 0.94,
            text: "94% stress".into(),
        },
    ]
}

/// Notes panel content as `(heading, lines)` sections, long enough to
/// overflow the panel body at the canary's default size so panel scrolling is
/// observable. Backends paint headings in the text colour, lines muted.
pub const NOTES: &[(&str, &[&str])] = &[
    ("docs-as-code canary", &[]),
    (
        "what this example is",
        &[
            "This example renders one panel-kit backend as executable documentation.",
            "It exercises spec-authored layout, chrome, input, persistence,",
            "badges, charts, tables, gauges, text, custom panels, and dock restore.",
        ],
    ),
    (
        "panel scrolling",
        &[
            "Wheel and PgUp/PgDn scroll the panel under the pointer first.",
            "Content consumes the gesture until its edge, then the gesture bubbles",
            "to the workspace, matching the web shell's overflow chaining.",
            "Shift+wheel scrolls sideways; the Nodes table scrolls row-wise",
            "under a sticky header.",
        ],
    ),
    (
        "keys",
        &[
            "p · palette            m · minimize       f · float",
            "t · tile               Tab · cycle focus  1-9 · restore",
            "arrows · move          Shift+arrows · resize",
            "PgUp/PgDn · page scroll this panel, then the workspace",
            "q · quit",
        ],
    ),
    (
        "why it is long",
        &[
            "Canary bodies are small, so short text would hide the scrollbar and",
            "make panel scrolling indistinguishable from workspace scrolling.",
            "Overflowing text and node rows keep both paths observable.",
        ],
    ),
    (
        "reading order",
        &[
            "1 · project the snapshot into reusable scratch",
            "2 · draw root, panels, chrome, controls, dock, scrollbars",
            "3 · paint body content with the panel's scroll offsets",
            "4 · route wheel gestures through the same offsets",
            "",
            "Every step above is a renderer-neutral core calculation or a thin",
            "backend painter; every backend canary runs the identical sequence.",
            "",
            "Scroll this panel with the wheel to confirm the offsets move.",
        ],
    ),
];

/// Heading of the Workspace panel's card.
pub const WORKSPACE_CARD_HEADING: &str = "Host-owned composition";

/// Body lines of the Workspace panel's card for the panel titled `title`.
/// `header_actions` counts clicks on the panel's header actions; a backend
/// without header actions reports 0.
pub fn workspace_card_lines(title: &str, maximized: bool, header_actions: u32) -> [String; 2] {
    [
        format!(
            "Panel “{title}” was declared by Nix, resolved into SpecPanelId, and projected into panel parts."
        ),
        format!("maximized: {maximized} · header actions: {header_actions}"),
    ]
}

/// Theme panel swatches: `(semantic slot, caption)`.
pub const THEME_SWATCHES: [(panel_kit_core::theme::ThemeColor, &str); 3] = [
    (panel_kit_core::theme::ThemeColor::Primary, "primary · mode light"),
    (panel_kit_core::theme::ThemeColor::Tertiary, "tertiary · minimize"),
    (panel_kit_core::theme::ThemeColor::Secondary, "secondary · maximize"),
];

/// Theme panel heading for the active preset.
pub fn theme_preset_label(paper: bool) -> &'static str {
    if paper {
        "preset: paper (press p)"
    } else {
        "preset: spec default (press p)"
    }
}

/// [`NOTES`] as one newline-joined block for text painters without per-line
/// styling: sections separated by a blank line.
pub fn notes_text() -> String {
    NOTES
        .iter()
        .map(|(heading, body)| {
            std::iter::once(*heading)
                .chain(body.iter().copied())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}
