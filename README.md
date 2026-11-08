# panel-kit

Generic Dioxus panel-workspace library. Every view is a panel you can
move/resize/minimize/maximize, with floating (free placement) and tiling
(auto grid) workspace modes, printer-CMY operation lights, pointer and
keyboard window management, tiling drag-to-swap (lift, follow, settle), a minimized-panel dock,
and versioned layout persistence. Includes reusable badge, spinner, dropdown,
cascade, and interactive-table widgets; panel-header actions and
loading-workspace primitives; Grafana and lightweight IDE panels; retained-DOM
keepalive composition; an optional Bevy canvas bridge; and a Monaco-based code
editor (`editor::MonacoEditor`) with `nix`, `.pest`, and `toml` languages, a
`cmd/ctrl+enter` submit action, and a token-matched dark theme.

Factored out of [jump-cannon](https://github.com/ocasazza/jump-cannon) and
apple-notes-ocr-flow, which both consume it as a git dependency:

```toml
[dependencies]
panel-kit = { git = "https://github.com/ocasazza/panel-kit" }
```

## Crates

The repo is a small workspace — one state machine, two renderers, one data
layer:

- **`panel-kit-core`** (`crates/panel-kit-core`) — the renderer-agnostic
  state machine: `PanelWin`, `WinState`, `Mode`, `SurfaceProfile`, keyboard
  commands, drag/resize and tile lift-and-swap math, viewport clamping, and the versioned
  `SavedLayoutV2` persistence shape. Units are abstract (CSS px on the web,
  cells in a terminal).
- **`panel-kit`** (repo root) — the Dioxus web shell: signals, pointer and
  keyboard event translation, accessible DOM chrome, and V2 localStorage
  persistence with automatic V1 migration.
- **`panel-kit-tui`** (`crates/panel-kit-tui`) — the ratatui shell: the same
  workspace drawn in terminal cells with crossterm input, printer-CMY
  controls, a dock line, and JSON-file persistence. Try it:
  `cargo run -p panel-kit --features spec-plan --example workspace_tui`. Via
  [ratzilla](https://github.com/orhun/ratzilla) this renderer can also
  target the browser DOM — one panel codebase, web and terminal skins. Its
  demos live in the root `examples/`: `workspace_tui`, `workspace_browser_tui`,
  `trio_tui`, `trio_browser_tui`.
- **`panel-kit-grammar`** (`crates/panel-kit-grammar`) — topos and grammar
  abstractions. A *grammar* is a declarative import package (`pest` runtime
  grammar with a capture map, or `json` pointers) that turns input into a
  renderer-neutral `Site`. A `pest` grammar that declares `line_records` parses
  appended lines apart (`CompiledGrammar::parse_append`), so a live feed
  extends its `Site` without reparsing. A *topos* is one UI/UX regime over that
  site: a
  `WorkspaceSpec` plus `Stalks` (what each object/morphism sort means: label,
  badge, assembly-stage prose) plus `sheaves` from binding ids to panel content
  models. Physics tuning (repulsion, mass, weight, rest length) is not authored
  per regime: it is the inverse image `p_R*` of a shared physics `BaseTopos`,
  carried by a `GeometricMorphism` (`Physics`). `Topos::global_sections(site,
  physics)` returns the `GlobalSections` hosts resolve `DataSource::Binding` ids
  against, `Topos::section(site, physics, binding)` resolves one binding without
  evaluating the other sheaves, and `Topos::physics_section(site, physics)` the
  engine parameters a layout consumes. Pure, sync, wasm32-clean.

The core crate is the contract: renderer-neutral state, surface
classification, geometry, pointer/keyboard semantics, and persistence live
there. The Dioxus and ratatui crates translate platform events and draw over
that same interface.

Panel chrome is intentionally compact across renderers: panel controls and
titles are inset into the top border row, following the ratatui `Block::title`
treatment, instead of using a separate full-width header section. This keeps
more panel height available for content while preserving the same drag,
reorder, minimize, maximize, and mode-toggle controls.

## Usage

Applications own the workspace. Start with a `PanelKind` enum, construct a
`panel_kit_core::reducer::Snapshot`, keep a reusable
`panel_kit_core::frame::ProjectionBuffer`, reduce browser events through
`panel_kit_core::reducer::reduce`, and render only the web parts you want:

```rust
let snapshot = panel_kit_core::reducer::Snapshot::from_defaults(
    default_layout(),
    panel_kit::Mode::Floating,
    viewport,
);
let frame = panel_kit_core::frame::project_into(input, &mut projection_buffer);

rsx! {
    style { {panel_kit::CSS} }
    div { class: "{panel_kit::widgets::root::root_class(&frame)}", tabindex: "0",
        header { class: "topbar", /* app-specific controls */ }
        for panel in frame.panels.iter().copied() {
            {
                let meta = catalog.get(panel.key).expect("projected panel is in the catalog");
                panel_kit::widgets::panel::panel_shell(panel, None, rsx! {
                    {panel_kit::widgets::panel::panel_chrome_with_events(panel, meta, emit, None, None)}
                    {panel_kit::widgets::panel::panel_body(rsx! { /* body for panel.key */ })}
                    {panel_kit::widgets::panel::resize_grip(panel, emit)}
                })
            }
        }
        {panel_kit::widgets::dock::dock(frame.dock, &catalog, emit, None)}
    }
}
```

Inject `panel_kit::CSS` once at the app root, then layer app-specific styles
after it; override the `:root` variables to retheme.

### Operation lights

The blue/yellow/pink operation cluster is deliberately printer-CMY rather
than macOS red/yellow/green: blue toggles floating/tiling, yellow minimizes,
and pink maximizes/restores. None of the operations destroys anything, so a
destructive red control would teach the wrong model.

### Surface tiers

`panel_kit::surface::surface_profile(width)` reports `Compact`, `Tablet`, or
`Regular` plus coarse-pointer, hover, and keyboard capabilities. The web
defaults are `<760px`, `<1180px`, and everything wider. Compact surfaces force
tiling and withhold the affordances that move or resize a panel by hand; tablet
and regular surfaces keep move, resize, minimize, and maximize behavior.
`panel_kit::widgets::root::root_class(&frame)` emits exactly one of `compact`,
`tablet`, or `regular` alongside `ws-root`, plus `coarse` when the primary
pointer is imprecise.

Two rules follow from that, and both are load-bearing:

- **Sizing follows the pointer, not the width.** `--hit-min` is `24px` by
  default and `44px` under `.ws-root.coarse`. A 1400px kiosk touchscreen is a
  regular surface that still needs 44px targets; a 900px mouse-driven window
  does not.
- **A tier never traps a window state.** Layouts persist across surfaces, so a
  panel maximized on a desktop arrives maximized on a phone. Compact therefore
  still renders the magenta restore light — and only that one — for a maximized
  panel.

### Keyboard window management

Translate a root `onkeydown` event with `panel_kit::input::keyboard_event`,
then apply it with `panel_kit_core::reducer::reduce` if the host did not give
the focused editor or application shortcut first refusal. Arrow keys move the
focused panel, Shift+arrows resize, and Alt+arrows fine-move. `m`, `f`, `t`,
and Escape minimize, maximize, toggle mode, and restore; Tab and Shift+Tab
cycle focus, while Enter raises. Bare shortcuts are ignored while a text input
owns focus.

### Cell-authored specs on pixel hosts

A `WorkspaceSpec` authored in `Units::Cells` renders natively in the terminal.
Pixel hosts (web, Bevy) call `ResolvedWorkspace::into_logical_px(width,
height)` once after `resolve`: the authored viewport maps onto the surface, so
geometry, clamp, tile metrics, surface thresholds, and keyboard steps scale by
the cell, while chrome takes the pixel profile
(`ChromeProjectionInput::full(ChromeMetrics::WEB)`). CSS-px specs pass through
unchanged.

### Versioned persistence

Web workspaces write `SavedLayoutV2` with schema version, `Units::CssPx`,
captured viewport, mode, and panels when the host applies `SavePolicy` or calls
`persist_snapshot`. Existing `{ panels, tiling }` V1 records remain readable
through `restore_snapshot`. Core exposes `StoredLayout`, `migrate_v1`,
`reconcile_units`, and `LayoutStore` so every renderer performs the same
upgrade and rescales foreign viewport/unit geometry. Named views persist
through the same reader and writer, one V2 record per view.

### Named views

For several named, switchable layouts inside one workspace, the application now
owns the view switcher state instead of calling a web controller hook. Keep a
core `SavedViews` registry, store it at `views_registry_key(base)`, and create
one `LocalStorageLayoutStore` per active view with `view_layout_key(base, name)`:

```rust
let registry = panel_kit_core::views::SavedViews::new(&["User", "Sessions"]);
let store = panel_kit::store::LocalStorageLayoutStore::new(
    panel_kit_core::views::view_layout_key("myapp_layout", &registry.active),
);
// host: restore_snapshot/store + reduce + SavePolicy + web parts
```

The registry persists at `myapp_layout:views`, each view's layout at
`myapp_layout:view:<name>`; a pre-views layout at the bare base key still
migrates into the first view on first run (copied, never deleted). See the
`views` example for the complete host-owned switcher and parts loop.

### Loading before WASM

A Rust component cannot render until the browser has downloaded and
instantiated the application's WASM module. Panel Kit therefore exposes one
loading-workspace contract at both sides of that boundary:

- `BOOT_CSS` and `BOOT_HTML` are static, JavaScript-free assets for immediate
  first paint. Keep the Dioxus mount element empty, copy the fragment as its
  immediately following sibling, replace its application/status text, and
  inline the critical stylesheet in `<head>`.
- `LoadingWorkspace` renders the same shell after Dioxus mounts, for app-owned
  phases such as graph fetches or GPU initialization. Its `progress` prop
  turns the header bar determinate with a mandatory percentage once a phase
  is measurable; `None` keeps the static fragment's honest indeterminate bar.

`BOOT_CSS` cannot read `panel_kit::CSS` — it paints before WASM exists — so
its palette is generated from the same core token source rather than
hand-copied, and stays value-identical to the `:root` variables by
construction.

The static fragment carries `data-panel-kit-static-boot`; the stylesheet's
adjacent-sibling selector hides only that fragment after Dioxus marks its mount
root, so no cleanup JavaScript is needed. Do not put the fragment inside the
mount element: Dioxus does not clear pre-existing children.

```html
<head>
  <style>/* exact contents of panel-kit::BOOT_CSS */</style>
  <link data-trunk rel="rust" href="Cargo.toml" />
</head>
<body>
  <div id="main"></div>
  <!-- exact contents of panel-kit::BOOT_HTML, immediately adjacent -->
  <section class="panel-kit-boot" data-panel-kit-static-boot
           role="status" aria-live="polite">
    <!-- panel-kit-boot-bar + panel-kit-boot-panels contract -->
  </section>
</body>
```

### Loading after mount: stores, gates, and the global bar

Page hydration is store-shaped (pinia-style): render the workspace chrome
immediately, then let every async data source load lazily behind one shared
state vocabulary.

```rust,no_run
use panel_kit::loading::{loading_store, LoadingGate, GlobalLoadingBar};

// One store per data source; same id, same store, from any component.
let store = loading_store("branches", "loading branches…");
use_future(move || async move {
    store.begin_with("connecting");
    // …fetch, reporting store.update(Some(fraction), Some(stage))…
    store.succeed(); // or store.fail("message")
});

// Panel level: the gate renders a ProgressBar (with its percentage while
// determinate) until the store is Ready, then the children.
rsx! { LoadingGate { store, div { "loaded content" } } }

// Workspace level: one compact bar in the top bar aggregates every pending
// store (mean of the reported fractions; hidden while nothing is pending).
rsx! { GlobalLoadingBar {} }
```

The display contract: a bar beats a spinner (`Spinner` stays for tiny inline
waits only), and a determinate bar always shows its percentage — `None` is
the honest indeterminate state, never a fabricated number. Stores are
ephemeral in-flight status only; rewind/undo of application state belongs to
the app's own snapshot-timeline framework, and a rewind is just a
`begin` → `succeed` transition on a store so replays surface on the same
bars. See `examples/loading.rs` for the full arc.

## Documentation

API docs are rustdoc-first — the crate root has a quick start, theming
notes, and a guide to the examples. Breaking 0.2.x consumers should start
with the [1.0 migration guide](MIGRATION.md):

```sh
cargo doc --no-deps --open
```

(Once published to crates.io, the same docs will be on docs.rs, built for
`wasm32-unknown-unknown`.)

## Examples

One browser demo per component. The workspace canary exercises the full
window-manager surface; focused component demos cover every public parameter.
From `nix develop` (which provides a matching dioxus-cli 0.6.x, lld, and
wasm-bindgen-cli):

```sh
dx serve --example workspace --platform web
```

| example | shows |
| --- | --- |
| `workspace` | host-owned `Snapshot` + `ProjectionBuffer` + composable web parts; floating pointer and keyboard move/resize/raise; host-owned move/resize snap toggles in the dock trailing slot; layout reset via store clear plus snapshot replacement; tiling lift-and-swap drag and span resize; viewport clamping; wheel chaining; live surface state; and versioned localStorage persistence |
| `views` | host-owned named views over one workspace: core `SavedViews`, per-view `LocalStorageLayoutStore` records (`panel_kit_example_views:view:<name>` + the `:views` registry), explicit `SavePolicy`, a composable parts loop, a switcher bar with create/rename/delete, per-view reset, and the legacy single-layout migration copy |
| `badge` | all ten `BadgeKind`s, every prop (`active`, `with_x`, `with_plus`, `small`, `override_color`, `accent_color`, both `BadgeClickKind`s, `emit_hover`) behind live toggles, an event log proving every `BadgeAction` variant fires, and a `tag_hue` FNV hue-spread row |
| `dropdown` | grouped searchable single-select with host-owned popup and selected value |
| `cascade` | host-owned Miller-column navigation retained through a parent render storm |
| `table` | core `TableModel` painting with host-owned row selection, dense mode, full-value tooltips, and empty state |
| `spinner` | `Spinner` with and without `label`, plus a live-editable label |
| `loading_workspace` | the post-mount `LoadingWorkspace` twin of the static pre-WASM boot contract |
| `theming` | the documented full-palette retheme path: `:root` variable overrides layered after `panel_kit::CSS`, with three switchable presets |
| `editor` | `editor::MonacoEditor`: two-way `Signal<String>` binding, `on_change` event log, imperative `EditorHandle` (set/read value, language, read-only, layout), reactive `language`/`read_only` props, the `nix`/`pest`/`toml` Monarch tokenizers, the `on_submit` action, and the minimal `toml` language on real samples |
| `topos` | two regimes (`topos-agentic`, `topos-membrane`) over one session site parsed by either the `trace-lines` (pest) or `trace-json` grammar, running one shared physics base (`topos-physics`). `t` switches regime (per-topos layout persisted; the Physics panel stays identical, only the UI/UX sheaves change), `g` switches grammar (identical content), `n`/`x` append a valid/malformed line (diagnostic, last good site kept), `r` resets. Each regime carries an active subobject on its per-sort badges: click a Roles/Species badge (web) or `s`/space (terminals) to toggle a sort; switching transports it by `∃f`/`f*` and the status line names the morphism |
| `trio` | one site — the agent sessions and the OKF knowledge objects, joined by `produces`/`verifies`, parsed by `trace-lines` or `trace-json` — read in three regimes (`topos-agentic`, `topos-membrane`, `topos-okf-catalog`), all of them always available, running one shared physics base (`topos-physics`). `t` switches to the next regime, `g` re-reads the site with the next grammar, `n`/`p` move the selection, `l` re-solves the graph layout, `r` resets the workspace layout. A regime switch keeps the arrangement and recolours it (the tuning is one base pullback, so only the sorts and colours move); a grammar switch re-solves from the positions on screen. No pair is ever refused, so no control is disabled |

`dx build --example <name> --platform web` produces the same app
statically under `target/dx/<name>/debug/web/public`.

### Monaco editor assets

`editor::MonacoEditor` vendors a minified ESM build of `monaco-editor`
under `assets/vendor/monaco-editor-0.56.0/` (MIT; `LICENSE` and
`ThirdPartyNotices.txt` included). The crate injects a small loader shim
itself; the consuming app only needs to serve that directory — copy it into
the app's own `assets/` (for trunk add `<link data-trunk rel="copy-dir"
href="assets/vendor" />` to `index.html`; for Tauri the same files ride
along in `frontendDist`), or call `editor::set_monaco_asset_base` before
first mount to point elsewhere. The wasm binary itself stays small: the
multi-MB bundle is fetched at runtime, once.

### Browser TUI canary

The ratatui backend also has a browser/WASM canary built with Ratzilla:

```sh
trunk serve examples/workspace_browser_tui.html \
  --example workspace_browser_tui \
  --features spec-plan \
  --address 127.0.0.1 \
  --port 8082
```

This example is intentionally comprehensive: workspace chrome, floating and
tiling interactions, dock restore, badges, action log, spinner, theming,
scrollable content, time-series chart, and gauges. It is executable
documentation for the shared core interface.

### Visual parity check (web ↔ Bevy)

The web canary is the pixel reference for the Bevy backend.
`tools/visual-parity` captures both and fails on any difference it cannot
explain:

```sh
nix develop -c cargo run -p visual-parity -- check   # exit 0 = parity holds
```

It renders the Bevy workspace example offscreen (1440×900 logical, scale 2,
authored layout, no saved layout read or written), builds the Dioxus
workspace example to wasm, serves it on a loopback port, and captures it in
headless Chrome at the same size with `prefers-reduced-motion: reduce`, so
animations rest at a fixed frame on both sides. It then pairs every panel and
compares frame geometry, border, surface and grip pixels, traffic lights,
every text run (position, width, line height, size, weight, colour), the
Nodes data-table toolbar and header, and the Graph panel (host box, each
node's disc, ink at each edge's midpoint). Tolerances: ±1 px geometry, exact
sRGB for solid fills, ΔE76 < 2 for anti-aliased samples; there are no
exemptions. `target/visual-parity/report.md` holds every row and per-panel
web | Bevy | diff images; `visual-parity compare WEB.png BEVY.png OUT`
re-compares existing captures.

The comparator's own tests run with the workspace suite. The capture needs a
GPU and a local Chrome (`PANEL_KIT_CHROME` overrides the lookup), so it is
not a Nix sandbox check: run it before merging Bevy or web chrome changes.

CI runs the workspace tests and this check on every pull request and push to
main, on a hosted macOS arm64 runner (`.github/workflows/parity.yml`); the
`visual-parity` artifact carries `report.md`, `web.png`, `bevy.png` and the
`*-side.png` images, including when the run fails.

### Topos demos (web, terminal, browser terminal)

The same physics base, two regimes, and two grammars drive all three hosts.
`topos-physics` is a regime-neutral base topos whose stalks are the only authored
physics parameters (repulsion, mass per particle sort; stiffness, rest length per
bond sort). Each regime — `topos-agentic` reads the sessions as
planner/coder/reviewer roles, `topos-membrane` as self-assembling lipid species —
carries a geometric morphism `p_R` into the base, so its physics tuning is the
inverse image `p_R*` and the Physics panel is identical under both regimes. The
morphism `f: topos-agentic → topos-membrane` commutes over the base
(`p_membrane ∘ f = p_agentic`) and is non-injective (planner, reviewer ↦ head),
so the active subobject on the per-sort badges transports by the image `∃f` on
agentic→membrane and the inverse image `f*` on membrane→agentic; the status line
names the morphism used. Toggle a sort by clicking its badge (web), or `s` to
move the focus and space to toggle it (terminals). Specs, grammars, and data come
from Nix (`nix/specs/topos-{agentic,membrane}-subobject.nix`, `nix/specs/topos-physics.nix`,
`nix/specs/morphism-*.nix`, `nix/grammars/`, `nix/data/`) through
`PANEL_KIT_TOPOS_A/B`, `PANEL_KIT_TOPOS_PHYSICS`, `PANEL_KIT_MORPHISM`,
`PANEL_KIT_MORPHISM_A_PHYSICS`, `PANEL_KIT_MORPHISM_B_PHYSICS`,
`PANEL_KIT_GRAMMAR_LINES/JSON`, and `PANEL_KIT_DATA_LINES/JSON`. The base and
morphism specs are surface-neutral; `nix develop` exports the web set and
overrides variables exported before entering it, so terminal surfaces set only
their `PANEL_KIT_TOPOS_A/B` specs inside the shell.

Every panel is a section over the **active subobject** `U` (the selected sorts):
`Γ(U, F)` of the subsite `site.restrict_objects(object_sorting, U)`. Toggling or
transporting `U` re-restricts every table, chart, gauge, meter, assembly and
physics panel — the per-sort badges are the controls and keep listing all sorts.
The **Morphism** panel renders `f` against `U` live (per-sort rows plus the
`∃f`/`∀f`/`f*` summary), and the **Invariants** panel proves, as recomputed
status rows, that the two grammars present one site, that the physics section is
regime-invariant, and that it commutes with restriction to `U`.

What to try: click the **Planner** badge so `U = {planner}` — every panel narrows
to planner sessions. Press `t`: `∃f` widens `U` to `{head}`, and the membrane
panels now cover the planner **and** reviewer objects f merges into head. Press
`t` again: `f*` pulls `U` back to `{planner, reviewer}` — the whole fiber, the
information f could not tell apart. Press `g` to switch grammar: nothing changes,
because `trace-lines` and `trace-json` present the same site.

```sh
dx serve --example topos --platform web                     # web
nix build .#topos-tui-native && ./result/bin/topos_tui        # terminal
nix build .#topos-browser-tui-wasm                          # browser terminal

# browser terminal dev server, from the repo root inside `nix develop`:
export PANEL_KIT_TOPOS_A=$(nix build --no-link --print-out-paths .#topos-agentic-subobject-ascii)
export PANEL_KIT_TOPOS_B=$(nix build --no-link --print-out-paths .#topos-membrane-subobject-ascii)
trunk serve examples/topos_browser_tui.html --example topos_browser_tui --features spec-plan
```

### Trio demo (web, terminal, browser terminal)

One physics base, two sites, three regimes drive all three hosts. `topos-physics`
is a regime-neutral base topos whose stalks are the only authored physics
parameters (repulsion, mass per particle sort; stiffness, rest length per bond
sort). Each regime carries a geometric morphism `p_R` into the base, so its
tuning is the inverse image `p_R*` and is identical in every regime that reads
one site: the Physics panel never moves, and so does the graph's arrangement.
What moves along a morphism is presentation — the sort, and the colour that
sort wears.

`topos-agentic` reads the agent traces as planner/coder/reviewer roles and
memory, `topos-membrane` as the same sessions as self-assembling lipid species,
and `topos-okf-catalog` reads the OKF knowledge catalog as its normalised types
and bundles. The three morphisms `f: agentic → membrane`,
`ψ: okf-catalog → membrane`, and `χ: okf-catalog → agentic` all commute over the
base, and the triangle holds: `f ∘ χ = ψ`. So a regime reads a site exactly when
the site's native regime reaches it along one of them — traces in agentic
(native) and membrane (along `f`); OKF in okf-catalog (native), agentic (along
`χ`), and membrane (along `ψ`). Traces read in okf-catalog has no morphism, and
the header offers it disabled, naming the reason, rather than inventing one.

Physics is the invariant, presentation is not: crossing a morphism transports the
selection by the image `∃` forward and the inverse image `*` back, recolours
every node by the codomain's sorts without moving it, and names the route in the
status line. Switching site or grammar brings different objects, so the graph
re-solves instead. The graph panel is the workspace's declared `graph` external
component: jump-cannon's WebGPU view when the browser has an adapter, and a
force solve in panel-kit's own painters (web SVG, terminal braille) over the same
positions otherwise, so the traces visibly assemble into chains, sheets, tubes,
and closed membranes on every surface.

Specs, grammars, and data come from Nix (`nix/specs/topos-{agentic,membrane,okf-catalog}.nix`,
`nix/specs/topos-physics.nix`, `nix/specs/morphism-*.nix`, `nix/grammars/`,
`nix/data/`) through the `PANEL_KIT_TRIO_*` variables the flake's `trioEnv`
exports. The base and morphism specs are surface-neutral; `nix develop` exports
the web set and overrides variables exported before entering it, so terminal
surfaces set their own trio specs inside the shell.

Every panel is a section over the **active subobject** `U` (the selected sorts):
`Γ(U, F)` of the subsite `site.restrict_objects(object_sorting, U)`. Toggling or
transporting `U` re-restricts every table, chart, gauge, meter, assembly and
physics panel — the per-sort badges are the controls and keep listing all sorts.
The **Morphism** panel renders `f` against `U` live (per-sort rows plus the
`∃f`/`∀f`/`f*` summary), and the **Invariants** panel proves, as recomputed
status rows, that the two grammars present one site, that the physics section is
regime-invariant, and that it commutes with restriction to `U`.

What to try: click the **Planner** badge so `U = {planner}` — every panel narrows
to planner sessions. Press `t`: `∃f` widens `U` to `{head}`, and the membrane
panels now cover the planner **and** reviewer objects f merges into head. Press
`t` again: `f*` pulls `U` back to `{planner, reviewer}` — the whole fiber, the
information f could not tell apart. Press `g` to switch grammar: nothing changes,
because `trace-lines` and `trace-json` present the same site.

```sh
dx serve --example trio --platform web                     # web
nix build .#trio-tui-native && ./result/bin/trio_tui        # terminal
nix build .#trio-browser-tui-wasm                           # browser terminal

# browser terminal dev server, from the repo root inside `nix develop`:
export PANEL_KIT_TRIO_AGENTIC=$(nix build --no-link --print-out-paths .#topos-agentic-ascii)
export PANEL_KIT_TRIO_MEMBRANE=$(nix build --no-link --print-out-paths .#topos-membrane-ascii)
export PANEL_KIT_TRIO_OKF_CATALOG=$(nix build --no-link --print-out-paths .#topos-okf-catalog-ascii)
trunk serve examples/trio_browser_tui.html --example trio_browser_tui --features spec-plan
```

`topos-trio-test` runs the trio host façade (`examples/support/trio_host.rs`) over
the real Nix JSON: every workspace binding of every available (site, regime) pair
resolves, the refused pair is refused by name, the graph's tuning is invariant
across a site's regimes while its sorts and colours move, and a selection
survives the trip along `ψ`.

Note: `Cargo.lock` pins `wasm-bindgen` to the exact version of nixpkgs'
`wasm-bindgen-cli` (dx refuses to bindgen with a mismatched CLI); keep the
two in lockstep when bumping the flake.

### omp auto-loop host (web)

Two regimes over the omp auto-loop's own state: `topos-autoloop-trace` over its
projection and `topos-autoloop-control` over its control site, glued along
`repo` (design: `.specs/design/topos-autoloop-2026-10-04.md`). The host reads
`GET /api/trace` and `GET /api/site` from the loop; a row of an editable control
table opens the edits its topos offers, and writing one POSTs the
`autoloop-control` emitter's bytes to `/api/site`, then reloads. Specs come from
`PANEL_KIT_TOPOS_PHYSICS` and `PANEL_KIT_AUTOLOOP_*`, exported by `nix develop`.

The host calls the loop same-origin, so `dx serve` proxies `/api/` to the loop's
dashboard. A local, uncommitted `Dioxus.toml`:

```toml
[application]

[web.app]
title = "omp auto-loop"

[[web.proxy]]
backend = "http://127.0.0.1:8798/api/"
```

```sh
dx serve --example autoloop --platform web
```

`topos-autoloop-test` runs both regimes and the host façade
(`examples/support/autoloop_host.rs`) over fixtures the loop's producers wrote
(`nix/data/autoloop-*`).

## Developing against a local checkout

In the consuming app's workspace `Cargo.toml`:

```toml
[patch."https://github.com/ocasazza/panel-kit"]
panel-kit = { path = "../../panel-kit" }
```
