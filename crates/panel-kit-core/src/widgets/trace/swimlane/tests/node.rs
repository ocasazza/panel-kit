//! The inspector's content: what each kind of node resolves to.

use super::super::*;
use super::fixtures::ev;
use crate::widgets::trace::model::{Edge, EdgeKind, Event, EventKind, Lane, LaneKind};
use crate::widgets::trace::query::Tone;
use crate::widgets::trace::SessionTree;

fn id(event: &Event) -> String {
    event.id.clone()
}

/// `imp-1` with three events by two agents; the middle one titles and
/// statuses it; plus one loose event.
fn events() -> Vec<Event> {
    vec![
        ev("imp-1", 1.0).title("first draft"),
        ev("imp-1", 2.0)
            .agent("omp")
            .title("second draft")
            .status("review"),
        ev("imp-1", 3.0),
        ev("imp-2", 2.0),
        Event::new("loose:1", ev("x", 4.0).at, EventKind::Commit)
            .summary("a commit\nwith a body")
            .commit("abc123"),
    ]
}

fn tree() -> SessionTree {
    let lane = |id: &str, item: &str| Lane {
        id: id.to_string(),
        item_id: item.to_string(),
        kind: LaneKind::Branch,
        label: format!("branch {id}"),
        from: ev("a", 1.0).at,
        to: ev("a", 2.0).at,
        chosen: Some(false),
        parent: None,
        agent: Some("omp".to_string()),
    };
    let edge = |lane: &str, item: &str| Edge {
        kind: EdgeKind::Fork,
        lane: lane.to_string(),
        item_id: item.to_string(),
        at: ev("a", 1.0).at,
    };
    SessionTree {
        agent: None,
        lanes: vec![lane("imp-1/b", "imp-1"), lane("imp-2/b", "imp-2")],
        edges: vec![edge("imp-1/b", "imp-1"), edge("imp-2/b", "imp-2")],
    }
}

#[test]
fn an_item_node_states_the_item_and_opens_no_row() {
    let events = events();
    let got = Node::Item("imp-1".to_string())
        .inspect(&events, Some(&tree()), 56)
        .unwrap();
    let head = got.header.unwrap();
    assert_eq!(head.id, "imp-1");
    assert_eq!(head.title.as_deref(), Some("second draft"));
    assert_eq!(head.status.as_deref(), Some("review"));
    assert_eq!(head.tone, Tone::Yellow);
    assert_eq!(head.events, 3);
    assert_eq!(head.agents, ["omp", "worker"]);
    assert_eq!(head.lanes, 1);
    assert!(head.first < head.last);
    assert!(got.open.is_empty() && got.more == 0 && got.blocks.is_empty());
    assert_eq!(got.item.unwrap().id, "imp-1");
}

#[test]
fn the_tree_is_pruned_to_the_item() {
    let events = events();
    let got = Node::Item("imp-2".to_string())
        .inspect(&events, Some(&tree()), 56)
        .unwrap();
    assert_eq!(got.tree.lanes.len(), 1);
    assert_eq!(got.tree.lanes[0].id, "imp-2/b");
    assert_eq!(got.tree.edges.len(), 1);
}

#[test]
fn an_event_node_is_its_item_with_that_row_open() {
    let events = events();
    let got = Node::Event(id(&events[1]))
        .inspect(&events, None, 56)
        .unwrap();
    assert_eq!(got.header.unwrap().id, "imp-1");
    assert_eq!(got.open, [id(&events[1])]);
}

#[test]
fn a_cluster_opens_its_members_up_to_twelve_and_counts_the_rest() {
    let many: Vec<Event> = (0..15).map(|n| ev("imp-1", f64::from(n))).collect();
    let ids: Vec<String> = many.iter().map(id).collect();
    let got = Node::Cluster(ids.clone()).inspect(&many, None, 56).unwrap();
    assert_eq!(got.open, ids[..CLUSTER_OPEN_CAP]);
    assert_eq!(got.more, 3);
    let few = Node::Cluster(ids[..4].to_vec())
        .inspect(&many, None, 56)
        .unwrap();
    assert_eq!((few.open.len(), few.more), (4, 0));
}

#[test]
fn a_lane_node_is_a_fact_block() {
    let events = events();
    let got = Node::Lane("imp-1/b".to_string())
        .inspect(&events, Some(&tree()), 56)
        .unwrap();
    assert!(got.header.is_none() && got.item.is_none());
    assert_eq!(got.blocks.len(), 1);
    assert_eq!(got.blocks[0].label, "branch imp-1/b");
    let labels: Vec<&str> = got.blocks[0].facts.iter().map(|f| f.label).collect();
    assert!(labels.contains(&"outcome") && labels.contains(&"agent"));
}

#[test]
fn an_event_without_an_item_is_a_fact_block_not_a_rail() {
    let events = events();
    let got = Node::Event("loose:1".to_string())
        .inspect(&events, None, 56)
        .unwrap();
    assert!(got.item.is_none() && got.header.is_none());
    let text = got.blocks[0]
        .facts
        .iter()
        .find(|f| f.label == "text")
        .unwrap();
    assert_eq!(text.lines, ["a commit", "with a body"]);
}

#[test]
fn a_node_that_names_nothing_resolves_to_none() {
    let events = events();
    assert!(Node::Item("nope".to_string())
        .inspect(&events, None, 56)
        .is_none());
    assert!(Node::Event("nope".to_string())
        .inspect(&events, None, 56)
        .is_none());
    assert!(Node::Cluster(vec!["nope".to_string()])
        .inspect(&events, None, 56)
        .is_none());
    assert!(Node::Lane("nope".to_string())
        .inspect(&events, Some(&tree()), 56)
        .is_none());
    assert!(Node::Lane("imp-1/b".to_string())
        .inspect(&events, None, 56)
        .is_none());
}

#[test]
fn keys_differ_per_node_and_a_node_round_trips_through_json() {
    let nodes = [
        Node::Item("a".to_string()),
        Node::Event("a".to_string()),
        Node::Cluster(vec!["a".to_string(), "b".to_string()]),
        Node::Lane("a".to_string()),
    ];
    let keys: std::collections::BTreeSet<String> = nodes.iter().map(Node::key).collect();
    assert_eq!(keys.len(), nodes.len());
    for node in nodes {
        let json = serde_json::to_string(&node).unwrap();
        assert_eq!(serde_json::from_str::<Node>(&json).unwrap(), node);
    }
}

#[test]
fn a_merged_burst_keeps_every_member_id_in_draw_order() {
    let a = ev("imp-1", 1.0);
    let b = ev("imp-1", 1.0001);
    let c = ev("imp-1", 9.0);
    let dots = cluster([(&a, 10.0), (&b, 12.0), (&c, 100.0)]);
    assert_eq!(dots[0].members, [id(&a), id(&b)]);
    assert_eq!(dots[1].members, [id(&c)]);
    let merged = merge(dots);
    assert_eq!(merged[0].members, [id(&a), id(&b)]);
    let joined = merge(vec![Dot::of(&a, 10.0), Dot::of(&b, 11.0)]);
    assert_eq!(joined[0].members, [id(&a), id(&b)]);
}
