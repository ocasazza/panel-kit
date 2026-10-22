//! The GRAPH body: the head (no-item rows, legend, show-all) over the canvas,
//! the width the layout is made at, and the scroller the canvas is windowed by.

use dioxus::prelude::*;
use panel_kit_core::widgets::event_graph::query::{show_all_label, GraphAction, LegendChip};
use panel_kit_core::widgets::event_graph::swimlane::metrics::ROW;
use panel_kit_core::widgets::event_graph::swimlane::{content_width, Graph, Node, StripRow};

use super::canvas::Canvas;
use super::dom;
use super::legend::legend;
use super::shared::Shared;
use super::window;

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

/// Widest count printed in full beside a no-item label; a longer one is cut to
/// `12.4k` or `1.2M`, the exact figure staying in the tooltip.
const COUNT_CELLS: usize = 6;

/// `count` as digits while they fit `cells`, else with a `k`, `M` or `G` tail:
/// one decimal under 10 and 100 of a unit would hide, none past it.
fn fit_count(count: usize, cells: usize) -> String {
    let digits = count.to_string();
    if digits.len() <= cells {
        return digits;
    }
    let (unit, tail) = match count {
        0..=999_999 => (1e3, 'k'),
        1_000_000..=999_999_999 => (1e6, 'M'),
        _ => (1e9, 'G'),
    };
    let scaled = count as f64 / unit;
    if scaled < 100.0 {
        format!("{:.1}{tail}", (scaled * 10.0).floor() / 10.0)
    } else {
        format!("{}{tail}", scaled.floor())
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
            span { id: "{id}-strip-count-{kind}", class: "pk-ag-count", "{fit_count(row.count, COUNT_CELLS)}" }
        }
    }
}

/// The graph figure. It reports its content width whenever it differs from
/// the width the layout was made at, and the host answers with a new layout.
/// The first report is immediate, the default width being a guess; the rest
/// wait out a resize in progress. The scroller around the canvas reports how
/// far it is scrolled when the rows on screen leave the mounted ones.
#[component]
pub(super) fn GraphBody(
    id: String,
    graph: Shared<Graph>,
    events: Shared<Vec<panel_kit_core::widgets::event_graph::Event>>,
    chips: Vec<LegendChip>,
    inline_legend: bool,
    show_all: bool,
    selected: Option<String>,
    on_action: EventHandler<GraphAction>,
    on_select: EventHandler<String>,
    on_open: EventHandler<Node>,
) -> Element {
    let mut ticket = use_signal(|| 0u64);
    let mut scroll = use_signal(|| (0.0, window::DEFAULT_VIEWPORT));
    let canvas = format!("{id}-canvas");
    let laid_out = graph.width;
    let rows = graph.rows;
    let on_resize = {
        let canvas = canvas.clone();
        move |event: Event<ResizeData>| {
            let Ok(size) = event.get_content_box_size() else {
                return;
            };
            let (top, height) = *scroll.peek();
            let measured = size.height.min(window::MAX_VIEWPORT);
            if measured >= 1.0 && (measured - height).abs() >= 1.0 {
                let top = dom::scroll(&canvas).map_or(top, |now| now.top);
                scroll.set((top, measured));
            }
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
        }
    };
    let on_scroll = {
        let canvas = canvas.clone();
        let items = graph.item_count();
        move |_| {
            let Some(now) = dom::scroll(&canvas) else {
                return;
            };
            let (top, height) = *scroll.peek();
            let mounted = window::mounted(items, top, height, ROW, rows);
            let measured = now.height.min(window::MAX_VIEWPORT);
            if !window::covers(mounted, now.top, measured, ROW, rows) {
                scroll.set((now.top, measured));
            }
        }
    };
    // A new layout moves the rows under the scroller, which the browser
    // clamps without a window of ours to tell: read it again.
    use_effect(use_reactive((&graph,), {
        let canvas = canvas.clone();
        move |_| {
            let Some(now) = dom::scroll(&canvas) else {
                return;
            };
            let measured = now.height.min(window::MAX_VIEWPORT);
            if measured >= 1.0 && *scroll.peek() != (now.top, measured) {
                scroll.set((now.top, measured));
            }
        }
    }));
    // Focus scrolls a row clear of the strip and the axis pinned over it.
    let pinned = format!(
        "scroll-padding: {}px 0 {}px;",
        graph.strip.len() as f64 * ROW,
        graph.height as f64 - graph.axis_y
    );

    rsx! {
        figure { id: "{id}-graph", class: "pk-ag-fig",
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
                div {
                    class: "pk-ag-canvas",
                    id: "{canvas}",
                    style: "{pinned}",
                    "data-rows": "{graph.rows}",
                    onresize: on_resize,
                    onscroll: on_scroll,
                    Canvas {
                        id: id.clone(),
                        graph: graph.clone(),
                        events,
                        scroll,
                        selected,
                        on_action,
                        on_select,
                        on_open,
                    }
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

    #[test]
    fn a_count_is_printed_in_full_while_it_fits_and_cut_to_a_tail_after() {
        for count in [0, 7, 758, 12_412, 333_333, 999_999] {
            assert_eq!(fit_count(count, COUNT_CELLS), count.to_string());
        }
        assert_eq!(fit_count(1_000_000, COUNT_CELLS), "1.0M");
        assert_eq!(fit_count(1_249_999, COUNT_CELLS), "1.2M");
        assert_eq!(fit_count(12_400_000, COUNT_CELLS), "12.4M");
        assert_eq!(fit_count(123_456_789, COUNT_CELLS), "123M");
        assert_eq!(fit_count(2_000_000_000, COUNT_CELLS), "2.0G");
    }

    #[test]
    fn a_narrower_cell_budget_cuts_thousands_to_a_k() {
        assert_eq!(fit_count(9_999, 4), "9999");
        assert_eq!(fit_count(12_412, 4), "12.4k");
        assert_eq!(fit_count(124_120, 4), "124k");
        assert_eq!(fit_count(999_999, 4), "999k");
    }
}
