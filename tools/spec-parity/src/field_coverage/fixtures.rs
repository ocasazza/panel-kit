use panel_kit_core::spec::{BackendKind, BindingManifest, WorkspaceSpec};
use serde_json::{json, Value};

pub(super) fn binding_manifest_for(spec: &WorkspaceSpec, backend: BackendKind) -> BindingManifest {
    BindingManifest {
        backend,
        panels: spec
            .panels
            .iter()
            .map(|panel| panel.provider_declaration())
            .collect(),
    }
}

pub(super) fn reference_spec_value() -> Value {
    json!({
        "spec_version": 1,
        "id": "workspace-spec-reference",
        "layout": {
            "units": "Cells",
            "viewport": [128.0, 52.0],
            "preferred_mode": "Floating",
            "clamp": {
                "outer_w": 0.0, "outer_h": 0.0, "floor_w": 24.0, "floor_h": 8.0,
                "inner": 2.0, "edge": 0.0, "min_w": 20.0, "min_h": 5.0,
                "max_frac": 0.75
            },
            "tile": {
                "resize": { "row": 4.0, "col_floor": 12.0, "outer": 0.0 },
                "row_min": 4.0, "gap": 0.0, "padding": 0.0, "fill_viewport": true
            }
        },
        "surface": {
            "compact_max": 60.0,
            "tablet_max": 110.0,
            "fallback_capabilities": { "coarse_pointer": false, "hover": true, "keyboard": true },
            "resize_policy": "preserve_intent"
        },
        "chrome": {
            "metrics": { "inset": 1.0, "dock_h": 3.0 },
            "hit_target_min": 1.0,
            "panel_header_h": 1.0,
            "panel_frame": true,
            "title_in_border": true,
            "mode_control": true,
            "minimize_control": true,
            "maximize_control": true,
            "resize_grip": true,
            "dock": true,
            "dock_label": "dock:"
        },
        "input": {
            "steps": { "coarse": 2.0, "fine": 1.0 },
            "bindings": [{
                "chord": { "key": "left", "shift": false, "alt": false, "ctrl": false, "meta": false },
                "command": { "kind": "move", "dx": -16.0, "dy": 0.0 }
            }]
        },
        "glyphs": "unicode",
        "theme": {
            "colors": {
                "background": "#0a0a0a", "surface": "#0d0d0d", "text": "#ededed",
                "text_muted": "#7a7a7a", "divider": "#262626", "border": "#5f5f5f",
                "inverse_background": "#ededed", "inverse_text": "#0a0a0a",
                "focus_ring": "#ededed", "primary": "#3b9bff", "secondary": "#ff5fc3",
                "tertiary": "#ffbd2e", "highlight": "#5ef38c", "success": "#27c93f",
                "failure": "#ff5f56", "error": "#ff5f56", "warning": "#ffbd2e",
                "info": "#83b7cc"
            },
            "typography": {
                "family": "ui-monospace", "body_size": 13.0, "body_line_height": 1.5,
                "label_size": 0.72, "label_weight": 700, "label_tracking": 0.06
            },
            "density": {
                "panel_radius": 4.0, "badge_radius": 999.0,
                "spacing_xs": 4.0, "spacing_sm": 6.0, "spacing_md": 8.0
            }
        },
        "persistence": {
            "enabled": true, "key": "panel_kit_reference",
            "restore": true, "save_policy": "on_settle"
        },
        "panels": [{
            "id": "custom",
            "title": "Custom",
            "slug": "custom",
            "window": { "x": 1.0, "y": 1.0, "w": 24.0, "h": 8.0, "state": "Floating", "z": 1, "tile_w": 1, "tile_h": 2 },
            "content": { "kind": "custom", "binding": "canary.custom" }
        }]
    })
}
