---
name: Composable Library Design
description: Prior art and mechanics for turning a controller-style Rust UI project into a composable library (headless core + backend adapters, reducer/projection, caller-owned scratch) with a strict, Rust-authoritative declarative spec authorable in pure Nix and mechanized parity checks
topics: rust-library-design, library-vs-framework, headless-core, reducers, projection, panel-workspace-systems, nix, json-schema, schemars, serde, crane, parity-checking
created: 2026-09-14
updated: 2026-09-14
scratchpad: .specs/scratchpad/1659969c.md
---

# Composable Library Design

## Overview

How to structure a Rust UI workspace project as a **library** rather than a
framework: the host application owns state, events, effects, and render order;
a renderer-neutral core provides pure transitions, geometry, and a semantic
projection; thin backend crates expose native painters; and a strict,
Rust-authoritative spec type can be authored declaratively in pure Nix and
checked mechanically for parity. Grounded in production prior art
(AccessKit, taffy, Zellij, dockview, tmux, nixcfg-rs, nixpkgs `pkgs.formats`).

## Key Concepts

- **Library vs framework**: a library is called by the host (caller owns
  control flow); a framework calls the host (inversion of control). Lean
  library: plain data + free functions, no god-object controller.
- **Headless core + adapters**: framework-neutral semantic model in one crate;
  per-platform adapter crates translate native input and paint native
  primitives (AccessKit pattern).
- **Reducer over controller**: `reduce(snapshot, event, ctx) -> report` — pure
  transition, effects reported not performed (Elm/redux/statig lineage).
- **Caller-owned scratch projection**: engine writes into host-owned reusable
  buffers and returns a frame borrowing them (taffy low-level
  `LayoutPartialTree` pattern); zero steady-state allocation.
- **Rust-authoritative spec**: strict serde types are the single source of
  truth; JSON Schema is *generated* (schemars), never hand-written; other
  languages (Nix) validate against the generated schema.
- **Stable string identity**: persisted/cross-boundary IDs are stable strings;
  intern once into cheap copyable indices for hot paths.
- **Authored config vs operator state**: two separate schemas — declarative
  authored workspace spec vs serialized mutable layout (tmux layout strings vs
  Zellij KDL; `WorkspaceSpec` vs `SavedLayoutV2`).

## Documentation & References

| Resource | Description | Link |
|----------|-------------|------|
| Rust API Guidelines | Official checklist for idiomatic, interop-friendly library APIs | https://rust-lang.github.io/api-guidelines/ |
| AccessKit README/ARCHITECTURE | Rust-canonical schema, generated JSON Schema, per-platform adapters, push model, stable IDs | https://github.com/AccessKit/accesskit |
| taffy docs (high vs low-level API) | Layout-as-library; `LayoutPartialTree` trait + custom caller-owned trees | https://docs.rs/taffy |
| taffy repo & consumers | Powers Servo, Bevy, Slint, Zed/GPUI, iocraft; `custom_tree_*` examples | https://github.com/DioxusLabs/taffy |
| Xilem ARCHITECTURE.md | Declarative Elm-style layer diffed onto independently usable retained layer | https://github.com/linebender/xilem/blob/main/xilem/ARCHITECTURE.md |
| Zellij layout docs | Declarative KDL workspace specs: panes, chrome toggles, templates, plugin references | https://zellij.dev/documentation/layouts.html |
| tmux layout strings | Serialized operator layout state (checksummed tree grammar, stable pane IDs) | https://github.com/tmux/tmux/wiki/Getting-Started |
| dockview docs | Web dock manager as a library: registered panels, toJSON/fromJSON, partial engines | https://dockview.dev/docs/overview/introduction/ |
| statig docs | Rust event-driven state machines: `Outcome`, context injection, zero-heap | https://docs.rs/statig |
| serde container attributes | `deny_unknown_fields` official caveats (incl. `flatten`) | https://serde.rs/container-attrs.html |
| schemars docs | `JsonSchema` derive honors serde attrs; feature flags; stability caveats | https://docs.rs/schemars |
| serde_path_to_error | Path-tracked deserialization errors (JSON-pointer-style attribution) | https://docs.rs/serde_path_to_error |
| NixOS RFC 42 / pkgs.formats | Declarative settings: eval-time checked attrsets + `generate` store files | https://github.com/NixOS/rfcs/blob/master/rfcs/0042-config-option.md |
| nixcfg-rs | schemars → JSON Schema → Nix options, with committed-schema drift check in `nix flake check` | https://github.com/mushrowan/nixcfg-rs |
| crane trunk examples | Canonical dual-lane flake: native `doCheck=true` + wasm `CARGO_BUILD_TARGET` lane | https://crane.dev/examples/trunk.html |

## Recommended Libraries & Tools

| Name | Purpose | Maturity | Notes |
|------|---------|----------|-------|
| schemars `1.x` | Generate JSON Schema from spec types | Stable (v1) | Pin major: emitted schema shape may change between versions (not considered breaking upstream); commit canonical schema |
| serde + `deny_unknown_fields` | Strict decode of authored specs | Stable | Never combine with `flatten`; prefer internally tagged enums |
| serde_path_to_error `0.1.x` | Pointer-style path on first decode error | Stable | Full all-errors accumulation still needs a validation pass over the typed value |
| statig (reference, not dependency) | Reducer/state-machine shape precedent; `no_std`, zero heap | Stable | Study its `Outcome`/context/introspection design |
| taffy (reference) | Caller-owned storage layout library precedent | Stable | Low-level trait API = model for scratch/borrowed-frame design |
| crane | Nix builds; dual lanes | Stable | Native lane without `CARGO_BUILD_TARGET`, `doCheck = true`; wasm lane `doCheck = false` |
| nixpkgs `pkgs.formats` / `lib.generators` | Pattern for eval-time-checked Nix → store-file generation | Stable | Model for a `mkSpec`-style producer function |

### Recommended Stack

Rust-authoritative strict types + optional feature-gated `schemars` export;
pure-Nix producer that normalizes, validates against the committed generated
schema, and emits a JSON store path; a native-only checker binary doing strict
decode + semantic compare; dual crane lanes. Do not adopt nixcfg-rs or
fromJsonSchema as dependencies — vendor the small pieces (pure-Nix schema
subset validator is ~120 lines in prior art).

## Patterns & Best Practices

### Headless core + backend adapters

**When to use**: multiple render targets (DOM + terminal), headless testing,
host-owned event priority.
**Trade-offs**: core must stay renderer-neutral (no DOM/ratatui types);
backends keep native semantics instead of a lowest-common-denominator IR.
**Example**:
```rust
// core: pure projection into caller-owned scratch
pub struct ProjectionBuffer<K> { panels: Vec<PanelProjection<K>>, /* capacity */ }
pub fn project_into<'a, K>(input: ProjectionInput<'_, K>,
    scratch: &'a mut ProjectionBuffer<K>) -> ProjectedFrame<'a, K>;
// frame borrows only scratch; never store it in signals/props
```

### Reducer with reported effects

**When to use**: host must own event ordering and first refusal.
**Trade-offs**: no callbacks/IoC in core; hosts translate native input at the
boundary; settle/continuous phases make persistence policy explicit.
**Example**:
```rust
pub fn reduce<K>(snap: &mut Snapshot<K>, ev: WorkspaceEvent<K>,
    ctx: ReduceContext<'_>) -> Reduction<K>; // { changed, phase, focus_request }
```

### Strict spec: tagged enums, no flatten

**When to use**: any authored/config schema decoded strictly.
**Trade-offs**: `deny_unknown_fields` + `flatten` is unsupported (officially);
untagged enums produce unusable errors; internally tagged enums give strict,
schema-friendly shapes.
**Example**:
```rust
#[derive(Serialize, Deserialize)]
#[serde(tag = "source", content = "value", rename_all = "snake_case",
        deny_unknown_fields)]
pub enum DataSource<T> { Inline { value: T }, Binding { id: String } }
```

### Rust → schema → pure-Nix validation pipeline

**When to use**: Nix authors config consumed by Rust; shape-only jq checks
have drifted before.
**Trade-offs**: schema regeneration is a build step; Nix validator must cover
the *emitted subset* (objects/required/`additionalProperties:false`, arrays,
tagged alternatives, enums, strings, numeric bounds) — precedented at ~120
lines of Nix.
**Flow**:
```text
Rust types --schemars--> committed canonical schema (checked, never hand-edited)
Nix attrset --validate+normalize--> JSON store path --strict serde decode--> typed spec
native checker: schema regen diff, strict decode, provider-set compare both
directions, per-leaf backend disposition (applied/observed/bound/approximated)
```

### Dual crane lanes + build-time config delivery

**When to use**: workspace ships wasm artifacts but needs host `cargo test`.
**Example**:
```nix
hostArgs = (builtins.removeAttrs commonArgs [ "CARGO_BUILD_TARGET" ]) // {
  src = hostSrc; doCheck = true;   # native lane: cargo test runs
};
wasmArgs  = commonArgs;            # CARGO_BUILD_TARGET=wasm32, doCheck=false
# Nix JSON reaches Rust only as a build-time input:
# PANEL_KIT_WORKSPACE_SPEC = "${specJson}"  ->  include_str!(env!("..."))
```

### Persistence as a trait, policy as data

`trait Store { load; save; clear }` in core; backends own only byte transport;
`SavePolicy` (manual/on-settle/on-change) decides writes from reduction
phases; exact write counts are testable with a fake store. Serialize stable
string IDs (never indices) so persistence survives reordering.

## Similar Implementations

### AccessKit
- **Source**: github.com/AccessKit/accesskit (read 2026-09-14)
- **Approach**: canonical schema in Rust; JSON Schema/proto bindings generated;
  per-platform adapter crates; only the adapter retains the full tree (works
  with immediate-mode hosts given stable IDs); separate consumer crate for
  platform-independent derived logic.
- **Applicability**: template for core/backends split and "generated from Rust"
  schema authority.

### taffy
- **Source**: github.com/DioxusLabs/taffy + docs.rs (read 2026-09-14)
- **Approach**: library dependency of Servo/Bevy/Slint/Zed/iocraft; high-level
  owned tree AND low-level traits where the caller owns storage, caching, and
  dispatch (`custom_tree_vec.rs`, `custom_tree_owned_partial.rs`).
- **Applicability**: proves caller-owned scratch + borrowed results at scale;
  two-tier API is the model for full-frame vs single-panel projectors.

### Zellij / tmux / dockview
- **Source**: official docs (search-verified 2026-09-14)
- **Approach**: Zellij: declarative KDL layouts, chrome toggles
  (`borderless`), templates with `children` injection, plugin-by-reference;
  tmux: checksummed serialized layout strings with stable pane IDs (operator
  state, not config); dockview: registered panel components, toJSON/fromJSON
  round-trip, partial engines (grid/split/paneview).
- **Applicability**: validates declarative workspace specs, the
  authored-config vs operator-state split, and dock-as-separate-part.

### nixcfg-rs
- **Source**: github.com/mushrowan/nixcfg-rs (read 2026-09-14)
- **Approach**: `rust struct → schemars(+extensions) → JSON Schema → Nix lib`;
  checked-in `schema.json` + `nix flake check` diff + `cargo x update-schema`
  regeneration.
- **Applicability**: the committed-schema drift-check workflow, independent
  reinvention; hobby-scale, so vendor the pattern not the crate.

## Common Pitfalls & Solutions

| Issue | Impact | Solution |
|-------|--------|----------|
| `#[serde(flatten)]` + `deny_unknown_fields` | High — unknown fields silently accepted | Never flatten in strict types; nest explicitly (serde official note) |
| Untagged enums in specs | Medium — "data did not match any variant" hides the real field error | Internally tagged (`tag = "kind"`); add `serde_path_to_error` for decode attribution |
| Unpinned schemars | Medium — emitted schema shape changes across versions flip parity checks | Pin major; canonicalize before compare; regen is an explicit named check stage |
| Single crane lane with wasm `doCheck=false` | High — no tests ever run in CI | Native lane without `CARGO_BUILD_TARGET`, `doCheck = true` (crane trunk pattern) |
| Hand-mirrored canary fixtures ("shape-only parity") | High — checks stay green while content drifts (panel-kit's 7-vs-9 lesson) | Single author; bidirectional exact provider-set compare; JSON-pointer diffs |
| Frame/scratch borrows stored in reactive state | Medium — borrow-checker fights, runtime panics | Render-scoped borrow: project, paint to owned values, drop guard before mutation; compile-fail doctest |
| Storing indices in persisted state | Medium — reordering corrupts saves | Persist stable string IDs; intern to copyable indices at resolve time |
| Runtime Nix dependency | High — consuming apps break outside Nix | Nix evaluates at build time only; deliver JSON as store path via `env!`+`include_str!` |
| Lowest-common-denominator paint IR | Medium — backends lose native semantics | Projection carries semantic geometry/state only; native painters stay per-backend |

## Recommendations

1. **Rust stays authoritative**: strict serde types define the spec; schema is
   generated (feature-gated `schemars`), committed, and drift-checked — never
   hand-edited (AccessKit + nixcfg-rs pattern).
2. **Kill the god object**: host owns state/control flow; core exposes free
   functions (reducer, projectors, geometry) usable without any controller,
   store, or full frame; backends expose independently callable native parts.
3. **Caller-owned scratch**: projection writes into reusable buffers and
   borrows them back (taffy low-level pattern); assert zero steady-state
   allocation with an allocator-counting test.
4. **One author, mechanized parity**: Nix (or any external author) is the sole
   canary author; a native checker proves schema/value/provider/disposition
   parity with ordered pointer diagnostics.
5. **Two schemas, two purposes**: authored declarative spec (versioned,
   immutable) vs serialized operator layout state (mergeable, migratable) —
   never conflate them.

## Implementation Guidance

### Installation

```toml
# core Cargo.toml — spec machinery stays optional
[dependencies]
serde = { version = "1", features = ["derive"] }

[features]
default = []
spec-json = ["dep:serde_json"]          # optional serde_json
spec-schema = ["spec-json", "dep:schemars"]  # implies spec-json

[dependencies.schemars]
version = "1"        # v1 line verified 2026-09-14; pin major — schema shape
optional = true      # may change across versions (upstream documents this)
```

```toml
# native checker only — no browser runtime
panel-kit = { path = "..", default-features = false, features = ["spec-plan"] }
serde_path_to_error = "0.1"   # verify exact pin on crates.io at adoption
```

### Configuration

- Commit the canonical generated schema (e.g.
  `nix/schema/workspace-spec.schema.json`); a check regenerates, canonicalizes
  (sorted keys), and diffs it.
- Nix producer: normalize attrsets (every required field present, nulls
  explicit), validate against the committed schema with a vendored pure-Nix
  subset validator, emit JSON via `pkgs.writeText`/`lib.generators`.
- Wasm consumers receive the JSON as `PANEL_KIT_WORKSPACE_SPEC = "${spec}"`
  and embed with `include_str!(env!("PANEL_KIT_WORKSPACE_SPEC"))`.

### Integration Points

- Backend crates: input translation (native events → core events, including
  wheel-consumed disposition) + native painters + store adapters; nothing else.
- Persistence: `LayoutStore` trait + `SavePolicy` in core; one logical record
  per store instance; `clear` required for reset semantics.
- Examples are canaries: buildable per backend, exercising the full spec
  surface; add a build check per documented backend.

## Code Examples

### Standalone single-panel projection (no controller)

```rust
let projected = project_panel(&panels[i], i, PanelProjectionInput {
    viewport, preferred_mode, surface, focused, pointer_dragging: false,
    tile_dragging: false, workspace_scroll: 0.0,
    clamp: &Clamp::WEB, chrome: &ChromeSpec::surface_only(),
    tile_grid: None, tiled_origin: None,
})?;
let el = panel_surface(projected, Some("nodes-standalone"), rsx! { NodesBody {} });
// host composes its own dock/navigation; nothing implicit is mounted
```

### Pure-Nix spec producer shape

```nix
pkgs.lib.mkWorkspaceSpec {
  spec_version = 1;
  id = "workspace-canary";
  layout.units = "Cells";          # exact normalized snake_case Rust names
  chrome.dock = false;             # chrome parts are explicit booleans
  panels = [ { id = "nodes"; content.kind = "custom";
               content.binding = "app.nodes"; } ];
}
# -> { value; json; panel_ids; bindings; }  (validated against committed schema)
```

### Strict tagged content model

```rust
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ContentSpec {
    Custom { binding: String },
    TimeSeries { source: DataSource<Vec<SeriesModel>>, unit: String },
}
```

## Sources & Verification

| Source | Type | Last Verified |
|--------|------|---------------|
| https://rust-lang.github.io/api-guidelines/ | Official | 2026-09-14 |
| https://github.com/AccessKit/accesskit | Official repo (read) | 2026-09-14 |
| https://docs.rs/taffy · https://github.com/DioxusLabs/taffy | Official (read) | 2026-09-14 |
| https://github.com/linebender/xilem/blob/main/xilem/ARCHITECTURE.md | Official (search) | 2026-09-14 |
| https://zellij.dev/documentation/layouts.html | Official (search) | 2026-09-14 |
| tmux wiki / SO #59439821 / SU #489508 | Official + community (search) | 2026-09-14 |
| https://dockview.dev/docs/overview/introduction/ | Official (search) | 2026-09-14 |
| https://docs.rs/statig | Official (read) | 2026-09-14 |
| https://serde.rs/container-attrs.html · serde#1600, #2634 | Official + issues (search) | 2026-09-14 |
| https://docs.rs/schemars (+ v1 announcement) | Official (read) | 2026-09-14 |
| https://docs.rs/serde_path_to_error | Official (read) | 2026-09-14 |
| NixOS RFC 42 · nixpkgs formats.nix · Discourse | Official (search) | 2026-09-14 |
| https://github.com/mushrowan/nixcfg-rs | Community (read) | 2026-09-14 |
| r/NixOS pure-Nix JSON-schema validator · nix-effects · fromJsonSchema | Community (search) | 2026-09-14 |
| https://crane.dev/examples/trunk.html (+ workspace variant) | Official (search) | 2026-09-14 |

## Changelog

| Date | Changes |
|------|---------|
| 2026-09-14 | Initial creation for task: Refactor panel-kit into a composable library with a feature-complete pure-Nix WorkspaceSpec |
