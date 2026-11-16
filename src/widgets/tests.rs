use dioxus::prelude::*;
use panel_kit_core::badge::{BadgeAction, BadgeKind, BadgeSpec};
use panel_kit_core::widgets::charts::{
    BoxItemView, FiveNum, FlameSpanModel, GaugeModel, SeriesView,
};
use panel_kit_core::widgets::forms::{FormInput, FormState};
use panel_kit_core::widgets::spinner::SpinnerModel;
use panel_kit_core::widgets::{ContentView, ScrollPolicy};

use super::content_view;
use super::snapshot_expectations;
use super::snapshot_semantics::assert_widget_snapshot;

#[allow(dead_code)]
#[path = "../../examples/support/canary_content.rs"]
mod canary_content;

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
fn WideSeriesProbe() -> Element {
    // x spans 0..29 and y spans 38..1450: the scale must read the values.
    let points = [(0.0, 38.0), (29.0, 1450.0)];
    let series = [SeriesView {
        name: "latency",
        points: &points,
    }];
    content_view(
        ContentView::TimeSeries {
            series: &series,
            unit: "ms",
        },
        0,
        noop_action(),
    )
}

#[test]
fn time_series_scale_labels_the_value_axis() {
    let html = dioxus_ssr::render_element(rsx! { WideSeriesProbe {} });

    assert!(
        html.contains(">1450 ms<"),
        "max label must be the largest value: {html}"
    );
    assert!(
        html.contains(">38 ms<"),
        "min label must be the smallest value: {html}"
    );
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
            {content_view(ContentView::Text { text: "auto wrapped", scroll: ScrollPolicy::Auto }, 0, noop_action())}
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

/// The canary form with its select open on the second option.
#[component]
fn FormProbe() -> Element {
    let model = canary_content::canary_inputs();
    let region = model.fields.iter().position(|field| field.id == "region");
    let state = FormState {
        focus: region,
        open: true,
        highlighted: Some(1),
        ..FormState::default()
    };
    super::forms::form(
        "probe",
        &model,
        &state,
        Some(EventHandler::new(|_: FormInput| {})),
    )
}

#[test]
fn form_paints_every_control_kind_with_its_semantics() {
    let html = dioxus_ssr::render_element(rsx! { FormProbe {} });

    assert_widget_snapshot(&html, snapshot_expectations::FORM);
    assert!(!html.contains("inert"), "an operable form is not inert: {html}");
    // Boolean attributes render unquoted, outside the snapshot's reach.
    let tag = |id: &str| {
        let start = html.find(id).unwrap_or_else(|| panic!("{id} not painted: {html}"));
        &html[start..start + html[start..].find('>').unwrap()]
    };
    assert!(tag(r#"id="probe-drain""#).contains("checked=true"), "the checked box");
    assert!(tag(r#"value="warm""#).contains("checked=true"), "the chosen radio");
    assert!(!tag(r#"value="hot""#).contains("checked=true"), "an unchosen radio");
    assert!(tag(r#"id="probe-owner""#).contains("disabled=true"), "the disabled field");
    assert!(!tag(r#"id="probe-host""#).contains("disabled=true"), "an enabled field");
}

#[component]
fn InertFormProbe() -> Element {
    let model = canary_content::canary_inputs();
    let state = FormState::default();
    content_view(ContentView::Form { model: &model, state: &state }, 0, noop_action())
}

#[test]
fn a_form_painted_without_a_handler_is_inert_and_closed() {
    let html = dioxus_ssr::render_element(rsx! { InertFormProbe {} });

    assert!(html.contains(r#"inert="true""#), "{html}");
    assert!(!html.contains("pk-dropdown-popup"), "a closed select paints no list: {html}");
    assert!(html.contains(r#"aria-expanded="false""#), "{html}");
}
