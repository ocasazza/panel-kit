# Step 27: Controller deletion and cutover documentation

**Task File:** `.specs/tasks/todo/refactor-panel-kit-composable-library.refactor.md`

> The task file moves between `.specs/tasks/{draft,todo,in-progress,done}/` as work progresses; if it is not at this path, resolve it by its filename under `.specs/tasks/`.

**Phase:** Phase 8
**Model:** opus
**Agent:** developer
**Depends on:** `22-web-canary-example`, `23-tui-native-canary`, `24-browser-tui-canary`, `25-example-migrations`, `26-views-cutover`
**Parallel with:** None
**Note:** None

**Goal:** Execute the clean cutover (design §12, slice 18, HR-22, D-11): delete BOTH controllers with every in-repo consumer already migrated (steps 22–26), leaving no compatibility facade, deprecated alias, or old-path re-export (§16.10).

Web (`src/lib.rs`): delete `Workspace<K>` (lib.rs:495-532), `use_workspace` (:550-569), `use_workspace_state` (:575-751), private `save_layout`/`load_layout` (:447-485), and the controller impl block (:753-1405 incl. render_with_header/header/dock). Keep: CSS/BOOT_CSS/BOOT_HTML (:169-183), `LoadingWorkspace` (:195-246), `PanelHeaderButton` (:254-287), `Spinner` (:1426-1440), `tip_pos` (:1449-1472), `is_editing` (:393-402), boot_contract_tests (:1474-1638). Rewrite the crate docs quick-start (:34-84) to the composable API. TUI (`crates/panel-kit-tui/src/lib.rs`): delete `TuiWorkspace` (:191-831) and `Zone` (:129-142) — the backend-local trait already moved in step 09; keep `Charset` (:83-110) and helpers. Verify retained-but-untouched files: `src/editor.rs` (web-only Monaco widget — RETAIN per ledger), `examples/badge.rs`, `examples/spinner.rs` (component-only examples — RETAIN), core `views.rs`/`loading.rs`, `nix/lib/mkLayout.nix`. Fix doc links referencing deleted items in `src/loading.rs:33,181` (`Workspace::root_class` references).

Docs: README §48-77 rewritten to composable adoption wording (§12.1.13); MIGRATION.md gains the full major-version cutover section (both controllers, `LayoutStore` move to core, `Theme` defaults move, widget model moves, D-11 views break — clean-cutover style, no shims, pin-then-migrate per PRODUCT.md:111-118); CHANGELOG.md release notes. Cutover proofs rely on COMPILATION and observable behavior — no source-text tests (CK-35).

#### Expected Output

- `src/lib.rs` — controller fully deleted, docs rewritten, module surface clean
- `crates/panel-kit-tui/src/lib.rs` — `TuiWorkspace`/`Zone` deleted
- `src/loading.rs` — doc-link fixes
- `README.md`, `MIGRATION.md`, `CHANGELOG.md` — cutover documentation
- Regression proof that old controller usage no longer compiles

#### Success Criteria

- [X] Controller cutover builds/checks green under this assignment's focused-check constraint; obsolete controller tokens are grep-clean outside `MIGRATION.md` before/after snippets (`src`, `crates`, `examples`, `README.md`, `CHANGELOG.md`)
- [X] A doctest compile assertion proves old controller usage no longer compiles (`src/lib.rs` crate docs); rewritten doctests compile against the new surface
- [X] Retained files verified untouched except documented doc-links: `src/editor.rs`, `examples/badge.rs`, `examples/spinner.rs`, core `views.rs`/`loading.rs`, `nix/lib/mkLayout.nix`
- [X] README/MIGRATION/CHANGELOG document the cutover incl. D-11 and the consumer pin policy; no stale adoption wording (§12.1.13)
- [ ] Full project-wide suite (`cargo test --workspace`, `nix flake check`) intentionally deferred to the main agent per Phase 8 orchestration; focused cutover checks passed below

#### Subtasks

- [X] Delete the web controller block from `src/lib.rs` (listed ranges); rewrite crate-doc quick-start; keep the enumerated retained items
- [X] Delete `TuiWorkspace`/`Zone` from `crates/panel-kit-tui/src/lib.rs`; keep `Charset`/helpers
- [X] Fix `src/loading.rs:33,181` doc links; verify retained-file diffs are empty (editor.rs, badge/spinner examples, core views/loading, mkLayout)
- [X] Rewrite README §48-77; write the MIGRATION.md cutover section; update CHANGELOG.md
- [X] Add the no-longer-compiles proof (doctest asserting the removed API is absent via compilation of the new surface); run focused cutover checks

#### Completion Notes

Files changed for this step:

- `src/lib.rs` — removed the web controller, persistence helpers, controller-only render/input helpers, and old quick start; retained `CSS`, `BOOT_CSS`, `BOOT_HTML`, `LoadingWorkspace`, `PanelHeaderButton`, `Spinner`, `tip_pos`, `is_editing`, and boot contract tests.
- `crates/panel-kit-tui/src/lib.rs` — removed `TuiWorkspace`, `Zone`, controller tests, and controller-only imports; retained `Charset`, `rect_from_region`, and `write_header_text` for backend parts.
- `src/loading.rs` — updated loading doc links from the removed root-class method to `widgets::root::root_class`.
- `README.md`, `MIGRATION.md`, `CHANGELOG.md` — updated adoption, migration, and release wording for the clean major-version cutover, D-11 named views removal, core `LayoutStore`, core theme defaults, core widget models, and pin-then-migrate policy.
- `crates/panel-kit-core/src/reducer.rs`, `crates/panel-kit-core/src/reducer/types.rs`, `src/widgets/root.rs`, `crates/panel-kit-core/src/badge_tests.rs`, `crates/panel-kit-tui/src/badge.rs` — removed stale controller-name references from library sources/tests without changing behavior.

Verification evidence:

- `cargo test --doc -p panel-kit --features web-runtime` — 6 passed, including the clean-cutover compile-fail doctest for the removed controller type.
- `cargo test --doc -p panel-kit-tui --features spec-plan` — 1 passed, 1 ignored.
- `cargo check -p panel-kit --lib --features web-runtime`.
- `cargo check -p panel-kit --features web-runtime --example badge --example spinner --example theming --example loading --example loading_workspace --example editor --example views` — passed with pre-existing dead-code warnings from shared example support.
- `PANEL_KIT_WORKSPACE_SPEC=/tmp/panel-kit-workspace-canary.json cargo check -p panel-kit --features web-runtime --example workspace` — passed (compile-time include stub only).
- `cargo check -p panel-kit-tui --lib --features spec-plan`.
- `cargo check -p panel-kit-tui --features spec-plan --example workspace --example browser_tui`.
- `cargo test -p panel-kit --lib --features web-runtime` — 25 passed.
- `cargo test -p panel-kit-core badge` — 7 passed, 64 filtered.
- `cargo test -p panel-kit-tui --lib tui_badge` — 2 passed, 4 filtered.
- `grep` for `use_workspace`, `Workspace<`, `TuiWorkspace`, `use_views`, `save_layout`, and `load_layout` is clean outside `MIGRATION.md`.
- `git diff --name-only -- src/editor.rs examples/badge.rs examples/spinner.rs crates/panel-kit-core/src/views.rs crates/panel-kit-core/src/loading.rs nix/lib/mkLayout.nix` returned no output.

#### Blockers & Risks

| Type | Item | Impact | Likelihood | Mitigation / Resolution |
|------|------|--------|------------|-------------------------|
| Risk | Hidden in-repo consumer of a deleted item breaks the build late | Med | Low | Steps 22–26 removed all known consumers; the compiler enumerates any remainder — fix forward, never re-add a facade |
| Risk | Deletion accidentally removes a retained helper (BOOT_CSS, PanelHeaderButton, tip_pos, is_editing) | High | Low | The retained list is explicit above; boot_contract_tests (:1474-1638) must stay green |
| Risk | Docs drift (README promises something the new API lacks) | Med | Med | Quick-start text must mirror the actual examples (steps 22/25 patterns) |
