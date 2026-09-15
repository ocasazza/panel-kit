# Step 09: Persistence promoted to core

**Task File:** `.specs/tasks/todo/refactor-panel-kit-composable-library.refactor.md`

> The task file moves between `.specs/tasks/{draft,todo,in-progress,done}/` as work progresses; if it is not at this path, resolve it by its filename under `.specs/tasks/`.

**Phase:** Phase 3
**Model:** sonnet
**Agent:** developer
**Depends on:** `04-reducer-input`
**Parallel with:** `07-frame-projection`
**Note:** None

**Goal:** Promote persistence to a core port (design §7, slice 7, D-5): `crates/panel-kit-core/src/persist.rs` with the `LayoutStore` trait (load/save/**clear**), `RestoreContext`, `restore_snapshot`, `persist_snapshot`, and the stable-ID codec over `PanelCatalog` — orchestrating the existing `migrate_v1` (core lib.rs:1038), `reconcile_units` (:1055), `merge_defaults` (:1080) instead of duplicating schema logic. Add the two backend adapters: `src/store.rs` (`LocalStorageLayoutStore::new(impl Into<String>)`, gloo adapter replacing src/lib.rs:141,447-485 internals) and `crates/panel-kit-tui/src/store.rs` (`JsonFileLayoutStore` — the FileLayoutStore from tui lib.rs:157-171 made public at its new path). Delete the TUI-local `LayoutStore` trait + `FileLayoutStore` (§12.1.3/12.1.4) and rewire `TuiWorkspace` to the core trait + new adapter WITHOUT changing TuiWorkspace behavior (the controller itself dies in step 27). Core `views.rs` (`SavedViews` registry, key scheme) stays untouched — it is already renderer-neutral.

Wire-compat invariants frozen (D-13): `Units`="CssPx"/"Cells", `Mode`="Floating"/"Tiling", `WinState` spellings, stable IDs = current serde variant names. Strict errors replace the TUI's silent `.ok()` drop of invalid saved layouts (tui lib.rs:247): invalid JSON/future version return errors WITHOUT overwriting defaults; invalid saved viewport fails before division.

#### Expected Output

- `crates/panel-kit-core/src/persist.rs` — LayoutStore(+clear), RestoreContext, restore_snapshot, persist_snapshot, stable-ID codec
- `src/store.rs` (NEW) — LocalStorageLayoutStore
- `crates/panel-kit-tui/src/store.rs` (NEW) — JsonFileLayoutStore
- `crates/panel-kit-tui/src/lib.rs` — local trait/FileLayoutStore deleted; TuiWorkspace rewired; `pub mod store;`
- `crates/panel-kit-core/src/lib.rs` — `pub mod persist;`
- Unit/integration tests

#### Success Criteria

- [X] `core_store_restores_v1_reconciles_merges_saves_v2` passes (slice 7 first-failing test): V1 migration, unit reconciliation, stable-ID mapping both directions, defaults merge, V2 encode
- [X] `store_clear_is_observable` passes: clear visible; unknown saved IDs ignored; newly declared IDs appended
- [X] Error paths: invalid JSON / future version → error, defaults untouched; invalid saved viewport → error before division
- [X] `LayoutStore` exists ONLY in core (no TUI-local declaration); serde spellings byte-identical to today (round-trip tests against fixtures captured from current saved layouts)
- [X] `cargo test -p panel-kit-core -p panel-kit-tui` green; `cargo build --workspace` green (TuiWorkspace compiles against core trait)

#### Subtasks

- [X] Write the failing test `core_store_restores_v1_reconciles_merges_saves_v2` in `crates/panel-kit-core/src/persist.rs`
- [X] Implement the trait (+`clear`), codec, and lifecycle orchestration over `migrate_v1`/`reconcile_units`/`merge_defaults`
- [X] Write `src/store.rs` (gloo localStorage adapter) and `crates/panel-kit-tui/src/store.rs` (JSON file adapter, public)
- [X] Delete TUI-local trait + FileLayoutStore (tui lib.rs:144-171); rewire TuiWorkspace + the TUI example store usage; add `pub mod store;`/`pub mod persist;`
- [X] Tests: clear observability, error paths (invalid JSON, future version, invalid viewport), serde-spelling fixtures, unknown/new panel IDs

#### Review Iteration 1 Notes

- [X] Persistence errors are now observable: web `save_layout`/`load_layout` return `LayoutError` and caller hooks report failures to the browser console; TUI save/event/restore paths return `Result<_, LayoutError>`.
- [X] Stable-ID catalog construction now uses `PanelCatalog::from_panel_kind_layout` in core, shared by web and TUI callers.


#### Review Iteration 2 Notes

- [X] Split the oversized persistence proof file into `persist/tests.rs` plus cohesive `persist/tests/common.rs`, `persist/tests/lifecycle.rs`, and `persist/tests/save_policy.rs` modules; each module is under 200 lines.
- [X] Replaced substring-based wire-spelling checks with normalized full JSON fixture equality for default V2 saves and V1-restored V2 saves.

#### Blockers & Risks

| Type | Item | Impact | Likelihood | Mitigation / Resolution |
|------|------|--------|------------|-------------------------|
| Risk | Stable-ID mapping breaks existing saved layouts (user data) | High | Low | Round-trip fixtures from real current saves; IDs = current serde variant names (D-13); `mkLayout` output must decode identically |
| Risk | Rewiring TuiWorkspace while it still exists introduces behavior drift | Med | Med | TuiWorkspace is deleted in step 27; here it must merely compile and keep its current tests (if any) green — adapter is behavior-compatible |
| Risk | `clear()` semantics ambiguous (clear vs delete record) | Med | Low | One store instance = one logical record; clear empties that record; step 10's reset-clear test pins the observable effect |
