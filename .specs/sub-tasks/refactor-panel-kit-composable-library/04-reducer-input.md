# Step 04: Reducer — pointer, wheel, and viewport events

**Task File:** `.specs/tasks/todo/refactor-panel-kit-composable-library.refactor.md`

> The task file moves between `.specs/tasks/{draft,todo,in-progress,done}/` as work progresses; if it is not at this path, resolve it by its filename under `.specs/tasks/`.

**Phase:** Phase 2
**Model:** sonnet
**Agent:** developer
**Depends on:** `03-reducer-commands`
**Parallel with:** `05-widget-models`, `06-theme-generation`
**Note:** None

**Goal:** Complete the pure reducer (design §4.2, slice 3): pointer move/resize/reorder paths delegating to the existing `begin_drag` (core lib.rs:736), `begin_tile_resize` (:761), `apply_drag` (:786), `reorder_tile` (:818), and dock `restore` (:843); wheel disposition (`WheelDisposition`, backend-decided `ContentConsumed` = no-op in core); and `ResizePolicy`-explicit viewport handling.

This step makes HR-1 true end-to-end: all event classes reduce through core without importing a backend. Pointer motion/hover = `ChangePhase::Continuous`; pointer-up/commands/dock-restore/accepted wheel/viewport = `Settled` (write-count table depends on this, step 10). `ViewportChanged` applies `ResizePolicy::ScaleFloating` (ratio policy) or `PreserveIntent` (viewport/profile update only); non-finite and non-positive dimensions are rejected (BVA: 0, negative, NaN). `WorkspaceEvent::Wheel { delta_y, disposition }` — `ContentConsumed` is a no-op; an accepted workspace wheel is `Settled`.

#### Expected Output

- `crates/panel-kit-core/src/reducer.rs` — pointer/wheel/viewport event variants, `WheelDisposition`, `ResizePolicy`, delegated transitions
- Unit tests in `reducer.rs`

#### Success Criteria

- [X] `reducer_replays_move_resize_reorder_and_settle_phases` passes (slice 3 first-failing test)
- [X] `reducer_preserves_wheel_precedence` passes: ContentConsumed wheel no-op; accepted workspace wheel Settled (decision table: disposition × change)
- [X] `viewport_resize_policy_is_explicit` passes: ScaleFloating applies ratio policy; PreserveIntent updates viewport/profile only; 0/negative/NaN rejected
- [X] `reducer_reuses_existing_command_and_pointer_transitions` fully green (pointer half added): same transitions as calling the five free functions directly
- [X] Invalid indices/targets (drag on unknown panel, reorder out of range) are no-ops, not panics
- [X] `cargo test -p panel-kit-core` green

#### Subtasks

- [X] Write failing tests in `crates/panel-kit-core/src/reducer.rs`: settle-phase replay, wheel precedence table, resize-policy BVA, invalid-target no-ops
- [X] Implement pointer event variants delegating to `begin_drag`/`begin_tile_resize`/`apply_drag`/`reorder_tile`; dock-restore via `restore`
- [X] Implement `WheelDisposition` handling and `ViewportChanged { size, policy }` with explicit `ResizePolicy`
- [X] Add property-style replay tests comparing reduce against direct calls of the delegated functions for each pointer class
- [X] Run `cargo test -p panel-kit-core`

#### Blockers & Risks

| Type | Item | Impact | Likelihood | Mitigation / Resolution |
|------|------|--------|------------|-------------------------|
| Risk | Settle-phase classification drifts from the design §7 write-count table | High | Med | The phase table is asserted here and re-asserted mechanically in step 10's fake-store counts |
| Risk | Viewport ratio policy subtly differs from today's web `use_workspace` scaling | Med | Med | Port the arithmetic from src/lib.rs resize path; replay one web-scale scenario as a fixed vector test |
