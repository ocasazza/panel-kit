# Step 11: Fully specified partial projectors

**Task File:** `.specs/tasks/todo/refactor-panel-kit-composable-library.refactor.md`

> The task file moves between `.specs/tasks/{draft,todo,in-progress,done}/` as work progresses; if it is not at this path, resolve it by its filename under `.specs/tasks/`.

**Phase:** Phase 4
**Model:** sonnet
**Agent:** developer
**Depends on:** `08-frame-proofs`
**Parallel with:** `14-web-painters`, `15-tui-painters`
**Note:** None

**Goal:** Add the partial projectors to `crates/panel-kit-core/src/frame.rs` (design §5.3, slice 6 core half): `PanelProjectionInput<'_>`, `project_panel(&PanelWin<K>, source_index, PanelProjectionInput) -> Option<PanelProjection<K>>`, `project_chrome`, and `project_dock_into` — each requiring NO `Snapshot`, catalog, store, or dock (HR-2, HR-6). Also the standalone-surface switch: `ChromeSpec::surface_only()` (the spec type lands in step 16; here express it as the frame-level chrome-configuration input, e.g. a `ChromeProjectionInput` or the core-side equivalent the parts consume) that disables every chrome/dock part so a lone `panel_surface` has no implicit chrome.

`project_panel` is the function a host calls to render ONE panel surface anywhere (a floating window, a cell in its own layout, a sidebar) — it must be constructible from panel + geometry inputs alone. The partial projectors share the same record types and math as `project_into` (no second geometry implementation — CK-27).

#### Expected Output

- Partial projector API in `crates/panel-kit-core/src/frame.rs`
- Chrome-configuration input (surface-only mode) consumed by parts in steps 12/13
- Unit tests

#### Success Criteria

- [X] `project_panel_requires_no_snapshot_catalog_store_or_dock` passes (slice 6 first-failing test — write it first): constructible from a `PanelWin` + geometry inputs only; signature has no Snapshot/catalog/store/dock parameter
- [X] `standalone_surface_has_no_implicit_chrome` passes: surface-only chrome input yields a projection with every chrome/dock element disabled (unit-level, backend-agnostic)
- [X] Partial projector outputs are consistent with `project_into` for the same panel (fixed-vector equivalence test)
- [X] `cargo test -p panel-kit-core` green

#### Subtasks

- [X] Write the failing tests `project_panel_requires_no_snapshot_catalog_store_or_dock` and `standalone_surface_has_no_implicit_chrome` in `crates/panel-kit-core/src/frame.rs`
- [X] Implement `PanelProjectionInput`, `project_panel`, `project_chrome`, `project_dock_into` reusing the step-07 record types and geometry helpers
- [X] Add the surface-only chrome configuration input (name it so step 16's `ChromeSpec::surface_only()` lowers onto it naturally)
- [X] Equivalence tests: partial projections == corresponding regions of the full frame
- [X] Run `cargo test -p panel-kit-core`

#### Blockers & Risks

| Type | Item | Impact | Likelihood | Mitigation / Resolution |
|------|------|--------|------------|-------------------------|
| Risk | Partial projectors duplicate geometry logic instead of sharing it | Med | Med | Reuse the same internal helpers as `project_into`; equivalence tests pin consistency |
| Risk | Chrome input shape diverges from step 16's `ChromeSpec`, forcing rework | Med | Med | Design §9's ChromeSpec fields (metrics, hit_target_min, …) are the vocabulary; keep the frame-level input a projection of those |
