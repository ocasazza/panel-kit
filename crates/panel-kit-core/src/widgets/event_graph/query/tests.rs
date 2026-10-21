//! Query grammar, window, legend and toolbar-state suites (`query_test.exs`,
//! `activity_view_test.exs`, `activity_live_test.exs`, legend block of
//! `graph_test.exs`), ported. Sequences are step tables run by one harness.

use proptest::prelude::*;
use rstest::rstest;

use super::*;

// -- fixtures -----------------------------------------------------------------

fn filters(agent: Option<&str>, item: Option<&str>, kind: Option<&str>) -> Filters {
    Filters {
        agent: agent.map(str::to_string),
        item: item.map(str::to_string),
        kind: kind.map(str::to_string),
    }
}

fn ev(agent: Option<&str>, item: Option<&str>, kind: &str) -> EventFields {
    EventFields {
        agent: agent.map(str::to_string),
        item_id: item.map(str::to_string),
        kind: kind.to_string(),
    }
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn day(y: i64, m: u32, d: u32) -> i64 {
    days_from_civil(y, m, d) * DAY
}

// -- query grammar (Q1..Q9 + extras) --------------------------------------------

#[test]
fn q1_reads_each_key_into_filters_and_window() {
    let q = parse("agent:worker item:imp-16 kind:commit range:7d");
    assert_eq!(
        q.filters,
        filters(Some("worker"), Some("imp-16"), Some("commit"))
    );
    assert_eq!(q.range.as_deref(), Some("7d"));
    assert!(q.errors.is_empty());
}

#[rstest]
#[case("3h")]
#[case("2w")]
#[case("2026-W40")]
#[case("2026-09-30")]
#[case("2026-09-01/2026-10-08")]
fn q2_range_carries_any_window_spelling(#[case] token: &str) {
    let q = parse(&format!("range:{token}"));
    assert_eq!(q.range.as_deref(), Some(token));
    assert!(q.errors.is_empty());
}

#[rstest]
#[case("range:5y")]
#[case("range:/2026-10-08")]
fn q3_unparseable_range_is_dropped_whole(#[case] line: &str) {
    let q = parse(line);
    assert_eq!(q.range, None);
    assert_eq!(q.errors, vec![line.to_string()]);
}

#[test]
fn q4_partial_line_leaves_the_rest_unset() {
    let q = parse("agent:worker kind:commit");
    assert_eq!(q.filters, filters(Some("worker"), None, Some("commit")));
    assert_eq!(q.range, None);
    assert!(q.errors.is_empty());
}

#[test]
fn q5_bad_tokens_are_reported_and_good_ones_still_apply() {
    let q = parse("agent:worker nonsense foo:bar range:5y item: kind:commit");
    assert_eq!(q.filters, filters(Some("worker"), None, Some("commit")));
    assert_eq!(q.range, None);
    assert_eq!(
        q.errors,
        strings(&["nonsense", "foo:bar", "range:5y", "item:"])
    );
}

#[rstest]
#[case("tree:on", true, &[])]
#[case("tree:on tree:off", false, &[])]
#[case("tree:off tree:on", true, &[])]
#[case("tree:maybe", false, &["tree:maybe"])]
#[case("tree:", false, &["tree:"])]
fn q6_tree_flag(#[case] line: &str, #[case] tree: bool, #[case] errors: &[&str]) {
    let q = parse(line);
    assert_eq!(q.tree, tree);
    assert_eq!(q.errors, strings(errors));
}

#[test]
fn q6_format_writes_tree_last() {
    assert_eq!(
        format(&Filters::default(), Some("7d"), true),
        "range:7d tree:on"
    );
}

#[test]
fn q7_repeated_key_keeps_the_last() {
    assert_eq!(
        parse("agent:worker agent:omp").filters.agent.as_deref(),
        Some("omp")
    );
}

#[rstest]
#[case(Some("  "))]
#[case(Some(""))]
#[case(None)]
fn q8_blank_and_nil_are_empty(#[case] text: Option<&str>) {
    let q = parse_opt(text);
    assert_eq!(q, ParsedQuery::default());
    assert_eq!(q, parse(""));
}

#[test]
fn q9_format_round_trips_and_drops_the_default_window() {
    let line = "agent:worker item:imp-16 kind:commit range:7d";
    let q = parse(line);
    assert_eq!(format(&q.filters, q.range.as_deref(), q.tree), line);
    assert_eq!(
        format(&q.filters, Some("24h"), false),
        "agent:worker item:imp-16 kind:commit"
    );
    assert_eq!(format(&Filters::default(), None, false), "");
}

#[test]
fn format_is_canonical_order_regardless_of_input_order() {
    let q = parse("tree:on range:7d kind:k item:i agent:a");
    assert_eq!(
        format(&q.filters, q.range.as_deref(), q.tree),
        "agent:a item:i kind:k range:7d tree:on"
    );
}

#[rstest]
#[case("range:", &["range:"], None)]
#[case("item:a:b", &[], None)]
#[case("Agent:x", &["Agent:x"], None)]
#[case("range:7d range:5y", &["range:5y"], Some("7d"))]
#[case("range:5y range:7d", &["range:5y"], Some("7d"))]
#[case("range:24h", &[], Some("24h"))]
#[case("agent: worker", &["agent:", "worker"], None)]
fn extra_token_cases(#[case] line: &str, #[case] errors: &[&str], #[case] range: Option<&str>) {
    let q = parse(line);
    assert_eq!(q.errors, strings(errors));
    assert_eq!(q.range.as_deref(), range);
}

#[test]
fn colon_in_value_is_kept_verbatim() {
    assert_eq!(parse("item:a:b").filters.item.as_deref(), Some("a:b"));
}

#[test]
fn explicit_default_range_is_accepted_but_formatted_away() {
    let q = parse("range:24h");
    assert_eq!(format(&q.filters, q.range.as_deref(), q.tree), "");
}

#[test]
fn format_skips_empty_filter_values() {
    assert_eq!(
        format(&filters(Some(""), Some("i"), None), None, false),
        "item:i"
    );
}

// -- window ---------------------------------------------------------------------

#[rstest]
#[case("1h", true)]
#[case("0h", true)]
#[case("14d", true)]
#[case("2w", true)]
#[case("1H", false)]
#[case("5y", false)]
#[case("5m", false)]
#[case("h", false)]
#[case("", false)]
#[case("2026-09-30", true)]
#[case("2026-02-30", false)]
#[case("2024-02-29", true)]
#[case("2025-02-29", false)]
#[case("2026-13-01", false)]
#[case("2026-9-30", false)]
#[case("2026-W40", true)]
#[case("2026-W4", true)]
#[case("2026-W0", false)]
#[case("2026-W54", false)]
#[case("2026-W53", true)]
#[case("2025-W53", false)]
#[case("2026-w40", false)]
#[case("2026-09-01/2026-10-08", true)]
#[case("2026-09-01/2026-09-01", true)]
#[case("2026-10-08/2026-09-01", false)]
#[case("/2026-10-08", false)]
#[case("2026-09-01/", false)]
#[case("2026-09-01/2026-10-08/2026-11-01", false)]
#[case("2026-09-01/2026-02-30", false)]
#[case("２０２６-09-30", false)]
#[case("99999999999999999999h", false)]
fn window_grammar(#[case] token: &str, #[case] valid: bool) {
    assert_eq!(Window::parse(token).is_ok(), valid, "{token}");
}

#[test]
fn relative_window_is_open_to_now() {
    let now = 1_800_000_000;
    let i = Window::parse("3h").unwrap().resolve(now);
    assert_eq!(
        i,
        Interval {
            since: now - 3 * 3_600,
            until: None
        }
    );
    assert!(i.contains(now) && i.contains(now + 10) && !i.contains(now - 3 * 3_600 - 1));
    assert_eq!(
        Window::parse("2w").unwrap().resolve(now).since,
        now - 14 * DAY
    );
}

#[test]
fn day_window_is_half_open() {
    let i = Window::parse("2026-09-30").unwrap().resolve(0);
    assert_eq!(
        i,
        Interval {
            since: day(2026, 9, 30),
            until: Some(day(2026, 10, 1))
        }
    );
    assert!(i.contains(day(2026, 9, 30)) && !i.contains(day(2026, 10, 1)));
}

#[test]
fn week_forty_runs_monday_to_monday() {
    let i = Window::parse("2026-W40").unwrap().resolve(0);
    assert_eq!(
        i,
        Interval {
            since: day(2026, 9, 28),
            until: Some(day(2026, 10, 5))
        }
    );
}

#[test]
fn span_includes_the_last_day() {
    let i = Window::parse("2026-09-01/2026-10-08").unwrap().resolve(0);
    assert_eq!(
        i,
        Interval {
            since: day(2026, 9, 1),
            until: Some(day(2026, 10, 9))
        }
    );
}

#[test]
fn window_label_is_the_token_as_typed() {
    assert_eq!(Window::parse("0014d").unwrap().label(), "0014d");
}

#[test]
fn civil_known_days() {
    assert_eq!(days_from_civil(1970, 1, 1), 0);
    assert_eq!(civil_from_days(0), (1970, 1, 1));
    assert_eq!(civil_from_days(days_from_civil(2024, 2, 29)), (2024, 2, 29));
    assert_eq!(weekday_monday0(days_from_civil(2026, 9, 28)), 0);
}

// -- filters --------------------------------------------------------------------

#[test]
fn p16_filters_narrow_and_unknown_agent_matches_nothing() {
    let events = [
        ev(Some("worker"), Some("imp-1"), "note"),
        ev(Some("worker"), Some("imp-2"), "note"),
        ev(Some("omp"), Some("imp-3"), "note"),
    ];
    let kept = |f: &Filters| events.iter().filter(|e| f.keep(*e)).count();
    assert_eq!(kept(&Filters::default()), 3);
    assert_eq!(kept(&filters(Some("omp"), None, None)), 1);
    assert_eq!(kept(&filters(None, Some("imp-2"), None)), 1);
    assert_eq!(kept(&filters(Some("nobody"), None, None)), 0);
    assert_eq!(kept(&filters(Some("Worker"), None, None)), 0);
    assert_eq!(kept(&filters(Some("worker"), Some("imp-3"), None)), 0);
}

#[test]
fn blank_or_missing_agent_never_matches_an_agent_filter() {
    let f = filters(Some("worker"), None, None);
    assert!(!f.keep(&ev(None, None, "note")));
    assert!(!f.keep(&ev(Some(""), None, "note")));
}

#[test]
fn p19_blank_agent_is_unattributed() {
    assert!(is_attributed(Some("worker")));
    assert!(!is_attributed(Some("")));
    assert!(!is_attributed(Some("  ")));
    assert!(!is_attributed(None));
}

// -- completions ----------------------------------------------------------------

#[test]
fn p17_completions_are_whole_lines_over_unfiltered_events() {
    let events = [
        ev(Some("worker"), Some("imp-1"), "note"),
        ev(Some("omp"), Some("imp-1"), "commit"),
    ];
    let c = completions(&filters(None, None, Some("note")), None, false, &events);
    let values: Vec<&str> = c.iter().map(|c| c.value.as_str()).collect();
    assert!(values.contains(&"agent:worker kind:note"));
    assert!(values.contains(&"agent:omp kind:note"));
    assert!(values.contains(&"kind:note range:7d"));
}

#[test]
fn completions_order_dedupe_and_blank_removal() {
    let events = [
        ev(Some("zed"), Some("b"), "note"),
        ev(Some("alpha"), Some("a"), "commit"),
        ev(Some("zed"), None, "note"),
        ev(Some(" "), Some(""), "note"),
        ev(None, None, "note"),
    ];
    let c = completions(&Filters::default(), None, false, &events);
    let labels: Vec<&str> = c.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "agent:alpha",
            "agent:zed",
            "item:a",
            "item:b",
            "kind:commit",
            "kind:note",
            "range:24h",
            "range:7d",
            "range:30d",
            "range:90d",
            "tree:on",
        ]
    );
}

#[test]
fn completion_24h_equals_the_no_range_line_and_tree_option_turns_tree_on() {
    let f = filters(Some("a"), None, None);
    let c = completions::<EventFields>(&f, Some("7d"), false, &[]);
    let by = |label: &str| c.iter().find(|c| c.label == label).unwrap().value.clone();
    assert_eq!(by("range:24h"), "agent:a");
    assert_eq!(by("range:30d"), "agent:a range:30d");
    assert_eq!(by("tree:on"), "agent:a range:7d tree:on");
}

#[test]
fn completions_mark_the_active_ones() {
    let events = [
        ev(Some("worker"), None, "note"),
        ev(Some("omp"), None, "note"),
    ];
    let c = completions(&filters(Some("omp"), None, None), Some("7d"), true, &events);
    let active: Vec<&str> = c
        .iter()
        .filter(|c| c.active)
        .map(|c| c.label.as_str())
        .collect();
    assert_eq!(active, ["agent:omp", "range:7d", "tree:on"]);
    let none = completions(&Filters::default(), None, false, &events);
    let active: Vec<&str> = none
        .iter()
        .filter(|c| c.active)
        .map(|c| c.label.as_str())
        .collect();
    assert_eq!(active, ["range:24h"]);
}

// -- tones and legend -------------------------------------------------------------

#[test]
fn l4_legend_order_is_blue_green_yellow_red_hollow() {
    let colours: Vec<&str> = legend().iter().map(|e| e.colour).collect();
    assert_eq!(colours, ["blue", "green", "yellow", "red", "hollow"]);
    assert_eq!(legend()[0].brief(), "claimed · in progress · branch head …");
}

#[test]
fn legend_means_match_the_tables_exactly() {
    let means = |tone: Tone| {
        legend()
            .iter()
            .find(|e| e.tone == tone)
            .unwrap()
            .means
            .clone()
    };
    assert_eq!(
        means(Tone::Blue),
        strings(&[
            "claimed",
            "in progress",
            "branch head",
            "item claimed",
            "pr opened",
            "pr reviewed",
            "run reasoning",
            "run started",
            "run thought",
            "run tool",
        ])
    );
    assert_eq!(
        means(Tone::Green),
        strings(&["done", "pr merged", "run finished"])
    );
    assert_eq!(means(Tone::Yellow), strings(&["review", "handoff"]));
    assert_eq!(means(Tone::Red), strings(&["blocked"]));
    assert_eq!(
        means(Tone::Hollow),
        strings(&[
            "item created",
            "item released",
            "item status",
            "note",
            "commit"
        ])
    );
}

#[test]
fn l2_legend_names_what_colours_mean() {
    let entry = |tone: Tone| legend().iter().find(|e| e.tone == tone).unwrap();
    assert!(entry(Tone::Blue).means.contains(&"claimed".to_string()));
    assert!(entry(Tone::Green).means.contains(&"pr merged".to_string()));
    assert!(entry(Tone::Yellow).means.contains(&"handoff".to_string()));
    assert!(entry(Tone::Red).means.contains(&"blocked".to_string()));
    assert!(entry(Tone::Hollow)
        .means
        .contains(&"item released".to_string()));
    assert_eq!(entry(Tone::Hollow).colour, "hollow");
}

#[rstest]
#[case(Tone::Blue, "claimed · in progress · branch head …")]
#[case(Tone::Green, "done · pr merged · run finished")]
#[case(Tone::Yellow, "review · handoff")]
#[case(Tone::Red, "blocked")]
#[case(Tone::Hollow, "item created · item released · item status …")]
fn legend_brief(#[case] tone: Tone, #[case] brief: &str) {
    assert_eq!(
        legend().iter().find(|e| e.tone == tone).unwrap().brief(),
        brief
    );
}

#[test]
fn legend_title_lists_every_meaning() {
    let blue = &legend()[0];
    assert_eq!(blue.title().split(" · ").count(), blue.means.len());
}

#[test]
fn l1_every_drawn_tone_is_in_the_legend_and_back() {
    let kinds = [
        "item_created",
        "item_claimed",
        "item_released",
        "item_status",
        "note",
        "handoff",
        "run_started",
        "run_tool",
        "run_thought",
        "run_reasoning",
        "run_model",
        "run_finished",
        "commit",
        "branch_head",
        "pr_opened",
        "pr_reviewed",
        "pr_merged",
    ];
    let statuses = [
        "open",
        "claimed",
        "in_progress",
        "review",
        "blocked",
        "done",
    ];
    let drawn: BTreeSet<Tone> = kinds
        .iter()
        .flat_map(|k| statuses.iter().map(move |s| tone_for_event(k, Some(s))))
        .collect();
    let listed: BTreeSet<Tone> = legend().iter().map(|e| e.tone).collect();
    assert!(drawn.len() >= 4);
    assert_eq!(drawn, listed);
}

#[rstest]
#[case("blocked", Tone::Red)]
#[case("claimed", Tone::Blue)]
#[case("done", Tone::Green)]
#[case("in_progress", Tone::Blue)]
#[case("review", Tone::Yellow)]
#[case("open", Tone::Hollow)]
#[case("whatever", Tone::Hollow)]
fn status_tones(#[case] status: &str, #[case] tone: Tone) {
    assert_eq!(tone_for_status(Some(status)), tone);
}

#[test]
fn nil_status_is_hollow() {
    assert_eq!(tone_for_status(None), Tone::Hollow);
}

#[rstest]
#[case("item_claimed", Tone::Blue)]
#[case("handoff", Tone::Yellow)]
#[case("run_finished", Tone::Green)]
#[case("pr_merged", Tone::Green)]
#[case("path", Tone::Blue)]
#[case("branch", Tone::Blue)]
#[case("note", Tone::Hollow)]
#[case("heartbeat", Tone::Hollow)]
#[case("collab", Tone::Hollow)]
#[case("nonsense", Tone::Hollow)]
fn kind_tones(#[case] kind: &str, #[case] tone: Tone) {
    assert_eq!(tone_for_kind(kind), tone);
}

#[test]
fn item_status_events_take_the_status_tone() {
    assert_eq!(tone_for_event("item_status", Some("blocked")), Tone::Red);
    assert_eq!(tone_for_event("item_status", None), Tone::Hollow);
    assert_eq!(tone_for_event("handoff", Some("blocked")), Tone::Yellow);
}

#[test]
fn l5_merged_cluster_takes_the_most_severe_tone() {
    assert_eq!(
        Tone::most_severe([Tone::Hollow, Tone::Red, Tone::Green]),
        Some(Tone::Red)
    );
    assert_eq!(Tone::most_severe([]), None);
    assert_eq!(Tone::Green.merge(Tone::Blue), Tone::Green);
    assert_eq!(Tone::Blue.merge(Tone::Yellow), Tone::Yellow);
}

#[test]
fn severity_ranks_err_over_warn_over_ok_over_info_over_neutral() {
    let ranks: Vec<u8> = [
        Tone::Red,
        Tone::Yellow,
        Tone::Green,
        Tone::Blue,
        Tone::Hollow,
    ]
    .iter()
    .map(|t| t.severity())
    .collect();
    assert_eq!(ranks, [4, 3, 2, 1, 0]);
}

#[test]
fn tone_names_are_the_data_tone_values() {
    let names: Vec<&str> = Tone::ALL.iter().map(|t| t.name()).collect();
    assert_eq!(names, ["info", "ok", "warn", "err", "neutral"]);
}

// -- tone filter (extension) ---------------------------------------------------------

#[test]
fn empty_tone_set_shows_everything() {
    let s = GraphState::default();
    assert!(Tone::ALL.iter().all(|t| s.tone_visible(*t)));
    assert!(s.legend_chips().iter().all(|c| !c.pressed && !c.dimmed));
}

#[test]
fn selected_tones_are_pressed_and_the_rest_dimmed() {
    let mut s = GraphState::default();
    assert_eq!(
        s.reduce(GraphAction::ToggleTone(Tone::Red)),
        Effect::Relayout
    );
    s.reduce(GraphAction::ToggleTone(Tone::Green));
    assert!(s.tone_visible(Tone::Red) && s.tone_visible(Tone::Green));
    assert!(!s.tone_visible(Tone::Blue) && !s.tone_visible(Tone::Hollow));
    let chips = s.legend_chips();
    let pressed: Vec<Tone> = chips.iter().filter(|c| c.pressed).map(|c| c.tone).collect();
    let dimmed: Vec<Tone> = chips.iter().filter(|c| c.dimmed).map(|c| c.tone).collect();
    assert_eq!(pressed, [Tone::Green, Tone::Red]);
    assert_eq!(dimmed, [Tone::Blue, Tone::Yellow, Tone::Hollow]);
}

#[test]
fn toggling_a_tone_twice_clears_it() {
    let mut s = GraphState::default();
    s.reduce(GraphAction::ToggleTone(Tone::Blue));
    s.reduce(GraphAction::ToggleTone(Tone::Blue));
    assert_eq!(s, GraphState::default());
}

#[test]
fn clear_tones_reports_nothing_when_already_clear() {
    let mut s = GraphState::default();
    assert_eq!(s.reduce(GraphAction::ClearTones), Effect::None);
    s.reduce(GraphAction::ToggleTone(Tone::Blue));
    assert_eq!(s.reduce(GraphAction::ClearTones), Effect::Relayout);
    assert!(s.tones.is_empty());
}

#[test]
fn a_submitted_query_resets_the_tone_filter() {
    let mut s = GraphState::default();
    s.reduce(GraphAction::ToggleTone(Tone::Blue));
    s.reduce(GraphAction::Query("agent:a".into()));
    assert!(s.tones.is_empty());
}

#[test]
fn tone_filter_counts_as_filtering() {
    let mut s = GraphState::default();
    assert!(!s.is_filtering());
    s.reduce(GraphAction::ToggleTone(Tone::Blue));
    assert!(s.is_filtering());
}

// -- toolbar state -----------------------------------------------------------------

/// One reducer step: the action and the effect it must report.
type Step = (GraphAction, Effect);

fn run(state: &mut GraphState, steps: Vec<Step>) {
    for (action, effect) in steps {
        let label = format!("{action:?}");
        assert_eq!(state.reduce(action), effect, "{label}");
    }
}

fn query(text: &str) -> GraphAction {
    GraphAction::Query(text.to_string())
}

fn mode(token: &str) -> GraphAction {
    GraphAction::SetMode(token.to_string())
}

fn follow(item: Option<&str>) -> GraphAction {
    GraphAction::FollowItem(item.map(str::to_string))
}

#[test]
fn defaults_match_mount() {
    let s = GraphState::default();
    assert_eq!(s.window, "24h");
    assert_eq!(s.mode, Mode::Graph);
    assert_eq!(s.graph_width, 1200);
    assert!(!s.tree && !s.show_all && !s.unattributed_open && !s.compact);
    assert!(s.loose_kinds.is_empty() && s.query_errors.is_empty() && s.tones.is_empty());
    assert_eq!(s.followed, Followed::Unset);
}

#[test]
fn p2_default_line_is_empty() {
    assert_eq!(GraphState::default().line(), "");
}

#[test]
fn p1_line_shows_selected_filters() {
    let mut s = GraphState::default();
    s.filters.agent = Some("worker".into());
    s.window = "7d".into();
    assert_eq!(s.line(), "agent:worker range:7d");
}

#[test]
fn brief_toolbar_state_formats_as_in_the_screenshot() {
    let s = GraphState {
        window: "30d".into(),
        tree: true,
        ..GraphState::default()
    };
    assert_eq!(s.line(), "range:30d tree:on");
}

#[test]
fn v1_filter_follows_the_selected_item_and_clears_with_it() {
    let mut s = GraphState::default();
    run(&mut s, vec![(follow(Some("imp-16")), Effect::Reload)]);
    assert_eq!(s.line(), "item:imp-16");
    assert_eq!(s.reduce(follow(Some("imp-17"))), Effect::Reload);
    assert_eq!(s.line(), "item:imp-17");
    assert_eq!(s.reduce(follow(None)), Effect::Reload);
    assert_eq!(s.filters.item, None);
    assert_eq!(s.line(), "");
}

#[test]
fn v2_an_edit_stands_until_the_selection_changes() {
    let mut s = GraphState::default();
    s.reduce(follow(Some("imp-16")));
    s.reduce(query("item:imp-99 range:7d"));
    assert_eq!(
        (s.filters.item.as_deref(), s.window.as_str()),
        (Some("imp-99"), "7d")
    );
    assert_eq!(s.reduce(follow(Some("imp-16"))), Effect::None);
    assert_eq!(s.filters.item.as_deref(), Some("imp-99"));
    assert_eq!(s.reduce(follow(Some("imp-20"))), Effect::Reload);
    assert_eq!(s.filters.item.as_deref(), Some("imp-20"));
}

#[test]
fn following_none_first_is_a_change_from_unset() {
    let mut s = GraphState::default();
    assert_eq!(s.reduce(follow(None)), Effect::Reload);
    assert_eq!(s.followed, Followed::Item(None));
    assert_eq!(s.reduce(follow(None)), Effect::None);
}

#[test]
fn following_clears_query_errors() {
    let mut s = GraphState::default();
    s.reduce(query("bogus:1"));
    s.reduce(follow(Some("imp-1")));
    assert!(s.query_errors.is_empty());
}

#[test]
fn v3_mode_switch_keeps_filters_window_and_tree() {
    let mut s = GraphState::default();
    s.reduce(query("agent:worker range:7d tree:on"));
    run(&mut s, vec![(mode("lanes"), Effect::Reload)]);
    assert_eq!(s.mode, Mode::Lanes);
    assert_eq!(s.line(), "agent:worker range:7d tree:on");
    run(&mut s, vec![(mode("collaboration"), Effect::Reload)]);
    assert_eq!(s.mode, Mode::Collab);
    assert_eq!(s.line(), "agent:worker range:7d tree:on");
}

#[test]
fn p12_unknown_mode_is_graph() {
    let mut s = GraphState::default();
    s.reduce(mode("lanes"));
    s.reduce(mode("nonsense"));
    assert_eq!(s.mode, Mode::Graph);
}

#[test]
fn mode_clears_query_errors_and_leaving_graph_forgets_strips() {
    let mut s = GraphState::default();
    s.reduce(query("bogus:1"));
    s.reduce(GraphAction::ToggleStripKind("commit".into()));
    assert_eq!(s.loose_kinds, strings(&["commit"]));
    s.reduce(mode("lanes"));
    assert!(s.query_errors.is_empty());
    assert!(s.loose_kinds.is_empty());
}

#[test]
fn strip_toggle_is_ignored_outside_graph_mode() {
    let mut s = GraphState::default();
    s.reduce(mode("lanes"));
    assert_eq!(
        s.reduce(GraphAction::ToggleStripKind("commit".into())),
        Effect::None
    );
    assert!(s.loose_kinds.is_empty());
}

#[test]
fn strip_kinds_append_in_order_and_remove_on_second_toggle() {
    let mut s = GraphState::default();
    for kind in ["commit", "branch_head", "pr_merged"] {
        assert_eq!(
            s.reduce(GraphAction::ToggleStripKind(kind.into())),
            Effect::Relayout
        );
    }
    assert_eq!(
        s.loose_kinds,
        strings(&["commit", "branch_head", "pr_merged"])
    );
    s.reduce(GraphAction::ToggleStripKind("branch_head".into()));
    assert_eq!(s.loose_kinds, strings(&["commit", "pr_merged"]));
    assert!(s.is_strip_open("commit") && !s.is_strip_open("branch_head"));
}

#[test]
fn p11_exactly_the_current_mode_chip_is_pressed() {
    let mut s = GraphState::default();
    s.reduce(mode("collaboration"));
    let chips = s.mode_chips();
    let ids: Vec<&str> = chips.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "activity-modes-graph",
            "activity-modes-lanes",
            "activity-modes-collaboration"
        ]
    );
    let labels: Vec<&str> = chips.iter().map(|c| c.label).collect();
    assert_eq!(labels, ["graph", "lanes", "collab"]);
    let pressed: Vec<bool> = chips.iter().map(|c| c.pressed).collect();
    assert_eq!(pressed, [false, false, true]);
    assert_eq!(chips[2].aria_label, "show collab");
}

#[test]
fn toolbar_visibility_matrix() {
    assert!(Mode::Graph.has_query_line() && Mode::Graph.has_legend());
    assert!(Mode::Lanes.has_query_line() && !Mode::Lanes.has_legend());
    assert!(!Mode::Collab.has_query_line() && !Mode::Collab.has_legend());
}

#[rstest]
#[case(733, 733, Effect::Relayout)]
#[case(1900, 1900, Effect::Relayout)]
#[case(0, 1200, Effect::None)]
#[case(-5, 1200, Effect::None)]
#[case(i64::MAX, u32::MAX, Effect::Relayout)]
fn v5_graph_width(#[case] reported: i64, #[case] width: u32, #[case] effect: Effect) {
    let mut s = GraphState::default();
    assert_eq!(s.graph_width, 1200);
    assert_eq!(s.reduce(GraphAction::GraphWidth(reported)), effect);
    assert_eq!(s.graph_width, width);
}

#[test]
fn p4_query_sets_filters_and_window_together() {
    let mut s = GraphState::default();
    run(
        &mut s,
        vec![(query("agent:worker range:90d"), Effect::Reload)],
    );
    assert_eq!(s.filters.agent.as_deref(), Some("worker"));
    assert_eq!(s.window, "90d");
}

#[test]
fn p5_a_line_without_a_filter_clears_it() {
    let mut s = GraphState::default();
    s.reduce(query("agent:worker range:7d"));
    s.reduce(query("range:7d"));
    assert_eq!(s.filters, Filters::default());
    assert_eq!(s.window, "7d");
}

#[test]
fn a_line_replaces_window_and_tree_when_omitted() {
    let mut s = GraphState::default();
    s.reduce(query("range:7d tree:on"));
    s.reduce(query("agent:a"));
    assert_eq!((s.window.as_str(), s.tree), ("24h", false));
}

#[test]
fn p6_unreadable_tokens_are_ignored_and_named() {
    let mut s = GraphState::default();
    s.reduce(query("agent:worker bogus:1 range:5y"));
    assert_eq!(s.filters.agent.as_deref(), Some("worker"));
    assert_eq!(s.window, "24h");
    assert_eq!(s.query_errors, strings(&["bogus:1", "range:5y"]));
    assert_eq!(
        s.errors_text().as_deref(),
        Some("ignored: bogus:1 range:5y")
    );
    s.reduce(query("agent:worker"));
    assert_eq!(s.errors_text(), None);
}

#[test]
fn p7_tree_toggle_and_typed_tree_are_the_same_state() {
    let mut s = GraphState::default();
    assert!(!s.tree);
    run(&mut s, vec![(GraphAction::ToggleTree, Effect::Reload)]);
    assert!(s.tree);
    assert_eq!(s.line(), "tree:on");
    s.reduce(GraphAction::ToggleTree);
    assert!(!s.tree);
    s.reduce(query("agent:worker tree:on"));
    assert!(s.tree);
    assert_eq!(s.filters.agent.as_deref(), Some("worker"));
    assert_eq!(s.line(), "agent:worker tree:on");
}

#[test]
fn tree_toggle_keeps_query_errors() {
    let mut s = GraphState::default();
    s.reduce(query("bogus:1"));
    s.reduce(GraphAction::ToggleTree);
    assert_eq!(s.query_errors, strings(&["bogus:1"]));
}

#[test]
fn p8_second_agent_replaces_the_first() {
    let mut s = GraphState::default();
    s.reduce(query("agent:worker"));
    s.reduce(query("agent:omp"));
    assert_eq!(s.filters.agent.as_deref(), Some("omp"));
}

#[test]
fn p9_open_unattributed_stays_open_across_a_query() {
    let mut s = GraphState::default();
    assert_eq!(s.reduce(GraphAction::ToggleUnattributed), Effect::None);
    s.reduce(query("agent:worker"));
    assert!(s.unattributed_open);
    s.reduce(GraphAction::ToggleUnattributed);
    assert!(!s.unattributed_open);
}

#[test]
fn p10_mode_keeps_filters_and_window() {
    let mut s = GraphState::default();
    s.reduce(query("agent:worker range:7d"));
    s.reduce(mode("collaboration"));
    assert_eq!(s.filters.agent.as_deref(), Some("worker"));
    assert_eq!(s.window, "7d");
    assert_eq!(s.mode, Mode::Collab);
}

#[test]
fn show_all_toggles_and_survives_query_mode_and_tree() {
    let mut s = GraphState::default();
    assert_eq!(s.reduce(GraphAction::ToggleShowAll), Effect::Relayout);
    s.reduce(query("agent:a"));
    s.reduce(GraphAction::ToggleTree);
    s.reduce(mode("lanes"));
    assert!(s.show_all);
    s.reduce(GraphAction::ToggleShowAll);
    assert!(!s.show_all);
}

#[test]
fn refresh_reloads_without_touching_state() {
    let mut s = GraphState::default();
    s.reduce(query("agent:a bogus"));
    let before = s.clone();
    assert_eq!(s.reduce(GraphAction::Refresh), Effect::Reload);
    assert_eq!(s, before);
}

#[test]
fn show_all_button_label() {
    assert_eq!(show_all_label(false, 2351), "show all (2351)");
    assert_eq!(show_all_label(true, 2351), "show newest 20");
}

#[test]
fn window_label_resolves_against_now() {
    let mut s = GraphState::default();
    s.reduce(query("range:2026-09-30"));
    assert_eq!(
        s.interval(0),
        Some(Interval {
            since: day(2026, 9, 30),
            until: Some(day(2026, 10, 1))
        })
    );
    s.window = "garbage".into();
    assert_eq!(s.interval(0), None);
}

#[test]
fn tokens_are_active_query_parts_with_their_removal_line() {
    let mut s = GraphState::default();
    assert!(s.tokens().is_empty());
    s.reduce(query("kind:note agent:a range:7d tree:on"));
    let tokens = s.tokens();
    let view: Vec<(&str, &str, &str)> = tokens
        .iter()
        .map(|t| (t.key, t.label.as_str(), t.remove.as_str()))
        .collect();
    assert_eq!(
        view,
        [
            ("agent", "agent:a", "kind:note range:7d tree:on"),
            ("kind", "kind:note", "agent:a range:7d tree:on"),
            ("range", "range:7d", "agent:a kind:note tree:on"),
            ("tree", "tree:on", "agent:a kind:note range:7d"),
        ]
    );
    s.reduce(query(&tokens[2].remove));
    assert_eq!(s.window, "24h");
}

#[test]
fn state_completions_use_state_line_context() {
    let mut s = GraphState::default();
    s.reduce(query("kind:note range:7d"));
    let events = [ev(Some("worker"), None, "note")];
    let c = s.completions(&events);
    assert!(c
        .iter()
        .any(|c| c.value == "agent:worker kind:note range:7d" && !c.active));
    assert!(c.iter().any(|c| c.label == "kind:note" && c.active));
}

// -- url ----------------------------------------------------------------------------

#[rstest]
#[case(
    "mode=lanes&agent=worker&window=90d",
    "?mode=lanes&agent=worker&window=90d"
)]
#[case("mode=lanes&agent=worker&tree=1", "?mode=lanes&agent=worker&tree=1")]
#[case(
    "mode=collaboration&agent=worker&window=7d",
    "?mode=collaboration&agent=worker&window=7d"
)]
#[case(
    "mode=lanes&agent=worker&unattributed=1",
    "?mode=lanes&agent=worker&unattributed=1"
)]
#[case("", "")]
fn url_round_trips(#[case] raw: &str, #[case] expect: &str) {
    let params: Vec<(&str, &str)> = raw.split('&').filter_map(|p| p.split_once('=')).collect();
    assert_eq!(GraphState::from_params(&params).query_string(), expect);
}

#[test]
fn url_params_are_sanitised() {
    let s = GraphState::from_params(&[
        ("agent", ""),
        ("item", "   "),
        ("window", "5y"),
        ("mode", "nonsense"),
        ("tree", "yes"),
        ("kind", "note"),
    ]);
    assert_eq!(s.filters, filters(None, None, Some("note")));
    assert_eq!(
        (s.window.as_str(), s.mode, s.tree),
        ("24h", Mode::Graph, false)
    );
}

#[test]
fn url_values_are_form_encoded() {
    let s = GraphState::from_params(&[("window", "2026-09-01/2026-10-08"), ("item", "a b:c")]);
    assert_eq!(
        s.query_string(),
        "?item=a+b%3Ac&window=2026-09-01%2F2026-10-08"
    );
}

#[test]
fn url_last_param_wins() {
    let s = GraphState::from_params(&[("agent", "a"), ("agent", "b")]);
    assert_eq!(s.filters.agent.as_deref(), Some("b"));
}

#[test]
fn state_serde_round_trips() {
    let mut s = GraphState::default();
    s.reduce(follow(None));
    s.reduce(query("agent:a range:7d"));
    s.reduce(GraphAction::ToggleTone(Tone::Red));
    let json = serde_json::to_string(&s).unwrap();
    assert_eq!(serde_json::from_str::<GraphState>(&json).unwrap(), s);
    assert!(serde_json::from_str::<GraphState>(r#"{"bogus":1}"#).is_err());
    let action = GraphAction::ToggleTone(Tone::Hollow);
    let json = serde_json::to_string(&action).unwrap();
    assert_eq!(json, r#"{"toggle_tone":"hollow"}"#);
    assert_eq!(serde_json::from_str::<GraphAction>(&json).unwrap(), action);
}

// -- properties ---------------------------------------------------------------------

const VALID_WINDOWS: [&str; 8] = [
    "7d",
    "30d",
    "90d",
    "3h",
    "2w",
    "2026-W40",
    "2026-09-30",
    "2026-09-01/2026-10-08",
];

fn value() -> impl Strategy<Value = Option<String>> {
    prop::option::of("[a-zA-Z0-9_.:/-]{1,8}")
}

fn tone() -> impl Strategy<Value = Tone> {
    prop::sample::select(Tone::ALL.to_vec())
}

fn action() -> impl Strategy<Value = GraphAction> {
    prop_oneof![
        "[a-z:0-9 ]{0,24}".prop_map(GraphAction::Query),
        prop::sample::select(vec![
            "agent:a range:7d",
            "agent:b tree:on kind:note",
            "range:5y item:",
            "range:2026-W40 item:imp-1",
            "",
        ])
        .prop_map(|s| GraphAction::Query(s.to_string())),
        Just(GraphAction::ToggleTree),
        prop::sample::select(vec!["graph", "lanes", "collaboration", "x"])
            .prop_map(|s| GraphAction::SetMode(s.to_string())),
        Just(GraphAction::ToggleShowAll),
        Just(GraphAction::ToggleUnattributed),
        any::<i64>().prop_map(GraphAction::GraphWidth),
        prop::sample::select(vec!["commit", "branch_head", "pr_merged"])
            .prop_map(|s| GraphAction::ToggleStripKind(s.to_string())),
        Just(GraphAction::Refresh),
        prop::option::of("[a-z0-9-]{1,6}").prop_map(GraphAction::FollowItem),
        tone().prop_map(GraphAction::ToggleTone),
        Just(GraphAction::ClearTones),
    ]
}

proptest! {
    #[test]
    fn parse_never_panics_and_errors_are_input_tokens(text in "\\PC{0,60}") {
        let q = parse(&text);
        let tokens: Vec<&str> = text.split_whitespace().collect();
        prop_assert!(q.errors.iter().all(|e| tokens.contains(&e.as_str())));
        let positions: Vec<usize> = q
            .errors
            .iter()
            .scan(0, |from, e| {
                let at = tokens[*from..].iter().position(|t| t == e)? + *from;
                *from = at + 1;
                Some(at)
            })
            .collect();
        prop_assert_eq!(positions.len(), q.errors.len(), "errors keep input order");
    }

    #[test]
    fn parse_of_format_is_identity_on_canonical_queries(
        agent in value(), item in value(), kind in value(),
        window in prop::option::of(prop::sample::select(VALID_WINDOWS.to_vec())),
        tree in any::<bool>(),
    ) {
        let q = ParsedQuery {
            filters: Filters { agent, item, kind },
            range: window.map(str::to_string),
            tree,
            errors: Vec::new(),
        };
        let line = format(&q.filters, q.range.as_deref(), q.tree);
        prop_assert_eq!(parse(&line), q);
    }

    #[test]
    fn format_of_parse_is_idempotent(text in "[a-z0-9:/ -]{0,60}") {
        let once = parse(&text);
        let line = format(&once.filters, once.range.as_deref(), once.tree);
        let twice = parse(&line);
        prop_assert!(twice.errors.is_empty());
        prop_assert_eq!(format(&twice.filters, twice.range.as_deref(), twice.tree), line);
    }

    #[test]
    fn relative_window_since_is_now_minus_length(
        n in 0i64..100_000, unit in prop::sample::select(vec![('h', 3_600i64), ('d', DAY), ('w', 7 * DAY)]),
        now in 0i64..4_000_000_000,
    ) {
        let w = Window::parse(&format!("{n}{}", unit.0)).unwrap();
        prop_assert_eq!(w.resolve(now), Interval { since: now - n * unit.1, until: None });
    }

    #[test]
    fn civil_days_round_trip(days in -800_000i64..800_000) {
        let (y, m, d) = civil_from_days(days);
        prop_assert_eq!(days_from_civil(y, m, d), days);
        prop_assert!((1..=12).contains(&m) && d >= 1 && d <= days_in_month(y, m));
    }

    #[test]
    fn real_days_parse_to_a_day_long_interval(y in 1970i64..2100, m in 1u32..=12, d in 1u32..=28) {
        let i = Window::parse(&format!("{y:04}-{m:02}-{d:02}")).unwrap().resolve(0);
        prop_assert_eq!(i.since, day(y, m, d));
        prop_assert_eq!(i.until, Some(i.since + DAY));
    }

    #[test]
    fn weeks_start_on_monday_and_last_seven_days(y in 1900i64..2200, w in 1i64..=52) {
        let i = Window::parse(&format!("{y:04}-W{w:02}")).unwrap().resolve(0);
        let monday = i.since / DAY;
        prop_assert_eq!(i.since % DAY, 0);
        prop_assert_eq!(weekday_monday0(monday), 0);
        prop_assert_eq!(i.until, Some(i.since + 7 * DAY));
        prop_assert_eq!(civil_from_days(monday + 3).0, y);
    }

    #[test]
    fn invalid_calendar_days_never_parse(y in 1970i64..2100, m in 1u32..=12, extra in 1u32..=10) {
        let d = days_in_month(y, m) + extra;
        let token = format!("{y:04}-{m:02}-{d:02}");
        prop_assert!(Window::parse(&token).is_err());
    }

    #[test]
    fn spans_need_ordered_ends(a in 0i64..40_000, b in 0i64..40_000) {
        let fmt = |days: i64| {
            let (y, m, d) = civil_from_days(days);
            format!("{y:04}-{m:02}-{d:02}")
        };
        let ok = Window::parse(&format!("{}/{}", fmt(a), fmt(b))).is_ok();
        prop_assert_eq!(ok, a <= b);
    }

    #[test]
    fn merged_tone_is_a_member_with_max_severity(tones in prop::collection::vec(tone(), 1..8)) {
        let merged = Tone::most_severe(tones.iter().copied()).unwrap();
        prop_assert!(tones.contains(&merged));
        prop_assert!(tones.iter().all(|t| t.severity() <= merged.severity()));
    }

    #[test]
    fn reducer_invariants_hold_over_any_sequence(actions in prop::collection::vec(action(), 0..40)) {
        let mut s = GraphState::default();
        for a in actions {
            let was_show_all = s.show_all;
            let keep_show_all = matches!(a, GraphAction::Query(_) | GraphAction::ToggleTree | GraphAction::SetMode(_));
            let effect = s.reduce(a.clone());
            prop_assert!(s.graph_width > 0);
            prop_assert!(Window::is_valid(&s.window));
            prop_assert!(s.mode == Mode::Graph || s.loose_kinds.is_empty());
            let unique: BTreeSet<&String> = s.loose_kinds.iter().collect();
            prop_assert_eq!(unique.len(), s.loose_kinds.len());
            if keep_show_all {
                prop_assert_eq!(s.show_all, was_show_all);
            }
            if matches!(a, GraphAction::Refresh) {
                prop_assert_eq!(effect, Effect::Reload);
            }
            let reparsed = parse(&s.line());
            prop_assert!(reparsed.errors.is_empty());
            prop_assert_eq!(&reparsed.filters, &s.filters);
            prop_assert_eq!(reparsed.tree, s.tree);
            prop_assert_eq!(reparsed.range.unwrap_or_else(|| DEFAULT_WINDOW.to_string()), s.window.clone());
            prop_assert_eq!(s.tones.is_empty(), Tone::ALL.iter().all(|t| s.tone_visible(*t)));
        }
    }

    #[test]
    fn submitting_the_shown_line_changes_nothing(actions in prop::collection::vec(action(), 0..20)) {
        let mut s = GraphState::default();
        for a in actions {
            s.reduce(a);
        }
        let mut after = s.clone();
        after.reduce(GraphAction::Query(s.line()));
        after.query_errors = s.query_errors.clone();
        after.tones = s.tones.clone();
        prop_assert_eq!(after, s);
    }

    #[test]
    fn tone_toggle_twice_is_identity(setup in prop::collection::vec(tone(), 0..5), t in tone()) {
        let mut s = GraphState::default();
        for x in setup {
            s.reduce(GraphAction::ToggleTone(x));
        }
        let before = s.clone();
        s.reduce(GraphAction::ToggleTone(t));
        s.reduce(GraphAction::ToggleTone(t));
        prop_assert_eq!(s, before);
    }

    #[test]
    fn visible_tones_are_exactly_the_selected_ones(sel in prop::collection::btree_set(tone(), 1..5)) {
        let s = GraphState { tones: sel.clone(), ..GraphState::default() };
        for t in Tone::ALL {
            prop_assert_eq!(s.tone_visible(t), sel.contains(&t));
        }
    }

    #[test]
    fn filters_keep_is_conjunction_of_fields(
        a in value(), i in value(), k in value(),
        ea in value(), ei in value(), ek in "[a-z]{1,4}",
    ) {
        let f = Filters { agent: a.clone(), item: i.clone(), kind: k.clone() };
        let e = EventFields { agent: ea.clone(), item_id: ei.clone(), kind: ek.clone() };
        let want = a.is_none_or(|a| ea.as_deref() == Some(a.as_str()))
            && i.is_none_or(|i| ei.as_deref() == Some(i.as_str()))
            && k.is_none_or(|k| ek == k);
        prop_assert_eq!(f.keep(&e), want);
    }

    #[test]
    fn completions_are_valid_lines_that_change_one_key(
        events in prop::collection::vec(("[a-c]{1,2}", "[x-z]{1,2}", "[k-m]{1,2}"), 0..10),
        agent in value(),
    ) {
        let evs: Vec<EventFields> = events
            .into_iter()
            .map(|(a, i, k)| EventFields { agent: Some(a), item_id: Some(i), kind: k })
            .collect();
        let base = Filters { agent, ..Filters::default() };
        for c in completions(&base, None, false, &evs) {
            let q = parse(&c.value);
            prop_assert!(q.errors.is_empty(), "{}", c.value);
        }
    }
}
