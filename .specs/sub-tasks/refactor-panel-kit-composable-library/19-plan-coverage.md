# Step 19: Concrete plan lowering and leaf-disposition coverage

**Task File:** `.specs/tasks/todo/refactor-panel-kit-composable-library.refactor.md`

> The task file moves between `.specs/tasks/{draft,todo,in-progress,done}/` as work progresses; if it is not at this path, resolve it by its filename under `.specs/tasks/`.

**Phase:** Phase 6
**Model:** sonnet
**Agent:** developer
**Depends on:** `16-workspace-spec`, `18-spec-parity-checker`
**Parallel with:** `20-canary-spec`
**Note:** None

**Goal:** Add the concrete production spec plans and full leaf-disposition coverage (design §3.1 last paragraph, §11.3, slice 14, D-9): `src/spec_plan.rs` (pure `WebSpecPlan` + `pub lower_spec(&ResolvedWorkspace) -> WebSpecPlan` — NO web-sys/DOM types; holds only owned core/scalar configuration) behind a new root feature `spec-plan`; the TUI equivalent (`panel-kit-tui::spec_plan`/`TuiSpecPlan`, natively compilable, same one-plan rule); root `Cargo.toml` feature restructure: `default = ["web-runtime"]`, `web-runtime` gates the existing Dioxus/gloo/wasm deps + painter/input/store modules (design §3.1); `crates/panel-kit-tui/Cargo.toml` gains the matching feature wiring. The checker (step 18) gains the leaf walker: every schema leaf gets a backend disposition `Applied | ObservedAtRuntime | BoundByHost | Approximated` from the concrete plans; repository specs reject `Unsupported`. NO public plan trait / capability lattice / manifest API (CK-37 — the checker stays private tooling over concrete plans).

Theme dispositions complete here: TUI typography/density are `Approximated(reason)`, never silently omitted (HR-11 final half). The three field-drift classes go red (HR-15): `rust_only_field_is_rejected` (schema-required leaf missing from Nix output), `nix_only_field_is_rejected` (additional property fails Nix schema validation AND strict serde), `backend_ignored_field_is_rejected` (schema leaf with no plan disposition). The web painters consume `WebSpecPlan` when `web-runtime` is enabled (one plan consumed by production painters — not a checker duplicate).

#### Expected Output

- `src/spec_plan.rs` — `WebSpecPlan` + `lower_spec` (feature `spec-plan`)
- `crates/panel-kit-tui` — `TuiSpecPlan` + lowering (+ Cargo.toml feature wiring)
- Root `Cargo.toml` — `default = ["web-runtime"]`, `spec-plan` feature; module gating in `src/lib.rs`
- `tools/spec-parity` — leaf-disposition walker stage + three drift-class tests
- Tests

#### Success Criteria

- [X] `backend_ignored_field_is_rejected` passes (slice 14 first-failing test): a schema leaf with no backend disposition fails the checker
- [X] `rust_only_field_is_rejected` and `nix_only_field_is_rejected` pass: both field-drift directions red with pointer diagnostics
- [X] Checker builds natively with NO browser runtime in its dependency graph (`cargo tree -p spec-parity` shows no web-sys/gloo/dioxus)
- [X] TUI plan marks typography/density `Approximated(reason)`; repository specs reject `Unsupported`
- [X] Default-feature web crate unchanged in behavior (`web-runtime` on by default); `cargo build -p panel-kit` and `cargo build -p panel-kit --no-default-features --features spec-plan` both green
- [X] `cargo test -p spec-parity -p panel-kit-tui` green

#### Subtasks

- [X] Write the failing drift-class tests (three classes) in the checker's test module
- [X] Restructure root `Cargo.toml` features (`web-runtime` default, `spec-plan`); gate web modules in `src/lib.rs` accordingly
- [X] Implement `WebSpecPlan`/`lower_spec` in `src/spec_plan.rs` (owned core data only); TUI `TuiSpecPlan` + lowering + Cargo feature
- [X] Add the leaf-disposition walker to the checker; wire dispositions for every schema leaf (including theme approximations)
- [X] Verify checker dependency purity with `cargo tree`; run the full checker against repository specs

#### Blockers & Risks

| Type | Item | Impact | Likelihood | Mitigation / Resolution |
|------|------|--------|------------|-------------------------|
| Risk | Feature split breaks the wasm build lane (crane builds default features) | Med | Med | `web-runtime` default keeps the existing build identical; wasm lane pin verification via `nix build .#panel-kit` if available, else target build |
| Risk | Native checker accidentally links web-sys via default features | High | Med | Explicit `default-features = false, features = ["spec-plan"]` dependency; `cargo tree` purity test |
| Risk | Disposition set incomplete (a leaf silently unclassified) | Med | Med | The walker's own test enumerates schema leaves and fails on ANY unclassified leaf — that IS `backend_ignored_field_is_rejected` |

#### Files Changed

| File | Action | Notes |
|------|--------|-------|
| `Cargo.toml` | Modified | Added `default = ["web-runtime"]`, `web-runtime`, and `spec-plan`; made browser/runtime deps optional. |
| `src/lib.rs` | Modified | Gated browser/Dioxus modules and APIs behind `web-runtime`; exposed `spec_plan` behind `spec-plan`. |
| `src/spec_plan.rs` | Created | Added pure `WebSpecPlan` and `lower_spec(&ResolvedWorkspace)` with no DOM/web-sys types. |
| `crates/panel-kit-core/src/spec.rs` | Modified | Preserved spec id/version/panels in `ResolvedWorkspace`; added provider/binding helpers used by checker and plans. |
| `crates/panel-kit-tui/Cargo.toml` | Modified | Added `spec-plan` feature wiring. |
| `crates/panel-kit-tui/src/lib.rs` | Modified | Exposed `spec_plan` only under `spec-plan`. |
| `crates/panel-kit-tui/src/spec_plan.rs` | Created | Added native `TuiSpecPlan`, `lower_spec`, and concrete ratatui theme conversion with existing approximation dispositions. |
| `tools/spec-parity/Cargo.toml` | Modified | Checker depends on `panel-kit` with `default-features = false, features = ["spec-plan"]` and on TUI `spec-plan`. |
| `tools/spec-parity/src/field_coverage.rs` | Removed | Split the oversized checker implementation into private cohesive modules while preserving the `field_coverage` module API. |
| `tools/spec-parity/src/field_coverage/mod.rs` | Created | Keeps the public checker entry points and plan-coverage orchestration private to the `spec-parity` crate. |
| `tools/spec-parity/src/field_coverage/leaf_pointers.rs` | Created | Isolates schema/value leaf pointer extraction. |
| `tools/spec-parity/src/field_coverage/backend_disposition.rs` | Created | Isolates concrete web/TUI backend disposition derivation and TUI approximation handling. |
| `tools/spec-parity/src/field_coverage/fixtures.rs` | Created | Moves Step 19 reference fixture builders out of production code behind `cfg(test)`. |
| `tools/spec-parity/src/field_coverage/tests.rs` | Created | Keeps the three drift-class proofs and TUI approximation proof as focused private tests. |
| `tools/spec-parity/src/repository.rs` | Modified | Runs plan coverage against the normalized `mkWorkspaceSpec` fixture before the Step 20 canary compare. |

#### Verification Evidence

- Red phase: `cargo test -p spec-parity field_coverage --no-default-features` initially failed on missing `panel_kit::spec_plan`, missing `panel_kit_tui::spec_plan`, missing `FieldDisposition`, missing leaf-walker functions, and missing `PanelSpec::provider_declaration`.
- Green focused tests: `cargo test -p spec-parity -p panel-kit-tui` — 31 passed, 1 ignored.
- Build checks: `cargo build -p panel-kit`; `cargo build -p panel-kit --no-default-features --features spec-plan`.
- Dependency purity: `cargo tree -p spec-parity` completed; tree contains `panel-kit`, `panel-kit-core`, `panel-kit-tui`, `schemars`, `serde`, `serde_json`, `ratatui`/terminal dependencies and no `web-sys`, `gloo`, or `dioxus`.
- Nix-only field proof: `nix eval --raw --file nix/tests/workspace-spec-unknown-attr.nix json` fails with `/chrome/new_field: unknown field; allowed fields: ...`.
- Repository checker: `cargo run -p spec-parity -- check` now reaches plan coverage, then fails only on the existing Step 20 seven-vs-nine canary drift (`Flame`/`Distribution` missing), which remains out of scope for this step.
- Phase6Review maintainability fix: `cargo test -p spec-parity field_coverage` — 4 passed, 0 failed, 10 filtered out.
- Warning check after split: `cargo check -p spec-parity` — finished cleanly with no field-coverage unused-import warning.
- Split-size proof: `wc -l tools/spec-parity/src/field_coverage/{mod.rs,leaf_pointers.rs,backend_disposition.rs,tests.rs,fixtures.rs}` — largest module is 111 lines (`tests.rs`); former 412-line mixed file removed.
