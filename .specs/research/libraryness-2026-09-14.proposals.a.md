# panel-kit library-ness + pure-Nix specs — Phase 1 exploration (Explorer A)

Task: `local://tot-task.md` (binding). Repo ground truth: `local://panel-kit-context.md`
plus direct reads of `crates/panel-kit-core/src/lib.rs` (core), `src/lib.rs` (Dioxus
shell), `crates/panel-kit-tui/src/lib.rs` + `src/theme.rs` (TUI shell),
`nix/lib/mkLayout.nix`, `nix/examples/workspace-canary.nix`, `flake.nix`,
`AGENTS.md`, `README.md`, `examples/workspace.rs`,
`crates/panel-kit-tui/examples/workspace_canary.rs`. Design only; no code.

---

## Step 1 — Problem decomposition

**Core problem.** panel-kit is factored correctly *vertically* (core state math in
`panel-kit-core`, two thin backends — the AGENTS.md rule) but not *horizontally*: each
backend is a **framework shell** that owns the state container, the render tree, the
persistence mechanism, and the policy knobs, and the Nix DSL
(`nix/lib/mkLayout.nix`) expresses only the persisted `SavedLayout` JSON — with no
build-proven parity against the Rust examples. The design task is to make the pieces
composable and opt-in (library), keep core as the contract, and make a Nix-authored
specification fully interchangeable with a Rust example, proven by `nix flake check`.

**Binding constraints.**
- AGENTS.md: core-first (`panel-kit-core` is the state-machine shape both renderers
  implement); backends = platform adapters only; examples are canaries (web examples
  → `wasm32-unknown-unknown`, TUI examples native).
- Toolchain pins: dioxus 0.6 via `nixpkgs-dioxus` (nixos-25.05), `wasm-bindgen-cli`
  in lockstep with `Cargo.lock`'s 0.2.121 (`flake.nix:6-8,160-164`). Crane builds use
  a wasm32 toolchain (`flake.nix:42-45`).
- Two known consumers (jump-cannon, apple-notes-ocr-flow) via git dependency
  (`README.md:9-15`); README currently promises "Public API unchanged from before the
  split" (`README.md:26-27`). Breaking changes must be named and staged.
- "Prefer deleting/simplifying over adding abstraction; every new abstraction must be
  justified by a concrete composition it enables" (task constraints).

**Subproblems every solution must address** (evidence in parens):

1. **Core state ownership / reducer shape.** Core ships free functions over
   `&mut [PanelWin<K>]` (`begin_drag` core:447, `apply_drag` core:497,
   `reorder_tile` core:552, `restore` core:554, `visible_panels` core:581,
   `merge_defaults` core:597) but **no aggregate state type** — each backend
   re-implements owning `panels + mode + drag + scroll` and the load/merge/save
   lifecycle (`use_workspace` src/lib.rs:298-397; `TuiWorkspace::new`
   tui/lib.rs:186-218 with its own `merge_defaults` call at tui/lib.rs:193).
2. **Persistence abstraction.** Web: private `save_layout`/`load_layout`
   (src/lib.rs:212-240) over gloo-storage, keyed by `storage_key: &'static str`,
   seeded by a `fn() -> Vec<PanelWin<K>>` **pointer** that cannot capture config
   (src/lib.rs:298-300). TUI: `store: Option<PathBuf>` + JSON file
   (`TuiWorkspace::save` tui/lib.rs:230-240). Format (`SavedLayout`, core:586-592) is
   shared; mechanism, keying, and save-on-settle policy are hardcoded per backend.
3. **Render composition without IoC.** `Workspace<K>::render(body)` (impl block
   src/lib.rs:399-829) and `TuiWorkspace::render(f, area, body)` (tui/lib.rs:251-256)
   own the entire panel/dock tree; the app injects a body closure — the framework
   smell called out in the task. You cannot render one panel, interleave your own
   nodes, or use the chrome without adopting the whole shell.
4. **PanelKind openness.** `trait PanelKind: Copy + Eq + Hash + Serialize +
   DeserializeOwned + 'static` with `title(self) -> &'static str` (core:35-41) forces
   a closed, compile-time enum. Nix specs name kinds as bare strings
   (`kind = "Workspace"`, nix/examples/workspace-canary.nix:20-26) matched to Rust
   variants **by convention only** — no runtime-defined panels, no data-driven spec.
5. **Theme/chrome in core.** Chrome *math* is core (`workspace_chrome` core:191,
   `ChromeMetrics` core:156, `Clamp`/`Clamp::WEB` core:309-355); the *palette* is not:
   `Theme` exists only on TUI (`theme.rs:10-37`, presets `DARK`/`PAPER` theme.rs:42-74)
   while web theming is `:root` CSS variables inside the `CSS` blob (src/lib.rs:64-71,
   `pub const CSS` src/lib.rs:109).
6. **Widget parity.** TUI-only modules `charts, meter, scroll, spinner, status, table,
   theme` (tui/lib.rs:36-42); web ships only `badge` + `Spinner` (src/lib.rs:88,
   src/lib.rs:850). Even badge has three shapes: core `badge.rs` (`BadgeKind`,
   `tag_hue`), the Dioxus `badge` component (props/events), and the TUI badge struct.
   The TUI canary exercises charts/gauges/tables (`workspace_canary.rs` imports
   `panel_kit_core::badge::{tag_hue, BadgeKind}` and defines `Metrics`, `STAGES`,
   `capacity_items`, `node_rows`) — none expressible in Nix.
7. **Nix spec schema & feature-completeness.** `mkLayout { tiling, panels }` emits
   only `SavedLayout` JSON (mkLayout.nix:83-102). Not expressible: mode toggle vs
   saved tiling flag, chrome metrics, theme/palette, badges, widgets/content
   bindings, persistence key/policy, mobile breakpoint (760px hardcoded inside
   `viewport_is_mobile` src/lib.rs:142-153), charset (`Charset` tui/lib.rs:79-90),
   scroll behavior. Flake exports only
   `lib.{mkLayout,winStates,tileWMax,tileHMax}` (flake.nix:111).
8. **Parity proof mechanism.** The `layout-canary-schema` check is a jq *shape* test
   (flake.nix:113-138); `nix/examples/workspace-canary.nix` hand-mirrors
   `workspace_canary.rs::defaults()` with **no** Rust deserialization round-trip and
   no value comparison — parity is unverified (context file, confirmed by flake.nix).
9. **Consumer migration.** jump-cannon + apple-notes-ocr-flow compile against
   `use_workspace`, `Workspace`, `PanelKind`, `LayoutBuilder` (README.md:52-70,
   patch mechanism README.md:127-134); any redesign must name its breaking deltas.

**Evaluation criteria.** (a) library-ness: bypassable IoC, replaceable persistence,
composable pieces usable independently; (b) core-firstness per AGENTS.md; (c) Nix spec
feature-complete relative to the Rust examples; (d) parity proven by `nix flake check`;
(e) canary examples for every backend incl. the Nix path; (f) explicit, staged
migration for the two consumers; (g) minimal new abstraction; (h) TDD-decomposable
acceptance criteria.

---

## Step 2 — Solution-space map

Independent dimensions along which approaches vary:

- **D1 Core state shape:** free fns only (status quo) · plain `WorkspaceState<K>`
  aggregate + existing free fns · Elm-style reducer (`Action` + `update`) ·
  instruction-stream algebra (interpreter) · generic-free data registry.
- **D2 Persistence:** hardcoded per backend · `LayoutStore` trait with backend impls ·
  spec-declared policy (key/format/when) consumed by core · command/sink model.
- **D3 Render composition:** shell owns tree + body callback · shell + documented
  escape hatches · per-panel/per-dock components the user assembles · spec-driven
  generic runner (no user render code at all).
- **D4 Panel identity:** closed `Copy` enum trait (status quo) · trait kept + built-in
  string-id kind for data-driven use · registry (`id → metadata`) · no generics.
- **D5 Nix strategy:** extended hand-rolled emitter · Rust-types-first JSON Schema
  export (schemars) validated against in Nix · NixOS-module-system typed options ·
  Nix→Rust codegen at build time · Nix emits runtime-consumed JSON artifact.
- **D6 Parity proof:** jq shape (status quo) · Rust round-trip bin in flake check ·
  committed golden diff (Rust→JSON vs Nix→JSON) · single-source generation (mirror
  impossible) · end-to-end example built from spec + snapshot equality.
- **D7 Widgets:** leave asymmetric + document · semantic widget model in core rendered
  per backend · separate widget crate(s).

Cross-cutting trade-off axes: breakage vs cleanliness; abstraction weight vs
composability; compile-time safety (generics/enums) vs runtime data-driven
flexibility; codegen vs runtime data; Nix-expression complexity vs Rust check
binaries.

---

## Step 3 — Six approaches

### A1. Headless core reducer + pluggable store + escape-hatch shells

**Summary.** Grow `panel-kit-core` a small `WorkspaceState<K>`/`Action` reducer and a
`LayoutStore` persistence trait, keep the shells as optional sugar over granular
per-panel pieces, and extend the Nix DSL to a versioned `WorkspaceSpec` JSON whose
parity with Rust is proven by a round-trip check binary in `nix flake check`.

**Description.** Core keeps its existing vocabulary — `begin_drag`/`apply_drag`
(core:447/497), `reorder_tile`/`restore` (core:552/554), `visible_panels` (core:581),
`merge_defaults` (core:597) — and gains a plain-data `WorkspaceState<K> { panels,
mode, drag, ws_scroll }` plus an `Action`-style API (`update(&mut state, action,
&env)`) that *wraps* those functions rather than duplicating them. Both backends
delete their hand-rolled lifecycle: `use_workspace` (src/lib.rs:298) and
`TuiWorkspace::new` (tui/lib.rs:186) collapse to constructors over the shared state,
so load/merge/save/mode/drag ownership exists exactly once (AGENTS.md: "if behavior
should match, encode it in core first"). Persistence becomes a minimal core trait —
roughly `load() -> Option<String>` / `save(&str)` over the already-shared
`SavedLayout` JSON (core:586) — with `LocalStore` in the Dioxus crate and
`JsonFileStore` in the TUI crate; `defaults` generalizes from the `fn()` pointer
(src/lib.rs:300) to an owned value or `impl FnOnce`, and the save-on-settle policy
becomes explicit data instead of web-shell behavior.

Library-ness lands as *escape hatches beside the shell*: the web `Workspace<K>` keeps
`render(body)`/`dock()`/`root_class()` but the crate additionally exports the
granular pieces those are made of — a per-panel chrome component (`panel_element(ws,
idx, body)`), a dock component, and viewport projection helpers built on
`effective_rect` (core:404) and `workspace_chrome` (core:191); TUI mirrors this with
`draw_panel(f, area, state, idx, body)`. The body-callback remains the easy path
(jump-cannon/apple-notes-ocr-flow keep compiling modulo the constructor change), but
a user can own the tree and call pieces. Theme moves to core as a renderer-neutral
`Palette` (hex fields mirroring `Theme`'s CSS-var names, theme.rs:7-37) with
`Theme::from(&Palette)` and a generated `:root` var block replacing the hardcoded
dark values.

Nix: `nix/lib/` grows a versioned `mkSpec` (or a `spec = { ... }` envelope around
today's `mkLayout`, mkLayout.nix:83) covering everything the Rust examples express:
panels + geometry + `WinState` + tile spans (already there), mode, chrome metrics
(`ChromeMetrics` fields, core:156-161), palette, per-panel badges and widget/content
bindings (table/chart/meter/status per the TUI canary's `node_rows`/`capacity_items`),
persistence `{ key, format, policy }`, mobile breakpoint, and charset
(`Charset::Unicode|Ascii`, tui/lib.rs:83-90). A new core type `WorkspaceSpec`
(serde) is the single interchange contract. Parity is proven by a new native check
binary (`spec-check`) wired into `checks`: it deserializes the JSON produced from
`nix/examples/workspace-canary.nix` (upgraded), re-serializes, and asserts equality
against a golden JSON emitted by a Rust unit test beside
`workspace_canary.rs::defaults()` (workspace_canary.rs:36-50) — replacing today's
jq-only `layout-canary-schema` check (flake.nix:118-138). A web example
(`examples/spec-workspace.rs`) seeds from the same JSON, making the Nix↔Rust
interchange a canary on both backends.

**Key design decisions and rationale.**
- Reducer wraps existing free fns — deletes the duplicated lifecycle code in both
  shells instead of adding a parallel API (constraint: prefer deleting).
- `LayoutStore` is deliberately tiny (two methods, string payloads): format stays
  `SavedLayout`; the spec envelope is a separate artifact so persisted layouts from
  existing users (localStorage keys like `panel_kit_example_workspace`,
  examples/workspace.rs:42) keep loading unchanged.
- `PanelKind` trait is untouched (closed enums stay the ergonomic default); Nix-driven
  examples use a built-in string-kind impl of it. No identity-model revolution.
- Schema direction: Rust types are the source of truth (core-first), Nix emits and a
  Rust check validates — matches how `mkLayout.nix` already documents itself against
  core's serde schema (mkLayout.nix:3-16).

**Trade-offs.** Gain: real composability, replaceable persistence, single lifecycle,
and build-proven spec parity at moderate cost; the two consumers migrate with one
constructor change. Sacrifice: the README's "Public API unchanged" promise
(README.md:26-27) breaks once and must be re-anchored; a reducer layer + a spec type
are two new abstractions (justified: each deletes duplicated behavior and enables the
Nix interchange respectively); `WorkspaceSpec` vs `SavedLayout` are two JSON contracts
that must be clearly documented or they will confuse users.

**Probability:** 0.90 — this is the high-mass center of the space: minimal breakage,
directly encodes the task's interpretation, reuses every existing seam.

**Complexity:** medium-high (core reducer + store trait + spec schema + two examples
+ three flake checks).

**Potential risks and failure modes.** Spec-schema scope creep (content bindings for
five chart types can balloon — mitigate by making widget bindings an enum with
per-widget option attrsets); `Action` bikeshed stalling implementation; golden-JSON
flake from float formatting (`builtins.toJSON` vs serde_json — `mkLayout.nix`'s
`toFloat` hack at mkLayout.nix:43-46 shows this is already load-bearing); web escape
hatches leaking CSS-class coupling; consumers needing the `[patch]` path
(README.md:132) to pin mid-migration.

### A2. Composition-first: dissolve the shell into a user-owned tree

**Summary.** Invert render ownership outright: backends ship state handles plus
per-panel/per-dock primitives the app assembles itself, the current `Workspace` /
`TuiWorkspace` become thin optional facades for migration, and Nix
feature-completeness is proven end-to-end by a generic spec-runner example built from
a Nix-authored JSON with zero hand-written panel code.

**Description.** The web crate exposes `use_workspace_state(...) -> WorkspaceSignals`
(state + action methods + persistence, **no DOM**) plus composable components —
`WorkspaceSurface`, `PanelChrome`, `Dock` — that the app places inside its own `rsx!`
tree in any order, wrapping any app nodes it likes. Event translation becomes
explicit library calls: DOM events → core `PointerEvent` (core:109-116) → state
methods; the wheel-chaining heuristic (`panel_body_absorbs_wheel`, src/lib.rs:210)
ships as a function apps invoke, not hidden shell behavior; `viewport_is_mobile`
(src/lib.rs:153) similarly. On TUI, `TuiWorkspace::render` (tui/lib.rs:251) is split
into `layout_pass(&state, area) -> Vec<PanelSlot>`, `draw_chrome(f, &slot, theme)`,
and `hit_test(&state, pos) -> Hit` — exactly the `zones`/`dock_chips` bookkeeping the
shell already keeps internally (tui/lib.rs:168-171) — so users can interleave rows
between panels or replace the dock. `Workspace<K>` and `TuiWorkspace<K>` survive as
deprecated facades composing those pieces, which is what jump-cannon and
apple-notes-ocr-flow compile against today (README.md:52-70).

Nix completeness is demonstrated *by construction*: each backend gains a generic
`spec_runner` example that builds an entire workspace from a `WorkspaceSpec` JSON —
string panel ids bound to widget modules (`table`, `charts::time_series`, `meter`,
`status` — tui/lib.rs:36-42), palette → `Theme` (theme.rs:10), charset, persistence
policy. `nix build .#workspace-from-nix` embeds the upgraded
`nix/examples/workspace-canary.nix` output into the runner, and a `checks` job builds
it for native (TUI) and wasm32 (web runner, matching the `browser-tui-example`
precedent at flake.nix:143-146). Interchangeability is then literal: a Nix-authored
app builds and runs; parity beyond schema is asserted by a snapshot check that the
runner's normalized state equals the Rust canary's (`workspace_canary.rs`) for the
same input.

**Key design decisions and rationale.**
- Facade retention keeps both git-dependency consumers compiling while the pieces
  become the documented primary API — the staged-breakage path the task requires.
- The TUI layout/hit split is not new design, it's publishing what
  `TuiWorkspace::render` already does internally (`zones`, `dock_chips`, `hover`,
  tui/lib.rs:168-179) — deleting the monopoly, not the code.
- String-id spec runner sidesteps `PanelKind` genericity for data-driven apps without
  touching the trait for hand-written apps: two coexisting identity models, opt-in.
- Spec-runner examples satisfy "examples as canaries" for the Nix path explicitly
  (AGENTS.md:28-36).

**Trade-offs.** Gain: maximal library-ness (user owns the tree; every capability is
an independently callable value); the Nix path is proven end-to-end, not just
schema-deep. Sacrifice: a long dual-API surface (facade + pieces) to maintain;
fine-grained Dioxus components fight rsx ergonomics (Signal plumbing through props);
the runner's widget registry is only as complete as the TUI widget set — web runner
parity lags until web widgets exist (subproblem 6); more examples/checks to keep
green.

**Probability:** 0.85 — the strongest pure answer to "composition over inheritance",
second only to A1 because of its migration weight.

**Complexity:** high (two backends re-exposed + registry + two runner examples +
snapshot infra).

**Potential risks and failure modes.** Facade and pieces drift into subtly different
behavior — needs a shared-state test asserting the facade is sugar over the pieces;
Dioxus 0.6 component/prop model may force awkward `Signal` re-creation per panel
(component granularity risk); ratatui version churn breaking snapshot parity; runner
widget bindings becoming a second, weaker widget API that rots.

### A3. Contract-first: schemars schema export + typed Nix options + golden parity

**Summary.** Treat the interchange contract as the product: derive `JsonSchema` on
core's spec types, export a versioned JSON Schema as a flake package, rebuild the Nix
DSL as typed `lib.types.submodule` options validated against that schema, and prove
parity with a committed golden-JSON diff in `nix flake check` — with only surgical
library-ness changes to Rust.

**Description.** The measured gap is mostly contract-shaped: `mkLayout` covers one of
~nine feature areas (geometry/state only — subproblem 7) and its parity is jq-shape
only (flake.nix:118-138). A3 fixes the contract mechanically: core (or a tiny
`panel-kit-spec` crate re-exporting core types) gains `schemars` derives on
`SavedLayout` (core:586), a new `WorkspaceSpec` (panels, mode, `ChromeMetrics`
core:156, palette, badges/widgets, persistence policy, breakpoint, charset), and a
`schema-export` binary that emits a versioned JSON Schema file as
`packages.panel-kit-schema`. The Nix side is rebuilt as `nix/lib/spec.nix` with
`lib.types.submodule` options mirroring the schema, plus a check that validates a
probe attrset against the exported schema (Rust validator binary run under
`nix flake check`) — spec typos then fail at *Nix eval* time, not at Rust
deserialize time. The existing hand-maintained `workspace-canary.nix` mirror gets a
guard: a Rust unit test beside `workspace_canary.rs::defaults()`
(workspace_canary.rs:36-50) serializes the canary `WorkspaceSpec` to a committed
golden `specs/canary.json`; a check re-evaluates the Nix spec to JSON and `diff`s
byte-for-byte (float-stability already engineered via `toFloat`, mkLayout.nix:43-46);
the reverse direction is a `WorkspaceSpec::from_json` round-trip test.

Library-ness changes are surgical and low-risk: the `LayoutStore` trait +
non-`fn()`-pointer defaults from A1 (subproblems 1-2), core `Palette` projected to
`Theme::from` and a generated CSS-var block (theme.rs:7-37, src/lib.rs:64-71), and
nothing else — shells keep their shape this phase. Rationale: the task's parity and
feature-completeness goals are where the objective evidence is; render-composition
work (A2-style) can land as later slices on a now-stable contract.

**Key design decisions and rationale.**
- Rust types as schema source of truth (core-first, AGENTS.md:3-23) — the Nix options
  are checked against the export, never hand-trusted (fixes today's convention-only
  `kind = "Workspace"` string matching, mkLayout.nix:13-16).
- Versioned envelope (`{ v = 1; ... }`) from day one so the spec can evolve without
  breaking persisted layouts (`SavedLayout` stays the persistence format).
- Golden files are committed, so drift shows up as reviewable diffs rather than CI
  mysteries; flake exports grow `lib.{mkSpec, schema}` alongside today's
  `lib.{mkLayout, ...}` (flake.nix:111).

**Trade-offs.** Gain: the strongest, cheapest correctness story for the Nix side and
a contract future phases can build on; near-zero churn for consumers. Sacrifice:
leaves the framework smells (IoC render, god-handle) mostly intact for this phase;
`schemars` is a new core dependency with derive noise; schema↔option coverage checks
are themselves a test artifact to maintain; golden diffs churn on cosmetic
serialization changes.

**Probability:** 0.82 — very likely to be *part* of the final design; slightly less
likely to be the whole first move because it under-delivers the library-ness half.

**Complexity:** medium (derives + export bin + Nix options + two checks).

**Potential risks and failure modes.** schemars version churn inside the pinned
toolchain (0.8 vs 1.0 derive differences); option↔schema coverage check giving false
confidence if options are written by hand and drift silently (mitigate: generate
option stubs from the schema); byte-stable golden requiring canonical float/JSON
settings on both sides; "spec-complete" temptation to over-model widget content that
belongs to apps.

### A4. Instruction-stream core: the workspace as an interpreted command algebra

**Summary.** Re-model the entire library as a closed `Cmd` instruction set with a
pure interpreter in core — backends merely translate platform events into commands
and interpret draw commands, persistence is a `Persist` command routed to an injected
sink, and a Nix spec *is a program*: a JSON tape of commands that the same
interpreter executes.

**Description.** Core defines `enum Cmd { BeginDrag{..}, PointerMove{x,y}, PointerUp,
Reorder{a,b}, Restore(k), SetMode(Mode), Wheel(d), Persist, DrawPanel{idx, rect},
DrawDock{chips}, ... }` and `step(state, cmd) -> (state, Vec<Event>)` — a pure
interpreter that subsumes today's free functions (`begin_drag` core:447, `apply_drag`
core:497, `reorder_tile` core:552, `restore` core:554 become interpreter arms, and
the "shell calls these functions" contract in core's doc comment, core:1-21, becomes
"shell emits these commands"). Backends contain zero logic: DOM/crossterm events map
1:1 onto `PointerEvent`/`PointerEventKind` (core:85-116) then to `Cmd`s; rendering is
an interpretation of `Draw*` commands against Dioxus `rsx!` or a ratatui `Frame`.
User code composes command sequences as values — capabilities literally compose
(functional composition over object composition). A Nix spec is feature-complete *by
construction*: everything any Rust example does is a command sequence, so
`mkProgram` emits a JSON `Cmd` tape and "spec vs example" reduces to "two inputs to
one interpreter"; parity is proven by recording canonical command traces from
`workspace_canary.rs` and the ratzilla `browser_tui` example (built by
`checks.browser-tui-example`, flake.nix:143-146) and diffing them against
Nix-emitted tapes in a check.

Why it sits in the tail of the distribution: Rust's lack of HKT/free-monad
ergonomics makes every call site `interpret(step(state, cmd))` ceremony; `Draw`
commands fight both renderers' native models (rsx! is a tree you declare, ratatui
borrows a `Frame`); the migration cost for jump-cannon/apple-notes-ocr-flow is total;
and it is maximal added abstraction, directly against the task's
"prefer deleting/simplifying" constraint. It remains in the space because it is the
*purest* "library, not framework" answer — control flow is entirely in the caller's
hands, and the state machine becomes trivially property-testable.

**Key design decisions and rationale.** `#[non_exhaustive]` closed `Cmd` set
versioned in core; traces (not screenshots) as the parity artifact; persistence as a
command so policy (when to save) becomes data inspectable in the tape.

**Trade-offs.** Gain: total composability and testability; one execution semantic
shared by three frontends (web, TUI, Nix); parity degenerates into input equality.
Sacrifice: the entire public API (three times over), runtime overhead, docs/examples
rewrite, and readability — this is framework-shaped weight in anti-framework
clothing.

**Probability:** 0.04 — almost certainly rejected at design review for weight and
migration cost.

**Complexity:** high (arguably very high).

**Potential risks and failure modes.** Tape format versioning hell; interaction
ordering semantics (who owns batching?) undefined without more design; backend
`Draw` interpreters quietly re-growing shells; command-granularity churn breaking
recorded traces; consumers cannot follow.

### A5. Nix as the source of truth: build-time codegen of the Rust examples

**Summary.** Flip the parity direction entirely — the Nix spec is primary and Rust
example code is generated from it at build time (panel enums, `defaults()`, palettes,
chrome metrics, persistence keys), deleting the hand-maintained mirror so drift is
*impossible* rather than detected.

**Description.** Today `nix/examples/workspace-canary.nix` hand-mirrors
`workspace_canary.rs::defaults()` (workspace_canary.rs:36-50) and nothing verifies
them against each other (subproblem 8). A5 deletes the mirror: a deterministic Rust
generator (`spec-gen`, native bin) consumes the spec JSON and emits the
example-specific Rust source — the `Panel` enum + `PanelKind::title` impl
(workspace_canary.rs:7-34), the `defaults()` currently built with `LayoutBuilder`
(core:278-307), `Theme` constants, `ChromeMetrics`, storage key — into a flake
derivation; crane's source fileset (flake.nix:46-56) is extended with the generated
directory so `nix build .#workspace-tui-canary` compiles generated code. The web
examples do the same for wasm32. `PanelKind` stays for consumers who want hand-written
kinds; the generator path is for examples and for any app that prefers declaring its
panel set in Nix. Feature completeness is trivial: the spec can express anything the
generator can emit, and since Rust examples are *produced from* the spec, "Nix spec
and Rust example are interchangeable" holds by construction.

Why tail-of-distribution: it contradicts the repo's own canary philosophy — AGENTS.md
calls examples "executable documentation" (and README.md:97-102 curates them as
such); generated examples are build artifacts, not documentation, and reviewing them
means reviewing the generator. Codegen inside `nix develop` complicates the pinned
dioxus-0.6 / wasm-bindgen-0.2.121 lockstep (flake.nix:6-8, 160-164), spec errors
surface as generated-code compile errors (poor DX), and Hydra gains an eval/build
step with the farm's constraints (flake.nix:19-30). It also inverts the AGENTS.md
core-first rule in spirit: the contract now lives in the generator's templates, not
in `panel-kit-core` types.

**Key design decisions and rationale.** Generator written in Rust (deterministic,
pretty output, runs native in the devshell); generation cached by the flake; spec
schema shared with core types so the generator never invents fields.

**Trade-offs.** Gain: zero-drift forever, total Nix expressiveness (it emits Rust),
the canary mirror code is deleted. Sacrifice: examples stop being readable
documentation; build complexity and generator maintenance; opaque errors; a second
"source of truth" culture shock for contributors.

**Probability:** 0.06 — an elegant answer to the parity clause specifically, but it
trades away the repo's stated values to get there.

**Complexity:** high.

**Potential risks and failure modes.** Hydra evaluation cost and sandboxing of the
generated-source path; rustfmt/IDE story for generated files; partial adoption
(some examples generated, some hand-written) creating exactly the two-sources
problem it was meant to kill; consumers see no library-ness improvement at all.

### A6. Pure-data workspace: registry + string ids, no generics — Nix JSON *is* the runtime config

**Summary.** Delete `PanelKind` genericity altogether: panels are string ids in a
runtime `Registry` (id → title/slug/defaults/widget bindings), the whole workspace is
one serde-round-trippable `WorkspaceDoc`, both shells consume doc + registry, and a
Nix-built `workspace.json` is loaded at runtime by the stock example binaries —
making Nix/Rust interchangeability degenerate (same bytes, same types) at the cost of
compile-time panel exhaustiveness.

**Description.** `PanelWin<K>` (core:241-266) becomes non-generic over a
`PanelId = SmolStr`; a `Registry` value (a plain map — a *library* value, not a
global) supplies `title`, slug (reusing `kind_slug`, core:608-620), default
geometry, badge factories, and widget bindings; `reorder_tile`/`restore`, which rely
on `K: PartialEq + Copy` (core:552-564), monomorphize once to the id type. The full
app-facing state — panels, mode, palette, chrome metrics, widget bindings,
persistence policy, breakpoint, charset — collapses into one `WorkspaceDoc` that
serde round-trips; the web and TUI shells take `(doc, registry)` and nothing else.
The persistence format is unaffected in practice: `SavedLayout` already serializes
kinds as bare variant-name strings (mkLayout.nix:13-16 documents this), so ids are
wire-compatible. Feature completeness and parity become near-free: the Rust canaries
construct their `WorkspaceDoc` in code, a check serializes it and diffs against the
Nix-emitted doc (one schema, one type — no mirror semantics), and a "Nix-only" app is
the stock runner binary + `workspace.json` produced by `nix build`, with zero
per-app Rust. Runtime-defined panels, plugins, and user-authored layouts all fall out
naturally — the maximal openness pole of D4.

Why tail-of-distribution: every consumer body closure is a `match` over its enum
(e.g. `examples/workspace.rs`'s four-variant `Panel`, examples/workspace.rs:59-76);
string ids turn typos into runtime misses and identity checks into string compares;
the README's usage story is literally "the app supplies a `PanelKind` impl"
(README.md:48-50); and both git-dependency consumers would need deep rewrites, which
the task's migration clause makes very expensive. It also weakens core's cheapest
invariant (`Copy + Eq + Hash`, core:29-36) for a flexibility nobody has yet asked
for.

**Key design decisions and rationale.** Registry is injected data (composition, not a
framework global); `WorkspaceDoc` is the single serde surface for both persistence
and spec interchange; `kind_slug` and all geometry/drag free functions survive
unchanged — only the identity parameterization changes.

**Trade-offs.** Gain: dynamic panels, spec==state (parity degenerate), zero-code Nix
apps, one non-generic API across backends. Sacrifice: compile-time exhaustiveness and
`Copy`-cheap identities everywhere; consumer rewrites; typo-class runtime bugs;
slightly worse hot-path identity compares (irrelevant at panel counts, but a
documented regression from the trait's stated rationale, core:29-36).

**Probability:** 0.08 — the honest extreme of "data-driven composability", unlikely
to survive contact with the consumers and the enum-ergonomics culture of the repo.

**Complexity:** high.

**Potential risks and failure modes.** Registry lookup failures at render time
(require fallback/panic policy); doc schema churn breaking persisted layouts if not
carefully versioned; widget-content bindings still need code for anything interactive
(Nix can describe a chart but not an event handler — "feature complete" hits a
philosophical wall the design must scope); consumer migration cost likely fatal.

---

## Step 4 — Diversity verification

| # | D1 state | D2 persistence | D3 render | D4 identity | D5 Nix | D6 parity | D7 widgets |
|---|---|---|---|---|---|---|---|
| A1 | aggregate + reducer | trait store | shell + escape hatches | trait kept (+string kind) | extended emitter → spec JSON | Rust round-trip check bin | document + core badge model |
| A2 | signals/state handle | trait store | **user-owned tree of pieces** | trait kept + registry runner | spec JSON → runner | **end-to-end built example + snapshot** | runner binds TUI widget set |
| A3 | (unchanged) | trait store only | unchanged shell | unchanged trait | **schema-first, typed options** | **committed golden diff** | minimal |
| A4 | **instruction interpreter** | Persist cmd | draw-command interpretation | cmds carry ids | **Nix emits a command tape** | **trace equality** | cmds |
| A5 | unchanged | unchanged | generated examples | **codegen'd enums** | **Nix→Rust codegen (truth flips)** | **generation (undriftable)** | generated |
| A6 | **generic-free data doc** | doc save | shells consume doc+registry | **string-id registry, no trait** | **Nix JSON = runtime config** | **type identity** | registry bindings |

- Genuinely different: A1/A2/A3 disagree on the two live axes (state shape, render
  ownership, contract direction) while sharing the pragmatic center; A4–A6 each occupy
  a distinct extreme (algebraic interpreter / direction-of-truth codegen /
  type-design revolution) that the high-probability three deliberately avoid.
- Coverage: conventional (A1 incremental, A3 contract-first), semi-conventional
  (A2 composition-first), and unconventional (A4, A5, A6) regions are all sampled;
  both codegen and runtime-data poles of the codegen-vs-data axis appear (A5 vs A6).
- No approach is a minor variation of another: they differ in at least three of the
  seven dimensions.
