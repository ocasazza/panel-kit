# ToT Phase 1 — Explorer C: `libraryness` solution-space exploration

Repo ground truth verified 2026-09-14 against `/Users/casazza/Repositories/ocasazza/panel-kit` (core `crates/panel-kit-core/src/lib.rs`, Dioxus shell `src/lib.rs`, TUI shell `crates/panel-kit-tui/src/lib.rs`, `nix/lib/mkLayout.nix`, `flake.nix`, `AGENTS.md`).

## Step 1 — Problem decomposition

**Core problem.** panel-kit has a genuinely renderer-neutral core (`panel-kit-core`: pure data + free functions like `begin_drag`/`apply_drag`/`merge_defaults` over `Vec<PanelWin<K>>`), but the two backend crates are frameworks, not libraries: they own the state container, the render tree, and the persistence policy, and the app injects a body callback (`Workspace::render(body)` in `src/lib.rs`, `TuiWorkspace::render(f, area, body)` in `crates/panel-kit-tui/src/lib.rs:251`). Meanwhile the "pure Nix specification" story is a fragment: `nix/lib/mkLayout.nix` emits only `SavedLayout<K>` JSON (geometry/state/tiling) and its parity is checked by a jq shape assertion (`checks.layout-canary-schema`, `flake.nix:118-138`) — nothing proves a Rust deserializer accepts it, and nothing covers theme, chrome metrics, badges, widgets/content, persistence policy, breakpoint, or charset.

**Subproblems any solution must address:**

1. **State ownership / reducer shape** — core has no workspace-level state type; each backend re-owns `panels/mode/drag/tile_drag/ws_scroll` (`Workspace<K>` signals in `src/lib.rs:250-277`; `TuiWorkspace<K>` fields in `crates/panel-kit-tui/src/lib.rs:156-180`) and re-implements load/merge/save.
2. **Persistence abstraction** — hardcoded per backend: gloo-storage localStorage behind `save_layout`/`load_layout` (private fns, `src/lib.rs:212-240`) with `storage_key: &'static str`, vs `Option<PathBuf>` JSON in `TuiWorkspace::new` (`crates/panel-kit-tui/src/lib.rs:186`). `defaults: fn()` pointer cannot capture config.
3. **Render composition without IoC** — the shells own the DOM/cell tree; the app's content is a guest closure. Chrome pieces (dock, traffic lights, tooltip) are not individually callable.
4. **PanelKind openness** — `trait PanelKind: Copy + Eq + Hash + Serialize + DeserializeOwned + 'static` with `fn title(self) -> &'static str` (`crates/panel-kit-core/src/lib.rs:35-41`) forces a closed compile-time enum; a Nix-authored spec has only strings.
5. **Theme/chrome in core** — `Theme` exists only on TUI (`crates/panel-kit-tui/src/theme.rs`, `Theme::DARK`/`PAPER`); web theming is `:root` CSS variables (`src/lib.rs:64-71`, `--inv-bg`/`--mono` have no TUI counterpart; `Theme::badge_info` has no CSS counterpart listed).
6. **Widget parity** — TUI-only modules `charts`, `meter`, `scroll`, `status`, `table` (`crates/panel-kit-tui/src/lib.rs:36-42`); web has only `badge` + `Spinner`. Badge is modeled in core (`panel_kit_core::badge::{BadgeKind, BadgeAction, tag_hue}`) but rendered with different shapes per backend.
7. **Nix spec schema & feature-completeness** — extend beyond `SavedLayout` to: panel registry (kinds/titles/slugs), mode, chrome metrics, theme, badges, widget/content bindings, persistence policy, mobile breakpoint (hardcoded 760 px in `viewport_is_mobile`, `src/lib.rs:142-153`), charset (`Charset::Unicode/Ascii`, `crates/panel-kit-tui/src/lib.rs:83-90`).
8. **Parity proof mechanism** — replace the jq shape check with a `nix flake check` job that actually deserializes Nix-emitted JSON into the Rust spec type (and ideally renders it on every backend).
9. **Consumer migration** — jump-cannon and apple-notes-ocr-flow depend on the git `panel-kit` crate; the README promises "Public API unchanged". Every breaking change must be named.

**Success criteria.** (a) A user can take any piece (geometry, reducer, persistence, theme, one widget) without the rest; (b) backends are thin per AGENTS.md; (c) a Nix spec and a Rust example are interchangeable with parity proven by CI; (d) each supported backend + the Nix path has a buildable canary example; (e) migration for the two consumers is explicit and mechanical.

## Step 2 — Solution-space map

- **State architecture:** free functions only (status quo core) · plain-data snapshot + event reducer · state object with methods in core · configured backend handle (status quo shells) · no shared state at all (widget kit).
- **Render composition:** shell-owned tree with body closure (status quo) · core `layout()/project()` returning rects/regions, backends expose granular chrome widgets · shell demoted to optional sugar · spec-driven rendering with no hand-written Rust.
- **PanelKind openness:** closed generic enum (status quo) · blanket `PanelKind` impl for a string newtype (`PanelId`) · metadata-as-data registry (`PanelMeta { title, slug, … }`) replacing the trait method.
- **Nix spec strategy:** hand-written Nix attrset mirroring Rust types (status quo, grown) · Rust-types-first with `schemars` JSON Schema committed and Nix validated against it · NixOS module system typed options · Nix as source of truth with Rust consuming Nix output at build time (include/`build.rs`).
- **Parity proof:** jq shape check (status quo) · native crane-built Rust check binary deserializing spec JSON · golden round-trip byte comparison · full (spec × backend) example-build matrix.
- **Trade-off axes:** breaking the two consumers vs. cleanliness; abstraction weight vs. composability; codegen/build-time coupling vs. runtime data; closed-enum type safety vs. data-driven openness; minimalism (delete/unify) vs. completeness.

---

## Approach A — Core-Owned Reducer with Injectable Store (schema-first Nix spec)

**Summary.** Move the workspace state container and a persistence trait into `panel-kit-core`; backends shrink to adapters that hold core state and translate platform events/rendering; the Nix DSL grows into a full `mkWorkspaceSpec` validated against a schemars-exported JSON Schema, with parity proven by a native Rust deserialization check.

**Description.**
Today `crates/panel-kit-core/src/lib.rs` ships pure data (`PanelWin<K>`, `Mode`, `Drag`, `Clamp`, `TileMetrics`, `SavedLayout<K>`) and free functions, but each backend re-implements the same orchestration: `use_workspace` (`src/lib.rs:298-397`) restores/merges/saves via private `load_layout`/`save_layout`, installs a ResizeObserver, scales geometry on resize; `TuiWorkspace::new` (`crates/panel-kit-tui/src/lib.rs:186-218`) duplicates the restore + `merge_defaults` + mode-recovery logic against a JSON file. This approach introduces `WorkspaceState<K>` in core — a plain struct `{ panels, mode, drag, tile_drag, ws_scroll }` plus an event-shaped method surface that delegates to the existing free functions (`begin_drag`, `apply_drag`, `reorder_tile`, `restore`, `merge_defaults`) — so cross-backend behavior (resize re-projection policy, settle-then-save policy, mode persistence) is encoded once, per the AGENTS.md rule "if behavior should match, encode it in core first". Persistence becomes `trait LayoutStore { fn load(&self) -> Option<SavedLayout<...>>; fn save(&self, &SavedLayout) }` with an in-memory impl in core (tests, Nix path), a gloo-storage impl in the Dioxus crate, and a JSON-file impl in the TUI crate. `use_workspace` survives as sugar: a hook that wires `WorkspaceState` into signals plus a `LayoutStore` — but the state and store are public, so an app can own the loop.

On the Nix side, `nix/lib/mkLayout.nix` grows into `mkWorkspaceSpec` covering everything the examples express: panel registry (kind strings + titles, feeding `kind_slug`), layout (`SavedLayout` as today), chrome metrics (`ChromeMetrics` values), theme token set (unifying `assets/panel-kit.css` `:root` vars and `crates/panel-kit-tui/src/theme.rs` fields into one core `Theme` type — the unification is the deletion lever: today's two parallel 14-field palettes collapse into one), content bindings (widget type + data per panel), persistence policy (enabled/key/format), breakpoint, charset. Direction of truth is Rust-first: a small `schema` feature derives `schemars::JsonSchema` on the spec types and a `packages.spec-schema` writes `spec.schema.json`; the Nix lib validates specs against invariants mirrored from it, and a new crane-built native check binary `spec-canary-check` reads `packages.spec-canary` JSON and `serde_json::from_str::<WorkspaceSpec<TestPanel>>` it — replacing the jq-only `layout-canary-schema` (`flake.nix:118-138`) with actual deserialization, i.e. real parity proof.

Canaries: each backend gains an example that boots from the Nix-emitted JSON (`spec_workspace` on wasm32 for web, native for TUI), so `nix flake check` proves both directions (Nix→Rust parse, Rust→render) without hand-mirroring `nix/examples/workspace-canary.nix` against `crates/panel-kit-tui/examples/workspace_canary.rs::defaults()`.

**Key design decisions & rationale.**
- State object in core, not messages: the free-function math already exists and is well-tested; bundling it is additive-unifying, not a rewrite. Methods keep call sites short (`state.begin_drag(idx, …)`).
- `LayoutStore` trait instead of a broader "platform" trait: persistence is the only genuinely hardcoded policy today (`&'static str` key + `fn()` pointer in `use_workspace`; `Option<PathBuf>` in `TuiWorkspace::new`); a narrow trait deletes two hardcodings without inventing a framework.
- `defaults` becomes `impl Fn() -> Vec<PanelWin<K>> + 'static` (closure-capturing) behind the config struct — named breaking change for jump-cannon/apple-notes-ocr-flow, mechanical to fix.
- Rust-first schema (schemars export consumed by Nix, not the reverse): keeps `panel-kit` usable by non-Nix consumers and makes the schema the CI-enforced contract.
- Theme unified in core as data; Dioxus backend generates its `:root { --bg: … }` block from it, TUI keeps `Theme` as a type alias/adaptation to `ratatui::style::Color`.

**Trade-offs.**
- Gain: single source of cross-backend behavior; real parity check; persistence/breakpoint/theme replaceable; `nix/lib/mkLayout.nix`'s scope gap closed.
- Sacrifice: core grows (state, theme, spec types, schemars dep behind a feature); `use_workspace` signature breaks (two consumers must patch call sites); the shell still owns the render tree by default (IoC only partially addressed — the body closure remains, though layout computation via `effective_rect`/`workspace_chrome`/`visible_panels` is now core API users can call directly).

**Probability.** 0.85 — the natural convergent design: it follows every stated repo rule, is incremental, and keeps both shells recognizable.

**Complexity.** Medium.

**Potential risks & failure modes.**
- `WorkspaceState<K>` methods can quietly re-grow into the god-object the task warns about; discipline needed to keep it data + thin methods over the free functions.
- schemars derive on generic `WorkspaceSpec<K>` plus the Nix mirror can drift in *error messages*, not shape — spec typos fail far from their source unless the Nix lib validates eagerly at eval.
- `SavedLayout` round-trip through a string-keyed spec needs a `PanelKind`-for-strings story even in the conservative version (see `PanelId` in B); deferring it leaves "pure Nix specs" unable to name panels the way enums do.
- ResizeObserver/viewport logic is DOM-specific; if pulled into core naively it drags web_sys deps into a wasm-only crate's contract — must stay as an adapter concern.

---

## Approach B — Headless Event Reducer + Frame Projection; Backends as Widget Kits

**Summary.** Delete the shell-as-controller entirely: core gains an Elm-style `enum Event` + pure `reduce(&mut Snapshot, Event, ctx)` and a pure `project(snapshot, viewport) -> Frame` layout output; each backend crate becomes a library of individually-callable render functions over `Frame`, with an optional thin "recipes" module that shows how to assemble the old behavior.

**Description.**
The framework smell is precisely that the app hands control to `Workspace::render(body)` (`src/lib.rs`) and `TuiWorkspace::render(&mut self, f, area, body)` (`crates/panel-kit-tui/src/lib.rs:251`) and gets called back. This approach inverts it. Core defines `Snapshot<K>` (plain data: `panels, mode, drag, tile_drag, ws_scroll`, `Copy` where possible) and `enum Event { PointerDown{..}, PointerMove{..}, PointerUp, Wheel{dy}, SetMode(Mode), Restore(K), Viewport{w,h} }` — a renderer-neutral widening of the existing `PointerEvent`/`PointerEventKind` (`crates/panel-kit-core/src/lib.rs:76-116`). `reduce` is a pure dispatch over the existing free functions: `PointerDown` → `begin_drag`/`begin_tile_resize`, `PointerMove` → `apply_drag`/`reorder_tile`, `Viewport` → re-projection policy (the ratio-scaling currently buried in `use_workspace`'s `recompute` closure, `src/lib.rs:331-348`, becomes testable core logic). `project(snapshot, viewport, &Clamp, &TileMetrics) -> Frame` returns chrome regions (via `workspace_chrome`), per-panel on-screen rects (via `effective_rect` + `visible_panels`), the dock list, and z-order — everything a renderer needs, with no renderer types in it.

The backends then publish a *widget kit* instead of a controller: `panel_kit::widgets::{panel_chrome, dock, traffic_lights, tooltip, badge, spinner}` each taking plain inputs (`Frame` rows, `Theme`) and returning `Element`; `panel_kit_tui::widgets::{…}` drawing the same into `Frame` cells. The wheel-chaining policy currently DOM-coupled (`panel_body_absorbs_wheel`, `src/lib.rs:169-210`) becomes an explicit `Event::Wheel { body_scrolls: bool }` decided by the host. Persistence is reduced to two free functions plus `SavedLayout`/`merge_defaults` as today — the *host* decides when to call save (settle policy exposed as `snapshot.dragging()`, mirroring `TuiWorkspace::dragging`). A `shell` module in each backend keeps a one-call `use_workspace`/`TuiWorkspace` convenience that assembles reducer + kit + store, demoted from "the API" to "one composition".

For Nix completeness and openness, core adds `pub struct PanelId(pub Arc<str>)` with a `PanelKind` impl (title from a side-table `PanelMeta`), so a Nix-authored spec maps onto `Snapshot<PanelId>` without a compile-time enum; hand-written apps keep their own `K`. The full `WorkspaceSpec` (as in A: theme, chrome, content, persistence, breakpoint, charset) is authored in Nix via `mkSpec`, serialized to JSON, and the parity check is a native Rust binary that `reduce`/`project`-drives the spec into a `Frame` and asserts golden values — proving *behavioral* parity, not just shape.

**Key design decisions & rationale.**
- Events, not methods: the reducer surface is data (`Event`), so backends translate platform input once at the boundary (explicitly demanded by AGENTS.md: "platform events should be translated at backend boundaries into core input types") and the whole interaction grammar becomes property-testable in core without a DOM or terminal.
- `project()` returns renderer-neutral geometry; both backends already consume exactly these primitives (`effective_rect`, `workspace_chrome`, `visible_panels`) internally — this just publishes them as the composition boundary.
- `PanelId(Arc<str>)` closes the Nix↔Rust impedance gap at the type level while `PanelKind` stays generic; no registry machinery.
- Shell demoted, not deleted-then-re-added: consumers get a migration target that looks familiar, while new users compose freely.

**Trade-offs.**
- Gain: maximal library-ness (user owns the loop; every chrome piece callable; persistence policy host-owned), the strongest test surface (pure `reduce`/`project` over `Event`), and Nix specs become first-class citizens via `PanelId`.
- Sacrifice: biggest behavioral rewrite of the two shells (`src/lib.rs:399-829` `impl Workspace` and `crates/panel-kit-tui/src/lib.rs:182-677` `impl TuiWorkspace` largely re-plumbed); both consumers break beyond a signature change (their event handlers and render calls move); `Signal` ergonomics must be re-won by the host or the convenience shell.

**Probability.** 0.80 — the canonical "headless core + widget kit" answer to a library-ness critique; slightly less likely than A because it breaks more than the README's "Public API unchanged" posture allows.

**Complexity.** Medium.

**Potential risks & failure modes.**
- Dioxus reactivity: a pure `Snapshot` outside `Signal`s can degrade fine-grained updates; the convenience shell must wrap signals carefully or web perf regresses.
- Event enum sprawl: every new interaction (e.g. tile hover-shuffle) adds variants; without discipline `Event` becomes the new god-type.
- Golden-frame parity tests are sensitive to float formatting and theme defaults; must pin `Clamp::WEB`/`TileMetrics::WEB` and TUI cell metrics (`ROW_CELLS`) explicitly.
- Two identity systems (`K` enums vs `PanelId`) double the generic test matrix.

---

## Approach C — Config-Object Facelift: `WorkspaceConfig` + Runtime Spec Parsing, Minimal Breakage

**Summary.** Keep both shells and their render model exactly as they are; replace the hardcoded parameters with one builder-style `WorkspaceConfig<K>` (storage, defaults closure, breakpoint, clamp, theme, charset), and make "pure Nix spec" a runtime-input feature: the shells can boot from a spec JSON emitted by an extended `mkLayout`.

**Description.**
The smallest change that addresses every named pain point without moving control flow. `use_workspace(storage_key: &'static str, defaults: fn() -> Vec<PanelWin<K>>)` (`src/lib.rs:298-301`) and `TuiWorkspace::new(store: Option<PathBuf>, defaults: fn())` (`crates/panel-kit-tui/src/lib.rs:186`) collapse into `WorkspaceConfig<K>::new(defaults)` with `.storage(LayoutStore)`, `.breakpoint(px)`, `.clamp(&Clamp)`, `.theme(Theme)`, `.charset(Charset)`. The mobile breakpoint stops being a hidden constant in `viewport_is_mobile` (`src/lib.rs:142-153`, 760 px) and becomes config read by both backends; TUI gains the breakpoint concept for narrow terminals. Persistence hardcodings (gloo-storage in `save_layout`/`load_layout`, `src/lib.rs:212-240`; `fs::write` in `TuiWorkspace::save`, `crates/panel-kit-tui/src/lib.rs:230-240) become the default `LayoutStore` selections, overridable. `defaults` accepting a closure (capturing e.g. a spec-derived layout) is the single breaking change to the two consumers, who today call `use_workspace("key", default_layout)` per the doc example (`src/lib.rs:40-60`).

The Nix side extends `nix/lib/mkLayout.nix` into `mkSpec { panels?; registry; mode; chrome; theme; content; persistence; breakpoint; charset }` — a superset emitting today's `SavedLayout` JSON at `spec.layout.json` plus a `spec.json` document. A new `panel-kit-spec` module (core feature) defines `WorkspaceSpec<PanelId>` with `PanelId(Arc<str>)`; `WorkspaceConfig::from_spec(&str)` parses Nix output at runtime, so "a Nix spec and a Rust example are interchangeable" is literal: `packages.spec-canary` JSON (`flake.nix:82-83` pattern) feeds a wasm web example and a native TUI example, both wired into `checks`. Parity proof stays lightweight but real: the existing jq check is upgraded to a small native `spec-decode` check binary that deserializes every spec the flake exports; the hand-mirrored `nix/examples/workspace-canary.nix` (which duplicates `defaults()` from `crates/panel-kit-tui/examples/workspace_canary.rs`) is deleted in favor of one Nix file consumed by both sides.

Theme unification rides along minimally: a core `Theme` token struct mirrors the CSS variable list (`src/lib.rs:64-71`); web emits `:root` overrides from it, TUI maps it onto `Theme::DARK`-style presets (`crates/panel-kit-tui/src/theme.rs`). Widget parity is explicitly out of scope beyond badge/theme (deferred, documented as non-goal) — this approach buys spec completeness for the *workspace* surface, not the TUI-only widget zoo (`charts`/`meter`/`table` stay TUI-only, `crates/panel-kit-tui/src/lib.rs:36-42`).

**Key design decisions & rationale.**
- Config object over trait injection: kills the `&'static str`/`fn()` pair and both persistence hardcodings with one boring, familiar pattern; no new architecture to learn.
- Runtime spec parsing instead of schema tooling: avoids schemars/codegen machinery entirely; the contract is "Nix emits JSON that `panel-kit-spec` deserializes", enforced by one check binary — proportionate to the repo's size.
- `PanelId` only on the spec path: consumers keep their enums; the spec feature is additive.
- Delete `nix/examples/workspace-canary.nix` duplication: the minimalism win — one description, two consumers, no hand-mirroring.

**Trade-offs.**
- Gain: near-zero risk, fast migration (consumers change one call), breakpoint/persistence/theme become replaceable, Nix path becomes feature-complete for workspace semantics with a real decode check.
- Sacrifice: the IoC critique is *not* answered — `Workspace::render(body)` and `TuiWorkspace::render` keep owning the tree; state ownership stays duplicated per backend (load/merge/save orchestration still written twice, just configured differently); widget asymmetry persists; spec cannot express TUI-only content, so "totally feature complete alongside the Rust examples" is only partially honored (the web examples' surface is covered; the TUI canary's charts/table content is not).

**Probability.** 0.82 — the path of least resistance that a maintainer protecting two git consumers and a "Public API unchanged" README promise would most plausibly pick first.

**Complexity.** Low.

**Potential risks & failure modes.**
- Solves configuration, not library-ness: the task's central ask (composition over the `Workspace` handle) remains open; risk of the design being judged as ducking the question.
- Runtime parsing means spec errors surface at app boot, not build time; the decode check mitigates only for specs the flake knows about.
- Two theme representations still exist transitively (core tokens + CSS file + `Theme` presets) unless the CSS blob is regenerated — half-unification can add a third source of truth.
- `WorkspaceConfig` can accrete options until it is a settings dump; needs a discipline rule (every option must name the hardcoding it deletes).

---

## Approach D — Nix as the Source of Truth: Spec-Compiled Examples

**Summary.** Invert the direction of truth: every example workspace — including the Rust ones — is *authored only in Nix*; the flake evaluates `nix/specs/*.nix` into JSON that Rust examples embed at build time (`include_str!` via crane `src`/env), so "parity between Nix spec and Rust example" becomes a category error: there is only the spec, and backends are viewers.

**Description.**
Today `nix/examples/workspace-canary.nix` mirrors `crates/panel-kit-tui/examples/workspace_canary.rs::defaults()` by hand, and nothing verifies they agree (`flake.nix:74-83` comment admits the mirror; the jq check only validates shape). This approach deletes the mirroring by deleting the duplication: `defaults()` in every canary is replaced by `include_str!` of a spec JSON produced by the flake (vendored into the repo for non-Nix builds, regenerated and diffed by a check so the vendor copy can't rot). The full `WorkspaceSpec` in Nix (authored with NixOS-module-system typed options — free validation, defaults, `mkIf` merging) covers layout, registry (kinds + titles feeding `kind_slug` slugs), chrome (`ChromeMetrics`), theme (one token set), content model (which widget — `chart.time_series`, `table`, `meter`, `badge` — renders in which panel, plus literal demo data as plain Nix lists), persistence policy, breakpoint, charset. A new `panel-kit-spec` crate is the sole Rust definition of the spec (`JsonSchema`-derived, schema exported as a package for editor tooling); `Snapshot::from_spec(&WorkspaceSpec<PanelId>)` seeds state, `merge_defaults` gains a spec-driven form, and each backend canary renders the *same* embedded spec — the parity matrix in `checks` (web example on wasm32, TUI example native, `browser-tui-example` style jobs at `flake.nix:143-146`) is what proves feature-completeness: if the spec can't express something a canary shows, the canary can't be written, and the gap is structurally visible.

The library-ness moves ride the same rail: since the spec fully describes the workspace, the shells reduce to `SpecViewer` implementations and nothing prevents a host from consuming `panel-kit-spec` + core directly. `PanelId(Arc<str>)` replaces generic `K` *in the spec path*; hand-written enum apps remain supported but are now the secondary story. jump-cannon/apple-notes-ocr-flow migrate by authoring a Nix spec (or keeping Rust defaults behind `WorkspaceConfig::with_defaults`, the escape hatch).

**Key design decisions & rationale.**
- Single-description principle: parity-by-construction instead of parity-by-check; the hand-mirror (`nix/examples/workspace-canary.nix` vs `defaults()`) is the deleted weight.
- NixOS module system for authoring: typed options give validation/merging/defaults for free and make the DSL self-documenting — no bespoke `normalizePanel` assertions (`nix/lib/mkLayout.nix:48-81`) to maintain.
- Build-time embedding with a vendored fallback keeps the crate buildable outside Nix (plain `cargo build`), at the cost of a sync check.
- Spec crate as the only schema definition: schemars export replaces the hand-maintained comment block in `mkLayout.nix:1-16` as the human contract.

**Trade-offs.**
- Gain: parity is no longer a test to maintain but an architectural invariant; the Nix DSL is feature-complete by construction (anything less breaks the canaries); maximal deletion of duplicated description.
- Sacrifice: Rust-on-Nix build coupling at the example layer (crane `src` filesets must include vendored specs); a second authoring language for app developers; the runtime-mutable app (user drags panels, state diverges from spec) must clearly treat the spec as *defaults*, not state — a subtle mental model; weakest fit for jump-cannon/apple-notes-ocr-flow, whose layouts are code-defined today.

**Probability.** 0.06 — requires embracing Nix as an authoring language for Rust examples, which most Rust library maintainers would resist; only natural if the operator's environment is Nix-first (which this repo's Hydra farm suggests, but the git consumers do not).

**Complexity.** High.

**Potential risks & failure modes.**
- Vendored-JSON drift if the sync check is skipped locally; CI-only failure discovery.
- Nix evaluation becomes a build prerequisite for content changes → slow example iteration (`dx serve` hot path unaffected only if the vendored file is the source during dev).
- Spec literalism: demo data living in Nix attrsets can lag behind richer programmatic content (the `Metrics`/`noise()` animation machinery in `workspace_canary.rs:112-257` doesn't fit literal data); risks a `dynamic` escape hatch that quietly reintroduces Rust-authored content and erodes the single-description claim.
- NixOS module system inside a flake `lib` export is unusual for downstream consumers; API ergonomics (`lib.${system}.mkSpec` types) need care to avoid leaking module-system internals.

---

## Approach E — Shell Deletion: Capability Kit and Metadata-as-Data

**Summary.** The most deletion-biased answer: remove the `Workspace`/`TuiWorkspace` controller-handles entirely, demote both crates to pure widget/adapter libraries over core, convert `PanelKind` metadata from a trait method to plain data (`PanelMeta`), and author the full Nix spec with NixOS-module typed options whose JSON is decoded by a tiny spec module — composition all the way down, no assembly you cannot bypass.

**Description.**
The task's bias instruction — "what could be DELETED or unified rather than added" — taken literally. Delete: the `Workspace<K>` signal bundle and its 430-line `impl` (`src/lib.rs:250-829`), `TuiWorkspace<K>` and its 500-line `impl` (`crates/panel-kit-tui/src/lib.rs:156-677`), the private duplicated load/save pair (`src/lib.rs:212-240` / TUI `save`), and the `PanelKind::title` method (metadata becomes `PanelMeta { id, title }` data; `kind_slug` already shows titles are just strings for slugs/CSS classes, `crates/panel-kit-core/src/lib.rs:608-620`). What remains in core: the existing free functions, plus `Snapshot<K>` + `Event`/`reduce` (as in B) and pure `project()`. Each backend crate exports only leaf capabilities with flat signatures — `chrome::panel(rect, title, state)`, `chrome::dock(minimized)`, `badge::Badge` (already core-modeled via `panel_kit_core::badge::{BadgeKind, BadgeAction, tag_hue}`), `Spinner`, and on TUI the existing `charts`/`meter`/`scroll`/`status`/`table` modules (`crates/panel-kit-tui/src/lib.rs:36-42`) — none holding state, none owning trees. Theme collapses to one core token set consumed by both (deleting the parallel `Theme` struct vs CSS-var list duplication, `crates/panel-kit-tui/src/theme.rs:9-37` vs `src/lib.rs:64-71`). Persistence is *example code*: a `recipes` module shows a 20-line localStorage/JSON-file integration using `SavedLayout`/`merge_defaults`; the library ships zero storage policy. The old `use_workspace` ergonomics live on as a documented recipe + optional `panel-kit-shell` facade crate that consumers can adopt — clearly marked as one composition, not the product.

Nix completeness: `mkSpec` written with NixOS module types covering registry (ids + `PanelMeta`), layout, chrome metrics, theme tokens, content bindings (including the TUI widget kinds), persistence policy (describes what the *recipe* would configure), breakpoint, charset. Parity: `nix flake check` runs a native decode+golden-project check (spec JSON → `Snapshot<PanelId>` → `project()` → compare against golden `Frame`), and every backend builds one canary assembled entirely from kit pieces fed by the spec — the canary *is* the proof that the kit composes.

**Key design decisions & rationale.**
- Metadata-as-data over trait-method: kills the closed-enum constraint at its root (a spec-authored panel has a `PanelMeta`, not a compile-time variant) while letting enum apps keep `K` with a const metadata table.
- Zero persistence in-library: the hardcodings are deleted, not abstracted — no `LayoutStore` trait to design around; policy lives where it belongs (the app).
- Kit pieces over shell: directly answers "composition over inheritance / no god-object handle"; every piece is independently consumable, satisfying the take-just-geometry criterion.
- NixOS module types: the spec's validation/defaults/merging come from `lib.types`, not from bespoke asserts (`normalizePanel`, `nix/lib/mkLayout.nix:50-81`).

**Trade-offs.**
- Gain: the purest library; smallest long-term maintenance surface; single theme/widget model; the Nix spec describes everything because "everything" is now data + flat renderers.
- Sacrifice: both consumers break hard (README's "Public API unchanged" is abandoned; semver 0.3 + migration doc required); apps that *want* batteries-included shell behavior must assemble ~50 lines of glue or take the facade; Dioxus signal wiring becomes the app's problem (the ergonomics that made `use_workspace` pleasant were real work).

**Probability.** 0.05 — maximally aligned with the task's minimalism rhetoric but misaligned with the repo's stated consumer promises and the "batteries" value proposition of a panel *kit*; a reviewer would likely push back to B.

**Complexity.** Medium (deletion is still a rewrite of both shells' interiors, plus facade).

**Potential risks & failure modes.**
- Ergonomic regression: assembling chrome/dock/drag by hand in every app re-creates the boilerplate the shell existed to remove; the facade crate becomes load-bearing and the deletion is cosmetic.
- Dioxus without a central `Signal` owner risks missed-re render bugs in user code — support burden moves to issue tracker.
- Golden-`Frame` checks for TUI depend on `Charset`/`ROW_CELLS` invariants; churn on every chrome tweak (high-maintenance golden files) unless snapshots are coarse.
- Losing `TuiWorkspace::zones` hit-testing (private, `crates/panel-kit-tui/src/lib.rs:136-148`) means kit consumers must rebuild pointer routing — the recipe must ship it or the kit is unusable.

---

## Approach F — Spec Compiler with Capability-Assembled Shells and a Parity Matrix

**Summary.** Treat the whole problem as building a small compiler: Nix spec (surface language, NixOS modules) → versioned IR (`WorkspaceSpec` JSON, schemars-schema'd, with `spec_version`) → multiple backends as "code generators"/renderers, with the shells re-implemented as *demonstrated compositions* of published capabilities (`DragController`, `PersistencePolicy`, `Breakpoint`, `ThemeTokens`, `ContentModel`) and CI proving coverage via an explicit (spec × backend) parity matrix.

**Description.**
The maximal-generalization design. Layer 1 — surface: `mkSpec` in NixOS module form (all dimensions: registry/`PanelMeta`, layout, `ChromeMetrics`, theme tokens, per-panel content bindings including TUI widgets, persistence policy, breakpoint, charset). Layer 2 — IR: `panel-kit-spec` crate defines `WorkspaceSpec<PanelId>` (`PanelId(Arc<str>)`), versioned (`spec_version: u32`) with a migration rule per version so old persisted specs keep loading (generalizing what `merge_defaults` does for layouts, `crates/panel-kit-core/src/lib.rs:597-606`). Layer 3 — backends: each consumes the same IR; core's `Kernel<K>` (snapshot + event reduce + project, as in B) is the shared semantics; the Dioxus and ratatui shells are *re-built out of capabilities*: `DragController`, `PersistencePolicy` (store trait), `Breakpoint(px)`, `ThemeTokens`, `ContentModel<K>` — each a small standalone value the host can replace or omit, and the shipped shell is literally `Workspace::compose(capabilities…)`, existing as documentation-by-code of one legal assembly. Widget parity is modeled IR-side: core gains view-model types (`chart::Series`, `MeterModel`, `TableColumn`, extending the existing pattern of `panel_kit_core::badge` where semantics already live core-side and only drawing is backend-side), which both backends render — closing the TUI-only gap (`charts`/`meter`/`table`).

The distinctive piece is the parity matrix as a first-class flake artifact: `checks` enumerates `nix/specs/*.nix × {web-wasm, tui-native, browser-tui}` build/render jobs (extending `checks.browser-tui-example`, `flake.nix:143-146`, and the `hydraJobs` aggregation at `flake.nix:170+`), plus a decode check per spec (`spec-version pinning` so a schema change fails every cell). `lib.${system}` (currently `{ mkLayout, winStates, tileWMax, tileHMax }`, `flake.nix:111`) becomes `{ mkSpec, specTypes, specSchema }` for downstream flakes. Consumers migrate onto capabilities: jump-cannon keeps `use_workspace`-shaped composition with default capabilities (compat wrapper generated from the same capability assembly), so the README promise survives as a compatibility profile.

**Key design decisions & rationale.**
- IR with versioning: makes "Nix spec and Rust example interchangeable" an engineered contract with an evolution story, not a snapshot-in-time match; mirrors how `SavedLayout` already survives panel-set changes via `merge_defaults`.
- Shells as demonstrated compositions: the god-object critique is answered structurally — the shell is one entry in the space, reproducible by users, not a privileged path.
- Widget view-models in core: applies the AGENTS.md core-first rule to the biggest current asymmetry (TUI-only widget modules), making cross-backend parity achievable where it's today impossible.
- Matrix CI: converts "feature-complete" from a design claim into a falsifiable build artifact — a spec feature no backend renders is an empty matrix cell that fails.

**Trade-offs.**
- Gain: every constraint of the task is met and *verifiable in CI*; cleanest long-term evolution story; consumers keep a compat profile; downstream flakes get a real typed DSL.
- Sacrifice: the heaviest design on the table — spec crate + IR versioning + capability lattice + view-model layer + matrix CI is a lot of machinery for a 3-crate library; directly tensions with the task's "prefer deleting/simplifying over adding abstraction; every new abstraction must be justified by a concrete composition it enables".

**Probability.** 0.08 — the "complete" answer a spec-driven pipeline would happily implement, but the abstraction budget (IR versioning, capability lattice) exceeds what the evidence (two backends, two consumers, one canary layout) justifies; likely pruned back to A or B, keeping F's parity-matrix check as a fragment.

**Complexity.** High.

**Potential risks & failure modes.**
- Over-engineering: capability lattice + IR versioning could outmass the library it serves; reviewers may judge it as rebuilding a framework with more steps.
- Matrix CI cost: N spec files × 3 backends of crane jobs inflates Hydra evaluation/build time against the farm's stated capacity constraints (`flake.nix:19-30`).
- Widget view-models in core risk dragging backend concerns backward (chart data shapes are app-driven; `apple-notes`-flavored stage colors already leaked into canary constants, `workspace_canary.rs:146`); the core boundary must stay at geometry/semantics, not data curation.
- Spec versioning adds a migration tax forever; without real downstream spec users it is speculative generality.

---

## Step 4 — Diversity verification

- **Genuinely different?** A (state-in-core + trait store, shell retained as sugar, schema-first), B (event reducer + projection, backends as widget kits), C (config object, zero structural change, runtime spec parse) differ on state ownership, IoC, and breakage — not naming or packaging. D (direction-of-truth inversion: Nix authors, Rust embeds), E (deletion of shells; metadata-as-data; zero library persistence), F (compiler/IR + capability assembly + matrix CI) occupy unconventional regions.
- **Space coverage?** State architecture {object-with-methods, plain-data+events, backend-handle, none, assembled-capabilities} ✓; render composition {shell+sugar, kit, config-shell, spec-driven, matrix} ✓; PanelKind {closed enum, `PanelId` newtype, `PanelMeta` data} ✓; Nix strategy {hand-attrset-grown (A/C), schemars-validated (A/B/F), NixOS modules (D/E/F), source-of-truth embedding (D)} ✓; parity {jq (baseline), native decode, golden projection, matrix} ✓; consumer posture {mechanical break (A/B), compat profile (C/F), hard break (E), author-in-Nix (D)} ✓.
- **Conventional vs. unconventional covered?** A/B/C are the convergent designs (combined probability mass ≈ 0.85+ of typical expert proposals); D/E/F each sit below 0.10 while remaining implementable and arguable from real repo evidence.
