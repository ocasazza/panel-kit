//! The colour legend: one toggle per tone. A pressed chip keeps its tone's
//! dots lit and a faded chip stands for a tone the filter hides.

use dioxus::prelude::*;
use panel_kit_core::widgets::trace::query::{GraphAction, LegendChip};

/// The legend as a list of tone toggles. `compact` drops the meanings and
/// keeps the colour words; a clear button ends the list while a tone is picked.
pub(super) fn legend(
    id: &str,
    chips: &[LegendChip],
    compact: bool,
    on_action: EventHandler<GraphAction>,
) -> Element {
    let picked = chips.iter().any(|chip| chip.pressed);
    let list = if compact {
        "pk-tr-legend pk-tr-legend-compact"
    } else {
        "pk-tr-legend"
    };
    rsx! {
        ul { id: "{id}-graph-legend", class: list, aria_label: "legend",
            for chip in chips.iter() {
                {
                    let tone = chip.tone;
                    let name = tone.name();
                    let class = match (chip.pressed, chip.dimmed) {
                        (true, _) => "pk-tr-lg on",
                        (false, true) => "pk-tr-lg dim",
                        (false, false) => "pk-tr-lg",
                    };
                    rsx! {
                        li {
                            key: "{name}",
                            class: "pk-tr-legend-{name}",
                            "data-tone": "{name}",
                            title: "{chip.title}",
                            button {
                                r#type: "button",
                                class: class,
                                aria_pressed: "{chip.pressed}",
                                onclick: move |_| on_action.call(GraphAction::ToggleTone(tone)),
                                svg {
                                    class: "pk-tr-lg-dot",
                                    width: "10",
                                    height: "10",
                                    view_box: "0 0 10 10",
                                    "aria-hidden": "true",
                                    circle { class: "pk-tr-dot pk-tr-{name}", cx: "5", cy: "5", r: "3" }
                                }
                                span { class: "pk-tr-legend-name", "{chip.colour}" }
                                if !compact {
                                    span { class: "pk-tr-legend-means", "{chip.brief}" }
                                }
                            }
                        }
                    }
                }
            }
            if picked {
                li { class: "pk-tr-legend-clear",
                    button {
                        r#type: "button",
                        class: "pk-tr-lg-clear",
                        title: "show every colour again",
                        aria_label: "clear colour filter",
                        onclick: move |_| on_action.call(GraphAction::ClearTones),
                        "×"
                    }
                }
            }
        }
    }
}
