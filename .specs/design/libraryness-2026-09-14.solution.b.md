# Solution B — Headless Event Reducer + Borrowed Frame Projection

**Selected proposal:** C-B, “Headless Event Reducer + Frame Projection; Backends as Widget Kits”  
**Status:** implementation-ready design for `/plan-task` → `/implement-task`  
**Shared vocabulary:** the complete Nix-authored description is **WorkspaceSpec**; the mechanized Nix/Rust check is **spec-parity**.

## 1. Problem statement and verified baseline

panel-kit already has useful renderer-neutral parts, but the public adoption path is still a framework-shaped shell.

- Core already owns `PanelWin<K>`, `Mode`, `WinState`, `PointerEvent`, `Region`, `WorkspaceChrome`, `ChromeMetrics`, and the geometry/interaction free functions (`crates/panel-kit-core/src/lib.rs:335-459`, `crates/panel-kit-core/src/lib.rs:502-707`, `crates/panel-kit-core/src/lib.rs:717-971`). The new design composes these; it does not re-propose them.
- Core already owns `SurfaceProfile` and `effective_mode` (`crates/panel-kit-core/src/lib.rs:68-165`), the keyboard command layer (`FocusContext`, `Key`, `KeyChord`, `PanelCommand`, `command_for`, `apply_command`, `crates/panel-kit-core/src/lib.rs:167-332`, `crates/panel-kit-core/src/lib.rs:864-971`), and V2 persistence (`SavedLayoutV2`, `StoredLayout`, `migrate_v1`, `reconcile_units`, `crates/panel-kit-core/src/lib.rs:973-1063`). These remain authoritative.
- The web API still bundles eight Dioxus signals in `Workspace<K>` and asks the application to surrender the panel subtree to `Workspace::render(body)` (`src/lib.rs:285-323`, `src/lib.rs:766-930`). `use_workspace` also owns restoration, `ResizeObserver`, resize scaling, and settle-time persistence (`src/lib.rs:332-470`).
- The TUI has the same ownership problem: `TuiWorkspace<K>` owns panels, mode, focus, theme, drag, store, viewport, hit zones, hover, charset, and scroll state (`crates/panel-kit-tui/src/lib.rs:186-222`), while `TuiWorkspace::render` computes layout, paints chrome, invokes the body callback, and records hit zones (`crates/panel-kit-tui/src/lib.rs:335-660`).
- Persistence is duplicated and misplaced. Web has private localStorage-specific `save_layout`/`load_layout` (`src/lib.rs:249-283`). `LayoutStore` already exists, but only in the TUI backend (`crates/panel-kit-tui/src/lib.rs:144-171`). It must move to core rather than be invented again.
- The renderer shells currently allocate during projection. Web clones the complete panel vector and collects visible indices in every `Workspace::render` (`src/lib.rs:776-789`); TUI creates visible/order, tile layout, and minimized-panel vectors while rendering (`crates/panel-kit-tui/src/lib.rs:389-440`, `crates/panel-kit-tui/src/lib.rs:617-641`).
- Widget and theme data are asymmetric. The shared badge module models `BadgeKind` and actions (`crates/panel-kit-core/src/badge.rs:14-122`), but web takes twelve component parameters (`src/badge.rs:113-160`) while TUI adds a second, smaller `Badge` struct (`crates/panel-kit-tui/src/badge.rs:11-38`). `charts`, `meter`, `scroll`, `status`, and `table` are TUI-only exports (`crates/panel-kit-tui/src/lib.rs:35-42`).
- Theme values have three representations. CSS declares fifteen semantic colors plus size/font tokens (`assets/panel-kit.css:5-29`); TUI `Theme` repeats thirteen colors and omits `inv-bg`/`inv-fg` (`crates/panel-kit-tui/src/theme.rs:7-37`); `DESIGN.md` repeats the palette and typography values (`DESIGN.md:1-63`).
- Nix is layout-only. `mkLayout` produces the `SavedLayoutV2` fields and panel geometry (`nix/lib/mkLayout.nix:1-43`, `nix/lib/mkLayout.nix:71-135`), while the current check only runs `jq` over that Nix-produced JSON (`flake.nix:118-149`). It never asks Rust to decode or project the value.
- That check is demonstrably green over drift: the Nix canary lists seven panels (`nix/examples/workspace-canary.nix:24-32`) and even asserts seven (`flake.nix:135-136`), while `workspace_canary.rs::defaults` constructs nine, including `Flame` and `Distribution` (`crates/panel-kit-tui/examples/workspace_canary.rs:7-18`, `crates/panel-kit-tui/examples/workspace_canary.rs:36-50`). The Nix `Notes` and `Theme` positions are stale as well.
- The web canary exercises persistence, five panel identities/titles, geometry, initial minimized state, tile spans, arbitrary DOM bodies, focus, keyboard commands, native body scroll, workspace scroll, restore/reset, responsive profiles, and a tooltip (`examples/workspace.rs:6-33`, `examples/workspace.rs:65-102`, `examples/workspace.rs:109-312`). A complete specification surface must account for all of these without pretending arbitrary application code is serializable.

The root problem is not missing methods. It is the ownership boundary: applications should own state storage, event order, effects, and the render tree. Core should reduce renderer-neutral input and project a semantic frame. Backends should translate native events and paint individual semantic pieces with their native widget systems.

## 2. Core mechanism and design decisions

### 2.1 Non-negotiable mechanism

1. The host owns a plain `Snapshot<K>` and decides where it lives: a Dioxus `Signal`, a struct in a crossterm loop, an ECS resource, or no aggregate at all.
2. The host translates native input through a backend adapter and synchronously calls core `reduce`.
3. Core mutates only the supplied snapshot and returns a small effect description. It performs no I/O and calls no renderer.
4. The host calls `project_into` using reusable scratch storage. Core returns a borrowed, semantic `ProjectedFrame`: resolved workspace/chrome/dock regions, ordered visible panels, tile placement, z-order, interaction/focus flags, panel chrome hit regions, and content extents.
5. The host chooses which backend widgets to call and in what tree/order. Web widgets emit semantic DOM; TUI widgets use ratatui widgets. The projected frame is not a cell buffer, a VDOM, or a lowest-common-denominator drawing-command tape.

The optional `recipes` modules demonstrate the old complete workspace composition, but expose no controller with a `render` method. A recipe may wire a `Signal<Snapshot<K>>`, a `ProjectionBuffer`, store effects, and backend adapters; each remains separately accessible and replaceable.

### 2.2 Answer to judge concerns

| Concern | Design answer |
| --- | --- |
| C-B overlaps a pure spec compiler | **WorkspaceSpec is an input adapter, not the runtime architecture.** `Snapshot`, `reduce`, `project_into`, and every widget work in ordinary Cargo-only Rust without Nix or WorkspaceSpec. Nix evaluates once at build time and emits JSON that Rust deserializes. There is no code generator, capability lattice, runtime interpreter, or Nix-owned control flow. Hand-authored Rust definitions use the same core pieces. |
| Frame projection may allocate every render | Projection is `project_into(&Snapshot, ..., &mut ProjectionBuffer) -> ProjectedFrame<'_>`. Scratch vectors are reserved once and reused. Steady-state projection performs **zero heap allocations and zero panel-ID/string clones**; it writes numeric/index records into existing capacity and sorts them in place. Growth occurs only when panel/row count exceeds reserved capacity. |
| Frame may reduce backends to a lowest common denominator | The frame is semantic layout, not pixels/glyphs/nodes. It exposes outer/body/control regions and tile placement, but not how to paint them. TUI still uses `Block::title`, `Block::inner`, `Table`, `Chart`, `Gauge`, and `Scrollbar`. Web still uses semantic `section`/`button`, keyed DOM nodes, focus, CSS Grid, and native `overflow:auto`. Backend-only escape hatches remain public. |
| Original proposal cited `ROW_CELLS`, which is not a current symbol | The citation is removed. The current TUI renderer reads `TileMetrics::CELLS.row` directly (`crates/panel-kit-tui/src/lib.rs:411-430`); `MIGRATION.md:389-408` explicitly records deletion of the old local constant. No API in this design is based on `ROW_CELLS`. |
| `PanelId(Arc<str>)` conflicts with current `PanelKind: Copy` | This is corrected explicitly. Current `PanelKind` requires `Copy` and a static title (`crates/panel-kit-core/src/lib.rs:31-43`), so an `Arc<str>` cannot implement it. The clean cutover replaces title-bearing `PanelKind` with cloneable `PanelKey`; titles/slugs move to catalog data. `PanelId(Arc<str>)` then satisfies the new identity contract and remains allocation-free during projection. |
| No god-object replacement | `Snapshot` is public plain data, not a handle; all existing lower-level free functions remain. `project_panel`, `project_chrome`, and `project_dock_into` allow partial/single-panel composition without constructing a whole workspace frame. Neither backend introduces a mandatory state owner. |

## 3. Target crate and module layout

```text
crates/panel-kit-core/src/
  lib.rs                 re-exports; existing primitives retained
  reducer.rs             Snapshot, WorkspaceEvent, ReduceContext, reduce
  frame.rs               ProjectionBuffer, ProjectedFrame, partial projectors, hit_test
  panel.rs               PanelKey, PanelId, PanelMeta, PanelCatalog
  persist.rs             moved LayoutStore, V1/V2 load/save helpers
  theme.rs               ThemeTokens and renderer-neutral Color
  widgets/
    mod.rs                WidgetSpec, DataSource, WidgetView
    badge.rs              one complete BadgeSpec + existing action semantics
    charts.rs             Series/Flame/Box/Gauge models and validation
    table.rs              renderer-neutral cells/column sizing
    meter.rs status.rs scroll.rs spinner.rs
  spec.rs                WorkspaceSpec serde type, validation, resolution

src/                     `panel-kit`, Dioxus backend only
  lib.rs                 backend re-exports, no Workspace controller
  input.rs               Dioxus → WorkspaceEvent; DOM focus/wheel classification
  theme.rs               ThemeTokens → CSS custom properties
  widgets/
    panel.rs dock.rs badge.rs charts.rs table.rs meter.rs status.rs scroll.rs spinner.rs
  recipes.rs             optional hook/effect assembly, no render ownership
  assets/panel-kit.css   structural stylesheet; no duplicated default palette literals

crates/panel-kit-tui/src/
  lib.rs                 backend re-exports, no TuiWorkspace controller
  input.rs               crossterm/ratzilla → WorkspaceEvent; frame hit-test adapter
  theme.rs               ThemeTokens → ratatui Color conversion only
  widgets/
    panel.rs dock.rs badge.rs charts.rs table.rs meter.rs status.rs scroll.rs spinner.rs
  recipes.rs             optional event-loop assembly, no state/render owner

nix/lib/
  mkLayout.nix           retained as the V2 persisted-layout builder
  mkWorkspaceSpec.nix    complete WorkspaceSpec builder and eager Nix assertions
nix/examples/
  workspace-spec-canary.nix   sole declarative canary workspace description
```

Every new abstraction enables a concrete composition:

- `Snapshot` + `reduce`: one interaction grammar can live in a Dioxus signal, terminal loop, or app store.
- `ProjectionBuffer` + borrowed frame: both backends share layout semantics without render-time heap churn.
- partial projectors: a consumer can render one panel, its own dock, or no built-in chrome.
- `PanelCatalog`: runtime Nix strings and hand-written Rust enums share metadata without static-title constraints.
- `ThemeTokens`: one palette drives CSS and ratatui.
- shared widget models: a Nix widget declaration and dynamic Rust data can be painted by either backend, while backend-native body widgets remain possible.
- `LayoutStore` in core: the same V1/V2 transport path works for localStorage, files, memory, or an application store.
- WorkspaceSpec: complete build-time configuration interchange, not required for ordinary library use.

## 4. Core identity, state, and reducer API

### 4.1 Panel identity and metadata

```rust
pub trait PanelKey:
    Clone + Eq + std::hash::Hash + Serialize + DeserializeOwned + 'static
{}
impl<T> PanelKey for T where
    T: Clone + Eq + std::hash::Hash + Serialize + DeserializeOwned + 'static
{}

#[derive(Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PanelId(pub std::sync::Arc<str>);

pub struct PanelMeta<K> {
    pub key: K,
    pub stable_id: std::sync::Arc<str>,
    pub title: std::sync::Arc<str>,
    pub slug: std::sync::Arc<str>,
    pub content: Vec<WidgetSpec>,
}

pub struct PanelCatalog<K> {
    entries: Vec<PanelMeta<K>>,
    by_key: std::collections::HashMap<K, usize>,
    by_stable_id: std::collections::HashMap<std::sync::Arc<str>, usize>,
}
impl<K: PanelKey> PanelCatalog<K> {
    pub fn try_new(entries: Vec<PanelMeta<K>>) -> Result<Self, CatalogError>;
    pub fn get(&self, key: &K) -> Option<&PanelMeta<K>>;
    pub fn get_by_id(&self, stable_id: &str) -> Option<&PanelMeta<K>>;
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &PanelMeta<K>>;
    pub fn len(&self) -> usize;
}
```

`PanelId` requires serde's existing dependency to enable its `rc` feature so `Arc<str>` serializes as the stable string. That is a compile-time feature change, not a new runtime dependency.

`PanelKey` has no title method. Hand-written enum users create a catalog; Nix resolution creates `PanelId` values and a catalog once. Duplicate `stable_id`, duplicate keys, empty IDs/titles, invalid slugs, or layout references with no catalog entry fail before state is constructed. Slugs default through the existing `kind_slug` algorithm but may be supplied explicitly when CSS compatibility requires it.

`PanelWin<K>` changes from `Copy` to `Clone`; functions that currently require `Copy` take borrowed keys or clone only when state ownership truly changes. This is a named breaking change. Projection stores source indices and catalog indices, so it never clones `K`, `Arc<str>`, title, slug, or content on a frame.

### 4.2 State and events

```rust
#[derive(Clone)]
pub struct Snapshot<K> {
    pub panels: Vec<PanelWin<K>>,
    pub preferred_mode: Mode,
    pub viewport: Viewport,
    pub focused: Option<K>,
    pub drag: Option<Drag>,
    pub tile_drag: Option<K>,
    pub workspace_scroll: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub width: f64,
    pub height: f64,
    pub units: Units,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelPart { Body, Header, ResizeGrip, Mode, Minimize, Maximize }

#[derive(Clone, Debug, PartialEq)]
pub enum HitTarget<K> {
    Workspace,
    Panel { key: K, part: PanelPart },
    Dock { key: K },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WheelDisposition {
    ContentConsumed,
    BubbleToWorkspace,
}

#[derive(Clone, Debug, PartialEq)]
pub enum WorkspaceEvent<K> {
    Pointer { target: HitTarget<K>, event: PointerEvent },
    Key { chord: KeyChord, focus: FocusContext<K> },
    Command { target: Option<K>, command: PanelCommand },
    Wheel { delta_y: f64, disposition: WheelDisposition },
    ViewportChanged(Viewport),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReduceContext<'a> {
    pub surface: SurfaceProfile,
    pub clamp: &'a Clamp,
    pub tile: &'a TileMetrics,
    /// Copy of the most recently projected grid; supplies the actual dynamic
    /// row/column step while a tiled resize is in flight.
    pub tile_grid: Option<TileGridProjection>,
    pub command_step: CommandStep,
    pub resize_policy: ResizePolicy,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reduction<K> {
    pub changed: bool,
    pub layout_settled: bool,
    pub focus_request: Option<FocusRequest<K>>,
}

pub fn reduce<K: PanelKey>(
    snapshot: &mut Snapshot<K>,
    event: WorkspaceEvent<K>,
    context: ReduceContext<'_>,
) -> Reduction<K>;
```

`reduce` is a dispatcher over existing core behavior:

- `Key` calls existing `command_for`, then `apply_command` with the supplied unit-specific step.
- `Command` provides programmatic restore/mode/minimize/maximize without synthesizing keyboard input.
- Pointer-down on a floating header/grip calls `begin_drag`; a tiled grip calls `begin_tile_resize`; pointer drag calls `apply_drag`; pointer enter over a panel during tile drag calls `reorder_tile`; pointer-up settles both drag kinds.
- `Wheel(ContentConsumed)` is a no-op. `Wheel(BubbleToWorkspace)` updates only workspace scrolling. The host, which knows the DOM element or app-owned TUI body offset, decides disposition.
- `ViewportChanged` updates the viewport, recomputes the caller-supplied surface, and either applies the current web ratio-scaling policy or preserves stored geometry. Invalid/non-finite or non-positive dimensions update no geometry and return `changed = false`.
- `Reduction` describes effects but performs none. `focus_request` lets web focus a restored dock chip or panel using the DOM; terminal hosts may update their own focus. `layout_settled` is the host’s signal to persist. There is no store inside the reducer.

Event growth is bounded by category: platform-specific raw events remain in adapters; application events remain in the application; widget actions such as `BadgeAction` remain widget actions. Only inputs that change shared workspace state belong in `WorkspaceEvent`.

### 4.3 No mandatory aggregate

The aggregate is convenience, not monopoly:

```rust
pub fn reduce_pointer<K: PanelKey>(
    panels: &mut Vec<PanelWin<K>>,
    drag: &mut Option<Drag>,
    tile_drag: &mut Option<K>,
    event: PointerEvent,
    target: HitTarget<K>,
    context: ReduceContext<'_>,
) -> Reduction<K>;

pub fn project_panel<K: PanelKey>(
    panel: &PanelWin<K>,
    source_index: usize,
    input: PanelProjectionInput<'_>,
) -> Option<PanelProjection>;

pub fn project_chrome(viewport: Viewport, metrics: &ChromeMetrics) -> WorkspaceChrome;
pub fn project_dock_into<K: PanelKey>(
    panels: &[PanelWin<K>],
    catalog: &PanelCatalog<K>,
    out: &mut Vec<DockProjection>,
);
```

A host rendering one inspector panel can call `project_panel` and `widgets::panel`; an application with its own navigation can omit dock projection; a read-only dashboard can call `project_into` but never `reduce`; a custom renderer can call only existing geometry functions.

## 5. Frame projection and allocation/lifetime contract

### 5.1 Public shape

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Placement {
    Floating,
    Tiled { column: u8, row: u16, column_span: u8, row_span: u8 },
    Maximized,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelChromeProjection {
    pub outer: Region,
    pub body: Region,
    pub header_hit: Region,
    pub mode_hit: Region,
    pub minimize_hit: Region,
    pub maximize_hit: Region,
    pub resize_hit: Region,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileLayoutMetrics {
    pub resize: TileMetrics,
    pub columns: u8,
    pub row_min: f64,
    pub gap: f64,
    pub padding: f64,
    pub fill_viewport: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileGridProjection {
    pub columns: u8,
    pub rows: u16,
    pub track_w: f64,
    pub track_h: f64,
    pub gap: f64,
    pub padding: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelProjection {
    pub source_index: usize,
    pub catalog_index: usize,
    pub region: Region,
    pub placement: Placement,
    pub z: i32,
    pub state: WinState,
    pub focused: bool,
    pub pointer_dragging: bool,
    pub tile_dragging: bool,
    pub chrome: PanelChromeProjection,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DockProjection {
    pub source_index: usize,
    pub catalog_index: usize,
    pub region: Region,
}

pub struct ProjectionBuffer {
    panel_order: Vec<usize>,
    panels: Vec<PanelProjection>,
    dock: Vec<DockProjection>,
    tile_rows: Vec<TileRowScratch>,
}

impl ProjectionBuffer {
    pub fn with_panel_capacity(panels: usize) -> Self;
    pub fn reserve_panels(&mut self, additional: usize);
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FrameStatus {
    Ready,
    TooSmall { minimum: Viewport },
}

pub struct ProjectedFrame<'a, K> {
    pub status: FrameStatus,
    pub chrome: WorkspaceChrome,
    pub mode: Mode,
    pub surface: SurfaceProfile,
    pub tile_grid: Option<TileGridProjection>,
    pub content_extent: Region,
    pub workspace_scroll: f64,
    pub panels: &'a [PanelProjection],       // already in paint order
    pub dock: &'a [DockProjection],
    snapshot: &'a Snapshot<K>,
    catalog: &'a PanelCatalog<K>,
}

impl<'a, K: PanelKey> ProjectedFrame<'a, K> {
    pub fn key(&self, panel: &PanelProjection) -> &'a K;
    pub fn meta(&self, panel: &PanelProjection) -> &'a PanelMeta<K>;
}

pub struct ProjectionInput<'a, K> {
    pub snapshot: &'a Snapshot<K>,
    pub catalog: &'a PanelCatalog<K>,
    pub surface: SurfaceProfile,
    pub chrome: &'a ChromeMetrics,
    pub clamp: &'a Clamp,
    pub tile: &'a TileLayoutMetrics,
}

pub fn project_into<'a, K: PanelKey>(
    input: ProjectionInput<'a, K>,
    scratch: &'a mut ProjectionBuffer,
) -> ProjectedFrame<'a, K>;

pub fn hit_test<K: PanelKey>(
    frame: &ProjectedFrame<'_, K>,
    point: (f64, f64),
) -> Option<HitTarget<K>>;
```

### 5.2 Allocation guarantees

`ProjectionBuffer::with_panel_capacity(snapshot.panels.len())` allocates at initialization. Each projection clears lengths without dropping capacity, fills records, and uses allocation-free `sort_unstable_by_key(|i| (z, source_index))` on `panel_order`; the unique source index makes the result deterministic despite the unstable sort. No title, slug, ID, `PanelWin`, content model, or widget data is cloned. `ProjectedFrame` cannot outlive either snapshot/catalog or scratch and cannot be retained across mutation; Rust enforces this through the returned borrow.

**Steady-state core projection cost:** no heap allocation; $O(n)$ traversal, $O(n \log n)$ in-place z sort only in non-maximized floating mode, and $O(n)$ tile layout. A later optimization may use counting/order reuse only if profiling justifies it. Adding panels beyond capacity may reallocate the scratch vectors once; hosts with a known upper bound reserve that bound. The optional diagnostics/test-only `OwnedFrame` conversion may allocate and is never used by backend render loops.

This contract is covered by an allocator-counted core test after one warm-up projection. Backend VDOM creation or ratatui widget-internal allocation is outside the core guarantee and must not be attributed to projection.

### 5.3 Native affordances are preserved

- **Ratatui:** `widgets::panel` receives `PanelProjection::region`, creates a native `Block`, attaches titles/control spans with `Block::title`, calls `Block::inner`, and renders the application’s native widget into that rectangle. In debug/test builds it asserts the native inner rect matches `chrome.body` after unit conversion. `Table`, `Chart`, `Gauge`, `Scrollbar`, and direct `Frame::buffer_mut` remain valid inside the body. The frame specifies semantics and hit regions, not glyphs.
- **DOM:** `widgets::panel` emits keyed `section`, toolbar buttons, and a `.panel-body`. `focused` maps to classes/ARIA, but browser focus remains actual DOM focus. `.panel-body` retains `overflow:auto`; content scroll offsets stay in the DOM. The adapter inspects whether the native body can scroll and emits `WheelDisposition`, preserving the current wheel-chain behavior described at `src/lib.rs:728-754`.
- **CSS Grid:** core first auto-places spans into a fixed column count, counts the required tracks, and resolves `track_w`/`track_h` from workspace size, padding, gap, and `row_min`. `ProjectedFrame::tile_grid` and each panel’s explicit row/column/span are therefore complete. The web kit emits explicit `grid-template-columns`, `grid-template-rows`, and placements from those values; CSS still provides intrinsic content sizing and native overflow, but it no longer invents placement or track count. TUI maps the same track plan to cell rectangles. A backend must report/debug-fail if its measured region differs from the projected region beyond its documented rounding tolerance.
- **Host-owned content:** a panel body is never converted into a frame draw list. Arbitrary Dioxus components, canvas/WebGPU, native ratatui widgets, text inputs, and backend-specific focus/scroll behavior remain application-owned.

For a tiled grid with $R$ required tracks, the shared resolver uses
`track_h = max(row_min, (workspace_h - 2*padding - (R-1)*gap) / R)` when `fill_viewport` is true and `track_h = row_min` otherwise. It uses `track_w = max(col_floor, (workspace_w - 2*padding - (columns-1)*gap) / columns)`. Content extent is the larger of the workspace and the resulting padded track extent. The same resolved track dimensions feed span-resize deltas, so projection and interaction cannot disagree. This intentionally replaces the web stylesheet’s implicit `minmax(..., 1fr)` placement and the TUI render loop’s private row packing with one core algorithm while retaining CSS Grid as the DOM painting primitive.

### 5.4 Projection edge cases

- Zero/too-small viewport returns chrome plus `FrameStatus::TooSmall`; panels/dock are empty. No negative or NaN region is emitted.
- A maximized panel is the only projected panel and fills the workspace region. Dock still projects minimized panels.
- Duplicate maximized states are rejected when loading/spec resolution occurs; for defensive projection, the frontmost maximized panel wins deterministically.
- Missing catalog metadata produces a validation error before projection; `project_into` has no per-frame error allocation.
- Tiled spans clamp to `SurfaceProfile::tile_columns()` and `TILE_H_MAX`; source state remains unchanged.
- Floating stored geometry remains intent. Only projected regions clamp, matching the existing `effective_rect` contract (`crates/panel-kit-core/src/lib.rs:659-674`).
- Stable paint order is `(z, source_index)` for floating and source order for tiling. Equal z values are deterministic.

## 6. Persistence as a separate capability

Move the existing trait unchanged in spirit from TUI to `panel-kit-core::persist`:

```rust
pub trait LayoutStore {
    fn load(&self) -> Result<Option<String>, String>;
    fn save(&self, json: &str) -> Result<(), String>;
}

pub fn load_snapshot<K: PanelKey>(
    store: &dyn LayoutStore,
    defaults: &Snapshot<K>,
    viewport: Viewport,
) -> Result<Snapshot<K>, LayoutError>;

pub fn save_snapshot<K: PanelKey>(
    store: &dyn LayoutStore,
    snapshot: &Snapshot<K>,
) -> Result<(), LayoutError>;
```

These helpers continue to use existing `StoredLayout`, `migrate_v1`, `reconcile_units`, and `merge_defaults`; V1 and V2 persisted JSON remain readable. Backends keep only transport adapters: `LocalStorageStore`, `FileLayoutStore`, and optional in-memory examples. Reducer/projector users can ignore persistence completely or serialize `SavedLayoutV2` themselves.

`PersistenceSpec` does not name a filesystem path or construct a backend. It declares `enabled`, logical `key`, and `save_policy` (`manual`, `on_settle`, or `on_change`). The host maps that policy to a supplied store. Thus a pure Nix spec describes intended policy without creating I/O or a runtime Nix dependency.

## 7. Widget-kit and theme unification

### 7.1 One semantic model, native painters

Core owns serializable, renderer-neutral **configuration/data models**. Each backend owns paint functions. The host calls the function it wants; a convenience `paint_builtin` match is optional and is not the only path.

```rust
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum DataSource<T> {
    Inline { value: T },
    Binding { name: Arc<str> },
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "widget", rename_all = "snake_case")]
pub enum WidgetSpec {
    Badges(BadgeStripSpec),
    Spinner(SpinnerSpec),
    TimeSeries(TimeSeriesSpec),
    Gauges(GaugeListSpec),
    Flame(FlameGraphSpec),
    BoxPlot(BoxPlotSpec),
    Table(TableSpec),
    Meter(MeterSpec),
    Status(StatusSpec),
    Scroll(ScrollSpec),
    Native { binding: Arc<str>, params: serde_json::Value },
}
```

`Native` is the deliberate escape hatch for content that should exploit a backend: the web canary’s textarea, live status component, tooltip target, and overflow probe use named native bindings rather than a fake cross-renderer DOM AST. Dynamic charts use `DataSource::Binding`; Nix selects the source, while application code produces borrowed data. Inline data makes a pure-Nix static canary self-contained.

Runtime widget views borrow data:

```rust
pub enum WidgetView<'a> {
    Badges(&'a [BadgeSpec]),
    TimeSeries(&'a [SeriesView<'a>]),
    Table(TableView<'a>),
    Native { binding: &'a str, params: &'a serde_json::Value },
    // remaining variants
}
```

This avoids cloning chart points, rows, badge strings, or flame spans every frame.

### 7.2 Complete badge cutover

Move all data-bearing props into one core `BadgeSpec`: `kind`, `field`, `value`, `active`, `with_x`, `with_plus`, `small`, `override_color: Option<Color>`, typed `accent_color: Option<ColorRef>` (a literal RGB color or semantic theme-token reference), `click_kind`, and `emit_hover`. Keep existing `BadgeAction`, `BadgeClickKind`, `tag_hue`, and `display_label` behavior. Replace the tuple `Rgb` alias with the same typed `Color` used by theme tokens, and delete `panel_kit_tui::badge::Badge`. Web becomes `badge(&BadgeSpec, on_action) -> Element`; TUI becomes `badge_spans(&BadgeSpec, &ResolvedTheme)` plus hit regions/action mapping. This closes the current loss of web-only flags (`with_x`, `with_plus`, `small`, accent color, hover) on TUI without admitting arbitrary CSS color strings into core.

`BadgeKind`, `BadgeClickKind`, and renderer-neutral badge data gain serde derives because WorkspaceSpec contains them; `BadgeAction` remains a runtime output and need not be serialized.

### 7.3 Charts/table/meter/status/scroll

Move only renderer-neutral shapes/algorithms to core:

- `Series`, `GaugeItem`, `FlameSpan`, `BoxItem`, and `FiveNum` become core models; `five_num` remains shared behavior. The current TUI shapes are verified at `crates/panel-kit-tui/src/charts.rs:29-35`, `crates/panel-kit-tui/src/charts.rs:103-113`, `crates/panel-kit-tui/src/charts.rs:163-181`, and `crates/panel-kit-tui/src/charts.rs:310-362`.
- Add web painters for time series, gauges, flame graph, and box plot. Web may use SVG/canvas; TUI continues to use ratatui `Chart`/`Gauge` and cell drawing.
- Core `TableSpec` uses semantic cells and backend-neutral column widths (`fixed`, `ratio`, `fill`). TUI retains a separately named `table_native` escape hatch accepting ratatui `Row` and `Constraint`; it is intentionally not representable in Nix. The shared table painter covers parity paths.
- Core meter calculates clamped fill and exposes charset-neutral fraction; Unicode/ASCII glyph choice remains in the backend. The present TUI `bar` allocation can be optimized independently (`crates/panel-kit-tui/src/meter.rs:9-26`).
- Status takes typed `Color` and text, not ratatui types; this preserves the current renderer-neutral intent (`crates/panel-kit-tui/src/status.rs:1-31`).
- Scroll bounds move to core. Actual DOM scroll position and application-owned TUI body offset remain native state, consistent with the current TUI module’s statement that scroll ownership belongs to the consumer (`crates/panel-kit-tui/src/scroll.rs:1-12`).

### 7.4 Canonical theme

```rust
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Color { pub r: u8, pub g: u8, pub b: u8 }

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "source", content = "value", rename_all = "snake_case")]
pub enum ColorRef {
    Literal(Color),
    Theme(ThemeColor),
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeColor {
    Bg, Panel, Fg, Dim, Line, Line2, InvBg, InvFg,
    Accent, Red, Yellow, Green, Blue, Pink, BadgeInfo,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ColorTokens {
    pub bg: Color,
    pub panel: Color,
    pub fg: Color,
    pub dim: Color,
    pub line: Color,
    pub line2: Color,
    pub inv_bg: Color,
    pub inv_fg: Color,
    pub accent: Color,
    pub red: Color,
    pub yellow: Color,
    pub green: Color,
    pub blue: Color,
    pub pink: Color,
    pub badge_info: Color,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ThemeTokens {
    pub colors: ColorTokens,
    pub typography: TypographyTokens,
    pub density: DensityTokens,
}

impl ThemeTokens {
    pub fn dark() -> Self;
    pub fn paper() -> Self;
}
```

Core becomes the only default-value source. Web `theme_vars` emits all CSS custom properties. `assets/panel-kit.css` keeps structural rules and `var(...)` references but loses default color literals. TUI `theme.rs` keeps `From<&ThemeTokens> for ResolvedTheme` and loses `DARK`/`PAPER` literals. `DESIGN.md` keeps rationale and semantic rules but its duplicate value frontmatter/table is generated from or replaced by references to `ThemeTokens`; it is not a third editable palette.

Typography/density tokens may be approximated by cell backends; approximation is explicit capability metadata, not silent omission. Chrome geometry remains in `ChromeMetrics`, `Clamp`, and `TileMetrics`, referenced by WorkspaceSpec rather than hidden inside theme.

## 8. WorkspaceSpec: Rust location, Nix schema, and data flow

### 8.1 Location and lifetime

`WorkspaceSpec` lives in **`panel-kit-core::spec` as the canonical serde type**. It is not a Nix module and not a separate compiler crate. Nix `mkWorkspaceSpec` mirrors that type and is exported through `lib.${system}`.

The **Nix evaluation is build-time only**. A flake evaluates a spec to JSON; Rust checks/builds embed that JSON with `include_str!(env!("PANEL_KIT_WORKSPACE_SPEC"))`. A non-Nix Cargo consumer can construct or deserialize the same Rust type. At application initialization, `WorkspaceSpec::resolve()` validates IDs/bindings and produces catalog, initial snapshot, policies, theme, and content bindings. The mutable `Snapshot` then diverges from defaults as the operator works; WorkspaceSpec is never the persisted runtime state. No consuming process invokes Nix.

The exact data flow is:

```text
Nix attrset
  → mkWorkspaceSpec normalization/assertions (build time)
  → JSON store path
  → serde WorkspaceSpec in panel-kit-core
  → validate + resolve once
  → PanelCatalog + initial Snapshot + policies/theme/content bindings
  → host-owned reduce loop
  → project_into reusable ProjectionBuffer
  → borrowed ProjectedFrame
  → host-selected Dioxus or ratatui widget functions
```

For Cargo-only consumers, construction begins at `WorkspaceSpec` or directly at `PanelCatalog`/`Snapshot`; all later arrows are identical.

### 8.2 Rust shape

```rust
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceSpec {
    pub name: String,
    pub units: Units,
    pub viewport: (f64, f64),
    pub preferred_mode: Mode,
    pub surface: SurfaceSpec,
    pub geometry: GeometrySpec,
    pub chrome: ChromeSpec,
    pub theme: ThemeTokens,
    pub keymap: KeymapSpec,
    pub glyphs: GlyphSet,
    pub persistence: PersistenceSpec,
    pub panels: Vec<PanelSpec>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PanelSpec {
    pub id: String,
    pub title: String,
    pub slug: Option<String>,
    pub window: WindowSpec,
    pub content: Vec<WidgetSpec>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowSpec {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub state: WinState,
    pub z: i32,
    pub tile_w: u8,
    pub tile_h: u8,
}

pub struct ResolvedWorkspace {
    pub catalog: PanelCatalog<PanelId>,
    pub initial: Snapshot<PanelId>,
    pub surface: SurfaceSpec,
    pub geometry: GeometrySpec,
    pub chrome: ChromeSpec,
    pub theme: ThemeTokens,
    pub keymap: KeymapSpec,
    pub glyphs: GlyphSet,
    pub persistence: PersistenceSpec,
}

#[derive(Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Badges,
    Spinner,
    TimeSeries,
    Gauges,
    FlameGraph,
    BoxPlot,
    Table,
    Meter,
    Status,
    Scroll,
    Native,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingDeclaration {
    pub name: String,
    pub kind: ProviderKind,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingManifest {
    pub backend: BackendKind,
    pub bindings: Vec<BindingDeclaration>,
}

impl WorkspaceSpec {
    pub fn validate(&self, bindings: &BindingManifest) -> Result<(), SpecErrors>;
    pub fn resolve(self, bindings: &BindingManifest) -> Result<ResolvedWorkspace, SpecErrors>;
}
```

There is no second WorkspaceSpec version/migration system. It is a build-time contract compiled in lockstep with the crate and Nix library. Versioned operator state remains the already-existing `SavedLayoutV2` contract.

### 8.3 Nix attrset schema

`mkWorkspaceSpec` accepts and normalizes this exact public surface:

```nix
panel-kit.lib.${system}.mkWorkspaceSpec {
  name = "workspace-canary";
  units = "Cells";                 # "CssPx" | "Cells"
  viewport = [ 128.0 52.0 ];
  preferred_mode = "Floating";

  surface = {
    compact_max = 60.0;
    tablet_max = 110.0;
    capabilities = {
      coarse_pointer = false;
      hover = true;
      keyboard = true;
    };
    resize_policy = "preserve_intent"; # or "scale_floating"
  };

  geometry = {
    clamp = {
      outer_w = 0.0; outer_h = 0.0;
      floor_w = 24.0; floor_h = 8.0;
      inner = 2.0; edge = 0.0;
      min_w = 20.0; min_h = 5.0;
    };
    tile = {
      resize = { row = 4.0; col_floor = 12.0; outer = 0.0; };
      row_min = 4.0; columns = 4;
      gap = 0.0; padding = 0.0; fill_viewport = true;
    };
  };

  chrome = {
    inset = 1.0;
    dock_h = 3.0;
    panel_border = 1.0;
    title_in_border = true;
    controls = [ "mode" "minimize" "maximize" ];
    dock_label = "dock:";
    topbar = { binding = "canary.topbar"; params = { }; };
  };

  theme = {
    colors = {
      bg = "#0a0a0a"; panel = "#0d0d0d"; fg = "#ededed";
      dim = "#7a7a7a"; line = "#262626"; line2 = "#5f5f5f";
      inv_bg = "#ededed"; inv_fg = "#0a0a0a";
      accent = "#5ef38c"; red = "#ff5f56"; yellow = "#ffbd2e";
      green = "#27c93f"; blue = "#3b9bff"; pink = "#ff5fc3";
      badge_info = "#83b7cc";
    };
    typography = {
      family = "ui-monospace";
      body_size = 13.0; body_line_height = 1.5;
      label_size = 0.72; label_weight = 700; label_tracking = 0.06;
    };
    density = {
      hit_min = 24.0; panel_head_h = 22.0;
      panel_radius = 4.0; badge_radius = 999.0;
    };
  };

  keymap = {
    command_step = { coarse = 1.0; fine = 1.0; };
    bindings = [
      { key = "Tab"; modifiers = [ ]; context = "workspace"; command = "focus_next"; }
      # Complete default command_for table, normalized by the Nix helper.
    ];
  };

  glyphs = "Unicode";              # "Unicode" | "Ascii"
  persistence = {
    enabled = true;
    key = "panel_kit_canary";
    save_policy = "on_settle";     # "manual" | "on_settle" | "on_change"
  };

  panels = [
    {
      id = "Workspace"; title = "Workspace"; slug = "workspace";
      window = {
        x = 1.0; y = 0.0; w = 62.0; h = 11.0;
        state = "Normal"; z = 1; tile_w = 1; tile_h = 2;
      };
      content = [ { widget = "native"; binding = "canary.workspace"; params = { }; } ];
    }
    {
      id = "Capacity"; title = "Capacity";
      window = {
        x = 65.0; y = 22.0; w = 63.0; h = 8.0;
        state = "Normal"; z = 7; tile_w = 2; tile_h = 2;
      };
      content = [ {
        widget = "gauges";
        source = "binding";
        name = "canary.capacity";
      } ];
    }
  ];
}
```

The library exports `mkWorkspaceSpec`, `defaultTheme`, `webDefaults`, `cellDefaults`, `defaultKeymap`, and existing `mkLayout` helpers. Defaults are explicit composable attrsets merged before normalization; callers may override them. The returned value is `{ value; json; panelIds; bindings; }`. Nix asserts types/basic ranges eagerly; Rust remains authoritative for complete validation.

`surface.capabilities` supplies an explicit value for fixed/non-probing renderers and parity fixtures. A backend that can observe current pointer/hover/keyboard capabilities replaces that value before calling `SurfaceProfile::from_logical_width`; the spec never freezes a browser as mouse-only or touch-only.

### 8.4 Validation rules

- Reject unknown fields through serde; Nix also rejects unexpected attributes.
- Require finite positive viewport and panel sizes; finite positions; unique non-empty panel IDs/slugs; non-empty titles.
- Enforce `tile_w` in `1..=TILE_W_MAX`, `tile_h` in `1..=TILE_H_MAX`, ordered/non-negative surface thresholds, non-negative chrome/geometry metrics, and RGB syntax.
- Require each `DataSource::Binding`, `Native` binding, and topbar binding to appear in the supplied `BindingManifest` for the backend canary being built.
- Validate all inline chart numbers are finite, ratios clamp only at render time but invalid NaN is rejected, flame depth is a valid preorder tree, table row widths match columns, and badge URL/wikilink payloads contain required fields.
- Preserve declaration order as tiling order; default z is one-based declaration order exactly as current `mkLayout` does (`nix/lib/mkLayout.nix:99-103`).
- Persistence key is required only when enabled. The key never selects a transport.

## 9. Field-by-field feature-completeness table

“Expressed” means the pure Nix WorkspaceSpec contains either the data itself or a named binding to application behavior. It does **not** mean Nix serializes Rust closures, Dioxus VNodes, WebGPU, or a metrics update algorithm.

| Existing example surface | Verified evidence | WorkspaceSpec field / representation | Backend/runtime responsibility |
| --- | --- | --- | --- |
| Panel identity and title | Web enum/title: `examples/workspace.rs:65-84`; TUI nine identities/titles: `crates/panel-kit-tui/examples/workspace_canary.rs:7-34` | `panels[].id`, `title`, optional `slug` | Catalog resolves once; backends read borrowed metadata. |
| Declaration/tiling order and z | Web `default_layout`: `examples/workspace.rs:88-102`; TUI `defaults`: `crates/panel-kit-tui/examples/workspace_canary.rs:36-50` | panel list order and `window.z` | Projector produces deterministic paint and tile order. |
| Floating geometry | Web `default_layout`: `examples/workspace.rs:88-102`; TUI `defaults`: `crates/panel-kit-tui/examples/workspace_canary.rs:36-50` | `window.{x,y,w,h}` plus top-level `units`/`viewport` | Core `effective_rect`/projection. |
| Initial normal/minimized/maximized state | Web Help starts minimized: `examples/workspace.rs:90-98` | `window.state` | Reducer/projector. |
| Tile spans | Web `.with_tile`: `examples/workspace.rs:90-100`; TUI spans: `crates/panel-kit-tui/examples/workspace_canary.rs:39-48` | `window.tile_w`, `tile_h` | Core validates/clamps projected span. |
| Preferred/effective mode | Web reads `effective_mode`: `examples/workspace.rs:114-117`; core resolver exists at `crates/panel-kit-core/src/lib.rs:158-165` | `preferred_mode`, `surface.*` | Existing `effective_mode`. |
| Compact/tablet/regular thresholds and capabilities | Web displays profile/capability: `examples/workspace.rs:118-128` | `surface.compact_max`, `tablet_max`, `capabilities` | Backend measures current width/capabilities; core classifies. |
| Viewport scaling policy | Web currently ratio-scales in `use_workspace`: `src/lib.rs:372-397` | `surface.resize_policy` | Reducer handles `ViewportChanged`. |
| Clamp/tile/chrome metrics | Core defaults: `crates/panel-kit-core/src/lib.rs:580-652`; chrome: `crates/panel-kit-core/src/lib.rs:422-459` | `geometry.clamp`, `geometry.tile`, `chrome` | Projector. |
| Root/workspace/dock and panel chrome | TUI native border/control rendering: `crates/panel-kit-tui/src/lib.rs:370-387`, `crates/panel-kit-tui/src/lib.rs:483-615`; web chrome: `src/lib.rs:871-924`, `src/lib.rs:937-1005` | `chrome.{inset,dock_h,panel_border,title_in_border,controls,dock_label}` | Native `Block` or semantic DOM painters. |
| Topbar/app chrome | Web topbar/reset/restore: `examples/workspace.rs:271-300` | `chrome.topbar` native binding and params | Host owns actual topbar tree/actions. |
| Full palette | CSS: `assets/panel-kit.css:5-23`; TUI Theme: `crates/panel-kit-tui/src/theme.rs:10-36` | `theme.colors` includes all fifteen, including inverse pair | Core canonical; web variables/TUI colors derived. |
| Typography/density | CSS vars/root styles: `assets/panel-kit.css:22-35`; design tokens: `DESIGN.md:20-63` | `theme.typography`, `theme.density` | Web exact; cell backend maps meaningful subset and declares approximations. |
| Charset | Current `Charset` variants: `crates/panel-kit-tui/src/lib.rs:84-110` | `glyphs` | Backend selects Unicode/ASCII glyph set; semantic frame unchanged. |
| Key bindings and steps | Existing keyboard layer: `crates/panel-kit-core/src/lib.rs:167-332`; web behavior documented at `examples/workspace.rs:27-29` | `keymap.bindings`, `command_step` | Reducer uses existing command semantics with configured mapping. |
| Pointer move/resize/reorder/raise | Web root and panel handlers: `examples/workspace.rs:272-276`; shell implementation `src/lib.rs:879-920` | No callback serialization; geometry policy plus shared event grammar | Backend translates native event and hit target; reducer mutates. |
| Native text editing/focus | Web Notes textarea and text-input behavior: `examples/workspace.rs:130-140` | `WidgetSpec::Native { binding = "web.notes", params }` | Dioxus provider emits textarea; browser owns caret/focus. |
| Native inner scroll and wheel chaining | Web overflow probe: `examples/workspace.rs:141-153`; current chain: `src/lib.rs:728-754` | content binding plus scroll policy implicit in event adapter | DOM decides `WheelDisposition`; core owns only workspace scroll. |
| Workspace scroll and below-fold panel | Web status/below-fold: `examples/workspace.rs:156-213`, `examples/workspace.rs:258-264` | panel geometry plus reducer snapshot field; optional status binding | Projector reports extent; reducer clamps scroll. |
| Programmatic restore/reset | Web controls: `examples/workspace.rs:282-298` | topbar/native binding declares action names; no function serialization | Host emits `WorkspaceEvent::Command` or replaces snapshot. |
| Tooltip placement | Web `tip_pos` use: `examples/workspace.rs:219-236`, `examples/workspace.rs:303-308` | native binding `web.status` with tooltip params | `tip_pos` remains an independently callable web utility. |
| Persistence key/policy and V1/V2 | Web key: `examples/workspace.rs:44-46`, persisted note `examples/workspace.rs:238-240`; V1/V2 core types `crates/panel-kit-core/src/lib.rs:973-1063` | `persistence.{enabled,key,save_policy}` | Host supplies store; existing V1/V2 loader remains. |
| Badges: all kinds and nested payloads | Canary badges: `crates/panel-kit-tui/examples/workspace_canary.rs:64-109`; shared kinds: `crates/panel-kit-core/src/badge.rs:22-61` | `WidgetSpec::Badges`; complete `BadgeSpec`, including URL, wikilink, entity, flags, colors/actions | Native badge painter emits action. |
| Live time series | `Metrics::series`: `crates/panel-kit-tui/examples/workspace_canary.rs:189-201`; model currently `crates/panel-kit-tui/src/charts.rs:29-35` | `time_series` with binding source, unit, series metadata | Host returns borrowed samples; web/TUI native chart. |
| Gauges | Canary values: `crates/panel-kit-tui/examples/workspace_canary.rs:267-290`; current model `crates/panel-kit-tui/src/charts.rs:103-113` | `gauges` inline or binding source | Native Gauge/SVG painter. |
| Flame graph | Canary generator: `crates/panel-kit-tui/examples/workspace_canary.rs:216-256`; model `crates/panel-kit-tui/src/charts.rs:163-181` | `flame` inline/binding source | Shared validation; native painter. |
| Box plot/distribution | Canary `boxes`: `crates/panel-kit-tui/examples/workspace_canary.rs:203-214`; model/statistics `crates/panel-kit-tui/src/charts.rs:310-362` | `box_plot` inline/binding source | Core five-number behavior, native painter. |
| Node table + status + meter | Rows: `crates/panel-kit-tui/examples/workspace_canary.rs:52-62`; TUI modules documented at `crates/panel-kit-tui/src/table.rs:1-17`, `crates/panel-kit-tui/src/status.rs:1-31`, `crates/panel-kit-tui/src/meter.rs:1-26` | `table` cells may contain text/status/meter; or binding source | Native table retains styling; `table_native` escape hatch exists. |
| Scrollable content | TUI consumer-owned offset contract: `crates/panel-kit-tui/src/scroll.rs:1-12`; web preview overflow above | `scroll` policy/source or `Native` binding | Scroll offset stays with host/native control. |
| Spinner | Web exported function: `src/lib.rs:1072-1105`; TUI spinner is tick-based (`crates/panel-kit-tui/src/spinner.rs:10-29`) | `spinner.{label,tick_source}` | CSS animation or tick-selected glyph. |
| Dynamic metrics algorithm | `Metrics::tick`/`noise`: `crates/panel-kit-tui/examples/workspace_canary.rs:111-187` | named data bindings; algorithm is **not** serialized | Rust provider remains application code. Binding completeness is checked. |
| Arbitrary app CSS/content | Web canary `DEMO_CSS`: `examples/workspace.rs:48-63` | semantic theme fields where shared; native content binding/host stylesheet where backend-specific | Deliberately not a CSS/DOM language in core. |

This is feature-complete as a workspace specification: every configurable semantic value and every content dependency is declarable in Nix. Executable application behavior remains executable code, selected through checked bindings.

## 10. Mechanized spec-parity proof

### 10.1 Delete the false proof

Delete `checks.layout-canary-schema` and its `jq` assertions (`flake.nix:118-149`). Delete the seven-panel `nix/examples/workspace-canary.nix`. Replace it with full `workspace-spec-canary.nix`. Delete the Rust `Panel` enum and `defaults()` hand-mirror from `workspace_canary.rs`; the file becomes the executable data/provider manifest for the Nix-authored panel IDs and widget bindings.

The declarative spec owns metadata, initial layout, chrome/theme/keymap/persistence, and binding names. Rust owns implementations of dynamic/native bindings. These are complementary facts, not two hand-copied layouts.

### 10.2 Binding manifest catches the live 7-vs-9 drift

`workspace_canary.rs` exports an exact canary manifest containing the executable panel/body registrations for:

`Workspace, Activity, Flame, Notes, Badges, Nodes, Capacity, Distribution, Theme`.

The native parity runner compares the set of WorkspaceSpec `(panel id, content binding, supported widget kind)` entries against that manifest in both directions. Running it **before** replacing the stale Nix canary fails with a diagnostic equivalent to:

```text
spec-parity: canary panel set differs
  missing from Nix WorkspaceSpec: Flame, Distribution
  Nix-only bindings: (none)
```

Thus the currently live seven-versus-nine drift is the first red test, not a prose observation. After migration, adding a new executable canary panel/provider without adding it to Nix, removing a provider still referenced by Nix, or changing a binding name fails the same exact-set comparison. Layout geometry has one source (Nix), so positional hand-mirror drift is removed rather than compared forever.

### 10.3 Named `nix flake check` jobs

1. **`checks.spec-parity`** (native):
   - evaluates `workspace-spec-canary.nix` to JSON;
   - executes a native Rust runner that deserializes `WorkspaceSpec` with serde and rejects unknown/missing fields;
   - validates catalog IDs, complete binding manifest, widget inputs, theme/keymap/metrics;
   - resolves to `Snapshot<PanelId>`;
   - projects cell and CSS-pixel frames through `project_into`;
   - checks structural invariants and emits a canonical semantic digest (integers/IEEE values, not formatted float strings);
   - verifies serialize → deserialize → resolve produces the same digest.
2. **`checks.spec-canary-tui-native`**: builds and runs the ratatui workspace example natively using the generated WorkspaceSpec and canary bindings. It verifies every projected panel/widget path can be painted.
3. **`checks.spec-canary-web-wasm`**: builds the Dioxus workspace example for `wasm32-unknown-unknown` with `PANEL_KIT_WORKSPACE_SPEC` pointing to the Nix output. It proves native content bindings and every web widget painter type-check on the supported target.
4. **`checks.spec-canary-browser-tui-wasm`**: retains the Ratzilla/browser-TUI canary build for wasm32, now consuming the same generated spec and selecting ASCII glyphs through an override.
5. **`checks.workspace-spec-errors`**: evaluates representative invalid Nix attrsets and matches stable error categories (duplicate ID, missing binding, invalid span/color), then runs Rust validation for cases Nix cannot prove.

All jobs are included in `hydraJobs.<system>` and the release aggregate, preserving the current checks-to-Hydra pattern (`flake.nix:181-213`). The matrix is deliberately one canonical spec × three shipped surfaces, not every spec × every backend; it does not multiply with downstream specs.

### 10.4 What fails on disagreement

| Disagreement | Failing job |
| --- | --- |
| Nix emits a field/type/variant Rust does not accept | `spec-parity` serde decode |
| Rust makes a required field Nix omits | `spec-parity` decode |
| Nix panel/binding set differs from canary Rust providers (including current 7 vs 9) | `spec-parity` exact manifest comparison |
| Geometry/mode/chrome produces invalid or nondeterministic frame | `spec-parity` projection invariants/digest round trip |
| Shared widget exists in spec/core but a backend painter is absent | corresponding web/TUI example build |
| Theme token is added in core but not emitted to CSS/TUI conversion | backend-specific theme completeness compile/unit check, transitively flake check |
| Web-only code leaks into core/native runner | `spec-parity` or TUI native build |
| Native-only code leaks into Dioxus path | `spec-canary-web-wasm` |

Canonical parity compares typed values and structural digests, never pretty JSON bytes or formatted floats. This avoids the float-format sensitivity identified in the original proposal while retaining current `mkLayout` numeric normalization for human-stable output.

## 11. Backend responsibilities: keep versus delete

### 11.1 Dioxus backend keeps

- Dioxus event decoding, DOM focus inspection (`TextInput`), pointer capture/prevent-default, and wheel-scrollability inspection.
- Dioxus components/functions that paint individual projected panels, dock, controls, badges, spinner, and shared widget models.
- Semantic DOM/ARIA, keyed nodes, native focus, native scrolling, CSS Grid, canvas/SVG options, `tip_pos`, and structural CSS.
- `LocalStorageStore` and optional small recipes for Dioxus signal/effect wiring.

### 11.2 Dioxus backend deletes

- Public `Workspace<K>` controller and `use_workspace(storage_key, defaults)` as the primary/only API.
- `Workspace::render`, `dock`, `root_class`, event-handler methods, and direct public signal bundle.
- Private localStorage `save_layout`/`load_layout` schema logic.
- Duplicated visibility/order/geometry projection and per-render panel-vector clone.
- Hardcoded default theme values in CSS.

### 11.3 TUI backend keeps

- Crossterm/ratzilla event decoding; conversion between `Region` and ratatui `Rect`.
- Ratatui-native panel/dock/control and widget paint functions, `Block::title`, and public native escape hatches.
- File and browser-storage transports.
- Unicode/ASCII glyph implementation and terminal color conversion.
- Optional event-loop recipe.

### 11.4 TUI backend deletes

- Public `TuiWorkspace<K>` controller and its render/input/persistence ownership.
- Private `Zone`, `zones`, `dock_chips`, and backend-computed hit testing; the projected frame contains those regions.
- Duplicated visibility/order/tile/chrome/scroll projection.
- TUI-local `LayoutStore` definition (moved to core).
- TUI-local data-bearing `Badge` and duplicate default theme literals.

## 12. Explicit deletion ledger

The implementation is incomplete until these removals occur:

1. `Workspace<K>` and `TuiWorkspace<K>` controller structs and their shell-owned `render` methods.
2. Web `save_layout`/`load_layout` and TUI load/merge/save schema duplication; shared helpers replace both.
3. TUI-local `LayoutStore`; only core owns the trait.
4. TUI `Zone` and renderer-owned geometry/hit-map vectors.
5. Web per-render `panels.clone()`/visible collection and TUI projection scratch allocations, replaced by `ProjectionBuffer`.
6. TUI data `badge::Badge` and the tuple `Rgb` alias; `BadgeSpec` and typed `Color` are shared.
7. Parallel default palette literals in `assets/panel-kit.css`, `panel-kit-tui/src/theme.rs`, and `DESIGN.md`; core tokens are authoritative.
8. Layout-only Nix canary, `jq` shape-only check, and the Rust `defaults()` geometry mirror.
9. Static-title `PanelKind` trait; metadata becomes data.
10. Documentation claiming adoption is only “PanelKind + body callback” (`README.md:48-77`); it is replaced with reducer/frame/widget-kit examples.

Existing geometry/command/persistence free functions, `SurfaceProfile`/`effective_mode`, `SavedLayoutV2`/`StoredLayout`/migration, and `mkLayout` are retained.

## 13. Consumer migration (MIGRATION.md style, clean cutover)

No deprecated aliases or compatibility controller remains. Both consumers stay on their current git pins until their migration branch is complete, matching the existing pin-first policy (`MIGRATION.md:1-23`, `PRODUCT.md:111-118`). Existing V1/V2 layout JSON continues to load.

### 13.1 Breaking changes common to both consumers

#### Replace `PanelKind` with catalog metadata

**Before**

```rust
impl panel_kit::PanelKind for Panel {
    fn title(self) -> &'static str {
        match self { Panel::Graph => "Graph", Panel::Inspector => "Inspector" }
    }
}
```

**After**

```rust
fn panel_catalog() -> PanelCatalog<Panel> {
    PanelCatalog::try_new(vec![
        PanelMeta::new(Panel::Graph, "Graph", "Graph", "graph", graph_content()),
        PanelMeta::new(Panel::Inspector, "Inspector", "Inspector", "inspector", inspector_content()),
    ]).expect("static panel catalog is valid")
}
```

`PanelKey` is blanket-implemented; enum consumers delete only the title-bearing trait impl and retain their enum plus its existing serde variant names. Their `PanelMeta.stable_id` uses that same text. Nix-created catalogs use transparent-string `PanelId`, so both paths preserve the serialized key contract. `PanelWin<K>` is `Clone`, not `Copy`; update dereferences/moves accordingly.

#### Replace shell construction with owned snapshot/scratch/store pieces

**Before**

```rust
let ws = panel_kit::use_workspace("myapp_layout", default_layout);
```

**After**

```rust
let catalog = use_hook(panel_catalog);
let mut snapshot = use_signal(|| Snapshot::from_defaults(
    default_layout(), Mode::Floating, Viewport::css_px(initial_viewport())
));
let mut projection = use_hook(|| ProjectionBuffer::with_panel_capacity(catalog.len()));
let store = use_hook(|| LocalStorageStore::new("myapp_layout"));
```

The optional recipe may create those four values, but returns them separately; it never owns rendering.

#### Replace method handlers with adapter + reducer calls

**Before**

```rust
onpointermove: move |e| ws.handle_pointer_move(&e),
onpointerup: move |e| ws.handle_pointer_up(&e),
onkeydown: move |e| ws.handle_key(&e),
```

**After**

```rust
onpointermove: move |e| {
    if let Some(input) = panel_kit::input::pointer_move(&e, &last_frame) {
        reduce(&mut snapshot.write(), input, web_reduce_context());
    }
},
onpointerup: move |e| {
    reduce(&mut snapshot.write(), panel_kit::input::pointer_up(&e), web_reduce_context());
},
onkeydown: move |e| {
    if let Some(input) = panel_kit::input::key(&e, current_focus()) {
        e.prevent_default();
        reduce(&mut snapshot.write(), input, web_reduce_context());
    }
},
```

Applications get first refusal on an event and pass only unconsumed input to panel-kit.

#### Replace `render`/`dock` with an application-owned loop

**Before**

```rust
{ws.render(panel_body)}
{ws.dock()}
```

**After**

```rust
let frame = project_into(
    ProjectionInput::web(&snapshot.read(), &catalog, surface),
    &mut projection,
);
rsx! {
    div { class: panel_kit::widgets::root_class(&frame),
        for panel in frame.panels {
            {panel_kit::widgets::panel(&frame, panel,
                panel_body(frame.key(panel), panel.state == WinState::Maximized))}
        }
        {panel_kit::widgets::dock(&frame, move |key| {
            reduce(&mut snapshot.write(), WorkspaceEvent::Command {
                target: Some(key), command: PanelCommand::Restore,
            }, web_reduce_context());
        })}
    }
}
```

The exact Dioxus borrowing arrangement may project into memoized/owned signal-local scratch before `rsx!`; it must preserve the zero-allocation core projection contract and must not recreate a controller.

#### Replace programmatic methods and public signals

- `ws.restore(kind)` → `reduce(... WorkspaceEvent::Command { target: Some(kind), command: Restore }, ...)`.
- `ws.effective_mode()` → `frame.mode` or existing `effective_mode(snapshot.preferred_mode, &surface)`.
- `ws.surface_profile()` → the host’s `SurfaceProfile` value.
- `ws.panels`, `ws.mode`, `ws.focused`, `ws.ws_scroll` → fields on the host-owned `Signal<Snapshot<_>>`.
- `ws.root_class()` → stateless `widgets::root_class(&frame)`.
- `render_with_header` users render header actions beside `widgets::panel_chrome`; no body callback is invoked inside a shell component.
- `panel_kit::CSS` → `panel_kit::style(&ThemeTokens)` plus the structural CSS constant, or one recipe call that installs both.

#### Persistence

Load through core `load_snapshot` at initialization; on `Reduction.layout_settled`, call `save_snapshot` according to policy. Existing `{ panels, tiling }` V1 and `SavedLayoutV2` continue through `StoredLayout` migration. Do not rewrite or clear users’ layout keys merely because the API changed.

### 13.2 jump-cannon cutover

jump-cannon has two independently persisted workspaces. Migrate both in one branch:

1. Keep both existing storage keys unchanged.
2. Replace each `use_workspace` call with its own `Signal<Snapshot<Panel>>`, `ProjectionBuffer`, and `LocalStorageStore`.
3. Replace the `ViewWs(Workspace<Panel>)` prop wrapper with a small prop containing only the corresponding snapshot signal/catalog reference; equality is signal identity.
4. In the `OPEN_PANEL` effect, emit `PanelCommand::Restore` against the active snapshot rather than calling `ws.restore`.
5. Preserve event priority: command palette and graph/camera shortcuts run first; only their unconsumed key events go through `panel_kit::input::key` and `reduce`. This is an improvement enabled by host-owned control flow.
6. Replace each `render_with_header`/`dock` pair with a panel loop. Keep jump-cannon’s `panel_body` and `panel_header_actions` as native Dioxus functions; do not serialize them into WorkspaceSpec.
7. Move the panel ID/title list to `PanelCatalog`; keep the current serde names so saved layouts deserialize.
8. Update badge call sites from the twelve-parameter component to `BadgeSpec` plus an action handler.
9. Build its wasm app, then update the git revision. Do not update the pin before the source migration.

### 13.3 apple-notes-ocr-flow cutover

apple-notes-ocr-flow follows the same clean cutover, with its OCR/reviewer bodies remaining native:

1. Preserve its existing localStorage key and panel serde IDs.
2. Replace `PanelKind` title matching with `PanelCatalog` entries.
3. Hold its reviewer `Snapshot` in its existing app state/Dioxus signal and allocate one `ProjectionBuffer` beside it.
4. Translate root pointer/key/wheel events and call `reduce`; route OCR editor/text-input events first so panel commands do not steal typing.
5. Render projected panels explicitly and dispatch existing OCR panel-body functions by catalog key/content binding.
6. Express static default geometry, theme, chrome, and binding names as either Rust WorkspaceSpec or a Nix WorkspaceSpec. Its OCR pipeline, document editor, and live chart samples stay Rust providers.
7. Load old layouts through `load_snapshot`, save on `layout_settled`, and update the pinned revision only after its wasm build succeeds.

### 13.4 TUI consumers

- `TuiWorkspace::new(path, defaults)` is removed. Construct a `Snapshot`, `ProjectionBuffer`, catalog, and `FileLayoutStore`.
- `TuiWorkspace::with_store` is removed; pass any core `LayoutStore` to `load_snapshot`/`save_snapshot`.
- `TuiWorkspace::render` becomes an explicit `project_into` + ordered calls to `panel_kit_tui::widgets`.
- `handle_key`/`handle_mouse` become `panel_kit_tui::input` translation followed by core `reduce`.
- `theme` and `charset` become explicit values passed to widget painters.

## 14. Dependency-ordered TDD implementation slices

Each slice starts with the named failing test. Later slices do not begin until the preceding contract is green.

| Slice | Dependency | First failing test | Implementation boundary / proof |
| --- | --- | --- | --- |
| 1. Panel identity/catalog | existing core types | `panel_catalog_accepts_arc_panel_id_and_rejects_duplicate_stable_ids` | Add `PanelKey`, `PanelId`, catalog; migrate core functions from `Copy` to borrow/Clone. Existing persistence tests remain green. |
| 2. Reducer command/key path | 1 + existing command layer | `reduce_key_matches_command_for_and_apply_command` | Add Snapshot/event/context/reduction; delegate keyboard/programmatic commands. |
| 3. Reducer pointer/viewport/wheel | 2 | `reducer_replays_move_resize_reorder_settle_and_bubbled_wheel` | Delegate existing drag/reorder/scroll; cover text/content-consumed wheel and resize policies. |
| 4. Frame shape | 1–3 | `projected_frame_resolves_chrome_paint_order_dock_and_control_regions` | Add buffer/frame, max/min/tile/floating semantics, hit test. |
| 5. Frame allocation/lifetime | 4 | `project_into_allocates_zero_after_reserved_warmup` | Remove temporary Vec/string/ID clones; allocator counter proves steady state. Compile-fail doctest proves frame cannot outlive scratch/snapshot. |
| 6. Partial composition/native affordances | 4 | `single_panel_projection_matches_full_frame_panel` | Add partial projectors. TUI test checks `Block::inner` against projected body; web component test checks semantic buttons/focus/overflow attributes. |
| 7. Persistence move | 2 | `core_store_loads_v1_reconciles_units_merges_defaults_and_saves_v2` | Move trait/helpers; delete backend schema logic. |
| 8. Theme canonicalization | 4 | `dark_theme_emits_complete_css_and_tui_token_sets` | Add core tokens/conversions; remove backend/doc literals. Fail when any of fifteen colors is unconverted. |
| 9. Shared badge/widgets | 8 | `badge_spec_routes_every_action_identically_across_backend_adapters` | Complete BadgeSpec then shared chart/table/meter/status/scroll models and borrowed views. |
| 10. WorkspaceSpec Rust | 1, 7–9 | `workspace_spec_rejects_unknown_fields_duplicate_ids_and_missing_bindings` | Add serde/validation/resolution; no Nix yet. |
| 11. Nix WorkspaceSpec | 10 | **`spec_parity_reports_flame_and_distribution_missing_from_nix`** | Introduce native runner against current stale canary; observe 7-vs-9 failure first. Add full `mkWorkspaceSpec`; replace hand mirror; exact binding set passes. |
| 12. Backend widget kits | 4, 8–11 | `tui_panel_uses_native_block_inner_for_projected_body` and `web_panel_preserves_native_focus_and_overflow` | Split renderers into individual painters; delete controller render methods and zones. |
| 13. Canary/check matrix | 11–12 | `nix flake check` jobs initially absent/red | Add `spec-parity`, native TUI run, web wasm build, browser-TUI wasm build, invalid-spec check; delete jq check. |
| 14. Consumer/docs clean cutover | all | consumer compile failures at removed `PanelKind`, `use_workspace`, `render`, `TuiWorkspace` | Apply migration guide, examples, README, DESIGN/MIGRATION changes; no aliases. Build each named example/consumer in its supported target. |

No new permanent test is added merely to assert wiring. The tests above defend observable contracts: event transitions, projection invariants, zero allocations, native affordances, schema validation, binding completeness, persistence compatibility, and target builds.

## 15. Verifiable acceptance criteria

| ID | Criterion | Verification mapping |
| --- | --- | --- |
| AC-1 | A host can reduce keyboard, pointer move/resize/reorder, mode, dock restore, viewport, and wheel events without importing a backend crate. | Core reducer unit tests from slices 2–3. |
| AC-2 | `SurfaceProfile`/`effective_mode`, keyboard command layer, and V2 persistence are reused rather than duplicated/replaced. | Core tests call existing functions; code review plus existing tests. |
| AC-3 | A complete projected frame contains root/workspace/dock, paint-ordered panels, resolved regions/placement/z, per-panel state/focus/drag flags, body/header/control/grip regions, dock items, and content extent. | `projected_frame_resolves_chrome_paint_order_dock_and_control_regions`. |
| AC-4 | After reserving and one warm-up, unchanged-capacity `project_into` performs zero heap allocations and no panel-key/string clones. | Allocator-counted core unit test. |
| AC-5 | A host can project/render one panel or custom dock without `Snapshot` controller or full-frame backend shell. | `single_panel_projection_matches_full_frame_panel` plus small core example build. |
| AC-6 | TUI panel chrome uses ratatui `Block::title`/`inner`; web panel keeps DOM focus and native scroll/wheel disposition. | Focused backend unit tests and TUI/web example builds. |
| AC-7 | One core `BadgeSpec` represents every current web data prop and TUI field; the backend-supplied action handler remains code, and all badge actions route identically. | Core badge/action unit test and both backend badge example builds. |
| AC-8 | Charts, table, meter, status, scroll, spinner, and badges have shared semantic models and both shipped backends can paint the parity subset, while native escape hatches remain. | `spec-canary-tui-native`, `spec-canary-web-wasm`, `spec-canary-browser-tui-wasm`. |
| AC-9 | All fifteen palette colors plus typography/density values have one editable default source in core and complete web/TUI conversions. | `dark_theme_emits_complete_css_and_tui_token_sets`; doc/source review removes duplicate literals. |
| AC-10 | WorkspaceSpec lives in core, is usable from Cargo-only Rust, and Nix is build-time only. No runtime process invokes Nix. | Core WorkspaceSpec unit tests; example source embeds generated JSON; dependency review. |
| AC-11 | Nix WorkspaceSpec covers every row in the feature-completeness table through a value or checked binding. | Full Nix canary plus `spec-parity` validation. |
| AC-12 | `spec-parity` fails against the current seven-panel Nix / nine-panel Rust canary and names `Flame` and `Distribution`; it passes after the full spec replaces the hand mirror. | Slice-11 first-red capture, then `checks.spec-parity`. |
| AC-13 | Any Nix/Rust field mismatch, missing binding, invalid semantic value, or absent backend widget path makes a named flake check fail. | Checks and failure table in §10. |
| AC-14 | Web canary builds for `wasm32-unknown-unknown`; terminal canary builds/runs natively; browser-TUI canary builds for wasm32. | Three named flake jobs. |
| AC-15 | `LayoutStore` is defined only in core; V1 and V2 saved layouts still load, reconcile, merge defaults, and save V2. | Core persistence test; no TUI trait definition remains. |
| AC-16 | Web `Workspace`/`use_workspace` and `TuiWorkspace` controller APIs, duplicated projection/load-save code, duplicate badge data, theme literals, jq check, and layout mirror are deleted with no deprecated aliases. | Compile-fail consumer migration plus deletion ledger review. |
| AC-17 | jump-cannon and apple-notes-ocr-flow preserve storage keys/serde IDs, migrate to host-owned snapshot/frame pieces, and build before updating their pins. | Their supported wasm example/app builds recorded in implementation phase. |
| AC-18 | Existing `mkLayout` continues to emit SavedLayoutV2 for persistence-specific users. | Existing mkLayout evaluation/round-trip check retained under spec-parity. |

## 16. Self-verification questions and answers

### Q1. Can a backend still use ratatui `Block::title` insets and DOM focus/scroll?

**Yes.** The frame contains semantic outer/body/hit regions and state, not pre-rendered text/cells/nodes. TUI constructs a native `Block`, uses `Block::title` and `Block::inner`, and verifies its inner rect against the projection. Web emits actual buttons/sections, leaves `.panel-body` as native overflow, and sends DOM-derived wheel disposition to core. Arbitrary body content is host-owned. This directly avoids a draw-command lowest common denominator.

### Q2. What is the per-frame allocation cost?

**Core projection is zero allocations in steady state.** `ProjectionBuffer` owns all variable-length scratch vectors, reserved once. Projection stores indices and numeric structs, so it does not clone `PanelWin`, `PanelId`, title, slug, or widget data. It may grow capacity only when panel/row count exceeds reservation. Backend render frameworks may allocate their own nodes/widgets; that is separately measurable and not hidden by the frame API.

### Q3. Does spec-parity actually fail on the live seven-versus-nine drift?

**Yes.** The first slice-11 test runs the native exact-set comparison against the current Nix canary and the executable canary binding manifest. It must name missing `Flame` and `Distribution`. The fix does not merely change `7` to `9`: it replaces the hand-copied layout with one full Nix WorkspaceSpec and keeps an exact Rust provider/binding manifest, so future executable/declarative panel drift remains a build failure.

### Q4. Is WorkspaceSpec secretly the spec-compiler solution?

**No.** WorkspaceSpec is an optional serde input into the same public catalog/snapshot/policy values that Rust can construct directly. Nix runs only during builds; no code is generated and no runtime interpreter controls rendering. The defining C-B mechanism remains the reducer plus borrowed projected frame and independently callable native widget kits. Removing WorkspaceSpec would leave the runtime architecture intact; removing `reduce`/`project_into` would not.

### Q5. Did replacing `PanelKind` trade correctness for dynamic strings?

**No.** `PanelId` strings allocate only during load/resolution and remain `Arc<str>` thereafter. `PanelCatalog::try_new` enforces unique stable IDs/slugs and valid non-empty titles; `WorkspaceSpec::validate` enforces binding coverage before state exists. Persistence continues to serialize stable strings, not order-dependent numeric IDs. Hand-written enums retain typed identity through the blanket `PanelKey` bound and catalog metadata. The cost is a deliberate clean-cutover API change, called out in migration and tested through V1/V2 layout loading.

## 17. Changes from the original C-B proposal

1. **Corrected the invalid identity claim.** The proposal said `PanelId(Arc<str>)` could implement current `PanelKind`; it cannot because `PanelKind` requires `Copy`. This design replaces the static-title trait with `PanelKey` plus `PanelCatalog` and preserves stable-string persistence.
2. **Made frame lifetime/allocation explicit.** `project()` becomes `project_into(..., &mut ProjectionBuffer) -> ProjectedFrame<'_>`, with indices/borrowed metadata, capacity growth rules, and an allocator-counted acceptance test.
3. **Made the frame semantic rather than a paint tape.** Native ratatui blocks/widgets, DOM focus/scroll/Grid, arbitrary Dioxus bodies, and backend-native table escape hatches are explicitly preserved.
4. **Removed the fabricated/stale `ROW_CELLS` citation.** The design uses the verified current authority `TileMetrics::CELLS.row`.
5. **Separated reducer effects from I/O.** `Reduction` reports settle/focus effects; host-owned persistence/focus performs them.
6. **Prevented event-enum sprawl.** WorkspaceEvent only covers shared workspace transitions; widget and application events remain outside it.
7. **Defined complete widget/theme convergence.** One BadgeSpec, shared semantic widget models, web counterparts for TUI-only widgets, canonical core theme, and named native bindings close the actual asymmetries.
8. **Replaced fragile golden float output.** spec-parity compares typed semantic structure/digests and exact binding sets, not JSON bytes or formatted floats.
9. **Made parity catch the observed drift.** The first red test is the current missing `Flame`/`Distribution` failure. The final architecture deletes the geometry mirror while continuing to compare required Rust providers with Nix bindings.
10. **Bounded CI cost.** One canonical WorkspaceSpec is checked across the three shipped surfaces; downstream specs do not expand a global Cartesian matrix.
11. **Specified the no-god-object escape hatches.** Partial projectors, existing geometry functions, host-owned state, and independently callable widgets are first-class, while recipes never regain render ownership.
12. **Grounded migration in a clean cutover.** Exact removed signatures and replacement calls are named for jump-cannon, apple-notes-ocr-flow, and TUI users; persisted layouts remain compatible.

## 18. Explicit non-goals

- No runtime Nix evaluator, Nix daemon dependency, generated Rust application code, or Nix-controlled event loop.
- No replacement for `SurfaceProfile`/`effective_mode`, `command_for`/`apply_command`, or V1/V2 migration; this design composes the current implementations.
- No serialized Dioxus/DOM/ratatui widget tree and no universal drawing-command IR.
- No attempt to serialize application algorithms, async OCR/graph work, WebGPU canvases, or closures. WorkspaceSpec names checked native/data bindings.
- No persistence transport hidden inside reducer or frame; hosts choose stores and timing.
- No retained deprecated `Workspace`/`TuiWorkspace` facade or alias. Migration is pin-gated and clean.
- No promise of byte-identical JSON or pixel-identical backends. Parity is typed semantics, projected geometry within unit/rounding rules, binding completeness, and successful native backend painting.
- No new platform backend beyond Dioxus web, native ratatui, and the existing Ratzilla browser-TUI canary.
- No optimization beyond eliminating avoidable projection allocation/cloning until profiling identifies another measured bottleneck.
- No redesign of application-specific CSS, content, or focus policy; backend-native composition is an intentional library advantage.
