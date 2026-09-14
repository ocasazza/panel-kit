# Migrating from 0.2.x to 1.0.0

panel-kit 1.0.0 makes the shared core authoritative for surface classification, keyboard commands, pointer semantics, persistence units, and terminal tile rows. The public-API breaks below are intentional clean cutovers; there are no deprecated aliases. Git-pinned consumers remain on their current revision until they update the pin.

## Update dependency versions

**What changed:** all three crates and the internal `panel-kit-core` requirements are `1.0.0`.

**Why:** this is the first stable contract for the unified web and terminal window manager, and the clean-cutover API changes require a major version.

Before:

```toml
panel-kit = { version = "0.2", git = "https://github.com/ocasazza/panel-kit", rev = "OLD_REV" }
```

After migrating the code in this guide, update the pin:

```toml
panel-kit = { version = "1.0.0", git = "https://github.com/ocasazza/panel-kit", rev = "1.0_REV" }
```

Use the equivalent edit for direct `panel-kit-core` or `panel-kit-tui` dependencies.

## Replace mobile detection with a surface profile

**What changed:** the web-only `viewport_is_mobile()` function and `Workspace::is_mobile` signal were removed. `Workspace` now exposes `profile: Signal<SurfaceProfile>` and the `surface_profile() -> SurfaceProfile` accessor. `SurfaceProfile` contains a `SurfaceClass` (`Compact`, `Tablet`, or `Regular`) and `SurfaceCapabilities` (`coarse_pointer`, `hover`, and `keyboard`). Web thresholds are 760 and 1180 CSS px; terminal thresholds are 60 and 110 cells. Core also exposes `effective_mode(preferred, &profile)`.

**Why:** width alone did not describe touch, hover, or keyboard input, and a web-named boolean could not be shared with terminal, tablet, native, and embedded renderers. Compact forces tiling; tablet and regular retain window management.

Before:

```rust
if *ws.is_mobile.read() || panel_kit::viewport_is_mobile() {
    // narrow, static shell
}
```

After:

```rust
use panel_kit::SurfaceClass;

let profile = ws.surface_profile();
match profile.class {
    SurfaceClass::Compact => { /* forced tiling, no window-management chrome */ }
    SurfaceClass::Tablet => { /* window management with larger hit targets */ }
    SurfaceClass::Regular => { /* full workspace */ }
}
let has_hover = profile.caps.hover;
```

Core-only renderers construct their own profile in renderer units:

```rust
let profile = SurfaceProfile::from_logical_width(
    width_in_cells,
    CELLS_COMPACT_MAX,
    CELLS_TABLET_MAX,
    capabilities,
);
let mode = effective_mode(preferred_mode, &profile);
```

## Rename web mouse entry points to pointer entry points

**What changed:** the Dioxus renderer now accepts `dioxus::events::PointerEvent` and removed the mouse-named methods without aliases.

| 0.2.x | 1.0.0 |
| --- | --- |
| `begin_drag(idx, kind, &MouseEvent)` | `begin_pointer_drag(idx, kind, &PointerEvent)` |
| `begin_tile_resize(idx, &MouseEvent)` | `begin_pointer_tile_resize(idx, &PointerEvent)` |
| `handle_mouse_move(&MouseEvent)` | `handle_pointer_move(&PointerEvent)` |
| `handle_mouse_up()` | `handle_pointer_up(&PointerEvent)` |

**Why:** a web workspace must accept mouse, pen, and touch through one event path, while core retains renderer-neutral `PointerEvent` semantics.

Before:

```rust
onmousemove: move |event| ws.handle_mouse_move(&event),
onmouseup: move |_| ws.handle_mouse_up(),
```

After:

```rust
onpointermove: move |event| ws.handle_pointer_move(&event),
onpointerup: move |event| ws.handle_pointer_up(&event),
```

Consumers with custom panel chrome must make the corresponding `begin_pointer_drag` and `begin_pointer_tile_resize` substitutions.

## Route keyboard commands through Workspace

**What changed:** web `Workspace` adds `handle_key(&KeyboardEvent)`, `focused: Signal<Option<K>>`, and `focused() -> Option<K>`. Core now owns `FocusContext`, `Key`, `KeyChord`, `PanelCommand`, `CommandStep`, `command_for`, and `apply_command`. The terminal renderer adds `TuiWorkspace::handle_key(chord, focus) -> bool` and a public `focused: Option<K>`.

**Why:** applications previously implemented partial, drifting shortcuts. One core command table now gives every renderer the same move, resize, minimize, maximize, restore, mode, raise, and focus-cycle behavior, while text inputs retain bare keys.

Before:

```rust
onkeydown: move |event| {
    if !panel_kit::is_editing() && event.key() == Key::Character("t".into()) {
        ws.mode.set(/* locally toggle the mode */);
    }
},
```

After:

```rust
onkeydown: move |event| ws.handle_key(&event),

// Optional status or app-specific focus UI:
let focused_panel = ws.focused();
```

The built-in bindings are arrows to move, Shift+arrows to resize, Alt+arrows to fine-move, `m` minimize, `f` maximize, `t` toggle mode, Escape restore, Tab/Shift+Tab cycle focus, and Enter raise. The web adapter selects `FocusContext::TextInput` automatically for an active input or textarea. A terminal event loop passes its context explicitly:

```rust
let handled = ws.handle_key(chord, FocusContext::Panel(panel_kind));
```

## Update responsive CSS selectors

**What changed:** the root class `mobile` was removed. `Workspace::root_class()` now emits `ws-root` plus exactly one of `compact`, `tablet`, or `regular`, and may additionally emit `dragging`. The focused panel is marked `.panel.focused`.

**Why:** consumers need three capability-aware density tiers rather than a phone/not-phone split, and keyboard focus must be visible independently of pointer drag state.

Before:

```css
.ws-root.mobile .app-toolbar { display: none; }
```

After:

```css
.ws-root.compact .app-toolbar { display: none; }
.ws-root.tablet .app-toolbar { min-height: var(--hit-min); }
.panel.focused .app-focus-extra { outline: 1px solid var(--focus-ring); }
```

Audit every consumer stylesheet for `.mobile`; the library no longer emits it.

## Migrate persisted layouts to schema V2

**What changed:** `SavedLayout<K>` remains public and deserializable only as the legacy V1 `{ panels, tiling }` reader shape. New writes use `SavedLayoutV2<K>`:

```rust
SavedLayoutV2 {
    version: LAYOUT_SCHEMA_VERSION,
    units: Units::CssPx, // or Units::Cells
    viewport: (width, height),
    mode,
    panels,
}
```

`StoredLayout<K>` is an untagged reader with V2 attempted before V1. `migrate_v1(old, units, viewport)` tags a V1 record; `reconcile_units(layout, to_units, viewport)` rescales floating geometry from a foreign unit space or capture viewport.

**Why:** `{ panels, tiling }` did not identify its schema, coordinate units, or capture viewport, so geometry could not move safely between CSS pixels, terminal cells, or differently sized surfaces.

Before, for custom persistence:

```rust
let old: SavedLayout<Panel> = serde_json::from_str(&json)?;
serde_json::to_string(&SavedLayout {
    panels,
    tiling: mode == Mode::Tiling,
})?;
```

After:

```rust
let stored: StoredLayout<Panel> = serde_json::from_str(&json)?;
let layout = match stored {
    StoredLayout::V1(old) => migrate_v1(old, Units::CssPx, viewport),
    StoredLayout::V2(v2) => reconcile_units(v2, Units::CssPx, viewport),
};
serde_json::to_string(&SavedLayoutV2 {
    version: LAYOUT_SCHEMA_VERSION,
    units: Units::CssPx,
    viewport,
    mode: layout.mode,
    panels: layout.panels,
})?;
```

Web consumers using `use_workspace` need no storage code change: it automatically reads V1 or V2 and writes only V2. `TuiWorkspace` does the same with `Units::Cells`; its native `new(Option<PathBuf>, defaults)` constructor remains available.

## Update Nix mkLayout calls

**What changed:** the flake's public `mkLayout` input changed from
`{ tiling ? false, panels }` to
`{ units, viewport, mode ? "Floating", panels }`. It now emits the V2
`version`, `units`, `viewport`, and `mode` fields. The flake also exports
`modes`, `unitKinds`, and `schemaVersion` alongside the existing helpers.

**Why:** the declarative Nix writer must not keep producing ambiguous V1
records after the Rust renderers move to versioned, unit-aware persistence.

Before:

```nix
layout = panel-kit.lib.${system}.mkLayout {
  tiling = false;
  panels = [ /* CSS-pixel or cell geometry was indistinguishable */ ];
};
```

After for a terminal-authored layout:

```nix
layout = panel-kit.lib.${system}.mkLayout {
  units = "Cells";
  viewport = [ 128.0 40.0 ];
  mode = "Floating";
  panels = [ /* geometry captured against that viewport */ ];
};
```

Use `units = "CssPx"` and the authoring browser viewport for a web layout.
The generated `.value` and `.json` now match `SavedLayoutV2` directly.

## Use LayoutStore for non-file TUI persistence

**What changed:** `panel-kit-tui` adds `LayoutStore`:

```rust
pub trait LayoutStore {
    fn load(&self) -> Result<Option<String>, String>;
    fn save(&self, json: &str) -> Result<(), String>;
}
```

Construct a workspace with `TuiWorkspace::with_store(Box<dyn LayoutStore>, defaults)` when the transport is not a native file.

**Why:** the workspace must own V1/V2 parsing and unit reconciliation, while native files, browser localStorage, and future backends should only transport opaque JSON.

Before, a browser TUI had no supported persistence constructor and had to keep storage outside `TuiWorkspace`.

After:

```rust
let ws = TuiWorkspace::with_store(Box::new(BrowserStore::new("app-layout")), defaults);
```

Implement `load` and `save` on `BrowserStore`; do not duplicate the layout schema in the transport.

## Update theme tokens and panel depth

**What changed:** default `--line2` changed from `#3a3a3a` to `#5f5f5f`, reaching 3.0437:1 against `--panel`. New root tokens are `--focus-ring: var(--fg)`, `--hit-min: 24px`, `--panel-head-h: 22px`, and `--badge-info: #83b7cc`. The resting `.panel` shadow was removed. `Theme::DARK.line2` in the terminal renderer mirrors `Rgb(95, 95, 95)`.

**Why:** `--line2` bounds objects and must remain visible; focus rings, minimum targets, header height, and informational badges must be rethemeable; resting shadows violate flat web/terminal parity.

Before:

```css
:root {
  --line2: #3a3a3a;
}
.panel { box-shadow: 0 6px 24px #0007; }
.badge { --badge-c: #83b7cc; }
```

After:

```css
:root {
  --line2: #5f5f5f;
  --focus-ring: var(--fg);
  --hit-min: 24px;
  --panel-head-h: 22px;
  --badge-info: #83b7cc;
}
.panel { box-shadow: none; }
```

If a consumer overrides the complete palette, add all new tokens and verify `--line2` at 3:1 against its panel. Remove consumer shadows that were compensating for the old panel elevation; use the edge and `.panel.focused` state instead.

## Update TUI semantic color assumptions

**What changed:** TUI tags now use `badge_info`, dock chips and the first
categorical chart series use `fg`, scroll thumbs use `line2`, and healthy
gauges use `green`. `accent` remains reserved for live/transient signals:
dragging, hovered grips, and spinners.

**Why:** the 1.0 palette gives each saturated color one meaning and keeps the
phosphor accent rare. Static tags and chart series are not live activity.

Before, app-owned drawing that attempted to match 0.2 chrome:

```rust
let tag_color = theme.accent;
let first_series_color = theme.accent;
let healthy_color = theme.accent;
```

After:

```rust
let tag_color = theme.badge_info;
let first_series_color = theme.fg;
let healthy_color = theme.green;
let scroll_thumb_color = theme.line2;
```

Update visual snapshots and any custom widgets that mirror these semantics;
the `Theme` field names themselves remain stable.

## Pass renderer-neutral RGB values to TUI drawing helpers

**What changed:** `charts::FlameSpan::color` and `charts::BoxItem::color`
changed from `Option<ratatui::style::Color>` to
`Option<panel_kit_core::badge::Rgb>`. `meter::span` and
`status::{dot, style, labeled}` likewise accept `Rgb`. The internal
`charts::series_colors` helper is no longer public.

**Why:** semantic data should carry a renderer-neutral RGB triple; conversion
to ratatui's drawing type belongs inside the TUI backend. A consumer's custom
chart palette is application policy, not panel-kit state.

Before:

```rust
use ratatui::style::Color;

let span = FlameSpan {
    label: \"parse\".into(),
    depth: 1,
    value: 24.0,
    color: Some(Color::Rgb(31, 94, 194)),
};
let status = panel_kit_tui::status::labeled(Color::Rgb(39, 201, 63), \"ready\");
let palette = panel_kit_tui::charts::series_colors(&theme);
```

After:

```rust
let span = FlameSpan {
    label: \"parse\".into(),
    depth: 1,
    value: 24.0,
    color: Some((31, 94, 194)),
};
let status = panel_kit_tui::status::labeled((39, 201, 63), \"ready\");
let palette = [theme.fg, theme.blue, theme.pink]; // app-owned custom palette
```

## Import terminal pointer types from core

**What changed:** the 0.2 names `TuiMouseButton`, `TuiMouseEventKind`, and
`TuiMouseEvent` were aliases for core types and are removed. Import
`PointerButton`, `PointerEventKind`, and `PointerEvent` from
`panel-kit_core` directly. Their variants and fields, and
`TuiWorkspace::handle_mouse(&mut self, PointerEvent)`, are unchanged.

**Why:** renderer-neutral input types belong to core; TUI-prefixed aliases
created a second name for the same contract and obscured cross-renderer code.

Before:

```rust
use panel_kit_tui::{TuiMouseButton, TuiMouseEvent, TuiMouseEventKind};

ws.handle_mouse(TuiMouseEvent {
    kind: TuiMouseEventKind::Down(TuiMouseButton::Primary),
    x,
    y,
});
```

After:

```rust
use panel_kit_core::{PointerButton, PointerEvent, PointerEventKind};

ws.handle_mouse(PointerEvent {
    kind: PointerEventKind::Down(PointerButton::Primary),
    x,
    y,
});
```

## Read terminal tile rows from TileMetrics

**What changed:** `TileMetrics::CELLS.row` is now `4.0` and is the authority for terminal `tile_h`. The TUI renderer deleted its local `ROW_CELLS` constant.

**Why:** core previously said six cells while the terminal actually rendered four, allowing persisted spans and custom renderers to disagree.

Before:

```rust
const ROW_CELLS: f64 = 4.0;
let height = panel.tile_h as f64 * ROW_CELLS;
```

After:

```rust
let height = panel.tile_h as f64 * TileMetrics::CELLS.row;
```

Do not copy the value into a backend-local constant.

## Expect a CSS grid in tiling mode

**What changed:** `.ws.tiling` is a CSS grid instead of a wrapping flex container. Columns are `repeat(var(--tile-cols), minmax(0, 1fr))` and rows are `grid-auto-rows: minmax(var(--tile-row-min), 1fr)`. A tiled panel now receives inline `grid-column: span <tile_w>; grid-row: span <tile_h>;` instead of `flex` and `height`. `--tile-cols` is written inline on `.ws` from the new `SurfaceProfile::tile_columns()` (1 compact, 2 tablet, `TILE_W_MAX` regular), and the renderer clamps each emitted span to it.

**Why:** `tile_w` / `tile_h` are spans, and wrapping flex could not honour a span while also dividing a bounded height among rows. The flex line was sized by its content, so a panel whose body overflowed grew its row instead of scrolling inside it — measured at 588px of workspace overflow — while an under-filled workspace stranded ~380px of dead space below the tiles. With the grid, rows are sized by the container, so a tile may be shorter than its content and `.panel-body` scrolls; the workspace fills exactly when the tiles fit and scrolls only once `--tile-row-min` is reached.

Span clamping is not cosmetic: a DOM grid answers a span wider than its column count by manufacturing zero-width implicit columns, which hides the panel completely. A four-wide tile on the two-column tablet tier collapses to full width instead of disappearing.

**Consumer impact:** none, unless your app styles `.ws.tiling` or `.ws.tiling .panel` directly.

Before, a consumer override that assumed flex:

```css
/* no longer has any effect — the container is not a flex container */
.ws.tiling .panel.panel-inspector { flex: 1 1 100%; height: 420px; }
```

After, express the same intent as spans and let the grid size the row:

```css
.ws.tiling .panel.panel-inspector { grid-column: 1 / -1; }
```

Do not reintroduce a fixed `height` on a tiled panel: it either strands dead space or overflows the workspace, and it defeats the panel body's own scrolling. If a panel needs more room, give it a larger `tile_h` span.

## Key touch sizing to the pointer, not the tier

**What changed:** `Workspace::root_class()` now also emits `coarse` when the detected `SurfaceCapabilities::coarse_pointer` is true, so the full set is `ws-root` + one of `compact`/`tablet`/`regular` + optional `coarse` + optional `dragging`. `--hit-min` is `24px` by default and `44px` under `.ws-root.coarse`. It is no longer raised by the `compact` or `tablet` tier.

**Why:** width describes how much room a surface has, not how precisely that room can be hit. The tier-keyed rule was wrong at both ends: a 1400px kiosk touchscreen classifies as `regular` and was getting 24px targets, while a 900px desktop window driven by a mouse was getting 44px. `SurfaceCapabilities` was already being populated from `matchMedia('(pointer: coarse)')` and read by nothing; it now does the job it was detected for.

**Consumer impact:** only if your CSS keyed sizing off the tier.

Before:

```css
.ws-root.compact .my-toolbar-button,
.ws-root.tablet  .my-toolbar-button { min-height: 44px; }
```

After — inherit the token, or key off `coarse` directly:

```css
.my-toolbar-button { min-height: var(--hit-min); }
```

## Expect a restore control on compact surfaces

**What changed:** core adds `SurfaceProfile::must_offer_restore(WinState) -> bool`. Both renderers now draw the magenta restore light — and only that one — for a maximized panel on a surface that otherwise withholds window management. The CSS rules `.compact .lights { display:none }` and `.compact .resize { display:none }` were removed; the renderer is the single authority for which chrome exists.

**Why:** this fixed a reachable trap, not a style preference. `SavedLayoutV2` carries `WinState` across surfaces on purpose, so maximizing a panel on a desktop and reopening the layout on a phone delivered a `Maximized` panel to a tier with no lights, no grip, and an inert `max-hint` span. Measured: one visible panel, four unreachable siblings, and no library affordance to undo it — escapable only with a physical keyboard (`Escape`), an app-specific reset button, or clearing storage.

The rule is now explicit: a tier may withhold the means to *enter* a window state, never the means to *leave* one. Minimized panels already escape through the dock, so only `Maximized` needs the override.

**Consumer impact:** none in code. Expect one tappable light to appear on compact when a panel is maximized, and remove any consumer CSS that hides `.lights` or `.resize` by tier — it will now hide that escape hatch too:

```css
/* Delete rules like this; the renderer already withholds what the tier withholds. */
.ws-root.compact .lights { display: none; }
```

## Read colours from canonical tokens

**What changed:** `panel_kit_core::tokens` is the authoritative palette and
monospace source. Each `Token` carries `name`, `hex`, and `rgb`; the module
exports `BG`, `PANEL`, `FG`, `DIM`, `LINE`, `LINE2`, `INV_BG`, `INV_FG`,
`ACCENT`, `RED`, `YELLOW`, `GREEN`, `BLUE`, `PINK`, `BADGE_INFO`, the
`tokens::DARK` slice, `tokens::MONO`, and `tokens::by_name`. The CSS-facing
surface remains `assets/panel-kit.css`, and a test now parses its declarations
and pins every colour to the canonical token.

**Why:** the palette previously had three hand-maintained copies, and the boot
copy had already drifted by seven near-miss values: `#09090a` instead of
`#0a0a0a`, `#efeff0` instead of `#ededed`, `#242427` instead of `#262626`,
`#77777d` instead of `#7a7a7a`, `#389cf6` instead of `#3b9bff`, `#f6be3c`
instead of `#ffbd2e`, and `#ee60c2` instead of `#ff5fc3`. It also carried a
different font stack. A renderer or integration can now consume one Rust
source instead of relying on visually plausible copies.

**Consumer impact:** native renderers, generated stylesheets, and app-owned
themes should read `panel_kit_core::tokens`; web CSS overrides still target
the custom properties declared by `panel_kit::CSS`.

Before:

```rust
const APP_ACCENT_HEX: &str = "#5ef38c";
const APP_ACCENT_RGB: (u8, u8, u8) = (0x5e, 0xf3, 0x8c);
```

After:

```rust
use panel_kit_core::tokens;

let accent_hex = tokens::ACCENT.hex;
let accent_rgb = tokens::ACCENT.rgb;
```

## Edit the boot stylesheet template

**What changed:** the hand-maintained `assets/panel-kit-boot.css` was deleted.
The source is now `assets/panel-kit-boot.css.in`; the root `build.rs`
substitutes canonical palette and font tokens, writes the rendered stylesheet
to `OUT_DIR`, and `panel_kit::BOOT_CSS` includes that generated file. An
unknown template placeholder deliberately fails the build rather than shipping
unresolved text.

**Why:** the pre-WASM shell must paint before it can read the injected
stylesheet, but that timing requirement does not justify another
hand-maintained palette. Generation keeps the independent asset synchronized
with the Rust source of truth. Its traffic-light decoration is now three
discrete dots drawn with `box-shadow`, not a fake gradient.

**Consumer impact:** if you vendored or patched the deleted stylesheet, stop
editing it and make rule changes in `assets/panel-kit-boot.css.in`. If you
copied the old asset path into an app, consume `panel_kit::BOOT_CSS` from the
crate instead.

Before:

```rust
let boot_css = include_str!("../vendor/panel-kit/assets/panel-kit-boot.css");
```

After:

```rust
let boot_css = panel_kit::BOOT_CSS;
```

## Let named views migrate per-view layouts

**What changed:** every per-view record managed by `use_views` now passes
through the same `StoredLayout` reader as `use_workspace`, so a pre-1.0 V1
record migrates to V2 automatically. The internal `save_layout` and
`load_layout` helpers gained a `viewport` parameter and are now `pub(crate)`.

**Why:** named views must not preserve an unversioned persistence path beside
the V2 workspace path. Routing both through `StoredLayout` applies the same V1
migration and viewport reconciliation to every record. The behavior was
verified with a seeded V1 per-view record that returned as V2.

**Consumer impact:** this is internal plumbing, not a consumer API change;
continue calling `use_views` normally.

Before, the internal helpers had no viewport context:

```rust
save_layout(key, &panels, mode);
let saved = load_layout(key, &defaults);
```

After, `use_views` supplies the current viewport:

```rust
save_layout(key, &panels, mode, viewport);
let saved = load_layout(key, &defaults, viewport);
```

## Derive the terminal dark theme from tokens

**What changed:** `Theme::DARK` is derived from `panel_kit_core::tokens`
through `const fn rgb(Token)`; the dark preset contains no copied colour
literals.

**Why:** a literal terminal table was another place for the shared design to
drift. `Theme::PAPER` remains hand-authored and unchanged, with verified
ratios of `line2/panel` 3.4763:1, `dim/bg` 5.0071:1, and `dim/panel`
5.3679:1.

**Consumer impact:** if an app constructed a `Theme` by copying the dark hex
values, read the corresponding core tokens instead. Existing
`Theme::DARK` users need no code change.

Before:

```rust
let accent = ratatui::style::Color::Rgb(0x5e, 0xf3, 0x8c);
```

After:

```rust
use panel_kit_core::tokens;

let (r, g, b) = tokens::ACCENT.rgb;
let accent = ratatui::style::Color::Rgb(r, g, b);
```

## Supply canonical tokens to Monaco

**What changed:** the embedded editor no longer selects Monaco's `vs-dark`
theme or maintains a JavaScript palette table. Rust supplies
`tokens::DARK` and `tokens::MONO` to the loader, which builds the
non-inheriting `panel-kit-dark` theme. The editor uses the canonical monospace
stack at 13px with a 19.5px line height, and accent is limited to the cursor
and active or focused affordances.

**Why:** an embedded editor is still part of the same console. Live
verification confirmed that its `#0d0d0d` background equals `--panel` and its
`#5ef38c` cursor equals `--accent`; a built-in Monaco palette or copied JS
table could not guarantee either invariant.

**Consumer impact:** consumers that register or override a Monaco theme should
supply values read from `panel_kit_core::tokens`, rather than copying the dark
palette into JavaScript.

Before:

```rust
let editor_theme = "vs-dark";
```

After:

```rust
use panel_kit::editor::PANEL_KIT_DARK_THEME;
use panel_kit_core::tokens;

let editor_theme = PANEL_KIT_DARK_THEME;
let editor_background = tokens::PANEL.hex;
let editor_cursor = tokens::ACCENT.hex;
```

## Keep loading surfaces inside the workspace root

**What changed:** `.pk-progress-track` now uses `var(--bg)` instead of the
translucent `rgba(255,255,255,.06)`. Under reduced motion, an indeterminate
fill no longer becomes a full-width, half-opacity bar that falsely reads as
complete; it remains a static 40% sliver with `transform:none`. Four
`.ws-root.compact` rules reflow the global bar, its header, label, and detail
at phone width.

**Why:** translucency is a surface treatment a terminal cell grid cannot
express, and a full bar communicates completion regardless of its opacity.
Loading leaves deliberately inherit the tier from their `.ws-root` ancestor
instead of accepting a second `SurfaceProfile` prop, so there is only one
surface decision to keep correct.

**Consumer impact:** `GlobalLoadingBar` must stay inside the element carrying
`Workspace::root_class()` so it inherits compact, tablet, or regular styling.

Before:

```css
.pk-progress-track { background: rgba(255,255,255,.06); }
@media (prefers-reduced-motion: reduce) {
  .pk-progress-fill.indeterminate { width:100%; opacity:.5; }
}
```

After:

```css
.pk-progress-track { background:var(--bg); }
@media (prefers-reduced-motion: reduce) {
  .pk-progress-fill.indeterminate { animation:none; width:40%; transform:none; }
}
```

## Consumer notice

**Title:** Migrate pinned panel-kit dependency to 1.0.0

**Body:**

panel-kit 1.0.0 is available with a stable shared web/TUI contract. This is an intentionally breaking release. `ocasazza/jump-cannon` and `olivecasazza/apple-notes-ocr-flow` remain safely on their currently pinned git revisions until each repository completes the migration and updates its pin; no automatic upgrade is required.

Required edits before moving the pin:

- [ ] Replace `viewport_is_mobile()` / `Workspace::is_mobile` with `SurfaceProfile` and handle compact, tablet, and regular tiers.
- [ ] Rename Dioxus mouse handlers to `begin_pointer_drag`, `begin_pointer_tile_resize`, `handle_pointer_move`, and `handle_pointer_up`, and use pointer events.
- [ ] Route root keyboard events through `Workspace::handle_key`; account for the new `focused` signal where app focus UI is rendered.
- [ ] Add the new `--focus-ring`, `--hit-min`, `--panel-head-h`, and `--badge-info` theme tokens; update `--line2` and remove resting panel shadows.
- [ ] Read app-owned palette colours from `panel_kit_core::tokens` instead of copying hex values.
- [ ] Stop vendoring or patching `assets/panel-kit-boot.css`; edit `assets/panel-kit-boot.css.in` and consume `panel_kit::BOOT_CSS` from the crate.
- [ ] Stop writing legacy `{ panels, tiling }` JSON. Let `use_workspace` migrate it automatically, or read `StoredLayout` and write `SavedLayoutV2` in custom persistence.
- [ ] If using the Nix DSL, add `units`, `viewport`, and `mode` to every `mkLayout` call so it emits V2.
- [ ] Replace consumer CSS keyed on `.mobile` with `.compact` and, where appropriate, `.tablet` / `.regular`. Do not key control sizing off the tier — inherit `var(--hit-min)` or key off `.coarse`.
- [ ] Keep `GlobalLoadingBar` inside the element carrying `Workspace::root_class()` so it inherits the active surface tier.
- [ ] Delete any consumer CSS that hides `.lights` or `.resize` by tier; it now suppresses the compact restore control too.
- [ ] In TUI consumers, pass `Rgb` tuples to drawing helpers; import `PointerButton`, `PointerEventKind`, and `PointerEvent` from core; replace local row constants with `TileMetrics::CELLS.row`; use `LayoutStore` for non-file persistence.
- [ ] Update the panel-kit git revision only after the repository builds and its workspace canary passes with these edits.

Migration details and before/after snippets: <https://github.com/ocasazza/panel-kit/blob/main/MIGRATION.md>
