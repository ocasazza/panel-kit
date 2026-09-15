# Step 16: Strict WorkspaceSpec with ordered JSON-pointer diagnostics

**Task File:** `.specs/tasks/todo/refactor-panel-kit-composable-library.refactor.md`

> The task file moves between `.specs/tasks/{draft,todo,in-progress,done}/` as work progresses; if it is not at this path, resolve it by its filename under `.specs/tasks/`.

**Phase:** Phase 5
**Model:** opus
**Agent:** developer
**Depends on:** `01-panel-identity`, `02-theme-core`, `05-widget-models`
**Parallel with:** `25-example-migrations`, `26-views-cutover`
**Note:** None

**Goal:** Implement the authoritative strict `WorkspaceSpec` in `crates/panel-kit-core/src/spec.rs` (design §9, slice 11, D-8): `WORKSPACE_SPEC_VERSION`, `WorkspaceSpec` + `LayoutSpec`/`SurfaceSpec`/`ChromeSpec`/`InputSpec`/`PersistenceSpec`/`PanelSpec`/`WindowSpec` — EVERY serialized struct `deny_unknown_fields`, required-nullable distinct from absent, internally tagged enums (never `flatten`), `Color` via the canonical `#rrggbb` codec from step 02 — plus `validate`/`resolve(&BindingManifest) -> ResolvedWorkspace` and `SpecErrors` reporting EVERY semantic error in document order as JSON pointers (HR-12).

The hard serde problem: derive-only deserialization stops at the first error and cannot distinguish present-null from absent. Design the decode strategy deliberately (e.g. deserialize to `serde_json::Value` then a validating walk, or custom `Deserialize` impls with per-field recovery) so unknown fields, missing fields, and every semantic class ALL accumulate — order = document order, pointer at the offending location, allowed-fields listed in the message (design §11.4 output examples are the contract). Add the opt-in features to `crates/panel-kit-core/Cargo.toml`: `spec-json` (optional `serde_json`), `spec-schema` (optional `schemars`, implies `spec-json`); neither default (CK-28, HR-18). `#[cfg_attr(feature = "spec-schema", derive(JsonSchema))]` on all serialized structs; serde/JsonSchema derives added to `Clamp`/`TileMetrics`/`ChromeMetrics`/`CommandStep` (core lib.rs:599-671,441-462,274-294) with NO semantic change; `Key` external form + `PanelCommand` snake_case serde come from step 03. `resolve` lowers spec → `ResolvedWorkspace` (catalog + defaults + theme + chrome + policy inputs) consumed by `Snapshot::from_defaults`-style seeding and step 19's plans.

#### Expected Output

- `crates/panel-kit-core/src/spec.rs` — full strict spec tree, validate/resolve, SpecErrors, BindingManifest, ResolvedWorkspace
- `crates/panel-kit-core/Cargo.toml` — `spec-json`/`spec-schema` opt-in features
- Derive additions on `Clamp`/`TileMetrics`/`ChromeMetrics`/`CommandStep`
- `crates/panel-kit-core/src/lib.rs` — `pub mod spec;` + cfg-gated re-exports
- Unit tests (strictness, pointers, null-vs-absent, feature gating)

#### Success Criteria

- [X] `workspace_spec_reports_strict_errors_by_pointer` passes (slice 11 first-failing test — write it first): unknown field, missing field, and each semantic class report ordered pointers in document order, with allowed-fields messages
- [X] Required-nullable: present-as-null is accepted where the schema says nullable; absence of a required field is an error (BVA: null vs missing)
- [X] `Color` codec: canonical lowercase `#rrggbb` only (inherits step 02's codec; spec rejects `#RRGGBB`/`fff`)
- [X] `plain_rust_enum_path_has_no_spec_schema_dependency` passes: building `-p panel-kit-core` with default features pulls no `serde_json`/`schemars` into the dependency graph (assert via `cargo tree` in a test script or feature-matrix CI check)
- [X] All spec round-trips: a valid spec serializes/deserializes losslessly with `spec-json`
- [X] `cargo test -p panel-kit-core --features spec-json,spec-schema` green AND default-feature `cargo test -p panel-kit-core` green

#### Subtasks

- [X] Write the failing test `workspace_spec_reports_strict_errors_by_pointer` plus null-vs-absent and codec tests in `crates/panel-kit-core/src/spec.rs`
- [X] Implement the spec structs with `deny_unknown_fields` per design §9; choose and document the error-accumulating decode strategy
- [X] Implement `validate`/`resolve` → `ResolvedWorkspace`; `SpecErrors` ordering = document order
- [X] Wire `spec-json`/`spec-schema` features in `crates/panel-kit-core/Cargo.toml`; add derives to `Clamp`/`TileMetrics`/`ChromeMetrics`/`CommandStep`
- [X] Add the dependency-graph purity check (script or test) proving default features exclude serde_json/schemars
- [X] Run `cargo test -p panel-kit-core` and `cargo test -p panel-kit-core --features spec-json,spec-schema`

#### Implementation Notes

- Added `crates/panel-kit-core/src/spec.rs` with the strict spec tree, `WORKSPACE_SPEC_VERSION`, `SpecErrors`/`SpecDiagnostic`, `BindingManifest`, `ResolvedWorkspace`, `WorkspaceSpec::validate`, and `WorkspaceSpec::resolve`.
- Decode strategy: `spec-json` parsing walks `serde_json::Value` first (with `serde_json/preserve_order`) to accumulate unknown/missing/semantic diagnostics in JSON-pointer order, then performs typed serde deserialization only after structural checks pass. This preserves present-null vs absent for the manifest's required-nullable `binding` field.
- Moved `serde_json` behind `spec-json` and added `spec-schema -> spec-json + schemars`; root web and TUI crates enable `panel-kit-core/spec-json` because their existing persistence paths still use the JSON lifecycle APIs.
- Added `crates/panel-kit-core/tests/dependency_graph.rs` to assert the default normal dependency graph excludes `serde_json` and `schemars`.
- Phase 5 review fix: split JSON strict-shape walkers into `crates/panel-kit-core/src/spec/strict.rs`; the walker now descends into every `ContentSpec` variant and both `DataSource` variants before typed serde decode, so malformed nested content accumulates ordered JSON-pointer diagnostics.
- Phase 5 review fix: removed the default-build `Color` import from `spec.rs`; `Color` is now used only inside the `spec-json` strict walker module.
- Phase 5 re-review 1 fix: `tools/spec-parity` now deterministically post-processes the generated `WorkspaceSpec` schema so strict required-nullable badge fields (`override_color`, `accent_color`) are present in `BadgeSpec.required` while retaining their `["array","null"]` type.
- Phase 5 re-review 2 fix: gated `persist.rs` imports used only by `spec-json` persistence helpers and collapsed the maximized-panel validation branch so focused core clippy is warning-clean in both default and spec feature modes.


#### Verification Evidence

- `cargo test -p panel-kit-core` → 74 passed.
- `cargo test -p panel-kit-core --features spec-json,spec-schema` → 85 passed.
- `cargo tree -p panel-kit-core --edges normal --no-default-features` showed only `serde` plus proc-macro support dependencies; no `serde_json` or `schemars`.
- Phase 5 review fix red/green: `cargo test -p panel-kit-core --features spec-json content_variants_reject_unknown_variant_fields_by_pointer` failed before the strict content walker (`left: ""`, expected `/panels/0/content/unexpected`) and now passes.
- `cargo test -p panel-kit-core --features spec-json data_source_variants_reject_ambiguous_and_missing_fields_by_pointer` → passed.
- `cargo test -p panel-kit-core --features spec-json nested_content_diagnostics_accumulate_in_panel_order` → passed.
- `cargo test -p panel-kit-core --features spec-json,spec-schema spec::tests` → 9 passed.
- `cargo test -p panel-kit-core --features spec-json,spec-schema` → 88 passed.
- `cargo check -p panel-kit-core --features spec-json,spec-schema` → passed with no warnings from the changed spec code.
- Phase 5 re-review 1 fix red/green: `cargo test -p spec-parity badge_schema_requires_nullable_color_fields` failed before schema post-processing (`BadgeSpec.required should include required-nullable field override_color`) and now passes.
- `cargo test -p panel-kit-core --features spec-json,spec-schema spec::tests` → 9 passed, confirming strict decoder behavior stayed green.
- Phase 5 re-review 2: `cargo clippy -p panel-kit-core --no-default-features -- -D warnings` → passed.
- Phase 5 re-review 2: `cargo clippy -p panel-kit-core --features spec-json,spec-schema -- -D warnings` → passed.
- Phase 5 re-review 2: `cargo test -p panel-kit-core --features spec-json,spec-schema spec::tests::workspace_spec_reports_strict_errors_by_pointer` → 1 passed.

#### Blockers & Risks

| Type | Item | Impact | Likelihood | Mitigation / Resolution |
|------|------|--------|------------|-------------------------|
| Risk | Derive-only serde cannot accumulate errors / distinguish null-vs-absent | High | High | This is THE design decision of this step: Value-stage validating walk or custom Deserialize; §11.4 output format is the acceptance contract, not the mechanism |
| Risk | Feature wiring leaks serde_json into default builds | Med | Med | `cargo tree` purity check is an explicit success criterion; optional deps only |
| Risk | Spec tree misses a §10.4 completeness row, breaking Nix parity later | Med | Med | Enumerate §10.4 rows as a test checklist inside this step; step 20/22 authoring will hit any gap loudly via the checker |
