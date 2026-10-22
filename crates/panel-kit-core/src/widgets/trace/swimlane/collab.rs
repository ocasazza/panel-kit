//! COLLAB mode: who worked with whom, and who is holding what nobody has touched.
//!
//! A pure function over the activity events: the same events draw the same
//! table whatever order they arrive in.
//!
//! A pair is two agents who both appear on an item, and a row only when the
//! evidence is counted: notes (theirs each way, never summed), reviews,
//! handoffs (`a → b` means `b` took the item over from `a`), shared branches,
//! and co-commits (commits a merged pull request on the item brought in that
//! name no agent). A thread is one item's story across more than one agent.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::Serialize;

use super::super::model::{is_named, Event, EventKind, Micros};

/// An item still held by an agent unseen for longer than this is stale.
pub const STALE_AFTER_DAYS: i64 = 3;

const MICROS_PER_DAY: i64 = 86_400_000_000;
const MICROS_PER_SEC: i64 = 1_000_000;

/// A value the host may or may not supply for an item.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Supplied<T> {
    /// The host did not say; derive it from the item's events.
    #[default]
    Absent,
    /// The host said. `Present(None)` is an explicit "nobody" and wins over
    /// what the events suggest.
    Present(Option<T>),
}

/// What the host knows of an item's current state.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ItemInfo {
    /// Item title.
    pub title: Option<String>,
    /// Current status; judged against `claimed` and `in_progress` for staleness.
    pub status: Supplied<String>,
    /// Current holder; the stale test reads it.
    pub assignee: Supplied<String>,
}

/// An item changing hands.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Handoff {
    /// Who held it.
    pub from: String,
    /// Who took it over.
    pub to: String,
    /// The item.
    pub item_id: String,
    /// When, microseconds.
    pub at: Micros,
}

/// Two agents and the evidence that they worked together.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Pair {
    /// The lesser name by string order.
    pub a: String,
    /// The greater name.
    pub b: String,
    /// Items they both acted on, sorted.
    pub shared_items: Vec<String>,
    /// Notes `a` wrote on shared items.
    pub notes_ab: usize,
    /// Notes `b` wrote on shared items.
    pub notes_ba: usize,
    /// Pull-request reviews by either of them on shared items.
    pub reviews: usize,
    /// Handoffs between them, oldest first.
    pub handoffs: Vec<Handoff>,
    /// Branches on the shared items, sorted.
    pub branches: Vec<String>,
    /// Commits on shared items that name no agent.
    pub co_commits: usize,
}

impl Pair {
    /// Evidence in the pair: notes both ways, handoffs, reviews, co-commits.
    pub fn weight(&self) -> usize {
        self.notes_ab + self.notes_ba + self.handoffs.len() + self.reviews + self.co_commits
    }
}

/// One step of an item's thread.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Step {
    /// When, truncated to the second, in microseconds.
    pub at: Micros,
    /// What happened.
    pub kind: EventKind,
    /// Who did it; `None` for a blank agent.
    pub agent: Option<String>,
    /// The item.
    pub item_id: String,
    /// `created`, `claimed`, `handed from x`, the note's text, `PR #12 merged`.
    pub label: String,
}

/// One item's story between agents.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Thread {
    /// The item.
    pub item_id: String,
    /// The host's title, else the first title an event carries.
    pub title: Option<String>,
    /// The host's status for the item.
    pub status: Option<String>,
    /// Created, claimed, released, handed-off, noted and merged steps in order.
    pub steps: Vec<Step>,
    /// Named agents on the item, sorted.
    pub agents: Vec<String>,
    /// Whether the item is held by an agent nobody has heard from since.
    pub stale: bool,
    /// The silent holder.
    pub stale_agent: Option<String>,
    /// When the silent holder was last heard from.
    pub stale_since: Option<Micros>,
}

/// Pairs and threads over a set of events.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Collaboration {
    /// Pairs with evidence, busiest first, then by name.
    pub pairs: Vec<Pair>,
    /// Items with more than one agent, stale holders first, then by item number.
    pub threads: Vec<Thread>,
}

impl Collaboration {
    /// Builds the graph over `events`. `items` gives each item's current
    /// state so threads can be judged against now; an item without an entry
    /// is judged from its events alone. `now` is a parameter so the stale
    /// test is a fact and not a race.
    pub fn build<'a>(
        events: impl IntoIterator<Item = &'a Event>,
        items: &HashMap<String, ItemInfo>,
        now: Micros,
    ) -> Collaboration {
        // Events with no item are no conversation about an item.
        let mut by_item: BTreeMap<&str, Vec<&Event>> = BTreeMap::new();
        // Commits name no item; the merge carries their shas, so they are
        // indexed once. Of two commits with one sha the later is the commit.
        let mut commits: HashMap<&str, &Event> = HashMap::new();
        for event in events {
            if let Some(item) = event.item_id.as_deref() {
                by_item.entry(item).or_default().push(event);
            }
            if let (EventKind::Commit, Some(sha)) = (event.kind, event.commit.as_deref()) {
                commits.insert(sha, event);
            }
        }

        Collaboration {
            pairs: pairs(&by_item, &commits),
            threads: threads(&by_item, items, now),
        }
    }

    /// Pairs plus threads: what the view's empty state counts.
    pub fn size(&self) -> usize {
        self.pairs.len() + self.threads.len()
    }
}

/// Distinct named agents of `events`, in order of first appearance. A blank
/// agent is nobody.
fn named_agents<'a>(events: &[&'a Event]) -> Vec<&'a str> {
    let mut agents: Vec<&str> = Vec::new();
    for event in events {
        if let Some(agent) = event.agent.as_deref().filter(|agent| is_named(Some(agent))) {
            if !agents.contains(&agent) {
                agents.push(agent);
            }
        }
    }
    agents
}

/// Partial pair while items are folded in.
#[derive(Default)]
struct Tally {
    shared_items: Vec<String>,
    notes_ab: usize,
    notes_ba: usize,
    reviews: usize,
    handoffs: Vec<Handoff>,
    branches: BTreeSet<String>,
    co_commits: usize,
}

fn pairs(by_item: &BTreeMap<&str, Vec<&Event>>, commits: &HashMap<&str, &Event>) -> Vec<Pair> {
    let mut tallies: BTreeMap<(&str, &str), Tally> = BTreeMap::new();
    for (item, item_events) in by_item {
        let agents = named_agents(item_events);
        if agents.len() < 2 {
            continue;
        }
        let branches: BTreeSet<&str> = item_events
            .iter()
            .filter_map(|event| event.branch.as_deref())
            .collect();
        let co_commits = co_commits(item, item_events, commits);

        for (i, a) in agents.iter().enumerate() {
            for b in &agents[i + 1..] {
                let (a, b) = if a < b { (*a, *b) } else { (*b, *a) };
                let tally = tallies.entry((a, b)).or_default();
                tally.shared_items.push((*item).to_string());
                tally.notes_ab += count(item_events, EventKind::Note, |agent| agent == Some(a));
                tally.notes_ba += count(item_events, EventKind::Note, |agent| agent == Some(b));
                tally.reviews += count(item_events, EventKind::PrReviewed, |agent| {
                    agent == Some(a) || agent == Some(b)
                });
                tally.handoffs.extend(handoffs(item_events, a, b));
                tally
                    .branches
                    .extend(branches.iter().map(|branch| (*branch).to_string()));
                tally.co_commits += co_commits;
            }
        }
    }

    let mut pairs: Vec<Pair> = tallies
        .into_iter()
        .map(|((a, b), mut tally)| {
            tally.handoffs.sort_by(|x, y| {
                (x.at, &x.item_id, &x.from, &x.to).cmp(&(y.at, &y.item_id, &y.from, &y.to))
            });
            Pair {
                a: a.to_string(),
                b: b.to_string(),
                shared_items: tally.shared_items,
                notes_ab: tally.notes_ab,
                notes_ba: tally.notes_ba,
                reviews: tally.reviews,
                handoffs: tally.handoffs,
                branches: tally.branches.into_iter().collect(),
                co_commits: tally.co_commits,
            }
        })
        // An edge needs notes, a review, a handoff or a co-commit behind it:
        // two names on one line say nothing about how the fleet collaborates.
        .filter(|pair| pair.weight() > 0)
        .collect();
    pairs.sort_by(|x, y| {
        y.weight()
            .cmp(&x.weight())
            .then_with(|| x.a.cmp(&y.a))
            .then_with(|| x.b.cmp(&y.b))
    });
    pairs
}

fn count(events: &[&Event], kind: EventKind, by: impl Fn(Option<&str>) -> bool) -> usize {
    events
        .iter()
        .filter(|event| event.kind == kind && by(event.agent.as_deref()))
        .count()
}

/// Commits a merged pull request on the item brought in that the index has
/// read and that name no agent: they belong to whoever worked the item.
fn co_commits(item: &str, item_events: &[&Event], commits: &HashMap<&str, &Event>) -> usize {
    let mut shas: Vec<&str> = Vec::new();
    for event in item_events
        .iter()
        .filter(|event| event.kind == EventKind::PrMerged && event.item_id.as_deref() == Some(item))
    {
        for sha in &event.detail.commits {
            if !shas.contains(&sha.as_str()) {
                shas.push(sha);
            }
        }
    }
    shas.iter()
        .filter(|sha| {
            commits
                .get(*sha)
                .is_some_and(|commit| !is_named(commit.agent.as_deref()))
        })
        .count()
}

fn handoffs(item_events: &[&Event], a: &str, b: &str) -> Vec<Handoff> {
    item_events
        .iter()
        .filter(|event| event.kind == EventKind::Handoff)
        .filter_map(|event| {
            let from = event.detail.from.as_deref()?;
            let to = event.agent.as_deref()?;
            ((from == a && to == b) || (from == b && to == a)).then(|| Handoff {
                from: from.to_string(),
                to: to.to_string(),
                item_id: event.item_id.clone().unwrap_or_default(),
                at: event.at,
            })
        })
        .collect()
}

fn threads(
    by_item: &BTreeMap<&str, Vec<&Event>>,
    items: &HashMap<String, ItemInfo>,
    now: Micros,
) -> Vec<Thread> {
    let mut threads: Vec<Thread> = by_item
        .iter()
        .filter_map(|(item, item_events)| thread(item, items.get(*item), item_events, now))
        .collect();
    // A stale holder first: it is the one row that wants a human.
    threads.sort_by_cached_key(|thread| (!thread.stale, item_number(&thread.item_id)));
    threads
}

/// Sort key of an item id: `imp-N` by N, anything else after them by name.
fn item_number(item_id: &str) -> (u8, i64, String) {
    match item_id.replace("imp-", "").parse::<i64>() {
        Ok(number) => (0, number, item_id.to_string()),
        Err(_) => (1, 0, item_id.to_string()),
    }
}

fn thread(
    item: &str,
    info: Option<&ItemInfo>,
    item_events: &[&Event],
    now: Micros,
) -> Option<Thread> {
    let agents = named_agents(item_events);
    if agents.len() < 2 {
        return None;
    }
    let mut step_events: Vec<&Event> = item_events
        .iter()
        .copied()
        .filter(|event| is_step(event.kind))
        .collect();
    step_events.sort_by_key(|event| event.order_key());
    let steps = step_events
        .into_iter()
        .map(|event| Step {
            at: event.at - event.at.rem_euclid(MICROS_PER_SEC),
            kind: event.kind,
            agent: event.agent.clone().filter(|agent| is_named(Some(agent))),
            item_id: item.to_string(),
            label: step_label(event),
        })
        .collect();

    let stale = stale_holder(info, item_events, &agents, now);
    let title = info.and_then(|info| info.title.clone()).or_else(|| {
        item_events
            .iter()
            .find_map(|event| event.detail.title.clone())
    });
    let status = match info.map(|info| &info.status) {
        Some(Supplied::Present(status)) => status.clone(),
        _ => None,
    };
    let mut sorted: Vec<String> = agents.iter().map(|agent| (*agent).to_string()).collect();
    sorted.sort();

    Some(Thread {
        item_id: item.to_string(),
        title,
        status,
        steps,
        agents: sorted,
        stale: stale.is_some(),
        stale_agent: stale.as_ref().map(|(agent, _)| agent.clone()),
        stale_since: stale.and_then(|(_, since)| since),
    })
}

fn is_step(kind: EventKind) -> bool {
    matches!(
        kind,
        EventKind::ItemCreated
            | EventKind::ItemClaimed
            | EventKind::ItemReleased
            | EventKind::Handoff
            | EventKind::Note
            | EventKind::PrMerged
    )
}

fn step_label(event: &Event) -> String {
    let summary = event.summary.as_deref();
    match event.kind {
        EventKind::Handoff => format!("handed from {}", event.detail.from.as_deref().unwrap_or("")),
        EventKind::ItemCreated => "created".to_string(),
        EventKind::ItemClaimed => "claimed".to_string(),
        EventKind::ItemReleased => "released".to_string(),
        EventKind::PrMerged => match event.pr {
            Some(pr) => format!("PR #{pr} merged"),
            None => "PR # merged".to_string(),
        },
        kind => summary.map_or_else(|| kind.as_str().to_string(), str::to_string),
    }
}

/// An item still held by an agent nobody has heard from since. The claim is
/// the host's `assignee` when it supplies one (even `None`: unheld), else the
/// item's last claim or handoff event; likewise the status.
fn stale_holder(
    info: Option<&ItemInfo>,
    item_events: &[&Event],
    agents: &[&str],
    now: Micros,
) -> Option<(String, Option<Micros>)> {
    let holder = match info.map(|info| &info.assignee) {
        Some(Supplied::Present(holder)) => holder.as_deref(),
        _ => latest(item_events, |kind| {
            matches!(kind, EventKind::ItemClaimed | EventKind::Handoff)
        })
        .and_then(|event| event.agent.as_deref()),
    };
    let status = match info.map(|info| &info.status) {
        Some(Supplied::Present(status)) => status.as_deref(),
        _ => latest(item_events, |kind| {
            matches!(
                kind,
                EventKind::ItemStatus | EventKind::ItemClaimed | EventKind::ItemReleased
            )
        })
        .and_then(|event| event.detail.status.as_deref()),
    };

    let holder = holder.filter(|holder| !holder.is_empty())?;
    if !matches!(status, Some("claimed" | "in_progress")) || !agents.contains(&holder) {
        return None;
    }
    let since = item_events
        .iter()
        .filter(|event| event.agent.as_deref() == Some(holder))
        .max_by_key(|event| event.order_key())
        .map(|event| event.at);
    let floor = now.saturating_sub(STALE_AFTER_DAYS * MICROS_PER_DAY);
    since
        .is_none_or(|at| at < floor)
        .then(|| (holder.to_string(), since))
}

/// The newest event of a kind `wanted` accepts.
fn latest<'a>(events: &[&'a Event], wanted: impl Fn(EventKind) -> bool) -> Option<&'a Event> {
    events
        .iter()
        .copied()
        .filter(|event| wanted(event.kind))
        .max_by_key(|event| event.order_key())
}

/// Current items for [`Collaboration::build`], read off every event the index
/// holds: the status of the item's last `item_status` event (`None` when it
/// has none), its first title, and no assignee.
pub fn current_items(events: &[Event]) -> HashMap<String, ItemInfo> {
    let mut items: HashMap<String, ItemInfo> = HashMap::new();
    for event in events {
        let Some(item) = event.item_id.as_ref() else {
            continue;
        };
        let info = items.entry(item.clone()).or_insert_with(|| ItemInfo {
            title: None,
            status: Supplied::Present(None),
            assignee: Supplied::Absent,
        });
        if info.title.is_none() {
            info.title = event.detail.title.clone();
        }
        if event.kind == EventKind::ItemStatus {
            info.status = Supplied::Present(event.detail.status.clone());
        }
    }
    items
}
