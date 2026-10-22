# Adversarial Review — panel-kit P0: grammar switching + data-source grammar

**Date:** 2026-10-02
**Reviewer stance:** ruthless; every claim grounded in the current tree.
**Verdict:** Part 1 (switching) is a real P0 but is mis-architected against a type that no longer exists. Part 2 (data-source grammar) is premature abstraction that violates the refactor's own non-goals and should be cut from P0 entirely.

Evidence base (read before writing this):
- `crates/panel-kit-core/src/reducer/types.rs:21-68` — shipped host state is `Snapshot<K>`, not `WorkspaceState`.
- `crates/panel-kit-core/src/reducer.rs:42-45` — mutation is the free fn `reduce(&mut Snapshot, event, context)`.
- `crates/panel-kit-core/src/persist.rs:100-112` — `layout_key_for_spec` already exists; zero callers.
- `crates/panel-kit-core/src/loading.rs:1-25` — core **already** models async-source lifecycle as pure `LoadStatus` state.
- `crates/panel-kit-core/src/lib.rs:3` — "pure data and math — no DOM, no terminal, no async."
- `crates/panel-kit-core/Cargo.toml:11-19` — deps are serde/serde_json/schemars only; no transport runtime.
- `examples/support/spec_workspace.rs:1-72` — `SpecWorkspaceState` self-declares "example support, not a public controller facade."
- `.specs/design/libraryness-2026-09-14.selection.md:26` — refactor charter explicitly forbids a monolithic `WorkspaceState`.
- `.specs/tasks/done/refactor-panel-kit-composable-library.refactor.md:24,58-60` — §16 non-goals exclude async/remote data; HR-2/HR-20/HR-21 require deleted controllers and preserved keys.
- jump-cannon `crates/importer/src/lib.rs:1-64,29-37` and `src/json/config.rs:20-195` — the "package model" is a 3-engine graph-projection engine with a native-vs-wasm security boundary.

---

## 1. Top problems, ranked by severity

### P1 — CRITICAL: `switch_spec` targets a type the refactor deleted, and re-grows the god object the refactor spent 28 sub-tasks removing

**The plan (lines 9-14):** `WorkspaceState::switch_spec(new_resolved, on_switched)` that swaps "snapshot signal + resolved Rc + store Rc."

**Evidence it's wrong:**
- There is **no `WorkspaceState` type in the shipped library.** The completed refactor replaced it with host-owned `Snapshot<K>` (`reducer/types.rs:29`) mutated by a free function `reduce(&mut Snapshot, …)` (`reducer.rs:42`). `Snapshot<K>` has fields `panels / preferred_mode / viewport / focused / drag / tile_drag / workspace_scroll` — **no store, no resolved, no snapshot signal.** `switch_spec` as described cannot be a method on it without violating the core-purity invariant that `reducer/types.rs:21-27` documents verbatim ("persistence transports and render caches never enter core state").
- The design charter explicitly bans exactly this move: *"Do not trade `Workspace<K>` for an equally monolithic `WorkspaceState` that apps must route everything through"* (`selection.md:26`), and HR-2 requires *"both backend controllers deleted"* (`refactor…md:40`).
- The only type that actually bundles `resolved: Rc<ResolvedWorkspace>` + `snapshot: Signal<…>` + `store: Rc<…>` is `SpecWorkspaceState` (`examples/support/spec_workspace.rs:29-41`) — whose own module header says *"This is example support, not a public controller facade"* (`spec_workspace.rs:2-6`). It is not library API.

So the plan's central API is attributed to a nonexistent type, and the only place it *could* live is example glue. Landing it as a library controller method resurrects the precise god object the refactor deleted.

**Recommendation:** Do not add a stateful controller method. The reusable, headless kernel of a switch is three pure calls the host already owns: `persist_snapshot(old_store, &old_snap, catalog)` → build new `LayoutStore` → `restore_snapshot(new_store, new_defaults, …)`. Ship that composition as (a) a documented host recipe exercised by the web example, and optionally (b) one pure core free fn operating over `&dyn LayoutStore` + `Snapshot` + `PanelCatalog`. This keeps the swap browser-free testable (closing attack vector 6, which the plan never addresses) and respects HR-2. The signal/Rc swap stays in the app, where `SpecWorkspaceState` already lives.

---

### P2 — CRITICAL: the data-source grammar (Part 2) is not P0; it violates the refactor's explicit non-goals and has no in-repo consumer

**The plan (lines 16-32):** upstream jump-cannon's importer package model into `panel-kit-core` as an abstract grammar, as part of P0.

**Evidence it's out of scope:**
- The refactor that *just landed* (it is in `.specs/tasks/done/`) lists as an explicit §16 non-goal: *"no async/remote persistence or generic key/value storage"* (`refactor…md:24`). A data-source grammar is remote/async data acquisition by definition.
- `panel-kit-core/src/lib.rs:3` states the crate is "no async"; `Cargo.toml:16-19` carries no transport/runtime dependency. Part 2 contradicts the crate's stated identity.
- There is **no consumer in panel-kit that needs this at P0.** The two real consumers (jump-cannon, apple-notes-ocr-flow) already own their data loading; the refactor's acceptance only requires that they keep working (HR-20/HR-21). Nothing in P0 renders a graph.
- The design is **undecided.** The companion proposals file is dated the same day and still lists six competing approaches with probabilities and open risks (`data-source-grammar-2026-10-02.proposals.a.md:8-114`). Shipping an unselected 6-way design as "MVP" is future work wearing a P0 badge — exactly the scope-creep the attack brief (line 39) warned about.

**Recommendation:** Cut Part 2 from P0. Ship Part 1 (switching) alone. Re-open the data-source grammar as its own task, gated on (1) a real driving consumer inside panel-kit, (2) a single selected approach, and (3) an explicit amendment to the refactor's "no async/remote data" non-goal. P0's actual deliverable is grammar *switching*; the grammar *content* abstraction is a separate, later decision.

---

### P3 — HIGH: `DataSourceSpec`/`DataSourceProvider` couple pure core to graph + transport concepts, and misrepresent the importer model

**The plan (lines 23-31):** `DataSourceSpec` carries "field schema, relationship types, collection declarations"; `DataSourceProvider` exposes `load()`, `schema()`, `progress_stream()`.

**Evidence of coupling:**
- The jump-cannon importer is a **graph-projection engine**, not a generic data loader: collections declare `Produces::{Nodes, Edges}` (`json/config.rs:188-195`), node identity is `httpjson:{source_id}:{local}` (`json/config.rs:24-27`), and the config tree is Pagination/Collection/NodeRules/EdgeRule/EdgeListRules/Dedupe/Preflight (`json/config.rs:126-357`). "Relationship types" and "collection declarations" are nodes/edges/collections. panel-kit-core has **zero** graph types — its content vocabulary is display widgets: `ContentKind = {Custom, Text, Editor, Badges, Table, TimeSeries, Gauges, Flamegraph, Boxplot, Meter, Status, Spinner}` (`spec.rs:276-301`). A grep of `crates/panel-kit-core/src` for `Node|Edge|Graph` finds only a geometry padding field. Putting `DataSourceSpec` in core imports a foreign domain into a panel-layout crate.
- `DataSourceProvider::load()`/`progress_stream()` imports **transport and streaming** into a crate whose entire identity and feature-gating is "no async" (`lib.rs:3`). Even the proposals file admits approach 2's `load()` is "async-compatible" (`proposals.a.md:35`). The importer itself solved this with a hard `native` vs `wasm32 --no-default-features` split (`importer/src/lib.rs:39-45`); core has no such split and must not acquire one.
- "Upstream **the** importer package model" is factually inaccurate: the importer has **three** engines — pest (inline grammar `{root_rule, grammar, captures}`), json (the fields/collections manifest), and tvix (an inline Nix expression over typed variables) (`importer/src/lib.rs:50-64`). The plan's "field schema + collection declarations" describes **only the json engine.** It cannot represent pest or tvix. The abstraction as drawn upstreams a fraction and calls it the whole.

**Recommendation:** Keep the grammar/graph engine where it is (jump-cannon, or a new standalone `data-grammar` crate) — never in `panel-kit-core`. Panel-kit's correct seam already exists: a panel declares `ContentKind` + `binding: Option<String>` (`spec.rs:303-314`), the app resolves the binding to data, and the pure `loading.rs` `LoadStatus` store carries lifecycle. Core consumes *results* through that seam; it never defines a provider trait or a graph schema. If a provider trait must exist, it lives in the shell/app crate, not core.

---

### P4 — HIGH: the callback-based "topos composition" cannot inject state, races the first render, and has no async/in-flight story

**The plan (lines 14, 33-35):** step 5 fires `on_switched: Callback<(old_id, new_id)>` "so the app can inject domain state (topos, visual encoding)"; the callback "lets the app inject the data-source grammar change and domain state."

**Evidence it doesn't compose:**
- A Dioxus `Callback<(old_id, new_id)>` returns `()`. It is fire-and-forget; it **cannot inject or install** new domain state into the switch transaction — it can only trigger a side effect via captured signals. Calling a post-hoc notification "topos composition" oversells a plain event hook; panel-kit provides no composition surface, only a ping.
- **Ordering race:** the hook fires at step 5, *after* step 4 has already swapped snapshot/resolved/store. The new workspace's first projection therefore runs before domain state (topos/visual encoding) is installed — a one-frame window of stale or missing state.
- **No async story:** a data-source-grammar switch is inherently async (load the new source), but `switch_spec` is synchronous and so is the callback. There is no loading/empty/error state for the transition — even though core *already ships one* (`loading.rs:22-25`, `LoadStatus`). The plan reinvents nothing and reuses nothing here.
- **No quiescing of in-flight interaction (attack vector 7, unaddressed):** if `snapshot.drag.is_some()` or a tile reorder is live, swapping the snapshot signal mid-gesture leaves the pointer-up handler reducing against the *new* snapshot, against panel ids that may not exist in the new spec → corrupted layout or silent no-op. Likewise a debounced `OnSettle` save (`persist.rs:47-54`) in flight against the old store can land after the Rc swap.

**Recommendation:** Specify switching as an explicit, host-driven, ordered transaction, not a callback afterthought: (1) abort/settle in-flight gestures and pending saves; (2) `persist_snapshot` the old state; (3) construct the new resolved + store + domain state **fully before installing**; (4) swap atomically; (5) drive the first data load through the existing `LoadStatus` store so the UI renders loading/empty/error. If a hook is retained, give it a return value the controller installs, or have the host assemble the new state and pass it *in* — don't notify after the fact. Drop the "topos" vocabulary; it is a documented recipe, not a mechanism.

---

### P5 — MEDIUM: layout-loss regression — `layout_key_for_spec` changes the stored key with no migration, and currently has zero callers

**The plan (line 7):** `layout_key_for_spec(base, spec_id)` namespaces localStorage keys so switching "doesn't clobber layout."

**Evidence of regression:**
- The function returns `{base}/{spec_id}` (`persist.rs:110-112`). Today the web host keys its store by the bare `resolved.persistence.key` (`spec_workspace.rs:47-48`: `LocalStorageLayoutStore::new(key.clone())`), **not** via `layout_key_for_spec`. The moment a consumer adopts namespacing, the key changes from `base` → `base/{id}`, orphaning every existing saved layout. On first boot after upgrade, all existing users silently get a default layout.
- There is **no key migration.** The only migration machinery in the tree is V1→V2 layout *schema* migration (`migrate_v1`/`reconcile_units`, cited throughout `09-persistence-core.md:14-16`); nothing renames localStorage keys.
- This breaks the refactor's own guarantees HR-20 (jump-cannon "preserved keys/IDs") and HR-21 (apple-notes "persisted key/serde IDs") (`refactor…md:58-59`).
- `layout_key_for_spec` currently has **zero callers** (grep across the repo finds only its definition + doc). It is speculative infra already landed, and its doc comment's claim *"Existing single-spec consumers see no change"* (`persist.rs:107-109`) is true only because nobody calls it; the first caller makes it false.

**Recommendation:** Make the primary/initial spec id resolve to the legacy bare key — i.e. guarantee `layout_key_for_spec(base, initial_id) == base` for the first grammar (special-case the primary id, or store a `primary_spec_id` on the resolved workspace) — or run a one-time read-rename `base → base/{initial_id}` on the first switch-enabled boot. Fix the misleading doc comment to state the migration contract. Add a core unit test pinning that the primary spec maps to the legacy key.

---

## 2. What is over-engineered

- **All of Part 2 at P0.** No in-repo consumer, undecided 6-way design (`proposals.a.md:8-114`), and a direct collision with the just-ratified "no async/remote data" non-goal (`refactor…md:24`). This is the single largest over-reach.
- **`DataSourceProvider::progress_stream()`** (line 26). Streaming transport in a crate that is "no async" (`lib.rs:3`). Core already represents progress as pure state (`loading.rs` `LoadStatus` with `begin/update/succeed/fail`); a stream method is strictly redundant and strictly transport-coupling.
- **`DataSourceSpec` carrying "relationship types / collection declarations"** (line 24). This re-implements a graph modeling layer that already exists, mature and security-reviewed, in jump-cannon's importer. Duplicating a 3-engine projection engine into a panel library is net-negative surface.
- **The "topos" concept itself** (lines 20, 33-35). It is vocabulary layered over a plain event hook plus app-side wiring. The callback already decouples layout-switch from data-switch, so a unifying "grammar + domain state + data-source grammar" abstraction buys no mechanism — only nouns. The design charter demands each abstraction *"justify … with a concrete composition it enables"* (`selection.md:27`); topos enables none that a documented recipe doesn't.
- **A second version lineage for the package format** (line 24, "versioned, like WorkspaceSpec"). The importer is already `format_version = 3` with explicit upgrade-rejection and a `HARD_LIMITS` ceiling (`importer/src/lib.rs:25-27,66-80`). A parallel core `DataSourceSpec` version invites exactly the drift the proposals file flags as its top risk (`proposals.a.md:27`).

## 3. What is under-specified

- **Ownership of `switch_spec`.** Core `Snapshot<K>` has no store/resolved/signal (`reducer/types.rs:29-47`); `SpecWorkspaceState` is example-only (`spec_workspace.rs:2-6`). The plan never names a real library home. (P1)
- **How `on_switched` "injects" state** when `Callback<(old,new)>` returns `()`. The injection mechanism and its ordering relative to the first projection are undefined. (P4)
- **`DataSourceProvider::load()` return type.** Graph? Opaque bytes? Rows? This determines whether core must grow a graph type — the whole coupling question hinges on an unspecified signature. (P3)
- **The security boundary.** The importer is explicit: `pest_vm` has no fuel/deadline, native loading is for *trusted* packages only, and untrusted uploads **must** run in a resource-limited Wasm sandbox (`importer/src/lib.rs:29-37`). A grammar that can carry an inline pest grammar or a tvix Nix expression is executable input. The plan is silent on where this obligation lives once core "owns the grammar." (P2/P3)
- **Multi-engine representation.** pest (`root_rule/grammar/captures`) and tvix (Nix expr + typed `[[parser.variables]]`) do not fit "field schema + collections" (`importer/src/lib.rs:50-64`). The spec shape can't express 2 of 3 existing engines. (P3)
- **Credential redaction.** The importer redacts tokens from `Debug`, capability scopes, and error messages (`importer/src/lib.rs:21-23`). If any instance config transits panel-kit types, the redaction contract is unstated. (P3)
- **Testability of the switch (attack vector 6).** Nothing stated. Signal-bound swap logic is not headless-testable; the testable kernel (save-old/restore-new over a fake `LayoutStore`) is never factored out. (P1)
- **In-flight interaction + save coalescing during a switch (attack vector 7).** No quiesce step; interaction with `SavePolicy::OnSettle` debouncing is unspecified. (P4)
- **Key migration** from legacy bare `base` to `{base}/{id}`. (P5)

## 4. Bottom line

Ship **only Part 1**, and re-shape it: switching is a pure, host-composed, ordered transaction (`persist old → build store → restore new`), tested headlessly, with legacy-key back-compat — **not** a `WorkspaceState::switch_spec` controller method (that type is gone and the charter bans its return). **Cut Part 2** from P0: the data-source grammar has no consumer, no selected design, and directly violates the refactor's own "no async/remote data" non-goal. If it is ever built, the graph/transport engine stays out of `panel-kit-core`; panel-kit consumes results through its existing `ContentKind` + `binding` + `LoadStatus` seam. Retire the "topos" framing — it names a recipe, not a mechanism.
