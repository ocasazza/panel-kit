---
title: Refactor panel-kit into a composable library with a feature-complete pure-Nix WorkspaceSpec
design_spec: .specs/design/libraryness-2026-09-14.md
---

## Initial User Prompt

on how we can improve the library'ness of this repository

considering the differences (and advantages) of a library vs a framework I want to improve panel-kit's value for users by leaning into the advantages and composability (in the abstraction / composition vs inheritence sense) of this repository

I'd also like you to consider pure nix based panel-kit specifications that are totally feature complete alongside the rust based examples

## Description

panel-kit is consumed today as a framework, not a library: the web `Workspace` controller and the TUI `TuiWorkspace` own state, event effects, persistence, and the render loop (`use_workspace(...)` + `ws.render(body)` + `ws.dock()`), so a consuming application cannot render a single panel surface, keep input priority for its own editors and shortcuts, run two independent workspaces, or replace only one piece of chrome. At the same time the declarative Nix surface stops at layout geometry (`mkLayout`): it cannot author theme, chrome, input, content, or persistence policy — and its one canary check stays green even though the Nix canary lists seven panels while the Rust canary defines nine. This refactor reverses that ownership: applications own state, event ordering, effects, and render order; `panel-kit-core` provides pure transitions, borrowed projection, and persistence as an independent capability; each backend exposes independently callable native painters; and a strict, Rust-authoritative `WorkspaceSpec` can be authored in pure Nix at build time and mechanically checked for parity with the Rust-based examples.

The authoritative implementation specification is the design document referenced in the frontmatter (`design_spec`), which this task executes end-to-end: host-owned `Snapshot` + free `reduce`, `PanelKey`/`SpecPanelId`/`PanelCatalog`, borrowed `ProjectionBuffer`/`ProjectedFrame` with fully specified partial projectors, backend composition parts replacing `render`, `LayoutStore`/`SavePolicy` promoted to core with exact write-count semantics, one theme source including `focus_ring`, unified badge/widget models, a strict `WorkspaceSpec` with schemars export, `mkWorkspaceSpec` plus schema-driven Nix validation, the `tools/spec-parity` checker with ordered JSON-pointer drift diagnostics, a separate host crane lane so native tests and checkers actually execute on both Hydra systems, deletion of both controllers with no compatibility shims, and migration of the two pinned consumers (jump-cannon, apple-notes-ocr-flow) following the clean-cutover build-before-pin policy.

Who benefits: application developers (jump-cannon needs two concurrent workspaces and app-first input refusal; apple-notes-ocr-flow needs reviewer/editor event priority), Nix workspace authors (build-time validated, feature-complete declarative specs embedded as JSON), and maintainers (drift turns CI red with pointer-level diagnostics instead of passing silently; theme and widget models have one source of truth; native tests actually run in `nix flake check`).

**Scope**:
- Included: everything the design spec normatively specifies (§§3–13), executed in its §15 dependency-ordered TDD slices with each slice's named first-failing proof — identity/catalog, reducer, frame projection with allocation/lifetime proofs, partial composition and native parts, persistence promotion and save policy, theme unification, badge/widget unification, strict Rust spec, Nix schema/producer, live drift comparator, plan field coverage, authoritative nine-panel canary, check/build lanes, the three backend canaries, controller deletion and consumer migrations, and the §12 deletion ledger.
- Excluded: all design §16 non-goals — no runtime Nix evaluator/daemon/store, no universal DOM/RSX/ratatui AST or draw-command IR, no serialization of closures or live data, no pixel-identical backends, no new backend, no async/remote persistence or generic key/value storage, no second spec-migration framework, no redesign of existing surface/geometry/persistence primitives, no forced Nix or `WorkspaceSpec` adoption for Rust consumers, no retained controller facades, no speculative optimization beyond removing demonstrated allocation.

**User Scenarios**:
1. **Primary Flow**: an app developer replaces the controller with a host-owned `Snapshot`, reduces unconsumed events through core, projects into a reusable buffer, and composes `panel_surface`/`panel_chrome`/`traffic_lights`/`resize_grip`/`dock` in its own render order, applying its own persistence policy after each reduction.
2. **Alternative Flows**: a standalone single-surface panel (web or TUI, no shell/dock/store/observer); replace-only-the-dock with app navigation driven from `frame.dock`; full pure-Nix workspace authoring validated at build time and embedded as JSON via a store path; a Cargo-only consumer using default features with no Nix or schemars; downstream `mkLayout` usage unchanged.
3. **Error Handling**: invalid specs report every error in document order as JSON pointers; every drift class (Rust-only field, Nix-only field, backend-ignored field, provider mismatch, panel-set drift) makes a named check red with ordered diagnostics; invalid saved layouts error without overwriting defaults; invalid event targets are no-ops rather than panics; non-finite viewports are rejected; too-small viewports yield an honest `TooSmall` frame.

---

## Acceptance Criteria

**Checklist:**

| ID | Question | Category | Importance |
|----|----------|----------|------------|
| HR-1 | Does host code reduce keyboard, pointer move/resize/reorder, commands, dock restore, wheel, and viewport events via the core reducer without importing a backend (`reducer_reuses_existing_command_and_pointer_transitions`, `reducer_preserves_wheel_precedence`, `viewport_resize_policy_is_explicit`)? | hard_rule | essential |
| HR-2 | Are both backend controllers deleted, existing free functions still usable, and `project_panel` free of any `Snapshot`/catalog/store/dock requirement (`project_panel_requires_no_snapshot_catalog_store_or_dock` + standalone core example build)? | hard_rule | essential |
| HR-3 | Does the full projected frame contain chrome, deterministic paint order, placement/z/state/focus/drag, hit regions, dock entries, and content extent (`projected_frame_resolves_all_semantic_regions`)? | hard_rule | essential |
| HR-4 | After reserve/warm-up, does `project_into` allocate zero, clone no keys/strings/content, and fail compilation when the frame outlives a second mutable scratch borrow (`project_into_allocates_zero_after_reserved_warmup` + compile-fail doctest)? | hard_rule | essential |
| HR-5 | Do web CSS Grid and TUI cell conversion consume the identical `TileGridProjection` inputs (`web_and_tui_use_same_tile_grid_projection` + `checks.spec-parity`)? | hard_rule | essential |
| HR-6 | Can a web or TUI app render one panel surface standalone and replace only the dock, without a controller (`standalone_surface_has_no_implicit_chrome`, one-panel web wasm build, TUI offscreen-buffer test)? | hard_rule | essential |
| HR-7 | Does TUI retain `Block::title`/`inner` and web retain semantic DOM, focus, native overflow, ARIA, and wheel disposition (`tui_panel_inner_matches_projection`, `web_panel_preserves_focus_aria_and_overflow`)? | hard_rule | essential |
| HR-8 | Does `LayoutStore` exist only in core with V1/V2 restore, stable-ID mapping, reconciliation, merge, save, and clear all working (`core_store_restores_v1_reconciles_merges_saves_v2`, `store_clear_is_observable`)? | hard_rule | essential |
| HR-9 | Does `SavePolicy` produce the exact write-count transition table (Manual 0/0/0, OnSettle 0/1/0, OnChange 1/1/0 across Continuous/Settled/Unchanged), proven with a fake store, with reset-clear not re-saving (`save_policy_fake_store_write_counts`)? | hard_rule | essential |
| HR-10 | Does one core badge/widget model cover both surfaces, with core badge tests living in core and web tests asserting only rendering/actions (`badge_spec_covers_web_and_tui_fields`, `badge_actions_match_across_backends`)? | hard_rule | essential |
| HR-11 | Do dark/paper palette (including `focus_ring`), typography, and density have one editable core source with complete web/TUI conversions or explicit approximations, verified by `checks.theme-parity` (`dark_theme_emits_every_web_and_tui_token`)? | hard_rule | essential |
| HR-12 | Does strict `WorkspaceSpec` reject unknown and missing fields (never silently defaulting) and report every semantic error in document order as JSON pointers (`workspace_spec_reports_strict_errors_by_pointer`)? | hard_rule | essential |
| HR-13 | Does pure Nix express every design §10.4 completeness row (all content kinds, theme, chrome, input, persistence) through a value, binding, observation, or explicit approximation (`checks.spec-parity` schema/value/plan matrix artifact)? | hard_rule | essential |
| HR-14 | Does the historical seven-versus-nine canary drift fail without an editable expected count, reporting Flame/Distribution plus ordered pointers, with a permanent negative comparator test preserving the class (`spec_parity_rejects_live_seven_vs_nine_canary` + captured initial red)? | hard_rule | essential |
| HR-15 | Do Rust-only field, Nix-only field, and backend-ignored field drift each make a named check red (`rust_only_field_is_rejected`, `nix_only_field_is_rejected`, `backend_ignored_field_is_rejected`)? | hard_rule | essential |
| HR-16 | Do `checks.core-unit-tests` and `checks.spec-parity` actually execute natively despite the wasm lane's `doCheck=false` (host crane lane, no `CARGO_BUILD_TARGET`), on both Hydra systems? | hard_rule | essential |
| HR-17 | Do `checks.workspace-spec-web-wasm` and `checks.workspace-spec-browser-tui-wasm` build for wasm32 (incl. ASCII override) and `checks.workspace-spec-tui-native` build and run `--check-offscreen` from the same Nix canary spec, exercising every content kind? | hard_rule | essential |
| HR-18 | Can a default-feature Cargo-only consumer use `PanelKind`, core geometry/reducer/persistence, and backend parts without Nix or schemars entering its dependency graph (`plain_rust_enum_path_has_no_spec_schema_dependency` + Cargo-only example build)? | hard_rule | essential |
| HR-19 | Does `mkLayout` remain an unchanged V2 layout-only API with its round-trip proven inside `checks.spec-parity`? | hard_rule | essential |
| HR-20 | Does jump-cannon support two independent workspaces sharing no state or store, with preserved keys/IDs/resize behavior, building before its pin update (`two_concurrent_workspaces_do_not_share_state_or_store`, `jump-cannon-app-ui-wasm`)? | hard_rule | essential |
| HR-21 | Does apple-notes-ocr-flow preserve reviewer/editor event priority and its persisted key/serde IDs, with its real wasm target (inventoried from its actual checkout) building before pin update (`apple-notes-ocr-flow-reviewer-wasm`)? | hard_rule | essential |
| HR-22 | Does every design §12 deletion land with its replacement, leaving no compatibility controller, deprecated alias, old-path re-export, second renderer, or stale README adoption wording? | hard_rule | essential |
| CK-23 | Does `nix flake check` pass with all named checks (`core-unit-tests`, `spec-parity`, `theme-parity`, `workspace-spec-tui-native`, `workspace-spec-web-wasm`, `workspace-spec-browser-tui-wasm`, `clippy`, `doc`) green? | hard_rule | essential |
| CK-24 | Does the clippy gate pass with zero warnings (`cargoClippy --all-targets -- -D warnings`) on the changed workspace? | hard_rule | essential |
| CK-25 | Does the rustdoc gate pass with zero warnings (`cargoDoc` with `RUSTDOCFLAGS="-D warnings"`, `missing_docs` promoted)? | hard_rule | essential |
| CK-26 | Does the reducer delegate to the existing `command_for`, `apply_command`, `begin_drag`, `begin_tile_resize`, `apply_drag`, `reorder_tile`, and `restore` functions rather than duplicating transition logic? | principle | important |
| CK-27 | Is the new code free of function/logic/concept duplication that already exists elsewhere (theme literals, badge models, geometry, persistence schema logic)? | principle | important |
| CK-28 | Is every new serialized struct strict (`deny_unknown_fields`) with required-nullable handled distinctly from absent, and are `spec-json`/`spec-schema` opt-in (non-default) with `web-runtime` as the root default and the native checker depending on `spec-plan` only? | hard_rule | essential |
| CK-29 | Does every selected test type in the Test Matrix (unit, integration, component, contract, smoke) have at least one corresponding implemented test/check? | hard_rule | essential |
| CK-30 | Does every Test Matrix row (main + edge + error across types) have a corresponding implemented test/check? | hard_rule | essential |
| CK-31 | Does every testable checklist item resolve to at least one real, passing named proof — no orphans? | hard_rule | essential |
| CK-32 | Does every entry in the Test Cases to Cover list have an implemented test/check? | hard_rule | essential |
| CK-33 | Are all parity/schema checks free of hand-pinned expected values (e.g. jq `panels \| length == N` or expected-field lists) that an editor could change to mask drift? | pitfall | pitfall |
| CK-34 | Are the generated schema (`nix/schema/workspace-spec.schema.json`) and the DESIGN.md token region machine-generated with no hand edits? | pitfall | pitfall |
| CK-35 | Do all cutover/deletion proofs rely on compilation and observable behavior rather than tests that grep source text? | pitfall | pitfall |
| CK-36 | Is `ProjectedFrame` never stored in a Dioxus signal, memo, component prop, or retained closure (render-scoped borrow dropped before event mutation)? | pitfall | pitfall |
| CK-37 | Is the implementation free of a public plan trait, capability lattice, or semantic-manifest API (the checker stays private tooling over concrete `WebSpecPlan`/`TuiSpecPlan`)? | pitfall | pitfall |
| CK-38 | Do panel-kit TUI painters avoid constructing temporary visible/order/dock `Vec`s or per-frame title `String`s (iterating the borrowed frame and catalog)? | pitfall | pitfall |
| CK-39 | Did the task make meaningful, scope-appropriate small improvements to touched code (renames, dead-code removal, missing docs) without expanding scope? | principle | optional |

**Regular Checks:**



- [ ] Build passes: `nix build .#panel-kit` (wasm32 crane lane) and the new host crane lane builds `tools/spec-parity` and the native canary
- [ ] Lint passes with zero new errors/warnings: `checks.clippy` — `cargoClippy --all-targets -- -D warnings`
- [ ] Docs pass: `checks.doc` — `cargoDoc` with `RUSTDOCFLAGS="-D warnings"`
- [ ] Tests pass: `nix flake check` green on `x86_64-linux` and `aarch64-darwin`, including `checks.core-unit-tests` (`cargo test -p panel-kit-core --features spec-json,spec-schema`) and `checks.spec-parity`
- [ ] No code duplication: new code does not duplicate function/logic/concept that already exists elsewhere
- [ ] Boy Scout Rule: scope-appropriate small improvements made to touched code (renames, dead-code removal, missing types) without scope creep
- [ ] Reuse honored: the reducer imports/calls the existing `command_for`, `apply_command`, `begin_drag`, `begin_tile_resize`, `apply_drag`, `reorder_tile`, and `restore` functions instead of reimplementing them
- [ ] Every test type selected in the **Test Matrix** (unit / integration / component / contract / smoke) has at least one corresponding test
- [ ] Every **Test Matrix** row (main + edge + error) has a corresponding test
- [ ] Every testable checklist item resolves to at least one real, passing test — no orphans
- [ ] Every entry in the **Test Cases to Cover** list has an implemented test

**Rubric:**

| Criterion | Weight |
|-----------|--------|
| Composability & Clean Cutover | 0.18 |
| Parity Machinery Rigor | 0.16 |
| Projection Performance & Borrow Discipline | 0.13 |
| Spec Strictness & Validation Diagnostics | 0.10 |
| Persistence Lifecycle Exactness | 0.09 |
| Test Evidence Quality | 0.08 |
| Single-Source Theme & Widget Unification | 0.07 |
| Native Fidelity Preservation | 0.07 |
| Architecture Economy & Feature Gating | 0.06 |
| Project Guidelines Alignment | 0.06 |

**Rubric Score Definitions:**

Scale: 1-5 integers, anchor-relative — each criterion pins `score_2`/`score_4`, and 1/3/5 are placed relative to them: 1 = worse than `score_2` on the named axis; 2 = matches `score_2`; 3 = between the anchors, not clearly nearer either; 4 = matches `score_4`; 5 = better than `score_4` on the SAME axis the contrast names.

### Composability & Clean Cutover

Is the ownership reversal real? Can an app genuinely compose parts (standalone surface, chrome-optional, replace-only-dock, two independent workspaces) without a hidden god object, and did the full §12 deletion ledger land with no facade, alias, or re-export left behind?

Inspect the public surface and the §12 ledger diff: enumerate what a standalone single-panel app must construct, and whether any controller, convenience wrapper, or old path remains. Then place the artifact against the anchors.

#### Anchors

**contrast**: Parts are independently callable and the controller is fully deleted, not retained as a convenience wrapper with its per-frame clone.

**score_2**:

```text
pub fn use_composed_workspace(key: &'static str, defaults: fn() -> Vec<PanelWin<Panel>>) -> Workspace<Panel> {
    Workspace::new(key, defaults) // retained for convenience
}
```

**score_4**:

```text
pub fn project_panel<K: PanelKey>(panel: &PanelWin<K>, source_index: usize, input: PanelProjectionInput<'_>) -> Option<PanelProjection<K>>;
// host loop composes panel_surface / chrome / lights / grip / dock itself; no controller exists
```

### Parity Machinery Rigor

Does the parity checker mechanically prove schema match, strict decode, typed equality, bidirectional provider manifests, and per-leaf backend dispositions — failing red on every drift class with ordered JSON-pointer diagnostics and no editable expected values?

Read the checker's comparison strategy and one failure output per drift class; verify no hand-pinned counts or expected-field lists and that the negative comparator preserves the seven-versus-nine class. Then place the artifact against the anchors.

#### Anchors

**contrast**: Drift produces ordered pointer-level diffs in both directions with set summaries, not a pinned expected count that an editor could change.

**score_2**:

```text
jq -e '(.panels | length == 9)' ${workspaceSpecJson} > /dev/null
```

**score_4**:

```text
/panels/7/id: nix=<missing>, rust="Distribution"
/panels/length: nix=7, rust=9
missing from Nix: Flame, Distribution
```

### Projection Performance & Borrow Discipline

Does projection honor the allocation and lifetime contract everywhere — warmed-path zero allocation, no clones of keys/strings/content, TUI borrowed iteration with no temporary vectors or per-frame title strings, a compile-fail borrow proof, and the frame never retained in signals or props?

Trace one web render and one TUI render end-to-end for allocations and clones; check the allocator-counted test's rigor (serial, warmed, unchanged capacity) and the compile-fail doctest. Then place the artifact against the anchors.

#### Anchors

**contrast**: The render path copies numeric records into reusable caller-owned scratch, not clones the panel vector and collects indices every frame.

**score_2**:

```text
let panels: Vec<PanelWin<K>> = self.panels.read().clone();
let visible: Vec<usize> = (0..panels.len()).collect();
```

**score_4**:

```text
let frame = project_into(input, &mut scratch); // zero-alloc after warm-up; frame borrows scratch only
```

### Spec Strictness & Validation Diagnostics

Is every serialized struct strict (`deny_unknown_fields`), is required-null distinguished from absent, does `Color` use the canonical `#rrggbb` codec, and do validation errors accumulate in document order as JSON pointers covering every semantic class?

Pick three malformed documents (unknown field, missing field, semantic violation) and read the reported diagnostics; check null-versus-absent handling on a nullable field. Then place the artifact against the anchors.

#### Anchors

**contrast**: Unknown and missing fields are rejected with a pointer at the offending location, not silently defaulted away.

**score_2**:

```text
# decode of {"chrome": {..., "new_field": 1}} succeeds, new_field ignored
assert!(serde_json::from_str::<WorkspaceSpec>(&doc).is_ok());
```

**score_4**:

```text
/chrome/new_field: unknown field; allowed fields: metrics, hit_target_min, ...
/panels/3/window/w: missing required field (or: must be finite and positive)
```

### Persistence Lifecycle Exactness

Is persistence an exact core capability: load/save/clear trait in core only, V1/V2 lifecycle with stable-ID mapping and unit reconciliation, and the SavePolicy write-count table implemented precisely (including reset-clear not immediately re-saving)?

Read the fake-store write-count test's cases against the design §7 table (3 policies × 3 reduction kinds) and the V1/V2 restore path including unknown and newly declared IDs. Then place the artifact against the anchors.

#### Anchors

**contrast**: Save timing follows the exact policy table verified by write-count assertions, not an implicit effect that writes whenever something changed.

**score_2**:

```text
// save on every signal write, no policy, no clear
effect(move || persist(&snapshot.read()));
```

**score_4**:

```text
// SavePolicy::OnSettle: 0 writes on Continuous, 1 on Settled, 0 on Unchanged
assert_eq!(store.save_calls(), &[expected_settled_json]);
store.clear(); assert!(store.save_calls().is_empty()); // reset does not re-save
```

### Test Evidence Quality

Do the proofs carry evidentiary weight — each slice's first failing test actually fails on the pre-slice tree, the negative comparator preserves the historical drift class, assertions observe behavior (compilation, rendered output, write counts) rather than source text, and tests are deterministic and hermetic?

Sample three named proofs: would each have failed before its slice landed? Are any assertions grepping source or asserting non-emptiness? Is the offscreen/SSR evidence deterministic? Then place the artifact against the anchors.

#### Anchors

**contrast**: Proofs fail on the unfixed tree and assert observable behavior, not pass trivially or grep source text.

**score_2**:

```text
// "proof" that would pass on the old tree too
assert!(std::fs::read_to_string("src/lib.rs")?.contains("project_into"));
```

**score_4**:

```text
// permanent negative comparator, fails on real drift
spec_parity_rejects_live_seven_vs_nine_canary(); // asserts mismatch class + exact pointers
```

### Single-Source Theme & Widget Unification

Do theme tokens (including `focus_ring`, typography, density) and badge/widget models exist exactly once in core, with generated web/TUI/DESIGN.md consumers verified by theme-parity, dispositions instead of omissions on TUI, and core badge tests living in core?

Count editable sources for one token (e.g. `focus_ring`) across core, CSS, TUI, and DESIGN.md; check TUI typography/density dispositions and where the badge tests live. Then place the artifact against the anchors.

#### Anchors

**contrast**: Each token value is editable in exactly one place (core) with generated consumers, not duplicated as backend literals.

**score_2**:

```text
// crates/panel-kit-tui/src/theme.rs
pub const DARK: [Color; 13] = [/* hand-maintained */];
```

**score_4**:

```text
// core: the only editable source; CSS vars, TUI conversion, DESIGN.md region generated
impl ThemeTokens { pub fn dark() -> Self { /* ... */ } }
// tui plan: typography: Approximated("terminal host font")
```

### Native Fidelity Preservation

Did composability preserve native semantics — TUI `Block::title`/`inner` and native widgets, web semantic DOM/ARIA/focus/native overflow/wheel disposition, CSS Grid from explicit placement — with differences expressed as explicit approximations rather than silent omissions or a lowest-common-denominator path?

Compare one rendered TUI panel (`Block::title`/`inner` versus projected regions) and one web panel's SSR markup (roles, focus, overflow) before and after; check the plan's dispositions for terminal-incapable tokens. Then place the artifact against the anchors.

#### Anchors

**contrast**: Backend painters keep their native primitives driven by semantic projections, not flatten rendering into a shared command tape.

**score_2**:

```text
// core emits generic paint ops both backends must replay
pub enum PaintOp { Rect(Rect), Text(String), /* ... */ }
```

**score_4**:

```text
let block = Block::bordered().title(title_span); let body = block.inner(area); // native ratatui
// web: semantic DOM + CSS Grid with explicit track/placement values from TileGridProjection
```

### Architecture Economy & Feature Gating

Does the design show restraint — no public plan trait, capability lattice, or manifest API, the checker private over concrete plans, `spec-json`/`spec-schema` opt-in, `web-runtime` the root default, the Cargo-only path clean, and core dependency-light — while doing the job?

List the public items the task adds; check feature wiring (defaults, the checker's feature set) and the Cargo-only consumer's resolved dependency graph. Then place the artifact against the anchors.

#### Anchors

**contrast**: The checker is private tooling over concrete plans with opt-in features, not a public capability abstraction every backend must implement.

**score_2**:

```text
pub trait BackendPlan { fn disposition(&self, leaf: Leaf) -> FieldDisposition; }
pub trait ParityProjection { /* ... */ } // public trait stack added to core
```

**score_4**:

```text
// tools/spec-parity (private): calls concrete web::lower_spec / tui::lower_spec
// core: #[cfg_attr(feature = "spec-schema", derive(JsonSchema))] — opt-in only
```

### Project Guidelines Alignment

Does the implementation honor AGENTS.md — core owns renderer-neutral semantics, backends translate platform events into core input types and paint only, behavior that should match is encoded in core first, examples stay broad and buildable as canaries, loading/design-language conventions are untouched — and the MIGRATION.md clean-cutover consumer policy (build before pin update, no shims)?

Walk each AGENTS.md rule against the diff: is there any renderer-specific event or state type that a core abstraction could own? Is any shared behavior duplicated in both backends instead of core? Was any canary narrowed? Did the loading/design-language surface change? Is consumer guidance written in MIGRATION.md style? Then place the artifact against the anchors.

#### Anchors

**contrast**: Renderer-neutral semantics live in core and backends only translate/paint, not each backend re-implementing shared behavior the rule assigns to core.

**score_2**:

```text
// both backends privately re-implement the same wheel-chaining decision
fn should_scroll_workspace(delta: f64) -> bool { /* duplicated in src/ and crates/panel-kit-tui/ */ }
```

**score_4**:

```text
// core owns the semantics; backends translate platform input into core events
pub enum WorkspaceEvent<K> { Wheel { delta_y: f64, disposition: WheelDisposition }, /* ... */ }
```

**Test Strategy:**



**Criticality:** HIGH — clean major-version cutover of a published library's public API consumed by two git-pinned applications; layout persistence is user data; the parity machinery itself is load-bearing for CI/release integrity.

**Test Matrix:**

| Type | Size | Framework | Dependencies | Gate |
|------|------|-----------|--------------|------|
| unit | small | cargo test (native crane lane, `checks.core-unit-tests`) | in-memory fake `LayoutStore` | Gate 1 |
| integration | medium | cargo test + ratatui TestBackend + `tools/spec-parity` harness | mkLayout/mkWorkspaceSpec JSON fixtures, TestBackend buffers, in-memory fake stores | Gate 2 |
| component | medium | ratatui TestBackend (`--check-offscreen`), dioxus-ssr | fixed-size offscreen frames, SSR rendered markup | Gate 3 |
| contract | medium | `tools/spec-parity` (schemars schema + custom checker) as `checks.spec-parity` | generated schemars schema, `nix/specs/*.nix` JSON, concrete `WebSpecPlan`/`TuiSpecPlan` lowering | Gate 4 |
| smoke | large | `nix flake check` / Hydra (crane build lanes) | wasm32 toolchain lane, host crane lane, jump-cannon and apple-notes-ocr-flow build targets | Gate 5 |

**Test Cases to Cover**

#### HR-1: Does the core reducer cover all event classes purely?
- [unit] `reduce_key_matches_command_for_and_apply_command` — key events produce the same transitions as the existing command path [EP: per event class]
- [unit] `reducer_preserves_wheel_precedence` — ContentConsumed wheel is a no-op; accepted workspace wheel is Settled [Decision table: disposition × change]
- [unit] `viewport_resize_policy_is_explicit` — ScaleFloating applies the ratio policy; PreserveIntent updates viewport/profile only; non-finite/non-positive dimensions rejected [BVA: 0, negative, NaN]
- [unit] `reducer_replays_move_resize_reorder_and_settle_phases` — pointer motion/hover = Continuous; pointer-up/commands/dock-restore/wheel/viewport = Settled [State transition]
- [unit] invalid indices/targets are no-ops, not panics [EP: invalid partition]

#### HR-2: Is there no god object, with free functions still usable?
- [unit] `project_panel_requires_no_snapshot_catalog_store_or_dock` — the partial projector needs none of them
- [integration] standalone core example builds and links no backend

#### HR-3: Is the projected frame semantically complete?
- [unit] `projected_frame_resolves_all_semantic_regions` — chrome, order, placement/z/state/focus/drag, hit regions, dock, content extent
- [unit] TooSmall viewport emits `FrameStatus::TooSmall` without negative/NaN rectangles [BVA: at/below minimum]
- [unit] floating paint order is `(z, source_index)`; tiling is source order; maximized emits only the deterministic frontmost; minimized appears only in the dock [EP: per mode]

#### HR-4: Zero steady-state allocation plus borrow proof?
- [unit] `project_into_allocates_zero_after_reserved_warmup` — allocator-counted, serial, unchanged capacity
- [unit] compile-fail doctest: the frame cannot survive a second mutable scratch borrow

#### HR-5: Is tile geometry single-sourced?
- [unit] `web_and_tui_use_same_tile_grid_projection` — both backend plans consume identical track/span inputs

#### HR-6: Standalone surface and replace-only-dock?
- [unit] `standalone_surface_has_no_implicit_chrome` — `ChromeSpec::surface_only()` disables every chrome/dock part
- [component] TUI offscreen: a standalone panel draws surface + body rect with no dock/title row
- [smoke] one-panel web wasm build check

#### HR-7: Native fidelity retained?
- [component] `tui_panel_inner_matches_projection` — `Block::title`/`inner` body rect equals the projected body region
- [component] `web_panel_preserves_focus_aria_and_overflow` — SSR markup carries focus/ARIA/overflow semantics and wheel disposition

#### HR-8: Persistence owned by core?
- [integration] `core_store_restores_v1_reconciles_merges_saves_v2` — V1 migration, unit reconciliation, stable-ID mapping both directions, defaults merge, V2 encode
- [unit] `store_clear_is_observable` — clear visible; unknown saved IDs ignored; newly declared IDs appended
- [integration] error paths: invalid JSON / future version return errors without overwriting defaults; invalid saved viewport fails before division

#### HR-9: SavePolicy write counts exact?
- [unit] `save_policy_fake_store_write_counts` — 3 policies × 3 reduction kinds = exact 0/1 write counts; reset-clear does not re-save

#### HR-10: One badge/widget model?
- [unit] `badge_spec_covers_web_and_tui_fields` — one model carries every field both surfaces used
- [unit] `badge_actions_match_across_backends` — action behavior equivalent across backends
- [unit] core badge tests (including `tag_hue`) live in core; web tests assert rendered labels/ARIA/actions only

#### HR-11: One theme source?
- [unit] `dark_theme_emits_every_web_and_tui_token` — every token including `focus_ring` emitted/converted
- [contract] `checks.theme-parity` — core tokens = generated CSS variables = TUI conversion = generated DESIGN.md region
- [unit] TUI plan marks typography/density `Applied` or `Approximated(reason)`, never silently omitted

#### HR-12: Strict spec validation with ordered pointers?
- [unit] `workspace_spec_reports_strict_errors_by_pointer` — unknown field, missing field, and each semantic class report ordered pointers in document order
- [unit] required-nullable is present-as-null while absence is rejected [BVA: null vs missing]
- [unit] `Color` codec emits/accepts canonical lowercase `#rrggbb` only

#### HR-13: Pure Nix feature-complete?
- [contract] `checks.spec-parity` schema/value/plan matrix — every §10.4 completeness row has a value/binding/observation/approximation; all 12 content kinds exercised; repository specs reject `Unsupported`

#### HR-14: Live drift goes red?
- [contract] `spec_parity_rejects_live_seven_vs_nine_canary` — mismatch class + pointers (Flame, Distribution) with no editable expected count; initial red captured before migration

#### HR-15: All field-drift classes red?
- [contract] `rust_only_field_is_rejected` — a schema-required leaf missing from the Nix output fails
- [contract] `nix_only_field_is_rejected` — an additional property fails Nix schema validation and strict serde
- [contract] `backend_ignored_field_is_rejected` — a schema leaf with no backend-plan disposition fails

#### HR-16: Native CI execution?
- [smoke] `checks.core-unit-tests` executes on the host lane (no `CARGO_BUILD_TARGET`, `doCheck=true`) on x86_64-linux and aarch64-darwin
- [smoke] `checks.spec-parity` runs natively in the same lane

#### HR-17: Three backend canaries from one spec?
- [smoke] `checks.workspace-spec-web-wasm` builds with the spec JSON passed as a build-time store path
- [smoke] `checks.workspace-spec-browser-tui-wasm` builds including the ASCII override
- [component] `checks.workspace-spec-tui-native` `--check-offscreen` renders a deterministic fixed-size frame with expected body/chrome regions

#### HR-18: Cargo-only path clean?
- [unit] `plain_rust_enum_path_has_no_spec_schema_dependency` — default features pull no schemars/serde_json into the consumer graph
- [smoke] a Cargo-only example builds without Nix involvement

#### HR-19: mkLayout retained?
- [contract] mkLayout output decodes as `SavedLayoutV2` and round-trips inside `checks.spec-parity` (V1/V2 lifecycle compatibility)

#### HR-20: jump-cannon two-workspace migration?
- [unit] `two_concurrent_workspaces_do_not_share_state_or_store` — independent snapshots/scratch/stores; storage keys and serde IDs preserved
- [smoke] `jump-cannon-app-ui-wasm` builds for its supported wasm target before the pin update

#### HR-21: apple-notes-ocr-flow reviewer migration?
- [smoke] `apple-notes-ocr-flow-reviewer-wasm` — its real reviewer target (resolved during call-site inventory) builds before the pin update; editor/text input keeps priority over panel commands

#### HR-22: Clean cutover?
- [integration] rewritten doctests/examples compile against only the new surface (old controller usage no longer compiles)
- [contract] ledger diff review: every §12.1 item deleted alongside its replacement

#### CK-26: Reducer reuses existing transitions?
- [unit] `reducer_reuses_existing_command_and_pointer_transitions` — delegation verified against the same existing free functions, no duplicated transition logic

**Definition of Done:**

- [ ] Every `essential` checklist item answers YES
- [ ] All Regular Checks pass
- [ ] Every test case in **Test Cases to Cover** is implemented and passing
- [ ] All 22 design-spec acceptance criteria (AC-01..AC-22) are green with their named proofs
- [ ] Every §12.1 deletion has landed with its replacement; no compatibility controller, alias, re-export, or facade remains, and README adoption wording reflects the composable surface
- [ ] The §11.4 drift classes were demonstrated red before the canary migration, and the permanent negative comparator preserves the seven-versus-nine class
- [ ] Existing V1/V2 records, storage keys, serde IDs, and `mkLayout` outputs keep working throughout
- [ ] Both consumer build proofs (`jump-cannon-app-ui-wasm`, `apple-notes-ocr-flow-reviewer-wasm`) exist before any pin update

## Architecture Overview

**References**: Skill (research prior art) `.claude/skills/composable-library-design/SKILL.md` · Analysis (current-tree impact) `.specs/analysis/analysis-refactor-panel-kit-composable-library.md` · Design (authoritative implementation spec, per frontmatter) `.specs/design/libraryness-2026-09-14.md` · Scratchpad `.specs/scratchpad/91bc0a43.md`

### Solution Strategy

Execute the design-spec synthesis end-to-end as a clean major-version cutover that reverses ownership: host applications own state, event ordering, effects, and render order; `panel-kit-core` owns renderer-neutral identity (`PanelKey`/`SpecPanelId`/`PanelCatalog`), pure reduction (`Snapshot` + free `reduce` delegating to the existing transition functions), single-sourced geometry and borrowed projection (`ProjectionBuffer`/`ProjectedFrame`/`project_into` + fully specified partial projectors), persistence (`LayoutStore`(+`clear`)/`SavePolicy`/V1-V2 lifecycle), one theme source, unified widget models, and the strict authored `WorkspaceSpec`; backends become translate-and-paint adapters exposing independently callable native parts (`panel_surface`/`panel_chrome`/`traffic_lights`/`resize_grip`/`dock` on web, `draw_*` on TUI); the spec is authorable in pure Nix (`mkWorkspaceSpec`), validated against a generated committed schema, delivered as a build-time JSON store path, and mechanically parity-checked by the native `tools/spec-parity` executable running in a new host crane lane. Both controllers are deleted with no facades or re-exports (design §12 ledger, amended by decision D-11 below); `mkLayout`, `PanelKind`, persisted serde spellings, and stable IDs are preserved so existing V1/V2 records and `mkLayout` outputs keep working throughout.

**Architecture Pattern**: Hexagonal (Ports & Adapters) with a functional core / imperative shell. The repository already runs this pattern locally — core `badge`/`loading` models with per-backend painters, and `LayoutStore` already a trait-plus-adapter port (`crates/panel-kit-tui/src/lib.rs:149-171`) — and AGENTS.md's core-first rule mandates it. Pure `reduce`/`project_into`/`validate`/`resolve` form the functional core; web/TUI input translation, stores, painters, spec lowering, and the checker are adapters; the host application is the imperative shell that owns control flow. Prior art grounding (skill): AccessKit (core/adapters, Rust-canonical generated schema), taffy (caller-owned scratch + borrowed results), Zellij/tmux (authored-config vs operator-state split), nixcfg-rs (committed generated schema + drift check in `nix flake check`), crane trunk (dual lanes).

### Key Decisions

| # | Decision | Reasoning |
|---|---|---|
| D-1 | **Ownership reversal via free reducer** — host-owned `Snapshot<K>` + `reduce()` reporting effects (`Reduction{changed, phase, focus_request}`), never performing them; no callbacks/IoC in core. | Removes the inversion of control that blocks app-first input refusal, two independent workspaces (jump-cannon), and replace-only-one-part composition. Delegates to the existing `command_for`/`apply_command`/`begin_drag`/`begin_tile_resize`/`apply_drag`/`reorder_tile`/`restore` (CK-26 forbids duplicating transition logic); wheel disposition stays a backend decision (`ContentConsumed` = no-op in core). |
| D-2 | **Identity layer, not identity break** — retain `PanelKind`, add narrower `PanelKey` bound + interned `SpecPanelId(u32)` + `PanelCatalog` stable-string mapping; numeric indices never serialize. | Keeps plain-Rust enum consumers source-compatible (HR-18) while letting Nix-owned runtime strings exist; stable IDs = current serde variant names so V1/V2 saves survive unchanged. |
| D-3 | **Borrowed projection into caller-owned scratch** — `project_into(input, &mut ProjectionBuffer) -> ProjectedFrame` borrowing only scratch; partial `project_panel` requires no Snapshot/catalog/store/dock. | Deletes the demonstrated waste (web clones the panel vector + collects visible indices every render at `src/lib.rs:1077-1083`; TUI builds scratch Vecs at `tui/lib.rs:402-452,628-650`). Taffy's `LayoutPartialTree` pattern proves the shape at scale; proven by allocator-counted test + compile-fail doctest (HR-4). One `TileGridProjection` feeds CSS Grid, TUI cells, and span-resize deltas (HR-5). |
| D-4 | **Backend parts, no `render`** — port `panel_surface`/`panel_chrome`/`traffic_lights`/`resize_grip`/`dock` and `draw_*` equivalents from the controller monoliths; native fidelity preserved (ratatui `Block::title`/`inner`, semantic DOM/ARIA/focus/native overflow). | Composability demands independently callable parts; §16.2 forbids a paint IR, so parts stay native painters over semantic projections. `panel_surface` never calls other parts — chrome is opt-in per call (HR-6/HR-7). |
| D-5 | **Persistence promoted to core as a port** — `LayoutStore` (load/save/**clear**) + `SavePolicy{Manual,OnSettle,OnChange}` with the exact write-count table; one store instance = one logical record; adapters `LocalStorageLayoutStore` (web) and `JsonFileLayoutStore` (TUI). | Cross-backend persistence belongs in core under the AGENTS.md rule; the write-count table replaces implicit Dioxus effects with testable data (HR-8/HR-9); reset-clear not re-saving is exact. Backends own only byte transport. |
| D-6 | **One theme source** — core `ThemeTokens` (incl. `focus_ring`) is the only editable palette/typography/density source; CSS `:root`, TUI conversion, and the DESIGN.md token region become generated, verified by `checks.theme-parity`. | Today three hand-maintained copies exist (`assets/panel-kit.css:5-29`, TUI theme literals, DESIGN.md); the existing build.rs substitution engine (build.rs:54-93) already proves generation. TUI marks typography/density `Approximated(reason)`, never silent omission. |
| D-7 | **Unified widget/badge models in core** — one `BadgeSpec` superset + chart/table/meter/status/scroll/spinner semantic models move to `core/widgets/`; painters stay backend-native; runtime views borrow (`ContentView<'a>`). | Generalizes the existing badge model/painter split (analysis Pattern 1); web gains charts/table/meter/status painters from the same models the TUI already renders; core badge tests move into core (design §12.1.10); no per-paint cloning of points/rows/strings. |
| D-8 | **Rust-authoritative strict spec** — `deny_unknown_fields` on every serialized struct, required-nullable distinct from absent, canonical `#rrggbb` codec, ordered JSON-pointer `SpecErrors`, internally tagged enums (never `flatten`); `schemars` opt-in (`spec-schema` ⊃ `spec-json`), schema generated + committed + drift-checked. | Strictness is what makes every drift class fail loudly instead of silently defaulting (HR-12/HR-15); generated schema avoids a hand-mirrored option tree (the 7-vs-9 lesson); opt-in features keep schemars/serde_json out of plain consumers' graphs (HR-18, CK-28). |
| D-9 | **Nix as sole canary author + native parity checker** — `mkWorkspaceSpec` (normalizing producer + vendored pure-Nix schema-subset validator) emits JSON via `pkgs.writeText`; `tools/spec-parity` (private, concrete `WebSpecPlan`/`TuiSpecPlan`, no public trait stack) proves schema/value/provider/disposition parity with ordered pointer diagnostics; permanent negative comparator preserves the seven-vs-nine class; initial red captured before canary migration. | Permanent hand-mirrored fixtures are the wrong ownership model (they stayed green through real drift); bidirectional exact provider comparison + schema-leaf dispositions catch every drift class mechanically (CK-33: no editable expected counts). |
| D-10 | **Dual crane lanes** — host lane (no `CARGO_BUILD_TARGET`, `doCheck=true`, dedicated `rustHost` toolchain, `hostSrc` unions `./tools`) runs `core-unit-tests`, `spec-parity`, `theme-parity`, native TUI; wasm lane unchanged; the jq `layout-canary-schema` check is deleted. | Today's single wasm lane pins `CARGO_BUILD_TARGET=wasm32` + `doCheck=false` (flake.nix:61-62), so no Rust test ever runs in CI (HR-16); `rustWasm` installs only the wasm target (flake.nix:42-44), hence the separate host toolchain. Hydra aggregation picks up new check attrs on both systems automatically. |
| D-11 | **`src/views.rs` resolved — delete the web hook, keep the feature as library primitives + example** *(resolves the analysis-flagged open decision)*: delete web `use_views`/`Views` (a controller composite — it embeds `Workspace<K>` and imports `use_workspace_state`/`save_layout`/`load_layout`, all in the §12.1 deletion ledger); retain core `SavedViews` registry/key-scheme untouched; rewrite `examples/views.rs` as a host-owned composition (per-view `LocalStorageLayoutStore` + `restore_snapshot`/`persist_snapshot` + explicit `SavePolicy` + parts loop); update README §127-144 and add a MIGRATION.md entry. Web-hook deletion and the example rewrite land in the slice-17/18 cutover (after slices 6-7 provide parts/persistence; `src/views.rs` cannot outlive `Workspace`). | A migrated hook would be a retained mini-controller — it owns signals, installs implicit persistence effects, and hides the `SavePolicy` choice — violating the task's own exclusion ("no retained controller facades"), design §16.10, and the rubric's retention anchors. Deleting the feature outright would orphan the retained core registry and silently drop a documented capability. The composition preserves the shipped semantics (switch/rename/delete/first-run legacy copy, key scheme `{base}:views`/`{base}:view:{name}`) with host-owned timing; registry is operator state, correctly outside `WorkspaceSpec`. This amends the §12 ledger as item 12.1.14. |
| D-12 | **Design-spec staleness reconciled** *(resolves the analysis-flagged ~200-line drift)*: the design doc remains the normative authority for semantics (signatures §§4-9, tables §7/§10.4/§11.4, slice order §15, ledger §12); the analysis file's current-tree-verified citations override the design's stale file:line anchors wherever they disagree, and implementers cite current-tree lines. Spot-verified remappings: `use_workspace` 293-470→550-569; `Workspace<K>` →495-532 + impl 753-1405; web `save/load_layout` 249-283→447-485; per-render clone 776-789→1077-1083; `TuiWorkspace` 186-222→191-831; `layout-canary-schema`→flake.nix:126-150; core transition fns at lib.rs:301-1080. | Every semantic claim behind the stale anchors re-verifies on the current tree (clone-per-render, controller ownership, 7-vs-9, wasm-only lane, CSS `:root`) — the drift is positional only, so no design decision changes; the citation rule prevents implementers from editing wrong lines during the ledger diff review (HR-22). |
| D-13 | **Wire compatibility frozen** — `Units`/`Mode`/`WinState` serde spellings unchanged; stable IDs = existing variant names; `mkLayout` untouched (round-trip proven inside `spec-parity`); `PanelCommand` gains only a *new* tagged snake_case serde (it has none today). | Persistence is user data: any respelling invalidates every saved layout; the retained `mkLayout` covers persistence seeding (HR-19, design §13.4). |

### Trade-offs Accepted

- **Clean break over migration shims**: both public controllers and the `use_views` hook are deleted with no aliases (accepting a major-version break and pin-then-migrate coordination for jump-cannon and apple-notes-ocr-flow) in exchange for the no-facade guarantee and a single source of truth (HR-2/HR-22, §16.10).
- **No universal paint IR**: backends keep duplicate *painters* over shared *semantics*; parity covers typed semantics, geometry within unit/rounding rules, provider completeness, and explicit approximations — not pixels (§16.2/§16.4).
- **Strictness over authoring ergonomics**: normalized Nix specs must contain every field (nulls explicit); verbosity is the price of mechanical drift detection (D-8).
- **Wheel-as-Settled semantics**: each accepted workspace-wheel event is one write under `OnSettle`/`OnChange`; high-rate hosts choose `Manual` and coalesce — the core hides no timers or gesture heuristics.
- **Terminal limits as dispositions**: TUI typography/density are `Approximated(reason)` rather than faked; web-only escape hatches (CSS accent color, `table_native`) stay explicitly backend-local.
- **Checker purity over runtime coverage**: `tools/spec-parity` links only pure `spec-plan` lowering, so browser-observed behavior is proven by the wasm build checks + SSR/offscreen component tests instead of the native checker.
- **Schema regeneration as a build step**: contributors regenerate rather than hand-edit `nix/schema/workspace-spec.schema.json` and generated token regions (CK-34); schemars stays major-pinned because its emitted shape may change between versions.

### Architecture Decomposition

Full signatures are normative in design §§4-9; module layout in design §3.1. Components by subsystem (all paths current-tree):

| Component | Path | Responsibility | Reuses From |
|---|---|---|---|
| Identity/catalog | `crates/panel-kit-core/src/panel.rs` (NEW) | `PanelKey`, `SpecPanelId`, `PanelMeta`, `PanelCatalog` (stable-ID maps, dup rejection) | Extends `PanelKind` (core lib.rs:44-50) |
| Reducer | `core/src/reducer.rs` (NEW) | `Snapshot`, `WorkspaceEvent`, `ReduceContext`, `Reduction`, `reduce()` — pure | Delegates to `command_for/apply_command/begin_drag/begin_tile_resize/apply_drag/reorder_tile/restore` (core lib.rs:301-889) |
| Frame projection | `core/src/frame.rs` (NEW) | `ProjectionBuffer`/`ProjectedFrame`/`project_into`, partial projectors, `hit_test`, paint order, `TooSmall` | `effective_rect/front_z/floating_content_height/clamp_scroll` (core lib.rs:674-721) |
| Persistence | `core/src/persist.rs` (NEW) | `LayoutStore`(+`clear`), `SavePolicy`, `restore_snapshot`/`persist_snapshot`, stable-ID codec | Orchestrate `migrate_v1/reconcile_units/merge_defaults` (core lib.rs:1038-1080) |
| Spec | `core/src/spec.rs` (NEW) | strict `WorkspaceSpec` tree, `validate`/`resolve` → `ResolvedWorkspace`, ordered pointer `SpecErrors`, `BindingManifest` | serde/JsonSchema derives on `Clamp/TileMetrics/ChromeMetrics/CommandStep/Key/PanelCommand` |
| Theme | `core/src/theme.rs` (NEW) | `Color`(`#rrggbb`), `ThemeTokens::{dark,paper}` incl. `focus_ring`, typography/density | Absorbs `tokens.rs:32-231`; build.rs:54-93 engine emits consumers |
| Widget models | `core/src/widgets/*` (NEW) | `ContentSpec` (12 kinds), `DataSource`, borrowed `ContentView`, `BadgeSpec`, chart/table/meter/status models | Moves TUI models + color math (`five_num`, `hue_color`, …); extends `core/badge.rs` |
| Core surface | `core/src/lib.rs` (UPDATE) | narrow re-exports; all free fns retained; declare new mods; `views.rs`/`loading.rs` RETAIN | Existing primitives unchanged |
| Web adapters | `src/{input,surface,store,spec_plan,theme}.rs` (NEW) + `src/widgets/*` (NEW) | DOM→event translation, `observe_viewport`, `LocalStorageLayoutStore`, pure `WebSpecPlan`/`lower_spec` (feature `spec-plan`), CSS var emitter, composition parts + painters | Ports from `src/lib.rs:393-445,595-655,1120-1404`; `BrowserStore` reference |
| Web surface | `src/lib.rs` (UPDATE, major), `src/badge.rs` (UPDATE), `src/views.rs` (DELETE per D-11) | controller/persistence/hook deletion; painter over core `BadgeSpec`; doc rewrites | Keeps CSS/BOOT, `LoadingWorkspace`, `PanelHeaderButton`, `Spinner`, `tip_pos`, boot tests |
| TUI adapters | `crates/panel-kit-tui/src/{input,store,theme}.rs` (NEW/UPDATE) + `src/widgets/*` (NEW) | crossterm/ratzilla→events, hit-test adapter, `JsonFileLayoutStore`, conversion-only theme, `draw_*` parts + `TuiHitBuffer` | Ports `Block`/title/inner from tui lib.rs:454-668; `FileLayoutStore` made public |
| TUI surface | `tui/src/lib.rs` (UPDATE, major), painters (UPDATE) | `TuiWorkspace`/`Zone`/backend-local trait deleted; data structs move out | Keeps `Charset`, span/color helpers |
| Parity checker | `tools/spec-parity/` (NEW, unpublished member) | schema diff, strict decode, provider compare, concrete plan lowering, leaf dispositions, pointer diffs, `mkLayout` round-trip, negative comparator | New — core placement would dependency-cycle (design §3.1) |
| Nix surface | `nix/lib/{mkWorkspaceSpec,validateSchema}.nix` (NEW), `nix/schema/*.json` (generated), `nix/specs/{workspace-canary,web-workspace}.nix` (NEW; DELETE `nix/examples/workspace-canary.nix`) | producer/validator/schema/authoritative nine-panel canary | `mkLayout.nix:64-135` DSL pattern; provider manifest from `workspace_canary.rs:52-290` |
| Build lanes | `flake.nix` (UPDATE), `Cargo.toml` (members += tools), `build.rs`/`assets/panel-kit.css`/`DESIGN.md` (UPDATE) | host/wasm crane split, six named checks, generated token regions | crane trunk dual-lane pattern; existing build.rs engine |
| Examples & consumers | `examples/*.rs`, `tui/examples/*.rs` (UPDATE; `views.rs` rewrite per D-11); jump-cannon, apple-notes-ocr-flow | host-owned loops; spec store-path via `include_str!(env!(...))`; `--check-offscreen`; migrations build before pin updates | AGENTS.md:57-69 canaries; MIGRATION.md policy |
| Docs | `README.md`, `MIGRATION.md`, `CHANGELOG.md`, `hydra-project.json` (UPDATE) | composable quick-start, cutover section incl. D-11 break, release notes | MIGRATION.md 0.2→1.0 precedent |

### Data Flow

**Authoring** (build time only; no runtime Nix): `nix/specs/*.nix` attrset → `mkWorkspaceSpec` validates against the committed generated schema (`schemars` ← `core::spec`, drift-checked) → normalized JSON via `pkgs.writeText` → store path consumed as `PANEL_KIT_WORKSPACE_SPEC` by wasm examples (`include_str!(env!(...))`) and by `tools/spec-parity` (strict decode → `validate`/`resolve` → `web::lower_spec`/`tui::lower_spec` → every schema leaf gets a disposition `Applied|ObservedAtRuntime|BoundByHost|Approximated`, repository specs reject `Unsupported`).

**Runtime** (per event, host-owned): native DOM/crossterm event → **host first refusal** → backend `input.rs` translates to `WorkspaceEvent<K>` → `reduce(&mut Snapshot, …) -> Reduction` → host applies `SavePolicy` (persist via store adapter) → `project_into(snapshot, &mut scratch) -> ProjectedFrame` (borrows scratch only) → host's render order over parts (`panel_surface` → `panel_chrome` → lights/grip → app body/header actions → optional dock; TUI `draw_*` over the same frame); frame dropped before any event mutation (compile-fail proof).

### Expected Changes

Consistent with the analysis file: **34 modified / 41 created / 1 file deleted** plus major in-file deletions (both controllers, badge test block, canary mirror, web views hook).

```text
crates/panel-kit-core/         panel.rs reducer.rs frame.rs persist.rs spec.rs theme.rs widgets/* (NEW)
                               lib.rs tokens.rs badge.rs (UPDATE/ABSORB); views.rs loading.rs (RETAIN)
src/                           input.rs surface.rs store.rs spec_plan.rs theme.rs widgets/* (NEW)
                               lib.rs (major deletion), badge.rs loading.rs (UPDATE), views.rs (DELETE, D-11)
crates/panel-kit-tui/          input.rs store.rs widgets/* (NEW); lib.rs (major deletion)
                               theme.rs badge.rs charts.rs table.rs meter.rs status.rs scroll.rs (UPDATE)
tools/spec-parity/             Cargo.toml src/main.rs (NEW; root members += tools)
nix/                           lib/mkWorkspaceSpec.nix lib/validateSchema.nix (NEW); lib/mkLayout.nix (RETAIN)
                               schema/workspace-spec.schema.json (generated, committed)
                               specs/workspace-canary.nix specs/web-workspace.nix (NEW)
                               examples/workspace-canary.nix (DELETE)
flake.nix                      host/wasm lane split, rustHost toolchain, hostSrc += ./tools,
                               six named checks replace layout-canary-schema
build.rs assets/panel-kit.css DESIGN.md   generated token regions (UPDATE)
README.md MIGRATION.md CHANGELOG.md hydra-project.json Cargo.lock (UPDATE)
examples/                      workspace.rs views.rs theming.rs loading.rs loading_workspace.rs editor.rs (UPDATE;
                               views.rs rewritten host-owned per D-11); badge.rs spinner.rs (RETAIN)
crates/panel-kit-tui/examples/ workspace.rs browser_tui.rs workspace_canary.rs (UPDATE; geometry mirror deleted)
external                       jump-cannon, apple-notes-ocr-flow migrate per design §13 (build proofs before pin updates)
```

**Cutover note (HR-22)**: every design §12.1 deletion lands in the same slice as its replacement, amended by D-11 (ledger item 12.1.14: web `use_views`/`Views` hook deleted; core `SavedViews` registry retained; example rewritten). Implementation follows design §15's 18 dependency-ordered TDD slices with each slice's named first-failing proof; ordering amendments from this synthesis: D-11 lands at the slice-17/18 cutover, and the initial seven-vs-nine red is captured at slice 13 before the slice-15 canary migration. Current-tree file:line anchors from the analysis file govern throughout (D-12).

---

## Implementation Process

You MUST launch for each step a separate agent, instead of performing all steps yourself. And for each step marked as parallel, you MUST launch separate agents in parallel.

**CRITICAL:** For each agent you MUST:
1. Use the **Model** and **Agent** type specified in the step's sub-task file (e.g., `haiku`, `sonnet`, `tech-writer`)
2. Provide the path to THIS task file AND the path to that step's sub-task file
3. Require agent to implement exactly that step, not more, not less, not other steps

**CRITICAL:** Verification is done at PHASE level, not per step. When every step of a phase is complete, you MUST launch the code reviewer ONCE for that phase, at the **Reviewer model** named for that phase in the Phase Overview.

### Parallelization Overview

```
01-panel-identity [sonnet]        02-theme-core [sonnet]
        │   (parallel, width 2)           │
        ▼                                 │
03-reducer-commands [sonnet]              │
        │                                 │
═══ end of Phase 1 (review: opus) ═══     │
        │                                 │
        ▼                                 ▼
04-reducer-input [sonnet]   05-widget-models [sonnet]   06-theme-generation [sonnet]
        │                  (parallel, width 3)          │
        │                                 │             │
═══ end of Phase 2 (review: opus) ═══     │             │
        │                                 │             │
   ┌────┴──────┐                          │             │
   ▼           ▼                          │             │
07-frame   09-persistence                 │             │
[opus]     [sonnet]  (width 2)            │             │
   │           │                           │             │
   ▼           ▼                           │             │
08-frame   10-save-policy                 │             │
[sonnet]   [sonnet]  (width 2)            │             │
   │                                       │             │
═══ end of Phase 3 (review: opus) ═══     │             │
   │                                       │             │
   ▼                                       ▼             ▼
11-partial-projectors [sonnet] ∥ 14-web-painters [sonnet] ∥ 15-tui-painters [sonnet]
        │                       (parallel, width 3)
        ▼
12-web-parts [sonnet] ∥ 13-tui-parts [sonnet]  (width 2)
        │
═══ end of Phase 4 (review: opus) ═══
        │
        ▼
16-workspace-spec [opus] ∥ 25-example-migrations [sonnet] ∥ 26-views-cutover [sonnet]
        │                       (parallel, width 3)
        ▼
17-nix-producer [sonnet]
        │
═══ end of Phase 5 (review: opus) ═══
        │
        ▼
18-spec-parity-checker [opus]
        │
        ├──────────────────────────────┬──────────────┐
        ▼                              ▼              │
19-plan-coverage [sonnet] ∥ 20-canary-spec [sonnet]  │
        │                              │  (width 2)   │
═══ end of Phase 6 (review: opus) ═══   │              │
        │                              │              │
        ▼                              ▼              ▼
21-flake-lanes [sonnet]  (needs 18, 19, 20, 10, 06)
        │
        ├──────────────────────────────┬──────────────┘
        ▼                              ▼
22-web-canary-example [sonnet] ∥ 23-tui-native-canary [sonnet]  (width 2)
                                       │
                                       ▼
                       24-browser-tui-canary [sonnet]
                                       │
═══ end of Phase 7 (review: opus) ═══
                                       │
                                       ▼
                       27-controller-cutover [opus]
                                       │
                          ┌────────────┴────────────┐
                          ▼                         ▼
          28-jump-cannon-migration [sonnet] ∥ 29-apple-notes-migration [sonnet]
                                      (width 2)
                                       │
═══ end of Phase 8 (review: opus) ═══
```

| Step | Phase | Model | Agent | Depends on | Parallel with | Sub-Task File |
|------|-------|-------|-------|------------|---------------|---------------|
| `01-panel-identity` [DONE] | Phase 1 | sonnet | developer | None | `02-theme-core` | `.specs/sub-tasks/refactor-panel-kit-composable-library/01-panel-identity.md` |
| `02-theme-core` [DONE] | Phase 1 | sonnet | developer | None | `01-panel-identity` | `.specs/sub-tasks/refactor-panel-kit-composable-library/02-theme-core.md` |
| `03-reducer-commands` [DONE] | Phase 1 | sonnet | developer | `01-panel-identity` | None | `.specs/sub-tasks/refactor-panel-kit-composable-library/03-reducer-commands.md` |
| `04-reducer-input` [DONE] | Phase 2 | sonnet | developer | `03-reducer-commands` | `05-widget-models`, `06-theme-generation` | `.specs/sub-tasks/refactor-panel-kit-composable-library/04-reducer-input.md` |
| `05-widget-models` [DONE] | Phase 2 | sonnet | developer | `02-theme-core` | `04-reducer-input`, `06-theme-generation` | `.specs/sub-tasks/refactor-panel-kit-composable-library/05-widget-models.md` |
| `06-theme-generation` [DONE] | Phase 2 | sonnet | developer | `02-theme-core` | `04-reducer-input`, `05-widget-models` | `.specs/sub-tasks/refactor-panel-kit-composable-library/06-theme-generation.md` |
| `07-frame-projection` [DONE] | Phase 3 | opus | developer | `04-reducer-input` | `09-persistence-core` | `.specs/sub-tasks/refactor-panel-kit-composable-library/07-frame-projection.md` |
| `08-frame-proofs` [DONE] | Phase 3 | sonnet | developer | `07-frame-projection` | `10-save-policy` | `.specs/sub-tasks/refactor-panel-kit-composable-library/08-frame-proofs.md` |
| `09-persistence-core` [DONE] | Phase 3 | sonnet | developer | `04-reducer-input` | `07-frame-projection` | `.specs/sub-tasks/refactor-panel-kit-composable-library/09-persistence-core.md` |
| `10-save-policy` [DONE] | Phase 3 | sonnet | developer | `09-persistence-core` | `08-frame-proofs` | `.specs/sub-tasks/refactor-panel-kit-composable-library/10-save-policy.md` |
| `11-partial-projectors` [DONE] | Phase 4 | sonnet | developer | `08-frame-proofs` | `14-web-painters`, `15-tui-painters` | `.specs/sub-tasks/refactor-panel-kit-composable-library/11-partial-projectors.md` |
| `14-web-painters` [DONE] | Phase 4 | sonnet | developer | `05-widget-models`, `06-theme-generation` | `11-partial-projectors`, `15-tui-painters` | `.specs/sub-tasks/refactor-panel-kit-composable-library/14-web-painters.md` |
| `15-tui-painters` [DONE] | Phase 4 | sonnet | developer | `05-widget-models`, `06-theme-generation` | `11-partial-projectors`, `14-web-painters` | `.specs/sub-tasks/refactor-panel-kit-composable-library/15-tui-painters.md` |
| `12-web-parts` [DONE] | Phase 4 | sonnet | developer | `11-partial-projectors` | `13-tui-parts` | `.specs/sub-tasks/refactor-panel-kit-composable-library/12-web-parts.md` |
| `13-tui-parts` [DONE] | Phase 4 | sonnet | developer | `11-partial-projectors` | `12-web-parts` | `.specs/sub-tasks/refactor-panel-kit-composable-library/13-tui-parts.md` |
| `16-workspace-spec` [DONE] | Phase 5 | opus | developer | `01-panel-identity`, `02-theme-core`, `05-widget-models` | `25-example-migrations`, `26-views-cutover` | `.specs/sub-tasks/refactor-panel-kit-composable-library/16-workspace-spec.md` |
| `25-example-migrations` [DONE] | Phase 5 | sonnet | developer | `06-theme-generation`, `09-persistence-core`, `10-save-policy`, `12-web-parts` | `16-workspace-spec`, `26-views-cutover` | `.specs/sub-tasks/refactor-panel-kit-composable-library/25-example-migrations.md` |
| `26-views-cutover` [DONE] | Phase 5 | sonnet | developer | `12-web-parts`, `16-workspace-spec`, `25-example-migrations` | `17-nix-producer` | `.specs/sub-tasks/refactor-panel-kit-composable-library/26-views-cutover.md` |
| `17-nix-producer` [DONE] | Phase 5 | sonnet | developer | `16-workspace-spec` | None | `.specs/sub-tasks/refactor-panel-kit-composable-library/17-nix-producer.md` |
| `18-spec-parity-checker` [DONE] | Phase 6 | opus | developer | `17-nix-producer` | None | `.specs/sub-tasks/refactor-panel-kit-composable-library/18-spec-parity-checker.md` |
| `19-plan-coverage` [DONE] | Phase 6 | sonnet | developer | `16-workspace-spec`, `18-spec-parity-checker` | `20-canary-spec` | `.specs/sub-tasks/refactor-panel-kit-composable-library/19-plan-coverage.md` |
| `20-canary-spec` [DONE] | Phase 6 | sonnet | developer | `18-spec-parity-checker` | `19-plan-coverage` | `.specs/sub-tasks/refactor-panel-kit-composable-library/20-canary-spec.md` |
| `21-flake-lanes` [DONE] | Phase 7 | sonnet | developer | `06-theme-generation`, `10-save-policy`, `18-spec-parity-checker`, `19-plan-coverage`, `20-canary-spec` | None | `.specs/sub-tasks/refactor-panel-kit-composable-library/21-flake-lanes.md` |
| `22-web-canary-example` [DONE] | Phase 7 | sonnet | developer | `14-web-painters`, `19-plan-coverage`, `20-canary-spec`, `21-flake-lanes` | `23-tui-native-canary` | `.specs/sub-tasks/refactor-panel-kit-composable-library/22-web-canary-example.md` |
| `23-tui-native-canary` [DONE] | Phase 7 | sonnet | developer | `13-tui-parts`, `15-tui-painters`, `19-plan-coverage`, `20-canary-spec`, `21-flake-lanes` | `22-web-canary-example` | `.specs/sub-tasks/refactor-panel-kit-composable-library/23-tui-native-canary.md` |
| `24-browser-tui-canary` [DONE] | Phase 7 | sonnet | developer | `15-tui-painters`, `20-canary-spec`, `21-flake-lanes` | `23-tui-native-canary` | `.specs/sub-tasks/refactor-panel-kit-composable-library/24-browser-tui-canary.md` |
| `27-controller-cutover` [DONE] | Phase 8 | opus | developer | `22-web-canary-example`, `23-tui-native-canary`, `24-browser-tui-canary`, `25-example-migrations`, `26-views-cutover` | None | `.specs/sub-tasks/refactor-panel-kit-composable-library/27-controller-cutover.md` |
| `28-jump-cannon-migration` [DONE] | Phase 8 | sonnet | developer | `27-controller-cutover` | `29-apple-notes-migration` | `.specs/sub-tasks/refactor-panel-kit-composable-library/28-jump-cannon-migration.md` |
| `29-apple-notes-migration` [DONE] | Phase 8 | sonnet | developer | `27-controller-cutover` | `28-jump-cannon-migration` | `.specs/sub-tasks/refactor-panel-kit-composable-library/29-apple-notes-migration.md` |

### Phase Overview

#### Phase 1: Core foundation — identity, theme source, command reducer [REVIEWED]

Steps: `01-panel-identity`, `02-theme-core`, `03-reducer-commands`
Reviewer model: `opus`
Acceptance Criteria that should be fulfiled:
Checklist items:
- `HR-1` — Does host code reduce keyboard, pointer, command, dock restore, wheel, and viewport events via the core reducer without importing a backend? (command-path half: `reduce_key_matches_command_for_and_apply_command`)
- `CK-26` — Does the reducer delegate to the existing `command_for`/`apply_command`/… functions rather than duplicating transition logic? (command path)
- `CK-27` — Is the new code free of duplication that already exists elsewhere (theme values absorbed, not copied)?

Rubrics:
- `Single-Source Theme & Widget Unification`
- `Project Guidelines Alignment`

#### Phase 2: Reducer completion, unified widget models, generated theme consumers [REVIEWED]

Steps: `04-reducer-input`, `05-widget-models`, `06-theme-generation`
Reviewer model: `opus`
Acceptance Criteria that should be fulfiled:
Checklist items:
- `HR-1` — All event classes (pointer move/resize/reorder, wheel precedence, viewport policies, invalid-target no-ops) reduce purely via core: `reducer_preserves_wheel_precedence`, `viewport_resize_policy_is_explicit`
- `HR-10` — One core badge/widget model covering both surfaces; core badge tests live in core (`badge_spec_covers_web_and_tui_fields`; painter halves due Phase 4)
- `HR-11` — One editable theme source with complete web/TUI conversions (`dark_theme_emits_every_web_and_tui_token`; `checks.theme-parity` wiring due Phase 7)
- `CK-26` — Pointer-path delegation verified (`reducer_reuses_existing_command_and_pointer_transitions` complete)
- `CK-27` — Widget models moved (not duplicated) into core
- `CK-34` — DESIGN.md token region and CSS `:root` are generated, never hand-edited

Rubrics:
- `Single-Source Theme & Widget Unification`
- `Project Guidelines Alignment`

#### Phase 3: Borrowed frame projection and core persistence [REVIEWED]

Steps: `07-frame-projection`, `09-persistence-core`, `08-frame-proofs`, `10-save-policy`
Reviewer model: `opus`
Acceptance Criteria that should be fulfiled:
Checklist items:
- `HR-2` — Free functions still usable; standalone core example builds and links no backend (controller-deletion half due Phase 8)
- `HR-3` — Full projected frame contains chrome, deterministic paint order, placement/z/state/focus/drag, hit regions, dock entries, content extent (`projected_frame_resolves_all_semantic_regions`)
- `HR-4` — Zero allocation after reserve/warm-up, no clones, compile-fail borrow proof (`project_into_allocates_zero_after_reserved_warmup`)
- `HR-8` — `LayoutStore` only in core; V1/V2 restore, stable-ID mapping, reconciliation, merge, save, clear (`core_store_restores_v1_reconciles_merges_saves_v2`, `store_clear_is_observable`)
- `HR-9` — Exact SavePolicy write-count table with fake store; reset-clear not re-saving (`save_policy_fake_store_write_counts`)
- `CK-36` — `ProjectedFrame` never stored in signals/memos/props (borrow discipline established; web wiring checked Phase 4)

Rubrics:
- `Projection Performance & Borrow Discipline`
- `Persistence Lifecycle Exactness`
- `Test Evidence Quality`

#### Phase 4: Composition parts and unified painters [REVIEWED]

Steps: `11-partial-projectors`, `14-web-painters`, `15-tui-painters`, `12-web-parts`, `13-tui-parts`
Reviewer model: `opus`
Acceptance Criteria that should be fulfiled:
Checklist items:
- `HR-2` — `project_panel` free of any Snapshot/catalog/store/dock requirement (`project_panel_requires_no_snapshot_catalog_store_or_dock`)
- `HR-5` — Web CSS Grid and TUI cell conversion consume the identical `TileGridProjection` (`web_and_tui_use_same_tile_grid_projection`)
- `HR-6` — Standalone single-surface panel and replace-only-the-dock without a controller (`standalone_surface_has_no_implicit_chrome` + offscreen test; wasm smoke due Phase 7)
- `HR-7` — TUI `Block::title`/`inner` and web semantic DOM/focus/ARIA/overflow/wheel preserved (`tui_panel_inner_matches_projection`, `web_panel_preserves_focus_aria_and_overflow`)
- `HR-10` — Badge/widget unification complete: web tests assert only rendering/actions (`badge_actions_match_across_backends`)
- `CK-27` — Painters consume shared core models; no second geometry or model implementation
- `CK-36` — Web render loop keeps frames render-scoped over non-reactive scratch
- `CK-38` — TUI painters iterate the borrowed frame; no temporary visible/order/dock `Vec`s or per-frame title `String`s

Rubrics:
- `Composability & Clean Cutover`
- `Native Fidelity Preservation`
- `Single-Source Theme & Widget Unification`
- `Projection Performance & Borrow Discipline`

#### Phase 5: Strict WorkspaceSpec, Nix producer, and example migrations [REVIEWED]

Steps: `16-workspace-spec`, `25-example-migrations`, `26-views-cutover`, `17-nix-producer`
Reviewer model: `opus`
Acceptance Criteria that should be fulfiled:
Checklist items:
- `HR-12` — Strict `WorkspaceSpec` rejects unknown/missing fields, reports every semantic error in document order as JSON pointers (`workspace_spec_reports_strict_errors_by_pointer`)
- `HR-18` — Default-feature Cargo-only consumer path free of Nix/schemars/serde_json dependencies (`plain_rust_enum_path_has_no_spec_schema_dependency`)
- `CK-28` — Every serialized struct strict (`deny_unknown_fields`), required-nullable distinct from absent, `spec-json`/`spec-schema` opt-in non-default
- `CK-34` — Generated schema (`nix/schema/workspace-spec.schema.json`) machine-generated, no hand edits

Rubrics:
- `Spec Strictness & Validation Diagnostics`
- `Architecture Economy & Feature Gating`

#### Phase 6: Parity machinery and the authoritative canary [REVIEWED]

Steps: `18-spec-parity-checker`, `19-plan-coverage`, `20-canary-spec`
Reviewer model: `opus`
Acceptance Criteria that should be fulfiled:
Checklist items:
- `HR-13` — Pure Nix expresses every §10.4 completeness row through value/binding/observation/approximation (matrix proven locally; CI green due Phase 7)
- `HR-14` — Historical seven-versus-nine drift fails without an editable expected count, reporting Flame/Distribution plus ordered pointers; permanent negative comparator preserves the class; initial red captured before migration
- `HR-15` — Rust-only, Nix-only, and backend-ignored field drift each make a named check red (`rust_only_field_is_rejected`, `nix_only_field_is_rejected`, `backend_ignored_field_is_rejected`)
- `HR-19` — `mkLayout` remains an unchanged V2 layout-only API with round-trip proven inside the checker
- `CK-33` — No hand-pinned expected values (jq counts or expected-field lists) anywhere in parity checks
- `CK-35` — Parity proofs assert observable behavior, not source text
- `CK-37` — No public plan trait, capability lattice, or semantic-manifest API; checker stays private over concrete plans

Rubrics:
- `Parity Machinery Rigor`
- `Test Evidence Quality`
- `Architecture Economy & Feature Gating`

#### Phase 7: Dual build lanes and the three backend canaries [REVIEWED]

Steps: `21-flake-lanes`, `22-web-canary-example`, `23-tui-native-canary`, `24-browser-tui-canary`
Reviewer model: `opus`
Acceptance Criteria that should be fulfiled:
Checklist items:
- `HR-6` — One-panel web wasm build check green (smoke half)
- `HR-11` — `checks.theme-parity` green in CI
- `HR-13` — `checks.spec-parity` schema/value/plan matrix green in CI
- `HR-16` — `checks.core-unit-tests` and `checks.spec-parity` execute natively (host crane lane, no `CARGO_BUILD_TARGET`) on both Hydra systems
- `HR-17` — `checks.workspace-spec-web-wasm` and `checks.workspace-spec-browser-tui-wasm` build for wasm32 (incl. ASCII override); `checks.workspace-spec-tui-native` builds and runs `--check-offscreen` from the same Nix canary spec, exercising every content kind
- `CK-23` — `nix flake check` passes with all named checks green
- `CK-24` — Clippy gate passes with zero warnings on the changed workspace
- `CK-25` — Rustdoc gate passes with zero warnings
- `CK-29` — Every selected test type (unit, integration, component, contract, smoke) has at least one implemented test/check
- `CK-30` — Every Test Matrix row (main + edge + error) has an implemented test/check
- `CK-31` — Every testable checklist item resolves to at least one real, passing named proof (in-repo set)
- `CK-32` — Every Test Cases to Cover entry implemented (in-repo set; consumer proofs due Phase 8)

Rubrics:
- `Parity Machinery Rigor`
- `Test Evidence Quality`
- `Native Fidelity Preservation`

#### Phase 8: Clean cutover and consumer migrations [REVIEWED]

Steps: `27-controller-cutover`, `28-jump-cannon-migration`, `29-apple-notes-migration`
Reviewer model: `opus`
Acceptance Criteria that should be fulfiled:
Checklist items:
- `HR-2` — Both backend controllers deleted; free functions and partial projectors are the surface (final)
- `HR-20` — jump-cannon supports two independent workspaces sharing no state or store, keys/IDs/resize preserved, building before pin update (`two_concurrent_workspaces_do_not_share_state_or_store`, `jump-cannon-app-ui-wasm`)
- `HR-21` — apple-notes-ocr-flow preserves reviewer/editor priority and persisted IDs; real wasm target builds before pin update (`apple-notes-ocr-flow-reviewer-wasm`)
- `HR-22` — Every §12 deletion (incl. D-11 web `use_views` hook and `src/views.rs`) landed with its replacement; no facade/alias/re-export/stale README wording
- `CK-23` — `nix flake check` green at the final tree
- `CK-24` — Clippy gate zero warnings at the final tree
- `CK-25` — Rustdoc gate zero warnings at the final tree
- `CK-29` — Test-type coverage complete including consumer proofs
- `CK-30` — Test Matrix fully implemented including consumer proofs
- `CK-31` — No orphaned testable checklist items (final)
- `CK-32` — Every Test Cases to Cover entry implemented (final)
- `CK-35` — Cutover/deletion proofs rely on compilation and observable behavior, not source-text tests
- `CK-39` — Scope-appropriate Boy Scout improvements on touched code without scope creep

Rubrics:
- `Composability & Clean Cutover`
- `Project Guidelines Alignment`
