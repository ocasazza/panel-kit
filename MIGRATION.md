# Migrating to panel-kit 1.0.0

panel-kit 1.0.0 is a clean major-version cutover from controller-owned
workspaces to a composable library surface. There are no deprecated aliases,
compatibility facades, old-path re-exports, or shim features. Git-pinned
consumers should stay on their current revision until their migration branch
builds, then update the pin in one pull request.

## Pin policy for git consumers

Consumers such as jump-cannon and apple-notes-ocr-flow pin panel-kit by git
revision. Treat the revision as the compatibility boundary:

1. Keep production on the old revision while editing the consumer.
2. Migrate source code to the host-owned state/reducer/projection pattern below.
3. Build the consumer on its real target.
4. Update the panel-kit revision only after the migrated consumer build passes.

This is pull-based per consumer. A panel-kit release does not need to keep the
old controller path alive for unmigrated consumers.

## Replace the web controller with host-owned composition

**What changed:** `panel_kit::Workspace`, `panel_kit::use_workspace`, and the
controller methods (`render`, `render_with_header`, `dock`, keyboard/pointer
handlers, root-class accessors, and restore helpers) are removed.

**Why:** applications must own event priority, persistence timing, and render
order. The library now supplies pure reducer/projection functions plus Dioxus
parts that hosts call explicitly.

Before:

```rust
let ws = panel_kit::use_workspace("myapp_layout", default_layout);
rsx! {
    div { class: ws.root_class(), tabindex: "0",
        onkeydown: move |event| ws.handle_key(&event),
        {ws.render_with_header(body, header_actions)}
        {ws.dock()}
    }
}
```

After:

```rust
use panel_kit::store::LocalStorageLayoutStore;
use panel_kit::{input, surface, widgets};
use panel_kit_core::frame::{project_into, ProjectionBuffer, ProjectionInput};
use panel_kit_core::persist::{apply_save_decision, restore_snapshot, SavePolicy};
use panel_kit_core::reducer::{reduce, Snapshot, WorkspaceEvent};

let store = LocalStorageLayoutStore::new("myapp_layout");
let mut snapshot = restore_snapshot(&store, defaults, &catalog, restore_context)?;
let mut scratch = ProjectionBuffer::with_panel_capacity(catalog.len());
let frame = project_into(ProjectionInput { snapshot: &snapshot, surface, chrome: &chrome, clamp, tile: &tile }, &mut scratch);

// Host policy stays visible: decide first refusal, translate to WorkspaceEvent,
// reduce, then apply the selected persistence policy.
let reduction = reduce(&mut snapshot, WorkspaceEvent::Command { target, command }, context);
apply_save_decision(SavePolicy::OnSettle.decide(&reduction), &store, &snapshot, &catalog)?;

rsx! {
    div { class: "{widgets::root::root_class(&frame)}", tabindex: "0",
        for panel in frame.panels.iter().copied() {
            // The host chooses the body and which chrome parts to mount.
        }
        {widgets::dock::dock(frame.dock, &catalog, emit)}
    }
}
```

## Replace the TUI controller with host-owned composition

**What changed:** `panel_kit_tui::TuiWorkspace` and its constructors, render,
input, scroll, restore, and persistence methods are removed.

**Why:** the terminal backend is now a renderer over the same core reducer and
projection model as the web backend. Hosts keep their crossterm/ratzilla loop,
translate input at the boundary, and draw ratatui parts in their own order.

Before:

```rust
let mut ws = panel_kit_tui::TuiWorkspace::new(Some(path), defaults);
ws.render(frame, area, &mut |frame, body, panel, maximized| {
    draw_body(frame, body, panel, maximized);
})?;
ws.handle_mouse(mouse_event)?;
```

After:

```rust
use panel_kit_core::frame::{project_into, ProjectionBuffer, ProjectionInput};
use panel_kit_core::reducer::{reduce, Snapshot};
use panel_kit_tui::store::JsonFileLayoutStore;
use panel_kit_tui::widgets::{self, TuiHitBuffer};

let store = JsonFileLayoutStore::new(path);
let mut snapshot = restore_snapshot(&store, defaults, &catalog, restore_context)?;
let mut projection = ProjectionBuffer::with_panel_capacity(catalog.len());
let mut hits = TuiHitBuffer::with_capacity(catalog.len(), catalog.len());
let frame_model = project_into(ProjectionInput { snapshot: &snapshot, surface, chrome: &chrome, clamp, tile: &tile }, &mut projection);

widgets::root::draw_root(frame, area, &theme, charset);
for panel in frame_model.panels.iter().copied() {
    let meta = catalog.get(panel.key).expect("projected panel is cataloged");
    widgets::panel::draw_panel_surface(frame, panel, &theme, charset, &mut hits);
    let body = widgets::panel::draw_panel_chrome(frame, panel, meta, &theme, charset, &mut hits);
    widgets::panel::draw_traffic_lights(frame, panel, frame_model.mode, hover, &theme, charset, &mut hits);
    widgets::panel::draw_resize_grip(frame, panel, hover, &theme, &mut hits);
    draw_body(frame, body, panel.key);
}
widgets::dock::draw_dock(frame, dock_area, frame_model.dock, dock_context, &mut hits);

if let Some(event) = panel_kit_tui::input::workspace_event_from_pointer(&hits, pointer) {
    let reduction = reduce(&mut snapshot, event, context);
    apply_save_decision(save_policy.decide(&reduction), &store, &snapshot, &catalog)?;
}
```

## Move persistence ports to core

**What changed:** the storage trait now lives at
`panel_kit_core::persist::LayoutStore` with `load`, `save`, and `clear`.
`panel_kit_tui::store::JsonFileLayoutStore` and
`panel_kit::store::LocalStorageLayoutStore` are backend transports only. There
is no old-path re-export from the TUI crate.

**Why:** JSON schema migration, stable-ID mapping, V1/V2 reconciliation, save
policy, and clear semantics are renderer-neutral. A transport should only move
opaque JSON bytes.

Before:

```rust
struct BrowserStore;
impl panel_kit_tui::LayoutStore for BrowserStore {
    fn load(&self) -> Result<Option<String>, String> { todo!() }
    fn save(&self, json: &str) -> Result<(), String> { todo!() }
}
```

After:

```rust
struct BrowserStore;
impl panel_kit_core::persist::LayoutStore for BrowserStore {
    fn load(&self) -> Result<Option<String>, String> { todo!() }
    fn save(&self, json: &str) -> Result<(), String> { todo!() }
    fn clear(&self) -> Result<(), String> { todo!() }
}
```

## Use core theme defaults and backend conversions

**What changed:** editable theme defaults live in
`panel_kit_core::theme::ThemeTokens`. Web CSS variables and terminal colors are
emitted or converted from that source; TUI defaults are no longer a second
editable palette.

**Why:** palette, typography, density, and `focus_ring` must not drift between
CSS, ratatui, generated design docs, and parity checks.

After:

```rust
let tokens = panel_kit_core::theme::ThemeTokens::dark();
let tui_theme = panel_kit_tui::ResolvedTuiTheme::from(&tokens);
let css_root = panel_kit::theme::css_root_block(&tokens);
```

## Use core widget models

**What changed:** badge, chart, table, meter, status, scroll, and spinner data
models live in `panel_kit_core::widgets` and `panel_kit_core::badge`. The web
and TUI crates retain renderer-native painters over those models.

**Why:** the same semantic widget data should render on both backends. Backend
modules now decide only how to paint, not what the widget means.

## Move named views into host-owned state

**What changed:** the web `use_views` hook and `Views<K>` controller composite
were removed. The renderer-neutral registry remains in `panel_kit_core::views`
(`SavedViews`, `views_registry_key`, and `view_layout_key`), and web
applications compose it with `LocalStorageLayoutStore`, `restore_snapshot`,
`persist_snapshot` or `SavePolicy`, and the independent web parts.

**Why:** switching, creating, renaming, and deleting views are application
control flow. A second controller would reintroduce the ownership inversion that
1.0 removes.

Before:

```rust
let views = panel_kit::use_views("myapp_layout", default_layout, &["User", "Sessions"]);
let ws = views.workspace;
```

After:

```rust
use panel_kit_core::views::{view_layout_key, views_registry_key, SavedViews};

let registry = SavedViews::new(&["User", "Sessions"]);
let store = panel_kit::store::LocalStorageLayoutStore::new(
    view_layout_key("myapp_layout", &registry.active),
);
// Host-owned restore_snapshot/reduce/SavePolicy/project_into/web-parts loop.
```

Preserve the existing keys: `{base}:views` for the registry and
`{base}:view:{name}` for each layout. On first run, copy any legacy layout at
the bare `{base}` key into the first view's key; do not move or delete the bare
key.

## mkLayout remains layout-only

`nix/lib/mkLayout.nix` is unchanged as the layout-only `SavedLayoutV2` producer.
Feature-complete authored workspaces use `mkWorkspaceSpec`, which is validated
against the generated Rust schema and intentionally emits a different document
shape.

## Migration checklist

- [ ] Keep the consumer pinned to its old panel-kit revision while editing.
- [ ] Replace web controller setup with host-owned `Snapshot`, `reduce`,
      `ProjectionBuffer`, web input adapters, web parts, and an explicit store.
- [ ] Replace terminal controller setup with host-owned `Snapshot`, `reduce`,
      `ProjectionBuffer`, `TuiHitBuffer`, TUI input adapters, TUI parts, and an
      explicit store.
- [ ] Implement any custom persistence transport against
      `panel_kit_core::persist::LayoutStore`, including `clear`.
- [ ] Read theme defaults from `panel_kit_core::theme::ThemeTokens` and convert
      through the backend renderer.
- [ ] Move widget data construction to core badge/widget models and leave
      backend modules to paint.
- [ ] Replace named views with an app-owned `SavedViews` registry plus one store
      per active view.
- [ ] Build the consumer on its real target.
- [ ] Update the panel-kit git revision only after the migrated consumer build
      passes.
