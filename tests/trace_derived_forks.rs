//! The default sample window with the tree on, against real SSR output. The
//! filler items hold no explicit lanes, so every fork the painter draws past
//! the heroes' own comes from the events of its item.

use std::rc::Rc;

use dioxus::prelude::*;
use panel_kit::widgets::trace::swimlane::Trace;
use panel_kit_core::widgets::trace::query::GraphState;
use panel_kit_core::widgets::trace::sample::{self, Activity};

#[derive(Clone)]
struct Case {
    state: GraphState,
    activity: Activity,
}

impl PartialEq for Case {
    fn eq(&self, _: &Self) -> bool {
        false
    }
}

#[component]
fn Probe(case: Case) -> Element {
    rsx! {
        Trace {
            state: case.state,
            events: Rc::new(case.activity.events),
            tree: Some(Rc::new(case.activity.tree)),
            now: sample::DEMO_NOW,
            on_action: |_| {},
        }
    }
}

fn render(state: GraphState) -> String {
    let case = Case {
        state,
        activity: sample::activity(),
    };
    let mut dom = VirtualDom::new_with_props(Probe, ProbeProps { case });
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

fn state(show_all: bool, tree: bool) -> GraphState {
    GraphState {
        window: "30d".to_string(),
        tree,
        compact: true,
        graph_width: 1200,
        show_all,
        ..GraphState::default()
    }
}

/// Fork leaves drawn: the tip square at the end of each fork lane.
fn fork_leaves(html: &str) -> usize {
    html.matches("class=\"pk-tr-fork-node").count()
}

#[test]
fn the_default_sample_draws_many_more_fork_leaves_than_its_twenty_heroes_hold() {
    let heroes = sample::activity().tree.lanes.len();
    let capped = fork_leaves(&render(state(false, true)));
    let all = fork_leaves(&render(state(true, true)));
    eprintln!("{heroes} hero lanes; {capped} fork leaves capped, {all} with the first rows mounted");

    // The heroes without explicit lanes draw derived ones.
    assert!(capped > heroes, "{capped} leaves for {heroes} hero lanes");
    // One screen of the full window holds more than twice the heroes' lanes.
    assert!(
        all >= 2 * heroes,
        "{all} leaves for {heroes} hero lanes: the filler's tree is missing"
    );
}

#[test]
fn with_the_tree_off_the_default_sample_draws_no_fork_leaf() {
    assert_eq!(fork_leaves(&render(state(true, false))), 0);
    assert_eq!(fork_leaves(&render(state(false, false))), 0);
}
