# Step 03: Reducer — snapshot and command path

**Task File:** `.specs/tasks/todo/refactor-panel-kit-composable-library.refactor.md`

> The task file moves between `.specs/tasks/{draft,todo,in-progress,done}/` as work progresses; if it is not at this path, resolve it by its filename under `.specs/tasks/`.

**Phase:** Phase 1
**Model:** sonnet
**Agent:** developer
**Depends on:** `01-panel-identity`
**Parallel with:** None
**Note:** None

**Goal:** Create `crates/panel-kit-core/src/reducer.rs` with host-owned `Snapshot<K>`, `WorkspaceEvent<K>`, `ReduceContext`, `Reduction` (`{changed, phase, focus_request}`), `ChangePhase`, and the pure `reduce()` covering the keyboard/command path by DELEGATING to the existing `command_for` (core lib.rs:296) and `apply_command` (core lib.rs:889) — never duplicating transition logic (CK-26, D-1).

`Snapshot<K>` is the superset of the web controller's 9 signals (`src/lib.rs:495-525`) and TUI state (`crates/panel-kit-tui/src/lib.rs:191-222`): panels, preferred_mode, viewport, focused, drag, tile_drag, workspace_scroll. `reduce` performs no I/O and invokes no callback (design §3.2 rule 3). `Key` events map through `command_for`; also give `PanelCommand` its NEW tagged snake_case serde (it has none today — core lib.rs:241-271) and the external Nix form for `Key` (string or `{char}`, design §10.1) — these serde additions are additive and frozen-safe (D-13). Key equality with today's behavior is proven by replaying the existing keymap test cases through `reduce`.

#### Expected Output

- `crates/panel-kit-core/src/reducer.rs` — Snapshot, WorkspaceEvent (key/command variants at minimum), ReduceContext, Reduction, ChangePhase, reduce()
- `crates/panel-kit-core/src/lib.rs` — serde derives added to `PanelCommand`/`Key`; `pub mod reducer;`
- Unit tests in `reducer.rs`

#### Success Criteria

- [X] `reduce_key_matches_command_for_and_apply_command` passes (design §15 slice 2 first-failing test — write it first, watch it fail)
- [X] `reducer_reuses_existing_command_and_pointer_transitions` (command half): reduce's key path calls `command_for` + `apply_command` — asserted behaviorally (same state transitions as calling them directly), not by grepping source
- [X] Commands produce `ChangePhase::Settled`; `Reduction.changed` is false for no-op commands
- [X] `cargo test -p panel-kit-core` green; existing keymap tests unchanged and passing

#### Subtasks

- [X] Write the failing test `reduce_key_matches_command_for_and_apply_command` in `crates/panel-kit-core/src/reducer.rs`
- [X] Implement `Snapshot<K>` (from_defaults constructor matching web/TUI default seeding), `WorkspaceEvent` key/command variants, `ReduceContext`, `Reduction`, `ChangePhase`
- [X] Implement `reduce()` command path delegating to `command_for`/`apply_command`; invalid command targets are no-ops, not panics
- [X] Add serde: `PanelCommand` tagged snake_case; `Key` external string/`{char}` form (additive only — existing spellings untouched, D-13)
- [X] Add tests: no-op command reports `changed: false`; Snapshot::from_defaults matches `merge_defaults` seeding; add `pub mod reducer;` to core `lib.rs`

#### Blockers & Risks

| Type | Item | Impact | Likelihood | Mitigation / Resolution |
|------|------|--------|------------|-------------------------|
| Risk | Snapshot field set misses a TUI-only or web-only field, forcing later reducer rework | Med | Med | Derive the field list from both controllers (src/lib.rs:495-525, tui lib.rs:191-222) as cited in the analysis; document each field's origin in a comment |
| Risk | New serde on `PanelCommand` accidentally changes existing `PanelKind`/layout wire forms | High | Low | Additive derives only; run the existing v1-migrate/reconcile tests which pin `SavedLayoutV2` round-trips |

#### Notes (implementation)

- Slice-2 red captured before implementation: with the type contract in place and `reduce` stubbed to return `Reduction::unchanged()`, `reduce_key_matches_command_for_and_apply_command`, `reducer_reuses_existing_command_and_pointer_transitions`, and `reduce_replays_existing_keymap_cases` all failed with real assertion divergences ("panels diverged"), then went green on the delegated implementation.
- Snapshot field origins documented as a table on the struct (web signal ↔ TUI field per field); `profile`/`surface` is `ReduceContext` input, web `pending_dock_focus` maps to `Reduction::focus_request` (reported, not stored).
- The key path is strict delegation: `command_for` → `apply_command` on the focused panel, matching both controllers' command handoff. The TUI's focus-adoption-from-context and the web minimize dock-chip focus stay host policies (documented on `reduce_key`); the pointer/dock paths that will report `focus_request` land in step 04.
- Review iteration 2: split the reducer's public state/event/report types into `reducer/types.rs` and split unit tests into `reducer/tests/{fixtures,key_path,command_path,snapshot,serde}.rs`, leaving `reducer.rs` focused on dispatch and command delegation. Each reducer file is now under 200 lines.
- Review iteration 2: replaced the per-command full `snapshot.panels.clone()` change detector with an `apply_command` wrapper that snapshots only the selected `PanelWin<K>` plus copyable `mode`/`focused` fields before delegation.
- Additive core changes required by delegation: `#[derive(Clone, Copy)]` on `Clamp` (`ReduceContext` holds `&Clamp`, `apply_command` takes it by value) and `Debug` on `Mode`/`DragKind`/`Drag` (snapshot-state diagnostics). `Units`/`Mode`/`WinState` serde spellings untouched; existing v1-migrate/reconcile keymap tests pass unmodified.
