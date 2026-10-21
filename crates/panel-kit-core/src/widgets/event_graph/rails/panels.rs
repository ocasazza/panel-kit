//! Session-page models next to the rail: metrics, sparkline, command palette,
//! the orchestrator reasoning trace, and the run-trajectory rail.

use std::collections::HashSet;

use super::super::model::Event;
use super::super::model::EventKind;
use super::super::model::ItemRef;
use super::super::model::Micros;
use super::super::model::SessionTree;
use super::super::query::Tone;
use super::super::time;
use super::one_line;
use super::plural;
use super::vertical;
use super::VerticalGraph;

/// Shown for an absent value; a recorded zero prints as `0`.
pub const DASH: &str = "—";

/// A metric value as a host hands it over.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MetricValue {
    /// Instant, shown as `HH:MM:SS`.
    Clock(Micros),
    /// Count, shown in decimal.
    Count(i64),
    /// Text, shown as is.
    Text(String),
}

/// Text of a metric cell: [`DASH`] when absent.
pub fn shown(value: Option<&MetricValue>) -> String {
    match value {
        None => DASH.to_owned(),
        Some(MetricValue::Clock(at)) => time::format_clock(*at),
        Some(MetricValue::Count(n)) => n.to_string(),
        Some(MetricValue::Text(text)) => text.clone(),
    }
}

/// Lowercase `label` with each run of characters outside `[a-z0-9]` as one `-`.
pub fn slug(label: &str) -> String {
    let mut out = String::with_capacity(label.len());
    let mut dash = false;
    for c in label.to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            out.push(c);
            dash = false;
        } else if !dash {
            out.push('-');
            dash = true;
        }
    }
    out
}

/// One row of the metrics pane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetricRow {
    /// Element id, `<pane id>-<slug>`.
    pub dom_id: String,
    /// Label.
    pub label: String,
    /// Value text.
    pub value: String,
    /// No value recorded: drawn faint.
    pub absent: bool,
}

/// Resolve metric rows under pane id `id`.
pub fn metric_rows(id: &str, rows: &[(&str, Option<MetricValue>)]) -> Vec<MetricRow> {
    rows.iter()
        .map(|(label, value)| MetricRow {
            dom_id: format!("{id}-{}", slug(label)),
            label: (*label).to_owned(),
            value: shown(value.as_ref()),
            absent: value.is_none(),
        })
        .collect()
}

/// Run state as a session head chip tone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionState {
    /// Starting up.
    Starting,
    /// Running.
    Running,
    /// Paused.
    Paused,
    /// Finished cleanly.
    Finished,
    /// Failed.
    Failed,
    /// Cancelled.
    Cancelled,
    /// Host lost.
    Lost,
}

impl SessionState {
    /// Chip tone: starting neutral, running info, paused/cancelled warn,
    /// finished ok, failed/lost err.
    pub fn tone(self) -> Tone {
        match self {
            SessionState::Starting => Tone::Hollow,
            SessionState::Running => Tone::Blue,
            SessionState::Paused | SessionState::Cancelled => Tone::Yellow,
            SessionState::Finished => Tone::Green,
            SessionState::Failed | SessionState::Lost => Tone::Red,
        }
    }
}

const BLOCKS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// One glyph of the sparkline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SparkBar {
    /// Block glyph, `·` for unknown, `—` for no series.
    pub glyph: char,
    /// State unknown: drawn faint.
    pub unknown: bool,
}

/// Sparkline glyphs and its scale caption.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spark {
    /// One bar per value; a single unknown `—` for an empty series.
    pub bars: Vec<SparkBar>,
    /// `24 minutes · full bar 35 tokens`, or `no minutes recorded`.
    pub scale: String,
}

/// Sparkline over `values` (absent = unknown) spanning `window` minutes.
pub fn spark(values: &[Option<u64>], window: usize) -> Spark {
    if values.is_empty() {
        return Spark {
            bars: vec![SparkBar {
                glyph: '—',
                unknown: true,
            }],
            scale: "no minutes recorded".to_owned(),
        };
    }
    let peak = values.iter().flatten().copied().max();
    let scale = peak.unwrap_or(0);
    let bars = values
        .iter()
        .map(|value| match value {
            None => SparkBar {
                glyph: '·',
                unknown: true,
            },
            Some(0) => SparkBar {
                glyph: BLOCKS[0],
                unknown: false,
            },
            Some(_) if scale == 0 => SparkBar {
                glyph: '·',
                unknown: true,
            },
            Some(v) => {
                let level = (u128::from(*v) * 8).div_ceil(u128::from(scale)).clamp(1, 8);
                SparkBar {
                    glyph: BLOCKS[level as usize - 1],
                    unknown: false,
                }
            }
        })
        .collect();
    let scale = match peak {
        None => "no minutes recorded".to_owned(),
        Some(p) => format!(
            "{} · full bar {}",
            plural(window, "minute"),
            plural(p as usize, "token")
        ),
    };
    Spark { bars, scale }
}

/// A run event reduced to what the tokens-per-minute series reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TokenTick {
    /// Event instant.
    pub at: Micros,
    /// Input plus output tokens of a `model_response`; 0 for anything else.
    pub tokens: u64,
}

const MICROS_PER_MINUTE: i64 = 60_000_000;

/// Last `minutes` minute buckets ending at the latest event: per-minute token
/// sum, `None` for a minute with no token-carrying event; empty without events.
pub fn tokens_per_minute(events: &[TokenTick], minutes: usize) -> Vec<Option<u64>> {
    let Some(last) = events
        .iter()
        .map(|e| e.at.div_euclid(MICROS_PER_MINUTE))
        .max()
    else {
        return Vec::new();
    };
    let first = last - minutes as i64 + 1;
    let mut buckets: Vec<Option<u64>> = vec![None; minutes];
    for tick in events.iter().filter(|t| t.tokens > 0) {
        let minute = tick.at.div_euclid(MICROS_PER_MINUTE);
        if minute >= first {
            let slot = &mut buckets[(minute - first) as usize];
            *slot = Some(slot.unwrap_or(0) + tick.tokens);
        }
    }
    buckets
}

/// Palette cursor after moving by `delta`; it stops at both ends.
pub fn palette_cursor(cursor: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        return 0;
    }
    cursor.saturating_add_signed(delta).min(len - 1)
}

/// Kind of an orchestrator-trace node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceKind {
    /// Root of the trace.
    Root,
    /// Alternative explored.
    Branch,
    /// Action taken.
    Action,
    /// Result observed.
    Result,
    /// Anything else.
    Other,
}

impl TraceKind {
    /// Row icon: `◆ ◇ ▸ ●`, `·` otherwise.
    pub fn icon(self) -> char {
        match self {
            TraceKind::Root => '◆',
            TraceKind::Branch => '◇',
            TraceKind::Action => '▸',
            TraceKind::Result => '●',
            TraceKind::Other => '·',
        }
    }
}

/// Status of an orchestrator-trace node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceStatus {
    /// Succeeded.
    Ok,
    /// Failed.
    Err,
    /// Needs attention.
    Warn,
    /// Informational.
    Info,
    /// Not started.
    Pending,
    /// In progress.
    Active,
    /// Abandoned: struck through.
    Pruned,
    /// Unrecognised status.
    Unknown,
}

/// Colour token of a trace row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceColour {
    /// `--ok`.
    Ok,
    /// `--err`.
    Err,
    /// `--warn`.
    Warn,
    /// `--info`.
    Info,
    /// `--fg-dim`.
    Dim,
    /// `--fg-faint`.
    Faint,
    /// `--fg`.
    Fg,
}

impl TraceStatus {
    /// Colour of icon and label.
    pub fn colour(self) -> TraceColour {
        match self {
            TraceStatus::Ok => TraceColour::Ok,
            TraceStatus::Err => TraceColour::Err,
            TraceStatus::Warn => TraceColour::Warn,
            TraceStatus::Info | TraceStatus::Active => TraceColour::Info,
            TraceStatus::Pending => TraceColour::Dim,
            TraceStatus::Pruned => TraceColour::Faint,
            TraceStatus::Unknown => TraceColour::Fg,
        }
    }
}

/// Node of the orchestrator reasoning tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceNode {
    /// Unique id.
    pub id: String,
    /// Node kind.
    pub kind: TraceKind,
    /// Node status.
    pub status: TraceStatus,
    /// Label.
    pub label: String,
    /// Instant, if recorded.
    pub at: Option<Micros>,
    /// Child nodes.
    pub children: Vec<TraceNode>,
}

/// One visible row of the trace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceRow {
    /// Node id.
    pub id: String,
    /// Depth, 0 for roots.
    pub depth: usize,
    /// Left padding in px: `depth * 16 + 8`.
    pub padding_px: usize,
    /// Kind icon.
    pub icon: char,
    /// Icon and label colour.
    pub colour: TraceColour,
    /// Label struck through (pruned).
    pub strike: bool,
    /// Label.
    pub label: String,
    /// `HH:MM:SS`, or empty.
    pub time: String,
    /// Node has children: a toggle is drawn.
    pub has_children: bool,
    /// Children are shown.
    pub expanded: bool,
}

/// Visible rows depth-first; children of an id in `collapsed` are hidden.
pub fn trace_rows(roots: &[TraceNode], collapsed: &HashSet<String>) -> Vec<TraceRow> {
    fn walk(
        nodes: &[TraceNode],
        depth: usize,
        collapsed: &HashSet<String>,
        out: &mut Vec<TraceRow>,
    ) {
        for node in nodes {
            let has_children = !node.children.is_empty();
            let expanded = has_children && !collapsed.contains(&node.id);
            out.push(TraceRow {
                id: node.id.clone(),
                depth,
                padding_px: depth * 16 + 8,
                icon: node.kind.icon(),
                colour: node.status.colour(),
                strike: node.status == TraceStatus::Pruned,
                label: node.label.clone(),
                time: node.at.map(time::format_clock).unwrap_or_default(),
                has_children,
                expanded,
            });
            if expanded {
                walk(&node.children, depth + 1, collapsed, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(roots, 0, collapsed, &mut out);
    out
}

/// Flip `id` between collapsed and expanded.
pub fn toggle_trace_node(collapsed: &mut HashSet<String>, id: &str) {
    if !collapsed.remove(id) {
        collapsed.insert(id.to_owned());
    }
}

/// A run event as the trajectory rail reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunTick {
    /// Run's own counter; 0 when absent.
    pub sequence: Option<i64>,
    /// Event instant; the previous event's instant when absent.
    pub timestamp: Option<Micros>,
    /// Run-event kind (`tool_call`, `model_response`, ...).
    pub kind: String,
    /// Iteration of a `program` event.
    pub iteration: Option<i64>,
    /// Worded text of the event (see [`describe_tool_call`] and friends); the kind when `None`.
    pub text: Option<String>,
}

/// Activity kind a run-event kind is drawn as.
pub fn run_event_kind(kind: &str) -> EventKind {
    match kind {
        "run_started" => EventKind::RunStarted,
        "tool_call" | "tool_result" => EventKind::RunTool,
        "reasoning" => EventKind::RunReasoning,
        "model_request" | "model_response" => EventKind::RunModel,
        "run_finished" => EventKind::RunFinished,
        "run_failed" => EventKind::RunFailed,
        "run_cancelled" => EventKind::RunCancelled,
        _ => EventKind::RunThought,
    }
}

/// Trajectory rail of one run: single column, no lanes, neutral item line.
/// `None` for a run with no events.
pub fn run_trajectory(events: &[RunTick], run_id: &str) -> Option<VerticalGraph> {
    if events.is_empty() {
        return None;
    }
    let mut previous: Micros = 0;
    let mapped: Vec<Event> = events
        .iter()
        .map(|tick| {
            let at = tick.timestamp.unwrap_or(previous);
            previous = at;
            let summary = if tick.kind == "program" {
                match tick.iteration {
                    Some(n) => format!("program attempt {n}"),
                    None => "program attempt".to_owned(),
                }
            } else {
                one_line(tick.text.as_deref().unwrap_or(&tick.kind))
            };
            Event::new(
                format!("{run_id}:{}", tick.sequence.unwrap_or(0)),
                at,
                run_event_kind(&tick.kind),
            )
            .item(run_id)
            .summary(summary)
        })
        .collect();
    let item = ItemRef {
        id: run_id.to_owned(),
        status: Some("run".to_owned()),
    };
    Some(vertical(&mapped, &item, &SessionTree::default()))
}

/// Cut `text` to `limit` characters, appending `…` when cut.
pub fn clip(text: &str, limit: usize) -> String {
    if text.chars().count() > limit {
        let mut out: String = text.chars().take(limit).collect();
        out.push('…');
        out
    } else {
        text.to_owned()
    }
}

/// Character limit of a worded run-event summary.
pub const SUMMARY_LIMIT: usize = 160;

/// `{tool} ← {input}`.
pub fn describe_tool_call(tool: &str, input: &str) -> String {
    format!("{tool} ← {}", clip(input, SUMMARY_LIMIT))
}

/// `{tool} → {outcome}`, plus ` — {error}` when the call failed; the outcome
/// is `result` when none is recorded.
pub fn describe_tool_result(tool: &str, outcome: Option<&str>, error: Option<&str>) -> String {
    let error = match error {
        Some(e) if !e.is_empty() => format!(" — {}", clip(e, SUMMARY_LIMIT)),
        _ => String::new(),
    };
    format!("{tool} → {}{error}", outcome.unwrap_or("result"))
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use rstest::rstest;

    use super::*;
    use crate::widgets::event_graph::time::from_utc;

    #[test]
    fn absent_is_a_dash_and_zero_is_a_fact() {
        assert_eq!(shown(None), "—");
        assert_eq!(shown(Some(&MetricValue::Count(0))), "0");
        assert_eq!(
            shown(Some(&MetricValue::Text("pool-model".into()))),
            "pool-model"
        );
        assert_eq!(
            shown(Some(&MetricValue::Clock(from_utc(2026, 10, 1, 4, 5, 6)))),
            "04:05:06"
        );
    }

    #[rstest]
    #[case("tokens in", "tokens-in")]
    #[case("lm last", "lm-last")]
    #[case("host mem", "host-mem")]
    #[case("A  b__C", "a-b-c")]
    #[case("tool calls", "tool-calls")]
    fn slugs(#[case] label: &str, #[case] want: &str) {
        assert_eq!(slug(label), want);
    }

    #[test]
    fn metric_rows_mark_absent_values() {
        let rows = metric_rows(
            "session-pane-metrics",
            &[
                ("tokens in", Some(MetricValue::Count(750))),
                ("lm last", None),
            ],
        );
        assert_eq!(rows[0].dom_id, "session-pane-metrics-tokens-in");
        assert_eq!((rows[0].value.as_str(), rows[0].absent), ("750", false));
        assert_eq!((rows[1].value.as_str(), rows[1].absent), ("—", true));
    }

    #[rstest]
    #[case(SessionState::Starting, Tone::Hollow)]
    #[case(SessionState::Running, Tone::Blue)]
    #[case(SessionState::Paused, Tone::Yellow)]
    #[case(SessionState::Finished, Tone::Green)]
    #[case(SessionState::Failed, Tone::Red)]
    #[case(SessionState::Cancelled, Tone::Yellow)]
    #[case(SessionState::Lost, Tone::Red)]
    fn session_state_tones(#[case] state: SessionState, #[case] want: Tone) {
        assert_eq!(state.tone(), want);
    }

    fn glyphs(s: &Spark) -> String {
        s.bars.iter().map(|b| b.glyph).collect()
    }

    #[test]
    fn spark_scales_to_the_peak() {
        let s = spark(&[Some(15), Some(25), Some(35), None, Some(0)], 24);
        assert_eq!(glyphs(&s), "▄▆█·▁");
        assert_eq!(s.scale, "24 minutes · full bar 35 tokens");
        assert!(s.bars[3].unknown && !s.bars[4].unknown);
    }

    #[test]
    fn spark_without_series_or_values() {
        let empty = spark(&[], 24);
        assert_eq!(glyphs(&empty), "—");
        assert!(empty.bars[0].unknown);
        assert_eq!(empty.scale, "no minutes recorded");
        let none = spark(&[None, None], 24);
        assert_eq!(glyphs(&none), "··");
        assert_eq!(none.scale, "no minutes recorded");
    }

    #[test]
    fn spark_singular_units() {
        assert_eq!(spark(&[Some(1)], 1).scale, "1 minute · full bar 1 token");
    }

    #[test]
    fn spark_all_zero_has_a_zero_peak() {
        let s = spark(&[Some(0), Some(0)], 2);
        assert_eq!(glyphs(&s), "▁▁");
        assert_eq!(s.scale, "2 minutes · full bar 0 tokens");
    }

    #[test]
    fn tokens_per_minute_buckets_end_at_the_last_event() {
        let t0 = from_utc(2026, 10, 1, 12, 0, 0);
        let events = [
            TokenTick { at: t0, tokens: 15 },
            TokenTick {
                at: t0 + 10_000_000,
                tokens: 5,
            },
            TokenTick {
                at: t0 + 2 * MICROS_PER_MINUTE,
                tokens: 35,
            },
            TokenTick {
                at: t0 + 3 * MICROS_PER_MINUTE,
                tokens: 0,
            },
        ];
        let series = tokens_per_minute(&events, 24);
        assert_eq!(series.len(), 24);
        assert_eq!(series[23], None);
        assert_eq!(series[22], Some(35));
        assert_eq!(series[21], None);
        assert_eq!(series[20], Some(20));
        assert_eq!(series.iter().flatten().sum::<u64>(), 55);
        assert!(tokens_per_minute(&[], 24).is_empty());
    }

    #[test]
    fn tokens_older_than_the_window_are_dropped() {
        let t0 = from_utc(2026, 10, 1, 12, 0, 0);
        let events = [
            TokenTick { at: t0, tokens: 9 },
            TokenTick {
                at: t0 + 30 * MICROS_PER_MINUTE,
                tokens: 1,
            },
        ];
        assert_eq!(
            tokens_per_minute(&events, 24).iter().flatten().sum::<u64>(),
            1
        );
    }

    #[rstest]
    #[case(0, 3, -1, 0)]
    #[case(0, 3, 1, 1)]
    #[case(2, 3, 1, 2)]
    #[case(1, 3, -5, 0)]
    #[case(0, 0, 1, 0)]
    fn palette_cursor_stops_at_both_ends(
        #[case] cursor: usize,
        #[case] len: usize,
        #[case] delta: isize,
        #[case] want: usize,
    ) {
        assert_eq!(palette_cursor(cursor, len, delta), want);
    }

    fn leaf(id: &str, kind: TraceKind, status: TraceStatus) -> TraceNode {
        TraceNode {
            id: id.into(),
            kind,
            status,
            label: id.into(),
            at: None,
            children: vec![],
        }
    }

    fn sample_trace() -> Vec<TraceNode> {
        let mut root = leaf("root", TraceKind::Root, TraceStatus::Info);
        let mut branch = leaf("b", TraceKind::Branch, TraceStatus::Pruned);
        branch
            .children
            .push(leaf("a", TraceKind::Action, TraceStatus::Active));
        root.children.push(branch);
        let mut res = leaf("r", TraceKind::Result, TraceStatus::Ok);
        res.at = Some(from_utc(2026, 10, 1, 1, 2, 3));
        root.children.push(res);
        vec![root]
    }

    #[test]
    fn trace_rows_are_depth_first_with_padding_icons_and_colours() {
        let rows = trace_rows(&sample_trace(), &HashSet::new());
        let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["root", "b", "a", "r"]);
        assert_eq!(
            rows.iter().map(|r| r.padding_px).collect::<Vec<_>>(),
            [8, 24, 40, 24]
        );
        assert_eq!(rows.iter().map(|r| r.icon).collect::<String>(), "◆◇▸●");
        assert_eq!(rows[1].colour, TraceColour::Faint);
        assert!(rows[1].strike && !rows[0].strike);
        assert_eq!(rows[2].colour, TraceColour::Info);
        assert_eq!(
            (rows[3].time.as_str(), rows[0].time.as_str()),
            ("01:02:03", "")
        );
        assert!(
            rows[0].has_children && rows[0].expanded && !rows[3].has_children && !rows[3].expanded
        );
    }

    #[test]
    fn collapsing_hides_descendants_and_toggles_back() {
        let mut collapsed = HashSet::new();
        toggle_trace_node(&mut collapsed, "b");
        let rows = trace_rows(&sample_trace(), &collapsed);
        assert_eq!(
            rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            ["root", "b", "r"]
        );
        assert!(!rows[1].expanded && rows[1].has_children);
        toggle_trace_node(&mut collapsed, "b");
        assert_eq!(trace_rows(&sample_trace(), &collapsed).len(), 4);
    }

    #[rstest]
    #[case(TraceStatus::Ok, TraceColour::Ok)]
    #[case(TraceStatus::Err, TraceColour::Err)]
    #[case(TraceStatus::Warn, TraceColour::Warn)]
    #[case(TraceStatus::Info, TraceColour::Info)]
    #[case(TraceStatus::Pending, TraceColour::Dim)]
    #[case(TraceStatus::Active, TraceColour::Info)]
    #[case(TraceStatus::Pruned, TraceColour::Faint)]
    #[case(TraceStatus::Unknown, TraceColour::Fg)]
    fn trace_colours(#[case] status: TraceStatus, #[case] want: TraceColour) {
        assert_eq!(status.colour(), want);
        assert_eq!(TraceKind::Other.icon(), '·');
    }

    fn tick(seq: i64, kind: &str, at: Option<Micros>, text: Option<&str>) -> RunTick {
        RunTick {
            sequence: Some(seq),
            timestamp: at,
            kind: kind.into(),
            iteration: None,
            text: text.map(str::to_owned),
        }
    }

    #[test]
    fn no_events_is_no_trajectory() {
        assert!(run_trajectory(&[], "run-1").is_none());
    }

    #[test]
    fn trajectory_is_one_column_neutral_item_line() {
        let t0 = from_utc(2026, 10, 1, 0, 0, 0);
        let g = run_trajectory(
            &[
                tick(0, "run_started", Some(t0), Some("")),
                tick(1, "tool_call", Some(t0 + 1), Some("grep ←  x\ny")),
                tick(2, "program", Some(t0 + 2), None),
                tick(3, "run_finished", None, None),
            ],
            "run-1",
        )
        .unwrap_or_else(|| unreachable!("non-empty"));
        assert_eq!(g.item.tone, Tone::Hollow);
        assert!(g.item.forks.is_empty() && g.item.rows.iter().all(|r| r.col == 0));
        let labels: Vec<&str> = g.item.rows.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "run started",
                "grep ← x y",
                "program attempt",
                "run_finished"
            ]
        );
        assert_eq!(g.item.rows[3].at, t0 + 2);
        assert_eq!(g.item.rows[0].id, "run-1:0");
    }

    #[test]
    fn program_events_name_their_iteration() {
        let mut t = tick(1, "program", Some(0), None);
        t.iteration = Some(3);
        let g = run_trajectory(&[t], "r").unwrap_or_else(|| unreachable!("non-empty"));
        assert_eq!(g.item.rows[0].label, "program attempt 3");
        assert_eq!(
            g.item.rows[0].kind,
            super::super::RowKind::Event(EventKind::RunThought)
        );
    }

    #[rstest]
    #[case("run_started", EventKind::RunStarted)]
    #[case("tool_call", EventKind::RunTool)]
    #[case("tool_result", EventKind::RunTool)]
    #[case("reasoning", EventKind::RunReasoning)]
    #[case("model_request", EventKind::RunModel)]
    #[case("model_response", EventKind::RunModel)]
    #[case("run_finished", EventKind::RunFinished)]
    #[case("run_failed", EventKind::RunFailed)]
    #[case("run_cancelled", EventKind::RunCancelled)]
    #[case("program", EventKind::RunThought)]
    #[case("anything", EventKind::RunThought)]
    fn run_kinds(#[case] kind: &str, #[case] want: EventKind) {
        assert_eq!(run_event_kind(kind), want);
    }

    #[test]
    fn failed_and_cancelled_runs_do_not_draw_green() {
        assert_eq!(EventKind::RunFailed.tone(), Tone::Red);
        assert_eq!(EventKind::RunCancelled.tone(), Tone::Yellow);
        assert_eq!(EventKind::RunFinished.tone(), Tone::Green);
    }

    #[test]
    fn missing_timestamps_fall_back_to_the_previous_event_not_a_clock() {
        let g = run_trajectory(
            &[
                tick(0, "run_started", None, None),
                tick(1, "run_finished", None, None),
            ],
            "r",
        )
        .unwrap_or_else(|| unreachable!("non-empty"));
        assert_eq!(g.item.rows.iter().map(|r| r.at).collect::<Vec<_>>(), [0, 0]);
    }

    #[test]
    fn describe_words() {
        assert_eq!(describe_tool_call("grep", "foo"), "grep ← foo");
        assert_eq!(describe_tool_result("grep", None, None), "grep → result");
        assert_eq!(
            describe_tool_result("grep", Some("ok"), Some("")),
            "grep → ok"
        );
        assert_eq!(
            describe_tool_result("grep", Some("error"), Some("boom")),
            "grep → error — boom"
        );
        let long = "x".repeat(200);
        let clipped = describe_tool_call("t", &long);
        assert_eq!(clipped.chars().count(), "t ← ".chars().count() + 160 + 1);
        assert!(clipped.ends_with('…'));
        assert_eq!(clip("abc", 3), "abc");
    }

    proptest! {
        #[test]
        fn spark_bars_are_monotone_in_value(values in proptest::collection::vec(proptest::option::of(0u64..1000), 1..30)) {
            let s = spark(&values, 24);
            prop_assert_eq!(s.bars.len(), values.len());
            for (a, va) in s.bars.iter().zip(&values) {
                for (b, vb) in s.bars.iter().zip(&values) {
                    if let (Some(x), Some(y)) = (va, vb) {
                        if x <= y { prop_assert!(a.glyph <= b.glyph); }
                    }
                }
            }
        }

        #[test]
        fn tokens_per_minute_conserves_in_window_tokens(ats in proptest::collection::vec(0i64..(30 * MICROS_PER_MINUTE), 1..20), tokens in proptest::collection::vec(0u64..100, 20)) {
            let events: Vec<TokenTick> = ats.iter().zip(&tokens).map(|(at, t)| TokenTick { at: *at, tokens: *t }).collect();
            let series = tokens_per_minute(&events, 24);
            prop_assert_eq!(series.len(), 24);
            let last = ats.iter().map(|a| a / MICROS_PER_MINUTE).max().unwrap_or(0);
            let want: u64 = events.iter().filter(|e| e.at / MICROS_PER_MINUTE > last - 24).map(|e| e.tokens).sum();
            prop_assert_eq!(series.iter().flatten().sum::<u64>(), want);
        }

        #[test]
        fn trace_rows_hide_exactly_the_collapsed_subtrees(collapse_b in any::<bool>(), collapse_root in any::<bool>()) {
            let mut collapsed = HashSet::new();
            if collapse_b { collapsed.insert("b".to_owned()); }
            if collapse_root { collapsed.insert("root".to_owned()); }
            let n = trace_rows(&sample_trace(), &collapsed).len();
            let want = if collapse_root { 1 } else if collapse_b { 3 } else { 4 };
            prop_assert_eq!(n, want);
        }
    }
}
