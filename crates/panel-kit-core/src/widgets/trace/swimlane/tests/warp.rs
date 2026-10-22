//! `warp_test.exs` and the axis behaviours it implies: span/gap layout, the
//! monotone `x`, tick spacing and labels, gap tooltips, and axis invariants.

use proptest::prelude::*;
use rstest::rstest;

use super::super::*;
use super::fixtures::*;
use crate::widgets::trace::model::Micros;
use crate::widgets::trace::time;

const WIDTH: f64 = 1400.0;
const HOUR_MS: i64 = 3_600_000;
const EPS: f64 = 1.0e-6;

fn scale(times: &[Micros], width: f64, params: &WarpParams) -> Warp {
    Warp::scale(times.iter().copied(), width, params)
}

fn plain(times: &[Micros]) -> Warp {
    scale(times, WIDTH, &WarpParams::default())
}

/// The instants `hours` after [`since`], one per entry.
fn hours(hs: &[f64]) -> Vec<Micros> {
    hs.iter().map(|h| at(*h)).collect()
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() <= EPS, "{a} != {b}");
}

fn kind(warp: &Warp, kind: TickKind) -> Vec<Tick> {
    warp.ticks
        .iter()
        .filter(|t| t.kind == kind)
        .cloned()
        .collect()
}

/// One event at each end of a week and fifty inside the hour starting at 3d.
fn week_with_one_busy_hour() -> Vec<Micros> {
    let mut times = vec![at(0.0)];
    times.extend((0..50).map(|n| at(72.0 + f64::from(n) / 50.0)));
    times.push(at(168.0));
    times
}

fn week_axis() -> Warp {
    let params = WarpParams {
        since: Some(at(0.0)),
        now: Some(at(168.0)),
        ..WarpParams::default()
    };
    scale(&week_with_one_busy_hour(), WIDTH, &params)
}

/// No two ticks of one kind (one label row) are closer than 72px.
fn assert_spaced(ticks: &[Tick]) {
    for k in [TickKind::Time, TickKind::Day] {
        let mut xs: Vec<f64> = ticks.iter().filter(|t| t.kind == k).map(|t| t.x).collect();
        xs.sort_by(f64::total_cmp);
        for pair in xs.windows(2) {
            assert!(
                pair[1] - pair[0] >= 72.0 - EPS,
                "{k:?} ticks {} and {} are closer than 72px",
                pair[0],
                pair[1]
            );
        }
    }
}

/// `^[A-Z][a-z]{2} \d+ [A-Z][a-z]{2}$`, checked by hand.
fn is_day_label(label: &str) -> bool {
    let word = |w: &str| {
        let mut chars = w.chars();
        w.chars().count() == 3
            && chars.next().is_some_and(|c| c.is_ascii_uppercase())
            && chars.all(|c| c.is_ascii_lowercase())
    };
    let parts: Vec<&str> = label.split(' ').collect();
    parts.len() == 3
        && word(parts[0])
        && !parts[1].is_empty()
        && parts[1].chars().all(|c| c.is_ascii_digit())
        && word(parts[2])
}

/// Width of the busy span before `gaps[k]`: from the previous gap (or `left`).
fn span_before_gap(warp: &Warp, k: usize, left: f64) -> f64 {
    let start = if k == 0 { left } else { warp.gaps[k - 1].x1 };
    warp.gaps[k].x0 - start
}

// ---- scale/3 ----

#[test]
fn a_seven_day_range_with_one_busy_hour_gives_that_hour_at_least_60_percent_of_the_width() {
    let axis = week_axis();
    let busy = axis.x(at(73.0)) - axis.x(at(72.0));

    assert!(busy >= 0.6 * WIDTH, "busy hour is {busy}px");
}

#[test]
fn linear_time_would_have_given_that_hour_under_1_percent_of_the_width() {
    let (busy_hours, week_hours) = (1.0_f64, 168.0_f64);
    let linear_px = busy_hours / week_hours * WIDTH;

    assert!(linear_px < 0.01 * WIDTH, "linear hour is {linear_px}px");
}

#[test]
fn an_idle_gap_longer_than_two_times_is_exactly_24px_however_long() {
    let axis = plain(&hours(&[0.0, 1.0, 5.0, 6.0]));

    assert_eq!(axis.gaps.len(), 1);
    near(axis.gaps[0].x1 - axis.gaps[0].x0, 24.0);
    assert_eq!(axis.gaps[0].ms, 4 * HOUR_MS);
    near(axis.x(at(5.0)) - axis.x(at(1.0)), 24.0);

    let far = plain(&hours(&[0.0, 1.0, 24.0 * 30.0, 24.0 * 30.0 + 1.0]));
    assert_eq!(far.gaps.len(), 1);
    near(far.gaps[0].x1 - far.gaps[0].x0, 24.0);
}

#[test]
fn a_gap_of_two_times_or_less_is_not_a_gap() {
    assert!(plain(&hours(&[0.0, 2.0, 4.0])).gaps.is_empty());
}

#[test]
fn x_is_monotonic_and_spans_the_reserved_edges() {
    let params = WarpParams {
        left: 50.0,
        right: 60.0,
        ..WarpParams::default()
    };
    let axis = scale(&week_with_one_busy_hour(), WIDTH, &params);
    let xs: Vec<f64> = [0.0, 10.0, 72.0, 72.5, 73.0, 100.0, 168.0]
        .iter()
        .map(|h| axis.x(at(*h)))
        .collect();

    assert!(xs.windows(2).all(|w| w[0] <= w[1]), "{xs:?}");
    near(xs[0], 50.0);
    near(xs[xs.len() - 1], WIDTH - 60.0);
}

#[test]
fn inside_a_busy_span_time_is_linear() {
    let axis = plain(&hours(&[0.0, 1.0, 2.0, 3.0]));
    let d1 = axis.x(at(1.0)) - axis.x(at(0.0));
    let d2 = axis.x(at(2.0)) - axis.x(at(1.0));

    near(d1, d2);
}

#[test]
fn a_window_bound_before_the_first_event_shows_as_a_gap() {
    let params = WarpParams {
        since: Some(at(0.0)),
        ..WarpParams::default()
    };
    let axis = scale(&hours(&[100.0, 101.0]), WIDTH, &params);

    assert_eq!(axis.gaps.len(), 1);
    assert_eq!(axis.gaps[0].ms, 100 * HOUR_MS);
}

#[test]
fn no_events_no_ticks_no_gaps() {
    let axis = plain(&[]);

    assert!(axis.ticks.is_empty());
    assert!(axis.gaps.is_empty());
    assert_eq!(axis.x(at(5.0)), 0.0);
}

#[test]
fn coincident_events_share_an_x() {
    let axis = plain(&hours(&[0.0, 0.0, 0.0]));

    assert_eq!(axis.x(at(0.0)), axis.x(at(0.0)));
}

// ---- ticks ----

#[test]
fn hour_ticks_appear_inside_the_busy_span_and_not_in_gaps() {
    let axis = week_axis();
    let times = kind(&axis, TickKind::Time);

    assert!(!times.is_empty());
    let (busy0, busy1) = (axis.x(at(72.0)), axis.x(at(73.0)));
    assert!(times.iter().all(|t| t.x >= busy0 && t.x <= busy1));
}

#[test]
fn day_ticks_are_kept_across_the_whole_range() {
    let axis = week_axis();
    let days = kind(&axis, TickKind::Day);

    assert!(days.len() >= 2);
    assert!(days.iter().all(|d| is_day_label(&d.label)), "{days:?}");
}

#[test]
fn ticks_of_one_kind_are_never_closer_than_72px() {
    assert_spaced(&week_axis().ticks);
}

#[test]
fn a_300px_span_over_two_hours_has_at_most_4_ticks() {
    let start = since() + (8 * 3600 + 7 * 60) * SEC;
    let axis = scale(&[start, start + 2 * HOUR], 300.0, &WarpParams::default());

    assert!(!axis.ticks.is_empty());
    assert!(axis.ticks.len() <= 4, "{:?}", axis.ticks);
    assert_spaced(&axis.ticks);
}

#[test]
fn labels_stay_72px_apart_for_a_table_of_widths_and_durations() {
    let mut cases = 0;
    for width in [200.0, 300.0, 600.0, 1400.0, 2400.0] {
        for minutes in [5i64, 20, 60, 119, 600, 2000, 10_000] {
            for start_min in [0i64, 7, 55] {
                let start = since() + (start_min * 60 + 3600) * SEC;
                let stop = start + minutes * 60 * SEC;
                let mid = start + (minutes / 2) * 60 * SEC;
                let axis = scale(&[start, mid, stop], width, &WarpParams::default());

                assert_spaced(&axis.ticks);
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 105);
}

#[test]
fn two_busy_spans_keep_their_labels_72px_apart_across_the_gap() {
    let axis = scale(
        &hours(&[8.0, 9.5, 11.0, 14.0, 15.5, 17.0]),
        400.0,
        &WarpParams::default(),
    );

    assert_eq!(axis.gaps.len(), 1);
    assert_spaced(&axis.ticks);
}

#[test]
fn a_day_boundary_is_a_day_tick_not_a_time_tick() {
    let axis = plain(&hours(&[22.0, 23.0, 24.0, 25.0, 26.0]));
    let days = kind(&axis, TickKind::Day);

    assert_eq!(days.len(), 1);
    assert_eq!(days[0].label, "Fri 2 Oct");
    near(days[0].x, axis.x(at(24.0)));
    let times = kind(&axis, TickKind::Time);
    assert!(!times.iter().any(|t| t.label == "00:00"));
    assert!(!times.is_empty());
}

#[test]
fn day_labels_across_a_month_boundary_name_the_month_and_never_run_backwards() {
    let sep = time::from_utc(2026, 9, 8, 8, 0, 0);
    let oct = time::from_utc(2026, 10, 4, 9, 0, 0);
    let last = time::from_utc(2026, 10, 6, 10, 0, 0);
    let params = WarpParams {
        now: Some(last),
        ..WarpParams::default()
    };
    let axis = scale(&[sep, sep + 1800 * SEC, oct, last], WIDTH, &params);
    let labels: Vec<String> = kind(&axis, TickKind::Day)
        .into_iter()
        .map(|t| t.label)
        .collect();
    let months: Vec<&str> = labels
        .iter()
        .filter_map(|l| l.split(' ').next_back())
        .collect();
    let index = |m: &str| ["Sep", "Oct"].iter().position(|x| *x == m);

    assert!(
        months.contains(&"Sep") && months.contains(&"Oct"),
        "{labels:?}"
    );
    assert!(
        months.windows(2).all(|w| index(w[0]) <= index(w[1])),
        "{labels:?}"
    );
    assert!(labels.last().is_some_and(|l| l.ends_with("Oct")));
}

#[test]
fn a_one_minute_step_is_used_when_the_span_is_wide_enough() {
    let axis = plain(&hours(&[8.0, 8.1]));
    let labels: Vec<String> = kind(&axis, TickKind::Time)
        .into_iter()
        .map(|t| t.label)
        .collect();

    assert!(
        labels.iter().any(|l| l == "08:01") && labels.iter().any(|l| l == "08:02"),
        "{labels:?}"
    );
}

// ---- implied behaviours ----

#[test]
fn bare_bounds_carry_zero_weight_and_only_add_the_following_gap() {
    let params = WarpParams {
        since: Some(at(0.0)),
        left: 10.0,
        right: 20.0,
        ..WarpParams::default()
    };
    let axis = scale(&hours(&[100.0, 101.0]), WIDTH, &params);

    near(axis.x(at(0.0)), 10.0);
    assert_eq!(axis.gaps.len(), 1);
    near(axis.gaps[0].x0, 10.0);
    near(axis.gaps[0].x1 - axis.gaps[0].x0, 24.0);
    near(axis.x(at(100.0)), 34.0);
    near(axis.x(at(101.0)), WIDTH - 20.0);
}

#[test]
fn a_bare_now_bound_adds_a_trailing_gap_of_24px() {
    let params = WarpParams {
        now: Some(at(100.0)),
        left: 10.0,
        right: 20.0,
        ..WarpParams::default()
    };
    let axis = scale(&hours(&[0.0, 1.0]), WIDTH, &params);

    assert_eq!(axis.gaps.len(), 1);
    near(axis.gaps[0].x0, axis.x(at(1.0)));
    near(axis.gaps[0].x1 - axis.gaps[0].x0, 24.0);
    near(axis.x(at(100.0)), axis.gaps[0].x1);
    near(axis.x(at(1.0)), WIDTH - 20.0 - 24.0);
}

#[test]
fn both_bounds_far_from_the_events_add_one_gap_each() {
    let params = WarpParams {
        since: Some(at(-50.0)),
        now: Some(at(100.0)),
        ..WarpParams::default()
    };
    let axis = scale(&hours(&[0.0, 1.0]), WIDTH, &params);

    assert_eq!(axis.gaps.len(), 2);
    assert!(axis.gaps.iter().all(|g| (g.x1 - g.x0 - 24.0).abs() <= EPS));
    near(axis.x(at(-50.0)), 0.0);
    near(axis.x(at(0.0)), 24.0);
    near(axis.x(at(1.0)), WIDTH - 24.0);
}

#[test]
fn a_span_is_never_narrower_than_10px_while_there_is_room() {
    // Five two-event spans of 1 minute against one span of 200 events over 300h.
    let mut times = Vec::new();
    for k in 0..5 {
        let base = at(f64::from(k) * 10.0);
        times.extend([base, base + 60 * SEC]);
    }
    times.extend((0..200).map(|n| at(50.0 + f64::from(n) * 1.5)));
    let params = WarpParams {
        left: 16.0,
        right: 8.0,
        ..WarpParams::default()
    };
    let axis = scale(&times, WIDTH, &params);

    assert_eq!(axis.gaps.len(), 5);
    for k in 0..5 {
        let w = span_before_gap(&axis, k, 16.0);
        assert!(w >= 10.0 - EPS, "tiny span {k} is {w}px");
    }
    near(axis.x(*times.last().unwrap()), WIDTH - 8.0);
}

#[test]
fn when_room_runs_out_spans_collapse_to_10px_pins_without_negative_widths() {
    let mut times = Vec::new();
    for k in 0..10 {
        let base = at(f64::from(k) * 5.0);
        times.extend([base, base + 60 * SEC]);
    }
    let axis = scale(&times, 60.0, &WarpParams::default());

    assert_eq!(axis.gaps.len(), 9);
    for k in 0..9 {
        near(span_before_gap(&axis, k, 0.0), 10.0);
        near(axis.gaps[k].x1 - axis.gaps[k].x0, 24.0);
    }
    let last = axis.x(*times.last().unwrap());
    near(last - axis.gaps[8].x1, 10.0);
    near(last, 10.0 * 10.0 + 9.0 * 24.0);
    let xs: Vec<f64> = times.iter().map(|t| axis.x(*t)).collect();
    assert!(xs.iter().all(|x| x.is_finite()) && xs.windows(2).all(|w| w[0] <= w[1]));
}

#[test]
fn at_a_span_gap_boundary_the_span_wins() {
    let axis = plain(&hours(&[0.0, 1.0, 5.0, 6.0]));
    let gap = &axis.gaps[0];

    assert_eq!(axis.x(at(1.0)), gap.x0);
    assert_eq!(axis.x(at(5.0)), gap.x1);
    near(axis.x(at(3.0)), (gap.x0 + gap.x1) / 2.0);
    assert!(axis.x(at(1.0) + SEC) > gap.x0);
}

#[test]
fn a_zero_duration_span_draws_every_event_at_its_left_edge() {
    let axis = plain(&hours(&[0.0, 10.0, 10.0, 20.0]));

    assert_eq!(axis.gaps.len(), 2);
    assert_eq!(axis.x(at(10.0)), axis.gaps[0].x1);
    assert!(axis.gaps[1].x0 - axis.gaps[0].x1 > 0.0);
    assert!(axis.x(at(10.0) + SEC) > axis.gaps[1].x0);

    let lone = plain(&hours(&[3.0, 3.0, 3.0]));
    assert_eq!(lone.x(at(3.0)), 0.0);
    assert!(lone.gaps.is_empty());
}

#[test]
fn x_clamps_to_the_first_and_last_instants() {
    let params = WarpParams {
        left: 30.0,
        right: 40.0,
        ..WarpParams::default()
    };
    let axis = scale(&hours(&[1.0, 2.0]), WIDTH, &params);

    for h in [-1000.0, -5.0, 0.0, 1.0] {
        assert_eq!(axis.x(at(h)), 30.0, "x({h}h)");
    }
    for h in [2.0, 3.0, 100.0, 100_000.0] {
        assert_eq!(axis.x(at(h)), WIDTH - 40.0, "x({h}h)");
    }
    assert_eq!(axis.x(i64::MIN), 30.0);
    assert_eq!(axis.x(i64::MAX), WIDTH - 40.0);
}

#[test]
fn bounds_extend_the_clamp_range() {
    let params = WarpParams {
        since: Some(at(-3.0)),
        now: Some(at(9.0)),
        left: 5.0,
        ..WarpParams::default()
    };
    let axis = scale(&hours(&[1.0, 2.0]), WIDTH, &params);

    assert_eq!(axis.x(at(-100.0)), 5.0);
    assert_eq!(axis.x(at(-3.0)), 5.0);
    assert_eq!(axis.x(at(9.0)), axis.x(at(900.0)));
}

#[test]
fn no_events_with_bounds_more_than_2h_apart_keeps_one_gap() {
    let params = WarpParams {
        since: Some(at(0.0)),
        now: Some(at(10.0)),
        left: 5.0,
        ..WarpParams::default()
    };
    let axis = scale(&[], WIDTH, &params);

    assert!(!axis.is_empty());
    assert_eq!(axis.gaps.len(), 1);
    assert_eq!(axis.gaps[0].ms, 10 * HOUR_MS);
    near(axis.gaps[0].x0, 5.0);
    near(axis.gaps[0].x1, 29.0);
    near(axis.x(at(0.0)), 5.0);
    near(axis.x(at(10.0)), 29.0);
}

#[test]
fn no_events_with_bounds_within_2h_keeps_a_zero_width_axis() {
    let params = WarpParams {
        since: Some(at(0.0)),
        now: Some(at(1.0)),
        left: 5.0,
        ..WarpParams::default()
    };
    let axis = scale(&[], WIDTH, &params);

    assert!(!axis.is_empty());
    assert!(axis.gaps.is_empty());
    assert_eq!(axis.x(at(0.0)), 5.0);
    assert_eq!(axis.x(at(1.0)), 5.0);
}

#[test]
fn no_events_and_no_bounds_maps_everything_to_left() {
    let params = WarpParams {
        left: 12.0,
        right: 7.0,
        ..WarpParams::default()
    };
    let axis = scale(&[], WIDTH, &params);

    assert!(axis.is_empty());
    assert!(axis.ticks.is_empty() && axis.gaps.is_empty());
    for t in [i64::MIN, at(-9.0), at(0.0), at(9.0), i64::MAX] {
        assert_eq!(axis.x(t), 12.0);
    }
}

#[test]
fn an_idle_of_exactly_2h_does_not_split_and_2h_plus_1ms_does() {
    assert_eq!(warp::IDLE_MS, 7_200_000);
    assert!(plain(&[at(0.0), at(2.0)]).gaps.is_empty());

    let axis = plain(&[at(0.0), at(2.0) + 1000]);
    assert_eq!(axis.gaps.len(), 1);
    assert_eq!(axis.gaps[0].ms, 7_200_001);
}

#[test]
fn times_are_floored_to_the_millisecond() {
    let axis = plain(&hours(&[0.0, 1.0]));
    let mid = at(0.5);

    assert_eq!(axis.x(mid + 100), axis.x(mid + 999));
    assert_eq!(axis.x(mid), axis.x(mid + 999));
    assert!(axis.x(mid + 1000) > axis.x(mid + 999));
    assert_eq!(
        plain(&[at(0.0) + 1, at(0.0) + 999, at(1.0)]),
        plain(&[at(0.0), at(0.0), at(1.0)]),
    );
}

#[rstest]
#[case::three_hours(3 * HOUR_MS, "3h")]
#[case::twenty_three_fifty_nine(23 * HOUR_MS + 59 * 60_000, "23h")]
#[case::one_day(24 * HOUR_MS, "1d")]
#[case::hundred_hours(100 * HOUR_MS, "4d")]
#[case::thirty_days(30 * 24 * HOUR_MS, "30d")]
fn gap_idle_label_and_title(#[case] ms: i64, #[case] label: &str) {
    let gap = Gap {
        x0: 0.0,
        x1: 24.0,
        ms,
    };

    assert_eq!(gap.idle_label(), label);
    assert_eq!(gap.title(), format!("{label} with nothing happening"));
}

#[test]
fn time_ticks_skip_midnights_and_stay_36px_from_a_day_tick() {
    let axis = plain(&hours(&[22.0, 23.0, 24.0, 25.0, 26.0]));
    let days = kind(&axis, TickKind::Day);

    assert_eq!(days.len(), 1);
    assert_eq!(days[0].label, "Fri 2 Oct");
    near(days[0].x, axis.x(at(24.0)));
    let times = kind(&axis, TickKind::Time);
    assert!(!times.is_empty());
    assert!(times.iter().all(|t| t.label != "00:00"));
    assert!(
        times.iter().all(|t| (t.x - days[0].x).abs() >= 36.0),
        "{times:?}"
    );
}

#[test]
fn day_labels_across_a_month_boundary_read_in_calendar_order() {
    let start = time::from_utc(2026, 9, 29, 0, 0, 0);
    let times: Vec<Micros> = (0..=54).map(|k| start + k * 2 * HOUR).collect();
    let axis = plain(&times);
    let labels: Vec<String> = kind(&axis, TickKind::Day)
        .into_iter()
        .map(|t| t.label)
        .collect();

    assert_eq!(
        labels,
        [
            "Tue 29 Sep",
            "Wed 30 Sep",
            "Thu 1 Oct",
            "Fri 2 Oct",
            "Sat 3 Oct"
        ]
    );
}

// ---- properties ----

#[derive(Clone, Debug)]
struct Input {
    times: Vec<Micros>,
    width: f64,
    left: f64,
    right: f64,
    since: Option<Micros>,
    now: Option<Micros>,
}

impl Input {
    fn params(&self) -> WarpParams {
        WarpParams {
            since: self.since,
            now: self.now,
            left: self.left,
            right: self.right,
            ..WarpParams::default()
        }
    }

    fn warp(&self) -> Warp {
        scale(&self.times, self.width, &self.params())
    }

    /// Every instant the axis is built from, sorted.
    fn points(&self) -> Vec<Micros> {
        let mut points = self.times.clone();
        points.extend(self.since);
        points.extend(self.now);
        points.sort_unstable();
        points
    }
}

fn arb_input() -> impl Strategy<Value = Input> {
    let secs = 0i64..90 * 86_400;
    (
        prop::collection::vec(secs.clone(), 0..=60),
        100u32..3000,
        0u32..80,
        0u32..80,
        prop::option::of(secs.clone()),
        prop::option::of(secs),
    )
        .prop_map(|(times, width, left, right, since_s, now_s)| Input {
            times: times.into_iter().map(|s| since() + s * SEC).collect(),
            width: f64::from(width),
            left: f64::from(left),
            right: f64::from(right),
            since: since_s.map(|s| since() + s * SEC),
            now: now_s.map(|s| since() + s * SEC),
        })
}

proptest! {
    #[test]
    fn x_is_non_decreasing_over_a_sorted_probe_set(input in arb_input()) {
        let warp = input.warp();
        let points = input.points();
        let mut probes = points.clone();
        probes.extend(points.windows(2).map(|w| w[0] + (w[1] - w[0]) / 2));
        probes.extend(points.first().map(|t| t - HOUR));
        probes.extend(points.last().map(|t| t + HOUR));
        probes.sort_unstable();

        let xs: Vec<f64> = probes.iter().map(|t| warp.x(*t)).collect();
        for pair in xs.windows(2) {
            prop_assert!(pair[0] <= pair[1] + EPS, "{} then {}", pair[0], pair[1]);
        }
        prop_assert!(xs.iter().all(|x| *x >= input.left - EPS));
    }

    #[test]
    fn the_first_point_is_at_left_and_the_last_at_the_right_edge_when_there_is_room(input in arb_input()) {
        let warp = input.warp();
        let points = input.points();
        let (Some(first), Some(last)) = (points.first(), points.last()) else {
            prop_assert!(warp.is_empty());
            return Ok(());
        };
        prop_assert!((warp.x(*first) - input.left).abs() <= EPS);

        let spans = warp.gaps.len() as f64 + 1.0;
        let room = input.width - input.left - input.right - 24.0 * warp.gaps.len() as f64;
        // A single zero-duration span draws its instant at its left edge instead.
        if !input.times.is_empty() && room >= 10.0 * spans && first != last {
            let edge = input.width - input.right;
            prop_assert!((warp.x(*last) - edge).abs() <= EPS, "{} vs {edge}", warp.x(*last));
            prop_assert!(warp.x(*last + 50 * HOUR) <= edge + EPS);
        }
    }

    #[test]
    fn every_gap_is_24px_and_gaps_run_left_to_right_without_overlap(input in arb_input()) {
        let warp = input.warp();

        for gap in &warp.gaps {
            prop_assert!((gap.x1 - gap.x0 - 24.0).abs() <= EPS, "{gap:?}");
            prop_assert!(gap.ms > warp::IDLE_MS);
        }
        for pair in warp.gaps.windows(2) {
            prop_assert!(pair[1].x0 >= pair[0].x1 - EPS, "{pair:?}");
        }
    }

    #[test]
    fn ticks_of_each_kind_are_72px_apart_and_inside_the_axis(input in arb_input()) {
        let warp = input.warp();

        for k in [TickKind::Time, TickKind::Day] {
            let mut xs: Vec<f64> = warp.ticks.iter().filter(|t| t.kind == k).map(|t| t.x).collect();
            xs.sort_by(f64::total_cmp);
            for pair in xs.windows(2) {
                prop_assert!(pair[1] - pair[0] >= 72.0 - EPS, "{k:?}: {} then {}", pair[0], pair[1]);
            }
        }
        match input.points().last() {
            None => prop_assert!(warp.ticks.is_empty()),
            Some(last) => {
                let end = warp.x(*last);
                for tick in &warp.ticks {
                    prop_assert!(tick.x >= input.left - EPS && tick.x <= end + EPS, "{tick:?} outside {}..{end}", input.left);
                }
            }
        }
    }

    #[test]
    fn the_same_input_twice_gives_the_same_axis(input in arb_input()) {
        let (a, b) = (input.warp(), input.warp());

        prop_assert_eq!(&a, &b);
        prop_assert_eq!(&a.ticks, &b.ticks);
        prop_assert_eq!(&a.gaps, &b.gaps);
        for t in input.points() {
            prop_assert_eq!(a.x(t), b.x(t));
        }
    }

    #[test]
    fn permuting_the_input_times_gives_the_same_axis(
        (input, shuffled) in arb_input().prop_flat_map(|input| {
            let times = Just(input.times.clone()).prop_shuffle();
            (Just(input), times)
        })
    ) {
        let a = input.warp();
        let b = scale(&shuffled, input.width, &input.params());

        prop_assert_eq!(a, b);
    }
}
