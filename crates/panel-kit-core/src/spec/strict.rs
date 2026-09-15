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
    let Some(object) = object_at(value, pointer, errors) else { return };
    check_fields(object, pointer, WORKSPACE_FIELDS, errors);
    walk_number_field(object, pointer, "spec_version", errors);
    walk_string_field(object, pointer, "id", errors);
    if let Some(value) = object.get("layout") { walk_layout(value, "/layout", errors); }
    if let Some(value) = object.get("surface") { walk_surface(value, "/surface", errors); }
    if let Some(value) = object.get("chrome") { walk_chrome(value, "/chrome", errors); }
    if let Some(value) = object.get("input") { walk_input(value, "/input", errors); }
    if let Some(value) = object.get("theme") { walk_theme(value, "/theme", errors); }
    if let Some(value) = object.get("persistence") { walk_persistence(value, "/persistence", errors); }
    if let Some(value) = object.get("panels") { walk_panels(value, "/panels", errors); }
}

/// Walk a provider manifest JSON value and accumulate strict shape diagnostics.
pub(super) fn walk_manifest(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else { return };
    check_fields(object, pointer, MANIFEST_FIELDS, errors);
    if let Some(panels) = object.get("panels") { walk_manifest_panels(panels, "/panels", errors); }
}

fn walk_layout(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else { return };
    check_fields(object, pointer, LAYOUT_FIELDS, errors);
    if let Some(viewport) = object.get("viewport") { walk_viewport(viewport, "/layout/viewport", errors); }
    if let Some(clamp) = object.get("clamp") { walk_object_fields(clamp, "/layout/clamp", CLAMP_FIELDS, errors); }
    if let Some(tile) = object.get("tile") { walk_tile(tile, "/layout/tile", errors); }
}

fn walk_tile(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else { return };
    check_fields(object, pointer, TILE_LAYOUT_FIELDS, errors);
    if let Some(resize) = object.get("resize") { walk_object_fields(resize, "/layout/tile/resize", TILE_METRIC_FIELDS, errors); }
}

fn walk_surface(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else { return };
    check_fields(object, pointer, SURFACE_FIELDS, errors);
    if let (Some(compact), Some(tablet)) = (
        object.get("compact_max").and_then(|value| value.as_f64()),
        object.get("tablet_max").and_then(|value| value.as_f64()),
    ) {
        if !tablet.is_finite() || tablet <= compact {
            errors.push("/surface/tablet_max", "tablet_max must be finite and greater than compact_max");
        }
    }
    if let Some(caps) = object.get("fallback_capabilities") {
        walk_object_fields(caps, "/surface/fallback_capabilities", SURFACE_CAP_FIELDS, errors);
    }
}

fn walk_chrome(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else { return };
    check_fields(object, pointer, CHROME_FIELDS, errors);
    if let Some(metrics) = object.get("metrics") { walk_object_fields(metrics, "/chrome/metrics", CHROME_METRIC_FIELDS, errors); }
}

fn walk_input(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else { return };
    check_fields(object, pointer, INPUT_FIELDS, errors);
    if let Some(steps) = object.get("steps") { walk_object_fields(steps, "/input/steps", COMMAND_STEP_FIELDS, errors); }
    if let Some(bindings) = object.get("bindings") { walk_bindings(bindings, "/input/bindings", errors); }
}

fn walk_bindings(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(bindings) = array_at(value, pointer, errors) else { return };
    let mut seen = HashSet::new();
    for (index, binding) in bindings.iter().enumerate() {
        let item = format!("{pointer}/{index}");
        let Some(object) = object_at(binding, &item, errors) else { continue };
        check_fields(object, &item, COMMAND_BINDING_FIELDS, errors);
        if let Some(chord) = object.get("chord") {
            walk_object_fields(chord, &format!("{item}/chord"), KEY_CHORD_FIELDS, errors);
            if !seen.insert(chord.to_string()) {
                errors.push(format!("{item}/chord"), "duplicate key chord");
            }
        }
    }
}

fn walk_theme(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else { return };
    check_fields(object, pointer, THEME_FIELDS, errors);
    if let Some(colors) = object.get("colors") { walk_colors(colors, "/theme/colors", errors); }
    if let Some(typography) = object.get("typography") { walk_object_fields(typography, "/theme/typography", TYPOGRAPHY_FIELDS, errors); }
    if let Some(density) = object.get("density") { walk_object_fields(density, "/theme/density", DENSITY_FIELDS, errors); }
}

fn walk_colors(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else { return };
    check_fields(object, pointer, COLOR_FIELDS, errors);
    for field in COLOR_FIELDS {
        if let Some(value) = object.get(*field) {
            match value.as_str() {
                Some(text) if Color::parse(text).is_ok() => {}
                Some(_) => errors.push(format!("{pointer}/{field}"), "expected canonical lowercase #rrggbb color"),
                None => errors.push(format!("{pointer}/{field}"), "expected color string"),
            }
        }
    }
}

fn walk_persistence(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else { return };
    check_fields(object, pointer, PERSISTENCE_FIELDS, errors);
}

fn walk_panels(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(panels) = array_at(value, pointer, errors) else { return };
    let mut ids = HashSet::new();
    let mut slugs = HashSet::new();
    for (index, panel) in panels.iter().enumerate() {
        let item = format!("{pointer}/{index}");
        let Some(object) = object_at(panel, &item, errors) else { continue };
        check_fields(object, &item, PANEL_FIELDS, errors);
        if let Some(id) = string_value(object, "id") {
            if id.is_empty() { errors.push(format!("{item}/id"), "panel id must not be empty"); }
            if !ids.insert(id.to_string()) { errors.push(format!("{item}/id"), "duplicate panel id"); }
        }
        if let Some(slug) = string_value(object, "slug") {
            if slug.is_empty() { errors.push(format!("{item}/slug"), "panel slug must not be empty"); }
            if !slugs.insert(slug.to_string()) { errors.push(format!("{item}/slug"), "duplicate panel slug"); }
        }
        if let Some(window) = object.get("window") { walk_window(window, &format!("{item}/window"), errors); }
        if let Some(content) = object.get("content") { walk_content(content, &format!("{item}/content"), errors); }
    }
}

fn walk_window(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else { return };
    check_fields(object, pointer, WINDOW_FIELDS, errors);
    if let Some(tile_w) = object.get("tile_w").and_then(|value| value.as_u64()) {
        if tile_w == 0 || tile_w > TILE_W_MAX as u64 { errors.push(format!("{pointer}/tile_w"), "tile_w must be in 1..=4"); }
    }
    if let Some(tile_h) = object.get("tile_h").and_then(|value| value.as_u64()) {
        if tile_h == 0 || tile_h > TILE_H_MAX as u64 { errors.push(format!("{pointer}/tile_h"), "tile_h must be in 1..=6"); }
    }
}

fn walk_content(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else { return };
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
            if let Some(source) = object.get("source") { walk_data_source(source, &join(pointer, "source"), errors, walk_text_model); }
        }
        "editor" => check_fields(object, pointer, CONTENT_EDITOR_FIELDS, errors),
        "badges" => {
            check_fields(object, pointer, CONTENT_SOURCE_FIELDS, errors);
            if let Some(source) = object.get("source") {
                walk_data_source(source, &join(pointer, "source"), errors, |value, value_pointer, errors| {
                    walk_array_items(value, value_pointer, errors, walk_badge_spec);
                });
            }
        }
        "table" => {
            check_fields(object, pointer, CONTENT_SOURCE_FIELDS, errors);
            if let Some(source) = object.get("source") { walk_data_source(source, &join(pointer, "source"), errors, walk_table_model); }
        }
        "time_series" => {
            check_fields(object, pointer, CONTENT_TIME_SERIES_FIELDS, errors);
            if let Some(source) = object.get("source") {
                walk_data_source(source, &join(pointer, "source"), errors, |value, value_pointer, errors| {
                    walk_array_items(value, value_pointer, errors, walk_series_model);
                });
            }
        }
        "gauges" => {
            check_fields(object, pointer, CONTENT_SOURCE_FIELDS, errors);
            if let Some(source) = object.get("source") {
                walk_data_source(source, &join(pointer, "source"), errors, |value, value_pointer, errors| {
                    walk_array_items(value, value_pointer, errors, walk_gauge_model);
                });
            }
        }
        "flamegraph" => {
            check_fields(object, pointer, CONTENT_SOURCE_FIELDS, errors);
            if let Some(source) = object.get("source") {
                walk_data_source(source, &join(pointer, "source"), errors, |value, value_pointer, errors| {
                    walk_array_items(value, value_pointer, errors, walk_flame_span_model);
                });
            }
        }
        "boxplot" => {
            check_fields(object, pointer, CONTENT_SOURCE_FIELDS, errors);
            if let Some(source) = object.get("source") {
                walk_data_source(source, &join(pointer, "source"), errors, |value, value_pointer, errors| {
                    walk_array_items(value, value_pointer, errors, walk_box_item_model);
                });
            }
        }
        "meter" => {
            check_fields(object, pointer, CONTENT_SOURCE_FIELDS, errors);
            if let Some(source) = object.get("source") { walk_data_source(source, &join(pointer, "source"), errors, walk_meter_model); }
        }
        "status" => {
            check_fields(object, pointer, CONTENT_SOURCE_FIELDS, errors);
            if let Some(source) = object.get("source") { walk_data_source(source, &join(pointer, "source"), errors, walk_status_model); }
        }
        "spinner" => {
            check_fields(object, pointer, CONTENT_SPINNER_FIELDS, errors);
            if let Some(label) = object.get("label") { walk_data_source(label, &join(pointer, "label"), errors, walk_string_value); }
        }
        _ => errors.push(format!("{pointer}/kind"), format!("unsupported content kind {kind}")),
    }
}

fn walk_data_source<F>(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink, walk_inline_value: F)
where
    F: Fn(&serde_json::Value, &str, &mut ErrorSink),
{
    let Some(object) = object_at(value, pointer, errors) else { return };
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
            if let Some(value) = object.get("value") { walk_inline_value(value, &join(pointer, "value"), errors); }
        }
        "binding" => {
            check_fields(object, pointer, DATA_SOURCE_BINDING_FIELDS, errors);
            walk_string_field(object, pointer, "id", errors);
        }
        _ => errors.push(format!("{pointer}/source"), "expected inline or binding"),
    }
}

fn walk_text_model(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else { return };
    check_fields(object, pointer, TEXT_MODEL_FIELDS, errors);
    walk_string_field(object, pointer, "text", errors);
}

fn walk_badge_spec(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else { return };
    check_fields(object, pointer, BADGE_SPEC_FIELDS, errors);
}

fn walk_table_model(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else { return };
    check_fields(object, pointer, TABLE_MODEL_FIELDS, errors);
    if let Some(columns) = object.get("columns") { walk_array_items(columns, &join(pointer, "columns"), errors, walk_table_column); }
    if let Some(rows) = object.get("rows") { walk_array_items(rows, &join(pointer, "rows"), errors, walk_table_row); }
}

fn walk_table_column(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else { return };
    check_fields(object, pointer, TABLE_COLUMN_FIELDS, errors);
    if let Some(width) = object.get("width") { walk_column_width(width, &join(pointer, "width"), errors); }
}

fn walk_column_width(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else { return };
    if object.len() != 1 {
        errors.push(pointer, "column width must have exactly one variant");
        return;
    }
    if let Some(fixed) = object.get("Fixed") {
        walk_object_fields(fixed, &join(pointer, "Fixed"), COLUMN_WIDTH_FIXED_FIELDS, errors);
    } else if let Some(flex) = object.get("Flex") {
        walk_object_fields(flex, &join(pointer, "Flex"), COLUMN_WIDTH_FLEX_FIELDS, errors);
    } else if let Some(name) = object.keys().next() {
        errors.push(format!("{pointer}/{name}"), "unsupported column width variant");
    }
}

fn walk_table_row(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else { return };
    check_fields(object, pointer, TABLE_ROW_FIELDS, errors);
    if let Some(cells) = object.get("cells") { walk_array_items(cells, &join(pointer, "cells"), errors, walk_table_cell); }
}

fn walk_table_cell(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else { return };
    let Some(kind) = string_value(object, "kind") else {
        errors.push(format!("{pointer}/kind"), "missing required field");
        return;
    };
    match kind {
        "text" => check_fields(object, pointer, TABLE_CELL_TEXT_FIELDS, errors),
        "status" => check_fields(object, pointer, TABLE_CELL_VALUE_FIELDS, errors),
        "meter" => check_fields(object, pointer, TABLE_CELL_VALUE_FIELDS, errors),
        _ => errors.push(format!("{pointer}/kind"), format!("unsupported table cell kind {kind}")),
    }
}

fn walk_series_model(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else { return };
    check_fields(object, pointer, SERIES_MODEL_FIELDS, errors);
}

fn walk_gauge_model(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else { return };
    check_fields(object, pointer, GAUGE_MODEL_FIELDS, errors);
}

fn walk_flame_span_model(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else { return };
    check_fields(object, pointer, FLAME_SPAN_MODEL_FIELDS, errors);
}

fn walk_box_item_model(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else { return };
    check_fields(object, pointer, BOX_ITEM_MODEL_FIELDS, errors);
}

fn walk_meter_model(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else { return };
    check_fields(object, pointer, METER_MODEL_FIELDS, errors);
}

fn walk_status_model(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(object) = object_at(value, pointer, errors) else { return };
    check_fields(object, pointer, STATUS_MODEL_FIELDS, errors);
}

fn walk_string_value(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    if !value.is_string() { errors.push(pointer, "expected string"); }
}

fn walk_manifest_panels(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(panels) = array_at(value, pointer, errors) else { return };
    for (index, panel) in panels.iter().enumerate() {
        let item = format!("{pointer}/{index}");
        let Some(object) = object_at(panel, &item, errors) else { continue };
        check_fields(object, &item, PROVIDER_FIELDS, errors);
    }
}

fn walk_viewport(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink) {
    let Some(values) = array_at(value, pointer, errors) else { return };
    for index in 0..2 {
        match values.get(index).and_then(|value| value.as_f64()) {
            Some(number) if number.is_finite() && number > 0.0 => {}
            Some(_) => errors.push(format!("{pointer}/{index}"), "viewport dimension must be finite and positive"),
            None => errors.push(format!("{pointer}/{index}"), "missing viewport dimension"),
        }
    }
}

fn walk_object_fields(value: &serde_json::Value, pointer: &str, fields: &[&str], errors: &mut ErrorSink) {
    if let Some(object) = object_at(value, pointer, errors) { check_fields(object, pointer, fields, errors); }
}

fn walk_array_items<F>(value: &serde_json::Value, pointer: &str, errors: &mut ErrorSink, walk_item: F)
where
    F: Fn(&serde_json::Value, &str, &mut ErrorSink),
{
    let Some(items) = array_at(value, pointer, errors) else { return };
    for (index, item) in items.iter().enumerate() {
        walk_item(item, &format!("{pointer}/{index}"), errors);
    }
}

fn check_fields(object: &JsonMap, pointer: &str, allowed: &[&str], errors: &mut ErrorSink) {
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            errors.push(format!("{pointer}/{key}"), format!("unknown field; allowed fields: {}", allowed.join(", ")));
        }
    }
    for field in allowed {
        if !object.contains_key(*field) {
            errors.push(format!("{pointer}/{field}"), "missing required field");
        }
    }
}

fn object_at<'a>(value: &'a serde_json::Value, pointer: &str, errors: &mut ErrorSink) -> Option<&'a JsonMap> {
    value.as_object().or_else(|| {
        errors.push(pointer, "expected object");
        None
    })
}

fn array_at<'a>(value: &'a serde_json::Value, pointer: &str, errors: &mut ErrorSink) -> Option<&'a Vec<serde_json::Value>> {
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

const WORKSPACE_FIELDS: &[&str] = &["spec_version", "id", "layout", "surface", "chrome", "input", "glyphs", "theme", "persistence", "panels"];
const LAYOUT_FIELDS: &[&str] = &["units", "viewport", "preferred_mode", "clamp", "tile"];
const TILE_LAYOUT_FIELDS: &[&str] = &["resize", "row_min", "gap", "padding", "fill_viewport"];
const SURFACE_FIELDS: &[&str] = &["compact_max", "tablet_max", "fallback_capabilities", "resize_policy"];
const CHROME_FIELDS: &[&str] = &["metrics", "hit_target_min", "panel_header_h", "panel_frame", "title_in_border", "mode_control", "minimize_control", "maximize_control", "resize_grip", "dock", "dock_label"];
const INPUT_FIELDS: &[&str] = &["steps", "bindings"];
const COMMAND_BINDING_FIELDS: &[&str] = &["chord", "command"];
const PERSISTENCE_FIELDS: &[&str] = &["enabled", "key", "restore", "save_policy"];
const PANEL_FIELDS: &[&str] = &["id", "title", "slug", "window", "content"];
const WINDOW_FIELDS: &[&str] = &["x", "y", "w", "h", "state", "z", "tile_w", "tile_h"];
const MANIFEST_FIELDS: &[&str] = &["backend", "panels"];
const PROVIDER_FIELDS: &[&str] = &["panel_id", "content_kind", "binding"];
const CLAMP_FIELDS: &[&str] = &["outer_w", "outer_h", "floor_w", "floor_h", "inner", "edge", "min_w", "min_h"];
const TILE_METRIC_FIELDS: &[&str] = &["row", "col_floor", "outer"];
const SURFACE_CAP_FIELDS: &[&str] = &["coarse_pointer", "hover", "keyboard"];
const CHROME_METRIC_FIELDS: &[&str] = &["inset", "dock_h"];
const COMMAND_STEP_FIELDS: &[&str] = &["coarse", "fine"];
const KEY_CHORD_FIELDS: &[&str] = &["key", "shift", "alt", "ctrl", "meta"];
const THEME_FIELDS: &[&str] = &["colors", "typography", "density"];
const COLOR_FIELDS: &[&str] = &["bg", "panel", "fg", "dim", "line", "line2", "inverse_bg", "inverse_fg", "accent", "red", "yellow", "green", "blue", "pink", "badge_info", "focus_ring"];
const TYPOGRAPHY_FIELDS: &[&str] = &["family", "body_size", "body_line_height", "label_size", "label_weight", "label_tracking"];
const DENSITY_FIELDS: &[&str] = &["panel_radius", "badge_radius", "spacing_xs", "spacing_sm", "spacing_md"];

const CONTENT_CUSTOM_FIELDS: &[&str] = &["kind", "binding"];
const CONTENT_TEXT_FIELDS: &[&str] = &["kind", "source", "scroll"];
const CONTENT_EDITOR_FIELDS: &[&str] = &["kind", "binding", "multiline", "placeholder"];
const CONTENT_SOURCE_FIELDS: &[&str] = &["kind", "source"];
const CONTENT_TIME_SERIES_FIELDS: &[&str] = &["kind", "source", "unit"];
const CONTENT_SPINNER_FIELDS: &[&str] = &["kind", "label"];
const DATA_SOURCE_INLINE_FIELDS: &[&str] = &["source", "value"];
const DATA_SOURCE_BINDING_FIELDS: &[&str] = &["source", "id"];
const TEXT_MODEL_FIELDS: &[&str] = &["text"];
const BADGE_SPEC_FIELDS: &[&str] = &["kind", "field", "value", "active", "with_x", "with_plus", "small", "override_color", "accent_color", "click_kind", "emit_hover"];
const TABLE_MODEL_FIELDS: &[&str] = &["columns", "rows"];
const TABLE_COLUMN_FIELDS: &[&str] = &["key", "title", "width", "align"];
const TABLE_ROW_FIELDS: &[&str] = &["cells"];
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
