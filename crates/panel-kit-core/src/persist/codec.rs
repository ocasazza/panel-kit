use crate::persist::{LayoutError, RestoreContext};
use crate::reducer::{Snapshot, Viewport};
use crate::{
    merge_defaults, migrate_v1, reconcile_units, Mode, PanelCatalog, PanelKey, PanelWin,
    SavedLayoutV2, StoredLayout, LAYOUT_SCHEMA_VERSION,
};

pub(super) fn restored_snapshot<K: PanelKey>(
    defaults: Snapshot<K>,
    json: &str,
    catalog: &PanelCatalog<K>,
    context: RestoreContext,
) -> Result<Snapshot<K>, LayoutError> {
    let layout = decode_layout(json, context)?;
    let mut panels = decode_panels(layout.panels, catalog);
    merge_defaults(&mut panels, &defaults.panels);

    Ok(snapshot_from_layout(defaults, layout.mode, panels, context))
}

pub(super) fn saved_layout<K: PanelKey>(
    snapshot: &Snapshot<K>,
    catalog: &PanelCatalog<K>,
) -> Result<SavedLayoutV2<String>, LayoutError> {
    Ok(SavedLayoutV2 {
        version: LAYOUT_SCHEMA_VERSION,
        units: snapshot.viewport.units,
        viewport: (snapshot.viewport.width, snapshot.viewport.height),
        mode: snapshot.preferred_mode,
        panels: encode_panels(&snapshot.panels, catalog)?,
    })
}

pub(super) fn validate_viewport(viewport: (f64, f64)) -> Result<(), LayoutError> {
    if viewport.0.is_finite() && viewport.1.is_finite() && viewport.0 > 0.0 && viewport.1 > 0.0 {
        return Ok(());
    }

    Err(LayoutError::InvalidViewport { viewport })
}

fn decode_layout(
    json: &str,
    context: RestoreContext,
) -> Result<SavedLayoutV2<String>, LayoutError> {
    let stored = serde_json::from_str::<StoredLayout<String>>(json)
        .map_err(|error| LayoutError::Decode(error.to_string()))?;

    match stored {
        StoredLayout::V1(old) => Ok(migrate_v1(old, context.units, context.viewport)),
        StoredLayout::V2(layout) => reconcile_v2(layout, context),
    }
}

fn reconcile_v2(
    layout: SavedLayoutV2<String>,
    context: RestoreContext,
) -> Result<SavedLayoutV2<String>, LayoutError> {
    if layout.version != LAYOUT_SCHEMA_VERSION {
        return Err(LayoutError::UnsupportedVersion(layout.version));
    }
    validate_viewport(layout.viewport)?;

    Ok(reconcile_units(layout, context.units, context.viewport))
}

fn snapshot_from_layout<K: PanelKey>(
    mut defaults: Snapshot<K>,
    mode: Mode,
    panels: Vec<PanelWin<K>>,
    context: RestoreContext,
) -> Snapshot<K> {
    defaults.panels = panels;
    defaults.preferred_mode = mode;
    defaults.viewport = Viewport {
        width: context.viewport.0,
        height: context.viewport.1,
        units: context.units,
    };
    defaults.focused = defaults
        .focused
        .filter(|focused| defaults.panels.iter().any(|panel| panel.kind == *focused));
    defaults.drag = None;
    defaults.tile_drag = None;
    defaults.workspace_scroll = 0.0;
    defaults
}

fn encode_panels<K: PanelKey>(
    panels: &[PanelWin<K>],
    catalog: &PanelCatalog<K>,
) -> Result<Vec<PanelWin<String>>, LayoutError> {
    panels
        .iter()
        .map(|panel| encode_panel(panel, catalog))
        .collect()
}

fn encode_panel<K: PanelKey>(
    panel: &PanelWin<K>,
    catalog: &PanelCatalog<K>,
) -> Result<PanelWin<String>, LayoutError> {
    let stable_id = catalog
        .stable_id(panel.kind)
        .ok_or(LayoutError::MissingStableId)?;

    Ok(PanelWin {
        kind: stable_id.to_owned(),
        x: panel.x,
        y: panel.y,
        w: panel.w,
        h: panel.h,
        state: panel.state,
        z: panel.z,
        tile_w: panel.tile_w,
        tile_h: panel.tile_h,
    })
}

fn decode_panels<K: PanelKey>(
    panels: Vec<PanelWin<String>>,
    catalog: &PanelCatalog<K>,
) -> Vec<PanelWin<K>> {
    panels
        .into_iter()
        .filter_map(|panel| decode_panel(panel, catalog))
        .collect()
}

fn decode_panel<K: PanelKey>(
    panel: PanelWin<String>,
    catalog: &PanelCatalog<K>,
) -> Option<PanelWin<K>> {
    let meta = catalog.get_by_stable_id(&panel.kind)?;

    Some(PanelWin {
        kind: meta.key,
        x: panel.x,
        y: panel.y,
        w: panel.w,
        h: panel.h,
        state: panel.state,
        z: panel.z,
        tile_w: panel.tile_w,
        tile_h: panel.tile_h,
    })
}
