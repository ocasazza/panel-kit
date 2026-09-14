# ToT Phase 2 — Pruning report, Judge A (PrunJudgeA)

- Rubric: `.specs/research/libraryness-2026-09-14.pruning-spec.yaml`, applied verbatim (20 checklist items, 9 weighted dimensions, essential cap 2.0, −0.25 per important NO / pitfall YES, floor 1.0, ranked choice + diversity rule).
- All 18 proposals scored (A1–A6, B1–B6, C-A…C-F). Scores are default-2 disciplined: every score ≥3 is backed by a quoted excerpt in the dimension tables. Exactly 8 of 162 dimension judgements (4.9%) are 5s, per the <5% rule.
- Labels: **strong** ≥ 3.6 · **viable** 2.5–3.59 · **invalid** = essential-gate cap applied or floor reached (no uncapped proposal landed in 1.5–2.49, so no "weak" label was used).
- Probability self-reports were used only for CK-017, never as scoring evidence. Length/risk-count/confident tone not rewarded; Nix coverage measured against the fixed 11-item list only.

## Repo-state note for citation verification (important)

`local://panel-kit-context.md` and all three proposal files describe **HEAD `6120e9b` (2026-07-27)**: core `crates/panel-kit-core/src/lib.rs` = 620 lines, `SavedLayout<K>` current, no keyboard surface. The **working tree has a large uncommitted refactor** on top of HEAD (core → 1240 lines: `SavedLayoutV2`/`StoredLayout`/`Units`/`migrate_v1`/`reconcile_units`, `SurfaceProfile`/`SurfaceClass`/`effective_mode` with `WEB_COMPACT_MAX = 760.0` now in core, `Key`/`KeyChord`/`PanelCommand`/`command_for`/`apply_command`, `floating_content_height`; src/lib.rs → 966; TUI → 823). Citations were therefore verified against **HEAD** (`git show HEAD:<path>`), the state the proposals were written against. Consequences are recorded under *Selection rationale → concerns*; they do not penalize explorers (CK-008 asks whether cited files/symbols/attrs exist — they do).

## Citation spot-checks (CK-007/CK-008 evidence)

Verified at HEAD (symbol → cited location → actual):

| Claim | Cited as | Actual at HEAD | Verdict |
|---|---|---|---|
| core `begin_drag` / `apply_drag` / `merge_defaults` | core:447 / 497 / 597 | 447 / 497 / 597 | exact |
| core `PanelKind` trait bounds | core:35-41, 29-41 | trait at 35-38 | exact |
| core `SavedLayout`, `ChromeMetrics`, `Clamp`(+`Clamp::WEB` 333, `TileMetrics::WEB` 370), `kind_slug` 609, `LayoutBuilder` 280, `PanelWin` 242, `PointerEventKind` 85, `PointerEvent` 109 | various | all present | real |
| TUI `TuiWorkspace::new(store, defaults)` 186, `save` 230, `render(f,area,body)` 251, `dragging` 243, `Charset` 84-90, modules 35-42, `Zone` 136, fields/zones/dock_chips/hover 156-180/169-171, `ROW_CELLS` 64 | various | all present | real |
| web `pub const CSS` src/lib.rs:109; `:root` vars :64-71; `viewport_is_mobile` 760px :142-153; `panel_body_absorbs_wheel` :169-210; `use_workspace(storage_key: &'static str, defaults: fn()…)` | various | CSS 109 exact, `:root` 64-71 exact, 760.0 at 144, wheel fn 170, `use_workspace` at **275** (cited 298) | symbols real; systematic +23…+41 line offset in src/lib.rs only |
| `theme.rs` `Theme` 10, `DARK` 42 / `PAPER` 60 | theme.rs:7-37, 42-74 | 10 / 42 / 60 | real |
| `flake.nix`: lib export 111 (exact), `layout-canary-schema` jq 118-138, `browser-tui-example` 143-146, `layout-canary` pkg 82-83, `update-version` 98-108, wasm toolchain 42-45, fileset 46-56, wasm-bindgen 0.2.121 lockstep ~160-164, farm constraints 19-30 (exact), `hydraJobs` 179 | various | all present | real |
| `nix/lib/mkLayout.nix`: serde-schema comment 1-28, kind-string convention 13-16, `kinds` "for codegen" 28, `toFloat` 43-46 (exact), `normalizePanel` 50, `{value,json,kinds}` emit 96-104 | various | all present | real |
| README 26-27 "Public API unchanged…" (exact), 9-15 git consumers, 48-50 `PanelKind`+body callback, 99 examples/`LayoutBuilder`, 132 `[patch…]` | various | all present | real |
| AGENTS.md:21-23 "Platform events should be translated at backend boundaries into core input types" (quoted by C-B); :19-20 "encode the behavior in core first"; :25-36 canaries/wasm32 | 3-23, 28-36 | verbatim | real |
| `workspace_canary.rs` `defaults()` 36-50 (exact), `Panel` enum 7-34 (exact), `noise()`/`WINDOW` 111-117, `STAGES` "color-matched to apple-notes' stages" 145-146 (exact, C-F), imports `tag_hue, BadgeKind` line 1 | various | all present | real |
| `examples/workspace.rs` `STORAGE_KEY = "panel_kit_example_workspace"` ~42, 4-variant `Panel` 59-68 (A6) | 42, 59-76 | 42-43, 60-67 | real |
| `badge.rs` `BadgeKind::Entity { ty }` 33-38 | 34-37 | 33-38 | real |

**CK-008 verdict: NO for all 18 proposals.** No cited file, symbol, or flake attribute is absent from the repository. The only systematic error is a +23…+41 line offset in `src/lib.rs` citations (`use_workspace` 298 vs 275; `impl Workspace` 399 vs 367; `Spinner` 850 vs 809) — these line numbers are copied from `local://panel-kit-context.md` itself, so it is a context-file artifact, not fabrication; every symbol and structural claim checks out.

**Bonus ground-truth finding (supports the parity-proof premise):** at HEAD the hand-mirror has *already drifted*: `nix/examples/workspace-canary.nix` declares **7 panels** (no `Flame`, no `Distribution`; `Notes` at y=27 h=13) while `workspace_canary.rs::defaults()` declares **9 panels** (`Flame` y=27 h=11, `Notes` y=39 h=13). The jq shape check (`layout-canary-schema`) passes nonetheless — proof that shape-only parity is already failing silently.

---

## A1 — Headless core reducer + pluggable store + escape-hatch shells

Gist: core gains `WorkspaceState`/`Action` wrapping existing free fns + a two-method `LayoutStore` trait; shells collapse to constructors; granular per-panel pieces published beside the shell; Nix grows a versioned `WorkspaceSpec` proven by a native round-trip check.

| CK | ans | evidence |
|---|---|---|
| CK-001 | YES | all seven fields present (Probability 0.90, Complexity medium-high, risks) |
| CK-002 | YES | "`defaults` generalizes from the `fn()` pointer…to an owned value"; save-on-settle "becomes explicit data"; render bypassable via published pieces |
| CK-003 | YES | "load/merge/save/mode/drag ownership exists exactly once"; `LayoutStore` core trait, `LocalStore`/`JsonFileStore` backend impls; `Palette` + `WorkspaceSpec` in core |
| CK-004 | YES | names mode, `ChromeMetrics`, palette, badges + widget/content bindings, persistence `{key, format, policy}`, breakpoint, charset |
| CK-005 | YES | "`spec-check`…deserializes the JSON produced from `nix/examples/workspace-canary.nix`, re-serializes, and asserts equality" — replaces jq check |
| CK-006 | YES | "A web example (`examples/spec-workspace.rs`) seeds from the same JSON"; TUI canary; "three flake checks" |
| CK-007 | YES | core:447/497, mkLayout.nix:83, flake.nix:118-138, workspace_canary.rs:36-50 — all verified |
| CK-008 | NO | spot-check passed |
| CK-009 | YES | crane-built check bins inside `nix develop`; pins untouched |
| CK-010 | YES | reducer "wraps existing free fns — deletes the duplicated lifecycle"; spec type "enables the Nix interchange" |
| CK-011 | YES | "the two consumers migrate with one constructor change"; README promise "breaks once and must be re-anchored" |
| CK-012 | YES | "persisted layouts from existing users (localStorage keys like `panel_kit_example_workspace`…) keep loading unchanged" |
| CK-013 | YES | shell+hatches hybrid; distinct from A2 (dissolve) and A3 (schema-first) |
| CK-014 | NO | spec JSON additive; plain-cargo use unaffected |
| CK-015 | NO | round-trip check replaces hand-mirror |
| CK-016 | NO | shell optional beside public pieces; no new facade |
| CK-017 | NO | 0.90 w/ medium-high (not high) complexity; major risks carry mitigations — borderline, documented |
| CK-018 | NO | `Palette` core-once; `Theme::from(&Palette)`/CSS block are projections |
| CK-019 | NO | both halves addressed |
| CK-020 | NO | API sketches only |

| dimension (w) | score | evidence |
|---|---|---|
| Composability (0.16) | 4 | "exports the granular pieces those are made of — `panel_element(ws, idx, body)`, a dock component"; body-callback remains the default path |
| Core-Contract (0.15) | 4 | state/store/theme/spec all core; web `Workspace` Signal bundle persists as adapter wrapper |
| Nix Coverage (0.16) | **5** | full 11-item list + "a new core type `WorkspaceSpec` (serde) is the single interchange contract" |
| Parity Proof (0.15) | 4 | deserialize → re-serialize → "asserts equality against a golden JSON"; canary `defaults()` still authored in Rust (checked, not derived) |
| Net Simplification (0.08) | 4 | "`use_workspace`…and `TuiWorkspace::new` collapse to constructors"; `fn()` pointer + `&'static str` key deleted |
| Migration (0.08) | 4 | one-constructor-change named; persisted-JSON status stated; no staged alias |
| Feasibility (0.06) | 4 | crane check bins, `include_str!` seeding, `toFloat` float-stability (mkLayout.nix:43-46) reused |
| Risk (0.06) | 3 | float flake + scope creep mitigated; "`Action` bikeshed", CSS-class coupling unmitigated |
| TDD (0.10) | 4 | reducer → store → spec → check → examples; first test evident per slice |

Weighted **4.10** · penalties: none · **final 4.10 — strong**

## A2 — Composition-first: dissolve the shell into a user-owned tree

Gist: backends ship state handles + per-panel/per-dock primitives the app assembles; current shells become deprecated facades; Nix completeness via generic spec-runner examples + snapshot check.

| CK | ans | evidence |
|---|---|---|
| CK-001 | YES | all fields (0.85, high, risks) |
| CK-002 | YES | "the app assembles itself"; wheel heuristic "ships as a function apps invoke, not hidden shell behavior" |
| CK-003 | **NO** | "`WorkspaceSignals` (state + action methods + persistence, no DOM)" lives in the web crate; theme stays TUI `Theme` (theme.rs:10); "web runner parity lags until web widgets exist" — no core homes for state container/persistence/theme/widget model |
| CK-004 | YES | "builds an entire workspace from a `WorkspaceSpec` JSON — string panel ids bound to widget modules…, palette → `Theme`, charset, persistence policy" |
| CK-005 | YES | "a snapshot check that the runner's normalized state equals the Rust canary's for the same input" + build checks |
| CK-006 | YES | "`checks` job builds it for native (TUI) and wasm32 (web runner…`browser-tui-example` precedent at flake.nix:143-146)" |
| CK-007 | YES | tui:168-179 zones/dock_chips (verified 169-171), flake.nix:143-146, core:109-116 |
| CK-008 | NO | spot-check passed |
| CK-009 | YES | no toolchain changes |
| CK-010 | YES | publishing internals = "deleting the monopoly, not the code"; facade+pieces each justified |
| CK-011 | YES | shells "survive as deprecated facades…which is what jump-cannon and apple-notes-ocr-flow compile against today" |
| CK-012 | **NO** | persisted SavedLayout JSON compatibility never addressed |
| CK-013 | YES | distinct within file A |
| CK-014 | NO | no Nix requirement for library use |
| CK-015 | NO | runner consumes Nix output; snapshot check compares |
| CK-016 | NO | no mandatory handle |
| CK-017 | **YES** | 0.85 alongside "Complexity: high" and unmitigated risks (Signal re-creation, ratatui churn, runner rot) |
| CK-018 | **YES** | "the runner's widget registry is only as complete as the TUI widget set" — widget/content model stays per-backend |
| CK-019 | NO | both halves addressed |
| CK-020 | NO | sketches only |

| dimension (w) | score | evidence |
|---|---|---|
| Composability (0.16) | 4 | user-owned tree, deprecated facades; strongest render story, but pieces take `ws` handle |
| Core-Contract (0.15) | 2 | state/persistence left per-backend; widget registry per-backend; "web runner parity lags" |
| Nix Coverage (0.16) | 3 | theme/widgets/persistence/charset named; chrome/breakpoint/badges not enumerated |
| Parity Proof (0.15) | 4 | end-to-end Nix-authored app + snapshot state equality vs Rust canary |
| Net Simplification (0.08) | 3 | "a long dual-API surface (facade + pieces) to maintain" — self-admitted cost |
| Migration (0.08) | 4 | facades keep consumers compiling; staged deprecation implied |
| Feasibility (0.06) | 4 | reuses `browser-tui-example` crane precedent; snapshot infra new but plausible |
| Risk (0.06) | 4 | facade drift → shared-state test; component-granularity risk specific |
| TDD (0.10) | 3 | slices implied per backend; ordering unstated |

Weighted 3.36 · **essential cap 2.0 (CK-003 NO)** · penalties −0.75 (CK-012, CK-017, CK-018) · **final 1.25 — invalid**

## A3 — Contract-first: schemars schema export + typed Nix options + golden parity

| CK | ans | evidence |
|---|---|---|
| CK-001 | YES | all fields |
| CK-002 | YES | adopts "`LayoutStore` trait + non-`fn()`-pointer defaults from A1"; `Palette` to core |
| CK-003 | YES | spec types + schema in core/`panel-kit-spec`; shells keep only render |
| CK-004 | YES | "`WorkspaceSpec` (panels, mode, `ChromeMetrics`…palette, badges/widgets, persistence policy, breakpoint, charset)" |
| CK-005 | YES | "a check re-evaluates the Nix spec to JSON and `diff`s byte-for-byte" vs Rust-emitted golden; `from_json` round-trip test |
| CK-006 | YES | existing checks retained; Nix path exercised by schema + golden checks (examples untouched) |
| CK-007 | YES | core:586/156, mkLayout.nix:43-46, flake.nix:111/118-138 — verified |
| CK-008 | NO | spot-check passed |
| CK-009 | YES | schemars is a cargo dep; runs in crane |
| CK-010 | YES | contract machinery justified by eval-time typo failures; admits coverage checks are weight |
| CK-011 | **NO** | "near-zero churn for consumers" contradicts the adopted A1 store/defaults changes (which A1 prices as a breaking constructor change); no signatures |
| CK-012 | YES | "`SavedLayout` stays the persistence format"; versioned envelope |
| CK-013 | YES | schema-first contract focus, distinct |
| CK-014 | NO | schema export is additive |
| CK-015 | NO | golden diff is a check, not a mirror |
| CK-016 | NO | none |
| CK-017 | NO | 0.82 / medium / mitigated — consistent |
| CK-018 | NO | `Palette` core-once with backend projections |
| CK-019 | NO | both halves (library-ness surgical but present) |
| CK-020 | NO | sketches only |

| dimension (w) | score | evidence |
|---|---|---|
| Composability (0.16) | 3 | "leaves the framework smells (IoC render, god-handle) mostly intact for this phase" — store/theme/defaults freed |
| Core-Contract (0.15) | 3 | spec/schema/palette core; state container lifecycle stays per-backend |
| Nix Coverage (0.16) | 4 | full list, versioned envelope, options mirror schema; maps onto core types |
| Parity Proof (0.15) | 4 | byte-for-byte golden diff + reverse round-trip; Rust-side serialization |
| Net Simplification (0.08) | 3 | "schema↔option coverage checks are themselves a test artifact to maintain" |
| Migration (0.08) | 2 | "near-zero churn for consumers" — one-line claim, internally shaky |
| Feasibility (0.06) | 4 | writeText golden, runCommand diff, schemars bin — all existing patterns |
| Risk (0.06) | 4 | schemars 0.8-vs-1.0 churn, option-drift false confidence (mitigate: generate stubs), canonical floats |
| TDD (0.10) | 4 | derives → export bin → options → probe check → golden check; first tests evident |

Weighted 3.45 · penalties −0.25 (CK-011) · **final 3.20 — viable**

## A4 — Instruction-stream core (workspace as interpreted command algebra)

| CK | ans | evidence |
|---|---|---|
| CK-001 | YES | all fields |
| CK-002 | YES | "control flow is entirely in the caller's hands"; persistence a routed `Persist` command |
| CK-003 | YES | interpreter subsumes free fns in core; "Backends contain zero logic" |
| CK-004 | YES | open `Cmd` set ("…") — "everything any Rust example does is a command sequence" (by construction) |
| CK-005 | YES | traces recorded from canaries "and diffing them against Nix-emitted tapes in a check" |
| CK-006 | YES | `checks.browser-tui-example` (flake.nix:143-146) cited as the example-build precedent |
| CK-007 | YES | core:447/497, core:1-21 doc — verified |
| CK-008 | NO | spot-check passed |
| CK-009 | YES | no toolchain change |
| CK-010 | **NO** | self-admitted "maximal added abstraction, directly against the task's 'prefer deleting/simplifying' constraint"; interpreter arms/tape infra not each tied to a composition |
| CK-011 | YES | "the migration cost for jump-cannon/apple-notes-ocr-flow is total" — impact stated |
| CK-012 | **NO** | persisted SavedLayout compatibility unaddressed |
| CK-013 | YES | distinct extreme |
| CK-014 | NO | JSON tape; no Nix dependency for cargo use |
| CK-015 | NO | traces derived, not mirrored |
| CK-016 | NO | none |
| CK-017 | NO | 0.04 / high / heavy risks — consistent |
| CK-018 | NO | single core semantics |
| CK-019 | NO | both halves |
| CK-020 | NO | sketches only |

| dimension (w) | score | evidence |
|---|---|---|
| Composability (0.16) | 4 | caller owns control flow; but "`interpret(step(state, cmd))` ceremony" at every call site |
| Core-Contract (0.15) | 4 | one interpreter, three frontends; backend Draw interpreters risk "quietly re-growing shells" |
| Nix Coverage (0.16) | 4 | tape covers everything by construction; concrete 11-item mapping not enumerated |
| Parity Proof (0.15) | 4 | trace equality — strong; command-granularity churn breaks recorded traces |
| Net Simplification (0.08) | 2 | "framework-shaped weight in anti-framework clothing" — self-verdict |
| Migration (0.08) | 2 | "consumers cannot follow"; no path |
| Feasibility (0.06) | 2 | "interaction ordering semantics…undefined"; Draw commands "fight both renderers' native models" |
| Risk (0.06) | 3 | specific (tape versioning, ordering, trace churn) but unmitigated |
| TDD (0.10) | 3 | property-testable core, but whole-API rewrite first |

Weighted 3.40 · penalties −0.50 (CK-010, CK-012) · **final 2.90 — viable**

## A5 — Nix as source of truth: build-time codegen of the Rust examples

| CK | ans | evidence |
|---|---|---|
| CK-001 | YES | all fields |
| CK-002 | **NO** | no framework-ish mechanism removed/bypassed — own risks concede "consumers see no library-ness improvement at all" |
| CK-003 | **NO** | "the contract now lives in the generator's templates, not in `panel-kit-core` types" — self-admitted AGENTS inversion |
| CK-004 | **NO** | emissions enumerated as enums/`defaults()`/palettes/chrome/storage keys — badges, widgets/content, breakpoint, charset not expressible inputs |
| CK-005 | YES | "compiles generated code…`nix flake check` builds the canary examples against it" — parity by compilation |
| CK-006 | YES | web + TUI canaries compiled from generated sources |
| CK-007 | YES | mkLayout `kinds`, flake fileset 46-56, update-version — verified |
| CK-008 | NO | spot-check passed |
| CK-009 | YES | generator runs in devshell/flake; codegen "complicates the pinned…lockstep" but doesn't change pins |
| CK-010 | **NO** | generator pipeline added; "build complexity and generator maintenance" — against deleting/simplifying |
| CK-011 | **NO** | consumer impact never stated |
| CK-012 | **NO** | persisted layouts unaddressed |
| CK-013 | YES | distinct (direction-of-truth codegen) |
| CK-014 | NO | "the generator path is for examples and…apps that prefer declaring its panel set in Nix" — optional |
| CK-015 | NO | generation is a build step, not hand-mirroring |
| CK-016 | NO | none |
| CK-017 | NO | 0.06 / high — consistent |
| CK-018 | NO | n/a |
| CK-019 | **YES** | Nix-spec mechanism with no change to the Rust composition model |
| CK-020 | NO | sketches only |

| dimension (w) | score | evidence |
|---|---|---|
| Composability (0.16) | 1 | "nothing changes about control-flow ownership" |
| Core-Contract (0.15) | 2 | contract leaves core for generator templates |
| Nix Coverage (0.16) | 3 | enums/defaults/theme/chrome/persistence-key; no content bindings |
| Parity Proof (0.15) | 4 | definitional (Rust is Nix output), bounded by generator capability |
| Net Simplification (0.08) | 2 | adds pipeline; mirror deleted only via generation |
| Migration (0.08) | 2 | silent on consumers |
| Feasibility (0.06) | 3 | plausible (Rust bin + fileset extension); rustfmt/IDE/generated-review unresolved |
| Risk (0.06) | 3 | specific (Hydra sandboxing, partial adoption two-sources); weak mitigations |
| TDD (0.10) | 2 | pipeline-first; no slice decomposition |

Weighted 2.42 · **essential cap 2.0 (CK-002/003/004 NO)** · penalties −1.00 (CK-010, CK-011, CK-012, CK-019) · floor → **final 1.00 — invalid**

## A6 — Pure-data workspace: registry + string ids, no generics

| CK | ans | evidence |
|---|---|---|
| CK-001 | YES | all fields |
| CK-002 | YES | dissolves `PanelKind` closed-enum constraint (context-file evidence item); non-generic shells |
| CK-003 | YES | `WorkspaceDoc` + registry are core data; "the web and TUI shells take `(doc, registry)` and nothing else" |
| CK-004 | YES | doc covers "panels, mode, palette, chrome metrics, widget bindings, persistence policy, breakpoint, charset" |
| CK-005 | YES | "a check serializes it and diffs against the Nix-emitted doc (one schema, one type)" + `nix build` runner |
| CK-006 | YES | "a 'Nix-only' app is the stock runner binary + `workspace.json`" (stock examples both backends) |
| CK-007 | YES | core:241-266/608-620, examples/workspace.rs:59-76 (4-variant Panel — verified), mkLayout.nix:13-16 |
| CK-008 | NO | spot-check passed |
| CK-009 | YES | serde/SmolStr only |
| CK-010 | YES | registry→runtime panels, doc→spec==state; trait deletion net-negative surface |
| CK-011 | YES | "both git-dependency consumers would need deep rewrites" — impact stated |
| CK-012 | YES | "`SavedLayout` already serializes kinds as bare variant-name strings…so ids are wire-compatible" |
| CK-013 | YES | distinct (identity-model revolution) |
| CK-014 | NO | runtime JSON; doc constructible in plain Rust |
| CK-015 | NO | doc diff check |
| CK-016 | NO | shells remain, but take values |
| CK-017 | NO | 0.08 / high / fatal risks — consistent |
| CK-018 | NO | single core model |
| CK-019 | NO | both halves |
| CK-020 | NO | sketches only |

| dimension (w) | score | evidence |
|---|---|---|
| Composability (0.16) | 3 | registry/doc composable and data-driven; render IoC of shells remains |
| Core-Contract (0.15) | 4 | everything core data, backends render doc; monomorphized transitions |
| Nix Coverage (0.16) | 4 | full list; "Nix can describe a chart but not an event handler" — self-scoped wall |
| Parity Proof (0.15) | 4 | type identity (one schema) + runner consumes `workspace.json`; canary doc still hand-authored |
| Net Simplification (0.08) | 3 | deletes genericity ("one non-generic API across backends") at cost of compile-time safety |
| Migration (0.08) | 2 | "consumer migration cost likely fatal" — no path |
| Feasibility (0.06) | 3 | all serde; "registry lookup failures at render time (require fallback/panic policy)" unresolved |
| Risk (0.06) | 4 | lookup-failure policy, schema churn vs persisted layouts, philosophical wall — specific |
| TDD (0.10) | 3 | doc round-trip/registry tests natural; ordering unstated |

Weighted 3.44 · penalties: none · **final 3.44 — viable**

## B1 — Core `WorkspaceState` + `LayoutStore` trait: shells become thin adapters

| CK | ans | evidence |
|---|---|---|
| CK-001 | YES | all fields |
| CK-002 | YES | "`use_workspace(store: impl LayoutStore<K>, defaults: impl Fn()…)` — real closures, replaceable key/format/backend" |
| CK-003 | YES | "`WorkspaceState::restore(store, defaults)`"; store trait core, mechanisms in backends |
| CK-004 | **NO** | Nix spec left geometry-only (`packages.layout-canary` / `SavedLayout`); no theme/chrome/badges/widgets/persistence/breakpoint/charset inputs |
| CK-005 | YES | "`spec-roundtrip`…deserializes `packages.layout-canary`…into `SavedLayout<K>` and asserts it equals…`defaults()`" |
| CK-006 | YES | Nix path exercised by the check; example checks unchanged (weakly — no new canaries) |
| CK-007 | YES | core lib.rs:447-564, src/lib.rs:298-314, flake.nix:82-83/118-138 — verified |
| CK-008 | NO | spot-check passed |
| CK-009 | YES | crane test bin; no toolchain change |
| CK-010 | YES | plain struct + free fns "keeps transitions individually testable without a dispatcher indirection" |
| CK-011 | YES | old signature quoted; new signature given; "README 'Public API unchanged' promise ends" |
| CK-012 | YES | "`SavedLayout` wire format untouched: existing users' persisted layouts survive" |
| CK-013 | YES | state-model minimalism, distinct from B2/B3 |
| CK-014 | NO | none |
| CK-015 | NO | check compares Nix JSON vs Rust values (mirror checked, not trusted) |
| CK-016 | NO | none |
| CK-017 | NO | 0.85 / medium — consistent |
| CK-018 | NO | store trait core-once |
| CK-019 | **YES** | Rust library-ness only; Nix spec scope untouched (explicitly leaves `PanelKind`/kinds unexpressible) |
| CK-020 | NO | sketches only |

| dimension (w) | score | evidence |
|---|---|---|
| Composability (0.16) | 3 | "`Workspace::render(body)` keeps its body closure" — self-admitted IoC remains; state/persistence freed |
| Core-Contract (0.15) | **5** | "restore/merge logic currently duplicated…collapses into `WorkspaceState::restore(store, defaults)`"; matches documented core model (core lib.rs:3-16) |
| Nix Coverage (0.16) | 1 | spec unchanged from `mkLayout` (geometry only) |
| Parity Proof (0.15) | 4 | exact anchor: deserialize + assert `== defaults()` |
| Net Simplification (0.08) | 4 | duplicated lifecycle deleted; `merge_defaults` "testable without a browser" |
| Migration (0.08) | 4 | old/new signatures + persisted-data status; no staging |
| Feasibility (0.06) | 4 | crane test bin; wasm store testing flagged honestly |
| Risk (0.06) | 3 | "`WorkspaceState` becomes dead packaging beside real logic" — specific, unmitigated |
| TDD (0.10) | 4 | restore-in-core tests → store impls → check; ordered |

Weighted 3.45 · **essential cap 2.0 (CK-004 NO)** · penalties −0.25 (CK-019) · **final 1.75 — invalid**

## B2 — Schema-first `WorkspaceSpec`: one document, schemars contract

| CK | ans | evidence |
|---|---|---|
| CK-001 | YES | all fields |
| CK-002 | YES | breakpoint ("currently hardcoded in `viewport_is_mobile`"), storage policy ("key + backend selector"), theme tokens made spec data |
| CK-003 | YES | spec + theme tokens in core ("RGB triples, like `badge::Rgb`, with per-backend converters") |
| CK-004 | YES | panels/mode/chrome/clamp/theme/breakpoint/charset/storage policy/content bindings enumerated |
| CK-005 | YES | "`spec-parity` check deserializes the Nix JSON into `WorkspaceSpec`, re-serializes, and byte-compares" |
| CK-006 | YES | canary "reads `spec-canary.json` (or embeds it via `include_str!` on the web, mirroring how `CSS` is embedded at src/lib.rs:109)" |
| CK-007 | YES | core:155/329/608-620, theme.rs:10-37, badge.rs:34-37, flake.nix:45-66/111/118-138 — verified |
| CK-008 | NO | spot-check passed |
| CK-009 | YES | schemars wasm-compatible ("adds compile time" only) |
| CK-010 | YES | spec justified by drift-killing; content bindings "capped at example parity" |
| CK-011 | **NO** | jump-cannon / apple-notes-ocr-flow impact never stated |
| CK-012 | **NO** | persisted-layout compatibility never stated |
| CK-013 | YES | document-centric contract, distinct |
| CK-014 | NO | schema export additive |
| CK-015 | NO | examples consume Nix JSON; hand-mirror deleted |
| CK-016 | NO | none |
| CK-017 | **YES** | 0.82 alongside "Complexity: high" and unmitigated risks (schema churn red-flagging downstream, wasm compile time) |
| CK-018 | NO | theme core-once with converters |
| CK-019 | NO | both halves |
| CK-020 | NO | sketches only |

| dimension (w) | score | evidence |
|---|---|---|
| Composability (0.16) | 3 | theme/breakpoint/storage policy become data; render IoC and `fn()` defaults untouched |
| Core-Contract (0.15) | 4 | spec/theme/clamp in core; slug mapping via `kind_slug`/`PanelKind::title` |
| Nix Coverage (0.16) | 4 | full 11-item list; content bindings "scoped exactly to what the canary examples render" |
| Parity Proof (0.15) | **5** | deserialize → re-serialize → byte-compare AND examples consume the spec (hand-mirror deleted) — no unchecked duplication |
| Net Simplification (0.08) | 4 | deletes `nix/examples/workspace-canary.nix` mirror; theme duplication collapsed |
| Migration (0.08) | 2 | silent on the two consumers |
| Feasibility (0.06) | 4 | crane schema bin + `include_str!` pattern (CSS at src/lib.rs:109 — verified); sum-type discriminator conventions needed |
| Risk (0.06) | 3 | "`WorkspaceSpec<String>` vs `WorkspaceSpec<K>` drift" mitigated by tests; schema churn unmitigated |
| TDD (0.10) | 4 | spec serde round-trip → mkWorkspace → parity check → example conversion |

Weighted 3.77 · penalties −0.75 (CK-011, CK-012, CK-017) · **final 3.02 — viable**

## B3 — Non-breaking facade: capability modules beside compatibility shells

| CK | ans | evidence |
|---|---|---|
| CK-001 | YES | all fields |
| CK-002 | YES | "`use_workspace_with(WorkspaceConfig { store, breakpoint, clamp, theme, charset })` exposes what is today unconfigurable (the 760 px breakpoint…)" |
| CK-003 | **NO** | cross-backend `LayoutStore` placed in "`panel_kit::persist`" (the Dioxus crate), not core; state container stays per-backend |
| CK-004 | **NO** | Nix toolbox = `mkTheme`, `mkChrome` only — no badges/widgets/persistence/breakpoint/charset; "no monolithic spec document" by design |
| CK-005 | YES | "the same Rust round-trip check as B1" |
| CK-006 | YES | existing checks retained; Nix path exercised (weakly) |
| CK-007 | YES | core:145-191/385-404/566-581, badge precedent — verified |
| CK-008 | NO | spot-check passed |
| CK-009 | YES | pure refactor |
| CK-010 | YES | `RenderModel` "derived from functions that already exist in core…no new semantics"; each capability reversible |
| CK-011 | YES | "Keep `use_workspace(storage_key, defaults)` and `TuiWorkspace::new(store, defaults)` byte-compatible" + why (README promise, cited) |
| CK-012 | YES | byte-compatible constructors ⇒ wire format untouched |
| CK-013 | YES | compat-centric layering, distinct |
| CK-014 | NO | none |
| CK-015 | NO | round-trip check |
| CK-016 | NO | old entry points kept; config is opt-in |
| CK-017 | NO | 0.80 / medium — consistent |
| CK-018 | NO | spinner gets one core model (badge precedent) |
| CK-019 | NO | both halves, partially |
| CK-020 | NO | sketches only |

| dimension (w) | score | evidence |
|---|---|---|
| Composability (0.16) | 3 | "`layout_model`…users who want their own tree call `layout_model` directly and skip `Workspace::render`"; framework path remains default (self-admitted) |
| Core-Contract (0.15) | 3 | `RenderModel` core; `LayoutStore` misplaced in web crate; TUI-only widgets stay |
| Nix Coverage (0.16) | 2 | geometry + theme + chrome — anchor score_2 verbatim |
| Parity Proof (0.15) | 4 | inherits B1's deserialize-and-assert check |
| Net Simplification (0.08) | 2 | "two ways to do everything (legacy hardcoded path + capability path) persists indefinitely" |
| Migration (0.08) | 4 | byte-compatible, quoted signatures, "every extraction ships independently and can be reverted" |
| Feasibility (0.06) | 4 | pure refactor over existing fns; zero novel machinery |
| Risk (0.06) | 4 | dual-API calcification, `RenderModel` third-semantics divergence "enforced by shell-refactor + golden tests" |
| TDD (0.10) | 4 | per-extraction slices, independently shippable |

Weighted 3.21 · **essential cap 2.0 (CK-003, CK-004 NO)** · penalties: none · **final 2.00 — invalid**

## B4 — Nix as source of truth: generate Rust at build time

| CK | ans | evidence |
|---|---|---|
| CK-001 | YES | all fields |
| CK-002 | **NO** | no framework mechanism touched; focus entirely on codegen of consts |
| CK-003 | **NO** | contract moves into the generator; core untouched |
| CK-004 | **NO** | emissions = enum + `DEFAULTS` + `THEME` + `CLAMP`; badges/widgets/content/breakpoint/charset not inputs |
| CK-005 | YES | "Nix spec → working Rust app proven by compilation, no round-trip test needed" (definitional) |
| CK-006 | YES | "`nix flake check` builds the canary examples against it" |
| CK-007 | YES | mkLayout.nix:29 `kinds` "for codegen" — verified verbatim; flake fileset; update-version precedent |
| CK-008 | NO | spot-check passed |
| CK-009 | YES | generation inside the flake; but breaks plain `cargo build` unless committed |
| CK-010 | **NO** | "an entire pipeline added, against the repo's 'prefer deleting/simplifying' constraint" — self-admitted |
| CK-011 | **NO** | "Downstream git-dependency consumers (jump-cannon) can't see generated crates unless committed" — artifact visibility only, no API statement |
| CK-012 | **NO** | persisted layouts unaddressed |
| CK-013 | YES | distinct |
| CK-014 | NO | optional path |
| CK-015 | NO | generation is a build step |
| CK-016 | NO | none |
| CK-017 | NO | 0.08 / high — consistent |
| CK-018 | NO | n/a |
| CK-019 | **YES** | Nix mechanism with no Rust composition-model change |
| CK-020 | NO | sketches only |

| dimension (w) | score | evidence |
|---|---|---|
| Composability (0.16) | 1 | app control flow unchanged |
| Core-Contract (0.15) | 2 | "no schema layer at all (Nix is the schema)" — contract leaves core types |
| Nix Coverage (0.16) | 3 | enums/defaults/theme/clamp consts; no content |
| Parity Proof (0.15) | 4 | definitional parity (compilation) — bounded by generator capability |
| Net Simplification (0.08) | 2 | adds generator + fileset sync; deletes mirror only indirectly |
| Migration (0.08) | 2 | consumers unaddressed |
| Feasibility (0.06) | 2 | "Circularity: crane needs a source tarball before it can build the generator that reads Nix" — unresolved |
| Risk (0.06) | 3 | specific (circularity, stale-checkout drift, dual sources, rollback cost); unmitigated |
| TDD (0.10) | 2 | pipeline big-bang |

Weighted 2.36 · **essential cap 2.0 (CK-002/003/004 NO)** · penalties −1.00 (CK-010, CK-011, CK-012, CK-019) · floor → **final 1.00 — invalid**

## B5 — Data-driven registry: runtime PanelId, plugin providers (framework pole)

| CK | ans | evidence |
|---|---|---|
| CK-001 | YES | all fields |
| CK-002 | YES | "`render(body)` closures disappear (the registry supplies content)"; `PanelKind` closed enum replaced |
| CK-003 | YES | descriptor model core; "each backend becomes an interpreter over descriptors" |
| CK-004 | YES | spec "configures panels, geometry, theme, charset, breakpoint, and persistence" + declarative content |
| CK-005 | **NO** | parity "by construction" only — no `checks.*` job named anywhere |
| CK-006 | **NO** | no example builds or checks mentioned |
| CK-007 | YES | core lib.rs:29-41/608, tui:35-43, badge.rs — verified |
| CK-008 | NO | spot-check passed |
| CK-009 | YES | plain data + serde |
| CK-010 | YES | registry/providers/descriptors each tied (runtime panels, parity by construction) — weight priced in other dims |
| CK-011 | YES | "jump-cannon/apple-notes-ocr-flow need total rewrites" |
| CK-012 | **NO** | persisted layouts with string ids unaddressed |
| CK-013 | YES | the coherent opposite pole |
| CK-014 | NO | startup spec load |
| CK-015 | NO | n/a |
| CK-016 | **YES** | "a registry is a service locator" — self-admitted mandatory handle |
| CK-017 | NO | 0.05 / high — consistent |
| CK-018 | NO | one core model, two interpreters |
| CK-019 | NO | both halves |
| CK-020 | NO | sketches only |

| dimension (w) | score | evidence |
|---|---|---|
| Composability (0.16) | 2 | "providers are inversion of control the user cannot bypass" — self-verdict |
| Core-Contract (0.15) | 4 | one model, two interpreters; compile-time match parity traded for dispatch |
| Nix Coverage (0.16) | 4 | "panels that don't exist in Rust can still be declared and mounted" |
| Parity Proof (0.15) | 2 | "parity by construction" — prose only, no check job |
| Net Simplification (0.08) | 2 | "the descriptor language is a UI framework in disguise" |
| Migration (0.08) | 2 | total rewrites, no path |
| Feasibility (0.06) | 3 | buildable data machinery; per-render allocation/dyn dispatch costs named |
| Risk (0.06) | 3 | descriptor LCD convergence + escape hatches — specific, unmitigated |
| TDD (0.10) | 2 | interpreter rewrite big-bang |

Weighted 2.74 · **essential cap 2.0 (CK-005 NO)** · penalties −0.75 (CK-006, CK-012, CK-016) · **final 1.25 — invalid**

## B6 — Dissolve the monolith: independent leaf crates, no Workspace type

| CK | ans | evidence |
|---|---|---|
| CK-001 | YES | all fields |
| CK-002 | YES | "Delete the shell objects (`Workspace`, `TuiWorkspace`) from the public API — demoting them to examples" |
| CK-003 | YES | renderer-neutral leaves (geometry/state/persist/theme/badge/spinner) + thin per-backend kits — core spirit kept, packaging split |
| CK-004 | **NO** | "no monolithic spec because there is no monolithic object to specify"; "'feature-complete Nix spec' gets harder" — self-admitted |
| CK-005 | YES | "per-leaf round-trip checks" (`mkRect`, `mkPanelWin`, `mkTheme`, `mkLayout`) |
| CK-006 | **NO** | "Canary coverage shrinks because the canaries *were* the shells" |
| CK-007 | YES | core:119-191/385-437, `Zone` tui:136-148, ResizeObserver/recompute — verified |
| CK-008 | NO | spot-check passed |
| CK-009 | YES | code movement only |
| CK-010 | YES | massive deletion; "every piece independently usable" justifies leaves |
| CK-011 | YES | "Both consumers break despite the README promise" |
| CK-012 | **NO** | persisted-user-file compatibility unaddressed |
| CK-013 | YES | crate-granularity dissolution pole |
| CK-014 | NO | none |
| CK-015 | NO | round-trip checks |
| CK-016 | NO | no handle at all |
| CK-017 | NO | 0.04 / medium / risks — consistent |
| CK-018 | NO | leaves are the shared homes |
| CK-019 | NO | both halves (Nix toolbox weak but present) |
| CK-020 | NO | sketches only |

| dimension (w) | score | evidence |
|---|---|---|
| Composability (0.16) | 4 | "every piece independently usable, zero IoC, composition literal in consumer code" |
| Core-Contract (0.15) | 3 | leaves renderer-neutral but the contract fragments across six crates |
| Nix Coverage (0.16) | 2 | per-leaf composables ≈ geometry + theme |
| Parity Proof (0.15) | 3 | per-leaf round-trips; no workspace-level spec to prove |
| Net Simplification (0.08) | 4 | deletes ~900+680 lines of shell surface |
| Migration (0.08) | 2 | consumers reimplement "ResizeObserver + scale-on-resize recompute…wheel scroll-chaining…hit-zone tracking" |
| Feasibility (0.06) | 4 | mechanically simple |
| Risk (0.06) | 3 | "Consumer-copied shell code drifts immediately (two consumers, two bugs)" — specific, unmitigated |
| TDD (0.10) | 3 | per-leaf tests natural; consumer migration big-bang |

Weighted 3.06 · **essential cap 2.0 (CK-004 NO)** · penalties −0.50 (CK-006, CK-012) · **final 1.50 — invalid**

## C-A — Core-Owned Reducer with Injectable Store (schema-first Nix spec)

| CK | ans | evidence |
|---|---|---|
| CK-001 | YES | all fields |
| CK-002 | YES | "`&'static str` key + `fn()` pointer in `use_workspace`; `Option<PathBuf>` in `TuiWorkspace::new`" deleted via store trait + closure defaults |
| CK-003 | YES | `WorkspaceState` + `LayoutStore` + core `Theme` + spec types in core; gloo/JSON-file impls in backends |
| CK-004 | YES | registry, layout, chrome, theme, content bindings, persistence policy, breakpoint, charset |
| CK-005 | YES | "`spec-canary-check` reads `packages.spec-canary` JSON and `serde_json::from_str::<WorkspaceSpec<TestPanel>>` it — replacing the jq-only…check" |
| CK-006 | YES | "each backend gains an example that boots from the Nix-emitted JSON (`spec_workspace` on wasm32 for web, native for TUI)" |
| CK-007 | YES | src/lib.rs:298-397, tui:186-218, flake.nix:118-138, theme unification — verified |
| CK-008 | NO | spot-check passed |
| CK-009 | YES | schemars behind a feature; crane-built check |
| CK-010 | YES | "a narrow trait deletes two hardcodings without inventing a framework"; state bundling "additive-unifying" |
| CK-011 | YES | "`use_workspace` signature breaks (two consumers must patch call sites)…mechanical to fix"; closure defaults named |
| CK-012 | **NO** | already-persisted user JSON compatibility never explicitly stated |
| CK-013 | YES | distinct within file C |
| CK-014 | NO | "keeps `panel-kit` usable by non-Nix consumers" |
| CK-015 | NO | "without hand-mirroring" — both sides consume one JSON |
| CK-016 | NO | "`use_workspace` survives as sugar…the state and store are public, so an app can own the loop" |
| CK-017 | NO | 0.85 / medium — consistent |
| CK-018 | NO | TUI `Theme` becomes alias/adaptation of core data |
| CK-019 | NO | both halves |
| CK-020 | NO | sketches only |

| dimension (w) | score | evidence |
|---|---|---|
| Composability (0.16) | 4 | "an app can own the loop"; "the shell still owns the render tree by default" — layout math becomes callable core API |
| Core-Contract (0.15) | 4 | state/store/theme/spec/schema core; shells keep signal orchestration |
| Nix Coverage (0.16) | 4 | full list + registry + schemars contract; "two parallel 14-field palettes collapse into one" |
| Parity Proof (0.15) | 4 | native deserialize check + both-backend boots; no value-equality assert vs Rust-side spec |
| Net Simplification (0.08) | 4 | duplicated orchestration deleted; palette unification is "the deletion lever" |
| Migration (0.08) | 3 | signature break + mechanical fix named; persisted-JSON status missing |
| Feasibility (0.06) | 4 | feature-gated schemars, crane check bin, jq-check replacement — existing patterns named |
| Risk (0.06) | 4 | god-object regrowth, web_sys leakage ("must stay as an adapter concern"), PanelKind-for-strings gap — specific |
| TDD (0.10) | 4 | restore-in-core tests → serde round-trip → schema export → check → examples |

Weighted 3.92 · penalties −0.25 (CK-012) · **final 3.67 — strong**

## C-B — Headless Event Reducer + Frame Projection; Backends as Widget Kits

| CK | ans | evidence |
|---|---|---|
| CK-001 | YES | all fields |
| CK-002 | YES | "The framework smell is precisely that the app hands control to `Workspace::render(body)`…This approach inverts it"; save policy host-owned |
| CK-003 | YES | `Snapshot`/`Event`/`reduce`/`project` core; kits render `Frame`; AGENTS boundary rule quoted verbatim (verified AGENTS.md:21-23) |
| CK-004 | YES | "The full `WorkspaceSpec` (as in A: theme, chrome, content, persistence, breakpoint, charset)" |
| CK-005 | YES | "a native Rust binary that `reduce`/`project`-drives the spec into a `Frame` and asserts golden values — proving *behavioral* parity" |
| CK-006 | **NO** | only the native parity check is named; no wasm32 web / native TUI example check mentioned |
| CK-007 | YES | tui:251, core:76-116 (`PointerEvent` 109 verified), `Clamp::WEB`/`TileMetrics::WEB`/`ROW_CELLS` all verified real |
| CK-008 | NO | spot-check passed |
| CK-009 | YES | pure core + per-backend kits |
| CK-010 | YES | Event data (boundary translation + property testing), `project()` (composition boundary), `PanelId` (Nix mapping) each tied |
| CK-011 | YES | "both consumers break beyond a signature change (their event handlers and render calls move)" |
| CK-012 | **NO** | persisted SavedLayout compatibility unaddressed |
| CK-013 | YES | event-reducer + kit vs A (methods + sugar) vs C (config) |
| CK-014 | NO | spec JSON additive |
| CK-015 | NO | golden check derives from spec |
| CK-016 | NO | shell demoted to "one composition"; kit pieces flat |
| CK-017 | NO | 0.80 / medium — consistent |
| CK-018 | NO | one `Frame` model core |
| CK-019 | NO | both halves |
| CK-020 | NO | sketches only |

| dimension (w) | score | evidence |
|---|---|---|
| Composability (0.16) | 4 | "user owns the loop; every chrome piece callable; persistence policy host-owned"; convenience shell still the familiar entry |
| Core-Contract (0.15) | **5** | "`reduce` is a pure dispatch over the existing free functions"; "backends translate platform input once at the boundary (explicitly demanded by AGENTS.md)" |
| Nix Coverage (0.16) | 4 | full list inherited from A + `PanelId` maps spec onto `Snapshot<PanelId>` |
| Parity Proof (0.15) | **5** | golden-`Frame` behavioral parity — "proving *behavioral* parity, not just shape"; check drives the actual spec |
| Net Simplification (0.08) | 3 | "biggest behavioral rewrite of the two shells…largely re-plumbed" — rewrite, not deletion |
| Migration (0.08) | 3 | breakage specific (handlers + render calls move); no signatures, no persisted status |
| Feasibility (0.06) | 4 | over existing fns; golden sensitivity mitigated by pinning `Clamp::WEB`/`TileMetrics::WEB`/`ROW_CELLS` (all real) |
| Risk (0.06) | 4 | "a pure `Snapshot` outside `Signal`s can degrade fine-grained updates" + Event-sprawl discipline + float pinning |
| TDD (0.10) | 4 | property-testable `reduce`/`project` first, then kits, then shell |

Weighted 4.14 · penalties −0.50 (CK-006, CK-012) · **final 3.64 — strong**

## C-C — Config-Object Facelift: WorkspaceConfig + Runtime Spec Parsing

| CK | ans | evidence |
|---|---|---|
| CK-001 | YES | all fields |
| CK-002 | YES | "kills the `&'static str`/`fn()` pair and both persistence hardcodings"; 760 px breakpoint → config |
| CK-003 | **NO** | "state ownership stays duplicated per backend (load/merge/save orchestration still written twice, just configured differently)" — self-admitted |
| CK-004 | YES | `mkSpec { panels?; registry; mode; chrome; theme; content; persistence; breakpoint; charset }` (letter yes; content depth self-limited) |
| CK-005 | YES | "upgraded to a small native `spec-decode` check binary that deserializes every spec the flake exports" |
| CK-006 | YES | "`packages.spec-canary` JSON…feeds a wasm web example and a native TUI example, both wired into `checks`" |
| CK-007 | YES | src/lib.rs:298-301 signature quoted exactly; tui:186; flake.nix:82-83 — verified |
| CK-008 | NO | spot-check passed |
| CK-009 | YES | "avoids schemars/codegen machinery entirely" |
| CK-010 | YES | "every option must name the hardcoding it deletes" — discipline rule proposed |
| CK-011 | YES | "`defaults` accepting a closure…is the single breaking change to the two consumers" + doc-example citation |
| CK-012 | **NO** | persisted JSON compatibility unaddressed |
| CK-013 | YES | zero-structural-change pole |
| CK-014 | NO | runtime parse; no Nix build dep |
| CK-015 | NO | mirror deleted, one Nix file consumed by both sides |
| CK-016 | NO | config object, not a handle |
| CK-017 | NO | 0.82 / low — consistent |
| CK-018 | NO | core tokens + backend projection (transitional duplication is a risk, not a definition) |
| CK-019 | NO | both halves present (weakly on pillar 1) |
| CK-020 | NO | sketches only |

| dimension (w) | score | evidence |
|---|---|---|
| Composability (0.16) | 2 | "the IoC critique is *not* answered — `Workspace::render(body)` and `TuiWorkspace::render` keep owning the tree" |
| Core-Contract (0.15) | 3 | spec/`PanelId`/theme tokens core; orchestration stays duplicated (self-admitted) |
| Nix Coverage (0.16) | 3 | "spec cannot express TUI-only content, so 'totally feature complete'…is only partially honored" |
| Parity Proof (0.15) | 3 | decode-only check (no value equality vs Rust side) |
| Net Simplification (0.08) | 3 | deletes hand-mirror ("one description, two consumers"); god-config risk self-flagged |
| Migration (0.08) | 4 | single breaking change named with doc-example citation; low risk |
| Feasibility (0.06) | 4 | "proportionate to the repo's size"; existing flake patterns cited |
| Risk (0.06) | 4 | "Solves configuration, not library-ness…risk of the design being judged as ducking the question" — honest, specific |
| TDD (0.10) | 4 | config/from_spec tests, decode check, example wiring |

Weighted 3.14 · **essential cap 2.0 (CK-003 NO)** · penalties −0.25 (CK-012) · **final 1.75 — invalid**

## C-D — Nix as the Source of Truth: Spec-Compiled Examples

| CK | ans | evidence |
|---|---|---|
| CK-001 | YES | all fields |
| CK-002 | YES | weak — "the shells reduce to `SpecViewer` implementations and nothing prevents a host from consuming `panel-kit-spec` + core directly" (shell bypassable; no specific hardcoded policy cited) |
| CK-003 | YES | `panel-kit-spec` crate sole schema; `Snapshot::from_spec` + `merge_defaults` spec-form in core; shells = viewers |
| CK-004 | YES | layout, registry, chrome, theme, content model (widget + literal demo data), persistence, breakpoint, charset |
| CK-005 | YES | canaries embed the spec (`include_str!`); "the parity matrix in `checks`…is what proves feature-completeness" |
| CK-006 | YES | "web example on wasm32, TUI example native, `browser-tui-example` style jobs at flake.nix:143-146" |
| CK-007 | YES | flake.nix:74-83 mirror comment (verbatim-verified), mkLayout.nix:48-81, workspace_canary.rs:112-257 — verified |
| CK-008 | NO | spot-check passed |
| CK-009 | YES | "vendored…fallback keeps the crate buildable outside Nix (plain `cargo build`)" |
| CK-010 | YES | single-description deletes duplication; vendoring+sync-check is priced as cost |
| CK-011 | YES | "jump-cannon/apple-notes-ocr-flow migrate by authoring a Nix spec (or keeping Rust defaults behind `WorkspaceConfig::with_defaults`)" |
| CK-012 | **NO** | persisted-layout compat unstated ("spec as defaults, not state" is a mental model, not a compat statement) |
| CK-013 | YES | direction-of-truth embedding, distinct |
| CK-014 | NO | library use Nix-free; embedding at example layer |
| CK-015 | NO | single description |
| CK-016 | NO | none |
| CK-017 | NO | 0.06 / high — consistent |
| CK-018 | NO | one token set |
| CK-019 | NO | both halves |
| CK-020 | NO | sketches only |

| dimension (w) | score | evidence |
|---|---|---|
| Composability (0.16) | 3 | host may consume core directly; shell IoC remains for shell users |
| Core-Contract (0.15) | 4 | spec crate sole schema; shells reduced to viewers |
| Nix Coverage (0.16) | 4 | full list; "feature-complete by construction (anything less breaks the canaries)" |
| Parity Proof (0.15) | 4 | parity-by-construction + matrix; "if the spec can't express something a canary shows, the canary can't be written" |
| Net Simplification (0.08) | 3 | "maximal deletion of duplicated description" vs build coupling weight (both stated) |
| Migration (0.08) | 3 | escape hatch named; "weakest fit…whose layouts are code-defined today" — honest |
| Feasibility (0.06) | 3 | "the `Metrics`/`noise()` animation machinery…doesn't fit literal data; risks a `dynamic` escape hatch" — real wall |
| Risk (0.06) | 4 | vendored drift, slow iteration, spec literalism, module-system ergonomics — all specific |
| TDD (0.10) | 3 | spec crate → embedding → per-backend canary → matrix |

Weighted 3.52 · penalties −0.25 (CK-012) · **final 3.27 — viable**

## C-E — Shell Deletion: Capability Kit and Metadata-as-Data

| CK | ans | evidence |
|---|---|---|
| CK-001 | YES | all fields |
| CK-002 | YES | "remove the `Workspace`/`TuiWorkspace` controller-handles entirely…delete…the private duplicated load/save pair…and the `PanelKind::title` method" |
| CK-003 | YES | core keeps free fns + `Snapshot`/`Event`/`reduce`/`project`; "each backend crate exports only leaf capabilities…none holding state, none owning trees" |
| CK-004 | YES | registry (`PanelMeta`), layout, chrome, theme, content bindings (incl. TUI widget kinds), persistence policy, breakpoint, charset |
| CK-005 | YES | "a native decode+golden-project check (spec JSON → `Snapshot<PanelId>` → `project()` → compare against golden `Frame`)" |
| CK-006 | YES | "every backend builds one canary assembled entirely from kit pieces fed by the spec — the canary *is* the proof that the kit composes" |
| CK-007 | YES | src/lib.rs:250-829, tui:156-677, core:608-620, `zones` tui:136-148 — verified |
| CK-008 | NO | spot-check passed (incl. `ROW_CELLS`) |
| CK-009 | YES | deletion + kit; no toolchain change |
| CK-010 | YES | "Zero persistence in-library: the hardcodings are deleted, not abstracted — no `LayoutStore` trait to design around" |
| CK-011 | YES | "both consumers break hard (README's 'Public API unchanged' is abandoned; semver 0.3 + migration doc required)" + facade option |
| CK-012 | **NO** | persisted-user-file compatibility unaddressed (recipes reuse `SavedLayout`/`merge_defaults` implicitly) |
| CK-013 | YES | deletion pole vs B (reducer+kit with shell) vs C (config) |
| CK-014 | NO | spec decode check; plain-cargo usable |
| CK-015 | NO | canaries fed by spec |
| CK-016 | NO | no handle; facade explicitly "one composition, not the product" |
| CK-017 | NO | 0.05 / medium / named risks — consistent (low probability justified by consumer promises, not risk denial) |
| CK-018 | NO | one core token set; TUI widgets stay as kit leaves (rendering) |
| CK-019 | NO | both halves |
| CK-020 | NO | sketches only |

| dimension (w) | score | evidence |
|---|---|---|
| Composability (0.16) | **5** | "composition all the way down, no assembly you cannot bypass"; every policy caller-supplied; shell deleted, facade optional |
| Core-Contract (0.15) | 4 | kit pieces flat over core projection; facade+recipes keep a residue of shell semantics alive |
| Nix Coverage (0.16) | 4 | full list incl. TUI widget kinds; persistence policy only "describes what the *recipe* would configure" |
| Parity Proof (0.15) | **5** | decode + golden-project + kit-assembled canaries fed by the spec — proof and canary coincide |
| Net Simplification (0.08) | **5** | deletes two ~430/500-line impl blocks + `PanelKind::title` + private load/save; "smallest long-term maintenance surface" |
| Migration (0.08) | 3 | hard break named + facade crate + semver/migration doc; no signatures |
| Feasibility (0.06) | 4 | existing core fns + NixOS module types; Dioxus-signal and hit-test costs honestly flagged |
| Risk (0.06) | 4 | "the facade crate becomes load-bearing and the deletion is cosmetic"; coarse-snapshot mitigation; `zones` recipe must ship |
| TDD (0.10) | 4 | reduce/project tests → golden check → kit → spec-fed canaries; each leaves the workspace building |

Weighted 4.31 · penalties −0.25 (CK-012) · **final 4.06 — strong**

## C-F — Spec Compiler with Capability-Assembled Shells and a Parity Matrix

| CK | ans | evidence |
|---|---|---|
| CK-001 | YES | all fields |
| CK-002 | YES | "the shipped shell is literally `Workspace::compose(capabilities…)`, existing as documentation-by-code of one legal assembly" |
| CK-003 | YES | `Kernel` + widget view-models in core ("extending the existing pattern of `panel_kit_core::badge`") |
| CK-004 | YES | "all dimensions: registry/`PanelMeta`, layout, `ChromeMetrics`, theme tokens, per-panel content bindings including TUI widgets, persistence policy, breakpoint, charset" |
| CK-005 | YES | "decode check per spec (`spec-version` pinning so a schema change fails every cell)" + matrix jobs |
| CK-006 | YES | "`checks` enumerates `nix/specs/*.nix` × {web-wasm, tui-native, browser-tui}" |
| CK-007 | YES | flake.nix:111 (exact), :143-146, hydraJobs :170+, workspace_canary.rs:146 (exact — STAGES "color-matched to apple-notes' stages") |
| CK-008 | NO | spot-check passed |
| CK-009 | YES | crane jobs; matrix cost vs farm flagged (flake.nix:19-30 — verified) |
| CK-010 | **NO** | "spec crate + IR versioning + capability lattice + view-model layer + matrix CI is a lot of machinery…directly tensions with 'prefer deleting/simplifying'" — self-admitted; "speculative generality" |
| CK-011 | YES | "jump-cannon keeps `use_workspace`-shaped composition with default capabilities (compat wrapper…) so the README promise survives" |
| CK-012 | **NO** | IR versioning covers specs, not already-persisted `SavedLayout` files of existing users |
| CK-013 | YES | compiler/IR + matrix, distinct |
| CK-014 | NO | none |
| CK-015 | NO | matrix derives |
| CK-016 | NO | capabilities are small standalone values |
| CK-017 | NO | 0.08 / high — consistent |
| CK-018 | NO | view-models core-once |
| CK-019 | NO | both halves |
| CK-020 | NO | sketches only |

| dimension (w) | score | evidence |
|---|---|---|
| Composability (0.16) | 4 | capabilities "each a small standalone value the host can replace or omit" |
| Core-Contract (0.15) | 4 | Kernel + view-models core; backends render IR |
| Nix Coverage (0.16) | 4 | full list; typed DSL export `lib.{mkSpec, specTypes, specSchema}` |
| Parity Proof (0.15) | 4 | matrix CI makes feature-completeness "a falsifiable build artifact" |
| Net Simplification (0.08) | 2 | "the heaviest design on the table" — self-verdict |
| Migration (0.08) | 3 | compat profile concrete; signatures/persisted status absent |
| Feasibility (0.06) | 3 | mechanisms exist; "matrix CI cost…inflates Hydra evaluation/build time against the farm's stated capacity constraints" |
| Risk (0.06) | 4 | over-engineering, farm capacity, view-model boundary (canary-constants evidence) — all specific |
| TDD (0.10) | 3 | layer-wise decomposable; integration matrix last |

Weighted 3.60 · penalties −0.50 (CK-010, CK-012) · **final 3.10 — viable**

---

## Comparison table (all 18)

| id | name (short) | weighted | cap | penalties | final | label |
|---|---|---|---|---|---|---|
| A1 | core reducer + store + escape hatches | 4.10 | — | — | **4.10** | strong |
| A2 | dissolve shell into user-owned tree | 3.36 | 2.0 (CK-003) | −0.75 (012,017,018) | 1.25 | invalid |
| A3 | schemars contract + typed options + golden | 3.45 | — | −0.25 (011) | 3.20 | viable |
| A4 | instruction-stream interpreter | 3.40 | — | −0.50 (010,012) | 2.90 | viable |
| A5 | Nix→Rust codegen of examples | 2.42 | 2.0 (CK-002/3/4) | −1.00 (010,011,012,019) | 1.00 | invalid |
| A6 | registry + string ids, WorkspaceDoc | 3.44 | — | — | 3.44 | viable |
| B1 | core WorkspaceState + LayoutStore | 3.45 | 2.0 (CK-004) | −0.25 (019) | 1.75 | invalid |
| B2 | schema-first WorkspaceSpec document | 3.77 | — | −0.75 (011,012,017) | 3.02 | viable |
| B3 | non-breaking facade + capabilities | 3.21 | 2.0 (CK-003/4) | — | 2.00 | invalid |
| B4 | Nix→Rust crate codegen | 2.36 | 2.0 (CK-002/3/4) | −1.00 (010,011,012,019) | 1.00 | invalid |
| B5 | registry + providers (framework pole) | 2.74 | 2.0 (CK-005) | −0.75 (006,012,016) | 1.25 | invalid |
| B6 | leaf crates, no Workspace | 3.06 | 2.0 (CK-004) | −0.50 (006,012) | 1.50 | invalid |
| C-A | core reducer + store + schema-first spec | 3.92 | — | −0.25 (012) | **3.67** | strong |
| C-B | event reducer + frame projection kits | 4.14 | — | −0.50 (006,012) | **3.64** | strong |
| C-C | WorkspaceConfig + runtime spec parse | 3.14 | 2.0 (CK-003) | −0.25 (012) | 1.75 | invalid |
| C-D | Nix-authored, spec-compiled examples | 3.52 | — | −0.25 (012) | 3.27 | viable |
| C-E | shell deletion, capability kit, PanelMeta | 4.31 | — | −0.25 (012) | **4.06** | strong |
| C-F | spec compiler + capability lattice + matrix | 3.60 | — | −0.50 (010,012) | 3.10 | viable |

Score order: A1 4.10 · C-E 4.06 · C-A 3.67 · C-B 3.64 · A6 3.44 · C-D 3.27 · A3 3.20 · C-F 3.10 · B2 3.02 · A4 2.90 · B3 2.00 · B1 1.75 = C-C 1.75 (tie-break: B1 four-dim sum 13 > C-C 11) · B6 1.50 · A2 1.25 = B5 1.25 (tie-break: A2 14 > B5 13) · B4 1.00 = A5 1.00 (tie-break: B4 11 > A5 10).

## Selection rationale

**1st — A1 (4.10).** The only proposal that clears every essential and important checklist item with zero penalties: it is the complete skeleton the task literally asks for — core `WorkspaceState`/`Action` wrapping the existing free functions (deletion of the duplicated lifecycle, not a parallel API), a minimal two-method `LayoutStore` dissolving the `&'static str` key / `fn()` pointer / hardcoded save-on-settle, granular render pieces published beside a retained shell (composability without a forced rewrite), the only proposal-side enumeration of the full 11-item Nix feature list mapped onto one core `WorkspaceSpec` serde type, and a value-level round-trip parity check replacing the jq shape check — which this judge confirmed is already failing silently (7-vs-9-panel mirror drift at HEAD). Migration is priced honestly (one constructor change, README promise re-anchored, persisted JSON explicitly preserved).

**2nd — C-E (4.06).** The strongest answer to pillar 1 on pure merit: it deletes the god-objects instead of decorating them (`PanelKind::title` → `PanelMeta` data; zero in-library persistence; kit pieces with flat signatures), and its parity story (decode + golden-`project` + spec-fed canaries) is the only one where the canary *is* the composability proof. It loses to A1 only on migration weight (0.08 dimension) and its indirect persistence-policy spec. It is mechanistically distinct from A1 (deletion vs retention+escape-hatches).

**3rd — C-B (3.64).** The headless `Event`/`reduce` + `project() -> Frame` inversion with widget kits and `PanelId(Arc<str>)`; best core-contract fidelity (the only proposal that operationalizes AGENTS.md's boundary-translation rule as data) and the strongest parity proof (behavioral golden frames, not just deserialization). Its penalties are concrete and cheap for an expansion agent to fix: name wasm32/native example checks (CK-006) and state persisted-`SavedLayout` compatibility (CK-012).

**Diversity rule applied.** Raw 3rd place C-A (3.67) shares A1's core mechanism — core state container + injectable store + feature-complete spec + Rust-side parity check with the shell retained (C-A self-describes as "the natural convergent design"); its deltas (schemars export, deserialize-only check, no render hatches) are parameterizations of the same architecture, so expanding both would produce convergent designs. Per the spec's diversity rule C-A is dropped in favor of the next-highest distinct mechanism, C-B. {A1, C-E, C-B} are pairwise distinct: retention-with-hatches vs deletion-kit vs event-algebra inversion. (C-E adopts C-B's `Snapshot`/`Event` vocabulary but its load-bearing decisions differ: delete vs demote the shell, `PanelMeta` data vs `PanelId`+trait, zero library persistence vs host-owned save policy — CK-013 would still be YES for the pair.)

**Runners-up worth mining for fragments:** B2's slug-addressed spec + examples-consume-spec parity (fold into any winner); C-D's NixOS-module authoring and C-F's parity-matrix check (both compatible with C-E); B1's exact `spec-roundtrip` anchor.

### Concerns every expansion agent MUST address (forwarded from this review)

1. **Reconcile with the uncommitted working-tree refactor** (this is the biggest risk to any expansion): core at working-tree already has `SavedLayoutV2`/`StoredLayout`/`Units`/`migrate_v1`/`reconcile_units` (persisted shape is versioned NOW — CK-012 answers change meaning), `SurfaceProfile`/`effective_mode` with `WEB_COMPACT_MAX = 760.0` in core (the "hardcoded web breakpoint" premise is already half-fixed; the spec's `breakpoint` item must map onto `SurfaceProfile`, and `viewport_is_mobile` may already delegate), a keyboard command surface (`Key`/`KeyChord`/`PanelCommand`/`command_for`/`apply_command`) that every reducer/Event design must incorporate, and `floating_content_height`. Expansion specs must cite and build on the working tree, not HEAD.
2. **A1/C-B:** state explicitly how the `Action`/`Event` surface covers the new keyboard commands and `SurfaceProfile`, and add the missing value-equality assert (A1) / wasm32+native example checks (C-B) named in the penalties.
3. **C-E:** persisted-layout compatibility must be stated (recipes reuse `merge_defaults`; now also `StoredLayout`/`migrate_v1`); the `zones` hit-testing recipe must be concrete or the kit is unusable; Dioxus `Signal` ownership migration needs a worked example.
4. **All three:** consumers jump-cannon and apple-notes-ocr-flow need exact old→new signatures and a cutover order; the README "Public API unchanged" line (README.md:26-27 at HEAD) must be re-anchored in the same change.
5. **All three:** the existing hand-mirror `nix/examples/workspace-canary.nix` has already drifted (7 vs 9 panels) — the expansion should delete it, not upgrade it, and should state what happens to `checks.layout-canary-schema` (jq) once a real deserialization check exists.
6. **Float stability:** any golden/byte-comparison check must spec canonical JSON/float handling on both sides (`mkLayout.nix`'s `toFloat` hack at :43-46 is the existing load-bearing mechanism).

RANKING:
  1: A1 Headless core reducer + pluggable store + escape-hatch shells
  2: C-E Shell Deletion: Capability Kit and Metadata-as-Data
  3: C-B Headless Event Reducer + Frame Projection; Backends as Widget Kits
SCORES:
  A1: 4.10/5.0
  A2: 1.25/5.0
  A3: 3.20/5.0
  A4: 2.90/5.0
  A5: 1.00/5.0
  A6: 3.44/5.0
  B1: 1.75/5.0
  B2: 3.02/5.0
  B3: 2.00/5.0
  B4: 1.00/5.0
  B5: 1.25/5.0
  B6: 1.50/5.0
  C-A: 3.67/5.0
  C-B: 3.64/5.0
  C-C: 1.75/5.0
  C-D: 3.27/5.0
  C-E: 4.06/5.0
  C-F: 3.10/5.0
