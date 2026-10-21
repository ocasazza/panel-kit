//! The GRAPH body: the head (no-item rows, legend, show-all) over the canvas,
//! and the width the layout is made at.

use dioxus::prelude::*;
use panel_kit_core::widgets::event_graph::query::{show_all_label, GraphAction, LegendChip};
use panel_kit_core::widgets::event_graph::swimlane::{content_width, Graph, StripRow};

use super::canvas::Canvas;
use super::dom;
use super::legend::legend;
use super::shared::Shared;

/// Quiet time after the last resize before the new width is reported.
const WIDTH_DEBOUNCE_MS: i32 = 150;

/// What a measured width asks of the figure.
#[derive(Debug, PartialEq, Eq)]
enum Report {
    /// Nothing: the container is hidden or the layout already has this width.
    Skip,
    /// The first measurement: the default width was a guess, so lay out now.
    Now,
    /// A later measurement: report once the resize has settled.
    Later,
}

/// `next` is the measured content width, `laid_out` the layout's, and `seen`
/// counts the measurements of this figure including this one.
fn report(next: u32, laid_out: u32, seen: u64) -> Report {
    if next == 0 || next == laid_out {
        Report::Skip
    } else if seen == 1 {
        Report::Now
    } else {
        Report::Later
    }
}

/// A no-item row's toggle: its label, the event count and `aria-expanded`.
fn strip_button(id: &str, row: &StripRow, on_action: EventHandler<GraphAction>) -> Element {
    let kind = row.kind.as_str();
    rsx! {
        button {
            key: "{kind}",
            id: "{id}-strip-{kind}",
            r#type: "button",
            class: if row.open { "pk-ag-strip-toggle on" } else { "pk-ag-strip-toggle" },
            aria_expanded: "{row.open}",
            title: "{row.title()}",
            onclick: move |_| on_action.call(GraphAction::ToggleStripKind(kind.to_string())),
            span { class: "pk-ag-tl-label", "{row.label()}" }
            span { id: "{id}-strip-count-{kind}", class: "pk-ag-count", "{row.count}" }
        }
    }
}

/// The graph figure. It reports its content width whenever it differs from
/// the width the layout was made at, and the host answers with a new layout.
/// The first report is immediate, the default width being a guess; the rest
/// wait out a resize in progress.
#[component]
pub(super) fn GraphBody(
    id: String,
    graph: Shared<Graph>,
    chips: Vec<LegendChip>,
    inline_legend: bool,
    show_all: bool,
    selected: Option<String>,
    on_action: EventHandler<GraphAction>,
    on_select: EventHandler<String>,
) -> Element {
    let mut ticket = use_signal(|| 0u64);
    let laid_out = graph.width;
    let on_resize = move |event: Event<ResizeData>| {
        let Ok(size) = event.get_content_box_size() else {
            return;
        };
        let next = content_width(size.width, 0.0, 0.0);
        // Every resize voids the report still waiting, even one that ends
        // back at the laid-out width.
        let seen = *ticket.peek() + 1;
        ticket.set(seen);
        match report(next, laid_out, seen) {
            Report::Skip => return,
            Report::Now => {
                on_action.call(GraphAction::GraphWidth(i64::from(next)));
                return;
            }
            Report::Later => {}
        }
        spawn(async move {
            dom::sleep(WIDTH_DEBOUNCE_MS).await;
            if *ticket.peek() == seen {
                on_action.call(GraphAction::GraphWidth(i64::from(next)));
            }
        });
    };

    rsx! {
        figure { id: "{id}-graph", class: "pk-ag-fig", onresize: on_resize,
            if graph.is_empty() {
                p { id: "{id}-graph-empty", class: "pk-ag-empty", "nothing happened in this window" }
            } else {
                div { id: "{id}-graph-head", class: "pk-ag-head",
                    for row in graph.strip.iter() {
                        {strip_button(&id, row, on_action)}
                    }
                    if inline_legend {
                        {legend(&id, &chips, false, on_action)}
                    }
                    if graph.hidden > 0 || show_all {
                        button {
                            id: "{id}-show-all",
                            r#type: "button",
                            class: if show_all { "pk-ag-strip-toggle on" } else { "pk-ag-strip-toggle" },
                            aria_pressed: "{show_all}",
                            onclick: move |_| on_action.call(GraphAction::ToggleShowAll),
                            "{show_all_label(show_all, graph.total_items)}"
                        }
                    }
                }
                Canvas {
                    id: id.clone(),
                    graph: graph.clone(),
                    selected,
                    on_action,
                    on_select,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hidden_container_or_an_unchanged_width_reports_nothing() {
        assert_eq!(report(0, 1200, 1), Report::Skip);
        assert_eq!(report(0, 1200, 7), Report::Skip);
        assert_eq!(report(882, 882, 1), Report::Skip);
        assert_eq!(report(882, 882, 4), Report::Skip);
    }

    #[test]
    fn the_first_measurement_lays_out_at_once_and_later_ones_wait() {
        assert_eq!(report(882, 1200, 1), Report::Now);
        assert_eq!(report(640, 882, 2), Report::Later);
        assert_eq!(report(640, 882, 9), Report::Later);
    }
}
