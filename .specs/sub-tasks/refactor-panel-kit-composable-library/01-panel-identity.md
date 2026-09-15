# Step 01: Panel identity and catalog

**Task File:** `.specs/tasks/todo/refactor-panel-kit-composable-library.refactor.md`

> The task file moves between `.specs/tasks/{draft,todo,in-progress,done}/` as work progresses; if it is not at this path, resolve it by its filename under `.specs/tasks/`.

**Phase:** Phase 1
**Model:** sonnet
**Agent:** developer
**Depends on:** None
**Parallel with:** `02-theme-core`
**Note:** None

**Goal:** Introduce the identity layer — `PanelKey`, `SpecPanelId(u32)`, `PanelMeta<K>`, `PanelCatalog<K>` — in a new `crates/panel-kit-core/src/panel.rs`, and migrate the core transition-function bounds from `PanelKind` to `PanelKey` (blanket impl `impl<T: PanelKind> PanelKey for T`) with zero behavior change.

This is design §4.1 (slice 1). **Deviation (slice boundary):** `PanelMeta` ships with the identity fields (`key`/`stable_id`/`title`/`slug`) only; design §4.1's `content: ContentSpec` field lands with step `05-widget-models`, which creates `ContentSpec` — it cannot exist yet without pulling Phase 2 work forward. Everything later (reducer, frame, persistence, spec) is generic over `PanelKey`. `PanelKey: Copy + Eq + Hash + 'static` (design §4.1); `SpecPanelId` is a copyable interned index that is NEVER serialized (compile-fail doctest pins both serde directions); `PanelCatalog` provides `try_new`/`get`/`get_by_stable_id`/`stable_id`/`len` and rejects duplicate stable IDs. Stable IDs must equal the current serde variant names (e.g. `"Workspace"`, `"Badges"` — see `crates/panel-kit-tui/examples/workspace_canary.rs:7-18`) so V1/V2 saves survive (D-2, D-13). Keep `PanelKind` itself unchanged and public (HR-18). Only `apply_command`'s bound migrates now (analysis "Shared Abstractions"); other call sites migrate in later steps when they become generic.

#### Expected Output

- `crates/panel-kit-core/src/panel.rs` — `PanelKey`, `SpecPanelId`, `PanelMeta`, `PanelCatalog`
- `crates/panel-kit-core/src/lib.rs` — `pub mod panel;` + narrow re-exports; `apply_command` bound migrated to `PanelKey`
- Unit tests in `panel.rs` (or `tests/` per crate convention)

#### Success Criteria

- [X] `panel_catalog_resolves_spec_ids_once_and_rejects_duplicate_stable_ids` passes (design §15 slice 1 first-failing test — write it first, watch it fail)
- [X] Blanket impl means every existing `PanelKind` enum continues to work with all migrated functions without source changes
- [X] The 6 existing core tests (`crates/panel-kit-core/src/lib.rs:1105-1304`: surface, keymap, focus, reconcile, v1-migrate) stay green through the bound migration
- [X] `cargo test -p panel-kit-core` passes; `cargo build` on the whole workspace still succeeds (backends unaffected)
- [X] Review iteration 1: `PanelCatalog::try_new` rejects duplicate panel keys as well as duplicate stable IDs, preventing divergent key/stable-ID mappings.

#### Subtasks

- [X] Write the failing test `panel_catalog_resolves_spec_ids_once_and_rejects_duplicate_stable_ids` in `crates/panel-kit-core/src/panel.rs`
- [X] Implement `PanelKey`, `SpecPanelId`, `PanelMeta<K>`, `PanelCatalog<K>` in `crates/panel-kit-core/src/panel.rs` per design §4.1
- [X] Add `pub mod panel;` and re-exports to `crates/panel-kit-core/src/lib.rs`; migrate the `apply_command` generic bound to `PanelKey`
- [X] Add tests: stable IDs round-trip for an existing enum's serde variant names; duplicate-ID rejection; index non-serialization (compile-level or API-shape assertion)
- [X] Run `cargo test -p panel-kit-core` and `cargo build --workspace` to confirm no behavior change
- [X] Review iteration 1: Add duplicate-key rejection coverage and move panel identity tests into a focused `src/panel/tests.rs` module to reduce `panel.rs` size.

#### Blockers & Risks

| Type | Item | Impact | Likelihood | Mitigation / Resolution |
|------|------|--------|------------|-------------------------|
| Risk | Bound migration accidentally narrows what `PanelKind` users can do | Med | Low | Blanket impl `impl<T: PanelKind> PanelKey for T` keeps all closed enums compatible; existing tests prove it |
| Risk | Concurrent step 02 also adds a `mod` line to core `lib.rs` | Low | Med | This step touches only its own declarations; keep the `lib.rs` edit to `pub mod panel;` + re-exports |
| Risk | Stable-ID mapping drifts from existing serde variant names, silently breaking saved layouts | High | Low | Test pins stable IDs against `serde_json::to_string` of the existing canary `Panel` enum variants (D-13) |
