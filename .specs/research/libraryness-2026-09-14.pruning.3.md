# ToT Phase 2 — Pruning report (Judge C)

Judge: PrunJudgeC (independent; no coordination with PrunJudgeA/PrunJudgeB).
Spec applied verbatim: `.specs/research/libraryness-2026-09-14.pruning-spec.yaml`
(20 checklist items; 9 weighted rubric dimensions, weights 0.16/0.15/0.16/0.15/0.08/0.08/0.06/0.06/0.10; essential cap 2.0; −0.25 per important NO; −0.25 per pitfall YES; floor 1.0; ranked choice with big-four tie-break and cross-mechanism diversity rule).

Scoring formula used: `overall = clamp_min(1.0, min(weighted_sum, 2.0 if any essential NO) − 0.25×(important NO) − 0.25×(pitfall YES))`.

---

## 0. Citation spot-checks (spec judge-note: ≥1 per proposal; CK-007/CK-008 grounded here)

Verified directly against the working tree (`/Users/casazza/Repositories/ocasazza/panel-kit`):

| Claim (proposal) | Verification |
|---|---|
| `theme.rs:10-37` `Theme`, `:42-74` `DARK`/`PAPER` (A1, B2, C-A, C-E) | **EXACT** — `crates/panel-kit-tui/src/theme.rs` Theme at 10-37, DARK 42-56, PAPER 60-74 |
| `flake.nix:6-8` dioxus-0.6 pin, `:19-30` farm constraints, `:46-56` crane fileset, `:82-83` `layout-canary`, `:97-107` `update-version` (`cargo set-version`), `:111` `lib = { mkLayout, winStates, tileWMax, tileHMax }`, `:118-138` jq `layout-canary-schema`, `:143-146` `browser-tui-example`, `:155-166` devshell lockstep (A1, A2, A5, B1, B2, B4, C-D, C-F) | **ALL EXACT** line-for-line |
| `mkLayout.nix:1-16` schema comment, `:29` "kinds … (for codegen)", `:43-46` `toFloat`, `:48-81` `normalizePanel`, `:83-102` emit (A1, A3, B4, C-D, C-E) | **ALL EXACT** |
| `nix/examples/workspace-canary.nix:20-26` `kind = "Workspace"`; hand-mirror of `defaults()` with no Rust round-trip in checks (A1, B1, B2, C-D) | **CONFIRMED** — flake comment at `flake.nix:74-75` admits "a declarative mirror of the TUI canary's `defaults()`"; only jq shape check |
| `AGENTS.md` "Platform events should be translated at backend boundaries into core input types" (C-B), "If behavior should match, encode the behavior in core first" (A1, B1), "Examples are executable documentation" (A5) | **EXACT quotes** — AGENTS.md:18-23, :27 |
| `use_workspace(storage_key: &'static str, defaults: fn() -> Vec<PanelWin<K>>)` (all) | **EXACT** — `src/lib.rs:341-344` (current tree) |
| `TuiWorkspace::new(store: Option<PathBuf>, defaults: fn())` (all) | **EXACT** — `crates/panel-kit-tui/src/lib.rs:230` (current tree) |
| `TuiWorkspace::dragging` (C-B), `zones`/`dock_chips`/`hover` internals (A2, B6, C-E), `Charset` tui:83-90, modules tui:36-42 | **CONFIRMED** — :331, :211-213, :85, :35-42 (current) |
| `BadgeKind::Entity { ty }` `badge.rs:34-37` (B2) | **EXACT** |
| `workspace_canary.rs:7-34` `Panel`, `:36-50` `defaults()`, `Metrics`/`noise()` ≈:112-257 (A3, A5, C-D) | **CONFIRMED** (Panel :7-8, defaults :36-50, noise :117, Metrics :128) |
| `examples/workspace.rs` storage key `panel_kit_example_workspace` (A1, cites :42) | **CONFIRMED** at :46 (line off by 4; string real) |
| `panel_body_absorbs_wheel` `src/lib.rs:169-210` (A2, C-B), `recompute` `:331-348` (C-B) | **CONFIRMED** — :211 and :377 in current tree (context-era numbering) |
| **`ROW_CELLS` "TUI cell metrics" (C-B risk, C-E risk)** | **FABRICATED** — repo-wide grep (incl. gitignored `target/`) finds `ROW_CELLS` only in the proposal file itself. Real cell metrics are `TileMetrics::CELLS` / `Clamp::CELLS` / `CELLS_COMPACT_MAX`. → CK-008 YES for C-B and C-E |

**Repo drift finding (material, does not trigger CK-008 — spec says verify against repo *or* ground-truth context, and every cited symbol exists in `local://panel-kit-context.md`; proposals' line numbers match the context-era layout).** The working tree has moved past the context snapshot:

- `crates/panel-kit-core/src/lib.rs` is now **1240 lines** (context: ~620). All cited symbols exist (`begin_drag` :699, `merge_defaults` :1050, `SavedLayout` :959, `Clamp::WEB` :583, `kind_slug` :1062 …), but proposals' core line cites are context-era.
- **Persistence evolved**: core now has versioned `SavedLayoutV2`/`StoredLayout` (untagged V1|V2), `migrate_v1`, `reconcile_units`, `Units`, `LAYOUT_SCHEMA_VERSION`; the TUI crate **already ships `pub trait LayoutStore`** (:150, load/save over String JSON) + `FileLayoutStore` (:158); web persists V2 with automatic V1 migration. Proposals presenting "JSON file persistence hardcoded" / "no persistence abstraction" describe the context snapshot.
- **Breakpoint moved to core**: `viewport_is_mobile` no longer exists in `src/lib.rs`; core has `SurfaceProfile`/`SurfaceClass`/`effective_mode` + `WEB_COMPACT_MAX = 760.0`/`WEB_TABLET_MAX`/`CELLS_*`; `Workspace` now carries `profile: Signal<SurfaceProfile>` + `focused`. The "760px hardcoded in the web shell" diagnosis is stale (the policy is still not caller-supplied, but it *is* core-side and tiered).
- **Keyboard layer exists in core**: `Key`/`KeyChord`/`FocusContext`/`PanelCommand`/`command_for`/`apply_command`/`CommandStep` — C-B's proposed `enum Event` must subsume these.
- **README changed**: "Public API unchanged" no longer exists; README:86-87 now says "Breaking 0.2.x consumers should start with the [1.0 migration guide](MIGRATION.md)" (and `MIGRATION.md` is referenced but **missing** from the repo). Consumer-break constraint is softer than every proposal assumes.

Expansion agents MUST re-baseline all diagnoses against the current tree (see §Selection rationale).

---

## 1. Per-proposal evaluations

Dimensions: CG=Composability Gain (0.16), CCF=Core-Contract Fidelity (0.15), NSC=Nix Spec Coverage (0.16), PPS=Parity Proof Strength (0.15), NS=Net Simplification (0.08), MR=Migration Realism (0.08), TF=Technical Feasibility (0.06), RC=Risk Concreteness (0.06), TDD=TDD Sliceability (0.10).

### A1 — Headless core reducer + pluggable store + escape-hatch shells

| CK | Ans | Evidence |
|---|---|---|
| CK-001 | YES | All seven fields present |
| CK-002 | YES | "`defaults` generalizes from the `fn()` pointer (src/lib.rs:300) to an owned value"; "escape hatches beside the shell"; persistence as core trait |
| CK-003 | YES | "Core … gains a plain-data `WorkspaceState<K>`"; "`LocalStore` in the Dioxus crate and `JsonFileStore` in the TUI crate"; Palette core-side |
| CK-004 | YES | spec covers "chrome metrics … palette, per-panel badges and widget/content bindings … persistence `{ key, format, policy }`, mobile breakpoint, and charset" |
| CK-005 | YES | "a new native check binary (`spec-check`) wired into `checks`: it deserializes the JSON … re-serializes, and asserts equality" |
| CK-006 | YES | "`examples/spec-workspace.rs` seeds from the same JSON, making the Nix↔Rust interchange a canary on both backends" |
| CK-007 | YES | `begin_drag` core:447, `use_workspace` src/lib.rs:298, flake.nix:118-138 — all verified above |
| CK-008 | NO | No fabricated file/symbol/attribute found |
| CK-009 | YES | Respects dioxus 0.6 / wasm-bindgen pins; everything in crane/nix |
| CK-010 | YES | Reducer "deletes the duplicated lifecycle"; spec type "enables the Nix interchange" — each addition tied |
| CK-011 | YES | "the two consumers migrate with one constructor change"; "README's 'Public API unchanged' promise … breaks once" |
| CK-012 | YES | "persisted layouts from existing users (localStorage keys like `panel_kit_example_workspace` …) keep loading unchanged" |
| CK-013 | YES | Distinct from A2 (shell dissolved) and A3 (contract-only); differs in ≥3 of A-file's 7 dimensions |
| CK-014 | NO | Plain-cargo consumers unaffected; spec is an authoring path |
| CK-015 | NO | Mirror machine-checked by `spec-check`, not hand-trusted |
| CK-016 | NO | Shell remains but pieces exported beside it; no new mandatory handle |
| CK-017 | NO | 0.90 vs "medium-high" complexity; risks mostly evidenced/mitigated (`toFloat` precedent cited) — not the high-complexity+unmitigated pattern |
| CK-018 | NO | Theme→core Palette; widget bindings as spec enums; no per-backend re-definition |
| CK-019 | NO | Both halves: composition changes + feature-complete spec |
| CK-020 | NO | Approach + API sketches only |

Dimensions (score / evidence quote):
- CG 3.5 — "the web `Workspace<K>` keeps `render(body)` … but the crate additionally exports the granular pieces" — pieces independent, shell still the default owner of the tree.
- CCF 4.5 — "load/merge/save/mode/drag ownership exists exactly once" in core; backends reduced to adapters.
- NSC 4.5 — full 11-item list incl. "widget/content bindings (table/chart/meter/status …)" with an enum mechanism.
- PPS 4 — "deserializes … re-serializes, and asserts equality against a golden JSON emitted by a Rust unit test"; kind validation via string-kind impl.
- NS 4 — deletes both shells' hand-rolled lifecycle; flags own cost: "`WorkspaceSpec` vs `SavedLayout` are two JSON contracts".
- MR 3.5 — constructor change named for both consumers; persisted JSON safe; but no old/new signatures.
- TF 4.5 — reuses mkLayout's documented serde contract and `toFloat` float-stability precedent; crane check bin.
- RC 4 — "golden-JSON flake from float formatting (`builtins.toJSON` vs serde_json …)" with repo evidence; Action-bikeshed unmitigated.
- TDD 4 — enumerated work items (reducer, store, schema, examples, checks) each with an evident first test.

**Weighted 4.065 → penalties 0 → overall 4.07 (strong).**

### A2 — Composition-first: dissolve the shell into a user-owned tree

| CK | Ans | Evidence |
|---|---|---|
| CK-001 | YES | All seven fields |
| CK-002 | YES | "Invert render ownership outright: backends ship state handles plus per-panel/per-dock primitives the app assembles itself" |
| CK-003 | **NO** | State container stays per-backend ("`use_workspace_state(...) -> WorkspaceSignals`" web-side; TUI keeps its own); the cross-backend `WorkspaceSpec`/string-id model has no stated core home — "each backend gains a generic `spec_runner`" leaves the spec type unplaced |
| CK-004 | YES | "palette → `Theme` … charset, persistence policy" in the runner spec |
| CK-005 | YES | "a snapshot check that the runner's normalized state equals the Rust canary's" + `checks` job wasm32/native |
| CK-006 | YES | "a `checks` job builds it for native (TUI) and wasm32 (web runner, matching the `browser-tui-example` precedent at flake.nix:143-146)" |
| CK-007 | YES | `zones`/`dock_chips`/`hover` tui:168-179, `panel_body_absorbs_wheel` — verified |
| CK-008 | NO | None found |
| CK-009 | YES | Reuses browser-tui-example crane precedent; no pin changes |
| CK-010 | YES | Each primitive justified by user-owned-tree composition |
| CK-011 | YES | Facades "keep … jump-cannon and apple-notes-ocr-flow compiling" (staged) |
| CK-012 | **NO** | Persisted-`SavedLayout` JSON compatibility never addressed |
| CK-013 | YES | Distinct from A1/A3 (render-ownership axis) |
| CK-014 | NO | No Nix dependency for ordinary use |
| CK-015 | NO | End-to-end snapshot comparison, not hand-mirror |
| CK-016 | NO | Facades deprecated; no new handle |
| CK-017 | **YES** | 0.85 probability alongside "Complexity: high" and unmitigated risks ("Dioxus 0.6 component/prop model may force awkward `Signal` re-creation", "runner widget bindings becoming a second, weaker widget API that rots") |
| CK-018 | NO | Widget registry binds existing modules; no new per-backend duplicate definition |
| CK-019 | NO | Both halves present |
| CK-020 | NO | Sketches only |

Dimensions: CG 5 ("the app places inside its own `rsx!` tree in any order"); CCF 2.5 (state per-backend, spec home unstated); NSC 4 (palette/charset/persistence + widget bindings via registry); PPS 4.5 ("parity … asserted by a snapshot check that the runner's normalized state equals the Rust canary's"); NS 3.5 ("publishing what `TuiWorkspace::render` already does internally — deleting the monopoly"); MR 4 (facades + README:52-70 compile-target named); TF 4 (browser-tui precedent at flake.nix:143-146); RC 4 ("facade and pieces drift … needs a shared-state test"); TDD 3.5.

**Weighted 3.92 → essential cap 2.0 (CK-003 NO) → −0.25 (CK-012) − 0.25 (CK-017) → overall 1.50 (invalid).**

### A3 — Contract-first: schemars schema export + typed Nix options + golden parity

| CK | Ans | Evidence |
|---|---|---|
| CK-001 | YES | All seven |
| CK-002 | YES | "the `LayoutStore` trait + non-`fn()`-pointer defaults from A1 (subproblems 1-2)" — names fn-pointer/persistence mechanisms |
| CK-003 | YES | "core (or a tiny `panel-kit-spec` crate re-exporting core types) gains `schemars` derives" — types defined core-side |
| CK-004 | YES | "`WorkspaceSpec` (panels, mode, `ChromeMetrics` …, palette, badges/widgets, persistence policy, breakpoint, charset)" |
| CK-005 | YES | "a check re-evaluates the Nix spec to JSON and `diff`s byte-for-byte" + Rust validator binary |
| CK-006 | YES | Existing canaries retained; new checks exercise Nix path (weakly: no new wasm example named) |
| CK-007 | YES | mkLayout.nix:43-46 `toFloat`, workspace_canary.rs:36-50 — verified |
| CK-008 | NO | None found |
| CK-009 | YES | "schemars version churn inside the pinned toolchain (0.8 vs 1.0 …)" — risk named, pins respected |
| CK-010 | YES | Versioned envelope justified by persisted-layout evolution |
| CK-011 | YES | "near-zero churn for consumers" — impact stated (though it borrows A1's signature change; see MR) |
| CK-012 | YES | "`SavedLayout` stays the persistence format"; versioned envelope from day one |
| CK-013 | YES | Contract-tooling mechanism, distinct from A1's state model and A2's render dissolution |
| CK-014 | NO | Schema consumed by Nix, never required by Rust consumers |
| CK-015 | NO | Committed golden diff is machine-produced |
| CK-016 | NO | No new handle |
| CK-017 | NO | 0.82 / medium / mitigations present ("generate option stubs from the schema") |
| CK-018 | NO | Palette unified in core |
| CK-019 | NO | Both halves (modest Rust changes + full Nix contract) |
| CK-020 | NO | Sketches only |

Dimensions: CG 2.5 ("shells keep their shape this phase … leaves the framework smells (IoC render, god-handle) mostly intact"); CCF 4 (spec types + Palette core; backends untouched); NSC 4.5 (typed submodule options over full list); PPS 4.5 (golden diff + probe validation + reverse round-trip); NS 3 (adds schemars + coverage checks; deletes little); MR 2.5 ("near-zero churn for consumers" while adopting A1's `defaults`-signature change — inconsistent); TF 4.5; RC 4.5 ("option↔schema coverage check giving false confidence … mitigate: generate option stubs"); TDD 4.5 (derives → export → options → probe → golden).

**Weighted 3.825 → penalties 0 → overall 3.83 (viable).**

### A4 — Instruction-stream core: interpreted command algebra

| CK | Ans | Evidence |
|---|---|---|
| CK-001..CK-005 | YES | All essentials: shell control removed ("shells emit these commands"), everything in core interpreter, Nix tape feature-complete by construction, trace-diff check named |
| CK-006 | YES | Trace check + `checks.browser-tui-example` precedent (flake.nix:143-146, verified) |
| CK-007 | YES | core free fns, `PointerEvent` core:85-116, flake.nix:143-146 — verified |
| CK-008 | NO | None found |
| CK-009 | YES | Runs in devshell/crane |
| CK-010 | YES | Command tapes named as the concrete composition ("capabilities literally compose") |
| CK-011 | YES | "the migration cost for jump-cannon/apple-notes-ocr-flow is total" — stated |
| CK-012 | **NO** | Persisted-JSON compatibility unaddressed |
| CK-013 | YES | Distinct algebraic-interpreter region |
| CK-014..CK-016 | NO | No Nix requirement for library use; enum+fn is not a handle/facade |
| CK-017 | NO | 0.04 / high — consistent |
| CK-018 | NO | Single core semantics |
| CK-019 | NO | Both halves (tape = spec; Rust model re-modeled) |
| CK-020 | NO | Sketches only |

Dimensions: CG 4.5 ("User code composes command sequences as values"); CCF 5 (interpreter subsumes all free fns in core); NSC 4.5 ("everything any Rust example does is a command sequence"); PPS 4.5 ("recording canonical command traces … and diffing them against Nix-emitted tapes"); NS 1.5 ("maximal added abstraction, directly against the task's 'prefer deleting/simplifying' constraint" — own words); MR 2 (total cost named, no signatures); TF 2.5 ("`Draw` commands fight both renderers' native models … interaction ordering semantics … undefined" — own admission); RC 4 (tape versioning, batching ownership, trace churn — specific); TDD 3.

**Weighted 3.835 → −0.25 (CK-012) → overall 3.59 (viable).**

### A5 — Nix as the source of truth: build-time codegen of the Rust examples

| CK | Ans | Evidence |
|---|---|---|
| CK-001 | YES | All seven |
| CK-002 | **NO** | Removes no framework mechanism: it "delet[es] the hand-maintained mirror" (a Nix artifact), leaves `Workspace::render(body)`, the `fn()` pointer, storage key, persistence all untouched; own risk: "consumers see no library-ness improvement at all" |
| CK-003 | YES(weak) | "spec schema shared with core types so the generator never invents fields" |
| CK-004 | YES | "panel enums, `defaults()`, palettes, chrome metrics, persistence keys" generated from spec |
| CK-005 | YES | "`nix build .#workspace-tui-canary` compiles generated code" — drift impossible; build fails on divergence |
| CK-006 | YES | wasm32 + native generation wired into crane fileset |
| CK-007 | YES | `LayoutBuilder` core:278-307, workspace_canary.rs:7-34/:36-50, flake.nix:46-56 — verified |
| CK-008 | NO | None found |
| CK-009 | YES | "Generator written in Rust … runs native in the devshell" |
| CK-010 | YES | Generator justified by zero-drift |
| CK-011 | YES | Consumers unaffected (stated) |
| CK-012 | **NO** | Persisted-JSON compatibility unaddressed |
| CK-013 | YES | Distinct truth-direction mechanism |
| CK-014 | NO | Library consumers don't need Nix; only example generation does |
| CK-015 | NO | Generation, not mirroring |
| CK-016 | NO | No handle |
| CK-017 | NO | 0.06 / high — consistent |
| CK-018 | NO | None added per-backend |
| CK-019 | **YES** | "only the Nix spec with no change to the Rust composition model" — exactly this proposal's shape |
| CK-020 | NO | Sketches only |

Dimensions: CG 1.5; CCF 2 ("the contract now lives in the generator's templates, not in `panel-kit-core` types" — own admission); NSC 4.5; PPS 5 (definitional); NS 2 ("an entire pipeline added, against the repo's 'prefer deleting/simplifying' constraint"); MR 3; TF 3 (Hydra eval cost, sandboxing of generated source path); RC 4; TDD 3.5.

**Weighted 3.18 → essential cap 2.0 (CK-002 NO) → −0.25 (CK-019) − 0.25 (CK-012) → overall 1.50 (invalid).**

### A6 — Pure-data workspace: registry + string ids, `WorkspaceDoc`

CK-001 YES; CK-002 YES (deletes `PanelKind` genericity, doc-driven persistence); CK-003 YES(weak) (`PanelWin` non-generic = core change; `WorkspaceDoc` necessarily neutral); CK-004 YES (doc covers "palette, chrome metrics, widget bindings, persistence policy, breakpoint, charset"); CK-005 YES (serialize canary doc, diff vs Nix); CK-006 YES(weak); CK-007 YES (`reorder_tile`/`restore` `K: PartialEq + Copy` bounds verified at current :785/:808; `mkLayout.nix:13-16` exact); CK-008 NO; CK-009 YES; CK-010 YES (registry justified: "dynamic panels, spec==state, zero-code Nix apps"); CK-011 YES ("both git-dependency consumers would need deep rewrites"); CK-012 YES ("`SavedLayout` already serializes kinds as bare variant-name strings … wire-compatible"); CK-013 YES; CK-014 NO; CK-015 NO; CK-016 NO (registry is injected data, not a global handle — borderline, noted); CK-017 NO (0.08/high consistent); CK-018 NO; CK-019 NO; CK-020 NO.

Dimensions: CG 3.5 ("Runtime-defined panels, plugins, and user-authored layouts all fall out"); CCF 3.5 (identity model in core; registry home unstated); NSC 4.5 (full doc); PPS 5 ("one schema, one type — no mirror semantics"); NS 3 (deletes genericity; "weakens core's cheapest invariant (`Copy + Eq + Hash`)" — own note); MR 2 ("consumer migration cost likely fatal"); TF 3.5 ("Nix can describe a chart but not an event handler — 'feature complete' hits a philosophical wall" — honest); RC 4; TDD 3.5.

**Weighted 3.755 → penalties 0 → overall 3.76 (viable).**

### B1 — Core `WorkspaceState` + `LayoutStore` trait: shells become thin adapters

| CK | Ans | Evidence |
|---|---|---|
| CK-001 | YES | All seven |
| CK-002 | YES | "`use_workspace` becomes `use_workspace(store: impl LayoutStore<K>, defaults: impl Fn() …)` — real closures, replaceable key/format/backend" |
| CK-003 | YES | "`WorkspaceState::restore(store, defaults)`"; "the *mechanisms* (gloo, fs) stay in backends as impls" |
| CK-004 | **NO** | Nix scope unchanged: "a new `spec-roundtrip` flake check … deserializes `packages.layout-canary` … into `SavedLayout<K>`" — geometry only; no theme/chrome/widgets/persistence/breakpoint/charset inputs |
| CK-005 | YES | spec-roundtrip deserializes and "asserts it equals `…workspace_canary.rs::defaults()`" |
| CK-006 | YES | Native crane check; existing canaries kept |
| CK-007 | YES | core lib.rs:447-564, flake.nix:82-83 — verified |
| CK-008 | NO | None found |
| CK-009 | YES | No pin changes |
| CK-010 | YES | Struct "simply own[s] the whole set" over existing fns; trait deletes two hardcodings |
| CK-011 | YES | "`use_workspace`'s signature breaks both consumers (the README 'Public API unchanged' promise ends)" |
| CK-012 | YES | "`SavedLayout` wire format untouched: existing users' persisted layouts survive" |
| CK-013 | YES | State-model minimalism, distinct in-file |
| CK-014 | NO | — |
| CK-015 | NO | Machine-checked round-trip |
| CK-016 | NO | — |
| CK-017 | NO | 0.85 / medium — not the high-complexity pattern |
| CK-018 | NO | — |
| CK-019 | **YES** | "only Rust library-ness with no Nix spec mechanism" beyond today's scope — the Nix half is a stronger check of the *existing* geometry DSL, not a feature-complete spec |
| CK-020 | NO | Sketches only |

Dimensions: CG 3 ("render IoC remains (body closure still inverts control)" — stated); CCF 4.5; NSC 1.5 (SavedLayout only); PPS 4 (real value equality, geometry scope only); NS 4; MR 3.5 (signature break named for both consumers; persisted JSON survives); TF 4.5; RC 4 ("if transitions aren't actually moved, `WorkspaceState` becomes dead packaging" — sharp); TDD 4.

**Weighted 3.505 → essential cap 2.0 (CK-004 NO) → −0.25 (CK-019) → overall 1.75 (invalid).**

### B2 — Schema-first `WorkspaceSpec`: one document, schemars contract

CK-001 YES; CK-002 YES (storage policy "key + backend selector" in spec; breakpoint "currently hardcoded in `viewport_is_mobile`" spec'd); CK-003 YES (spec + theme tokens "into core as RGB triples, like `badge::Rgb`"); CK-004 YES (full list incl. "per-panel content bindings scoped exactly to what the canary examples render"); CK-005 YES ("`spec-parity` check deserializes the Nix JSON into `WorkspaceSpec`, re-serializes, and byte-compares"); CK-006 YES; CK-007 YES (`BadgeKind::Entity` badge.rs:34-37 EXACT); CK-008 NO; CK-009 YES ("wasm32 schemars compatibility is fine"); CK-010 YES; CK-011 **NO** (zero consumer mention — neither breaking changes nor an unchanged-API claim); CK-012 **NO** (already-persisted JSON compatibility unaddressed); CK-013 YES; CK-014 NO; CK-015 NO ("deleting the hand-mirror in `nix/examples/workspace-canary.nix`" — examples consume the spec); CK-016 NO; CK-017 NO (0.82/high borderline; conversion-drift risk names its test mitigation — given benefit of the doubt); CK-018 NO; CK-019 NO; CK-020 NO.

Dimensions: CG 3; CCF 4.5; NSC 5 (full list + slug→K mechanism via `kind_slug`/`PanelKind::title`, content capped at canary parity); PPS 4.5; NS 3.5 (deletes hand-mirror + per-example defaults; adds schemars surface needing versioning); MR 2.5 (silent on consumers); TF 4.5 (`include_str!` precedent = CSS at src/lib.rs:109 — verified :114); RC 4 ("Schema churn red-flags downstream flakes pinning `lib.${system}`" — specific); TDD 4.5.

**Weighted 4.07 → −0.25 (CK-011) − 0.25 (CK-012) → overall 3.57 (viable).**

### B3 — Non-breaking facade: capability modules beside compat front-ends

| CK | Ans | Evidence |
|---|---|---|
| CK-001 | YES | All seven |
| CK-002 | YES | Persistence hardcodings refactored; breakpoint/clamp/theme exposed via `use_workspace_with(WorkspaceConfig {…})` |
| CK-003 | **NO** | "new `panel_kit::persist::{LayoutStore, LocalStorage, JsonFile, Noop}` types" — the cross-backend store trait is defined in the Dioxus (wasm32-only) crate the TUI crate cannot even depend on; AGENTS.md violation as written |
| CK-004 | YES | "Nix grows as a toolbox … (`mkTheme`, `mkChrome`, `mkPanelWin` …)" |
| CK-005 | YES | "the same Rust round-trip check as B1" |
| CK-006 | YES | Existing checks kept; round-trip added |
| CK-007 | YES | core lib.rs:145-191/385-404/566-581 symbols verified |
| CK-008 | NO | None found |
| CK-009 | YES | — |
| CK-010 | YES | RenderModel composition named ("users … call `layout_model` directly and skip `Workspace::render`") |
| CK-011 | YES | "Keep `use_workspace(storage_key, defaults)` and `TuiWorkspace::new(store, defaults)` byte-compatible for jump-cannon/apple-notes-ocr-flow" |
| CK-012 | YES | Byte-compatible entry points; persistence refactor internal-only (serde shape definitionally untouched) |
| CK-013 | YES | Migration-posture mechanism, distinct in-file |
| CK-014 | NO | — |
| CK-015 | NO | Round-trip check |
| CK-016 | NO | Config is opt-in beside the legacy path (god-config *risk* acknowledged) |
| CK-017 | NO | 0.80 / medium |
| CK-018 | NO | Spinner gets core model + two renderers; TUI-only modules deferred, not duplicated |
| CK-019 | NO | Both halves |
| CK-020 | NO | Sketches only |

Dimensions: CG 3 ("the framework path remains the default … §1 … only partially honored" — stated); CCF 3 (RenderModel in core ✓; store trait in web crate ✗); NSC 3 (toolbox; no content bindings/persistence policy/charset); PPS 4; NS 2.5 ("two ways to do everything … persists indefinitely" — stated); MR 4.5; TF 4.5; RC 4.5 ("If `RenderModel` output ever diverges … a third semantics … enforced by shell-refactor + golden tests"); TDD 4 ("every extraction ships independently").

**Weighted 3.51 → essential cap 2.0 (CK-003 NO) → overall 2.00 (invalid).**

### B4 — Nix as source of truth: generate Rust from the spec at build time

CK-001 YES; CK-002 **NO** (no framework mechanism removed — shells, `render(body)`, `fn()` pointer, persistence all untouched; only the mirror dies); CK-003 **NO** (behavior lands in a generated crate + generator, not core; "Nix is the schema"); CK-004 YES (enum, `DEFAULTS`, `THEME`, `CLAMP` consts); CK-005 YES ("`nix flake check` builds the canary examples against it — Nix spec → working Rust app proven by compilation"); CK-006 YES; CK-007 YES (mkLayout.nix:29 "for codegen" EXACT; flake.nix:97-107 update-version EXACT); CK-008 NO; CK-009 YES (writeShellApplication/Rust bin inside flake); CK-010 YES; CK-011 YES ("Downstream git-dependency consumers (jump-cannon) can't see generated crates unless committed"); CK-012 **NO**; CK-013 YES; CK-014 NO (library consumers use the lib crate; generated crate is example-level — but see MR/NS); CK-015 NO (definitional); CK-016 NO; CK-017 NO (0.08/high consistent); CK-018 NO; CK-019 **YES** (Nix half only; no Rust composition change); CK-020 NO.

Dimensions: CG 1.5; CCF 2; NSC 3 (kinds/layout/theme/clamp; no content bindings, charset, breakpoint); PPS 5; NS 1.5 ("an entire pipeline added, against the repo's 'prefer deleting/simplifying' constraint" — own words); MR 3; TF 2.5 ("Circularity: crane needs a source tarball before it can build the generator" — own admission); RC 4.5; TDD 3.

**Weighted 2.85 → essential cap 2.0 (CK-002, CK-003 NO) → −0.25 (CK-019) − 0.25 (CK-012) → overall 1.50 (invalid).**

### B5 — Data-driven registry: runtime `PanelId`, plugin providers, spec-executing engine

CK-001 YES; CK-002 YES (`render(body)` closures disappear — though replaced by registry IoC); CK-003 YES (descriptors = "one model, two interpreters", core-side); CK-004 YES (spec configures "panels, geometry, theme, charset, breakpoint, and persistence"); CK-005 **NO** (no `nix flake check` job named anywhere — parity is by construction/prose); CK-006 **NO** (no wasm/native/Nix check mentioned); CK-007 YES; CK-008 NO; CK-009 YES; CK-010 YES; CK-011 YES ("jump-cannon/apple-notes-ocr-flow need total rewrites"); CK-012 **NO**; CK-013 YES; CK-014 NO; CK-015 NO; CK-016 **YES** ("a registry is a service locator, providers are inversion of control the user cannot bypass" — own admission); CK-017 NO (0.05 consistent); CK-018 NO; CK-019 NO; CK-020 NO.

Dimensions: CG 2; CCF 4; NSC 4.5; PPS 3; NS 1.5 ("the descriptor language is a UI framework in disguise" — own words); MR 1.5; TF 3; RC 4 (exceptionally honest self-critique); TDD 3.

**Weighted 3.05 → essential cap 2.0 (CK-005 NO) → −0.25 (CK-006) − 0.25 (CK-016) − 0.25 (CK-012) → overall 1.25 (invalid).**

### B6 — Dissolve the monolith: independent leaf crates, no Workspace type

CK-001 YES; CK-002 YES (shells deleted from public API); CK-003 YES (neutral leaves; render kits only); CK-004 YES (`mkTheme` named among leaf composables); CK-005 YES (per-leaf round-trip checks); CK-006 **NO** ("Canary coverage shrinks because the canaries *were* the shells" — own admission); CK-007 YES; CK-008 NO; CK-009 YES; CK-010 YES; CK-011 YES ("Both consumers break despite the README promise"); CK-012 **NO**; CK-013 YES; CK-014 NO; CK-015 NO; CK-016 NO (anti-handle); CK-017 NO (0.04 consistent); CK-018 NO; CK-019 NO; CK-020 NO.

Dimensions: CG 5; CCF 4.5; NSC 2 ("'feature-complete Nix spec' gets *harder* (no single spec target)" — own admission; no content bindings); PPS 3.5; NS 4.5 (max surface deletion); MR 1.5; TF 3; RC 4.5; TDD 3.

**Weighted 3.55 → −0.25 (CK-006) − 0.25 (CK-012) → overall 3.05 (weak).**

### C-A — Core-Owned Reducer with Injectable Store (schema-first Nix spec)

CK-001 YES; CK-002 YES ("`&'static str` key + `fn()` pointer in `use_workspace`; `Option<PathBuf>` in `TuiWorkspace::new`" — the two hardcodings deleted); CK-003 YES (`WorkspaceState<K>` + `LayoutStore` + core `Theme` + spec types in core; "backends shrink to adapters"); CK-004 YES (registry, chrome, theme tokens, content bindings, persistence policy, breakpoint, charset); CK-005 YES ("`spec-canary-check` reads `packages.spec-canary` JSON and `serde_json::from_str::<WorkspaceSpec<TestPanel>>` it — replacing the jq-only `layout-canary-schema`"); CK-006 YES (spec_workspace wasm32 + native, both in checks); CK-007 YES; CK-008 NO; CK-009 YES (schemars behind a `schema` feature); CK-010 YES; CK-011 YES ("named breaking change for jump-cannon/apple-notes-ocr-flow, mechanical to fix"); CK-012 **NO** (persisted-JSON compatibility not addressed — note: current repo already has V1→V2 `migrate_v1`, so the omission is more serious than the explorer knew); CK-013 YES; CK-014 NO; CK-015 NO; CK-016 NO; CK-017 NO (0.85/medium); CK-018 NO; CK-019 NO; CK-020 NO.

Dimensions: CG 3.5 ("the state and store are public, so an app can own the loop" but "the shell still owns the render tree by default" — stated); CCF 4.5; NSC 4.5 ("today's two parallel 14-field palettes collapse into one" — deletion lever named); PPS 4.5; NS 4 (palette unification = genuine deletion; additions each tied); MR 3.5 (closure breaking change named; no signatures; persisted data unmentioned); TF 4.5; RC 4 ("ResizeObserver … must stay as an adapter concern" — correct boundary call); TDD 4.

**Weighted 4.14 → −0.25 (CK-012) → overall 3.89 (viable).**

### C-B — Headless Event Reducer + Frame Projection; Backends as Widget Kits

CK-001 YES; CK-002 YES ("Delete the shell-as-controller entirely"; wheel policy becomes `Event::Wheel { body_scrolls }`); CK-003 YES (`Snapshot`/`Event`/`reduce`/`project` + `PanelId` in core; AGENTS boundary rule quoted verbatim — verified exact); CK-004 YES (full spec list); CK-005 YES ("a native Rust binary that `reduce`/`project`-drives the spec into a `Frame` and asserts golden values"); CK-006 **NO** (native golden check + Nix path yes; no wasm32 web-example check named); CK-007 YES (`TuiWorkspace::dragging` verified :331; `recompute` verified); CK-008 **YES** ("must pin `Clamp::WEB`/`TileMetrics::WEB` and TUI cell metrics (`ROW_CELLS`)" — `ROW_CELLS` does not exist anywhere in the repo); CK-009 YES; CK-010 YES; CK-011 YES ("both consumers break beyond a signature change (their event handlers and render calls move)"); CK-012 YES-weak ("`SavedLayout`/`merge_defaults` as today" — shape-preservation statement); CK-013 YES; CK-014 NO; CK-015 NO; CK-016 NO; CK-017 NO (0.80/medium); CK-018 NO; CK-019 NO; CK-020 NO.

Dimensions: CG 5 ("user owns the loop; every chrome piece callable; persistence policy host-owned"); CCF 5; NSC 4.5 (full list + `PanelId(Arc<str>)` mechanism); PPS 4.5 (behavioral golden-Frame parity); NS 3.5 (publishes existing internals as boundary; `Event` sprawl risk admitted); MR 3 (impact named qualitatively; no old/new signatures); TF 4 (Dioxus-reactivity risk real and named); RC 4.5 (approach-specific with pinned constants — minus the fabricated `ROW_CELLS`); TDD 4.5 ("property-testable in core without a DOM or terminal").

**Weighted 4.425 → −0.25 (CK-008) − 0.25 (CK-006) → overall 3.93 (viable).**

### C-C — Config-Object Facelift: `WorkspaceConfig` + Runtime Spec Parsing

CK-001 YES; CK-002 YES ("kills the `&'static str`/`fn()` pair and both persistence hardcodings"); CK-003 YES ("a new `panel-kit-spec` module (core feature)"; core `Theme` token struct); CK-004 YES (`mkSpec { panels?; registry; mode; chrome; theme; content; persistence; breakpoint; charset }`); CK-005 YES ("a small native `spec-decode` check binary that deserializes every spec the flake exports"); CK-006 YES ("a wasm web example and a native TUI example, both wired into `checks`"); CK-007 YES (`src/lib.rs:40-60` doc example — verified EXACT in current tree); CK-008 NO; CK-009 YES; CK-010 YES (with discipline rule: "every option must name the hardcoding it deletes"); CK-011 YES ("`defaults` accepting a closure … is the single breaking change to the two consumers"); CK-012 **NO**; CK-013 YES; CK-014 NO; CK-015 NO ("one Nix file consumed by both sides" — mirror deleted); CK-016 NO; CK-017 NO (0.82/low); CK-018 NO (widget asymmetry deferred as documented non-goal, not duplicated); CK-019 NO; CK-020 NO.

Dimensions: CG 2.5 ("the IoC critique is *not* answered" — own admission); CCF 3.5 ("state ownership stays duplicated per backend" — own admission); NSC 3.5 (workspace surface complete; "the TUI canary's charts/table content is not" — own admission vs "totally feature complete"); PPS 4; NS 4; MR 4.5 ("consumers change one call" with doc-example citation); TF 5; RC 4.5; TDD 4.5.

**Weighted 3.785 → −0.25 (CK-012) → overall 3.54 (viable).**

### C-D — Nix as the Source of Truth: Spec-Compiled Examples

CK-001 YES; CK-002 YES-weak (shells "reduce to `SpecViewer` implementations"; hosts can go direct); CK-003 **NO** ("A new `panel-kit-spec` crate is the *sole* Rust definition of the spec" — the shared spec type is defined outside `panel-kit-core`; `Snapshot` home unstated); CK-004 YES (NixOS-module spec covering layout, registry, chrome, theme, content model, persistence, breakpoint, charset); CK-005 YES (matrix builds + vendored sync diff + embedded deserialization); CK-006 YES (web-wasm + tui-native + browser-tui jobs); CK-007 YES (flake.nix:74-83 mirror comment EXACT; `normalizePanel` mkLayout.nix:48-81 EXACT); CK-008 NO; CK-009 YES (vendored fallback keeps plain cargo working); CK-010 YES; CK-011 YES ("weakest fit for jump-cannon/apple-notes-ocr-flow … migrate by authoring a Nix spec (or … `WorkspaceConfig::with_defaults`)"); CK-012 **NO**; CK-013 YES; CK-014 NO; CK-015 NO (single-description); CK-016 NO; CK-017 NO (0.06/high); CK-018 NO; CK-019 NO; CK-020 NO.

Dimensions: CG 3.5; CCF 3.5; NSC 5; PPS 5 (by construction + sync check); NS 3.5; MR 2.5; TF 3 ("Nix evaluation becomes a build prerequisite for content changes"; module-system-in-lib-export unusualness — own admissions); RC 4.5 ("the `Metrics`/`noise()` animation machinery in `workspace_canary.rs:112-257` doesn't fit literal data" — verified real); TDD 3.5.

**Weighted 3.915 → essential cap 2.0 (CK-003 NO) → −0.25 (CK-012) → overall 1.75 (invalid).**

### C-E — Shell Deletion: Capability Kit and Metadata-as-Data

CK-001 YES; CK-002 YES (deletes the `Workspace<K>` bundle + 430-line impl, `TuiWorkspace` + 500-line impl, load/save pairs, `PanelKind::title`); CK-003 YES (`Snapshot`/`Event`/`project` + `PanelMeta` + one theme token set in core; kits flat and stateless); CK-004 YES (NixOS-module `mkSpec` incl. TUI widget kinds); CK-005 YES ("native decode+golden-project check … the canary *is* the proof that the kit composes"); CK-006 YES (every backend builds a kit-fed canary); CK-007 YES (`kind_slug` core, `zones` private — verified); CK-008 **YES** ("Golden-`Frame` checks for TUI depend on `Charset`/`ROW_CELLS` invariants" — `ROW_CELLS` fabricated); CK-009 YES; CK-010 YES; CK-011 YES ("both consumers break hard … semver 0.3 + migration doc required"); CK-012 **NO**; CK-013 YES; CK-014 NO; CK-015 NO; CK-016 NO (anti-handle by design); CK-017 NO (0.05/medium consistent); CK-018 NO; CK-019 NO; CK-020 NO.

Dimensions: CG 5; CCF 4.5; NSC 4.5; PPS 4.5; NS 5 (most deletion-biased; "zero persistence in-library"); MR 2; TF 3.5 ("Losing `TuiWorkspace::zones` hit-testing … the recipe must ship it or the kit is unusable" — honest); RC 4.5; TDD 3.5 (delete-then-rebuild; big-bang risk).

**Weighted 4.26 → −0.25 (CK-008) − 0.25 (CK-012) → overall 3.76 (viable).**

### C-F — Spec Compiler with Capability-Assembled Shells and a Parity Matrix

CK-001 YES; CK-002 YES (shells "re-implemented as *demonstrated compositions* of published capabilities" — the god-object answered structurally); CK-003 YES (`Kernel<K>` + widget view-models in core, "extending the existing pattern of `panel_kit_core::badge`"); CK-004 YES (all dimensions incl. TUI widgets); CK-005 YES ("a decode check per spec (`spec-version pinning` so a schema change fails every cell)" + matrix); CK-006 YES (matrix extends `checks.browser-tui-example` flake.nix:143-146 + hydraJobs — verified); CK-007 YES; CK-008 NO; CK-009 YES (all inside crane/nix; farm capacity risk flagged against flake.nix:19-30 — verified); CK-010 **NO** (IR `spec_version` versioning "without real downstream spec users it is speculative generality" — own risk concedes an addition not tied to a concrete composition); CK-011 YES ("jump-cannon keeps `use_workspace`-shaped composition with default capabilities … the README promise survives as a compatibility profile"); CK-012 YES (versioned IR "so old persisted specs keep loading (generalizing what `merge_defaults` does)"); CK-013 YES; CK-014 NO; CK-015 NO; CK-016 NO (shell = one composition, not privileged); CK-017 NO (0.08/high consistent); CK-018 NO (view-models once in core); CK-019 NO; CK-020 NO.

Dimensions: CG 4.5 ("each a small standalone value the host can replace or omit"); CCF 5; NSC 5 (full list, NixOS modules, registry/PanelMeta); PPS 5 ("a spec feature no backend renders is an empty matrix cell that fails"); NS 1.5 ("the heaviest design on the table … directly tensions with the task's 'prefer deleting/simplifying'" — own admission); MR 4 (compat profile from the same assembly); TF 3.5 (matrix cost vs farm capacity, flake.nix:19-30); RC 5 (every risk approach-specific with failure signals); TDD 3.5.

**Weighted 4.32 → −0.25 (CK-010) → overall 4.07 (strong).**

---

## 2. Comparison table (all 18)

| ID | Name (short) | CG | CCF | NSC | PPS | NS | MR | TF | RC | TDD | Weighted | Gate/penalties | Overall | Label |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| A1 | Headless reducer + store + escape hatches | 3.5 | 4.5 | 4.5 | 4 | 4 | 3.5 | 4.5 | 4 | 4 | 4.065 | — | **4.07** | strong |
| A2 | Dissolve shell, user-owned tree | 5 | 2.5 | 4 | 4.5 | 3.5 | 4 | 4 | 4 | 3.5 | 3.92 | cap(CK-003); −0.25 CK-012; −0.25 CK-017 | **1.50** | invalid |
| A3 | Contract-first schemars + golden | 2.5 | 4 | 4.5 | 4.5 | 3 | 2.5 | 4.5 | 4.5 | 4.5 | 3.825 | — | **3.83** | viable |
| A4 | Cmd interpreter algebra | 4.5 | 5 | 4.5 | 4.5 | 1.5 | 2 | 2.5 | 4 | 3 | 3.835 | −0.25 CK-012 | **3.59** | viable |
| A5 | Nix→Rust codegen | 1.5 | 2 | 4.5 | 5 | 2 | 3 | 3 | 4 | 3.5 | 3.18 | cap(CK-002); −0.25 CK-019; −0.25 CK-012 | **1.50** | invalid |
| A6 | String-id registry + WorkspaceDoc | 3.5 | 3.5 | 4.5 | 5 | 3 | 2 | 3.5 | 4 | 3.5 | 3.755 | — | **3.76** | viable |
| B1 | Core state + LayoutStore | 3 | 4.5 | 1.5 | 4 | 4 | 3.5 | 4.5 | 4 | 4 | 3.505 | cap(CK-004); −0.25 CK-019 | **1.75** | invalid |
| B2 | Schema-first WorkspaceSpec | 3 | 4.5 | 5 | 4.5 | 3.5 | 2.5 | 4.5 | 4 | 4.5 | 4.07 | −0.25 CK-011; −0.25 CK-012 | **3.57** | viable |
| B3 | Non-breaking facade capabilities | 3 | 3 | 3 | 4 | 2.5 | 4.5 | 4.5 | 4.5 | 4 | 3.51 | cap(CK-003) | **2.00** | invalid |
| B4 | Nix source of truth, codegen | 1.5 | 2 | 3 | 5 | 1.5 | 3 | 2.5 | 4.5 | 3 | 2.85 | cap(CK-002, CK-003); −0.25 CK-019; −0.25 CK-012 | **1.50** | invalid |
| B5 | Registry/provider engine | 2 | 4 | 4.5 | 3 | 1.5 | 1.5 | 3 | 4 | 3 | 3.05 | cap(CK-005); −0.25 CK-006; −0.25 CK-016; −0.25 CK-012 | **1.25** | invalid |
| B6 | Leaf crates, no Workspace | 5 | 4.5 | 2 | 3.5 | 4.5 | 1.5 | 3 | 4.5 | 3 | 3.55 | −0.25 CK-006; −0.25 CK-012 | **3.05** | weak |
| C-A | Core reducer + injectable store, schema-first | 3.5 | 4.5 | 4.5 | 4.5 | 4 | 3.5 | 4.5 | 4 | 4 | 4.14 | −0.25 CK-012 | **3.89** | viable |
| C-B | Event reducer + projection, widget kits | 5 | 5 | 4.5 | 4.5 | 3.5 | 3 | 4 | 4.5 | 4.5 | 4.425 | −0.25 CK-008; −0.25 CK-006 | **3.93** | viable |
| C-C | Config-object facelift + runtime spec | 2.5 | 3.5 | 3.5 | 4 | 4 | 4.5 | 5 | 4.5 | 4.5 | 3.785 | −0.25 CK-012 | **3.54** | viable |
| C-D | Nix authors, Rust embeds | 3.5 | 3.5 | 5 | 5 | 3.5 | 2.5 | 3 | 4.5 | 3.5 | 3.915 | cap(CK-003); −0.25 CK-012 | **1.75** | invalid |
| C-E | Shell deletion, capability kit | 5 | 4.5 | 4.5 | 4.5 | 5 | 2 | 3.5 | 4.5 | 3.5 | 4.26 | −0.25 CK-008; −0.25 CK-012 | **3.76** | viable |
| C-F | Spec compiler + capabilities + parity matrix | 4.5 | 5 | 5 | 5 | 1.5 | 4 | 3.5 | 5 | 3.5 | 4.32 | −0.25 CK-010 | **4.07** | strong |

Essential-gate failures: A2 & B3 & C-D (CK-003), A5 & B4 (CK-002), B1 (CK-004), B5 (CK-005). Pitfall YES: CK-008 → C-B, C-E (`ROW_CELLS` fabricated); CK-016 → B5; CK-017 → A2; CK-019 → A5, B4, B1.

---

## 3. Selection rationale

**Ranked order (tie-breaks per spec):** C-F and A1 tie at 4.07; big-four sum (CG+CCF+NSC+PPS): C-F 19.5 > A1 16.5 → C-F 1st, A1 2nd. C-B 3rd (3.93). Then C-A 3.89, A3 3.83, C-E 3.76 (big-4 18.5) > A6 3.76 (big-4 16.5), A4 3.59, B2 3.57, C-C 3.54, B6 3.05, B3 2.00, C-D 1.75 (big-4 17.0) > B1 1.75 (big-4 13.0), A2 1.50 (big-4 16.0) > A5 1.50 (13.0) > B4 1.50 (11.5), B5 1.25.

**Diversity rule check on the top 3:** A1 (state-in-core + store + escape hatches + versioned spec) vs C-F (Nix→IR compiler, capability-assembled shells, matrix CI) vs C-B (Elm reducer + `project()` + widget kits) — three distinct organizing mechanisms; no pair fails CK-013. Noted overlap: C-F's Layer 3 explicitly reuses C-B's `Kernel` (snapshot/reduce/project) and both share `PanelId` + NixOS-module authoring; this is component reuse, not a shared core mechanism, but expansion agents must de-duplicate deliberately. Also noted: A1 and C-A (4th) are near-duplicates of each other (core `WorkspaceState` + `LayoutStore` + schemars-validated spec + native round-trip check) — expanding both would waste a slot; the A1 expansion should absorb C-A's theme-unification deletion lever and both-direction canaries.

**1st — C-F (4.07, strong).** The only proposal that turns "feature-complete" into a falsifiable CI artifact ("a spec feature no backend renders is an empty matrix cell that fails"), puts every shared behavior in core (`Kernel`, view-models extending the existing `panel_kit_core::badge` pattern), answers the god-object critique structurally (shells as demonstrated compositions of replaceable capabilities), and keeps a compat profile for jump-cannon/apple-notes-ocr-flow. Concerns the expansion agent MUST address:
- **Deletion budget (its one rubric failure, CK-010/NS 1.5):** name what gets DELETED, not just re-composed — e.g. does the capability shell replace `use_workspace`'s hardcoded lifecycle outright? Cut `spec_version` IR versioning or justify it against the *existing* `StoredLayout` V2/`migrate_v1`/`LAYOUT_SCHEMA_VERSION` machinery (repo already has a versioned persistence migration pattern to generalize — cite it instead of inventing a parallel one).
- **Re-baseline on the current tree** (see §0 drift): TUI already ships `pub trait LayoutStore`; breakpoint is already core (`SurfaceProfile`, `WEB_COMPACT_MAX`); keyboard commands exist (`command_for`/`apply_command`/`KeyChord`) and the capability set must include them.
- **Matrix cost vs the Hydra farm** (`flake.nix:19-30`): bound the matrix (N specs × backends) or gate cells.
- Consumer compat profile needs concrete old/new signatures (currently prose).

**2nd — A1 (4.07, strong).** The pragmatic center of mass: wraps the existing free functions in a core `WorkspaceState` (deleting the duplicated load/merge/save lifecycle in both shells), a minimal string-payload store trait, closure defaults, escape-hatch pieces beside the optional shell, and a versioned spec with a real round-trip check. Concerns:
- **Absorb C-A/B2 deltas:** core `Theme` unification as the deletion lever (two parallel palettes → one), and make examples *consume* the Nix JSON (B2's `include_str!` pattern) so no mirrored values remain.
- **Two JSON contracts** (`WorkspaceSpec` vs `SavedLayout`): must be reconciled with the *current* `StoredLayout` V1|V2 untagged enum + `migrate_v1` + `reconcile_units`; state explicitly that existing localStorage/JSON files still load.
- **Store trait already exists TUI-side** — design the core trait as a lift-and-generalize of `panel_kit_tui::LayoutStore` (:150), not a new invention; web `LocalStore` replaces private `save_layout`/`load_layout` (src/lib.rs:246/:266).
- Cap widget/content bindings at canary parity (its own scope-creep risk); name old/new `use_workspace`/`TuiWorkspace::new` signatures for both consumers.

**3rd — C-B (3.93, viable).** The strongest pure library-ness answer that still passes the essential gate: Elm-style `Event`/`reduce` + pure `project()` in core, backends demoted to individually-callable widget kits, persistence policy host-owned, `PanelId(Arc<str>)` closing the Nix↔Rust identity gap without registry machinery. Concerns:
- **Fabricated citation (CK-008):** replace `ROW_CELLS` with real cell metrics (`TileMetrics::CELLS`, `Clamp::CELLS`, `CELLS_COMPACT_MAX`).
- **`Event` must subsume the existing keyboard layer** (`KeyChord`/`PanelCommand`/`command_for`/`apply_command`, now in core) plus the `SurfaceProfile` viewport-reclassification path — the proposal predates these.
- **Name a wasm32 web-example check** (CK-006 failed) — mirror the `browser-tui-example` crane job.
- **Concrete breaking signatures** for jump-cannon/apple-notes-ocr-flow (event handlers and render calls move — show the diff), and the Dioxus `Signal`-wrapping story for the convenience shell (its own top risk).

**Not selected, briefly:** C-A (3.89) — excellent but a near-duplicate of A1 (diversity rule would block the pair); A3 (3.83) — strongest parity tooling but leaves pillar-1 IoC intact and its migration claim is self-inconsistent; C-E (3.76) — best deletion story, fatal consumer cost; A6 (3.76) — degenerate parity but stringly identity and fatal migration; B2 (3.57) — silently ignores consumers and persisted data; the seven capped proposals fail an essential gate (most importantly: any approach that leaves the Nix spec geometry-only (B1) or removes no framework mechanism (A5/B4) cannot meet the binding interpretation).

---

RANKING:
  1: C-F Spec Compiler with Capability-Assembled Shells and a Parity Matrix
  2: A1 Headless core reducer + pluggable store + escape-hatch shells
  3: C-B Headless Event Reducer + Frame Projection; Backends as Widget Kits
SCORES:
  A1: 4.07/5.0
  A2: 1.50/5.0
  A3: 3.83/5.0
  A4: 3.59/5.0
  A5: 1.50/5.0
  A6: 3.76/5.0
  B1: 1.75/5.0
  B2: 3.57/5.0
  B3: 2.00/5.0
  B4: 1.50/5.0
  B5: 1.25/5.0
  B6: 3.05/5.0
  C-A: 3.89/5.0
  C-B: 3.93/5.0
  C-C: 3.54/5.0
  C-D: 1.75/5.0
  C-E: 3.76/5.0
  C-F: 4.07/5.0
