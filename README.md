# panel-kit

Generic Dioxus panel-workspace library. Every view is a panel you can
move/resize/minimize/maximize, with floating (free placement) and tiling
(auto grid) workspace modes, printer-CMY operation lights, pointer and
keyboard window management, tiling drag-to-reorder, a minimized-panel dock,
and versioned layout persistence. Includes a reusable `Badge` chip component,
`Spinner`, panel-header actions, loading-workspace primitives, and a
Monaco-based code editor (`editor::MonacoEditor`) with a `.pest` grammar
language and a token-matched dark theme.

Factored out of [jump-cannon](https://github.com/ocasazza/jump-cannon) and
apple-notes-ocr-flow, which both consume it as a git dependency:

```toml
[dependencies]
panel-kit = { git = "https://github.com/ocasazza/panel-kit" }
```

## Crates

The repo is a small workspace — one state machine, two renderers:

- **`panel-kit-core`** (`crates/panel-kit-core`) — the renderer-agnostic
  state machine: `PanelWin`, `WinState`, `Mode`, `SurfaceProfile`, keyboard
  commands, drag/resize/reorder math, viewport clamping, and the versioned
  `SavedLayoutV2` persistence shape. Units are abstract (CSS px on the web,
  cells in a terminal).
- **`panel-kit`** (repo root) — the Dioxus web shell: signals, pointer and
  keyboard event translation, accessible DOM chrome, and V2 localStorage
  persistence with automatic V1 migration.
- **`panel-kit-tui`** (`crates/panel-kit-tui`) — the ratatui shell: the same
  workspace drawn in terminal cells with crossterm input, printer-CMY
  controls, a dock line, and JSON-file persistence. Try it:
  `cargo run -p panel-kit-tui --example workspace`. Via
  [ratzilla](https://github.com/orhun/ratzilla) this renderer can also
  target the browser DOM — one panel codebase, web and terminal skins.

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

The app supplies two things: a `PanelKind` impl (an enum of its panels) and a
body-render callback. Everything else — geometry, z-order, drag state,
viewport clamping, surface classification, keyboard commands, and
persistence — lives here.

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum Panel { Graph, Inspector }
impl panel_kit::PanelKind for Panel {
    fn title(self) -> &'static str { /* … */ }
}

let ws = panel_kit::use_workspace("myapp_layout", default_layout);
rsx! {
    style { {panel_kit::CSS} }
    div { class: ws.root_class(), tabindex: "0",
        onpointermove: move |event| ws.handle_pointer_move(&event),
        onpointerup: move |event| ws.handle_pointer_up(&event),
        onkeydown: move |event| ws.handle_key(&event),
        header { class: "topbar", /* app-specific */ }
        {ws.render(|kind, maximized| rsx! { /* panel body for `kind` */ })}
        {ws.dock()}
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

`Workspace::surface_profile()` reports `Compact`, `Tablet`, or `Regular`
plus coarse-pointer, hover, and keyboard capabilities. The web defaults are
`<760px`, `<1180px`, and everything wider. Compact surfaces force tiling and
withhold the affordances that move or resize a panel by hand; tablet and
regular surfaces keep move, resize, minimize, and maximize behavior.
`root_class()` emits exactly one of `compact`, `tablet`, or `regular`
alongside `ws-root`, plus `coarse` when the primary pointer is imprecise.

Two rules follow from that, and both are load-bearing:

- **Sizing follows the pointer, not the width.** `--hit-min` is `24px` by
  default and `44px` under `.ws-root.coarse`. A 1400px kiosk touchscreen is a
  regular surface that still needs 44px targets; a 900px mouse-driven window
  does not.
- **A tier never traps a window state.** Layouts persist across surfaces, so a
  panel maximized on a desktop arrives maximized on a phone. Compact
  therefore still renders the magenta restore light — and only that one — for
  a maximized panel.

### Keyboard window management

Pass the root `onkeydown` event to `Workspace::handle_key`. Arrow keys move
the focused panel, Shift+arrows resize, and Alt+arrows fine-move. `m`, `f`,
`t`, and Escape minimize, maximize, toggle mode, and restore; Tab and
Shift+Tab cycle focus, while Enter raises. Bare shortcuts are ignored while
a text input owns focus. The focused kind is available through
`Workspace::focused()` and the public `focused` signal.

### Versioned persistence

Web workspaces now write `SavedLayoutV2` with schema version, `Units::CssPx`,
capture viewport, mode, and panels. Existing `{ panels, tiling }` V1 records
remain readable and are migrated automatically. Core exposes `StoredLayout`,
`migrate_v1`, and `reconcile_units` so other renderers can perform the same
upgrade and rescale foreign viewport/unit geometry. Named views persist
through the same reader and writer, one V2 record per view.

### Named views

For several named, switchable layouts inside one workspace, `use_views`
layers a view registry over the same machinery — one `Workspace`, one
`render`/`dock` pair, a storage key per view:

```rust
let views = panel_kit::use_views("myapp_layout", default_layout, &["User", "Sessions"]);
let ws = views.workspace;
// views.names / views.active are signals; views.switch(name),
// views.create(name), views.rename(old, new), views.delete(name)
// manage the registry.
```

The registry persists at `myapp_layout:views`, each view's layout at
`myapp_layout:view:<name>`; a pre-views layout at the bare base key migrates
into the first view on first run (copied, never deleted). See the `views`
example for a full switcher UI — the hook ships no UI of its own.

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
| `workspace` | `use_workspace` + `PanelKind` + `LayoutBuilder`; floating pointer and keyboard move/resize/raise; blue mode, yellow minimize, and pink maximize/restore controls; tiling reorder and span resize; a restore-by-kind control; viewport clamping; inner-panel and workspace wheel chaining; live compact/tablet/regular, focus, `tile_w`/`tile_h`, and `ws_scroll` state; versioned localStorage persistence; and a `tip_pos` overlay |
| `views` | named views over one workspace: `use_views`, per-view persistence keys (`panel_kit_example_views:view:<name>` + the `:views` registry), a switcher bar with create/rename/delete, per-view reset, and the legacy single-layout migration |
| `badge` | all ten `BadgeKind`s, every prop (`active`, `with_x`, `with_plus`, `small`, `override_color`, `accent_color`, both `BadgeClickKind`s, `emit_hover`) behind live toggles, an event log proving every `BadgeAction` variant fires, and a `tag_hue` FNV hue-spread row |
| `spinner` | `Spinner` with and without `label`, plus a live-editable label |
| `loading_workspace` | the post-mount `LoadingWorkspace` twin of the static pre-WASM boot contract |
| `theming` | the documented full-palette retheme path: `:root` variable overrides layered after `panel_kit::CSS`, with three switchable presets |
| `editor` | `editor::MonacoEditor`: two-way `Signal<String>` binding, `on_change` event log, imperative `EditorHandle` (set/read value, language, read-only, layout), reactive `language`/`read_only` props, the `pest` Monarch tokenizer and minimal `toml` language on real samples |

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
trunk serve crates/panel-kit-tui/browser_tui.html \
  --example browser_tui \
  --address 127.0.0.1 \
  --port 8082
```

This example is intentionally comprehensive: workspace chrome, floating and
tiling interactions, dock restore, badges, action log, spinner, theming,
scrollable content, time-series chart, and gauges. It is executable
documentation for the shared core interface.

Note: `Cargo.lock` pins `wasm-bindgen` to the exact version of nixpkgs'
`wasm-bindgen-cli` (dx refuses to bindgen with a mismatched CLI); keep the
two in lockstep when bumping the flake.

## Developing against a local checkout

In the consuming app's workspace `Cargo.toml`:

```toml
[patch."https://github.com/ocasazza/panel-kit"]
panel-kit = { path = "../../panel-kit" }
```
