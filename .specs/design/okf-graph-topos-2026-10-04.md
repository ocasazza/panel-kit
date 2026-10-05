# OKF repository grammar, graph topoi, and the external-component contract

A third grammar/topos demo: a GitHub repository in Google's Open Knowledge
Format (OKF v0.2) is the **grammar**; two UI regimes over its knowledge base
(`okf-catalog`, `okf-lineage`) are the **topoi**; a geometric morphism between
them is visible as a **graph layout transported along the morphism**, drawn by
jump-cannon's WebGPU graph view (vendored, `vendor/jump-cannon/VENDORED.md`)
behind a new declarative **external-component contract** in core.

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

`LayoutAlgorithm` names map 1:1 onto vendored `graph-layouts` solvers; `Force`
is the GPU force simulation alone.

## Topoi and the morphism

- `okf-catalog`: object sorts are normalised OKF types plus `bundle`; morphism
  sorts are the link kinds plus `contains`. Graph laid out `concentric`,
  grouped by sort. Panels: graph, concepts table (type, status, tags, trust,
  stale_after), tag badges, per-type gauges, stable-share meter, selected
  concept text, repository/grammar text, morphism text, layout text.
- `okf-lineage`: coarser roles — `source` (table, dataset, policy), `derived`
  (metric, computation), `operator` (skill, attester), `context` (bundle,
  reference, concept); morphisms `derivation` (derived_from, computes),
  `execution` (executed_by, attested_by), `citation` (references),
  `containment` (contains). Graph laid out `dagre` along derivation.
- `f: okf-catalog → okf-lineage` is the type→role map φ (non-injective). Both
  regimes map to the existing physics base (`nix/lib/physicsData.nix`) and
  must commute (`validate_over`), so physics is the pullback of one base.

Switching topos along `f` is the visible morphism, on every surface:

1. Every node's colour and sort move along φ (fibres of φ merge clusters).
2. The codomain's layout is solved **seeded from the current positions**, and
   the renderer tweens old → new positions (≈800 ms; instant under
   `prefers-reduced-motion`); on WebGPU the force simulation then resumes with
   the codomain's pulled-back parameters (`update_layout_options`).
3. Selection transports by `∃f` (forward) / `f*` (back), as `topos_demo.rs` does.
4. The "Layout" text panel states domain algorithm → codomain algorithm, φ with
   fibre sizes, and the pulled-back force parameters that changed.

Selecting a node in the graph emits `Select`; the host restricts the selected
concept's sheaves (attributes text, neighbours) to that object.

## Surfaces

- Web (`examples/okf.rs`): GraphView when `navigator.gpu` exists, else the SVG
  fallback; the same positions drive both.
- TUI native and browser (`crates/panel-kit-tui/examples/okf.rs`,
  `browser_okf.rs`): braille painter over the same positions (no WebGPU in a
  terminal), same panels, same keys.
- Keys everywhere: `t` switch topos along f / back, `n`/`p` next/previous node
  selection, `l` re-solve layout, `r` reset.

## The trio: agentic, membrane, OKF

OKF is the third regime beside the agent-trace regimes. Agent traces with
edges between sessions assemble like lipids under Brownian motion: chains,
then sheets/rings, then flat tubes, then closed membranes. `assembly()`
already classifies each component of a site by β₁, χ, and boundary; the trace
data carries every stage. All three regimes pull their physics back from the
one base (`nix/lib/physicsData.nix`), so a morphism changes the presentation
while the assembly is invariant.

Morphisms (each pair related; all commute over the base):

- `f: agentic → membrane` (exists): planner, reviewer ↦ head; coder ↦ tail;
  memory ↦ solvent; continues ↦ tail_bond, shares ↦ lateral, consolidates ↦
  solvation.
- `ψ: okf-catalog → membrane`: table, dataset, policy ↦ head; metric,
  computation, skill ↦ tail; bundle, reference, concept, log ↦ solvent;
  derived_from, computes ↦ tail_bond; executed_by, attested_by ↦ lateral;
  references, contains ↦ solvation.
- `χ: okf-catalog → agentic`: table, dataset, policy ↦ planner; metric,
  computation, skill ↦ coder; bundle, reference, concept, log ↦ memory;
  derived_from, computes ↦ continues; executed_by, attested_by ↦ shares;
  references, contains ↦ consolidates.
- The triangle commutes: `f ∘ χ = ψ` on every sort.

Sites: the agent-trace site (grammar `trace-lines`) and the OKF site (grammar
`okf`). A regime views any site whose native regime has a morphism into it:
the site's kinds are sorted by its native regime, then pushed along the
morphism (composite sorting functor). Traces view in agentic (native) and
membrane (via f); OKF views in okf-catalog (native), agentic (via χ), and
membrane (via ψ). Traces in okf-catalog is not offered: no morphism
agentic → okf-catalog exists, and the demo says so instead of inventing one.

Graph: agentic and membrane declare the graph panel with `algorithm = force`,
so the pulled-back physics runs and components visibly assemble (the closed
membrane component settles into a shell, tubes into cylinders, sheets flat).
Every surface runs the force: WebGPU on the web, a deterministic CPU force
solve in `graph_layout` (per-node repulsion/mass, per-edge weight/rest
length, 3-D, projected to 2-D) for the SVG and braille painters. Moving along
a morphism recolours by φ and keeps positions; switching site re-solves.

Demo `trio` (web `examples/trio.rs`, TUI `trio`/`browser_trio`): header
selects site (traces | OKF) and regime (agentic | membrane | okf-catalog,
unavailable pairs disabled with the reason), shows the morphism taken, and
panels include the regime's sheaves plus Assembly (stage per component),
Morphism (the φ table with fibre counts and the commuting triangle), and
Physics (pulled-back tuning, identical across regimes).
