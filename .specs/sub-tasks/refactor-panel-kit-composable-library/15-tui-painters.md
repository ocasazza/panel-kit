# Step 15: TUI painters over core models

**Task File:** `.specs/tasks/todo/refactor-panel-kit-composable-library.refactor.md`

> The task file moves between `.specs/tasks/{draft,todo,in-progress,done}/` as work progresses; if it is not at this path, resolve it by its filename under `.specs/tasks/`.

**Phase:** Phase 4
**Model:** sonnet
**Agent:** developer
**Depends on:** `05-widget-models`, `06-theme-generation`
**Parallel with:** `11-partial-projectors`, `14-web-painters`
**Note:** None

**Goal:** Convert the TUI widget painters to consume the core widget models and delete the TUI-local data declarations (design §8.2, slice 10 TUI half, §12.1.8, D-7): `crates/panel-kit-tui/src/charts.rs` (delete `Series/GaugeItem/FlameSpan/BoxItem` + keep drawing fns, models moved in step 05 from lines 29-362), `table.rs`, `meter.rs` (delete `bar` data fn, keep painting), `status.rs`, `scroll.rs` (delete `wrap`/`max_offset` data fns), `badge.rs` (delete the data-bearing `Badge` struct at lines 13-102; painter over core `BadgeSpec`; keep spans/color helpers).

Native drawing stays (ratatui chart/gauge/table/scrollbar widgets, §12.2 "explicitly retain"). The badge TUI half of `badge_actions_match_across_backends` lands here. `TuiWorkspace` and the TUI canary examples continue to compile: their data construction switches from TUI-local structs to the core models (same values — the canary's provider functions return core-typed data after this step; that also prepares step 20's provider-manifest extraction). Keep `Charset` and span helpers public.

#### Expected Output

- `crates/panel-kit-tui/src/{badge,charts,table,meter,status,scroll}.rs` — painters only, data declarations deleted
- TUI canary example data switched to core model types (values unchanged)
- Unit/component tests

#### Success Criteria

- [X] No TUI-local widget data struct or data function remains (`Series`, `Badge` data struct, `bar`, `wrap`, `max_offset`, `five_num` deleted from TUI; native drawing functions all retained)
- [X] `badge_actions_match_across_backends` (TUI half) passes: badge click/hover semantics per `BadgeSpec`, mirroring step 14's web half
- [X] TUI canary rendering is pixel-identical before/after the model switch (TestBackend buffer comparison on the same provider data)
- [X] `cargo test -p panel-kit-tui` and `cargo build --workspace` green

#### Subtasks

- [X] Write failing tests: badge-actions equivalence, TestBackend buffer equality before/after model switch for the canary panels
- [X] Rewrite each TUI widget module as painter-only over core models; delete local data declarations
- [X] Switch `crates/panel-kit-tui/examples/workspace_canary.rs` provider functions to return core model types (values byte-identical)
- [X] Run `cargo test -p panel-kit-tui && cargo build --workspace`


Implementation note: TUI composition/controller scratch vectors remain untouched because this step explicitly excludes `13-tui-parts` and controller cutover; the widget painters and canary provider data now consume core models directly. Phase 4 review iteration 2 strengthened the canary proof with full provider buffer identity across badges, time-series, gauges, flamegraph, node table/status/meter cells, and boxplot, and split the oversized chart painter into focused chart-family modules.

#### Blockers & Risks

| Type | Item | Impact | Likelihood | Mitigation / Resolution |
|------|------|--------|------------|-------------------------|
| Risk | Rendering changes from model switch (field order/defaults) | Med | Low | Buffer-equality test against pre-switch rendering pins pixel identity |
| Risk | workspace_canary.rs edits collide with later steps (20/23/24) | Low | Low | This step touches ONLY the provider functions' return types; geometry/Panel enum untouched until steps 23/24 |
