//! JSON pre-walkers for strict authored spec diagnostics.
//!
//! These walkers mirror serde's strict tagged shapes before typed decoding so
//! callers receive every unknown/missing-field diagnostic with stable JSON
//! pointers instead of serde's first error only.

use std::collections::HashSet;

use crate::theme::Color;
use crate::{TILE_H_MAX, TILE_W_MAX};

use super::ErrorSink;

type JsonMap = serde_json::Map<String, serde_json::Value>;

/// Walk a workspace-spec JSON value and accumulate strict shape diagnostics.
pub(super) fn walk_workspace(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else {
        return;
    };
    check_fields_with_optional(object, pointer, WORKSPACE_FIELDS, WORKSPACE_OPTIONAL_FIELDS, errors);
    walk_number_field(object, pointer, "spec_version", errors);
    walk_string_field(object, pointer, "id", errors);
    if let Some(value) = object.get("layout") {
        walk_layout(value, "/layout", errors);
    }
    if let Some(value) = object.get("surface") {
        walk_surface(value, "/surface", errors);
    }
    if let Some(value) = object.get("chrome") {
        walk_chrome(value, "/chrome", errors);
    }
    if let Some(value) = object.get("input") {
        walk_input(value, "/input", errors);
    }
    if let Some(value) = object.get("theme") {
        walk_theme(value, "/theme", errors);
    }
    if let Some(value) = object.get("persistence") {
        walk_persistence(value, "/persistence", errors);
    }
    if let Some(value) = object.get("panels") {
        walk_panels(value, "/panels", errors);
    }
    if let Some(value) = object.get("components") {
        walk_components(value, "/components", errors);
    }
}

/// Walk a provider manifest JSON value and accumulate strict shape diagnostics.
pub(super) fn walk_manifest(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else {
        return;
    };
    check_fields(object, pointer, MANIFEST_FIELDS, errors);
    if let Some(panels) = object.get("panels") {
        walk_manifest_panels(panels, "/panels", errors);
    }
}

fn walk_layout(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else {
        return;
    };
    check_fields(object, pointer, LAYOUT_FIELDS, errors);
    if let Some(viewport) = object.get("viewport") {
        walk_viewport(viewport, "/layout/viewport", errors);
    }
    if let Some(clamp) = object.get("clamp") {
        walk_object_fields(clamp, "/layout/clamp", CLAMP_FIELDS, errors);
    }
    if let Some(tile) = object.get("tile") {
        walk_tile(tile, "/layout/tile", errors);
    }
}

fn walk_tile(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else {
        return;
    };
    check_fields(object, pointer, TILE_LAYOUT_FIELDS, errors);
    if let Some(resize) = object.get("resize") {
        if let Some(object) = object_at(resize, "/layout/tile/resize", errors) {
            check_fields_with_optional(
                object,
                "/layout/tile/resize",
                TILE_METRIC_FIELDS,
                TILE_METRIC_OPTIONAL_FIELDS,
                errors,
            );
        }
    }
}

fn walk_surface(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else {
        return;
    };
    check_fields(object, pointer, SURFACE_FIELDS, errors);
    if let (Some(compact), Some(tablet)) = (
        object.get("compact_max").and_then(|value| value.as_f64()),
        object.get("tablet_max").and_then(|value| value.as_f64()),
    ) {
        if !tablet.is_finite() || tablet <= compact {
            errors.push(
                "/surface/tablet_max",
                "tablet_max must be finite and greater than compact_max",
            );
        }
    }
    if let Some(caps) = object.get("fallback_capabilities") {
        walk_object_fields(
            caps,
            "/surface/fallback_capabilities",
            SURFACE_CAP_FIELDS,
            errors,
        );
    }
}

fn walk_chrome(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else {
        return;
    };
    check_fields(object, pointer, CHROME_FIELDS, errors);
    if let Some(metrics) = object.get("metrics") {
        walk_object_fields(metrics, "/chrome/metrics", CHROME_METRIC_FIELDS, errors);
    }
}

fn walk_input(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else {
        return;
    };
    check_fields(object, pointer, INPUT_FIELDS, errors);
    if let Some(steps) = object.get("steps") {
        walk_object_fields(steps, "/input/steps", COMMAND_STEP_FIELDS, errors);
    }
    if let Some(keymaps) = object.get("keymaps") {
        walk_keymaps(keymaps, "/input/keymaps", errors);
    }
}

fn walk_keymaps(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(keymaps) = array_at(value, pointer, errors) else {
        return;
    };
    for (index, layer) in keymaps.iter().enumerate() {
        let item = format!("{pointer}/{index}");
        let Some(object) = object_at(layer, &item, errors) else {
            continue;
        };
        check_fields(object, &item, KEYMAP_FIELDS, errors);
        let rules_pointer = format!("{item}/rules");
        let Some(rules) = object.get("rules").and_then(|r| array_at(r, &rules_pointer, errors)) else {
            continue;
        };
        for (rule_index, rule) in rules.iter().enumerate() {
            let rule_pointer = format!("{rules_pointer}/{rule_index}");
            if let Some(object) = object_at(rule, &rule_pointer, errors) {
                check_fields(object, &rule_pointer, KEYMAP_RULE_FIELDS, errors);
            }
        }
    }
}

fn walk_theme(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else {
        return;
    };
    check_fields(object, pointer, THEME_FIELDS, errors);
    if let Some(colors) = object.get("colors") {
        walk_colors(colors, "/theme/colors", errors);
    }
    if let Some(typography) = object.get("typography") {
        walk_object_fields(typography, "/theme/typography", TYPOGRAPHY_FIELDS, errors);
    }
    if let Some(density) = object.get("density") {
        walk_object_fields(density, "/theme/density", DENSITY_FIELDS, errors);
    }
}

fn walk_colors(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else {
        return;
    };
    check_fields(object, pointer, COLOR_FIELDS, errors);
    for field in COLOR_FIELDS {
        if let Some(value) = object.get(*field) {
            match value.as_str() {
                Some(text) if Color::parse(text).is_ok() => {}
                Some(_) => errors.push(
                    format!("{pointer}/{field}"),
                    "expected canonical lowercase #rrggbb color",
                ),
                None => errors.push(format!("{pointer}/{field}"), "expected color string"),
            }
        }
    }
}

fn walk_persistence(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else {
        return;
    };
    check_fields(object, pointer, PERSISTENCE_FIELDS, errors);
}

fn walk_panels(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(panels) = array_at(value, pointer, errors) else {
        return;
    };
    let mut ids = HashSet::new();
    let mut slugs = HashSet::new();
    for (index, panel) in panels.iter().enumerate() {
        let item = format!("{pointer}/{index}");
        let Some(object) = object_at(panel, &item, errors) else {
            continue;
        };
        check_fields(object, &item, PANEL_FIELDS, errors);
        if let Some(id) = string_value(object, "id") {
            if id.is_empty() {
                errors.push(format!("{item}/id"), "panel id must not be empty");
            }
            if !ids.insert(id.to_string()) {
                errors.push(format!("{item}/id"), "duplicate panel id");
            }
        }
        if let Some(slug) = string_value(object, "slug") {
            if slug.is_empty() {
                errors.push(format!("{item}/slug"), "panel slug must not be empty");
            }
            if !slugs.insert(slug.to_string()) {
                errors.push(format!("{item}/slug"), "duplicate panel slug");
            }
        }
        if let Some(window) = object.get("window") {
            walk_window(window, &format!("{item}/window"), errors);
        }
        if let Some(content) = object.get("content") {
            walk_content(content, &format!("{item}/content"), errors);
        }
    }
}

fn walk_window(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else {
        return;
    };
    check_fields(object, pointer, WINDOW_FIELDS, errors);
    if let Some(tile_w) = object.get("tile_w").and_then(|value| value.as_u64()) {
        if tile_w == 0 || tile_w > TILE_W_MAX as u64 {
            errors.push(format!("{pointer}/tile_w"), "tile_w must be in 1..=4");
        }
    }
    if let Some(tile_h) = object.get("tile_h").and_then(|value| value.as_u64()) {
        if tile_h == 0 || tile_h > TILE_H_MAX as u64 {
            errors.push(format!("{pointer}/tile_h"), "tile_h must be in 1..=6");
        }
    }
}

fn walk_content(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else {
        return;
    };
    let Some(kind) = string_value(object, "kind") else {
        if object.contains_key("kind") {
            errors.push(format!("{pointer}/kind"), "expected string");
        } else {
            errors.push(format!("{pointer}/kind"), "missing required field");
        }
        return;
    };

    match kind {
        "custom" => check_fields(object, pointer, CONTENT_CUSTOM_FIELDS, errors),
        "text" => {
            check_fields(object, pointer, CONTENT_TEXT_FIELDS, errors);
            if let Some(source) = object.get("source") {
                walk_data_source(source, &join(pointer, "source"), errors, walk_text_model);
            }
        }
        "editor" => check_fields(object, pointer, CONTENT_EDITOR_FIELDS, errors),
        "badges" => {
            check_fields(object, pointer, CONTENT_SOURCE_FIELDS, errors);
            if let Some(source) = object.get("source") {
                walk_data_source(
                    source,
                    &join(pointer, "source"),
                    errors,
                    |value, value_pointer, errors| {
                        walk_array_items(value, value_pointer, errors, walk_badge_spec);
                    },
                );
            }
        }
        "table" => {
            check_fields(object, pointer, CONTENT_SOURCE_FIELDS, errors);
            if let Some(source) = object.get("source") {
                walk_data_source(source, &join(pointer, "source"), errors, walk_table_model);
            }
        }
        "time_series" => {
            check_fields(object, pointer, CONTENT_TIME_SERIES_FIELDS, errors);
            if let Some(source) = object.get("source") {
                walk_data_source(
                    source,
                    &join(pointer, "source"),
                    errors,
                    |value, value_pointer, errors| {
                        walk_array_items(value, value_pointer, errors, walk_series_model);
                    },
                );
            }
        }
        "gauges" => {
            check_fields(object, pointer, CONTENT_SOURCE_FIELDS, errors);
            if let Some(source) = object.get("source") {
                walk_data_source(
                    source,
                    &join(pointer, "source"),
                    errors,
                    |value, value_pointer, errors| {
                        walk_array_items(value, value_pointer, errors, walk_gauge_model);
                    },
                );
            }
        }
        "flamegraph" => {
            check_fields(object, pointer, CONTENT_SOURCE_FIELDS, errors);
            if let Some(source) = object.get("source") {
                walk_data_source(
                    source,
                    &join(pointer, "source"),
                    errors,
                    |value, value_pointer, errors| {
                        walk_array_items(value, value_pointer, errors, walk_flame_span_model);
                    },
                );
            }
        }
        "boxplot" => {
            check_fields(object, pointer, CONTENT_SOURCE_FIELDS, errors);
            if let Some(source) = object.get("source") {
                walk_data_source(
                    source,
                    &join(pointer, "source"),
                    errors,
                    |value, value_pointer, errors| {
                        walk_array_items(value, value_pointer, errors, walk_box_item_model);
                    },
                );
            }
        }
        "meter" => {
            check_fields(object, pointer, CONTENT_SOURCE_FIELDS, errors);
            if let Some(source) = object.get("source") {
                walk_data_source(source, &join(pointer, "source"), errors, walk_meter_model);
            }
        }
        "status" => {
            check_fields(object, pointer, CONTENT_SOURCE_FIELDS, errors);
            if let Some(source) = object.get("source") {
                walk_data_source(source, &join(pointer, "source"), errors, walk_status_model);
            }
        }
        "spinner" => {
            check_fields(object, pointer, CONTENT_SPINNER_FIELDS, errors);
            if let Some(label) = object.get("label") {
                walk_data_source(label, &join(pointer, "label"), errors, walk_string_value);
            }
        }
        "external" => {
            check_fields(object, pointer, CONTENT_EXTERNAL_FIELDS, errors);
            walk_string_field(object, pointer, "component", errors);
            if let Some(source) = object.get("source") {
                walk_data_source(source, &join(pointer, "source"), errors, walk_graph_model);
            }
            if let Some(layout) = object.get("layout") {
                walk_graph_layout(layout, &join(pointer, "layout"), errors);
            }
        }
        _ => errors.push(
            format!("{pointer}/kind"),
            format!("unsupported content kind {kind}"),
        ),
    }
}

fn walk_components(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(components) = array_at(value, pointer, errors) else {
        return;
    };
    let mut ids = HashSet::new();
    for (index, component) in components.iter().enumerate() {
        let item = format!("{pointer}/{index}");
        let Some(object) = object_at(component, &item, errors) else {
            continue;
        };
        check_fields(object, &item, COMPONENT_FIELDS, errors);
        if let Some(id) = string_value(object, "id") {
            if id.is_empty() {
                errors.push(format!("{item}/id"), "component id must not be empty");
            }
            if !ids.insert(id.to_string()) {
                errors.push(format!("{item}/id"), "duplicate component id");
            }
        }
        check_enum_field(object, &item, "contract", COMPONENT_CONTRACTS, errors);
        check_enum_array(object, &item, "requires", CAPABILITIES, errors);
        check_enum_array(object, &item, "events", COMPONENT_EVENT_KINDS, errors);
        check_enum_field(object, &item, "fallback", COMPONENT_FALLBACKS, errors);
        if let Some(min_size) = object.get("min_size") {
            walk_positive_pair(min_size, &format!("{item}/min_size"), errors);
        }
    }
}

fn walk_graph_layout(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else {
        return;
    };
    check_fields(object, pointer, GRAPH_LAYOUT_FIELDS, errors);
    check_enum_field(object, pointer, "algorithm", LAYOUT_ALGORITHMS, errors);
    if let Some(group_by) = object.get("group_by") {
        if !group_by.is_null() && !group_by.is_string() {
            errors.push(format!("{pointer}/group_by"), "expected string or null");
        }
    }
    if let Some(params) = object.get("params") {
        walk_finite_numbers(params, &join(pointer, "params"), errors);
    }
}

fn walk_graph_model(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else {
        return;
    };
    check_fields(object, pointer, GRAPH_MODEL_FIELDS, errors);
    if let Some(nodes) = object.get("nodes") {
        walk_array_items(nodes, &join(pointer, "nodes"), errors, walk_graph_node);
    }
    if let Some(edges) = object.get("edges") {
        walk_array_items(edges, &join(pointer, "edges"), errors, walk_graph_edge);
    }
}

fn walk_graph_node(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else {
        return;
    };
    check_fields(object, pointer, GRAPH_NODE_FIELDS, errors);
    walk_string_field(object, pointer, "id", errors);
    walk_string_field(object, pointer, "label", errors);
    walk_string_field(object, pointer, "sort", errors);
    if let Some(color) = object.get("color") {
        walk_channel_triple(color, &join(pointer, "color"), errors);
    }
    for field in ["size", "mass", "repulsion"] {
        walk_finite_field(object, pointer, field, errors);
    }
}

fn walk_graph_edge(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else {
        return;
    };
    check_fields(object, pointer, GRAPH_EDGE_FIELDS, errors);
    for field in ["source", "target"] {
        match object.get(field).and_then(serde_json::Value::as_u64) {
            Some(_) => {}
            None => errors.push(format!("{pointer}/{field}"), "expected node index"),
        }
    }
    walk_string_field(object, pointer, "kind", errors);
    for field in ["weight", "rest_length"] {
        walk_finite_field(object, pointer, field, errors);
    }
}

fn walk_positive_pair(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(values) = array_at(value, pointer, errors) else {
        return;
    };
    for index in 0..2 {
        match values.get(index).and_then(serde_json::Value::as_f64) {
            Some(number) if number.is_finite() && number > 0.0 => {}
            Some(_) => errors.push(
                format!("{pointer}/{index}"),
                "extent must be finite and positive",
            ),
            None => errors.push(format!("{pointer}/{index}"), "missing extent"),
        }
    }
}

fn walk_channel_triple(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(values) = array_at(value, pointer, errors) else {
        return;
    };
    if values.len() != 3 {
        errors.push(pointer, "expected three sRGB channels");
        return;
    }
    for (index, channel) in values.iter().enumerate() {
        if !channel.is_u64() || channel.as_u64().is_some_and(|value| value > 255) {
            errors.push(format!("{pointer}/{index}"), "expected a u8 channel");
        }
    }
}

fn walk_finite_numbers(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else {
        return;
    };
    for (name, param) in object {
        match param.as_f64() {
            Some(number) if number.is_finite() => {}
            Some(_) => errors.push(
                format!("{pointer}/{name}"),
                "layout parameter must be finite",
            ),
            None => errors.push(format!("{pointer}/{name}"), "expected number"),
        }
    }
}

fn walk_finite_field(
    object: &JsonMap,
    pointer: &str,
    field: &str,
    errors: &mut ErrorSink,
) {
    match object.get(field) {
        Some(value) if value.as_f64().is_some_and(f64::is_finite) => {}
        Some(_) => errors.push(format!("{pointer}/{field}"), "expected finite number"),
        None => {}
    }
}

fn check_enum_field(
    object: &JsonMap,
    pointer: &str,
    field: &str,
    allowed: &[&str],
    errors: &mut ErrorSink,
) {
    let Some(value) = object.get(field) else {
        return;
    };
    match value.as_str() {
        Some(name) if allowed.contains(&name) => {}
        Some(name) => errors.push(
            format!("{pointer}/{field}"),
            format!("unsupported {field} {name}"),
        ),
        None => errors.push(format!("{pointer}/{field}"), "expected string"),
    }
}

fn check_enum_array(
    object: &JsonMap,
    pointer: &str,
    field: &str,
    allowed: &[&str],
    errors: &mut ErrorSink,
) {
    let item = join(pointer, field);
    let Some(values) = object.get(field).and_then(|value| array_at(value, &item, errors)) else {
        return;
    };
    for (index, value) in values.iter().enumerate() {
        let item = format!("{pointer}/{field}/{index}");
        match value.as_str() {
            Some(name) if allowed.contains(&name) => {}
            Some(name) => errors.push(&item, format!("unsupported {field} {name}")),
            None => errors.push(&item, "expected string"),
        }
    }
}

fn walk_data_source<F>(
    value: &serde_json::Value,
    pointer: &str,
    errors: &mut ErrorSink,
    walk_inline_value: F,
) where
    F: Fn(&serde_json::Value, &str, &mut ErrorSink),
{
    let Some(object) = object_at(value, pointer, errors) else {
        return;
    };
    let Some(source) = string_value(object, "source") else {
        if object.contains_key("source") {
            errors.push(format!("{pointer}/source"), "expected string");
        } else {
            errors.push(format!("{pointer}/source"), "missing required field");
        }
        return;
    };

    match source {
        "inline" => {
            check_fields(object, pointer, DATA_SOURCE_INLINE_FIELDS, errors);
            if let Some(value) = object.get("value") {
                walk_inline_value(value, &join(pointer, "value"), errors);
            }
        }
        "binding" => {
            check_fields(object, pointer, DATA_SOURCE_BINDING_FIELDS, errors);
            walk_string_field(object, pointer, "id", errors);
        }
        _ => errors.push(format!("{pointer}/source"), "expected inline or binding"),
    }
}

fn walk_text_model(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else {
        return;
    };
    check_fields(object, pointer, TEXT_MODEL_FIELDS, errors);
    walk_string_field(object, pointer, "text", errors);
}

fn walk_badge_spec(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else {
        return;
    };
    check_fields_with_optional(
        object,
        pointer,
        BADGE_SPEC_FIELDS,
        BADGE_SPEC_OPTIONAL_FIELDS,
        errors,
    );
}

fn walk_table_model(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else {
        return;
    };
    check_fields(object, pointer, TABLE_MODEL_FIELDS, errors);
    if let Some(columns) = object.get("columns") {
        walk_array_items(
            columns,
            &join(pointer, "columns"),
            errors,
            walk_table_column,
        );
    }
    if let Some(rows) = object.get("rows") {
        walk_array_items(rows, &join(pointer, "rows"), errors, walk_table_row);
    }
}

fn walk_table_column(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else {
        return;
    };
    check_fields_with_optional(
        object,
        pointer,
        TABLE_COLUMN_FIELDS,
        TABLE_COLUMN_OPTIONAL_FIELDS,
        errors,
    );
    if let Some(width) = object.get("width") {
        walk_column_width(width, &join(pointer, "width"), errors);
    }
}

fn walk_column_width(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else {
        return;
    };
    if object.len() != 1 {
        errors.push(pointer, "column width must have exactly one variant");
        return;
    }
    if let Some(fixed) = object.get("Fixed") {
        walk_object_fields(
            fixed,
            &join(pointer, "Fixed"),
            COLUMN_WIDTH_FIXED_FIELDS,
            errors,
        );
    } else if let Some(flex) = object.get("Flex") {
        walk_object_fields(
            flex,
            &join(pointer, "Flex"),
            COLUMN_WIDTH_FLEX_FIELDS,
            errors,
        );
    } else if let Some(name) = object.keys().next() {
        errors.push(
            format!("{pointer}/{name}"),
            "unsupported column width variant",
        );
    }
}

fn walk_table_row(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else {
        return;
    };
    check_fields_with_optional(
        object,
        pointer,
        TABLE_ROW_FIELDS,
        TABLE_ROW_OPTIONAL_FIELDS,
        errors,
    );
    if let Some(cells) = object.get("cells") {
        walk_array_items(cells, &join(pointer, "cells"), errors, walk_table_cell);
    }
}

fn walk_table_cell(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else {
        return;
    };
    let Some(kind) = string_value(object, "kind") else {
        errors.push(format!("{pointer}/kind"), "missing required field");
        return;
    };
    match kind {
        "text" => check_fields(object, pointer, TABLE_CELL_TEXT_FIELDS, errors),
        "status" => check_fields(object, pointer, TABLE_CELL_VALUE_FIELDS, errors),
        "meter" => check_fields(object, pointer, TABLE_CELL_VALUE_FIELDS, errors),
        _ => errors.push(
            format!("{pointer}/kind"),
            format!("unsupported table cell kind {kind}"),
        ),
    }
}

fn walk_series_model(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else {
        return;
    };
    check_fields(object, pointer, SERIES_MODEL_FIELDS, errors);
}

fn walk_gauge_model(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else {
        return;
    };
    check_fields(object, pointer, GAUGE_MODEL_FIELDS, errors);
}

fn walk_flame_span_model(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else {
        return;
    };
    check_fields(object, pointer, FLAME_SPAN_MODEL_FIELDS, errors);
}

fn walk_box_item_model(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else {
        return;
    };
    check_fields(object, pointer, BOX_ITEM_MODEL_FIELDS, errors);
}

fn walk_meter_model(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else {
        return;
    };
    check_fields(object, pointer, METER_MODEL_FIELDS, errors);
}

fn walk_status_model(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else {
        return;
    };
    check_fields(object, pointer, STATUS_MODEL_FIELDS, errors);
}

fn walk_string_value(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    if !value.is_string() {
        errors.push(pointer, "expected string");
    }
}

fn walk_manifest_panels(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(panels) = array_at(value, pointer, errors) else {
        return;
    };
    for (index, panel) in panels.iter().enumerate() {
        let item = format!("{pointer}/{index}");
        let Some(object) = object_at(panel, &item, errors) else {
            continue;
        };
        check_fields(object, &item, PROVIDER_FIELDS, errors);
    }
}

fn walk_viewport(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(values) = array_at(value, pointer, errors) else {
        return;
    };
    for index in 0..2 {
        match values.get(index).and_then(|value| value.as_f64()) {
            Some(number) if number.is_finite() && number > 0.0 => {}
            Some(_) => errors.push(
                format!("{pointer}/{index}"),
                "viewport dimension must be finite and positive",
            ),
            None => errors.push(format!("{pointer}/{index}"), "missing viewport dimension"),
        }
    }
}

fn walk_object_fields(
    value: &serde_json::Value,
    pointer: &str,
    fields: &[&str],
    errors: &mut ErrorSink,
) {
    if let Some(object) = object_at(value, pointer, errors) {
        check_fields(object, pointer, fields, errors);
    }
}

fn walk_array_items<F>(
    value: &serde_json::Value,
    pointer: &str,
    errors: &mut ErrorSink,
    walk_item: F,
) where
    F: Fn(&serde_json::Value, &str, &mut ErrorSink),
{
    let Some(items) = array_at(value, pointer, errors) else {
        return;
    };
    for (index, item) in items.iter().enumerate() {
        walk_item(item, &format!("{pointer}/{index}"), errors);
    }
}

fn check_fields(object: &JsonMap, pointer: &str, allowed: &[&str], errors: &mut ErrorSink) {
    check_fields_with_optional(object, pointer, allowed, &[], errors);
}

fn check_fields_with_optional(
    object: &JsonMap,
    pointer: &str,
    required: &[&str],
    optional: &[&str],
    errors: &mut ErrorSink,
) {
    for key in object.keys() {
        if !required.contains(&key.as_str()) && !optional.contains(&key.as_str()) {
            let allowed = required.iter().chain(optional).copied().collect::<Vec<_>>();
            errors.push(
                format!("{pointer}/{key}"),
                format!("unknown field; allowed fields: {}", allowed.join(", ")),
            );
        }
    }
    for field in required {
        if !object.contains_key(*field) {
            errors.push(format!("{pointer}/{field}"), "missing required field");
        }
    }
}

fn object_at<'a>(
    value: &'a serde_json::Value,
    pointer: &str,
    errors: &mut ErrorSink,
) -> Option<&'a JsonMap> {
    value.as_object().or_else(|| {
        errors.push(pointer, "expected object");
        None
    })
}

fn array_at<'a>(
    value: &'a serde_json::Value,
    pointer: &str,
    errors: &mut ErrorSink,
) -> Option<&'a Vec<serde_json::Value>> {
    value.as_array().or_else(|| {
        errors.push(pointer, "expected array");
        None
    })
}

fn walk_number_field(object: &JsonMap, pointer: &str, field: &str, errors: &mut ErrorSink) {
    if object.get(field).is_some_and(|value| !value.is_number()) {
        errors.push(format!("{pointer}/{field}"), "expected number");
    }
}

fn walk_string_field(object: &JsonMap, pointer: &str, field: &str, errors: &mut ErrorSink) {
    if object.get(field).is_some_and(|value| !value.is_string()) {
        errors.push(format!("{pointer}/{field}"), "expected string");
    }
}

fn string_value<'a>(object: &'a JsonMap, field: &str) -> Option<&'a str> {
    object.get(field).and_then(|value| value.as_str())
}

fn join(pointer: &str, segment: &str) -> String {
    format!("{pointer}/{segment}")
}

const WORKSPACE_FIELDS: &[&str] = &[
    "spec_version",
    "id",
    "layout",
    "surface",
    "chrome",
    "input",
    "glyphs",
    "theme",
    "persistence",
    "panels",
];
/// `#[serde(default)]` fields of `WorkspaceSpec`: accepted, never required.
const WORKSPACE_OPTIONAL_FIELDS: &[&str] = &["components"];
const LAYOUT_FIELDS: &[&str] = &["units", "viewport", "preferred_mode", "clamp", "tile"];
const TILE_LAYOUT_FIELDS: &[&str] = &["resize", "row_min", "gap", "padding", "fill_viewport"];
const SURFACE_FIELDS: &[&str] = &[
    "compact_max",
    "tablet_max",
    "fallback_capabilities",
    "resize_policy",
];
const CHROME_FIELDS: &[&str] = &[
    "metrics",
    "hit_target_min",
    "panel_header_h",
    "panel_frame",
    "title_in_border",
    "mode_control",
    "minimize_control",
    "maximize_control",
    "resize_grip",
    "dock",
    "dock_label",
];
const INPUT_FIELDS: &[&str] = &["steps", "keymaps"];
const KEYMAP_FIELDS: &[&str] = &["spec_version", "id", "modes", "rules"];
const KEYMAP_RULE_FIELDS: &[&str] = &["keys", "intent", "modes", "enter", "label", "group"];
const PERSISTENCE_FIELDS: &[&str] = &["enabled", "key", "restore", "save_policy"];
const PANEL_FIELDS: &[&str] = &["id", "title", "slug", "window", "content"];
const WINDOW_FIELDS: &[&str] = &["x", "y", "w", "h", "state", "z", "tile_w", "tile_h"];
const MANIFEST_FIELDS: &[&str] = &["backend", "panels"];
const PROVIDER_FIELDS: &[&str] = &["panel_id", "content_kind", "binding"];
const CLAMP_FIELDS: &[&str] = &[
    "outer_w", "outer_h", "floor_w", "floor_h", "inner", "edge", "min_w", "min_h", "max_frac",
];
const TILE_METRIC_FIELDS: &[&str] = &["row", "col_floor", "outer"];
/// `#[serde(default)]` fields of `TileMetrics`: accepted, never required.
const TILE_METRIC_OPTIONAL_FIELDS: &[&str] = &["gap", "padding"];
const SURFACE_CAP_FIELDS: &[&str] = &["coarse_pointer", "hover", "keyboard"];
const CHROME_METRIC_FIELDS: &[&str] = &["inset", "dock_h"];
const COMMAND_STEP_FIELDS: &[&str] = &["coarse", "fine"];
const THEME_FIELDS: &[&str] = &["colors", "typography", "density"];
const COLOR_FIELDS: &[&str] = &[
    "background",
    "surface",
    "text",
    "text_muted",
    "divider",
    "border",
    "inverse_background",
    "inverse_text",
    "focus_ring",
    "primary",
    "secondary",
    "tertiary",
    "highlight",
    "success",
    "failure",
    "error",
    "warning",
    "info",
];
const TYPOGRAPHY_FIELDS: &[&str] = &[
    "family",
    "body_size",
    "body_line_height",
    "label_size",
    "label_weight",
    "label_tracking",
];
const DENSITY_FIELDS: &[&str] = &[
    "panel_radius",
    "badge_radius",
    "spacing_xs",
    "spacing_sm",
    "spacing_md",
];

const CONTENT_CUSTOM_FIELDS: &[&str] = &["kind", "binding"];
const CONTENT_TEXT_FIELDS: &[&str] = &["kind", "source", "scroll"];
const CONTENT_EDITOR_FIELDS: &[&str] = &["kind", "binding", "multiline", "placeholder"];
const CONTENT_SOURCE_FIELDS: &[&str] = &["kind", "source"];
const CONTENT_TIME_SERIES_FIELDS: &[&str] = &["kind", "source", "unit"];
const CONTENT_SPINNER_FIELDS: &[&str] = &["kind", "label"];
const CONTENT_EXTERNAL_FIELDS: &[&str] = &["kind", "component", "source", "layout"];
const COMPONENT_FIELDS: &[&str] = &[
    "id",
    "contract",
    "requires",
    "keep_mounted",
    "min_size",
    "events",
    "fallback",
];
const GRAPH_MODEL_FIELDS: &[&str] = &["nodes", "edges"];
const GRAPH_NODE_FIELDS: &[&str] = &[
    "id",
    "label",
    "sort",
    "color",
    "size",
    "mass",
    "repulsion",
];
const GRAPH_EDGE_FIELDS: &[&str] = &[
    "source",
    "target",
    "kind",
    "weight",
    "rest_length",
];
const GRAPH_LAYOUT_FIELDS: &[&str] = &["algorithm", "group_by", "params"];
const COMPONENT_CONTRACTS: &[&str] = &["graph"];
const CAPABILITIES: &[&str] = &["webgpu"];
const COMPONENT_EVENT_KINDS: &[&str] = &["select", "hover"];
const COMPONENT_FALLBACKS: &[&str] = &["cells", "notice"];
const LAYOUT_ALGORITHMS: &[&str] = &[
    "force",
    "circle",
    "grid",
    "concentric",
    "spectral",
    "dagre",
    "fcose",
    "sphere",
];
const DATA_SOURCE_INLINE_FIELDS: &[&str] = &["source", "value"];
const DATA_SOURCE_BINDING_FIELDS: &[&str] = &["source", "id"];
const TEXT_MODEL_FIELDS: &[&str] = &["text"];
const BADGE_SPEC_FIELDS: &[&str] = &[
    "kind",
    "field",
    "value",
    "active",
    "with_x",
    "with_plus",
    "small",
    "override_color",
    "accent_color",
    "click_kind",
    "emit_hover",
];
const BADGE_SPEC_OPTIONAL_FIELDS: &[&str] = &["tone", "variant"];
const TABLE_MODEL_FIELDS: &[&str] = &["columns", "rows"];
const TABLE_COLUMN_FIELDS: &[&str] = &["key", "title", "width", "align"];
const TABLE_COLUMN_OPTIONAL_FIELDS: &[&str] = &["kind"];
const TABLE_ROW_FIELDS: &[&str] = &["id", "cells"];
const TABLE_ROW_OPTIONAL_FIELDS: &[&str] = &["tags"];
const TABLE_CELL_TEXT_FIELDS: &[&str] = &["kind", "value"];
const TABLE_CELL_VALUE_FIELDS: &[&str] = &["kind", "value"];
const COLUMN_WIDTH_FIXED_FIELDS: &[&str] = &["value"];
const COLUMN_WIDTH_FLEX_FIELDS: &[&str] = &["weight"];
const SERIES_MODEL_FIELDS: &[&str] = &["name", "points"];
const GAUGE_MODEL_FIELDS: &[&str] = &["label", "ratio", "text"];
const FLAME_SPAN_MODEL_FIELDS: &[&str] = &["label", "depth", "value", "color"];
const BOX_ITEM_MODEL_FIELDS: &[&str] = &["label", "samples", "color"];
const METER_MODEL_FIELDS: &[&str] = &["label", "ratio", "text", "color"];
const STATUS_MODEL_FIELDS: &[&str] = &["label", "state", "color"];
