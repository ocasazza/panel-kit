# Step 28: jump-cannon consumer migration (two independent workspaces)

**Task File:** `.specs/tasks/in-progress/refactor-panel-kit-composable-library.refactor.md`

> The task file moves between `.specs/tasks/{draft,todo,in-progress,done}/` as work progresses; if it is not at this path, resolve it by its filename under `.specs/tasks/`.

**Phase:** Phase 8
**Model:** sonnet
**Agent:** developer
**Depends on:** `27-controller-cutover`
**Parallel with:** `29-apple-notes-migration`
**Note:** None

**Goal:** Migrate the external git-pinned consumer **jump-cannon** to the composable API per design §13.2 (HR-20): two concurrent workspaces sharing NO state or store, preserved storage keys/serde IDs/resize behavior, input priority for the app's own handlers, and a wasm build proof (`jump-cannon-app-ui-wasm`) BEFORE any pin update.

FIRST resolve the checkout: locate the jump-cannon working copy (sibling repo, flake/git pin reference in this repo's docs, or the analyst's cited path — analysis cites `app/ui/Cargo.toml:10-13`, `app/ui/src/main.rs:916-917,889-939,1344-1385`). Inventory actual call sites before editing; if no accessible checkout exists in this environment, STOP and report the completed inventory recipe + migration patch plan as the deliverable with an explicit blocker note (do not fabricate). Migration follows design §13.2's ten steps and its "After" shape: one shared `PanelCatalog<Panel>`, two independent `Snapshot` signals + `ProjectionBuffer`s + `LocalStorageLayoutStore`s (keys `WORKSPACE_LAYOUT_KEY`/`SESSIONS_LAYOUT_KEY` preserved), `OPEN_PANEL` → `reduce(... Command { target, command: Restore })`, app handlers first / panel input second, `observe_viewport` → `ViewportChanged` with `ScaleFloating`, host render loop over parts, `SavePolicy::OnSettle` after reductions, badges → `BadgeSpec`. Build the app's wasm target against the migrated panel-kit (path/git override to this repo's revision), then update the pin.

#### Expected Output

- jump-cannon `app/ui` migrated (external repo)
- `two_concurrent_workspaces_do_not_share_state_or_store` test (in jump-cannon's test suite or as an integration check it runs)
- `jump-cannon-app-ui-wasm` build proof (build log/artifact reference)

#### Success Criteria

- [X] `two_concurrent_workspaces_do_not_share_state_or_store` passes: independent snapshots/scratch/stores; storage keys and serde IDs byte-preserved from the pre-migration source
- [X] App input priority preserved: palette/graph/camera/editor handlers consume events first; only unconsumed events reach panel input/reduce
- [X] Resize behavior preserved via explicit `ResizePolicy::ScaleFloating` (ratio scaling matches the pre-migration web behavior)
- [X] The app's supported wasm target builds (`jump-cannon-app-ui-wasm`) against the post-cutover panel-kit BEFORE the pin update; pin updated only after the build is green. Local path override used because panel-kit's post-cutover working tree is not yet committed (`HEAD` is still `d718ee0064974e0635dbc24f912ef013e53e4ece`).
- [X] No `use_workspace`/`Workspace<`/`render_with_header`/`.dock()` references remain in jump-cannon sources

#### Subtasks

- [X] Locate the jump-cannon checkout; inventory call sites, storage keys, panel serde IDs, and its wasm target (record findings)
- [X] Apply the design §13.2 ten-step migration in jump-cannon's `app/ui`
- [X] Add `two_concurrent_workspaces_do_not_share_state_or_store` (independent stores/scratch; same keys/IDs)
- [X] Build the wasm target against this repo's revision; update the git pin only after green; record the build proof. True git-rev pin advancement remains blocked until the post-cutover panel-kit changes have a committed revision; the consumer currently builds through the workspace `[patch]` path override.

#### Blockers & Risks

| Type | Item | Impact | Likelihood | Mitigation / Resolution |
|------|------|--------|------------|-------------------------|
| Blocker | jump-cannon checkout not accessible from this environment | High | Med | Inventory-first subtask; if absent, deliver the patch plan + blocker report — never fabricate migration claims |
| Risk | Signal-equality reactivity (main.rs:1344-1358 pattern) breaks with borrowed frames | Med | Med | Follow design §13.2 step 3: equality on signal/reference identity; frames are render-scoped (CK-36 pattern) |
| Risk | Storage-key/ID drift silently wipes user layouts | High | Low | Keys/IDs copied verbatim from inventory; the two-workspace test asserts them |

#### Implementation Notes

- jump-cannon checkout resolved at `../jump-cannon`; `app/ui` owns the migrated call sites.
- Inventory findings preserved: layout keys remain `jc_layout_v9` and `jc_sessions_layout_v1`; panel serde IDs remain the `Panel` enum variant names (`Graph`, `Nodes`, `Inspector`, `Document`, `Progress`, `Settings`, `Help`, `Filter`, `Metrics`, `Instances`, `Generate`, `Timeline`, `Debug`, `Worlds`, `History`, `Branches`, `Merge`, `GitHub`, `GpuSessions`, `Importers`); supported wasm proof is `cargo build -p jump-cannon-ui --target wasm32-unknown-unknown` run inside the jump-cannon nix devshell.
- `nix build .#app-web` was attempted and fails before Rust compilation because the local path override points outside the Nix `appSrc` sandbox; a true git rev pin is required before the reproducible flake build can consume these local panel-kit changes.
- Phase 8 review hygiene update: stale `Workspace::restore` wording was removed from `../jump-cannon/app/ui/src/main.rs` and `../jump-cannon/app/ui/src/palette.rs`; comments now describe the host-owned reducer restore command and the `OPEN_PANEL` drain path.
- Extracted Jump Cannon's consumer-local workspace plumbing from `../jump-cannon/app/ui/src/main.rs` into `../jump-cannon/app/ui/src/workspace.rs` plus `workspace/{catalog,events,state}.rs`; the module remains `pub(crate)` only and preserves app-first input priority, `SavePolicy::OnSettle`, `ResizePolicy::ScaleFloating`, storage keys, panel stable IDs, independent scratch buffers, and the existing badge migration behavior.
- Focused verification after the review fixes:
  - `grep` over `../jump-cannon/app/ui/src/{main.rs,palette.rs,workspace.rs,workspace}` found no `Workspace::restore`, `use_workspace`, `Workspace<`, `render_with_header`, `.dock()`, or `restore+raise` references.
  - `cargo test -p jump-cannon-ui two_concurrent_workspaces_do_not_share_state_or_store` from `../jump-cannon/app` passed: 1 test, 33 filtered; existing unrelated warnings remain in `api.rs`/`github.rs`.
  - `nix develop -c bash -lc 'cd app && cargo build -p jump-cannon-ui --target wasm32-unknown-unknown'` from `../jump-cannon` passed after entering the Nix devshell (direct host cargo lacked `lld`).
  - Phase 8 re-review 1 fix: consumer-local reducer helper renamed to `reduce_and_persist_workspace_event`, so every app-first input handoff now advertises the `SavePolicy::OnSettle`/`LocalStorageLayoutStore` persistence side effect at the call site.
  - Phase 8 re-review 1 fix: source-status polling hiccups in the graph boot loop and Importers apply tracker now emit deduplicated non-fatal `tracing::warn!` diagnostics via `api::source_status_poll_warning(...)` instead of silent `Err(_) => {}` arms; polling still continues and terminal failed/serving statuses keep their prior behavior.
  - Focused verification for Phase 8 re-review 1:
    - `cargo test -p jump-cannon-ui source_status_poll_warning_names_source_and_error_without_failing_the_wait` from `../jump-cannon/app` passed: 1 test, 34 filtered; existing unrelated warnings remain.
    - `cargo test -p jump-cannon-ui two_concurrent_workspaces_do_not_share_state_or_store` from `../jump-cannon/app` passed: 1 test, 34 filtered; storage keys, stable IDs, independent scratch/store, and `ResizePolicy::ScaleFloating` remain covered.
    - `nix develop -c bash -lc 'cd app && cargo build -p jump-cannon-ui --target wasm32-unknown-unknown'` from `../jump-cannon` passed; Nix printed the pre-existing dirty-tree/input override warnings before the successful wasm compile.
    - `grep` over `../jump-cannon/app/ui/src/workspace` found no remaining `reduce_workspace_event` references, and `grep` over the edited status pollers found no remaining empty source-status `Err(_) => {}` arms.
