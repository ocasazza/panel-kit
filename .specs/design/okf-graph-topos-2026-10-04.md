# OKF repository grammar, graph topoi, and the external-component contract

A third grammar/topos demo: a GitHub repository in Google's Open Knowledge
Format (OKF v0.2) is the **grammar**; `okf-catalog` is the **topos** that reads
it; the corpus joins the agent traces as one site read in three regimes
(`agentic`, `membrane`, `okf-catalog`), and the geometric morphisms between
those regimes are visible as a **graph layout transported along the
morphism**, drawn by jump-cannon's WebGPU graph view (vendored,
`vendor/jump-cannon/VENDORED.md`) behind a new declarative
**external-component contract** in core.

## Data

`GoogleCloudPlatform/knowledge-catalog` @ `58e16bdb7a34430f055ea57e84655cff37000c03`,
every bundle under `okf/bundles/` (acme_retail, crypto_bitcoin, ga4,
stackoverflow), fetched by Nix (`fetchFromGitHub`, pinned hash) and serialised
by `nix/data/okf-knowledge-catalog.nix` into one JSON envelope — no parsing in
Nix, only file enumeration:

```json
{ "repository": "GoogleCloudPlatform/knowledge-catalog",
  "rev": "58e16bdb…",
  "bundles": [ { "name": "acme_retail",
                 "files": [ { "path": "metrics/gross-margin.md", "text": "---\ntype: Metric…" } ] } ] }
```

Files are every `*.md` under the bundle, sorted by path.

## Grammar: the `okf` engine

OKF is YAML frontmatter plus CommonMark; a pest grammar cannot parse it
robustly (jump-cannon reached the same conclusion, `docs/importer-architecture.md`
§ OKF). `panel-kit-grammar` gains a third engine beside pest and json:
`ParserSpec::Okf(OkfSpec)`, wasm32-clean, whose *rules are data* in the package:

- `kinds`: OKF `type` → site kind (e.g. `"Attested Computation" → "computation"`);
  unmapped types → `default_kind`. A concept with no `type` is a diagnostic.
- `link_rules`: frontmatter pointer → morphism kind, e.g.
  `sources[*].resource → derived_from`, `executor.resource → executed_by`,
  `attester.resource → attested_by`, `computation → computes`.
- `body_links`: CommonMark links in the body → `references`.
- `fields`: frontmatter keys projected into `Object.fields` (scalar → string,
  list/map → compact JSON). `tags` → `Object.tags`. `title` → `Object.title`.
- `bundle_kind` / `contains_kind`: each bundle becomes an object
  (`<bundle>`, kind `bundle`) with `contains` morphisms to its concepts.

Concept id = `<bundle>/<bundle-relative path without .md>`. Reserved `index.md`
and `log.md` are not concepts. Link targets resolve relative to the linking
file (`./x.md`, `../dir/x.md`) and frontmatter resources relative to the bundle
root (`policies/x.md`); external, image, self, missing, and non-`.md` targets
create no morphism. Same semantics as jump-cannon `crates/okf-importer`
(`OKF_LINK_RULES`, `resolve_reference`). Derived field `trust`: `human-reviewed`
when `verified` lists a `human:` reviewer, else `machine-generated` when
`generated` is set, else `unattested`.

## External-component contract (core)

A body panel-kit does not render itself, declared by contract so validation,
lifecycle, sizing, events, and fallback are spec data instead of host policy.

Workspace spec (`WorkspaceSpec.components`, default empty):

```json
"components": [ { "id": "graph", "contract": "graph", "requires": ["webgpu"],
                  "keep_mounted": true, "min_size": [320.0, 240.0],
                  "events": ["select", "hover"], "fallback": "cells" } ]
```

Panel content:

```json
{ "kind": "external", "component": "graph",
  "source": { "source": "binding", "id": "okf.graph" },
  "layout": { "algorithm": "dagre", "group_by": "sort", "params": { "rank_sep": 2.0 } } }
```

- `contract` is a closed set; today only `graph`, whose data is
  `panel_kit_core::widgets::graph::GraphModel` and whose config is `GraphLayout`.
- `requires` is a closed capability set (`webgpu`). A backend that lacks a
  capability uses `fallback`: `cells` = the backend's built-in renderer-neutral
  graph painter over the same positions (web SVG, TUI braille), `notice` = an
  honest "renderer unavailable" body.
- `keep_mounted` = the panel stays mounted when minimized/maximized
  (`src/keepalive.rs`), required for an imperative canvas.
- `events` the component may emit; the host receives
  `ComponentEvent::{Select { node: String }, Hover { node: Option<String> }}`.
- Validation: every external panel names a declared component; every declared
  component is referenced; `min_size` is positive in the spec's units.

`GraphModel` (renderer-neutral):

```rust
pub struct GraphModel { pub nodes: Vec<GraphNode>, pub edges: Vec<GraphEdge> }
pub struct GraphNode { pub id: String, pub label: String, pub sort: String,
                       pub color: <core rgb type>, pub size: f32,
                       pub mass: f64, pub repulsion: f64 }
pub struct GraphEdge { pub source: u32, pub target: u32, pub kind: String,
                       pub weight: f64, pub rest_length: f64 }
pub enum LayoutAlgorithm { Force, Circle, Grid, Concentric, Spectral, Dagre, Fcose, Sphere }
pub struct GraphLayout { pub algorithm: LayoutAlgorithm,
                         pub group_by: Option<String>, pub params: BTreeMap<String, f64> }
```

`LayoutAlgorithm` names map 1:1 onto vendored `graph-layouts` solvers. `Force`
is the pulled-back physics itself: every node's mass and repulsion charge and
every edge's weight and rest length. The host's CPU force solve
(`graph_layout::solve`, one solve per connected component, packed on a grid)
is the static answer every painter can draw; on WebGPU the vendored engine runs
the same law live (`NodePhysics`/`EdgePhysics` per node and edge, options
derived from the CPU solve's constants in `graph_component::force_options`),
so the arrangement keeps relaxing and a morphism re-tunes the running
simulation in place.

## Topoi and the morphism

- `okf-catalog`: object sorts are normalised OKF types plus `bundle`; morphism
  sorts are the link kinds plus `contains`. Graph laid out `force`
  under the one pulled-back physics. Panels: graph, concepts table (type, status, tags, trust,
  stale_after), tag badges, per-type gauges, stable-share meter, selected
  concept text, repository/grammar text, morphism text, layout text.
- The same catalog is one regime of the trio's site: `f: agentic → membrane`,
  `ψ: okf-catalog → membrane`, `χ: okf-catalog → agentic` and
  `ω: agentic → okf-catalog`. All three regimes map to the existing physics base
  (`nix/lib/physicsData.nix`) and must commute (`validate_over`), so physics is
  the pullback of one base.

Switching regime along a morphism is the visible morphism, on every surface:

1. Every node's colour and sort move along the morphism (fibres merge clusters).
2. The codomain's layout is solved **seeded from the current positions**, and
   the renderer tweens old → new positions (≈800 ms; instant under
   `prefers-reduced-motion`). On WebGPU a `Force` graph mounts on the solve's
   seed sphere and assembles live; the physics the simulation runs is the
   codomain's pullback, pushed into the engine without re-seeding.
3. Selection transports by `∃` forward and `*` back, as `trio_host.rs` does.
4. The "Layout" text panel states domain algorithm → codomain algorithm, the
   morphism with fibre sizes, and the pulled-back force parameters that
   changed.

## Surfaces

- Web (`examples/trio.rs`): GraphView when `navigator.gpu` exists, else the SVG
  fallback; the same positions drive both.
- TUI native and browser (`examples/trio_tui.rs`, `examples/trio_browser_tui.rs`):
  braille painter over the same positions (no WebGPU in a terminal), same
  panels, same keys.
- Keys everywhere: `t` next regime / back, `n`/`p` next/previous node
  selection, `l` re-solve layout, `r` reset.

## The trio: agentic, membrane, OKF

**One site.** The agent trace and the OKF knowledge objects are one document
(`nix/data/trace.lines`, or the equivalent `nix/data/trace.json`, which is
generated from it). It carries the sessions — `planner`, `coder`, `reviewer`,
`memory` with `continues`, `shares`, `consolidates` — and the OKF objects beside
them — `bundle`, `table`, `dataset`, `metric`, `computation`, `policy`, `skill`,
`reference`, `concept`, `log` with `contains`, `derived_from`, `computes`,
`executed_by`, `attested_by`, `references` — joined by two cross-morphisms that
make it one graph: `produces` (a session ↦ the object it produced, 8 bonds) and
`verifies` (a reviewing session ↦ the object it attests, 8 bonds). Every object
kind carries its own fields: sessions `latency_ms`, `energy`, `t`, `status`;
knowledge objects `status`, `trust`, `stale_after`. Both grammars read the merged
site, so `g` re-reads it rather than switching sites.

**Three regimes, always available.** `agentic`, `membrane`, and `okf-catalog`
are three views of that one site; there is no site control and no pair is ever
refused. A regime sorts a kind **natively** when its topos is the identity on it
(`Topos::native_object_kinds`: the session kinds are native in `agentic`, the
OKF kinds in `okf-catalog`, and no kind in `membrane`); otherwise the kind is
pushed along the morphism from the regime that does sort it natively, which is
the composite sorting functor. Every regime therefore sorts every kind:
sessions reach `membrane` along `f` and `okf-catalog` along `ω`; knowledge
objects reach `agentic` along `χ` and `membrane` along `ψ`. `GeometricMorphism::
validate` checks that agreement only on the domain's native kinds, since a kind
the domain sorts by transport is one the codomain sorts itself.

**Morphisms and the laws.** `f: agentic → membrane` (planner, reviewer ↦ head;
coder ↦ tail; memory ↦ solvent; continues ↦ tail_bond, shares ↦ lateral,
consolidates ↦ solvation), `ψ: okf-catalog → membrane` (table, dataset, policy ↦
head; metric, computation, skill ↦ tail; bundle, reference, concept, log ↦
solvent; derived_from, computes ↦ tail_bond; executed_by, attested_by ↦ lateral;
references, contains ↦ solvation), `χ: okf-catalog → agentic` (table, dataset,
policy ↦ planner; metric, computation, skill ↦ coder; bundle, reference, concept,
log ↦ memory; derived_from, computes ↦ continues; executed_by, attested_by ↦
shares; references, contains ↦ consolidates), and `ω: agentic → okf-catalog`
(planner, reviewer ↦ policy; coder ↦ computation; memory ↦ concept; continues ↦
computes; shares ↦ executed_by; consolidates ↦ references). The session handoff
kinds `produces` and `verifies` are agentic `shares`, so the catalog reads them
as `executed_by`. `TrioHost::load` checks all four against their domain and
codomain, the triangle `f ∘ χ = ψ`, the square `ψ ∘ ω = f`, that each commutes
over the base (`p_membrane ∘ f = p_membrane ∘ ψ = p_agentic ∘ χ =
p_catalog ∘ ω`), and — per kind — that the physics pulled back from the base
along **every** path from the kind's native regime is the same engine and the
same numbers.

**Data transport.** A morphism carries attributes as well as sorts, field ↦
field, the trivial identity where the map is silent. `χ` and `ψ` map an OKF
object's `stale_after` onto the agentic `latency_ms` and the membrane `energy`
(the one numeric horizon an OKF object publishes); `status` crosses unchanged.
`ω` maps a session's `energy` onto the catalog's `trust` and its `latency_ms`
onto the catalog's `stale_after`. Every field is therefore available in every
regime: `host.resolve` answers every binding of every regime with real content
and no panel degrades to a diagnostic. The Morphism overlay states the maps.

**Physics.** Every regime pulls its tuning back from the one base
(`nix/lib/physicsData.nix`), so the graph's mass, repulsion, weight, and rest
length are identical in all three regimes; only sorts and colours move. The
agentic and membrane regimes assemble over their own handoff kinds
(`continues`, `shares`) — not the cross morphisms, which the assembly leaves out
— so the sessions still form chains, rings, sheets, tubes, and closed membranes
(`AssemblySheaf.kinds`), while the catalog assembles the knowledge objects over
`derived_from`, `computes`, `executed_by`, `attested_by`, `references`.

Demo `trio` (web `examples/trio.rs`, TUI `trio`/`browser_trio`): the header
selects the regime (all three, never disabled) and the site's grammar, and the
status line states, per native regime, the route its kinds take and how many
objects that covers. Panels are the regime's sheaves over the one site plus
Assembly (stage per component), Morphism (the routes, φ per kind, the triangle,
the square, and the attribute maps), Physics (the pulled-back tuning, one
section), and Selected (the object's fields as this regime reads them, the route
that transports them, and its neighbours).
