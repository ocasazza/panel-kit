//! Everything known about one node, as labelled lines, and which nodes are open.
//!
//! [`event_facts`] and [`lane_facts`] turn an event or a lane into [`Fact`]s: a
//! short label and one or more value lines already wrapped to a column count,
//! so a painter's height for an open node is `lines` rows and no measuring is
//! needed. [`Expansion`] is the set of open node ids; hosts own it.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::model::{Event, Lane, LaneKind};
use super::time::format_iso8601;

/// Default value-column width in cells.
pub const WRAP_COLS: usize = 56;

/// One labelled datum of a node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Fact {
    /// Short uppercase-able label, e.g. `commit`.
    pub label: &'static str,
    /// Value lines, at least one, each at most the wrap width in chars unless a
    /// single word is longer.
    pub lines: Vec<String>,
}

impl Fact {
    fn one(label: &'static str, value: impl Into<String>, cols: usize) -> Fact {
        Fact {
            label,
            lines: wrap(&value.into(), cols),
        }
    }
}

/// Total value lines of `facts`: the rows an open node adds.
pub fn line_count(facts: &[Fact]) -> usize {
    facts.iter().map(|f| f.lines.len()).sum()
}

/// Wrap `text` to `cols` chars: hard newlines kept, words kept whole, a word
/// longer than `cols` left on its own line. Blank input gives one empty line.
pub fn wrap(text: &str, cols: usize) -> Vec<String> {
    let cols = cols.max(1);
    let mut out = Vec::new();
    for paragraph in text.lines() {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            let have = line.chars().count();
            let need = word.chars().count();
            if have > 0 && have + 1 + need > cols {
                out.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        out.push(line);
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

/// Every populated field of `event`, in reading order: when, what, who, where
/// it came from, then the free text last.
pub fn event_facts(event: &Event, cols: usize) -> Vec<Fact> {
    let mut facts = vec![
        Fact::one("time", format_iso8601(event.at), cols),
        Fact::one("kind", event.kind.label(), cols),
    ];
    let mut text = |label: &'static str, value: Option<&str>| {
        if let Some(v) = value.filter(|v| !v.trim().is_empty()) {
            facts.push(Fact::one(label, v, cols));
        }
    };
    text("agent", event.agent.as_deref());
    text("item", event.item_id.as_deref());
    text("status", event.detail.status.as_deref());
    text("title", event.detail.title.as_deref());
    text("from", event.detail.from.as_deref());
    text("run", event.run_id.as_deref());
    text("branch", event.branch.as_deref());
    text("commit", event.commit.as_deref());
    if let Some(pr) = event.pr {
        facts.push(Fact::one("pr", format!("#{pr}"), cols));
    }
    if !event.detail.commits.is_empty() {
        facts.push(Fact {
            label: "commits",
            lines: event.detail.commits.clone(),
        });
    }
    facts.push(Fact::one("source", event.source.as_str(), cols));
    facts.push(Fact::one("id", event.id.clone(), cols));
    if let Some(s) = event.summary.as_deref().filter(|s| !s.trim().is_empty()) {
        facts.push(Fact::one("text", s, cols));
    }
    facts
}

/// Every populated field of `lane`.
pub fn lane_facts(lane: &Lane, cols: usize) -> Vec<Fact> {
    let kind = match lane.kind {
        LaneKind::Path => "path",
        LaneKind::Branch => "branch",
        LaneKind::Handoff => "handoff",
        LaneKind::Collab => "collab",
    };
    let mut facts = vec![
        Fact::one("lane", lane.label.clone(), cols),
        Fact::one("kind", kind, cols),
        Fact::one("from", format_iso8601(lane.from), cols),
        Fact::one("to", format_iso8601(lane.to), cols),
    ];
    if let Some(agent) = lane.agent.as_deref().filter(|a| !a.is_empty()) {
        facts.push(Fact::one("agent", agent, cols));
    }
    let outcome = match lane.chosen {
        Some(true) => "kept",
        Some(false) => "dropped",
        None => "open",
    };
    facts.push(Fact::one("outcome", outcome, cols));
    if let Some(parent) = &lane.parent {
        facts.push(Fact::one("parent", parent.clone(), cols));
    }
    facts.push(Fact::one("item", lane.item_id.clone(), cols));
    facts
}

/// The ids of the open nodes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Expansion(BTreeSet<String>);

impl Expansion {
    /// Nothing open.
    pub fn new() -> Self {
        Self::default()
    }

    /// `id` is open.
    pub fn is_open(&self, id: &str) -> bool {
        self.0.contains(id)
    }

    /// Open `id` if closed, close it if open; returns whether it is now open.
    pub fn toggle(&mut self, id: &str) -> bool {
        if self.0.remove(id) {
            false
        } else {
            self.0.insert(id.to_owned());
            true
        }
    }

    /// Open every id in `ids`.
    pub fn open_all<'a>(&mut self, ids: impl IntoIterator<Item = &'a str>) {
        self.0.extend(ids.into_iter().map(str::to_owned));
    }

    /// Close everything.
    pub fn close_all(&mut self) {
        self.0.clear();
    }

    /// Open node count.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Nothing is open.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Open ids, sorted.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::trace::model::EventKind;
    use rstest::rstest;

    #[rstest]
    #[case::fits("one two", 10, &["one two"])]
    #[case::breaks_on_a_word("one two three", 7, &["one two", "three"])]
    #[case::keeps_a_long_word_whole("a abcdefghij b", 4, &["a", "abcdefghij", "b"])]
    #[case::keeps_hard_newlines("a\n\nb", 9, &["a", "", "b"])]
    #[case::blank_is_one_empty_line("   ", 9, &[""])]
    #[case::empty_is_one_empty_line("", 9, &[""])]
    fn wrapping(#[case] text: &str, #[case] cols: usize, #[case] want: &[&str]) {
        assert_eq!(wrap(text, cols), want);
    }

    #[test]
    fn an_event_lists_only_what_it_has_and_puts_the_text_last() {
        let bare = Event::new("ledger:1", 0, EventKind::ItemCreated);
        let labels: Vec<_> = event_facts(&bare, 40).iter().map(|f| f.label).collect();
        assert_eq!(labels, ["time", "kind", "source", "id"]);

        let full = Event::new("git:abc", 0, EventKind::Commit)
            .agent("omp")
            .item("imp-1")
            .branch("fix/x")
            .commit("3fa9c21e")
            .summary("first line\nsecond line");
        let facts = event_facts(&full, 40);
        let labels: Vec<_> = facts.iter().map(|f| f.label).collect();
        assert_eq!(
            labels,
            ["time", "kind", "agent", "item", "branch", "commit", "source", "id", "text"]
        );
        assert_eq!(facts.last().unwrap().lines, ["first line", "second line"]);
        assert_eq!(line_count(&facts), labels.len() + 1);
    }

    #[test]
    fn a_blank_agent_is_not_a_fact() {
        let e = Event::new("l:1", 0, EventKind::Note).agent("  ").summary(" ");
        let labels: Vec<_> = event_facts(&e, 40).iter().map(|f| f.label).collect();
        assert!(!labels.contains(&"agent") && !labels.contains(&"text"));
    }

    #[test]
    fn expansion_toggles_and_clears() {
        let mut open = Expansion::new();
        assert!(open.toggle("a"));
        assert!(open.is_open("a"));
        assert!(!open.toggle("a"));
        open.open_all(["x", "y", "x"]);
        assert_eq!(open.len(), 2);
        open.close_all();
        assert!(open.is_empty());
    }
}
