//! Build-progress plumbing for a still-indexing source.
//!
//! Ported from jump-cannon's `build_progress.rs`: when an alternate source is
//! applied (or the boot server is still building the default), the graph
//! routes answer with a status and stream granular stage/page events into
//! that source's own progress log. Two surfaces render that wait, so the
//! event fold, the elapsed formatter, and the display-line rule live here and
//! both reuse them. The wire format is the host's — map it onto
//! [`ProgressEvent`] and feed [`BuildFeed::fold`].

use std::collections::VecDeque;

/// One progress event, renderer-neutral. Hosts translate their wire format
/// (jump-cannon's `api::ProgressEvent`) onto this.
#[derive(Clone, Debug, PartialEq)]
pub enum ProgressEvent {
    /// A stage began within a group.
    Start {
        /// The stage group, e.g. `fetch`.
        group: String,
        /// The stage label, e.g. `page 1`.
        label: String,
    },
    /// The current stage relabelled itself.
    UpdateLabel {
        /// The new stage label.
        label: String,
    },
    /// Determinate fraction in `0.0..=1.0`.
    SetProgress {
        /// Completion in `0.0..=1.0`.
        progress: f32,
    },
    /// The build finished.
    Finish,
    /// The build failed.
    Fail {
        /// Why the build failed.
        reason: String,
    },
    /// A free-form log line.
    Log {
        /// The log message.
        message: String,
    },
}

/// Most recent progress lines kept per build. A short tail is enough: the
/// panels show a live feed, not a scrollback.
const MAX_LINES: usize = 8;

/// A compact, newest-last tail of one source's progress log. `since` is the
/// server sequence cursor for the next progress poll; `lines` is the folded,
/// display-ready event tail.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BuildFeed {
    /// Server sequence cursor for the next poll.
    pub since: u64,
    /// Folded, display-ready event tail (newest last).
    pub lines: VecDeque<String>,
}

impl BuildFeed {
    /// Fold one progress response into the feed: advance the cursor and append
    /// each event's display line, dropping the oldest past [`MAX_LINES`].
    pub fn fold<'a>(&mut self, next_seq: u64, events: impl IntoIterator<Item = &'a ProgressEvent>) {
        self.since = next_seq;
        for event in events {
            if let Some(line) = event_line(event) {
                self.lines.push_back(line);
                while self.lines.len() > MAX_LINES {
                    self.lines.pop_front();
                }
            }
        }
    }
}

/// One progress event's display line, or `None` for events that carry nothing
/// worth a row (blank labels/messages).
pub fn event_line(event: &ProgressEvent) -> Option<String> {
    let line = match event {
        ProgressEvent::Start { group, label } => format!("{group}: {label}"),
        ProgressEvent::UpdateLabel { label } => label.clone(),
        ProgressEvent::SetProgress { progress } => format!("{:.0}%", progress * 100.0),
        ProgressEvent::Finish => "done".to_string(),
        ProgressEvent::Fail { reason } => format!("failed: {reason}"),
        ProgressEvent::Log { message } => message.clone(),
    };
    let line = line.trim().to_string();
    (!line.is_empty()).then_some(line)
}

/// Format an elapsed-milliseconds reading as `m:ss`. Imports run for minutes
/// at the product's target scale, so minutes never wrap to hours.
pub fn fmt_elapsed(ms: Option<u64>) -> String {
    let secs = ms.unwrap_or(0) / 1000;
    format!("{}:{:02}", secs / 60, secs % 60)
}

#[cfg(test)]
mod tests {
    use super::{event_line, fmt_elapsed, BuildFeed, ProgressEvent};

    #[test]
    fn display_lines_cover_every_event_kind() {
        assert_eq!(event_line(&ProgressEvent::Start { group: "fetch".into(), label: "page 1".into() }), Some("fetch: page 1".into()));
        assert_eq!(event_line(&ProgressEvent::UpdateLabel { label: "indexing".into() }), Some("indexing".into()));
        assert_eq!(event_line(&ProgressEvent::SetProgress { progress: 0.42 }), Some("42%".into()));
        assert_eq!(event_line(&ProgressEvent::Finish), Some("done".into()));
        assert_eq!(event_line(&ProgressEvent::Fail { reason: "disk full".into() }), Some("failed: disk full".into()));
        assert_eq!(event_line(&ProgressEvent::Log { message: "  ".into() }), None);
    }

    #[test]
    fn fold_keeps_a_short_tail_and_advances_the_cursor() {
        let mut feed = BuildFeed::default();
        let events: Vec<ProgressEvent> = (0..12)
            .map(|i| ProgressEvent::UpdateLabel { label: format!("step {i}") })
            .collect();
        feed.fold(77, &events);
        assert_eq!(feed.since, 77);
        assert_eq!(feed.lines.len(), 8);
        assert_eq!(feed.lines.front().map(String::as_str), Some("step 4"));
        assert_eq!(feed.lines.back().map(String::as_str), Some("step 11"));
    }

    #[test]
    fn elapsed_never_wraps_to_hours() {
        assert_eq!(fmt_elapsed(None), "0:00");
        assert_eq!(fmt_elapsed(Some(59_999)), "0:59");
        assert_eq!(fmt_elapsed(Some(61_000)), "1:01");
    }
}
