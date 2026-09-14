# Tree-of-Thoughts — Phase 1 proposals, Explorer B

Task: `local://tot-task.md` (library-ness + feature-complete pure-Nix specifications for panel-kit).
Repo ground truth: `local://panel-kit-context.md` + files read 2026-09-14.

## Step 1 — Problem decomposition

**Core problem, two halves:**

1. **Framework smell in Rust API.** The shells own control flow: `Workspace::render(body)` (src/lib.rs:399-829) and `TuiWorkspace::render(f, area, body)` (crates/panel-kit-tui/src/lib.rs:251-256) own the render tree and the app injects a body closure — inversion of control. State ownership is duplicated per backend (`use_workspace` at src/lib.rs:298 vs `TuiWorkspace::new` at crates/panel-kit-tui/src/lib.rs:186-218), and policies are hardcoded: localStorage via `save_layout`/`load_layout` (src/lib.rs:212-240), JSON file via `TuiWorkspace::save` (crates/panel-kit-tui/src/lib.rs:230-240), `storage_key: &'static str`, non-capturing `defaults: fn() -> Vec<PanelWin<K>>` pointers, and the 760 px breakpoint inside `viewport_is_mobile` (src/lib.rs:142-153). Core has no workspace-level state type at all — only free functions over `&mut [PanelWin<K>]` (`begin_drag`, `apply_drag`, `reorder_tile`, … crates/panel-kit-core/src/lib.rs:447-564).
2. **Nix layer is a geometry-only subset.** `nix/lib/mkLayout.nix` emits only `SavedLayout<K>` JSON (`{ value, json, kinds }`); nothing covers theme (`Theme`, crates/panel-kit-tui/src/theme.rs), chrome metrics (`ChromeMetrics`), badges (`crates/panel-kit-core/src/badge.rs`), widgets, charset (`Charset`), persistence policy, or breakpoint. Parity with Rust is asserted by a jq shape check only (`layout-canary-schema`, flake.nix:118-138); `nix/examples/workspace-canary.nix` mirrors the TUI canary's `defaults()` by hand with no Rust-side verification.

**Subproblems any solution must address (at least):** core state ownership/reducer shape; persistence abstraction; render composition without IoC; `PanelKind` openness vs data-driven specs; theme/chrome in core; widget parity across backends; Nix spec schema feature-completeness; parity-proof mechanism inside `nix flake check`; consumer migration (jump-cannon, apple-notes-ocr-flow on git deps; README.md:26-27 promises "Public API unchanged").

**Evaluation criteria:** opt-in composability (no forced god-object/IoC); core-is-the-contract (AGENTS.md); Nix⇄Rust parity proven by a build check; examples stay canaries (wasm32 web / native TUI); migration cost for the two known consumers; dioxus 0.6 + wasm-bindgen 0.2.121 lockstep intact (flake.nix:155-166); net deletion preferred over new abstraction.

## Step 2 — Solution-space map

| Axis | Options |
|---|---|
| Core state model | keep free fns; add `WorkspaceState<K>` struct + transitions; Elm-style `Model + Msg`; capability traits; runtime registry |
| Panel identity | closed `PanelKind` enum (today, core lib.rs:29-41); slug strings via `kind_slug` (core lib.rs:608); interned runtime ids |
| Persistence | hardcoded (today); `LayoutStore` trait in core; closures/values instead of `fn()` pointers |
| Render composition | shell-owned tree + body closure (today); headless `RenderModel` the app renders itself; per-backend component kits |
| Nix strategy | extend `mkLayout`; one `WorkspaceSpec` document validated against a Rust-emitted schema (schemars); NixOS-module typed options; Nix→Rust codegen; build-time embedded asset |
| Parity proof | jq (today); Rust deserialization/round-trip check binary; example seeded from Nix JSON; compilation of generated code |
| Migration | hard break; parallel opt-in API + facade; dissolve |

---

## Step 3 — Approaches

### B1 — Core `WorkspaceState` + `LayoutStore` trait: shells become thin adapters

**Summary.** Move the duplicated workspace state container (panels/mode/drag/scroll) and persistence policy into `panel-kit-core` as one plain struct plus pure transitions and a store trait, and make the Dioxus/ratatui shells thin signal/cell adapters over it.

**Description.** Add `panel_kit_core::WorkspaceState<K> { panels: Vec<PanelWin<K>>, mode: Mode, drag: Option<Drag>, tile_drag: Option<K>, ws_scroll: f64 }` with transitions in the existing free-function style (`begin_drag`/`apply_drag`/`reorder_tile`/`restore` already take `&mut [PanelWin<K>]` — core lib.rs:447-564; the struct versions simply own the whole set). The restore/merge logic currently duplicated in `use_workspace` (src/lib.rs:298-314, calling `load_layout` + `merge_defaults`) and `TuiWorkspace::new` (crates/panel-kit-tui/src/lib.rs:186-218) collapses into `WorkspaceState::restore(store, defaults)`. Persistence goes behind an object-safe core trait `LayoutStore<K>: fn load(&self) -> Option<SavedLayout<K>>; fn save(&mut self, &SavedLayout<K>)`, with backends shipping `LocalStorageStore` (wraps the gloo-storage code from src/lib.rs:212-240), `JsonFileStore(PathBuf)` (wraps `TuiWorkspace::save`), and `MemoryStore` for tests. `use_workspace` becomes `use_workspace(store: impl LayoutStore<K>, defaults: impl Fn() -> Vec<PanelWin<K>> + 'static)` — real closures, replaceable key/format/backend.

Render composition is deliberately *not* restructured in this approach: `Workspace::render(body)` keeps its body closure; the win is that state transitions, save-on-settle (src/lib.rs:379-386), and restore exist exactly once. Nix gets a real parity proof: a new `spec-roundtrip` flake check builds a tiny Rust test via crane that deserializes `packages.layout-canary` (flake.nix:82-83) into `SavedLayout<K>` and asserts it equals `crates/panel-kit-tui/examples/workspace_canary.rs::defaults()` — replacing jq-shape-only confidence (flake.nix:118-138).

**Key design decisions and rationale.** Plain struct + free fns over an Elm `Msg` enum: matches the documented core model ("owns a `Vec<PanelWin<K>>` plus a `Mode` and an `Option<Drag>`", core lib.rs:3-16) and keeps transitions individually testable without a dispatcher indirection. `LayoutStore` in core, not in backends: AGENTS.md puts "persisted layout shape, shared state transitions" in core; the *mechanisms* (gloo, fs) stay in backends as impls. `SavedLayout` wire format untouched: existing users' persisted layouts survive. `defaults` as closure/value: removes the `fn()`-pointer limitation cited in the context (cannot capture config).

**Trade-offs.** Gain: single source of truth for state/persistence, replaceable storage, honest tests of restore in core (`merge_defaults`, core lib.rs:597-606, becomes testable without a browser). Sacrifice: render IoC remains (body closure still inverts control), `PanelKind` stays a closed compile-time enum so Nix specs still cannot define panel kinds, and `use_workspace`'s signature breaks both consumers (the README "Public API unchanged" promise ends).

**Probability:** 0.85
**Complexity:** medium

**Risks and failure modes.** The Dioxus `Workspace` still needs its seven `Signal` fields (src/lib.rs:250-277) — if transitions aren't actually moved, `WorkspaceState` becomes dead packaging beside real logic. Generic `LayoutStore<K>` + dyn compatibility can force type-erasure gymnastics. Wasm file-store impls are untestable without wasm-bindgen-test wiring.

---

### B2 — Schema-first `WorkspaceSpec`: one document, schemars contract, Nix `mkWorkspace` emits, all examples consume

**Summary.** Define a feature-complete serializable `WorkspaceSpec` in core (superset of `SavedLayout`), derive `JsonSchema`, have `nix/lib/mkWorkspace.nix` emit it, and prove Nix⇄Rust parity by a round-trip `nix flake check` job; examples load the spec instead of hand-writing `defaults()`.

**Description.** Core gains `WorkspaceSpec` covering everything the examples express: panels (kind slug + geometry + `tile_w/tile_h` + `WinState` + z), `mode`, chrome (`ChromeMetrics`, core lib.rs:155), clamp (`Clamp`, core lib.rs:329 — today only `Clamp::WEB` exists as a constant), theme tokens (move the `Theme` field set from crates/panel-kit-tui/src/theme.rs:10-37 into core as RGB triples, like `badge::Rgb`, with per-backend converters), mobile breakpoint (currently hardcoded in `viewport_is_mobile`, src/lib.rs:142-153), charset (`Charset`, crates/panel-kit-tui/src/lib.rs:83-90), storage policy (key + backend selector), and per-panel content bindings scoped exactly to what the canary examples render (badge lists, chart series, table rows). Serde derives join `schemars::JsonSchema` so Rust is the contract authority.

Nix-side, `nix/lib/mkWorkspace.nix` (beside the existing `mkLayout`, which stays as the geometry-only layer it is) builds and validates the attrset; the flake exposes `packages.spec-canary` JSON and `packages.spec-schema` (emitted by a small Rust bin built with the existing crane setup, flake.nix:45-66). A new `spec-parity` check deserializes the Nix JSON into `WorkspaceSpec`, re-serializes, and byte-compares — machine-checked interchange, upgrading today's `layout-canary-schema` jq assertion (flake.nix:118-138). `crates/panel-kit-tui/examples/workspace_canary.rs` reads `spec-canary.json` (or embeds it via `include_str!` on the web, mirroring how `CSS` is embedded at src/lib.rs:109), deleting the hand-mirror in `nix/examples/workspace-canary.nix`.

**Key design decisions and rationale.** Rust-types-first with generated schema, not schema-first-by-hand: kills drift between the jq assertion and serde's actual expectations; the check fails on any field mismatch. Slug-based panel references: `kind_slug` (core lib.rs:608-620) already defines title→slug; specs address panels by slug and apps map slug→K via `PanelKind::title`, avoiding `K`-generic schema emission (the schema is emitted for `WorkspaceSpec<String>`). Theme in core as data: satisfies "if behavior should match, encode it in core first" (AGENTS.md) for the palette that today exists twice as `Theme::DARK` hex values and `:root` CSS vars (src/lib.rs docs at :64-71).

**Trade-offs.** Gain: genuine feature-complete Nix specs, parity proven by build, theme/chrome parity across backends, examples as true canaries of the spec path. Sacrifice: core grows serde+schemars surface and a public schema that needs versioning; Nix attrsets express sum types poorly (`BadgeKind::Entity { ty }`, crates/panel-kit-core/src/badge.rs:34-37, needs discriminator-field conventions); risk of the spec ballooning into a general declarative-UI framework config — the exact thing the user is trying to avoid — unless content bindings are capped at example parity.

**Probability:** 0.82
**Complexity:** high

**Risks and failure modes.** Schema churn red-flags downstream flakes pinning `lib.${system}` (flake.nix:111). Two types (`WorkspaceSpec<String>` vs `WorkspaceSpec<K>`) drift if conversion isn't exercised in tests. wasm32 schemars compatibility is fine but adds compile time to the wasm-only root crate.

---

### B3 — Non-breaking facade: extract capability modules, shells remain compatibility front-ends

**Summary.** Keep `use_workspace(storage_key, defaults)` and `TuiWorkspace::new(store, defaults)` byte-compatible for jump-cannon/apple-notes-ocr-flow, and add opt-in composable capabilities beside them (store trait, theme tokens, headless layout model, configured entry point).

**Description.** The hardcoded persistence in `save_layout`/`load_layout` (src/lib.rs:212-240) and `TuiWorkspace::save` (crates/panel-kit-tui/src/lib.rs:230-240) is refactored to internal uses of new `panel_kit::persist::{LayoutStore, LocalStorage, JsonFile, Noop}` types; a new opt-in constructor `use_workspace_with(WorkspaceConfig { store, breakpoint, clamp, theme, charset })` exposes what is today unconfigurable (the 760 px breakpoint, `Clamp::WEB`, `Theme`). For render ownership, add a headless `layout_model(state: &WorkspaceState, viewport) -> RenderModel` in core that composes existing pure fns (`workspace_chrome`, `effective_rect`, `visible_panels`, `front_z` — core lib.rs:145-191, 385-404, 566-581) into a per-frame draw list (regions, z order, dock chips), and refactor the shells' render internals over it so there is one semantics; users who want their own tree call `layout_model` directly and skip `Workspace::render`. Widget parity follows the badge precedent — `crates/panel-kit-core/src/badge.rs` already holds the renderer-neutral model (`BadgeKind`, `BadgeAction`, `tag_hue`) with two renderers — so spinner gets a core model + two renderers, while TUI-only modules (`charts`, `meter`, `table`, `status`) stay put until a web consumer exists. Nix grows as a toolbox beside `mkLayout` (`mkTheme`, `mkChrome`, `mkPanelWin` composing into `mkLayout`), plus the same Rust round-trip check as B1 — no monolithic spec document.

**Key design decisions and rationale.** Facade-first is the standard evolution move given two git-dependent consumers and the README's explicit "Public API unchanged" promise (README.md:26-27): every extraction ships independently and can be reverted. Nix-as-toolbox mirrors the library philosophy being asked for — composable functions, not a config file. `RenderModel` derived from functions that already exist in core means no new semantics, just an aggregation point.

**Trade-offs.** Gain: zero-pressure migration, incremental review, no consumer breakage. Sacrifice: two ways to do everything (legacy hardcoded path + capability path) persists indefinitely — the "second convention beside the existing one" smell; `WorkspaceConfig` tends to grow into a god-config; library-ness improves but the framework path remains the default and the task's §1 (no IoC the user cannot bypass) is only partially honored.

**Probability:** 0.80
**Complexity:** medium

**Risks and failure modes.** Dual API calcifies — the old path never gets deleted, so the design half-lands. The compat `fn()` pointer stays while the new path takes closures, teaching inconsistent style. If `RenderModel` output ever diverges from what the shells draw, it becomes a third semantics rather than the shared one (must be enforced by shell-refactor + golden tests).

---

### B4 — Nix as source of truth: generate Rust from the spec at build time

**Summary.** Flip the parity direction: `nix/specs/*.nix` are canonical and the flake generates a committed `panel-kit-spec` Rust crate (panel enum, default layout consts, theme/clamp consts) that examples and consumers compile against, making parity definitional.

**Description.** `mkLayout.nix` already returns `kinds` — "the list of distinct panel kinds (for codegen)" (nix/lib/mkLayout.nix:29) — this approach cashes that in. A generator (a `writeShellApplication` or a Rust bin run inside the flake) turns a Nix spec into `.rs`: a fieldless enum implementing `PanelKind` with `title` from Nix `title` fields, `pub const DEFAULTS: &[PanelWin<Panel>]` replacing every `LayoutBuilder` usage in examples (crates/panel-kit-tui/examples/workspace_canary.rs, examples/workspace.rs), `pub const THEME`, `pub const CLAMP`. The flake's crane source selection (`pkgs.lib.fileset.toSource`, flake.nix:46-56) is extended to include the generated crate, and `nix flake check` builds the canary examples against it — Nix spec → working Rust app proven by compilation, no round-trip test needed. `packages.update-version` (flake.nix:97-107, `cargo set-version --workspace`) is the in-repo precedent for a flake tool that rewrites workspace files.

**Key design decisions and rationale.** Codegen maximizes the "totally feature complete pure-Nix" reading: Nix cannot drift from Rust because Rust *is* Nix output. Panel kinds stay compile-time (full type safety, `match` exhaustiveness preserved) unlike B2's slug mapping. Generation lives in the flake so CI continuously proves the pipeline.

**Trade-offs.** Gain: single source of truth, compile-time kinds, no schema layer at all (Nix is the schema). Sacrifice: developer experience — `dx serve`/plain `cargo build` outside `nix develop` breaks unless generated code is committed, and committed generated code is precisely the hand-mirror (nix/examples/workspace-canary.nix) the task wants deleted, now with merge conflicts; build-graph complexity (cargo cannot invoke nix; crane source filtering must stay in sync with the generator's output paths); an entire pipeline added, against the repo's "prefer deleting/simplifying" constraint.

**Probability:** 0.08
**Complexity:** high

**Risks and failure modes.** Circularity: crane needs a source tarball before it can build the generator that reads Nix. Stale-checkout drift when devs edit generated files. Downstream git-dependency consumers (jump-cannon) can't see generated crates unless committed — reintroducing dual sources. Expensive to roll back once examples depend on it.

---

### B5 — Data-driven registry: runtime `PanelId`, plugin providers, panel-kit as a spec-executing engine

**Summary.** Embrace the framework pole: replace the closed `PanelKind` trait with interned runtime ids and a panel-provider registry whose content is a declarative widget-descriptor tree, so a Nix/JSON spec genuinely defines a complete app with no Rust panel code.

**Description.** `PanelKind` (core lib.rs:29-41: `Copy + Eq + Hash + 'static`, `title(self) -> &'static str`) is replaced by `PanelId` (slug string, normalized via `kind_slug`, core lib.rs:608) plus a `Registry` mapping id → `PanelMeta { title, defaults, provider }`. Providers implement a plugin trait returning declarative content — an enum of widget descriptors (text, badge list via `crates/panel-kit-core/src/badge.rs` types, chart series, table rows, meter) — and each backend becomes an interpreter over descriptors: the TUI widget modules (`charts`, `meter`, `scroll`, `spinner`, `status`, `table`, crates/panel-kit-tui/src/lib.rs:35-43) and web badge/spinner render the same model. `Workspace<K>`/`TuiWorkspace<K>` drop their generic (one non-generic `Workspace`), `render(body)` closures disappear (the registry supplies content), and a spec file loaded at startup configures panels, geometry, theme, charset, breakpoint, and persistence — the maximal reading of "pure Nix specifications, feature complete", since panels that don't exist in Rust can still be declared and mounted.

**Key design decisions and rationale.** Descriptor-based content structurally solves the web/TUI widget asymmetry the context file flags: one model, two interpreters, parity by construction. Runtime ids solve `PanelKind`'s closed-enum limitation directly. The registry is the natural endpoint of "specs define everything".

**Trade-offs.** Gain: total spec completeness, runtime extensibility, non-generic workspace types. Sacrifice: nearly everything §1 of the interpretation asks for — a registry is a service locator, providers are inversion of control the user cannot bypass, compile-time `match kind` safety (badge.rs's documented "identical match in browser and terminal") becomes stringly runtime dispatch, per-render allocation/dynamic dispatch costs, and the descriptor language is a UI framework in disguise.

**Probability:** 0.05
**Complexity:** high

**Risks and failure modes.** Descriptor sets converge to the lowest common denominator of CSS and terminal cells, or grow escape hatches that break parity anyway. jump-cannon/apple-notes-ocr-flow need total rewrites. Directly contradicts the binding interpretation ("composition over inheritance… no inversion of control"), so likely rejected — retained because it is the coherent opposite pole that makes the trade-off explicit.

---

### B6 — Dissolve the monolith: independent leaf crates, no Workspace type at all

**Summary.** Delete the shell objects (`Workspace`, `TuiWorkspace`) from the public API — demoting them to examples — and ship panel-kit as independent leaves (geometry, state, persist, theme, badge, spinner, per-backend render kits) that consumers compose in their own event loops and trees.

**Description.** panel-kit becomes a set of leaves, most of which already exist as pure cores: `panel-geometry` (`Region`, `effective_rect`, `Clamp`, `workspace_chrome`/`ChromeMetrics`, `max_scroll`/`clamp_scroll` — core lib.rs:119-191, 385-437), `panel-state` (`PanelWin`, `WinState`, `Mode`, `Drag` + `begin_drag`/`apply_drag`/`reorder_tile` transitions), `panel-persist` (`SavedLayout`, `merge_defaults`, core lib.rs:583-606, + store traits), `panel-theme`, `panel-badge` (crates/panel-kit-core/src/badge.rs is already standalone), `panel-spinner`, plus thin render kits: web Dioxus components the app assembles in its own `rsx!`, TUI widgets taking `&mut Frame` + `Rect` (like the existing `charts`/`meter` modules). Consumers own signals, the resize observer, scroll chaining, and the DOM/cell tree — calling `begin_drag`/`apply_drag` directly, exactly the usage model the core crate docs originally describe (core lib.rs:3-16). Nix mirrors the leaves: `nix/lib/` grows `mkRect`, `mkPanelWin`, `mkTheme`, `mkLayout` composables with per-leaf round-trip checks; no monolithic spec because there is no monolithic object to specify.

**Key design decisions and rationale.** This is the purest "library, not framework": every piece independently usable, zero IoC, composition literal in consumer code. It also satisfies "prefer deleting": the shells carry the bulk of the surface (~900 lines in src/lib.rs:250-896, ~680 in crates/panel-kit-tui/src/lib.rs:150-677) and the subtle logic they hide is exactly what a composability audit would question.

**Trade-offs.** Gain: maximal composability, trivially Nix-expressible leaves, no migration shims possible to linger. Sacrifice: every consumer reimplements the hard-won subtle parts — the ResizeObserver + scale-on-resize recompute in `use_workspace` (src/lib.rs:316-377), wheel scroll-chaining (`panel_body_absorbs_wheel`, src/lib.rs:169-210), TUI hit-zone tracking (`Zone`, crates/panel-kit-tui/src/lib.rs:136-148) — which is the duplicated-drift problem AGENTS.md and this repo's origin story (factored *out of* apple-notes-ocr-flow) exist to prevent. Both consumers break despite the README promise; "feature-complete Nix spec" gets *harder* (no single spec target); the ecosystem likely converges back to a shared shell crate, i.e., re-invents panel-kit.

**Probability:** 0.04
**Complexity:** medium

**Risks and failure modes.** Consumer-copied shell code drifts immediately (two consumers, two bugs). Hit-testing and event glue are real work to redeliver per app. Canary coverage shrinks because the canaries *were* the shells. Rejected-with-regret scenario: the audit concludes the shells are the value, and this approach deletes the product to save the library.

---

## Step 4 — Diversity verification

- **Distinct regions, not variations:** B1 = state-model-centric minimalism; B2 = schema/document-centric contract; B3 = migration/compat-centric layering; B4 = build-time codegen (Nix→Rust direction); B5 = runtime data-driven framework pole (opposite of the task's library framing); B6 = crate-granularity dissolution pole. Axes crossed: where the contract lives (core types / schema document / Nix source / runtime registry), when parity is proven (round-trip check / compilation / definitional / per-leaf checks), how much IoC remains (full shell / headless option / none), and migration posture (break / facade / dissolve).
- **Conventional vs unconventional covered:** B1-B3 are the high-probability conventional cluster; B4-B6 sample genuinely low-likelihood tails (codegen pipeline, engine/registry, full decomposition).
- **Probability spread:** 0.85 / 0.82 / 0.80 vs 0.08 / 0.05 / 0.04 — heads vs tails as instructed.
