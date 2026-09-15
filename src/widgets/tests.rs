use dioxus::prelude::*;
use panel_kit_core::badge::{BadgeAction, BadgeKind, BadgeSpec};
use panel_kit_core::widgets::charts::{
    BoxItemView, FiveNum, FlameSpanModel, GaugeModel, SeriesView,
};
use panel_kit_core::widgets::meter::MeterModel;
use panel_kit_core::widgets::spinner::SpinnerModel;
use panel_kit_core::widgets::status::{StatusModel, StatusState};
use panel_kit_core::widgets::table::{
    ColumnWidth, TableCell, TableColumn, TableRow, TableView, TextAlign,
};
use panel_kit_core::widgets::{ContentView, ScrollPolicy};

use super::content_view;
use super::snapshot_expectations;
use super::snapshot_semantics::assert_widget_snapshot;

fn noop_action() -> EventHandler<BadgeAction> {
    EventHandler::new(|_: BadgeAction| {})
}

#[component]
fn ChartsProbe() -> Element {
    let points = [(0.0, 1.0), (1.0, 3.5), (2.0, 2.0)];
    let series = [SeriesView {
        name: "latency",
        points: &points,
    }];
    let gauges = [GaugeModel {
        label: "queue".into(),
        ratio: 0.375,
        text: "3/8".into(),
    }];
    let flame = [FlameSpanModel {
        label: "root".into(),
        depth: 0,
        value: 8.0,
        color: Some((10, 20, 30)),
    }];
    let boxes = [BoxItemView {
        label: "p95",
        summary: FiveNum {
            min: 1.0,
            q1: 2.0,
            median: 3.0,
            q3: 4.0,
            max: 5.0,
        },
        color: None,
    }];

    rsx! {
        div { class: "snapshot",
            {content_view(ContentView::TimeSeries { series: &series, unit: "ms" }, 0, noop_action())}
            {content_view(ContentView::Gauges(&gauges), 0, noop_action())}
            {content_view(ContentView::Flamegraph(&flame), 0, noop_action())}
            {content_view(ContentView::Boxplot(&boxes), 0, noop_action())}
        }
    }
}

#[test]
fn charts_render_fixed_content_view_snapshots() {
    let html = dioxus_ssr::render_element(rsx! { ChartsProbe {} });

    assert_widget_snapshot(&html, snapshot_expectations::CHARTS);
}

#[component]
fn TableMeterStatusProbe() -> Element {
    let columns = [
        TableColumn {
            key: "service".into(),
            title: "Service".into(),
            width: ColumnWidth::Flex { weight: 2 },
            align: TextAlign::Left,
        },
        TableColumn {
            key: "health".into(),
            title: "Health".into(),
            width: ColumnWidth::Fixed { value: 8 },
            align: TextAlign::Center,
        },
        TableColumn {
            key: "load".into(),
            title: "Load".into(),
            width: ColumnWidth::Fixed { value: 10 },
            align: TextAlign::Right,
        },
    ];
    let rows = [TableRow {
        cells: vec![
            TableCell::Text("api".into()),
            TableCell::Status {
                label: "ok".into(),
                color: (39, 201, 63),
            },
            TableCell::Meter {
                ratio: 0.42,
                text: "42%".into(),
                color: Some((94, 243, 140)),
            },
        ],
    }];
    let meter = MeterModel {
        label: "memory".into(),
        ratio: 0.625,
        text: "5 GiB / 8 GiB".into(),
        color: Some((94, 243, 140)),
    };
    let status = StatusModel {
        label: "deploy".into(),
        state: StatusState::Warning,
        color: (255, 189, 46),
    };

    rsx! {
        div { class: "snapshot",
            {content_view(ContentView::Table(TableView { columns: &columns, rows: &rows }), 0, noop_action())}
            {content_view(ContentView::Meter(&meter), 0, noop_action())}
            {content_view(ContentView::Status(&status), 0, noop_action())}
        }
    }
}

#[test]
fn table_meter_and_status_render_semantic_dom_from_borrowed_views() {
    let html = dioxus_ssr::render_element(rsx! { TableMeterStatusProbe {} });

    assert_widget_snapshot(&html, snapshot_expectations::TABLE_METER_STATUS);
}

#[component]
fn ScrollSpinnerProbe() -> Element {
    let spinner = SpinnerModel {
        label: Some("syncing".into()),
    };

    rsx! {
        div { class: "snapshot",
            {content_view(ContentView::Text { text: "one\ntwo", scroll: ScrollPolicy::Clip }, 0, noop_action())}
            {content_view(ContentView::Text { text: "alpha beta", scroll: ScrollPolicy::Wrap }, 0, noop_action())}
            {content_view(ContentView::Spinner { label: "indexing" }, 10, noop_action())}
            {super::spinner::from_model(&spinner, 0)}
        }
    }
}

#[test]
fn scroll_and_spinner_consume_core_policy_and_model_inputs() {
    let html = dioxus_ssr::render_element(rsx! { ScrollSpinnerProbe {} });

    assert_widget_snapshot(&html, snapshot_expectations::SCROLL_SPINNER);
}

#[component]
fn BadgeStripProbe() -> Element {
    let badges = [
        BadgeSpec {
            active: true,
            with_plus: true,
            with_x: true,
            ..BadgeSpec::new("tag", "alpha", BadgeKind::Tag)
        },
        BadgeSpec::new(
            "link",
            "Panel Kit",
            BadgeKind::Wikilink {
                resolved: true,
                target: "Panel Kit".into(),
            },
        ),
    ];

    content_view(ContentView::Badges(&badges), 0, noop_action())
}

#[test]
fn badge_strip_renders_badges_from_core_specs_without_model_duplication() {
    let html = dioxus_ssr::render_element(rsx! { BadgeStripProbe {} });

    assert_widget_snapshot(&html, snapshot_expectations::BADGE_STRIP);
}
