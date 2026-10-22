# Panel-Kit Consumer Feature Matrix

**Date:** 2026-10-02  
**Purpose:** Catalog panel-kit feature usage across all consumers to validate grammar-switching design and ensure no consumer is broken.

---

## Consumers

| Consumer | Owner | panel-kit version | Purpose |
|----------|-------|-------------------|---------|
| jump-cannon | ocasazza | latest (git) | Graph explorer, multi-panel workspace |
| athena-console-web | olivecasazza | 1.1.1 (git) | Kubernetes admin console, multi-workspace |
| definitely-not-crosswords | olivecasazza | 1.0.0 (git) | Crossword game, multi-route workspaces |
| gerbil-space-program | ocasazza | 0.2.0 (git) | Bevy game with panel-kit UI overlay |
| snake-pit | ocasazza | latest (git) | Co-developed panel-kit, likely same API |

---

## Feature Matrix

### Core Traits

| Feature | jump-cannon | athena-console | crosswords | gerbil-space |
|---------|-------------|----------------|------------|--------------|
| PanelKind impl | ✅ 21 variants | ✅ 14 variants | ✅ 4-5 per route | ✅ |
| PanelCatalog | ✅ shared | ✅ shared | ✅ per-route | — |
| LayoutStore (localStorage) | ✅ OnSettle | ✅ OnSettle | ✅ route-keyed | — |
| Multi-workspace | ✅ via topos | ✅ User/Sessions views | ✅ route-switching | — |

### Persistence

| Feature | jump-cannon | athena-console | crosswords | gerbil-space |
|---------|-------------|----------------|------------|--------------|
| SavePolicy::OnSettle | ✅ | ✅ | ✅ | — |
| SavePolicy::OnChange | — | — | — | — |
| SavePolicy::Manual | — | — | — | — |
| restore_snapshot | ✅ | ✅ | ✅ | — |
| apply_save_decision | ✅ | ✅ | ✅ | — |
| Layout migration (V1→V2) | ✅ | ✅ | — | — |
| Custom storage keys | ✅ (jc_layout_v10) | ✅ (athena_console_web_v3) | ✅ (route-keyed) | ❌ (old API) |

### Shell & Input

| Feature | jump-cannon | athena-console | crosswords | gerbil-space |
|---------|-------------|----------------|------------|--------------|
| keyboard_event | ✅ | ✅ | ✅ | — |
| pointer_event | ✅ | ✅ | — | — |
| wheel_event | ✅ | ✅ | — | — |
| observe_viewport | ✅ | ✅ | — | — |
| surface_profile | ✅ | ✅ | — | — |
| clear_selection | ✅ | — | — | — |
| release_pointer | ✅ | — | — | — |
| is_editing | ✅ | — | — | — |

### Frame/Projection

| Feature | jump-cannon | athena-console | crosswords | gerbil-space |
|---------|-------------|----------------|------------|--------------|
| project_into | ✅ | ✅ | ✅ | — |
| ProjectedFrame | ✅ | ✅ | ✅ | — |
| ProjectionBuffer | ✅ | ✅ | ✅ | — |
| Clamp | ✅ | ✅ | ✅ | — |
| ChromeMetrics | ✅ | ✅ | ✅ | — |
| TileLayoutMetrics | ✅ | ✅ | ✅ | — |

### Reducer/Types

| Feature | jump-cannon | athena-console | crosswords | gerbil-space |
|---------|-------------|----------------|------------|--------------|
| Snapshot<K> | ✅ core state | ✅ core state | ✅ per-route | — |
| reduce() | ✅ | ✅ | ✅ | — |
| WorkspaceEvent | ✅ | ✅ | ✅ | — |
| HitTarget | ✅ | ✅ | — | — |
| Viewport | ✅ | ✅ | ✅ | — |
| Mode | ✅ | ✅ | ✅ (persisted) | — |
| CommandStep | ✅ | ✅ | — | — |
| ResizePolicy | ✅ | ✅ | — | — |
| SnapPolicy | ✅ | ✅ | — | — |
| TileMetrics | ✅ | ✅ | ✅ | — |
| WinState | ✅ | ✅ | ✅ | — |
| Units | ✅ | ✅ | ✅ | — |

### Layout

| Feature | jump-cannon | athena-console | crosswords | gerbil-space |
|---------|-------------|----------------|------------|--------------|
| LayoutBuilder | ✅ | ✅ | ✅ | ✅ |
| PanelWin | ✅ | ✅ | ✅ | ✅ |
| SavedLayoutV2 | ✅ | ✅ | — | ❌ |
| Mode switching | ✅ | ✅ | ✅ (persisted) | — |
| SurfaceClass | — | — | ✅ | — |
| Compact threshold | — | — | 760px | — |
| Workspace scroll | ✅ | — | — | — |
| Tile reorder | ✅ | — | — | — |

### UI Widgets

| Feature | jump-cannon | athena-console | crosswords | gerbil-space |
|---------|-------------|----------------|------------|--------------|
| Badge/BadgeSpec | ✅ nodes/inspector | ✅ admin/status | ✅ admin/status | — |
| BadgeKind | ✅ | ✅ | ✅ | — |
| BadgeAction | ✅ | — | — | — |
| BadgeClickKind | ✅ | — | — | — |
| LoadingGate | ✅ | — | — | — |
| loading_store | ✅ | — | — | — |
| Spinner | ✅ inline waits | — | — | — |
| MonacoEditor | ✅ importers | — | — | — |
| PANEL_KIT_DARK_THEME | ✅ | — | — | — |
| panel_kit::CSS | ✅ | ✅ | ✅ (theme) | ✅ |

### Spec (WorkspaceSpec)

| Feature | jump-cannon | athena-console | crosswords | gerbil-space |
|---------|-------------|----------------|------------|--------------|
| WorkspaceSpec (authored) | ❌ (legacy) | ❌ (legacy) | ❌ (legacy) | ❌ |
| ResolvedWorkspace | ❌ (new) | ❌ (new) | ❌ (new) | ❌ |
| SpecPanelId | ❌ (new) | ❌ (new) | ❌ (new) | ❌ |
| SpecWorkspaceState | ❌ (new) | ❌ (new) | ❌ (new) | ❌ |
| LayoutSpec | ❌ (new) | ❌ (new) | ❌ (new) | ❌ |
| SurfaceSpec | ❌ (new) | ❌ (new) | ❌ (new) | ❌ |
| ChromeSpec | ❌ (new) | ❌ (new) | ❌ (new) | ❌ |
| InputSpec | ❌ (new) | ❌ (new) | ❌ (new) | ❌ |
| Charset | ❌ (new) | ❌ (new) | ❌ (new) | ❌ |
| ThemeTokens | ❌ (new) | ❌ (new) | ❌ (new) | ❌ |
| PersistenceSpec | ❌ (new) | ❌ (new) | ❌ (new) | ❌ |
| PanelSpec | ❌ (new) | ❌ (new) | ❌ (new) | ❌ |

---

## Key Findings

1. **No consumer uses WorkspaceSpec yet** — the libraryness refactor just landed; all consumers are on the legacy API (host-owned state, manual catalog, custom storage keys). The grammar-switching feature targets the new API surface.

2. **All consumers use host-owned pattern** — `Snapshot<K>` + `reduce()` + `LocalStorageLayoutStore` + app-level signals. There is no `WorkspaceState` type; the adversarial review's P1 finding is confirmed.

3. **Multi-workspace already exists informally** — jump-cannon has topos (multiple WorkspaceSpecs), athena-console has User/Sessions views, crosswords has route-keyed workspaces. Grammar switching is the formalization of an existing pattern.

4. **Persistence keys are heterogeneous** — each consumer uses a different key scheme (`jc_layout_v10`, route-specific keys, etc.). `layout_key_for_spec` must preserve backward compatibility.

5. **Badge system widely used** — three of four consumers use Badge/BadgeSpec. Any grammar abstraction touching panel content must preserve badge integration.

6. **Loading stores partially adopted** — only jump-cannon uses `loading_store` + `LoadingGate`. Other consumers use inline spinners or custom loading states. The loading seam in core (`LoadStatus`) is the right abstraction point.

7. **gerbil-space-program is on 0.2.0** — significantly behind current API. It should not drive P0 design decisions; P0 must not break it but new features need not target it.

---

## Integration Risk Assessment

| Risk | Severity | Mitigation |
|------|----------|------------|
| Legacy key breakage on switch_spec adoption | HIGH | `layout_key_for_spec(base, "default") == base` guarantee; one-time migration on first switch |
| Consumers not using WorkspaceSpec | LOW | switch_spec is additive; legacy consumers unaffected until they adopt new API |
| SpecResolution breaking custom catalog | LOW | spec module already supports PanelCatalog composition |
| Persistence policy conflicts during switch | MEDIUM | Quiesce in-flight saves before switching; documented in recipe |