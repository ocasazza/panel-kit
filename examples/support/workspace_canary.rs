//! TUI canary: the shared canary content plus its ratatui painters.

#[allow(dead_code)]
#[path = "canary_content.rs"]
mod canary_content;
pub use canary_content::*;

#[cfg(all(feature = "spec-plan", not(target_arch = "wasm32")))]
pub mod content {
    use std::collections::BTreeSet;

    use panel_kit_core::badge::{BadgeKind, BadgeSpec};
    use panel_kit_core::reducer::Snapshot;
    use panel_kit_core::widgets::charts::{
        five_num, BoxItemModel, BoxItemView, SeriesModel, SeriesView,
    };
    use panel_kit_core::widgets::graph::{GraphLayout, GraphModel, GraphView};
    use panel_kit_core::widgets::meter::MeterModel;
    use panel_kit_core::widgets::status::StatusModel;
    use panel_kit_core::widgets::table::{TableModel, TableView};
    use panel_kit_core::widgets::{ContentSpec, DataSource, ScrollPolicy, TextModel};
    use panel_kit_core::{ResolvedWorkspace, SpecPanelId};
    use panel_kit_tui::charts::{boxplot, flame, gauges, time_series};
    use panel_kit_tui::graph::graph;
    use panel_kit_tui::scroll::PanelScroll;
    use panel_kit_tui::spinner::spinner;
    use panel_kit_tui::ResolvedTuiTheme;
    use ratatui::layout::Rect;
    use ratatui::style::Style;
    use ratatui::text::{Line, Span};
    use ratatui::widgets::Paragraph;

    use panel_kit_graph_layout::solve_2d;

    use crate::workspace_canary::{canary_graph, capacity_items, demo_badges, node_rows, Metrics};

    /// Mutable demo-provider state used by the executable native canary.
    pub struct DemoData {
        pub badges: Vec<BadgeSpec>,
        pub badge_zones: Vec<(Rect, usize)>,
        pub theme_zone: Rect,
        pub actions: Vec<String>,
        pub panels: PanelScroll<SpecPanelId>,
        pub paper: bool,
        pub tick: u64,
        metrics: Metrics,
        /// The Graph panel's model and its solved unit-square positions.
        graph: Option<(GraphModel, Vec<[f32; 2]>)>,
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
                panels: PanelScroll::with_capacity(10),
                paper: false,
                tick: 0,
                metrics: Metrics::new(),
                graph: None,
            }
        }

        /// Advance deterministic animated data by one frame.
        pub fn tick(&mut self) {
            self.tick = self.tick.wrapping_add(1);
            self.metrics.tick();
        }
    }

    /// Render the body content for one projected panel.
    ///
    /// `key` identifies the panel whose scroll offsets the content records.
    pub fn render_content(
        frame: &mut ratatui::Frame,
        area: Rect,
        key: SpecPanelId,
        content: &ContentSpec,
        context: ContentRenderContext<'_>,
        demo: &mut DemoData,
        content_kinds: &mut BTreeSet<&'static str>,
    ) {
        content_kinds.insert(content.kind());
        match content {
            ContentSpec::Custom { binding } if binding == "canary.workspace" => {
                render_workspace_intro(frame, area, key, context)
            }
            ContentSpec::Custom { binding } if binding == "canary.theme" => {
                render_theme_panel(frame, area, context.theme, demo)
            }
            ContentSpec::Custom { binding } => {
                render_binding_placeholder(frame, area, context.theme, binding)
            }
            ContentSpec::Text { source, scroll } => {
                render_text(frame, area, context.theme, source, *scroll, key, demo)
            }
            ContentSpec::Editor {
                binding,
                placeholder,
                ..
            } => render_editor_binding(frame, area, binding, placeholder),
            ContentSpec::Badges { source } => {
                render_badges(frame, area, context.theme, source, demo)
            }
            ContentSpec::Table { source } => {
                render_table(frame, area, context.theme, source, key, demo)
            }
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
            ContentSpec::External { source, layout, .. }
                if source.binding_id() == Some("canary.graph") =>
            {
                render_graph(frame, area, context.theme, layout, demo)
            }
            ContentSpec::External { component, source, .. } => render_binding_placeholder(
                frame,
                area,
                context.theme,
                source.binding_id().unwrap_or(component.as_str()),
            ),
        }
    }

    /// The shared Workspace card. Terminal panel headers carry no header
    /// actions, so their click count is 0.
    fn render_workspace_intro(
        frame: &mut ratatui::Frame,
        area: Rect,
        key: SpecPanelId,
        context: ContentRenderContext<'_>,
    ) {
        let title = context
            .resolved
            .catalog
            .get(key)
            .map_or("", |meta| meta.title.as_ref());
        let maximized = context.snapshot.panels.iter().any(|panel| {
            panel.kind == key && panel.state == panel_kit_core::WinState::Maximized
        });
        let [declared, state] =
            crate::workspace_canary::workspace_card_lines(title, maximized, 0);
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(
                    crate::workspace_canary::WORKSPACE_CARD_HEADING.to_uppercase(),
                    Style::default().fg(context.theme.text),
                )),
                Line::from(declared),
                Line::from(state),
            ])
            .wrap(ratatui::widgets::Wrap { trim: false })
            .style(Style::default().fg(context.theme.text_muted)),
            area,
        );
    }

    /// Paint the canary graph, solving it under `layout` on first use.
    fn render_graph(
        frame: &mut ratatui::Frame,
        area: Rect,
        theme: &ResolvedTuiTheme,
        layout: &GraphLayout,
        demo: &mut DemoData,
    ) {
        let (model, positions) = demo.graph.get_or_insert_with(|| {
            let model = canary_graph();
            let positions = solve_2d(&model, layout);
            (model, positions)
        });
        let view = GraphView { nodes: &model.nodes, edges: &model.edges };
        graph(frame, area, theme, view, positions, None);
    }

    fn render_binding_placeholder(
        frame: &mut ratatui::Frame,
        area: Rect,
        theme: &ResolvedTuiTheme,
        binding: &str,
    ) {
        frame.render_widget(
            Paragraph::new(format!("native binding: {binding}"))
                .style(Style::default().fg(theme.text_muted)),
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
        key: SpecPanelId,
        demo: &mut DemoData,
    ) {
        let lines = match source {
            DataSource::Inline { value } => value.text.lines().map(Line::from).collect(),
            DataSource::Binding { id } if id == "canary.notes" => notes_lines(theme, demo.tick),
            DataSource::Binding { id } => vec![Line::from(format!("text binding: {id}"))],
        };
        if scroll_policy == ScrollPolicy::Clip {
            frame.render_widget(Paragraph::new(lines), area);
            return;
        }

        panel_kit_tui::scroll::lines(frame, area, theme, &key, &mut demo.panels, lines);
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
                    Style::default().fg(theme.info),
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
        key: SpecPanelId,
        demo: &mut DemoData,
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
            &key,
            &mut demo.panels,
            TableView {
                columns: &model.columns,
                rows: &model.rows.iter().collect::<Vec<_>>(),
            },
            &panel_kit_core::widgets::data_table::TableState::default(),
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
                    Span::styled(model.label.as_str(), Style::default().fg(theme.text)),
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
                Span::styled(name, Style::default().fg(theme.text_muted)),
            ])
        };
        let mut lines = vec![Line::from(Span::styled(
            crate::workspace_canary::theme_preset_label(demo.paper),
            Style::default().fg(theme.text),
        ))];
        lines.extend(
            crate::workspace_canary::THEME_SWATCHES
                .iter()
                .map(|&(slot, name)| swatch(theme.color(slot), name)),
        );
        lines.push(spinner(demo.tick / 2, "spinner painter", theme));
        frame.render_widget(Paragraph::new(lines), area);
    }

    /// Shared notes sections painted as ratatui lines, closed by a spinner.
    fn notes_lines(theme: &ResolvedTuiTheme, tick: u64) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        for (index, (heading, body)) in crate::workspace_canary::NOTES.iter().enumerate() {
            if index > 0 {
                lines.push(Line::from(""));
            }
            lines.push(Line::from(Span::styled(
                *heading,
                Style::default().fg(theme.text).add_modifier(ratatui::style::Modifier::BOLD),
            )));
            lines.extend(body.iter().map(|line| Line::from(*line)));
        }
        lines.push(Line::from(""));
        lines.push(spinner(tick, "TUI canary running", theme));
        lines
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
