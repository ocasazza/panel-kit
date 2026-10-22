//! A density-warped time axis for bursty events.
//!
//! Linear time spends the width on days nothing happened. The warp is a
//! piecewise-linear map from time to pixels built from the events themselves:
//!
//! - Events no more than `idle_ms` (2h) apart form a busy span.
//! - A longer idle stretch collapses to a fixed `gap_px` (24px) gap.
//! - The width left is shared between busy spans in proportion to event count
//!   times duration, with a floor on both and a minimum width per span.
//!
//! Inside a span time is linear, so its [`TickKind::Time`] ticks sit at the
//! finest step whose labels stay 72px apart. [`TickKind::Day`] ticks (UTC
//! midnights) are kept everywhere, gaps included; the view draws the two kinds
//! on separate label rows, so each kind only competes with itself.
//!
//! `since` and `now` add zero-weight points: a window that starts long before
//! its first event shows the idle stretch as a gap rather than as nothing.
//!
//! Pure: the same times, bounds and width always give the same axis.

use serde::Deserialize;
use serde::Serialize;

use super::super::model::Micros;
use super::super::time;

/// Idle stretch that ends a busy span: 2h, inclusive.
pub const IDLE_MS: i64 = 7_200_000;
/// Width a collapsed idle stretch is drawn at.
pub const GAP_PX: f64 = 24.0;
/// Least width of a span that holds events, while there is room for it.
pub const MIN_SPAN_PX: f64 = 10.0;
/// Duration floor of a span's weight: a span of one instant still counts.
pub const MIN_WEIGHT_MS: i64 = 600_000;
/// Least distance between two ticks of one kind.
pub const MIN_TICK_PX: f64 = 72.0;
/// Time ticks closer than this to a day tick are dropped.
pub const LABEL_PX: f64 = 36.0;
/// Tick steps in minutes, finest first.
pub const STEPS_MIN: [i64; 9] = [1, 5, 15, 30, 60, 180, 360, 720, 1440];

const MINUTE_MS: i64 = 60_000;
const DAY_MS: i64 = 86_400_000;

/// An idle stretch drawn hatched, always `gap_px` wide.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gap {
    /// Left edge.
    pub x0: f64,
    /// Right edge.
    pub x1: f64,
    /// How long nothing happened, in milliseconds.
    pub ms: i64,
}

impl Gap {
    /// `Nd` from a day on, else `Nh`: the tooltip's idle duration.
    pub fn idle_label(&self) -> String {
        if self.ms >= DAY_MS {
            format!("{}d", self.ms / DAY_MS)
        } else {
            format!("{}h", self.ms / 3_600_000)
        }
    }

    /// Tooltip text: `{idle_label} with nothing happening`.
    pub fn title(&self) -> String {
        format!("{} with nothing happening", self.idle_label())
    }
}

/// Which label row a tick belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TickKind {
    /// A UTC midnight, labelled `Fri 2 Oct`.
    Day,
    /// A time of day inside a busy span, labelled `08:01`.
    Time,
}

impl TickKind {
    /// Snake-case name; the `ag-tick-{kind}` class suffix.
    pub fn as_str(self) -> &'static str {
        match self {
            TickKind::Day => "day",
            TickKind::Time => "time",
        }
    }
}

/// A grid line and its label.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tick {
    /// Position of the line.
    pub x: f64,
    /// Label text, UTC.
    pub label: String,
    /// Label row.
    pub kind: TickKind,
}

/// Bounds, reserved margins and thresholds of one axis.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Params {
    /// Window start as a zero-weight point.
    pub since: Option<Micros>,
    /// Window end as a zero-weight point.
    pub now: Option<Micros>,
    /// Pixels reserved at the left edge.
    pub left: f64,
    /// Pixels reserved at the right edge.
    pub right: f64,
    /// Idle stretch, in milliseconds, that ends a busy span.
    pub idle_ms: i64,
    /// Width of a collapsed idle stretch.
    pub gap_px: f64,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            since: None,
            now: None,
            left: 0.0,
            right: 0.0,
            idle_ms: IDLE_MS,
            gap_px: GAP_PX,
        }
    }
}

/// A run of points no more than `idle` apart.
#[derive(Clone, Copy, Debug)]
struct Span {
    t0: i64,
    t1: i64,
    n: u32,
}

/// One stretch of the axis, a busy span or the gap after it.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Segment {
    gap: bool,
    t0: i64,
    t1: i64,
    n: u32,
    px0: f64,
    px1: f64,
}

/// The axis: time to pixels, plus the ticks and gaps drawn on it.
#[derive(Clone, Debug, PartialEq)]
pub struct Warp {
    segments: Vec<Segment>,
    left: f64,
    /// Grid lines, sorted by x; days before times at equal x.
    pub ticks: Vec<Tick>,
    /// Idle stretches, left to right.
    pub gaps: Vec<Gap>,
}

impl Warp {
    /// The axis for events at `times` across `width` pixels. With no times and
    /// no bounds every instant maps to `params.left`.
    pub fn scale<I>(times: I, width: f64, params: &Params) -> Warp
    where
        I: IntoIterator<Item = Micros>,
    {
        let times = times.into_iter();
        let mut ms: Vec<i64> = Vec::with_capacity(times.size_hint().0);
        ms.extend(times.map(time::to_ms));
        // Events mostly arrive oldest first; only a shuffled input pays a sort.
        if !ms.is_sorted() {
            ms.sort_unstable();
        }
        let mut bounds: Vec<i64> = [params.since, params.now]
            .into_iter()
            .flatten()
            .map(time::to_ms)
            .collect();
        bounds.sort_unstable();

        let spans = spans(&ms, &bounds, params.idle_ms);
        if spans.is_empty() {
            return Warp {
                segments: Vec::new(),
                left: params.left,
                ticks: Vec::new(),
                gaps: Vec::new(),
            };
        }
        let segments = lay_out(
            &spans,
            width - params.left - params.right,
            params.left,
            params.gap_px,
        );
        let gaps = segments
            .iter()
            .filter(|seg| seg.gap)
            .map(|seg| Gap {
                x0: seg.px0,
                x1: seg.px1,
                ms: seg.t1 - seg.t0,
            })
            .collect();
        let ticks = ticks(&segments);
        Warp {
            segments,
            left: params.left,
            ticks,
            gaps,
        }
    }

    /// Pixel of an instant: linear inside a span, clamped to the axis ends.
    pub fn x(&self, at: Micros) -> f64 {
        position(&self.segments, time::to_ms(at)).unwrap_or(self.left)
    }

    /// Whether the axis has no segment: no events and no bounds.
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }
}

/// Busy spans of the sorted `times`, each weighing one, and the sorted
/// `bounds`, weighing nothing. The order of points at one instant cannot
/// change a span, so the two lists merge in either order.
fn spans(times: &[i64], bounds: &[i64], idle: i64) -> Vec<Span> {
    let mut spans: Vec<Span> = Vec::new();
    let mut push = |t: i64, n: u32| match spans.last_mut() {
        Some(span) if t - span.t1 <= idle => {
            span.t1 = t;
            span.n += n;
        }
        _ => spans.push(Span { t0: t, t1: t, n }),
    };
    let (mut a, mut b) = (0, 0);
    while a < times.len() || b < bounds.len() {
        if b == bounds.len() || (a < times.len() && times[a] <= bounds[b]) {
            push(times[a], 1);
            a += 1;
        } else {
            push(bounds[b], 0);
            b += 1;
        }
    }
    spans
}

fn lay_out(spans: &[Span], avail: f64, left: f64, gap_px: f64) -> Vec<Segment> {
    let gaps = spans.len() - 1;
    let room = (avail - gaps as f64 * gap_px).max(1.0);
    let widths = share(spans, room);
    let mut segments = Vec::with_capacity(spans.len() * 2 - 1);
    let mut px = left;
    for (i, (span, w)) in spans.iter().zip(&widths).enumerate() {
        let next = px + w;
        segments.push(Segment {
            gap: false,
            t0: span.t0,
            t1: span.t1,
            n: span.n,
            px0: px,
            px1: next,
        });
        px = next;
        if let Some(after) = spans.get(i + 1) {
            segments.push(Segment {
                gap: true,
                t0: span.t1,
                t1: after.t0,
                n: 0,
                px0: next,
                px1: next + gap_px,
            });
            px = next + gap_px;
        }
    }
    segments
}

/// Each span's share of `room`: event count times duration, floored; a span
/// with events never narrower than [`MIN_SPAN_PX`] while there is width for it.
fn share(spans: &[Span], room: f64) -> Vec<f64> {
    let weights: Vec<i64> = spans
        .iter()
        .map(|span| {
            if span.n == 0 {
                0
            } else {
                i64::from(span.n).saturating_mul((span.t1 - span.t0).max(MIN_WEIGHT_MS))
            }
        })
        .collect();
    floor_widths(&weights, room)
}

fn floor_widths(weights: &[i64], room: f64) -> Vec<f64> {
    let mut pinned = vec![false; weights.len()];
    let mut pinned_count = 0usize;
    let mut widths = vec![0.0; weights.len()];
    loop {
        let free_weight = weights
            .iter()
            .zip(&pinned)
            .filter(|(_, pinned)| !**pinned)
            .fold(0i64, |sum, (w, _)| sum.saturating_add(*w));
        let free_room = room - pinned_count as f64 * MIN_SPAN_PX;
        for ((width, weight), pinned) in widths.iter_mut().zip(weights).zip(&pinned) {
            *width = if *pinned {
                MIN_SPAN_PX
            } else if free_weight == 0 {
                0.0
            } else {
                free_room.max(0.0) * *weight as f64 / free_weight as f64
            };
        }
        let before = pinned_count;
        for ((weight, width), pinned) in weights.iter().zip(&widths).zip(pinned.iter_mut()) {
            if *weight > 0 && *width < MIN_SPAN_PX && !*pinned {
                *pinned = true;
                pinned_count += 1;
            }
        }
        if pinned_count == before || free_room <= 0.0 {
            return widths;
        }
    }
}

fn position(segments: &[Segment], t: i64) -> Option<f64> {
    let (first, last) = (segments.first()?, segments.last()?);
    Some(if t <= first.t0 {
        first.px0
    } else if t >= last.t1 {
        last.px1
    } else {
        // First segment with `t <= seg.t1`: at a span/gap boundary the span wins.
        let i = segments.partition_point(|seg| seg.t1 < t);
        segments.get(i).map_or(last.px1, |seg| interpolate(seg, t))
    })
}

fn interpolate(seg: &Segment, t: i64) -> f64 {
    if seg.t0 == seg.t1 {
        seg.px0
    } else {
        seg.px0 + (t - seg.t0) as f64 / (seg.t1 - seg.t0) as f64 * (seg.px1 - seg.px0)
    }
}

/// [`position`] over one segment alone.
fn position_in(seg: &Segment, t: i64) -> f64 {
    if t <= seg.t0 {
        seg.px0
    } else if t >= seg.t1 {
        seg.px1
    } else {
        interpolate(seg, t)
    }
}

fn next_multiple(t: i64, step: i64) -> i64 {
    (t + step - 1) / step * step
}

fn ticks(segments: &[Segment]) -> Vec<Tick> {
    let (Some(first), Some(last)) = (segments.first(), segments.last()) else {
        return Vec::new();
    };
    let days = day_ticks(segments, first.t0, last.t1);
    let mut ticks = time_ticks(segments, &days);
    ticks.splice(0..0, days);
    ticks.sort_by(|a, b| a.x.total_cmp(&b.x));
    ticks
}

/// UTC midnights across the axis, kept only where 72px clear of the last kept.
fn day_ticks(segments: &[Segment], t_min: i64, t_max: i64) -> Vec<Tick> {
    let mut kept: Vec<Tick> = Vec::new();
    let mut midnight = next_multiple(t_min, DAY_MS);
    while midnight <= t_max {
        let x = position(segments, midnight).unwrap_or(0.0);
        if kept.last().is_none_or(|prev| x - prev.x >= MIN_TICK_PX) {
            kept.push(Tick {
                x,
                label: time::format_day_label(midnight),
                kind: TickKind::Day,
            });
        }
        midnight += DAY_MS;
    }
    kept
}

/// Time-of-day ticks of every busy span, minus those near a day tick, kept
/// only where 72px clear of the last kept.
fn time_ticks(segments: &[Segment], days: &[Tick]) -> Vec<Tick> {
    let mut kept: Vec<Tick> = Vec::new();
    for seg in segments
        .iter()
        .filter(|seg| !seg.gap && seg.n > 0 && seg.t1 != seg.t0)
    {
        let per_min = (seg.px1 - seg.px0) / ((seg.t1 - seg.t0) as f64 / MINUTE_MS as f64);
        let Some(step) = STEPS_MIN
            .into_iter()
            .find(|step| *step as f64 * per_min >= MIN_TICK_PX)
        else {
            continue;
        };
        let step_ms = step * MINUTE_MS;
        let mut m = next_multiple(seg.t0, step_ms);
        while m < seg.t1 {
            if m % DAY_MS != 0 {
                let x = position_in(seg, m);
                let near_day = near_day_tick(days, x);
                if !near_day && kept.last().is_none_or(|prev| x - prev.x >= MIN_TICK_PX) {
                    kept.push(Tick {
                        x,
                        label: time::format_clock_label(m),
                        kind: TickKind::Time,
                    });
                }
            }
            m += step_ms;
        }
    }
    kept
}

/// Whether a day tick lies within [`LABEL_PX`] of `x`; `days` is sorted by x.
fn near_day_tick(days: &[Tick], x: f64) -> bool {
    let from = days.partition_point(|day| day.x < x - LABEL_PX - 1.0);
    days[from..]
        .iter()
        .take_while(|day| day.x <= x + LABEL_PX + 1.0)
        .any(|day| (day.x - x).abs() < LABEL_PX)
}
