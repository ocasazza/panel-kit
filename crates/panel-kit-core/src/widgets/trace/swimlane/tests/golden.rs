//! Differential check against the board's own Elixir `Graph.layout/2` and
//! `Warp.scale/3`: seeded random scenarios were run through the real modules
//! (`golden/gen.exs`) and their inputs and outputs recorded in
//! `golden/scenarios.json`; the port must reproduce every number, string and
//! ordering, floats to the last bit. `GOLDEN=<file>` replays another run.

use serde_json::{json, Value};

use super::super::*;
use crate::widgets::trace::model::{
    Edge, EdgeKind, Event, EventKind, Lane, LaneKind, SessionTree, Source,
};

/// serde_json parses floats approximately, so every number with a fraction
/// or exponent is quoted as `"$f:<text>"` first and read back with the exact
/// std parser.
fn tag_floats(raw: &str) -> String {
    let mut chars = raw.chars().peekable();
    let mut out = String::with_capacity(raw.len() + raw.len() / 8);
    let mut in_string = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            match c {
                '\\' => out.extend(chars.next()),
                '"' => in_string = false,
                _ => {}
            }
        } else if c == '"' {
            in_string = true;
            out.push(c);
        } else if c == '-' || c.is_ascii_digit() {
            let mut token = String::from(c);
            while let Some(&next) = chars.peek() {
                if !(next.is_ascii_digit() || matches!(next, '.' | 'e' | 'E' | '+' | '-')) {
                    break;
                }
                token.push(next);
                chars.next();
            }
            if token.contains(['.', 'e', 'E']) {
                out.push_str(&format!("\"$f:{token}\""));
            } else {
                out.push_str(&token);
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn revive(v: Value) -> Value {
    match v {
        Value::String(text) if text.starts_with("$f:") => text["$f:".len()..]
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map_or(Value::Null, Value::Number),
        Value::Object(map) => Value::Object(map.into_iter().map(|(k, v)| (k, revive(v))).collect()),
        Value::Array(items) => Value::Array(items.into_iter().map(revive).collect()),
        other => other,
    }
}

fn load() -> Value {
    let raw = match std::env::var("GOLDEN") {
        Ok(path) => std::fs::read_to_string(path).expect("GOLDEN file"),
        Err(_) => include_str!("golden/scenarios.json").to_string(),
    };
    revive(serde_json::from_str(&tag_floats(&raw)).expect("golden json"))
}

fn s(v: &Value) -> Option<String> {
    v.as_str().map(str::to_string)
}

fn event(v: &Value) -> Event {
    let mut e = Event::new(
        v["id"].as_str().unwrap(),
        v["at"].as_i64().unwrap(),
        EventKind::from_name(v["kind"].as_str().unwrap()).unwrap(),
    );
    e.agent = s(&v["agent"]);
    e.item_id = s(&v["item_id"]);
    e.summary = s(&v["summary"]);
    e.detail.status = s(&v["status"]);
    e.detail.title = s(&v["title"]);
    e
}

fn lane_kind(name: &str) -> LaneKind {
    match name {
        "path" => LaneKind::Path,
        "branch" => LaneKind::Branch,
        "handoff" => LaneKind::Handoff,
        _ => LaneKind::Collab,
    }
}

fn tree(v: &Value) -> SessionTree {
    SessionTree {
        agent: None,
        lanes: v["lanes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| Lane {
                id: s(&l["id"]).unwrap(),
                item_id: s(&l["item_id"]).unwrap(),
                kind: lane_kind(l["kind"].as_str().unwrap()),
                label: s(&l["label"]).unwrap(),
                from: l["from"].as_i64().unwrap(),
                to: l["to"].as_i64().unwrap(),
                chosen: l["chosen"].as_bool(),
                parent: None,
                agent: s(&l["agent"]),
            })
            .collect(),
        edges: v["edges"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| Edge {
                kind: if e["kind"] == "fork" {
                    EdgeKind::Fork
                } else {
                    EdgeKind::Merge
                },
                lane: s(&e["lane"]).unwrap(),
                item_id: s(&e["item_id"]).unwrap(),
                at: e["at"].as_i64().unwrap(),
            })
            .collect(),
    }
}

fn dot(d: &Dot) -> Value {
    json!({
        "id": d.id, "at": d.at, "kind": d.kind.as_str(), "agent": d.agent,
        "tone": d.tone.name(), "title": d.title, "count": d.count, "x": d.x,
    })
}

fn actual(g: &Graph) -> Value {
    json!({
        "width": g.width, "height": g.height, "lanes": g.lanes, "rows": g.rows,
        "total_items": g.total_items, "hidden": g.hidden, "axis_y": g.axis_y, "loose_y": g.loose_y,
        "items": g.items.iter().map(|i| json!({
            "id": i.id, "title": i.title, "status": i.status, "tone": i.tone.name(), "done": i.done,
            "lane": i.lane, "y": i.y, "x0": i.x0, "x1": i.x1, "label_x": i.label_x,
            "start_label_x": i.start_label_x, "last": i.last,
            "dots": i.dots.iter().map(dot).collect::<Vec<_>>(),
            "forks": i.forks.iter().map(|f| json!({
                "id": f.id, "kind": f.kind.as_str(), "label": f.label, "title": f.title,
                "chosen": f.chosen, "dead": f.dead, "y": f.y, "x0": f.x0, "x1": f.x1,
            })).collect::<Vec<_>>(),
            "edges": i.edges.iter().map(|e| json!({"kind": e.kind.as_str(), "dead": e.dead, "d": e.d})).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "strip": g.strip.iter().map(|r| json!({
            "kind": r.kind.as_str(), "count": r.count, "open": r.open, "y0": r.y0,
            "cells": r.cells.iter().map(|c| json!({"x": c.x, "w": c.w, "count": c.count, "alpha": c.alpha})).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "loose": g.loose.iter().map(|r| json!({
            "kind": r.kind.as_str(), "count": r.count, "y": r.y,
            "dots": r.dots.iter().map(dot).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "ticks": g.ticks.iter().map(|t| json!({"x": t.x, "label": t.label, "kind": t.kind.as_str()})).collect::<Vec<_>>(),
        "gaps": g.gaps.iter().map(|g| json!({"x0": g.x0, "x1": g.x1, "ms": g.ms})).collect::<Vec<_>>(),
    })
}

/// An SVG path with every number re-printed to one decimal. Elixir prints
/// the float 1100.0 as `1.1e3`, which is the same number; the port prints it
/// as `1100.0`.
fn canonical_path(d: &str) -> String {
    let mut out = String::new();
    let mut number = String::new();
    let flush = |number: &mut String, out: &mut String| {
        if !number.is_empty() {
            match number.parse::<f64>() {
                Ok(n) => out.push_str(&format!("{n:.1}")),
                Err(_) => out.push_str(number),
            }
            number.clear();
        }
    };
    for c in d.chars() {
        if c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E' | '-') {
            number.push(c);
        } else {
            flush(&mut number, &mut out);
            out.push(c);
        }
    }
    flush(&mut number, &mut out);
    out
}

/// First difference between two JSON values, as a path and both sides.
fn diff(path: &str, want: &Value, got: &Value) -> Option<String> {
    match (want, got) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, v) in a {
                match b.get(k) {
                    None => return Some(format!("{path}.{k}: missing in port")),
                    Some(w) => {
                        if let Some(d) = diff(&format!("{path}.{k}"), v, w) {
                            return Some(d);
                        }
                    }
                }
            }
            None
        }
        (Value::Array(a), Value::Array(b)) => {
            if a.len() != b.len() {
                return Some(format!("{path}: len {} vs port {}", a.len(), b.len()));
            }
            a.iter()
                .zip(b)
                .enumerate()
                .find_map(|(i, (x, y))| diff(&format!("{path}[{i}]"), x, y))
        }
        (Value::Number(a), Value::Number(b)) => {
            let same = match (a.as_f64(), b.as_f64()) {
                (Some(x), Some(y)) => x == y,
                _ => a == b,
            };
            (!same).then(|| format!("{path}: {a} vs port {b}"))
        }
        (Value::String(a), Value::String(b)) if path.ends_with(".d") => {
            let (a, b) = (canonical_path(a), canonical_path(b));
            (a != b).then(|| format!("{path}: {a} vs port {b}"))
        }
        _ => (want != got).then(|| format!("{path}: {want} vs port {got}")),
    }
}

#[test]
fn layouts_match_the_elixir_graph() {
    let golden = load();
    let mut failures = Vec::new();
    for case in golden["layouts"].as_array().unwrap() {
        let events: Vec<Event> = case["events"]
            .as_array()
            .unwrap()
            .iter()
            .map(event)
            .collect();
        let loose: Vec<Event> = case["loose"]
            .as_array()
            .unwrap()
            .iter()
            .map(event)
            .collect();
        let tree = tree(&case["tree"]);
        let kinds: Vec<EventKind> = match &case["loose_open"] {
            Value::Array(names) => names
                .iter()
                .filter_map(|n| EventKind::from_name(n.as_str().unwrap()))
                .collect(),
            _ => vec![],
        };
        let open = match &case["loose_open"] {
            Value::String(s) if s == "all" => LooseOpen::All,
            Value::Array(_) => LooseOpen::Kinds(&kinds),
            _ => LooseOpen::None,
        };
        let options = Options::new(
            case["width"].as_u64().unwrap() as u32,
            Range::new(
                case["since"].as_i64().unwrap(),
                case["now"].as_i64().unwrap(),
            ),
        )
        .limit(case["limit"].as_u64().map(|n| n as usize))
        .loose(&loose)
        .loose_open(open)
        .tree(&tree);

        let graph = Graph::layout(&events, &options);
        if let Some(d) = diff("", &case["expected"], &actual(&graph)) {
            failures.push(format!("{}: {d}", case["name"]));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        golden["layouts"].as_array().unwrap().len(),
        failures.join("\n")
    );
}

#[test]
fn warps_match_the_elixir_warp() {
    let golden = load();
    let mut failures = Vec::new();
    for case in golden["warps"].as_array().unwrap() {
        let times: Vec<i64> = case["times"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t.as_i64().unwrap())
            .collect();
        let params = WarpParams {
            since: case["since"].as_i64(),
            now: case["now"].as_i64(),
            left: case["left"].as_f64().unwrap(),
            right: case["right"].as_f64().unwrap(),
            ..WarpParams::default()
        };
        let warp = Warp::scale(
            times.iter().copied(),
            case["width"].as_f64().unwrap(),
            &params,
        );
        let xs: Vec<Value> = case["probes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| json!(warp.x(t.as_i64().unwrap())))
            .collect();
        let got = json!({
            "xs": xs,
            "ticks": warp.ticks.iter().map(|t| json!({"x": t.x, "label": t.label, "kind": t.kind.as_str()})).collect::<Vec<_>>(),
            "gaps": warp.gaps.iter().map(|g| json!({"x0": g.x0, "x1": g.x1, "ms": g.ms})).collect::<Vec<_>>(),
        });
        let want = json!({"xs": case["xs"], "ticks": case["ticks"], "gaps": case["gaps"]});
        if let Some(d) = diff("", &want, &got) {
            failures.push(format!("{}: {d}", case["name"]));
        }
    }
    assert!(
        failures.is_empty(),
        "{} differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

fn collab_event(v: &Value) -> Event {
    let mut e = event(v);
    e.seq = v["seq"].as_u64().unwrap();
    e.source = match v["source"].as_str().unwrap() {
        "ledger" => Source::Ledger,
        "run" => Source::Run,
        "git" => Source::Git,
        _ => Source::Github,
    };
    e.branch = s(&v["branch"]);
    e.commit = s(&v["commit"]);
    e.pr = v["pr"].as_u64();
    e.detail.from = s(&v["from"]);
    e.detail.commits = v["commits"]
        .as_array()
        .map(|shas| shas.iter().filter_map(s).collect())
        .unwrap_or_default();
    e
}

/// A key the Elixir map may lack: absent, or present as `{"v": value}`.
fn supplied(entry: &Value, key: &str) -> Supplied<String> {
    match entry.get(key) {
        None => Supplied::Absent,
        Some(set) => Supplied::Present(s(&set["v"])),
    }
}

fn collab_json(c: &Collaboration) -> Value {
    json!({
        "pairs": c.pairs.iter().map(|p| json!({
            "a": p.a, "b": p.b, "shared_items": p.shared_items,
            "notes_ab": p.notes_ab, "notes_ba": p.notes_ba, "reviews": p.reviews,
            "handoffs": p.handoffs.iter().map(|h| json!({"from": h.from, "to": h.to, "item_id": h.item_id, "at": h.at})).collect::<Vec<_>>(),
            "branches": p.branches, "co_commits": p.co_commits,
        })).collect::<Vec<_>>(),
        "threads": c.threads.iter().map(|t| json!({
            "item_id": t.item_id, "title": t.title, "status": t.status,
            "steps": t.steps.iter().map(|st| json!({
                "at": st.at, "kind": st.kind.as_str(), "agent": st.agent, "item_id": st.item_id, "label": st.label,
            })).collect::<Vec<_>>(),
            "agents": t.agents, "stale": t.stale, "stale_agent": t.stale_agent, "stale_since": t.stale_since,
        })).collect::<Vec<_>>(),
    })
}

#[test]
fn collaborations_match_the_elixir_collaboration() {
    let golden = load();
    let mut failures = Vec::new();
    let cases = golden["collabs"].as_array().unwrap();
    for case in cases {
        let events: Vec<Event> = case["events"]
            .as_array()
            .unwrap()
            .iter()
            .map(collab_event)
            .collect();
        let items: std::collections::HashMap<String, ItemInfo> = case["items"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(id, entry)| {
                let info = ItemInfo {
                    title: s(&entry["title"]),
                    status: supplied(entry, "status"),
                    assignee: supplied(entry, "assignee"),
                };
                (id.clone(), info)
            })
            .collect();

        let got = Collaboration::build(&events, &items, case["now"].as_i64().unwrap());
        if let Some(d) = diff("", &case["expected"], &collab_json(&got)) {
            failures.push(format!("{}: {d}", case["name"]));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}
