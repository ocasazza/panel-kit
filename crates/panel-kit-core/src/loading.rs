//! Renderer-neutral loading-state shape: one normalized snapshot per logical
//! data source, plus the aggregation math shared by every shell's global
//! progress surface.
//!
//! The model follows store-shaped state management (pinia-style): each async
//! resource owns one store with a small action vocabulary (`begin`, `update`,
//! `succeed`, `fail`), components subscribe to the store's snapshot, and a
//! registry of all stores feeds one global aggregate. The store machinery and
//! components live in the renderer shells (the Dioxus `panel-kit` crate);
//! this module is the pure data both sides agree on.
//!
//! Loading contract: a bar beats a spinner, and a determinate bar always
//! shows its percentage. `fraction: None` is the honest indeterminate state —
//! shells animate the bar and show no fabricated number.

/// Lifecycle of one async data source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LoadStatus {
    /// Nothing requested yet.
    Idle,
    /// Work in flight. `Some(f)` carries completion in `0.0..=1.0`; `None`
    /// is indeterminate (source cannot measure progress).
    Pending {
        /// Completion fraction in `0.0..=1.0`, or `None` when indeterminate.
        fraction: Option<f64>,
    },
    /// Data available; the gated content can render.
    Ready,
    /// The load failed; `LoadSnapshot::error` carries the message.
    Failed,
}

/// Point-in-time state of one loading store. Renderers display exactly this;
/// apps mutate it only through the shell's store actions.
#[derive(Clone, Debug, PartialEq)]
pub struct LoadSnapshot {
    /// Lifecycle state.
    pub status: LoadStatus,
    /// Human label for the source, e.g. `"loading branches…"`.
    pub label: String,
    /// Current phase detail, e.g. the running stage name.
    pub detail: Option<String>,
    /// Failure message when `status == LoadStatus::Failed`.
    pub error: Option<String>,
}

impl LoadSnapshot {
    /// Fresh store: `Idle`, carrying only its label.
    pub fn idle(label: impl Into<String>) -> Self {
        Self {
            status: LoadStatus::Idle,
            label: label.into(),
            detail: None,
            error: None,
        }
    }

    /// Completion fraction when pending and determinate.
    pub fn fraction(&self) -> Option<f64> {
        match self.status {
            LoadStatus::Pending { fraction } => fraction,
            _ => None,
        }
    }

    /// True while work is in flight.
    pub fn is_pending(&self) -> bool {
        matches!(self.status, LoadStatus::Pending { .. })
    }

    /// True when data is available and gated content should render.
    pub fn is_ready(&self) -> bool {
        matches!(self.status, LoadStatus::Ready)
    }
}

/// Roll-up over every registered store, for the global (workspace-level)
/// progress surface.
#[derive(Clone, Debug, PartialEq)]
pub struct AggregateLoad {
    /// Stores currently pending.
    pub pending: usize,
    /// Aggregate completion in `0.0..=1.0`: the mean of the determinate
    /// pending fractions. `None` when no pending store reports a fraction —
    /// the global bar then renders indeterminate rather than inventing a
    /// number.
    pub fraction: Option<f64>,
    /// Label of one pending store to display (the first registered).
    pub label: Option<String>,
}

/// Aggregate the pending entries of a store registry. Returns `None` when
/// nothing is pending, so shells hide the global surface entirely.
pub fn aggregate_pending<'a>(
    snapshots: impl Iterator<Item = &'a LoadSnapshot>,
) -> Option<AggregateLoad> {
    let mut pending = 0usize;
    let mut measured = 0usize;
    let mut sum = 0.0f64;
    let mut label = None;
    for snap in snapshots {
        if let LoadStatus::Pending { fraction } = snap.status {
            pending += 1;
            if label.is_none() {
                label = Some(snap.label.clone());
            }
            if let Some(f) = fraction {
                measured += 1;
                sum += f.clamp(0.0, 1.0);
            }
        }
    }
    (pending > 0).then(|| AggregateLoad {
        pending,
        fraction: (measured > 0).then(|| sum / measured as f64),
        label,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pending(label: &str, fraction: Option<f64>) -> LoadSnapshot {
        LoadSnapshot {
            status: LoadStatus::Pending { fraction },
            label: label.into(),
            detail: None,
            error: None,
        }
    }

    #[test]
    fn aggregate_is_none_with_nothing_pending() {
        let idle = LoadSnapshot::idle("a");
        let ready = LoadSnapshot {
            status: LoadStatus::Ready,
            ..LoadSnapshot::idle("b")
        };
        assert_eq!(aggregate_pending([idle, ready].iter()), None);
    }

    #[test]
    fn aggregate_means_determinate_fractions() {
        let a = pending("a", Some(0.25));
        let b = pending("b", Some(0.75));
        let agg = aggregate_pending([a, b].iter()).unwrap();
        assert_eq!(agg.pending, 2);
        assert_eq!(agg.fraction, Some(0.5));
        assert_eq!(agg.label.as_deref(), Some("a"));
    }

    #[test]
    fn indeterminate_only_aggregate_reports_no_fraction() {
        let a = pending("a", None);
        let agg = aggregate_pending([a].iter()).unwrap();
        assert_eq!(agg.pending, 1);
        assert_eq!(agg.fraction, None);
    }

    #[test]
    fn out_of_range_fractions_are_clamped() {
        let a = pending("a", Some(1.7));
        let agg = aggregate_pending([a].iter()).unwrap();
        assert_eq!(agg.fraction, Some(1.0));
    }
}
