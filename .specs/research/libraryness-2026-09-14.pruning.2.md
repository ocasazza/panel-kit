# ToT Phase 2 — Pruning evaluation (Judge 2 / PrunJudgeB)

Rubric: `.specs/research/libraryness-2026-09-14.pruning-spec.yaml`, applied verbatim
(20 checklist items; 9 weighted dimensions; essential gate cap 2.0; −0.25 per important
NO; −0.25 per pitfall YES; floor 1.0). All 18 proposals scored. Weights: Composability
0.16, Core-Contract 0.15, Nix Coverage 0.16, Parity Proof 0.15, Net Simplification
0.08, Migration 0.08, Feasibility 0.06, Risk 0.06, TDD 0.10.

## Citation verification (CK-007/CK-008 basis)

Verified ~45 distinct file:line/symbol citations against the repo (HEAD `6120e9b`,
the snapshot matching `local://panel-kit-context.md`). Sample results — all REAL:

- `begin_drag` core:447 ✓, `apply_drag` core:497 ✓, `SavedLayout` core:587≈586 ✓,
  `merge_defaults` core:597 ✓, `kind_slug` core:609≈608 ✓, `PanelKind` core:35-41 ✓,
  `PanelWin` core:242 ✓, `PointerEvent` core:109 ✓, `LayoutBuilder` core:280 ✓,
  `ChromeMetrics` core:156 ✓, `Clamp::WEB` core:333 ✓, `TileMetrics::WEB` core:370 ✓,
  core doc quote "owns a `Vec<PanelWin<K>>` plus a `Mode` and an `Option<Drag>`"
  core:6 ✓ (B1), `workspace_chrome` core:178 (cited 191 — minor drift, symbol real).
- `use_workspace` src/lib.rs:275 (cited 298 — drift), `save_layout`/`load_layout`
  src/lib.rs:205/212 ✓ (cited 212-240 ✓), `viewport_is_mobile` src/lib.rs:140 (cited
  142-153 ✓), `panel_body_absorbs_wheel` src/lib.rs:170 (cited 169-210 ✓), `CSS`
  src/lib.rs:109 ✓, `Workspace` struct src/lib.rs:227 (cited 250-277 — drift),
  `STORAGE_KEY` examples/workspace.rs:41-42 ✓, Panel enum examples/workspace.rs:59-76 ✓.
- TUI: modules tui/lib.rs:36-42 ✓, `Charset` :84 ✓, `Zone` :137 ✓, `TuiWorkspace`
  :156 ✓, `zones/dock_chips` :169-170 ✓, `new` :186 ✓, `save` :230 ✓, `render` :251 ✓,
  `dragging` :243 ✓, `ROW_CELLS` :64 ✓; theme.rs `Theme` :10 ✓, `DARK`/`PAPER`
  :42/:60 ✓; canary `Panel` :8, `defaults` :36, `noise` :117, `Metrics` :128,
  `STAGES` :146 ✓; `capacity_items`/`node_rows` in workspace_canary.rs ✓.
- flake.nix: dioxus pin :6-8 ✓, hydra capacity :19-30 ✓, wasm toolchain :42-45 ✓,
  fileset :46-56 ✓, canary-mirror comment :74-83 ✓, `layout-canary` :82-83 ✓,
  `update-version` :97-107 ✓, `lib.${system}` :111 ✓, jq check :113-138 ✓,
  `browser-tui-example` :142-145 ✓, wasm-bindgen lockstep :157-163 ✓, hydraJobs :170+ ✓.
- README :9-15/:26-27/:48-50/:52-70/:97-102/:127-134 ✓; badge.rs `BadgeKind::Entity`
  :34 ✓, `Rgb` :12 ✓; AGENTS.md :3-23/:28-36 incl. the verbatim "platform events
  should be translated at backend boundaries into core input types" ✓; mkLayout.nix
  `toFloat` :43-46 ✓, "for codegen" :29 ✓, `normalizePanel` :50 ✓.

Verdict: **no fabricated citations found in any of the 18 proposals** → CK-008 = NO
(no penalty) for all. Line drift in `src/lib.rs` (±20-40 lines) is snapshot drift, not
fabrication. Note: the working tree is being concurrently modified (src/lib.rs grew
928→966 lines during evaluation; `viewport_is_mobile` already replaced by
`surface_profile` there); HEAD + context file are the stable verification baseline.

Scoring discipline: default 2, deviations justified with quoted evidence; eight 5s
awarded across 162 dimension judgments (4.9% < 5% cap).

---

## A1 — Headless core reducer + pluggable store + escape-hatch shells

| item | ans | evidence |
|---|---|---|
| CK-001 | YES | all seven fields present |
| CK-002 | YES | "`defaults` generalizes from the `fn()` pointer (src/lib.rs:300) to an owned value"; save-on-settle "becomes explicit data instead of web-shell behavior" |
| CK-003 | YES | reducer + `LayoutStore` + `Palette` + `WorkspaceSpec` in core; "load/merge/save/mode/drag ownership exists exactly once"; `LocalStore`/`JsonFileStore` = backend mechanisms only |
| CK-004 | YES | spec covers "mode, chrome metrics, palette, per-panel badges and widget/content bindings, persistence `{ key, format, policy }`, mobile breakpoint, and charset" |
| CK-005 | YES | "`spec-check`" native check binary "deserializes the JSON produced from `nix/examples/workspace-canary.nix`… re-serializes, and asserts equality against a golden JSON emitted by a Rust unit test" |
| CK-006 | YES | web example `spec-workspace.rs` seeds from same JSON; "three flake checks" |
| CK-007 | YES | verified citations throughout |
| CK-008 | NO | none fabricated (spot-checked 9) |
| CK-009 | YES | native crane check; no pin changes |
| CK-010 | YES | each addition tied: reducer "deletes the duplicated lifecycle code in both shells"; spec "enables the Nix interchange" |
| CK-011 | YES | "jump-cannon/apple-notes-ocr-flow keep compiling modulo the constructor change"; README promise break named |
| CK-012 | YES | "persisted layouts from existing users… keep loading unchanged" (examples/workspace.rs:42 cited) |
| CK-013 | YES | distinct from A2 (tree inversion) and A3 (contract-only) |
| CK-014–020 | NO | no Nix dep, no hand-mirror, no god-object, probability consistent (0.90/medium-high/mitigated), no per-backend duplication, both halves, sketches only |

Dimensions: Composability **4** ("the crate additionally exports the granular pieces…
a per-panel chrome component, a dock component" — shell optional but body-closure
remains default; breakpoint story implicit). Core-Contract **5** ("state, persistence
shape, theme, spec" all core; backends = signals/CSS/mechanisms). Nix Coverage **5**
(all 11 items incl. widget bindings "an enum with per-widget option attrsets"; spec is
a core serde type). Parity **4** (`spec-check` deserialize+equality vs Rust-emitted
golden; web example seeds from JSON but canary `defaults()` values still duplicated).
Simplification **4** (deletes both shells' lifecycle; "two new abstractions (justified:
each deletes duplicated behavior…)"). Migration **4** (constructor change named;
persisted JSON status given). Feasibility **4** (reuses `toFloat` float hack
mkLayout.nix:43-46, existing check wiring). Risk **3** (float-formatting + scope-creep
mitigated; Action bikeshed/CSS coupling not). TDD **4** (reducer tests, serde
round-trip, `spec-check`, wasm example — first tests evident).

**Weighted 4.25. Penalties: none. Final 4.25 — strong.**

## A2 — Composition-first: dissolve the shell into a user-owned tree

Essential CK-003 **NO**: the cross-backend `WorkspaceSpec` type's home is unstated,
the widget binding registry is explicitly per-backend ("the runner's widget registry
is only as complete as the TUI widget set"), and the state container stays
per-backend (`WorkspaceSignals` web / TUI handle). → **cap 2.0.**

Checklist otherwise: CK-001 Y; CK-002 Y ("invert render ownership outright"); CK-004 Y
(widget modules, palette, charset, persistence policy; chrome/breakpoint unlisted);
CK-005 Y (snapshot check: "the runner's normalized state equals the Rust canary's");
CK-006 Y (browser-tui-example precedent cited); CK-007 Y; CK-008 NO; CK-009 Y; CK-010
Y; CK-011 Y ("deprecated facades… which is what jump-cannon and apple-notes-ocr-flow
compile against today"); CK-012 **NO** (persisted JSON unstated, −0.25); CK-013 Y;
CK-014–020 NO.

Dimensions: Composability **4** (user owns tree; but web kit fights "rsx ergonomics
(Signal plumbing through props)"). Core-Contract **2**. Nix Coverage **4**. Parity
**4** (end-to-end built example + snapshot). Simplification **3** ("a long dual-API
surface (facade + pieces) to maintain"). Migration **3** (facade keeps consumers;
persisted data unstated). Feasibility **4**. Risk **3** (facade-drift mitigated by
shared-state test; rest unmitigated). TDD **3**.

**Weighted 3.54 − 0.25 (CK-012) = 3.29 → capped 2.00 — weak (gated).**

## A3 — Contract-first: schemars schema export + typed Nix options + golden parity

Checklist: CK-001 Y; CK-002 Y ("the `LayoutStore` trait + non-`fn()`-pointer defaults
from A1"); CK-003 Y (spec types + Palette in core); CK-004 Y (full list incl.
badges/widgets/persistence/breakpoint/charset); CK-005 Y ("a check re-evaluates the
Nix spec to JSON and `diff`s byte-for-byte" against Rust-test-emitted golden +
validator bin); CK-006 Y; CK-007 Y; CK-008 NO; CK-009 Y (schemars churn risk noted,
pins intact); CK-010 Y; CK-011 **NO** ("near-zero churn for consumers" while the
imported A1 slice changes `use_workspace`'s signature — impact unstated, −0.25);
CK-012 Y ("`SavedLayout` stays the persistence format"); CK-013 Y; CK-014–020 NO
(CK-019 NO: it does include concrete library-ness changes, unevenly).

Dimensions: Composability **3** (store/defaults/palette replaceable; "shells keep
their shape this phase"). Core-Contract **3** (state container not moved; spec/theme
core). Nix Coverage **4** (full list; content-binding mechanism thinner than A1).
Parity **4** (committed golden + validator + from_json round-trip). Simplification
**3** ("schema↔option coverage checks are themselves a test artifact to maintain").
Migration **2** ("near-zero churn" one-liner). Feasibility **4**. Risk **4**
(option-stub generation mitigation, canonical float settings). TDD **4**.

**Weighted 3.45 − 0.25 (CK-011) = 3.20 — viable.**

## A4 — Instruction-stream core (command algebra)

Essential CK-004 **NO**: theme, badges, breakpoint, charset never named as spec
inputs — coverage asserted "by construction" only. → **cap 2.0.** Also CK-010 **NO**
("this is maximal added abstraction, directly against the task's
'prefer deleting/simplifying' constraint", −0.25); CK-012 **NO** (−0.25).

Checklist otherwise: CK-001 Y; CK-002 Y; CK-003 Y (interpreter core, "backends
contain zero logic"); CK-005 Y (trace diff against Nix-emitted tapes); CK-006 Y;
CK-007 Y (verified); CK-008 NO; CK-009 Y; CK-011 Y ("migration cost… is total");
CK-013 Y; CK-014–020 NO.

Dimensions: Composability **4** ("control flow is entirely in the caller's hands" —
but every call site pays `interpret(step(state, cmd))` ceremony). Core-Contract **4**.
Nix Coverage **3** (no named items/mechanism for content). Parity **4** ("parity
degenerates into input equality"). Simplification **1** ("framework-shaped weight in
anti-framework clothing"). Migration **2**. Feasibility **2** ("`Draw` commands fight
both renderers' native models"). Risk **3**. TDD **2** (big-bang).

**Weighted 3.22 − 0.50 → 2.72 → capped 2.00 — weak (gated).**

## A5 — Nix as source of truth: build-time codegen of Rust examples

Essential CK-002 **NO** (no IoC/hardcoded-policy mechanism touched — "consumers see
no library-ness improvement at all"), CK-003 **NO** ("the contract now lives in the
generator's templates, not in `panel-kit-core` types"), CK-005 **NO** (compilation of
generated code, not Rust-side deserialization/comparison). → **cap 2.0.** Also
CK-010 NO (−0.25), CK-012 NO (−0.25), CK-019 **YES** (Nix-only half; −0.25).

Dimensions: Composability **1** (nothing changes for app control flow).
Core-Contract **2**. Nix Coverage **4** ("the spec can express anything the generator
can emit"). Parity **5** ("since Rust examples are *produced from* the spec… 'Nix
spec and Rust example are interchangeable' holds by construction" — literal anchor-5
mechanism). Simplification **2**. Migration **3** (consumers unaffected, stated).
Feasibility **3** (Hydra eval/sandbox, generated-file DX). Risk **3**. TDD **3**.

**Weighted 2.91 − 0.75 = 2.16 → capped 2.00 — weak (gated).**

## A6 — Pure-data workspace: registry + string ids

Essential CK-005 **NO**: parity check never named as a `checks.*` job ("a check
serializes it and diffs" — no attribute, no deserialization detail). → **cap 2.0.**

Checklist otherwise: CK-001 Y; CK-002 Y (PanelKind closed-enum + shell constructor
hardcodings dissolved: "the web and TUI shells take `(doc, registry)` and nothing
else"); CK-003 Y; CK-004 Y (full list: "panels, mode, palette, chrome metrics, widget
bindings, persistence policy, breakpoint, charset"); CK-006 Y (weak); CK-007 Y;
CK-008 NO; CK-009 Y; CK-010 Y (runtime panels/plugins/Nix apps named); CK-011 Y
("both git-dependency consumers would need deep rewrites"); CK-012 Y ("ids are
wire-compatible"); CK-013 Y; CK-014–020 NO.

Dimensions: Composability **3** (registry injectable; render ownership unchanged).
Core-Contract **4**. Nix Coverage **4** ("spec==state (parity degenerate)"; content
wall honestly scoped). Parity **4** (type identity; check under-specified).
Simplification **3** (deletes genericity; adds registry/doc + typo bug class).
Migration **2** ("consumer migration cost likely fatal"). Feasibility **4**.
Risk **3**. TDD **2** (identity swap is workspace-wide big-bang).

**Weighted 3.34 → capped 2.00 — weak (gated).**

## B1 — Core `WorkspaceState` + `LayoutStore` trait

Essential CK-004 **NO**: the Nix side stays geometry-only — "a new `spec-roundtrip`
flake check … deserializes `packages.layout-canary` … into `SavedLayout<K>`" adds a
parity proof but zero new spec scope (its own trade-off: "`PanelKind` stays a closed
compile-time enum so Nix specs still cannot define panel kinds"). → **cap 2.0.**
This is exactly the geometry-only-extension shape the spec's rationale warns about.

Checklist otherwise: CK-001 Y; CK-002 Y (LocalStorageStore/JsonFileStore/MemoryStore;
closures); CK-003 Y; CK-005 Y (`spec-roundtrip` named, asserts == `defaults()`);
CK-006 Y; CK-007 Y (core:3-16 doc quote verified verbatim); CK-008 NO; CK-009 Y;
CK-010 Y; CK-011 Y (old+new `use_workspace` signatures, "breaks both consumers");
CK-012 Y ("SavedLayout wire format untouched"); CK-013 Y; CK-014–020 NO (CK-019 NO:
it does add a Nix mechanism, just not coverage).

Dimensions: Composability **3** ("render IoC remains (body closure still inverts
control)"). Core-Contract **4** (`WorkspaceState::restore(store, defaults)` collapses
duplicated lifecycle). Nix Coverage **1** (unchanged from `mkLayout` — anchor-1).
Parity **4** (anchor-4 exactly: native test bin, deserialize, assert ==).
Simplification **4**. Migration **4**. Feasibility **4**. Risk **3**. TDD **4**.

**Weighted 3.30 → capped 2.00 — weak (gated).**

## B2 — Schema-first `WorkspaceSpec`

Checklist: CK-001 Y; CK-002 Y (marginal — "storage policy (key + backend selector)"
replaces the hardcoded `&'static str` key on the spec path; theme duplication removed
from core); CK-003 Y (`WorkspaceSpec` + theme tokens in core, "per-backend
converters"); CK-004 Y (full list incl. "per-panel content bindings scoped exactly to
what the canary examples render"); CK-005 Y (`spec-parity`: "deserializes the Nix
JSON into `WorkspaceSpec`, re-serializes, and byte-compares"); CK-006 Y (examples read
/ `include_str!` the spec; checks wired); CK-007 Y; CK-008 NO; CK-009 Y; CK-010 Y
(content bindings "capped at example parity" — scoped); CK-011 **NO** (jump-cannon /
apple-notes-ocr-flow never mentioned, −0.25); CK-012 **NO** (users' persisted
SavedLayout JSON never addressed, −0.25); CK-013 Y; CK-014–020 NO.

Dimensions: Composability **2** (nothing changes about control flow; body closure
mandatory). Core-Contract **4** (spec/theme core; slug→K mapping mechanism:
"specs address panels by slug and apps map slug→K via `PanelKind::title`").
Nix Coverage **5** (all 11 items named; identity mechanism; schema emitted for
`WorkspaceSpec<String>`). Parity **5** (`spec-parity` byte-compare AND canary
"reads `spec-canary.json`… deleting the hand-mirror" — single source). Simplification
**3** (deletes the mirror; adds schema surface "that needs versioning"). Migration
**2** (silent). Feasibility **4** (sum-type discriminator convention for
`BadgeKind::Entity` addressed). Risk **3**. TDD **4**.

**Weighted 3.69 − 0.50 (CK-011, CK-012) = 3.19 — viable.**

## B3 — Non-breaking facade: capability modules beside compat shells

Essential CK-003 **NO**: the persistence abstraction is defined in a backend crate —
"new `panel_kit::persist::{LayoutStore, LocalStorage, JsonFile, Noop}`" is the
Dioxus (wasm-only) crate's namespace, yet both backends' persistence is "refactored
to internal uses" of it; a wasm-only crate cannot serve the native TUI crate. →
**cap 2.0.**

Checklist otherwise: CK-001 Y; CK-002 Y (`use_workspace_with(WorkspaceConfig{…})`
exposes "the 760 px breakpoint, `Clamp::WEB`, `Theme`"); CK-004 Y (mkTheme/mkChrome
beyond geometry — marginal); CK-005 Y ("the same Rust round-trip check as B1");
CK-006 Y; CK-007 Y; CK-008 NO; CK-009 Y; CK-010 Y; CK-011 Y ("byte-compatible for
jump-cannon/apple-notes-ocr-flow" + why); CK-012 Y (weak); CK-013 Y; CK-014–020 NO.

Dimensions: Composability **3** (headless `layout_model` escape hatch; "the
framework path remains the default" — own admission). Core-Contract **2** (shared
persistence concept added in backend namespace). Nix Coverage **2** (toolbox =
geometry + theme + chrome). Parity **4** (B1's check, inherited). Simplification
**2** ("two ways to do everything… persists indefinitely"). Migration **4**.
Feasibility **3** (the `panel_kit::persist` placement is broken as written).
Risk **3** (RenderModel-divergence mitigation tied to golden tests). TDD **4**.

**Weighted 2.94 → capped 2.00 — weak (gated).**

## B4 — Nix→Rust codegen (committed spec crate)

Essential CK-002 **NO**, CK-003 **NO** (PanelKind impls/consts live in the generated
crate; core untouched), CK-005 **NO** ("proven by compilation, no round-trip test
needed" — not a Rust-side deserialization/comparison). → **cap 2.0.** Also CK-010 NO
(−0.25), CK-012 NO (−0.25), CK-019 **YES** (−0.25).

Dimensions: Composability **1**. Core-Contract **2**. Nix Coverage **3** (theme/clamp/
defaults/enums named). Parity **4** (definitional; but committed-code fallback
"is precisely the hand-mirror… now with merge conflicts"). Simplification **2**.
Migration **3**. Feasibility **3** ("Circularity: crane needs a source tarball before
it can build the generator" — real, named). Risk **3**. TDD **3**.

**Weighted 2.60 − 0.75 = 1.85 (below cap) — weak.**

## B5 — Data-driven registry / spec-executing engine

Essential CK-005 **NO** (no parity check job described at all — spec "loaded at
startup" only). → **cap 2.0.** Also CK-006 **NO** (−0.25), CK-012 **NO** (−0.25),
CK-016 **YES** ("a registry is a service locator, providers are inversion of control
the user cannot bypass" — own admission, −0.25).

Dimensions: Composability **1** (worse than today). Core-Contract **4** (descriptor
model core, two interpreters). Nix Coverage **4**. Parity **2** (prose only).
Simplification **1** ("the descriptor language is a UI framework in disguise").
Migration **2** ("total rewrites"). Feasibility **3**. Risk **3** (honest).
TDD **2**.

**Weighted 2.50 − 0.75 = 1.75 (below cap) — weak.**

## B6 — Dissolve into leaf crates

Essential CK-005 **NO** ("per-leaf round-trip checks" — no named `checks.*` job).
→ **cap 2.0.** Also CK-006 **NO** ("Canary coverage shrinks because the canaries
*were* the shells" — own admission, −0.25), CK-012 **NO** (−0.25).

Dimensions: Composability **4** ("every piece independently usable, zero IoC" — not
5: subtle capabilities like the ResizeObserver re-projection are deleted, not exposed
as pieces, so not all capabilities remain composable). Core-Contract **4** (single
homes; backends thin kits). Nix Coverage **2** ("no monolithic spec because there is
no monolithic object to specify"). Parity **3**. Simplification **4** (deletes
~1580 lines of shells). Migration **2**. Feasibility **4**. Risk **3**. TDD **4**.

**Weighted 3.47 − 0.50 = 2.97 → capped 2.00 — weak (gated).**

## C-A — Core-Owned Reducer with Injectable Store (schema-first Nix spec)

Checklist: CK-001 Y; CK-002 Y ("persistence is the only genuinely hardcoded policy
today (`&'static str` key + `fn()` pointer…; `Option<PathBuf>`…) — a narrow trait
deletes two hardcodings"); CK-003 Y (`WorkspaceState` + `LayoutStore` + unified
`Theme` + spec in core; "ResizeObserver… must stay as an adapter concern");
CK-004 Y (registry, layout, chrome, theme, content bindings, persistence policy,
breakpoint, charset — all named); CK-005 Y ("`spec-canary-check` reads
`packages.spec-canary` JSON and `serde_json::from_str::<WorkspaceSpec<TestPanel>>`
it — replacing the jq-only `layout-canary-schema`"); CK-006 Y ("`spec_workspace` on
wasm32 for web, native for TUI"); CK-007 Y; CK-008 NO; CK-009 Y (schemars behind
feature); CK-010 Y (theme unification is "the deletion lever: today's two parallel
14-field palettes collapse into one"); CK-011 Y ("`use_workspace` signature breaks
(two consumers must patch call sites)… mechanical to fix"); CK-012 **NO**
(already-persisted localStorage/JSON files never addressed, −0.25); CK-013 Y;
CK-014–020 NO.

Dimensions: Composability **4** ("the state and store are public, so an app can own
the loop"; body closure remains by default). Core-Contract **5** (state, store,
theme, spec each core; gloo/fs impls per backend as mechanisms; DOM logic stays
adapter). Nix Coverage **4** (full list + registry/kind_slug mechanism; slightly less
specified content bindings than B2). Parity **4** (parse-only check + both backends'
canaries "boots from the Nix-emitted JSON" — no explicit value-equality assert).
Simplification **4**. Migration **4**. Feasibility **4** (web_sys-in-core trap
flagged with adapter solution). Risk **4** (god-object regrowth discipline, eager
Nix-eval validation mitigation). TDD **4**.

**Weighted 4.15 − 0.25 (CK-012) = 3.90 — strong.**

## C-B — Headless Event Reducer + Frame Projection; Backends as Widget Kits

Checklist: CK-001 Y; CK-002 Y ("the app hands control to `Workspace::render(body)`…
and gets called back. This approach inverts it"; wheel policy "decided by the host";
persistence host-owned); CK-003 Y (`Snapshot`/`Event`/`reduce`/`project` + `PanelId`
in core; backends publish widget kits; AGENTS boundary quote verified verbatim);
CK-004 Y (full spec "as in A"); CK-005 Y ("a native Rust binary that
`reduce`/`project`-drives the spec into a `Frame` and asserts golden values —
proving *behavioral* parity"); CK-006 Y (keeps canaries; Nix path exercised by the
parity check); CK-007 Y; CK-008 NO; CK-009 Y; CK-010 Y (events "property-testable in
core without a DOM or terminal"; `project()` "publishes them as the composition
boundary"); CK-011 Y ("both consumers break beyond a signature change (their event
handlers and render calls move)"); CK-012 Y ("`SavedLayout`/`merge_defaults` as
today" — format kept); CK-013 Y; CK-014–020 NO.

Dimensions: Composability **5** (user owns the loop; every chrome piece callable;
wheel/persistence/settle policies host-owned; shell demoted to "one composition").
Core-Contract **4** (kernel + PanelId core; kits render core outputs; widget content
models beyond badge remain per-backend). Nix Coverage **4**. Parity **4** (behavioral
golden Frame — semantically strongest; but golden files themselves a drift risk, no
single-source consumption). Simplification **3** ("biggest behavioral rewrite of the
two shells… largely re-plumbed"). Migration **3** (impact named; no staged path).
Feasibility **4** (pure Snapshot + Dioxus Signal reactivity risk honestly named).
Risk **4** (golden tests "must pin `Clamp::WEB`/`TileMetrics::WEB` and TUI cell
metrics (`ROW_CELLS`)" — mitigation tied to check invariants). TDD **4** (reduce/
project replay fixtures first).

**Weighted 4.00. Penalties: none. Final 4.00 — strong.**

## C-C — Config-Object Facelift

Essential CK-003 **NO**: `WorkspaceConfig` (consumed by both backends) has no stated
home crate, and the proposal's own trade-off keeps the shared lifecycle per-backend —
"state ownership stays duplicated per backend (load/merge/save orchestration still
written twice, just configured differently)". → **cap 2.0.** Also CK-012 **NO**
(−0.25).

Checklist otherwise: CK-001 Y; CK-002 Y (kills "`&'static str`/`fn()` pair and both
persistence hardcodings"); CK-004 Y (full attr list named; but "the TUI canary's
charts/table content is not" expressible — own admission); CK-005 Y (`spec-decode`
binary "deserializes every spec the flake exports"); CK-006 Y (spec feeds wasm web +
native TUI examples, wired into checks); CK-007 Y; CK-008 NO; CK-009 Y; CK-010 Y
("every option must name the hardcoding it deletes" discipline); CK-011 Y (single
breaking change, old signature + doc-example citation); CK-013 Y; CK-014 NO; CK-015
NO (hand-mirror deleted); CK-016 NO; CK-017 NO; CK-018 NO (duplication retained, not
newly defined); CK-019 NO; CK-020 NO.

Dimensions: Composability **2** ("the IoC critique is *not* answered" — own
admission). Core-Contract **2**. Nix Coverage **3** (full list but content deferred).
Parity **4** (decode + examples boot from one Nix file). Simplification **3** (mirror
deleted; "half-unification can add a third source of truth"). Migration **3**.
Feasibility **4** ("proportionate to the repo's size"). Risk **4** (self-aware
"judged as ducking the question"; decode-check mitigation). TDD **4**.

**Weighted 3.06 − 0.25 = 2.81 → capped 2.00 — weak (gated).**

## C-D — Nix as the Source of Truth: Spec-Compiled Examples

Essential CK-002 **NO** (no IoC/policy mechanism removed; shells become SpecViewers),
CK-003 **NO** ("a new `panel-kit-spec` crate is the sole Rust definition of the
spec" — outside core), CK-005 **NO** (parity "by construction" via embedded spec +
build matrix, not a Rust-side deserialization/comparison check). → **cap 2.0.**

Checklist otherwise: CK-001 Y; CK-004 Y (full coverage incl. "literal demo data as
plain Nix lists"); CK-006 Y (matrix: "web example on wasm32, TUI example native,
`browser-tui-example` style jobs at flake.nix:143-146"); CK-007 Y; CK-008 NO; CK-009
Y (vendored fallback keeps plain cargo); CK-010 Y (weak); CK-011 Y (migrate by
"authoring a Nix spec (or keeping Rust defaults behind `WorkspaceConfig::with_defaults`,
the escape hatch)"); CK-012 Y (spec-as-defaults mental model addressed; merge_defaults
"gains a spec-driven form"); CK-013 Y; CK-014 NO; CK-015 NO; CK-016 NO; CK-017 NO;
CK-018 NO; CK-019 NO; CK-020 NO.

Dimensions: Composability **3** ("nothing prevents a host from consuming
`panel-kit-spec` + core directly"). Core-Contract **3**. Nix Coverage **4** (limits
honestly scoped: "`Metrics`/`noise()` animation machinery… doesn't fit literal
data"). Parity **4** (single description; vendored sync check). Simplification **2**.
Migration **3**. Feasibility **3** (module-system-in-lib-export ergonomics risk).
Risk **4**. TDD **3**.

**Weighted 3.29 → capped 2.00 — weak (gated).**

## C-E — Shell Deletion: Capability Kit and Metadata-as-Data

Checklist: CK-001 Y; CK-002 Y (delete "the `Workspace<K>` signal bundle and its
430-line `impl`… `TuiWorkspace<K>` and its 500-line `impl`… the private duplicated
load/save pair… and the `PanelKind::title` method"); CK-003 Y (Snapshot/Event/reduce/
project + PanelMeta + one theme token set in core; backends = flat leaf capabilities);
CK-004 Y (mkSpec incl. "content bindings (including the TUI widget kinds)…
persistence policy… breakpoint, charset"); CK-005 Y ("a native decode+golden-project
check (spec JSON → `Snapshot<PanelId>` → `project()` → compare against golden
`Frame`)"); CK-006 Y ("every backend builds one canary assembled entirely from kit
pieces fed by the spec"); CK-007 Y; CK-008 NO; CK-009 Y; CK-010 Y; CK-011 Y ("both
consumers break hard… semver 0.3 + migration doc required"); CK-012 Y (weak — recipes
reuse `SavedLayout`/`merge_defaults`); CK-013 Y; CK-014–020 NO.

Dimensions: Composability **5** ("no assembly you cannot bypass"; persistence is
"example code"; shell = optional facade "clearly marked as one composition").
Core-Contract **4**. Nix Coverage **4**. Parity **4** (decode+golden-project+kit
canaries fed by spec). Simplification **4** (massive deletion; facade-crate risk
noted). Migration **2** (hard break, no staged path). Feasibility **3** ("Losing
`TuiWorkspace::zones` hit-testing… means kit consumers must rebuild pointer routing").
Risk **3**. TDD **3** ("deletion is still a rewrite of both shells' interiors").

**Weighted 3.78. Penalties: none. Final 3.78 — strong.**

## C-F — Spec Compiler with Capability-Assembled Shells and a Parity Matrix

Checklist: CK-001 Y; CK-002 Y (shells re-built from capabilities — "the shipped
shell is literally `Workspace::compose(capabilities…)`… one legal assembly";
breakpoint/persistence/theme become replaceable capability values); CK-003 Y (Kernel
+ widget view-models + IR in core); CK-004 Y (all dimensions incl. TUI widgets);
CK-005 Y ("a decode check per spec (`spec-version pinning` so a schema change fails
every cell)" + matrix build/render jobs); CK-006 Y (matrix "web-wasm, tui-native,
browser-tui" extending checks.browser-tui-example + hydraJobs); CK-007 Y; CK-008 NO;
CK-009 Y (weak — matrix cost flagged vs farm capacity); CK-010 **NO** (own admission:
"the abstraction budget (IR versioning, capability lattice) exceeds what the evidence
… justifies"; "speculative generality", −0.25); CK-011 Y ("jump-cannon keeps
`use_workspace`-shaped composition with default capabilities… compatibility
profile"); CK-012 Y ("migration rule per version so old persisted specs keep loading
(generalizing what `merge_defaults` does)"); CK-013 Y; CK-014–020 NO.

Dimensions: Composability **4** (capabilities "each a small standalone value the host
can replace or omit"). Core-Contract **4** (view-models in core close the TUI-only
gap). Nix Coverage **4**. Parity **4** ("a spec feature no backend renders is an
empty matrix cell that fails" — falsifiable coverage). Simplification **2**
("spec crate + IR versioning + capability lattice + view-model layer + matrix CI is
a lot of machinery for a 3-crate library"). Migration **3**. Feasibility **3**.
Risk **4** (matrix cost vs `flake.nix:19-30` capacity; stage-color leak
`workspace_canary.rs:146` cited). TDD **3**.

**Weighted 3.60 − 0.25 (CK-010) = 3.35 — viable.**

---

## Comparison table

| id | weighted | cap? (essential NO) | important NO (−0.25 ea) | pitfall YES (−0.25 ea) | **final** | label |
|---|---|---|---|---|---|---|
| A1 | 4.25 | — | — | — | **4.25** | strong |
| C-B | 4.00 | — | — | — | **4.00** | strong |
| C-A | 4.15 | — | CK-012 | — | **3.90** | strong |
| C-E | 3.78 | — | — | — | **3.78** | strong |
| C-F | 3.60 | — | CK-010 | — | **3.35** | viable |
| A3 | 3.45 | — | CK-011 | — | **3.20** | viable |
| B2 | 3.69 | — | CK-011, CK-012 | — | **3.19** | viable |
| A2 | 3.54 | CK-003 | CK-012 | — | **2.00** | weak (gated) |
| B6 | 3.47 | CK-005 | CK-006, CK-012 | — | **2.00** | weak (gated) |
| B1 | 3.30 | CK-004 | — | — | **2.00** | weak (gated) |
| C-D | 3.29 | CK-002/003/005 | — | — | **2.00** | weak (gated) |
| A6 | 3.34 | CK-005 | — | — | **2.00** | weak (gated) |
| A4 | 3.22 | CK-004 | CK-010, CK-012 | — | **2.00** | weak (gated) |
| B3 | 2.94 | CK-003 | — | — | **2.00** | weak (gated) |
| C-C | 3.06 | CK-003 | CK-012 | — | **2.00** | weak (gated) |
| A5 | 2.91 | CK-002/003/005 | CK-010, CK-012 | CK-019 | **2.00** | weak (gated) |
| B4 | 2.60 | CK-002/003/005 | CK-010, CK-012 | CK-019 | **1.85** | weak |
| B5 | 2.50 | CK-005 | CK-006, CK-012 | CK-016 | **1.75** | weak |

Cap semantics: final = min(weighted − penalties, 2.0) when any essential item is NO
(floor 1.0).

## Selection rationale

**1st — A1 (4.25).** The only proposal with no checklist deductions: full 11-item Nix
coverage with a content-binding mechanism, a named Rust-side round-trip parity check
replacing the jq check, state/persistence/theme/spec all homed in core, escape
hatches beside a retained shell, and an explicit persisted-JSON guarantee. It also
carries the spec's priority ordering correctly: it both extends coverage AND
mechanizes parity (`spec-check` deserializes + asserts equality), so it outranks
every geometry-only-extension proposal (B1, B3) by construction.
*Concerns for expansion:* (a) define the `Action` surface narrowly (its own
"Bikeshed" risk) — enumerate variants and forbid god-enum growth; (b) the
"built-in string-kind impl of `PanelKind`" for Nix kinds needs a concrete design
(kind-string validation inside `spec-check` against the canary enum); (c) golden-JSON
byte equality needs the float-canonicalization story extended beyond
`mkLayout.nix`'s `toFloat` (serde_json float formatting rules); (d) name the
slice order and each slice's first failing test; (e) state whether the web canary
embeds the spec JSON (vendored) or fetches it, and how plain-`cargo` builds see it.

**2nd — C-B (4.00).** The strongest library-ness mechanism: IoC actually inverted
(event reducer + `project()` + widget kits; wheel/persistence/settle policies
host-owned), plus behavioral golden-Frame parity — a stronger proof semantic than
shape checks — and an honest risk section with check-tied mitigations. Distinct from
A1 (data-shaped `Event` reducer + frame projection + kit publication vs method-state
+ escape hatches).
*Concerns for expansion:* (a) Dioxus reactivity plan for a pure `Snapshot` outside
`Signal`s (perf regression risk it names) — specify what the convenience shell owns;
(b) `Event` enum governance (`#[non_exhaustive]`? versioning?) so it doesn't become
the new god-type; (c) golden-Frame stability: pin `Clamp::WEB`/`TileMetrics::WEB`/
`ROW_CELLS` and choose coarse snapshots to avoid golden churn; (d) staged consumer
migration (facade first, then pieces) with named call-site deltas; (e) wire explicit
wasm32/native canary checks for the new kits; (f) bound the `K`+`PanelId` dual test
matrix.

**3rd — C-F (3.35, promoted by the diversity rule).** By raw score the next
candidates are C-A (3.90) and C-E (3.78), but C-A shares A1's core mechanism
(state-container-to-core + `LayoutStore` trait + full Nix spec + native Rust check —
a cross-file near-duplicate of the convergent design; CK-013 would be NO for the
pair) and C-E shares C-B's core mechanism (`Snapshot`/`Event`/`reduce`/`project` +
kit pieces — C-E's own text reuses "as in B"). Keeping the higher-scoring member of
each pair and promoting the next-highest distinct mechanism gives C-F: versioned IR +
capability-assembled shells + a falsifiable (spec × backend) parity matrix —
genuinely different from both A1 and C-B, and the only proposal that turns
"feature-complete" into a CI artifact ("an empty matrix cell that fails"). Its
−0.25 (CK-010, unjustified abstraction weight) is real: the expansion must prune.
*Concerns for expansion:* (a) justify or delete machinery — drop IR versioning until
a second spec version exists, keep the matrix + capability assembly (its own
"likely pruned back to A or B, keeping F's parity-matrix check as a fragment");
(b) scope the matrix to farm capacity (start 1 spec × 3 backends against
`flake.nix:19-30`); (c) define what a matrix "render job" proves beyond building
(golden output? exit code?); (d) make the jump-cannon compat wrapper an explicit,
tested artifact, not a promise; (e) hold the widget view-model boundary at
geometry/semantics (its `workspace_canary.rs:146` stage-color leak warning).

Not selected: C-A/C-E for mechanism duplication above; B2 (3.19) — best pure
contract-first Nix story but silent on consumers and persisted data, and its
coverage/parity ideas are already represented inside A1 and C-F; A3 (3.20) — same,
plus weaker library-ness.

RANKING:
  1: A1 Headless core reducer + pluggable store + escape-hatch shells
  2: C-B Headless Event Reducer + Frame Projection; Backends as Widget Kits
  3: C-F Spec Compiler with Capability-Assembled Shells and a Parity Matrix
SCORES:
  A1: 4.25/5.0
  A2: 2.00/5.0
  A3: 3.20/5.0
  A4: 2.00/5.0
  A5: 2.00/5.0
  A6: 2.00/5.0
  B1: 2.00/5.0
  B2: 3.19/5.0
  B3: 2.00/5.0
  B4: 1.85/5.0
  B5: 1.75/5.0
  B6: 2.00/5.0
  C-A: 3.90/5.0
  C-B: 4.00/5.0
  C-C: 2.00/5.0
  C-D: 2.00/5.0
  C-E: 3.78/5.0
  C-F: 3.35/5.0
