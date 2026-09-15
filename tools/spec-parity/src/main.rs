mod diagnostics;
mod field_coverage;
mod layout_roundtrip;
mod panel_compare;
mod provider_manifest;
mod repository;
mod schema;

use std::env;
use std::error::Error;
use std::fs;
use std::path::Path;

use panel_kit_core::spec::BindingManifest;

use crate::field_coverage::{compare_required_schema_leaves, require_backend_field_dispositions};
use crate::layout_roundtrip::round_trip_saved_layout;
use crate::panel_compare::{
    compare_panel_ids, panel_ids_from_manifest_json, panel_ids_from_saved_layout,
};
use crate::repository::check_repository;
use crate::schema::{check_committed_schema, decode_workspace_spec_text, workspace_spec_schema_json};

const USAGE: &str = "\
usage: spec-parity [check]
       spec-parity export-schema
       spec-parity check-schema <path>
       spec-parity decode <json-or-path>
       spec-parity decode-layout <json-or-path>
       spec-parity compare-panels <layout-json-or-path> <provider-manifest-json-or-path>
       spec-parity check-spec <workspace-spec-json-or-path> <provider-manifest-json-or-path>
       spec-parity check-required-leaves <json-or-path> <pointer>...
       spec-parity check-backend-fields <workspace> <backend> <schema-pointers-csv> <lowered-pointers-csv>";

/// Run the spec-parity command-line entry point.
fn main() {
    if let Err(error) = run() {
        eprint!("{error}");
        std::process::exit(1);
    }
}

/// Dispatch a command and preserve non-zero parity failures as plain diagnostics.
fn run() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "check".to_owned());

    match command.as_str() {
        "check" => check_repository(),
        "export-schema" => {
            print!("{}", workspace_spec_schema_json()?);
            Ok(())
        }
        "check-schema" => {
            let path = args.next().ok_or("check-schema requires a schema path\n")?;
            check_committed_schema(Path::new(&path))
        }
        "decode" => {
            let input = args.next().ok_or("decode requires JSON text or a path\n")?;
            decode_workspace_spec_text(&read_json_argument(&input)?)?;
            Ok(())
        }
        "decode-layout" => {
            let input = args
                .next()
                .ok_or("decode-layout requires JSON text or a path\n")?;
            round_trip_saved_layout(&read_json_argument(&input)?)?;
            Ok(())
        }
        "compare-panels" => {
            let layout = args
                .next()
                .ok_or("compare-panels requires a layout JSON input\n")?;
            let manifest = args
                .next()
                .ok_or("compare-panels requires a provider manifest JSON input\n")?;
            let nix_ids = panel_ids_from_saved_layout(&read_json_argument(&layout)?)?;
            let rust_ids = panel_ids_from_manifest_json(&read_json_argument(&manifest)?)?;

            compare_panel_ids("workspace-canary", &nix_ids, &rust_ids)?;
            Ok(())
        }
        "check-spec" => {
            let spec_input = args
                .next()
                .ok_or("check-spec requires a WorkspaceSpec JSON input\n")?;
            let manifest_input = args
                .next()
                .ok_or("check-spec requires a provider manifest JSON input\n")?;
            let spec = decode_workspace_spec_text(&read_json_argument(&spec_input)?)?;
            let manifest_value = serde_json::from_str(&read_json_argument(&manifest_input)?)?;
            let providers = BindingManifest::from_json_value(manifest_value)?;
            let _resolved = spec.resolve(&providers)?;
            Ok(())
        }
        "check-required-leaves" => {
            let input = args
                .next()
                .ok_or("check-required-leaves requires a JSON input\n")?;
            let pointers: Vec<String> = args.collect();
            let pointer_refs: Vec<&str> = pointers.iter().map(String::as_str).collect();
            let value = serde_json::from_str(&read_json_argument(&input)?)?;

            compare_required_schema_leaves(&value, &pointer_refs)?;
            Ok(())
        }
        "check-backend-fields" => {
            let workspace = args
                .next()
                .ok_or("check-backend-fields requires a workspace name\n")?;
            let backend = args
                .next()
                .ok_or("check-backend-fields requires a backend name\n")?;
            let schema = args
                .next()
                .ok_or("check-backend-fields requires schema pointers\n")?;
            let lowered = args
                .next()
                .ok_or("check-backend-fields requires lowered pointers\n")?;
            let schema_pointers = split_csv_pointers(&schema);
            let lowered_pointers = split_csv_pointers(&lowered);

            require_backend_field_dispositions(
                &workspace,
                &backend,
                &schema_pointers,
                &lowered_pointers,
            )?;
            Ok(())
        }
        _ => Err(format!("unknown command `{command}`\n{USAGE}\n").into()),
    }
}

/// Split comma-separated JSON pointers while keeping empty input as an empty list.
fn split_csv_pointers(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return Vec::new();
    }

    text.split(',').collect()
}


/// Read JSON from stdin, a path, or an inline command argument.
fn read_json_argument(input: &str) -> Result<String, Box<dyn Error>> {
    if input == "-" {
        return Ok(std::io::read_to_string(std::io::stdin())?);
    }

    let path = Path::new(input);
    if path.exists() {
        return Ok(fs::read_to_string(path)?);
    }

    Ok(input.to_owned())
}
