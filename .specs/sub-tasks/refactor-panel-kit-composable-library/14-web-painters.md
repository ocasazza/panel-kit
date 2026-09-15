# Step 14: Web painters over core models

**Task File:** `.specs/tasks/todo/refactor-panel-kit-composable-library.refactor.md`

> The task file moves between `.specs/tasks/{draft,todo,in-progress,done}/` as work progresses; if it is not at this path, resolve it by its filename under `.specs/tasks/`.

**Phase:** Phase 4
**Model:** sonnet
**Agent:** developer
**Depends on:** `05-widget-models`, `06-theme-generation`
**Parallel with:** `11-partial-projectors`, `15-tui-painters`
**Note:** None

**Goal:** Rewrite the web badge painter over core `BadgeSpec` and add the NEW web painters for charts/table/meter/status/scroll/spinner over the core widget models (design §8.2, slice 10 web half, D-7) — the web backend gains chart/table/meter/status as a NEW capability rendered from the same models the TUI already paints.

`src/badge.rs` becomes a painter over `panel_kit_core::badge::BadgeSpec` (the many data props converge on the spec; the web-only accent-color CSS-string escape hatch stays explicitly web-painter-side, §12.1.7). Core-symbol tests were already moved in step 05 — this step asserts only rendering/actions (HR-10). New painters in `src/widgets/{badge,charts,table,meter,status,scroll,spinner}.rs` consume `ContentView<'a>` borrowed data — no per-render cloning of points/rows/strings. Widget behavior must match the TUI painters' semantics for the same models (`badge_actions_match_across_backends` — action/click/hover equivalence asserted on web here and on TUI in step 15; the cross-backend assertion composes both halves).

**Review iteration 1:** Removed the web badge strip/component `BadgeSpec` clone path. The strip now renders borrowed specs directly, and badge event closures capture only the selected `BadgeAction` values needed by handlers.

**Review iteration 2:** Replaced widget SSR substring checks with complete normalized semantic snapshots for charts/gauges/flamegraph/boxplot, table/meter/status, scroll/spinner, and badge-strip groups. Painter behavior and public APIs were unchanged.

#### Expected Output

- `src/badge.rs` — painter over core `BadgeSpec`
- `src/widgets/{badge,charts,table,meter,status,scroll,spinner}.rs` — web painters
- `src/lib.rs` — widget module declarations
- SSR/unit tests

#### Success Criteria

- [X] `badge_actions_match_across_backends` (web half) passes: web badge actions (click_kind/emit_hover) behave per `BadgeSpec` semantics shared with the TUI half from step 15
- [X] Web tests assert only rendered labels/ARIA/actions (no core-symbol re-tests — those live in core since step 05)
- [X] Charts/table/meter/status painters render fixed `ContentView` data deterministically (SSR snapshots); scroll/spinner painters consume `ScrollPolicy`/`SpinnerModel`
- [X] No data-model structs introduced on the web side (painters only — CK-27)
- [X] `cargo test -p panel-kit` green

#### Subtasks

- [X] Write failing SSR tests for badge rendering/actions over `BadgeSpec` and fixed-model snapshots for charts/table/meter/status
- [X] Rewrite `src/badge.rs` as a `BadgeSpec` painter (keep accent-color escape hatch web-only)
- [X] Implement `src/widgets/{charts,table,meter,status,scroll,spinner,badge}.rs` painters over borrowed `ContentView` data
- [X] Wire widget module declarations into `src/lib.rs`
- [X] Run `cargo test -p panel-kit`

#### Blockers & Risks

| Type | Item | Impact | Likelihood | Mitigation / Resolution |
|------|------|--------|------------|-------------------------|
| Risk | Scope creep into web-native chart design beyond parity needs | Med | Med | Painters consume exactly the core models; no new model fields without a TUI/parity justification |
| Risk | Borrowed `ContentView` fights Dioxus reactivity (needs owned data in signals) | Med | Med | Models live in signals; `ContentView` borrows at render time only — same discipline as `ProjectedFrame` (CK-36 pattern) |
