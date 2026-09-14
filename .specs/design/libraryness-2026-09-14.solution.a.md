# A1 expanded design: headless reducer, injected persistence, composable shells, and a complete `WorkspaceSpec`

## Status and intent

This document expands **A1, “Headless core reducer + pluggable store + escape-hatch shells.”** It is an implementation specification for `/plan-task` and `/implement-task`; it does not contain an implementation.

The mechanism remains A1’s mechanism:

1. `panel-kit-core` owns a small headless state value and reducer over the existing core operations.
2. The persistence transport is an injected `LayoutStore` trait in core.
3. Dioxus and ratatui remain optional convenience shells, but their rendering is implemented from public, independently usable panel/chrome/dock/traffic-light parts.
4. A serde `WorkspaceSpec` in core is the interchange contract for a feature-complete Nix authoring surface.
5. A native Rust comparison program, run by named `nix flake check` jobs, proves semantic equality between the Nix and Rust canaries.

The application still owns its event loop and render tree. The reducer, persistence, projections, chrome parts, widgets, and complete shell are separately adoptable library pieces.

## 1. Problem statement and verified evidence

### 1.1 Core has shared operations but no shared lifecycle

The working tree already puts renderer-neutral concepts in `panel-kit-core`, as required by `AGENTS.md:3-23`. This includes:

- `PanelKind`, whose current bounds require a cheap `Copy` identity and whose `title` is a `&'static str` (`crates/panel-kit-core/src/lib.rs:31-43`);
- `SurfaceProfile` classification and `effective_mode` (`crates/panel-kit-core/src/lib.rs:68-165`);
- renderer-neutral chrome regions and metrics (`crates/panel-kit-core/src/lib.rs:406-459`);
- `PanelWin` geometry, state, z-order, and tile spans (`crates/panel-kit-core/src/lib.rs:502-544`);
- drag, resize, reorder, restore, visibility, and command reducers (`crates/panel-kit-core/src/lib.rs:717-851,870-971`);
- V1/V2 persisted layouts, migration, unit reconciliation, and default merging (`crates/panel-kit-core/src/lib.rs:973-1077`).

Those already-landed APIs are inputs to this design, not proposed work.

The missing part is one shared lifecycle over them. The web backend still owns eight Dioxus signals plus a dock-focus signal in `Workspace<K>` (`src/lib.rs:293-323`), performs V1/V2 loading and default merging in private `load_layout` (`src/lib.rs:267-283`), and saves to `gloo_storage::LocalStorage` in private `save_layout` (`src/lib.rs:249-265`). `use_workspace` also requires a static storage key and a non-capturing function pointer (`src/lib.rs:340-347`) and hardcodes resize tracking and save-on-settle effects (`src/lib.rs:348-470`).

The TUI independently owns panels, mode, focus, drag, pending layout, viewport, hit zones, charset, and scrolling (`crates/panel-kit-tui/src/lib.rs:191-222`), independently parses, migrates, reconciles, and merges a stored layout (`crates/panel-kit-tui/src/lib.rs:246-274,315-328`), and independently encodes and saves V2 (`crates/panel-kit-tui/src/lib.rs:284-303`). This duplicates behavior that `AGENTS.md:18-20` says belongs in core when it should match across backends.

### 1.2 `LayoutStore` exists, but in the wrong crate

`LayoutStore` is **not new**. The working tree already declares its two-method opaque-JSON transport in `panel-kit-tui` (`crates/panel-kit-tui/src/lib.rs:144-155`), implements a private file store (`crates/panel-kit-tui/src/lib.rs:157-171`), and exposes `TuiWorkspace::with_store` (`crates/panel-kit-tui/src/lib.rs:235-244`). The browser-TUI canary implements that backend-local trait for `localStorage` (`crates/panel-kit-tui/examples/browser_tui.rs:60-99`).

The change is to promote and slightly redesign this seam in `panel-kit-core`, add `clear` because the web canary exposes reset behavior, make file and browser implementations public backend adapters, and delete the duplicate web persistence functions. Core owns JSON interpretation; stores only transport opaque bytes.

### 1.3 The shells still own the render tree

The Dioxus `Workspace::render` computes visibility, mode, scrolling, panel geometry, panel DOM, header, traffic lights, resize control, and body placement in one method (`src/lib.rs:776-930`). Its header is private (`src/lib.rs:932-1006`); only the whole dock is public (`src/lib.rs:1024-1069`). A caller cannot use the library’s panel geometry while rendering a single unadorned panel, omit traffic lights but retain the panel border, or replace only the dock.

The TUI has the same issue at a larger scale. `TuiWorkspace::render` draws the root, dock, tile layout, every panel, border, title, lights, resize grip, content callback, hit zones, and scrollbar (`crates/panel-kit-tui/src/lib.rs:335-660`). Its `Zone` hit-test record is private (`crates/panel-kit-tui/src/lib.rs:129-142`).

The concrete previously impossible composition this design must unlock is:

> Render the `Nodes` panel as a standalone app-owned region using panel-kit geometry and focus behavior, with a panel border but **without** panel-kit’s title row, traffic lights, resize grip, workspace root, or dock; beside it, render an application-owned dock.

The named escape hatches are `project_panel`, web `PanelSurface`, TUI `draw_panel_surface`, and the fact that `PanelChrome`, `TrafficLights`, `ResizeGrip`, and `Dock` are separate opt-in parts. None of those chrome parts is implicitly called by `PanelSurface`.

### 1.4 The current Nix surface is only a persisted layout

`nix/lib/mkLayout.nix` accurately documents and emits `SavedLayoutV2<K>` with version, units, viewport, mode, and panel geometry (`nix/lib/mkLayout.nix:1-28,106-135`). It validates state and tile-span bounds (`nix/lib/mkLayout.nix:71-104`) and is exported through `lib.${system}` (`flake.nix:111-116`). That is useful and remains as the lower-level “layout only” API.

It cannot express titles, content bindings, badges, widgets, palette, font token, chrome parts and metrics, surface thresholds, keyboard policy, persistence policy, or charset. Those are visible in the actual examples:

- the web workspace defines five panel kinds and titles, a minimized default, precise geometry and spans, a localStorage key, app bodies, a custom topbar, a tooltip overlay, overflow/wheel behavior, and reset (`examples/workspace.rs:44-102,109-312`);
- the TUI canary defines nine kinds and nine layouts, including `Flame` and `Distribution` (`crates/panel-kit-tui/examples/workspace_canary.rs:7-50`), badges with all meaningful nested data (`crates/panel-kit-tui/examples/workspace_canary.rs:64-109`), dynamic time-series/flame/boxplot data (`crates/panel-kit-tui/examples/workspace_canary.rs:111-257`), and gauges (`crates/panel-kit-tui/examples/workspace_canary.rs:267-290`);
- the native TUI runner combines table, status, meter, scrolling, spinner, palette switching, charts, gauges, flame graph, and boxplot (`crates/panel-kit-tui/examples/workspace.rs:159-305`);
- the browser TUI selects ASCII glyphs and injects localStorage (`crates/panel-kit-tui/examples/browser_tui.rs:129-160`).

### 1.5 The existing parity check is green while parity is false

The current Nix canary declares seven panels and explicitly claims a one-for-one mirror (`nix/examples/workspace-canary.nix:1-6,24-32`). The Rust TUI canary declares nine: it additionally has `Flame` and `Distribution`, and its later panel positions differ (`crates/panel-kit-tui/examples/workspace_canary.rs:36-50`).

The check cannot see this. `checks.layout-canary-schema` only runs `jq` on the Nix-produced file and explicitly asserts length seven (`flake.nix:118-149`). It neither runs Rust nor compares against the Rust canary. Extending that `jq` expression is therefore not an acceptable parity mechanism.

### 1.6 Theme and widget models are asymmetric

The web stylesheet hardcodes palette values and other tokens in `:root` (`assets/panel-kit.css:1-29`). TUI separately hardcodes the dark and paper RGB values in a backend `Theme` (`crates/panel-kit-tui/src/theme.rs:7-80`). `DESIGN.md` carries a third literal palette and typography source (`DESIGN.md:1-63`).

The semantic badge kind/action logic already lives correctly in core (`crates/panel-kit-core/src/badge.rs:14-122`), but the TUI owns a separate `Badge` data struct (`crates/panel-kit-tui/src/badge.rs:11-38`) while the web component has additional independent properties such as `with_x`, `with_plus`, `small`, `accent_color`, and hover emission (`src/badge.rs:59-160`). Chart, table, meter, scroll, status, and theme modules are exported only by TUI (`crates/panel-kit-tui/src/lib.rs:35-44`). A full interchange spec cannot preserve that asymmetry.

## 2. Decisions and direct answers to judge feedback

| Concern | Design response |
| --- | --- |
| `WorkspaceState` could become a replacement god object. | `WorkspaceState` contains only mutable renderer-neutral interaction state. It contains no store, palette, panel metadata/catalog, viewport policy, backend handles, callbacks, or rendering. Its fields are public. Existing core free functions remain public. `reduce` is optional convenience, not an exclusive mutation gateway. Projection and shell parts accept the narrowest inputs (`&[PanelWin<K>]`, one `PanelProjection`, or one state field), so callers do not need a `WorkspaceState` to draw one panel. |
| `LayoutStore` was claimed as new although it already exists in TUI. | Treat `crates/panel-kit-tui/src/lib.rs:149-155` as the prototype. Move the trait to core, add `clear`, re-export it from core only, make the TUI file adapter public, and add the missing web localStorage adapter. Delete the TUI declaration and update the browser-TUI implementation/import. |
| A concrete new composition must be shown. | Section 6 shows a standalone `Nodes` panel with only `PanelSurface`/`draw_panel_surface`, no built-in chrome, and a custom dock. Section 6 also shows replacing just the dock and rendering two workspaces concurrently. `PanelSurface` is the key escape hatch; it never calls `PanelChrome`, `TrafficLights`, `ResizeGrip`, or `Dock`. |
| Shape-only parity is inadequate. | `checks.spec-parity` obtains one complete `WorkspaceSpec` from Rust and one from Nix, deserializes both through the core Rust type, validates both, and compares semantic values including ordered panel IDs and all nested content. It fails today with missing `/panels/2` (`Flame`) and `/panels/7` (`Distribution`) rather than passing a seven-item shape assertion. |
| `PanelKind` is closed and cannot back a Nix-defined panel set. | A built-in `String: PanelKind` implementation is impossible: `String` is not `Copy`, and runtime titles cannot satisfy `fn title(self) -> &'static str` (`crates/panel-kit-core/src/lib.rs:37-42`). Keep `PanelKind` unchanged for hand-written enum convenience, add a title-free `PanelKey` state bound and copyable `SpecPanelId`, and resolve Nix string IDs once into an indexed `SpecPanelCatalog`. Per-frame state stays allocation-free; persisted spec-runner layouts translate back to stable string IDs through the catalog. |
| Per-backend themes/widgets could remain divergent. | Put `Palette`, complete badge/widget data models, and binding descriptors in core. Web and TUI only render those models. Remove hardcoded TUI presets and web color declarations. Lock `DESIGN.md`’s generated token block to core with `theme-parity`. |
| No runtime Nix dependency. | Nix evaluates only while building/checking. It emits JSON. Rust can construct the same type directly or embed the JSON with `include_bytes!`; no Nix evaluator, Nix library, or process exists in the application at runtime. |

## 3. Target crate and module layout

No framework crate and no renderer-neutral “platform” trait are added.

```text
crates/panel-kit-core/src/
  lib.rs                 # narrow re-exports
  workspace.rs           # WorkspaceState, WorkspaceAction, reduce, projections
  persistence.rs         # promoted LayoutStore + V1/V2 lifecycle helpers
  spec.rs                # WorkspaceSpec, validation, resolution/catalog
  theme.rs               # Palette, PaletteToken, typography token
  widget.rs              # semantic widget/binding data
  badge.rs               # existing semantics + serializable BadgeSpec
  bin/panel-kit-spec.rs  # native validate/normalize/compare CLI for checks

src/                     # Dioxus backend
  lib.rs                 # hook/config, event adapters, convenience Workspace::render
  parts.rs               # PanelSurface, PanelChrome, TrafficLights, ResizeGrip, Dock
  store.rs               # LocalStorageLayoutStore
  theme.rs               # stylesheet/palette CSS generation
  widget.rs              # Dioxus renderers for core widget models
  badge.rs               # Dioxus view over core BadgeSpec

crates/panel-kit-tui/src/
  lib.rs                 # TuiWorkspace convenience and event translation
  parts.rs               # surface/chrome/lights/grip/dock drawing + public hit zones
  store.rs               # public JsonFileLayoutStore
  theme.rs               # ratatui Theme conversion from core Palette only
  widget.rs              # rendering of core widget models
  badge.rs               # drawing functions over core BadgeSpec; no duplicate Badge data type

nix/lib/
  mkLayout.nix           # retained lower-level SavedLayoutV2 emitter
  mkWorkspaceSpec.nix    # complete WorkspaceSpec emitter and Nix assertions

nix/examples/
  workspace-spec.nix     # complete Nix-authored canary; replaces workspace-canary.nix

crates/panel-kit-tui/examples/
  workspace_canary_spec.rs  # Rust-authored complete WorkspaceSpec fixture shared by emit/render examples
  emit_workspace_spec.rs    # native JSON emitter used by spec-parity
  workspace.rs               # native canary consumes the shared Rust fixture
  browser_tui.rs             # wasm canary consumes the shared Rust fixture

examples/
  workspace.rs            # existing hand-written-enum convenience canary
  spec_workspace.rs       # Dioxus wasm canary built from the Nix JSON artifact
```

`panel-kit-spec` belongs to the core package so it can run natively without linking the wasm-only Dioxus crate. It only validates, normalizes, and compares `panel-kit-core` types; it is not shipped into applications.

## 4. Core state and reducer API

### 4.1 Identity split: `PanelKind` convenience versus `PanelKey` state

```rust
pub trait PanelKey:
    Copy + Eq + Hash + Serialize + DeserializeOwned + 'static
{}

// Existing enums keep working without another impl.
impl<T: PanelKind> PanelKey for T {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpecPanelId(u32);
impl PanelKey for SpecPanelId {}

pub trait PanelLabels<K: PanelKey> {
    fn stable_id(&self, key: K) -> &str;
    fn title(&self, key: K) -> &str;
}
```

`PanelKind` and its `title() -> &'static str` stay source-compatible for Rust enum consumers. The convenience shells create an internal `PanelLabels<K>` adapter from `PanelKind`. Spec-driven runners use `SpecPanelCatalog`, which owns the strings once and returns borrowed `&str` by `SpecPanelId` index.

`SpecPanelId` is deliberately numeric and copyable. `WorkspaceSpec::resolve` assigns indices after validating unique stable IDs. Rendering and reduction do not clone or hash strings per frame. When a spec-driven layout persists, `ResolvedWorkspaceSpec::encode_layout` writes `SavedLayoutV2<String>` using stable IDs; `decode_layout` maps strings back through the catalog. Unknown saved IDs are ignored and missing current IDs are appended from defaults, preserving `merge_defaults` behavior. Index values never go on disk.

### 4.2 Small state container

```rust
#[derive(Clone, PartialEq)]
pub struct WorkspaceState<K: PanelKey> {
    pub panels: Vec<PanelWin<K>>,
    pub preferred_mode: Mode,
    pub focused: Option<K>,
    pub drag: Option<Drag>,
    pub tile_drag: Option<K>,
    pub scroll: f64,
}

impl<K: PanelKey> WorkspaceState<K> {
    pub fn new(panels: Vec<PanelWin<K>>, preferred_mode: Mode) -> Self;
}
```

Excluded on purpose:

- viewport and `SurfaceProfile` are renderer environment, not durable state;
- hit zones and hover are backend projections;
- `LayoutStore` and save policy are injected collaborators;
- theme, charset, chrome metrics, and panel metadata are immutable configuration;
- Dioxus signals and ratatui `Rect`/`Frame` are backend types;
- content/widget data is application state or immutable spec data.

This boundary prevents `WorkspaceState` from becoming the object through which every capability must pass.

### 4.3 Reducer

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorkspaceEnv {
    pub viewport: Region,
    pub profile: SurfaceProfile,
    pub clamp: Clamp,
    pub tile: TileMetrics,
    pub command_step: CommandStep,
}

#[derive(Clone, PartialEq)]
pub enum WorkspaceAction<K: PanelKey> {
    Command { target: Option<K>, command: PanelCommand },
    BeginDrag { index: usize, kind: DragKind, pointer: PointerEvent },
    BeginTileResize { index: usize, pointer: PointerEvent },
    MovePointer(PointerEvent),
    EndPointer(PointerEvent),
    BeginTileReorder(K),
    ReorderTile { dragged: K, target: K },
    Restore(K),
    Scroll { delta: f64, content_height: f64 },
    ReplaceLayout { panels: Vec<PanelWin<K>>, mode: Mode },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangePhase { Continuous, Settled }

#[must_use]
pub struct Reduction {
    pub changed: bool,
    pub phase: ChangePhase,
}

pub fn reduce<K: PanelKey>(
    state: &mut WorkspaceState<K>,
    action: WorkspaceAction<K>,
    env: &WorkspaceEnv,
) -> Reduction;
```

Rules:

- `reduce` wraps, rather than duplicates, existing `begin_drag`, `begin_tile_resize`, `apply_drag`, `reorder_tile`, `restore`, `clamp_scroll`, and `apply_command`.
- Pointer moves and hover-reorders are `Continuous`; pointer-up, commands, restore, scroll, and explicit layout replacement are `Settled`.
- Invalid indices and pointer kinds are no-ops (`changed = false`) and never panic.
- `effective_mode(state.preferred_mode, &env.profile)` is used where drag/reorder behavior depends on the surface. `SurfaceProfile` and `effective_mode` are not redesigned.
- Existing free functions remain public escape hatches. Apps may mutate `state.panels` directly or call a single free function without adopting `WorkspaceAction`.
- Reducer output contains no I/O command and no renderer callback. The shell asks the persistence policy whether a `Reduction` warrants a save.

### 4.4 Projections

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelProjection<K: PanelKey> {
    pub index: usize,
    pub key: K,
    pub rect: Region,
    pub maximized: bool,
    pub focused: bool,
    pub movable: bool,
    pub resizable: bool,
    pub z: i32,
}

pub struct WorkspaceProjection<K: PanelKey> {
    pub chrome: WorkspaceChrome,
    pub panels: Vec<PanelProjection<K>>,
    pub minimized: Vec<K>,
    pub effective_mode: Mode,
    pub content_height: f64,
}

pub fn project_panel<K: PanelKey>(
    panels: &[PanelWin<K>],
    index: usize,
    focused: Option<K>,
    env: &WorkspaceEnv,
) -> Option<PanelProjection<K>>;

pub fn project_workspace<K: PanelKey>(
    state: &WorkspaceState<K>,
    env: &WorkspaceEnv,
    chrome: ChromeMetrics,
) -> WorkspaceProjection<K>;
```

`project_panel` is the minimal single-panel escape hatch. It does not calculate a dock or require a store, palette, or full workspace. `project_workspace` provides shared ordering/visibility/chrome semantics for convenience shells. Backend-specific clipping and DOM scroll-chain inspection remain backend adapter work.

## 5. Persistence promotion and lifecycle

### 5.1 Core trait and policy

Promote the existing TUI trait, preserving its deliberately opaque payload:

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoreError(pub String);

pub trait LayoutStore {
    fn load(&self) -> Result<Option<String>, StoreError>;
    fn save(&self, json: &str) -> Result<(), StoreError>;
    fn clear(&self) -> Result<(), StoreError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SavePolicy {
    Manual,
    OnSettle,
}

impl SavePolicy {
    pub fn should_save(self, reduction: Reduction) -> bool;
}
```

`clear` is the only addition to the existing trait and is justified by the web workspace reset at `examples/workspace.rs:282-298`. It avoids leaking `gloo_storage` back into applications.

Do not add async methods, transactions, subscriptions, a key/value store abstraction, or a storage “platform” trait. A `LayoutStore` instance is scoped to one logical layout. The key/path belongs to its constructor.

### 5.2 Core lifecycle helpers

```rust
pub struct RestoreContext {
    pub units: Units,
    pub viewport: (f64, f64),
}

pub fn restore_state<K: PanelKey>(
    store: &dyn LayoutStore,
    defaults: WorkspaceState<K>,
    context: RestoreContext,
) -> Result<WorkspaceState<K>, PersistenceError>;

pub fn save_state<K: PanelKey>(
    store: &dyn LayoutStore,
    state: &WorkspaceState<K>,
    context: RestoreContext,
) -> Result<(), PersistenceError>;
```

For handwritten enum keys, these helpers deserialize `StoredLayout<K>`, call the already-existing `migrate_v1` or `reconcile_units`, call `merge_defaults`, and write `SavedLayoutV2<K>` with `LAYOUT_SCHEMA_VERSION`. For `SpecPanelId`, corresponding methods on `ResolvedWorkspaceSpec` translate through stable string IDs before calling the same lifecycle.

Edge cases:

- `load() -> Ok(None)` returns defaults.
- Invalid JSON or a future layout version returns a typed error and leaves defaults intact; it is not silently overwritten on initialization.
- Zero or non-finite saved viewports are rejected before `reconcile_units` can divide by zero.
- A store failure is observable from the shell’s `persistence_error()` accessor or explicit result; it never panics.
- `OnSettle` does not save on pointer motion or reorder hover; it saves after pointer-up and discrete commands.
- V1 and V2 records remain readable. Existing keys and file paths are reused, so this design does not invalidate persisted user layouts.

### 5.3 Backend implementations

```rust
// panel-kit (Dioxus/web)
pub struct LocalStorageLayoutStore { /* owned String key */ }
impl LocalStorageLayoutStore {
    pub fn new(key: impl Into<String>) -> Self;
}
impl LayoutStore for LocalStorageLayoutStore { /* getItem/setItem/removeItem */ }

// panel-kit-tui
pub struct JsonFileLayoutStore { /* PathBuf */ }
impl JsonFileLayoutStore {
    pub fn new(path: impl Into<PathBuf>) -> Self;
}
impl LayoutStore for JsonFileLayoutStore { /* read/write/remove */ }
```

The browser-TUI canary either uses `LocalStorageLayoutStore` if dependency direction permits or keeps its local adapter, but imports the trait from `panel_kit_core`. The trait is deleted from `panel-kit-tui` rather than re-exported under the old path.

## 6. Shell decomposition and escape-hatch compositions

### 6.1 Dioxus API

```rust
pub struct WebWorkspaceConfig<K: PanelKind> {
    pub initial: WorkspaceState<K>,
    pub store: Option<Rc<dyn LayoutStore>>,
    pub save_policy: SavePolicy,
    pub palette: Palette,
    pub chrome: ChromeMetrics,
    pub surface: SurfacePolicy,
}

impl<K: PanelKind> WebWorkspaceConfig<K> {
    pub fn new(initial: WorkspaceState<K>) -> Self;
    pub fn with_store(self, store: Rc<dyn LayoutStore>) -> Self;
}

pub fn use_workspace<K: PanelKind>(config: WebWorkspaceConfig<K>) -> Workspace<K>;

pub fn PanelSurface<K: PanelKey>(
    panel: PanelProjection<K>,
    class: String,
    children: Element,
) -> Element;

pub fn PanelChrome<K: PanelKey>(
    panel: PanelProjection<K>,
    labels: &dyn PanelLabels<K>,
    children: Option<Element>,
) -> Element;

pub fn TrafficLights<K: PanelKey>(
    panel: PanelProjection<K>,
    dispatch: EventHandler<WorkspaceAction<K>>,
) -> Element;

pub fn ResizeGrip<K: PanelKey>(
    panel: PanelProjection<K>,
    dispatch: EventHandler<WorkspaceAction<K>>,
) -> Element;

pub fn Dock<K: PanelKey>(
    minimized: Vec<K>,
    labels: &dyn PanelLabels<K>,
    dispatch: EventHandler<WorkspaceAction<K>>,
) -> Element;
```

The exact Dioxus props derive requirements may wrap projections/labels in small `Clone + PartialEq` props types during implementation; that must not change the ownership boundary above.

`Workspace::render(body)` and `Workspace::dock()` remain as convenience methods for enum consumers. `render` is rewritten only as a loop over `project_workspace` plus the public parts in this order: `PanelSurface` → `PanelChrome` containing `TrafficLights` → body → `ResizeGrip`. `dock()` calls the public `Dock`. There is one implementation of each part; no “simple path” duplicate renderer.

### 6.2 Ratatui API

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TuiHitZones<K: PanelKey> {
    pub panel: K,
    pub surface: Rect,
    pub header: Option<Rect>,
    pub lights: [Option<Rect>; 3],
    pub resize_grip: Option<Rect>,
}

pub fn draw_panel_surface(
    frame: &mut Frame,
    panel: &PanelProjection<impl PanelKey>,
    theme: &Theme,
    charset: Charset,
) -> Rect; // returns body rect

pub fn draw_panel_chrome<K: PanelKey>(...) -> Option<Rect>;
pub fn draw_traffic_lights<K: PanelKey>(...) -> [Option<Rect>; 3];
pub fn draw_resize_grip<K: PanelKey>(...) -> Option<Rect>;
pub fn draw_dock<K: PanelKey>(...) -> Vec<(Rect, K)>;
pub fn hit_test<K: PanelKey>(zones: &[TuiHitZones<K>], at: Position) -> Option<TuiHit<K>>;
```

`TuiWorkspace::render` remains convenience sugar built only from `project_workspace` and these functions. Its current private `Zone` is replaced by the public, optional-region `TuiHitZones`; omitting a part means there is no hit zone and therefore no hidden behavior.

### 6.3 Concrete standalone panel: impossible today, possible after this change

Web sketch:

```rust
let panel = project_panel(&state.panels, nodes_index, state.focused, &env).unwrap();
rsx! {
    // No Workspace::render, root, dock, title, lights, or grip.
    PanelSurface { panel, class: "nodes-standalone",
        NodesBody {}
    }
    MyApplicationDock {}
}
```

TUI sketch:

```rust
let panel = project_panel(&state.panels, nodes_index, state.focused, &env).unwrap();
let body = draw_panel_surface(frame, &panel, &theme, Charset::Unicode);
draw_nodes(frame, body);
draw_my_application_dock(frame, app_dock_rect); // not panel-kit draw_dock
```

This proves every chrome piece is bypassable. `PanelSurface`/`draw_panel_surface` draws only the bounded panel surface and returns/contains the content area. The app can additionally skip the surface and use only `project_panel` if it wants completely custom rendering.

### 6.4 Other enabled compositions

- **Replace only the dock:** call the convenience `render` for panels, do not call `Workspace::dock`, and render an app dock from `projection.minimized` plus `WorkspaceAction::Restore`.
- **Custom header actions:** compose `PanelChrome` with an application element in its `children` slot. This replaces older downstream reliance on `render_with_header` without restoring a monolithic special-case render method.
- **Two concurrent workspaces:** call `use_workspace` twice with independent state/store values and render both projections in separate DOM regions. No process-global workspace or storage key is introduced.
- **Geometry only:** call `project_panel`, `effective_rect`, or existing free functions without linking Dioxus/ratatui.

## 7. Theme and token unification

### 7.1 Core source of truth

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Palette {
    pub bg: Rgb,
    pub panel: Rgb,
    pub fg: Rgb,
    pub dim: Rgb,
    pub line: Rgb,
    pub line2: Rgb,
    pub inv_bg: Rgb,
    pub inv_fg: Rgb,
    pub accent: Rgb,
    pub red: Rgb,
    pub yellow: Rgb,
    pub green: Rgb,
    pub blue: Rgb,
    pub pink: Rgb,
    pub badge_info: Rgb,
}

impl Palette {
    pub const DARK: Self;
    pub const PAPER: Self;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypographyTokens {
    pub mono: String,
}
```

The 15 palette fields cover every color currently present in CSS and `DESIGN.md`; unlike current TUI `Theme`, they include the inverse selection pair (`assets/panel-kit.css:12-13`; `DESIGN.md:11-12`).

### 7.2 Backend adapters

- `assets/panel-kit.css` becomes structural CSS. Its hardcoded color `:root` declarations are deleted; non-color geometry variables may remain.
- Web exports `pub fn stylesheet(palette: &Palette, typography: &TypographyTokens) -> String`, which prepends a generated `:root` variable block to structural CSS. `PaletteToken`/`ColorRef` generate safe CSS; arbitrary strings are not accepted as renderer-neutral colors.
- TUI retains `Theme` only as `ratatui::Color` adaptation and implements `From<&Palette> for Theme`. `Theme::DARK` and `Theme::PAPER` numeric duplicates are deleted.
- The literal `colors` and mono frontmatter in `DESIGN.md` is a generated region emitted by `panel-kit-spec emit-design-tokens`. `checks.theme-parity` compares the generated region to the committed document. Human design rationale remains hand-written.

This makes core `Palette` authoritative while keeping `DESIGN.md` useful to design tooling and CSS useful without a build-time preprocessor.

## 8. `WorkspaceSpec`: Rust contract and runtime boundary

### 8.1 Location and lifetime

`WorkspaceSpec` lives in **`panel-kit-core::spec`** as versioned serde data. It is not a Nix module type and not a backend type.

Nix evaluation is **build-time only**. The resulting JSON is an immutable build artifact, normally embedded with `include_bytes!`. Rust applications may instead construct `WorkspaceSpec` directly. The spec is read at initialization to produce defaults, catalog, theme, and rendering policy; it is not the mutable saved layout and is not rewritten while the user drags panels. User state continues to persist as `SavedLayoutV2`.

Thus the two contracts have distinct jobs:

- `WorkspaceSpec` = immutable application definition/defaults/content bindings;
- `SavedLayoutV2`/`StoredLayout` = mutable user layout persistence, with existing V1 migration.

### 8.2 Rust shape

```rust
pub const WORKSPACE_SPEC_VERSION: u32 = 1;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceSpec {
    pub version: u32,
    pub units: Units,
    pub viewport: (f64, f64),
    pub mode: Mode,
    pub panels: Vec<PanelSpec>,
    pub chrome: ChromeSpec,
    pub surface: SurfacePolicySpec,
    pub input: InputSpec,
    pub theme: ThemeSpec,
    pub persistence: PersistenceSpec,
    pub charset: CharsetPolicy,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct PanelSpec {
    pub id: String,
    pub title: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub state: WinState,
    pub z: i32,
    pub tile_w: u8,
    pub tile_h: u8,
    pub content: ContentSpec,
}

impl WorkspaceSpec {
    pub fn validate(&self) -> Result<(), Vec<SpecError>>;
    pub fn resolve(self) -> Result<ResolvedWorkspaceSpec, Vec<SpecError>>;
}
```

The layout fields intentionally serialize exactly like a `PanelWin<String>` except that `id` replaces `kind` and metadata/content are adjacent. `resolve` constructs `PanelWin<SpecPanelId>` without per-frame strings.

Validation accumulates errors with JSON-pointer paths and rejects:

- unsupported `version`;
- duplicate/empty panel IDs, empty titles, or more than `u32::MAX` panels;
- non-finite or non-positive viewport/panel sizes;
- tile spans outside existing `1..=TILE_W_MAX` and `1..=TILE_H_MAX`;
- more than one initially maximized panel;
- invalid breakpoint ordering or negative chrome metrics;
- duplicate binding names with incompatible declared data kinds;
- non-finite widget numbers, negative flame weights, non-rectangular table rows, and ratios outside `0..=1`;
- persistence enabled without a non-empty key;
- string color injection rather than a palette token or RGB triple.

### 8.3 Chrome, surface, input, persistence, and charset

```rust
pub struct ChromeSpec {
    pub web: ChromeMetrics,
    pub cells: ChromeMetrics,
    pub parts: ChromeParts,
}

pub struct ChromeParts {
    pub workspace_root: bool,
    pub panel_surface: bool,
    pub panel_chrome: bool,
    pub traffic_lights: bool,
    pub resize_grip: bool,
    pub dock: bool,
}

pub struct SurfacePolicySpec {
    pub web: Breakpoints,
    pub cells: Breakpoints,
}

pub struct Breakpoints {
    pub compact_max: f64,
    pub tablet_max: f64,
}

pub struct InputSpec {
    pub keyboard: KeyboardSpec,
    pub web_step: CommandStep,
    pub cell_step: CommandStep,
}

pub struct KeyboardSpec {
    pub preset: KeyboardPreset, // Default or None
    pub bindings: Vec<KeyBindingSpec>,
}

pub struct KeyBindingSpec {
    pub chord: KeyChord,
    pub context: FocusScope, // Workspace, Panel, TextInput
    pub command: PanelCommand,
}

pub struct PersistenceSpec {
    pub enabled: bool,
    pub key: String,
    pub format: PersistenceFormat, // currently SavedLayoutV2 only
    pub load: bool,
    pub save: SavePolicy,
}

pub struct CharsetPolicy {
    pub terminal: Charset,
    pub browser_tui: Charset,
}
```

The default constructors use the already-existing `WEB_*`/`CELLS_*` thresholds, `CommandStep::WEB`/`CELLS`, `command_for` table, and `ChromeMetrics::WEB`/`CELLS`; this work exposes them in data rather than re-inventing them. `Charset` moves from the TUI backend (`crates/panel-kit-tui/src/lib.rs:79-110`) to core because the Nix spec and browser-TUI both need the renderer-neutral Unicode/ASCII choice.
To make this concrete serde shape compile, existing plain-data `ChromeMetrics`, `CommandStep`, `KeyChord`, and `PanelCommand` gain `Serialize`/`Deserialize`; `Charset` gains them when it moves to core. These are wire derives only. Their variants, defaults, and behavior do not change.

Custom bindings layer after the selected preset; an identical chord/context override replaces the preset entry. `TextInput` remains protected unless the spec explicitly binds that exact context. The existing `command_for`/`apply_command` remain the implementation of the default preset.

### 8.4 Theme and color references

```rust
pub struct ThemeSpec {
    pub palette: Palette,
    pub typography: TypographyTokens,
}

pub enum PaletteToken {
    Bg, Panel, Fg, Dim, Line, Line2, InvBg, InvFg,
    Accent, Red, Yellow, Green, Blue, Pink, BadgeInfo,
}

pub enum ColorRef {
    Token(PaletteToken),
    Rgb(Rgb),
}
```

This replaces the web-only arbitrary `accent_color: Option<String>` in the spec path. The standalone Dioxus `Badge` may keep a CSS-string escape hatch because it is explicitly web-only; the renderer-neutral `BadgeSpec` cannot.

### 8.5 Widget/content model and bindings

The core model carries semantic data, not ratatui widgets, Dioxus elements, animation engines, or apple-notes-specific stage policy.

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ContentSpec {
    Binding { name: String },
    Text { lines: DataSource<Vec<TextLineSpec>>, scroll: bool },
    Stack { children: Vec<ContentSpec> },
    Badges { items: DataSource<Vec<BadgeSpec>>, action: Option<String> },
    TimeSeries { unit: String, series: DataSource<Vec<SeriesSpec>> },
    Gauges { items: DataSource<Vec<GaugeSpec>> },
    Flame { spans: DataSource<Vec<FlameSpanSpec>> },
    BoxPlot { items: DataSource<Vec<BoxItemSpec>> },
    Table {
        columns: Vec<TableColumnSpec>,
        rows: DataSource<Vec<Vec<TableCellSpec>>>,
    },
    Spinner { label: String, tick: DataSource<u64> },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "kebab-case")]
pub enum DataSource<T> {
    Inline { value: T },
    Binding { name: String },
}

pub struct SeriesSpec { pub name: String, pub points: Vec<(f64, f64)> }
pub struct GaugeSpec { pub label: String, pub ratio: f64, pub text: String }
pub struct FlameSpanSpec { pub label: String, pub depth: u16, pub value: f64, pub color: Option<ColorRef> }
pub struct BoxItemSpec { pub label: String, pub samples: Vec<f64>, pub color: Option<ColorRef> }
pub struct TableColumnSpec { pub label: String, pub width: ColumnWidth }
pub enum TableCellSpec {
    Text(String),
    Status { color: ColorRef, label: String },
    Meter { ratio: f64, width: u16, color: ColorRef },
}
```

`BadgeSpec` moves the common data into core and includes the superset of current web/TUI options: `kind`, `field`, `value`, `active`, `with_x`, `with_plus`, `small`, `override_color`, renderer-neutral `accent_color`, `click_kind`, `emit_hover`, and optional action binding. Existing `BadgeKind`, `BadgeAction`, and `BadgeClickKind` gain serde derives; their semantics are not redesigned.

`DataSource::Binding` is how the spec expresses live application data without pretending that a Nix file can serialize Rust callbacks. A backend runner receives a `WidgetBindings` implementation for typed data plus a backend-specific callback for `ContentSpec::Binding`:

```rust
pub trait WidgetBindings {
    fn resolve<'a>(&'a self, name: &str, expected: WidgetDataKind)
        -> Option<WidgetData<'a>>;
}

pub enum WidgetData<'a> {
    Lines(&'a [TextLineSpec]),
    Badges(&'a [BadgeSpec]),
    Series(&'a [SeriesSpec]),
    Gauges(&'a [GaugeSpec]),
    Flame(&'a [FlameSpanSpec]),
    BoxItems(&'a [BoxItemSpec]),
    TableRows(&'a [Vec<TableCellSpec>]),
    Tick(u64),
}
```

Resolution is borrowed; a frame does not clone chart buffers. The current `Metrics` animation remains app code and exposes slices through bindings such as `metrics.activity`, `metrics.flame`, and `metrics.distribution`. This avoids leaking the canary’s `STAGES` or random generator into core.

The Dioxus and TUI backends both implement every `ContentSpec` variant. A missing binding renders an explicit backend error cell naming the binding and is also rejected by runner initialization when bindings are declared up front. Arbitrary app bodies (the web Notes editor, graph canvas, or theme picker) use `ContentSpec::Binding` and the backend-specific callback.

## 9. Pure-Nix authoring surface

### 9.1 Export

`nix/lib/mkWorkspaceSpec.nix` exports:

```nix
inputs.panel-kit.lib.${system}.mkWorkspaceSpec
inputs.panel-kit.lib.${system}.workspaceSpecVersion
inputs.panel-kit.lib.${system}.palettePresets
inputs.panel-kit.lib.${system}.contentKinds
```

`mkLayout`, `winStates`, `modes`, `unitKinds`, `schemaVersion`, `tileWMax`, and `tileHMax` remain exported as the lower-level persisted-layout API. `mkWorkspaceSpec` uses the same constants rather than duplicating span/state assertions.

### 9.2 Attrset schema

```nix
mkWorkspaceSpec {
  version = 1;
  units = "Cells";                    # "CssPx" | "Cells"
  viewport = [ 128.0 52.0 ];
  mode = "Tiling";                    # "Floating" | "Tiling"

  panels = [
    {
      id = "activity";                # stable persistence identity
      title = "Activity";
      x = 1.0; y = 12.0; w = 62.0; h = 14.0;
      state = "Floating";
      z = 2;
      tile_w = 1; tile_h = 3;
      content = {
        kind = "time-series";
        unit = "ms";
        series = { source = "binding"; name = "metrics.activity"; };
      };
    }
    {
      id = "capacity";
      title = "Capacity";
      x = 65.0; y = 22.0; w = 63.0; h = 8.0;
      tile_w = 2; tile_h = 2;
      content = {
        kind = "gauges";
        items = {
          source = "inline";
          value = [
            { label = "vfs"; ratio = 0.21; text = "31 / 148 files"; }
          ];
        };
      };
    }
  ];

  chrome = {
    web = { inset = 0.0; dock_h = 30.0; };
    cells = { inset = 1.0; dock_h = 3.0; };
    parts = {
      workspace_root = true;
      panel_surface = true;
      panel_chrome = true;
      traffic_lights = true;
      resize_grip = true;
      dock = true;
    };
  };

  surface = {
    web = { compact_max = 760.0; tablet_max = 1180.0; };
    cells = { compact_max = 60.0; tablet_max = 110.0; };
  };

  input = {
    keyboard.preset = "default";       # "default" | "none"
    keyboard.bindings = [ ];           # { chord, context, command }
    web_step = { coarse = 16.0; fine = 1.0; };
    cell_step = { coarse = 2.0; fine = 1.0; };
  };

  theme = {
    palette = "dark";                 # preset name or complete palette attrset
    typography.mono = ''ui-monospace, "SF Mono", "JetBrains Mono", monospace'';
  };

  persistence = {
    enabled = true;
    key = "panel-kit-tui-layout-v2";
    format = "saved-layout-v2";
    load = true;
    save = "on-settle";               # "manual" | "on-settle"
  };

  charset = {
    terminal = "Unicode";
    browser_tui = "Ascii";
  };
}
```

The result is:

```nix
{
  value = /* normalized WorkspaceSpec attrset */;
  json = builtins.toJSON value;
  panelIds = [ /* ordered stable IDs */ ];
}
```

Nix assertions provide early, friendly messages for required fields, known enum strings, obvious numeric bounds, duplicate IDs, and table row widths. Rust `WorkspaceSpec::validate` remains authoritative and runs in `spec-parity`; Nix assertions are not treated as a second schema.

### 9.3 Field-by-field completeness against the current examples

| Expressed behavior/data | Verified current source | `WorkspaceSpec` / Nix field | How the backend receives it |
| --- | --- | --- | --- |
| Panel identity and title | Web five variants/titles (`examples/workspace.rs:65-84`); TUI nine (`workspace_canary.rs:7-34`) | `panels[].id`, `panels[].title` | Resolved once to `SpecPanelId` + catalog. |
| Floating geometry | Web defaults (`examples/workspace.rs:88-102`); TUI defaults (`workspace_canary.rs:36-50`) | `units`, `viewport`, `panels[].{x,y,w,h}` | Builds `PanelWin<SpecPanelId>`; existing geometry functions project it. |
| Initial window state | Web Help is minimized (`examples/workspace.rs:90-98`) | `panels[].state` | Existing `WinState`. |
| Z-order | `LayoutBuilder` assigns current defaults; `PanelWin.z` is persisted (`core/lib.rs:502-533`) | `panels[].z`, defaulting by declaration order | Existing `front_z`/projection. |
| Tile spans | Both examples call `with_tile` (`examples/workspace.rs:90-100`; `workspace_canary.rs:39-48`) | `panels[].tile_w`, `tile_h` | Existing validated span fields. |
| Initial/effective mode | Web starts floating (`src/lib.rs:356`); TUI runner switches tiling (`tui/examples/workspace.rs:308-312`) | `mode`; `surface.*` | Existing `effective_mode`, not a new mode system. |
| Web persistence key/load/save/reset | Key/reset (`examples/workspace.rs:44-46,238-239,282-298`) | `persistence.{enabled,key,format,load,save}` | Injected `LocalStorageLayoutStore`; `clear` backs reset. |
| TUI file and browser localStorage persistence | Native file (`tui/examples/workspace.rs:308-311`); browser store (`browser_tui.rs:60-99,143-147`) | Same persistence fields; actual store is injected | `JsonFileLayoutStore` or localStorage implementation. The spec never chooses an arbitrary filesystem path. |
| Compact/tablet/regular breakpoints | Web uses core 760/1180 (`src/lib.rs:152-159`); TUI uses 60/110 (`tui/lib.rs:173-184`) | `surface.web`, `surface.cells` | Shell measures actual width/capabilities and constructs existing `SurfaceProfile`. |
| Chrome region metrics | Core current defaults (`core/lib.rs:422-459`) | `chrome.web`, `chrome.cells` | `workspace_chrome`. |
| Optional root/panel/title/lights/grip/dock | Web full render (`src/lib.rs:822-928`); TUI full render (`tui/lib.rs:335-660`) | `chrome.parts.*` | Convenience runner conditionally composes public parts; app API may override regardless. |
| Keyboard command table and step size | Web handler maps DOM keys (`src/lib.rs:533-587`); TUI maps keys (`tui/examples/workspace.rs:54-74`) | `input.keyboard`, `input.web_step`, `input.cell_step` | Existing `command_for`/`apply_command`; custom table overlays defaults. |
| Unicode versus ASCII TUI chrome | Charset enum (`tui/lib.rs:79-110`); browser chooses ASCII (`browser_tui.rs:143-147`) | `charset.{terminal,browser_tui}` | TUI part renderers. |
| Complete palette and mono token | CSS vars (`assets/panel-kit.css:5-29`), TUI theme (`theme.rs:7-80`), docs (`DESIGN.md:4-63`) | `theme.palette`, `theme.typography.mono` | Core palette → generated CSS or TUI `Theme`. |
| Arbitrary web panel bodies (Notes editor, Preview overflow, Status, Help, BelowFold) | `examples/workspace.rs:130-266` | `content = { kind = "binding"; name = ...; }` | Dioxus binding callback returns app-owned `Element`; spec describes placement/identity, not Rust code. |
| App topbar and tooltip overlay | `examples/workspace.rs:268-310` | Deliberately app-owned binding/slot outside workspace spec | Escape-hatch composition lets the app interleave these nodes. Their Rust event logic is not falsely serialized as data. |
| Badge kind nested fields | Entity/Wikilink/URL data (`workspace_canary.rs:64-109`) | `badges.items[].kind` tagged attrsets | Shared core `BadgeKind` serde. |
| Badge presentation/actions | Web superset (`src/badge.rs:59-160`); TUI current subset (`tui/badge.rs:11-101`) | `BadgeSpec` superset fields + action binding | Both backends render the same semantic model; action callback remains app-owned. |
| Time series | `Metrics::series` and rendering (`workspace_canary.rs:189-201`; `tui/examples/workspace.rs:217-219`) | `time-series { unit; series = inline|binding; }` | Binding returns borrowed series buffers. |
| Gauges | Capacity items (`workspace_canary.rs:267-290`) | `gauges.items` | Shared `GaugeSpec`. |
| Flame graph | Dynamic spans (`workspace_canary.rs:216-256`) | `flame.spans` | Shared flattened preorder `FlameSpanSpec`. |
| Boxplot | Stage samples/items (`workspace_canary.rs:133-146,203-214`) | `box-plot.items` | Shared `BoxItemSpec`; quartiles remain renderer helper behavior. |
| Table + status + meter cells | Nodes composition (`tui/examples/workspace.rs:230-253`) | `table.{columns,rows}` and `TableCellSpec::{Status,Meter,Text}` | Both backends render semantic cells. |
| Scrollable text | Notes uses `scroll::lines` (`tui/examples/workspace.rs:255-277`) | `text.scroll = true` or a binding returning lines | Backend scrolling adapter; workspace-level scroll remains reducer state. |
| Spinner | Notes and Theme (`tui/examples/workspace.rs:275-300`); web component (`src/lib.rs:1091-1105`) | `spinner { label; tick = inline|binding; }` | Backend widget renderer. |
| Live metrics/noise and theme-toggle logic | `Metrics::tick` (`workspace_canary.rs:164-187`) and `toggle_theme` (`tui/examples/workspace.rs:150-157`) | Named typed bindings / arbitrary content binding | Application code remains application code. Nix expresses the required binding contract, not an embedded programming language. |

The boundary in the last two rows is intentional: “feature-complete” means every library-visible configuration and every content/widget binding is expressible. It does not mean Nix becomes a general-purpose replacement for Dioxus components, ratatui event loops, network fetches, or animation code.

## 10. Nix → Rust → backend data flow

```text
Nix authoring (build time only)
  nix/lib/mkWorkspaceSpec.nix
             │ builtins.toJSON
             ▼
  WorkspaceSpec v1 JSON artifact
             │ include_bytes! / check CLI input
             ▼
  panel_kit_core::WorkspaceSpec (serde + validate)
             │ resolve once
             ├── WorkspaceState<SpecPanelId> defaults
             ├── SpecPanelCatalog (stable id/title/content)
             ├── Palette / chrome / surface / input / persistence policies
             └── required binding declarations
                         │
        application supplies stores + live bindings
                         │
             ┌───────────┴───────────┐
             ▼                       ▼
    panel-kit Dioxus parts     panel-kit-tui parts
    DOM events → Action        terminal events → Action
    widget model → DOM         widget model → ratatui
             │                       │
             └──── reduce in core ───┘
                         │ settled change
                         ▼
              injected LayoutStore
              SavedLayoutV2 only
```

A non-Nix Cargo consumer starts at the same Rust `WorkspaceSpec` box or continues to use a `PanelKind` enum plus `WebWorkspaceConfig`. Nix is never required to use the library.

## 11. Mechanized parity proof

### 11.1 Delete the current false proof

Delete:

- `nix/examples/workspace-canary.nix` in its current hand-mirror form;
- `checks.layout-canary-schema` and its `jq` length/key assertions;
- comments claiming the seven-panel Nix layout mirrors the Rust canary.

Replace the file with the independently authored, complete `nix/examples/workspace-spec.nix`; do not rename or “upgrade” the old file in place. The Rust canary moves its complete description to `workspace_canary_spec.rs`; both TUI runners consume that Rust value so there is not a third Rust layout list.

### 11.2 `panel-kit-spec` comparison semantics

Commands:

```text
panel-kit-spec validate FILE
panel-kit-spec normalize FILE
panel-kit-spec compare EXPECTED ACTUAL
panel-kit-spec emit-design-tokens
```

`compare`:

1. deserializes each file as `WorkspaceSpec`;
2. validates each and fails with JSON-pointer diagnostics;
3. compares the resulting Rust values with `PartialEq`;
4. on mismatch, prints ordered JSON-pointer differences;
5. treats object key order and integer-versus-float lexical spelling as irrelevant, but preserves array order and exact semantic number values.

It must not compare raw bytes. This avoids `builtins.toJSON`/`serde_json` formatting flakes while retaining meaningful order and values.

### 11.3 Named flake jobs

Add these jobs to `checks.${system}` and therefore to the existing `hydraJobs` aggregate:

1. **`spec-schema`** — evaluates every `nix/examples/*.nix` spec JSON and runs `panel-kit-spec validate`. It proves the pure-Nix value deserializes into the actual working-tree Rust type and satisfies invariants.
2. **`spec-parity`** — runs the native Rust `emit_workspace_spec` example to produce the Rust-authored canary, evaluates `nix/examples/workspace-spec.nix`, and runs `panel-kit-spec compare RUST NIX`. It compares every field, panel, nested widget, binding, and policy.
3. **`spec-tui-example`** — natively builds the ratatui example instantiated from the Nix spec artifact.
4. **`spec-web-example`** — builds `examples/spec_workspace.rs` for `wasm32-unknown-unknown` using the existing pinned wasm toolchain.
5. **`spec-browser-tui-example`** — builds the browser-TUI spec path for `wasm32-unknown-unknown` (it may share compilation with the existing `browser-tui-example`, but the job name remains explicit).
6. **`theme-parity`** — compares core-emitted color/typography tokens with the generated `DESIGN.md` frontmatter region and verifies generated CSS declares each `Palette` field exactly once.

The comparison and emitter need a native Rust toolchain in addition to the existing wasm `craneLib`; do not attempt to execute a wasm binary. Keep Dioxus 0.6 and wasm-bindgen CLI/Cargo.lock lockstep untouched (`flake.nix:6-8,169-175`).

### 11.4 Proof that the live 7-vs-9 drift fails

Before correcting fixtures, the first `spec-parity` run uses:

- Rust ordered IDs: `Workspace, Activity, Flame, Notes, Badges, Nodes, Capacity, Distribution, Theme` from `workspace_canary.rs:36-49`;
- current Nix IDs: `Workspace, Activity, Notes, Badges, Nodes, Capacity, Theme` from `workspace-canary.nix:24-31`.

The job exits non-zero. At minimum diagnostics identify:

```text
/panels/2/id: expected "flame", actual "notes"
/panels/7: expected panel "distribution", actual panel "theme"
/panels/8: expected panel "theme", actual missing
```

It also catches stale Notes and Theme `y` coordinates because the comparison is full-value, not set-only. The implementation slice must preserve this initial red result as TDD evidence before replacing the false canary and making both complete fixtures agree.

## 12. Deletions and clean cutover

Delete, rather than leave alternate paths:

1. Web private `save_layout` and `load_layout` (`src/lib.rs:249-283`) and direct `gloo_storage` use in `use_workspace`.
2. The `LayoutStore` declaration in `panel-kit-tui` (`tui/lib.rs:144-155`), private `FileLayoutStore`, and backend-local JSON lifecycle. The promoted core trait has one canonical import path.
3. TUI’s duplicated `restore_pending_layout` and `save` serialization bodies; shell methods call core lifecycle helpers.
4. The monolithic implementations inside both `render` methods. The public convenience names remain, but their bodies are compositions of public parts.
5. Private web `header` and private TUI `Zone` as exclusive seams; replace them with public granular parts/zones.
6. TUI `Theme::DARK`/`PAPER` literal palettes and the web CSS hardcoded color `:root` block.
7. TUI’s duplicate `Badge` data struct; draw core `BadgeSpec` instead.
8. Backend-local semantic chart/gauge/flame/box/table data structs after equivalent core models are in use. Rendering code remains backend-local.
9. `nix/examples/workspace-canary.nix` and `checks.layout-canary-schema` as described above.
10. Any README claim that applications must supply a body callback and let `Workspace` own everything; document the convenience shell and parts equally.

Do **not** delete `SurfaceProfile`, `effective_mode`, `command_for`, `apply_command`, `SavedLayoutV2`, `StoredLayout`, `migrate_v1`, `reconcile_units`, `merge_defaults`, or `mkLayout`. They are already-correct lower-level building blocks.

## 13. Consumer migration in `MIGRATION.md` style

This is a clean 2.0 cutover. There are no deprecated aliases, duplicate re-exports, or compatibility facades. Git-pinned consumers remain safe until they update their revision, matching the current notice style at `MIGRATION.md:435-454`.

### 13.1 Breaking change: construct web workspaces from owned state and an injected store

**What changed:**

```rust
// 1.0
pub fn use_workspace<K: PanelKind>(
    storage_key: &'static str,
    defaults: fn() -> Vec<PanelWin<K>>,
) -> Workspace<K>;

// 2.0
pub fn use_workspace<K: PanelKind>(
    config: WebWorkspaceConfig<K>,
) -> Workspace<K>;
```

**Why:** the old signature hardcodes localStorage, requires a static key, and rejects capturing closures (`src/lib.rs:340-347`). The new call accepts owned defaults and an injected transport/policy.

**Before:**

```rust
let ws = panel_kit::use_workspace(WORKSPACE_LAYOUT_KEY, default_layout);
```

**After:**

```rust
use std::rc::Rc;
use panel_kit::{LocalStorageLayoutStore, WebWorkspaceConfig};
use panel_kit_core::{LayoutStore, Mode, SavePolicy, WorkspaceState};

let initial = WorkspaceState::new(default_layout(), Mode::Floating);
let store: Rc<dyn LayoutStore> = Rc::new(
    LocalStorageLayoutStore::new(WORKSPACE_LAYOUT_KEY)
);
let ws = panel_kit::use_workspace(
    WebWorkspaceConfig::new(initial)
        .with_store(store)
        .save_policy(SavePolicy::OnSettle),
);
```

Existing V1/V2 JSON under the same key still loads, migrates/reconciles, and merges defaults. Do not bump a consumer key merely because the constructor changed.

### 13.2 Breaking change: inject the palette when installing web styles

**What changed:** the color-bearing `CSS` constant is removed. Structural CSS plus a core palette is emitted by `stylesheet`.

**Before:**

```rust
style { {panel_kit::CSS} }
```

**After:**

```rust
let panel_css = panel_kit::stylesheet(
    &panel_kit_core::Palette::DARK,
    &panel_kit_core::TypographyTokens::default(),
);
style { {panel_css} }
```

**Why:** hardcoded CSS, TUI constants, and `DESIGN.md` currently carry three copies of the palette (`assets/panel-kit.css:5-29`; `tui/src/theme.rs:39-74`; `DESIGN.md:4-63`).

### 13.3 Breaking change: TUI store and constructor paths

**What changed:** `panel_kit_tui::LayoutStore` is removed; import `panel_kit_core::LayoutStore`. `TuiWorkspace::new(Option<PathBuf>, fn())` and `with_store(Box<dyn LayoutStore>, fn())` become one config constructor over owned state. Use public `JsonFileLayoutStore` for native files.

```rust
let state = WorkspaceState::new(defaults(), Mode::Tiling);
let store = Box::new(JsonFileLayoutStore::new(path));
let mut ws = TuiWorkspace::new(
    TuiWorkspaceConfig::new(state).with_store(store)
);
```

`Theme::DARK/PAPER` become `Theme::from(&Palette::DARK/PAPER)`. The browser-TUI trait implementation changes only its import and adds `clear`.

### 13.4 Breaking change: renderer-neutral badge data

TUI callers replace `panel_kit_tui::badge::Badge` construction with `panel_kit_core::BadgeSpec`; Dioxus callers may continue using ergonomic component props or pass a `BadgeSpec`. Renderer-neutral `accent_color` accepts `ColorRef`, not an arbitrary CSS string. Web-only callers that intentionally need `var(...)` keep the Dioxus-only prop path.

### 13.5 `jump-cannon` cutover

The current public repository uses two independent workspace hooks with separate localStorage keys and only renders the selected view; it also uses a header-actions render path. Its migration is:

1. Keep the existing `Panel` enum, `PanelKind` impl, `LayoutBuilder` defaults, and saved keys. No enum or layout rewrite is required.
2. Construct `ws_user` and `ws_sessions` with two `WebWorkspaceConfig` values and two `LocalStorageLayoutStore` instances. This preserves the independent user/session layouts.
3. Keep the app-owned topbar, command palette ordering, graph canvas, and view switcher outside panel-kit.
4. Replace any old `render_with_header(body, header_actions)` use with explicit `PanelChrome { children: header_actions(...) }` while iterating the public workspace projection, or use a convenience `render_panels(body, chrome_children)` that is itself public-part composition. Do not add `render_with_header` back as a second monolith.
5. Replace the stylesheet injection as in 13.2, preserving app CSS after it.
6. Keep all pre-panel-kit layout migrations running before `use_workspace`; once they write the current key, core still reads their legacy `{ panels, tiling }` through `StoredLayout::V1`.
7. Build its wasm/Tauri UI, exercise both workspace switches, restore a minimized panel, and confirm old keys load before updating the git revision.

This design also permits a later simplification: the graph canvas can use `PanelSurface` with custom graph chrome while ordinary panels continue through convenience rendering. That is optional migration, not required for the pin update.

### 13.6 `apple-notes-ocr-flow` cutover

The repository documentation identifies apple-notes-ocr-flow as the other git consumer (`README.md:9-16`) and panel-kit’s crate docs state that this shell was extracted from its reviewer UI (`src/lib.rs:1-13`). Its minimal cutover is:

1. Keep its reviewer `Panel` enum/`PanelKind`, `LayoutBuilder` defaults, body match, and existing storage key.
2. Replace its one `use_workspace(key, defaults)` call with `WorkspaceState::new(defaults(), Mode::Floating)` plus `WebWorkspaceConfig` and `LocalStorageLayoutStore` as in 13.1.
3. Keep reviewer-specific OCR stage content as app-owned body bindings; do not move stage names, OCR state, or document actions into core.
4. If its review header injects actions, compose them through `PanelChrome.children`; if it only uses `render(body)` and `dock()`, those convenience calls remain.
5. Convert panel-kit style installation to `stylesheet(&Palette, &TypographyTokens)`. Map any custom palette to all 15 `Palette` fields.
6. Preserve the existing layout key so V1/V2 records load. Verify a previously arranged reviewer workspace before updating the git pin.

### 13.7 Migration notice checklist

Add a `MIGRATION.md` section titled “Migrating from 1.0.0 to 2.0.0” with:

- [ ] Replace both-argument `use_workspace` calls with `WebWorkspaceConfig` and an explicit store.
- [ ] Replace `panel_kit::CSS` with `stylesheet(Palette, TypographyTokens)`.
- [ ] Import `LayoutStore` from `panel_kit_core`; add `clear` to custom stores.
- [ ] Replace TUI path/default constructors with `TuiWorkspaceConfig` and public `JsonFileLayoutStore`.
- [ ] Replace TUI palette constants with `Theme::from(&Palette)`.
- [ ] Replace TUI `Badge` data with core `BadgeSpec`.
- [ ] Replace special render variants with composition of `PanelSurface`, `PanelChrome`, and optional children.
- [ ] Keep current persistence keys; verify V1 and V2 restore before moving the git revision.

## 14. Dependency-ordered TDD decomposition

Each slice starts with the named failing test. Later slices may depend only on green earlier slices.

### Slice 1 — core state container and reducer

**First failing test:** `workspace::tests::reducer_drag_then_settle_matches_existing_free_functions`.

- Create identical panels for a direct-free-function path and reducer path.
- Begin, move, and end a drag.
- Assert identical panels/drag state and `Continuous` then `Settled` phases.
- Add focused commands, reorder, invalid-index no-op, scroll clamp, compact forced-tiling cases.
- Implement `WorkspaceState`, `WorkspaceEnv`, `WorkspaceAction`, and `reduce` only by calling existing operations.

### Slice 2 — projection and single-panel independence

**First failing test:** `workspace::tests::project_panel_requires_no_workspace_chrome`.

- Project one panel from a slice and prove no dock/root data is computed.
- Add workspace ordering/maximize/minimize/focus projection tests.
- Keep projection pure and renderer-neutral.

### Slice 3 — promote `LayoutStore` and centralize lifecycle

**First failing test:** `persistence::tests::restore_v1_reconciles_merges_defaults_and_preserves_keyed_state`.

- Use an in-memory fake implementing the promoted trait.
- Load V1, migrate, merge a newly added panel, save V2, reload, and compare.
- Add invalid viewport/JSON/store-error/clear and `OnSettle` policy tests.
- Move the trait, add `clear`, implement lifecycle helpers, then delete backend duplicate JSON code.

### Slice 4 — backend stores and constructor migration

**First failing test:** `store::tests::json_file_store_round_trips_and_clears` (native scoped test).

- Implement public `JsonFileLayoutStore`.
- Implement web `LocalStorageLayoutStore`; exercise it through the `spec_workspace` browser smoke canary because the root crate is wasm-only.
- Change both shell constructors to owned state/config and capturing-capable values.
- Update examples before removing old signatures; then remove them in the same slice.

### Slice 5 — shell public parts

**First failing test:** `parts::tests::standalone_surface_draws_no_title_lights_grip_or_dock` using a ratatui buffer.

- Extract TUI surface, chrome, lights, grip, dock, and hit testing.
- Test omitted part means omitted pixels and hit zones.
- Extract equivalent Dioxus parts.
- Rebuild both convenience `render` methods solely from parts.
- Add `parts::tests::convenience_projection_matches_manual_all_parts_projection` to prevent facade/parts drift.
- Add a buildable standalone-panel example exercising the concrete composition.

### Slice 6 — palette/token unification

**First failing test:** `theme::tests::dark_palette_emits_all_fifteen_css_variables_once`.

- Add core `Palette`, token enums, and presets.
- Convert TUI theme from palette.
- Generate web palette CSS and remove stylesheet literals.
- Add `theme-parity` only after the core test is green.
- Update theming examples and migration docs; remove TUI preset duplicates.

### Slice 7 — shared badge and widget data models

**First failing test:** `widget::tests::canary_widget_models_serde_round_trip_without_renderer_types`.

- Move/define semantic data in core with serde.
- Consolidate `BadgeSpec` and add serde to existing badge enums.
- Adapt TUI functions, then web widgets, one variant at a time.
- For each variant, the first renderer test asserts observable output or buffer semantics, not field forwarding.
- Delete backend duplicate data structs only after all callers migrate.

### Slice 8 — core `WorkspaceSpec`, validation, and resolver

**First failing test:** `spec::tests::nine_panel_workspace_spec_resolves_unique_copyable_ids_and_stable_persistence_ids`.

- Define the Rust types and complete nine-panel fixture.
- Validate bounds/invariants with JSON-pointer errors.
- Resolve strings once to `SpecPanelId`/catalog.
- Round-trip spec-driven saved layouts through stable strings and test reordered spec declarations do not corrupt saved identity.
- Verify handwritten `PanelKind` enums still compile through the blanket `PanelKey` implementation.

### Slice 9 — pure-Nix `mkWorkspaceSpec`

**First failing check:** `checks.spec-schema` against the smallest Nix spec.

- Implement the pure function and friendly assertions.
- Expand `nix/examples/workspace-spec.nix` field by field to the complete TUI canary.
- Export the API under `lib.${system}`.
- Do not extend the old jq check.

### Slice 10 — semantic `spec-parity`

**First failing check:** `checks.spec-parity` run against the current live fixtures; it must report the 7-vs-9 mismatch before fixture correction.

- Add native `panel-kit-spec compare` and Rust canary emitter.
- Compare deserialized/validated Rust values, not bytes.
- Replace/delete the old Nix canary and jq job only after the expected red proof is captured.
- Make the new complete Rust/Nix fixtures equal.

### Slice 11 — backend spec runners and canary matrix

**First failing check:** `checks.spec-tui-example`.

- Instantiate TUI from the Nix JSON and binding registry.
- Then add `spec-browser-tui-example` and `spec-web-example` wasm builds.
- A declared `ContentSpec` without a renderer or required binding makes its backend job fail; no placeholder/no-op renderer is accepted.

### Slice 12 — consumer-facing migration and cleanup

**First failing example build:** existing `examples/workspace.rs` after old constructor/CSS removal.

- Migrate all repository examples and rustdocs.
- Add the 2.0 `MIGRATION.md` section and update README’s library-first composition story.
- Delete obsolete code and generated throwaway artifacts listed in section 12.
- Build external consumers only after their call sites are migrated; move git revisions last.

## 15. Verifiable acceptance criteria

| ID | Criterion | Proof |
| --- | --- | --- |
| AC-01 | Reducer drag, resize, reorder, restore, focus command, mode command, and scroll results match existing core functions, including no-op invalid indices. | Core unit tests from Slice 1. |
| AC-02 | `WorkspaceState` has no renderer, store, theme, metadata/catalog, viewport policy, or callback fields; free functions remain callable without it. | Core compile/unit test using `project_panel` and free functions over a bare panel slice. |
| AC-03 | A standalone panel can render with a surface but no title, traffic lights, grip, root, or dock, and omitted controls have no hit zones. | TUI buffer unit test `standalone_surface_draws_no_title_lights_grip_or_dock`; web standalone-panel wasm example build. |
| AC-04 | Convenience full rendering and manual all-parts composition consume the same projection and produce equivalent TUI buffers/DOM structure. | Parts parity unit test plus web example build. |
| AC-05 | `LayoutStore` is defined only in core; web localStorage, TUI file, and browser-TUI adapters implement it, including `clear`. | Core fake-store tests, native file-store unit test, `spec-web-example`, and `spec-browser-tui-example`. |
| AC-06 | Existing V1 and V2 persisted records under unchanged keys restore; V1 migrates, foreign units reconcile, and new default panels merge. | Core lifecycle round-trip unit tests using known V1/V2 fixtures. |
| AC-07 | `OnSettle` never writes during pointer motion/reorder hover and writes after pointer-up/discrete commands; `Manual` never auto-writes. | Core fake-store write-count transition tests. |
| AC-08 | Core `Palette::DARK` is the sole numeric dark palette source; CSS and TUI derive from it; all 15 documented tokens agree. | Core CSS-variable unit test and `checks.theme-parity`. |
| AC-09 | `WorkspaceSpec` validates and resolves all fields in section 8, and `SpecPanelId` rendering/reduction performs no per-frame string allocation. | Core spec/resolver tests; review of resolver API returning borrowed catalog strings and copy IDs. |
| AC-10 | A spec-driven saved layout uses stable string IDs and survives a reorder of panel declarations without assigning state to the wrong panel. | Core persistence unit test. |
| AC-11 | The Nix surface expresses every row of the completeness table, including all nine TUI panels/widgets/bindings and all five web body bindings. | `checks.spec-schema` over complete Nix examples. |
| AC-12 | `spec-parity` fails on the live seven-versus-nine fixture and, after correction, compares all fields semantically with useful JSON-pointer diagnostics. | Captured initial failing `checks.spec-parity`, then green `checks.spec-parity`; CLI unit tests for object-order/number-spelling normalization and array/value mismatches. |
| AC-13 | The Nix-authored complete spec builds on native TUI, browser-TUI wasm, and Dioxus wasm; no content kind is silently ignored. | `checks.spec-tui-example`, `checks.spec-browser-tui-example`, `checks.spec-web-example`. |
| AC-14 | Handwritten enum users remain supported: `PanelKind`, `LayoutBuilder`, existing core functions, `Workspace::render`, and `Workspace::dock` work after constructor/style migration. | Migrated `examples/workspace.rs` wasm build and core compile tests. |
| AC-15 | Two independent workspaces can be mounted concurrently with distinct state/store instances and no cross-save. | Web example or focused browser smoke scenario that mutates one workspace and observes only its store write. |
| AC-16 | The repository contains no old TUI `LayoutStore`, private web save/load functions, duplicate hardcoded TUI palette presets, duplicate TUI Badge data struct, old Nix hand-mirror, or jq-only parity check. | Targeted source assertions in `spec-schema`/`theme-parity` where behavioral, plus code review during cleanup; no source-text-only permanent Rust tests. |
| AC-17 | `jump-cannon` preserves both layout keys and both independent workspaces; apple-notes preserves its reviewer layout key and old V1/V2 data. | Each consumer’s wasm/application build plus a manual persisted-layout restore smoke scenario before moving its git pin. |
| AC-18 | Ordinary Rust consumers require no Nix at build or runtime; Nix-authored consumers require Nix only to produce the embedded JSON artifact. | Cargo example built directly without Nix-generated inputs, plus flake spec example builds. |

## 16. Explicit non-goals

1. **No new async runtime or async store trait.** Current stores are local file/localStorage and synchronous. Async remote persistence needs separate evidence.
2. **No general UI programming language in Nix.** Rust/Dioxus elements, ratatui closures, network calls, timers, and OCR/graph business logic remain named bindings.
3. **No replacement of the already-landed surface, keyboard-command, layout V2, migration, reconciliation, or layout-builder APIs.** This design composes them.
4. **No runtime Nix evaluator, Nix FFI, or Nix daemon dependency.**
5. **No inheritance hierarchy, renderer super-trait, plugin container, service locator, or all-capabilities `Workspace` trait.**
6. **No arbitrary CSS strings in renderer-neutral spec data.** Web-only component escape hatches may remain web-only.
7. **No promise of pixel-identical DOM and terminal rendering.** Parity is semantic spec/state/action parity; each backend renders appropriately in pixels or cells.
8. **No persistence of application content data.** `SavedLayoutV2` continues to persist layout only.
9. **No automatic migration of consumer git pins.** Consumers update only after their code and stored-layout smoke checks pass.
10. **No broad charting framework.** Widget models stop at the shapes the actual canaries exercise: time series, gauges, flame, boxplot, table/status/meter, text/scroll, badges, and spinner.

## 17. Self-verification questions and answers

### Q1. Does `spec-parity` actually fail on the current 7-vs-9 drift?

**Yes.** It compares ordered, validated `WorkspaceSpec` Rust values. Panel arrays are not normalized as sets. The current Rust value has `Flame` and `Distribution` and nine entries (`workspace_canary.rs:36-50`); current Nix has seven (`workspace-canary.nix:24-32`). It therefore fails before any renderer build, and full comparison also catches the stale coordinates. A jq schema assertion cannot pass in its place.

**Gap fixed while expanding A1:** the original proposal suggested deserialize/re-serialize plus golden JSON, which could still pass shape-only or flake on float spelling. This design requires an independently emitted Rust value, semantic comparison, and JSON-pointer differences.

### Q2. Can an application bypass every piece of panel-kit chrome?

**Yes.** The smallest API is `project_panel`, which returns geometry/state only. The app may draw everything itself. If it wants only the background/border/body bounds, it adds `PanelSurface`/`draw_panel_surface`. Header, lights, grip, root, and dock are separate calls and have separate optional hit zones. `PanelSurface` never calls them.

**Gap fixed:** merely making today’s private `header` public would still force the panel loop and dock policy through `Workspace::render`; the new narrow projection and surface part remove that hidden ownership.

### Q3. Is current `PanelKind` compatible with a Nix-authored panel set?

**No.** Its `Copy` and `&'static str` title contract excludes owned runtime strings (`core/lib.rs:37-42`). Claiming a built-in `String` implementation, as the original A1 sketch did, would not compile.

**Fix:** retain `PanelKind` for ergonomic enum consumers, introduce title-free `PanelKey`, resolve strings once to copyable `SpecPanelId`, and keep titles/stable IDs in `SpecPanelCatalog`. Saved layouts translate through stable strings, never ephemeral indices. This is the smallest identity addition that supports both existing consumers and data-driven specs without per-frame allocation.

### Q4. Is `WorkspaceState` now another mandatory god object?

**No.** It contains six mutable domain fields and has no dependencies. Stores, palette, spec/catalog, environment, bindings, renderer state, and callbacks are separate. Fields and free functions are public, and `project_panel` accepts a panel slice rather than the container. Convenience shells use the aggregate because coordinated interactions need those fields, but a library user can use any subset.

**Gap fixed:** the design explicitly forbids adding render/persist/theme methods to core `WorkspaceState`; those belong to free functions and injected adapters.

### Q5. Can the spec honestly represent dynamic charts and arbitrary app panels without becoming a framework?

**Yes.** Static widget data is inline; live data is a typed named binding returning borrowed values; arbitrary bodies are backend-specific named bindings. The Nix file declares the required contract and placement, while the application retains control flow and code. Missing or type-wrong bindings fail initialization/checks rather than rendering a no-op.

**Gap fixed:** this avoids both false “Nix serializes Rust closures” completeness and a giant interpreter embedded in core.

### Q6. Do existing saved layouts survive the identity and constructor changes?

**Yes for existing enum consumers.** `PanelKind` and the `SavedLayoutV2<K>` wire shape stay intact; consumers construct stores with their existing key/path. Core still uses `StoredLayout`, `migrate_v1`, `reconcile_units`, and `merge_defaults`. The new numeric `SpecPanelId` is never serialized; spec-driven persistence uses stable string IDs through its catalog.

## 18. Changes from the original A1 proposal

1. **Corrected repo drift:** `SurfaceProfile`/`effective_mode`, keyboard commands, V2 persistence, V1 migration, unit reconciliation, and V2 `mkLayout` are treated as existing. `LayoutStore` is explicitly promoted/redesigned from TUI, not introduced.
2. **Constrained `WorkspaceState`:** it is a small public data aggregate with an optional reducer, not the home of stores, rendering, theme, metadata, or policy.
3. **Made the escape hatch narrower:** A1’s `panel_element(ws, idx, body)` still depended on the workspace handle. This design adds `project_panel` and `PanelSurface`/`draw_panel_surface`, which can be used from a panel slice and do not imply chrome.
4. **Corrected the `PanelKind` feasibility error:** a built-in string kind cannot satisfy current bounds. `PanelKey` + indexed catalog supports Nix strings while preserving the enum path.
5. **Strengthened parity:** replaced golden/byte comparison with full semantic Rust-value comparison, explicit ordered panel checks, and diagnostics that fail on the live 7-vs-9 drift.
6. **Deleted rather than extended the false canary:** the old Nix mirror and jq check go away; a complete spec example and semantic check replace them.
7. **Closed widget/theme completeness gaps:** semantic widget/badge data and palette move to core; both backends render the complete canary surface. Dynamic algorithms remain bindings, not core policy.
8. **Made migration concrete:** exact old/new constructors, stylesheet/store/TUI/badge breaks, saved-layout compatibility, and separate jump-cannon/apple-notes cutovers are specified in the existing clean-cutover style.
9. **Bounded the check matrix:** one complete spec is checked across the three supported surfaces rather than multiplying every spec by every backend. This respects the constrained Hydra systems described at `flake.nix:19-30` while retaining canary coverage.
