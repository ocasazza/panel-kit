# Step 26: Views feature resolution — host-owned example, web hook deletion (D-11)

**Task File:** `.specs/tasks/todo/refactor-panel-kit-composable-library.refactor.md`

> The task file moves between `.specs/tasks/{draft,todo,in-progress,done}/` as work progresses; if it is not at this path, resolve it by its filename under `.specs/tasks/`.

**Phase:** Phase 5
**Model:** sonnet
**Agent:** developer
**Depends on:** `09-persistence-core`, `10-save-policy`, `12-web-parts`
**Parallel with:** `16-workspace-spec`, `25-example-migrations`
**Note:** None

**Goal:** Execute decision D-11 (resolves the analysis-flagged open decision the design omitted): DELETE the web `use_views`/`Views` controller composite (`src/views.rs` — it embeds `Workspace<K>` and imports `use_workspace_state`/`save_layout`/`load_layout` at src/views.rs:29, all in the §12.1 ledger as amended by D-11/12.1.14), RETAIN the core `SavedViews` registry/key-scheme (`crates/panel-kit-core/src/views.rs:72-176` — untouched), and REWRITE `examples/views.rs` as a host-owned composition: per-view `LocalStorageLayoutStore` at the existing `view_layout_key` scheme + `restore_snapshot`/`persist_snapshot` + explicit `SavePolicy` + parts loop.

The example keeps the documented UX (enumerate/switch/create/rename/delete views over the same storage-key scheme `{base}:views` / `{base}:view:{name}`, legacy-key migration copy preserved via the core registry helpers) but the view-switcher state becomes application state (signals owned by the example), not a library controller composite. Update README §127-144 (named-views section) to the composable wording, and add the D-11 entry to MIGRATION.md (the full cutover section is step 27; this step adds only the views-specific break: `use_views`/`Views` removed, core registry retained, example shows the pattern). Remove `mod views;` from `src/lib.rs` (module declaration removal only — no other lib.rs edits; the controller itself is step 27's).

#### Expected Output

- `src/views.rs` — DELETED
- `src/lib.rs` — `mod views;`/`pub use views::*;` declarations removed
- `examples/views.rs` — host-owned rewrite over core registry + stores + parts
- `README.md` §127-144 — composable named-views wording
- `MIGRATION.md` — D-11 views entry

#### Success Criteria

- [X] No `use_views`/`Views` web type exists; `cargo build -p panel-kit` green with the module removed
- [X] Core `crates/panel-kit-core/src/views.rs` byte-untouched (git diff empty)
- [X] The rewritten example preserves the storage-key scheme and the legacy pre-views layout migration copy (core `SavedViews` behavior; example keeps first-run copy semantics)
- [ ] Example builds for wasm32; view switching persists per-view layouts under the same keys as before
  - Implementation preserves per-view keys and switching persistence; focused wasm example check is blocked before this example by existing `panel-kit-tui` wasm compile errors in `crates/panel-kit-tui/src/input.rs` (`KeyCode::BackTab`, `KeyEvent::meta`).
- [X] README §127-144 documents the host-owned pattern; MIGRATION.md records the break and replacement

#### Subtasks

- [X] Rewrite `examples/views.rs` host-owned (registry signals + per-view store + SavePolicy + parts loop)
- [X] Delete `src/views.rs`; remove its declarations from `src/lib.rs`
- [X] Update README §127-144 and add the MIGRATION.md D-11 entry
- [ ] Build the example for wasm32; verify the core views.rs diff is empty
  - `cargo check --example views --target wasm32-unknown-unknown` blocked in `panel-kit-tui` dev-dependency before checking this example; `git diff -- crates/panel-kit-core/src/views.rs` is empty.

#### Implementation Notes

- Files changed: `examples/views.rs`, `src/lib.rs`, `src/views.rs` (deleted), `README.md`, `MIGRATION.md`, this sub-task file.
- Phase 5 review fix files changed: `examples/views.rs`, `src/lib.rs`, this sub-task file.
- Phase 5 review fix:
  - Registry persistence now returns a user-visible error string instead of silently ignoring `LocalStorage::set` failures.
  - Rename/delete layout-record moves and removes now return storage failures through the same demo error-reporting signal used by nearby view-registry actions.
  - `src/lib.rs` crate docs now describe host-owned `SavedViews`/per-view store composition and no longer reference the deleted `use_views` API.
- Phase 5 re-review 1 fix files changed: `examples/views.rs`, this sub-task file.
- Phase 5 re-review 1 fix:
  - `persist_view_snapshot` now returns a user-visible `Result<(), String>` and `switch_view` stops before activating the next view when the outgoing layout write fails, letting the existing `report(err, ...)` path surface the failure.
  - Legacy first-run migration now propagates destination-read, legacy-read, and copy failures as user-visible storage messages instead of discarding read errors or only logging copy errors.
  - Focused coverage now checks layout error strings include the action, storage key, and underlying cause.
- Phase 5 re-review 2 fix files changed: `examples/views.rs`, this sub-task file.
- Phase 5 re-review 2 fix:
  - Reducer save failures now set the demo error signal through `report(err, ...)` with the formatted layout action, per-view storage key, and underlying `LayoutError`, instead of console logging only.
  - Reset clear failures now use the same user-visible layout failure path before the host snapshot is reset.
  - V1 migration seeding failures now report a formatted raw storage error with action, view layout key, and cause instead of storing the backend error string directly.
  - Focused coverage now checks layout and raw-storage failures are shaped for the visible `report(err, ...)` path.
- Verification:
  - `cargo check --example views` — passed; only existing sibling Step 16 core warnings emitted.
  - `cargo test --example views` — passed, 4 tests.
  - `grep "use_views|Views<K>" src/lib.rs` — no matches.
  - `grep "let _ = .*(save_registry|move_layout_record|LocalStorage|storage\\.)" examples/views.rs` — no matches.
  - Phase 5 re-review 1 verification: `cargo test --example views` — passed, 4 tests; direct `nix develop .#rust --command cargo test --example views` was not available because the flake exposes no `.#rust` devshell on this system.
  - Phase 5 re-review 2 verification: `cargo test --example views` — passed, 6 tests.
  - Previous Step 26 verification: `cargo build -p panel-kit` passed (only sibling Step 16 core warnings emitted); `cargo check -p panel-kit --lib --target wasm32-unknown-unknown` passed (only sibling Step 16 core warnings emitted); `cargo check --example views --target wasm32-unknown-unknown` remained blocked by unrelated `panel-kit-tui` wasm errors; `git diff -- crates/panel-kit-core/src/views.rs` was empty.

#### Blockers & Risks

| Type | Item | Impact | Likelihood | Mitigation / Resolution |
|------|------|--------|------------|-------------------------|
| Risk | Storage-key scheme drift loses users' saved views | High | Low | Use the core `views_registry_key`/`view_layout_key` helpers verbatim; keep the legacy-copy migration branch |
| Risk | Deleting the module breaks hidden internal references in src/lib.rs | Med | Low | Compiler is the proof; grep `crate::views` before deleting and migrate any non-controller reference |
| Risk | README edit collides with step 27's README §48-77 rewrite | Low | Low | Different sections; step 27 runs in a later phase |
