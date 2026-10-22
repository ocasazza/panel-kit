//! The shared input vocabulary of the trace: activity events, session-tree
//! lanes and edges, and the instants they carry. The vertical rail, the swimlane
//! graph and the query layer all read these types.

use std::fmt;
use std::str::FromStr;

use serde::Deserialize;
use serde::Serialize;

use super::query::tone_for_event;
use super::query::tone_for_kind;
use super::query::Queryable;
use super::query::Tone;

/// Microseconds since 1970-01-01T00:00:00Z, UTC. Every instant in the event
/// graph is one; the swimlane's warped axis floors them to milliseconds.
pub type Micros = i64;

/// Where an event was read from. Declaration order is the tie-break rank of
/// [`Event::order_key`].
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// The work ledger the board itself writes.
    #[default]
    Ledger,
    /// A run's own trace.
    Run,
    /// Git history.
    Git,
    /// The GitHub API.
    Github,
}

impl Source {
    /// Every source, in rank order.
    pub const ALL: [Source; 4] = [Source::Ledger, Source::Run, Source::Git, Source::Github];

    /// Snake-case name, e.g. `ledger`.
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Ledger => "ledger",
            Source::Run => "run",
            Source::Git => "git",
            Source::Github => "github",
        }
    }

    /// Tie-break rank: ledger 0, run 1, git 2, github 3.
    pub fn rank(self) -> u8 {
        self as u8
    }
}

/// What an activity event records. The vocabulary is closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    /// A work item was created.
    ItemCreated,
    /// A work item was claimed.
    ItemClaimed,
    /// A work item was released.
    ItemReleased,
    /// A work item changed status.
    ItemStatus,
    /// A free-text note.
    Note,
    /// A liveness ping.
    Heartbeat,
    /// A run started.
    RunStarted,
    /// A run called a tool.
    RunTool,
    /// A run produced a thought.
    RunThought,
    /// A run produced reasoning.
    RunReasoning,
    /// A run called a model.
    RunModel,
    /// A run finished.
    RunFinished,
    /// A run ended with an error.
    RunFailed,
    /// A run was cancelled.
    RunCancelled,
    /// A git commit.
    Commit,
    /// A git branch head moved.
    BranchHead,
    /// A pull request was opened.
    PrOpened,
    /// A pull request was reviewed.
    PrReviewed,
    /// A pull request was merged.
    PrMerged,
    /// An item was handed to another agent.
    Handoff,
}

impl EventKind {
    /// Every kind, in declaration order.
    pub const ALL: [EventKind; 20] = [
        EventKind::ItemCreated,
        EventKind::ItemClaimed,
        EventKind::ItemReleased,
        EventKind::ItemStatus,
        EventKind::Note,
        EventKind::Heartbeat,
        EventKind::RunStarted,
        EventKind::RunTool,
        EventKind::RunThought,
        EventKind::RunReasoning,
        EventKind::RunModel,
        EventKind::RunFinished,
        EventKind::RunFailed,
        EventKind::RunCancelled,
        EventKind::Commit,
        EventKind::BranchHead,
        EventKind::PrOpened,
        EventKind::PrReviewed,
        EventKind::PrMerged,
        EventKind::Handoff,
    ];

    /// Every kind, sorted by [`EventKind::as_str`]: the order of the no-item
    /// strip rows.
    pub const BY_NAME: [EventKind; 20] = [
        EventKind::BranchHead,
        EventKind::Commit,
        EventKind::Handoff,
        EventKind::Heartbeat,
        EventKind::ItemClaimed,
        EventKind::ItemCreated,
        EventKind::ItemReleased,
        EventKind::ItemStatus,
        EventKind::Note,
        EventKind::PrMerged,
        EventKind::PrOpened,
        EventKind::PrReviewed,
        EventKind::RunCancelled,
        EventKind::RunFailed,
        EventKind::RunFinished,
        EventKind::RunModel,
        EventKind::RunReasoning,
        EventKind::RunStarted,
        EventKind::RunThought,
        EventKind::RunTool,
    ];

    /// Snake-case name (`data-kind`, `item:`/`kind:` filter values).
    pub fn as_str(self) -> &'static str {
        match self {
            EventKind::ItemCreated => "item_created",
            EventKind::ItemClaimed => "item_claimed",
            EventKind::ItemReleased => "item_released",
            EventKind::ItemStatus => "item_status",
            EventKind::Note => "note",
            EventKind::Heartbeat => "heartbeat",
            EventKind::RunStarted => "run_started",
            EventKind::RunTool => "run_tool",
            EventKind::RunThought => "run_thought",
            EventKind::RunReasoning => "run_reasoning",
            EventKind::RunModel => "run_model",
            EventKind::RunFinished => "run_finished",
            EventKind::RunFailed => "run_failed",
            EventKind::RunCancelled => "run_cancelled",
            EventKind::Commit => "commit",
            EventKind::BranchHead => "branch_head",
            EventKind::PrOpened => "pr_opened",
            EventKind::PrReviewed => "pr_reviewed",
            EventKind::PrMerged => "pr_merged",
            EventKind::Handoff => "handoff",
        }
    }

    /// Tone by kind alone, from the query layer's table; `item_status` is
    /// decided by its status instead, see [`Event::tone`].
    pub fn tone(self) -> Tone {
        tone_for_kind(self.as_str())
    }

    /// The name with `_` as a space: `pr merged`.
    pub fn label(self) -> String {
        let mut out = String::with_capacity(16);
        self.push_label(&mut out);
        out
    }

    /// Appends [`EventKind::label`] to `out`.
    pub fn push_label(self, out: &mut String) {
        out.extend(
            self.as_str()
                .chars()
                .map(|c| if c == '_' { ' ' } else { c }),
        );
    }

    /// Kind of a snake-case name; `None` outside the vocabulary.
    pub fn from_name(name: &str) -> Option<EventKind> {
        EventKind::ALL
            .into_iter()
            .find(|kind| kind.as_str() == name)
    }
}

impl fmt::Display for EventKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A name outside the closed event-kind vocabulary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownKind(pub String);

impl fmt::Display for UnknownKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown event kind: {:?}", self.0)
    }
}

impl std::error::Error for UnknownKind {}

impl FromStr for EventKind {
    type Err = UnknownKind;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        EventKind::from_name(name).ok_or_else(|| UnknownKind(name.to_string()))
    }
}

/// The slice of an event's detail map the graphs read.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Detail {
    /// Status the event records (`done`, `blocked`, ...).
    #[serde(default)]
    pub status: Option<String>,
    /// Item title as of the event.
    #[serde(default)]
    pub title: Option<String>,
    /// Previous holder named by a `handoff`.
    #[serde(default)]
    pub from: Option<String>,
    /// Commit shas a `pr_merged` brought in.
    #[serde(default)]
    pub commits: Vec<String>,
}

/// One normalised activity event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    /// Dedupe key: `ledger:12`, a sha, a branch.
    pub id: String,
    /// UTC instant, microseconds since the epoch.
    pub at: Micros,
    /// Monotone within its source; the last tie-break of [`Event::order_key`].
    #[serde(default)]
    pub seq: u64,
    /// Where the event was read from.
    #[serde(default)]
    pub source: Source,
    /// What happened.
    pub kind: EventKind,
    /// Acting agent; `None` and blank are unattributed to lanes and collab.
    #[serde(default)]
    pub agent: Option<String>,
    /// Work item; `None` is a loose event with no item.
    #[serde(default)]
    pub item_id: Option<String>,
    /// Run the event belongs to.
    #[serde(default)]
    pub run_id: Option<String>,
    /// Git branch.
    #[serde(default)]
    pub branch: Option<String>,
    /// Commit sha.
    #[serde(default)]
    pub commit: Option<String>,
    /// Pull request number.
    #[serde(default)]
    pub pr: Option<u64>,
    /// Free text, may span lines.
    #[serde(default)]
    pub summary: Option<String>,
    /// Status, title and the other detail keys the graphs read.
    #[serde(default)]
    pub detail: Detail,
}

impl Event {
    /// Ledger event with no agent, item, summary or detail.
    pub fn new(id: impl Into<String>, at: Micros, kind: EventKind) -> Self {
        Event {
            id: id.into(),
            at,
            seq: 0,
            source: Source::Ledger,
            kind,
            agent: None,
            item_id: None,
            run_id: None,
            branch: None,
            commit: None,
            pr: None,
            summary: None,
            detail: Detail::default(),
        }
    }

    /// Sets the acting agent.
    pub fn agent(mut self, agent: impl Into<String>) -> Self {
        self.agent = Some(agent.into());
        self
    }

    /// Sets the work item.
    pub fn item(mut self, item_id: impl Into<String>) -> Self {
        self.item_id = Some(item_id.into());
        self
    }

    /// Sets the summary text.
    pub fn summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    /// Sets the source.
    pub fn source(mut self, source: Source) -> Self {
        self.source = source;
        self
    }

    /// Sets the per-source sequence number.
    pub fn seq(mut self, seq: u64) -> Self {
        self.seq = seq;
        self
    }

    /// Sets the run.
    pub fn run(mut self, run_id: impl Into<String>) -> Self {
        self.run_id = Some(run_id.into());
        self
    }

    /// Sets the git branch.
    pub fn branch(mut self, branch: impl Into<String>) -> Self {
        self.branch = Some(branch.into());
        self
    }

    /// Sets the commit sha.
    pub fn commit(mut self, commit: impl Into<String>) -> Self {
        self.commit = Some(commit.into());
        self
    }

    /// Sets the pull request number.
    pub fn pr(mut self, pr: u64) -> Self {
        self.pr = Some(pr);
        self
    }

    /// Sets `detail.status`.
    pub fn status(mut self, status: impl Into<String>) -> Self {
        self.detail.status = Some(status.into());
        self
    }

    /// Sets `detail.title`.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.detail.title = Some(title.into());
        self
    }

    /// Sets `detail.from`, the previous holder of a handoff.
    pub fn from(mut self, from: impl Into<String>) -> Self {
        self.detail.from = Some(from.into());
        self
    }

    /// Sets `detail.commits`, the shas a merged pull request brought in.
    pub fn commits<S: Into<String>>(mut self, commits: impl IntoIterator<Item = S>) -> Self {
        self.detail.commits = commits.into_iter().map(Into::into).collect();
        self
    }

    /// Tone the event draws in: `item_status` events by their status, every
    /// other kind by its table.
    pub fn tone(&self) -> Tone {
        tone_for_event(self.kind.as_str(), self.detail.status.as_deref())
    }

    /// The order a lane draws in: time, then source rank, then `seq`.
    pub fn order_key(&self) -> (Micros, u8, u64) {
        (self.at, self.source.rank(), self.seq)
    }
}

impl Queryable for Event {
    fn agent(&self) -> Option<&str> {
        self.agent.as_deref()
    }

    fn item_id(&self) -> Option<&str> {
        self.item_id.as_deref()
    }

    fn kind(&self) -> &str {
        self.kind.as_str()
    }
}

/// Whether `agent` names someone: present and not blank.
pub fn is_named(agent: Option<&str>) -> bool {
    agent.is_some_and(|name| !name.trim().is_empty())
}

/// The kinds named by `names`, in order; names outside the vocabulary and
/// repeats are dropped. For turning a host's `kind` strings into
/// [`LooseOpen::Kinds`](super::swimlane::LooseOpen::Kinds).
pub fn kinds_of<S: AsRef<str>>(names: impl IntoIterator<Item = S>) -> Vec<EventKind> {
    let mut kinds: Vec<EventKind> = Vec::new();
    for name in names {
        if let Some(kind) = EventKind::from_name(name.as_ref()) {
            if !kinds.contains(&kind) {
                kinds.push(kind);
            }
        }
    }
    kinds
}

/// What a session-tree lane represents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaneKind {
    /// A child of a reasoning-graph node.
    Path,
    /// A git branch other than the trunk.
    Branch,
    /// Work handed to another agent.
    Handoff,
    /// Another agent working alongside the owner.
    Collab,
}

impl LaneKind {
    /// Snake-case name; also the word a fork's title starts with.
    pub fn as_str(self) -> &'static str {
        match self {
            LaneKind::Path => "path",
            LaneKind::Branch => "branch",
            LaneKind::Handoff => "handoff",
            LaneKind::Collab => "collab",
        }
    }
}

/// A line forked off an item's line, one row under it (a rail column, a
/// swimlane fork).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lane {
    /// Unique id: `imp-1/collab/omp`, `imp-1/branch/feat/x`.
    pub id: String,
    /// The item the lane belongs to.
    pub item_id: String,
    /// What the lane represents.
    pub kind: LaneKind,
    /// Agent, branch or path-node name; the tip node's caption.
    pub label: String,
    /// First instant of the lane.
    pub from: Micros,
    /// Last instant of the lane.
    pub to: Micros,
    /// `Some(true)` survived, `Some(false)` is dead, `None` undecided.
    #[serde(default)]
    pub chosen: Option<bool>,
    /// Lane this one forked from; `None` forks from the item line.
    #[serde(default)]
    pub parent: Option<String>,
    /// Agent that owns the lane.
    #[serde(default)]
    pub agent: Option<String>,
}

/// Fork or merge of a lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// The lane leaves its parent.
    Fork,
    /// The lane returns to its parent.
    Merge,
}

impl EdgeKind {
    /// Snake-case name; the CSS class suffix of the elbow.
    pub fn as_str(self) -> &'static str {
        match self {
            EdgeKind::Fork => "fork",
            EdgeKind::Merge => "merge",
        }
    }
}

/// The instant a lane forks or merges.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edge {
    /// Fork or merge.
    pub kind: EdgeKind,
    /// Id of the lane the edge belongs to.
    pub lane: String,
    /// The item the lane belongs to.
    pub item_id: String,
    /// The instant of the elbow.
    pub at: Micros,
}

/// The session tree of one work item, or the merged window of several: the
/// lanes forked off the item lines and the elbows where they fork and merge.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionTree {
    /// Owner of the item line (column 0); `None` when unknown or merged.
    #[serde(default)]
    pub agent: Option<String>,
    /// Lanes forked off the item lines, grouped per item in input order.
    #[serde(default)]
    pub lanes: Vec<Lane>,
    /// Fork and merge elbows.
    #[serde(default)]
    pub edges: Vec<Edge>,
}

/// The item whose rail is drawn.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemRef {
    /// Work item id; events of other items are dropped.
    pub id: String,
    /// Item status; wins over the last event's status.
    #[serde(default)]
    pub status: Option<String>,
}
