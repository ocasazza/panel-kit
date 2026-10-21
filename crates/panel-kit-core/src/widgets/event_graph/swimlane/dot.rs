//! Dots: one event, or a burst of events merged into one cluster glyph.

use serde::Serialize;

use super::super::model::{Event, EventKind, Micros};
use super::super::query::Tone;
use super::super::time;
use super::metrics::{clearance, CLUSTER_R, DOT_MERGE_PX, DOT_R};

/// One event, or a cluster of events closer than 6px, on one row.
///
/// A cluster keeps the first member's `id`, `at`, `kind`, `agent` and `x`, the
/// most severe tone among its members, and every member's title.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Dot {
    /// Id of the event, or of the first event of a cluster.
    pub id: String,
    /// Instant of the event, or of the first event of a cluster.
    pub at: Micros,
    /// Kind of the event, or of the first event of a cluster.
    pub kind: EventKind,
    /// Agent of the event, or of the first event of a cluster.
    pub agent: Option<String>,
    /// Colour class; the most severe member's for a cluster.
    pub tone: Tone,
    /// Tooltip: `Oct 1 00:10 · note · omp · said`; a cluster's joins its
    /// members' with a newline.
    pub title: String,
    /// Events merged; 2 or more is a cluster glyph.
    pub count: u32,
    /// Pixel of the event, or of the first event of a cluster.
    pub x: f64,
}

impl Dot {
    /// The dot of one event at pixel `x`.
    pub fn of(event: &Event, x: f64) -> Dot {
        Dot {
            id: event.id.clone(),
            at: event.at,
            kind: event.kind,
            agent: event.agent.clone(),
            tone: event.tone(),
            title: title(event),
            count: 1,
            x,
        }
    }

    /// Whether the dot is a cluster of two or more events.
    pub fn is_cluster(&self) -> bool {
        self.count >= 2
    }

    /// Radius of the glyph: 3px for one event, 5px for a cluster.
    pub fn radius(&self) -> f64 {
        if self.is_cluster() {
            CLUSTER_R
        } else {
            DOT_R
        }
    }

    /// Distance from the dot's centre to where text may start.
    pub fn clearance(&self) -> f64 {
        clearance(self.count)
    }

    /// Tooltip of the cluster glyph: `{count} events` over every title.
    pub fn cluster_title(&self) -> String {
        format!("{} events\n{}", self.count, self.title)
    }

    fn absorb(&mut self, event: &Event) {
        self.tone = self.tone.merge(event.tone());
        self.title.push('\n');
        push_title(&mut self.title, event);
        self.count += 1;
    }
}

/// Tooltip of one event: stamp, kind, agent and summary, empty parts dropped,
/// joined with ` · `.
pub fn title(event: &Event) -> String {
    let mut out = String::with_capacity(32 + event.summary.as_ref().map_or(0, String::len));
    push_title(&mut out, event);
    out
}

fn push_title(out: &mut String, event: &Event) {
    time::push_title_stamp(out, event.at);
    out.push_str(" · ");
    event.kind.push_label(out);
    for part in [event.agent.as_deref(), event.summary.as_deref()]
        .into_iter()
        .flatten()
    {
        if !part.is_empty() {
            out.push_str(" · ");
            out.push_str(part);
        }
    }
}

/// One-line caption: the summary with runs of whitespace collapsed to a
/// space and trimmed; the kind label when there is none.
pub fn caption(event: &Event) -> String {
    let Some(summary) = event.summary.as_deref() else {
        return event.kind.label();
    };
    let mut line = String::with_capacity(summary.len());
    let mut in_run = false;
    for c in summary.chars() {
        if matches!(c, ' ' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\r') {
            if !in_run {
                line.push(' ');
            }
            in_run = true;
        } else {
            line.push(c);
            in_run = false;
        }
    }
    match line.trim() {
        "" => event.kind.label(),
        text => text.to_string(),
    }
}

/// Dots of `events` at their pixels, bursts merged.
///
/// Sorted by pixel, ties by time, then input order; then folded left to
/// right: a dot closer than 6px to the most recent output dot joins it, so the
/// anchor is the first member's pixel and never slides. Events already in
/// time order with a monotone axis need no sort.
pub fn cluster<'a, I>(events: I) -> Vec<Dot>
where
    I: IntoIterator<Item = (&'a Event, f64)>,
{
    let order = |a: &(&Event, f64), b: &(&Event, f64)| {
        a.1.total_cmp(&b.1).then_with(|| a.0.at.cmp(&b.0.at))
    };
    let mut pairs: Vec<(&Event, f64)> = events.into_iter().collect();
    if !pairs.is_sorted_by(|a, b| order(a, b).is_le()) {
        pairs.sort_by(order);
    }
    let mut dots: Vec<Dot> = Vec::new();
    for (event, x) in pairs {
        match dots.last_mut() {
            Some(last) if x - last.x < DOT_MERGE_PX => last.absorb(event),
            _ => dots.push(Dot::of(event, x)),
        }
    }
    dots
}

/// [`cluster`] over dots already built: the same order and fold. Titles are
/// concatenated, tones merged, counts added.
pub fn merge(mut dots: Vec<Dot>) -> Vec<Dot> {
    dots.sort_by(|a, b| a.x.total_cmp(&b.x).then_with(|| a.at.cmp(&b.at)));
    let mut out: Vec<Dot> = Vec::with_capacity(dots.len());
    for dot in dots {
        match out.last_mut() {
            Some(last) if dot.x - last.x < DOT_MERGE_PX => {
                last.tone = last.tone.merge(dot.tone);
                last.title.push('\n');
                last.title.push_str(&dot.title);
                last.count += dot.count;
            }
            _ => out.push(dot),
        }
    }
    out
}
