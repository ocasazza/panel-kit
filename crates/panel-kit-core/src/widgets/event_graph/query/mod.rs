//! Query line, window grammar, legend and toolbar state of the event graph.
//!
//! Entry points (all pure; the painter only emits DOM from the returned structs):
//! - [`parse`] / [`parse_opt`] / [`format()`]: the `agent: item: kind: range: tree:` line.
//! - [`Window::parse`] + [`Window::resolve`]: the `range:` value; `now` is a parameter.
//! - [`Filters::keep`], [`completions`], [`GraphState::tokens`]: filtering and datalist.
//! - [`legend`], [`tone_for_event`], [`Tone::merge`]: tone tables shared by legend and dots.
//! - [`GraphState::reduce`] with [`GraphAction`] returning an [`Effect`]: toolbar state.
//! - [`GraphState::mode_chips`], [`GraphState::legend_chips`]: chips with selected state.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::LazyLock;

use serde::Deserialize;
use serde::Serialize;

use super::time::civil_from_days;
use super::time::days_from_civil;

/// Window label the line omits and a fresh state starts with.
pub const DEFAULT_WINDOW: &str = "24h";
/// Window shortcuts offered in completions.
pub const RANGES: [&str; 4] = ["24h", "7d", "30d", "90d"];
/// Item rows shown before `show all`.
pub const ROW_CAP: usize = 20;
/// Graph width in px until the host measures it.
pub const DEFAULT_GRAPH_WIDTH: u32 = 1200;

// -- window -------------------------------------------------------------------

const DAY: i64 = 86_400;

/// A `range:` value that is not a window spelling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvalidWindow(pub String);

impl fmt::Display for InvalidWindow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid window: {:?}", self.0)
    }
}

impl std::error::Error for InvalidWindow {}

/// Half-open interval `[since, until)` in epoch seconds; `until: None` is open to now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Interval {
    /// Inclusive lower bound.
    pub since: i64,
    /// Exclusive upper bound.
    pub until: Option<i64>,
}

impl Interval {
    /// Whether an event at `at` (epoch seconds) falls inside.
    pub fn contains(&self, at: i64) -> bool {
        at >= self.since && self.until.is_none_or(|until| at < until)
    }
}

/// A parsed `range:` value. The label is the token as typed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Window {
    /// `3h`, `14d`, `2w`: the last `seconds`, open to now.
    Relative {
        /// Token as typed.
        label: String,
        /// Length of the window.
        seconds: i64,
    },
    /// `2026-09-30`: one UTC day.
    Day {
        /// Token as typed.
        label: String,
        /// Day start, epoch seconds.
        since: i64,
        /// Next day start, epoch seconds.
        until: i64,
    },
    /// `2026-W40`: one ISO week, Monday to Monday.
    Week {
        /// Token as typed.
        label: String,
        /// Monday 00:00 UTC, epoch seconds.
        since: i64,
        /// Next Monday 00:00 UTC, epoch seconds.
        until: i64,
    },
    /// `2026-09-01/2026-10-08`: first day through the end of the last day.
    Span {
        /// Token as typed.
        label: String,
        /// First day start, epoch seconds.
        since: i64,
        /// Day after the last day, epoch seconds.
        until: i64,
    },
}

impl Window {
    /// Parses a `range:` value. Dispatch order: relative, span, day, week.
    pub fn parse(token: &str) -> Result<Window, InvalidWindow> {
        let bad = || InvalidWindow(token.to_string());
        let label = token.to_string();
        if let Some(seconds) = parse_relative(token) {
            return Ok(Window::Relative { label, seconds });
        }
        if token.contains('/') {
            let mut parts = token.split('/');
            let (Some(start), Some(end), None) = (parts.next(), parts.next(), parts.next()) else {
                return Err(bad());
            };
            let (Some(start), Some(end)) = (parse_date(start), parse_date(end)) else {
                return Err(bad());
            };
            if start > end {
                return Err(bad());
            }
            return Ok(Window::Span {
                label,
                since: start * DAY,
                until: (end + 1) * DAY,
            });
        }
        if let Some(day) = parse_date(token) {
            return Ok(Window::Day {
                label,
                since: day * DAY,
                until: (day + 1) * DAY,
            });
        }
        if let Some(monday) = parse_week(token) {
            return Ok(Window::Week {
                label,
                since: monday * DAY,
                until: (monday + 7) * DAY,
            });
        }
        Err(bad())
    }

    /// Whether `token` is a valid window spelling.
    pub fn is_valid(token: &str) -> bool {
        Window::parse(token).is_ok()
    }

    /// The token as typed.
    pub fn label(&self) -> &str {
        match self {
            Window::Relative { label, .. }
            | Window::Day { label, .. }
            | Window::Week { label, .. }
            | Window::Span { label, .. } => label,
        }
    }

    /// The interval at `now` (epoch seconds). The clock is never read here.
    pub fn resolve(&self, now: i64) -> Interval {
        match self {
            Window::Relative { seconds, .. } => Interval {
                since: now.saturating_sub(*seconds),
                until: None,
            },
            Window::Day { since, until, .. }
            | Window::Week { since, until, .. }
            | Window::Span { since, until, .. } => Interval {
                since: *since,
                until: Some(*until),
            },
        }
    }
}

/// `^(\d+)([hdw])$` as seconds; `None` on overflow or mismatch.
fn parse_relative(token: &str) -> Option<i64> {
    let (unit, digits) = token.as_bytes().split_last()?;
    let unit = match unit {
        b'h' => 3_600,
        b'd' => DAY,
        b'w' => 7 * DAY,
        _ => return None,
    };
    if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let mut n: i64 = 0;
    for digit in digits {
        n = n.checked_mul(10)?.checked_add(i64::from(digit - b'0'))?;
    }
    n.checked_mul(unit)
}

fn ascii_number(bytes: &[u8]) -> Option<i64> {
    if bytes.is_empty() || !bytes.iter().all(u8::is_ascii_digit) {
        return None;
    }
    Some(bytes.iter().fold(0i64, |n, d| n * 10 + i64::from(d - b'0')))
}

/// `YYYY-MM-DD` real calendar date as epoch days.
fn parse_date(token: &str) -> Option<i64> {
    let b = token.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let year = ascii_number(&b[0..4])?;
    let month = ascii_number(&b[5..7])?;
    let day = ascii_number(&b[8..10])?;
    if !(1..=12).contains(&month) || day < 1 || day > i64::from(days_in_month(year, month as u32)) {
        return None;
    }
    Some(days_from_civil(year, month as u32, day as u32))
}

/// `YYYY-Www` ISO week as the epoch day of its Monday.
fn parse_week(token: &str) -> Option<i64> {
    let b = token.as_bytes();
    if !(7..=8).contains(&b.len()) || b[4] != b'-' || b[5] != b'W' {
        return None;
    }
    let year = ascii_number(&b[0..4])?;
    let week = ascii_number(&b[6..])?;
    if !(1..=53).contains(&week) {
        return None;
    }
    let jan4 = days_from_civil(year, 1, 4);
    let week1_monday = jan4 - weekday_monday0(jan4);
    let monday = week1_monday + (week - 1) * 7;
    (civil_from_days(monday + 3).0 == year).then_some(monday)
}

/// Monday = 0 … Sunday = 6; epoch day 0 is a Thursday.
fn weekday_monday0(days: i64) -> i64 {
    (days + 3).rem_euclid(7)
}

fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

// -- filters and the query line -------------------------------------------------

/// One of the three filter keys, in canonical line order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterKey {
    /// `agent:`
    Agent,
    /// `item:`
    Item,
    /// `kind:`
    Kind,
}

impl FilterKey {
    /// Canonical order of keys in a formatted line.
    pub const ALL: [FilterKey; 3] = [FilterKey::Agent, FilterKey::Item, FilterKey::Kind];

    /// The key as written in the line.
    pub fn name(self) -> &'static str {
        match self {
            FilterKey::Agent => "agent",
            FilterKey::Item => "item",
            FilterKey::Kind => "kind",
        }
    }

    /// Case-sensitive key lookup.
    pub fn from_name(name: &str) -> Option<FilterKey> {
        FilterKey::ALL.into_iter().find(|key| key.name() == name)
    }
}

/// What an event exposes to the filters.
pub trait Queryable {
    /// Agent that produced the event; `None` or blank means no lane.
    fn agent(&self) -> Option<&str>;
    /// Item the event belongs to.
    fn item_id(&self) -> Option<&str>;
    /// Event kind name, e.g. `run_tool`.
    fn kind(&self) -> &str;
}

/// Plain event fields; implements [`Queryable`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventFields {
    /// Agent that produced the event.
    pub agent: Option<String>,
    /// Item the event belongs to.
    pub item_id: Option<String>,
    /// Event kind name.
    pub kind: String,
}

impl Queryable for EventFields {
    fn agent(&self) -> Option<&str> {
        self.agent.as_deref()
    }
    fn item_id(&self) -> Option<&str> {
        self.item_id.as_deref()
    }
    fn kind(&self) -> &str {
        &self.kind
    }
}

/// Whether an agent name is a real lane; blank agents have no lane.
pub fn is_attributed(agent: Option<&str>) -> bool {
    agent.is_some_and(|a| !a.trim().is_empty())
}

/// The `agent:` / `item:` / `kind:` filters; `None` is unset.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Filters {
    /// `agent:` value.
    pub agent: Option<String>,
    /// `item:` value.
    pub item: Option<String>,
    /// `kind:` value.
    pub kind: Option<String>,
}

impl Filters {
    /// Value of one key.
    pub fn get(&self, key: FilterKey) -> Option<&str> {
        match key {
            FilterKey::Agent => self.agent.as_deref(),
            FilterKey::Item => self.item.as_deref(),
            FilterKey::Kind => self.kind.as_deref(),
        }
    }

    /// Replaces one key.
    pub fn set(&mut self, key: FilterKey, value: Option<String>) {
        match key {
            FilterKey::Agent => self.agent = value,
            FilterKey::Item => self.item = value,
            FilterKey::Kind => self.kind = value,
        }
    }

    /// Copy with exactly one key replaced.
    pub fn with(&self, key: FilterKey, value: Option<String>) -> Filters {
        let mut next = self.clone();
        next.set(key, value);
        next
    }

    /// Whether any key holds a non-empty value.
    pub fn any(&self) -> bool {
        FilterKey::ALL
            .into_iter()
            .any(|key| self.get(key).is_some_and(|v| !v.is_empty()))
    }

    /// Event is kept iff every set filter equals the event's field (exact, case-sensitive).
    pub fn keep<E: Queryable + ?Sized>(&self, event: &E) -> bool {
        let want = |key| self.get(key).filter(|v| !v.is_empty());
        want(FilterKey::Agent).is_none_or(|a| event.agent() == Some(a))
            && want(FilterKey::Item).is_none_or(|i| event.item_id() == Some(i))
            && want(FilterKey::Kind).is_none_or(|k| event.kind() == k)
    }
}

/// Result of [`parse`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParsedQuery {
    /// Filters read from the line.
    pub filters: Filters,
    /// Raw `range:` token as typed; `None` when no valid one was given.
    pub range: Option<String>,
    /// `tree:on` seen last.
    pub tree: bool,
    /// Whole offending tokens, in input order.
    pub errors: Vec<String>,
}

/// Parses a query line. Never fails: unreadable tokens land in `errors`.
pub fn parse(text: &str) -> ParsedQuery {
    let mut query = ParsedQuery::default();
    for token in text.split_whitespace() {
        let ok = match token.split_once(':') {
            None => false,
            Some((key, value)) => match FilterKey::from_name(key) {
                Some(key) if !value.is_empty() => {
                    query.filters.set(key, Some(value.to_string()));
                    true
                }
                Some(_) => false,
                None if key == "range" => {
                    let valid = Window::is_valid(value);
                    if valid {
                        query.range = Some(value.to_string());
                    }
                    valid
                }
                None if key == "tree" && (value == "on" || value == "off") => {
                    query.tree = value == "on";
                    true
                }
                None => false,
            },
        };
        if !ok {
            query.errors.push(token.to_string());
        }
    }
    query
}

/// [`parse`] for a possibly absent line; `None` reads as empty.
pub fn parse_opt(text: Option<&str>) -> ParsedQuery {
    parse(text.unwrap_or(""))
}

/// Canonical line: filters in `agent item kind` order, `range:` unless default, `tree:on`.
pub fn format(filters: &Filters, window: Option<&str>, tree: bool) -> String {
    let mut parts: Vec<String> = Vec::with_capacity(5);
    for key in FilterKey::ALL {
        if let Some(value) = filters.get(key).filter(|v| !v.is_empty()) {
            parts.push(std::format!("{}:{value}", key.name()));
        }
    }
    if let Some(window) = window.filter(|w| *w != DEFAULT_WINDOW) {
        parts.push(std::format!("range:{window}"));
    }
    if tree {
        parts.push("tree:on".to_string());
    }
    parts.join(" ")
}

/// One datalist option: a whole line that changes exactly one key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Completion {
    /// Full replacement line.
    pub value: String,
    /// Display text, e.g. `agent:worker`.
    pub label: String,
    /// The current state already holds this key/value.
    pub active: bool,
}

/// Datalist options over `events` (all events in the window, unfiltered).
///
/// Order: `agent:` values, `item:` values, `kind:` values (each sorted, distinct,
/// blanks dropped), then `range:` shortcuts, then `tree:on`.
pub fn completions<E: Queryable>(
    filters: &Filters,
    window: Option<&str>,
    tree: bool,
    events: &[E],
) -> Vec<Completion> {
    let mut out = Vec::new();
    for key in FilterKey::ALL {
        let mut values: BTreeSet<&str> = BTreeSet::new();
        for event in events {
            let value = match key {
                FilterKey::Agent => event.agent(),
                FilterKey::Item => event.item_id(),
                FilterKey::Kind => Some(event.kind()),
            };
            if let Some(value) = value.filter(|v| !v.trim().is_empty()) {
                values.insert(value);
            }
        }
        for value in values {
            out.push(Completion {
                value: format(&filters.with(key, Some(value.to_string())), window, tree),
                label: std::format!("{}:{value}", key.name()),
                active: filters.get(key) == Some(value),
            });
        }
    }
    let current = window.unwrap_or(DEFAULT_WINDOW);
    for range in RANGES {
        out.push(Completion {
            value: format(filters, Some(range), tree),
            label: std::format!("range:{range}"),
            active: current == range,
        });
    }
    out.push(Completion {
        value: format(filters, window, true),
        label: "tree:on".to_string(),
        active: tree,
    });
    out
}

// -- tones and legend -----------------------------------------------------------

/// Dot colour class. Declaration order is legend order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    /// `info`
    Blue,
    /// `ok`
    Green,
    /// `warn`
    Yellow,
    /// `err`
    Red,
    /// `neutral`, drawn hollow.
    Hollow,
}

impl Tone {
    /// Legend order.
    pub const ALL: [Tone; 5] = [
        Tone::Blue,
        Tone::Green,
        Tone::Yellow,
        Tone::Red,
        Tone::Hollow,
    ];

    /// Colour word shown in the legend.
    pub fn colour(self) -> &'static str {
        match self {
            Tone::Blue => "blue",
            Tone::Green => "green",
            Tone::Yellow => "yellow",
            Tone::Red => "red",
            Tone::Hollow => "hollow",
        }
    }

    /// Tone name used in `data-tone` and `ag-{tone}` classes.
    pub fn name(self) -> &'static str {
        match self {
            Tone::Blue => "info",
            Tone::Green => "ok",
            Tone::Yellow => "warn",
            Tone::Red => "err",
            Tone::Hollow => "neutral",
        }
    }

    /// Cluster-merge rank: `err 4 > warn 3 > ok 2 > info 1 > neutral 0`.
    pub fn severity(self) -> u8 {
        match self {
            Tone::Red => 4,
            Tone::Yellow => 3,
            Tone::Green => 2,
            Tone::Blue => 1,
            Tone::Hollow => 0,
        }
    }

    /// The more severe of two tones; `self` wins ties.
    pub fn merge(self, other: Tone) -> Tone {
        if other.severity() > self.severity() {
            other
        } else {
            self
        }
    }

    /// Most severe tone of a cluster; `None` for an empty cluster.
    pub fn most_severe(tones: impl IntoIterator<Item = Tone>) -> Option<Tone> {
        tones.into_iter().reduce(Tone::merge)
    }
}

/// Item status to tone, sorted by status name. Other statuses are hollow.
pub const STATUS_TONES: [(&str, Tone); 5] = [
    ("blocked", Tone::Red),
    ("claimed", Tone::Blue),
    ("done", Tone::Green),
    ("in_progress", Tone::Blue),
    ("review", Tone::Yellow),
];

/// Event kind to tone, sorted by kind name. Other kinds are hollow.
pub const KIND_TONES: [(&str, Tone); 11] = [
    ("branch_head", Tone::Blue),
    ("handoff", Tone::Yellow),
    ("item_claimed", Tone::Blue),
    ("pr_merged", Tone::Green),
    ("pr_opened", Tone::Blue),
    ("pr_reviewed", Tone::Blue),
    ("run_finished", Tone::Green),
    ("run_reasoning", Tone::Blue),
    ("run_started", Tone::Blue),
    ("run_thought", Tone::Blue),
    ("run_tool", Tone::Blue),
];

/// Kinds the legend lists under HOLLOW, in display order.
pub const NEUTRAL_KINDS: [&str; 5] = [
    "item_created",
    "item_released",
    "item_status",
    "note",
    "commit",
];

/// Kinds drawn in a colour that the legend does not list: the lane kinds of a
/// fork, and the run endings a rail draws yellow (cancelled) and red (failed).
const UNLISTED_KIND_TONES: [(&str, Tone); 4] = [
    ("path", Tone::Blue),
    ("branch", Tone::Blue),
    ("run_cancelled", Tone::Yellow),
    ("run_failed", Tone::Red),
];

/// Tone of an item status; unknown, `open` and `None` are hollow.
pub fn tone_for_status(status: Option<&str>) -> Tone {
    status
        .and_then(|s| STATUS_TONES.iter().find(|(name, _)| *name == s))
        .map_or(Tone::Hollow, |(_, tone)| *tone)
}

/// Tone of an event kind; unknown kinds are hollow.
pub fn tone_for_kind(kind: &str) -> Tone {
    KIND_TONES
        .iter()
        .chain(UNLISTED_KIND_TONES.iter())
        .find(|(name, _)| *name == kind)
        .map_or(Tone::Hollow, |(_, tone)| *tone)
}

/// Tone of an event: `item_status` events take the tone of `detail.status`.
pub fn tone_for_event(kind: &str, status: Option<&str>) -> Tone {
    if kind == "item_status" {
        tone_for_status(status)
    } else {
        tone_for_kind(kind)
    }
}

/// `kind_label`: underscores to spaces, e.g. `pr merged`.
pub fn kind_label(kind: &str) -> String {
    kind.replace('_', " ")
}

/// One legend entry: colour and what it means, from the colouring tables.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LegendEntry {
    /// Tone of the entry.
    pub tone: Tone,
    /// Colour word.
    pub colour: &'static str,
    /// Statuses and kinds drawn in this tone, spaces for underscores.
    pub means: Vec<String>,
}

impl LegendEntry {
    /// First three meanings plus ` …` when more; all otherwise.
    pub fn brief(&self) -> String {
        if self.means.len() > 3 {
            std::format!("{} …", self.means[..3].join(" · "))
        } else {
            self.means.join(" · ")
        }
    }

    /// Tooltip: every meaning joined by ` · `.
    pub fn title(&self) -> String {
        self.means.join(" · ")
    }
}

static LEGEND: LazyLock<Vec<LegendEntry>> = LazyLock::new(|| {
    Tone::ALL
        .into_iter()
        .map(|tone| {
            let statuses = STATUS_TONES
                .iter()
                .filter(|(_, t)| *t == tone)
                .map(|(n, _)| *n);
            let kinds = KIND_TONES
                .iter()
                .filter(|(_, t)| *t == tone)
                .map(|(n, _)| *n);
            let neutral = NEUTRAL_KINDS
                .iter()
                .copied()
                .filter(|_| tone == Tone::Hollow);
            let mut means: Vec<String> = Vec::new();
            for name in statuses.chain(kinds).chain(neutral) {
                let label = kind_label(name);
                if !means.contains(&label) {
                    means.push(label);
                }
            }
            LegendEntry {
                tone,
                colour: tone.colour(),
                means,
            }
        })
        .collect()
});

/// The legend in fixed order BLUE GREEN YELLOW RED HOLLOW.
pub fn legend() -> &'static [LegendEntry] {
    &LEGEND
}

/// A legend entry plus its filter state, ready to paint.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LegendChip {
    /// Tone of the chip.
    pub tone: Tone,
    /// Colour word.
    pub colour: &'static str,
    /// Tooltip text.
    pub title: String,
    /// Short meaning text for the full legend.
    pub brief: String,
    /// `aria-pressed`: the tone is selected.
    pub pressed: bool,
    /// A tone filter is on and this tone is not in it; paint faded.
    pub dimmed: bool,
}

// -- mode and toolbar state ---------------------------------------------------

/// Which body the view shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Swimlane graph.
    #[default]
    Graph,
    /// Lanes sequence diagram.
    Lanes,
    /// Collaboration pairs and threads.
    Collab,
}

impl Mode {
    /// Chip order.
    pub const ALL: [Mode; 3] = [Mode::Graph, Mode::Lanes, Mode::Collab];

    /// Wire token: `graph`, `lanes`, `collaboration`.
    pub fn token(self) -> &'static str {
        match self {
            Mode::Graph => "graph",
            Mode::Lanes => "lanes",
            Mode::Collab => "collaboration",
        }
    }

    /// Chip label: `graph`, `lanes`, `collab`.
    pub fn label(self) -> &'static str {
        match self {
            Mode::Graph => "graph",
            Mode::Lanes => "lanes",
            Mode::Collab => "collab",
        }
    }

    /// Token lookup; unknown tokens read as `Graph`.
    pub fn from_token(token: &str) -> Mode {
        Mode::ALL
            .into_iter()
            .find(|mode| mode.token() == token)
            .unwrap_or_default()
    }

    /// The query line and TREE button exist in graph and lanes.
    pub fn has_query_line(self) -> bool {
        self != Mode::Collab
    }

    /// The legend exists in graph only.
    pub fn has_legend(self) -> bool {
        self == Mode::Graph
    }
}

/// Which lanes of the LANES diagram are drawn; the rest fold into one `+N` lane.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LanePick {
    /// The busiest eight when there are more lanes than the cap, every lane otherwise.
    #[default]
    Auto,
    /// Every lane.
    All,
    /// Exactly these agents, by name; an empty set draws none.
    Only(BTreeSet<String>),
}

/// A mode chip ready to paint.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ModeChip {
    /// Mode the chip selects.
    pub mode: Mode,
    /// DOM id `activity-modes-{token}`.
    pub id: String,
    /// Visible label.
    pub label: &'static str,
    /// `aria-label`: `show {label}`.
    pub aria_label: String,
    /// `aria-pressed`: this is the current mode.
    pub pressed: bool,
}

/// Last "selected item" prop the state applied.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Followed {
    /// No prop applied yet.
    #[default]
    Unset,
    /// The applied prop; `None` is "no selection".
    Item(Option<String>),
}

/// One active token of the query line, with the line that removes it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct QueryToken {
    /// Key name: `agent`, `item`, `kind`, `range` or `tree`.
    pub key: &'static str,
    /// Value as written; `on` for tree.
    pub value: String,
    /// `key:value`.
    pub label: String,
    /// The full line with this token cleared.
    pub remove: String,
}

/// Toolbar, mode and legend state of one event-graph view.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct GraphState {
    /// `range:` label; always a valid window token.
    pub window: String,
    /// Active filters.
    pub filters: Filters,
    /// Current body.
    pub mode: Mode,
    /// Draw forks as child lanes.
    pub tree: bool,
    /// LANES: the lanes drawn; the rest fold into one `+N` lane.
    pub lanes: LanePick,
    /// LANES: the agent whose rows alone are drawn.
    pub lane: Option<String>,
    /// LANES: the lane menu's filter text.
    pub lane_filter: String,
    /// Width in px the swimlane is laid out at; always > 0.
    pub graph_width: u32,
    /// Tokens the last submitted line ignored.
    pub query_errors: Vec<String>,
    /// Graph: lift the row cap.
    pub show_all: bool,
    /// No-item strip kinds currently expanded, in expansion order; graph mode only.
    pub loose_kinds: Vec<String>,
    /// Last applied selected-item prop.
    pub followed: Followed,
    /// Compact register (one bar) instead of stacked.
    pub compact: bool,
    /// Selected legend tones; empty shows every tone.
    pub tones: BTreeSet<Tone>,
}

impl Default for GraphState {
    fn default() -> Self {
        GraphState {
            window: DEFAULT_WINDOW.to_string(),
            filters: Filters::default(),
            mode: Mode::Graph,
            tree: false,
            lanes: LanePick::Auto,
            lane: None,
            lane_filter: String::new(),
            graph_width: DEFAULT_GRAPH_WIDTH,
            query_errors: Vec::new(),
            show_all: false,
            loose_kinds: Vec::new(),
            followed: Followed::Unset,
            compact: false,
            tones: BTreeSet::new(),
        }
    }
}

/// An event the toolbar can raise.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphAction {
    /// A submitted query line (raw text).
    Query(String),
    /// The TREE button.
    ToggleTree,
    /// A mode chip, by wire token.
    SetMode(String),
    /// The show-all button.
    ToggleShowAll,
    /// The lane menu: draw these lanes.
    PickLanes(LanePick),
    /// A lane header: draw only the rows touching this agent (`None` clears).
    FocusLane(Option<String>),
    /// The lane menu's filter box.
    LaneFilter(String),
    /// Measured graph width; applied only when a positive integer.
    GraphWidth(i64),
    /// A no-item strip row, by kind.
    ToggleStripKind(String),
    /// New activity arrived.
    Refresh,
    /// Selected-item prop (`None` clears it).
    FollowItem(Option<String>),
    /// A legend chip.
    ToggleTone(Tone),
    /// Drop the tone filter.
    ClearTones,
}

/// What the host must redo after [`GraphState::reduce`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Effect {
    /// Nothing to recompute.
    None,
    /// Re-lay out the graph from the events already read.
    Relayout,
    /// Re-read events, then lay out.
    Reload,
}

impl GraphState {
    /// Canonical query line for this state; what the input shows when not being edited.
    pub fn line(&self) -> String {
        format(&self.filters, Some(&self.window), self.tree)
    }

    /// The window as an interval at `now`; `None` if the label is not a window.
    pub fn interval(&self, now: i64) -> Option<Interval> {
        Window::parse(&self.window).ok().map(|w| w.resolve(now))
    }

    /// `ignored: …` text for the error span; `None` when nothing was ignored.
    pub fn errors_text(&self) -> Option<String> {
        (!self.query_errors.is_empty())
            .then(|| std::format!("ignored: {}", self.query_errors.join(" ")))
    }

    /// Whether `tone` is drawn; an empty selection shows every tone.
    pub fn tone_visible(&self, tone: Tone) -> bool {
        self.tones.is_empty() || self.tones.contains(&tone)
    }

    /// Whether any filter, non-default window, TREE or tone selection is on.
    pub fn is_filtering(&self) -> bool {
        self.filters.any() || self.window != DEFAULT_WINDOW || self.tree || !self.tones.is_empty()
    }

    /// Whether the strip row for `kind` is expanded.
    pub fn is_strip_open(&self, kind: &str) -> bool {
        self.loose_kinds.iter().any(|k| k == kind)
    }

    /// Mode chips with the current one pressed.
    pub fn mode_chips(&self) -> Vec<ModeChip> {
        Mode::ALL
            .into_iter()
            .map(|mode| ModeChip {
                mode,
                id: std::format!("activity-modes-{}", mode.token()),
                label: mode.label(),
                aria_label: std::format!("show {}", mode.label()),
                pressed: mode == self.mode,
            })
            .collect()
    }

    /// Legend chips with selected and faded state.
    pub fn legend_chips(&self) -> Vec<LegendChip> {
        legend()
            .iter()
            .map(|entry| LegendChip {
                tone: entry.tone,
                colour: entry.colour,
                title: entry.title(),
                brief: entry.brief(),
                pressed: self.tones.contains(&entry.tone),
                dimmed: !self.tone_visible(entry.tone),
            })
            .collect()
    }

    /// Active query tokens in line order, each with the line that removes it.
    pub fn tokens(&self) -> Vec<QueryToken> {
        let mut out = Vec::new();
        for key in FilterKey::ALL {
            if let Some(value) = self.filters.get(key).filter(|v| !v.is_empty()) {
                out.push(QueryToken {
                    key: key.name(),
                    value: value.to_string(),
                    label: std::format!("{}:{value}", key.name()),
                    remove: format(&self.filters.with(key, None), Some(&self.window), self.tree),
                });
            }
        }
        if self.window != DEFAULT_WINDOW {
            out.push(QueryToken {
                key: "range",
                value: self.window.clone(),
                label: std::format!("range:{}", self.window),
                remove: format(&self.filters, None, self.tree),
            });
        }
        if self.tree {
            out.push(QueryToken {
                key: "tree",
                value: "on".to_string(),
                label: "tree:on".to_string(),
                remove: format(&self.filters, Some(&self.window), false),
            });
        }
        out
    }

    /// Datalist options over the unfiltered events of the window.
    pub fn completions<E: Queryable>(&self, events: &[E]) -> Vec<Completion> {
        completions(&self.filters, Some(&self.window), self.tree, events)
    }

    /// Applies one action and reports what must be recomputed.
    pub fn reduce(&mut self, action: GraphAction) -> Effect {
        match action {
            GraphAction::Query(text) => {
                let parsed = parse(&text);
                self.query_errors = parsed.errors;
                self.filters = parsed.filters;
                self.window = parsed.range.unwrap_or_else(|| DEFAULT_WINDOW.to_string());
                self.tree = parsed.tree;
                self.tones.clear();
                Effect::Reload
            }
            GraphAction::ToggleTree => {
                self.tree = !self.tree;
                Effect::Reload
            }
            GraphAction::SetMode(token) => {
                self.query_errors.clear();
                self.mode = Mode::from_token(&token);
                if self.mode != Mode::Graph {
                    self.loose_kinds.clear();
                }
                Effect::Reload
            }
            GraphAction::ToggleShowAll => {
                self.show_all = !self.show_all;
                Effect::Relayout
            }
            GraphAction::PickLanes(pick) => {
                let drops_lane = match (&pick, &self.lane) {
                    (LanePick::Only(names), Some(lane)) => !names.contains(lane),
                    _ => false,
                };
                if drops_lane {
                    self.lane = None;
                }
                self.lanes = pick;
                Effect::None
            }
            GraphAction::FocusLane(agent) => {
                self.lane = agent.filter(|name| !name.trim().is_empty());
                Effect::None
            }
            GraphAction::LaneFilter(text) => {
                self.lane_filter = text;
                Effect::None
            }
            GraphAction::GraphWidth(width) if width > 0 => {
                self.graph_width = u32::try_from(width).unwrap_or(u32::MAX);
                Effect::Relayout
            }
            GraphAction::GraphWidth(_) => Effect::None,
            GraphAction::ToggleStripKind(kind) => {
                if self.mode != Mode::Graph {
                    return Effect::None;
                }
                match self.loose_kinds.iter().position(|k| *k == kind) {
                    Some(at) => {
                        self.loose_kinds.remove(at);
                    }
                    None => self.loose_kinds.push(kind),
                }
                Effect::Relayout
            }
            GraphAction::Refresh => Effect::Reload,
            GraphAction::FollowItem(item) => {
                if self.followed == Followed::Item(item.clone()) {
                    return Effect::None;
                }
                self.followed = Followed::Item(item.clone());
                self.filters.item = item;
                self.query_errors.clear();
                Effect::Reload
            }
            GraphAction::ToggleTone(tone) => {
                if !self.tones.remove(&tone) {
                    self.tones.insert(tone);
                }
                Effect::Relayout
            }
            GraphAction::ClearTones => {
                if self.tones.is_empty() {
                    Effect::None
                } else {
                    self.tones.clear();
                    Effect::Relayout
                }
            }
        }
    }

    /// View state from URL params (last occurrence wins); other fields default.
    pub fn from_params(params: &[(&str, &str)]) -> GraphState {
        let get = |name: &str| {
            params
                .iter()
                .rev()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| *v)
        };
        let filter = |name: &str| {
            get(name)
                .filter(|v| !v.trim().is_empty())
                .map(str::to_string)
        };
        GraphState {
            filters: Filters {
                agent: filter("agent"),
                item: filter("item"),
                kind: filter("kind"),
            },
            window: get("window")
                .filter(|w| Window::is_valid(w))
                .unwrap_or(DEFAULT_WINDOW)
                .to_string(),
            mode: get("mode").map_or(Mode::Graph, Mode::from_token),
            tree: get("tree") == Some("1"),
            lanes: match get("lanes") {
                None => LanePick::Auto,
                Some("all") => LanePick::All,
                Some(names) => LanePick::Only(
                    names
                        .split(',')
                        .filter(|n| !n.trim().is_empty())
                        .map(str::to_string)
                        .collect(),
                ),
            },
            lane: filter("lane"),
            ..GraphState::default()
        }
    }

    /// URL query string (`""` or `?…`): non-default fields, form-encoded.
    pub fn query_string(&self) -> String {
        let mut parts: Vec<(&str, &str)> = Vec::new();
        if self.mode != Mode::Graph {
            parts.push(("mode", self.mode.token()));
        }
        for key in FilterKey::ALL {
            if let Some(value) = self.filters.get(key).filter(|v| !v.is_empty()) {
                parts.push((key.name(), value));
            }
        }
        if self.window != DEFAULT_WINDOW {
            parts.push(("window", &self.window));
        }
        // Agent names containing `,` cannot round-trip and are skipped; so are blank ones.
        let lanes_value = match &self.lanes {
            LanePick::Auto => None,
            LanePick::All => Some("all".to_string()),
            LanePick::Only(names) => Some(
                names
                    .iter()
                    .filter(|n| !n.contains(',') && !n.trim().is_empty())
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(","),
            ),
        };
        if let Some(value) = &lanes_value {
            parts.push(("lanes", value));
        }
        if let Some(lane) = &self.lane {
            parts.push(("lane", lane));
        }
        if self.tree {
            parts.push(("tree", "1"));
        }
        if parts.is_empty() {
            return String::new();
        }
        let encoded: Vec<String> = parts
            .into_iter()
            .map(|(k, v)| std::format!("{}={}", encode_www_form(k), encode_www_form(v)))
            .collect();
        std::format!("?{}", encoded.join("&"))
    }
}

/// `x-www-form-urlencoded`: alphanumerics and `-._~` kept, space as `+`.
fn encode_www_form(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(char::from(byte));
            }
            b' ' => out.push('+'),
            _ => out.push_str(&std::format!("%{byte:02X}")),
        }
    }
    out
}

/// Label of the show-all button.
pub fn show_all_label(show_all: bool, total_items: usize) -> String {
    if show_all {
        std::format!("show newest {ROW_CAP}")
    } else {
        std::format!("show all ({total_items})")
    }
}

#[cfg(test)]
mod tests;
