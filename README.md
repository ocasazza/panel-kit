# panel-kit

Generic Dioxus panel-workspace library. Every view is a panel you can
move/resize/minimize/maximize, with floating (free placement) and tiling
(auto grid) workspace modes, macOS-style traffic lights, tiling
drag-to-reorder, a minimized-panel dock strip, and layout persistence to
localStorage. Includes a reusable `Badge` chip component, `Spinner`,
panel-header actions, loading-workspace primitives, and a Monaco-based code
editor (`editor::MonacoEditor`) with a `.pest` grammar language and a
palette-matched dark theme.

Factored out of [jump-cannon](https://github.com/ocasazza/jump-cannon) and
apple-notes-ocr-flow, which both consume it as a git dependency:

```toml
[dependencies]
panel-kit = { git = "https://github.com/ocasazza/panel-kit" }
```

## Crates

The repo is a small workspace — one state machine, two renderers:

- **`panel-kit-core`** (`crates/panel-kit-core`) — the renderer-agnostic
  state machine: `PanelWin`, `WinState`, `Mode`, the drag/resize/reorder
  math, viewport clamping, and the persisted `SavedLayout` shape. Pure data
  and math; units are abstract (px on the web, cells in a terminal).
- **`panel-kit`** (repo root) — the Dioxus web shell: signals, DOM events,
  CSS chrome, localStorage persistence. Public API unchanged from before
  the split; existing consumers don't need any changes.
- **`panel-kit-tui`** (`crates/panel-kit-tui`) — the ratatui shell: the same
  workspace drawn in terminal cells with crossterm mouse drag/resize,
  traffic lights that ring on hover, a dock line, and JSON-file persistence. Try it:
  `cargo run -p panel-kit-tui --example workspace`. Via
  [ratzilla](https://github.com/orhun/ratzilla) this renderer can also
  target the browser DOM — one panel codebase, web and terminal skins.

The core crate is the contract: renderer-neutral state, geometry, pointer
events, and persistence shape live there. The Dioxus and ratatui crates are
backends over that same interface; platform events are translated at the
backend boundary.

Panel chrome is intentionally compact across renderers: panel controls and
titles are inset into the top border row, following the ratatui `Block::title`
treatment, instead of using a separate full-width header section. This keeps
more panel height available for content while preserving the same drag,
reorder, minimize, maximize, and mode-toggle controls.

## Usage

The app supplies two things: a `PanelKind` impl (an enum of its panels) and a
body-render callback. Everything else — geometry, z-order, drag state,
viewport clamping, the mobile breakpoint, persistence — lives here.

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum Panel { Graph, Inspector }
impl panel_kit::PanelKind for Panel {
    fn title(self) -> &'static str { /* … */ }
}

let ws = panel_kit::use_workspace("myapp_layout", default_layout);
rsx! {
    style { {panel_kit::CSS} }
    div { class: ws.root_class(),
        onmousemove: move |e| ws.handle_mouse_move(&e),
        onmouseup: move |_| ws.handle_mouse_up(),
        header { class: "topbar", /* app-specific */ }
        {ws.render(|kind, maximized| rsx! { /* panel body for `kind` */ })}
        {ws.dock()}
    }
}
```

Inject `panel_kit::CSS` once at the app root, then layer app-specific styles
after it; override the `:root` variables to retheme.

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
  phases such as graph fetches or GPU initialization.

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

## Documentation

API docs are rustdoc-first — the crate root has a quick start, theming
notes, and a guide to the examples:

```sh
cargo doc --no-deps --open
```

(Once published to crates.io, the same docs will be on docs.rs, built for
`wasm32-unknown-unknown`.)

## Examples

One browser demo per component, each exercising every public parameter.
From `nix develop` (which provides a matching dioxus-cli 0.6.x, lld, and
wasm-bindgen-cli):

```sh
dx serve --example workspace --platform web
```

| example | shows |
| --- | --- |
| `workspace` | the whole workspace system: `use_workspace` + `PanelKind` + `LayoutBuilder`, floating mode (drag/resize/z-raise/traffic lights), tiling mode (red-light toggle, drag-header reorder, full-width panel via the `panel-<slug>` class), a panel that starts minimized in the dock, viewport clamping vs. stored geometry, localStorage persistence (`panel_kit_example_workspace`), the mobile stack, the `is_editing` shortcut gate, and a `tip_pos` tooltip overlay |
| `views` | named views over one workspace: `use_views`, per-view persistence keys (`panel_kit_example_views:view:<name>` + the `:views` registry), a switcher bar with create/rename/delete, per-view reset, and the legacy single-layout migration |
| `badge` | all ten `BadgeKind`s, every prop (`active`, `with_x`, `with_plus`, `small`, `override_color`, `accent_color`, both `BadgeClickKind`s, `emit_hover`) behind live toggles, an event log proving every `BadgeAction` variant fires, and a `tag_hue` FNV hue-spread row |
| `spinner` | `Spinner` with and without `label`, plus a live-editable label |
| `loading_workspace` | the post-mount `LoadingWorkspace` twin of the static pre-WASM boot contract |
| `theming` | the documented retheme path: `:root` variable overrides layered after `panel_kit::CSS`, with three switchable presets |
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
