# Step 13: TUI composition parts and hit-test adapter

**Task File:** `.specs/tasks/todo/refactor-panel-kit-composable-library.refactor.md`

> The task file moves between `.specs/tasks/{draft,todo,in-progress,done}/` as work progresses; if it is not at this path, resolve it by its filename under `.specs/tasks/`.

**Phase:** Phase 4
**Model:** sonnet
**Agent:** developer
**Depends on:** `11-partial-projectors`
**Parallel with:** `12-web-parts`
**Note:** None

**Goal:** Port the TUI backend's composition parts out of the `TuiWorkspace` render monolith (design §6.2, slice 6 TUI half, D-4): `crates/panel-kit-tui/src/widgets/root.rs` (`draw_root`, `draw_workspace_scrollbar`), `widgets/panel.rs` (`draw_panel_surface`, `draw_panel_chrome`, `draw_traffic_lights`, `draw_resize_grip` — the `Block`/title/inner treatment ported from tui lib.rs:454-668), `widgets/dock.rs` (`draw_dock`), plus the `TuiHitBuffer` and `crates/panel-kit-tui/src/input.rs` (crossterm/ratzilla events → `WorkspaceEvent`; hit-test adapter replacing the private `Zone`/zones/dock_chips dispatch, tui lib.rs:129-142,704-812).

Native ratatui fidelity is the bar (HR-7): `Block::title`/`inner` retained; charset conversion (`Charset`, tui lib.rs:83-110) kept and reused. Painters iterate the BORROWED frame + catalog — no temporary visible/order/dock `Vec`s, no per-frame title `String`s (CK-38; the scratch vectors at tui lib.rs:402-452,628-650 are what this deletes going forward). Cell conversion consumes the same `TileGridProjection` as web (HR-5). `TuiWorkspace` still exists and still compiles (deleted in step 27) — but its internals may now delegate to the new parts where trivially possible; if rewiring the monolith is riskier than leaving it, leave it — the parts must stand alone, the controller's fate is step 27's problem. Wire `pub mod input; pub mod widgets;` into the TUI `lib.rs`.

#### Expected Output

- `crates/panel-kit-tui/src/widgets/{root,panel,dock}.rs` — draw_* parts + `TuiHitBuffer`
- `crates/panel-kit-tui/src/input.rs` — event translation + hit-test adapter
- `crates/panel-kit-tui/src/lib.rs` — module declarations; `Charset` and span/color helpers retained
- Component tests with ratatui `TestBackend`

#### Success Criteria

- [X] `tui_panel_inner_matches_projection` passes: `Block::title`/`inner` body rect equals the projected body region for fixed layouts (TestBackend)
- [X] A standalone panel draws surface + body rect with NO dock/title row when composed alone; `draw_dock` callable alone (replace-only-the-dock composition, TestBackend buffer assertions)
- [X] `web_and_tui_use_same_tile_grid_projection` (TUI half): cell conversion consumes only `TileGridProjection` values; fixed-vector cell-rect output matches web's track/placement inputs
- [X] Painters hold no owned scratch vectors: drawing over the borrowed frame allocates no `Vec` of visible/order/dock entries (asserted by allocator-counted draw test mirroring step 08's approach)
- [X] `cargo test -p panel-kit-tui` green; `cargo build --workspace` green

#### Subtasks

- [X] Write failing TestBackend tests: inner-matches-projection, standalone-surface composition, dock-alone, cell-grid fixed vectors
- [X] Port `Block`/title/inner/lights/grip drawing from tui lib.rs:454-668 into `widgets/panel.rs`; dock + scrollbar into `widgets/{dock,root}.rs`
- [X] Implement `TuiHitBuffer` + `input.rs` (crossterm/ratzilla → `WorkspaceEvent`, hit-test adapter)
- [X] Wire `pub mod input; pub mod widgets;` into TUI `lib.rs`; keep `Charset` public
- [X] Add the allocator-counted draw test proving no scratch vectors on the parts path

#### Implementation Notes

- `TuiWorkspace` remains present for Phase 8 cutover; this step added standalone composition parts and did not migrate examples or delete the controller.
- The core cell projection body now matches ratatui `Block::inner` for `ChromeMetrics::CELLS`, while `surface_only` panels still project body equal to the outer surface.
- Review iteration 1: `draw_dock` now takes a dock render context instead of repeated positional dependencies, `draw_light` uses an internal command struct, and root/dock share one TUI widget border helper.
- Review iteration 3: allocator-counted composed TUI support now calls `draw_panel_chrome`, records the header hit path, reserves caller-owned hit storage before measurement, and compares only against a borrowed native `Block::title`/`inner` baseline so panel-kit scratch/title regressions stay visible.

#### Blockers & Risks

| Type | Item | Impact | Likelihood | Mitigation / Resolution |
|------|------|--------|------------|-------------------------|
| Risk | `Block::title`/`inner` geometry drifts from the monolith | High | Med | TestBackend golden buffers transcribed from the current monolith rendering of the same layouts |
| Risk | Hit-test adapter misses zones the old dispatch covered (dock chips, resize grips) | Med | Med | Enumerate the old zone kinds (tui lib.rs:129-142,704-812) and cover each in adapter tests |
| Risk | Temptation to rewire the whole monolith early | Low | Med | Controller delegation is OPTIONAL here; standalone parts + tests are the deliverable |
