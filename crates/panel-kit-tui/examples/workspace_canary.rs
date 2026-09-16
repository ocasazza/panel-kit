use panel_kit_core::spec::{BackendKind, BindingManifest, ContentKind, PanelProviderDeclaration};

#[cfg(feature = "spec-plan")]
use panel_kit_core::badge::{tag_hue, BadgeKind, BadgeSpec};
#[cfg(feature = "spec-plan")]
use panel_kit_core::theme::{GREEN, RED};
#[cfg(feature = "spec-plan")]
use panel_kit_core::widgets::charts::{
    five_num, BoxItemView, FlameSpanModel, GaugeModel, SeriesView,
};
#[cfg(feature = "spec-plan")]
use panel_kit_core::widgets::table::{
    ColumnWidth, TableCell, TableColumn, TableModel, TableRow, TextAlign,
};
#[cfg(feature = "spec-plan")]
use panel_kit_tui::badge::hue_color;

/// Executable provider declarations for the shared TUI canary.
///
/// The Nix-authored workspace owns titles and geometry. This retained Rust
/// source owns the executable data bindings that cannot live in Nix, and
/// `tools/spec-parity` compares the Nix output against this live manifest.
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

/// Convert a compact provider row into the strict spec declaration shape.
fn provider_declaration(
    (panel_id, content_kind, binding): (&str, ContentKind, &str),
) -> PanelProviderDeclaration {
    PanelProviderDeclaration {
        panel_id: panel_id.to_owned(),
        content_kind,
        binding: Some(binding.to_owned()),
    }
}

const CANARY_PROVIDERS: [(&str, ContentKind, &str); 9] = [
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

/// Demo rows for the Nodes table. Exercises the shared [`TableModel`] plus
/// TUI status/meter painters.
#[cfg(feature = "spec-plan")]
pub fn node_rows() -> TableModel {
    TableModel {
        columns: vec![
            TableColumn {
                key: "node".into(),
                title: "node".into(),
                width: ColumnWidth::Fixed { value: 12 },
                align: TextAlign::Left,
            },
            TableColumn {
                key: "load".into(),
                title: "load".into(),
                width: ColumnWidth::Fixed { value: 10 },
                align: TextAlign::Left,
            },
            TableColumn {
                key: "detail".into(),
                title: String::new(),
                width: ColumnWidth::Fixed { value: 10 },
                align: TextAlign::Left,
            },
        ],
        rows: vec![
            node_row("pdx-01 *", true, 0.42, "leader"),
            node_row("pdx-02", true, 0.18, "ready"),
            node_row("pdx-03", true, 0.71, "busy"),
            node_row("gfr-01", false, 0.0, "stale"),
        ],
    }
}

#[cfg(feature = "spec-plan")]
fn node_row(name: &str, healthy: bool, load: f64, detail: &str) -> TableRow {
    let color = if healthy { GREEN.rgb } else { RED.rgb };
    TableRow {
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

#[cfg(feature = "spec-plan")]
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

#[cfg(feature = "spec-plan")]
/// Number of points kept in each rolling time-series window.
pub const WINDOW: usize = 48;

/// Deterministic pseudo-random value in `0.0..1.0` from a counter — a small
/// hashed LCG so the canary animates without pulling in a `rand` dependency
/// (and stays reproducible across the terminal and wasm backends).
#[cfg(feature = "spec-plan")]
fn noise(seed: u64) -> f64 {
    let mut x = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51afd7ed558ccd);
    x ^= x >> 33;
    (x % 10_000) as f64 / 10_000.0
}

/// Rolling, per-frame metrics that drive the realtime charts. The examples
/// hold one of these in their app/demo struct and call [`Metrics::tick`]
/// each frame to push fresh samples; the chart renderers read the buffers.
#[cfg(feature = "spec-plan")]
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

#[cfg(feature = "spec-plan")]
impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

/// The boxplot / flame stage names, color-matched to apple-notes' stages.
#[cfg(feature = "spec-plan")]
pub const STAGES: [&str; 5] = ["segment", "markdown", "synthesis", "charts", "assemble"];

#[cfg(feature = "spec-plan")]
impl Metrics {
    /// A fresh metrics buffer pre-filled with one window of history so the
    /// charts read full immediately.
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

    /// Push one fresh sample onto every rolling buffer, dropping the oldest
    /// to keep the window bounded. Call once per rendered frame.
    pub fn tick(&mut self) {
        let n = self.n;
        let x = n as f64 * 0.1;
        // eval/ms: a slow sine drift plus jitter, 6..36 ms.
        let eval = 20.0 + 12.0 * (x * 0.6).sin() + 6.0 * (noise(n) - 0.5);
        // frame/ms: tight around the 16.6 ms budget with rare spikes.
        let spike = if noise(n ^ 0xa5) > 0.92 { 9.0 } else { 0.0 };
        let frame = 16.6 + 1.4 * (x * 1.3).sin() + spike + 1.0 * (noise(n ^ 0x5a) - 0.5);
        push_window(&mut self.eval, (x, eval.max(1.0)));
        push_window(&mut self.frame, (x, frame.max(1.0)));
        // Per-stage durations: each stage has a characteristic mean/spread.
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

    /// The two time-series for the Activity chart.
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

    /// A live flame/icicle tree: a "session" root over a handful of "job"
    /// branches, each split into the five stages, widths driven by the most
    /// recent per-stage sample so the graph breathes per frame.
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

/// Push `p` onto a rolling window, dropping the oldest point past [`WINDOW`].
#[cfg(feature = "spec-plan")]
fn push_window(buf: &mut Vec<(f64, f64)>, p: (f64, f64)) {
    if buf.len() >= WINDOW {
        buf.remove(0);
    }
    buf.push(p);
}

#[cfg(feature = "spec-plan")]
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
#[cfg(all(feature = "spec-plan", not(target_arch = "wasm32")))]
pub mod content {
    use std::collections::BTreeSet;

    use panel_kit_core::badge::{BadgeKind, BadgeSpec};
    use panel_kit_core::reducer::Snapshot;
    use panel_kit_core::widgets::charts::{
        five_num, BoxItemModel, BoxItemView, SeriesModel, SeriesView,
    };
    use panel_kit_core::widgets::meter::MeterModel;
    use panel_kit_core::widgets::status::StatusModel;
    use panel_kit_core::widgets::table::{TableModel, TableView};
    use panel_kit_core::widgets::{ContentSpec, DataSource, ScrollPolicy, TextModel};
    use panel_kit_core::{ResolvedWorkspace, SpecPanelId};
    use panel_kit_tui::charts::{boxplot, flame, gauges, time_series};
    use panel_kit_tui::spinner::spinner;
    use panel_kit_tui::ResolvedTuiTheme;
    use ratatui::layout::Rect;
    use ratatui::style::Style;
    use ratatui::text::{Line, Span};
    use ratatui::widgets::Paragraph;

    use crate::workspace_canary::{capacity_items, demo_badges, node_rows, Metrics};

    /// Mutable demo-provider state used by the executable native canary.
    pub struct DemoData {
        pub badges: Vec<BadgeSpec>,
        pub badge_zones: Vec<(Rect, usize)>,
        pub theme_zone: Rect,
        pub actions: Vec<String>,
        pub notes_scroll: usize,
        pub paper: bool,
        pub tick: u64,
        metrics: Metrics,
    }

    /// Borrowed workspace state required to paint one authored content panel.
    #[derive(Clone, Copy)]
    pub struct ContentRenderContext<'a> {
        pub resolved: &'a ResolvedWorkspace,
        pub snapshot: &'a Snapshot<SpecPanelId>,
        pub theme: &'a ResolvedTuiTheme,
    }

    impl DemoData {
        /// Build deterministic provider state for the native canary.
        pub fn new() -> Self {
            Self {
                badges: demo_badges(),
                badge_zones: Vec::new(),
                theme_zone: Rect::default(),
                actions: Vec::new(),
                notes_scroll: 0,
                paper: false,
                tick: 0,
                metrics: Metrics::new(),
            }
        }

        /// Advance deterministic animated data by one frame.
        pub fn tick(&mut self) {
            self.tick = self.tick.wrapping_add(1);
            self.metrics.tick();
        }
    }

    /// Render the body content for one projected panel.
    pub fn render_content(
        frame: &mut ratatui::Frame,
        area: Rect,
        content: &ContentSpec,
        context: ContentRenderContext<'_>,
        demo: &mut DemoData,
        content_kinds: &mut BTreeSet<&'static str>,
    ) {
        content_kinds.insert(content.kind());
        match content {
            ContentSpec::Custom { binding } if binding == "canary.workspace" => {
                render_workspace_intro(frame, area, context)
            }
            ContentSpec::Custom { binding } if binding == "canary.theme" => {
                render_theme_panel(frame, area, context.theme, demo)
            }
            ContentSpec::Custom { binding } => {
                render_binding_placeholder(frame, area, context.theme, binding)
            }
            ContentSpec::Text { source, scroll } => {
                render_text(frame, area, context.theme, source, *scroll, demo)
            }
            ContentSpec::Editor {
                binding,
                placeholder,
                ..
            } => render_editor_binding(frame, area, binding, placeholder),
            ContentSpec::Badges { source } => {
                render_badges(frame, area, context.theme, source, demo)
            }
            ContentSpec::Table { source } => render_table(frame, area, context.theme, source),
            ContentSpec::TimeSeries { source, unit } => {
                render_time_series(frame, area, context.theme, source, unit, demo)
            }
            ContentSpec::Gauges { source } => render_gauges(frame, area, context.theme, source),
            ContentSpec::Flamegraph { source } => {
                render_flame(frame, area, context.theme, source, demo)
            }
            ContentSpec::Boxplot { source } => {
                render_boxplot(frame, area, context.theme, source, demo)
            }
            ContentSpec::Meter { source } => render_meter(frame, area, context.theme, source),
            ContentSpec::Status { source } => render_status(frame, area, source),
            ContentSpec::Spinner { label } => {
                render_spinner(frame, area, context.theme, label, demo.tick)
            }
        }
    }

    fn render_workspace_intro(
        frame: &mut ratatui::Frame,
        area: Rect,
        context: ContentRenderContext<'_>,
    ) {
        let mode = match context.snapshot.preferred_mode {
            panel_kit_core::Mode::Floating => "floating",
            panel_kit_core::Mode::Tiling => "tiling",
        };
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(
                    "panel-kit-tui workspace-spec canary",
                    Style::default().fg(context.theme.fg),
                )),
                Line::from(""),
                Line::from(
                    "Host owns Snapshot, reducer effects, projection scratch, and render order.",
                ),
                Line::from(format!(
                    "Spec: {} · mode: {mode} · cells: {:.0}×{:.0}",
                    context.resolved.id,
                    context.snapshot.viewport.width,
                    context.snapshot.viewport.height
                )),
                Line::from("Input: crossterm → tui::input → core reduce → SavePolicy."),
            ])
            .style(Style::default().fg(context.theme.dim)),
            area,
        );
    }

    fn render_binding_placeholder(
        frame: &mut ratatui::Frame,
        area: Rect,
        theme: &ResolvedTuiTheme,
        binding: &str,
    ) {
        frame.render_widget(
            Paragraph::new(format!("native binding: {binding}"))
                .style(Style::default().fg(theme.dim)),
            area,
        );
    }

    fn render_editor_binding(
        frame: &mut ratatui::Frame,
        area: Rect,
        binding: &str,
        placeholder: &str,
    ) {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(format!("editor binding: {binding}")),
                Line::from(placeholder),
            ]),
            area,
        );
    }

    fn render_text(
        frame: &mut ratatui::Frame,
        area: Rect,
        theme: &ResolvedTuiTheme,
        source: &DataSource<TextModel>,
        scroll_policy: ScrollPolicy,
        demo: &mut DemoData,
    ) {
        let lines = match source {
            DataSource::Inline { value } => value.text.lines().map(Line::from).collect(),
            DataSource::Binding { id } if id == "canary.notes" => notes_lines(theme, demo.tick),
            DataSource::Binding { id } => vec![Line::from(format!("text binding: {id}"))],
        };
        demo.notes_scroll = match scroll_policy {
            ScrollPolicy::Clip => {
                frame.render_widget(Paragraph::new(lines), area);
                demo.notes_scroll
            }
            ScrollPolicy::Wrap | ScrollPolicy::Auto => {
                panel_kit_tui::scroll::lines(frame, area, theme, lines, demo.notes_scroll)
            }
        };
    }

    fn render_badges(
        frame: &mut ratatui::Frame,
        area: Rect,
        theme: &ResolvedTuiTheme,
        source: &DataSource<Vec<BadgeSpec>>,
        demo: &mut DemoData,
    ) {
        let owned;
        let badges = match source {
            DataSource::Inline { value } => value.as_slice(),
            DataSource::Binding { id } if id == "canary.badges" => demo.badges.as_slice(),
            DataSource::Binding { id } => {
                owned = vec![BadgeSpec::new("binding", id.clone(), BadgeKind::Tag)];
                owned.as_slice()
            }
        };
        demo.badge_zones.clear();
        for (row, (index, badge)) in badges.iter().enumerate().enumerate() {
            if row as u16 >= area.height.saturating_sub(4) {
                break;
            }
            let rect = Rect::new(
                area.x,
                area.y + row as u16,
                panel_kit_tui::badge::width(badge).min(area.width),
                1,
            );
            if source.binding_id() == Some("canary.badges") {
                demo.badge_zones.push((rect, index));
            }
            frame.render_widget(
                Paragraph::new(Line::from(panel_kit_tui::badge::spans(badge, theme))),
                rect,
            );
        }
        render_recent_actions(frame, area, theme, demo);
    }

    fn render_recent_actions(
        frame: &mut ratatui::Frame,
        area: Rect,
        theme: &ResolvedTuiTheme,
        demo: &DemoData,
    ) {
        let log_y = area.y + area.height.saturating_sub(3);
        let recent = demo
            .actions
            .iter()
            .rev()
            .take(3)
            .map(|action| {
                Line::from(Span::styled(
                    action.as_str(),
                    Style::default().fg(theme.badge_info),
                ))
            })
            .collect::<Vec<_>>();
        if log_y > area.y {
            frame.render_widget(
                Paragraph::new(recent),
                Rect::new(area.x, log_y, area.width, 3.min(area.height)),
            );
        }
    }

    fn render_table(
        frame: &mut ratatui::Frame,
        area: Rect,
        theme: &ResolvedTuiTheme,
        source: &DataSource<TableModel>,
    ) {
        let owned;
        let model = match source {
            DataSource::Inline { value } => value,
            DataSource::Binding { id } if id == "canary.nodes" => {
                owned = node_rows();
                &owned
            }
            DataSource::Binding { .. } => return,
        };
        panel_kit_tui::table::table(
            frame,
            area,
            theme,
            TableView {
                columns: &model.columns,
                rows: &model.rows,
            },
        );
    }

    fn render_time_series(
        frame: &mut ratatui::Frame,
        area: Rect,
        theme: &ResolvedTuiTheme,
        source: &DataSource<Vec<SeriesModel>>,
        unit: &str,
        demo: &DemoData,
    ) {
        let inline_views;
        let live_series;
        let series = match source {
            DataSource::Inline { value } => {
                inline_views = series_views(value);
                inline_views.as_slice()
            }
            DataSource::Binding { id } if id == "canary.activity" => {
                live_series = demo.metrics.series();
                live_series.as_slice()
            }
            DataSource::Binding { .. } => &[],
        };
        time_series(frame, area, theme, unit, series);
    }

    fn render_gauges(
        frame: &mut ratatui::Frame,
        area: Rect,
        theme: &ResolvedTuiTheme,
        source: &DataSource<Vec<panel_kit_core::widgets::charts::GaugeModel>>,
    ) {
        let owned;
        let gauges_data = match source {
            DataSource::Inline { value } => value.as_slice(),
            DataSource::Binding { id } if id == "canary.capacity" => {
                owned = capacity_items();
                owned.as_slice()
            }
            DataSource::Binding { .. } => &[],
        };
        gauges(frame, area, theme, gauges_data);
    }

    fn render_flame(
        frame: &mut ratatui::Frame,
        area: Rect,
        theme: &ResolvedTuiTheme,
        source: &DataSource<Vec<panel_kit_core::widgets::charts::FlameSpanModel>>,
        demo: &DemoData,
    ) {
        let owned;
        let spans = match source {
            DataSource::Inline { value } => value.as_slice(),
            DataSource::Binding { id } if id == "canary.flame" => {
                owned = demo.metrics.flame();
                owned.as_slice()
            }
            DataSource::Binding { .. } => &[],
        };
        flame(frame, area, theme, spans);
    }

    fn render_boxplot(
        frame: &mut ratatui::Frame,
        area: Rect,
        theme: &ResolvedTuiTheme,
        source: &DataSource<Vec<BoxItemModel>>,
        demo: &DemoData,
    ) {
        let inline_items;
        let live_items;
        let items = match source {
            DataSource::Inline { value } => {
                inline_items = box_item_views(value);
                inline_items.as_slice()
            }
            DataSource::Binding { id } if id == "canary.distribution" => {
                live_items = demo.metrics.boxes();
                live_items.as_slice()
            }
            DataSource::Binding { .. } => &[],
        };
        boxplot(frame, area, theme, items);
    }

    fn render_meter(
        frame: &mut ratatui::Frame,
        area: Rect,
        theme: &ResolvedTuiTheme,
        source: &DataSource<MeterModel>,
    ) {
        if let Some(model) = inline_or_default(
            source,
            MeterModel {
                label: "meter".into(),
                ratio: 0.5,
                text: "50%".into(),
                color: None,
            },
        ) {
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(model.label.as_str(), Style::default().fg(theme.fg)),
                    Span::raw(" "),
                    panel_kit_tui::meter::span_model(&model, 12),
                    Span::raw(" "),
                    Span::raw(model.text.as_str()),
                ])),
                area,
            );
        }
    }

    fn render_status(frame: &mut ratatui::Frame, area: Rect, source: &DataSource<StatusModel>) {
        if let Some(model) = inline_or_default(
            source,
            StatusModel {
                label: "status".into(),
                state: panel_kit_core::widgets::status::StatusState::Info,
                color: (131, 183, 204),
            },
        ) {
            frame.render_widget(Paragraph::new(panel_kit_tui::status::line(&model)), area);
        }
    }

    fn render_spinner(
        frame: &mut ratatui::Frame,
        area: Rect,
        theme: &ResolvedTuiTheme,
        label: &DataSource<String>,
        tick: u64,
    ) {
        let fallback = String::from("spinner");
        let text = match label {
            DataSource::Inline { value } => value.as_str(),
            DataSource::Binding { id } => id.as_str(),
        };
        let text = if text.is_empty() {
            fallback.as_str()
        } else {
            text
        };
        frame.render_widget(Paragraph::new(spinner(tick, text, theme)), area);
    }

    fn render_theme_panel(
        frame: &mut ratatui::Frame,
        area: Rect,
        theme: &ResolvedTuiTheme,
        demo: &mut DemoData,
    ) {
        demo.theme_zone = area;
        let swatch = |color, name: &'static str| {
            Line::from(vec![
                Span::styled("## ", Style::default().fg(color)),
                Span::styled(name, Style::default().fg(theme.dim)),
            ])
        };
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(
                    if demo.paper {
                        "preset: paper (press p)"
                    } else {
                        "preset: spec default (press p)"
                    },
                    Style::default().fg(theme.fg),
                )),
                swatch(theme.blue, "blue · mode light"),
                swatch(theme.yellow, "yellow · minimize"),
                swatch(theme.pink, "pink · maximize"),
                spinner(demo.tick / 2, "spinner painter", theme),
            ]),
            area,
        );
    }

    fn notes_lines(theme: &ResolvedTuiTheme, tick: u64) -> Vec<Line<'static>> {
        vec![
            Line::from(Span::styled("docs-as-code canary", Style::default().fg(theme.fg))),
            Line::from(""),
            Line::from("This example renders the terminal backend as executable documentation."),
            Line::from("It exercises spec-authored layout, chrome, input, persistence, badges, charts, tables, gauges, text, custom panels, and dock restore."),
            Line::from("Use p for palette, m/f/t for window state, Tab to cycle focus, 1-9 to restore, and PgUp/PgDn or the wheel to scroll."),
            Line::from(""),
            spinner(tick, "TUI canary running", theme),
        ]
    }

    fn series_views(series: &[SeriesModel]) -> Vec<SeriesView<'_>> {
        series
            .iter()
            .map(|item| SeriesView {
                name: item.name.as_str(),
                points: &item.points,
            })
            .collect()
    }

    fn box_item_views(items: &[BoxItemModel]) -> Vec<BoxItemView<'_>> {
        items
            .iter()
            .filter_map(|item| {
                five_num(&item.samples).map(|summary| BoxItemView {
                    label: item.label.as_str(),
                    summary,
                    color: item.color,
                })
            })
            .collect()
    }

    fn inline_or_default<T: Clone>(source: &DataSource<T>, fallback: T) -> Option<T> {
        match source {
            DataSource::Inline { value } => Some(value.clone()),
            DataSource::Binding { .. } => Some(fallback),
        }
    }
}
